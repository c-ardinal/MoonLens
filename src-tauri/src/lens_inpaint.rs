//! Pure-Rust inpainting for removing source text from captured lens
//! frames. The backend uses it to generate the PNG patch the Lens UI
//! draws under the translation; the `lens_eval` example binary reuses
//! the same functions so the evaluator never drifts from production.
//!
//! Pipeline:
//!  1. Build a text mask from each pixel's distance to sampled fg /
//!     bg (see [`text_mask`]).
//!  2. Dilate the mask so scanline boundaries fall on clean background.
//!  3. Run horizontal + vertical scanline linear interpolation between
//!     the masked spans' outer pixels (see [`fill_scanlines`]).
//!  4. Average the two axes for masked pixels; keep originals elsewhere.
//!
//! Scales well for the typical Lens bbox (≤ 1k pixels tall, ≤ a few
//! thousand wide): every step is O(width × height), no iteration.

use image::{Rgb, RgbImage};

/// Produce an inpainted copy of `bbox` from `src_rgb`, replacing
/// every pixel classified as source text with a scanline-interpolated
/// background value. Pixels outside the text mask are preserved
/// verbatim so the result seamlessly composites back into the
/// surrounding frame.
///
/// * `bg` — mean background colour sampled from the bbox rim.
/// * `fg` — mean foreground colour sampled from the bbox interior.
/// * `bbox` — `[x, y, w, h]` in `src_rgb` pixel coordinates.
pub fn inpaint_text_in_bbox(
    src_rgb: &RgbImage,
    bbox: [f32; 4],
    bg: [u8; 3],
    fg: [u8; 3],
) -> RgbImage {
    let img_w = src_rgb.width();
    let img_h = src_rgb.height();
    let x0 = bbox[0].max(0.0) as u32;
    let y0 = bbox[1].max(0.0) as u32;
    let w = (bbox[2] as u32).min(img_w.saturating_sub(x0));
    let h = (bbox[3] as u32).min(img_h.saturating_sub(y0));
    if w < 1 || h < 1 {
        return RgbImage::new(1, 1);
    }

    let mut patch = RgbImage::new(w, h);
    for dy in 0..h {
        for dx in 0..w {
            patch.put_pixel(dx, dy, *src_rgb.get_pixel(x0 + dx, y0 + dy));
        }
    }

    let mask = text_mask(&patch, bg, fg);
    // Dilate the mask so the scanline boundary pixels sit in clean
    // background — otherwise the fill interpolates between the
    // anti-aliased halo and reproduces a dim ghost of the glyphs.
    let dilate_radius = ((h as f32 / 8.0).round() as i32).clamp(3, 5);
    let mask = dilate_mask(&mask, w, h, dilate_radius);

    let h_pass = fill_scanlines(&patch, &mask, w, h, true, bg);
    let v_pass = fill_scanlines(&patch, &mask, w, h, false, bg);

    let mut out = patch;
    for dy in 0..h {
        for dx in 0..w {
            let idx = (dy * w + dx) as usize;
            if mask[idx] {
                let hp = h_pass.get_pixel(dx, dy).0;
                let vp = v_pass.get_pixel(dx, dy).0;
                out.put_pixel(
                    dx,
                    dy,
                    Rgb([
                        ((hp[0] as u16 + vp[0] as u16) / 2) as u8,
                        ((hp[1] as u16 + vp[1] as u16) / 2) as u8,
                        ((hp[2] as u16 + vp[2] as u16) / 2) as u8,
                    ]),
                );
            }
        }
    }
    out
}

/// Build the text-body mask. A pixel is "text" when it is closer
/// to `fg` than to `bg`. Halo pixels the classifier misses on
/// anti-aliased edges are caught by the post-mask dilation step,
/// not by weakening the classifier — the earlier `dbg >= 18 OR
/// dfg < dbg` rule grabbed far too many gradient / photo pixels
/// and the scanline fill collapsed them into a solid colour block
/// that showed through on any non-uniform surface.
fn text_mask(patch: &RgbImage, bg: [u8; 3], fg: [u8; 3]) -> Vec<bool> {
    let (w, h) = patch.dimensions();
    let mut mask = vec![false; (w * h) as usize];
    for dy in 0..h {
        for dx in 0..w {
            let p = patch.get_pixel(dx, dy).0;
            if color_dist_sq(p, fg) < color_dist_sq(p, bg) {
                mask[(dy * w + dx) as usize] = true;
            }
        }
    }
    mask
}

/// Morphological dilation on a boolean mask — every pixel within
/// Chebyshev distance `radius` of a masked pixel becomes masked.
/// Used to expand the text mask over anti-aliased halos that the
/// colour-distance mask alone misses.
pub fn dilate_mask(mask: &[bool], w: u32, h: u32, radius: i32) -> Vec<bool> {
    let mut out = vec![false; mask.len()];
    let wi = w as i32;
    let hi = h as i32;
    for y in 0..hi {
        for x in 0..wi {
            if !mask[(y * wi + x) as usize] {
                continue;
            }
            let y0 = (y - radius).max(0);
            let y1 = (y + radius).min(hi - 1);
            let x0 = (x - radius).max(0);
            let x1 = (x + radius).min(wi - 1);
            for ny in y0..=y1 {
                for nx in x0..=x1 {
                    out[(ny * wi + nx) as usize] = true;
                }
            }
        }
    }
    out
}

