//! Region selection on a frozen frame (Lens-style).
//!
//! When the capture hotkey fires we screenshot the monitor under the
//! cursor *first*, then show the full-screen `selector` webview with that
//! screenshot as its background. The user selects on a still image, so:
//!
//! - what they select is exactly what gets OCR'd (no race with video,
//!   scrolling or the selector window hiding itself before capture);
//! - the selector window can stay open after selection and paint the
//!   translation in place over the frozen image, like Google Lens.
//!
//! The frame lives in a process-wide slot; [`take_crop`] cuts selections
//! out of it and [`close`] releases it.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use image::RgbaImage;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

use crate::capture::{self, rect};
use crate::pipeline::CaptureFlowResult;

/// A full-monitor screenshot taken when the selector opened.
pub struct FrozenFrame {
    pub monitor_index: usize,
    pub image: RgbaImage,
    /// Fast-compressed PNG of `image`, served to the selector webview.
    pub png: Vec<u8>,
}

static FROZEN: Mutex<Option<Arc<FrozenFrame>>> = Mutex::new(None);

/// The last analysed selection, kept so the selector can re-translate it
/// into another language without running OCR again.
static LAST_ANALYSIS: Mutex<Option<CaptureFlowResult>> = Mutex::new(None);

/// Payload of the `moonlens://selector-frame` event.
#[derive(Debug, Clone, Serialize)]
pub struct FrameReady {
    pub width: u32,
    pub height: u32,
    /// When true the selector should immediately process the whole frame
    /// (the "capture window" hotkey) instead of waiting for a drag.
    pub auto_full: bool,
}

/// Selection rect in frozen-frame pixels.
#[derive(Debug, Clone, Copy, serde::Deserialize, Serialize, PartialEq, Eq)]
pub struct FrameRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Clamp a selection to the frame. `None` if nothing is left.
pub fn clamp_rect(r: FrameRect, width: u32, height: u32) -> Option<FrameRect> {
    let x = r.x.min(width);
    let y = r.y.min(height);
    let w = r.w.min(width - x);
    let h = r.h.min(height - y);
    (w > 0 && h > 0).then_some(FrameRect { x, y, w, h })
}

fn monitor_bounds(monitors: &[xcap::Monitor]) -> Result<Vec<rect::MonitorBounds>> {
    monitors
        .iter()
        .map(|m| -> Result<rect::MonitorBounds> {
            Ok(rect::MonitorBounds {
                origin_x: m.x()?,
                origin_y: m.y()?,
                w: m.width()?,
                h: m.height()?,
            })
        })
        .collect()
}

/// Freeze the monitor under the cursor and show the selector on it;
/// `auto_full` asks the selector to process the whole frame right away.
pub fn show_with(app: &AppHandle, auto_full: bool) -> Result<()> {
    let win = app
        .get_webview_window("selector")
        .ok_or_else(|| anyhow!("selector window missing"))?;
    // Make sure none of our own windows end up in the screenshot.
    win.hide().ok();
    crate::overlay::hide(app).ok();

    let monitors = xcap::Monitor::all().context("list monitors")?;
    let bounds = monitor_bounds(&monitors)?;
    let index = app
        .cursor_position()
        .ok()
        .and_then(|p| rect::monitor_at_point(&bounds, p.x as i32, p.y as i32))
        .unwrap_or(0);
    let monitor = monitors
        .get(index)
        .ok_or_else(|| anyhow!("no monitor available"))?;
    let b = bounds[index];

    let image = monitor.capture_image().context("capture monitor")?;
    let png = capture::png_bytes_fast(&image)?;
    let ready = FrameReady {
        width: image.width(),
        height: image.height(),
        auto_full,
    };
    *LAST_ANALYSIS.lock() = None;
    *FROZEN.lock() = Some(Arc::new(FrozenFrame {
        monitor_index: index,
        image,
        png,
    }));

    win.set_position(PhysicalPosition::new(b.origin_x, b.origin_y))?;
    win.set_size(PhysicalSize::new(b.w, b.h))?;
    app.emit_to("selector", "moonlens://selector-frame", ready)?;
    win.show()?;
    win.set_always_on_top(true)?;
    win.set_focus()?;
    Ok(())
}

/// Current frozen frame, if the selector is open.
pub fn frame() -> Option<Arc<FrozenFrame>> {
    FROZEN.lock().clone()
}

/// Crop `r` (frame pixels, `None` = whole frame) out of the frozen frame.
/// Returns the crop as PNG plus the clamped rect and monitor index.
pub fn take_crop(r: Option<FrameRect>) -> Result<(Vec<u8>, FrameRect, usize)> {
    let frame = frame().ok_or_else(|| anyhow!("no frozen frame; open the selector first"))?;
    let (fw, fh) = (frame.image.width(), frame.image.height());
    let wanted = r.unwrap_or(FrameRect {
        x: 0,
        y: 0,
        w: fw,
        h: fh,
    });
    let r = clamp_rect(wanted, fw, fh).ok_or_else(|| anyhow!("selection is empty"))?;
    let crop = image::imageops::crop_imm(&frame.image, r.x, r.y, r.w, r.h).to_image();
    let png = capture::png_bytes(&image::DynamicImage::ImageRgba8(crop))?;
    Ok((png, r, frame.monitor_index))
}

/// Remember an analysed (not yet translated) selection for
/// [`last_analysis`].
pub fn remember_analysis(bundle: &CaptureFlowResult) {
    *LAST_ANALYSIS.lock() = Some(bundle.clone());
}

/// The selection last passed to [`remember_analysis`], if the selector is
/// still showing it.
pub fn last_analysis() -> Option<CaptureFlowResult> {
    LAST_ANALYSIS.lock().clone()
}

/// Hide the selector and release the frozen frame.
pub fn hide(app: &AppHandle) -> Result<()> {
    *FROZEN.lock() = None;
    *LAST_ANALYSIS.lock() = None;
    if let Some(win) = app.get_webview_window("selector") {
        win.hide()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: u32, y: u32, w: u32, h: u32) -> FrameRect {
        FrameRect { x, y, w, h }
    }

    #[test]
    fn clamp_keeps_inside_rects() {
        assert_eq!(
            clamp_rect(r(10, 10, 50, 50), 100, 100),
            Some(r(10, 10, 50, 50))
        );
    }

    #[test]
    fn clamp_trims_overflow() {
        assert_eq!(
            clamp_rect(r(80, 90, 50, 50), 100, 100),
            Some(r(80, 90, 20, 10))
        );
    }

    #[test]
    fn clamp_rejects_empty_or_outside() {
        assert_eq!(clamp_rect(r(100, 0, 10, 10), 100, 100), None);
        assert_eq!(clamp_rect(r(0, 0, 0, 10), 100, 100), None);
    }
}
