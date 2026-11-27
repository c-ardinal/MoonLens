//! Following a scroll in a watched screen region.
//!
//! When the text under a window-translation window is scrolled, most of
//! what the region shows afterwards is what it showed before, a few
//! rows higher or lower. Reading all of it again and hiding every
//! translation meanwhile wastes that: [`Memory`] keeps the picture and
//! the OCR lines of the last reading, and [`Memory::follow`] finds out
//! how far the content moved since.
//!
//! * Lines whose pixels moved along keep their text at the new place,
//!   so their paragraphs (and translations) are not read again. Lines
//!   that left the picture stay known for a while: a paragraph half out
//!   of view keeps its whole text and with it its translation.
//! * Lines that did not move (a fixed header, a side bar) stay.
//! * Only the rows that came into view are read ([`Step::read`]) and
//!   added with [`Memory::add`].
//!
//! Anything that does not look like a plain vertical scroll (the
//! content changed as well, a video plays in the region) gives `None`
//! and the caller reads the whole region as before.
//!
//! Pure: pictures and rects in, decisions out.

use std::collections::HashMap;

use image::{DynamicImage, GrayImage};

use crate::{providers::OcrLine, scan_margin::Visible};

/// `[x, y, w, h]` in picture pixels. `y` may lie outside the picture.
pub type Rect = [f32; 4];

/// Vertical bands a picture is split into for comparing rows. A side
/// bar or a scroll bar that does not move with the text then spoils
/// only its own band.
const BANDS: u32 = 8;

/// Narrowest band (pixels).
const MIN_BAND: u32 = 16;

/// Luma range of a band row below which it shows no detail (empty
/// background) and says nothing about where the content went.
const FLAT: u8 = 12;

/// Rows of the earlier picture with the same content in a band beyond
/// which that content says nothing about the distance (a ruled table).
const TOO_COMMON: usize = 32;

/// Band rows that must agree on a distance for it to count as a scroll.
const MIN_VOTES: u32 = 24;

/// Share of the changed band rows that the scroll must explain.
const MIN_SHARE: f32 = 0.6;

/// Share of the picture height that must stay in view.
const MIN_OVERLAP: f32 = 0.1;

/// Share of the picture height above which reading the rows that came
/// into view is no cheaper than reading everything.
const MAX_READ: f32 = 0.75;

/// Luma difference up to which two pixels are the same.
const SAME: u8 = 8;

/// Luma difference from the background above which a pixel is ink.
const INK: u8 = 24;

/// Share of a rect's ink pixels that may differ for it to still count
/// as the same picture (the edge of a neighbouring box, a caret).
const TOLERATED: f32 = 0.1;

/// What became of a rect of the earlier picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// Its pixels are now [`Step::dy`] further down.
    Moved,
    /// Its pixels are where they were (a part that does not scroll).
    Fixed,
    /// It moved under a part that does not scroll: hidden in part.
    Covered,
    /// Its place shows something else.
    Changed,
}

/// A scroll found by [`Memory::follow`].
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// How far the content moved down (negative: up), in pixels.
    pub dy: i32,
    /// Rows `[top, bottom)` of the new picture that have to be read.
    /// `None`: nothing came into view.
    pub read: Option<(u32, u32)>,
    /// Fate of every rect passed to [`Memory::follow`].
    pub fates: Vec<Fate>,
}

/// A line of the last reading.
#[derive(Debug, Clone)]
struct Known {
    line: OcrLine,
    /// Read at an edge of the picture: maybe only part of the line.
    cut: bool,
    /// The window showed it, now or before a scroll took it out of
    /// view. Lines read in the margin around the window only wait
    /// here until a scroll brings them in.
    shown: bool,
}

/// Picture and lines of the last reading of a region.
#[derive(Default)]
pub struct Memory {
    picture: Option<GrayImage>,
    lines: Vec<Known>,
    /// The window inside the picture (a capture wider than the window,
    /// see [`crate::scan_margin`]). `None`: the picture is the window.
    window: Option<Visible>,
}

impl Memory {
    /// Forget everything (the region moved, a reading failed).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The whole of `image` was read and gave `lines`. `window`: the
    /// part of `image` the translation window shows.
    pub fn remember(&mut self, image: &DynamicImage, lines: &[OcrLine], window: Option<Visible>) {
        let picture = image.to_luma8();
        let height = picture.height();
        self.lines = lines
            .iter()
            .map(|line| Known {
                cut: at_edge(line.bbox, 0, height),
                shown: shows(window, line.bbox),
                line: line.clone(),
            })
            .collect();
        self.picture = Some(picture);
        self.window = window;
    }

