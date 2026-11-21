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

use futures::StreamExt;

use crate::{
    intent, lang, layout,
    plugins::PluginRegistry,
    providers::{
        asr_whisper::WhisperCppProvider, glossary::TranslateOptions, ocr_ocrs::OcrsProvider,
        qr_rqrr::RqrrProvider, search_serpapi::BrowserSearchProvider,
        translate_deepl::DeepLTranslator, translate_gemini::GeminiProvider,
        translate_llama::LlamaTranslator, translate_ollama::OllamaTranslator, Capability, Intent,
        OcrLine, ProviderError, ProviderHandle, TaskInput, TaskOutput, TranslateContext,
    },
    settings::{
        schema::{FallbackCondition, TaskChain},
        SettingsStore,
    },
    storage::Storage,
    translate_plan,
    translation_cache::{CachedTranslation, TranslationCache},
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

/// Callback fired before each translation request of a capture; see
/// [`Pipeline::translate_capture_with_progress`].
pub type ProgressSink = dyn Fn(&TranslationProgress) + Send + Sync;

/// Where a capture's translation stands, sent to the selector before
/// each request so it can show finished blocks and stream the next ones
/// into the right places.
#[derive(Debug, Clone, Serialize)]
pub struct TranslationProgress {
    pub blocks: Vec<LensBlock>,
    /// Indices into `blocks` that the next request streams into, in
    /// order.
    pub streaming: Vec<usize>,
}

fn notify(progress: Option<&ProgressSink>, bundle: &CaptureFlowResult, streaming: &[usize]) {
    if let Some(p) = progress {
        p(&TranslationProgress {
            blocks: bundle.blocks.clone(),
            streaming: streaming.to_vec(),
        });
    }
}

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
    /// Per-paragraph translations, so re-selected text shows instantly.
    cache: TranslationCache,
}

/// Time a local translator gets per source character on top of nothing:
/// the chain timeout is the floor. A CPU-only PC generates roughly 50
/// Japanese characters a second with the default 2B model, so a fixed
/// 15 s cut the later paragraphs of a long capture (about 1,800
/// characters) and left them untranslated.
const LOCAL_TRANSLATE_MS_PER_CHAR: u64 = 40;

/// `true` when an OCR chain error only says "no text here": every engine
/// returned a low-confidence (empty) result or is not installed. Timeouts
/// and engine errors are real failures and return `false`.
pub fn ocr_found_nothing(e: &ProviderError) -> bool {
    match e {
        ProviderError::LowConfidence => true,
        ProviderError::Other(msg) => msg.strip_prefix("all providers failed: ").is_some_and(|r| {
            r.split("; ").all(|reason| {
                reason.ends_with(&ProviderError::LowConfidence.to_string())
                    || reason.contains(": not installed")
                    || reason.ends_with(": not registered")
            })
        }),
        _ => false,
    }
}

/// Time local OCR gets per megapixel on top of the chain timeout floor.
/// A full 3840x2160 frame (8.3 MP) on a slower CPU needs more than the
/// default 5 s, and a timeout there used to read as "no text".
const LOCAL_OCR_MS_PER_MEGAPIXEL: u64 = 1500;

