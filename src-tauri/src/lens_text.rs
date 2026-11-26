//! Text view of a lens window shown outside of it: a companion window
//! docked to one side of the lens, or placed freely and kept at the
//! same offset. The companion follows the lens as it moves or resizes.
//!
//! [`Follow`] holds the placement rule and decides, poll by poll, where
//! the companion belongs; the window calls live at the bottom.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::lens::{self, geometry::LensOuterRect as Rect, LensRegistryHandle};

/// Space between the lens and a docked companion (CSS pixels).
const GAP_CSS: f64 = 4.0;
/// Size of a new companion along the axis its dock leaves free (CSS
/// pixels): the width beside the lens, the height above or below it.
const WIDTH_CSS: f64 = 340.0;
const HEIGHT_CSS: f64 = 180.0;
const MIN_WIDTH_CSS: f64 = 200.0;
const MIN_HEIGHT_CSS: f64 = 90.0;

/// Polls to wait for the window system to apply a placement before a
/// differing position counts as the user's doing.
const GRACE_POLLS: u8 = 5;

/// Where the companion goes relative to the lens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dock {
    Top,
    Bottom,
    Left,
    Right,
    /// Wherever the user puts it; the offset to the lens is kept.
    Free,
}

impl Dock {
    fn opposite(self) -> Option<Dock> {
        match self {
            Dock::Top => Some(Dock::Bottom),
            Dock::Bottom => Some(Dock::Top),
            Dock::Left => Some(Dock::Right),
            Dock::Right => Some(Dock::Left),
            Dock::Free => None,
        }
    }
}

/// Placement of one companion window. All rects are client areas in
/// physical pixels.
#[derive(Debug, Clone)]
pub struct Follow {
    dock: Dock,
    /// Free placement: companion corner minus lens corner.
    offset: (i32, i32),
    /// Width (beside the lens, free) and height (above / below, free).
    size: (u32, u32),
    gap: i32,
    /// Lens rect the companion was last placed for.
    lens: Option<Rect>,
    /// Where the companion was last put (or found, once accepted).
    placed: Option<Rect>,
    /// The companion was seen at `placed` (a placement that never
    /// arrives was refused by the window system).
    arrived: bool,
    grace: u8,
}

impl Follow {
    /// A companion at `dock`, sized for a display scale of `scale`.
    pub fn new(dock: Dock, scale: f64) -> Self {
        let px = |css: f64| (css * scale).round() as u32;
        Self {
            dock,
            // Free placement starts to the right of the lens; the first
            // placement replaces this with the real lens width.
            offset: (0, 0),
            size: (px(WIDTH_CSS), px(HEIGHT_CSS)),
            gap: px(GAP_CSS) as i32,
            lens: None,
            placed: None,
            arrived: false,
            grace: 0,
        }
    }

    pub fn dock(&self) -> Dock {
        self.dock
    }

    /// Change the dock; the next [`step`](Self::step) moves the window.
    /// Switching to free placement keeps the window where it is.
    pub fn set_dock(&mut self, dock: Dock) {
        if dock == self.dock {
            return;
        }
        if dock == Dock::Free {
            if let (Some(lens), Some(at)) = (self.lens, self.placed) {
                self.offset = (at.x - lens.x, at.y - lens.y);
                self.size = (at.w, at.h);
            }
        }
        self.dock = dock;
        self.lens = None;
    }

