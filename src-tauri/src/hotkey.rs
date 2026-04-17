//! Global hotkey registration wired to pipeline actions.
//!
//! Bindings are re-registered whenever settings change so rebind is
//! immediate. Each shortcut either emits a UI intent (`moonlens://hotkey`
//! + action name) or, for the capture shortcut, directly drives the full
//! pipeline and pushes overlay events.

use std::sync::Arc;

use anyhow::Result;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    app::AppState,
    capture::{self, CaptureRect},
    overlay::{self, OverlayEvent},
    pipeline::Pipeline,
    settings::SettingsStore,
};

pub fn register_all(
    app: &AppHandle,
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
) -> Result<()> {
    let s = settings.snapshot();
    let shortcut_mgr = app.global_shortcut();
    shortcut_mgr.unregister_all().ok();

    // Capture: run the full capture flow and drive the overlay.
    let app_c = app.clone();
    let pipeline_c = pipeline.clone();
    shortcut_mgr.on_shortcut(s.hotkeys.capture_region.as_str(), move |_app, _sc, event| {
        if !matches!(event.state(), ShortcutState::Pressed) {
            return;
        }
        let app = app_c.clone();
        let pipeline = pipeline_c.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = run_capture(&app, pipeline).await {
                tracing::error!("capture flow failed: {e:?}");
                overlay::emit(
                    &app,
                    OverlayEvent::Error {
                        message: e.to_string(),
                    },
                )
                .ok();
            }
        });
    })?;

    // Pin overlay toggle.
    let app_p = app.clone();
    shortcut_mgr.on_shortcut(s.hotkeys.toggle_pin_overlay.as_str(), move |_a, _s, e| {
        if matches!(e.state(), ShortcutState::Pressed) {
            let _ = app_p.emit("moonlens://hotkey", "toggle_pin_overlay");
        }
    })?;

    // All other hotkeys just surface an intent; the frontend routes them.
    let remaining: Vec<(&str, &str)> = vec![
        ("capture_window", s.hotkeys.capture_window.as_str()),
        ("toggle_subtitle", s.hotkeys.toggle_subtitle.as_str()),
        ("open_command_palette", s.hotkeys.open_command_palette.as_str()),
        ("open_history", s.hotkeys.open_history.as_str()),
        ("reverse_image_search", s.hotkeys.reverse_image_search.as_str()),
    ];
    for (action, accel) in remaining {
        let app_x = app.clone();
        let action = action.to_string();
        shortcut_mgr.on_shortcut(accel, move |_a, _s, e| {
            if matches!(e.state(), ShortcutState::Pressed) {
                let _ = app_x.emit("moonlens://hotkey", &action);
            }
        })?;
    }

    for (id, accel) in &s.hotkeys.custom {
        let app_x = app.clone();
        let id = id.clone();
        shortcut_mgr.on_shortcut(accel.as_str(), move |_a, _s, e| {
            if matches!(e.state(), ShortcutState::Pressed) {
                let _ = app_x.emit("moonlens://custom-hotkey", &id);
            }
        })?;
    }
    Ok(())
}

async fn run_capture(app: &AppHandle, pipeline: Arc<Pipeline>) -> anyhow::Result<()> {
    overlay::emit(app, OverlayEvent::Processing {
        message: "キャプチャ中…".into(),
    })
    .ok();

    // MVP strategy: snapshot the primary monitor and pass the whole frame
    // to the pipeline. The region-selector webview (future work) will
    // instead supply a rectangle before invoking `capture_rect`.
    let (image_png, width, origin_x, origin_y) =
        tokio::task::spawn_blocking(|| -> anyhow::Result<(Vec<u8>, u32, i32, i32)> {
            let monitors = xcap::Monitor::all()?;
            let monitor = monitors
                .first()
                .ok_or_else(|| anyhow::anyhow!("no monitor"))?;
            let rect = CaptureRect {
                monitor_index: 0,
                x: 0,
                y: 0,
                w: monitor.width(),
                h: monitor.height(),
            };
            let img = capture::capture_rect(rect)?;
            let png = capture::png_bytes(&img)?;
            Ok((png, monitor.width(), monitor.x(), monitor.y()))
        })
        .await??;

    let bundle = pipeline
        .run_capture_flow(image_png)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Position the overlay near the top-right of the captured monitor.
    let overlay_x = origin_x + width as i32 - 520;
    let overlay_y = origin_y + 40;
    overlay::show_near(app, overlay_x, overlay_y, Some(500))?;
    overlay::emit(
        app,
        OverlayEvent::Result {
            source_text: bundle.source_text,
            result: bundle.result,
        },
    )
    .ok();
    Ok(())
}

/// Convenience re-registration triggered from the settings save command.
pub fn reapply(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    register_all(app, state.settings.clone(), state.pipeline.clone())
}
