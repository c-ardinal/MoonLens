//! Multilingual OCR with PP-OCRv5 (PaddleOCR) models running on `rten`.
//!
//! `ocrs` only recognises Latin script. PP-OCRv5's mobile models read
//! Chinese, Japanese (kana + kanji) and English with one recognition
//! model, so this provider is what makes Japanese / Chinese screens
//! translatable. It runs the classic two-stage PaddleOCR pipeline:
//!
//! 1. **Detection** (DB): a probability map of "text pixels" is
//!    thresholded, connected regions become text-line boxes, and each
//!    box is expanded ("unclipped") back to the full glyph extent.
//! 2. **Recognition** (SVTR + CTC): every box is cropped, resized to a
//!    fixed height and decoded greedily against the model dictionary.
//!
//! The post-processing is pure Rust in [`post`] and unit tested; model
//! I/O lives in [`engine`].
//!
//! Models are downloaded on demand (see [`install_models_with_progress`])
//! from the npm registry tarball of `@rustors/model-ppocrv5-mobile`
//! (MIT, PP-OCRv5 weights are Apache-2.0), verified against a pinned
//! SHA-512, and stored next to the ocrs models.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::{Capability, OcrLine, Provider, ProviderError, TaskInput, TaskOutput};

/// Pinned model bundle. The registry tarball is immutable for a given
/// version and the integrity hash is checked after download.
pub const MODEL_TARBALL_URL: &str =
    "https://registry.npmjs.org/@rustors/model-ppocrv5-mobile/-/model-ppocrv5-mobile-0.3.2.tgz";
pub const MODEL_TARBALL_SHA512_B64: &str =
    "MncPfV5vObh5WwfJExZZ28fkPlwcuENU9ghCLpOQUoWSwMz6J2B+VZZ3eiTkzl+EsdgezQloqd/u7n3pMVhuiw==";

pub const DET_FILE: &str = "ppocrv5-det.rten";
pub const REC_FILE: &str = "ppocrv5-rec.rten";
pub const DICT_FILE: &str = "ppocrv5-dict.txt";

/// Files inside the tarball → names we store them under.
const TARBALL_FILES: [(&str, &str); 3] = [
    ("package/models/det.rten", DET_FILE),
    ("package/models/rec.rten", REC_FILE),
    ("package/models/dict.txt", DICT_FILE),
];

pub fn default_models_dir() -> Option<PathBuf> {
    super::ocr_ocrs::default_models_dir()
}

/// True when all three model files are present and non-empty.
pub fn models_installed(dir: &Path) -> bool {
    TARBALL_FILES.iter().all(|(_, f)| {
        std::fs::metadata(dir.join(f))
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    })
}

/// Tunables, read from `[providers.configs.ppocr]`.
#[derive(Debug, Clone, Copy)]
pub struct Params {
    /// Longest side the detector sees. Larger finds smaller text but is
    /// slower; screenshots need more than PaddleOCR's 960 default.
    pub det_max_side: u32,
    /// Probability above which a pixel counts as text.
    pub det_threshold: f32,
    /// Minimum mean probability for a region to be kept.
    pub box_threshold: f32,
    /// DB unclip ratio (how far boxes grow back out).
    pub unclip_ratio: f32,
    /// Lines whose mean character confidence is below this are dropped.
    pub min_confidence: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            det_max_side: 1600,
            det_threshold: 0.3,
            box_threshold: 0.6,
            unclip_ratio: 1.6,
            min_confidence: 0.5,
        }
    }
}

impl Params {
    pub fn from_config(cfg: &serde_json::Value) -> Self {
        let d = Self::default();
        let f = |k: &str, v: f32| {
            cfg.get(k)
                .and_then(|x| x.as_f64())
                .map(|x| x as f32)
                .unwrap_or(v)
        };
        Self {
            det_max_side: cfg
                .get("det_max_side")
                .and_then(|x| x.as_u64())
                .map(|x| x.clamp(320, 4096) as u32)
                .unwrap_or(d.det_max_side),
            det_threshold: f("det_threshold", d.det_threshold),
            box_threshold: f("box_threshold", d.box_threshold),
            unclip_ratio: f("unclip_ratio", d.unclip_ratio),
            min_confidence: f("min_confidence", d.min_confidence),
        }
    }
}

pub mod post {
    //! Pure pre/post-processing for the PP-OCR pipeline.

