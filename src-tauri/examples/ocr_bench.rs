//! OCR engine benchmark harness.
//!
//! Compares the current `ocrs` baseline against PP-OCRv5 mobile
//! (via `pure-onnx-ocr` / tract-onnx) on a fixture set with
//! ground-truth JSON. Emits per-run records and an aggregated
//! summary so we can decide whether to migrate the production OCR
//! provider away from `ocrs`.
//!
//! Run: `cargo run --release --example ocr_bench`
//! Default output: `tests/runs/ocr_bench/<engine>/<image>.json`,
//!                 `tests/runs/ocr_bench/summary.json`,
//!                 `tests/runs/ocr_bench/summary.md`.
//! Override with `MOONLENS_OCR_BENCH_OUTPUT_DIR=<path>` (e.g. point
//! it at `tests/snapshots/ocr_bench/` to refresh the committed
//! baseline once the diff has been reviewed).
//!
//! Iteration 2 (deferred): add a manga-ocr ONNX path via `ort` for a
//! Japanese-specialist comparison once iteration-1 results land.

use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{anyhow, Context, Result};
use image::DynamicImage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
struct GroundTruth {
    image: String,
    lang: String,
    lines: Vec<String>,
}

impl GroundTruth {
    fn joined(&self) -> String {
        self.lines.join(" ")
    }
}

trait BenchOcr {
    fn name(&self) -> &str;
    fn run(&self, img: &DynamicImage) -> Result<Vec<String>>;
}

// --- ocrs adapter ----------------------------------------------------------

struct OcrsEngine {
    engine: ocrs::OcrEngine,
}

impl OcrsEngine {
    fn try_new() -> Result<Self> {
        let dir = default_ocrs_models_dir()
            .context("could not resolve OS appdata dir for ocrs models")?;
        let det = dir.join("text-detection.rten");
        let rec = dir.join("text-recognition.rten");
        if !det.exists() || !rec.exists() {
            return Err(anyhow!(
                "ocrs models not found at {} (run the app once to download them, or use `cargo run --release --example lens_eval` which installs them)",
                dir.display()
            ));
        }
        let detection = rten::Model::load_file(&det)?;
        let recognition = rten::Model::load_file(&rec)?;
        let engine = ocrs::OcrEngine::new(ocrs::OcrEngineParams {
            detection_model: Some(detection),
            recognition_model: Some(recognition),
            ..Default::default()
        })
        .map_err(|e| anyhow!("ocrs init: {e}"))?;
        Ok(Self { engine })
    }
}

impl BenchOcr for OcrsEngine {
    fn name(&self) -> &str {
        "ocrs"
    }

    fn run(&self, img: &DynamicImage) -> Result<Vec<String>> {
        let rgb = img.to_rgb8();
        let (w, h) = (rgb.width(), rgb.height());
        let source = ocrs::ImageSource::from_bytes(rgb.as_raw(), (w, h))
            .map_err(|e| anyhow!("ocrs source: {e}"))?;
        let prepared = self
            .engine
            .prepare_input(source)
            .map_err(|e| anyhow!("ocrs prepare: {e}"))?;
        let words = self
            .engine
            .detect_words(&prepared)
            .map_err(|e| anyhow!("ocrs detect: {e}"))?;
        let lines = self.engine.find_text_lines(&prepared, &words);
        let texts = self
            .engine
            .recognize_text(&prepared, &lines)
            .map_err(|e| anyhow!("ocrs recognize: {e}"))?;
        Ok(texts
            .into_iter()
            .filter_map(|l| l.map(|x| x.to_string()))
            .filter(|t| !t.trim().is_empty())
            .collect())
    }
}

fn default_ocrs_models_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.map(|b| b.join("moonlens").join("models"))
}

// --- PaddleOCR (PP-OCRv5 mobile via tract-onnx) ----------------------------

struct PaddleEngine {
    engine: pure_onnx_ocr::OcrEngine,
}

impl PaddleEngine {
    fn try_new(project_root: &Path) -> Result<Self> {
        let dir = project_root.join("tests").join("models").join("ppocrv5");
        let det = dir.join("det.onnx");
        let rec = dir.join("rec.onnx");
        let dict = dir.join("ppocrv5_dict.txt");
        for (label, p) in [("det", &det), ("rec", &rec), ("dict", &dict)] {
            if !p.exists() {
                return Err(anyhow!(
                    "PP-OCRv5 {label} missing at {} (see tests/models/.gitignore for download instructions)",
                    p.display()
                ));
            }
        }
        let engine = pure_onnx_ocr::OcrEngineBuilder::new()
            .det_model_path(det)
            .rec_model_path(rec)
            .dictionary_path(dict)
            .det_limit_side_len(960)
            .det_unclip_ratio(1.5)
            .rec_batch_size(8)
            .build()
            .map_err(|e| anyhow!("PP-OCRv5 builder: {e}"))?;
        Ok(Self { engine })
    }
}