/// Width and height from a PNG header, without decoding the image.
fn png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(png[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(png[20..24].try_into().ok()?);
    Some((w, h))
}

/// Timeout for one provider call. Local translation scales with the
/// text length (generation time grows with it), local OCR with the image
/// area; everything else, and any cloud provider, keeps the chain's
/// fixed timeout.
pub fn effective_timeout(chain_ms: u32, input: &TaskInput, is_local: bool) -> Duration {
    let floor = u64::from(chain_ms);
    let ms = match input {
        TaskInput::Translate { text, .. } if is_local => {
            floor.max(text.chars().count() as u64 * LOCAL_TRANSLATE_MS_PER_CHAR)
        }
        TaskInput::Ocr { image_png, .. } if is_local => match png_dimensions(image_png) {
            Some((w, h)) => {
                floor.max(u64::from(w) * u64::from(h) * LOCAL_OCR_MS_PER_MEGAPIXEL / 1_000_000)
            }
            None => floor,
        },
        _ => floor,
    };
    Duration::from_millis(ms)
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
            cache: TranslationCache::default(),
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
            cache: TranslationCache::default(),
        }
    }

    /// Install a callback that every Ollama-style streaming provider
    /// will invoke per chunk. Re-runs provider discovery so existing
    /// provider instances see the new sink.
    pub fn set_chunk_sink(&self, sink: Option<ChunkSink>) {
        *self.chunk_sink.write() = sink;
        self.rebuild_providers();
    }

    /// Forget cached translations (provider or settings changed).
    pub fn clear_translation_cache(&self) {
        self.cache.clear();
    }

    /// Rebuild the in-memory provider map from the current settings. Called
    /// on startup and whenever settings hot-reload.
    pub fn rebuild_providers(&self) {
        // A different model or prompt must not be masked by old output.
        self.clear_translation_cache();
        let s = self.settings.snapshot();
        self.cache
            .set_ttl_secs(u64::from(s.performance.cache_ttl_seconds));
        let mut map: HashMap<String, ProviderHandle> = HashMap::new();
        let empty = serde_json::Value::Object(Default::default());
        let sink = self.chunk_sink.read().clone();
        let use_gpu = s.performance.use_gpu_if_available;
        let opts = Arc::new(TranslateOptions::from_settings(&s.translation));
        for (id, cfg) in &s.providers.configs {
            if let Some(h) = builtin_provider(id, cfg, sink.as_ref(), use_gpu, &opts) {
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
        // Any built-in id referenced by a chain without a `configs` entry
        // still gets a provider with default settings. Without this, a
        // user config that omits e.g. `[providers.configs.rqrr]` (the
        // shipped default does) silently disabled QR decoding.
        let p = &s.providers;
        for id in [
            &p.translate,
            &p.ocr,
            &p.asr,
            &p.explain,
            &p.summarize,
            &p.reverse_image_search,
            &p.qr_handler,
            &p.intent_classifier,
        ]
        .iter()
        .flat_map(|c| c.chain.iter())
        {
            if map.contains_key(id) {
                continue;
            }
            if let Some(h) = builtin_provider(id, &empty, sink.as_ref(), use_gpu, &opts) {
                map.insert(id.clone(), h);
            }
        }
        *self.providers.write() = map;
    }

    /// Preload the first local provider of the translate chain (and of
    /// explain / summarize when it differs) so the first capture after
    /// launch does not pay a multi-second model load. Best-effort:
    /// failures are logged, never surfaced.
    pub async fn warm_up(&self) {
        let s = self.settings.snapshot();
        let providers = self.providers.read().clone();
        let mut seen = std::collections::HashSet::new();
        for chain in [
            &s.providers.translate.chain,
            &s.providers.explain.chain,
            &s.providers.summarize.chain,
        ] {
            let first_local = chain
                .iter()
                .filter_map(|id| providers.get(id).map(|p| (id, p)))
                .find(|(_, p)| p.is_local());
            if let Some((id, p)) = first_local {
                if seen.insert(id.clone()) {
                    match p.warm_up().await {
                        Ok(()) => tracing::info!("warmed up provider {id}"),
                        Err(e) => tracing::warn!("warm-up of {id} failed: {e}"),
                    }
                }
            }
        }
    }

    /// Load the built-in translation model in the background when the
    /// translate chain uses it and it is installed, so the first capture does not pay the
    /// multi-second load. Set `preload = false` in its config to skip.
    pub fn preload_local_llm(&self) {
        let s = self.settings.snapshot();
        if !pick_chain(&s, Capability::Translate)
            .chain
            .iter()
            .any(|id| id == LOCAL_LLM_ID)
        {
            return;
        }
        let empty = serde_json::Value::Object(Default::default());
        let cfg = s.providers.configs.get(LOCAL_LLM_ID).unwrap_or(&empty);
        if cfg.get("preload").and_then(|v| v.as_bool()) == Some(false) {
            return;
        }
        let use_gpu = s.performance.use_gpu_if_available;
        let p = LlamaTranslator::new(LOCAL_LLM_ID, cfg, use_gpu);
        crate::providers::translate_llama::preload(p.model_path().to_path_buf(), use_gpu, cfg);
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
        // Providers that could not run because their model is missing.
        let mut not_installed: Vec<String> = Vec::new();
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
            let to = effective_timeout(chain.timeout_ms, &input, prov.is_local());
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
                Ok(Err(ProviderError::NotInstalled(what))) => {
                    reasons.push(format!("{id}: not installed ({what})"));
                    not_installed.push(what);
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
                        return Err(ProviderError::Timeout(to.as_millis() as u32));
                    }
                    reasons.push(format!("{id}: timeout"));
                }
            }
        }
        // Nothing ran because nothing is installed: say so, so the UI can
        // point at the download instead of showing a generic failure.
        if !not_installed.is_empty() && not_installed.len() == reasons.len() {
            return Err(ProviderError::NotInstalled(not_installed.join(", ")));
        }
        if chain.chain.is_empty() {
            return Err(ProviderError::Other(format!(
                "no provider configured for {capability:?}"
            )));
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
    /// result bundle. Equivalent to [`Self::analyze_capture`] followed by
    /// [`Self::translate_capture`].
    pub async fn run_capture_flow(
        &self,
        image_png: Vec<u8>,
    ) -> Result<CaptureFlowResult, ProviderError> {
        let mut bundle = self.analyze_capture(image_png).await?;
        if bundle.needs_translation() {
            self.translate_capture(&mut bundle).await?;
        }
        Ok(bundle)
    }

    /// First half of the capture flow: QR decode, OCR and layout. For text
    /// captures the returned bundle has positioned `blocks` without
    /// translations, so the UI can mask the source text and show progress
    /// while [`Self::translate_capture`] runs.
    pub async fn analyze_capture(
        &self,
        image_png: Vec<u8>,
    ) -> Result<CaptureFlowResult, ProviderError> {
        // Cheap heuristic classifier. A plugin-provided classifier can
        // override through the `intent_classifier` chain in future work.
        let img =
            image::load_from_memory(&image_png).map_err(|e| ProviderError::Other(e.to_string()))?;
        let stats = intent::image_stats(&img);
        let intent = stats.map_or(Intent::Unknown, intent::classify_stats);
        tracing::info!(
            "capture {}x{}: luma mean {:.1} std {:.1}, intent {intent:?}",
            img.width(),
            img.height(),
            stats.map_or(0.0, |s| s.mean),
            stats.map_or(0.0, |s| s.std),
        );

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
                    history_kind::QR,
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
                    blocks: vec![],
                    image_size: Some([img.width(), img.height()]),
                    source_language: None,
                    target_language: None,
                });
            }
        }

        // 2) OCR → layout → translate. Always attempted for text-like or
        //    unknown regions.
        let ocr_res = self
            .run(
                Capability::Ocr,
                TaskInput::Ocr {
                    image_png: image_png.clone(),
                    languages: vec![],
                },
            )
            .await;
        let lines: Vec<OcrLine> = match ocr_res {
            Ok(TaskResult {
                output: TaskOutput::Ocr { lines },
                used_provider,
                latency_ms,
                fallback_reasons,
                ..
            }) => {
                tracing::info!(
                    "OCR: {} lines via {used_provider} in {latency_ms} ms (fallbacks {fallback_reasons:?})",
                    lines.len()
                );
                lines
            }
            Ok(other) => {
                tracing::warn!("OCR returned unexpected output: {:?}", other.output);
                vec![]
            }
            // No OCR engine at all: "no text found" would be a lie.
            Err(e @ ProviderError::NotInstalled(_)) => return Err(e),
            // Every engine ran and found nothing: a real "no text".
            Err(e) if ocr_found_nothing(&e) => {
                tracing::info!("OCR found no text: {e}");
                vec![]
            }
            // Timeout or engine failure: report it instead of claiming
            // the region has no text.
            Err(e) => {
                tracing::warn!("OCR failed: {e}");
                return Err(e);
            }
        };
        let text_blocks = layout::group_lines(&lines);
        let ocr_text = layout::join_blocks(&text_blocks);
        let image_size = Some([img.width(), img.height()]);

        if ocr_text.trim().is_empty() {
            // Nothing text-like detected. Return the intent so the UI can
            // offer actions (reverse image search, explain, ...).
            return Ok(CaptureFlowResult {
                intent,
                source_text: None,
                source_rect: None,
                blocks: vec![],
                image_size,
                source_language: None,
                target_language: None,
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

        let rgba = img.to_rgba8();
        let blocks: Vec<LensBlock> = text_blocks
            .iter()
            .map(|b| {
                let colors = (b.bbox[2] > 0.0).then(|| layout::estimate_colors(&rgba, b.bbox));
                let bold = colors.is_some_and(|c| {
                    layout::looks_bold(
                        &b.text,
                        layout::stroke_ratio(&rgba, b.bbox, c, b.line_height),
                    )
                });
                LensBlock {
                    bbox: b.bbox,
                    line_height: b.line_height,
                    source: b.text.clone(),
                    translation: None,
                    colors,
                    same_language: false,
                    from_cache: false,
                    align: b.align,
                    bold,
                }
            })
            .collect();

        let mut bundle = CaptureFlowResult {
            intent: Intent::Text,
            source_language: lang::dominant(blocks.iter().map(|b| b.source.as_str()))
                .map(str::to_string),
            source_text: Some(ocr_text),
            source_rect: None,
            blocks,
            image_size,
            target_language: None,
            result: TaskResult {
                task: "ocr".into(),
                output: TaskOutput::Empty,
                used_provider: String::new(),
                via_cloud: false,
                latency_ms: 0,
                fallback_reasons: vec![],
            },
        };
        let target = self.settings.snapshot().subtitle.target_language.clone();
        self.prepare_translation(&mut bundle, &target);
        Ok(bundle)
    }

    /// Set the target language on an analysed bundle, flag the blocks
    /// that are already in it, and clear any previous translation so the
    /// bundle can be (re)translated with [`Self::translate_capture_to`].
    pub fn mark_same_language(bundle: &mut CaptureFlowResult, target: &str) {
        bundle.target_language = Some(target.to_string());
        for b in &mut bundle.blocks {
            b.same_language = lang::already_in(&b.source, target);
            b.translation = b.same_language.then(|| b.source.clone());
            b.from_cache = false;
        }
    }

    /// [`Self::mark_same_language`] plus filling blocks whose translation
    /// into `target` is cached. Call before showing the analysed bundle so
    /// cached paragraphs appear immediately; only the remaining blocks
    /// (see [`LensBlock::needs_request`]) go to the translator.
    pub fn prepare_translation(&self, bundle: &mut CaptureFlowResult, target: &str) {
        Self::mark_same_language(bundle, target);
        for b in &mut bundle.blocks {
            if b.same_language {
                continue;
            }
            if let Some(hit) = self.cache.get(target, &b.source) {
                b.translation = Some(hit.text);
                b.from_cache = true;
            }
        }
    }

    /// Second half of the capture flow: translate the analysed blocks in
    /// one request (keeping cross-paragraph context), map the result back
    /// onto each block, and record history.
    pub async fn translate_capture(
        &self,
        bundle: &mut CaptureFlowResult,
    ) -> Result<(), ProviderError> {
        let target = self.settings.snapshot().subtitle.target_language.clone();
        self.translate_capture_to(bundle, &target).await
    }

    /// [`Self::translate_capture`], recording history under `kind`.
    pub async fn translate_capture_recorded_as(
        &self,
        bundle: &mut CaptureFlowResult,
        kind: &str,
    ) -> Result<(), ProviderError> {
        let target = self.settings.snapshot().subtitle.target_language.clone();
        self.translate_capture_as(bundle, &target, None, kind).await
    }

    /// [`Self::translate_capture`] into an explicit target language (the
    /// selector's language switch). Blocks already in `target` are passed
    /// through untouched and left out of the request.
    pub async fn translate_capture_to(
        &self,
        bundle: &mut CaptureFlowResult,
        target_language: &str,
    ) -> Result<(), ProviderError> {
        self.translate_capture_with_progress(bundle, target_language, None)
            .await
    }

    /// [`Self::translate_capture_to`], calling `progress` before every
    /// translation request with the blocks translated so far and the
    /// blocks the request will stream into.
    ///
    /// With a translator that reads [`TranslateContext`] (the built-in
    /// PLaMo engine), a long capture is translated in parts: a small
    /// first part so the first translation appears quickly, then larger
    /// parts, each with the earlier parts and their translations as
    /// history, so nothing is translated with less context than in one
    /// request. Other translators get the whole capture in one request
    /// as before.
    pub async fn translate_capture_with_progress(
        &self,
        bundle: &mut CaptureFlowResult,
        target_language: &str,
        progress: Option<&ProgressSink>,
    ) -> Result<(), ProviderError> {
        self.translate_capture_as(bundle, target_language, progress, history_kind::CAPTURE)
            .await
    }

    /// [`Self::translate_capture_with_progress`], recording the result in
    /// history under `kind` (see [`history_kind`]) so the history page can
    /// tell region captures, Lens windows and live translation apart.
    pub async fn translate_capture_as(
        &self,
        bundle: &mut CaptureFlowResult,
        target_language: &str,
        progress: Option<&ProgressSink>,
        kind: &str,
    ) -> Result<(), ProviderError> {
        let s = self.settings.snapshot();
        self.prepare_translation(bundle, target_language);
        let pending: Vec<usize> = (0..bundle.blocks.len())
            .filter(|&i| bundle.blocks[i].needs_request())
            .collect();
        let target_language = target_language.to_string();

        if pending.is_empty() {
            // Everything is already readable or cached: no round-trip.
            let cached: Vec<CachedTranslation> = bundle
                .blocks
                .iter()
                .filter(|b| b.from_cache)
                .filter_map(|b| self.cache.get(&target_language, &b.source))
                .collect();
            let text = bundle
                .blocks
                .iter()
                .filter_map(|b| b.translation.clone())
                .collect::<Vec<_>>()
                .join(layout::BLOCK_SEPARATOR);
            bundle.result = TaskResult {
                task: "translate".into(),
                output: TaskOutput::Text(text),
                used_provider: cached
                    .first()
                    .map(|c| c.provider.clone())
                    .unwrap_or_else(|| "builtin.passthrough".into()),
                via_cloud: cached.iter().any(|c| c.via_cloud),
                latency_ms: 0,
                fallback_reasons: vec![],
            };
            return Ok(());
        }

        let sources: Vec<String> = pending
            .iter()
            .map(|&i| bundle.blocks[i].source.clone())
            .collect();
        let in_parts = self.translator_takes_context();
        let parts = if in_parts {
            let costs: Vec<usize> = sources.iter().map(|t| translate_plan::cost(t)).collect();
            translate_plan::plan(
                &costs,
                translate_plan::FIRST_PART,
                translate_plan::LATER_PART,
            )
        } else {
            std::iter::once(0..sources.len()).collect()
        };
        // The first part is read with the text after it as reference, so
        // a heading or a row of short labels is not translated blind. The
        // same reference stays in every later prompt so the prefix cache
        // still matches.
        let reference = if parts.len() > 1 {
            translate_plan::take_reference(
                sources[parts[1].start..].iter().map(String::as_str),
                layout::BLOCK_SEPARATOR,
                translate_plan::REFERENCE,
            )
        } else {
            None
        };

        let started = Instant::now();
        let mut history: Vec<(String, String)> = Vec::new();
        let mut first: Option<TaskResult> = None;
        let mut reasons: Vec<String> = Vec::new();
        let mut outputs: Vec<String> = Vec::new();
        for range in parts {
            let idx = &pending[range.clone()];
            notify(progress, bundle, idx);
            let request_text = sources[range.clone()].join(layout::BLOCK_SEPARATOR);
            let context = TranslateContext {
                history: history.clone(),
                reference: reference.clone(),
            };
            let translate = match self
                .run(
                    Capability::Translate,
                    TaskInput::Translate {
                        text: request_text.clone(),
                        source_language: "auto".into(),
                        target_language: target_language.clone(),
                        context: context.clone(),
                    },
                )
                .await
            {
                Ok(t) => t,
                Err(e) if first.is_none() => return Err(e),
                Err(e) => {
                    // Earlier parts are already on screen; keep them and
                    // leave the rest untranslated.
                    tracing::warn!("translating the rest of the capture failed: {e}");
                    reasons.push(e.to_string());
                    break;
                }
            };

            let whole = match &translate.output {
                TaskOutput::Text(t) => t.clone(),
                _ => String::new(),
            };
            let (translations, joined): (Vec<Option<String>>, String) =
                match layout::split_translation(&whole, idx.len()) {
                    Some(parts) => (parts.into_iter().map(Some).collect(), whole.clone()),
                    None if in_parts => {
                        // The translator merged or split paragraphs, so
                        // the per-block mapping is ambiguous. Translate
                        // each block on its own, still with the capture's
                        // context, so short labels keep their meaning.
                        let mut ctx = context;
                        let mut out = Vec::with_capacity(idx.len());
                        for (k, &bi) in idx.iter().enumerate() {
                            notify(progress, bundle, &[bi]);
                            let src = &sources[range.start + k];
                            let t = self
                                .translate_one(TaskInput::Translate {
                                    text: src.clone(),
                                    source_language: "auto".into(),
                                    target_language: target_language.clone(),
                                    context: ctx.clone(),
                                })
                                .await;
                            if let Some(t) = &t {
                                ctx.history.push((src.clone(), t.clone()));
                                bundle.blocks[bi].translation = Some(t.clone());
                            }
                            out.push(t);
                        }
                        let joined = out
                            .iter()
                            .flatten()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(layout::BLOCK_SEPARATOR);
                        (out, joined)
                    }
                    None => {
                        // Same, for translators without context: one
                        // request per block, concurrently.
                        let max = s.performance.max_concurrent_tasks.max(1) as usize;
                        let inputs: Vec<TaskInput> = sources[range.clone()]
                            .iter()
                            .map(|text| TaskInput::Translate {
                                text: text.clone(),
                                source_language: "auto".into(),
                                target_language: target_language.clone(),
                                context: TranslateContext::default(),
                            })
                            .collect();
                        let out: Vec<Option<String>> = futures::stream::iter(inputs)
                            .map(|input| self.translate_one(input))
                            .buffered(max)
                            .collect()
                            .await;
                        (out, whole.clone())
                    }
                };
            for (&i, t) in idx.iter().zip(translations) {
                if let Some(text) = &t {
                    self.cache.put(
                        &target_language,
                        &bundle.blocks[i].source,
                        CachedTranslation {
                            text: text.clone(),
                            provider: translate.used_provider.clone(),
                            via_cloud: translate.via_cloud,
                        },
                    );
                }
                bundle.blocks[i].translation = t;
            }
            if !joined.trim().is_empty() {
                outputs.push(joined.clone());
                history.push((request_text, joined));
                translate_plan::trim_history(&mut history, translate_plan::HISTORY);
            }
            reasons.extend(translate.fallback_reasons.iter().cloned());
            first.get_or_insert(translate);
        }
        let Some(first) = first else {
            return Ok(());
        };
        let whole = outputs.join(layout::BLOCK_SEPARATOR);
        self.record_history(
            kind,
            Some(&sources.join(layout::BLOCK_SEPARATOR)),
            Some(whole.as_str()).filter(|t| !t.is_empty()),
            Some(&first.used_provider),
            first.via_cloud,
        );
        bundle.result = TaskResult {
            output: TaskOutput::Text(whole),
            latency_ms: started.elapsed().as_millis() as u64,
            fallback_reasons: reasons,
            ..first
        };
        Ok(())
    }

    /// Whether the translator that will most likely take the next request
    /// reads [`TranslateContext`]: the first provider in the (sticky)
    /// translate chain that is registered, allowed and installed.
    fn translator_takes_context(&self) -> bool {
        let s = self.settings.snapshot();
        let chain = pick_chain(&s, Capability::Translate);
        let providers = self.providers.read();
        for id in apply_stickiness(&self.sticky, Capability::Translate, &chain.chain) {
            let Some(p) = providers.get(&id) else {
                continue;
            };
            if !p.is_local()
                && (!chain.allow_cloud || !s.privacy.allow_cloud_globally || !chain.allow_network)
            {
                continue;
            }
            if let Some(takes) = p.takes_translation_context() {
                return takes;
            }
        }
        false
    }

    async fn translate_one(&self, input: TaskInput) -> Option<String> {
        match self.run(Capability::Translate, input).await {
            Ok(TaskResult {
                output: TaskOutput::Text(t),
                ..
            }) => Some(t.trim().to_string()),
            _ => None,
        }
    }

    /// Append one row to history unless the user turned history off,
    /// then apply the retention settings.
    pub fn record_history(
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
        // Settings → 履歴: retention period and entry cap.
        if let Err(e) = self
            .storage
            .apply_retention(s.storage.retention_days, s.storage.max_entries)
        {
            tracing::warn!("history retention failed: {e}");
        }
    }
}

/// `kind` values of history rows written by the pipeline. Rows from
/// before these existed use `"translate"`.
pub mod history_kind {
    /// Region / full-screen capture (selector).
    pub const CAPTURE: &str = "capture";
    /// Lens window.
    pub const LENS: &str = "lens";
    /// Live translation (pinned region).
    pub const LIVE: &str = "live";
    /// Text translation page.
    pub const TEXT: &str = "text";
    /// Image translation page (an image file).
    pub const IMAGE: &str = "image";
    /// Decoded QR code.
    pub const QR: &str = "qr";
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
    /// Paragraph blocks with their position in the captured image and
    /// per-block translation. Drives the in-place (Lens-style) overlay.
    /// Empty for QR / non-text captures.
    #[serde(default)]
    pub blocks: Vec<LensBlock>,
    /// `[width, height]` of the captured image, the coordinate space of
    /// every `blocks[].bbox`.
    pub image_size: Option<[u32; 2]>,
    /// Best guess at the source language (`ja`, `en`, …), if any.
    #[serde(default)]
    pub source_language: Option<String>,
    /// Language the blocks were (or will be) translated into.
    #[serde(default)]
    pub target_language: Option<String>,
}

/// One paragraph of recognised text, positioned in the captured image.
#[derive(Debug, Clone, Serialize)]
pub struct LensBlock {
    /// `[x, y, w, h]` in captured-image pixels.
    pub bbox: [f32; 4],
    /// Median source line height in pixels (initial font-size hint).
    pub line_height: f32,
    pub source: String,
    /// `None` when translation failed for this block.
    pub translation: Option<String>,
    /// Background / text colour sampled from the capture, used to erase
    /// the original text and paint the translation over it.
    pub colors: Option<layout::BlockColors>,
    /// True when the block is already in the target language (or has no
    /// letters), so it was not sent to the translator and `translation`
    /// is the source text. The overlay leaves such blocks uncovered.
    #[serde(default)]
    pub same_language: bool,
    /// True when `translation` came from the translation cache rather
    /// than this request.
    #[serde(default)]
    pub from_cache: bool,
    /// Horizontal alignment of the source text.
    #[serde(default)]
    pub align: layout::Align,
    /// The source text looks bold (headings, buttons); the translation
    /// is drawn bold too.
    #[serde(default)]
    pub bold: bool,
}

impl LensBlock {
    /// Whether this block still has to be sent to the translator.
    pub fn needs_request(&self) -> bool {
        !self.same_language && !self.from_cache
    }
}

impl CaptureFlowResult {
    /// True when [`Pipeline::analyze_capture`] found text that still has
    /// to go through [`Pipeline::translate_capture`].
    pub fn needs_translation(&self) -> bool {
        matches!(self.intent, Intent::Text)
            && !self.blocks.is_empty()
            && matches!(self.result.output, TaskOutput::Empty)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureFlowRect {
    pub monitor_index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Construct a built-in provider by id. Returns `None` for ids that are
/// not built in (plugins, typos, `builtin.heuristic`).
fn builtin_provider(
    id: &str,
    cfg: &serde_json::Value,
    sink: Option<&ChunkSink>,
    use_gpu: bool,
    opts: &Arc<TranslateOptions>,
) -> Option<ProviderHandle> {
    Some(match id {
        LOCAL_LLM_ID | LOCAL_ASSIST_ID => {
            let mut p = LlamaTranslator::new(id, cfg, use_gpu).with_options(opts.clone());
            if let Some(sink) = sink {
                p = p.with_chunk_sink(sink.clone());
            }
            Arc::new(p)
        }
        id if id.starts_with("ollama.") => {
            let mut p = OllamaTranslator::new(id, cfg).with_options(opts.clone());
            if let Some(sink) = sink {
                p = p.with_chunk_sink(sink.clone());
            }
            Arc::new(p)
        }
        "deepl" => Arc::new(DeepLTranslator::new(id, cfg)),
        "gemini" => Arc::new(GeminiProvider::new(id, cfg)),
        crate::providers::asr_local::LOCAL_ASR_ID => Arc::new(
            crate::providers::asr_local::LocalWhisperProvider::new(id, cfg, use_gpu),
        ),
        id if id.starts_with("whisper_cpp.") => Arc::new(WhisperCppProvider::new(id, cfg)),
        "ocrs" => Arc::new(OcrsProvider::new(id, cfg)),
        "ppocr" => Arc::new(crate::providers::ocr_ppocr::PpOcrProvider::new(id, cfg)),
        "rqrr" => Arc::new(RqrrProvider::new(id, cfg)),
        id if id.starts_with("browser.") => Arc::new(BrowserSearchProvider::new(id, cfg)),
        _ => return None,
    })
}

/// OCR chain shipped before PP-OCR existed. Config files are seeded with
/// the defaults on first run, so existing users still carry it; treat it
/// as the current default so they get multilingual OCR too.
const LEGACY_DEFAULT_OCR_CHAIN: [&str; 2] = ["ocrs", "paddle"];
const DEFAULT_OCR_CHAIN: [&str; 2] = ["ppocr", "ocrs"];

fn upgrade_ocr_chain(mut chain: TaskChain) -> TaskChain {
    if chain.chain == LEGACY_DEFAULT_OCR_CHAIN {
        chain.chain = DEFAULT_OCR_CHAIN.iter().map(|s| s.to_string()).collect();
    }
    chain
}

/// Built-in llama.cpp provider running the translation model
/// (PLaMo 3 Translate by default).
pub const LOCAL_LLM_ID: &str = "llamacpp";
/// Built-in llama.cpp provider running the general model used for
/// explain / summarize (Gemma 4 by default).
pub const LOCAL_ASSIST_ID: &str = "llamacpp.assistant";

/// Before the built-in engine existed, LLM chains started with the
/// Ollama provider and fell back to cloud services. Config files seeded
/// with those defaults move to the fully local built-in engine (Ollama
/// and the cloud providers stay usable for anyone who lists them in a
/// chain of their own).
const LEGACY_LLM_CHAINS: [&[&str]; 2] = [
    &["ollama.gemma4", "deepl", "gemini"],
    &["ollama.gemma4", "gemini"],
];

fn upgrade_llm_chain(mut chain: TaskChain, local_id: &str) -> TaskChain {
    if LEGACY_LLM_CHAINS.iter().any(|c| chain.chain == *c) {
        chain.chain = vec![local_id.to_string()];
    }
    chain
}

fn pick_chain(s: &crate::settings::schema::Settings, cap: Capability) -> TaskChain {
    match cap {
        Capability::Translate => upgrade_llm_chain(s.providers.translate.clone(), LOCAL_LLM_ID),
        Capability::Ocr => upgrade_ocr_chain(s.providers.ocr.clone()),
        Capability::Asr => s.providers.asr.clone(),
        Capability::Explain => upgrade_llm_chain(s.providers.explain.clone(), LOCAL_ASSIST_ID),
        Capability::Summarize => upgrade_llm_chain(s.providers.summarize.clone(), LOCAL_ASSIST_ID),
        Capability::ReverseImageSearch => s.providers.reverse_image_search.clone(),
        Capability::QrHandler => s.providers.qr_handler.clone(),
        Capability::IntentClassifier => s.providers.intent_classifier.clone(),
    }
}

#[cfg(test)]
pub fn upgrade_ocr_chain_for_test(chain: Vec<String>) -> Vec<String> {
    upgrade_ocr_chain(TaskChain {
        chain,
        ..TaskChain::default()
    })
    .chain
}

#[cfg(test)]
pub fn upgrade_llm_chain_for_test(chain: Vec<String>, local_id: &str) -> Vec<String> {
    upgrade_llm_chain(
        TaskChain {
            chain,
            ..TaskChain::default()
        },
        local_id,
    )
    .chain
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
