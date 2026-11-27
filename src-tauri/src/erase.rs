//! Text-only erase for text on pictures and gradients.
//!
//! The in-place overlay normally hides the source text behind a box in
//! the background colour. On a photo or a gradient that box is a
//! visible patch. [`patch`] finds such blocks and returns a sparse
//! picture instead: the text pixels (and a thin fringe) repainted from
//! the pixels around them, everything else transparent. The overlay
//! draws it over the capture and writes the translation, outlined, on
//! top.
//!
//! Blocks on a plain background return `None` and keep the box: the
//! check reads a thin band around the rows and costs a few microseconds.

use base64::Engine;
use image::RgbaImage;
use serde::Serialize;

/// `[x, y, w, h]` in image pixels.
type Rect = [f32; 4];

/// Spread (5th to 95th percentile, any channel) of the pixels around
/// the rows above which the background is not plain. A plain background
/// with compression noise stays below 10; a gradient that a one-colour
/// box would visibly break is above 20.
const TEXTURED_SPREAD: i32 = 16;

/// Luma spread of the background above which the translation needs an
/// outline to stay readable.
const OUTLINE_SPREAD: i32 = 56;

/// Luma step between the pixels around the text and the open
/// background that marks a drop shadow or glow.
const HALO_CONTRAST: i32 = 20;

/// How much more often such pixels must occur around the text than in
/// the open background.
const HALO_SHARE: f32 = 0.12;

/// Share of the row pixels the mask may cover. Above it the text colour
/// is too close to the picture to tell the two apart.
const MAX_COVER: f32 = 0.6;

/// The repainted text pixels of one block.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ErasePatch {
    /// `[x, y, w, h]` of the picture in the pixels the block is placed
    /// in. `x` / `y` are negative where the picture starts outside a
    /// translation window (see `scan_margin`).
    pub rect: [i32; 4],
    /// PNG data URL: text pixels opaque, the rest transparent.
    pub image: String,
    /// The background varies enough that the translation needs an
    /// outline.
    pub outline: bool,
    /// Text colour read from the picture: the colour frequent inside
    /// the rows and rare around them. More reliable on a picture than
    /// the contrast-based `layout::estimate_colors`.
    #[serde(skip)]
    pub foreground: [u8; 3],
}

/// What [`patch`] reads of a block.
pub struct Target<'a> {
    /// Source text rows, top to bottom.
    pub rows: &'a [Rect],
    /// Right edge each row may be touched up to (see
    /// `layout::row_limits`); empty when unlimited.
    pub limits: &'a [f32],
    pub line_height: f32,
    /// Text colour, used when the picture does not single one out.
    pub foreground: [u8; 3],
}

/// Pixel rect, right and bottom exclusive.
#[derive(Clone, Copy)]
struct Area {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Area {
    fn w(&self) -> usize {
        (self.x1 - self.x0) as usize
    }
    fn h(&self) -> usize {
        (self.y1 - self.y0) as usize
    }
}

/// The erase picture for a block on a picture or gradient; `None` on a
/// plain background or when the text cannot be told from the picture.
pub fn patch(img: &RgbaImage, t: &Target<'_>) -> Option<ErasePatch> {
    let plan = plan(img, t)?;
    let rgba = fill(img, &plan);
    let png = crate::capture::png_bytes_fast(&rgba).ok()?;
    Some(ErasePatch {
        rect: [
            plan.area.x0 as i32,
            plan.area.y0 as i32,
            (plan.area.x1 - plan.area.x0) as i32,
            (plan.area.y1 - plan.area.y0) as i32,
        ],
        image: format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ),
        outline: plan.outline,
        foreground: plan.foreground,
    })
}

/// Where to erase: the working area and its text mask.
struct Plan {
    area: Area,
    /// One flag per pixel of `area`, row-major: repaint this pixel.
    mask: Vec<bool>,
    outline: bool,
    foreground: [u8; 3],
}