impl BenchOcr for PaddleEngine {
    fn name(&self) -> &str {
        "ppocrv5-mobile"
    }

    fn run(&self, img: &DynamicImage) -> Result<Vec<String>> {
        let results = self
            .engine
            .run_from_image(img)
            .map_err(|e| anyhow!("PP-OCRv5 run: {e}"))?;
        Ok(results
            .into_iter()
            .filter_map(|r| {
                let t = r.text.trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            })
            .collect())
    }
}

// --- manga-ocr (VisionEncoderDecoder via ort) ------------------------------

/// manga-ocr inference path: ViT encoder + char-level transformer
/// decoder. Hugging Face export from `kha-white/manga-ocr-base` via
/// `optimum-cli`. Greedy decoding (no KV cache) — slow per crop but
/// good enough for a bench.
struct MangaOcrRecognizer {
    encoder: parking_lot::Mutex<ort::session::Session>,
    decoder: parking_lot::Mutex<ort::session::Session>,
    vocab: Vec<String>,
}

impl MangaOcrRecognizer {
    const CLS_ID: i64 = 2; // decoder_start_token_id
    const SEP_ID: i64 = 3; // eos_token_id
    const MAX_LEN: usize = 300;
    const IMG_SIZE: usize = 224;

    fn try_new(project_root: &Path) -> Result<Self> {
        let dir = project_root.join("tests").join("models").join("manga_ocr");
        let enc_path = dir.join("encoder_model.onnx");
        let dec_path = dir.join("decoder_model.onnx");
        let vocab_path = dir.join("vocab.txt");
        for (label, p) in [
            ("encoder_model.onnx", &enc_path),
            ("decoder_model.onnx", &dec_path),
            ("vocab.txt", &vocab_path),
        ] {
            if !p.exists() {
                return Err(anyhow!(
                    "manga-ocr {label} missing at {} (run tests/models/download_manga_ocr.ps1)",
                    p.display()
                ));
            }
        }
        let encoder = ort_session_builder_with_gpu()?
            .commit_from_file(&enc_path)
            .map_err(|e| anyhow!("load encoder_model.onnx: {e}"))?;
        let decoder = ort_session_builder_with_gpu()?
            .commit_from_file(&dec_path)
            .map_err(|e| anyhow!("load decoder_model.onnx: {e}"))?;
        let vocab_text = fs::read_to_string(&vocab_path)?;
        let vocab: Vec<String> = vocab_text.lines().map(|s| s.to_string()).collect();
        Ok(Self {
            encoder: parking_lot::Mutex::new(encoder),
            decoder: parking_lot::Mutex::new(decoder),
            vocab,
        })
    }

    /// Resize to 224x224 (bilinear), convert to RGB, normalise with
    /// ImageNet-style mean=0.5 / std=0.5 (per preprocessor_config.json),
    /// emit (1,3,224,224) f32 in CHW order.
    fn preprocess(crop: &DynamicImage) -> ndarray::Array4<f32> {
        let resized = crop.resize_exact(
            Self::IMG_SIZE as u32,
            Self::IMG_SIZE as u32,
            image::imageops::FilterType::Triangle,
        );
        let rgb = resized.to_rgb8();
        let mut arr = ndarray::Array4::<f32>::zeros((1, 3, Self::IMG_SIZE, Self::IMG_SIZE));
        for (y, row) in rgb.rows().enumerate() {
            for (x, px) in row.enumerate() {
                let [r, g, b] = px.0;
                arr[[0, 0, y, x]] = (r as f32 / 255.0 - 0.5) / 0.5;
                arr[[0, 1, y, x]] = (g as f32 / 255.0 - 0.5) / 0.5;
                arr[[0, 2, y, x]] = (b as f32 / 255.0 - 0.5) / 0.5;
            }
        }
        arr
    }

