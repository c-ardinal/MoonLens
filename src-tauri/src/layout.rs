//! Text layout analysis for in-place ("Lens-style") translation.
//!
//! OCR engines hand back individual *lines*. Translating line by line
//! loses sentence context (a sentence wrapped over three lines becomes
//! three fragments), and translating everything as one blob loses the
//! position information needed to paint the translation back over the
//! original text. This module sits in between:
//!
//! 1. [`group_lines`] clusters lines into paragraph-like blocks using
//!    vertical proximity, horizontal overlap and line-height similarity.
//! 2. [`unwrap_lines`] joins the lines of a block into one string,
//!    de-hyphenating and using no separator between CJK characters.
//! 3. [`split_translation`] maps a translation of `"block\n\nblock"`
//!    back onto the blocks.
//! 4. [`estimate_colors`] samples the captured image so the renderer can
//!    erase the original text with the surrounding background colour
//!    and paint the translation in a matching foreground colour.
//!
//! Everything here is pure and unit tested; no display server needed.

use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::providers::OcrLine;

/// Axis-aligned rect in image pixels: `[x, y, w, h]`.
pub type BBox = [f32; 4];

/// A paragraph-like group of OCR lines.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextBlock {
    /// Unwrapped text of the whole block (lines joined).
    pub text: String,
    /// Union of the line rects.
    pub bbox: BBox,
    /// Median line height in pixels. The renderer uses it as the
    /// starting font size when fitting the translation into `bbox`.
    pub line_height: f32,
    /// Number of source lines in the block.
    pub line_count: usize,
}

fn bottom(b: &BBox) -> f32 {
    b[1] + b[3]
}
fn right(b: &BBox) -> f32 {
    b[0] + b[2]
}

fn union(a: &BBox, b: &BBox) -> BBox {
    let x0 = a[0].min(b[0]);
    let y0 = a[1].min(b[1]);
    let x1 = right(a).max(right(b));
    let y1 = bottom(a).max(bottom(b));
    [x0, y0, x1 - x0, y1 - y0]
}

fn median(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    values[values.len() / 2]
}

struct Building {
    bbox: BBox,
    last: BBox,
    lines: Vec<OcrLine>,
}

