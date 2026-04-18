//! WebAssembly Component Model plugin host.
//!
//! Each WASM plugin implements the `moonlens:plugin/provider` world defined
//! in `wit/moonlens.wit`. The host instantiates the component once at load
//! time, configures a sandbox via WASI preview2 with permissions filtered
//! per the manifest, then calls `provider.run` from the [`Provider`]
//! trait. From the pipeline's perspective WASM and sidecar plugins are
//! interchangeable.
//!
//! Bindings are generated at compile time with `wasmtime::component::bindgen!`
//! against the SDK WIT, which is kept in lockstep with the host WIT.

use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use parking_lot::Mutex;
use tokio::task;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

use crate::providers::{
    Capability, OcrLine, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
    TranscriptSegment,
};

use super::manifest::PluginManifest;

wasmtime::component::bindgen!({
    world: "plugin",
    path: "src/plugins/wit/moonlens.wit",
    async: false,
});

use moonlens::plugin::types as wit_types;

struct State {
    table: ResourceTable,
    wasi: WasiCtx,
}

impl WasiView for State {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi
    }
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

pub struct WasmPlugin {
    id: String,
    provides: Vec<String>,
    instance: Arc<Mutex<Option<LoadedInstance>>>,
    component_bytes: Vec<u8>,
    network_allow: Vec<String>,
}

struct LoadedInstance {
    store: Store<State>,
    bindings: Plugin,
}

impl WasmPlugin {
    pub fn load(manifest: &PluginManifest, dir: &Path) -> Result<Self> {
        let entry = dir.join(&manifest.plugin.entry);
        let component_bytes = std::fs::read(&entry)
            .with_context(|| format!("read wasm component at {}", entry.display()))?;

        // Sanity-check that the component instantiates eagerly so we can
        // surface load-time errors. We then drop the instance and rebuild
        // it on first call to keep memory usage low until the plugin is
        // actually invoked.
        let _ = build_instance(&component_bytes, &manifest.permissions.network)?;

        Ok(Self {
            id: manifest.plugin.id.clone(),
            provides: manifest.capabilities.provides.clone(),
            instance: Arc::new(Mutex::new(None)),
            component_bytes,
            network_allow: manifest.permissions.network.clone(),
        })
    }

    pub fn provider_handles(&self, plugin_id: &str) -> Vec<(String, ProviderHandle)> {
        self.provides
            .iter()
            .filter_map(|cap_str| parse_cap(cap_str).map(|c| (cap_str.clone(), c)))
            .map(|(cap_str, cap)| {
                let chain_id = format!("plugin.{plugin_id}.{cap_str}");
                let handle: ProviderHandle = Arc::new(WasmProvider {
                    id: chain_id.clone(),
                    capability: cap,
                    component: self.component_bytes.clone(),
                    instance: self.instance.clone(),
                    network_allow: self.network_allow.clone(),
                });
                (chain_id, handle)
            })
            .collect()
    }
}

fn parse_cap(s: &str) -> Option<Capability> {
    Some(match s {
        "translator" => Capability::Translate,
        "ocr" => Capability::Ocr,
        "asr" => Capability::Asr,
        "explain" => Capability::Explain,
        "summarize" => Capability::Summarize,
        "reverse_image_search" => Capability::ReverseImageSearch,
        "qr_handler" => Capability::QrHandler,
        "intent_classifier" => Capability::IntentClassifier,
        _ => return None,
    })
}

fn engine() -> Result<Engine> {
    let mut cfg = Config::new();
    cfg.wasm_component_model(true);
    cfg.consume_fuel(false);
    Engine::new(&cfg)
}

fn build_instance(bytes: &[u8], _network_allow: &[String]) -> Result<LoadedInstance> {
    let engine = engine()?;
    let component = Component::from_binary(&engine, bytes)?;

    let mut linker = Linker::<State>::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker).context("link wasi to linker")?;

    let mut wasi_builder = WasiCtxBuilder::new();
    // No filesystem, no env, no inherited stdio. Network capability is
    // handled by WASI sockets in p3; for now we keep WASM plugins offline
    // and expose `_network_allow` as a future hook.
    let wasi = wasi_builder.build();
    let state = State {
        table: ResourceTable::new(),
        wasi,
    };

    let mut store = Store::new(&engine, state);
    let bindings = Plugin::instantiate(&mut store, &component, &linker)
        .context("instantiate plugin component")?;
    Ok(LoadedInstance { store, bindings })
}

