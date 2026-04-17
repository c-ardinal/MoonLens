//! Global hotkey registration wired to pipeline actions. Bindings are
//! re-registered on every settings change so rebind is immediate.

use std::sync::Arc;

use anyhow::Result;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{pipeline::Pipeline, settings::SettingsStore};

pub fn register_all(
    app: &AppHandle,
    settings: Arc<SettingsStore>,
    _pipeline: Arc<Pipeline>,
) -> Result<()> {
    let s = settings.snapshot();
    let app2 = app.clone();

    let shortcut_mgr = app.global_shortcut();
    shortcut_mgr.unregister_all().ok();

    let bindings: Vec<(&str, &str)> = vec![
        ("capture_region", s.hotkeys.capture_region.as_str()),
        ("capture_window", s.hotkeys.capture_window.as_str()),
        ("toggle_pin_overlay", s.hotkeys.toggle_pin_overlay.as_str()),
        ("toggle_subtitle", s.hotkeys.toggle_subtitle.as_str()),
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
    for (action, accel) in bindings {
        let app3 = app2.clone();
        let action = action.to_string();
        let accel_owned = accel.to_string();
        shortcut_mgr.on_shortcut(accel, move |_app, _shortcut, event| {
            if matches!(event.state(), ShortcutState::Pressed) {
                let _ = app3.emit("moonlens://hotkey", &action);
                tracing::debug!("hotkey: {} ({accel_owned})", action);
            }
        })?;
    }
    for (id, accel) in &s.hotkeys.custom {
        let app3 = app2.clone();
        let id = id.clone();
        shortcut_mgr.on_shortcut(accel.as_str(), move |_app, _sc, event| {
            if matches!(event.state(), ShortcutState::Pressed) {
                let _ = app3.emit("moonlens://custom-hotkey", &id);
            }
        })?;
    }
    Ok(())
}
