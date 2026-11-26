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
    /// What the networks run on (`backend`, default `"auto"`).
    pub backend: Backend,
}

/// What runs the networks. The text read is the same up to rounding
/// in the arithmetic; the speed is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Backend {
    /// The fastest of those below that load on this PC, measured when
    /// the models load.
    #[default]
    Auto,
    /// ONNX Runtime on the GPU through DirectML (Windows).
    DirectMl,
    /// ONNX Runtime on the CPU.
    OnnxCpu,
    /// rten on the CPU: needs nothing beyond the base models.
    Rten,
}

impl Backend {
    /// The name in the config and in the log.
    pub fn name(self) -> &'static str {
        match self {
            Backend::Auto => "auto",
            Backend::DirectMl => "directml",
            Backend::OnnxCpu => "onnx-cpu",
            Backend::Rten => "rten",
        }
    }

    /// Unknown names mean `auto`.
    pub fn from_name(name: &str) -> Self {
        [Backend::DirectMl, Backend::OnnxCpu, Backend::Rten]
            .into_iter()
            .find(|b| b.name() == name)
            .unwrap_or_default()
    }
}

impl Default for Params {
    fn default() -> Self {
        Self {
            det_max_side: 1600,
            det_threshold: 0.3,
            box_threshold: 0.6,
            unclip_ratio: 1.6,
            min_confidence: 0.5,
            backend: Backend::Auto,
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
            backend: cfg
                .get("backend")
                .and_then(|x| x.as_str())
                .map(Backend::from_name)
                .unwrap_or_default(),
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

    use super::{post, Backend, Params, DET_FILE, DICT_FILE, REC_FILE};
    use crate::providers::OcrLine;

    const REC_HEIGHT: u32 = 48;
    const REC_MAX_WIDTH: u32 = 3200;
    const REC_WORKERS: usize = 4;

    /// What the caller allows the networks to run on.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Want {
        pub backend: Backend,
        /// `performance.use_gpu_if_available`.
        pub gpu: bool,
    }

    impl Default for Want {
        fn default() -> Self {
            Self {
                backend: Backend::Auto,
                gpu: true,
            }
        }
    }

    impl Want {
        /// The backends to choose between. When none of them loads
        /// (or the list is empty) the models run on rten, which needs
        /// nothing that may be missing.
        pub(super) fn candidates(self) -> Vec<Backend> {
            let mut list = match self.backend {
                Backend::Auto => vec![Backend::DirectMl, Backend::OnnxCpu],
                Backend::Rten => vec![],
                one => vec![one],
            };
            if !(self.gpu && cfg!(windows)) {
                // No GPU to use: what was meant for it runs on the CPU.
                list.retain(|b| *b != Backend::DirectMl);
                if self.backend == Backend::DirectMl {
                    list.push(Backend::OnnxCpu);
                }
            }
            list
        }
    }

    /// How a backend wants its recognition work cut up.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct RecPlan {
        /// Threads that call the network at the same time.
        pub(super) workers: usize,
        /// Lines per call, all of one padded width.
        pub(super) batch: usize,
    }

    /// Line pictures are padded on the right to a multiple of this
    /// width, on every backend alike.
    ///
    /// The network was trained on padded lines, and short lines read
    /// better with blank space after them than cut flush (on the 4K
    /// fixture: `HEIP` → `Help`, `Learn t i` → `Learn to edit`). The
    /// step also keeps the number of input shapes small, which a GPU
    /// pays for one by one.
    ///
    /// The line cache relies on a line's text depending on its own
    /// pixels only, so lines never share a call with wider ones
    /// (padding to the widest would tie a line to its batch mates):
    /// the padding of a line follows from its own width alone.
    const REC_WIDTH_STEP: u32 = 64;

    pub(super) fn padded_width(width: u32) -> u32 {
        width.div_ceil(REC_WIDTH_STEP) * REC_WIDTH_STEP
    }

    /// The two networks on one backend. Tensors are `f32`, row-major.
    pub(super) trait Networks: Send + Sync {
        fn backend(&self) -> Backend;
        fn plan(&self) -> RecPlan;
        /// `[1, 3, h, w]` in, the probability map and its `(h, w)` out.
        fn det(&self, input: Vec<f32>, h: usize, w: usize) -> Result<(Vec<f32>, usize, usize)>;
        /// `[n, 3, REC_HEIGHT, w]` in, `[n, steps, classes]` and its
        /// `(steps, classes)` out.
        fn rec(&self, input: Vec<f32>, n: usize, w: usize) -> Result<(Vec<f32>, usize, usize)>;
    }

    struct RtenNetworks {
        det: rten::Model,
        rec: rten::Model,
    }

    impl RtenNetworks {
        fn load(dir: &Path) -> Result<Self> {
            Ok(Self {
                det: rten::Model::load_file(dir.join(DET_FILE))
                    .map_err(|e| anyhow!("load {DET_FILE}: {e}"))?,
                rec: rten::Model::load_file(dir.join(REC_FILE))
                    .map_err(|e| anyhow!("load {REC_FILE}: {e}"))?,
            })
        }

