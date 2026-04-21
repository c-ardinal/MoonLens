//! Orchestrates capture -> intent -> provider chain -> storage + UI events.

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use parking_lot::RwLock;
use serde::Serialize;
use tokio::time::timeout;

use crate::{
    intent,
    plugins::PluginRegistry,
    providers::{
        asr_whisper::WhisperCppProvider, ocr_ocrs::OcrsProvider, qr_rqrr::RqrrProvider,
        search_serpapi::BrowserSearchProvider, translate_deepl::DeepLTranslator,
        translate_gemini::GeminiProvider, translate_ollama::OllamaTranslator, Capability, Intent,
        OcrLine, ProviderError, ProviderHandle, TaskInput, TaskOutput,
    },
    settings::{
        schema::{FallbackCondition, TaskChain},
        SettingsStore,
    },
    storage::Storage,
};

/// Outcome reported to the UI per task attempt. `used_provider`/`via_cloud`
/// power the privacy indicator and chain-visualization in the settings UI.
#[derive(Debug, Clone, Serialize)]
pub struct TaskResult {
    pub task: String,
    pub output: TaskOutput,
    pub used_provider: String,
    pub via_cloud: bool,
    pub latency_ms: u64,
    pub fallback_reasons: Vec<String>,
}

/// Callback signature used by streaming translators to publish each
/// token delta. Kept behind a type alias so `Pipeline` does not trip
/// the `clippy::type_complexity` lint every time it appears.
pub type ChunkSink = Arc<dyn Fn(String, bool) + Send + Sync>;

pub struct Pipeline {
    settings: Arc<SettingsStore>,
    storage: Arc<Storage>,
    plugins: Option<Arc<PluginRegistry>>,
    providers: RwLock<HashMap<String, ProviderHandle>>,
    sticky: RwLock<HashMap<String, (String, Instant)>>,
    /// Optional callback fired whenever a streaming translator emits a
    /// chunk. Used by the Tauri wiring to forward chunks to the
    /// overlay webview; unset in test builds that construct a pipeline
    /// directly.
    chunk_sink: RwLock<Option<ChunkSink>>,
}

impl Pipeline {
    pub fn new(
        settings: Arc<SettingsStore>,
        storage: Arc<Storage>,
        plugins: Arc<PluginRegistry>,
    ) -> Self {
        let me = Self {
            settings,
            storage,
            plugins: Some(plugins),
            providers: RwLock::new(HashMap::new()),
            sticky: RwLock::new(HashMap::new()),
            chunk_sink: RwLock::new(None),
        };
        me.rebuild_providers();
        me
    }

    /// Plugin-free constructor used by integration tests and any future
    /// headless entry point. Caller supplies an explicit provider map;
    /// no discovery or built-in lookup happens.
    pub fn with_providers(
        settings: Arc<SettingsStore>,
        storage: Arc<Storage>,
        providers: HashMap<String, ProviderHandle>,
    ) -> Self {
        Self {
            settings,
            storage,
            plugins: None,
            providers: RwLock::new(providers),
            sticky: RwLock::new(HashMap::new()),
            chunk_sink: RwLock::new(None),
        }
    }

    /// Install a callback that every Ollama-style streaming provider
    /// will invoke per chunk. Re-runs provider discovery so existing
    /// provider instances see the new sink.
    pub fn set_chunk_sink(&self, sink: Option<ChunkSink>) {
        *self.chunk_sink.write() = sink;
        self.rebuild_providers();
    }

    /// Fire-and-forget preload hook. Walks the current provider map and
    /// invokes each provider's `preload` — which is a noop for most of
    /// them but triggers the ocrs model load so the first lens capture
    /// does not pay the multi-second cold-start cost inline. Errors are
    /// logged and swallowed; a provider that cannot preload will still
    /// be tried on its first real call.
    pub async fn preload_all(&self) {
        let providers = self.providers.read().clone();
        for (id, handle) in providers.iter() {
            if let Err(e) = handle.preload().await {
                tracing::debug!("provider {id} preload skipped: {e}");
            }
        }
    }

