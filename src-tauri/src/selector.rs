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
    win.set_position(PhysicalPosition::new(monitor.x(), monitor.y()))?;
    win.set_size(PhysicalSize::new(monitor.width(), monitor.height()))?;
    win.show()?;
    win.set_always_on_top(true)?;
    win.set_focus()?;
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<()> {
    if let Some(win) = app.get_webview_window("selector") {
        win.hide()?;
    }
    Ok(())
}
