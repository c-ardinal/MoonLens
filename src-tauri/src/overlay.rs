//! Overlay window control and event bus toward the frontend.
//!
//! The overlay is a separate transparent Tauri window (`overlay` label).
//! Rust sends it updates via the app-wide event system; the Svelte route at
//! `/overlay/` listens on `moonlens://overlay` and re-renders.

use anyhow::Result;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::pipeline::TaskResult;
/// One shape for every message the overlay needs to handle. Extending the
/// overlay UI = add a variant + handle it in the Svelte page.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OverlayEvent {
    Processing {
        message: String,
    },
    /// Streaming chunk delta for in-place updates. Emitted by the
    /// Ollama streaming path so the overlay can render tokens as they
    /// arrive rather than waiting for the full response.
    Chunk {
        delta: String,
        done: bool,
    },
    Result {
        result: TaskResult,
        source_text: Option<String>,
        /// Content classification for the captured region. The UI uses
        /// this to adapt the action ribbon (QR → open URL first, photo
        /// → reverse image search first, etc).
        intent: Option<crate::providers::Intent>,
        /// The monitor-local rect that produced the capture. Required
        /// by the "Pin" action so the overlay can ask the region
        /// watcher to monitor the exact same area.
        source_rect: Option<crate::pipeline::CaptureFlowRect>,
    },
    Error {
        message: String,
    },
    Hidden,
}

pub fn emit(app: &AppHandle, event: OverlayEvent) -> Result<()> {
    app.emit("moonlens://overlay", event)?;
    Ok(())
}

pub fn show_near(app: &AppHandle, x: i32, y: i32, w: Option<u32>) -> Result<()> {
    let win = overlay(app)?;
    if let Some(w) = w {
        let _ = win.set_size(PhysicalSize::new(w, 320));
    }
    win.set_position(PhysicalPosition::new(x, y))?;
    win.show()?;
    Ok(())
}

pub fn show_centered(app: &AppHandle) -> Result<()> {
    let win = overlay(app)?;
    win.center()?;
    win.show()?;
    Ok(())
}

/// Show the overlay next to a watched (pinned) region without covering
/// it: below when there is room, else above, else to the right. Covering
/// the region would put the overlay into its own captures and make the
/// watcher retranslate forever. No-op when the overlay is already shown,
/// so a user who moved it keeps their position.
pub fn show_beside(app: &AppHandle, rect: &crate::capture::CaptureRect) -> Result<()> {
    let win = overlay(app)?;
    if win.is_visible().unwrap_or(false) {
        return Ok(());
    }
    let monitors = xcap::Monitor::all()?;
    let m = monitors
        .get(rect.monitor_index)
        .ok_or_else(|| anyhow::anyhow!("monitor {} missing", rect.monitor_index))?;
    let (mx, my, mw, mh) = (m.x()?, m.y()?, m.width()? as i32, m.height()? as i32);
    let size = win.outer_size()?;
    let (w, h) = (size.width as i32, size.height as i32);
    let pos = beside(
        (rect.x, rect.y, rect.w as i32, rect.h as i32),
        (w, h),
        (mw, mh),
    );
    win.set_position(PhysicalPosition::new(mx + pos.0, my + pos.1))?;
    win.show()?;
    Ok(())
}

/// Monitor-local position for a `win`-sized box next to `r` on a
/// `screen`-sized monitor. Pure so it can be tested.
pub fn beside(r: (i32, i32, i32, i32), win: (i32, i32), screen: (i32, i32)) -> (i32, i32) {
    const GAP: i32 = 8;
    let (x, y, w, h) = r;
    let clamp_x = |v: i32| v.clamp(0, (screen.0 - win.0).max(0));
    let clamp_y = |v: i32| v.clamp(0, (screen.1 - win.1).max(0));
    if y + h + GAP + win.1 <= screen.1 {
        (clamp_x(x), y + h + GAP)
    } else if y - GAP - win.1 >= 0 {
        (clamp_x(x), y - GAP - win.1)
    } else if x + w + GAP + win.0 <= screen.0 {
        (x + w + GAP, clamp_y(y))
    } else {
        // No free side: the left edge, as far from the region as possible.
        (0, clamp_y(y))
    }
}

pub fn hide(app: &AppHandle) -> Result<()> {
    let win = overlay(app)?;
    win.hide()?;
    emit(app, OverlayEvent::Hidden).ok();
    Ok(())
}

fn overlay(app: &AppHandle) -> Result<WebviewWindow> {
    app.get_webview_window("overlay")
        .ok_or_else(|| anyhow::anyhow!("overlay window missing"))
}

#[cfg(test)]
mod tests {
    use super::beside;

    #[test]
    fn prefers_below_then_above_then_right() {
        let screen = (1920, 1080);
        let win = (480, 320);
        assert_eq!(beside((100, 100, 400, 200), win, screen), (100, 308));
        assert_eq!(beside((100, 700, 400, 300), win, screen), (100, 372));
        assert_eq!(beside((100, 50, 400, 1000), win, screen), (508, 50));
        // Clamped inside the screen horizontally.
        assert_eq!(beside((1800, 100, 100, 100), win, screen), (1440, 208));
    }
}