    /// Rebuild the in-memory provider map from the current settings. Called
    /// on startup and whenever settings hot-reload.
    pub fn rebuild_providers(&self) {
        let s = self.settings.snapshot();
        let mut map: HashMap<String, ProviderHandle> = HashMap::new();
        let empty = serde_json::Value::Object(Default::default());
        let sink = self.chunk_sink.read().clone();
        for (id, cfg) in &s.providers.configs {
            let cfg = cfg.clone();
            let handle: Option<ProviderHandle> = match id.as_str() {
                id if id.starts_with("ollama.") => {
                    let mut p = OllamaTranslator::new(id, &cfg);
                    if let Some(sink) = &sink {
                        p = p.with_chunk_sink(sink.clone());
                    }
                    Some(Arc::new(p))
                }
                "deepl" => Some(Arc::new(DeepLTranslator::new(id, &cfg))),
                "gemini" => Some(Arc::new(GeminiProvider::new(id, &cfg))),
                id if id.starts_with("whisper_cpp.") => {
                    Some(Arc::new(WhisperCppProvider::new(id, &cfg)))
                }
                "ocrs" => Some(Arc::new(OcrsProvider::new(id, &cfg))),
                #[cfg(target_os = "windows")]
                "windows" => Some(Arc::new(
                    crate::providers::ocr_windows::WindowsOcrProvider::new(id, &cfg),
                )),
                "rqrr" => Some(Arc::new(RqrrProvider::new(id, &cfg))),
                id if id.starts_with("browser.") => {
                    Some(Arc::new(BrowserSearchProvider::new(id, &cfg)))
                }
                _ => None,
            };
            if let Some(h) = handle {
                map.insert(id.clone(), h);
            }
        }
        // Plugin providers register themselves through the registry. The
        // registry is optional so the pipeline can be constructed headless
        // in tests or future non-Tauri entry points.
        if let Some(plugins) = &self.plugins {
            for (id, handle) in plugins.providers() {
                map.insert(id, handle);
            }
        }
        // Ensure any id referenced in a chain without a configs entry still
        // has a sane default (e.g. `ollama.gemma4` without explicit config).
        for id in s.providers.translate.chain.iter().chain(
            s.providers
                .explain
                .chain
                .iter()
                .chain(s.providers.summarize.chain.iter()),
        ) {
            if !map.contains_key(id) && id.starts_with("ollama.") {
                let mut p = OllamaTranslator::new(id.as_str(), &empty);
                if let Some(sink) = &sink {
                    p = p.with_chunk_sink(sink.clone());
                }
                map.insert(id.clone(), Arc::new(p));
            }
        }
        *self.providers.write() = map;
    }

    /// Run a task through the configured chain, respecting privacy gates,
    /// timeouts, fallback conditions, and session stickiness.
    pub async fn run(
        &self,
        capability: Capability,
        input: TaskInput,
    ) -> Result<TaskResult, ProviderError> {
        let s = self.settings.snapshot();
        let chain = pick_chain(&s, capability);
        let allow_cloud_global = s.privacy.allow_cloud_globally;
        let providers = self.providers.read().clone();
        let mut reasons = Vec::new();
        let started = Instant::now();

        let ordered = apply_stickiness(&self.sticky, capability, &chain.chain);

        for id in ordered {
            let Some(prov) = providers.get(&id).cloned() else {
                reasons.push(format!("{id}: not registered"));
                continue;
            };
            if !prov.is_local() {
                if !chain.allow_cloud || !allow_cloud_global {
                    reasons.push(format!("{id}: cloud disabled"));
                    continue;
                }
                if !chain.allow_network {
                    reasons.push(format!("{id}: network disabled"));
                    continue;
                }
            }
            let to = Duration::from_millis(chain.timeout_ms as u64);
            let res = timeout(to, prov.run(input.clone())).await;
            match res {
                Ok(Ok(out)) => {
                    self.remember_sticky(capability, &id, chain.sticky_session_ms);
                    return Ok(TaskResult {
                        task: format!("{capability:?}").to_lowercase(),
                        output: out,
                        used_provider: id,
                        via_cloud: !prov.is_local(),
                        latency_ms: started.elapsed().as_millis() as u64,
                        fallback_reasons: reasons,
                    });
                }
                Ok(Err(e)) => {
                    if !should_fallback(&chain.fallback_on, &e) {
                        return Err(e);
                    }
                    reasons.push(format!("{id}: {e}"));
                }
                Err(_) => {
                    if !chain
                        .fallback_on
                        .iter()
                        .any(|c| matches!(c, FallbackCondition::Timeout))
                    {
                        return Err(ProviderError::Timeout(chain.timeout_ms));
                    }
                    reasons.push(format!("{id}: timeout"));
                }
            }
        }
        Err(ProviderError::Other(format!(
            "all providers failed: {}",
            reasons.join("; ")
        )))
    }

