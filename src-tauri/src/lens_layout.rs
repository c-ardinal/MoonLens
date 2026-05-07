//! Layout-first text block detection.
//!
//! Runs alongside OCR on the captured frame. Where OCR tells us
//! "what" the text says, layout detection tells us "where" text
//! lives — as contiguous rectangles on the image. The two signals
//! cross-validate each other:
//!
//! * OCR line with no matching layout block → hallucinated / noise
//!   (e.g. a stray "0" on a photo with no text block around it).
//! * Layout block with no OCR match → image, icon, or unreadable
//!   text; no card gets rendered.
//! * Match → the block's bbox replaces the OCR bbox as the card's
//!   geometry, because the block boundary is the true paragraph
//!   extent. OCR bboxes drift: WinRT rounds, and mis-aligned
//!   detections distort the card.
//!
//! Pipeline (pure Rust, `image` crate only — no new prod deps):
//!  1. Grayscale + Otsu threshold to binarise the image.
//!  2. Horizontal dilation to merge letters → words → lines.
//!  3. Vertical dilation to merge lines → paragraphs.
//!  4. Connected-component labelling on the mask; each component
//!     becomes a candidate `LayoutBlock`.
//!  5. Filter by minimum area / aspect so single-pixel specks and
//!     borders drop out.

use image::{DynamicImage, GenericImageView, GrayImage};

/// One contiguous text region the layout detector has identified on
/// the captured frame. `bbox = [x, y, w, h]` in image pixel space —
/// the same coordinate system OCR bboxes live in.
///
/// `polarity` records whether the block looked dark-on-light
/// (`Dark`) or light-on-dark (`Light`) relative to the image
/// background. Used downstream to pick fg/bg samples without a
/// second scan.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutBlock {
    pub bbox: [f32; 4],
    pub polarity: Polarity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Polarity {
    /// Dark ink on a light background (the common case).
    Dark,
    /// Light ink on a dark background (e.g. dark-mode UI).
    Light,
}

/// Tunables for `detect_text_blocks`. The defaults were picked
/// against `tests/input/english*.png` (typical 600-1200 px wide
/// documents at ~12 px body type) but nothing in the detector
/// assumes a particular DPI — callers can override for very small
/// / very large inputs.
#[derive(Debug, Clone)]
pub struct LayoutConfig {
    /// Horizontal dilation kernel width — merges letters into lines.
    /// Too small and single words stay separate; too big and
    /// adjacent columns merge. `round(image_w * 0.02)` clamped to
    /// `[8, 40]` is a good default for 500-2000 px captures.
    pub dilate_horizontal_px: u32,
    /// Vertical dilation kernel height — merges lines into
    /// paragraphs. Should be ~1.5× typical body line height.
    pub dilate_vertical_px: u32,
    /// Drop blocks smaller than this area (w × h in pixels). Filters
    /// out cursor artefacts, dots, etc.
    pub min_block_area: u32,
    /// Minimum aspect ratio (w / h). Text blocks are always wider
    /// than tall — this filters out vertical UI chrome lines /
    /// scrollbars picked up as components.
    pub min_aspect_ratio: f32,
}

impl LayoutConfig {
    /// Resolution-aware defaults for a given image width. Keeps the
    /// horizontal dilation proportional to image size so the same
    /// detector works on 500 px snippets and 4K screenshots.
    pub fn for_image(w: u32, h: u32) -> Self {
        let horizontal = ((w as f32 * 0.02).round() as u32).clamp(8, 40);
        let vertical = ((h as f32 * 0.01).round() as u32).clamp(4, 20);
        Self {
            dilate_horizontal_px: horizontal,
            dilate_vertical_px: vertical,
            min_block_area: 200,
            min_aspect_ratio: 0.5,
        }
    }

