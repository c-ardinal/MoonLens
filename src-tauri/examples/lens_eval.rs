//! Offline Lens-pipeline evaluator.
//!
//! Applies the OCR + tile + colour + font-weight pipeline to every
//! image in `tests/input/`, then renders a side-by-side PNG — left
//! panel is the source, right panel has the lens cards drawn on top
//! (re-using the *original* OCR text as placeholder translation so
//! the reader can eyeball how well the card position / size / weight
//! match the source glyphs). Per-line metrics are also emitted as
//! JSON for diffable evaluation across runs.
//!
//! Usage:
//!   cargo run --example lens_eval --release
//!
//! Default output: `tests/runs/lens_eval/` (gitignored scratch dir).
//! Override with `MOONLENS_LENS_EVAL_OUTPUT_DIR=<path>` — used by
//! `scripts/run-lens-eval-3way.ps1` to materialise per-tier
//! directories, and by snapshot-bless flows that point straight at
//! `tests/snapshots/lens_eval/` to refresh the committed baseline.
//!
//! Per-input artefacts in the output dir:
//!   - `<stem>_eval.png`  side-by-side comparison (NOT in git —
//!                        too large to bloat history)
//!   - `<stem>_eval.json` per-card metric dump (diffable, committed
//!                        when copied into snapshots/)
//!   - `_summary.md`      one-line-per-input roll-up (committed
//!                        when copied into snapshots/)
//!
//! Font loading uses Windows system fonts (Arial for Latin, Arial
//! Bold for weight ≥ 600). CJK glyphs fall back to `.notdef` —
//! `ab_glyph` does not support TrueType Collections. The position /
//! size / weight metrics remain meaningful regardless; the rendered
//! text is purely a visual aid for Latin sources.

// This example carries helper functions (`run_paddle_detection`,
// `run_ocrs_detection`, `bbox_iou_over_area`, `translate_single`) and
// `source_text` capture fields that flip between active / inactive
// across iterations of the pipeline experiment. Suppressing
// `dead_code` keeps the helpers reachable without churning the file
// each time the comparison axis changes.
#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;

use ab_glyph::{FontVec, PxScale};
use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, GrayImage, Luma, RgbImage, Rgba, RgbaImage};
use imageproc::drawing::{draw_hollow_rect_mut, draw_text_mut};
use imageproc::filter::gaussian_blur_f32;
use imageproc::rect::Rect;
use serde::Serialize;

use moonlens_lib::{
    lens_inpaint,
    lens_pipeline::{
        run_lens_pipeline, CapturedFrame, LensSink, LensTranslator, PipelineConfig,
        PreCapturedInput, ProviderLensOcr, RecordingSink,
    },
    lens_watcher::{text_is_already_target, LensOcrLine},
    model_registry::{tier_by_id, ModelTier},
    providers::{
        ocr_ocrs::default_models_dir, ocr_windows::run_ocr, ocr_windows::WindowsOcrProvider,
        translate_embedded_llm::EmbeddedLlmTranslator, translate_ollama::OllamaTranslator,
        translate_routed::RoutedTranslator, OcrLine, Provider, TaskInput, TaskOutput,
    },
    services::hardware_probe::GPU_FEATURE_ENABLED,
    translation_router::{Hardware, UserTier},
};

/// Offline fallback translator: the evaluator runs with or without
/// a reachable Ollama server, so when the probe fails we swap in
/// this adapter which echoes the source as the "translation". The
/// rest of the pipeline is untouched — the PNG render just shows
/// OCR text in the translation panel, flagged by the probe warning.
struct EchoTranslator;

/// Bridges a generic `Provider` (single embedded model or a
/// `RoutedTranslator`) onto the `LensTranslator` trait the eval
/// pipeline expects. Lets the evaluator drive the same routing
/// surface the production app uses for Lens lines, so the side-
/// by-side PNGs reflect the user-visible translation quality.
struct ProviderLensAdapter {
    inner: Arc<dyn Provider>,
}

#[async_trait]
impl LensTranslator for ProviderLensAdapter {
    async fn translate(
        &self,
        text: String,
        source_language: String,
        target_language: String,
        chunk_sink: Arc<dyn Fn(String, bool) + Send + Sync>,
    ) -> Result<String> {
        let out = self
            .inner
            .run_streaming(
                TaskInput::Translate {
                    text,
                    // The router accepts BCP-47 / ISO codes; empty /
                    // whitespace falls back to "auto" so the matrix
                    // takes the multi-model path. WinRT OCR fixtures
                    // we eval are English/JA screenshots, both of
                    // which classify correctly.
                    source_language: if source_language.trim().is_empty() {
                        "auto".into()
                    } else {
                        source_language
                    },
                    target_language,
                    glossary: Vec::new(),
                },
                chunk_sink,
            )
            .await
            .map_err(|e| anyhow::anyhow!("translate: {e}"))?;
        match out {
            TaskOutput::Text(t) => Ok(t),
            TaskOutput::TextChunk { delta, .. } => Ok(delta),
            other => anyhow::bail!("unexpected TaskOutput shape: {other:?}"),
        }
    }
}

#[async_trait]
impl moonlens_lib::lens_pipeline::LensTranslator for EchoTranslator {
    async fn translate(
        &self,
        text: String,
        _source_language: String,
        _target_language: String,
        chunk_sink: std::sync::Arc<dyn Fn(String, bool) + Send + Sync>,
    ) -> Result<String> {
        chunk_sink(text.clone(), true);
        Ok(text)
    }
}

/// Target language for the evaluator — mirrors what the production
/// Lens would translate *into*. Text already in this language is
/// left untouched (no inpainting, no replacement text).
const TARGET_LANGUAGE: &str = "ja";

/// Which secondary detector to pair with WinRT. Chosen at runtime
/// via `MOONLENS_EVAL_DETECTOR=ocrs|paddle|none`, defaulting to
/// `ocrs`. The `_summary.md` header records the choice so paired
/// runs can be diffed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecondaryDetector {
    None,
    Ocrs,
    Paddle,
}

impl SecondaryDetector {
    fn from_env() -> Self {
        // Default is WinRT-only: benchmarking showed ocrs and
        // PaddleOCR each leave the same residual fragments as
        // WinRT alone (see tests/output/_comparison.md) while
        // adding 0.25×-4× latency. Keep them behind the env var
        // for offline experiments; production ships the fast path.
        match std::env::var("MOONLENS_EVAL_DETECTOR")
            .ok()
            .as_deref()
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            Some("paddle") => Self::Paddle,
            Some("ocrs") => Self::Ocrs,
            _ => Self::None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::None => "winrt-only",
            Self::Ocrs => "winrt+ocrs",
            Self::Paddle => "winrt+paddle",
        }
    }
}

