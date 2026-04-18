//! Global hotkey registration wired to pipeline actions.
//!
//! Bindings are re-registered whenever settings change so rebind is
//! immediate. Each shortcut either emits a UI intent (the
//! `moonlens://hotkey` event with an action name payload) or — for the
//! capture shortcut — directly drives the full pipeline and pushes
//! overlay events.

use std::sync::Arc;

use anyhow::Result;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    app::AppState,
    overlay::{self, OverlayEvent},
    pipeline::Pipeline,
    selector,
    settings::SettingsStore,
    subtitle::SubtitleService,
};

pub fn register_all(
    app: &AppHandle,
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
    subtitle: Arc<SubtitleService>,
) -> Result<()> {
    let s = settings.snapshot();
    let shortcut_mgr = app.global_shortcut();
    shortcut_mgr.unregister_all().ok();

    // Capture: open the selector overlay; the renderer invokes
    // capture_and_process once a rectangle is dragged. We keep the prior
    // pipeline handle in scope so the closure is `move`-friendly even when
    // we add a "select-then-capture" shortcut variant later.
    let app_c = app.clone();
    let _pipeline_c = pipeline.clone();
    shortcut_mgr.on_shortcut(
        s.hotkeys.capture_region.as_str(),
        move |_app, _sc, event| {
            if !matches!(event.state(), ShortcutState::Pressed) {
                return;
            }
            let app = app_c.clone();
            if let Err(e) = selector::show(&app) {
                tracing::error!("selector show failed: {e:?}");
                overlay::emit(
                    &app,
                    OverlayEvent::Error {
                        message: e.to_string(),
                    },
                )
                .ok();
            }
        },
    )?;

    // Capture window (whole primary monitor) – mirrors the pre-selector
    // behavior and is useful for one-shot full-screen translations.
    let app_w = app.clone();
    let pipeline_w = pipeline.clone();
    shortcut_mgr.on_shortcut(s.hotkeys.capture_window.as_str(), move |_a, _s, e| {
        if !matches!(e.state(), ShortcutState::Pressed) {
            return;
        }
        let app = app_w.clone();
        let pipeline = pipeline_w.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = run_full_capture(&app, pipeline).await {
                tracing::error!("full capture failed: {e:?}");
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

    // Subtitle: toggle the live subtitle service.
    let app_s = app.clone();
    let subtitle_s = subtitle.clone();
    shortcut_mgr.on_shortcut(s.hotkeys.toggle_subtitle.as_str(), move |_a, _s, e| {
        if !matches!(e.state(), ShortcutState::Pressed) {
            return;
        }
        match subtitle_s.toggle(&app_s) {
            Ok(running) => {
                tracing::info!("subtitle running={running}");
            }
            Err(err) => {
                tracing::warn!("subtitle toggle failed: {err:?}");
                let _ = app_s.emit("moonlens://hotkey", "toggle_subtitle_failed");
            }
        }
    })?;

    // All other hotkeys just surface an intent; the frontend routes them.
    let remaining: Vec<(&str, &str)> = vec![
        (
            "open_command_palette",
            s.hotkeys.open_command_palette.as_str(),
        ),
        ("open_history", s.hotkeys.open_history.as_str()),
        (
            "reverse_image_search",
            s.hotkeys.reverse_image_search.as_str(),
        ),
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

async fn run_full_capture(app: &AppHandle, pipeline: Arc<Pipeline>) -> anyhow::Result<()> {
    use crate::capture;
    overlay::emit(
        app,
        OverlayEvent::Processing {
            message: "キャプチャ中…".into(),
        },
    )
    .ok();

    let (image_png, width, origin_x, origin_y) =
        tokio::task::spawn_blocking(|| -> anyhow::Result<(Vec<u8>, u32, i32, i32)> {
            let monitors = xcap::Monitor::all()?;
            let monitor = monitors
                .first()
                .ok_or_else(|| anyhow::anyhow!("no monitor"))?;
            // xcap 0.9 exposes monitor geometry as Result-returning getters
            // because any accessor can in principle call back into the
            // platform API. We resolve every field eagerly so the rest of
            // the function can work with plain integers.
            let w = monitor.width()?;
            let h = monitor.height()?;
            let x = monitor.x()?;
            let y = monitor.y()?;
            let img = capture::capture_rect(crate::capture::CaptureRect {
                monitor_index: 0,
                x: 0,
                y: 0,
                w,
                h,
            })?;
            let png = capture::png_bytes(&img)?;
            Ok((png, w, x, y))
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
            intent: Some(bundle.intent),
            result: bundle.result,
            source_rect: bundle.source_rect,
        },
    )
    .ok();
    Ok(())
}

/// Convenience re-registration triggered from the settings save command.
pub fn reapply(app: &AppHandle) -> Result<()> {
    let state = app.state::<AppState>();
    let s = state.inner();
    register_all(
        app,
        s.settings.clone(),
        s.pipeline.clone(),
        s.subtitle.clone(),
    )
}
