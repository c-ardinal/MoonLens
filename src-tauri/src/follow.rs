//! Keeping one window in place relative to an anchor rect that moves:
//! the text view next to its lens window ([`crate::lens_text`]), or one
//! of our windows on a window of another app ([`crate::attach`]).
//!
//! [`Follow`] holds the placement rule and decides, step by step, where
//! the follower belongs; [`move_to`] at the bottom puts a window there.

use serde::{Deserialize, Serialize};
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::lens::geometry::{self, LensOuterRect as Rect};

/// Space between the anchor and a docked follower (CSS pixels).
const GAP_CSS: f64 = 4.0;
/// Size of a new follower along the axis its dock leaves free (CSS
/// pixels): the width beside the anchor, the height above or below it.
pub const WIDTH_CSS: f64 = 340.0;
pub const HEIGHT_CSS: f64 = 180.0;

/// Steps to wait for the window system to apply a placement before a
/// differing position counts as the user's doing.
const GRACE_STEPS: u8 = 5;
/// Times a placement that did not arrive is made again before the spot
/// counts as refused by the window system (one may get lost while the
/// window is busy with something else, such as being shown).
const RETRIES: u8 = 2;

/// Where the follower goes relative to the anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dock {
    Top,
    Bottom,
    Left,
    Right,
    /// Wherever the user puts it; the offset to the anchor is kept.
    Free,
    /// Exactly over the anchor (plus the follower's header above it,
    /// see [`Follow::with_header`]). Moved or resized by the user, it
    /// becomes [`Dock::Free`].
    Cover,
    /// Inside the anchor, along its bottom edge.
    Inside,
}

impl Dock {
    fn opposite(self) -> Option<Dock> {
        match self {
            Dock::Top => Some(Dock::Bottom),
            Dock::Bottom => Some(Dock::Top),
            Dock::Left => Some(Dock::Right),
            Dock::Right => Some(Dock::Left),
            Dock::Free | Dock::Cover | Dock::Inside => None,
        }
    }
}

/// Placement of one follower window. All rects are client areas in
/// physical pixels.
#[derive(Debug, Clone)]
pub struct Follow {
    dock: Dock,
    /// Free placement: follower corner minus anchor corner.
    offset: (i32, i32),
    /// Width (beside the anchor, free) and height (above / below,
    /// inside, free).
    size: (u32, u32),
    gap: i32,
    /// Strip of the follower that goes above a covered anchor.
    header: u32,
    /// Anchor rect the follower was last placed for.
    anchor: Option<Rect>,
    /// Where the follower was last put (or found, once accepted).
    placed: Option<Rect>,
    /// The follower was seen at `placed` (a placement that never
    /// arrives was refused by the window system).
    arrived: bool,
    grace: u8,
    /// Placements made again for the current anchor rect.
    retries: u8,
}

impl Follow {
    /// A follower at `dock`, sized for a display scale of `scale`.
    pub fn new(dock: Dock, scale: f64) -> Self {
        let px = |css: f64| (css * scale).round() as u32;
        Self {
            dock,
            // Free placement starts to the right of the anchor; the first
            // placement replaces this with the real anchor width.
            offset: (0, 0),
            size: (px(WIDTH_CSS), px(HEIGHT_CSS)),
            gap: px(GAP_CSS) as i32,
            header: 0,
            anchor: None,
            placed: None,
            arrived: false,
            grace: 0,
            retries: 0,
        }
    }

    /// The follower's top `header` pixels are a header of its own:
    /// covering an anchor, it goes above the anchor where there is room.
    pub fn with_header(mut self, header: u32) -> Self {
        self.header = header;
        self
    }

    /// A follower that is at `at` while its anchor is at `anchor`, and
    /// is to stay there: free placement keeps that offset and size from
    /// the first [`step`](Self::step) on.
    pub fn keeping(mut self, anchor: Rect, at: Rect) -> Self {
        self.adopt(anchor, at);
        self.anchor = Some(anchor);
        self.arrived = true;
        self
    }