    /// The known lines to show, positioned in the last picture: the
    /// ones in the window, and the ones a scroll took out of it (their
    /// `y` lies outside).
    pub fn lines(&self) -> Vec<OcrLine> {
        self.lines
            .iter()
            .filter(|k| k.shown)
            .map(|k| k.line.clone())
            .collect()
    }

    /// Whether `image` is the remembered picture scrolled. If so the
    /// memory moves on to `image`: lines that moved along are shifted,
    /// the others dropped, and the returned [`Step`] says which rows are
    /// still unread. `rects` are places of the earlier picture the
    /// caller wants to know the fate of (shown translations).
    pub fn follow(&mut self, image: &DynamicImage, rects: &[Rect]) -> Option<Step> {
        let old = self.picture.as_ref()?;
        if (image.width(), image.height()) != old.dimensions() {
            return None;
        }
        let new = image.to_luma8();
        let (old_rows, new_rows) = (Rows::of(old), Rows::of(&new));
        let dy = distance(&old_rows, &new_rows)?;
        let height = new.height();
        let fates: Vec<Fate> = self
            .lines
            .iter()
            .map(|k| fate(old, &new, dy, k.line.bbox))
            .collect();
        // Rows of lines that did not move, with the room around them.
        let fixed: Vec<(f32, f32)> = self
            .lines
            .iter()
            .zip(&fates)
            .filter(|(_, f)| **f == Fate::Fixed)
            .map(|(k, _)| k.line.bbox)
            .map(|r| (r[1] - r[3], r[1] + 2.0 * r[3]))
            .collect();
        let stays = |y: i32| fixed.iter().any(|&(a, b)| a <= y as f32 && (y as f32) < b);
        let mut unread = unread_rows(&old_rows, &new_rows, dy, &stays);

        let mut kept = Vec::with_capacity(self.lines.len());
        for (known, fate) in self.lines.iter().zip(fates) {
            let rect = known.line.bbox;
            let moved = shifted(rect, dy);
            match fate {
                Fate::Fixed => kept.push(known.clone()),
                Fate::Covered => {}
                Fate::Moved => {
                    let was_in_view = rows_in_view(rect, height).is_some();
                    let in_view = rows_in_view(moved, height);
                    if known.cut {
                        // The rest of the line may have come into view.
                        if let (Some(rows), Some(span)) = (in_view, unread) {
                            let reach = rect[3].ceil() as u32;
                            if rows.0 <= span.1 + reach && span.0 <= rows.1 + reach {
                                unread = Some((span.0.min(rows.0), span.1.max(rows.1)));
                            }
                        }
                    } else if !was_in_view && in_view.is_some() {
                        // Came back into view unseen: read it again.
                    } else if moved[1] + moved[3] > -(height as f32)
                        && moved[1] < 2.0 * height as f32
                    {
                        kept.push(Known {
                            line: OcrLine {
                                bbox: moved,
                                ..known.line.clone()
                            },
                            cut: false,
                            shown: known.shown || shows(self.window, moved),
                        });
                    }
                }
                Fate::Changed => {
                    // Read again with the rows that came into view, or
                    // this is more than a scroll.
                    let rows = rows_in_view(moved, height)?;
                    let span = unread?;
                    let middle = (rows.0 + rows.1) / 2;
                    if middle < span.0 || middle >= span.1 {
                        return None;
                    }
                }
            }
        }
        let read = match unread {
            Some(span) => {
                let line = usual_line_height(&kept);
                let span = widen_to_gap(span, dy, &new_rows, (line * 3.0) as u32);
                Some(read_rows(span, line, height)?)
            }
            None => None,
        };
        let fates = rects.iter().map(|r| fate(old, &new, dy, *r)).collect();
        self.lines = kept;
        self.picture = Some(new);
        Some(Step { dy, read, fates })
    }