    fn recognise(&self, crop: &DynamicImage) -> Result<String> {
        let pixel_values = Self::preprocess(crop);
        let pix_shape = [1i64, 3, Self::IMG_SIZE as i64, Self::IMG_SIZE as i64];
        let pix_data: Vec<f32> = pixel_values.iter().copied().collect();
        let pix_tensor = ort::value::Tensor::from_array((pix_shape, pix_data))
            .map_err(|e| anyhow!("encoder tensor: {e}"))?;
        let mut encoder = self.encoder.lock();
        let enc_outputs = encoder
            .run(ort::inputs!["pixel_values" => pix_tensor])
            .map_err(|e| anyhow!("encoder run: {e}"))?;
        let (enc_shape, enc_data) = enc_outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!("encoder extract: {e}"))?;
        let enc_seq_len = enc_shape[1];
        let enc_hidden = enc_shape[2];
        let enc_data_owned: Vec<f32> = enc_data.to_vec();
        drop(enc_outputs);
        drop(encoder);

        let mut decoder = self.decoder.lock();
        let mut tokens: Vec<i64> = vec![Self::CLS_ID];
        // Repetition guard. The generation config says
        // `no_repeat_ngram_size=3`; we do something simpler: bail out
        // if the same token is emitted N times in a row, which is the
        // failure mode seen in practice (mangled crops collapse into
        // long runs of one char until the max-length cap).
        const REPEAT_CAP: usize = 6;
        for _ in 0..Self::MAX_LEN {
            let id_shape = [1i64, tokens.len() as i64];
            let id_tensor = ort::value::Tensor::from_array((id_shape, tokens.clone()))
                .map_err(|e| anyhow!("decoder ids tensor: {e}"))?;
            let enc_shape_owned = [1i64, enc_seq_len, enc_hidden];
            let enc_tensor =
                ort::value::Tensor::from_array((enc_shape_owned, enc_data_owned.clone()))
                    .map_err(|e| anyhow!("decoder enc tensor: {e}"))?;
            let dec_outputs = decoder
                .run(ort::inputs![
                    "input_ids" => id_tensor,
                    "encoder_hidden_states" => enc_tensor,
                ])
                .map_err(|e| anyhow!("decoder run: {e}"))?;
            let (logits_shape, logits_data) = dec_outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("decoder extract: {e}"))?;
            // shape: (1, current_len, vocab_size). Pull the last step.
            let vocab_size = logits_shape[2] as usize;
            let cur_len = logits_shape[1] as usize;
            let last_start = (cur_len - 1) * vocab_size;
            let last_logits = &logits_data[last_start..last_start + vocab_size];
            let mut best_id = 0i64;
            let mut best = f32::NEG_INFINITY;
            for (i, &v) in last_logits.iter().enumerate() {
                if v > best {
                    best = v;
                    best_id = i as i64;
                }
            }
            if best_id == Self::SEP_ID {
                break;
            }
            tokens.push(best_id);
            if tokens.len() > REPEAT_CAP {
                let tail = &tokens[tokens.len() - REPEAT_CAP..];
                if tail.iter().all(|t| *t == tail[0]) {
                    // Drop the offending repeated tail so it does not
                    // pollute the decoded text, then stop.
                    tokens.truncate(tokens.len() - REPEAT_CAP);
                    break;
                }
            }
        }

        let mut text = String::new();
        for &t in tokens.iter().skip(1) {
            if (t as usize) < self.vocab.len() {
                let s = &self.vocab[t as usize];
                // Skip the 5 BERT specials + the `<unusedN>` block (5..1005).
                if s.starts_with('[') && s.ends_with(']') {
                    continue;
                }
                if s.starts_with("<unused") {
                    continue;
                }
                // WordPiece continuation marker — strip it for char-level.
                let cleaned = s.strip_prefix("##").unwrap_or(s);
                text.push_str(cleaned);
            }
        }
        Ok(text)
    }
}

// --- GPU execution provider hook -------------------------------------------

/// Read `MOONLENS_OCR_GPU` once and return the requested execution
/// provider chain for ort sessions. Falls back to CPU when unset or
/// when the requested provider is not compiled in.
///
/// Recognised values: `directml`, `cuda`, `cpu` (default).
fn ort_session_config_from_env() -> Option<oar_ocr_core::core::config::OrtSessionConfig> {
    use oar_ocr_core::core::config::{OrtExecutionProvider, OrtSessionConfig};
    let raw = std::env::var("MOONLENS_OCR_GPU").ok()?;
    let provider = match raw.to_ascii_lowercase().as_str() {
        "directml" | "dml" => OrtExecutionProvider::DirectML { device_id: Some(0) },
        "cuda" => OrtExecutionProvider::CUDA {
            device_id: Some(0),
            gpu_mem_limit: None,
            arena_extend_strategy: None,
            cudnn_conv_algo_search: None,
            cudnn_conv_use_max_workspace: None,
        },
        "cpu" | "" => return None,
        other => {
            eprintln!("warning: MOONLENS_OCR_GPU={other} not recognised; falling back to CPU");
            return None;
        }
    };
    Some(
        OrtSessionConfig::new().with_execution_providers(vec![provider, OrtExecutionProvider::CPU]),
    )
}