fn plan(img: &RgbaImage, t: &Target<'_>) -> Option<Plan> {
    let (iw, ih) = img.dimensions();
    if t.rows.is_empty() || iw == 0 || ih == 0 || t.line_height < 6.0 {
        return None;
    }
    let margin = (t.line_height * 0.3).round().clamp(3.0, 10.0);
    // Rows widened by the margin, cut at the row limit and the image.
    let rows: Vec<Area> = t
        .rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let limit = t.limits.get(i).copied().unwrap_or(f32::INFINITY);
            let right = (r[0] + r[2] + margin).min(limit.max(r[0] + r[2]));
            area_of(
                [r[0] - margin, r[1] - margin, right, r[1] + r[3] + margin],
                iw,
                ih,
            )
        })
        .collect();
    let area = rows.iter().copied().reduce(|a, b| Area {
        x0: a.x0.min(b.x0),
        y0: a.y0.min(b.y0),
        x1: a.x1.max(b.x1),
        y1: a.y1.max(b.y1),
    })?;
    let (w, h) = (area.w(), area.h());

    // Pixels of the area that belong to a (widened) row, and of those
    // the ones inside the row proper, where the text is.
    let mut in_rows = vec![false; w * h];
    let mut in_text = vec![false; w * h];
    for (r, src) in rows.iter().zip(t.rows) {
        let text = area_of([src[0], src[1], src[0] + src[2], src[1] + src[3]], iw, ih);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                let i = (y - area.y0) as usize * w + (x - area.x0) as usize;
                in_rows[i] = true;
                in_text[i] |= text.is_some_and(|t| x >= t.x0 && x < t.x1 && y >= t.y0 && y < t.y1);
            }
        }
    }
    let px = |i: usize| {
        let p = img
            .get_pixel(area.x0 + (i % w) as u32, area.y0 + (i / w) as u32)
            .0;
        [p[0] as i32, p[1] as i32, p[2] as i32]
    };

    // Background: the band around the rows.
    let band: Vec<[i32; 3]> = (0..w * h)
        .filter(|&i| in_rows[i] && !in_text[i])
        .map(px)
        .collect();
    if band.len() < 16 {
        return None;
    }
    let spread = |lo: f32, hi: f32, value: &dyn Fn(&[i32; 3]) -> i32| {
        let mut v: Vec<i32> = band.iter().map(value).collect();
        v.sort_unstable();
        let at = |q: f32| v[((v.len() - 1) as f32 * q) as usize];
        (at(hi) - at(lo), at(0.5))
    };
    let channels: Vec<(i32, i32)> = (0..3).map(|c| spread(0.05, 0.95, &|p| p[c])).collect();
    if channels.iter().all(|(s, _)| *s < TEXTURED_SPREAD) {
        return None;
    }
    let luma = |p: &[i32; 3]| (299 * p[0] + 587 * p[1] + 114 * p[2]) / 1000;
    let outline = spread(0.05, 0.95, &luma).0 >= OUTLINE_SPREAD;

    // Text pixels: closer to the text colour than half way to the
    // typical background.
    let inside: Vec<[i32; 3]> = (0..w * h).filter(|&i| in_text[i]).map(px).collect();
    let fg = text_colour(&inside, &band).unwrap_or(t.foreground.map(|c| c as i32));
    let dist2 = |a: [i32; 3], b: [i32; 3]| (0..3).map(|c| (a[c] - b[c]).pow(2)).sum::<i32>();
    let median = [channels[0].1, channels[1].1, channels[2].1];
    let reach = ((dist2(fg, median) as f32).sqrt() * 0.5).clamp(36.0, 96.0);
    let reach2 = (reach * reach) as i32;
    let mask: Vec<bool> = (0..w * h)
        .map(|i| in_text[i] && dist2(px(i), fg) <= reach2)
        .collect();
    let text_px = in_text.iter().filter(|t| **t).count().max(1);
    let cover = mask.iter().filter(|m| **m).count() as f32 / text_px as f32;
    if !(0.005..=MAX_COVER).contains(&cover) {
        return None;
    }
    // Anti-aliased edges and a thin outline of the source text.
    let grow = (t.line_height * 0.08).round().clamp(1.0, 4.0) as usize;
    let mut near = mask.clone();
    dilate(&mut near, w, h, grow);
    // A drop shadow or glow reaches further: when pixels clearly darker
    // (or lighter) than the background, away from the text colour, are
    // more frequent just around the text than elsewhere, erase them too.
    let reach = (t.line_height * 0.25).round().clamp(grow as f32, margin) as usize;
    let mut far = mask;
    dilate(&mut far, w, h, reach);
    let open: Vec<i32> = (0..w * h)
        .filter(|&i| in_rows[i] && !far[i])
        .map(|i| luma(&px(i)))
        .collect();
    let halo: Vec<i32> = (0..w * h)
        .filter(|&i| in_rows[i] && far[i] && !near[i])
        .map(|i| luma(&px(i)))
        .collect();
    let shadowed = open.len() >= 16 && halo.len() >= 16 && {
        let mut sorted = open.clone();
        sorted.sort_unstable();
        let typical = sorted[sorted.len() / 2];
        // Away from the text colour: a shadow under light text is dark.
        let side = -(luma(&fg) - typical).signum();
        let off = |v: &[i32]| {
            v.iter()
                .filter(|l| (**l - typical) * side >= HALO_CONTRAST)
                .count() as f32
                / v.len() as f32
        };
        off(&halo) - off(&open) >= HALO_SHARE
    };
    let mut mask = if shadowed { far } else { near };
    for (m, inside) in mask.iter_mut().zip(&in_rows) {
        *m &= *inside;
    }
    Some(Plan {
        area,
        mask,
        outline,
        foreground: fg.map(|c| c as u8),
    })
}

