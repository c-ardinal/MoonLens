//! Single entry point the Lens production loop and the offline
//! evaluator both drive through. Every concrete dependency the
//! pipeline used to inline — capture, OCR engine, translator, and
//! the event fan-out — is injected as a trait object so the two
//! paths literally execute the same function, with only the input
//! source and output sink differing. That is the structural
//! invariant: if behaviour diverges between `lens_eval` and the
//! running Lens window, the divergence MUST live in one of the
//! injected adapters (where it is spot-able), not in the pipeline
//! logic itself.
//!
//! Principles the design enforces:
//!
//! * **DIP** — high-level policy (the pipeline) depends on the
//!   [`LensInput`], [`LensSink`], [`LensOcr`], and
//!   [`LensTranslator`] abstractions; low-level concretes
//!   (Tauri, WinRT, Ollama, PNG file I/O) are swapped at the call
//!   site.
//! * **SRP** — [`run_lens_pipeline`] owns sequencing ONLY. Capture
//!   specifics, OCR provider chains, translation chunk routing,
//!   and event emission each live behind a single-method trait.
//! * **OCP** — new sinks (integration tests, future GUI variants,
//!   CLI JSON dump) plug in without touching the pipeline.
//!
//! A mock-adapter integration test (see `pipeline_tests.rs`)
//! records the exact sink call sequence; any future refactor that
//! accidentally moves logic back into one caller will break it.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::lens_watcher::{compute_lens_cards, preprocess_ocr_lines, LensOcrLine};
use crate::providers::OcrLine;

/// One captured frame's pixel content + dimensions. The width and
/// height must match the PNG's decoded image dimensions so downstream
/// card geometry uses the same coordinate space as the capture.
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// The input side of the pipeline. Production reads from the screen
/// via `capture_lens`; the offline evaluator reads from a PNG file.
/// Both return the same [`CapturedFrame`] shape so the pipeline
/// can treat them identically.
pub trait LensInput: Send {
    fn capture(&mut self) -> Result<CapturedFrame>;
}

/// The output side of the pipeline. Production emits Tauri events;
/// the evaluator accumulates the same data and renders it to a PNG
/// at flow-done. Implementations MUST be idempotent under repeated
/// calls with the same `flow_id` — the pipeline guarantees exactly
/// one `on_flow_start` / `on_flow_done` pair per successful run
/// and one `on_cards_ready` in between.
pub trait LensSink: Send + Sync {
    fn on_flow_start(&self, flow_id: u64);
    fn on_cards_ready(&self, flow_id: u64, image_w: u32, image_h: u32, cards: &[LensOcrLine]);
    fn on_line_chunk(&self, flow_id: u64, line_index: usize, delta: String, done: bool);
    fn on_flow_done(&self, flow_id: u64, line_count: usize);
    /// Optional hook the production path uses to hide the Lens
    /// window from the capturer on platforms without a native
    /// capture-exclusion API. Default no-op so test / eval sinks
    /// do not have to care.
    fn on_mask(&self, _flow_id: u64, _masked: bool) {}
}

/// Pluggable OCR backend. Production uses a WinRT-backed provider
/// fetched from `SettingsStore`; the evaluator uses the same
/// adapter over the same provider — the whole point of the
/// refactor is that both go through THIS trait, not two separate
/// inlined `run_ocr(..)` calls.
#[async_trait]
pub trait LensOcr: Send + Sync {
    async fn recognize(&self, image_png: Vec<u8>) -> Result<Vec<OcrLine>>;
}

/// Pluggable translation backend. The `chunk_sink` closure is
/// invoked once per streaming delta the provider produces, so both
/// the Tauri event sink and the eval's accumulator see exactly
/// the same stream of tokens.
#[async_trait]
pub trait LensTranslator: Send + Sync {
    /// Translate a card's text into `target_language`. `source_language`
    /// is the user's configured source — typically `"auto"` so the
    /// provider runs its own script detector, but the channel is open
    /// so callers can pin a specific source (`"en"`, `"zh"`, …) when
    /// the user has set `translation.default_source_language` to a
    /// non-auto value.
    async fn translate(
        &self,
        text: String,
        source_language: String,
        target_language: String,
        chunk_sink: Arc<dyn Fn(String, bool) + Send + Sync>,
    ) -> Result<String>;