    /// Lines read from rows `[top, bottom)` of the picture after a
    /// [`Step`] (positioned in that crop). A line found again where a
    /// known one is stays as known: the crop may have cut it.
    pub fn add(&mut self, fresh: Vec<OcrLine>, (top, bottom): (u32, u32)) {
        let Some(height) = self.picture.as_ref().map(GrayImage::height) else {
            return;
        };
        let known = self.lines.len();
        for mut line in fresh {
            line.bbox[1] += top as f32;
            if self.lines[..known]
                .iter()
                .any(|k| overlap(k.line.bbox, line.bbox) >= 0.3)
            {
                continue;
            }
            self.lines.push(Known {
                cut: at_edge(line.bbox, top, bottom) || at_edge(line.bbox, 0, height),
                shown: shows(self.window, line.bbox),
                line,
            });
        }
    }
}

/// Whether `window` shows the line at `rect` (see
/// [`Visible::keep_lines`]).
fn shows(window: Option<Visible>, rect: Rect) -> bool {
    window.is_none_or(|w| rect[2] <= 0.0 || rect[3] <= 0.0 || w.shows(rect))
}

/// `rect` moved down by `dy`.
pub fn shifted(rect: Rect, dy: i32) -> Rect {
    [rect[0], rect[1] + dy as f32, rect[2], rect[3]]
}

/// Whether a line box reaches the top or bottom of rows `[top, bottom)`
/// closely enough that the line may continue beyond.
fn at_edge(rect: Rect, top: u32, bottom: u32) -> bool {
    let near = (rect[3] * 0.3).max(3.0);
    rect[1] - (top as f32) < near || bottom as f32 - (rect[1] + rect[3]) < near
}

/// Rows `[top, bottom)` of `rect` inside a picture of `height` rows.
fn rows_in_view(rect: Rect, height: u32) -> Option<(u32, u32)> {
    let top = rect[1].floor().max(0.0) as u32;
    let bottom = ((rect[1] + rect[3]).ceil().max(0.0) as u32).min(height);
    (top < bottom).then_some((top, bottom))
}

/// Intersection of two rects as a share of the smaller one.
fn overlap(a: Rect, b: Rect) -> f32 {
    let w = (a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0]);
    let h = (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]);
    let smaller = (a[2] * a[3]).min(b[2] * b[3]);
    if w <= 0.0 || h <= 0.0 || smaller <= 0.0 {
        return 0.0;
    }
    w * h / smaller
}

/// Median height of the kept lines (20 when there are none).
fn usual_line_height(kept: &[Known]) -> f32 {
    let mut heights: Vec<f32> = kept.iter().map(|k| k.line.bbox[3]).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    heights.get(heights.len() / 2).copied().unwrap_or(20.0)
}

/// The unread `span` begins where the rows stop having moved, which
/// can be in the middle of a line that showed in part before. Start it
/// at the empty row before that line instead (within `reach` rows).
fn widen_to_gap(span: (u32, u32), dy: i32, rows: &Rows, reach: u32) -> (u32, u32) {
    let empty = |y: u32| (0..rows.bands).all(|k| rows.flat(k, y as i32));
    if dy < 0 {
        let least = span.0.saturating_sub(reach);
        match (least..span.0).rev().find(|&y| empty(y)) {
            Some(gap) => (gap + 1, span.1),
            None => span,
        }
    } else {
        let most = (span.1 + reach).min(rows.height as u32);
        match (span.1..most).find(|&y| empty(y)) {
            Some(gap) => (span.0, gap),
            None => span,
        }
    }
}

/// Rows to hand to text recognition for the unread `span`: with room
/// above and below so the first and last line are whole, and tall
/// enough for the detector. `line` is the usual line height. `None`
/// when that is most of the picture.
fn read_rows(span: (u32, u32), line: f32, height: u32) -> Option<(u32, u32)> {
    let pad = (line * 0.75).clamp(8.0, 48.0) as u32;
    let mut top = span.0.saturating_sub(pad);
    let mut bottom = (span.1 + pad).min(height);
    let least = ((line * 3.0) as u32).max(64).min(height);
    if bottom - top < least {
        let more = least - (bottom - top);
        top = top.saturating_sub(more / 2);
        bottom = (top + least).min(height);
        top = bottom.saturating_sub(least);
    }
    ((bottom - top) as f32 <= MAX_READ * height as f32).then_some((top, bottom))
}

/// Content of every row of a picture, band by band.
struct Rows {
    bands: usize,
    height: usize,
    /// Hash of the pixels of band `k` in row `y`, at `y * bands + k`.
    content: Vec<u64>,
    flat: Vec<bool>,
}