/// The colour of the text: the one that is frequent inside the rows
/// and rare in the band around them. The text has one colour wherever
/// it stands; a picture behind it shows about the same colours inside
/// and around the rows. `None` when no colour stands out.
fn text_colour(inside: &[[i32; 3]], band: &[[i32; 3]]) -> Option<[i32; 3]> {
    // 16 levels per channel.
    let bin = |p: &[i32; 3]| ((p[0] >> 4) << 8 | (p[1] >> 4) << 4 | (p[2] >> 4)) as usize;
    let mut text = vec![0u32; 4096];
    let mut around = vec![0u32; 4096];
    inside.iter().for_each(|p| text[bin(p)] += 1);
    band.iter().for_each(|p| around[bin(p)] += 1);
    // A gradient shows slightly different shades inside the rows than
    // around them: a colour next to one seen around the rows counts as
    // seen there too.
    let near = |b: usize| {
        let (r, g, bl) = ((b >> 8) as i32, (b >> 4 & 15) as i32, (b & 15) as i32);
        let mut n = 0u32;
        for dr in -1..=1 {
            for dg in -1..=1 {
                for db in -1..=1 {
                    let (r, g, bl) = (r + dr, g + dg, bl + db);
                    if (0..16).contains(&r) && (0..16).contains(&g) && (0..16).contains(&bl) {
                        n += around[(r << 8 | g << 4 | bl) as usize];
                    }
                }
            }
        }
        n
    };
    let scale = inside.len() as f32 / band.len().max(1) as f32;
    let (best, excess) = (0..4096)
        .filter(|b| text[*b] > 0)
        .map(|b| (b, text[b] as f32 - near(b) as f32 * scale))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;
    if excess < inside.len() as f32 * 0.02 {
        return None;
    }
    let mut sum = [0i32; 3];
    for p in inside.iter().filter(|p| bin(p) == best) {
        for c in 0..3 {
            sum[c] += p[c];
        }
    }
    Some(sum.map(|c| c / text[best] as i32))
}

/// `[x0, y0, x1, y1]` clipped to the image; `None` when empty.
fn area_of(r: [f32; 4], iw: u32, ih: u32) -> Option<Area> {
    let x0 = (r[0].floor().max(0.0) as u32).min(iw);
    let y0 = (r[1].floor().max(0.0) as u32).min(ih);
    let x1 = (r[2].ceil().max(0.0) as u32).min(iw);
    let y1 = (r[3].ceil().max(0.0) as u32).min(ih);
    (x1 > x0 && y1 > y0).then_some(Area { x0, y0, x1, y1 })
}

