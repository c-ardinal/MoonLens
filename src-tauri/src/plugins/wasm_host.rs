//! WebAssembly Component Model plugin host.
//!
//! Each WASM plugin implements the `moonlens:plugin/provider` world defined
//! in `wit/moonlens.wit`. The host builds a sandboxed store with only the
//! capabilities declared in `plugin.toml`, then exposes its exports through
//! the standard [`Provider`] trait so the pipeline is transport-agnostic.
//!
//! Implementation note: the WIT bindings and the full wasmtime `Component`
//! wiring are intentionally left as a focused follow-up. This file
//! establishes the types and the Provider boundary so the rest of the
//! pipeline compiles and so the plugin SDK has a concrete target.

use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::providers::{
    Capability, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
};

use super::manifest::PluginManifest;

pub struct WasmPlugin {
    id: String,
    provides: Vec<String>,
    #[allow(dead_code)]
    component_bytes: Vec<u8>,
}

impl WasmPlugin {
    pub fn load(manifest: &PluginManifest, dir: &Path) -> Result<Self> {
        let entry = dir.join(&manifest.plugin.entry);
        let component_bytes = std::fs::read(&entry)
            .with_context(|| format!("read wasm component at {}", entry.display()))?;
        Ok(Self {
            id: manifest.plugin.id.clone(),
            provides: manifest.capabilities.provides.clone(),
            component_bytes,
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
                    plugin_id: self.id.clone(),
                    capability: cap,
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

struct WasmProvider {
    id: String,
    #[allow(dead_code)]
    plugin_id: String,
    capability: Capability,
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
        // WASM plugins run locally; network access (if any) is gated by the
        // manifest `permissions.network` allowlist.
        true
    }

    async fn run(&self, _input: TaskInput) -> Result<TaskOutput, ProviderError> {
        // TODO: instantiate wasmtime::component::Component and call the
        // matching export from wit/moonlens.wit. Kept as a stub so the
        // pipeline can still enumerate the provider and the SDK has a
        // concrete symbol to target.
        Err(ProviderError::Other(
            "wasm plugin runtime not yet wired up".into(),
        ))
    }
}
