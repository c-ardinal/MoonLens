//! OCR via PaddleOCR PP-OCRv5 (DBNet + SVTR) through `oar-ocr` /
//! ONNX Runtime.
//!
//! Primary OCR provider for MoonLens. The bench in
//! `docs/benchmarks/2026-05-14-ocr-bench-iter*.md` shows this engine
//! winning over `ocrs` on every language tested by a 5× CER margin,
//! plus a 3-5× latency speedup on GPU (DirectML).
//!
//! Architecture
//!  - One shared detector (`det.onnx`) + one **multilingual rec**
//!    (`rec_multi.onnx`, supports SC/TC/EN/JP).
//!  - Optional **Korean rec** (`rec_ko.onnx`) loaded in parallel — at
//!    inference time, each detection bbox is recognised by *both*
//!    recognisers and we keep the Korean output iff it contains
//!    Hangul. This is the "best-of-two" router described in the
//!    iter5 benchmark.
//!  - Vertical-JA reading order recovery (tategaki): when most
//!    detected bboxes look like character cells stacked in columns,
//!    reorder them right-to-left across columns, top-to-bottom
//!    within. Identical to the iter4 helper.
//!
//! `ocrs` is kept in the chain as a fallback for the corner case
//! where PP-OCRv5 models are missing on disk.

#![cfg(feature = "paddleocr-engine")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;

use super::{Capability, OcrLine, Provider, ProviderError, TaskInput, TaskOutput};

/// Filename layout under `<appdata>/moonlens/models/paddleocr/`.
const DET_FILE: &str = "det.onnx";
const REC_MULTI_FILE: &str = "rec_multi.onnx";
const DICT_MULTI_FILE: &str = "dict_multi.txt";
const REC_KO_FILE: &str = "rec_ko.onnx";
const DICT_KO_FILE: &str = "dict_ko.txt";

/// Mirror URLs. Hosted at `monkt/paddleocr-onnx` on HuggingFace,
/// Apache 2.0. Sizes: det 4.7 MB, rec_multi 16.5 MB, dict_multi
/// 74 KB, rec_ko 12.8 MB, dict_ko 47 KB. Total ~34 MB.
pub const DET_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/det.onnx";
pub const REC_MULTI_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/rec.onnx";
pub const DICT_MULTI_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/ppocrv5_dict.txt";
pub const REC_KO_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/korean/rec.onnx";
pub const DICT_KO_URL: &str =
    "https://huggingface.co/monkt/paddleocr-onnx/resolve/main/languages/korean/dict.txt";

pub fn default_models_dir() -> Option<PathBuf> {
    super::ocr_ocrs::default_models_dir().map(|p| p.join("paddleocr"))
}

/// PP-OCRv5 OCR provider with optional Korean recogniser chain.
pub struct OarOcrProvider {
    id: String,
    models_dir: PathBuf,
    use_gpu: bool,
    enable_korean: bool,
    /// Shared cached multilingual engine. Same `Arc<Mutex<Option>>>`
    /// pattern as `OcrsProvider` — first call pays the load cost,
    /// subsequent calls hit the cache.
    multi: Arc<Mutex<Option<oar_ocr::prelude::OAROCR>>>,
    /// Korean recogniser. `None` when `enable_korean=false` at
    /// construction time *or* when the KO files are not on disk.
    korean: Arc<Mutex<Option<oar_ocr::prelude::OAROCR>>>,
}

impl OarOcrProvider {
    pub fn new(
        id: impl Into<String>,
        models_dir: PathBuf,
        use_gpu: bool,
        enable_korean: bool,
    ) -> Self {
        Self {
            id: id.into(),
            models_dir,
            use_gpu,
            enable_korean,
            multi: Arc::new(Mutex::new(None)),
            korean: Arc::new(Mutex::new(None)),
        }
    }