    /// Like [`translate`] but with a per-request glossary that the
    /// underlying provider can splice into its system prompt. Default
    /// impl drops the glossary so legacy translators (echo, mocks)
    /// keep working unchanged.
    async fn translate_with_glossary(
        &self,
        text: String,
        source_language: String,
        target_language: String,
        glossary: Vec<(String, String)>,
        chunk_sink: Arc<dyn Fn(String, bool) + Send + Sync>,
    ) -> Result<String> {
        let _ = glossary;
        self.translate(text, source_language, target_language, chunk_sink)
            .await
    }

    /// Build a glossary for a batch of source lines that will be
    /// translated in sequence. Default returns empty (no glossary).
    /// The chain-backed implementation forwards to
    /// [`crate::pipeline::Pipeline::extract_glossary`].
    async fn extract_glossary(
        &self,
        lines: &[&str],
        source_language: &str,
        target_language: &str,
    ) -> Vec<(String, String)> {
        let _ = (lines, source_language, target_language);
        Vec::new()
    }
}

/// Tunables the pipeline reads on every run. Intentionally a plain
/// value struct — no references to `SettingsStore` — so tests can
/// construct one without pulling in the full settings machinery.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Source language code (`"auto"` to let the provider's
    /// detector decide, or an ISO code like `"en"` / `"zh"` /
    /// `"ko"` when the user has pinned the source). Threaded into
    /// every translate + glossary call below so the pipeline
    /// honours the user's `translation.default_source_language`
    /// setting end-to-end.
    pub source_language: String,
    pub target_language: String,
    pub max_concurrent_translate: usize,
    pub ocr_scale: u32,
    /// Minimum number of OCR lines required before the Phase 29
    /// glossary pre-pass kicks in. Below this, the upfront LLM
    /// call adds latency without any inter-line consistency
    /// problem to fix — for a single-line lens there are no
    /// "other lines" to keep aligned. Set to 0 to disable the
    /// gate entirely (every translation pays glossary latency).
    /// Set to a value larger than realistic line counts (e.g.
    /// `u32::MAX`) to disable the feature.
    pub glossary_min_lines: usize,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            // Matches the schema's `translation.default_source_language`
            // default so tests and production agree on the seed value.
            source_language: "auto".into(),
            target_language: "ja".into(),
            max_concurrent_translate: 2,
            // 3× Lanczos upscale: at 2× the OCR engine struggled
            // with body-text font sizes ~11 px (synth_small_body
            // returned 39 residual chars worth of fragments). Per
            // the OCR-preprocessing literature (Tesseract docs,
            // copyprogramming.com 2026 guide), small fonts need
            // ≥ 300 DPI for reliable recognition — at 96 DPI screen
            // capture that means roughly 3× upscale for body and
            // 4× for sub-10 px UI text. 3× is the sweet spot
            // before processing time grows quadratically.
            ocr_scale: 3,
            // 5 lines is the inflection point per the iter5 OCR
            // bench fixtures: paragraph-shaped Lens captures hit
            // 4-7 lines typically; below 5 the model rarely has
            // ambiguous proper nouns AND the glossary call's
            // ~300 ms latency would dominate the per-line decode.
            glossary_min_lines: 5,
        }
    }
}