impl Rows {
    fn of(img: &GrayImage) -> Self {
        let (w, h) = (img.width() as usize, img.height() as usize);
        let bands = (img.width() / MIN_BAND).clamp(1, BANDS) as usize;
        let raw = img.as_raw();
        let mut content = Vec::with_capacity(bands * h);
        let mut flat = Vec::with_capacity(bands * h);
        for y in 0..h {
            for k in 0..bands {
                let row = &raw[y * w + k * w / bands..y * w + (k + 1) * w / bands];
                let (mut hash, mut low, mut high) = (0xcbf2_9ce4_8422_2325u64, u8::MAX, u8::MIN);
                for &p in row {
                    hash = (hash ^ p as u64).wrapping_mul(0x0100_0000_01b3);
                    low = low.min(p);
                    high = high.max(p);
                }
                content.push(hash);
                flat.push(high - low < FLAT);
            }
        }
        Self {
            bands,
            height: h,
            content,
            flat,
        }
    }

    fn at(&self, k: usize, y: i32) -> Option<u64> {
        (0..self.height as i32)
            .contains(&y)
            .then(|| self.content[y as usize * self.bands + k])
    }

    fn flat(&self, k: usize, y: i32) -> bool {
        self.flat[y as usize * self.bands + k]
    }

    /// Band `k` of row `y` shows detail that was not there in `old`.
    fn new_detail(&self, old: &Rows, k: usize, y: i32) -> bool {
        !self.flat(k, y) && self.at(k, y) != old.at(k, y)
    }
}

/// How far the content of `old` moved down to give `new`: the distance
/// most changed band rows agree on. `None` when they do not agree (no
/// scroll, or one with other changes).
fn distance(old: &Rows, new: &Rows) -> Option<i32> {
    let h = new.height as i32;
    let farthest = h - ((h as f32 * MIN_OVERLAP) as i32).max(16);
    if farthest < 1 {
        return None;
    }
    let mut places: HashMap<(usize, u64), Vec<i32>> = HashMap::new();
    for y in 0..h {
        for k in 0..old.bands {
            if !old.flat(k, y) {
                places
                    .entry((k, old.content[y as usize * old.bands + k]))
                    .or_default()
                    .push(y);
            }
        }
    }
    let mut votes: HashMap<i32, u32> = HashMap::new();
    let mut changed: Vec<i32> = Vec::new();
    for y in 0..h {
        for k in 0..new.bands {
            if !new.new_detail(old, k, y) {
                continue;
            }
            changed.push(y);
            let content = new.content[y as usize * new.bands + k];
            let Some(rows) = places.get(&(k, content)).filter(|r| r.len() <= TOO_COMMON) else {
                continue;
            };
            for from in rows {
                let dy = y - from;
                if dy != 0 && dy.abs() <= farthest {
                    *votes.entry(dy).or_default() += 1;
                }
            }
        }
    }
    let (dy, agreed) = votes
        .into_iter()
        .max_by_key(|&(dy, n)| (n, std::cmp::Reverse(dy.abs()), dy))?;
    let explainable = changed
        .iter()
        .filter(|&&y| (0..h).contains(&(y - dy)))
        .count();
    (agreed >= MIN_VOTES && agreed as f32 >= MIN_SHARE * explainable as f32).then_some(dy)
}

/// Rows `[top, bottom)` of `new` that the scroll brought into view:
/// between the rows that moved and the part at that end of the picture
/// that is as it was (empty background, or rows for which `stays`
/// holds: a fixed footer whose lines are known).
fn unread_rows(old: &Rows, new: &Rows, dy: i32, stays: &dyn Fn(i32) -> bool) -> Option<(u32, u32)> {
    let h = new.height as i32;
    let moved = |y: i32| {
        (0..new.bands).any(|k| new.new_detail(old, k, y) && new.at(k, y) == old.at(k, y - dy))
    };
    let as_before = |y: i32| {
        (0..new.bands).all(|k| new.at(k, y) == old.at(k, y) && (new.flat(k, y) || stays(y)))
    };
    let (top, bottom) = if dy < 0 {
        let top = (0..h).rev().find(|&y| moved(y))? + 1;
        let mut bottom = h;
        while bottom > top && as_before(bottom - 1) {
            bottom -= 1;
        }
        (top, bottom)
    } else {
        let bottom = (0..h).find(|&y| moved(y))?;
        let mut top = 0;
        while top < bottom && as_before(top) {
            top += 1;
        }
        (top, bottom)
    };
    (top < bottom).then_some((top as u32, bottom as u32))
}

