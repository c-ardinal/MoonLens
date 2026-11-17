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
    /// Horizontal alignment of the source lines, so the translation is
    /// laid out the same way (centred headings stay centred).
    #[serde(default)]
    pub align: Align,
}

/// Horizontal text alignment of a block.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Infer alignment from line rects. Needs at least two lines of clearly
/// different widths; otherwise (single lines, justified paragraphs)
/// every edge lines up and left is the safe answer.
pub fn detect_align(lines: &[BBox]) -> Align {
    if lines.len() < 2 {
        return Align::Left;
    }
    let spread = |f: &dyn Fn(&BBox) -> f32| {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for b in lines {
            let v = f(b);
            lo = lo.min(v);
            hi = hi.max(v);
        }
        hi - lo
    };
    let mut heights: Vec<f32> = lines.iter().map(|b| b[3]).collect();
    let h = median(&mut heights).max(1.0);
    let left = spread(&|b| b[0]);
    let right = spread(&|b| right(b));
    let center = spread(&|b| b[0] + b[2] / 2.0);
    let widths = spread(&|b| b[2]);
    // All lines about the same width: alignment is not observable.
    if widths < h {
        return Align::Left;
    }
    let tol = 0.6 * h;
    if left <= tol {
        Align::Left
    } else if center <= tol && center < right {
        Align::Center
    } else if right <= tol {
        Align::Right
    } else {
        Align::Left
    }
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
    /// Gap between the first two lines, once known.
    spacing: Option<f32>,
    lines: Vec<OcrLine>,
}