    /// Detector input size: scale so the longest side is at most
    /// `max_side` (never upscale), then round each side to a multiple
    /// of 32 as the DB backbone requires.
    pub fn det_input_size(w: u32, h: u32, max_side: u32) -> (u32, u32) {
        let long = w.max(h).max(1) as f32;
        let scale = (max_side as f32 / long).min(1.0);
        let r = |v: u32| (((v as f32 * scale) / 32.0).round() as u32).max(1) * 32;
        (r(w), r(h))
    }

    /// Axis-aligned box in pixel coordinates (`x0,y0` inclusive,
    /// `x1,y1` exclusive).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct DetBox {
        pub x0: f32,
        pub y0: f32,
        pub x1: f32,
        pub y1: f32,
        pub score: f32,
    }

    /// DB post-processing on a `w×h` probability map: threshold, find
    /// 8-connected regions, keep those whose mean probability is at
    /// least `box_threshold`, and expand each box by the DB unclip
    /// distance `area * ratio / perimeter`.
    pub fn boxes_from_prob_map(
        prob: &[f32],
        w: usize,
        h: usize,
        threshold: f32,
        box_threshold: f32,
        unclip_ratio: f32,
    ) -> Vec<DetBox> {
        let mut seen = vec![false; w * h];
        let mut out = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        for start in 0..w * h {
            if seen[start] || prob[start] <= threshold {
                continue;
            }
            seen[start] = true;
            stack.push(start);
            let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
            let mut sum = 0.0f32;
            let mut n = 0usize;
            while let Some(i) = stack.pop() {
                let (x, y) = (i % w, i / w);
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
                sum += prob[i];
                n += 1;
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = x as i64 + dx;
                        let ny = y as i64 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                            continue;
                        }
                        let j = ny as usize * w + nx as usize;
                        if !seen[j] && prob[j] > threshold {
                            seen[j] = true;
                            stack.push(j);
                        }
                    }
                }
            }
            let bw = (x1 - x0 + 1) as f32;
            let bh = (y1 - y0 + 1) as f32;
            // Tiny specks are noise (PaddleOCR's min_size = 3).
            if bw.min(bh) < 3.0 {
                continue;
            }
            let score = sum / n as f32;
            if score < box_threshold {
                continue;
            }
            let d = bw * bh * unclip_ratio / (2.0 * (bw + bh));
            out.push(DetBox {
                x0: (x0 as f32 - d).max(0.0),
                y0: (y0 as f32 - d).max(0.0),
                x1: (x1 as f32 + 1.0 + d).min(w as f32),
                y1: (y1 as f32 + 1.0 + d).min(h as f32),
                score,
            });
        }
        out
    }

    /// Grow a raw DB region by the unclip distance `area * ratio /
    /// perimeter` (the inverse of the shrink applied at training time).
    pub fn unclip(b: &DetBox, ratio: f32, max_w: f32, max_h: f32) -> DetBox {
        let (w, h) = (b.x1 - b.x0, b.y1 - b.y0);
        let d = if w + h > 0.0 {
            w * h * ratio / (2.0 * (w + h))
        } else {
            0.0
        };
        DetBox {
            x0: (b.x0 - d).max(0.0),
            y0: (b.y0 - d).max(0.0),
            x1: (b.x1 + d).min(max_w),
            y1: (b.y1 + d).min(max_h),
            score: b.score,
        }
    }

    /// Recognition input width for a crop resized to `height`.
    pub fn rec_width(crop_w: u32, crop_h: u32, height: u32, max_w: u32) -> u32 {
        let w = (crop_w as f32 * height as f32 / crop_h.max(1) as f32).ceil() as u32;
        w.clamp(height / 2, max_w)
    }

    /// Greedy CTC decode of one `steps × classes` row-major score
    /// matrix. Class 0 is the CTC blank; class `i` maps to `dict[i-1]`;
    /// the class one past the dictionary is a space. Returns the text
    /// and the mean probability of the emitted characters.
    pub fn ctc_greedy_decode(
        scores: &[f32],
        steps: usize,
        classes: usize,
        dict: &[String],
    ) -> (String, f32) {
        let mut text = String::new();
        let mut conf_sum = 0.0f32;
        let mut emitted = 0usize;
        let mut prev = 0usize;
        for t in 0..steps {
            let row = &scores[t * classes..(t + 1) * classes];
            let (best, p) =
                row.iter()
                    .copied()
                    .enumerate()
                    .fold(
                        (0usize, f32::MIN),
                        |acc, (i, v)| if v > acc.1 { (i, v) } else { acc },
                    );
            if best != 0 && best != prev {
                if let Some(ch) = dict.get(best - 1) {
                    text.push_str(ch);
                } else if best == dict.len() + 1 {
                    text.push(' ');
                }
                conf_sum += p;
                emitted += 1;
            }
            prev = best;
        }
        let conf = if emitted == 0 {
            0.0
        } else {
            conf_sum / emitted as f32
        };
        (text.trim().to_string(), conf)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn det_size_is_capped_and_multiple_of_32() {
            assert_eq!(det_input_size(820, 360, 1600), (832, 352));
            assert_eq!(det_input_size(3840, 2160, 1600), (1600, 896));
            let (w, h) = det_input_size(10, 10, 1600);
            assert_eq!((w % 32, h % 32), (0, 0));
            assert!(w >= 32 && h >= 32);
        }

        #[test]
        fn prob_map_regions_become_boxes() {
            // 20x10 map with two separate text blobs.
            let (w, h) = (20usize, 10usize);
            let mut p = vec![0.0f32; w * h];
            for y in 2..6 {
                for x in 1..8 {
                    p[y * w + x] = 0.9;
                }
                for x in 12..18 {
                    p[y * w + x] = 0.8;
                }
            }
            // A low-confidence blob that must be dropped.
            p[8 * w + 2] = 0.35;
            p[8 * w + 3] = 0.35;
            p[8 * w + 4] = 0.35;
            p[9 * w + 2] = 0.35;
            p[9 * w + 3] = 0.35;
            p[9 * w + 4] = 0.35;
            p[7 * w + 2] = 0.35;
            p[7 * w + 3] = 0.35;
            p[7 * w + 4] = 0.35;
            let mut boxes = boxes_from_prob_map(&p, w, h, 0.3, 0.6, 0.0);
            boxes.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap());
            assert_eq!(boxes.len(), 2);
            assert_eq!(
                (boxes[0].x0, boxes[0].y0, boxes[0].x1, boxes[0].y1),
                (1.0, 2.0, 8.0, 6.0)
            );
            assert_eq!((boxes[1].x0, boxes[1].x1), (12.0, 18.0));
            assert!((boxes[1].score - 0.8).abs() < 1e-6);
        }

        #[test]
        fn unclip_expands_and_clamps() {
            let (w, h) = (10usize, 10usize);
            let mut p = vec![0.0f32; w * h];
            for y in 0..4 {
                for x in 0..4 {
                    p[y * w + x] = 1.0;
                }
            }
            let b = boxes_from_prob_map(&p, w, h, 0.3, 0.6, 2.0);
            // d = 16 * 2 / 16 = 2
            assert_eq!((b[0].x0, b[0].y0, b[0].x1, b[0].y1), (0.0, 0.0, 6.0, 6.0));
        }

        #[test]
        fn unclip_helper_matches_db_distance() {
            let raw = DetBox {
                x0: 10.0,
                y0: 10.0,
                x1: 14.0,
                y1: 14.0,
                score: 1.0,
            };
            let b = unclip(&raw, 2.0, 100.0, 100.0);
            assert_eq!((b.x0, b.y0, b.x1, b.y1), (8.0, 8.0, 16.0, 16.0));
            let clamped = unclip(&raw, 20.0, 15.0, 15.0);
            assert_eq!((clamped.x0, clamped.x1), (0.0, 15.0));
        }

        #[test]
        fn rec_width_keeps_aspect_and_clamps() {
            assert_eq!(rec_width(200, 20, 48, 3200), 480);
            assert_eq!(rec_width(5, 40, 48, 3200), 24);
            assert_eq!(rec_width(10000, 10, 48, 3200), 3200);
        }

        #[test]
        fn ctc_collapses_repeats_and_blanks() {
            let dict: Vec<String> = ["a", "b", "日"].iter().map(|s| s.to_string()).collect();
            let classes = dict.len() + 2; // blank + dict + space
                                          // steps: a a blank a b space 日
            let seq = [1usize, 1, 0, 1, 2, 4, 3];
            let mut scores = vec![0.0f32; seq.len() * classes];
            for (t, &c) in seq.iter().enumerate() {
                scores[t * classes + c] = 0.9;
            }
            let (text, conf) = ctc_greedy_decode(&scores, seq.len(), classes, &dict);
            assert_eq!(text, "aab 日");
            assert!((conf - 0.9).abs() < 1e-6);
        }

        #[test]
        fn ctc_all_blank_is_empty() {
            let dict = vec!["a".to_string()];
            let scores = vec![0.9, 0.1, 0.0, 0.9, 0.1, 0.0];
            assert_eq!(
                ctc_greedy_decode(&scores, 2, 3, &dict),
                (String::new(), 0.0)
            );
        }
    }
}