/// What became of `rect` of `old` in `new`, whose content moved down by
/// `dy`. A rect outside both pictures counts as moved.
fn fate(old: &GrayImage, new: &GrayImage, dy: i32, rect: Rect) -> Fate {
    let (w, h) = (new.width() as i32, new.height() as i32);
    let x0 = (rect[0].floor() as i32).clamp(0, w);
    let x1 = ((rect[0] + rect[2]).ceil() as i32).clamp(0, w);
    let y0 = rect[1].floor() as i32;
    let y1 = (rect[1] + rect[3]).ceil() as i32;
    let same = |a: u8, b: u8| a.abs_diff(b) <= SAME;
    let at = |img: &GrayImage, x: i32, y: i32| img.get_pixel(x as u32, y as u32)[0];

    // Background of the rect: its most frequent luma in `old`.
    let mut seen = [0u32; 256];
    for y in y0.clamp(0, h)..y1.clamp(0, h) {
        for x in x0..x1 {
            seen[at(old, x, y) as usize] += 1;
        }
    }
    let background = (0..=255u8).max_by_key(|&v| seen[v as usize]).unwrap_or(0);
    let ink = |x: i32, y: i32| at(old, x, y).abs_diff(background) > INK;

    // Still where it was?
    let (mut inked, mut differs) = (0u32, 0u32);
    for y in y0.clamp(0, h)..y1.clamp(0, h) {
        for x in x0..x1 {
            inked += ink(x, y) as u32;
            differs += !same(at(new, x, y), at(old, x, y)) as u32;
        }
    }
    if y0.clamp(0, h) < y1.clamp(0, h) && differs as f32 <= TOLERATED * inked as f32 && inked > 0 {
        return Fate::Fixed;
    }

    // Rows of the new place whose source rows are in `old`.
    let from = (y0 + dy).max(0).max(dy);
    let to = (y1 + dy).min(h).min(h + dy);
    let (mut inked, mut not_moved, mut unexplained) = (0u32, 0u32, 0u32);
    for y in from..to {
        for x in x0..x1 {
            inked += ink(x, y - dy) as u32;
            let p = at(new, x, y);
            if !same(p, at(old, x, y - dy)) {
                not_moved += 1;
                unexplained += !same(p, at(old, x, y)) as u32;
            }
        }
    }
    let allowed = TOLERATED * inked as f32;
    if not_moved as f32 <= allowed {
        Fate::Moved
    } else if unexplained as f32 <= allowed {
        Fate::Covered
    } else {
        Fate::Changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    const W: u32 = 400;
    const H: u32 = 300;
    /// Row pitch and height of the "text" lines of [`page`].
    const PITCH: u32 = 40;
    const LINE: u32 = 20;

    /// A tall white page with a "text" line every [`PITCH`] rows, each
    /// with its own pattern of dark strokes.
    fn page(lines: u32) -> GrayImage {
        let mut img = GrayImage::from_pixel(W, lines * PITCH, Luma([255]));
        for i in 0..lines {
            let mut state = 0x9e37_79b9u32.wrapping_mul(i + 1);
            for x in 20..W - 60 {
                if x % 4 == 0 {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                }
                for y in 0..LINE {
                    let bit = (state >> (8 + y / 4)) & 1;
                    if bit == 1 {
                        img.put_pixel(x, i * PITCH + 10 + y, Luma([20]));
                    }
                }
            }
        }
        img
    }

    /// Rows `[top, top + H)` of the page as a region picture.
    fn view(page: &GrayImage, top: u32) -> DynamicImage {
        DynamicImage::ImageLuma8(image::imageops::crop_imm(page, 0, top, W, H).to_image())
    }

    /// The lines of [`page`] that lie wholly inside the view at `top`.
    fn lines_in(top: u32, count: u32) -> Vec<OcrLine> {
        (0..count)
            .filter_map(|i| {
                let y = (i * PITCH + 10) as i32 - top as i32;
                (y >= 0 && y + LINE as i32 <= H as i32).then(|| OcrLine {
                    text: format!("line {i}"),
                    bbox: [20.0, y as f32, (W - 80) as f32, LINE as f32],
                    confidence: 1.0,
                })
            })
            .collect()
    }

    fn texts(memory: &Memory) -> Vec<String> {
        memory.lines().into_iter().map(|l| l.text).collect()
    }

    #[test]
    fn a_scroll_is_measured_and_only_the_new_rows_are_left_to_read() {
        let page = page(30);
        let mut memory = Memory::default();
        memory.remember(&view(&page, 400), &lines_in(400, 30), None);
        // Lines 10..=16 are in view (rows 410..690). Scroll down 100.
        let step = memory.follow(&view(&page, 500), &[]).unwrap();
        assert_eq!(step.dy, -100);
        let (top, bottom) = step.read.unwrap();
        // Rows 200..300 came into view; the read includes some room.
        assert!((120..=180).contains(&top), "{top}");
        assert_eq!(bottom, H);
        // Line 10 left the view (rows -90..-70) and is still known;
        // line 17 came into view and is not known yet.
        let known = texts(&memory);
        assert!(known.contains(&"line 10".to_string()), "{known:?}");
        assert!(known.contains(&"line 16".to_string()));
        assert!(!known.contains(&"line 17".to_string()));
        let line_16 = memory.lines().into_iter().find(|l| l.text == "line 16");
        assert_eq!(line_16.unwrap().bbox[1], 150.0);
    }

    #[test]
    fn scrolling_up_reads_the_rows_at_the_top() {
        let page = page(30);
        let mut memory = Memory::default();
        memory.remember(&view(&page, 400), &lines_in(400, 30), None);
        let step = memory.follow(&view(&page, 340), &[]).unwrap();
        assert_eq!(step.dy, 60);
        let (top, bottom) = step.read.unwrap();
        assert_eq!(top, 0);
        assert!((60..=120).contains(&bottom), "{bottom}");
    }

    #[test]
    fn new_lines_join_the_known_ones_and_a_cut_line_is_not_read_twice() {
        let page = page(30);
        let mut memory = Memory::default();
        memory.remember(&view(&page, 400), &lines_in(400, 30), None);
        let step = memory.follow(&view(&page, 500), &[]).unwrap();
        let (top, bottom) = step.read.unwrap();
        // Line 17 showed its upper half before (rows 190..200 now): the
        // rows to read start above it, not in its middle.
        assert!((171..=180).contains(&top), "{top}");
        // A reading that finds the lower part of line 16 (known) again,
        // and lines 17 and 18.
        let fresh = vec![
            OcrLine {
                text: "half of line 16".into(),
                bbox: [20.0, 160.0 - top as f32, 320.0, 10.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "line 17".into(),
                bbox: [20.0, 190.0 - top as f32, 320.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "line 18".into(),
                bbox: [20.0, 230.0 - top as f32, 320.0, 20.0],
                confidence: 1.0,
            },
        ];
        memory.add(fresh, (top, bottom));
        let known = texts(&memory);
        assert!(!known.contains(&"half of line 16".to_string()), "{known:?}");
        assert!(known.contains(&"line 17".to_string()));
        let line_18 = memory.lines().into_iter().find(|l| l.text == "line 18");
        assert_eq!(line_18.unwrap().bbox[1], 230.0);
    }

    #[test]
    fn a_line_cut_by_the_edge_is_read_again_once_more_of_it_shows() {
        let page = page(30);
        let mut memory = Memory::default();
        // Line 17 (rows 690..710) is cut by the bottom edge at 700.
        let mut lines = lines_in(400, 30);
        lines.push(OcrLine {
            text: "upper half".into(),
            bbox: [20.0, 290.0, 320.0, 10.0],
            confidence: 1.0,
        });
        memory.remember(&view(&page, 400), &lines, None);
        let step = memory.follow(&view(&page, 440), &[]).unwrap();
        assert!(!texts(&memory).contains(&"upper half".to_string()));
        let (top, _) = step.read.unwrap();
        assert!(top <= 250, "the cut line is inside the rows to read: {top}");
    }

    #[test]
    fn a_fixed_header_stays_and_text_that_slides_under_it_is_dropped() {
        let page = page(30);
        let with_header = |top: u32| {
            let mut img = view(&page, top).to_luma8();
            for y in 0..50 {
                for x in 0..W {
                    let v = if (x / 6 + y / 6) % 2 == 0 { 60 } else { 200 };
                    img.put_pixel(x, y, Luma([v]));
                }
            }
            DynamicImage::ImageLuma8(img)
        };
        let header = OcrLine {
            text: "header".into(),
            bbox: [10.0, 10.0, 300.0, 30.0],
            confidence: 1.0,
        };
        let mut lines: Vec<OcrLine> = lines_in(400, 30)
            .into_iter()
            .filter(|l| l.bbox[1] >= 50.0)
            .collect();
        lines.push(header.clone());
        let mut memory = Memory::default();
        memory.remember(&with_header(400), &lines, None);
        let step = memory
            .follow(
                &with_header(460),
                &[header.bbox, [20.0, 130.0, 320.0, 20.0]],
            )
            .unwrap();
        assert_eq!(step.dy, -60);
        assert_eq!(step.fates, vec![Fate::Fixed, Fate::Moved]);
        let known = memory.lines();
        let header_now = known.iter().find(|l| l.text == "header").unwrap();
        assert_eq!(header_now.bbox, header.bbox);
        // Line 11 (rows 50..70 before) is now under the header.
        assert!(!known.iter().any(|l| l.text == "line 11"), "{known:?}");
        assert!(known.iter().any(|l| l.text == "line 13"));
    }

    #[test]
    fn a_scroll_bar_in_the_region_does_not_hide_the_scroll() {
        let page = page(30);
        let with_bar = |top: u32, thumb: u32| {
            let mut img = view(&page, top).to_luma8();
            for y in 0..H {
                for x in W - 12..W {
                    let v = if (thumb..thumb + 60).contains(&y) {
                        90
                    } else {
                        230
                    };
                    img.put_pixel(x, y, Luma([v]));
                }
            }
            DynamicImage::ImageLuma8(img)
        };
        let mut memory = Memory::default();
        memory.remember(&with_bar(400, 100), &lines_in(400, 30), None);
        let step = memory.follow(&with_bar(480, 120), &[]).unwrap();
        assert_eq!(step.dy, -80);
        assert!(texts(&memory).contains(&"line 14".to_string()));
    }

    #[test]
    fn other_text_at_the_same_place_is_not_a_scroll() {
        let (a, b) = (page(30), page(60));
        let mut memory = Memory::default();
        memory.remember(&view(&a, 400), &lines_in(400, 30), None);
        // Rows 1600.. of another page: other strokes on the same rows.
        assert_eq!(memory.follow(&view(&b, 1600), &[]), None);
        // Nothing was given up: the memory still holds the reading.
        assert_eq!(memory.lines().len(), 7);
    }

    #[test]
    fn a_scroll_with_a_change_in_the_kept_part_is_read_in_full() {
        let page = page(30);
        let mut memory = Memory::default();
        memory.remember(&view(&page, 400), &lines_in(400, 30), None);
        // Line 13 (rows 530..550 of the page) is overwritten while the
        // page scrolls by 40.
        let mut changed = page.clone();
        for y in 530..550 {
            for x in 20..340 {
                changed.put_pixel(x, y, Luma([if x % 3 == 0 { 20 } else { 255 }]));
            }
        }
        assert_eq!(memory.follow(&view(&changed, 440), &[]), None);
    }

    #[test]
    fn an_unchanged_picture_is_no_scroll() {
        let page = page(30);
        let mut memory = Memory::default();
        memory.remember(&view(&page, 400), &lines_in(400, 30), None);
        assert_eq!(memory.follow(&view(&page, 400), &[]), None);
    }

    #[test]
    fn a_line_that_comes_back_into_view_is_read_again() {
        let page = page(30);
        let mut memory = Memory::default();
        memory.remember(&view(&page, 400), &lines_in(400, 30), None);
        memory.follow(&view(&page, 500), &[]).unwrap();
        assert!(texts(&memory).contains(&"line 10".to_string()));
        // Back up: line 10 is in view again, but its rows were not
        // compared with anything, so it is not taken from memory.
        let step = memory.follow(&view(&page, 400), &[]).unwrap();
        assert_eq!(step.dy, 100);
        assert!(!texts(&memory).contains(&"line 10".to_string()));
        assert_eq!(step.read.unwrap().0, 0);
    }
}
