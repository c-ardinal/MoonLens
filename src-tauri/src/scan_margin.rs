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
    fn shows(&self, rect: [f32; 4]) -> bool {
        let need = MIN_SHARE * rect[2].min(rect[3]).max(0.0);
        self.cut(rect)
            .is_some_and(|[_, _, w, h]| w >= need && h >= need)
    }

    /// `rect` cut to the window, in window pixels.
    fn local(&self, rect: [f32; 4]) -> Option<[f32; 4]> {
        self.cut(rect)
            .map(|[x, y, w, h]| [x - self.x as f32, y - self.y as f32, w, h])
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

    /// The rows the window shows. Rows wholly in the margin belong to
    /// whatever lies next to the window and are not translated.
    pub fn keep_lines(&self, lines: Vec<OcrLine>) -> Vec<OcrLine> {
        lines.into_iter().filter(|l| self.shows(l.bbox)).collect()
    }

    /// Move blocks read in the capture to window pixels and cut their
    /// boxes to the window: the text keeps what was read beyond the
    /// frame, the place it is drawn at does not leave the window.
    pub fn to_window(&self, blocks: &mut Vec<LensBlock>) {
        blocks.retain_mut(|b| {
            let Some(bbox) = self.local(b.bbox) else {
                return false;
            };
            b.bbox = bbox;
            b.lines = b.lines.iter().filter_map(|l| self.local(*l)).collect();
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
        }
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
