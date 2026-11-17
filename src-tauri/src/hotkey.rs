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
    _pipeline: Arc<Pipeline>,
    subtitle: Arc<SubtitleService>,
) -> Result<()> {
    let s = settings.snapshot();
    let shortcut_mgr = app.global_shortcut();
    shortcut_mgr.unregister_all().ok();

    // Capture: freeze the monitor under the cursor and open the
    // selector on it; the renderer processes the selection in place.
    // Screen capture blocks for ~100 ms, so keep it off the event loop.
    let app_c = app.clone();
    shortcut_mgr.on_shortcut(
        s.hotkeys.capture_region.as_str(),
        move |_app, _sc, event| {
            if matches!(event.state(), ShortcutState::Pressed) {
                open_selector(&app_c, false);
            }
        },
    )?;

    // Capture window: same frozen-frame selector, but the whole monitor
    // is translated immediately (Lens "translate screen").
    let app_w = app.clone();
    shortcut_mgr.on_shortcut(s.hotkeys.capture_window.as_str(), move |_a, _s, e| {
        if matches!(e.state(), ShortcutState::Pressed) {
            open_selector(&app_w, true);
        }
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

    // Reverse image search: select a region on the frozen frame and send
    // it to visual search.
    let app_r = app.clone();
    shortcut_mgr.on_shortcut(s.hotkeys.reverse_image_search.as_str(), move |_a, _s, e| {
        if matches!(e.state(), ShortcutState::Pressed) {
            open_image_search(&app_r);
        }
    })?;

    // All other hotkeys just surface an intent; the frontend routes them.
    let remaining: Vec<(&str, &str)> = vec![
        (
            "open_command_palette",
            s.hotkeys.open_command_palette.as_str(),
        ),
        ("open_history", s.hotkeys.open_history.as_str()),
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

/// Open the frozen-frame selector without blocking the caller.
pub fn open_selector(app: &AppHandle, auto_full: bool) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = selector::show_with(&app, auto_full) {
            tracing::error!("selector show failed: {e:?}");
            overlay::show_centered(&app).ok();
            overlay::emit(
                &app,
                OverlayEvent::Error {
                    message: format!("画面キャプチャに失敗しました: {e}"),
                },
            )
            .ok();
        }
    });
}

/// Open the selector in image-search mode (see [`selector::show_for_search`]).
pub fn open_image_search(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = selector::show_for_search(&app) {
            tracing::error!("selector show failed: {e:?}");
        }
    });
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