struct WasmProvider {
    id: String,
    capability: Capability,
    component: Vec<u8>,
    instance: Arc<Mutex<Option<LoadedInstance>>>,
    network_allow: Vec<String>,
}

#[async_trait]
impl Provider for WasmProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        std::slice::from_ref(&self.capability)
    }
    fn is_local(&self) -> bool {
        // Network access requires p3 wasi sockets which we have not wired
        // yet; while that is the case all WASM plugins are local-only.
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let component = self.component.clone();
        let net = self.network_allow.clone();
        let cell = self.instance.clone();
        // wasmtime is synchronous in the configuration we use; offload to
        // the blocking pool so the async runtime stays responsive.
        let out: Result<TaskOutput, ProviderError> = task::spawn_blocking(move || {
            let mut guard = cell.lock();
            if guard.is_none() {
                *guard = Some(build_instance(&component, &net).map_err(provider_other)?);
            }
            let inst = guard.as_mut().unwrap();
            let wit_input = to_wit_input(input);
            let res = inst
                .bindings
                .moonlens_plugin_provider()
                .call_run(&mut inst.store, &wit_input)
                .map_err(provider_other)?;
            match res {
                Ok(out) => Ok(from_wit_output(out)),
                Err(err) => Err(map_wit_error(err)),
            }
        })
        .await
        .map_err(provider_other_join)?;
        out
    }
}

fn provider_other<E: std::fmt::Display>(e: E) -> ProviderError {
    ProviderError::Other(e.to_string())
}

fn provider_other_join(e: tokio::task::JoinError) -> ProviderError {
    ProviderError::Other(e.to_string())
}

fn to_wit_input(input: TaskInput) -> wit_types::TaskInput {
    use wit_types::TaskInput as W;
    match input {
        TaskInput::Translate {
            text,
            source_language,
            target_language,
        } => W::Translate(wit_types::TranslateInput {
            text,
            source_language,
            target_language,
        }),
        TaskInput::Ocr {
            image_png,
            languages,
        } => W::Ocr(wit_types::OcrInput {
            image_png,
            languages,
        }),
        TaskInput::Asr {
            audio_pcm16_mono_16k,
            language_hint,
        } => W::Asr(wit_types::AsrInput {
            audio: audio_pcm16_mono_16k,
            language_hint,
        }),
        TaskInput::Explain { prompt, image_png } => {
            W::Explain(wit_types::ExplainInput { prompt, image_png })
        }
        TaskInput::Summarize { text } => W::Summarize(text),
        TaskInput::ReverseImageSearch { image_url } => W::ReverseImageSearch(image_url),
        TaskInput::QrHandler { payload } => W::QrHandler(payload),
        TaskInput::IntentClassifier { image_png } => W::IntentClassifier(image_png),
    }
}

fn from_wit_output(out: wit_types::TaskOutput) -> TaskOutput {
    use wit_types::TaskOutput as W;
    match out {
        W::Text(t) => TaskOutput::Text(t),
        W::TextChunk(c) => TaskOutput::TextChunk {
            delta: c.delta,
            done: c.done,
        },
        W::Ocr(lines) => TaskOutput::Ocr {
            lines: lines
                .into_iter()
                .map(|l| OcrLine {
                    text: l.text,
                    bbox: [l.bbox.0, l.bbox.1, l.bbox.2, l.bbox.3],
                    confidence: l.confidence,
                })
                .collect(),
        },
        W::Transcript(segments) => TaskOutput::Transcript {
            segments: segments
                .into_iter()
                .map(|s| TranscriptSegment {
                    start_s: s.start_s,
                    end_s: s.end_s,
                    text: s.text,
                })
                .collect(),
        },
        W::OpenedExternalUrl(u) => TaskOutput::OpenedExternalUrl(u),
        W::Intent(_) => TaskOutput::Intent(crate::providers::Intent::Unknown),
        W::Empty => TaskOutput::Empty,
    }
}

fn map_wit_error(err: wit_types::ProviderError) -> ProviderError {
    use wit_types::ProviderError as W;
    match err {
        W::Timeout(ms) => ProviderError::Timeout(ms),
        W::Offline => ProviderError::Offline,
        W::RateLimit => ProviderError::RateLimit,
        W::UnsupportedLanguage((a, b)) => ProviderError::UnsupportedLanguage(a, b),
        W::LowConfidence => ProviderError::LowConfidence,
        W::Other(s) => ProviderError::Other(s),
    }
}