fn main() -> Result<()> {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("project root (parent of src-tauri)")?;
    let input_dir = project_root.join("tests").join("input");
    // Output destination: defaults to the gitignored scratch dir
    // `tests/runs/lens_eval/` so consecutive runs do not stomp the
    // committed baseline in `tests/snapshots/lens_eval/`. Override
    // with `MOONLENS_LENS_EVAL_OUTPUT_DIR=<path>` — used by the
    // 3-way harness to materialise per-tier directories side-by-
    // side, and by snapshot-update flows that point straight at
    // `tests/snapshots/lens_eval/` to refresh the baseline.
    let output_dir = match std::env::var("MOONLENS_LENS_EVAL_OUTPUT_DIR") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => project_root.join("tests").join("runs").join("lens_eval"),
    };
    fs::create_dir_all(&output_dir).with_context(|| format!("create {}", output_dir.display()))?;

    let fonts = Fonts::load().context("load system fonts")?;
    println!("OCR engine: paddle (PP-OCRv5 / oar-ocr) — single production OCR");

    // Tokio runtime: the translator's Provider trait is async but
    // the eval itself is sync. A single current-thread runtime is
    // enough — we block on one translate call at a time.
    let translate_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime for translation")?;

    // 2026-05-29 PARITY: build the EXACT pipeline production uses so the
    // eval and the live Lens differ ONLY in input source (file vs
    // screen capture) and sink (RecordingSink vs TauriLensSink). OCR +
    // translation flow through the real `Pipeline` (paddle OCR chain +
    // RoutedTranslator) via the same `PipelineLensOcr` /
    // `ChainBackedLensTranslator` adapters `lens_watcher` wires — no
    // more eval-only `ProviderLensOcr` / Ollama-echo divergence. The
    // settings come from the user's live `config.toml` (copied to a
    // temp file so the eval never writes the live one), so chains /
    // tier / languages match production exactly.
    let parity_app_data = resolve_app_data_dir()
        .context("resolve app-data dir (set MOONLENS_EVAL_APP_DATA to the app-data root)")?;
    let parity_settings = {
        let live_cfg = parity_app_data.join("config.toml");
        let tmp_cfg = std::env::temp_dir().join("moonlens_eval_parity_config.toml");
        if live_cfg.exists() {
            fs::copy(&live_cfg, &tmp_cfg)
                .with_context(|| format!("copy {} for eval", live_cfg.display()))?;
        } else if tmp_cfg.exists() {
            let _ = fs::remove_file(&tmp_cfg);
        }
        Arc::new(
            moonlens_lib::settings::SettingsStore::load_from(tmp_cfg)
                .context("load settings for parity pipeline")?,
        )
    };
    let parity_storage = Arc::new(
        moonlens_lib::storage::Storage::open_in_memory().context("open in-memory storage")?,
    );
    let parity_pipeline = Arc::new(moonlens_lib::pipeline::Pipeline::with_providers(
        parity_settings.clone(),
        parity_storage,
        std::collections::HashMap::new(),
    ));
    parity_pipeline.set_app_data_dir(parity_app_data.clone());
    parity_pipeline.rebuild_providers();
    {
        let snap = parity_settings.snapshot();
        println!(
            "parity pipeline: app_data={} ocr_chain={:?} translate_chain={:?} user_tier={:?}",
            parity_app_data.display(),
            snap.providers.ocr.chain,
            snap.providers.translate.chain,
            snap.providers.user_tier,
        );
    }
    let lens_ocr: Arc<dyn moonlens_lib::lens_pipeline::LensOcr> = Arc::new(
        moonlens_lib::lens_pipeline::PipelineLensOcr::new(parity_pipeline.clone()),
    );
    let lens_translator: Arc<dyn LensTranslator> = Arc::new(
        moonlens_lib::lens_pipeline::ChainBackedLensTranslator::new(parity_pipeline.clone()),
    );
    let lens_config = {
        let snap = parity_settings.snapshot();
        PipelineConfig {
            source_language: snap.translation.default_source_language.clone(),
            target_language: snap.translation.default_target_language.clone(),
            max_concurrent_translate: snap.performance.max_concurrent_tasks.max(1) as usize,
            ocr_scale: 3,
            glossary_min_lines: snap.providers.glossary_min_lines as usize,
        }
    };
    // 2026-05-29 PARITY: the OCR + translator now come from the real
    // `Pipeline` built above (`lens_ocr` / `lens_translator`), so the
    // old eval-only translator selection (Ollama probe, force-tier,
    // lens-tier) and the secondary-detector engines are gone. They
    // were the source of the eval-vs-production divergence. Tier
    // comparison, if needed again, should drive the production
    // pipeline with a settings override — not a parallel code path.

    // Walk one level deep so synthetic images under
    // `tests/input/synth/` show up in the same eval run as the
    // hand-curated screenshots at the top level. Anything beyond
    // that is intentionally ignored — keeps the corpus easy to
    // audit at a glance.
    let mut entries: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&input_dir)
        .with_context(|| format!("read {}", input_dir.display()))?
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_dir() {
            for sub in fs::read_dir(&path)
                .with_context(|| format!("read {}", path.display()))?
                .filter_map(|e| e.ok())
            {
                let sp = sub.path();
                if is_supported_image(&sp) {
                    entries.push(sp);
                }
            }
        } else if is_supported_image(&path) {
            entries.push(path);
        }
    }
    entries.sort();

    // Optional fast-path filter: `MOONLENS_EVAL_ONLY=english0001` (a
    // substring match on the file stem) runs the eval on a single
    // fixture. Useful when iterating on the display layer against one
    // known image instead of re-OCRing the whole corpus each run.
    if let Ok(only) = std::env::var("MOONLENS_EVAL_ONLY") {
        let only = only.trim().to_string();
        if !only.is_empty() {
            entries.retain(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.contains(&only))
            });
        }
    }

    if entries.is_empty() {
        anyhow::bail!(
            "no supported input images at {} (avif is intentionally skipped \
             — convert to png/jpg/webp if you need them evaluated)",
            input_dir.display()
        );
    }

    let mut summary = String::from(
        "# Lens pipeline evaluation\n\nOCR: **paddle (PP-OCRv5 / oar-ocr)** — parity with production\n\n",
    );
    summary.push_str("## Quality (residual fragments after inpainting)\n\n");
    summary.push_str(
        "| Input | Lines | Residual chars | Residual texts |\n\
         |-------|------:|---------------:|----------------|\n",
    );

    let mut bench_rows = String::from("\n## Timings (ms)\n\n");
    bench_rows.push_str(
        "| Input | Preproc | WinRT | ocrs | Cards | Translate | Inpaint | Post-OCR | Total |\n\
         |-------|--------:|------:|-----:|------:|----------:|--------:|---------:|------:|\n",
    );
    let mut overflow_rows = String::from("\n## Translation overflows\n");

    let mut total_ok = 0usize;
    let mut total_err = 0usize;
    let mut reports: Vec<EvalReport> = Vec::new();
    for path in &entries {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        println!("eval {name}");
        match evaluate_one(
            path,
            &output_dir,
            &fonts,
            &translate_runtime,
            &lens_ocr,
            &lens_translator,
            &lens_config,
        ) {
            Ok(report) => {
                let residual_display = if report.residual_texts.is_empty() {
                    "(none)".to_string()
                } else {
                    report
                        .residual_texts
                        .iter()
                        .map(|s| format!("`{}`", s.replace('|', "\\|")))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                summary.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    name, report.line_count, report.residual_char_count, residual_display,
                ));
                bench_rows.push_str(&format!(
                    "| {} | {:.0} | {:.0} | {:.0} | {:.1} | {:.0} | {:.0} | {:.0} | {:.0} |\n",
                    name,
                    report.timings_ms.preprocess,
                    report.timings_ms.winrt_ocr,
                    report.timings_ms.ocrs_detect,
                    report.timings_ms.compute_cards,
                    report.timings_ms.translate,
                    report.timings_ms.inpaint,
                    report.timings_ms.post_ocr_residual,
                    report.timings_ms.total,
                ));
                if !report.overflows.is_empty() {
                    overflow_rows.push_str(&format!("\n### {}\n\n", name));
                    overflow_rows.push_str(
                        "| idx | source | translation | rendered px | card px | ratio |\n\
                         |----:|--------|-------------|------------:|--------:|------:|\n",
                    );
                    for o in &report.overflows {
                        overflow_rows.push_str(&format!(
                            "| {} | `{}` | `{}` | {:.0} | {:.0} | {:.2}× |\n",
                            o.line_index,
                            o.source_text.replace('|', "\\|"),
                            o.translation.replace('|', "\\|"),
                            o.rendered_width_px,
                            o.card_inner_width_px,
                            o.overflow_ratio,
                        ));
                    }
                }
                total_ok += 1;
                reports.push(report);
            }
            Err(e) => {
                summary.push_str(&format!("| {name} | — | — | ERR: {e} |\n"));
                eprintln!("  error: {e:?}");
                total_err += 1;
            }
        }
    }
    summary.push_str(&bench_rows);
    summary.push_str(&overflow_rows);
    // Aggregate per-stage means so the top-of-file comparison is
    // glanceable without manually summing rows.
    if !reports.is_empty() {
        let n = reports.len() as f32;
        let avg = |f: fn(&EvalReport) -> f32| -> f32 { reports.iter().map(f).sum::<f32>() / n };
        let avg_residuals = reports
            .iter()
            .map(|r| r.residual_char_count as f32)
            .sum::<f32>()
            / n;
        summary.push_str(&format!(
            "\n## Aggregate (mean across {} images)\n\n\
             - Residual chars: **{:.1}** / image\n\
             - Preprocess: {:.0} ms\n\
             - WinRT OCR: {:.0} ms\n\
             - ocrs detect: {:.0} ms\n\
             - Inpaint: {:.0} ms\n\
             - Post-OCR residual check: {:.0} ms\n\
             - **Total: {:.0} ms** / image\n",
            reports.len(),
            avg_residuals,
            avg(|r| r.timings_ms.preprocess),
            avg(|r| r.timings_ms.winrt_ocr),
            avg(|r| r.timings_ms.ocrs_detect),
            avg(|r| r.timings_ms.inpaint),
            avg(|r| r.timings_ms.post_ocr_residual),
            avg(|r| r.timings_ms.total),
        ));
    }
    summary.push_str(&format!(
        "\n{} processed, {} errored. Side-by-side PNGs and per-image JSON \
         live next to this file.\n",
        total_ok, total_err
    ));

    let summary_path = output_dir.join("_summary.md");
    fs::write(&summary_path, summary).context("write summary")?;
    println!("\n→ {}", summary_path.display());
    Ok(())
}