/// End-to-end Lens pipeline. Given an input source, a sink, and
/// pluggable OCR / translation providers, produces the full card
/// render sequence callers downstream assemble into either Tauri
/// events (production) or a static PNG (evaluator).
///
/// Flow:
///   1. `input.capture()` → PNG + dims.
///   2. `sink.on_flow_start(flow_id)`.
///   3. 2× Lanczos upscale → OCR → bbox scale-back.
///   4. [`preprocess_ocr_lines`] drops noise + flags target-native
///      lines.
///   5. Decode the PNG for rim colour / weight sampling.
///   6. [`compute_lens_cards`] emits one card per paragraph block
///      (sub-group-aware so skipped intrusions never engulfed).
///   7. `sink.on_cards_ready(...)`.
///   8. One translation call per card; each streaming chunk
///      forwards to `sink.on_line_chunk(...)`.
///   9. `sink.on_flow_done(line_count)`.
///
/// A 0-card outcome still fires `on_cards_ready` + `on_flow_done`
/// so the UI exits its streaming state instead of hanging.
pub async fn run_lens_pipeline(
    input: &mut dyn LensInput,
    sink: Arc<dyn LensSink>,
    ocr: Arc<dyn LensOcr>,
    translator: Arc<dyn LensTranslator>,
    config: &PipelineConfig,
    flow_id: u64,
) -> Result<()> {
    let frame = input.capture()?;
    sink.on_flow_start(flow_id);

    let scale = config.ocr_scale.max(1);
    let scaled_png = crate::lens_watcher::preprocess_png_for_ocr(&frame.png, scale)
        .unwrap_or_else(|_| frame.png.clone());
    let raw_lines = ocr.recognize(scaled_png).await.unwrap_or_default();

    let inv_scale = 1.0 / scale as f32;
    let scaled_back: Vec<OcrLine> = raw_lines
        .into_iter()
        .map(|mut l| {
            for v in l.bbox.iter_mut() {
                *v *= inv_scale;
            }
            l
        })
        .collect();

    let (ocr_lines, skip_flags) = preprocess_ocr_lines(scaled_back, &config.target_language);
    let decoded = image::load_from_memory(&frame.png).ok();
    let cards = compute_lens_cards(
        &ocr_lines,
        &skip_flags,
        decoded.as_ref(),
        frame.width,
        frame.height,
    );
    sink.on_cards_ready(flow_id, frame.width, frame.height, &cards);

    if cards.is_empty() {
        sink.on_flow_done(flow_id, 0);
        return Ok(());
    }

    let max_concurrent = config.max_concurrent_translate.max(1);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));
    // Normalise the source code: an empty / whitespace-only setting
    // is treated as `"auto"` so the provider's detector still runs.
    // Any other value passes through verbatim — the embedded LLM,
    // DeepL, and Gemini providers all consume it for their prompt /
    // API payload.
    let source_lang = if config.source_language.trim().is_empty() {
        "auto".to_string()
    } else {
        config.source_language.clone()
    };
    let target_lang = config.target_language.clone();

    // Phase 29: build a per-batch glossary so the same proper noun
    // renders the same way across every line in this capture. Gate
    // on `min_lines` — single-line / two-line lenses have nothing
    // to disambiguate and the upfront LLM call would just waste
    // ~300 ms. Producers that hit the gate but fail to return a
    // glossary fall through to `Vec::new()`.
    let glossary = if cards.len() >= config.glossary_min_lines.max(1) {
        let line_refs: Vec<&str> =
            cards.iter().map(|c| c.text.as_str()).collect();
        translator
            .extract_glossary(&line_refs, &source_lang, &target_lang)
            .await
    } else {
        Vec::new()
    };
    if !glossary.is_empty() {
        tracing::info!(
            "lens glossary: {} entries from {} lines",
            glossary.len(),
            cards.len()
        );
    }

    let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::with_capacity(cards.len());
    for card in cards.iter() {
        let sem = semaphore.clone();
        let translator = translator.clone();
        let source_c = source_lang.clone();
        let target_c = target_lang.clone();
        let text = card.text.clone();
        let line_index = card.line_index;
        let sink_c = sink.clone();
        let glossary_c = glossary.clone();
        let chunk_sink: Arc<dyn Fn(String, bool) + Send + Sync> = {
            let sink_for_chunk = sink.clone();
            Arc::new(move |delta, done| {
                sink_for_chunk.on_line_chunk(flow_id, line_index, delta, done);
            })
        };
        let task = tokio::spawn(async move {
            let _permit = match sem.acquire_owned().await {
                Ok(p) => p,
                Err(_) => return,
            };
            if let Err(e) = translator
                .translate_with_glossary(text, source_c, target_c, glossary_c, chunk_sink)
                .await
            {
                tracing::info!("translate failed for line {line_index}: {e}");
            }
            let _ = sink_c;
        });
        tasks.push(task);
    }
    for t in tasks {
        let _ = t.await;
    }

    sink.on_flow_done(flow_id, cards.len());
    Ok(())
}

