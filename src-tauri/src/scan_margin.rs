//! Reading a little beyond a translation window's frame.
//!
//! A window placed tightly over a text cuts words and rows at its
//! frame; read like that they come out wrong, and so does the
//! translation. The capture is therefore taken `performance.scan_margin_px`
//! wider on every side. Rows that lie outside the window are dropped
//! again, rows the frame cuts are read whole, and every position handed
//! on is clamped to the window, so nothing is ever drawn outside it.

use crate::{capture::rect::GlobalRect, pipeline::LensBlock, providers::OcrLine};

/// Share of a row's thickness (its height; the width of an upright row)
/// that has to lie inside the window, across and along the row, for the
/// row to count as shown in it. Half a character, roughly.
const MIN_SHARE: f32 = 0.5;

/// The window's region inside a capture, in capture pixels. `x` / `y`
/// are negative and the size exceeds the capture where a monitor edge
/// clipped the capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Visible {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// `observed` grown by `margin` pixels on every side.
pub fn widened(observed: GlobalRect, margin: u32) -> GlobalRect {
    let m = margin as i32;
    GlobalRect {
        x: observed.x.saturating_sub(m),
        y: observed.y.saturating_sub(m),
        w: observed.w.saturating_add(margin.saturating_mul(2)),
        h: observed.h.saturating_add(margin.saturating_mul(2)),
    }
}

/// Where `observed` lies in a capture that covers `captured`.
pub fn visible_in(observed: GlobalRect, captured: GlobalRect) -> Visible {
    Visible {
        x: observed.x.saturating_sub(captured.x),
        y: observed.y.saturating_sub(captured.y),
        w: observed.w,
        h: observed.h,
    }
}

/// Whether `rect` has an area, i.e. the engine reported where it is.
fn placed(rect: [f32; 4]) -> bool {
    rect[2] > 0.0 && rect[3] > 0.0
}

impl Visible {
    /// `rect` (`[x, y, w, h]`, capture pixels) cut to the window, in
    /// capture pixels. `None` when nothing of it is inside.
    fn cut(&self, rect: [f32; 4]) -> Option<[f32; 4]> {
        let [x, y, w, h] = rect;
        let x0 = x.max(self.x as f32);
        let y0 = y.max(self.y as f32);
        let x1 = (x + w).min(self.x as f32 + self.w as f32);
        let y1 = (y + h).min(self.y as f32 + self.h as f32);
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
    }

    /// Whether the row at `rect` is one the window shows: enough of it
    /// lies inside, not just a sliver at the frame.
    pub fn shows(&self, rect: [f32; 4]) -> bool {
        let need = MIN_SHARE * rect[2].min(rect[3]).max(0.0);
        self.cut(rect)
            .is_some_and(|[_, _, w, h]| w >= need && h >= need)
    }

    /// `rect` cut to the window, in window pixels.
    fn local(&self, rect: [f32; 4]) -> Option<[f32; 4]> {
        self.cut(rect)
            .map(|[x, y, w, h]| [x - self.x as f32, y - self.y as f32, w, h])
    }

    /// `rect` above or below the window, in window pixels and cut to
    /// the window's sides. `None` when it lies beside the window or
    /// reaches into it.
    fn out_of_view(&self, rect: [f32; 4]) -> Option<[f32; 4]> {
        let [x, y, w, h] = rect;
        let x0 = x.max(self.x as f32);
        let x1 = (x + w).min(self.x as f32 + self.w as f32);
        let outside = y + h <= self.y as f32 || y >= self.y as f32 + self.h as f32;
        (x1 > x0 && outside).then_some([x0 - self.x as f32, y - self.y as f32, x1 - x0, h])
    }

    /// `rect` (window pixels) back in capture pixels.
    pub fn in_capture(&self, rect: [f32; 4]) -> [f32; 4] {
        [
            rect[0] + self.x as f32,
            rect[1] + self.y as f32,
            rect[2],
            rect[3],
        ]
    }