#[cfg(feature = "ocrs-engine")]
pub mod engine {
    //! Model loading and inference.

    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use anyhow::{anyhow, Context, Result};
    use image::{imageops::FilterType, DynamicImage, RgbImage};
    use parking_lot::Mutex;
    use rten_tensor::prelude::*;
    use rten_tensor::{NdTensor, Tensor};

    use super::{post, Params, DET_FILE, DICT_FILE, REC_FILE};
    use crate::providers::OcrLine;

    const REC_HEIGHT: u32 = 48;
    const REC_MAX_WIDTH: u32 = 3200;
    const REC_BATCH: usize = 1;
    // The line cache relies on a line's text depending on its own pixels
    // only. In a larger batch the padding to the widest line would make
    // it depend on its batch mates.
    const _: () = assert!(REC_BATCH == 1);
    const REC_WORKERS: usize = 4;
    /// Unclip ratio for the bbox we report (≈ the visible glyph extent).
    const REPORT_UNCLIP_RATIO: f32 = 0.9;

    /// Decoded `(text, confidence)` per crop of one recognition batch.
    type BatchResult = Result<Vec<(String, f32)>>;

    /// Recognised lines kept for reuse (see [`LineCache`]). A full 4K
    /// screen has a few hundred lines; this holds several screens.
    const LINE_CACHE_SIZE: usize = 2048;