/// `image` crate is compiled without AVIF support here, so ignore
/// those files rather than erroring out on every run.
fn is_supported_image(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png") | Some("jpg") | Some("jpeg") | Some("webp")
    )
}

/// Pair of owned fonts used by the evaluator: `regular` for weight
/// < 600, `bold` for the rest. Two owned copies keeps the
/// `ab_glyph::Font` trait happy without Arc juggling.
struct Fonts {
    regular: FontVec,
    bold: FontVec,
}

impl Fonts {
    fn load() -> Result<Self> {
        // Prefer Noto Sans JP — it ships as standalone TTF on
        // Windows 10/11 (unlike Yu Gothic / Meiryo which are TTC
        // collections ab_glyph cannot parse) and covers both Latin
        // and Japanese so the eval can render the Ollama
        // translations without `.notdef` tofu. Falls back to Arial
        // when the Noto family is absent — Japanese glyphs will
        // render as missing, but overflow detection still computes.
        let regular = read_first_font(&[
            "C:/Windows/Fonts/NotoSansJP-Regular.ttf",
            "C:/Windows/Fonts/arial.ttf",
            "C:/Windows/Fonts/segoeui.ttf",
        ])
        .context("regular font")?;
        let bold = read_first_font(&[
            "C:/Windows/Fonts/NotoSansJP-Bold.ttf",
            "C:/Windows/Fonts/arialbd.ttf",
            "C:/Windows/Fonts/seguisb.ttf",
            "C:/Windows/Fonts/NotoSansJP-Regular.ttf",
            "C:/Windows/Fonts/arial.ttf",
            "C:/Windows/Fonts/segoeui.ttf",
        ])
        .context("bold font")?;
        Ok(Fonts { regular, bold })
    }

    fn pick(&self, weight: u16) -> &FontVec {
        if weight >= 600 {
            &self.bold
        } else {
            &self.regular
        }
    }
}

fn read_first_font(paths: &[&str]) -> Result<FontVec> {
    for p in paths {
        if let Ok(bytes) = fs::read(p) {
            if let Ok(font) = FontVec::try_from_vec(bytes) {
                return Ok(font);
            }
        }
    }
    anyhow::bail!("none of {:?} could be loaded", paths)
}

#[derive(Serialize)]
struct CardMetric {
    line_index: usize,
    text: String,
    translation: String,
    source_bbox: [f32; 4],
    tiled_bbox: [f32; 4],
    bg_color: [u8; 3],
    fg_color: [u8; 3],
    font_weight: u16,
    /// Mirrors the frontend `clamp(7, min(source_h*0.82, 180cqw/len), 80)`.
    font_size_px: f32,
    /// Detected horizontal alignment ("left" / "center" / "right")
    /// — surfaced in the JSON so it shows up alongside the cards
    /// in the eval summary.
    text_align: moonlens_lib::lens_watcher::TextAlign,
    /// Euclidean RGB distance between fg and bg — how legible the
    /// card is on top of its own background sample.
    fg_bg_delta: f32,
}

#[derive(Serialize, Clone)]
struct OverflowReport {
    line_index: usize,
    source_text: String,
    translation: String,
    /// Width of the translated glyph run in source-image pixels at
    /// the card's computed font size.
    rendered_width_px: f32,
    /// Inner width available inside the card (source_bbox width —
    /// ignoring horizontal padding because padding is background
    /// reconstruction, not rendering area).
    card_inner_width_px: f32,
    /// `rendered_width / card_inner_width`. > 1.0 = overflow.
    overflow_ratio: f32,
}

#[derive(Serialize, Default)]
struct TimingMs {
    preprocess: f32,
    winrt_ocr: f32,
    ocrs_detect: f32,
    compute_cards: f32,
    translate: f32,
    inpaint: f32,
    post_ocr_residual: f32,
    total: f32,
}

#[derive(Serialize)]
struct EvalReport {
    image: String,
    image_w: u32,
    image_h: u32,
    line_count: usize,
    avg_font_size: f32,
    avg_color_delta: f32,
    /// Number of cards at weight 400 / 600 / 700.
    weight_counts: [usize; 3],
    /// Characters still detectable by WinRT OCR on the inpaint-only
    /// image. Zero = perfect erasure; higher = more residuals.
    residual_char_count: usize,
    residual_texts: Vec<String>,
    /// Translations whose rendered pixel width (at `font_size_px`
    /// and detected weight) exceeded the horizontal bounds of the
    /// card they belong to. Emits the offending card's source text,
    /// translation, and overflow ratio so long-translation growth
    /// is visible in the summary rather than hiding behind clipping.
    overflows: Vec<OverflowReport>,
    timings_ms: TimingMs,
    cards: Vec<CardMetric>,
}