        fn run(model: &rten::Model, input: Vec<f32>, shape: [usize; 4]) -> Result<Tensor<f32>> {
            let input: Tensor<f32> = NdTensor::from_data(shape, input).into();
            model
                .run_one(input.view().into(), None)
                .map_err(|e| anyhow!("run: {e}"))?
                .try_into()
                .map_err(|e| anyhow!("output: {e:?}"))
        }
    }

    impl Networks for RtenNetworks {
        fn backend(&self) -> Backend {
            Backend::Rten
        }

        // rten parallelises inside each op, but one line is too small a
        // workload to keep every core busy: a few threads, each reading
        // one line at a time, measured ~2× faster on 4 cores.
        fn plan(&self) -> RecPlan {
            RecPlan {
                workers: REC_WORKERS,
                batch: 1,
            }
        }

        fn det(&self, input: Vec<f32>, h: usize, w: usize) -> Result<(Vec<f32>, usize, usize)> {
            let out = Self::run(&self.det, input, [1, 3, h, w])?;
            let shape = out.shape().to_vec();
            let (ph, pw) = (shape[shape.len() - 2], shape[shape.len() - 1]);
            Ok((out.iter().copied().collect(), ph, pw))
        }

        fn rec(&self, input: Vec<f32>, n: usize, w: usize) -> Result<(Vec<f32>, usize, usize)> {
            let out = Self::run(&self.rec, input, [n, 3, REC_HEIGHT as usize, w])?;
            let shape = out.shape().to_vec();
            if shape.len() != 3 || shape[0] != n {
                anyhow::bail!("unexpected rec output shape {shape:?}");
            }
            Ok((out.iter().copied().collect(), shape[1], shape[2]))
        }
    }

    #[cfg(feature = "ocr-onnx")]
    struct OnnxNetworks {
        models: crate::providers::ocr_onnx::runtime::Models,
        backend: Backend,
        plan: RecPlan,
    }

    #[cfg(feature = "ocr-onnx")]
    impl OnnxNetworks {
        /// The GPU takes many lines per call.
        const GPU_PLAN: RecPlan = RecPlan {
            workers: 1,
            batch: 16,
        };

        fn load(dir: &Path, backend: Backend) -> Result<Self> {
            use crate::providers::ocr_onnx::runtime::{Device, Models};
            Ok(if backend == Backend::DirectMl {
                Self {
                    models: Models::load(dir, Device::DirectMl, 1)?,
                    backend,
                    plan: Self::GPU_PLAN,
                }
            } else {
                // Line by line: a batch gains nothing on the CPU
                // (measured).
                let workers = cpu_rec_workers();
                Self {
                    models: Models::load(dir, Device::Cpu, workers)?,
                    backend,
                    plan: RecPlan { workers, batch: 1 },
                }
            })
        }
    }

    #[cfg(feature = "ocr-onnx")]
    impl Networks for OnnxNetworks {
        fn backend(&self) -> Backend {
            self.backend
        }

        fn plan(&self) -> RecPlan {
            self.plan
        }

        fn det(&self, input: Vec<f32>, h: usize, w: usize) -> Result<(Vec<f32>, usize, usize)> {
            let (data, shape) = self.models.det(input, h, w)?;
            if shape.len() < 2 {
                anyhow::bail!("unexpected det output shape {shape:?}");
            }
            Ok((data, shape[shape.len() - 2], shape[shape.len() - 1]))
        }

        fn rec(&self, input: Vec<f32>, n: usize, w: usize) -> Result<(Vec<f32>, usize, usize)> {
            let (data, shape) = self.models.rec(input, n, REC_HEIGHT as usize, w)?;
            if shape.len() != 3 || shape[0] != n {
                anyhow::bail!("unexpected rec output shape {shape:?}");
            }
            Ok((data, shape[1], shape[2]))
        }
    }

    /// Threads reading lines on the CPU through ONNX Runtime, each on
    /// a session of its own with one thread: for work as small as one
    /// line this beat one session spread over the cores (25 lines on
    /// 4 cores: about 0.3 s against 0.45 s).
    #[cfg(feature = "ocr-onnx")]
    fn cpu_rec_workers() -> usize {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .clamp(1, 6)
    }

    /// Why `backend` cannot be used with the models in `dir`: the load
    /// error, or the error of its first reading.
    #[cfg(test)]
    pub(super) fn unavailable(dir: &Path, backend: Backend) -> String {
        match networks(dir, backend) {
            Err(e) => format!("load: {e:#}"),
            Ok(n) => match n.det(vec![0.0; 3 * 320 * 480], 320, 480) {
                Err(e) => format!("run: {e:#}"),
                Ok(_) => "loads and runs".to_string(),
            },
        }
    }

    fn networks(dir: &Path, backend: Backend) -> Result<Box<dyn Networks>> {
        match backend {
            Backend::Rten => Ok(Box::new(RtenNetworks::load(dir)?)),
            #[cfg(feature = "ocr-onnx")]
            Backend::DirectMl | Backend::OnnxCpu => Ok(Box::new(OnnxNetworks::load(dir, backend)?)),
            other => Err(anyhow!("{} is not part of this build", other.name())),
        }
    }
    /// Unclip ratio for the bbox we report (≈ the visible glyph extent).
    const REPORT_UNCLIP_RATIO: f32 = 0.9;