/// Returns the ort `SessionBuilder` for the manga-ocr direct path
/// with the same EP chain as the oar-ocr engines so CPU vs GPU
/// comparison stays apples-to-apples.
fn ort_session_builder_with_gpu() -> Result<ort::session::builder::SessionBuilder> {
    let builder = ort::session::Session::builder().map_err(|e| anyhow!("ort builder: {e}"))?;
    let raw = match std::env::var("MOONLENS_OCR_GPU") {
        Ok(v) => v,
        Err(_) => return Ok(builder),
    };
    let builder = match raw.to_ascii_lowercase().as_str() {
        "directml" | "dml" => builder
            .with_execution_providers([ort::ep::DirectML::default().build()])
            .map_err(|e| anyhow!("with_execution_providers: {e}"))?,
        "cuda" => builder
            .with_execution_providers([ort::ep::CUDA::default().build()])
            .map_err(|e| anyhow!("with_execution_providers: {e}"))?,
        _ => builder,
    };
    Ok(builder)
}

// --- vertical-JA reading-order recovery ------------------------------------

/// One detected region with bbox extents and the recognised text.
/// Used as the intermediate format for the column-reorder pass.
#[derive(Debug, Clone)]
struct Boxed {
    text: String,
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
}

impl Boxed {
    fn cx(&self) -> f32 {
        (self.x_min + self.x_max) * 0.5
    }
    fn w(&self) -> f32 {
        (self.x_max - self.x_min).max(1.0)
    }
    fn h(&self) -> f32 {
        (self.y_max - self.y_min).max(1.0)
    }
}

/// Detect tategaki-shaped layouts and reorder boxes into Japanese
/// vertical reading order (top→bottom within a column, right→left
/// across columns). Returns the reordered Vec on a hit; returns the
/// original Vec untouched when the layout is horizontal — so callers
/// can call this unconditionally.
///
/// Heuristic (all must hold):
///  - at least 4 boxes total (anything smaller is too noisy to judge),
///  - ≥ 50% of boxes are "char-cell-like" (h ≥ w · 0.5) — rules out
///    sentence-shaped boxes from horizontal layouts,
///  - greedy x-clustering yields ≥ 2 columns,
///  - average column depth ≥ 2 boxes — rules out single-row headers.
fn reorder_for_vertical_ja(boxes: Vec<Boxed>) -> Vec<Boxed> {
    if boxes.len() < 4 {
        return boxes;
    }
    let char_like = boxes.iter().filter(|b| b.h() >= b.w() * 0.5).count();
    if char_like * 2 < boxes.len() {
        return boxes;
    }

    let mut median_w: Vec<f32> = boxes.iter().map(|b| b.w()).collect();
    median_w.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = median_w[median_w.len() / 2];
    let col_gap = (med * 0.7).max(8.0);

    let mut idx: Vec<usize> = (0..boxes.len()).collect();
    idx.sort_by(|&a, &b| boxes[a].cx().partial_cmp(&boxes[b].cx()).unwrap());

    let mut columns: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = vec![idx[0]];
    let mut last_cx = boxes[idx[0]].cx();
    for &i in &idx[1..] {
        let cx = boxes[i].cx();
        if cx - last_cx > col_gap {
            columns.push(std::mem::take(&mut cur));
        }
        cur.push(i);
        last_cx = cx;
    }
    if !cur.is_empty() {
        columns.push(cur);
    }

    if columns.len() < 2 {
        return boxes;
    }
    let avg_depth = boxes.len() as f32 / columns.len() as f32;
    if avg_depth < 2.0 {
        return boxes;
    }

    // Top-down within each column.
    for col in &mut columns {
        col.sort_by(|&a, &b| boxes[a].y_min.partial_cmp(&boxes[b].y_min).unwrap());
    }
    // Right-to-left across columns: sort by the column's *leftmost*
    // box's cx descending so even uneven-height columns line up.
    columns.sort_by(|a, b| boxes[b[0]].cx().partial_cmp(&boxes[a[0]].cx()).unwrap());

    let order: Vec<usize> = columns.into_iter().flatten().collect();
    let mut owned = boxes;
    // permute in place via take+swap so we do not need T: Default
    let mut out: Vec<Boxed> = Vec::with_capacity(owned.len());
    for i in order {
        out.push(std::mem::replace(
            &mut owned[i],
            Boxed {
                text: String::new(),
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 0.0,
            },
        ));
    }
    out
}