    /// The part of `image` (a capture) the window shows. `None` when
    /// the window lies outside it.
    pub fn crop(&self, image: &image::DynamicImage) -> Option<image::DynamicImage> {
        let [x, y, w, h] = self.cut([0.0, 0.0, image.width() as f32, image.height() as f32])?;
        Some(image.crop_imm(x as u32, y as u32, w as u32, h as u32))
    }

    /// The rows the window shows. Rows wholly in the margin belong to
    /// whatever lies next to the window and are not translated.
    /// A row without a place (an engine that returns text only) cannot
    /// be told apart and is kept.
    pub fn keep_lines(&self, lines: Vec<OcrLine>) -> Vec<OcrLine> {
        lines
            .into_iter()
            .filter(|l| !placed(l.bbox) || self.shows(l.bbox))
            .collect()
    }

    /// Move blocks read in the capture to window pixels and cut their
    /// boxes to the window: the text keeps what was read beyond the
    /// frame, the place it is drawn at does not leave the window.
    /// Rows above or below the window (a scroll took them out of view,
    /// their paragraph is still shown in part) keep their place, so the
    /// translation stays laid out over the whole paragraph.
    pub fn to_window(&self, blocks: &mut Vec<LensBlock>) {
        blocks.retain_mut(|b| {
            if !placed(b.bbox) {
                return true;
            }
            let Some(mut bbox) = self.local(b.bbox) else {
                return false;
            };
            // The erase picture keeps its size: what lies beyond the
            // frame is not drawn by the window.
            if let Some(e) = &mut b.erase {
                e.rect[0] -= self.x;
                e.rect[1] -= self.y;
            }
            // `row_limits` runs parallel to `lines`: a row that takes no
            // place in the window takes its limit along.
            let limited = b.row_limits.len() == b.lines.len();
            let rows = std::mem::take(&mut b.lines);
            let limits = std::mem::take(&mut b.row_limits);
            for (i, row) in rows.into_iter().enumerate() {
                let row = match (self.local(row), self.out_of_view(row)) {
                    (Some(row), _) => row,
                    (None, Some(row)) => {
                        let bottom = (bbox[1] + bbox[3]).max(row[1] + row[3]);
                        bbox[1] = bbox[1].min(row[1]);
                        bbox[3] = bottom - bbox[1];
                        row
                    }
                    (None, None) => continue,
                };
                b.lines.push(row);
                if limited {
                    b.row_limits
                        .push((limits[i] - self.x as f32).clamp(0.0, self.w as f32));
                }
            }
            b.bbox = bbox;
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, w: u32, h: u32) -> GlobalRect {
        GlobalRect { x, y, w, h }
    }

    fn line(text: &str, bbox: [f32; 4]) -> OcrLine {
        OcrLine {
            text: text.into(),
            bbox,
            confidence: 1.0,
        }
    }

    fn block(bbox: [f32; 4], lines: Vec<[f32; 4]>) -> LensBlock {
        LensBlock {
            bbox,
            line_height: 20.0,
            source: "text".into(),
            translation: None,
            colors: None,
            same_language: false,
            from_cache: false,
            align: Default::default(),
            bold: false,
            lines,
            row_limits: vec![],
            erase: None,
        }
    }

    #[test]
    fn the_erase_picture_moves_to_window_pixels_with_its_block() {
        let mut b = block([20.0, 40.0, 100.0, 20.0], vec![]);
        b.erase = Some(crate::erase::ErasePatch {
            rect: [18, 38, 104, 24],
            image: String::new(),
            outline: false,
            foreground: [0, 0, 0],
        });
        let mut blocks = vec![b];
        window().to_window(&mut blocks);
        // Starts 12 px left of the frame; the size stays.
        assert_eq!(blocks[0].erase.as_ref().unwrap().rect, [-12, 8, 104, 24]);
    }

    /// A 200x100 window read with a 30 px margin.
    fn window() -> Visible {
        visible_in(rect(500, 400, 200, 100), rect(470, 370, 260, 160))
    }

    #[test]
    fn the_capture_grows_by_the_margin_on_every_side() {
        assert_eq!(
            widened(rect(500, 400, 200, 100), 30),
            rect(470, 370, 260, 160)
        );
        assert_eq!(
            widened(rect(500, 400, 200, 100), 0),
            rect(500, 400, 200, 100)
        );
        assert_eq!(
            widened(rect(-10, 5, 200, 100), 30),
            rect(-40, -25, 260, 160)
        );
    }

    #[test]
    fn the_window_is_found_inside_the_capture() {
        assert_eq!(
            window(),
            Visible {
                x: 30,
                y: 30,
                w: 200,
                h: 100
            }
        );
    }

    #[test]
    fn a_monitor_edge_that_clips_the_capture_keeps_positions_right() {
        // The margin could not grow past the left monitor edge.
        let v = visible_in(rect(10, 400, 200, 100), rect(0, 370, 240, 160));
        assert_eq!((v.x, v.y), (10, 30));
        // The window itself hangs over the edge: its origin lies
        // outside the capture.
        let v = visible_in(rect(-50, 400, 200, 100), rect(0, 370, 180, 160));
        assert_eq!((v.x, v.y), (-50, 30));
        let mut blocks = vec![block([0.0, 40.0, 100.0, 20.0], vec![])];
        v.to_window(&mut blocks);
        assert_eq!(blocks[0].bbox, [50.0, 10.0, 100.0, 20.0]);
    }

    #[test]
    fn a_row_cut_by_the_frame_is_kept_whole() {
        let v = window();
        // Starts 25 px left of the window and ends 20 px right of it.
        let kept = v.keep_lines(vec![line("whole sentence", [5.0, 60.0, 245.0, 20.0])]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].text, "whole sentence");
    }

    #[test]
    fn rows_in_the_margin_are_dropped() {
        let v = window();
        let kept = v.keep_lines(vec![
            line("above", [40.0, 2.0, 100.0, 20.0]),
            line("left", [0.0, 60.0, 28.0, 20.0]),
            line("inside", [40.0, 60.0, 100.0, 20.0]),
            line("below", [40.0, 135.0, 100.0, 20.0]),
        ]);
        let texts: Vec<_> = kept.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["inside"]);
    }