// --------------------------------------------------------------
// Reusable adapters — shared between production and the evaluator.
// --------------------------------------------------------------

/// Feed an already-captured frame into the pipeline. Production
/// runs its own capture loop (for hash-based dedup + mask signalling
/// + window-alive checks) and hands the resulting frame to the
/// pipeline via this adapter; the offline evaluator reads a PNG
/// file up front and uses the same wrapper. Returning `Err` on a
/// second call keeps the pipeline a single-shot operation — callers
/// that want to process multiple frames loop outside.
pub struct PreCapturedInput {
    frame: Option<CapturedFrame>,
}

impl PreCapturedInput {
    pub fn new(frame: CapturedFrame) -> Self {
        Self { frame: Some(frame) }
    }
}

impl LensInput for PreCapturedInput {
    fn capture(&mut self) -> Result<CapturedFrame> {
        self.frame
            .take()
            .ok_or_else(|| anyhow::anyhow!("PreCapturedInput already consumed"))
    }
}

/// Buffer every sink call in memory so a caller can inspect the
/// full pipeline outcome after the run completes. The evaluator
/// uses this to grab cards + translations to render into its PNG
/// output; integration tests use it to assert the exact call
/// sequence the pipeline produced.
pub struct RecordingSink {
    state: std::sync::Mutex<RecordedRun>,
}

#[derive(Debug, Clone, Default)]
pub struct RecordedRun {
    pub flow_start_fired: bool,
    pub image_w: u32,
    pub image_h: u32,
    pub cards: Vec<LensOcrLine>,
    /// `line_index` → accumulated translation text (concatenated
    /// deltas). Stored in insertion order of first chunk.
    pub translations: Vec<(usize, String, bool)>,
    pub flow_done_line_count: Option<usize>,
    pub mask_events: Vec<(u64, bool)>,
}

impl RecordingSink {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            state: std::sync::Mutex::new(RecordedRun::default()),
        })
    }

    /// Snapshot the recorded run. Safe to call at any time; typically
    /// called after `run_lens_pipeline` returns.
    pub fn snapshot(&self) -> RecordedRun {
        self.state.lock().unwrap().clone()
    }

    /// For every card, return its final translation (concatenation
    /// of all non-empty deltas). Cards without any chunk recorded
    /// map to an empty string — keeping the return vec aligned with
    /// `cards` one-to-one.
    pub fn cards_with_translations(&self) -> Vec<(LensOcrLine, String)> {
        let state = self.state.lock().unwrap();
        state
            .cards
            .iter()
            .map(|card| {
                let combined: String = state
                    .translations
                    .iter()
                    .filter(|(idx, _, _)| *idx == card.line_index)
                    .map(|(_, text, _)| text.as_str())
                    .collect::<Vec<_>>()
                    .concat();
                (card.clone(), combined)
            })
            .collect()
    }
}

impl Default for RecordingSink {
    fn default() -> Self {
        Self {
            state: std::sync::Mutex::new(RecordedRun::default()),
        }
    }
}

/// Fixed-payload [`LensOcr`] used in integration tests so we can
/// feed the pipeline a deterministic set of lines without standing
/// up a real OCR provider.
pub struct MockLensOcr {
    pub lines: Vec<OcrLine>,
}

#[async_trait]
impl LensOcr for MockLensOcr {
    async fn recognize(&self, _image_png: Vec<u8>) -> Result<Vec<OcrLine>> {
        Ok(self.lines.clone())
    }
}