    fn remember_sticky(&self, cap: Capability, id: &str, ms: u32) {
        if ms == 0 {
            return;
        }
        let key = format!("{cap:?}");
        self.sticky.write().insert(
            key,
            (
                id.to_string(),
                Instant::now() + Duration::from_millis(ms as u64),
            ),
        );
    }

    /// High-level capture flow: given a region-of-interest PNG, classify
    /// content, run the appropriate providers, and return a user-ready
    /// result bundle. Used by the hotkey handler.
    pub async fn run_capture_flow(
        &self,
        image_png: Vec<u8>,
    ) -> Result<CaptureFlowResult, ProviderError> {
        let s = self.settings.snapshot();
        // Cheap heuristic classifier. A plugin-provided classifier can
        // override through the `intent_classifier` chain in future work.
        let img =
            image::load_from_memory(&image_png).map_err(|e| ProviderError::Other(e.to_string()))?;
        let intent = intent::classify(&img);

        // 1) Try QR — decoding is almost free and short-circuits the rest.
        if let Ok(res) = self
            .run(
                Capability::QrHandler,
                TaskInput::IntentClassifier {
                    image_png: image_png.clone(),
                },
            )
            .await
        {
            if let TaskOutput::Text(payload) = &res.output {
                self.record_history(
                    "qr",
                    None,
                    Some(payload),
                    Some(&res.used_provider),
                    res.via_cloud,
                );
                return Ok(CaptureFlowResult {
                    intent: Intent::Qr,
                    source_text: None,
                    result: res,
                    source_rect: None,
                });
            }
        }

        // 2) OCR → translate. Always attempted for text-like or unknown regions.
        let ocr_res = self
            .run(
                Capability::Ocr,
                TaskInput::Ocr {
                    image_png: image_png.clone(),
                    languages: vec![],
                },
            )
            .await;
        let ocr_text = match ocr_res.as_ref() {
            Ok(r) => match &r.output {
                TaskOutput::Ocr { lines } => lines
                    .iter()
                    .map(|l: &OcrLine| l.text.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            },
            Err(_) => String::new(),
        };

        if ocr_text.trim().is_empty() {
            // Nothing text-like detected. Return the intent so the UI can
            // offer actions (reverse image search, explain, ...).
            return Ok(CaptureFlowResult {
                intent,
                source_text: None,
                source_rect: None,
                result: TaskResult {
                    task: "capture".into(),
                    output: TaskOutput::Intent(intent),
                    used_provider: "builtin.heuristic".into(),
                    via_cloud: false,
                    latency_ms: 0,
                    fallback_reasons: vec![],
                },
            });
        }

        let translate = self
            .run(
                Capability::Translate,
                TaskInput::Translate {
                    text: ocr_text.clone(),
                    source_language: "auto".into(),
                    target_language: s.subtitle.target_language.clone(),
                },
            )
            .await?;

        self.record_history(
            "translate",
            Some(&ocr_text),
            match &translate.output {
                TaskOutput::Text(t) => Some(t.as_str()),
                _ => None,
            },
            Some(&translate.used_provider),
            translate.via_cloud,
        );

        Ok(CaptureFlowResult {
            intent: Intent::Text,
            source_text: Some(ocr_text),
            result: translate,
            source_rect: None,
        })
    }

    fn record_history(
        &self,
        kind: &str,
        source_text: Option<&str>,
        result_text: Option<&str>,
        provider: Option<&str>,
        via_cloud: bool,
    ) {
        let s = self.settings.snapshot();
        if !s.storage.keep_history {
            return;
        }
        if let Err(e) =
            self.storage
                .append(kind, source_text, result_text, provider, via_cloud, None)
        {
            tracing::warn!("history append failed: {e}");
        }
    }
}

/// Output of [`Pipeline::run_capture_flow`]. The frontend renders `result`
/// directly; `source_text` is shown above translations for context.
#[derive(Debug, Clone, Serialize)]
pub struct CaptureFlowResult {
    pub intent: Intent,
    pub source_text: Option<String>,
    pub result: TaskResult,
    /// The rect that produced this capture, in monitor-local pixel
    /// coordinates. The overlay uses this to pin the region watcher
    /// onto the same area the user just captured.
    pub source_rect: Option<CaptureFlowRect>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureFlowRect {
    pub monitor_index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

fn pick_chain(s: &crate::settings::schema::Settings, cap: Capability) -> TaskChain {
    match cap {
        Capability::Translate => s.providers.translate.clone(),
        Capability::Ocr => s.providers.ocr.clone(),
        Capability::Asr => s.providers.asr.clone(),
        Capability::Explain => s.providers.explain.clone(),
        Capability::Summarize => s.providers.summarize.clone(),
        Capability::ReverseImageSearch => s.providers.reverse_image_search.clone(),
        Capability::QrHandler => s.providers.qr_handler.clone(),
        Capability::IntentClassifier => s.providers.intent_classifier.clone(),
    }
}

/// Test-friendly alias for [`should_fallback`]. Kept minimal on purpose.
#[cfg(test)]
pub fn should_fallback_for_test(conds: &[FallbackCondition], e: &ProviderError) -> bool {
    should_fallback(conds, e)
}

/// Test-friendly alias for [`apply_stickiness`] that avoids building an
/// `RwLock`; tests pass an owned `HashMap` instead.
#[cfg(test)]
pub fn apply_stickiness_for_test(
    key: &str,
    map: &HashMap<String, (String, Instant)>,
    chain: &[String],
) -> Vec<String> {
    if let Some((preferred, deadline)) = map.get(key) {
        if Instant::now() < *deadline {
            let mut ordered = Vec::with_capacity(chain.len());
            ordered.push(preferred.clone());
            for id in chain {
                if id != preferred {
                    ordered.push(id.clone());
                }
            }
            return ordered;
        }
    }
    chain.to_vec()
}

fn should_fallback(conds: &[FallbackCondition], e: &ProviderError) -> bool {
    conds.iter().any(|c| {
        matches!(
            (c, e),
            (FallbackCondition::Timeout, ProviderError::Timeout(_))
                | (FallbackCondition::Error, ProviderError::Other(_))
                | (FallbackCondition::RateLimit, ProviderError::RateLimit)
                | (FallbackCondition::Offline, ProviderError::Offline)
                | (
                    FallbackCondition::UnsupportedLanguage,
                    ProviderError::UnsupportedLanguage(_, _)
                )
                | (
                    FallbackCondition::LowConfidence,
                    ProviderError::LowConfidence
                )
        )
    })
}

fn apply_stickiness(
    sticky: &RwLock<HashMap<String, (String, Instant)>>,
    cap: Capability,
    chain: &[String],
) -> Vec<String> {
    let key = format!("{cap:?}");
    let guard = sticky.read();
    if let Some((preferred, deadline)) = guard.get(&key) {
        if Instant::now() < *deadline {
            let mut ordered = Vec::with_capacity(chain.len());
            ordered.push(preferred.clone());
            for id in chain {
                if id != preferred {
                    ordered.push(id.clone());
                }
            }
            return ordered;
        }
    }
    chain.to_vec()
}