    #[test]
    fn a_sliver_inside_the_frame_is_not_enough() {
        let v = window();
        // 4 px of a 20 px row reach into the window from above.
        assert!(v
            .keep_lines(vec![line("sliver", [40.0, 14.0, 100.0, 20.0])])
            .is_empty());
        // Half of the row is inside.
        assert_eq!(
            v.keep_lines(vec![line("half", [40.0, 20.0, 100.0, 20.0])])
                .len(),
            1
        );
        // Less than half a character reaches in from the left.
        assert!(v
            .keep_lines(vec![line("edge", [0.0, 60.0, 36.0, 20.0])])
            .is_empty());
    }

    #[test]
    fn an_upright_row_is_judged_by_its_width() {
        let v = window();
        assert_eq!(
            v.keep_lines(vec![line("縦", [20.0, 40.0, 20.0, 80.0])])
                .len(),
            1
        );
        assert!(v
            .keep_lines(vec![line("縦", [14.0, 40.0, 20.0, 80.0])])
            .is_empty());
    }

    #[test]
    fn blocks_are_drawn_inside_the_window_only() {
        let v = window();
        let mut blocks = vec![
            block(
                [5.0, 20.0, 250.0, 60.0],
                vec![
                    [5.0, 20.0, 250.0, 20.0],
                    [5.0, 40.0, 100.0, 20.0],
                    [5.0, 60.0, 20.0, 20.0],
                ],
            ),
            block([0.0, 0.0, 20.0, 20.0], vec![[0.0, 0.0, 20.0, 20.0]]),
        ];
        v.to_window(&mut blocks);
        assert_eq!(blocks.len(), 1);
        let b = &blocks[0];
        assert_eq!(b.bbox, [0.0, 0.0, 200.0, 50.0]);
        // The third row lies left of the window and takes no place.
        assert_eq!(b.lines, [[0.0, 0.0, 200.0, 10.0], [0.0, 10.0, 75.0, 20.0]]);
        for [x, y, w, h] in b.lines.iter().chain([&b.bbox]).copied() {
            assert!(x >= 0.0 && y >= 0.0 && x + w <= 200.0 && y + h <= 100.0);
        }
        assert_eq!(v.in_capture(b.bbox), [30.0, 30.0, 200.0, 50.0]);
    }