    /// A docked follower that is `w` × `h` already and keeps that size
    /// along the axis its dock leaves free (height above, below and
    /// inside the anchor, width beside it).
    pub fn sized(mut self, w: u32, h: u32) -> Self {
        self.size = (w, h);
        self
    }

    /// Change the header strip (the display scale changed); the next
    /// [`step`](Self::step) places the window anew.
    pub fn set_header(&mut self, header: u32) {
        if header != self.header {
            self.header = header;
            self.anchor = None;
        }
    }

    pub fn dock(&self) -> Dock {
        self.dock
    }

    /// Whether the follower was last placed for another anchor rect
    /// than `anchor` (so a [`step`](Self::step) would move it).
    pub fn anchor_moved(&self, anchor: Rect) -> bool {
        self.anchor != Some(anchor)
    }

    /// Change the dock; the next [`step`](Self::step) moves the window.
    /// Switching to free placement keeps the window where it is.
    pub fn set_dock(&mut self, dock: Dock) {
        if dock == self.dock {
            return;
        }
        if dock == Dock::Free {
            if let (Some(anchor), Some(at)) = (self.anchor, self.placed) {
                self.offset = (at.x - anchor.x, at.y - anchor.y);
                self.size = (at.w, at.h);
            }
        }
        self.dock = dock;
        self.anchor = None;
    }

    /// Rect for the follower of an anchor at `anchor`, kept on
    /// `monitor`. A follower docked outside without room on its side
    /// goes to the opposite side when there is room.
    pub fn target(&self, anchor: Rect, monitor: Option<Rect>) -> Rect {
        let (w, h) = self.size;
        let gap = self.gap;
        match self.dock {
            // On the anchor, wherever that is: no fitting to the monitor.
            Dock::Cover => {
                let top = monitor.map_or(i32::MIN, |m| m.y);
                return geometry::window_for_region(anchor, self.header, top);
            }
            Dock::Inside => {
                let room = |len: u32| len.saturating_sub(2 * gap as u32).max(1);
                let h = h.min(room(anchor.h));
                return Rect {
                    x: anchor.x + gap.min(anchor.w as i32 / 2),
                    y: anchor.y + anchor.h as i32 - gap.min(anchor.h as i32 / 2) - h as i32,
                    w: room(anchor.w),
                    h,
                };
            }
            _ => {}
        }
        let at = |dock: Dock| match dock {
            Dock::Top => Rect {
                x: anchor.x,
                y: anchor.y - gap - h as i32,
                w: anchor.w,
                h,
            },
            Dock::Bottom => Rect {
                x: anchor.x,
                y: anchor.y + anchor.h as i32 + gap,
                w: anchor.w,
                h,
            },
            Dock::Left => Rect {
                x: anchor.x - gap - w as i32,
                y: anchor.y,
                w,
                h: anchor.h,
            },
            Dock::Right => Rect {
                x: anchor.x + anchor.w as i32 + gap,
                y: anchor.y,
                w,
                h: anchor.h,
            },
            Dock::Free | Dock::Cover | Dock::Inside => Rect {
                x: anchor.x + self.offset.0,
                y: anchor.y + self.offset.1,
                w,
                h,
            },
        };
        let want = at(self.dock);
        let Some(m) = monitor else {
            return want;
        };
        let fits = |r: Rect| {
            r.x >= m.x
                && r.y >= m.y
                && r.x + r.w as i32 <= m.x + m.w as i32
                && r.y + r.h as i32 <= m.y + m.h as i32
        };
        let chosen = match self.dock.opposite().map(at) {
            Some(other) if !fits(want) && fits(other) => other,
            _ => want,
        };
        let (w, h) = (chosen.w.min(m.w), chosen.h.min(m.h));
        Rect {
            x: chosen.x.min(m.x + m.w as i32 - w as i32).max(m.x),
            y: chosen.y.min(m.y + m.h as i32 - h as i32).max(m.y),
            w,
            h,
        }
    }

