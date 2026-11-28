//! Click-through for subtitle (音声翻訳) windows, the same as for
//! window-translation windows ([`crate::lens`]): in click-through mode
//! the window body lets the mouse through to the app underneath, while
//! the title bar, the resize edges and the grip stay clickable, so the
//! mode can always be switched back from the window itself. The
//! shortcut `hotkeys.toggle_subtitle_operate` switches every subtitle
//! window at once.
//!
//! The renderer owns the mode (button and shortcut event) and reports
//! it with whether its own popups cover the body ([`Mouse`]); a poll per
//! window applies it ([`run`]).

use std::collections::HashMap;
use std::time::Duration;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::lens::geometry;

/// Operate mode on (`true`) / off for a subtitle window (shortcut).
pub const SUBTITLE_OPERATE_EVENT: &str = "moonlens://subtitle-operate";

/// Height of the subtitle title bar in CSS pixels (fixed in
/// `routes/subtitle`, the same as the window-translation header).
const HEADER_CSS: u32 = crate::lens::HEADER_CSS;

/// How often the pointer position is checked.
const POLL_MS: u64 = 40;

/// Mouse handling of one window as its renderer reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mouse {
    /// Operate mode as the user set it (button or shortcut).
    pub operate: bool,
    /// Controls of the renderer cover the body (source menu, attach
    /// menu, suggestion, restart button): the body takes the mouse too.
    pub interactive: bool,
}

impl Mouse {
    /// Whether the whole body takes the mouse.
    pub fn body(self) -> bool {
        self.operate || self.interactive
    }
}

/// Mouse handling of every subtitle window, by window label.
#[derive(Default)]
pub struct Windows {
    entries: Mutex<HashMap<String, Mouse>>,
}

impl Windows {
    pub fn set(&self, label: &str, mouse: Mouse) {
        self.entries.lock().insert(label.to_string(), mouse);
    }

    pub fn forget(&self, label: &str) {
        self.entries.lock().remove(label);
    }

    /// Whether the body of `label` takes the mouse. A window that has
    /// not reported yet takes it: usable rather than unreachable.
    pub fn body(&self, label: &str) -> bool {
        self.entries.lock().get(label).is_none_or(|m| m.body())
    }

    /// Shortcut: when any window is in operate mode, switch all of them
    /// to click-through; otherwise put all of them in operate mode.
    /// Recorded right away, so a second press in quick succession sees
    /// it. Returns the new mode and the windows to tell.
    pub fn toggle(&self) -> (bool, Vec<String>) {
        let mut entries = self.entries.lock();
        let operate = !entries.values().any(|m| m.operate);
        for m in entries.values_mut() {
            m.operate = operate;
        }
        (operate, entries.keys().cloned().collect())
    }
}

/// Shortcut action: switch every subtitle window (see [`Windows::toggle`]).
/// Each window applies the mode itself and reports it back.
pub fn toggle_operate(app: &AppHandle, windows: &Windows) -> bool {
    let (operate, labels) = windows.toggle();
    for label in labels {
        let _ = app.emit_to(
            tauri::EventTarget::WebviewWindow { label },
            SUBTITLE_OPERATE_EVENT,
            operate,
        );
    }
    operate
}

/// Apply the mouse handling of the subtitle window `label` until it is
/// gone. Windows can only ignore the cursor as a whole, so the pointer
/// position decides while the body lets the mouse through.
pub async fn run(app: AppHandle, label: String) {
    let mut applied: Option<bool> = None;
    let mut since_apply = 0u32;
    loop {
        tokio::time::sleep(Duration::from_millis(POLL_MS)).await;
        let Some(win) = app.get_webview_window(&label) else {
            break;
        };
        let windows = &app.state::<crate::app::AppState>().subtitle_mouse;
        let take = windows.body(&label) || pointer_on_controls(&app, &win);
        since_apply += 1;
        // Re-apply now and then: showing the window again can reset it.
        if applied != Some(take) || since_apply >= 25 {
            if win.set_ignore_cursor_events(!take).is_ok() {
                applied = Some(take);
            }
            since_apply = 0;
        }
    }
    app.state::<crate::app::AppState>()
        .subtitle_mouse
        .forget(&label);
}

fn pointer_on_controls(app: &AppHandle, win: &WebviewWindow) -> bool {
    let (Some(p), Ok(inner), Ok(pos), Ok(size)) = (
        crate::lens::pointer_position(app),
        crate::lens::inner_rect(win),
        win.outer_position(),
        win.outer_size(),
    ) else {
        // Unknown: keep the window usable rather than unreachable.
        return true;
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    let outer = geometry::LensOuterRect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    };
    let px = |v: f64| (v * scale).ceil() as u32;
    geometry::hits_controls(
        outer,
        inner,
        px(HEADER_CSS as f64),
        px(crate::lens::EDGE_CSS),
        px(crate::lens::GRIP_CSS),
        p.0,
        p.1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const THROUGH: Mouse = Mouse {
        operate: false,
        interactive: false,
    };
    const OPERATE: Mouse = Mouse {
        operate: true,
        interactive: false,
    };

    #[test]
    fn the_body_lets_the_mouse_through_only_in_click_through_mode_without_popups() {
        let w = Windows::default();
        // Not reported yet: takes the mouse.
        assert!(w.body("subtitle-1"));
        w.set("subtitle-1", THROUGH);
        assert!(!w.body("subtitle-1"));
        w.set("subtitle-1", OPERATE);
        assert!(w.body("subtitle-1"));
        // An open menu over the body takes the mouse in click-through mode.
        w.set(
            "subtitle-1",
            Mouse {
                operate: false,
                interactive: true,
            },
        );
        assert!(w.body("subtitle-1"));
        w.forget("subtitle-1");
        assert!(w.body("subtitle-1"));
    }

    #[test]
    fn the_shortcut_releases_every_window_when_any_is_operated_else_operates_all() {
        let w = Windows::default();
        w.set("subtitle-1", OPERATE);
        w.set("subtitle-2", THROUGH);
        let (operate, mut labels) = w.toggle();
        labels.sort();
        assert!(!operate);
        assert_eq!(labels, ["subtitle-1", "subtitle-2"]);
        assert!(!w.body("subtitle-1") && !w.body("subtitle-2"));
        // A second press right away sees the first one.
        let (operate, _) = w.toggle();
        assert!(operate);
        assert!(w.body("subtitle-1") && w.body("subtitle-2"));
    }

    #[test]
    fn the_shortcut_without_windows_changes_nothing() {
        let w = Windows::default();
        let (_, labels) = w.toggle();
        assert!(labels.is_empty());
    }

    #[test]
    fn the_title_bar_edges_and_grip_stay_clickable_in_click_through_mode() {
        let outer = geometry::LensOuterRect {
            x: 0,
            y: 0,
            w: 400,
            h: 200,
        };
        let hit = |x, y| geometry::hits_controls(outer, outer, HEADER_CSS, 6, 18, x, y);
        assert!(hit(200, 10), "title bar");
        assert!(hit(2, 100), "left edge");
        assert!(hit(395, 195), "grip");
        assert!(!hit(200, 100), "body");
        assert!(!hit(500, 100), "outside");
    }
}