    pub struct PpOcr {
        det: rten::Model,
        rec: rten::Model,
        dict: Vec<String>,
        lines: Mutex<LineCache>,
    }

    /// Text already recognised from a line image, by the image's pixels.
    ///
    /// A watched screen region mostly keeps its lines from one reading
    /// to the next (a chat gains a message, a dialogue box changes while
    /// the UI around it stays). Recognition takes most of the OCR time
    /// and gives the same text for the same pixels, so a line whose crop
    /// is pixel for pixel one read before is not recognised again.
    /// Detection still runs on every reading.
    #[derive(Default)]
    pub(super) struct LineCache {
        map: std::collections::HashMap<u64, (String, f32, u64)>,
        /// Counts lookups; entries remember when they were last used.
        clock: u64,
    }

    impl LineCache {
        /// Identifies a line crop by size and pixels.
        pub(super) fn key(crop: &RgbImage) -> u64 {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (crop.width(), crop.height()).hash(&mut h);
            crop.as_raw().hash(&mut h);
            h.finish()
        }

        pub(super) fn get(&mut self, key: u64) -> Option<(String, f32)> {
            self.clock += 1;
            let e = self.map.get_mut(&key)?;
            e.2 = self.clock;
            Some((e.0.clone(), e.1))
        }

        pub(super) fn put(&mut self, key: u64, text: String, confidence: f32, cap: usize) {
            self.clock += 1;
            self.map.insert(key, (text, confidence, self.clock));
            if self.map.len() > cap {
                // Drop the least recently used half in one pass.
                let mut used: Vec<u64> = self.map.values().map(|e| e.2).collect();
                used.sort_unstable();
                let cut = used[used.len() / 2];
                self.map.retain(|_, e| e.2 > cut);
            }
        }
    }

    static CACHE: Mutex<Option<(PathBuf, Arc<PpOcr>)>> = Mutex::new(None);

    /// Cached engine for `dir` (models load once per process).
    pub fn engine_for(dir: &Path) -> Result<Arc<PpOcr>> {
        let mut guard = CACHE.lock();
        if let Some((d, e)) = guard.as_ref() {
            if d == dir {
                return Ok(e.clone());
            }
        }
        let e = Arc::new(PpOcr::load(dir)?);
        *guard = Some((dir.to_path_buf(), e.clone()));
        Ok(e)
    }