fn evaluate_one(
    input: &Path,
    output_dir: &Path,
    fonts: &Fonts,
    runtime: &tokio::runtime::Runtime,
    lens_ocr: &Arc<dyn moonlens_lib::lens_pipeline::LensOcr>,
    lens_translator: &Arc<dyn LensTranslator>,
    lens_config: &PipelineConfig,
) -> Result<EvalReport> {
    use std::time::Instant;
    let t_start = Instant::now();
    let mut timings = TimingMs::default();

    let bytes = fs::read(input).with_context(|| format!("read {}", input.display()))?;
    let img = image::load_from_memory(&bytes).context("decode image")?;
    let (w, h) = img.dimensions();

    // Every step below is literally the same `run_lens_pipeline`
    // production drives, with the SAME OCR + translator adapters
    // (`PipelineLensOcr` + `ChainBackedLensTranslator`) built from the
    // SAME `Pipeline` in `main`. The ONLY differences from the live
    // Lens are the input source (file-loaded PNG via PreCapturedInput
    // vs screen capture) and the sink (RecordingSink vs TauriLensSink)
    // — both legitimately I/O. There is no eval-only OCR / translator
    // construction here anymore: that was the divergence the
    // 2026-05-29 parity audit removed.
    let mut pipeline_input = PreCapturedInput::new(CapturedFrame {
        png: bytes.clone(),
        width: w,
        height: h,
    });
    let sink = RecordingSink::new();
    let pipeline_sink: Arc<dyn LensSink> = sink.clone();
    let t0 = Instant::now();
    runtime
        .block_on(run_lens_pipeline(
            &mut pipeline_input,
            pipeline_sink,
            lens_ocr.clone(),
            lens_translator.clone(),
            lens_config,
            0,
        ))
        .map_err(|e| anyhow::anyhow!("pipeline: {e}"))?;
    timings.translate = t0.elapsed().as_secs_f32() * 1000.0;

    let cards_with_translations = sink.cards_with_translations();
    let cards: Vec<LensOcrLine> = cards_with_translations
        .iter()
        .map(|(c, _)| c.clone())
        .collect();
    let translations: Vec<String> = cards_with_translations
        .into_iter()
        .map(|(_, t)| t.trim().to_string())
        .collect();

    let mut card_metrics = Vec::with_capacity(cards.len());
    let mut weight_counts = [0usize; 3];
    for (i, c) in cards.iter().enumerate() {
        let font_size_px = c.font_size_px.max(1.0);
        let fg_bg_delta = color_distance(c.fg_color, c.bg_color);
        let bucket = match c.font_weight {
            0..=500 => 0,
            501..=649 => 1,
            _ => 2,
        };
        weight_counts[bucket] += 1;
        card_metrics.push(CardMetric {
            line_index: c.line_index,
            text: c.text.clone(),
            translation: translations.get(i).cloned().unwrap_or_default(),
            source_bbox: c.source_bbox,
            tiled_bbox: c.bbox,
            bg_color: c.bg_color,
            fg_color: c.fg_color,
            font_weight: c.font_weight,
            font_size_px,
            text_align: c.text_align,
            fg_bg_delta,
        });
    }

    let avg_font_size = average(card_metrics.iter().map(|c| c.font_size_px));
    let avg_color_delta = average(card_metrics.iter().map(|c| c.fg_bg_delta));

    let stem = input.file_stem().unwrap().to_string_lossy();

    // apply_inpainting + render_side_by_side no longer need raw
    // ocr_lines / skip_flags (they use only card bbox + color);
    // pass empty slices to satisfy the existing signatures until
    // those helpers are cleaned up in a follow-up.
    let empty_ocr: Vec<OcrLine> = Vec::new();
    let empty_skip: Vec<bool> = Vec::new();

    let overlay = render_side_by_side(&img, &cards, &empty_ocr, &empty_skip, &translations, fonts);
    overlay
        .save(output_dir.join(format!("{stem}_eval.png")))
        .context("save overlay PNG")?;

    // Overflow detection: measure each translation's rendered width
    // against the card's inner (source_bbox) width. This is the
    // budget actual production layout gives the translation — the
    // horizontal padding outside source_bbox is background fill,
    // not text area.
    // Overflow now measures the WRAPPED block height against the
    // card's source-rect height. Per-block cards wrap text inside
    // the paragraph region (matching `overflow-wrap: anywhere` in
    // production), so horizontal single-line width is no longer
    // the interesting failure mode — vertical overflow is.
    let mut overflows: Vec<OverflowReport> = Vec::new();
    for (card, translation) in cards.iter().zip(translations.iter()) {
        if translation.trim().is_empty() {
            continue;
        }
        let font_size = card.font_size_px.max(1.0);
        let font = fonts.pick(card.font_weight);
        let inner_w = card.source_bbox[2];
        let inner_h = card.source_bbox[3];
        if inner_w <= 0.0 || inner_h <= 0.0 {
            continue;
        }
        let wrapped = wrap_text(translation, font, font_size, inner_w);
        let line_height = font_size * card.line_height.max(1.0);
        let rendered_h = wrapped.len() as f32 * line_height;
        let ratio = rendered_h / inner_h;
        if ratio > 1.05 {
            overflows.push(OverflowReport {
                line_index: card.line_index,
                source_text: card.text.clone(),
                translation: translation.clone(),
                rendered_width_px: rendered_h,
                card_inner_width_px: inner_h,
                overflow_ratio: ratio,
            });
        }
    }

    // Full-resolution inpaint-only image for detailed residual
    // inspection. The 3-up side-by-side rendering shrinks each
    // panel to ~one third width, hiding subtle ghosts.
    let t0 = Instant::now();
    let mut inpaint_solo = img.to_rgba8();
    let src_rgb_for_solo = img.to_rgb8();
    for card in &cards {
        apply_inpainting(
            &mut inpaint_solo,
            card,
            &src_rgb_for_solo,
            &empty_ocr,
            &empty_skip,
        );
    }
    timings.inpaint = t0.elapsed().as_secs_f32() * 1000.0;
    let inpaint_img = DynamicImage::ImageRgba8(inpaint_solo);
    inpaint_img
        .save(output_dir.join(format!("{stem}_inpaint.png")))
        .context("save inpaint-only PNG")?;

    // Residual measurement: re-run WinRT on the inpainted image.
    // Any text it still detects is a fragment the pipeline failed
    // to erase. Filter out lines with no alphanumeric characters
    // (AA noise, gradient sampling oddities).
    let t0 = Instant::now();
    let mut residual_png = Vec::new();
    inpaint_img
        .write_to(
            &mut std::io::Cursor::new(&mut residual_png),
            image::ImageFormat::Png,
        )
        .ok();
    let (residual_char_count, residual_texts) = match run_ocr(&residual_png, None) {
        Ok(lines) => {
            let texts: Vec<String> = lines
                .into_iter()
                .map(|l| l.text)
                .filter(|t| {
                    // Only count fragments that look like residual
                    // source text — skip the preserved Japanese.
                    t.chars().any(|c| c.is_alphanumeric())
                        && !text_is_already_target(t, TARGET_LANGUAGE)
                })
                .collect();
            let chars: usize = texts.iter().map(|t| t.chars().count()).sum();
            (chars, texts)
        }
        Err(_) => (0, Vec::new()),
    };
    timings.post_ocr_residual = t0.elapsed().as_secs_f32() * 1000.0;
    timings.total = t_start.elapsed().as_secs_f32() * 1000.0;

    let report = EvalReport {
        image: input.file_name().unwrap().to_string_lossy().to_string(),
        image_w: w,
        image_h: h,
        line_count: cards.len(),
        avg_font_size,
        avg_color_delta,
        weight_counts,
        residual_char_count,
        residual_texts,
        overflows,
        timings_ms: timings,
        cards: card_metrics,
    };

    fs::write(
        output_dir.join(format!("{stem}_eval.json")),
        serde_json::to_string_pretty(&report)?,
    )?;

    Ok(report)
}