    pub fn from_default_dir(
        id: impl Into<String>,
        use_gpu: bool,
        enable_korean: bool,
    ) -> Self {
        let dir = default_models_dir().unwrap_or_else(|| PathBuf::from("./models/paddleocr"));
        Self::new(id, dir, use_gpu, enable_korean)
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    /// Returns the `OrtSessionConfig` used when constructing each
    /// engine. `None` means "let oar-ocr pick its CPU default".
    fn ort_config(&self) -> Option<oar_ocr_core::core::config::OrtSessionConfig> {
        use oar_ocr_core::core::config::{OrtExecutionProvider, OrtSessionConfig};
        if !self.use_gpu {
            return None;
        }
        // Only one EP variant supported today (DirectML on Windows).
        // CUDA / TensorRT would slot in here behind compile-time
        // feature flags later.
        #[cfg(feature = "paddleocr-directml")]
        {
            return Some(OrtSessionConfig::new().with_execution_providers(vec![
                OrtExecutionProvider::DirectML { device_id: Some(0) },
                OrtExecutionProvider::CPU,
            ]));
        }
        #[allow(unreachable_code)]
        None
    }

    fn try_load_multi(&self) -> anyhow::Result<oar_ocr::prelude::OAROCR> {
        let det = self.models_dir.join(DET_FILE);
        let rec = self.models_dir.join(REC_MULTI_FILE);
        let dict = self.models_dir.join(DICT_MULTI_FILE);
        for (label, p) in [("det", &det), ("rec_multi", &rec), ("dict_multi", &dict)] {
            if !p.exists() {
                anyhow::bail!("paddleocr {label} missing at {}", p.display());
            }
        }
        let mut b = oar_ocr::prelude::OAROCRBuilder::new(det, rec, dict);
        if let Some(cfg) = self.ort_config() {
            b = b.ort_session(cfg);
        }
        b.build()
            .map_err(|e| anyhow::anyhow!("oar-ocr multilingual build: {e}"))
    }

    fn try_load_korean(&self) -> anyhow::Result<oar_ocr::prelude::OAROCR> {
        let det = self.models_dir.join(DET_FILE);
        let rec = self.models_dir.join(REC_KO_FILE);
        let dict = self.models_dir.join(DICT_KO_FILE);
        for (label, p) in [("det", &det), ("rec_ko", &rec), ("dict_ko", &dict)] {
            if !p.exists() {
                anyhow::bail!("paddleocr {label} missing at {}", p.display());
            }
        }
        let mut b = oar_ocr::prelude::OAROCRBuilder::new(det, rec, dict);
        if let Some(cfg) = self.ort_config() {
            b = b.ort_session(cfg);
        }
        b.build()
            .map_err(|e| anyhow::anyhow!("oar-ocr korean build: {e}"))
    }
}

#[async_trait]
impl Provider for OarOcrProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Ocr]
    }
    fn is_local(&self) -> bool {
        true
    }

    async fn preload(&self) -> Result<(), ProviderError> {
        // Eagerly construct both engines so the first OCR request
        // doesn't pay model-load latency. Korean is best-effort —
        // missing KO files demote it to a warning, multilingual
        // alone remains operational.
        let multi_cache = self.multi.clone();
        let korean_cache = self.korean.clone();
        let models_dir = self.models_dir.clone();
        let use_gpu = self.use_gpu;
        let enable_korean = self.enable_korean;
        let id = self.id.clone();

        tokio::task::spawn_blocking(move || -> Result<(), ProviderError> {
            let provider = OarOcrProvider {
                id,
                models_dir,
                use_gpu,
                enable_korean,
                multi: multi_cache.clone(),
                korean: korean_cache.clone(),
            };

            {
                let mut g = multi_cache.lock();
                if g.is_none() {
                    let start = std::time::Instant::now();
                    let engine = provider
                        .try_load_multi()
                        .map_err(|e| ProviderError::Other(e.to_string()))?;
                    *g = Some(engine);
                    tracing::info!(
                        "paddleocr multi engine preloaded in {}ms (gpu={})",
                        start.elapsed().as_millis(),
                        provider.use_gpu
                    );
                }
            }

            if enable_korean {
                let mut g = korean_cache.lock();
                if g.is_none() {
                    match provider.try_load_korean() {
                        Ok(engine) => {
                            *g = Some(engine);
                            tracing::info!("paddleocr ko engine preloaded");
                        }
                        Err(e) => tracing::warn!(
                            "paddleocr ko preload skipped: {e}"
                        ),
                    }
                }
            }
            Ok(())
        })
        .await
        .map_err(|e| ProviderError::Other(e.to_string()))?
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Ocr { image_png, .. } = input else {
            return Err(ProviderError::Other(
                "OarOcrProvider handles Ocr only".into(),
            ));
        };

        let multi_cache = self.multi.clone();
        let korean_cache = self.korean.clone();
        let models_dir = self.models_dir.clone();
        let use_gpu = self.use_gpu;
        let enable_korean = self.enable_korean;
        let provider_id = self.id.clone();

        let lines = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<OcrLine>> {
            // Construct a borrow-only stub so try_load_* can reuse
            // its config-extraction methods; engines are stored in
            // the shared `Arc<Mutex>` caches the caller passed in.
            let cfg = OarOcrProvider {
                id: provider_id.clone(),
                models_dir: models_dir.clone(),
                use_gpu,
                enable_korean,
                multi: multi_cache.clone(),
                korean: korean_cache.clone(),
            };

            let mut multi_guard = multi_cache.lock();
            if multi_guard.is_none() {
                let start = std::time::Instant::now();
                *multi_guard = Some(cfg.try_load_multi()?);
                tracing::info!(
                    "paddleocr multi engine loaded in {}ms (gpu={})",
                    start.elapsed().as_millis(),
                    use_gpu
                );
            }
            let multi = multi_guard.as_ref().expect("just inserted");

            // Decode source image.
            let img = image::load_from_memory(&image_png)?;
            let rgb = img.to_rgb8();
            let img_w = img.width() as f32;
            let img_h = img.height() as f32;

            // Run multilingual rec.
            let multi_results = multi
                .predict(vec![rgb.clone()])
                .map_err(|e| anyhow::anyhow!("paddleocr multi predict: {e}"))?;
            drop(multi_guard);

            // Build the working set of `Boxed` lines (extents +
            // multi-rec text + confidence).
            let mut boxes: Vec<Boxed> = Vec::new();
            for r in &multi_results {
                for region in &r.text_regions {
                    if let Some(b) = boxed_from_oar_region(region) {
                        if !b.text.is_empty() {
                            boxes.push(b);
                        }
                    }
                }
            }

            // Optional Korean override: for any bbox whose ko-rec
            // output contains Hangul, swap in the Korean reading.
            // Detection is shared so we expect 1:1 bbox alignment
            // (same det.onnx). If counts differ for any reason we
            // fall back to multi-only without trying to match.
            if enable_korean {
                let mut ko_guard = korean_cache.lock();
                if ko_guard.is_none() {
                    match cfg.try_load_korean() {
                        Ok(e) => *ko_guard = Some(e),
                        Err(e) => tracing::warn!("paddleocr ko engine unavailable: {e}"),
                    }
                }
                if let Some(ko_engine) = ko_guard.as_ref() {
                    let ko_results = ko_engine.predict(vec![rgb]).map_err(|e| {
                        anyhow::anyhow!("paddleocr ko predict: {e}")
                    })?;
                    let ko_boxes: Vec<Boxed> = ko_results
                        .iter()
                        .flat_map(|r| r.text_regions.iter())
                        .filter_map(boxed_from_oar_region)
                        .collect();
                    if ko_boxes.len() == boxes.len() {
                        for (i, ko) in ko_boxes.into_iter().enumerate() {
                            if contains_hangul(&ko.text) {
                                boxes[i].text = ko.text;
                                boxes[i].confidence = ko.confidence;
                            }
                        }
                    } else {
                        tracing::debug!(
                            "paddleocr ko bbox count {} != multi {} — skipping ko override",
                            ko_boxes.len(),
                            boxes.len()
                        );
                    }
                }
            }

            // Tategaki reading-order pass.
            let reordered = reorder_for_vertical_ja(boxes);

            Ok(reordered
                .into_iter()
                .map(|b| OcrLine {
                    text: b.text,
                    bbox: [
                        b.x_min.max(0.0),
                        b.y_min.max(0.0),
                        (b.x_max - b.x_min).max(0.0).min(img_w),
                        (b.y_max - b.y_min).max(0.0).min(img_h),
                    ],
                    confidence: b.confidence,
                })
                .collect())
        })
        .await
        .map_err(|e| ProviderError::Other(e.to_string()))?
        .map_err(|e| ProviderError::Other(e.to_string()))?;

        if lines.is_empty() {
            return Err(ProviderError::LowConfidence);
        }
        Ok(TaskOutput::Ocr { lines })
    }
}