    impl PpOcr {
        pub fn load(dir: &Path) -> Result<Self> {
            let det = rten::Model::load_file(dir.join(DET_FILE))
                .map_err(|e| anyhow!("load {DET_FILE}: {e}"))?;
            let rec = rten::Model::load_file(dir.join(REC_FILE))
                .map_err(|e| anyhow!("load {REC_FILE}: {e}"))?;
            let dict = std::fs::read_to_string(dir.join(DICT_FILE))
                .with_context(|| format!("read {DICT_FILE}"))?
                .lines()
                .map(|l| l.trim_end_matches('\r').to_string())
                .collect();
            Ok(Self {
                det,
                rec,
                dict,
                lines: Mutex::new(LineCache::default()),
            })
        }

        /// Full pipeline: returns lines with bboxes in `img` pixels.
        pub fn recognize(&self, img: &DynamicImage, p: &Params) -> Result<Vec<OcrLine>> {
            let rgb = img.to_rgb8();
            let t = std::time::Instant::now();
            let boxes = self.detect(&rgb, p)?;
            tracing::debug!("ppocr det: {} boxes in {:?}", boxes.len(), t.elapsed());
            let (fw, fh) = (rgb.width() as f32, rgb.height() as f32);
            let mut crops: Vec<(post::DetBox, RgbImage)> = boxes
                .into_iter()
                .filter_map(|raw| {
                    // Crop generously so glyph edges are intact, but report
                    // a tighter box: it is what the overlay paints over and
                    // what paragraph grouping measures line gaps with.
                    let b = post::unclip(&raw, p.unclip_ratio, fw, fh);
                    let reported = post::unclip(&raw, REPORT_UNCLIP_RATIO, fw, fh);
                    let x = b.x0.floor() as u32;
                    let y = b.y0.floor() as u32;
                    let w = (b.x1.ceil() as u32).min(rgb.width()).saturating_sub(x);
                    let h = (b.y1.ceil() as u32).min(rgb.height()).saturating_sub(y);
                    if w < 2 || h < 2 {
                        return None;
                    }
                    let mut crop = image::imageops::crop_imm(&rgb, x, y, w, h).to_image();
                    // Vertical text column: rotate so it reads left→right.
                    if h as f32 >= w as f32 * 1.5 && h > 2 * REC_HEIGHT / 3 {
                        crop = image::imageops::rotate270(&crop);
                    }
                    Some((reported, crop))
                })
                .collect();
            // Similar aspect ratios in one batch keep padding small.
            crops.sort_by(|a, b| {
                let ra = a.1.width() as f32 / a.1.height() as f32;
                let rb = b.1.width() as f32 / b.1.height() as f32;
                ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut lines = Vec::new();
            // Lines read before (same pixels) keep their text; only the
            // rest goes through recognition.
            let keys: Vec<u64> = crops.iter().map(|c| LineCache::key(&c.1)).collect();
            let mut known: Vec<Option<(String, f32)>> = {
                let mut cache = self.lines.lock();
                keys.iter().map(|k| cache.get(*k)).collect()
            };
            let fresh: Vec<usize> = (0..crops.len()).filter(|&i| known[i].is_none()).collect();
            tracing::debug!(
                "ppocr rec: {} lines, {} from the line cache",
                crops.len(),
                crops.len() - fresh.len()
            );
            // Recognise lines on a few worker threads. rten parallelises
            // inside each op too, but one line is too small a workload
            // to keep every core busy; measured ~2× faster on 4 cores.
            let workers = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(2)
                .clamp(1, REC_WORKERS);
            let chunks: Vec<&[usize]> = fresh.chunks(REC_BATCH).collect();
            let next = std::sync::atomic::AtomicUsize::new(0);
            let results: Mutex<Vec<(usize, BatchResult)>> = Mutex::new(Vec::new());
            std::thread::scope(|scope| {
                for _ in 0..workers.min(chunks.len()) {
                    scope.spawn(|| loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(chunk) = chunks.get(i) else { break };
                        let r = self.recognize_batch(chunk.iter().map(|&c| &crops[c].1));
                        results.lock().push((i, r));
                    });
                }
            });
            let mut results = results.into_inner();
            results.sort_by_key(|(i, _)| *i);
            {
                let mut cache = self.lines.lock();
                for (i, texts) in results {
                    for (&c, (text, conf)) in chunks[i].iter().zip(texts?) {
                        cache.put(keys[c], text.clone(), conf, LINE_CACHE_SIZE);
                        known[c] = Some((text, conf));
                    }
                }
            }
            for ((b, _), read) in crops.iter().zip(known) {
                let Some((text, conf)) = read else { continue };
                if text.is_empty() || conf < p.min_confidence {
                    continue;
                }
                lines.push(OcrLine {
                    text,
                    bbox: [b.x0, b.y0, b.x1 - b.x0, b.y1 - b.y0],
                    confidence: conf,
                });
            }
            Ok(lines)
        }

        fn detect(&self, rgb: &RgbImage, p: &Params) -> Result<Vec<post::DetBox>> {
            let (iw, ih) = (rgb.width(), rgb.height());
            let (dw, dh) = post::det_input_size(iw, ih, p.det_max_side);
            let resized = image::imageops::resize(rgb, dw, dh, FilterType::Triangle);
            const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
            const STD: [f32; 3] = [0.229, 0.224, 0.225];
            // PaddleOCR feeds BGR (OpenCV) with these means per channel.
            let input =
                NdTensor::<f32, 4>::from_fn([1, 3, dh as usize, dw as usize], |[_, c, y, x]| {
                    let px = resized.get_pixel(x as u32, y as u32).0;
                    let v = px[2 - c] as f32 / 255.0;
                    (v - MEAN[c]) / STD[c]
                });
            let input: Tensor<f32> = input.into();
            let out: Tensor<f32> = self
                .det
                .run_one(input.view().into(), None)
                .map_err(|e| anyhow!("det run: {e}"))?
                .try_into()
                .map_err(|e| anyhow!("det output: {e:?}"))?;
            let shape = out.shape().to_vec();
            let (ph, pw) = (shape[shape.len() - 2], shape[shape.len() - 1]);
            let prob: Vec<f32> = out.iter().copied().collect();
            let sx = iw as f32 / pw as f32;
            let sy = ih as f32 / ph as f32;
            Ok(
                post::boxes_from_prob_map(&prob, pw, ph, p.det_threshold, p.box_threshold, 0.0)
                    .into_iter()
                    .map(|b| post::DetBox {
                        x0: b.x0 * sx,
                        y0: b.y0 * sy,
                        x1: (b.x1 * sx).min(iw as f32),
                        y1: (b.y1 * sy).min(ih as f32),
                        score: b.score,
                    })
                    .collect(),
            )
        }

        fn recognize_batch<'a, I: Iterator<Item = &'a RgbImage>>(
            &self,
            crops: I,
        ) -> Result<Vec<(String, f32)>> {
            let resized: Vec<RgbImage> = crops
                .map(|c| {
                    let w = post::rec_width(c.width(), c.height(), REC_HEIGHT, REC_MAX_WIDTH);
                    image::imageops::resize(c, w, REC_HEIGHT, FilterType::Triangle)
                })
                .collect();
            if resized.is_empty() {
                return Ok(vec![]);
            }
            let max_w = resized.iter().map(|r| r.width()).max().unwrap_or(1) as usize;
            let n = resized.len();
            let input =
                NdTensor::<f32, 4>::from_fn([n, 3, REC_HEIGHT as usize, max_w], |[i, c, y, x]| {
                    let r = &resized[i];
                    if x as u32 >= r.width() {
                        return 0.0;
                    }
                    let v = r.get_pixel(x as u32, y as u32).0[2 - c] as f32 / 255.0;
                    (v - 0.5) / 0.5
                });
            let input: Tensor<f32> = input.into();
            let out: Tensor<f32> = self
                .rec
                .run_one(input.view().into(), None)
                .map_err(|e| anyhow!("rec run: {e}"))?
                .try_into()
                .map_err(|e| anyhow!("rec output: {e:?}"))?;
            let shape = out.shape().to_vec();
            if shape.len() != 3 || shape[0] != n {
                anyhow::bail!("unexpected rec output shape {shape:?}");
            }
            let (steps, classes) = (shape[1], shape[2]);
            let data: Vec<f32> = out.iter().copied().collect();
            Ok((0..n)
                .map(|i| {
                    let slice = &data[i * steps * classes..(i + 1) * steps * classes];
                    post::ctc_greedy_decode(slice, steps, classes, &self.dict)
                })
                .collect())
        }
    }
}

pub struct PpOcrProvider {
    id: String,
    models_dir: PathBuf,
    params: Params,
}

impl PpOcrProvider {
    pub fn new(id: impl Into<String>, config: &serde_json::Value) -> Self {
        let models_dir = config
            .get("models_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .or_else(default_models_dir)
            .unwrap_or_else(|| PathBuf::from("./models"));
        Self {
            id: id.into(),
            models_dir,
            params: Params::from_config(config),
        }
    }
}

#[async_trait]
impl Provider for PpOcrProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Ocr]
    }
    fn is_local(&self) -> bool {
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Ocr { image_png, .. } = input else {
            return Err(ProviderError::Other(
                "PpOcrProvider handles Ocr only".into(),
            ));
        };
        #[cfg(feature = "ocrs-engine")]
        {
            if !models_installed(&self.models_dir) {
                // Let the chain fall through to ocrs until the user
                // installs the multilingual models.
                return Err(ProviderError::NotInstalled("PP-OCRv5".into()));
            }
            let dir = self.models_dir.clone();
            let params = self.params;
            let lines =
                tokio::task::spawn_blocking(move || -> Result<Vec<OcrLine>, ProviderError> {
                    let engine = engine::engine_for(&dir).map_err(|e| {
                        tracing::warn!("PP-OCR models unavailable: {e}");
                        ProviderError::LowConfidence
                    })?;
                    let img = image::load_from_memory(&image_png)
                        .map_err(|e| ProviderError::Other(e.to_string()))?;
                    engine
                        .recognize(&img, &params)
                        .map_err(|e| ProviderError::Other(e.to_string()))
                })
                .await
                .map_err(|e| ProviderError::Other(e.to_string()))??;
            if lines.is_empty() {
                return Err(ProviderError::LowConfidence);
            }
            Ok(TaskOutput::Ocr { lines })
        }
        #[cfg(not(feature = "ocrs-engine"))]
        {
            let _ = (image_png, &self.models_dir, self.params);
            Err(ProviderError::LowConfidence)
        }
    }
}