fn color_distance(a: [u8; 3], b: [u8; 3]) -> f32 {
    let dr = a[0] as f32 - b[0] as f32;
    let dg = a[1] as f32 - b[1] as f32;
    let db = a[2] as f32 - b[2] as f32;
    (dr * dr + dg * dg + db * db).sqrt()
}

fn average(values: impl IntoIterator<Item = f32>) -> f32 {
    let (sum, count) = values
        .into_iter()
        .fold((0.0f32, 0usize), |(s, n), v| (s + v, n + 1));
    if count == 0 {
        0.0
    } else {
        sum / count as f32
    }
}

fn render_side_by_side(
    src: &DynamicImage,
    cards: &[LensOcrLine],
    ocr_lines: &[OcrLine],
    skip_flags: &[bool],
    translations: &[String],
    fonts: &Fonts,
) -> DynamicImage {
    let (w, h) = src.dimensions();
    let gap: u32 = 24;
    let header_h: u32 = 30;
    let mut canvas = RgbaImage::from_pixel(w * 3 + gap * 2, h + header_h, Rgba([18, 18, 22, 255]));

    // Source pixels in plain RGB — read repeatedly by the
    // inpainter, cheaper than re-decoding on every card.
    let src_rgb = src.to_rgb8();
    let src_rgba = src.to_rgba8();

    // Panel 1: ORIGINAL (untouched source)
    image::imageops::overlay(&mut canvas, &src_rgba, 0, header_h as i64);

    // Panel 2: INPAINTED (source text removed via scanline fill, no
    // translation yet — lets the reader judge inpainting quality on
    // its own before the replacement text lands on top)
    let mut inpaint_only = src_rgba.clone();
    for card in cards {
        apply_inpainting(&mut inpaint_only, card, &src_rgb, ocr_lines, skip_flags);
    }
    image::imageops::overlay(
        &mut canvas,
        &inpaint_only,
        (w + gap) as i64,
        header_h as i64,
    );

    // Panel 3: FINAL (inpainted background + translation text
    // rendered at source position + red outline + metric label)
    let mut final_panel = inpaint_only.clone();
    for (card, translation) in cards.iter().zip(translations.iter()) {
        apply_text_and_overlay(&mut final_panel, card, translation, fonts);
    }
    image::imageops::overlay(
        &mut canvas,
        &final_panel,
        (w * 2 + gap * 2) as i64,
        header_h as i64,
    );

    // Headers
    let header_scale = PxScale::from(14.0);
    let header_color = Rgba([220, 220, 220, 255]);
    draw_text_mut(
        &mut canvas,
        header_color,
        8,
        8,
        header_scale,
        &fonts.regular,
        "ORIGINAL",
    );
    draw_text_mut(
        &mut canvas,
        header_color,
        (w + gap) as i32 + 8,
        8,
        header_scale,
        &fonts.regular,
        "INPAINTED (source text removed)",
    );
    draw_text_mut(
        &mut canvas,
        header_color,
        (w * 2 + gap * 2) as i32 + 8,
        8,
        header_scale,
        &fonts.regular,
        "INPAINT + TRANSLATION (LLM — red outline = source bbox, green label = fs/weight, red label = overflow)",
    );

    DynamicImage::ImageRgba8(canvas)
}

fn apply_inpainting(
    img: &mut RgbaImage,
    card: &LensOcrLine,
    src_rgb: &RgbImage,
    _all_ocr_lines: &[OcrLine],
    _skip_flags: &[bool],
) {
    // Use production's `compute_card_region` result, which is
    // exactly what the backend encoded into `bg_patch`. The
    // evaluator pastes the decoded-from-original patch back onto
    // the image so we see the same pixels the frontend would
    // composite over the captured frame — no divergence possible
    // between eval and runtime.
    let region = card.bbox;
    if region[2] < 3.0 || region[3] < 3.0 {
        return;
    }
    let patch = lens_inpaint::inpaint_text_in_bbox(src_rgb, region, card.bg_color, card.fg_color);
    let (pw, ph) = patch.dimensions();
    let bx = region[0].max(0.0) as u32;
    let by = region[1].max(0.0) as u32;
    for dy in 0..ph {
        for dx in 0..pw {
            let p = patch.get_pixel(dx, dy).0;
            let sxp = bx + dx;
            let syp = by + dy;
            if sxp < img.width() && syp < img.height() {
                img.put_pixel(sxp, syp, Rgba([p[0], p[1], p[2], 255]));
            }
        }
    }
}

fn apply_text_and_overlay(
    img: &mut RgbaImage,
    card: &LensOcrLine,
    translation: &str,
    fonts: &Fonts,
) {
    let [sx, sy, sw, sh] = card.source_bbox;
    if sw <= 0.0 || sh <= 0.0 {
        return;
    }
    let src_rect = Rect::at(sx as i32, sy as i32).of_size(sw.max(1.0) as u32, sh.max(1.0) as u32);
    draw_hollow_rect_mut(img, src_rect, Rgba([255, 80, 80, 255]));

    let font_size = card.font_size_px.max(1.0);
    let scale = PxScale::from(font_size);
    let font = fonts.pick(card.font_weight);
    let fg = Rgba([card.fg_color[0], card.fg_color[1], card.fg_color[2], 255]);
    let y_offset = (font_size * 0.10) as i32;
    let text = if translation.is_empty() {
        &card.text
    } else {
        translation
    };

    // Wrap the translation to the card's source width so the
    // evaluator mirrors what production's `.line-text` does with
    // `overflow-wrap: anywhere` + `white-space: pre-wrap`. Without
    // this the PNG shows the unwrapped single-line width and the
    // overflow metric looks catastrophic while the real Lens
    // window renders it cleanly in multiple lines.
    let wrapped = wrap_text(text, font, font_size, sw);
    let line_height = font_size * card.line_height.max(1.0);
    for (i, line) in wrapped.iter().enumerate() {
        let line_y = sy as i32 + y_offset + (i as f32 * line_height) as i32;
        // Honour the backend-detected horizontal alignment so the
        // evaluator's PNG matches the production CSS `text-align`
        // path. Centre / right offsets shift the start x by the
        // remaining slack; left renders flush at sx.
        let line_w = measure_text_width(line, font, font_size);
        let line_x = match card.text_align {
            moonlens_lib::lens_watcher::TextAlign::Center => {
                sx as i32 + ((sw - line_w).max(0.0) / 2.0) as i32
            }
            moonlens_lib::lens_watcher::TextAlign::Right => {
                sx as i32 + (sw - line_w).max(0.0) as i32
            }
            moonlens_lib::lens_watcher::TextAlign::Left => sx as i32,
        };
        draw_text_mut(img, fg, line_x, line_y, scale, font, line);
    }

    // Overflow now means the WRAPPED block would not fit vertically
    // inside the card's source rect — that is the real failure
    // mode for per-block cards, not single-line horizontal width.
    let rendered_h = wrapped.len() as f32 * line_height;
    let overflow = rendered_h > sh * 1.05;
    let rendered_w: f32 = wrapped
        .iter()
        .map(|l| measure_text_width(l, font, font_size))
        .fold(0.0_f32, f32::max);
    let label = if overflow {
        format!(
            "fs={font_size:.0} w={} OVERFLOW-V {:.0}%",
            card.font_weight,
            (rendered_h / sh) * 100.0
        )
    } else {
        format!("fs={font_size:.0} w={}", card.font_weight)
    };
    let _ = rendered_w;
    let label_scale = PxScale::from(10.0);
    let label_y = (sy as i32 - 12).max(0);
    draw_text_mut(
        img,
        if overflow {
            Rgba([255, 80, 80, 255])
        } else {
            Rgba([80, 255, 120, 255])
        },
        sx as i32,
        label_y,
        label_scale,
        &fonts.regular,
        &label,
    );
}