impl LensSink for RecordingSink {
    fn on_flow_start(&self, _flow_id: u64) {
        self.state.lock().unwrap().flow_start_fired = true;
    }
    fn on_cards_ready(&self, _flow_id: u64, image_w: u32, image_h: u32, cards: &[LensOcrLine]) {
        let mut s = self.state.lock().unwrap();
        s.image_w = image_w;
        s.image_h = image_h;
        s.cards = cards.to_vec();
    }
    fn on_line_chunk(&self, _flow_id: u64, line_index: usize, delta: String, done: bool) {
        let mut s = self.state.lock().unwrap();
        if !delta.is_empty() {
            s.translations.push((line_index, delta, done));
        } else if done {
            // Record a trailing done marker with empty delta so
            // consumers that care about completion order can see it.
            s.translations.push((line_index, String::new(), true));
        }
    }
    fn on_flow_done(&self, _flow_id: u64, line_count: usize) {
        self.state.lock().unwrap().flow_done_line_count = Some(line_count);
    }
    fn on_mask(&self, flow_id: u64, masked: bool) {
        self.state
            .lock()
            .unwrap()
            .mask_events
            .push((flow_id, masked));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::OcrLine;

    fn white_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    /// The structural invariant the whole refactor enforces: given
    /// the same input + OCR + translator, the pipeline fires
    /// exactly one `on_flow_start`, one `on_cards_ready`, one
    /// translation event per card, and one `on_flow_done` — in
    /// that order. Any future refactor that moves logic back into
    /// a caller (and so changes what the sink observes) breaks
    /// this test. That is the point: divergence becomes a failing
    /// unit test, not a silent behaviour drift.
    #[tokio::test]
    async fn pipeline_fires_sink_callbacks_in_fixed_order() {
        let png = white_png(200, 60);
        let mut input = PreCapturedInput::new(CapturedFrame {
            png,
            width: 200,
            height: 60,
        });
        let sink = RecordingSink::new();
        let ocr = Arc::new(MockLensOcr {
            lines: vec![OcrLine {
                text: "Hello world".into(),
                bbox: [20.0, 20.0, 140.0, 20.0],
                confidence: 1.0,
            }],
        }) as Arc<dyn LensOcr>;
        let translator = Arc::new(EchoLensTranslator) as Arc<dyn LensTranslator>;
        let config = PipelineConfig::default();

        run_lens_pipeline(
            &mut input,
            sink.clone() as Arc<dyn LensSink>,
            ocr,
            translator,
            &config,
            7,
        )
        .await
        .expect("pipeline run succeeds");

        let recorded = sink.snapshot();
        assert!(recorded.flow_start_fired, "on_flow_start must fire");
        assert_eq!(recorded.image_w, 200);
        assert_eq!(recorded.image_h, 60);
        assert_eq!(recorded.cards.len(), 1, "one OCR line → one card");
        assert_eq!(recorded.flow_done_line_count, Some(1));
        // Echo translator forwards input → input becomes the translation.
        let pairs = sink.cards_with_translations();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].1, "Hello world");
    }

    /// Regression: an empty OCR result must still close the flow
    /// (otherwise the UI hangs on its streaming placeholder).
    #[tokio::test]
    async fn pipeline_closes_flow_even_when_ocr_returns_nothing() {
        let png = white_png(80, 40);
        let mut input = PreCapturedInput::new(CapturedFrame {
            png,
            width: 80,
            height: 40,
        });
        let sink = RecordingSink::new();
        let ocr = Arc::new(MockLensOcr { lines: Vec::new() }) as Arc<dyn LensOcr>;
        let translator = Arc::new(EchoLensTranslator) as Arc<dyn LensTranslator>;
        let config = PipelineConfig::default();

        run_lens_pipeline(
            &mut input,
            sink.clone() as Arc<dyn LensSink>,
            ocr,
            translator,
            &config,
            0,
        )
        .await
        .unwrap();

        let recorded = sink.snapshot();
        assert!(recorded.flow_start_fired);
        assert_eq!(recorded.cards.len(), 0);
        assert_eq!(recorded.flow_done_line_count, Some(0));
    }

    /// Regression: the pipeline MUST call `on_cards_ready` BEFORE
    /// emitting any `on_line_chunk`. The frontend depends on this
    /// order to allocate card slots before chunk routing begins —
    /// a divergence here would silently drop the first streaming
    /// tokens of every translation.
    #[tokio::test]
    async fn pipeline_emits_cards_before_chunks() {
        #[derive(Default)]
        struct OrderingSink {
            events: std::sync::Mutex<Vec<&'static str>>,
        }
        impl LensSink for OrderingSink {
            fn on_flow_start(&self, _f: u64) {
                self.events.lock().unwrap().push("flow_start");
            }
            fn on_cards_ready(&self, _f: u64, _w: u32, _h: u32, _cards: &[LensOcrLine]) {
                self.events.lock().unwrap().push("cards_ready");
            }
            fn on_line_chunk(&self, _f: u64, _i: usize, _d: String, _done: bool) {
                self.events.lock().unwrap().push("line_chunk");
            }
            fn on_flow_done(&self, _f: u64, _n: usize) {
                self.events.lock().unwrap().push("flow_done");
            }
        }

        let png = white_png(200, 60);
        let mut input = PreCapturedInput::new(CapturedFrame {
            png,
            width: 200,
            height: 60,
        });
        let sink: Arc<OrderingSink> = Arc::new(OrderingSink::default());
        // Use bbox at 2× coordinate space because pipeline applies
        // inv_scale=0.5 after the upscale preprocess — post-scale
        // the bbox becomes (10..60)×(10..20), plenty of room for a
        // card.
        let ocr = Arc::new(MockLensOcr {
            lines: vec![OcrLine {
                text: "Hello world".into(),
                bbox: [20.0, 20.0, 120.0, 40.0],
                confidence: 1.0,
            }],
        }) as Arc<dyn LensOcr>;
        let translator = Arc::new(EchoLensTranslator) as Arc<dyn LensTranslator>;
        let config = PipelineConfig::default();

        run_lens_pipeline(
            &mut input,
            sink.clone() as Arc<dyn LensSink>,
            ocr,
            translator,
            &config,
            1,
        )
        .await
        .unwrap();

        let events = sink.events.lock().unwrap().clone();
        let cards_idx = events.iter().position(|e| *e == "cards_ready");
        let first_chunk_idx = events.iter().position(|e| *e == "line_chunk");
        assert!(cards_idx.is_some(), "cards_ready must fire, got {events:?}");
        assert!(
            first_chunk_idx.is_some(),
            "line_chunk must fire, got {events:?}"
        );
        assert!(
            cards_idx.unwrap() < first_chunk_idx.unwrap(),
            "cards_ready must fire BEFORE line_chunk — got {events:?}"
        );
    }
}