/// Download, verify and unpack the PP-OCRv5 model bundle into
/// `models_dir`. `progress(filename, downloaded, total)` is called as
/// bytes arrive. No-op when the models are already installed.
#[cfg(feature = "ocrs-engine")]
pub async fn install_models_with_progress(
    models_dir: &Path,
    progress: Option<&super::ocr_ocrs::ProgressCb>,
) -> anyhow::Result<()> {
    use futures::StreamExt;

    if models_installed(models_dir) {
        return Ok(());
    }
    std::fs::create_dir_all(models_dir)?;
    let label = "ppocrv5-mobile";
    let resp = reqwest::Client::new()
        .get(MODEL_TARBALL_URL)
        .send()
        .await?
        .error_for_status()?;
    let total = resp.content_length();
    let mut stream = resp.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(total.unwrap_or(16 << 20) as usize);
    while let Some(chunk) = stream.next().await {
        buf.extend_from_slice(&chunk?);
        if let Some(cb) = progress {
            cb(label, buf.len() as u64, total);
        }
    }
    let dir = models_dir.to_path_buf();
    tokio::task::spawn_blocking(move || verify_and_unpack(&buf, &dir)).await??;
    Ok(())
}

/// Check the bundle against the pinned SHA-512, then unpack it.
#[cfg(feature = "ocrs-engine")]
pub fn verify_and_unpack(tgz: &[u8], dir: &Path) -> anyhow::Result<()> {
    use base64::Engine as _;
    use sha2::{Digest, Sha512};
    let digest = base64::engine::general_purpose::STANDARD.encode(Sha512::digest(tgz));
    if digest != MODEL_TARBALL_SHA512_B64 {
        anyhow::bail!("PP-OCR model bundle failed integrity check");
    }
    unpack(tgz, dir)
}