// --- bbox + reorder + Hangul detection ------------------------------------

/// Intermediate format used between detection and the post-pass.
#[derive(Debug, Clone)]
struct Boxed {
    text: String,
    confidence: f32,
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
    let (text, confidence) = match region.text_with_confidence() {
        Some((t, c)) => (t.trim().to_string(), c),
        None => (String::new(), 0.0),
    };
    Some(Boxed {
        text,
        confidence,
        x_min,
        y_min,
        x_max,
        y_max,
    })
}

/// `true` when the string contains any Hangul codepoint (modern
/// syllables U+AC00..D7AF or Jamo U+1100..11FF). Used to decide
/// whether the Korean recogniser's output is preferred for a given
/// bbox over the multilingual one. Verified by the iter5 bench:
/// Korean rec never emits Hangul on non-Korean inputs.
fn contains_hangul(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0xAC00..=0xD7AF).contains(&u) || (0x1100..=0x11FF).contains(&u)
    })
}

/// Detect tategaki layouts and reorder boxes into Japanese vertical
/// reading order (top→bottom within a column, right→left across
/// columns). Returns the boxes untouched when the layout is not
/// vertical — so callers can call this unconditionally. Identical
/// heuristic to the bench (iter4), kept inline so this provider has
/// no test-only dependency.
fn reorder_for_vertical_ja(boxes: Vec<Boxed>) -> Vec<Boxed> {
    if boxes.len() < 4 {
        return boxes;
    }
    let char_like = boxes.iter().filter(|b| b.h() >= b.w() * 0.5).count();
    if char_like * 2 < boxes.len() {
        return boxes;
    }

    let mut widths: Vec<f32> = boxes.iter().map(|b| b.w()).collect();
    widths.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_w = widths[widths.len() / 2];
    let col_gap = (median_w * 0.7).max(8.0);

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

    for col in &mut columns {
        col.sort_by(|&a, &b| boxes[a].y_min.partial_cmp(&boxes[b].y_min).unwrap());
    }
    columns.sort_by(|a, b| boxes[b[0]].cx().partial_cmp(&boxes[a[0]].cx()).unwrap());

    let order: Vec<usize> = columns.into_iter().flatten().collect();
    let mut owned = boxes;
    let mut out: Vec<Boxed> = Vec::with_capacity(owned.len());
    for i in order {
        out.push(std::mem::replace(
            &mut owned[i],
            Boxed {
                text: String::new(),
                confidence: 0.0,
                x_min: 0.0,
                y_min: 0.0,
                x_max: 0.0,
                y_max: 0.0,
            },
        ));
    }
    out
}