/// Bridge any [`Provider`](crate::providers::Provider) with
/// `Capability::Ocr` into a [`LensOcr`]. Useful when a single
/// provider is the only OCR option and chain fall-through is not
/// needed (unit tests, minimal harnesses).
pub struct ProviderLensOcr {
    provider: Arc<dyn crate::providers::Provider>,
}

impl ProviderLensOcr {
    pub fn new(provider: Arc<dyn crate::providers::Provider>) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl LensOcr for ProviderLensOcr {
    async fn recognize(&self, image_png: Vec<u8>) -> Result<Vec<OcrLine>> {
        use crate::providers::{TaskInput, TaskOutput};
        let out = self
            .provider
            .run(TaskInput::Ocr {
                image_png,
                languages: vec![],
            })
            .await
            .map_err(|e| anyhow::anyhow!("ocr: {e}"))?;
        match out {
            TaskOutput::Ocr { lines } => Ok(lines),
            _ => anyhow::bail!("ocr provider returned non-Ocr output"),
        }
    }
}

/// Bridge the `Pipeline` chain router into [`LensOcr`] so the
/// production loop keeps its configured fall-through semantics
/// (WinRT → ocrs plugin → ...). Both production and the evaluator
/// go through this adapter so settings-driven chain changes
/// propagate to both at once.
pub struct PipelineLensOcr {
    pipeline: Arc<crate::pipeline::Pipeline>,
}

impl PipelineLensOcr {
    pub fn new(pipeline: Arc<crate::pipeline::Pipeline>) -> Self {
        Self { pipeline }
    }
}

#[async_trait]
impl LensOcr for PipelineLensOcr {
    async fn recognize(&self, image_png: Vec<u8>) -> Result<Vec<OcrLine>> {
        use crate::providers::{Capability, TaskInput, TaskOutput};
        let res = self
            .pipeline
            .run(
                Capability::Ocr,
                TaskInput::Ocr {
                    image_png,
                    languages: vec![],
                },
            )
            .await
            .map_err(|e| anyhow::anyhow!("ocr chain: {e}"))?;
        match res.output {
            TaskOutput::Ocr { lines } => Ok(lines),
            _ => anyhow::bail!("ocr chain returned non-Ocr output"),
        }
    }
}

/// Run translations through the user-configured provider chain.
///
/// Replaces the earlier `OllamaLensTranslator`, which short-circuited
/// straight to `OllamaTranslator::new("ollama.gemma4", ...)` regardless
/// of `providers.translate.chain`. That hard-coded path turned the
/// "local-first, opt-in" provider design into "Ollama is mandatory" —
/// a fresh install with `chain = []` (the design default) had Lens
/// silently failing because the runtime ignored the chain entirely.
///
/// This adapter is the inverse: it carries the active [`Pipeline`]
/// and routes every Lens translation through `Pipeline::run_with_sink`,
/// so whatever providers the user has configured (embedded LLM,
/// plugins, Ollama, DeepL, Gemini, ...) drive the Lens just like they
/// drive the one-shot capture flow.
pub struct ChainBackedLensTranslator {
    pipeline: Arc<crate::pipeline::Pipeline>,
}

impl ChainBackedLensTranslator {
    pub fn new(pipeline: Arc<crate::pipeline::Pipeline>) -> Self {
        Self { pipeline }
    }
}

/// Minimal translator that echoes its input verbatim. Used by
/// integration tests (and any offline scenario) to exercise the
/// pipeline's translation-routing code without needing a live
/// LLM server.
pub struct EchoLensTranslator;

#[async_trait]
impl LensTranslator for EchoLensTranslator {
    async fn translate(
        &self,
        text: String,
        _source_language: String,
        _target_language: String,
        chunk_sink: Arc<dyn Fn(String, bool) + Send + Sync>,
    ) -> Result<String> {
        chunk_sink(text.clone(), true);
        Ok(text)
    }
}

#[async_trait]
impl LensTranslator for ChainBackedLensTranslator {
    async fn translate(
        &self,
        text: String,
        source_language: String,
        target_language: String,
        chunk_sink: Arc<dyn Fn(String, bool) + Send + Sync>,
    ) -> Result<String> {
        self.translate_with_glossary(
            text,
            source_language,
            target_language,
            Vec::new(),
            chunk_sink,
        )
        .await
    }

    async fn translate_with_glossary(
        &self,
        text: String,
        source_language: String,
        target_language: String,
        glossary: Vec<(String, String)>,
        chunk_sink: Arc<dyn Fn(String, bool) + Send + Sync>,
    ) -> Result<String> {
        use crate::providers::{Capability, TaskInput, TaskOutput};
        let res = self
            .pipeline
            .run_with_sink(
                Capability::Translate,
                TaskInput::Translate {
                    text,
                    source_language,
                    target_language,
                    glossary,
                },
                chunk_sink,
            )
            .await
            .map_err(|e| anyhow::anyhow!("translate: {e}"))?;
        match res.output {
            TaskOutput::Text(s) => Ok(s),
            _ => anyhow::bail!("translator returned non-Text output"),
        }
    }

    async fn extract_glossary(
        &self,
        lines: &[&str],
        source_language: &str,
        target_language: &str,
    ) -> Vec<(String, String)> {
        self.pipeline
            .extract_glossary(lines, source_language, target_language)
            .await
    }
}