    #[test]
    fn rows_a_scroll_took_out_of_view_keep_their_place() {
        let v = window();
        // Two rows above the window, one inside.
        let mut blocks = vec![block(
            [40.0, -10.0, 100.0, 60.0],
            vec![
                [40.0, -10.0, 100.0, 20.0],
                [40.0, 10.0, 100.0, 20.0],
                [40.0, 30.0, 100.0, 20.0],
            ],
        )];
        v.to_window(&mut blocks);
        let b = &blocks[0];
        assert_eq!(
            b.lines,
            [
                [10.0, -40.0, 100.0, 20.0],
                [10.0, -20.0, 100.0, 20.0],
                [10.0, 0.0, 100.0, 20.0]
            ]
        );
        assert_eq!(b.bbox, [10.0, -40.0, 100.0, 60.0]);
    }

    #[test]
    fn the_window_part_of_a_capture_is_cut_out() {
        let capture = image::DynamicImage::new_rgba8(260, 160);
        let inside = window().crop(&capture).unwrap();
        assert_eq!((inside.width(), inside.height()), (200, 100));
        // A window hanging over the capture's left edge.
        let v = visible_in(rect(-50, 400, 200, 100), rect(0, 370, 180, 160));
        let inside = v.crop(&image::DynamicImage::new_rgba8(180, 160)).unwrap();
        assert_eq!((inside.width(), inside.height()), (150, 100));
        let off = Visible {
            x: 300,
            y: 0,
            w: 10,
            h: 10,
        };
        assert!(off.crop(&capture).is_none());
    }

    #[test]
    fn row_limits_follow_their_rows_into_the_window() {
        let v = window();
        let mut b = block(
            [5.0, 20.0, 250.0, 60.0],
            vec![
                [5.0, 20.0, 250.0, 20.0],
                [5.0, 40.0, 100.0, 20.0],
                [5.0, 60.0, 20.0, 20.0],
            ],
        );
        b.row_limits = vec![255.0, 130.0, 25.0];
        let mut blocks = vec![b];
        v.to_window(&mut blocks);
        // The third row lies outside; the first limit ends at the frame.
        assert_eq!(blocks[0].lines.len(), 2);
        assert_eq!(blocks[0].row_limits, [200.0, 100.0]);
    }

    #[test]
    fn text_without_a_place_is_kept() {
        let v = window();
        let kept = v.keep_lines(vec![line("text only", [0.0; 4])]);
        assert_eq!(kept.len(), 1);
        let mut blocks = vec![block([0.0; 4], vec![])];
        v.to_window(&mut blocks);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].bbox, [0.0; 4]);
    }

    #[test]
    fn without_a_margin_nothing_changes() {
        let v = visible_in(rect(500, 400, 200, 100), rect(500, 400, 200, 100));
        let lines = vec![
            line("a", [0.0, 0.0, 200.0, 20.0]),
            line("b", [150.0, 80.0, 50.0, 20.0]),
        ];
        assert_eq!(v.keep_lines(lines.clone()).len(), 2);
        let mut blocks = vec![block([0.0, 0.0, 200.0, 100.0], vec![lines[1].bbox])];
        v.to_window(&mut blocks);
        assert_eq!(blocks[0].bbox, [0.0, 0.0, 200.0, 100.0]);
        assert_eq!(blocks[0].lines, [[150.0, 80.0, 50.0, 20.0]]);
    }
}
