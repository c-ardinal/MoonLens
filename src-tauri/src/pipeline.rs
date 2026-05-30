//! Orchestrates capture -> intent -> provider chain -> storage + UI events.

use std::{
    collections::HashMap,
    path::PathBuf,
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
        asr_whisper::WhisperCppProvider,
        ocr_ocrs::OcrsProvider,
        qr_rqrr::RqrrProvider,
        search_serpapi::BrowserSearchProvider,
        translate_deepl::DeepLTranslator,
        translate_embedded_llm::{EmbeddedLlmTranslator, EmbeddedLoadNotifier},
        translate_gemini::GeminiProvider,
        translate_ollama::OllamaTranslator,
        translate_routed::RoutedTranslator,
        Capability, Intent, OcrLine, ProviderError, ProviderHandle, TaskInput, TaskOutput,
    },
    services::hardware_probe::GPU_FEATURE_ENABLED,
    settings::{
        schema::{FallbackCondition, Providers as ProvidersSettings, TaskChain},
        SettingsStore,
    },
    storage::Storage,
    translation_router::Hardware,
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
    /// Resolved Tauri app-data directory, used by local providers
    /// that need to look up bundled state on disk (currently the
    /// embedded LLM model). Optional so test fixtures that
    /// instantiate a `Pipeline` without an `AppHandle` do not have
    /// to invent a path.
    app_data_dir: RwLock<Option<PathBuf>>,
    /// Side-cache of `Arc<EmbeddedLlmTranslator>` keyed by provider
    /// id. `rebuild_providers` consults this map and reuses the
    /// existing instance when the resolved `model_path` has not
    /// changed — without this, every settings save (or every chunk-
    /// sink swap, since `set_chunk_sink` calls `rebuild_providers`)
    /// would drop the multi-hundred-MB GGUF that was already loaded
    /// into the engine and force a cold reload on the next translate
    /// call. The concrete-typed cache is needed because
    /// `Arc<dyn Provider>` cannot be downcast back to
    /// `EmbeddedLlmTranslator` to compare paths.
    embedded_cache: RwLock<HashMap<String, Arc<EmbeddedLlmTranslator>>>,
    /// Optional callback fired by every embedded provider around its
    /// `Model::build` call. Wired to a Tauri emitter at startup so
    /// the layout shell can show a "ローカル LLM をロード中…" badge
    /// during the multi-second cold-start (and clear it when the
    /// load completes / fails). Unset in headless / CI builds.
    embedded_load_notifier: RwLock<Option<EmbeddedLoadNotifier>>,
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
            app_data_dir: RwLock::new(None),
            embedded_cache: RwLock::new(HashMap::new()),
            embedded_load_notifier: RwLock::new(None),
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
            app_data_dir: RwLock::new(None),
            embedded_cache: RwLock::new(HashMap::new()),
            embedded_load_notifier: RwLock::new(None),
        }
    }

    /// Install a notifier fired by every embedded provider around its
    /// `Model::build` call. Replaces any previously installed
    /// notifier. Drops the cached `Arc<EmbeddedLlmTranslator>`
    /// instances so the next rebuild rewires them with the new
    /// notifier — otherwise the cache would keep handing back
    /// instances bound to the OLD notifier closure forever.
    pub fn set_embedded_load_notifier(&self, notifier: Option<EmbeddedLoadNotifier>) {
        *self.embedded_load_notifier.write() = notifier;
        self.embedded_cache.write().clear();
        self.rebuild_providers();
    }

    /// Wire the resolved Tauri app-data directory in. Called once
    /// during app setup before any provider call so the embedded
    /// LLM path resolves to the real `<app-data>/moonlens/models/`
    /// folder. Re-runs provider discovery so already-instantiated
    /// `EmbeddedLlmTranslator` handles pick up the path.
    pub fn set_app_data_dir(&self, path: PathBuf) {
        *self.app_data_dir.write() = Some(path);
        self.rebuild_providers();
    }

    /// Read-only accessor for the resolved app-data directory. Used
    /// by the model-download IPC to know where to write the GGUF
    /// without re-deriving the path.
    pub fn app_data_dir(&self) -> Option<PathBuf> {
        self.app_data_dir.read().clone()
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

    /// Resolve (or reuse) an `EmbeddedLlmTranslator` for the given
    /// id + config. Reuses the cached instance when both the
    /// resolved `model_path` AND the requested `gpu_layers` match
    /// what we already loaded, so a settings save that changes
    /// neither (the common case — flipping a UI overlay opacity
    /// shouldn't reload a 1 GB GGUF) is free, while a real GPU /
    /// model swap evicts the stale entry.
    ///
    /// Pre-Phase-27 bug: the cache only compared `model_path`. A
    /// user flipping `performance.use_gpu_if_available` ON kept
    /// the existing CPU-loaded translator forever because the
    /// path was unchanged; the new `gpu_layers` value was
    /// silently discarded. Including `gpu_layers` in the
    /// comparison key closes that gap.
    fn embedded_handle(
        &self,
        id: &str,
        cfg: &serde_json::Value,
        app_data: Option<&PathBuf>,
    ) -> ProviderHandle {
        // Build the would-be new instance first so we can compare
        // paths without holding the cache lock during construction.
        let mut candidate = EmbeddedLlmTranslator::new(id, cfg);
        if let Some(dir) = app_data {
            candidate = candidate.with_app_data_dir(dir);
        }
        candidate = candidate.with_gpu_layers(self.resolved_gpu_layers());
        if let Some(notifier) = self.embedded_load_notifier.read().as_ref() {
            candidate = candidate.with_load_notifier(notifier.clone());
        }
        let want_path = candidate.model_path().to_path_buf();
        let want_gpu_layers = candidate.gpu_layers();

        let mut cache = self.embedded_cache.write();
        if let Some(existing) = cache.get(id) {
            if existing.model_path() == want_path && existing.gpu_layers() == want_gpu_layers {
                tracing::debug!(
                    "pipeline: reusing loaded embedded provider `{id}` \
                     (path + gpu_layers unchanged)"
                );
                return existing.clone();
            }
            tracing::info!(
                "pipeline: embedded provider `{id}` config changed \
                 (path: {} → {}, gpu_layers: {} → {}); reloading",
                existing.model_path().display(),
                want_path.display(),
                existing.gpu_layers(),
                want_gpu_layers,
            );
        }
        let arc = Arc::new(candidate);
        cache.insert(id.to_string(), arc.clone());
        arc
    }

    /// Translate the user's `performance.use_gpu_if_available`
    /// toggle into the `n_gpu_layers` value the llama.cpp runtime
    /// expects. Two-step gate:
    ///
    ///   - The toggle has to be ON in user settings.
    ///   - The binary has to be built with at least one GPU
    ///     backend feature (`embedded-llm-vulkan` /
    ///     `embedded-llm-cuda` / `embedded-llm-metal`). Flipping
    ///     the toggle on a feature-disabled build is a no-op
    ///     because the runtime has no GPU path to dispatch to.
    ///
    /// When both gates pass we return `99` ("offload all layers")
    /// rather than auto-detecting the model's layer count — the
    /// upstream runtime is documented to cap the value at the
    /// real layer count, so passing 99 is the canonical "as much
    /// GPU as possible" knob. A future enhancement could pick a
    /// smaller value based on probed VRAM, but the binary
    /// currently has no VRAM probe.
    fn resolved_gpu_layers(&self) -> u32 {
        let snap = self.settings.snapshot();
        if snap.performance.use_gpu_if_available && GPU_FEATURE_ENABLED {
            99
        } else {
            0
        }
    }

    /// Build a `RoutedTranslator` for `id == "embedded.llm"` when
    /// the user has opted into a routing tier. Returns `None` when
    /// `user_tier` is unset (legacy single-model surface stays in
    /// effect) or when the wrapper cannot be constructed.
    ///
    /// The wrapper is rebuilt fresh on every `rebuild_providers`
    /// call instead of going through `embedded_cache` because the
    /// cache compares model_path against a single GGUF; a routed
    /// wrapper owns 1-2 GGUFs and would not fit that comparison.
    /// The cost is small — `RoutedTranslator::new` only allocates
    /// thin wrappers around `EmbeddedLlmTranslator`, and each of
    /// those workers lazy-loads its GGUF on first inference.
    fn routed_handle(
        &self,
        id: &str,
        providers: &ProvidersSettings,
        app_data: Option<&PathBuf>,
    ) -> Option<ProviderHandle> {
        let user_tier = providers.user_tier?;
        let hardware = if GPU_FEATURE_ENABLED {
            Hardware::Gpu
        } else {
            Hardware::Cpu
        };
        let notifier = self.embedded_load_notifier.read().clone();
        let gpu_layers = self.resolved_gpu_layers();
        let rt = RoutedTranslator::new(id, user_tier, hardware, gpu_layers, app_data, notifier);
        tracing::info!(
            "pipeline: registered RoutedTranslator for user_tier={:?} hw={:?} \
             gpu_layers={gpu_layers}",
            user_tier,
            hardware
        );
        Some(Arc::new(rt))
    }

    /// Rebuild the in-memory provider map from the current settings. Called
    /// on startup and whenever settings hot-reload.
    pub fn rebuild_providers(&self) {
        let s = self.settings.snapshot();
        let mut map: HashMap<String, ProviderHandle> = HashMap::new();
        let empty = serde_json::Value::Object(Default::default());
        let sink = self.chunk_sink.read().clone();
        let app_data = self.app_data_dir.read().clone();
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
                id if id == "embedded.llm" || id.starts_with("embedded.") => {
                    // Routing-aware path: when the user has opted
                    // into a translation tier, the `embedded.llm`
                    // slot is filled by a `RoutedTranslator` that
                    // owns one model per tier_id the matrix can
                    // dispatch to, and a per-call `route()` call
                    // selects between them. This only fires for
                    // the canonical `embedded.llm` id — auxiliary
                    // ids (e.g. `embedded.llm.bench`) stay on the
                    // legacy single-tier path because they target
                    // a specific model, not a routing tier.
                    if id == "embedded.llm" {
                        if let Some(routed) =
                            self.routed_handle(id, &s.providers, app_data.as_ref())
                        {
                            Some(routed)
                        } else {
                            Some(self.embedded_handle(id, &cfg, app_data.as_ref()))
                        }
                    } else {
                        Some(self.embedded_handle(id, &cfg, app_data.as_ref()))
                    }
                }
                "deepl" => Some(Arc::new(DeepLTranslator::new(id, &cfg))),
                "gemini" => Some(Arc::new(GeminiProvider::new(id, &cfg))),
                id if id.starts_with("whisper_cpp.") => {
                    Some(Arc::new(WhisperCppProvider::new(id, &cfg)))
                }
                "ocrs" => Some(Arc::new(OcrsProvider::new(id, &cfg))),
                #[cfg(feature = "paddleocr-engine")]
                "paddle" => Some(Arc::new(
                    crate::providers::ocr_paddle::OarOcrProvider::from_config(
                        id,
                        &cfg,
                        s.performance.use_gpu_if_available,
                    ),
                )),
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
        // 2026-05-28: the `windows` OCR auto-register was removed when
        // OCR consolidated to the single `paddle` (PP-OCRv5) engine.
        // Auto-register the Phase 28 PP-OCRv5
        // (`paddle`) provider. Users upgrading from pre-Phase-28
        // installs may still have a chain that lacks the
        // `[providers.configs.paddle]` block — without this fallback
        // the chain's "paddle" id would resolve to "not registered"
        // and we would fall through to ocrs, defeating the whole
        // migration.
        #[cfg(feature = "paddleocr-engine")]
        if !map.contains_key("paddle") {
            let default_cfg = serde_json::json!({});
            map.insert(
                "paddle".into(),
                Arc::new(crate::providers::ocr_paddle::OarOcrProvider::from_config(
                    "paddle",
                    &default_cfg,
                    s.performance.use_gpu_if_available,
                )),
            );
            tracing::info!(
                "pipeline: auto-registered `paddle` OCR provider (user.toml lacked the section)"
            );
        }
        // Ensure any id referenced in a chain without a configs entry still
        // has a sane default (e.g. `ollama.gemma4` or `embedded.llm` with
        // no explicit `[providers.configs."<id>"]` block in user.toml).
        for id in s.providers.translate.chain.iter().chain(
            s.providers
                .explain
                .chain
                .iter()
                .chain(s.providers.summarize.chain.iter()),
        ) {
            if map.contains_key(id) {
                continue;
            }
            if id.starts_with("ollama.") {
                let mut p = OllamaTranslator::new(id.as_str(), &empty);
                if let Some(sink) = &sink {
                    p = p.with_chunk_sink(sink.clone());
                }
                map.insert(id.clone(), Arc::new(p));
            } else if id == "embedded.llm" || id.starts_with("embedded.") {
                let handle = self.embedded_handle(id, &empty, app_data.as_ref());
                map.insert(id.clone(), handle);
            }
        }
        // Drop cached embedded engines whose ids are no longer in the
        // resolved provider map. Keeping them around would only waste
        // RAM since nothing can reach them through `Pipeline::run` /
        // `run_with_sink` once they leave the map.
        {
            let mut cache = self.embedded_cache.write();
            cache.retain(|id, _| map.contains_key(id));
        }
        *self.providers.write() = map;
    }

    /// Run a task through the configured chain, respecting privacy gates,
    /// timeouts, fallback conditions, and session stickiness.
    /// Extract a per-request glossary from a batch of source lines
    /// by walking the translate chain in priority order and asking
    /// each provider via [`Provider::extract_glossary`]. Returns the
    /// first non-empty `Some(entries)` reply; falls back to `Vec::new()`
    /// when no provider can help (cloud APIs, missing models). Errors
    /// from individual providers are swallowed into a debug log so a
    /// glossary failure never blocks the per-line translation path
    /// that follows.
    ///
    /// This is the entry point the lens pipeline calls before kicking
    /// off the per-line translate spawn loop (Phase 29).
    pub async fn extract_glossary(
        &self,
        lines: &[&str],
        source_language: &str,
        target_language: &str,
    ) -> Vec<(String, String)> {
        if lines.is_empty() {
            return Vec::new();
        }
        let s = self.settings.snapshot();
        let chain = s.providers.translate.chain.clone();
        let providers = self.providers.read().clone();
        for id in chain {
            let Some(p) = providers.get(&id) else {
                continue;
            };
            match p
                .extract_glossary(lines, source_language, target_language)
                .await
            {
                Ok(Some(entries)) => {
                    tracing::debug!(
                        "glossary: {} entries from {id} ({} lines, {source_language}->{target_language})",
                        entries.len(),
                        lines.len()
                    );
                    return entries;
                }
                Ok(None) => continue,
                Err(e) => {
                    tracing::debug!("glossary: {id} failed: {e}");
                    continue;
                }
            }
        }
        Vec::new()
    }

    pub async fn run(
        &self,
        capability: Capability,
        input: TaskInput,
    ) -> Result<TaskResult, ProviderError> {
        self.run_inner(capability, input, None).await
    }

    /// Streaming variant of [`Self::run`]. The supplied `sink` is
    /// invoked once per token delta the picked provider emits; the
    /// final aggregated text is still returned in `TaskResult.output`
    /// so non-streaming consumers can use the same call. Lens routes
    /// per-line sinks through this entry point so the chain choice
    /// (embedded LLM, Ollama, plugin, cloud) is invisible to the
    /// streaming UI.
    pub async fn run_with_sink(
        &self,
        capability: Capability,
        input: TaskInput,
        sink: ChunkSink,
    ) -> Result<TaskResult, ProviderError> {
        self.run_inner(capability, input, Some(sink)).await
    }

    async fn run_inner(
        &self,
        capability: Capability,
        input: TaskInput,
        sink: Option<ChunkSink>,
    ) -> Result<TaskResult, ProviderError> {
        let s = self.settings.snapshot();
        let chain = pick_chain(&s, capability);
        let allow_cloud_global = s.privacy.allow_cloud_globally;
        let providers = self.providers.read().clone();
        let mut reasons = Vec::new();
        let started = Instant::now();

        // 2026-05-28: OCR consolidated to the single `paddle`
        // (PP-OCRv5) engine. The previous Windows-only auto-migration
        // here force-prepended `windows` to every OCR chain — which
        // is exactly why the bench-recommended PaddleOCR never ran in
        // production: WinRT always sat first and always "succeeded"
        // (it's the OS engine, needs no models), so the chain never
        // fell through to `paddle`. Removing the force-prepend lets
        // the configured chain (`["paddle"]`) actually take effect.
        let effective_chain: Vec<String> = chain.chain.clone();

        let ordered = apply_stickiness(&self.sticky, capability, &effective_chain);
        tracing::debug!(
            "pipeline: {capability:?} chain ordered {:?} (registered: {:?})",
            ordered,
            providers.keys().collect::<Vec<_>>()
        );

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
            // Dispatch through `run_streaming` when the caller wants
            // per-token deltas. Providers that do not natively stream
            // fall through the trait's default impl, which just emits
            // the final Text as a single `done=true` chunk so the UI
            // exits its streaming state cleanly.
            let res = match &sink {
                Some(s) => timeout(to, prov.run_streaming(input.clone(), s.clone())).await,
                None => timeout(to, prov.run(input.clone())).await,
            };
            match res {
                Ok(Ok(out)) => {
                    self.remember_sticky(capability, &id, chain.sticky_session_ms);
                    tracing::info!(
                        "pipeline: {capability:?} served by `{id}` in {}ms (skipped: {reasons:?})",
                        started.elapsed().as_millis()
                    );
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
                    // Capture flow honours the GLOBAL translation pair
                    // (`translation.default_*`) — `subtitle.*` is reserved
                    // for the audio transcription path so users can run
                    // different pairs side-by-side (screen EN→JA,
                    // audio ZH→EN, etc.). Source defaults to `auto`
                    // unless the user explicitly pins a script.
                    source_language: if s.translation.default_source_language.trim().is_empty() {
                        "auto".into()
                    } else {
                        s.translation.default_source_language.clone()
                    },
                    target_language: s.translation.default_target_language.clone(),
                    glossary: Vec::new(),
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

    /// Public history-recording entry point for callers outside the
    /// capture flow (subtitle service, lens watcher). Kind values are
    /// the same string tags the history list filter consumes:
    /// `"translate"`, `"qr"`, `"audio"`, `"lens"`.
    pub fn record_external_history(
        &self,
        kind: &str,
        source_text: Option<&str>,
        result_text: Option<&str>,
        provider: Option<&str>,
        via_cloud: bool,
    ) {
        self.record_history(kind, source_text, result_text, provider, via_cloud);
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
impl Pipeline {
    /// Read-only accessor for the embedded cache used by
    /// `rebuild_providers` to preserve loaded engines across config
    /// changes. Tests use this to assert Arc identity (via `ptr_eq`)
    /// before and after rebuilds.
    pub fn embedded_cache_for_test(&self) -> HashMap<String, Arc<EmbeddedLlmTranslator>> {
        self.embedded_cache.read().clone()
    }

    /// Test-only handle to the underlying SettingsStore so tests can
    /// mutate settings without going through the Tauri command layer.
    pub fn settings_for_test(&self) -> Arc<SettingsStore> {
        self.settings.clone()
    }
}

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