// --- Model download --------------------------------------------------------

/// Progress callback signature, mirroring `ocr_ocrs::ProgressCb`.
pub type ProgressCb = dyn Fn(&str, u64, Option<u64>) + Send + Sync;

/// Download all 5 PP-OCRv5 model files into `models_dir`. Existing
/// non-empty files are kept (so re-running is cheap).
pub async fn install_models_with_progress(
    models_dir: &Path,
    install_korean: bool,
    progress: Option<&ProgressCb>,
) -> anyhow::Result<()> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    std::fs::create_dir_all(models_dir)?;
    let client = reqwest::Client::new();

    let mut files: Vec<(&str, &str)> = vec![
        (DET_URL, DET_FILE),
        (REC_MULTI_URL, REC_MULTI_FILE),
        (DICT_MULTI_URL, DICT_MULTI_FILE),
    ];
    if install_korean {
        files.push((REC_KO_URL, REC_KO_FILE));
        files.push((DICT_KO_URL, DICT_KO_FILE));
    }

    for (url, filename) in files {
        let path = models_dir.join(filename);
        if path.exists()
            && std::fs::metadata(&path)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
            if let Some(cb) = progress {
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                cb(filename, size, Some(size));
            }
            continue;
        }
        let resp = client.get(url).send().await?.error_for_status()?;
        let total = resp.content_length();
        let mut stream = resp.bytes_stream();
        let tmp_path = path.with_extension(format!(
            "{}.part",
            path.extension().and_then(|s| s.to_str()).unwrap_or("dat")
        ));
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        let mut downloaded: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if let Some(cb) = progress {
                cb(filename, downloaded, total);
            }
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&tmp_path, &path).await?;
    }

    // PaddleOCR `use_space_char=True` convention: the multilingual
    // and Korean recognisers were trained with space appended at the
    // end of the dict. The Baidu-shipped dict files do not include
    // it, so we patch it in here. Idempotent — only appends if the
    // last line isn't already a single space.
    for filename in [DICT_MULTI_FILE, DICT_KO_FILE] {
        let p = models_dir.join(filename);
        if !p.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&p)?;
        let needs_space = !content.ends_with(" \n") && !content.ends_with(" ");
        if needs_space {
            let mut buf = content;
            if !buf.ends_with('\n') {
                buf.push('\n');
            }
            buf.push(' ');
            buf.push('\n');
            std::fs::write(&p, buf)?;
        }
    }

    Ok(())
}