    /// One step: the anchor is at `anchor`, the follower at `actual`.
    /// Returns the rect to move the follower to, if any.
    ///
    /// The anchor moved: the follower goes along. The follower differs
    /// from where it was put while the anchor held still: the user moved
    /// or resized it. Free placement adopts that as the new offset, a
    /// dock as the new size along its free axis, and a follower that
    /// covered its anchor is placed freely from then on (see
    /// [`dock`](Self::dock)).
    pub fn step(&mut self, anchor: Rect, actual: Rect, monitor: Option<Rect>) -> Option<Rect> {
        if self.anchor != Some(anchor) {
            if self.anchor.is_none() && self.placed.is_none() {
                self.offset = (anchor.w as i32 + self.gap, 0);
            }
            self.anchor = Some(anchor);
            self.retries = 0;
            return self.put(self.target(anchor, monitor), actual);
        }
        if self.placed == Some(actual) {
            self.arrived = true;
            self.grace = 0;
            self.retries = 0;
            return None;
        }
        if self.grace > 0 {
            // Our own placement may not have arrived yet.
            self.grace -= 1;
            return None;
        }
        match self.dock {
            Dock::Free => {
                self.adopt(anchor, actual);
                None
            }
            Dock::Cover => {
                if !self.arrived {
                    // Never seen on the anchor. Not the user.
                    return self.settle(anchor, actual, monitor);
                }
                self.dock = Dock::Free;
                self.adopt(anchor, actual);
                None
            }
            Dock::Top | Dock::Bottom | Dock::Inside => {
                self.size.1 = actual.h;
                self.settle(anchor, actual, monitor)
            }
            Dock::Left | Dock::Right => {
                self.size.0 = actual.w;
                self.settle(anchor, actual, monitor)
            }
        }
    }

    /// Take the follower's place as the one to keep.
    fn adopt(&mut self, anchor: Rect, actual: Rect) {
        self.offset = (actual.x - anchor.x, actual.y - anchor.y);
        self.size = (actual.w, actual.h);
        self.placed = Some(actual);
    }

    /// Docked and found elsewhere: put it back, unless that was tried
    /// [`RETRIES`] times over and the window system keeps it where it is
    /// (a minimum size, say).
    fn settle(&mut self, anchor: Rect, actual: Rect, monitor: Option<Rect>) -> Option<Rect> {
        let target = self.target(anchor, monitor);
        if self.placed == Some(target) && !self.arrived {
            if self.retries < RETRIES {
                self.retries += 1;
                return self.put(target, actual);
            }
            self.placed = Some(actual);
            self.arrived = true;
            return None;
        }
        self.put(target, actual)
    }

    fn put(&mut self, target: Rect, actual: Rect) -> Option<Rect> {
        self.placed = Some(target);
        self.arrived = target == actual;
        if self.arrived {
            self.grace = 0;
            return None;
        }
        self.grace = GRACE_STEPS;
        Some(target)
    }
}

/// The monitor `rect` is on (by its centre; else the nearest one).
pub fn monitor_of(win: &WebviewWindow, rect: Rect) -> Option<Rect> {
    let (cx, cy) = (rect.x + rect.w as i32 / 2, rect.y + rect.h as i32 / 2);
    let monitors = win.available_monitors().ok()?;
    monitors
        .iter()
        .map(|m| Rect {
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
        })
        .min_by_key(|m| {
            let dx = (m.x - cx).max(cx - (m.x + m.w as i32 - 1)).max(0) as i64;
            let dy = (m.y - cy).max(cy - (m.y + m.h as i32 - 1)).max(0) as i64;
            dx * dx + dy * dy
        })
}