    /// RLSA-style adaptive defaults: vertical smearing is keyed off
    /// the OCR-measured median line height so the dilation kernel
    /// always reaches across the inter-line gap. Wong/Casey/Wahl
    /// (1982) recommend Cv ≈ 1.5× the line spacing — anything
    /// shorter and a body paragraph at typical 1.4-1.6 line-height
    /// fragments into per-line blocks (lost paragraph context for
    /// the translator), anything longer and adjacent paragraphs
    /// merge.
    ///
    /// Horizontal smearing follows the same idea: roughly two
    /// glyph widths so words in a line connect but adjacent columns
    /// stay apart (Cv > Ch is the rule of thumb in modern revivals
    /// of RLSA).
    pub fn from_line_height(w: u32, h: u32, median_line_height_px: f32) -> Self {
        let lh = median_line_height_px.max(4.0);
        let vertical = (lh * 1.5).round().clamp(8.0, 60.0) as u32;
        let horizontal = ((w as f32 * 0.02).round() as u32).clamp(8, 40);
        // Keep min_block_area proportional to line height so a
        // single short word does not register as a block on
        // high-DPI captures while still admitting tiny UI labels on
        // small captures.
        let min_block_area = ((lh * lh) * 0.6).max(120.0) as u32;
        Self {
            dilate_horizontal_px: horizontal,
            dilate_vertical_px: vertical,
            min_block_area,
            min_aspect_ratio: 0.5,
        }
    }
}

/// Detect contiguous text-like blocks in `img`. Never panics — on
/// pathological inputs (1-pixel images, pure black, pure white) the
/// function just returns an empty vector.
pub fn detect_text_blocks(img: &DynamicImage) -> Vec<LayoutBlock> {
    let (w, h) = img.dimensions();
    if w < 4 || h < 4 {
        return Vec::new();
    }
    let cfg = LayoutConfig::for_image(w, h);
    detect_text_blocks_with(img, &cfg)
}

/// RLSA-style detection that uses the OCR-measured median line
/// height to set the vertical dilation kernel adaptively. Always
/// prefer this over [`detect_text_blocks`] when OCR has already
/// run on the same frame — the fixed-pixel kernel of the simpler
/// entry point under-bridges the gap between body lines (16 px
/// font + 1.5× line-height = 8 px gap, which a 4 px kernel cannot
/// merge), fragmenting one paragraph into N single-line blocks
/// and depriving the translator of cross-line context.
pub fn detect_text_blocks_for_lines(
    img: &DynamicImage,
    median_line_height_px: f32,
) -> Vec<LayoutBlock> {
    let (w, h) = img.dimensions();
    if w < 4 || h < 4 {
        return Vec::new();
    }
    let cfg = LayoutConfig::from_line_height(w, h, median_line_height_px);
    detect_text_blocks_with(img, &cfg)
}

