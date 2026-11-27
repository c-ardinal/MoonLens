//! Multilingual OCR with PP-OCR (PaddleOCR) models.
//!
//! `ocrs` only recognises Latin script. PP-OCR reads Chinese, Japanese
//! (kana + kanji) and Latin script with one recognition model, so this
//! provider is what makes Japanese / Chinese screens translatable. It
//! runs the classic two-stage PaddleOCR pipeline:
//!
//! 1. **Detection** (DB): a probability map of "text pixels" is
//!    thresholded, connected regions become text-line boxes, and each
//!    box is expanded ("unclipped") back to the full glyph extent.
//! 2. **Recognition** (CTC): every box is cropped, resized to a fixed
//!    height and decoded greedily against the model dictionary.
//!
//! The post-processing is pure Rust in [`post`] and unit tested; model
//! I/O lives in [`engine`].
//!
//! The models are PP-OCRv6 small (Apache-2.0), in the ONNX form
//! PaddlePaddle publishes: downloaded on demand (see
//! [`install_models_with_progress`]), verified against a pinned
//! SHA-256, and stored next to the ocrs models. On the pages both
//! generations were measured on, v6 small misread 0.56 % of the
//! Japanese characters where v5 mobile misread 1.46 %, and read a
//! little faster. Lines in scripts these models do not know go to the
//! models of [`super::ocr_scripts`].
//!
//! An install made by an earlier version holds PP-OCRv5 mobile
//! instead. It keeps working ([`Generation::V5`]) until the user
//! fetches the OCR models again, which replaces it.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use super::ocr_onnx::Remote;
use super::{Capability, OcrLine, Provider, ProviderError, TaskInput, TaskOutput};

pub const DET_FILE: &str = "ppocrv6-det.onnx";
pub const REC_FILE: &str = "ppocrv6-rec.onnx";

/// The models, pinned to a commit of the publisher's repository.
pub static MODELS: [Remote; 2] = [
    Remote {
        url: "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx/resolve/28fe5895c24fd108c19eb3e8479f4ab385fbfc62/inference.onnx",
        sha256: "d73e0058b7a8086bbd57f3d10b8bcd4ff95363f67e06e2762b5e814fe9c9410e",
        file: DET_FILE,
    },
    Remote {
        url: "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx/resolve/b8f84f0b80c529de40b4fbb3544b84fa7233a513/inference.onnx",
        sha256: "5435fd747c9e0efe15a96d0b378d5bd157e9492ed8fd80edf08f30d02fa24634",
        file: REC_FILE,
    },
];

/// The recognition model's characters, one per line in class order.
const DICT: &str = include_str!("ocr_dicts/ppocrv6.txt");

/// PP-OCRv5 mobile as earlier versions installed it: the rten bundle,
/// its dictionary, and the ONNX pair ONNX Runtime ran.
const V5_RTEN: (&str, &str) = ("ppocrv5-det.rten", "ppocrv5-rec.rten");
const V5_DICT: &str = "ppocrv5-dict.txt";
const V5_ONNX: (&str, &str) = ("ppocrv5-det.onnx", "ppocrv5-rec.onnx");

/// Which generation of the models a folder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Generation {
    /// PP-OCRv6 small: what this version installs.
    V6,
    /// PP-OCRv5 mobile, left by an earlier version.
    V5,
}

