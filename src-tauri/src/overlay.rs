//! Overlay window control: show/hide, move near source region, pinned mode.
//!
//! The actual UI is rendered by the `overlay` webview route; this module is
//! the Rust-side puppet-master (position, visibility, event forwarding).

use anyhow::Result;
use tauri::{AppHandle, Manager, WebviewWindow};

pub fn show_near(app: &AppHandle, x: i32, y: i32) -> Result<()> {
    let win = overlay(app)?;
    win.set_position(tauri::PhysicalPosition::new(x, y))?;
    win.show()?;
    Ok(())
}

pub fn hide(app: &AppHandle) -> Result<()> {
    overlay(app)?.hide()?;
    Ok(())
}

fn overlay(app: &AppHandle) -> Result<WebviewWindow> {
    app.get_webview_window("overlay")
        .ok_or_else(|| anyhow::anyhow!("overlay window missing"))
}