/// Group OCR lines into paragraph blocks.
///
/// A line joins the most recent compatible block when
/// - its top is close enough below the block's last line (≤ 0.9 line
///   heights, tightened once the block's own line spacing is known, and
///   when the previous line stopped short of the right edge),
/// - it starts at the block's left edge (or shares the centre),
/// - its height is within 1.2× of the block's last line (headings vs body
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
            align: Align::Left,
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
            // Overlap with the previous *line*, not the block: a wide
            // paragraph above two columns must not swallow the right one.
            let overlap = right(&lb).min(right(&b.last)) - lb[0].max(b.last[0]);
            let h = prev_h.max(lh);
            // CJK text is often set with 1.6–1.8 line height, so allow up
            // to ~0.9 line heights between wrapped lines...
            let mut max_gap = h * 0.9;
            // ...but once a block has a line spacing, a clearly larger gap
            // is a paragraph break,
            if let Some(spacing) = b.spacing {
                max_gap = max_gap.min(spacing + h * 0.3);
            }
            // and a previous line that stops well short of the block's
            // right edge ended its paragraph (wrapped lines fill the width).
            if right(&b.last) < right(&b.bbox) - h * 1.5 {
                max_gap = max_gap.min(h * 0.25);
            }
            // Wrapped lines share the left edge (or the centre, for
            // centred text); a line starting elsewhere is another column.
            let center = |r: &BBox| r[0] + r[2] / 2.0;
            let aligned = (lb[0] - b.bbox[0]).abs() <= h * 1.5
                || (center(&lb) - center(&b.last)).abs() <= h * 1.5;
            // Lines on the same row (gap negative) belong together only
            // when they also overlap horizontally; otherwise they are
            // separate columns / UI labels.
            if gap <= max_gap
                && gap >= -prev_h * 0.5
                && size_ratio <= 1.2
                && overlap > 0.0
                && aligned
            {
                target = Some(i);
                break;
            }
        }
        match target {
            Some(i) => {
                let b = &mut blocks[i];
                if b.spacing.is_none() {
                    b.spacing = Some((lb[1] - bottom(&b.last)).max(0.0));
                }
                b.bbox = union(&b.bbox, &lb);
                b.last = lb;
                b.lines.push(line);
            }
            None => blocks.push(Building {
                bbox: lb,
                last: lb,
                spacing: None,
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
                align: detect_align(&b.lines.iter().map(|l| l.bbox).collect::<Vec<_>>()),
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

/// Typical stroke thickness of the text in `bbox` relative to the line
/// height, used to tell bold from regular text. Pixels are split into
/// ink / background between the two estimated luminances, and the
/// median horizontal ink run is taken: most runs cross vertical stems,
/// whose width is the stroke weight. The ink threshold sits 65 % of the
/// way from background to text colour so anti-aliased edges (which make
/// light-on-dark text look heavier) are mostly excluded. `None` when
/// there is too little ink to judge.
pub fn stroke_ratio(
    img: &RgbaImage,
    bbox: BBox,
    colors: BlockColors,
    line_height: f32,
) -> Option<f32> {
    let (iw, ih) = (img.width() as i64, img.height() as i64);
    let x0 = (bbox[0].floor() as i64).clamp(0, iw);
    let y0 = (bbox[1].floor() as i64).clamp(0, ih);
    let x1 = ((bbox[0] + bbox[2]).ceil() as i64).clamp(0, iw);
    let y1 = ((bbox[1] + bbox[3]).ceil() as i64).clamp(0, ih);
    let bg = luma(colors.background);
    let fg = luma(colors.foreground);
    if line_height < 6.0 || (bg - fg).abs() < 40.0 || x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mid = bg + (fg - bg) * 0.65;
    let dark_ink = fg < bg;
    // Sample at most ~200 rows.
    let step = (((y1 - y0) / 200).max(1)) as usize;
    let mut runs: Vec<u32> = Vec::new();
    for y in (y0..y1).step_by(step) {
        let mut run = 0u32;
        for x in x0..=x1 {
            let ink = x < x1 && {
                let p = img.get_pixel(x as u32, y as u32).0;
                let l = luma([p[0], p[1], p[2]]);
                if dark_ink {
                    l < mid
                } else {
                    l > mid
                }
            };
            if ink {
                run += 1;
            } else if run > 0 {
                runs.push(run);
                run = 0;
            }
        }
    }
    if runs.len() < 20 {
        return None;
    }
    runs.sort_unstable();
    Some(runs[runs.len() / 2] as f32 / line_height)
}

/// Whether the text in a block looks bold, from [`stroke_ratio`].
/// CJK glyphs have thinner strokes relative to their height than Latin
/// ones, so each script gets its own threshold (calibrated on rendered
/// regular / bold samples at 13–32 px, 1x and 2x scale, light and dark
/// backgrounds).
pub fn looks_bold(text: &str, stroke_ratio: Option<f32>) -> bool {
    let Some(r) = stroke_ratio else {
        return false;
    };
    let (mut cjk, mut other) = (0usize, 0usize);
    for c in text.chars().filter(|c| c.is_alphanumeric()) {
        if is_cjk(c) {
            cjk += 1;
        } else {
            other += 1;
        }
    }
    let threshold = if cjk >= other { 0.093 } else { 0.112 };
    r >= threshold
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
    fn paragraph_break_is_detected_from_line_spacing() {
        // Tall boxes (PP-OCR style) make the absolute gap rule too loose;
        // the jump from ~0 px line spacing to 11 px must still split.
        let lines = vec![
            line("first line of para one", 20.0, 63.0, 730.0, 21.0),
            line("second line of para one", 20.0, 83.0, 460.0, 24.0),
            line("para two", 22.0, 118.0, 738.0, 21.0),
        ];
        let blocks = group_lines(&lines);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].text, "para two");
    }

    #[test]
    fn wide_paragraph_does_not_swallow_a_right_column() {
        let lines = vec![
            line("wide paragraph spanning the page", 20.0, 100.0, 740.0, 20.0),
            line("left column", 30.0, 125.0, 340.0, 18.0),
            line("right column", 420.0, 120.0, 340.0, 18.0),
        ];
        let blocks = group_lines(&lines);
        let right = blocks.iter().find(|b| b.text.contains("right")).unwrap();
        assert_eq!(right.text, "right column");
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
    fn alignment_from_line_edges() {
        let h = 20.0;
        // Centred heading: different widths, shared centre.
        let centred = [[100.0, 0.0, 400.0, h], [175.0, 24.0, 250.0, h]];
        assert_eq!(detect_align(&centred), Align::Center);
        // Ragged-right paragraph.
        let left = [
            [10.0, 0.0, 400.0, h],
            [10.0, 24.0, 380.0, h],
            [10.0, 48.0, 120.0, h],
        ];
        assert_eq!(detect_align(&left), Align::Left);
        // Right-aligned (numbers column, RTL-ish captions).
        let right = [[100.0, 0.0, 300.0, h], [250.0, 24.0, 150.0, h]];
        assert_eq!(detect_align(&right), Align::Right);
        // Justified / single line: not observable.
        let justified = [[10.0, 0.0, 400.0, h], [10.0, 24.0, 395.0, h]];
        assert_eq!(detect_align(&justified), Align::Left);
        assert_eq!(detect_align(&[[0.0, 0.0, 10.0, h]]), Align::Left);
    }

    #[test]
    fn group_lines_reports_centred_blocks() {
        let l = |x: f32, y: f32, w: f32| OcrLine {
            text: "x".into(),
            bbox: [x, y, w, 20.0],
            confidence: 1.0,
        };
        let blocks = group_lines(&[l(100.0, 0.0, 400.0), l(175.0, 24.0, 250.0)]);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].align, Align::Center);
    }

    #[test]
    fn stroke_ratio_separates_thin_and_thick_strokes() {
        // White background, black vertical stems 2 px (regular) or 4 px
        // (bold) wide on a 20 px line.
        let stems = |w: u32| {
            let mut img = RgbaImage::from_pixel(200, 20, Rgba([255, 255, 255, 255]));
            for x0 in (10..190).step_by(12) {
                for x in x0..x0 + w {
                    for y in 2..18 {
                        img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                    }
                }
            }
            img
        };
        let colors = BlockColors {
            background: [255, 255, 255],
            foreground: [0, 0, 0],
        };
        let bbox = [0.0, 0.0, 200.0, 20.0];
        let thin = stroke_ratio(&stems(2), bbox, colors, 20.0).unwrap();
        let thick = stroke_ratio(&stems(4), bbox, colors, 20.0).unwrap();
        assert!((thin - 0.1).abs() < 1e-6);
        assert!((thick - 0.2).abs() < 1e-6);
        assert!(!looks_bold("Hello world", Some(0.09)));
        assert!(looks_bold("Hello world", Some(0.13)));
        assert!(!looks_bold("日本語の文章", Some(0.07)));
        assert!(looks_bold("日本語の文章", Some(0.1)));
        assert!(!looks_bold("anything", None));
        // No contrast: undecidable.
        let flat = BlockColors {
            background: [128, 128, 128],
            foreground: [120, 120, 120],
        };
        assert!(stroke_ratio(&stems(2), bbox, flat, 20.0).is_none());
    }

    #[test]
    fn join_blocks_uses_blank_line() {
        let b = |t: &str| TextBlock {
            text: t.into(),
            bbox: [0.0; 4],
            line_height: 0.0,
            line_count: 1,
            align: Align::Left,
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
