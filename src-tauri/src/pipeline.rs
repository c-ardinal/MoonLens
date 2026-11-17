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

    /// Rebuild the in-memory provider map from the current settings. Called
    /// on startup and whenever settings hot-reload.
    pub fn rebuild_providers(&self) {
        let s = self.settings.snapshot();
        let mut map: HashMap<String, ProviderHandle> = HashMap::new();
        let empty = serde_json::Value::Object(Default::default());
        let sink = self.chunk_sink.read().clone();
        for (id, cfg) in &s.providers.configs {
            if let Some(h) = builtin_provider(id, cfg, sink.as_ref()) {
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
            if let Some(h) = builtin_provider(id, &empty, sink.as_ref()) {
                map.insert(id.clone(), h);
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
                ..
            }) => lines,
            _ => vec![],
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
            .map(|b| LensBlock {
                bbox: b.bbox,
                line_height: b.line_height,
                source: b.text.clone(),
                translation: None,
                colors: (b.bbox[2] > 0.0).then(|| layout::estimate_colors(&rgba, b.bbox)),
                same_language: false,
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
        Self::mark_same_language(&mut bundle, &target);
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

    /// [`Self::translate_capture`] into an explicit target language (the
    /// selector's language switch). Blocks already in `target` are passed
    /// through untouched and left out of the request.
    pub async fn translate_capture_to(
        &self,
        bundle: &mut CaptureFlowResult,
        target_language: &str,
    ) -> Result<(), ProviderError> {
        let s = self.settings.snapshot();
        Self::mark_same_language(bundle, target_language);
        let pending: Vec<usize> = (0..bundle.blocks.len())
            .filter(|&i| !bundle.blocks[i].same_language)
            .collect();
        let target_language = target_language.to_string();

        if pending.is_empty() {
            // Everything is already readable: no round-trip at all.
            bundle.result = TaskResult {
                task: "translate".into(),
                output: TaskOutput::Text(bundle.source_text.clone().unwrap_or_default()),
                used_provider: "builtin.passthrough".into(),
                via_cloud: false,
                latency_ms: 0,
                fallback_reasons: vec![],
            };
            return Ok(());
        }

        let sources: Vec<String> = pending
            .iter()
            .map(|&i| bundle.blocks[i].source.clone())
            .collect();
        let request_text = sources.join(layout::BLOCK_SEPARATOR);
        let translate = self
            .run(
                Capability::Translate,
                TaskInput::Translate {
                    text: request_text.clone(),
                    source_language: "auto".into(),
                    target_language: target_language.clone(),
                },
            )
            .await?;

        let whole = match &translate.output {
            TaskOutput::Text(t) => t.clone(),
            _ => String::new(),
        };
        let translations: Vec<Option<String>> =
            match layout::split_translation(&whole, pending.len()) {
                Some(parts) => parts.into_iter().map(Some).collect(),
                None => {
                    // The translator merged or split paragraphs, so the
                    // per-block mapping is ambiguous. Translate each block
                    // on its own (concurrently) so the in-place overlay can
                    // still put every translation over its source.
                    let max = s.performance.max_concurrent_tasks.max(1) as usize;
                    let inputs: Vec<TaskInput> = sources
                        .iter()
                        .map(|text| TaskInput::Translate {
                            text: text.clone(),
                            source_language: "auto".into(),
                            target_language: target_language.clone(),
                        })
                        .collect();
                    futures::stream::iter(inputs)
                        .map(|input| self.translate_one(input))
                        .buffered(max)
                        .collect()
                        .await
                }
            };
        for (&i, t) in pending.iter().zip(translations) {
            bundle.blocks[i].translation = t;
        }

        self.record_history(
            "translate",
            Some(&request_text),
            Some(whole.as_str()).filter(|t| !t.is_empty()),
            Some(&translate.used_provider),
            translate.via_cloud,
        );
        bundle.result = translate;
        Ok(())
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
) -> Option<ProviderHandle> {
    Some(match id {
        id if id.starts_with("ollama.") => {
            let mut p = OllamaTranslator::new(id, cfg);
            if let Some(sink) = sink {
                p = p.with_chunk_sink(sink.clone());
            }
            Arc::new(p)
        }
        "deepl" => Arc::new(DeepLTranslator::new(id, cfg)),
        "gemini" => Arc::new(GeminiProvider::new(id, cfg)),
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

fn pick_chain(s: &crate::settings::schema::Settings, cap: Capability) -> TaskChain {
    match cap {
        Capability::Translate => s.providers.translate.clone(),
        Capability::Ocr => upgrade_ocr_chain(s.providers.ocr.clone()),
        Capability::Asr => s.providers.asr.clone(),
        Capability::Explain => s.providers.explain.clone(),
        Capability::Summarize => s.providers.summarize.clone(),
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
