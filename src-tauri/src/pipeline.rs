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
    plugins::PluginRegistry,
    providers::{
        asr_whisper::WhisperCppProvider, ocr_ocrs::OcrsProvider, qr_rqrr::RqrrProvider,
        search_serpapi::BrowserSearchProvider, translate_deepl::DeepLTranslator,
        translate_gemini::GeminiProvider, translate_ollama::OllamaTranslator, Capability,
        Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
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

pub struct Pipeline {
    settings: Arc<SettingsStore>,
    #[allow(dead_code)]
    storage: Arc<Storage>,
    plugins: Arc<PluginRegistry>,
    providers: RwLock<HashMap<String, ProviderHandle>>,
    sticky: RwLock<HashMap<String, (String, Instant)>>,
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
            plugins,
            providers: RwLock::new(HashMap::new()),
            sticky: RwLock::new(HashMap::new()),
        };
        me.rebuild_providers();
        me
    }

    /// Rebuild the in-memory provider map from the current settings. Called
    /// on startup and whenever settings hot-reload.
    pub fn rebuild_providers(&self) {
        let s = self.settings.snapshot();
        let mut map: HashMap<String, ProviderHandle> = HashMap::new();
        let empty = toml::Value::Table(Default::default());
        for (id, cfg) in &s.providers.configs {
            let cfg = cfg.clone();
            let handle: Option<ProviderHandle> = match id.as_str() {
                id if id.starts_with("ollama.") => {
                    Some(Arc::new(OllamaTranslator::new(id, &cfg)))
                }
                "deepl" => Some(Arc::new(DeepLTranslator::new(id, &cfg))),
                "gemini" => Some(Arc::new(GeminiProvider::new(id, &cfg))),
                id if id.starts_with("whisper_cpp.") => {
                    Some(Arc::new(WhisperCppProvider::new(id, &cfg)))
                }
                "ocrs" => Some(Arc::new(OcrsProvider::new(id, &cfg))),
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
        // Plugin providers register themselves through the registry.
        for (id, handle) in self.plugins.providers() {
            map.insert(id, handle);
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
                map.insert(
                    id.clone(),
                    Arc::new(OllamaTranslator::new(id.as_str(), &empty)),
                );
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
        self.sticky
            .write()
            .insert(key, (id.to_string(), Instant::now() + Duration::from_millis(ms as u64)));
    }
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

fn should_fallback(conds: &[FallbackCondition], e: &ProviderError) -> bool {
    conds.iter().any(|c| matches!((c, e),
        (FallbackCondition::Timeout, ProviderError::Timeout(_))
        | (FallbackCondition::Error, ProviderError::Other(_))
        | (FallbackCondition::RateLimit, ProviderError::RateLimit)
        | (FallbackCondition::Offline, ProviderError::Offline)
        | (FallbackCondition::UnsupportedLanguage, ProviderError::UnsupportedLanguage(_, _))
        | (FallbackCondition::LowConfidence, ProviderError::LowConfidence)
    ))
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