fn boxed_from_oar_region(region: &oar_ocr::prelude::TextRegion) -> Option<Boxed> {
    let pts = &region.bounding_box.points;
    if pts.is_empty() {
        return None;
    }
    let mut x_min = f32::MAX;
    let mut x_max = f32::MIN;
    let mut y_min = f32::MAX;
    let mut y_max = f32::MIN;
    for p in pts {
        x_min = x_min.min(p.x);
        x_max = x_max.max(p.x);
        y_min = y_min.min(p.y);
        y_max = y_max.max(p.y);
    }
    let text = region
        .text_with_confidence()
        .map(|(t, _)| t.trim().to_string())
        .unwrap_or_default();
    Some(Boxed {
        text,
        x_min,
        y_min,
        x_max,
        y_max,
    })
}

// --- manga-hybrid (oar-ocr detection + manga-ocr recognition) --------------

struct MangaHybridEngine {
    detector: oar_ocr::prelude::OAROCR,
    recogniser: MangaOcrRecognizer,
}

impl MangaHybridEngine {
    fn try_new(project_root: &Path) -> Result<Self> {
        let dir = project_root.join("tests").join("models").join("ppocrv5");
        let det = dir.join("det.onnx");
        let rec = dir.join("rec.onnx");
        let dict = dir.join("ppocrv5_dict.txt");
        let mut b = oar_ocr::prelude::OAROCRBuilder::new(det, rec, dict);
        if let Some(cfg) = ort_session_config_from_env() {
            b = b.ort_session(cfg);
        }
        let detector = b
            .build()
            .map_err(|e| anyhow!("hybrid detector build: {e}"))?;
        let recogniser = MangaOcrRecognizer::try_new(project_root)?;
        Ok(Self {
            detector,
            recogniser,
        })
    }
}

impl BenchOcr for MangaHybridEngine {
    fn name(&self) -> &str {
        "manga-hybrid"
    }

    fn run(&self, img: &DynamicImage) -> Result<Vec<String>> {
        let rgb = img.to_rgb8();
        let results = self
            .detector
            .predict(vec![rgb])
            .map_err(|e| anyhow!("hybrid detect: {e}"))?;
        let mut boxes = Vec::new();
        for r in &results {
            for region in &r.text_regions {
                let pts = &region.bounding_box.points;
                if pts.is_empty() {
                    continue;
                }
                let mut x_min = f32::MAX;
                let mut x_max = f32::MIN;
                let mut y_min = f32::MAX;
                let mut y_max = f32::MIN;
                for p in pts {
                    x_min = x_min.min(p.x);
                    x_max = x_max.max(p.x);
                    y_min = y_min.min(p.y);
                    y_max = y_max.max(p.y);
                }
                let pad = 4.0;
                let x = (x_min - pad).max(0.0) as u32;
                let y = (y_min - pad).max(0.0) as u32;
                let w = ((x_max + pad).ceil() - x as f32).max(1.0) as u32;
                let h = ((y_max + pad).ceil() - y as f32).max(1.0) as u32;
                let w = w.min(img.width().saturating_sub(x));
                let h = h.min(img.height().saturating_sub(y));
                if w == 0 || h == 0 {
                    continue;
                }
                let crop = img.crop_imm(x, y, w, h);
                match self.recogniser.recognise(&crop) {
                    Ok(text) => {
                        let t = text.trim().to_string();
                        if !t.is_empty() {
                            boxes.push(Boxed {
                                text: t,
                                x_min,
                                y_min,
                                x_max,
                                y_max,
                            });
                        }
                    }
                    Err(e) => tracing::warn!("manga-hybrid rec failed: {e}"),
                }
            }
        }
        let reordered = reorder_for_vertical_ja(boxes);
        Ok(reordered.into_iter().map(|b| b.text).collect())
    }
}

// --- PP-OCRv5 via oar-ocr (ONNX Runtime / ort) -----------------------------

struct OarOcrEngine {
    name: &'static str,
    engine: oar_ocr::prelude::OAROCR,
}