    /// Decoded `(text, confidence)` per crop of one recognition batch.
    type BatchResult = Result<Vec<(String, f32)>>;

    /// Recognised lines kept for reuse (see [`LineCache`]). A full 4K
    /// screen has a few hundred lines; this holds several screens.
    const LINE_CACHE_SIZE: usize = 2048;

    pub struct PpOcr {
        networks: Box<dyn Networks>,
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
    ///
    /// The detector does not draw the same box around an unchanged line
    /// every time: a change elsewhere in the picture (a new chat line, a
    /// blinking cursor) moves its edges by a pixel, and the crop is then
    /// another picture of the same line. So the places lines were read
    /// at are kept too ([`Seen`]): a line detected near such a place
    /// whose pixels there are unchanged is that line.
    #[derive(Default)]
    pub(super) struct LineCache {
        map: std::collections::HashMap<u64, (String, f32, u64)>,
        /// Counts lookups; entries remember when they were last used.
        clock: u64,
        /// Where lines were read, newest last.
        seen: std::collections::VecDeque<Seen>,
    }

    /// A line as it was read: the crop (`[x, y, w, h]` in the picture),
    /// the box reported for it and the [`LineCache::key`] of the crop.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(super) struct Seen {
        pub(super) rect: [u32; 4],
        pub(super) reported: post::DetBox,
        pub(super) key: u64,
        /// Size of the picture when the crop ends at one of its edges.
        /// Such a crop has no margin past its glyphs on that side, so it
        /// stands only in a picture of this size: in a larger one (the
        /// window was resized) more of the line may show beyond it.
        pub(super) cut_by: Option<(u32, u32)>,
    }

    /// See [`Seen::cut_by`].
    pub(super) fn cut_by(rgb: &RgbImage, [x, y, w, h]: [u32; 4]) -> Option<(u32, u32)> {
        (x == 0 || y == 0 || x + w >= rgb.width() || y + h >= rgb.height())
            .then_some(rgb.dimensions())
    }

    /// How far (pixels) each edge of a detected crop may lie from a crop
    /// read before for the two to count as the same place.
    ///
    /// Must stay below the distance a crop extends past its glyphs (the
    /// DB unclip distance, at least 0.4 × the shorter glyph side): text
    /// added next to a line then either lies inside the earlier crop,
    /// whose pixels no longer match, or moves the detected edge further
    /// than this. A tenth of the shorter crop side is under half of it.
    pub(super) fn same_place_tolerance(rect: [u32; 4]) -> u32 {
        (rect[2].min(rect[3]) / 10).clamp(1, 6)
    }

    pub(super) fn same_place(a: [u32; 4], b: [u32; 4]) -> bool {
        let tol = same_place_tolerance(a).min(same_place_tolerance(b));
        let near = |p: u32, q: u32| p.abs_diff(q) <= tol;
        near(a[0], b[0])
            && near(a[1], b[1])
            && near(a[0] + a[2], b[0] + b[2])
            && near(a[1] + a[3], b[1] + b[3])
    }