/// Linear-interpolation fill for every masked span along one axis.
/// `horizontal = true` walks rows left-to-right; `false` walks
/// columns top-to-bottom. Boundary pixels either side of the span
/// seed the interpolation; when a span touches the edge of the
/// patch we fall back to `fallback` (typically the sampled `bg`).
pub fn fill_scanlines(
    patch: &RgbImage,
    mask: &[bool],
    w: u32,
    h: u32,
    horizontal: bool,
    fallback: [u8; 3],
) -> RgbImage {
    let mut out = patch.clone();
    let (outer_n, inner_n) = if horizontal { (h, w) } else { (w, h) };
    for o in 0..outer_n {
        let mut i = 0u32;
        while i < inner_n {
            let coord = |k: u32| -> (u32, u32) {
                if horizontal {
                    (k, o)
                } else {
                    (o, k)
                }
            };
            let (x, y) = coord(i);
            if !mask[(y * w + x) as usize] {
                i += 1;
                continue;
            }
            let start = i;
            let mut end = start;
            while end < inner_n {
                let (ex_x, ex_y) = coord(end);
                if !mask[(ex_y * w + ex_x) as usize] {
                    break;
                }
                end += 1;
            }
            let left = if start > 0 {
                let (lx, ly) = coord(start - 1);
                Some(patch.get_pixel(lx, ly).0)
            } else {
                None
            };
            let right = if end < inner_n {
                let (rx, ry) = coord(end);
                Some(patch.get_pixel(rx, ry).0)
            } else {
                None
            };
            for k in start..end {
                let (kx, ky) = coord(k);
                let px = match (left, right) {
                    (Some(l), Some(r)) => {
                        let span = (end - start + 1) as f32;
                        let t = (k - start + 1) as f32 / span;
                        [
                            (l[0] as f32 * (1.0 - t) + r[0] as f32 * t) as u8,
                            (l[1] as f32 * (1.0 - t) + r[1] as f32 * t) as u8,
                            (l[2] as f32 * (1.0 - t) + r[2] as f32 * t) as u8,
                        ]
                    }
                    (Some(l), None) => l,
                    (None, Some(r)) => r,
                    (None, None) => fallback,
                };
                out.put_pixel(kx, ky, Rgb(px));
            }
            i = end;
        }
    }
    out
}

fn color_dist_sq(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    fn solid(w: u32, h: u32, color: [u8; 3]) -> RgbImage {
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgb(color));
            }
        }
        img
    }

    #[test]
    fn inpaint_removes_dark_text_on_light_bg() {
        let mut img = solid(40, 24, [230, 230, 230]);
        // Vertical "glyph" stroke — 2 px wide, 20 px tall.
        for y in 2..22 {
            for x in 18..20 {
                img.put_pixel(x, y, Rgb([20, 20, 20]));
            }
        }
        let out = inpaint_text_in_bbox(&img, [0.0, 0.0, 40.0, 24.0], [230, 230, 230], [20, 20, 20]);
        // The glyph pixels should now be close to the bg colour.
        for y in 2..22 {
            for x in 18..20 {
                let p = out.get_pixel(x, y).0;
                let dr = p[0] as i32 - 230;
                let dg = p[1] as i32 - 230;
                let db = p[2] as i32 - 230;
                let dist_sq = dr * dr + dg * dg + db * db;
                assert!(
                    dist_sq < 40 * 40,
                    "pixel at ({x},{y}) = {p:?} should be near bg"
                );
            }
        }
    }

    #[test]
    fn inpaint_preserves_untouched_background() {
        let img = solid(20, 20, [180, 190, 200]);
        let out = inpaint_text_in_bbox(&img, [0.0, 0.0, 20.0, 20.0], [180, 190, 200], [0, 0, 0]);
        for y in 0..20 {
            for x in 0..20 {
                assert_eq!(out.get_pixel(x, y).0, [180, 190, 200]);
            }
        }
    }

    #[test]
    fn dilate_mask_expands_by_radius() {
        // Single masked pixel at (5, 5) in a 11×11 mask.
        let (w, h) = (11u32, 11u32);
        let mut mask = vec![false; (w * h) as usize];
        mask[(5 * w + 5) as usize] = true;
        let out = dilate_mask(&mask, w, h, 2);
        for y in 3..=7 {
            for x in 3..=7 {
                assert!(out[(y * w + x) as usize], "({x},{y}) should be masked");
            }
        }
        // Outside the 5×5 Chebyshev radius stays unset.
        assert!(!out[(2 * w + 5) as usize]);
        assert!(!out[(8 * w + 5) as usize]);
    }
}