#[cfg(feature = "ocrs-engine")]
fn unpack(tgz: &[u8], dir: &Path) -> anyhow::Result<()> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tgz));
    let mut found = 0;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().to_string();
        if let Some((_, name)) = TARBALL_FILES.iter().find(|(src, _)| *src == path) {
            // Write to a temp name then rename so a crash never leaves a
            // truncated model that looks installed.
            let tmp = dir.join(format!("{name}.part"));
            let mut f = std::fs::File::create(&tmp)?;
            std::io::copy(&mut entry, &mut f)?;
            drop(f);
            std::fs::rename(&tmp, dir.join(name))?;
            found += 1;
        }
    }
    if found != TARBALL_FILES.len() {
        anyhow::bail!("PP-OCR model bundle is missing files ({found}/3)");
    }
    Ok(())
}

#[cfg(all(test, feature = "ocrs-engine"))]
mod tests {
    use super::*;

    #[test]
    fn unpack_extracts_only_known_files_and_requires_all() {
        let mut tgz = Vec::new();
        {
            let enc = flate2::write::GzEncoder::new(&mut tgz, flate2::Compression::fast());
            let mut b = tar::Builder::new(enc);
            for (src, _) in TARBALL_FILES {
                let data = b"model";
                let mut h = tar::Header::new_gnu();
                h.set_size(data.len() as u64);
                h.set_mode(0o644);
                h.set_cksum();
                b.append_data(&mut h, src, &data[..]).unwrap();
            }
            let mut h = tar::Header::new_gnu();
            h.set_size(1);
            h.set_cksum();
            b.append_data(&mut h, "package/README.md", &b"x"[..])
                .unwrap();
            b.into_inner().unwrap().finish().unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        assert!(!models_installed(dir.path()));
        unpack(&tgz, dir.path()).unwrap();
        assert!(models_installed(dir.path()));
        assert!(!dir.path().join("README.md").exists());

        let empty = tempfile::tempdir().unwrap();
        let mut bad = Vec::new();
        {
            let enc = flate2::write::GzEncoder::new(&mut bad, flate2::Compression::fast());
            let b = tar::Builder::new(enc);
            b.into_inner().unwrap().finish().unwrap();
        }
        assert!(unpack(&bad, empty.path()).is_err());
    }

    #[test]
    fn tampered_bundle_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        assert!(verify_and_unpack(b"not the bundle", dir.path()).is_err());
        assert!(!models_installed(dir.path()));
    }