    /// The picture of the line in `rect` of `rgb`, as recognition reads
    /// it. `None` when `rect` does not lie inside the picture.
    pub(super) fn line_image(rgb: &RgbImage, [x, y, w, h]: [u32; 4]) -> Option<RgbImage> {
        if w < 2 || h < 2 || x + w > rgb.width() || y + h > rgb.height() {
            return None;
        }
        let crop = image::imageops::crop_imm(rgb, x, y, w, h).to_image();
        // Vertical text column: rotate so it reads left→right.
        Some(if h as f32 >= w as f32 * 1.5 && h > 2 * REC_HEIGHT / 3 {
            image::imageops::rotate270(&crop)
        } else {
            crop
        })
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

        /// The line read before at the place of `rect` when `rgb` still
        /// shows the same pixels there: its record and its picture.
        pub(super) fn seen_at(&self, rgb: &RgbImage, rect: [u32; 4]) -> Option<(Seen, RgbImage)> {
            self.seen
                .iter()
                .rev()
                .filter(|s| same_place(rect, s.rect) && self.map.contains_key(&s.key))
                .filter(|s| s.cut_by.is_none_or(|size| size == rgb.dimensions()))
                .find_map(|s| {
                    let image = line_image(rgb, s.rect)?;
                    (Self::key(&image) == s.key).then_some((*s, image))
                })
        }

        /// Remember where the lines of one reading were read.
        pub(super) fn remember(&mut self, lines: impl IntoIterator<Item = Seen>, cap: usize) {
            for line in lines {
                if !self.seen.contains(&line) {
                    self.seen.push_back(line);
                }
            }
            while self.seen.len() > cap {
                self.seen.pop_front();
            }
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

    /// What an engine was loaded for: the models directory, the wish
    /// and whether the ONNX Runtime parts were installed then (they can
    /// arrive while the app runs).
    type Loaded = (PathBuf, Want, bool);

    static CACHE: Mutex<Option<(Loaded, Arc<PpOcr>)>> = Mutex::new(None);

    /// Cached engine for `dir` (models load once per process).
    pub fn engine_for(dir: &Path, want: Want) -> Result<Arc<PpOcr>> {
        let want = Want {
            gpu: want.gpu && !GPU_FAILED.load(std::sync::atomic::Ordering::Relaxed),
            ..want
        };
        let key: Loaded = (
            dir.to_path_buf(),
            want,
            crate::providers::ocr_onnx::installed(dir),
        );
        let mut guard = CACHE.lock();
        if let Some((k, e)) = guard.as_ref() {
            if *k == key {
                return Ok(e.clone());
            }
        }
        let e = Arc::new(PpOcr::load(dir, want)?);
        *guard = Some((key, e.clone()));
        Ok(e)
    }

    /// Set once a reading on the GPU failed: for the rest of the run
    /// the GPU is not a candidate (see [`read_falling_back`]).
    static GPU_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    /// Read `img` on `engine`. A GPU backend passed the check at load,
    /// but a reading can still fail later (a larger picture than the
    /// graphics memory holds, a driver reset): the picture is then read
    /// on the CPU, and so is everything after it until the app
    /// restarts.
    pub fn read_falling_back(
        dir: &Path,
        want: Want,
        engine: &PpOcr,
        img: &DynamicImage,
        p: &Params,
    ) -> Result<Vec<OcrLine>> {
        match engine.recognize(img, p) {
            Err(e) if engine.backend() == Backend::DirectMl => {
                tracing::warn!("ocr on the GPU failed, continuing on the CPU: {e:#}");
                GPU_FAILED.store(true, std::sync::atomic::Ordering::Relaxed);
                engine_for(dir, want)?.recognize(img, p)
            }
            read => read,
        }
    }

    /// A stand-in for a line of text: bars of varying width.
    fn bench_line(seed: u32) -> RgbImage {
        RgbImage::from_fn(320, REC_HEIGHT, |x, y| {
            let on = (x / (3 + (x / 40 + seed) % 5)).is_multiple_of(2) && (8..40).contains(&y);
            image::Rgb(if on { [20, 20, 20] } else { [240, 240, 240] })
        })
    }

    impl PpOcr {
        /// Load the models on the backend `want` asks for; for
        /// [`Backend::Auto`], on the fastest one that loads here.
        ///
        /// "Fastest" is measured, not assumed: a GPU behind a slow
        /// driver or a software adapter loses to the CPU. Each
        /// candidate reads the same small workload; the timings go to
        /// the log. rten is the fallback, not a candidate: ONNX Runtime
        /// on the CPU read lines about twice as fast wherever both were
        /// measured, and loading rten only to time it costs a second.
        pub fn load(dir: &Path, want: Want) -> Result<Self> {
            let dict: Vec<String> = std::fs::read_to_string(dir.join(DICT_FILE))
                .with_context(|| format!("read {DICT_FILE}"))?
                .lines()
                .map(|l| l.trim_end_matches('\r').to_string())
                .collect();
            let engine = |networks| Self {
                networks,
                dict: dict.clone(),
                lines: Mutex::new(LineCache::default()),
            };
            let mut loaded: Vec<Self> = Vec::new();
            for backend in want.candidates() {
                match networks(dir, backend) {
                    Ok(n) => loaded.push(engine(n)),
                    Err(e) => tracing::info!("ocr backend {} unavailable: {e:#}", backend.name()),
                }
            }
            // One candidate needs no race, but must still prove it runs.
            let mut timed: Vec<(Self, std::time::Duration)> = Vec::new();
            for e in loaded {
                match e.bench() {
                    Ok(t) => timed.push((e, t)),
                    Err(err) => {
                        tracing::info!("ocr backend {} failed: {err:#}", e.backend().name())
                    }
                }
            }
            let report: Vec<String> = timed
                .iter()
                .map(|(e, t)| format!("{} {} ms", e.backend().name(), t.as_millis()))
                .collect();
            let best = match timed.into_iter().min_by_key(|(_, t)| *t) {
                Some((e, _)) => e,
                None => engine(networks(dir, Backend::Rten)?),
            };
            tracing::info!(
                "ocr backend: {} (measured: {})",
                best.backend().name(),
                if report.is_empty() {
                    "nothing else available".to_string()
                } else {
                    report.join(", ")
                }
            );
            Ok(best)
        }

        /// ONNX Runtime on the CPU, cutting the work as the GPU plan
        /// does: what a GPU reads, where no GPU is at hand.
        #[cfg(all(test, feature = "ocr-onnx"))]
        pub(super) fn load_cpu_with_gpu_plan(dir: &Path) -> Result<Self> {
            let mut networks = OnnxNetworks::load(dir, Backend::OnnxCpu)?;
            networks.plan = OnnxNetworks::GPU_PLAN;
            let mut engine = Self::load(
                dir,
                Want {
                    backend: Backend::Rten,
                    gpu: false,
                },
            )?;
            engine.networks = Box::new(networks);
            Ok(engine)
        }

        /// The backend the models run on.
        pub fn backend(&self) -> Backend {
            self.networks.backend()
        }

        /// Time of one detection and eight lines, after a first pass
        /// that pays the one-time costs (graph and shader compilation).
        fn bench(&self) -> Result<std::time::Duration> {
            let (h, w) = (320usize, 480usize);
            let lines: Vec<RgbImage> = (0..8).map(bench_line).collect();
            let lines: Vec<&RgbImage> = lines.iter().collect();
            let pass = || -> Result<std::time::Duration> {
                let t = std::time::Instant::now();
                self.networks.det(vec![0.0; 3 * h * w], h, w)?;
                self.read_lines(&lines)?;
                Ok(t.elapsed())
            };
            pass()?;
            pass()
        }

        /// Full pipeline: returns lines with bboxes in `img` pixels.
        pub fn recognize(&self, img: &DynamicImage, p: &Params) -> Result<Vec<OcrLine>> {
            let rgb = img.to_rgb8();
            let t = std::time::Instant::now();
            let boxes = self.detect(&rgb, p)?;
            tracing::debug!("ppocr det: {} boxes in {:?}", boxes.len(), t.elapsed());
            let (fw, fh) = (rgb.width() as f32, rgb.height() as f32);
            // Crop generously so glyph edges are intact, but report a
            // tighter box: it is what the overlay paints over and what
            // paragraph grouping measures line gaps with.
            let detected: Vec<Seen> = boxes
                .into_iter()
                .map(|raw| {
                    let b = post::unclip(&raw, p.unclip_ratio, fw, fh);
                    let reported = post::unclip(&raw, REPORT_UNCLIP_RATIO, fw, fh);
                    let x = b.x0.floor() as u32;
                    let y = b.y0.floor() as u32;
                    let w = (b.x1.ceil() as u32).min(rgb.width()).saturating_sub(x);
                    let h = (b.y1.ceil() as u32).min(rgb.height()).saturating_sub(y);
                    Seen {
                        rect: [x, y, w, h],
                        reported,
                        key: 0,
                        cut_by: cut_by(&rgb, [x, y, w, h]),
                    }
                })
                .collect();
            // A line read before at (nearly) the same place and unchanged
            // there is read as before, box included; the rest is cropped
            // where it was detected now.
            let mut crops: Vec<(Seen, RgbImage)> = {
                let cache = self.lines.lock();
                detected
                    .into_iter()
                    .filter_map(|d| {
                        cache.seen_at(&rgb, d.rect).or_else(|| {
                            let image = line_image(&rgb, d.rect)?;
                            let key = LineCache::key(&image);
                            Some((Seen { key, ..d }, image))
                        })
                    })
                    .collect()
            };
            // Two detections of one line that was read before as a whole.
            let mut places = std::collections::HashSet::new();
            crops.retain(|c| places.insert(c.0.rect));
            // Similar aspect ratios in one batch keep padding small.
            crops.sort_by(|a, b| {
                let ra = a.1.width() as f32 / a.1.height() as f32;
                let rb = b.1.width() as f32 / b.1.height() as f32;
                ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut lines = Vec::new();
            // Lines read before (same pixels) keep their text; only the
            // rest goes through recognition.
            let keys: Vec<u64> = crops.iter().map(|c| c.0.key).collect();
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
            let images: Vec<&RgbImage> = fresh.iter().map(|&i| &crops[i].1).collect();
            let texts = self.read_lines(&images)?;
            {
                let mut cache = self.lines.lock();
                for (&c, (text, conf)) in fresh.iter().zip(texts) {
                    cache.put(keys[c], text.clone(), conf, LINE_CACHE_SIZE);
                    known[c] = Some((text, conf));
                }
                cache.remember(crops.iter().map(|c| c.0), LINE_CACHE_SIZE);
            }
            for ((seen, _), read) in crops.iter().zip(known) {
                let b = &seen.reported;
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
            let (dh, dw) = (dh as usize, dw as usize);
            let mut input = vec![0f32; 3 * dh * dw];
            for (i, px) in resized.pixels().enumerate() {
                for c in 0..3 {
                    let v = px.0[2 - c] as f32 / 255.0;
                    input[c * dh * dw + i] = (v - MEAN[c]) / STD[c];
                }
            }
            let (prob, ph, pw) = self.networks.det(input, dh, dw)?;
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

        /// Text and confidence of each line picture, in order, read
        /// the way the backend's [`RecPlan`] asks.
        fn read_lines(&self, images: &[&RgbImage]) -> Result<Vec<(String, f32)>> {
            let plan = self.networks.plan();
            let width = |i: usize| {
                let c = images[i];
                padded_width(post::rec_width(
                    c.width(),
                    c.height(),
                    REC_HEIGHT,
                    REC_MAX_WIDTH,
                ))
            };
            // Calls of one padded width each, at most `batch` lines.
            let mut order: Vec<usize> = (0..images.len()).collect();
            order.sort_by_key(|&i| width(i));
            let chunks: Vec<&[usize]> = order
                .chunk_by(|&a, &b| width(a) == width(b))
                .flat_map(|same| same.chunks(plan.batch))
                .collect();
            let workers = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(2)
                .clamp(1, plan.workers);
            let next = std::sync::atomic::AtomicUsize::new(0);
            let results: Mutex<Vec<(usize, BatchResult)>> = Mutex::new(Vec::new());
            let work = || loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(chunk) = chunks.get(i) else { break };
                let lines: Vec<&RgbImage> = chunk.iter().map(|&c| images[c]).collect();
                let r = self.recognize_batch(&lines, width(chunk[0]));
                results.lock().push((i, r));
            };
            if workers.min(chunks.len()) <= 1 {
                work();
            } else {
                std::thread::scope(|scope| {
                    for _ in 0..workers.min(chunks.len()) {
                        scope.spawn(work);
                    }
                });
            }
            let mut texts: Vec<(String, f32)> = vec![(String::new(), 0.0); images.len()];
            for (i, batch) in results.into_inner() {
                for (&c, read) in chunks[i].iter().zip(batch?) {
                    texts[c] = read;
                }
            }
            Ok(texts)
        }

        /// One call of the recognition network: `lines` side by side
        /// in a batch `width` wide, each padded on the right.
        fn recognize_batch(&self, lines: &[&RgbImage], width: u32) -> Result<Vec<(String, f32)>> {
            if lines.is_empty() {
                return Ok(vec![]);
            }
            let (n, h, w) = (lines.len(), REC_HEIGHT as usize, width as usize);
            let mut input = vec![0f32; n * 3 * h * w];
            for (i, line) in lines.iter().enumerate() {
                let lw = post::rec_width(line.width(), line.height(), REC_HEIGHT, REC_MAX_WIDTH);
                let resized = image::imageops::resize(*line, lw, REC_HEIGHT, FilterType::Triangle);
                for (x, y, px) in resized.enumerate_pixels() {
                    for c in 0..3 {
                        let v = px.0[2 - c] as f32 / 255.0;
                        input[((i * 3 + c) * h + y as usize) * w + x as usize] = (v - 0.5) / 0.5;
                    }
                }
            }
            let (data, steps, classes) = self.networks.rec(input, n, w)?;
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
    use_gpu: bool,
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
            use_gpu: true,
        }
    }

    /// `performance.use_gpu_if_available`: without it the GPU backend
    /// is not considered.
    pub fn with_gpu(mut self, use_gpu: bool) -> Self {
        self.use_gpu = use_gpu;
        self
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
            let want = engine::Want {
                backend: params.backend,
                gpu: self.use_gpu,
            };
            let lines =
                tokio::task::spawn_blocking(move || -> Result<Vec<OcrLine>, ProviderError> {
                    let engine = engine::engine_for(&dir, want).map_err(|e| {
                        tracing::warn!("PP-OCR models unavailable: {e}");
                        ProviderError::LowConfidence
                    })?;
                    let img = image::load_from_memory(&image_png)
                        .map_err(|e| ProviderError::Other(e.to_string()))?;
                    engine::read_falling_back(&dir, want, &engine, &img, &params)
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
            let _ = (image_png, &self.models_dir, self.params, self.use_gpu);
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
        let engine = engine::PpOcr::load(dir.path(), engine::Want::default()).unwrap();
        // Nothing but the base models here: rten runs them.
        assert_eq!(engine.backend(), Backend::Rten);
        let img = image::DynamicImage::new_rgb8(64, 32);
        assert!(engine
            .recognize(&img, &Params::default())
            .unwrap()
            .is_empty());
    }

    /// The backends must read the same text: speed is never bought
    /// with reading quality. Needs the installed models (rten and
    /// ONNX) and ONNX Runtime in the default models directory; reads
    /// the lens fixtures plus `MOONLENS_OCR_IMAGES`. Also reports what
    /// [`Backend::Auto`] chooses here and how long each backend took.
    #[cfg(feature = "ocr-onnx")]
    #[test]
    #[ignore]
    fn backends_read_the_same_lines() {
        use engine::{PpOcr, Want};
        let dir = default_models_dir().unwrap();
        let on = |backend, gpu| PpOcr::load(&dir, Want { backend, gpu }).unwrap();
        let rten = on(Backend::Rten, false);
        let cpu = on(Backend::OnnxCpu, false);
        assert_eq!(cpu.backend(), Backend::OnnxCpu, "ONNX Runtime did not load");
        let mut others = vec![
            ("onnx-cpu", cpu),
            (
                "gpu plan on cpu",
                PpOcr::load_cpu_with_gpu_plan(&dir).unwrap(),
            ),
        ];
        // Where DirectML starts (any Windows with a Direct3D 12 adapter,
        // the software one included) it must read the same text too.
        let directml = on(Backend::DirectMl, true);
        if directml.backend() == Backend::DirectMl {
            others.push(("directml", directml));
        } else {
            println!(
                "directml: not available here ({})",
                engine::unavailable(&dir, Backend::DirectMl)
            );
        }
        println!("auto chooses: {}", on(Backend::Auto, true).backend().name());

        let mut images: Vec<PathBuf> = std::fs::read_dir("tests/fixtures/lens")
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "png"))
            .collect();
        if let Some(extra) = std::env::var_os("MOONLENS_OCR_IMAGES") {
            images.extend(std::env::split_paths(&extra));
        }
        images.sort();
        let read = |engine: &PpOcr, img: &image::DynamicImage| {
            let t = std::time::Instant::now();
            let mut lines = engine.recognize(img, &Params::default()).unwrap();
            let took = t.elapsed();
            lines.sort_by_key(|l| (l.bbox[1] as i32 / 8, l.bbox[0] as i32));
            let texts: Vec<String> = lines.into_iter().map(|l| l.text).collect();
            (texts, took)
        };
        let (mut total, mut differing) = (0, 0);
        let mut spent = vec![std::time::Duration::ZERO; others.len() + 1];
        for path in &images {
            let img = image::open(path).unwrap();
            let (want, took) = read(&rten, &img);
            spent[0] += took;
            total += want.len();
            for (i, (name, engine)) in others.iter().enumerate() {
                let (got, took) = read(engine, &img);
                spent[i + 1] += took;
                let missing: Vec<_> = want.iter().filter(|l| !got.contains(l)).collect();
                let extra: Vec<_> = got.iter().filter(|l| !want.contains(l)).collect();
                let n = missing.len().max(extra.len());
                if n > 0 {
                    println!("{}: {name} differs in {n} lines", path.display());
                    println!("  rten: {missing:?}\n  {name}: {extra:?}");
                }
                differing += n;
            }
        }
        println!("rten: {:?} for {total} lines", spent[0]);
        for ((name, _), t) in others.iter().zip(&spent[1..]) {
            println!("{name}: {t:?}");
        }
        println!(
            "{differing} differing readings of {} ({total} lines x {} backends)",
            total * others.len(),
            others.len()
        );
        // Rounding may flip a character in a rare line; more than one
        // reading in a hundred means the backends do not agree.
        assert!(differing * 100 <= total * others.len(), "backends disagree");
    }

    /// An install from an earlier version has no ONNX Runtime; when it
    /// arrives while the app runs, the next load uses it. Needs the
    /// installed models and runtime; run alone (`--exact`), since the
    /// library loads once per process.
    #[cfg(feature = "ocr-onnx")]
    #[test]
    #[ignore]
    fn the_runtime_is_used_once_it_arrives() {
        use crate::providers::ocr_onnx;
        use engine::{PpOcr, Want};
        let installed = default_models_dir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        for file in [
            DET_FILE,
            REC_FILE,
            DICT_FILE,
            ocr_onnx::DET_FILE,
            ocr_onnx::REC_FILE,
        ] {
            std::fs::copy(installed.join(file), dir.path().join(file)).unwrap();
        }
        let want = Want {
            backend: Backend::Auto,
            gpu: false,
        };
        assert_eq!(
            PpOcr::load(dir.path(), want).unwrap().backend(),
            Backend::Rten
        );

        let library = ocr_onnx::RUNTIME.as_ref().unwrap().library;
        let to = ocr_onnx::runtime_dir(dir.path());
        std::fs::create_dir_all(&to).unwrap();
        std::fs::copy(
            ocr_onnx::runtime_dir(&installed).join(library),
            to.join(library),
        )
        .unwrap();
        assert_eq!(
            PpOcr::load(dir.path(), want).unwrap().backend(),
            Backend::OnnxCpu
        );
    }

    #[test]
    fn the_gpu_is_a_candidate_only_where_it_is_allowed_and_exists() {
        use engine::Want;
        let auto = |gpu| {
            Want {
                backend: Backend::Auto,
                gpu,
            }
            .candidates()
        };
        assert_eq!(auto(false), [Backend::OnnxCpu]);
        assert_eq!(auto(true).contains(&Backend::DirectMl), cfg!(windows));
        assert_eq!(auto(true).last(), Some(&Backend::OnnxCpu));
        let pinned = |backend| Want { backend, gpu: true }.candidates();
        assert!(pinned(Backend::Rten).is_empty());
        assert_eq!(pinned(Backend::OnnxCpu), [Backend::OnnxCpu]);
        // GPU use switched off wins over a pinned GPU backend, which
        // then runs on the CPU.
        let no_gpu = Want {
            backend: Backend::DirectMl,
            gpu: false,
        };
        assert_eq!(no_gpu.candidates(), [Backend::OnnxCpu]);
    }

    #[test]
    fn a_line_is_padded_by_its_own_width_alone() {
        assert_eq!(engine::padded_width(1), 64);
        assert_eq!(engine::padded_width(64), 64);
        assert_eq!(engine::padded_width(65), 128);
        assert_eq!(engine::padded_width(3200), 3200);
    }

    #[test]
    fn backend_names_round_trip_and_unknown_ones_mean_auto() {
        for b in [Backend::DirectMl, Backend::OnnxCpu, Backend::Rten] {
            assert_eq!(Backend::from_name(b.name()), b);
        }
        assert_eq!(Backend::from_name("auto"), Backend::Auto);
        assert_eq!(Backend::from_name("cuda"), Backend::Auto);
        let cfg = serde_json::json!({ "backend": "onnx-cpu" });
        assert_eq!(Params::from_config(&cfg).backend, Backend::OnnxCpu);
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

    /// Dark picture with a light bar standing in for a line of text.
    fn page(bar: [u32; 4]) -> image::RgbImage {
        use image::{Rgb, RgbImage};
        let mut img = RgbImage::from_pixel(200, 80, Rgb([20, 20, 20]));
        for y in bar[1]..bar[1] + bar[3] {
            for x in bar[0]..bar[0] + bar[2] {
                img.put_pixel(x, y, Rgb([230, 230, 230]));
            }
        }
        img
    }

    fn seen(rect: [u32; 4], picture: &image::RgbImage) -> engine::Seen {
        engine::Seen {
            rect,
            reported: post::DetBox {
                x0: rect[0] as f32,
                y0: rect[1] as f32,
                x1: (rect[0] + rect[2]) as f32,
                y1: (rect[1] + rect[3]) as f32,
                score: 1.0,
            },
            key: engine::LineCache::key(&engine::line_image(picture, rect).unwrap()),
            cut_by: engine::cut_by(picture, rect),
        }
    }

    #[test]
    fn a_line_detected_a_pixel_off_is_the_line_read_before() {
        use engine::LineCache;
        let picture = page([20, 30, 100, 12]);
        let read = seen([10, 20, 120, 32], &picture);
        let mut cache = LineCache::default();
        cache.put(read.key, "Hello".into(), 0.9, 8);
        cache.remember([read], 8);

        // The detector moved the box by a pixel or two on unchanged
        // pixels: the earlier reading stands, at its own place.
        let (found, _) = cache.seen_at(&picture, [11, 19, 121, 34]).unwrap();
        assert_eq!(found, read);
        assert_eq!(cache.get(found.key), Some(("Hello".into(), 0.9)));
        // A box somewhere else is another line.
        assert!(cache.seen_at(&picture, [10, 40, 120, 32]).is_none());
    }

    #[test]
    fn a_line_whose_pixels_changed_is_read_again() {
        use engine::LineCache;
        let picture = page([20, 30, 100, 12]);
        let read = seen([10, 20, 120, 32], &picture);
        let mut cache = LineCache::default();
        cache.put(read.key, "Hello".into(), 0.9, 8);
        cache.remember([read], 8);

        // One more glyph at the end of the line, inside the earlier crop.
        let longer = page([20, 30, 106, 12]);
        assert!(cache.seen_at(&longer, [10, 20, 122, 32]).is_none());
        // The same place in a smaller picture (the window was resized).
        let small = image::RgbImage::new(100, 40);
        assert!(cache.seen_at(&small, [10, 20, 120, 32]).is_none());
    }

    #[test]
    fn a_line_cut_by_the_picture_edge_is_read_again_in_a_larger_picture() {
        use engine::LineCache;
        use image::GenericImage;
        // The line runs into the right edge: its crop has no margin there.
        let picture = page([20, 30, 180, 12]);
        let read = seen([10, 20, 190, 32], &picture);
        let mut cache = LineCache::default();
        cache.put(read.key, "Hell".into(), 0.9, 8);
        cache.remember([read], 8);
        assert!(cache.seen_at(&picture, [10, 20, 190, 32]).is_some());

        // The window grows by three pixels: the old crop is unchanged,
        // but more of the last glyph shows beside it.
        let mut wider = image::RgbImage::from_pixel(203, 80, image::Rgb([20, 20, 20]));
        wider.copy_from(&picture, 0, 0).unwrap();
        assert!(cache.seen_at(&wider, [10, 20, 193, 32]).is_none());
    }

    #[test]
    fn text_added_beside_a_line_moves_the_box_out_of_tolerance() {
        // A crop extends past its glyphs by the DB unclip distance, at
        // least 0.4 × the shorter glyph side for the default ratio. Text
        // outside the earlier crop moves the detected edge by more than
        // that, which must never count as the same place.
        for (w, h) in [(12u32, 12u32), (300, 14), (14, 300), (40, 60), (2000, 48)] {
            let p = Params::default();
            let raw = post::DetBox {
                x0: 100.0,
                y0: 100.0,
                x1: 100.0 + w as f32,
                y1: 100.0 + h as f32,
                score: 1.0,
            };
            let grown = post::unclip(&raw, p.unclip_ratio, 1e6, 1e6);
            let extends = (raw.x0 - grown.x0).floor() as u32;
            let crop = [0, 0, w + 2 * extends, h + 2 * extends];
            assert!(
                engine::same_place_tolerance(crop) < extends,
                "{w}x{h}: tolerance {} reaches past the crop margin {extends}",
                engine::same_place_tolerance(crop)
            );
        }
    }

    #[test]
    fn remembered_places_are_bounded_and_not_repeated() {
        use engine::LineCache;
        let picture = page([20, 30, 100, 12]);
        let mut cache = LineCache::default();
        let a = seen([10, 20, 120, 32], &picture);
        cache.put(a.key, "a".into(), 1.0, 64);
        cache.remember([a, a], 4);
        cache.remember([a], 4);
        // Newer places push the oldest out.
        let others: Vec<_> = (0..4).map(|i| seen([i, 50, 60, 20], &picture)).collect();
        cache.remember(others, 4);
        assert!(cache.seen_at(&picture, a.rect).is_none());
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