/// Grow the mask by `n` pixels (square, two passes).
fn dilate(mask: &mut [bool], w: usize, h: usize, n: usize) {
    let mut tmp = vec![false; mask.len()];
    for y in 0..h {
        let row = &mask[y * w..(y + 1) * w];
        for x in 0..w {
            let (a, b) = (x.saturating_sub(n), (x + n + 1).min(w));
            tmp[y * w + x] = row[a..b].iter().any(|m| *m);
        }
    }
    for y in 0..h {
        let (a, b) = (y.saturating_sub(n), (y + n + 1).min(h));
        for x in 0..w {
            mask[y * w + x] = (a..b).any(|yy| tmp[yy * w + x]);
        }
    }
}

/// One level of the fill pyramid: colour sums weighted by `weight`.
struct Level {
    w: usize,
    h: usize,
    /// Weighted colour per pixel (`rgb * weight`).
    color: Vec<[f32; 3]>,
    weight: Vec<f32>,
}

/// Repaint the masked pixels from the known ones around them
/// (push-pull: average the known pixels down a pyramid, then fill the
/// holes of each level from the level below). Linear in the area.
fn fill(img: &RgbaImage, plan: &Plan) -> RgbaImage {
    let (w, h) = (plan.area.w(), plan.area.h());
    let mut base = Level {
        w,
        h,
        color: vec![[0.0; 3]; w * h],
        weight: vec![0.0; w * h],
    };
    for i in 0..w * h {
        if !plan.mask[i] {
            let p = img
                .get_pixel(plan.area.x0 + (i % w) as u32, plan.area.y0 + (i / w) as u32)
                .0;
            base.color[i] = [p[0] as f32, p[1] as f32, p[2] as f32];
            base.weight[i] = 1.0;
        }
    }
    let mut levels = vec![base];
    while levels.last().is_some_and(|l| l.w > 1 || l.h > 1) {
        let next = shrink(levels.last().expect("non-empty"));
        levels.push(next);
    }
    for i in (0..levels.len() - 1).rev() {
        let (fine, coarse) = levels.split_at_mut(i + 1);
        pull(&mut fine[i], &coarse[0]);
    }
    let base = &levels[0];
    let mut out = RgbaImage::new(w as u32, h as u32);
    for (i, p) in out.pixels_mut().enumerate() {
        if plan.mask[i] {
            let c = base.color[i];
            p.0 = [
                c[0].round() as u8,
                c[1].round() as u8,
                c[2].round() as u8,
                255,
            ];
        }
    }
    out
}

/// Halve a level: each pixel is the weighted mean of its 2x2 block.
fn shrink(l: &Level) -> Level {
    let (w, h) = (l.w.div_ceil(2), l.h.div_ceil(2));
    let mut out = Level {
        w,
        h,
        color: vec![[0.0; 3]; w * h],
        weight: vec![0.0; w * h],
    };
    for y in 0..l.h {
        for x in 0..l.w {
            let (s, d) = (y * l.w + x, (y / 2) * w + x / 2);
            for c in 0..3 {
                out.color[d][c] += l.color[s][c];
            }
            out.weight[d] += l.weight[s];
        }
    }
    // Keep the mean colour; a block with any known pixel counts as known.
    for d in 0..w * h {
        let n = out.weight[d];
        if n > 0.0 {
            for c in &mut out.color[d] {
                *c /= n;
            }
            out.weight[d] = 1.0;
        }
    }
    out
}