/// Move and size `win` so that its client area is `target`. `actual` is
/// its client area now.
///
/// Position and size go to the window system as one change where it
/// takes them that way: set one after the other, a follower shows up for
/// a frame at the new size in the old place.
pub fn move_to(win: &WebviewWindow, actual: Rect, target: Rect) {
    // The window calls take the outer frame; the rects here are client
    // areas (a frameless window on Windows 11 has an invisible border).
    let (dx, dy) = match (win.inner_position(), win.outer_position()) {
        (Ok(inner), Ok(outer)) => (inner.x - outer.x, inner.y - outer.y),
        _ => (0, 0),
    };
    let (x, y) = (target.x - dx, target.y - dy);
    let resize = (actual.w, actual.h) != (target.w, target.h);
    #[cfg(windows)]
    if let (Ok(hwnd), Ok(inner), Ok(outer)) = (win.hwnd(), win.inner_size(), win.outer_size()) {
        let w = target.w + outer.width.saturating_sub(inner.width);
        let h = target.h + outer.height.saturating_sub(inner.height);
        if os::set_window_pos(hwnd.0, x, y, w as i32, h as i32, resize) {
            return;
        }
    }
    if resize {
        let _ = win.set_size(PhysicalSize::new(target.w, target.h));
    }
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// Show `win` again without making it the active window: the app the
/// user is in keeps the keyboard.
pub fn show_inactive(win: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(hwnd) = win.hwnd() {
        os::show_inactive(hwnd.0);
        return;
    }
    let _ = win.show();
}

#[cfg(windows)]
mod os {
    use std::ffi::c_void;

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowPos(
            hwnd: *mut c_void,
            insert_after: *mut c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(hwnd: *mut c_void, command: i32) -> i32;
    }
    const SW_SHOWNA: i32 = 8;

    pub fn show_inactive(hwnd: *mut c_void) {
        // SAFETY: as for `set_window_pos`.
        unsafe { ShowWindow(hwnd, SW_SHOWNA) };
    }

    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;

    /// Outer frame of `hwnd` to (`x`, `y`) and, with `resize`, to
    /// `w` x `h`, in one call.
    pub fn set_window_pos(hwnd: *mut c_void, x: i32, y: i32, w: i32, h: i32, resize: bool) -> bool {
        let mut flags = SWP_NOZORDER | SWP_NOACTIVATE;
        if !resize {
            flags |= SWP_NOSIZE;
        }
        // SAFETY: `hwnd` is a window of this process, alive for the call
        // (the caller holds its `WebviewWindow`); no pointers are passed.
        unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), x, y, w, h, flags) != 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENS: Rect = Rect {
        x: 500,
        y: 400,
        w: 460,
        h: 320,
    };
    const MONITOR: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    const ELSEWHERE: Rect = Rect {
        x: 10,
        y: 10,
        w: 340,
        h: 180,
    };

    fn shifted(r: Rect, dx: i32, dy: i32) -> Rect {
        Rect {
            x: r.x + dx,
            y: r.y + dy,
            ..r
        }
    }

    /// Place a new companion and let the window system apply it.
    fn placed(dock: Dock) -> (Follow, Rect) {
        let mut f = Follow::new(dock, 1.0);
        let at = f
            .step(LENS, ELSEWHERE, Some(MONITOR))
            .expect("first placement");
        assert_eq!(f.step(LENS, at, Some(MONITOR)), None);
        (f, at)
    }

    #[test]
    fn a_follower_kept_in_place_stays_and_goes_along_from_there() {
        let mut f = Follow::new(Dock::Free, 1.0).keeping(LENS, ELSEWHERE);
        assert_eq!(f.step(LENS, ELSEWHERE, Some(MONITOR)), None);
        let moved = shifted(LENS, 30, 20);
        assert_eq!(
            f.step(moved, ELSEWHERE, Some(MONITOR)),
            Some(shifted(ELSEWHERE, 30, 20))
        );
    }

    #[test]
    fn a_docked_follower_keeps_the_size_it_came_with() {
        let mut below = Follow::new(Dock::Bottom, 1.0).sized(500, 77);
        let at = below.step(LENS, ELSEWHERE, Some(MONITOR)).unwrap();
        assert_eq!((at.w, at.h), (LENS.w, 77));
        let mut beside = Follow::new(Dock::Right, 1.0).sized(123, 500);
        let at = beside.step(LENS, ELSEWHERE, Some(MONITOR)).unwrap();
        assert_eq!((at.w, at.h), (123, LENS.h));
    }

    #[test]
    fn docks_touch_their_side_and_span_the_lens() {
        let (_, top) = placed(Dock::Top);
        assert_eq!((top.x, top.w), (LENS.x, LENS.w));
        assert_eq!(top.y + top.h as i32 + 4, LENS.y);

        let (_, bottom) = placed(Dock::Bottom);
        assert_eq!((bottom.x, bottom.w), (LENS.x, LENS.w));
        assert_eq!(bottom.y, LENS.y + LENS.h as i32 + 4);

        let (_, left) = placed(Dock::Left);
        assert_eq!((left.y, left.h), (LENS.y, LENS.h));
        assert_eq!(left.x + left.w as i32 + 4, LENS.x);

        let (_, right) = placed(Dock::Right);
        assert_eq!((right.y, right.h), (LENS.y, LENS.h));
        assert_eq!(right.x, LENS.x + LENS.w as i32 + 4);
    }

    #[test]
    fn a_dock_without_room_goes_to_the_opposite_side() {
        let f = Follow::new(Dock::Top, 1.0);
        let lens = Rect { y: 50, ..LENS };
        let r = f.target(lens, Some(MONITOR));
        assert_eq!(r.y, lens.y + lens.h as i32 + 4);
    }

    #[test]
    fn without_room_on_either_side_it_stays_on_the_monitor() {
        let f = Follow::new(Dock::Left, 1.0);
        let r = f.target(MONITOR, Some(MONITOR));
        assert_eq!((r.x, r.y, r.h), (0, 0, MONITOR.h));
    }

    #[test]
    fn the_companion_follows_the_lens() {
        for dock in [
            Dock::Top,
            Dock::Bottom,
            Dock::Left,
            Dock::Right,
            Dock::Free,
            Dock::Inside,
        ] {
            let (mut f, at) = placed(dock);
            let moved = shifted(LENS, 120, -70);
            assert_eq!(
                f.step(moved, at, Some(MONITOR)),
                Some(shifted(at, 120, -70)),
                "{dock:?}"
            );
        }
    }

    #[test]
    fn a_docked_companion_resizes_with_the_lens() {
        let (mut f, at) = placed(Dock::Bottom);
        let wider = Rect {
            w: 700,
            h: 200,
            ..LENS
        };
        let r = f.step(wider, at, Some(MONITOR)).unwrap();
        assert_eq!((r.w, r.y), (700, LENS.y + 200 + 4));
    }

    #[test]
    fn free_placement_keeps_the_offset_the_user_chose() {
        let (mut f, at) = placed(Dock::Free);
        // Starts to the right of the lens.
        assert_eq!((at.x, at.y), (LENS.x + LENS.w as i32 + 4, LENS.y));
        // The user drags and resizes the companion: nothing is put back.
        let dragged = Rect {
            x: 100,
            y: 700,
            w: 500,
            h: 240,
        };
        assert_eq!(f.step(LENS, dragged, Some(MONITOR)), None);
        // The lens moves: same offset, same size.
        let moved = shifted(LENS, 300, -200);
        assert_eq!(
            f.step(moved, dragged, Some(MONITOR)),
            Some(shifted(dragged, 300, -200))
        );
    }

    #[test]
    fn a_docked_companion_keeps_the_size_the_user_gave_it_and_snaps_back() {
        let (mut f, at) = placed(Dock::Right);
        // Wider by the left edge, and dragged down a little.
        let resized = Rect {
            x: at.x + 30,
            y: at.y + 15,
            w: 420,
            h: at.h,
        };
        let back = f.step(LENS, resized, Some(MONITOR)).unwrap();
        assert_eq!(back, Rect { w: 420, ..at });
    }

    #[test]
    fn a_docked_companion_resized_across_the_lens_spans_it_again() {
        // Right dock, grip dragged straight up: only the height changes,
        // which the dock sets, so the target is the one already placed.
        let (mut f, at) = placed(Dock::Right);
        let shorter = Rect { h: at.h - 80, ..at };
        assert_eq!(f.step(LENS, shorter, Some(MONITOR)), Some(at));
        assert_eq!(f.step(LENS, at, Some(MONITOR)), None);
    }

    #[test]
    fn a_late_window_system_is_not_taken_for_the_user() {
        let (mut f, at) = placed(Dock::Free);
        let moved = shifted(LENS, 50, 0);
        let target = f.step(moved, at, Some(MONITOR)).unwrap();
        // The old position is still reported for a few polls.
        for _ in 0..3 {
            assert_eq!(f.step(moved, at, Some(MONITOR)), None);
        }
        assert_eq!(f.step(moved, target, Some(MONITOR)), None);
        // The offset is unchanged: the next move lands where expected.
        let again = shifted(moved, 0, 40);
        assert_eq!(
            f.step(again, target, Some(MONITOR)),
            Some(shifted(target, 0, 40))
        );
    }

    #[test]
    fn a_window_system_that_refuses_a_spot_is_not_fought() {
        let (mut f, at) = placed(Dock::Top);
        let refused = shifted(at, 0, 9);
        let mut placements = 0;
        for _ in 0..100 {
            if f.step(LENS, refused, Some(MONITOR)).is_some() {
                placements += 1;
            }
        }
        assert!(placements <= 3, "{placements} placements");
    }

    #[test]
    fn switching_to_free_placement_keeps_the_window_where_it_is() {
        let (mut f, at) = placed(Dock::Bottom);
        f.set_dock(Dock::Free);
        assert_eq!(f.step(LENS, at, Some(MONITOR)), None);
        let moved = shifted(LENS, -40, 25);
        assert_eq!(f.step(moved, at, Some(MONITOR)), Some(shifted(at, -40, 25)));
    }

    #[test]
    fn switching_the_dock_moves_the_window() {
        let (mut f, at) = placed(Dock::Bottom);
        f.set_dock(Dock::Left);
        let r = f.step(LENS, at, Some(MONITOR)).unwrap();
        assert_eq!(r.x + r.w as i32 + 4, LENS.x);
        assert_eq!((r.y, r.h), (LENS.y, LENS.h));
    }

    #[test]
    fn a_cover_lies_on_the_anchor_with_its_header_above() {
        let mut f = Follow::new(Dock::Cover, 1.0).with_header(32);
        let at = f.step(LENS, ELSEWHERE, Some(MONITOR)).unwrap();
        assert_eq!(
            at,
            Rect {
                x: LENS.x,
                y: LENS.y - 32,
                w: LENS.w,
                h: LENS.h + 32
            }
        );
        // At the top of the monitor the header overlaps the anchor.
        assert_eq!(f.target(MONITOR, Some(MONITOR)), MONITOR);
        // An anchor hanging off the monitor is covered where it is.
        let off = shifted(LENS, 1800, 900);
        let r = f.target(off, Some(MONITOR));
        assert_eq!(
            (r.x, r.w, r.y + r.h as i32),
            (off.x, off.w, off.y + off.h as i32)
        );
    }

    #[test]
    fn a_new_header_height_places_the_cover_anew() {
        let mut f = Follow::new(Dock::Cover, 1.0).with_header(32);
        let at = f.step(LENS, ELSEWHERE, Some(MONITOR)).unwrap();
        assert_eq!(f.step(LENS, at, Some(MONITOR)), None);
        // Moved to a display at 150 %: the header is 48 pixels there.
        f.set_header(48);
        let r = f.step(LENS, at, Some(MONITOR)).unwrap();
        assert_eq!((r.y, r.h), (LENS.y - 48, LENS.h + 48));
        f.set_header(48);
        assert_eq!(f.step(LENS, r, Some(MONITOR)), None);
    }

    #[test]
    fn a_cover_follows_moves_and_resizes() {
        let (mut f, at) = placed(Dock::Cover);
        let anchor = Rect {
            x: 40,
            y: 90,
            w: 800,
            h: 600,
        };
        assert_eq!(f.step(anchor, at, Some(MONITOR)), Some(anchor));
        assert_eq!(f.dock(), Dock::Cover);
    }

    #[test]
    fn a_cover_the_user_moves_keeps_its_new_offset_and_size() {
        let (mut f, at) = placed(Dock::Cover);
        // The user shrinks the window onto one part of the anchor.
        let part = Rect {
            x: at.x + 100,
            y: at.y + 200,
            w: 200,
            h: 80,
        };
        assert_eq!(f.step(LENS, part, Some(MONITOR)), None);
        assert_eq!(f.dock(), Dock::Free);
        let moved = shifted(LENS, -300, 150);
        assert_eq!(
            f.step(moved, part, Some(MONITOR)),
            Some(shifted(part, -300, 150))
        );
        // Covering again snaps back onto the anchor.
        f.set_dock(Dock::Cover);
        assert_eq!(
            f.step(moved, shifted(part, -300, 150), Some(MONITOR)),
            Some(moved)
        );
    }

    #[test]
    fn a_cover_the_window_system_keeps_elsewhere_stays_a_cover() {
        // A minimum size, say: the placement never arrives.
        let mut f = Follow::new(Dock::Cover, 1.0);
        let tiny = Rect {
            w: 40,
            h: 30,
            ..LENS
        };
        let kept = Rect {
            w: 200,
            h: 90,
            ..LENS
        };
        assert_eq!(f.step(tiny, ELSEWHERE, Some(MONITOR)), Some(tiny));
        let mut placements = 0;
        for _ in 0..100 {
            if f.step(tiny, kept, Some(MONITOR)).is_some() {
                placements += 1;
            }
        }
        assert!(placements <= 2, "{placements} placements");
        assert_eq!(f.dock(), Dock::Cover);
    }

    #[test]
    fn a_placement_that_got_lost_is_made_again() {
        let mut f = Follow::new(Dock::Cover, 1.0);
        let at = f.step(LENS, ELSEWHERE, Some(MONITOR)).unwrap();
        // The window stays where it was: once the wait is over, again.
        let again: Vec<Rect> = (0..10)
            .filter_map(|_| f.step(LENS, ELSEWHERE, Some(MONITOR)))
            .collect();
        assert_eq!(again.first(), Some(&at));
        // This one arrives; the user moving it afterwards is the user.
        assert_eq!(f.step(LENS, at, Some(MONITOR)), None);
        assert_eq!(f.dock(), Dock::Cover);
    }

    #[test]
    fn inside_runs_along_the_bottom_edge_of_the_anchor() {
        let (mut f, at) = placed(Dock::Inside);
        assert_eq!((at.x, at.w), (LENS.x + 4, LENS.w - 8));
        assert_eq!(at.y + at.h as i32 + 4, LENS.y + LENS.h as i32);
        // Taller by the user's hand: kept, and still on the bottom edge.
        let taller = Rect {
            y: at.y - 60,
            h: at.h + 30,
            ..at
        };
        let back = f.step(LENS, taller, Some(MONITOR)).unwrap();
        assert_eq!(back.h, at.h + 30);
        assert_eq!(back.y + back.h as i32 + 4, LENS.y + LENS.h as i32);
        // An anchor lower than the follower: no taller than the anchor.
        let low = Rect { h: 100, ..LENS };
        let r = f.target(low, Some(MONITOR));
        assert!(r.y >= low.y && r.h <= low.h, "{r:?}");
    }

    #[test]
    fn a_step_for_the_same_anchor_is_told_apart() {
        let (f, _) = placed(Dock::Right);
        assert!(!f.anchor_moved(LENS));
        assert!(f.anchor_moved(shifted(LENS, 1, 0)));
    }

    #[test]
    fn dock_names_match_the_renderer() {
        assert_eq!(serde_json::to_string(&Dock::Free).unwrap(), "\"free\"");
        assert_eq!(
            serde_json::from_str::<Dock>("\"bottom\"").unwrap(),
            Dock::Bottom
        );
    }
}