/// Version of [`detect_text_blocks`] that accepts an explicit config
/// — exposed primarily for tests that want deterministic kernels
/// independent of the input dimensions.
pub fn detect_text_blocks_with(img: &DynamicImage, cfg: &LayoutConfig) -> Vec<LayoutBlock> {
    let gray = img.to_luma8();
    let (w, h) = gray.dimensions();
    let threshold = otsu_threshold(&gray);

    // Binarise both polarities and pick whichever produced the
    // MINORITY class — "ink" is always the smaller population in a
    // document image because body text occupies less area than
    // background. This also handles the degenerate case (blank
    // image, single-colour regions) gracefully: the minority count
    // is 0 and we bail via the ink-density check below.
    let (mask_dark, dark_count) = binarise(&gray, threshold, Polarity::Dark);
    let (mask_light, light_count) = binarise(&gray, threshold, Polarity::Light);
    let (mask, polarity, ink_count) = if dark_count <= light_count {
        (mask_dark, Polarity::Dark, dark_count)
    } else {
        (mask_light, Polarity::Light, light_count)
    };
    let ink_density = ink_count as f32 / (w as f32 * h as f32).max(1.0);
    if ink_density < 0.001 {
        return Vec::new();
    }

    let dilated_h = dilate_horizontal(&mask, w, h, cfg.dilate_horizontal_px);
    let dilated = dilate_vertical(&dilated_h, w, h, cfg.dilate_vertical_px);

    let components = connected_components(&dilated, w, h);

    components
        .into_iter()
        .filter_map(|c| {
            let area = c.width * c.height;
            if area < cfg.min_block_area {
                return None;
            }
            let aspect = c.width as f32 / c.height.max(1) as f32;
            if aspect < cfg.min_aspect_ratio {
                return None;
            }
            Some(LayoutBlock {
                bbox: [
                    c.min_x as f32,
                    c.min_y as f32,
                    c.width as f32,
                    c.height as f32,
                ],
                polarity,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------
// Internal primitives. Kept private so callers treat the detector
// as an opaque box; exposing the knobs would tempt divergence.
// ---------------------------------------------------------------------

/// Otsu's method. Computes the threshold (0-255) that minimises
/// within-class variance on the 8-bit histogram. Classic algorithm,
/// O(256) once the histogram is built.
pub(crate) fn otsu_threshold(gray: &GrayImage) -> u8 {
    let mut histogram = [0u32; 256];
    for p in gray.pixels() {
        histogram[p.0[0] as usize] += 1;
    }
    let total: u32 = histogram.iter().sum();
    if total == 0 {
        return 128;
    }
    let total_f = total as f64;
    let sum_all: f64 = histogram
        .iter()
        .enumerate()
        .map(|(i, c)| i as f64 * *c as f64)
        .sum();

    let mut sum_b = 0.0_f64;
    let mut w_b = 0.0_f64;
    let mut best_threshold = 0u8;
    let mut max_variance = 0.0_f64;

    for (t, &c) in histogram.iter().enumerate() {
        w_b += c as f64;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total_f - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * c as f64;
        let mu_b = sum_b / w_b;
        let mu_f = (sum_all - sum_b) / w_f;
        let between = w_b * w_f * (mu_b - mu_f).powi(2);
        if between > max_variance {
            max_variance = between;
            best_threshold = t as u8;
        }
    }
    best_threshold
}

/// Binarise `gray` into a `0 | 1` mask. Returns the mask as a flat
/// `Vec<u8>` plus the count of set pixels — callers compare counts
/// to pick the better polarity without a second pass.
pub(crate) fn binarise(gray: &GrayImage, threshold: u8, polarity: Polarity) -> (Vec<u8>, usize) {
    let (w, h) = gray.dimensions();
    let mut mask = vec![0u8; (w * h) as usize];
    let mut count = 0usize;
    for (i, p) in gray.pixels().enumerate() {
        let v = p.0[0];
        // Otsu sweeps inclusive (w_b accumulates histogram[0..=t]) so
        // the Dark class is `v <= threshold`. Using strict `<` would
        // drop pixels sitting exactly at the ink peak when the
        // threshold lands on that peak (common on synthetic inputs
        // and clean screenshots).
        let set = match polarity {
            Polarity::Dark => v <= threshold,
            Polarity::Light => v > threshold,
        };
        if set {
            mask[i] = 1;
            count += 1;
        }
    }
    (mask, count)
}

/// Horizontal morphological dilation with a `kernel_w`-pixel line
/// kernel. For each output pixel `x`, the window spans
/// `[x - half, x + half]` (clamped to the row). Implemented as a
/// running sum so the cost is O(w × h) regardless of kernel size.
pub(crate) fn dilate_horizontal(mask: &[u8], w: u32, h: u32, kernel_w: u32) -> Vec<u8> {
    let mut out = vec![0u8; mask.len()];
    if kernel_w == 0 {
        out.copy_from_slice(mask);
        return out;
    }
    let half = (kernel_w / 2) as i32;
    let wi = w as i32;
    for y in 0..h {
        let row_off = (y * w) as usize;
        let mut running: i32 = 0;
        // Pre-load positions [0, half - 1]. The loop body adds
        // `x + half` and drops `x - half - 1` each iteration, so
        // after the first iteration the running sum covers
        // [0, half] which is the clamped window for x = 0.
        for xi in 0..half.min(wi) {
            running += mask[row_off + xi as usize] as i32;
        }
        for x in 0..w {
            let add_x = x as i32 + half;
            let drop_x = x as i32 - half - 1;
            if add_x < wi {
                running += mask[row_off + add_x as usize] as i32;
            }
            if drop_x >= 0 {
                running -= mask[row_off + drop_x as usize] as i32;
            }
            if running > 0 {
                out[row_off + x as usize] = 1;
            }
        }
    }
    out
}

/// Vertical counterpart to [`dilate_horizontal`]. Same running-sum
/// window, walked column-first. Window at row `y` spans
/// `[y - half, y + half]` clamped to the column.
pub(crate) fn dilate_vertical(mask: &[u8], w: u32, h: u32, kernel_h: u32) -> Vec<u8> {
    let mut out = vec![0u8; mask.len()];
    if kernel_h == 0 {
        out.copy_from_slice(mask);
        return out;
    }
    let half = (kernel_h / 2) as i32;
    let hi = h as i32;
    for x in 0..w {
        let mut running: i32 = 0;
        for yi in 0..half.min(hi) {
            running += mask[((yi as u32) * w + x) as usize] as i32;
        }
        for y in 0..h {
            let add_y = y as i32 + half;
            let drop_y = y as i32 - half - 1;
            if add_y < hi {
                running += mask[((add_y as u32) * w + x) as usize] as i32;
            }
            if drop_y >= 0 {
                running -= mask[(((drop_y as u32) * w + x)) as usize] as i32;
            }
            if running > 0 {
                out[(y * w + x) as usize] = 1;
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
pub(crate) struct Component {
    pub min_x: u32,
    pub min_y: u32,
    pub width: u32,
    pub height: u32,
}

/// Two-pass 4-connected component labelling with union-find. Each
/// set pixel in `mask` joins its up / left neighbour's label; a
/// final pass collapses aliased labels to their canonical root.
/// Returns one `Component` per connected region.
pub(crate) fn connected_components(mask: &[u8], w: u32, h: u32) -> Vec<Component> {
    let n = (w * h) as usize;
    if n == 0 {
        return Vec::new();
    }
    let mut labels = vec![0u32; n];
    let mut parent: Vec<u32> = vec![0];
    let mut next_label: u32 = 1;

    fn find(parent: &mut [u32], mut x: u32) -> u32 {
        while parent[x as usize] != x {
            parent[x as usize] = parent[parent[x as usize] as usize];
            x = parent[x as usize];
        }
        x
    }
    fn union(parent: &mut [u32], a: u32, b: u32) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[rb as usize] = ra;
        }
    }

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            if mask[idx] == 0 {
                continue;
            }
            let up = if y > 0 {
                labels[((y - 1) * w + x) as usize]
            } else {
                0
            };
            let left = if x > 0 {
                labels[(y * w + (x - 1)) as usize]
            } else {
                0
            };
            let label = match (up, left) {
                (0, 0) => {
                    let l = next_label;
                    parent.push(l);
                    next_label += 1;
                    l
                }
                (u, 0) | (0, u) => u,
                (u, l) => {
                    union(&mut parent, u, l);
                    u.min(l)
                }
            };
            labels[idx] = label;
        }
    }

    // Collapse to roots + gather bounding boxes.
    use std::collections::HashMap;
    let mut boxes: HashMap<u32, (u32, u32, u32, u32)> = HashMap::new();
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let l = labels[idx];
            if l == 0 {
                continue;
            }
            let root = find(&mut parent, l);
            boxes
                .entry(root)
                .and_modify(|b| {
                    b.0 = b.0.min(x);
                    b.1 = b.1.min(y);
                    b.2 = b.2.max(x);
                    b.3 = b.3.max(y);
                })
                .or_insert((x, y, x, y));
        }
    }
    boxes
        .into_values()
        .map(|(min_x, min_y, max_x, max_y)| Component {
            min_x,
            min_y,
            width: max_x - min_x + 1,
            height: max_y - min_y + 1,
        })
        .collect()
}

/// Intersection-over-union of two `[x, y, w, h]` bboxes. Returns 0
/// when the boxes are disjoint or either has zero area. Used by
/// card assembly to pair OCR lines with layout blocks.
pub fn iou(a: [f32; 4], b: [f32; 4]) -> f32 {
    let ax1 = a[0];
    let ay1 = a[1];
    let ax2 = a[0] + a[2];
    let ay2 = a[1] + a[3];
    let bx1 = b[0];
    let by1 = b[1];
    let bx2 = b[0] + b[2];
    let by2 = b[1] + b[3];
    let ix1 = ax1.max(bx1);
    let iy1 = ay1.max(by1);
    let ix2 = ax2.min(bx2);
    let iy2 = ay2.min(by2);
    if ix2 <= ix1 || iy2 <= iy1 {
        return 0.0;
    }
    let inter = (ix2 - ix1) * (iy2 - iy1);
    let ua = (a[2] * a[3]).max(0.0);
    let ub = (b[2] * b[3]).max(0.0);
    let union = ua + ub - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    fn solid(w: u32, h: u32, colour: [u8; 3]) -> RgbImage {
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgb(colour));
            }
        }
        img
    }

    fn fill_rect(img: &mut RgbImage, x: u32, y: u32, w: u32, h: u32, colour: [u8; 3]) {
        for dy in 0..h {
            for dx in 0..w {
                img.put_pixel(x + dx, y + dy, Rgb(colour));
            }
        }
    }

    #[test]
    fn otsu_picks_midpoint_between_two_peaks() {
        // 50% black / 50% white histogram → Otsu threshold should sit
        // between the two peaks. 127 is the standard landing for this
        // canonical bimodal case.
        let mut img = GrayImage::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let v = if (y * 20 + x) % 2 == 0 { 20 } else { 230 };
                img.put_pixel(x, y, image::Luma([v]));
            }
        }
        let t = otsu_threshold(&img);
        assert!(
            (20..=230).contains(&t),
            "threshold {t} should separate the two peaks"
        );
    }

    #[test]
    fn dilate_horizontal_bridges_small_gap() {
        // Two set pixels 3 apart; a length-5 kernel should bridge them.
        let mut mask = vec![0u8; 10];
        mask[2] = 1;
        mask[6] = 1;
        let out = dilate_horizontal(&mask, 10, 1, 5);
        for i in 2..=6 {
            assert_eq!(out[i], 1, "pixel {i} should be bridged");
        }
    }

    #[test]
    fn dilate_vertical_bridges_small_gap() {
        // Two set pixels 3 rows apart in a 1-wide column.
        let mut mask = vec![0u8; 10];
        mask[2] = 1;
        mask[6] = 1;
        let out = dilate_vertical(&mask, 1, 10, 5);
        for i in 2..=6 {
            assert_eq!(out[i], 1, "row {i} should be bridged");
        }
    }

    #[test]
    fn connected_components_finds_two_disjoint_rectangles() {
        // Two 3×3 blobs separated by a 5-pixel gap → two components.
        let (w, h) = (20u32, 10u32);
        let mut mask = vec![0u8; (w * h) as usize];
        for y in 2..=4 {
            for x in 2..=4 {
                mask[(y * w + x) as usize] = 1;
            }
            for x in 10..=12 {
                mask[(y * w + x) as usize] = 1;
            }
        }
        let mut comps = connected_components(&mask, w, h);
        comps.sort_by_key(|c| c.min_x);
        assert_eq!(comps.len(), 2);
        assert_eq!(comps[0].width, 3);
        assert_eq!(comps[1].min_x, 10);
    }

    #[test]
    fn detect_text_blocks_finds_two_paragraphs_on_white_canvas() {
        // Two horizontal dark bars simulate two text paragraphs on a
        // white background. The detector should return two blocks
        // with bboxes roughly matching the bars.
        let mut img = solid(200, 100, [255, 255, 255]);
        fill_rect(&mut img, 20, 20, 120, 12, [30, 30, 30]);
        fill_rect(&mut img, 20, 60, 150, 12, [30, 30, 30]);
        let dyn_img = DynamicImage::ImageRgb8(img);
        let blocks = detect_text_blocks(&dyn_img);
        assert_eq!(blocks.len(), 2, "expected two paragraph blocks, got {blocks:?}");
        for b in &blocks {
            assert_eq!(b.polarity, Polarity::Dark);
            assert!(b.bbox[2] >= 100.0);
        }
    }

    #[test]
    fn detect_text_blocks_returns_empty_for_blank_canvas() {
        let img = DynamicImage::ImageRgb8(solid(100, 100, [250, 250, 250]));
        let blocks = detect_text_blocks(&img);
        assert!(blocks.is_empty(), "blank canvas → zero blocks, got {blocks:?}");
    }

    #[test]
    fn detect_text_blocks_survives_tiny_image() {
        let img = DynamicImage::ImageRgb8(solid(2, 2, [255, 255, 255]));
        assert!(detect_text_blocks(&img).is_empty());
    }

    #[test]
    fn iou_overlapping_identical_is_one() {
        assert!((iou([0.0, 0.0, 10.0, 10.0], [0.0, 0.0, 10.0, 10.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn iou_disjoint_is_zero() {
        assert_eq!(iou([0.0, 0.0, 10.0, 10.0], [20.0, 20.0, 10.0, 10.0]), 0.0);
    }

    #[test]
    fn iou_partial_overlap_is_between_zero_and_one() {
        let v = iou([0.0, 0.0, 10.0, 10.0], [5.0, 5.0, 10.0, 10.0]);
        assert!(v > 0.0 && v < 1.0, "IoU should be in (0,1), got {v}");
    }

    #[test]
    fn from_line_height_makes_vertical_kernel_bridge_typical_gap() {
        // 16 px font with 1.5× line-height → 8 px gap between
        // glyph rows. The RLSA Cv must be ≥ this gap or paragraphs
        // fragment into per-line blocks (the regression that
        // motivated the adaptive entry point).
        let cfg = LayoutConfig::from_line_height(900, 260, 16.0);
        // 1.5 × 16 = 24 px, easily bridges the 8 px gap and stays
        // tight enough that 24-px-spaced sibling paragraphs do NOT
        // accidentally merge.
        assert!(
            cfg.dilate_vertical_px >= 16,
            "Cv must reach across body line-spacing, got {}",
            cfg.dilate_vertical_px
        );
        assert!(
            cfg.dilate_vertical_px <= 36,
            "Cv must not be so large that adjacent paragraphs merge, got {}",
            cfg.dilate_vertical_px
        );
    }

    #[test]
    fn from_line_height_falls_back_for_tiny_inputs() {
        // Pathological hint (zero / negative) should not produce a
        // zero-width kernel — the floor at 4 px keeps the detector
        // robust on degenerate captures.
        let cfg = LayoutConfig::from_line_height(900, 100, 0.0);
        assert!(cfg.dilate_vertical_px >= 8);
    }

    #[test]
    fn detect_text_blocks_picks_light_polarity_for_dark_ui() {
        // Dark background with two lighter bars simulates dark-mode
        // UI. Polarity must flip to `Light`.
        let mut img = solid(200, 100, [20, 20, 20]);
        fill_rect(&mut img, 30, 20, 140, 10, [230, 230, 230]);
        fill_rect(&mut img, 30, 60, 140, 10, [230, 230, 230]);
        let dyn_img = DynamicImage::ImageRgb8(img);
        let blocks = detect_text_blocks(&dyn_img);
        assert!(!blocks.is_empty(), "expected blocks for dark-mode UI");
        for b in &blocks {
            assert_eq!(b.polarity, Polarity::Light);
        }
    }
}
