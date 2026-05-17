//! Mini launcher window control.
//!
//! The launcher is a small floating window positioned near the
//! center-top of the primary monitor. It hosts a Spotlight-style
//! quick-translate input fed by the user's clipboard (or typed
//! freehand). Activated via the tray menu's "クリップボードを翻訳"
//! item or the `open_command_palette` global hotkey, which the
//! frontend listens for and routes to `/launcher`.
//!
//! The window itself is declared in `tauri.conf.json` with
//! `decorations: false; transparent: true; alwaysOnTop: true`, mirroring
//! the existing overlay / subtitle setups so the visual treatment stays
//! consistent across floating surfaces.

use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager, PhysicalPosition};

/// Reveal the launcher window. Position is recalculated each call so
/// monitor changes (display unplug, DPI swap) re-center correctly
/// instead of trapping the launcher off-screen.
pub fn show(app: &AppHandle) -> Result<()> {
    let win = app
        .get_webview_window("launcher")
        .ok_or_else(|| anyhow!("launcher window missing"))?;

    // Center horizontally on the primary monitor, biased slightly
    // toward the top third — the same placement Spotlight / Raycast
    // use, which keeps the input out of the eye's resting line.
    if let Some((mx, my, mw, mh)) = primary_monitor_rect() {
        // The window's declared size is 520×220. We re-query the
        // outer size to honor any user resize that survived a
        // previous session, falling back to the declared values.
        let outer = win.outer_size().ok();
        let (ww, _wh) = outer.map_or((520u32, 220u32), |s| (s.width, s.height));
        let x = mx + ((mw as i32 - ww as i32) / 2).max(0);
        let y = my + (mh as i32 / 4).max(0);
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }

    win.show()?;
    win.set_always_on_top(true)?;
    win.set_focus()?;

    // Windows occasionally hands keyboard focus back to the source
    // app right after `set_focus` on a chromeless window. Re-bind
    // foreground status via SetForegroundWindow — allowed here
    // because we just consumed a user input (tray click or global
    // hotkey), which satisfies the Windows foreground-activation
    // rules.
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        if let Ok(raw) = win.hwnd() {
            let hwnd = HWND(raw.0 as _);
            // SAFETY: `hwnd` is a live Tauri window handle.
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }

    Ok(())
}

/// Hide the launcher (Esc / blur). The window is kept alive so the
/// next show() is cheap and any frontend state survives until quit.
pub fn hide(app: &AppHandle) -> Result<()> {
    if let Some(win) = app.get_webview_window("launcher") {
        win.hide()?;
    }
    Ok(())
}

/// Resolve the primary monitor's logical rect `(x, y, width, height)`.
/// Returns `None` if any of xcap's lookups fail — the launcher caller
/// then falls back to Tauri's `center: true` config attribute.
fn primary_monitor_rect() -> Option<(i32, i32, u32, u32)> {
    let monitors = xcap::Monitor::all().ok()?;
    let monitor = monitors.into_iter().next()?;
    let x = monitor.x().ok()?;
    let y = monitor.y().ok()?;
    let w = monitor.width().ok()?;
    let h = monitor.height().ok()?;
    Some((x, y, w, h))
}