pub async fn install_models(models_dir: &Path, install_korean: bool) -> anyhow::Result<()> {
    install_models_with_progress(models_dir, install_korean, None).await
}

// --- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hangul_detection() {
        assert!(contains_hangul("동해 물과 백두산이"));
        assert!(contains_hangul("Hello 안녕"));
        assert!(!contains_hangul("Hello world"));
        assert!(!contains_hangul("こんにちは"));
        assert!(!contains_hangul("你好"));
        assert!(!contains_hangul(""));
    }

    fn b(text: &str, x: f32, y: f32, w: f32, h: f32) -> Boxed {
        Boxed {
            text: text.into(),
            confidence: 1.0,
            x_min: x,
            y_min: y,
            x_max: x + w,
            y_max: y + h,
        }
    }

    #[test]
    fn reorder_passes_through_horizontal() {
        // Two horizontal lines, sentence-shaped (wide boxes). Should
        // be returned unchanged.
        let input = vec![
            b("Hello world", 10.0, 10.0, 200.0, 20.0),
            b("Second line", 10.0, 40.0, 200.0, 20.0),
        ];
        let out = reorder_for_vertical_ja(input.clone());
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].text, "Hello world");
        assert_eq!(out[1].text, "Second line");
    }

    #[test]
    fn reorder_reads_tategaki_right_to_left() {
        // Two columns of three char-cells each. Column 0 is the
        // rightmost (read first), column 1 is to its left.
        // Layout:
        //   col1=x:10  col0=x:40
        //   y:10  い     あ
        //   y:40  ろ     い
        //   y:70  は     う
        // Reading order: あ→い→う (col 0 top-down), then い→ろ→は (col 1)
        let cells = vec![
            // col 1 (leftmost) — provided FIRST to ensure reorder
            // does not depend on input order
            b("い", 10.0, 10.0, 22.0, 22.0),
            b("ろ", 10.0, 40.0, 22.0, 22.0),
            b("は", 10.0, 70.0, 22.0, 22.0),
            // col 0 (rightmost)
            b("あ", 40.0, 10.0, 22.0, 22.0),
            b("い", 40.0, 40.0, 22.0, 22.0),
            b("う", 40.0, 70.0, 22.0, 22.0),
        ];
        let out = reorder_for_vertical_ja(cells);
        let joined: String = out.iter().map(|b| b.text.as_str()).collect();
        assert_eq!(joined, "あいういろは");
    }
}