/// Group OCR lines into paragraph blocks.
///
/// A line joins the most recent compatible block when
/// - its top is no further than ~0.6 line heights below the block's last line
///   (wrapped lines sit ~0.1–0.3 apart, paragraphs ~0.7+),
/// - its height is within 1.3× of the block's last line (headings vs body
///   text become separate blocks), and
/// - it horizontally overlaps the block (keeps columns apart).
///
/// Lines with an empty/zero bbox (OCR engines that do not report
/// geometry) are put in a single block so callers still get text.
pub fn group_lines(lines: &[OcrLine]) -> Vec<TextBlock> {
    let mut usable: Vec<OcrLine> = lines
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .cloned()
        .collect();
    if usable.is_empty() {
        return vec![];
    }
    let no_geometry = usable.iter().all(|l| l.bbox[2] <= 0.0 || l.bbox[3] <= 0.0);
    if no_geometry {
        let text = unwrap_lines(usable.iter().map(|l| l.text.as_str()));
        return vec![TextBlock {
            text,
            bbox: [0.0, 0.0, 0.0, 0.0],
            line_height: 0.0,
            line_count: usable.len(),
        }];
    }

    // Reading order: top to bottom, then left to right.
    usable.sort_by(|a, b| {
        a.bbox[1]
            .partial_cmp(&b.bbox[1])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.bbox[0]
                    .partial_cmp(&b.bbox[0])
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    let mut blocks: Vec<Building> = Vec::new();
    for line in usable {
        let lb = line.bbox;
        let lh = lb[3].max(1.0);
        let mut target: Option<usize> = None;
        // Prefer the most recently started block that fits; scanning in
        // reverse keeps multi-column layouts from merging.
        for (i, b) in blocks.iter().enumerate().rev() {
            let prev_h = b.last[3].max(1.0);
            let gap = lb[1] - bottom(&b.last);
            let size_ratio = (lh / prev_h).max(prev_h / lh);
            let overlap = right(&lb).min(right(&b.bbox)) - lb[0].max(b.bbox[0]);
            let max_gap = prev_h.max(lh) * 0.6;
            // Lines on the same row (gap negative) belong together only
            // when they also overlap horizontally; otherwise they are
            // separate columns / UI labels.
            if gap <= max_gap && gap >= -prev_h * 0.5 && size_ratio <= 1.3 && overlap > 0.0 {
                target = Some(i);
                break;
            }
        }
        match target {
            Some(i) => {
                let b = &mut blocks[i];
                b.bbox = union(&b.bbox, &lb);
                b.last = lb;
                b.lines.push(line);
            }
            None => blocks.push(Building {
                bbox: lb,
                last: lb,
                lines: vec![line],
            }),
        }
    }

    let mut out: Vec<TextBlock> = blocks
        .into_iter()
        .map(|b| {
            let mut heights: Vec<f32> = b.lines.iter().map(|l| l.bbox[3]).collect();
            TextBlock {
                text: unwrap_lines(b.lines.iter().map(|l| l.text.as_str())),
                bbox: b.bbox,
                line_height: median(&mut heights),
                line_count: b.lines.len(),
            }
        })
        .collect();
    out.sort_by(|a, b| {
        a.bbox[1]
            .partial_cmp(&b.bbox[1])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.bbox[0]
                    .partial_cmp(&b.bbox[0])
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    out
}

/// True for scripts written without spaces between words.
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F   // CJK punctuation
        | 0x3040..=0x30FF // Hiragana + Katakana
        | 0x3400..=0x4DBF // CJK Ext A
        | 0x4E00..=0x9FFF // CJK Unified
        | 0xF900..=0xFAFF // CJK compatibility
        | 0xFF00..=0xFFEF // Full-width forms
        | 0x0E00..=0x0E7F // Thai
    )
}

/// Join wrapped lines into running text.
///
/// - `"inter-" + "national"` → `"international"` (soft hyphen at a wrap)
/// - CJK on either side of the break → no separator
/// - otherwise a single space
pub fn unwrap_lines<'a, I: IntoIterator<Item = &'a str>>(lines: I) -> String {
    let mut out = String::new();
    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if out.is_empty() {
            out.push_str(line);
            continue;
        }
        let prev = out.chars().last().unwrap_or(' ');
        let next = line.chars().next().unwrap_or(' ');
        let before_prev = out.chars().rev().nth(1).unwrap_or(' ');
        if prev == '-' && before_prev.is_alphabetic() && next.is_lowercase() {
            out.pop();
        } else if !(is_cjk(prev) || is_cjk(next)) {
            out.push(' ');
        }
        out.push_str(line);
    }
    out
}

/// Separator used when several blocks are translated in one request.
/// A blank line is preserved by every translator we ship (DeepL,
/// Gemini, Ollama-hosted LLMs) and does not need a special prompt.
pub const BLOCK_SEPARATOR: &str = "\n\n";

/// Join block texts for a single translation request.
pub fn join_blocks(blocks: &[TextBlock]) -> String {
    blocks
        .iter()
        .map(|b| b.text.as_str())
        .collect::<Vec<_>>()
        .join(BLOCK_SEPARATOR)
}

/// Split a translation of [`join_blocks`] output back into per-block
/// strings. Returns `None` when the paragraph count does not match, so
/// the caller can fall back to translating blocks one by one.
pub fn split_translation(translated: &str, expected: usize) -> Option<Vec<String>> {
    if expected == 0 {
        return Some(vec![]);
    }
    if expected == 1 {
        let t = translated.trim();
        return Some(vec![t.to_string()]);
    }
    let parts: Vec<String> = split_paragraphs(translated);
    if parts.len() == expected {
        return Some(parts);
    }
    // Some LLMs single-space the paragraphs. If the line count matches
    // exactly, accept that too.
    let lines: Vec<String> = translated
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() == expected {
        return Some(lines);
    }
    None
}

fn split_paragraphs(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for line in s.lines() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                parts.push(cur.join("\n").trim().to_string());
                cur.clear();
            }
        } else {
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        parts.push(cur.join("\n").trim().to_string());
    }
    parts
}

/// Background / foreground colour estimate for one block.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockColors {
    pub background: [u8; 3],
    pub foreground: [u8; 3],
}

