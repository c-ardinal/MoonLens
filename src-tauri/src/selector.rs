//! Region-selection window control.
//!
//! On Linux/macOS we resize the existing `selector` webview to span the
//! primary monitor and let CSS render the dimming + drag rectangle. The
//! webview captures pointer events and invokes `capture_and_process` once
//! the user releases the mouse.

use anyhow::{anyhow, Result};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

pub fn show(app: &AppHandle) -> Result<()> {
    let monitors = xcap::Monitor::all()?;
    let monitor = monitors
        .first()
        .ok_or_else(|| anyhow!("no monitor available"))?;
    let win = app
        .get_webview_window("selector")
        .ok_or_else(|| anyhow!("selector window missing"))?;
    // xcap 0.9's monitor getters can fail if the display server races
    // with our access; propagate the error so the selector never ends
    // up sized for the wrong monitor.
    let (x, y, w, h) = (
        monitor.x()?,
        monitor.y()?,
        monitor.width()?,
        monitor.height()?,
    );
    win.set_position(PhysicalPosition::new(x, y))?;
    win.set_size(PhysicalSize::new(w, h))?;
    win.show()?;
    win.set_always_on_top(true)?;
    win.set_focus()?;

    // On Windows the webview sometimes needs an extra nudge to take
    // keyboard focus after the global-shortcut-triggered `set_focus`
    // — without this, the very first Escape keypress goes to the
    // source app instead of our selector window. `SetForegroundWindow`
    // is allowed here because we just received user input (the
    // hotkey), satisfying Windows' foreground-activation rules.
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
        if let Ok(raw) = win.hwnd() {
            let hwnd = HWND(raw.0 as _);
            // SAFETY: `hwnd` is a live Tauri window handle; the API
            // accepts any valid HWND and returns a BOOL.
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<()> {
    if let Some(win) = app.get_webview_window("selector") {
        win.hide()?;
    }
    Ok(())
}