    /// Needs the real bundle: download MODEL_TARBALL_URL and run
    /// `MOONLENS_PPOCR_TGZ=/path/to.tgz cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn real_bundle_verifies_unpacks_and_loads() {
        let tgz = std::fs::read(std::env::var("MOONLENS_PPOCR_TGZ").unwrap()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        verify_and_unpack(&tgz, dir.path()).unwrap();
        assert!(models_installed(dir.path()));
        let engine = engine::PpOcr::load(dir.path()).unwrap();
        let img = image::DynamicImage::new_rgb8(64, 32);
        assert!(engine
            .recognize(&img, &Params::default())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn line_cache_knows_a_line_by_its_pixels() {
        use engine::LineCache;
        use image::{Rgb, RgbImage};
        let a = RgbImage::from_pixel(40, 12, Rgb([10, 20, 30]));
        let mut b = a.clone();
        b.put_pixel(39, 11, Rgb([10, 20, 31]));
        // Same pixel data in another shape is another line.
        let c = RgbImage::from_pixel(12, 40, Rgb([10, 20, 30]));
        assert_eq!(LineCache::key(&a), LineCache::key(&a.clone()));
        assert_ne!(LineCache::key(&a), LineCache::key(&b));
        assert_ne!(LineCache::key(&a), LineCache::key(&c));

        let mut cache = LineCache::default();
        assert_eq!(cache.get(LineCache::key(&a)), None);
        cache.put(LineCache::key(&a), "Hello".into(), 0.9, 8);
        assert_eq!(cache.get(LineCache::key(&a)), Some(("Hello".into(), 0.9)));
        assert_eq!(cache.get(LineCache::key(&b)), None);
    }

    #[test]
    fn line_cache_drops_the_least_recently_used_lines_when_full() {
        use engine::LineCache;
        let mut cache = LineCache::default();
        for k in 0..8u64 {
            cache.put(k, k.to_string(), 1.0, 8);
        }
        // Line 0 is still on screen; the next new line overflows the cache.
        assert!(cache.get(0).is_some());
        cache.put(8, "8".into(), 1.0, 8);
        assert!(cache.get(0).is_some(), "a line in use was dropped");
        assert!(cache.get(8).is_some(), "the newest line was dropped");
        assert!(cache.get(1).is_none(), "the oldest unused line was kept");
        assert!((0..=8).filter(|k| cache.get(*k).is_some()).count() <= 8);
    }

    #[test]
    fn params_read_config_with_defaults() {
        let p =
            Params::from_config(&serde_json::json!({"det_max_side": 99999, "det_threshold": 0.4}));
        assert_eq!(p.det_max_side, 4096);
        assert!((p.det_threshold - 0.4).abs() < 1e-6);
        assert!((p.box_threshold - Params::default().box_threshold).abs() < 1e-6);
    }
}