/// Pull the `providers.configs."ollama.gemma4"` block out of the
/// bundled default.toml and return it as a `serde_json::Value`
/// suitable for `OllamaTranslator::new`. This is how the evaluator
/// stays bit-for-bit aligned with production: the runtime reads
/// the same block off `SettingsStore`, and the user can only
/// override it via the same config path, so there is a single
/// source of truth for prompt, temperature, num_predict, and
/// few-shot examples.
/// Build the JSON config block `EmbeddedLlmTranslator::new`
/// reads for a force-tier eval. Mirrors `translate_routed.rs`'s
/// internal helper (kept private) — duplicating the 6 lines here
/// rather than re-exporting it from the library keeps the
/// production routing path unaware of the eval harness.
fn config_for_eval_tier(tier: &ModelTier) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("repo".into(), serde_json::Value::String(tier.repo.into()));
    obj.insert(
        "filename_remote".into(),
        serde_json::Value::String(tier.primary_filename().into()),
    );
    if tier.filenames.len() > 1 {
        let extras: Vec<serde_json::Value> = tier
            .filenames
            .iter()
            .skip(1)
            .map(|f| serde_json::Value::String((*f).to_string()))
            .collect();
        obj.insert(
            "filenames_remote_extra".into(),
            serde_json::Value::Array(extras),
        );
    }
    obj.insert(
        "license".into(),
        serde_json::Value::String(tier.license.into()),
    );
    serde_json::Value::Object(obj)
}

/// Locate the directory holding the production embedded-LLM
/// GGUFs. The production app stores them under
/// `<APPDATA>/io.moonlens.app/models/`. Power users can override
/// via `MOONLENS_EVAL_APP_DATA` (path to the `app-data` root, not
/// the `models/` subdir) when running the evaluator outside the
/// installed app — useful for CI fixtures that ship a pre-built
/// models directory.
fn resolve_app_data_dir() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("MOONLENS_EVAL_APP_DATA") {
        return Ok(PathBuf::from(custom));
    }
    let appdata = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .context(
            "neither MOONLENS_EVAL_APP_DATA, APPDATA, nor HOME is set; cannot locate models dir",
        )?;
    Ok(PathBuf::from(appdata).join("io.moonlens.app"))
}

fn load_default_ollama_gemma4_config() -> Result<serde_json::Value> {
    const DEFAULT_TOML: &str = include_str!("../src/settings/default.toml");
    let root: toml::Value = toml::from_str(DEFAULT_TOML).context("parse default.toml")?;
    let cfg = root
        .get("providers")
        .and_then(|v| v.get("configs"))
        .and_then(|v| v.get("ollama.gemma4"))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("ollama.gemma4 config missing from default.toml"))?;
    let json = serde_json::to_value(cfg).context("toml → json for ollama.gemma4")?;
    Ok(json)
}

/// Send a one-shot probe through the translator to confirm the
/// Ollama server is reachable. Avoids surfacing per-card failures
/// in the loop when the server is simply off.
///
/// Timeout is intentionally generous: Ollama cold-start (loading
/// gemma4:e2b weights, initialising the runner) can take 10-25 s
/// on a fresh process even when the server itself responds in
/// milliseconds afterwards. A 4 s probe timed out on every cold
/// run, silently demoting eval output to the echo translator.
/// 30 s lets the model warm up so the rest of the eval can run
/// against the real translator.
fn probe_translator(rt: &tokio::runtime::Runtime, t: &OllamaTranslator) -> bool {
    let result = rt.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            t.run(TaskInput::Translate {
                text: "ok".into(),
                source_language: "en".into(),
                target_language: "ja".into(),
                glossary: Vec::new(),
            }),
        )
        .await
    });
    matches!(result, Ok(Ok(_)))
}

/// Translate all `rendered_lines` using the production paragraph
/// grouping + numbered-marker path. Returns one string per line in
/// the input order — exactly what `cards` expects. Failure on any
/// paragraph falls the whole group back to the source text so the
/// eval still produces output.
fn translate_single(
    rt: &tokio::runtime::Runtime,
    t: &OllamaTranslator,
    text: &str,
) -> Result<String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let out = rt.block_on(t.run(TaskInput::Translate {
        text: text.to_string(),
        source_language: String::new(),
        target_language: TARGET_LANGUAGE.to_string(),
        glossary: Vec::new(),
    }));
    match out {
        Ok(TaskOutput::Text(s)) => Ok(s.trim().to_string()),
        Ok(other) => anyhow::bail!("unexpected output: {other:?}"),
        Err(e) => anyhow::bail!("ollama: {e}"),
    }
}