fn luma(p: [u8; 3]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// Estimate the background (border ring median) and text colour
/// (pixels inside the box that contrast most with the background).
///
/// Falls back to white-on-black / black-on-white based on background
/// luminance when the box has no contrasting pixels.
pub fn estimate_colors(img: &RgbaImage, bbox: BBox) -> BlockColors {
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    if iw == 0 || ih == 0 {
        return BlockColors {
            background: [255, 255, 255],
            foreground: [0, 0, 0],
        };
    }
    let pad = 3i64;
    let x0 = (bbox[0].floor() as i64 - pad).clamp(0, iw - 1);
    let y0 = (bbox[1].floor() as i64 - pad).clamp(0, ih - 1);
    let x1 = ((bbox[0] + bbox[2]).ceil() as i64 + pad).clamp(0, iw - 1);
    let y1 = ((bbox[1] + bbox[3]).ceil() as i64 + pad).clamp(0, ih - 1);

    let px = |x: i64, y: i64| {
        let p = img.get_pixel(x as u32, y as u32).0;
        [p[0], p[1], p[2]]
    };

    // Ring: the outer 2 px of the padded box.
    let mut ring: Vec<[u8; 3]> = Vec::new();
    for x in x0..=x1 {
        for d in 0..2 {
            ring.push(px(x, (y0 + d).min(y1)));
            ring.push(px(x, (y1 - d).max(y0)));
        }
    }
    for y in y0..=y1 {
        for d in 0..2 {
            ring.push(px((x0 + d).min(x1), y));
            ring.push(px((x1 - d).max(x0), y));
        }
    }
    let background = median_color(&mut ring);
    let bg_l = luma(background);

    // Inside: collect pixels far from the background luminance.
    let mut far: Vec<([u8; 3], f32)> = Vec::new();
    let step = (((x1 - x0) * (y1 - y0)) as f64 / 40_000.0).sqrt().max(1.0) as usize;
    for y in (y0..=y1).step_by(step) {
        for x in (x0..=x1).step_by(step) {
            let p = px(x, y);
            let d = (luma(p) - bg_l).abs();
            if d > 48.0 {
                far.push((p, d));
            }
        }
    }
    let foreground = if far.len() < 4 {
        if bg_l > 128.0 {
            [0, 0, 0]
        } else {
            [255, 255, 255]
        }
    } else {
        // Average of the most contrasting quarter: anti-aliased edge
        // pixels would otherwise wash the colour out.
        far.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let take = (far.len() / 4).max(1);
        let mut acc = [0u32; 3];
        for (p, _) in &far[..take] {
            for c in 0..3 {
                acc[c] += p[c] as u32;
            }
        }
        [
            (acc[0] / take as u32) as u8,
            (acc[1] / take as u32) as u8,
            (acc[2] / take as u32) as u8,
        ]
    };
    BlockColors {
        background,
        foreground,
    }
}

fn median_color(v: &mut [[u8; 3]]) -> [u8; 3] {
    if v.is_empty() {
        return [255, 255, 255];
    }
    let mut out = [0u8; 3];
    for (c, slot) in out.iter_mut().enumerate() {
        let mut ch: Vec<u8> = v.iter().map(|p| p[c]).collect();
        ch.sort_unstable();
        *slot = ch[ch.len() / 2];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn line(text: &str, x: f32, y: f32, w: f32, h: f32) -> OcrLine {
        OcrLine {
            text: text.into(),
            bbox: [x, y, w, h],
            confidence: 1.0,
        }
    }

    #[test]
    fn wrapped_paragraph_becomes_one_block() {
        let lines = vec![
            line("The quick brown fox", 10.0, 10.0, 200.0, 16.0),
            line("jumps over the lazy", 10.0, 30.0, 190.0, 16.0),
            line("dog.", 10.0, 50.0, 40.0, 16.0),
        ];
        let blocks = group_lines(&lines);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].text,
            "The quick brown fox jumps over the lazy dog."
        );
        assert_eq!(blocks[0].line_count, 3);
        assert_eq!(blocks[0].bbox, [10.0, 10.0, 200.0, 56.0]);
        assert_eq!(blocks[0].line_height, 16.0);
    }

    #[test]
    fn large_gap_splits_paragraphs() {
        let lines = vec![
            line("First paragraph.", 10.0, 10.0, 200.0, 16.0),
            line("Second paragraph.", 10.0, 60.0, 200.0, 16.0),
        ];
        let blocks = group_lines(&lines);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].text, "First paragraph.");
        assert_eq!(blocks[1].text, "Second paragraph.");
    }

    #[test]
    fn heading_is_separate_from_body() {
        let lines = vec![
            line("Big Heading", 10.0, 10.0, 300.0, 40.0),
            line("body text right below", 10.0, 55.0, 280.0, 14.0),
        ];
        assert_eq!(group_lines(&lines).len(), 2);
    }

    #[test]
    fn columns_stay_apart() {
        let lines = vec![
            line("left col one", 10.0, 10.0, 150.0, 16.0),
            line("right col one", 400.0, 10.0, 150.0, 16.0),
            line("left col two", 10.0, 30.0, 150.0, 16.0),
            line("right col two", 400.0, 30.0, 150.0, 16.0),
        ];
        let blocks = group_lines(&lines);
        assert_eq!(blocks.len(), 2);
        let texts: Vec<&str> = blocks.iter().map(|b| b.text.as_str()).collect();
        assert!(texts.contains(&"left col one left col two"));
        assert!(texts.contains(&"right col one right col two"));
    }

    #[test]
    fn lines_without_geometry_fall_back_to_single_block() {
        let lines = vec![line("a", 0.0, 0.0, 0.0, 0.0), line("b", 0.0, 0.0, 0.0, 0.0)];
        let blocks = group_lines(&lines);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].text, "a b");
    }

    #[test]
    fn empty_lines_are_ignored() {
        assert!(group_lines(&[line("   ", 0.0, 0.0, 10.0, 10.0)]).is_empty());
    }

    #[test]
    fn unwrap_dehyphenates_and_respects_cjk() {
        assert_eq!(unwrap_lines(["inter-", "national"]), "international");
        assert_eq!(unwrap_lines(["well-", "Known"]), "well- Known");
        assert_eq!(unwrap_lines(["日本語の", "文章です"]), "日本語の文章です");
        assert_eq!(unwrap_lines(["hello", "world"]), "hello world");
        assert_eq!(unwrap_lines(["  a ", "", " b"]), "a b");
    }

    #[test]
    fn split_translation_paragraphs() {
        assert_eq!(
            split_translation("一つ目\n\n二つ目\n\n三つ目", 3).unwrap(),
            vec!["一つ目", "二つ目", "三つ目"]
        );
        // Single newline separators are accepted when the count matches.
        assert_eq!(
            split_translation("一つ目\n二つ目", 2).unwrap(),
            vec!["一つ目", "二つ目"]
        );
        // Extra whitespace around paragraphs is trimmed.
        assert_eq!(
            split_translation("\n  a \n\n\n b\n", 2).unwrap(),
            vec!["a", "b"]
        );
        assert!(split_translation("merged into one", 2).is_none());
        assert_eq!(
            split_translation("whole\n\nthing", 1).unwrap(),
            vec!["whole\n\nthing"]
        );
    }

    #[test]
    fn join_blocks_uses_blank_line() {
        let b = |t: &str| TextBlock {
            text: t.into(),
            bbox: [0.0; 4],
            line_height: 0.0,
            line_count: 1,
        };
        assert_eq!(join_blocks(&[b("a"), b("b")]), "a\n\nb");
    }

    #[test]
    fn colors_dark_text_on_light_background() {
        let mut img = RgbaImage::from_pixel(100, 40, Rgba([250, 248, 240, 255]));
        for x in 20..80 {
            for y in 15..25 {
                img.put_pixel(x, y, Rgba([20, 30, 40, 255]));
            }
        }
        let c = estimate_colors(&img, [15.0, 10.0, 70.0, 20.0]);
        assert_eq!(c.background, [250, 248, 240]);
        assert_eq!(c.foreground, [20, 30, 40]);
    }

    #[test]
    fn colors_light_text_on_dark_background() {
        let mut img = RgbaImage::from_pixel(100, 40, Rgba([10, 10, 10, 255]));
        for x in 20..80 {
            for y in 15..25 {
                img.put_pixel(x, y, Rgba([230, 230, 230, 255]));
            }
        }
        let c = estimate_colors(&img, [15.0, 10.0, 70.0, 20.0]);
        assert_eq!(c.background, [10, 10, 10]);
        assert_eq!(c.foreground, [230, 230, 230]);
    }

    #[test]
    fn colors_without_contrast_pick_readable_default() {
        let img = RgbaImage::from_pixel(50, 50, Rgba([30, 30, 30, 255]));
        let c = estimate_colors(&img, [10.0, 10.0, 20.0, 20.0]);
        assert_eq!(c.foreground, [255, 255, 255]);
    }

    #[test]
    fn colors_bbox_outside_image_does_not_panic() {
        let img = RgbaImage::from_pixel(10, 10, Rgba([200, 200, 200, 255]));
        let _ = estimate_colors(&img, [50.0, 50.0, 20.0, 20.0]);
        let _ = estimate_colors(&img, [-5.0, -5.0, 3.0, 3.0]);
    }
}
