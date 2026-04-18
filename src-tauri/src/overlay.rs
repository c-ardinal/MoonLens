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