    /// Rect for the companion of a lens at `lens`, kept on `monitor`. A
    /// docked companion without room on its side goes to the opposite
    /// side when there is room.
    pub fn target(&self, lens: Rect, monitor: Option<Rect>) -> Rect {
        let (w, h) = self.size;
        let at = |dock: Dock| match dock {
            Dock::Top => Rect {
                x: lens.x,
                y: lens.y - self.gap - h as i32,
                w: lens.w,
                h,
            },
            Dock::Bottom => Rect {
                x: lens.x,
                y: lens.y + lens.h as i32 + self.gap,
                w: lens.w,
                h,
            },
            Dock::Left => Rect {
                x: lens.x - self.gap - w as i32,
                y: lens.y,
                w,
                h: lens.h,
            },
            Dock::Right => Rect {
                x: lens.x + lens.w as i32 + self.gap,
                y: lens.y,
                w,
                h: lens.h,
            },
            Dock::Free => Rect {
                x: lens.x + self.offset.0,
                y: lens.y + self.offset.1,
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

    /// One poll: the lens is at `lens`, the companion at `actual`.
    /// Returns the rect to move the companion to, if any.
    ///
    /// The lens moved: the companion goes along. The companion differs
    /// from where it was put while the lens held still: the user moved
    /// or resized it, which free placement adopts as the new offset and
    /// a dock as the new size along its free axis.
    pub fn step(&mut self, lens: Rect, actual: Rect, monitor: Option<Rect>) -> Option<Rect> {
        if self.lens != Some(lens) {
            if self.lens.is_none() && self.placed.is_none() {
                self.offset = (lens.w as i32 + self.gap, 0);
            }
            self.lens = Some(lens);
            return self.put(self.target(lens, monitor), actual);
        }
        if self.placed == Some(actual) {
            self.arrived = true;
            self.grace = 0;
            return None;
        }
        if self.grace > 0 {
            // Our own placement may not have arrived yet.
            self.grace -= 1;
            return None;
        }
        match self.dock {
            Dock::Free => {
                self.offset = (actual.x - lens.x, actual.y - lens.y);
                self.size = (actual.w, actual.h);
                self.placed = Some(actual);
                None
            }
            Dock::Top | Dock::Bottom => {
                self.size.1 = actual.h;
                self.settle(lens, actual, monitor)
            }
            Dock::Left | Dock::Right => {
                self.size.0 = actual.w;
                self.settle(lens, actual, monitor)
            }
        }
    }

    /// Docked and found elsewhere: put it back, unless that was tried
    /// and the window system keeps it where it is.
    fn settle(&mut self, lens: Rect, actual: Rect, monitor: Option<Rect>) -> Option<Rect> {
        let target = self.target(lens, monitor);
        if self.placed == Some(target) && !self.arrived {
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
        self.grace = GRACE_POLLS;
        Some(target)
    }
}

/// Label of the companion window of lens `id` (covered by the `lens-*`
/// capability).
pub fn label(id: &str) -> String {
    format!("{id}-text")
}

fn monitor_rect(win: &WebviewWindow) -> Option<Rect> {
    let m = win.current_monitor().ok()??;
    Some(Rect {
        x: m.position().x,
        y: m.position().y,
        w: m.size().width,
        h: m.size().height,
    })
}

/// Show the text view of lens `id` outside of it at `dock`, or close
/// the companion (`None`). Creates the window: call on the main thread.
pub fn set_dock(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    id: &str,
    dock: Option<Dock>,
) -> Result<()> {
    let label = label(id);
    let Some(dock) = dock else {
        registry.set_text(id, None);
        if let Some(win) = app.get_webview_window(&label) {
            win.close().ok();
        }
        return Ok(());
    };
    let lens_win = app
        .get_webview_window(id)
        .ok_or_else(|| anyhow!("lens window {id} not found"))?;
    let scale = lens_win.scale_factor().unwrap_or(1.0);
    let known = registry.with_text(id, |f| f.set_dock(dock)).is_some();
    if !known {
        registry.set_text(id, Some(Follow::new(dock, scale)));
    }
    if app.get_webview_window(&label).is_none() {
        let url = format!("/lens/text/?id={id}");
        let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
            .title("MoonLens テキスト")
            .inner_size(WIDTH_CSS, HEIGHT_CSS)
            .min_inner_size(MIN_WIDTH_CSS, MIN_HEIGHT_CSS)
            .decorations(false)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(true)
            // Like the lens: no window reads its own translations back.
            .content_protected(lens::EXCLUDED_FROM_CAPTURE)
            .visible(false)
            .build()
            .context("create lens text window")?;
        follow(app, registry, id, &lens_win);
        win.show().ok();
        let _ = win.set_always_on_top(true);
    } else {
        follow(app, registry, id, &lens_win);
    }
    Ok(())
}

/// Keep the companion of lens `id` in place (one poll; see
/// [`Follow::step`]). Nothing to do without a companion.
pub fn follow(app: &AppHandle, registry: &LensRegistryHandle, id: &str, lens_win: &WebviewWindow) {
    let Some(win) = app.get_webview_window(&label(id)) else {
        return;
    };
    let (Ok(lens_rect), Ok(actual)) = (lens::inner_rect(lens_win), lens::inner_rect(&win)) else {
        return;
    };
    let monitor = monitor_rect(lens_win);
    let Some(Some(target)) = registry.with_text(id, |f| f.step(lens_rect, actual, monitor)) else {
        return;
    };
    let _ = win.set_size(PhysicalSize::new(target.w, target.h));
    // `set_position` takes the outer frame; the rects here are client
    // areas (a frameless window on Windows 11 has an invisible border).
    let (dx, dy) = match (win.inner_position(), win.outer_position()) {
        (Ok(inner), Ok(outer)) => (inner.x - outer.x, inner.y - outer.y),
        _ => (0, 0),
    };
    let _ = win.set_position(PhysicalPosition::new(target.x - dx, target.y - dy));
}

/// Close the companion of lens `id`, if any (the lens is closing).
pub fn close(app: &AppHandle, id: &str) {
    if let Some(win) = app.get_webview_window(&label(id)) {
        win.close().ok();
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
        for dock in [Dock::Top, Dock::Bottom, Dock::Left, Dock::Right, Dock::Free] {
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
        assert!(placements <= 1, "{placements} placements");
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
    fn dock_names_match_the_renderer() {
        assert_eq!(serde_json::to_string(&Dock::Free).unwrap(), "\"free\"");
        assert_eq!(
            serde_json::from_str::<Dock>("\"bottom\"").unwrap(),
            Dock::Bottom
        );
    }
}