/// Fill the holes of `fine` from `coarse` (already complete), smoothly
/// interpolated.
fn pull(fine: &mut Level, coarse: &Level) {
    let at = |x: isize, y: isize| {
        let x = x.clamp(0, coarse.w as isize - 1) as usize;
        let y = y.clamp(0, coarse.h as isize - 1) as usize;
        coarse.color[y * coarse.w + x]
    };
    for y in 0..fine.h {
        for x in 0..fine.w {
            let i = y * fine.w + x;
            if fine.weight[i] > 0.0 {
                continue;
            }
            // Bilinear sample of the coarse level at this pixel's centre.
            let fx = (x as f32 + 0.5) / 2.0 - 0.5;
            let fy = (y as f32 + 0.5) / 2.0 - 0.5;
            let (x0, y0) = (fx.floor(), fy.floor());
            let (tx, ty) = (fx - x0, fy - y0);
            let (x0, y0) = (x0 as isize, y0 as isize);
            let (a, b, c, d) = (
                at(x0, y0),
                at(x0 + 1, y0),
                at(x0, y0 + 1),
                at(x0 + 1, y0 + 1),
            );
            for k in 0..3 {
                let top = a[k] + (b[k] - a[k]) * tx;
                let bottom = c[k] + (d[k] - c[k]) * tx;
                fine.color[i][k] = top + (bottom - top) * ty;
            }
            fine.weight[i] = 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    const W: u32 = 240;
    const H: u32 = 60;
    /// The row of the test pictures.
    const ROW: Rect = [20.0, 20.0, 200.0, 20.0];

    /// A picture whose background comes from `bg`, with "text": vertical
    /// white bars 3 px wide inside [`ROW`].
    fn picture(bg: impl Fn(u32, u32) -> [u8; 3]) -> RgbaImage {
        RgbaImage::from_fn(W, H, |x, y| {
            let bar = (22..218).contains(&x) && (22..38).contains(&y) && x % 12 < 3;
            let [r, g, b] = if bar { [255, 255, 255] } else { bg(x, y) };
            Rgba([r, g, b, 255])
        })
    }

    fn gradient(x: u32, _: u32) -> [u8; 3] {
        [(x / 2) as u8, 40, (200 - x / 2) as u8]
    }

    fn target(rows: &[Rect]) -> Target<'_> {
        Target {
            rows,
            limits: &[],
            line_height: 20.0,
            foreground: [255, 255, 255],
        }
    }

    fn decode(p: &ErasePatch) -> RgbaImage {
        let b64 = p
            .image
            .strip_prefix("data:image/png;base64,")
            .expect("data url");
        let png = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("base64");
        image::load_from_memory(&png).expect("png").to_rgba8()
    }

    #[test]
    fn plain_background_keeps_the_box() {
        let img = picture(|_, _| [30, 30, 34]);
        assert_eq!(patch(&img, &target(&[ROW])), None);
    }

    #[test]
    fn slight_noise_is_still_plain() {
        let img = picture(|x, y| {
            let n = ((x * 7 + y * 13) % 5) as u8;
            [200 + n, 200 + n, 204 - n]
        });
        assert_eq!(patch(&img, &target(&[ROW])), None);
    }

    #[test]
    fn text_on_a_gradient_is_repainted_with_the_gradient() {
        let img = picture(gradient);
        let rows = [ROW];
        let p = patch(&img, &target(&rows)).expect("gradient needs the erase picture");
        let out = decode(&p);
        assert_eq!(
            [out.width(), out.height()],
            [p.rect[2] as u32, p.rect[3] as u32]
        );
        let mut repainted = 0;
        for (x, y, px) in out.enumerate_pixels() {
            let (ix, iy) = (p.rect[0] as u32 + x, p.rect[1] as u32 + y);
            let was_text = img.get_pixel(ix, iy).0 == [255, 255, 255, 255];
            if was_text {
                assert_eq!(px[3], 255, "text pixel {ix},{iy} left as is");
            }
            if px[3] == 255 {
                repainted += 1;
                let want = gradient(ix, iy);
                for c in 0..3 {
                    assert!(
                        (px[c] as i32 - want[c] as i32).abs() <= 12,
                        "pixel {ix},{iy}: {:?} instead of {want:?}",
                        px.0
                    );
                }
            }
        }
        // Text pixels and a fringe only: most of the picture stays clear.
        let share = repainted as f32 / (out.width() * out.height()) as f32;
        assert!((0.1..0.6).contains(&share), "repainted share {share}");
        // A smooth gradient keeps the text readable without an outline.
        assert!(!p.outline);
    }

    #[test]
    fn a_strongly_varying_background_asks_for_an_outline() {
        let img = picture(|x, y| {
            if (x / 30 + y / 30) % 2 == 0 {
                [20, 30, 90]
            } else {
                [150, 120, 60]
            }
        });
        let p = patch(&img, &target(&[ROW])).expect("picture needs the erase picture");
        assert!(p.outline);
    }

    #[test]
    fn nothing_right_of_the_row_limit_is_touched() {
        let img = picture(gradient);
        let rows = [ROW];
        let limits = [120.0];
        let t = Target {
            limits: &limits,
            ..target(&rows)
        };
        let p = patch(&img, &t).expect("patch");
        assert!(p.rect[0] + p.rect[2] <= 220);
        let out = decode(&p);
        for (x, _, px) in out.enumerate_pixels() {
            // The row itself may be repainted up to its end, never past it.
            assert!(px[3] == 0 || p.rect[0] as u32 + x < 220);
        }
    }

    #[test]
    fn text_in_the_colour_of_the_picture_keeps_the_box() {
        // Nearly white picture, white text: no way to tell them apart.
        let img = picture(|x, _| [(215 + x / 6) as u8, 250, 250]);
        assert_eq!(patch(&img, &target(&[ROW])), None);
    }

    #[test]
    fn a_drop_shadow_is_erased_with_the_text() {
        // White bars with a dark shadow 4 px to the right on a gradient.
        let base = picture(gradient);
        let img = RgbaImage::from_fn(W, H, |x, y| {
            let shadow =
                (22..218).contains(&x) && (22..38).contains(&y) && (3..7).contains(&(x % 12));
            if shadow {
                Rgba([0, 0, 0, 255])
            } else {
                *base.get_pixel(x, y)
            }
        });
        let rows = [ROW];
        let p = patch(&img, &target(&rows)).expect("patch");
        let out = decode(&p);
        for (x, y, px) in out.enumerate_pixels() {
            let (ix, iy) = (p.rect[0] as u32 + x, p.rect[1] as u32 + y);
            if img.get_pixel(ix, iy).0 == [0, 0, 0, 255] {
                assert_eq!(px[3], 255, "shadow pixel {ix},{iy} left as is");
            }
        }
    }

    #[test]
    fn the_text_colour_is_read_from_the_picture() {
        // Red bars on a dark picture: close to the background in
        // brightness, so the caller's guess (white) is wrong.
        let base = picture(|x, y| {
            if (x / 30 + y / 30) % 2 == 0 {
                [20, 30, 90]
            } else {
                [60, 50, 40]
            }
        });
        let img = RgbaImage::from_fn(W, H, |x, y| {
            let p = *base.get_pixel(x, y);
            if p.0 == [255, 255, 255, 255] {
                Rgba([200, 30, 60, 255])
            } else {
                p
            }
        });
        let p = patch(&img, &target(&[ROW])).expect("patch");
        assert_eq!(p.foreground, [200, 30, 60]);
        let out = decode(&p);
        for (x, y, px) in out.enumerate_pixels() {
            if img.get_pixel(p.rect[0] as u32 + x, p.rect[1] as u32 + y).0 == [200, 30, 60, 255] {
                assert_eq!(px[3], 255);
            }
        }
    }

    #[test]
    fn a_gradient_across_the_rows_is_not_taken_for_the_text() {
        // Top to bottom: the shades inside the row do not occur above
        // or below it, the text colour still has to win.
        let img = picture(|_, y| [(250 - y) as u8, (250 - y) as u8, (255 - y) as u8]);
        let dark = RgbaImage::from_fn(W, H, |x, y| {
            let p = *img.get_pixel(x, y);
            if p.0 == [255, 255, 255, 255] {
                Rgba([34, 34, 34, 255])
            } else {
                p
            }
        });
        let rows = [ROW];
        let t = Target {
            foreground: [34, 34, 34],
            ..target(&rows)
        };
        let p = patch(&dark, &t).expect("patch");
        assert_eq!(p.foreground, [34, 34, 34]);
    }

    #[test]
    fn rows_outside_the_image_are_ignored() {
        let img = picture(gradient);
        assert_eq!(patch(&img, &target(&[[500.0, 500.0, 80.0, 20.0]])), None);
        assert_eq!(patch(&img, &target(&[])), None);
    }
}