impl OarOcrEngine {
    fn try_new(project_root: &Path) -> Result<Self> {
        let dir = project_root.join("tests").join("models").join("ppocrv5");
        let det = dir.join("det.onnx");
        let rec = dir.join("rec.onnx");
        let dict = dir.join("ppocrv5_dict.txt");
        for (label, p) in [("det", &det), ("rec", &rec), ("dict", &dict)] {
            if !p.exists() {
                return Err(anyhow!(
                    "oar-ocr {label} model missing at {} (shared with pure-onnx-ocr — see tests/models/.gitignore)",
                    p.display()
                ));
            }
        }
        let mut b = oar_ocr::prelude::OAROCRBuilder::new(det, rec, dict);
        if let Some(cfg) = ort_session_config_from_env() {
            b = b.ort_session(cfg);
        }
        let engine = b.build().map_err(|e| anyhow!("oar-ocr builder: {e}"))?;
        Ok(Self {
            name: "ppocrv5-oar",
            engine,
        })
    }

    /// Korean variant: keeps the shared det.onnx but swaps the rec
    /// model + dict for the PaddleOCR Korean recogniser. Use to
    /// quantify the Korean blind spot of the multilingual rec.
    fn try_new_korean(project_root: &Path) -> Result<Self> {
        let det = project_root
            .join("tests")
            .join("models")
            .join("ppocrv5")
            .join("det.onnx");
        let ko_dir = project_root.join("tests").join("models").join("ppocr_ko");
        let rec = ko_dir.join("rec.onnx");
        let dict = ko_dir.join("dict.txt");
        for (label, p) in [("det", &det), ("ko-rec", &rec), ("ko-dict", &dict)] {
            if !p.exists() {
                return Err(anyhow!(
                    "ko {label} model missing at {} (run tests/models/download_korean_ocr.ps1)",
                    p.display()
                ));
            }
        }
        let mut b = oar_ocr::prelude::OAROCRBuilder::new(det, rec, dict);
        if let Some(cfg) = ort_session_config_from_env() {
            b = b.ort_session(cfg);
        }
        let engine = b.build().map_err(|e| anyhow!("oar-ocr-ko builder: {e}"))?;
        Ok(Self {
            name: "ppocrv5-oar-ko",
            engine,
        })
    }
}

impl BenchOcr for OarOcrEngine {
    fn name(&self) -> &str {
        self.name
    }

    fn run(&self, img: &DynamicImage) -> Result<Vec<String>> {
        let rgb = img.to_rgb8();
        let results = self
            .engine
            .predict(vec![rgb])
            .map_err(|e| anyhow!("oar-ocr predict: {e}"))?;
        let mut boxes = Vec::new();
        for r in &results {
            for region in &r.text_regions {
                if let Some(b) = boxed_from_oar_region(region) {
                    if !b.text.is_empty() {
                        boxes.push(b);
                    }
                }
            }
        }
        let reordered = reorder_for_vertical_ja(boxes);
        Ok(reordered.into_iter().map(|b| b.text).collect())
    }
}

// --- CER / WER helpers -----------------------------------------------------