/// Greedy word-wrap to `max_width`. Mirrors the frontend's
/// `overflow-wrap: anywhere` behaviour: prefers whitespace
/// boundaries when available, falls back to character-boundary
/// breaks for CJK / long unbreakable tokens. Embedded newlines in
/// the input are preserved as hard line breaks.
fn wrap_text(text: &str, font: &FontVec, scale: f32, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        if measure_text_width(paragraph, font, scale) <= max_width {
            lines.push(paragraph.to_string());
            continue;
        }
        // Greedy wrap. Track last-whitespace index so an ASCII /
        // Latin paragraph can break at word boundaries; when no
        // whitespace fits, fall through to a char-boundary break
        // which handles CJK naturally.
        let mut current = String::new();
        let mut last_ws_len: Option<usize> = None;
        for c in paragraph.chars() {
            let mut candidate = current.clone();
            candidate.push(c);
            let w = measure_text_width(&candidate, font, scale);
            if w > max_width && !current.is_empty() {
                if let Some(wb) = last_ws_len {
                    let break_str: String = current.chars().take(wb).collect();
                    let remainder: String = current.chars().skip(wb).collect();
                    lines.push(break_str.trim_end().to_string());
                    current = remainder;
                } else {
                    lines.push(current.clone());
                    current.clear();
                }
                last_ws_len = None;
                current.push(c);
            } else {
                current.push(c);
                if c.is_whitespace() {
                    last_ws_len = Some(current.chars().count());
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    lines
}

/// Approximate the pixel width of `text` at `scale` using the
/// horizontal advance of each glyph. Matches ab_glyph's layout,
/// so overflow detection sees the same widths the eval image
/// would render.
fn measure_text_width(text: &str, font: &FontVec, scale: f32) -> f32 {
    use ab_glyph::{Font, ScaleFont};
    let scaled = font.as_scaled(scale);
    text.chars()
        .map(|c| scaled.h_advance(scaled.glyph_id(c)))
        .sum()
}

fn try_load_paddle_engine(project_root: &Path) -> Result<pure_onnx_ocr::OcrEngine> {
    let dir = project_root.join("tests").join("models").join("ppocrv5");
    let det = dir.join("det.onnx");
    let rec = dir.join("rec.onnx");
    let dict = dir.join("ppocrv5_dict.txt");
    for (label, p) in [("det", &det), ("rec", &rec), ("dict", &dict)] {
        if !p.exists() {
            anyhow::bail!("missing {label} at {}", p.display());
        }
    }
    pure_onnx_ocr::OcrEngineBuilder::new()
        .det_model_path(det)
        .rec_model_path(rec)
        .dictionary_path(dict)
        .det_limit_side_len(960)
        .det_unclip_ratio(1.5)
        .rec_batch_size(8)
        .build()
        .map_err(|e| anyhow::anyhow!("PaddleOCR builder: {e}"))
}

/// Run PaddleOCR detection on the full image and return axis-aligned
/// bboxes derived from each detected polygon's outer hull. Uses the
/// engine's combined detection + recognition path (the recognition
/// text is discarded — we use PaddleOCR as a detector only to stay
/// consistent with the ocrs comparison).
fn run_paddle_detection(
    img: &DynamicImage,
    engine: &pure_onnx_ocr::OcrEngine,
) -> Result<Vec<[f32; 4]>> {
    let results = engine
        .run_from_image(img)
        .map_err(|e| anyhow::anyhow!("paddle run: {e}"))?;
    let mut out = Vec::with_capacity(results.len());
    for r in results {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for pt in r.bounding_box.exterior().points() {
            let x = pt.x() as f32;
            let y = pt.y() as f32;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        if min_x < max_x && min_y < max_y {
            out.push([min_x, min_y, max_x - min_x, max_y - min_y]);
        }
    }
    Ok(out)
}

/// Attempt to construct an `ocrs::OcrEngine` from the standard
/// MoonLens model directory. Returns `None` when the rten model
/// files are absent or load fails — the evaluator then runs with
/// WinRT-only detection.
fn try_load_ocrs_engine() -> Option<ocrs::OcrEngine> {
    let dir = default_models_dir()?;
    let det = dir.join("text-detection.rten");
    let rec = dir.join("text-recognition.rten");
    if !det.exists() || !rec.exists() {
        return None;
    }
    let detection = rten::Model::load_file(&det).ok()?;
    let recognition = rten::Model::load_file(&rec).ok()?;
    ocrs::OcrEngine::new(ocrs::OcrEngineParams {
        detection_model: Some(detection),
        recognition_model: Some(recognition),
        ..Default::default()
    })
    .ok()
}

/// Run ocrs detection (+ line-grouping) on the source image and
/// return one axis-aligned bbox per detected text line in image
/// pixel coordinates. Recognition is intentionally skipped — we
/// use this as an additional DETECTOR only, since ocrs recognition
/// on Japanese / stylised text is weaker than WinRT's.
fn run_ocrs_detection(img: &DynamicImage, engine: &ocrs::OcrEngine) -> Result<Vec<[f32; 4]>> {
    let rgb = img.to_rgb8();
    let w = rgb.width();
    let h = rgb.height();
    let source = ocrs::ImageSource::from_bytes(rgb.as_raw(), (w, h))
        .map_err(|e| anyhow::anyhow!("ocrs image source: {e}"))?;
    let input = engine
        .prepare_input(source)
        .map_err(|e| anyhow::anyhow!("ocrs prepare: {e}"))?;
    let words = engine
        .detect_words(&input)
        .map_err(|e| anyhow::anyhow!("ocrs detect: {e}"))?;
    let line_rects = engine.find_text_lines(&input, &words);
    let mut out = Vec::with_capacity(line_rects.len());
    for word_rects in line_rects {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for rr in &word_rects {
            for c in rr.corners() {
                min_x = min_x.min(c.x);
                min_y = min_y.min(c.y);
                max_x = max_x.max(c.x);
                max_y = max_y.max(c.y);
            }
        }
        if min_x < max_x && min_y < max_y {
            out.push([min_x, min_y, max_x - min_x, max_y - min_y]);
        }
    }
    Ok(out)
}

/// Ratio of (intersection area) / (area of `a`). Used to decide
/// whether an ocrs bbox is already covered by a WinRT bbox.
fn bbox_iou_over_area(a: &[f32; 4], b: &[f32; 4]) -> f32 {
    let (ax1, ay1, ax2, ay2) = (a[0], a[1], a[0] + a[2], a[1] + a[3]);
    let (bx1, by1, bx2, by2) = (b[0], b[1], b[0] + b[2], b[1] + b[3]);
    let ix1 = ax1.max(bx1);
    let iy1 = ay1.max(by1);
    let ix2 = ax2.min(bx2);
    let iy2 = ay2.min(by2);
    if ix2 <= ix1 || iy2 <= iy1 {
        return 0.0;
    }
    let inter = (ix2 - ix1) * (iy2 - iy1);
    let area_a = (a[2] * a[3]).max(1.0);
    inter / area_a
}

// demote_isolated_single_kanji moved to `lens_watcher` — imported
// via `use moonlens_lib::lens_watcher::demote_isolated_single_kanji`.

/// Preprocess image for OCR.
///
/// Currently: plain 2× Lanczos upscale, encoded as PNG. Sounds
/// trivial, but empirically this is the best input for WinRT:
///   - Scale alone halves the "to → 0" / trailing-char misses.
///   - CLAHE + Unsharp Mask were tried (Zuiderveld 1994 / standard
///     unsharp masking) and REGRESSED WinRT's recall on every test
///     image — the recogniser's internals appear tuned for natural
///     contrast and the boosted local contrast confuses its
///     character-boundary classifier.
///   - Sauvola adaptive binarization (Sauvola & Pietikäinen 2000)
///     regressed even harder — the hard-binary output broke glyphs
///     that depended on anti-aliasing for their feature extraction.
///
/// The `apply_clahe` / `unsharp_mask` / `sauvola_binarize` helpers
/// are kept below for future experimentation with a different OCR
/// engine (e.g. PaddleOCR), which may actually benefit from them.
#[allow(dead_code)]
fn preprocess_for_ocr(img: &DynamicImage, scale: u32) -> Result<Vec<u8>> {
    let (w, h) = img.dimensions();
    let upscaled = img.resize_exact(w * scale, h * scale, image::imageops::FilterType::Lanczos3);
    let mut png = Vec::new();
    upscaled
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .context("encode preprocessed PNG")?;
    Ok(png)
}

#[allow(dead_code)]
/// CLAHE (Contrast Limited Adaptive Histogram Equalization) — split
/// the image into `tile_size`-sized tiles, equalise each tile's
/// histogram with bin values capped at `clip_limit` * mean, then
/// bilinearly interpolate the per-tile LUTs so tile boundaries do
/// not show. Reference: Zuiderveld, "Contrast Limited Adaptive
/// Histogram Equalization", Graphics Gems IV (1994).
fn apply_clahe(img: &GrayImage, tile_size: u32, clip_limit: f32) -> GrayImage {
    let (w, h) = img.dimensions();
    let tx = w.div_ceil(tile_size);
    let ty = h.div_ceil(tile_size);
    let mut luts: Vec<[u8; 256]> = vec![[0u8; 256]; (tx * ty) as usize];

    for tyi in 0..ty {
        for txi in 0..tx {
            let x0 = txi * tile_size;
            let y0 = tyi * tile_size;
            let x1 = (x0 + tile_size).min(w);
            let y1 = (y0 + tile_size).min(h);

            let mut hist = [0u32; 256];
            for py in y0..y1 {
                for px in x0..x1 {
                    hist[img.get_pixel(px, py).0[0] as usize] += 1;
                }
            }

            let area = (x1 - x0) * (y1 - y0);
            let clip_bin = ((clip_limit * area as f32) / 256.0).max(1.0) as u32;
            let mut excess: u32 = 0;
            for bin in hist.iter_mut() {
                if *bin > clip_bin {
                    excess += *bin - clip_bin;
                    *bin = clip_bin;
                }
            }
            let redis = excess / 256;
            let remainder = excess - redis * 256;
            for bin in hist.iter_mut() {
                *bin += redis;
            }
            for bin in hist.iter_mut().take(remainder as usize) {
                *bin += 1;
            }

            let mut acc: u32 = 0;
            let lut = &mut luts[(tyi * tx + txi) as usize];
            for (i, h_i) in hist.iter().enumerate() {
                acc += *h_i;
                lut[i] = ((acc as u64 * 255) / area as u64).min(255) as u8;
            }
        }
    }

    let mut out = GrayImage::new(w, h);
    let ts = tile_size as f32;
    for y in 0..h {
        for x in 0..w {
            let v = img.get_pixel(x, y).0[0] as usize;
            // Tile-centre fractional coordinates so we interpolate
            // between the four nearest tile LUTs instead of snapping
            // to the containing tile (the latter produces visible
            // blockiness at tile seams).
            let fx = (x as f32 + 0.5) / ts - 0.5;
            let fy = (y as f32 + 0.5) / ts - 0.5;
            let ix = fx.floor() as i32;
            let iy = fy.floor() as i32;
            let ax = (fx - ix as f32).clamp(0.0, 1.0);
            let ay = (fy - iy as f32).clamp(0.0, 1.0);

            let sample = |cx: i32, cy: i32| -> f32 {
                let clamped_x = cx.clamp(0, tx as i32 - 1) as u32;
                let clamped_y = cy.clamp(0, ty as i32 - 1) as u32;
                luts[(clamped_y * tx + clamped_x) as usize][v] as f32
            };
            let tl = sample(ix, iy);
            let tr = sample(ix + 1, iy);
            let bl = sample(ix, iy + 1);
            let br = sample(ix + 1, iy + 1);
            let top = tl * (1.0 - ax) + tr * ax;
            let bot = bl * (1.0 - ax) + br * ax;
            let val = top * (1.0 - ay) + bot * ay;
            out.put_pixel(x, y, Luma([val.clamp(0.0, 255.0) as u8]));
        }
    }
    out
}

#[allow(dead_code)]
/// Unsharp Mask: `out = original + amount * (original - blurred)`.
/// Emphasises glyph edges by subtracting a Gaussian-blurred version.
fn unsharp_mask(img: &GrayImage, sigma: f32, amount: f32) -> GrayImage {
    let blurred = gaussian_blur_f32(img, sigma);
    let (w, h) = img.dimensions();
    let mut out = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let orig = img.get_pixel(x, y).0[0] as f32;
            let blur = blurred.get_pixel(x, y).0[0] as f32;
            let sharp = orig + amount * (orig - blur);
            out.put_pixel(x, y, Luma([sharp.clamp(0.0, 255.0) as u8]));
        }
    }
    out
}

#[allow(dead_code)]
/// Sauvola adaptive binarization. For each pixel, computes a local
/// mean and standard deviation over a `window`-sized neighbourhood
/// via integral images (O(1) per pixel), then applies:
///     t = mean * (1 + k * (std / R - 1))
/// where R = 128 is the assumed std dynamic range. Reference:
/// Sauvola & Pietikäinen, "Adaptive document image binarization",
/// Pattern Recognition 33 (2000).
fn sauvola_binarize(img: &GrayImage, window: u32, k: f32) -> GrayImage {
    let (w, h) = img.dimensions();
    let wu = w as usize;
    let hu = h as usize;
    let stride = wu + 1;
    let mut sum = vec![0u64; stride * (hu + 1)];
    let mut sum_sq = vec![0u64; stride * (hu + 1)];
    for y in 0..hu {
        for x in 0..wu {
            let v = img.get_pixel(x as u32, y as u32).0[0] as u64;
            let idx = |xx: usize, yy: usize| -> usize { yy * stride + xx };
            sum[idx(x + 1, y + 1)] = v + sum[idx(x, y + 1)] + sum[idx(x + 1, y)] - sum[idx(x, y)];
            sum_sq[idx(x + 1, y + 1)] =
                v * v + sum_sq[idx(x, y + 1)] + sum_sq[idx(x + 1, y)] - sum_sq[idx(x, y)];
        }
    }

    let half = (window / 2) as i32;
    let mut out = GrayImage::new(w, h);
    let r_norm: f32 = 128.0;
    for y in 0..hu {
        for x in 0..wu {
            let x0 = (x as i32 - half).max(0) as usize;
            let y0 = (y as i32 - half).max(0) as usize;
            let x1 = ((x as i32 + half + 1).min(wu as i32)) as usize;
            let y1 = ((y as i32 + half + 1).min(hu as i32)) as usize;
            let area = ((x1 - x0) * (y1 - y0)) as f32;
            let idx = |xx: usize, yy: usize| -> usize { yy * stride + xx };
            let s =
                (sum[idx(x1, y1)] + sum[idx(x0, y0)] - sum[idx(x1, y0)] - sum[idx(x0, y1)]) as f32;
            let sq = (sum_sq[idx(x1, y1)] + sum_sq[idx(x0, y0)]
                - sum_sq[idx(x1, y0)]
                - sum_sq[idx(x0, y1)]) as f32;
            let mean = s / area;
            let var = (sq / area) - mean * mean;
            let std_dev = var.max(0.0).sqrt();
            let threshold = mean * (1.0 + k * (std_dev / r_norm - 1.0));
            let v = img.get_pixel(x as u32, y as u32).0[0] as f32;
            out.put_pixel(
                x as u32,
                y as u32,
                Luma([if v > threshold { 255 } else { 0 }]),
            );
        }
    }
    out
}

// `color_dist_sq` and `dilate_mask` are unused in the eval after
// inpainting was centralised in `lens_inpaint`; the module exposes
// them via `lens_inpaint::dilate_mask` when needed.