fn present(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

impl Generation {
    /// The generation to run from `dir`: the current one when it is
    /// there, else the earlier one.
    pub fn on_disk(dir: &Path) -> Option<Self> {
        if MODELS.iter().all(|m| m.present(dir)) {
            Some(Self::V6)
        } else if [V5_RTEN.0, V5_RTEN.1, V5_DICT]
            .iter()
            .all(|f| present(&dir.join(f)))
        {
            Some(Self::V5)
        } else {
            None
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::V6 => "PP-OCRv6 small",
            Self::V5 => "PP-OCRv5 mobile",
        }
    }

    /// `(detection, recognition)` files rten loads.
    fn rten_files(self) -> (&'static str, &'static str) {
        match self {
            Self::V6 => (DET_FILE, REC_FILE),
            Self::V5 => V5_RTEN,
        }
    }

    /// `(detection, recognition)` files ONNX Runtime loads.
    #[cfg(feature = "ocr-onnx")]
    fn onnx_files(self) -> (&'static str, &'static str) {
        match self {
            Self::V6 => (DET_FILE, REC_FILE),
            Self::V5 => V5_ONNX,
        }
    }

    fn dict(self, dir: &Path) -> anyhow::Result<Vec<String>> {
        use anyhow::Context;
        Ok(match self {
            Self::V6 => super::ocr_scripts::dict_lines(DICT),
            Self::V5 => super::ocr_scripts::dict_lines(
                &std::fs::read_to_string(dir.join(V5_DICT))
                    .with_context(|| format!("read {V5_DICT}"))?,
            ),
        })
    }
}

pub fn default_models_dir() -> Option<PathBuf> {
    super::ocr_ocrs::default_models_dir()
}

/// True when models of either generation are in `dir`: OCR works.
pub fn models_installed(dir: &Path) -> bool {
    Generation::on_disk(dir).is_some()
}

/// True when the models this version installs are in `dir`.
pub fn models_current(dir: &Path) -> bool {
    Generation::on_disk(dir) == Some(Generation::V6)
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

    use anyhow::{anyhow, Result};
    use image::{imageops::FilterType, DynamicImage, RgbImage};
    use parking_lot::Mutex;
    use rten_tensor::prelude::*;
    use rten_tensor::{NdTensor, Tensor};

    use super::{post, Backend, Generation, Params};
    use crate::providers::ocr_scripts::{self, Script};
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

    /// A recognition network. Tensors are `f32`, row-major.
    pub(super) trait Reader: Send + Sync {
        /// `[n, 3, REC_HEIGHT, w]` in, `[n, steps, classes]` and its
        /// `(steps, classes)` out.
        fn rec(&self, input: Vec<f32>, n: usize, w: usize) -> Result<(Vec<f32>, usize, usize)>;
    }

    /// The two networks on one backend.
    pub(super) trait Networks: Reader {
        fn backend(&self) -> Backend;
        fn plan(&self) -> RecPlan;
        /// `[1, 3, h, w]` in, the probability map and its `(h, w)` out.
        fn det(&self, input: Vec<f32>, h: usize, w: usize) -> Result<(Vec<f32>, usize, usize)>;
        /// One more recognition network (the model `file` in `dir`) on
        /// the same backend, called the way [`Networks::plan`] says.
        fn reader(&self, dir: &Path, file: &str) -> Result<Box<dyn Reader>>;
    }

    struct RtenReader(rten::Model);

    impl RtenReader {
        fn load(dir: &Path, file: &str) -> Result<Self> {
            rten::Model::load_file(dir.join(file))
                .map(Self)
                .map_err(|e| anyhow!("load {file}: {e}"))
        }
    }

    impl Reader for RtenReader {
        fn rec(&self, input: Vec<f32>, n: usize, w: usize) -> Result<(Vec<f32>, usize, usize)> {
            let out = RtenNetworks::run(&self.0, input, [n, 3, REC_HEIGHT as usize, w])?;
            let shape = out.shape().to_vec();
            if shape.len() != 3 || shape[0] != n {
                anyhow::bail!("unexpected rec output shape {shape:?}");
            }
            Ok((out.iter().copied().collect(), shape[1], shape[2]))
        }
    }

    struct RtenNetworks {
        det: rten::Model,
        rec: RtenReader,
    }

    impl RtenNetworks {
        fn load(dir: &Path, generation: Generation) -> Result<Self> {
            let (det, rec) = generation.rten_files();
            Ok(Self {
                det: rten::Model::load_file(dir.join(det))
                    .map_err(|e| anyhow!("load {det}: {e}"))?,
                rec: RtenReader::load(dir, rec)?,
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

        fn reader(&self, dir: &Path, file: &str) -> Result<Box<dyn Reader>> {
            Ok(Box::new(RtenReader::load(dir, file)?))
        }
    }

    impl Reader for RtenNetworks {
        fn rec(&self, input: Vec<f32>, n: usize, w: usize) -> Result<(Vec<f32>, usize, usize)> {
            self.rec.rec(input, n, w)
        }
    }

    #[cfg(feature = "ocr-onnx")]
    use crate::providers::ocr_onnx::runtime::{Detector, Device, Recognizer};

    #[cfg(feature = "ocr-onnx")]
    struct OnnxReader(Recognizer);

    #[cfg(feature = "ocr-onnx")]
    impl Reader for OnnxReader {
        fn rec(&self, input: Vec<f32>, n: usize, w: usize) -> Result<(Vec<f32>, usize, usize)> {
            let (data, shape) = self.0.run(input, n, REC_HEIGHT as usize, w)?;
            if shape.len() != 3 || shape[0] != n {
                anyhow::bail!("unexpected rec output shape {shape:?}");
            }
            Ok((data, shape[1], shape[2]))
        }
    }

    #[cfg(feature = "ocr-onnx")]
    struct OnnxNetworks {
        det: Detector,
        rec: OnnxReader,
        device: Device,
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

        /// Sessions a script model gets on the CPU. Its lines are few
        /// (see [`PpOcr::read_other_scripts`]), and every session
        /// holds the model in memory once more.
        const SCRIPT_SESSIONS: usize = 2;

        fn load(dir: &Path, generation: Generation, backend: Backend) -> Result<Self> {
            let (det, rec) = generation.onnx_files();
            let (device, plan) = if backend == Backend::DirectMl {
                (Device::DirectMl, Self::GPU_PLAN)
            } else {
                // Line by line: a batch gains nothing on the CPU
                // (measured).
                let workers = cpu_rec_workers();
                (Device::Cpu, RecPlan { workers, batch: 1 })
            };
            Ok(Self {
                det: Detector::load(dir, det, device)?,
                rec: OnnxReader(Recognizer::load(dir, rec, device, plan.workers)?),
                device,
                backend,
                plan,
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
            let (data, shape) = self.det.run(input, h, w)?;
            if shape.len() < 2 {
                anyhow::bail!("unexpected det output shape {shape:?}");
            }
            Ok((data, shape[shape.len() - 2], shape[shape.len() - 1]))
        }

        fn reader(&self, dir: &Path, file: &str) -> Result<Box<dyn Reader>> {
            let sessions = self.plan.workers.min(Self::SCRIPT_SESSIONS);
            Ok(Box::new(OnnxReader(Recognizer::load(
                dir,
                file,
                self.device,
                sessions,
            )?)))
        }
    }

    #[cfg(feature = "ocr-onnx")]
    impl Reader for OnnxNetworks {
        fn rec(&self, input: Vec<f32>, n: usize, w: usize) -> Result<(Vec<f32>, usize, usize)> {
            self.rec.rec(input, n, w)
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
        let Some(generation) = Generation::on_disk(dir) else {
            return "no models".to_string();
        };
        match networks(dir, generation, backend) {
            Err(e) => format!("load: {e:#}"),
            Ok(n) => match n.det(vec![0.0; 3 * 320 * 480], 320, 480) {
                Err(e) => format!("run: {e:#}"),
                Ok(_) => "loads and runs".to_string(),
            },
        }
    }

    fn networks(dir: &Path, generation: Generation, backend: Backend) -> Result<Box<dyn Networks>> {
        match backend {
            Backend::Rten => Ok(Box::new(RtenNetworks::load(dir, generation)?)),
            #[cfg(feature = "ocr-onnx")]
            Backend::DirectMl | Backend::OnnxCpu => {
                Ok(Box::new(OnnxNetworks::load(dir, generation, backend)?))
            }
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
        /// The installed models of [`ocr_scripts`].
        scripts: Vec<ScriptReader>,
    }

    /// The recognition model of one more script, with its dictionary.
    struct ScriptReader {
        script: &'static Script,
        reader: Box<dyn Reader>,
        dict: Vec<String>,
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
        /// The script models a line has been through, one bit per
        /// installed script ([`LineCache::set_tried`]).
        tried: std::collections::HashMap<u64, u32>,
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

        /// The script models the line `key` has been through.
        pub(super) fn tried(&self, key: u64) -> u32 {
            self.tried.get(&key).copied().unwrap_or(0)
        }

        /// Note that the line `key` has been through the script models
        /// in `scripts`. A line read while a script was not known to be
        /// on screen is read by its model once it is.
        pub(super) fn set_tried(&mut self, key: u64, scripts: u32) {
            if scripts != 0 {
                *self.tried.entry(key).or_default() |= scripts;
            }
            if self.tried.len() > 2 * self.map.len().max(64) {
                let map = &self.map;
                self.tried.retain(|k, _| map.contains_key(k));
            }
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
    /// and what was installed then (ONNX Runtime, the generation of
    /// the models, the script models): all of these can change while
    /// the app runs.
    type Loaded = (PathBuf, Want, bool, Option<Generation>, Vec<&'static str>);

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
            Generation::on_disk(dir),
            ocr_scripts::installed(dir).iter().map(|s| s.id).collect(),
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
            let generation = Generation::on_disk(dir)
                .ok_or_else(|| anyhow!("no PP-OCR models in the folder"))?;
            let dict = generation.dict(dir)?;
            let engine = |networks| Self {
                networks,
                dict: dict.clone(),
                lines: Mutex::new(LineCache::default()),
                scripts: Vec::new(),
            };
            let mut loaded: Vec<Self> = Vec::new();
            for backend in want.candidates() {
                match networks(dir, generation, backend) {
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
            let mut best = match timed.into_iter().min_by_key(|(_, t)| *t) {
                Some((e, _)) => e,
                None => engine(networks(dir, generation, Backend::Rten)?),
            };
            best.scripts = best.load_scripts(dir);
            let scripts: Vec<&str> = best.scripts.iter().map(|s| s.script.id).collect();
            tracing::info!(
                "ocr backend: {} (measured: {}), models: {}, scripts: {}",
                best.backend().name(),
                if report.is_empty() {
                    "nothing else available".to_string()
                } else {
                    report.join(", ")
                },
                generation.name(),
                if scripts.is_empty() {
                    "none".to_string()
                } else {
                    scripts.join(", ")
                }
            );
            Ok(best)
        }

        /// The installed script models, on this engine's backend. One
        /// that does not load is left out: the rest still read.
        fn load_scripts(&self, dir: &Path) -> Vec<ScriptReader> {
            ocr_scripts::installed(dir)
                .into_iter()
                .filter_map(
                    |script| match self.networks.reader(dir, script.model.file) {
                        Ok(reader) => Some(ScriptReader {
                            script,
                            reader,
                            dict: script.dict(),
                        }),
                        Err(e) => {
                            tracing::warn!("ocr script {} unavailable: {e:#}", script.id);
                            None
                        }
                    },
                )
                .collect()
        }

        /// The scripts read beside the standard model's.
        pub fn scripts(&self) -> Vec<&'static str> {
            self.scripts.iter().map(|s| s.script.id).collect()
        }

        /// ONNX Runtime on the CPU, cutting the work as the GPU plan
        /// does: what a GPU reads, where no GPU is at hand.
        #[cfg(all(test, feature = "ocr-onnx"))]
        pub(super) fn load_cpu_with_gpu_plan(dir: &Path) -> Result<Self> {
            let generation = Generation::on_disk(dir).ok_or_else(|| anyhow!("no models"))?;
            let mut networks = OnnxNetworks::load(dir, generation, Backend::OnnxCpu)?;
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
                self.read_lines(None, &lines)?;
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
            let mut texts = self.read_lines(None, &images)?;
            let before = known.iter().flatten().map(|(text, _)| text.as_str());
            let (on_screen, tried) = self.read_other_scripts(&images, &mut texts, before)?;
            // A line read before the script of another line showed up
            // (a number in a Korean sentence, alone on screen until the
            // next sentence appears) has not been through its model.
            let late: Vec<usize> = {
                let cache = self.lines.lock();
                (0..crops.len())
                    .filter(|&i| known[i].is_some() && on_screen & !cache.tried(keys[i]) != 0)
                    .collect()
            };
            if !late.is_empty() {
                let scripts: Vec<usize> = (0..self.scripts.len())
                    .filter(|&s| on_screen & (1 << s) != 0)
                    .collect();
                let images: Vec<&RgbImage> = late.iter().map(|&i| &crops[i].1).collect();
                let mut again: Vec<(String, f32)> =
                    late.iter().filter_map(|&i| known[i].clone()).collect();
                let mut on = vec![true; self.scripts.len()];
                let all = (0..late.len()).collect();
                self.read_with_scripts(&scripts, all, &images, &mut again, &mut on)?;
                let mut cache = self.lines.lock();
                for (&i, (text, conf)) in late.iter().zip(again) {
                    cache.put(keys[i], text.clone(), conf, LINE_CACHE_SIZE);
                    cache.set_tried(keys[i], on_screen);
                    known[i] = Some((text, conf));
                }
            }
            {
                let mut cache = self.lines.lock();
                for ((&c, (text, conf)), tried) in fresh.iter().zip(texts).zip(tried) {
                    cache.put(keys[c], text.clone(), conf, LINE_CACHE_SIZE);
                    cache.set_tried(keys[c], tried);
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

        /// Replace the standard readings `texts` of the line pictures
        /// `images` by those of the script models where these read
        /// better ([`ocr_scripts::reads_better`]).
        ///
        /// A script model costs as much per line as the standard one,
        /// so the lines are not all read by every model:
        ///
        /// 1. The scripts of the lines read `before` in the same
        ///    picture are known to be on screen. Their models read the
        ///    lines whose standard reading is in doubt.
        /// 2. Of the doubtful lines no model took, a few of the longest
        ///    go to the other models. A script that takes one of them
        ///    is on screen too, and reads the rest.
        /// 3. Doubtful lines still not taken go to the models left.
        /// 4. The lines the standard model was sure of go to the models
        ///    of the scripts on screen only: a number in a Korean
        ///    sentence comes out of the standard model as the number
        ///    alone, read with confidence.
        ///
        /// A screen without the installed scripts costs nothing beyond
        /// its doubtful lines; on a Korean screen every new line is
        /// read twice, whatever else is installed.
        ///
        /// Returns the scripts on screen and, per line, the scripts
        /// whose models it has been through, one bit per script of
        /// `self.scripts`.
        fn read_other_scripts<'a>(
            &self,
            images: &[&RgbImage],
            texts: &mut [(String, f32)],
            before: impl Iterator<Item = &'a str>,
        ) -> Result<(u32, Vec<u32>)> {
            /// Lines that show which scripts are on screen (step 2).
            const PROBE: usize = 4;
            if self.scripts.is_empty() {
                return Ok((0, vec![0; images.len()]));
            }
            let mut on_screen = vec![false; self.scripts.len()];
            for text in before {
                for (s, reader) in self.scripts.iter().enumerate() {
                    on_screen[s] |= reader.script.owns(text);
                }
            }
            let (mut doubtful, sure): (Vec<usize>, Vec<usize>) =
                (0..images.len()).partition(|&i| {
                    let (text, confidence) = &texts[i];
                    ocr_scripts::doubtful(text, *confidence, images[i].width(), images[i].height())
                });
            // Longest first: the more text, the surer the script.
            doubtful.sort_by_key(|&i| std::cmp::Reverse(images[i].width() / images[i].height()));
            // A doubtful line goes through every model unless one takes it.
            let every = (1u32 << self.scripts.len()) - 1;
            let mut tried = vec![0; images.len()];
            for &i in &doubtful {
                tried[i] = every;
            }
            let those = |on: &[bool], want: bool| -> Vec<usize> {
                (0..on.len()).filter(|&s| on[s] == want).collect()
            };

            let known = those(&on_screen, true);
            let mut open =
                self.read_with_scripts(&known, doubtful, images, texts, &mut on_screen)?;
            let rest = open.split_off(open.len().min(PROBE));
            let others = those(&on_screen, false);
            let mut probe_on = on_screen.clone();
            let mut open = self.read_with_scripts(&others, open, images, texts, &mut probe_on)?;
            let found: Vec<usize> = (0..probe_on.len())
                .filter(|&s| probe_on[s] && !on_screen[s])
                .collect();
            on_screen = probe_on;
            open.extend(self.read_with_scripts(&found, rest, images, texts, &mut on_screen)?);
            let left = those(&on_screen, false);
            self.read_with_scripts(&left, open, images, texts, &mut on_screen)?;

            let known = those(&on_screen, true);
            let shown = known.iter().fold(0, |bits, &s| bits | 1 << s);
            for &i in &sure {
                tried[i] = shown;
            }
            self.read_with_scripts(&known, sure, images, texts, &mut on_screen)?;
            Ok((shown, tried))
        }

        /// Read the pictures `lines` (indices into `images`) with each
        /// of `scripts` (indices into `self.scripts`) and keep, per
        /// line, the surest reading that is better than the standard
        /// one in `texts`. A script that took a line is marked in
        /// `on_screen`. Returns the lines no script took.
        fn read_with_scripts(
            &self,
            scripts: &[usize],
            lines: Vec<usize>,
            images: &[&RgbImage],
            texts: &mut [(String, f32)],
            on_screen: &mut [bool],
        ) -> Result<Vec<usize>> {
            if scripts.is_empty() || lines.is_empty() {
                return Ok(lines);
            }
            let pictures: Vec<&RgbImage> = lines.iter().map(|&i| images[i]).collect();
            let mut best: Vec<Option<(String, f32, usize)>> = vec![None; lines.len()];
            for &s in scripts {
                let script = self.scripts[s].script;
                let read = self.read_lines(Some(&self.scripts[s]), &pictures)?;
                for (k, (text, confidence)) in read.into_iter().enumerate() {
                    let text = script.in_writing_order(text);
                    let standard = &texts[lines[k]];
                    if ocr_scripts::reads_better(
                        script,
                        (&standard.0, standard.1),
                        (&text, confidence),
                    ) && best[k].as_ref().is_none_or(|b| confidence > b.1)
                    {
                        best[k] = Some((text, confidence, s));
                    }
                }
            }
            let mut open = Vec::new();
            for (line, read) in lines.into_iter().zip(best) {
                match read {
                    Some((text, confidence, s)) => {
                        texts[line] = (text, confidence);
                        on_screen[s] = true;
                    }
                    None => open.push(line),
                }
            }
            Ok(open)
        }

        /// Text and confidence of each line picture, in order, read
        /// the way the backend's [`RecPlan`] asks: by the standard
        /// model, or by the model of `script`.
        fn read_lines(
            &self,
            script: Option<&ScriptReader>,
            images: &[&RgbImage],
        ) -> Result<Vec<(String, f32)>> {
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
                let r = self.recognize_batch(script, &lines, width(chunk[0]));
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
        fn recognize_batch(
            &self,
            script: Option<&ScriptReader>,
            lines: &[&RgbImage],
            width: u32,
        ) -> Result<Vec<(String, f32)>> {
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
            let (reader, dict): (&dyn Reader, &[String]) = match script {
                Some(s) => (s.reader.as_ref(), &s.dict),
                None => (self.networks.as_ref(), &self.dict),
            };
            let (data, steps, classes) = reader.rec(input, n, w)?;
            Ok((0..n)
                .map(|i| {
                    let slice = &data[i * steps * classes..(i + 1) * steps * classes];
                    post::ctc_greedy_decode(slice, steps, classes, dict)
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
                return Err(ProviderError::NotInstalled("PP-OCR".into()));
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

/// Download and verify the models into `models_dir`.
/// `progress(filename, downloaded, total)` is called as bytes arrive.
/// No-op when the models are already installed. The models an earlier
/// version left there are deleted once the new ones are in place.
#[cfg(feature = "ocrs-engine")]
pub async fn install_models_with_progress(
    models_dir: &Path,
    progress: Option<&super::ocr_ocrs::ProgressCb>,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(models_dir)?;
    for model in &MODELS {
        model.install(models_dir, progress).await?;
    }
    remove_earlier_generation(models_dir);
    Ok(())
}

/// Delete the PP-OCRv5 files (about 43 MB) from a folder that holds
/// the current models. Best effort: a file that stays is only unused.
fn remove_earlier_generation(models_dir: &Path) {
    if !models_current(models_dir) {
        return;
    }
    for file in [V5_RTEN.0, V5_RTEN.1, V5_DICT, V5_ONNX.0, V5_ONNX.1] {
        let _ = std::fs::remove_file(models_dir.join(file));
    }
}

#[cfg(all(test, feature = "ocrs-engine"))]
mod tests {
    use super::*;

    fn folder(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            std::fs::write(dir.path().join(f), b"model").unwrap();
        }
        dir
    }

    #[test]
    fn the_current_generation_is_used_when_both_are_on_disk() {
        let v5 = [V5_RTEN.0, V5_RTEN.1, V5_DICT, V5_ONNX.0, V5_ONNX.1];
        let none = folder(&[]);
        assert_eq!(Generation::on_disk(none.path()), None);
        assert!(!models_installed(none.path()));

        // An install by an earlier version: OCR works, on v5.
        let earlier = folder(&v5);
        assert_eq!(Generation::on_disk(earlier.path()), Some(Generation::V5));
        assert!(models_installed(earlier.path()));
        assert!(!models_current(earlier.path()));

        // One of the two new files is not an install.
        std::fs::write(earlier.path().join(DET_FILE), b"model").unwrap();
        assert_eq!(Generation::on_disk(earlier.path()), Some(Generation::V5));
        std::fs::write(earlier.path().join(REC_FILE), b"model").unwrap();
        assert_eq!(Generation::on_disk(earlier.path()), Some(Generation::V6));
        assert!(models_current(earlier.path()));

        // An empty file is a download that never finished.
        let empty = folder(&[DET_FILE]);
        std::fs::write(empty.path().join(REC_FILE), b"").unwrap();
        assert_eq!(Generation::on_disk(empty.path()), None);
    }

    #[test]
    fn the_earlier_generation_is_deleted_only_once_the_current_one_is_there() {
        let v5 = [V5_RTEN.0, V5_RTEN.1, V5_DICT, V5_ONNX.0, V5_ONNX.1];
        let dir = folder(&v5);
        std::fs::write(dir.path().join(DET_FILE), b"model").unwrap();
        remove_earlier_generation(dir.path());
        assert_eq!(Generation::on_disk(dir.path()), Some(Generation::V5));

        std::fs::write(dir.path().join(REC_FILE), b"model").unwrap();
        std::fs::write(dir.path().join("text-detection.rten"), b"ocrs").unwrap();
        remove_earlier_generation(dir.path());
        for f in v5 {
            assert!(!dir.path().join(f).exists(), "{f} is still there");
        }
        assert!(models_current(dir.path()));
        assert!(dir.path().join("text-detection.rten").exists());
    }

    #[test]
    fn the_dictionary_is_whole() {
        let dict = Generation::V6.dict(Path::new("")).unwrap();
        // The recognition model has 18710 classes: these, the blank
        // and the space.
        assert_eq!(dict.len(), 18708);
        assert!(dict.iter().all(|l| l.chars().count() == 1));
        // A line that is blank space must survive editors and git.
        assert!(dict.iter().any(|l| l == "\u{3000}"));
        assert_eq!(dict[0], "!");
    }

    /// Needs the real models in `MOONLENS_PPOCR_DIR` (the two files of
    /// [`MODELS`], under their names there).
    #[test]
    #[ignore]
    fn real_models_verify_and_load() {
        let from = PathBuf::from(std::env::var("MOONLENS_PPOCR_DIR").unwrap());
        let dir = tempfile::tempdir().unwrap();
        for model in &MODELS {
            let bytes = std::fs::read(from.join(model.file)).unwrap();
            crate::providers::ocr_onnx::verify_and_store(&bytes, model, dir.path()).unwrap();
        }
        assert!(models_current(dir.path()));
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
    /// with reading quality. Needs the installed models and ONNX
    /// Runtime in the default models directory; reads
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

    /// Share of the characters of `page` (a fixture of
    /// `tests/fixtures/ocr`) that `engine` did not read as written:
    /// the edit distance between each line and what was read inside
    /// its box, over the length of the lines. Blank space is left out
    /// and full-width forms count as their plain ones.
    fn misread(engine: &engine::PpOcr, page: &str) -> f32 {
        fn distance(a: &[char], b: &[char]) -> usize {
            let mut row: Vec<usize> = (0..=b.len()).collect();
            for (i, ca) in a.iter().enumerate() {
                let mut diagonal = row[0];
                row[0] = i + 1;
                for (j, cb) in b.iter().enumerate() {
                    let cost = diagonal + usize::from(ca != cb);
                    diagonal = row[j + 1];
                    row[j + 1] = cost.min(row[j] + 1).min(diagonal + 1);
                }
            }
            row[b.len()]
        }
        let dir = Path::new("tests/fixtures/ocr");
        let truth: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join(format!("{page}.json"))).unwrap())
                .unwrap();
        let img = image::open(dir.join(format!("{page}.png"))).unwrap();
        let read = engine.recognize(&img, &Params::default()).unwrap();
        // A full-width form and its plain one are the same letter here
        // (the model writes "(" for "（" and "~" for "〜").
        let letters = |s: &str| -> Vec<char> {
            s.chars()
                .filter(|c| !c.is_whitespace())
                .map(|c| match c {
                    '\u{ff01}'..='\u{ff5e}' => char::from_u32(c as u32 - 0xfee0).unwrap(),
                    '〜' => '~',
                    c => c,
                })
                .collect()
        };
        let (mut wrong, mut all) = (0, 0);
        for line in truth["lines"].as_array().unwrap() {
            let place: Vec<f32> = line["box"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            let mut inside: Vec<&OcrLine> = read
                .iter()
                .filter(|l| {
                    let middle = l.bbox[1] + l.bbox[3] / 2.0;
                    (place[1]..=place[1] + place[3]).contains(&middle)
                })
                .collect();
            inside.sort_by(|a, b| a.bbox[0].total_cmp(&b.bbox[0]));
            let got: String = inside.iter().map(|l| l.text.as_str()).collect();
            let want = letters(line["text"].as_str().unwrap());
            wrong += distance(&want, &letters(&got));
            all += want.len();
        }
        wrong as f32 / all as f32
    }

    /// A page in each additional script is read by that script's
    /// model, and a Japanese page is read as it is without them.
    /// Needs the installed models with every script model
    /// (`cargo run --release --example fetch_models`).
    #[test]
    #[ignore]
    fn other_scripts_are_read() {
        use crate::providers::ocr_scripts::SCRIPTS;
        use engine::{PpOcr, Want};
        let dir = default_models_dir().unwrap();
        let with_scripts = PpOcr::load(&dir, Want::default()).unwrap();
        assert_eq!(
            with_scripts.scripts().len(),
            SCRIPTS.len(),
            "a script model is missing or did not load"
        );
        // The same standard models and nothing else.
        let bare = tempfile::tempdir().unwrap();
        for file in [DET_FILE, REC_FILE] {
            std::fs::copy(dir.join(file), bare.path().join(file)).unwrap();
        }
        let standard = PpOcr::load(bare.path(), Want::default()).unwrap();
        assert!(standard.scripts().is_empty());

        for script in &SCRIPTS {
            let (before, after) = (
                misread(&standard, script.id),
                misread(&with_scripts, script.id),
            );
            println!(
                "{}: {:.1} % misread, {:.1} % without its model",
                script.id,
                after * 100.0,
                before * 100.0
            );
            assert!(
                before > 0.5,
                "{}: the page needs no script model",
                script.id
            );
            assert!(after < 0.1, "{}: not read by its model", script.id);
        }

        // Every script model at hand takes nothing from a Japanese
        // page. One backend for both engines, so that the readings can
        // be compared to the letter.
        let rten = Want {
            backend: Backend::Rten,
            gpu: false,
        };
        let img = image::open("tests/fixtures/ocr/japanese.png").unwrap();
        let texts = |engine: &PpOcr| {
            let mut lines = engine.recognize(&img, &Params::default()).unwrap();
            lines.sort_by_key(|l| (l.bbox[1] as i32, l.bbox[0] as i32));
            lines.into_iter().map(|l| l.text).collect::<Vec<_>>()
        };
        let want = texts(&PpOcr::load(bare.path(), rten).unwrap());
        assert!(want.len() >= 16, "the Japanese page was not read");
        assert_eq!(texts(&PpOcr::load(&dir, rten).unwrap()), want);
        assert!(misread(&with_scripts, "japanese") < 0.02);
    }

    /// A line that was alone on screen and read by the standard model
    /// with confidence (a number in a Korean sentence) is read by the
    /// script's model once another line shows the script: the page
    /// reads the same as with nothing read before. Needs the installed
    /// models with the Korean one.
    #[test]
    #[ignore]
    fn a_line_read_before_its_script_showed_up_is_read_again() {
        use engine::{PpOcr, Want};
        let dir = default_models_dir().unwrap();
        let page = image::open("tests/fixtures/ocr/korean.png")
            .unwrap()
            .to_rgb8();
        // Blank space is left out: where a model puts it depends on
        // the lines read in one batch with the line.
        let read = |engine: &PpOcr, img: &image::RgbImage| {
            let img = image::DynamicImage::ImageRgb8(img.clone());
            let mut lines = engine.recognize(&img, &Params::default()).unwrap();
            lines.sort_by_key(|l| (l.bbox[1] as i32, l.bbox[0] as i32));
            for line in &mut lines {
                line.text.retain(|c| !c.is_whitespace());
            }
            lines
        };
        let whole = read(&PpOcr::load(&dir, Want::default()).unwrap(), &page);
        let line = |has: &str| whole.iter().find(|l| l.text.contains(has)).unwrap().bbox;
        // A page of two lines cut from the fixture: the end of a line,
        // a syllable or two and a number, and a sentence below it.
        let paper = *page.get_pixel(0, 0);
        let paste = |onto: &mut image::RgbImage, [x, y, w, h]: [f32; 4], to: (u32, u32)| {
            let pad = h * 0.4;
            let (x, y) = ((x - pad).max(0.0) as u32, (y - pad).max(0.0) as u32);
            let (w, h) = ((w + 2.0 * pad) as u32, (h + 2.0 * pad) as u32);
            let piece = image::imageops::crop_imm(&page, x, y, w, h).to_image();
            image::imageops::replace(onto, &piece, to.0 as i64, to.1 as i64);
        };
        let [x, y, w, h] = line("310");
        let end = [x + w - 4.2 * h, y, 4.2 * h, h];
        let mut alone = image::RgbImage::from_pixel(900, 240, paper);
        paste(&mut alone, end, (40, 40));
        let mut both = alone.clone();
        paste(&mut both, line("비밀번호"), (40, 140));

        let want: Vec<String> = read(&PpOcr::load(&dir, Want::default()).unwrap(), &both)
            .into_iter()
            .map(|l| l.text)
            .collect();
        let engine = PpOcr::load(&dir, Want::default()).unwrap();
        let first: Vec<String> = read(&engine, &alone).into_iter().map(|l| l.text).collect();
        println!("alone: {first:?}, with the sentence: {want:?}");
        assert_ne!(first[..], want[..1], "the line reads the same alone");
        let then: Vec<String> = read(&engine, &both).into_iter().map(|l| l.text).collect();
        assert_eq!(then, want);
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
        for file in [DET_FILE, REC_FILE] {
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