fn levenshtein<T: Eq>(a: &[T], b: &[T]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let m = a.len();
    let n = b.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut cur = vec![0usize; n + 1];
    for i in 1..=m {
        cur[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (cur[j - 1] + 1).min(prev[j] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[n]
}

fn normalize_for_cer(s: &str) -> String {
    // Strip all whitespace so layout differences (engines split lines
    // differently) do not penalise pure recognition quality. Lowercase
    // ASCII so capitalisation drift on the Latin path is forgiven.
    s.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn cer(expected: &str, got: &str) -> f64 {
    let e: Vec<char> = normalize_for_cer(expected).chars().collect();
    let g: Vec<char> = normalize_for_cer(got).chars().collect();
    if e.is_empty() {
        return if g.is_empty() { 0.0 } else { 1.0 };
    }
    levenshtein(&e, &g) as f64 / e.len() as f64
}

fn wer(expected: &str, got: &str) -> f64 {
    // Whitespace-split. Works for Latin/space-delimited; for Japanese
    // this collapses to a single "word" and matches CER, which is fine
    // — Japanese readers care about character accuracy anyway.
    let e: Vec<&str> = expected.split_whitespace().collect();
    let g: Vec<&str> = got.split_whitespace().collect();
    if e.is_empty() {
        return if g.is_empty() { 0.0 } else { 1.0 };
    }
    levenshtein(&e, &g) as f64 / e.len() as f64
}

// --- Records ---------------------------------------------------------------

#[derive(Debug, Serialize)]
struct RunRecord {
    engine: String,
    image: String,
    lang: String,
    expected_chars: usize,
    detected_lines: usize,
    detected_text: String,
    cer: f64,
    wer: f64,
    latency_ms: f64,
}

#[derive(Debug, Serialize)]
struct Summary {
    engine: String,
    runs: usize,
    avg_cer: f64,
    avg_wer: f64,
    avg_latency_ms: f64,
    per_lang_cer: std::collections::BTreeMap<String, f64>,
}

// --- Main ------------------------------------------------------------------

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("project root (parent of src-tauri)")?
        .to_path_buf();
    let input_dir = project_root.join("tests").join("input");
    let gt_dir = project_root.join("tests").join("ocr_gt");
    // Per-run scratch dir under `tests/runs/ocr_bench/`. The
    // committed baseline lives at `tests/snapshots/ocr_bench/` and
    // is updated by copying this dir over after a baseline-bless
    // workflow (see `tests/README.md`).
    let out_dir = match std::env::var("MOONLENS_OCR_BENCH_OUTPUT_DIR") {
        Ok(p) if !p.is_empty() => std::path::PathBuf::from(p),
        _ => project_root.join("tests").join("runs").join("ocr_bench"),
    };
    fs::create_dir_all(&out_dir).with_context(|| format!("create {}", out_dir.display()))?;

    let gts = load_ground_truth(&gt_dir)?;
    if gts.is_empty() {
        return Err(anyhow!(
            "no ground truth found at {} — author <image>.json files there first",
            gt_dir.display()
        ));
    }
    println!("loaded {} ground-truth fixtures", gts.len());

    let mut engines: Vec<Box<dyn BenchOcr>> = Vec::new();
    match OcrsEngine::try_new() {
        Ok(e) => engines.push(Box::new(e)),
        Err(e) => eprintln!("skip ocrs: {e}"),
    }
    match PaddleEngine::try_new(&project_root) {
        Ok(e) => engines.push(Box::new(e)),
        Err(e) => eprintln!("skip ppocrv5-mobile: {e}"),
    }
    match OarOcrEngine::try_new(&project_root) {
        Ok(e) => engines.push(Box::new(e)),
        Err(e) => eprintln!("skip ppocrv5-oar: {e}"),
    }
    match OarOcrEngine::try_new_korean(&project_root) {
        Ok(e) => engines.push(Box::new(e)),
        Err(e) => eprintln!("skip ppocrv5-oar-ko: {e}"),
    }
    match MangaHybridEngine::try_new(&project_root) {
        Ok(e) => engines.push(Box::new(e)),
        Err(e) => eprintln!("skip manga-hybrid: {e}"),
    }
    if engines.is_empty() {
        return Err(anyhow!("no engines available; nothing to bench"));
    }

    let mut all_records: Vec<RunRecord> = Vec::new();

    for engine in &engines {
        let engine_dir = out_dir.join(engine.name());
        fs::create_dir_all(&engine_dir)?;
        println!("\n=== engine: {} ===", engine.name());

        // Warm-up on the first image so the cold-start cost does not
        // pollute the recorded latency for an arbitrary fixture.
        if let Some(gt) = gts.first() {
            let img_path = input_dir.join(&gt.image);
            if let Ok(img) = image::open(&img_path) {
                let _ = engine.run(&img);
            }
        }

        for gt in &gts {
            let img_path = input_dir.join(&gt.image);
            let img =
                image::open(&img_path).with_context(|| format!("open {}", img_path.display()))?;

            let start = Instant::now();
            let detected = match engine.run(&img) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("  {} on {}: ERROR {e}", engine.name(), gt.image);
                    continue;
                }
            };
            let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

            let detected_text = detected.join(" ");
            let expected = gt.joined();
            let cer_v = cer(&expected, &detected_text);
            let wer_v = wer(&expected, &detected_text);

            let record = RunRecord {
                engine: engine.name().into(),
                image: gt.image.clone(),
                lang: gt.lang.clone(),
                expected_chars: expected.chars().count(),
                detected_lines: detected.len(),
                detected_text,
                cer: cer_v,
                wer: wer_v,
                latency_ms,
            };
            println!(
                "  {:42} cer={:.3} wer={:.3} {:>7.1}ms",
                gt.image, cer_v, wer_v, latency_ms
            );

            let stem = Path::new(&gt.image)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            fs::write(
                engine_dir.join(format!("{stem}.json")),
                serde_json::to_string_pretty(&record)?,
            )?;
            all_records.push(record);
        }
    }

    let summaries = summarise(&all_records);
    fs::write(
        out_dir.join("summary.json"),
        serde_json::to_string_pretty(&summaries)?,
    )?;
    fs::write(
        out_dir.join("summary.md"),
        render_markdown(&summaries, &all_records),
    )?;

    println!("\n=== summary ===");
    for s in &summaries {
        println!(
            "  {:18}  runs={:>2}  avg_cer={:.3}  avg_wer={:.3}  avg_latency={:>7.1}ms",
            s.engine, s.runs, s.avg_cer, s.avg_wer, s.avg_latency_ms
        );
    }
    println!("\nwrote {}", out_dir.display());
    Ok(())
}

fn load_ground_truth(gt_dir: &Path) -> Result<Vec<GroundTruth>> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(gt_dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    paths.sort();
    for p in paths {
        let content = fs::read_to_string(&p).with_context(|| format!("read {}", p.display()))?;
        let gt: GroundTruth =
            serde_json::from_str(&content).with_context(|| format!("parse {}", p.display()))?;
        out.push(gt);
    }
    Ok(out)
}

fn summarise(records: &[RunRecord]) -> Vec<Summary> {
    use std::collections::BTreeMap;
    let mut by_engine: BTreeMap<String, Vec<&RunRecord>> = BTreeMap::new();
    for r in records {
        by_engine.entry(r.engine.clone()).or_default().push(r);
    }
    let mut out = Vec::new();
    for (engine, runs) in by_engine {
        let n = runs.len() as f64;
        let avg_cer = runs.iter().map(|r| r.cer).sum::<f64>() / n;
        let avg_wer = runs.iter().map(|r| r.wer).sum::<f64>() / n;
        let avg_latency_ms = runs.iter().map(|r| r.latency_ms).sum::<f64>() / n;

        let mut by_lang: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for r in &runs {
            by_lang.entry(r.lang.clone()).or_default().push(r.cer);
        }
        let per_lang_cer = by_lang
            .into_iter()
            .map(|(k, v)| {
                let avg = v.iter().sum::<f64>() / v.len() as f64;
                (k, avg)
            })
            .collect();
        out.push(Summary {
            engine,
            runs: runs.len(),
            avg_cer,
            avg_wer,
            avg_latency_ms,
            per_lang_cer,
        });
    }
    out
}

fn render_markdown(summaries: &[Summary], records: &[RunRecord]) -> String {
    let mut s = String::new();
    s.push_str("# OCR bench (iteration 1)\n\n");
    s.push_str("Generated by `cargo run --release --example ocr_bench`.\n\n");
    s.push_str("## Overall\n\n");
    s.push_str("| Engine | Runs | Avg CER | Avg WER | Avg Latency (ms) |\n");
    s.push_str("|---|---:|---:|---:|---:|\n");
    for x in summaries {
        s.push_str(&format!(
            "| {} | {} | {:.3} | {:.3} | {:.1} |\n",
            x.engine, x.runs, x.avg_cer, x.avg_wer, x.avg_latency_ms
        ));
    }
    s.push_str("\n## Per-language CER\n\n");
    let mut langs: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for x in summaries {
        for k in x.per_lang_cer.keys() {
            langs.insert(k.clone());
        }
    }
    s.push_str("| Engine |");
    for l in &langs {
        s.push_str(&format!(" {} |", l));
    }
    s.push('\n');
    s.push_str("|---|");
    for _ in &langs {
        s.push_str("---:|");
    }
    s.push('\n');
    for x in summaries {
        s.push_str(&format!("| {} |", x.engine));
        for l in &langs {
            match x.per_lang_cer.get(l) {
                Some(v) => s.push_str(&format!(" {:.3} |", v)),
                None => s.push_str(" — |"),
            }
        }
        s.push('\n');
    }
    s.push_str("\n## Per-image\n\n");
    s.push_str("| Image | Lang | Engine | CER | WER | Latency (ms) |\n");
    s.push_str("|---|---|---|---:|---:|---:|\n");
    let mut sorted: Vec<&RunRecord> = records.iter().collect();
    sorted.sort_by(|a, b| a.image.cmp(&b.image).then_with(|| a.engine.cmp(&b.engine)));
    for r in sorted {
        s.push_str(&format!(
            "| {} | {} | {} | {:.3} | {:.3} | {:.1} |\n",
            r.image, r.lang, r.engine, r.cer, r.wer, r.latency_ms
        ));
    }
    s
}
