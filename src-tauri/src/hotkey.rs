//! Global hotkey registration wired to pipeline actions.
//!
//! Bindings are re-registered whenever settings change so rebind is
//! immediate. Each shortcut either emits a UI intent (the
//! `moonlens://hotkey` event with an action name payload) or acts
//! directly (window translation, subtitles).

use std::sync::Arc;

use anyhow::Result;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::tr;
use crate::{
    app::AppState, pipeline::Pipeline, settings::SettingsStore, subtitle::SubtitleService,
};

/// Notice id for shortcuts that could not be registered.
pub const NOTICE_HOTKEYS: &str = "hotkeys";

pub fn register_all(
    app: &AppHandle,
    settings: Arc<SettingsStore>,
    _pipeline: Arc<Pipeline>,
    subtitle: Arc<SubtitleService>,
) -> Result<()> {
    let s = settings.snapshot();
    let shortcut_mgr = app.global_shortcut();
    shortcut_mgr.unregister_all().ok();
    // One bad or duplicate binding must not take every other hotkey down
    // with it: register each one independently and report the failures.
    let mut failed: Vec<String> = Vec::new();

    // Window translation: open one more window under the mouse pointer.
    // (`capture_window` is no longer bound: a maximized window is the
    // full-screen translation.)
    let app_c = app.clone();
    bind(
        shortcut_mgr,
        &mut failed,
        s.hotkeys.capture_region.as_str(),
        move |_app, _sc, event| {
            if matches!(event.state(), ShortcutState::Pressed) {
                crate::lens::open_new(&app_c);
            }
        },
    );

    // Window translation: open a window when there is none, otherwise
    // close them all.
    let app_p = app.clone();
    bind(
        shortcut_mgr,
        &mut failed,
        s.hotkeys.toggle_pin_overlay.as_str(),
        move |_a, _s, e| {
            if matches!(e.state(), ShortcutState::Pressed) {
                let state = app_p.state::<AppState>();
                crate::lens::toggle(&app_p, &state.lenses);
            }
        },
    );

    // Window translation: mouse through the windows / operate them.
    let app_o = app.clone();
    bind(
        shortcut_mgr,
        &mut failed,
        s.hotkeys.toggle_window_operate.as_str(),
        move |_a, _s, e| {
            if matches!(e.state(), ShortcutState::Pressed) {
                let state = app_o.state::<AppState>();
                crate::lens::toggle_operate(&app_o, &state.lenses);
            }
        },
    );

    // Subtitle: toggle the live subtitle service.
    let app_s = app.clone();
    let subtitle_s = subtitle.clone();
    bind(
        shortcut_mgr,
        &mut failed,
        s.hotkeys.toggle_subtitle.as_str(),
        move |_a, _s, e| {
            if !matches!(e.state(), ShortcutState::Pressed) {
                return;
            }
            subtitle_s.toggle_checked(&app_s);
        },
    );

    // History and the command palette live in the main window: bring it
    // up first, or the event lands in a hidden window and nothing happens.
    // (Reverse image search is not registered: it sends the image to a web
    // service, and MoonLens stays fully local.)
    let app_h = app.clone();
    bind(
        shortcut_mgr,
        &mut failed,
        s.hotkeys.open_history.as_str(),
        move |_a, _s, e| {
            if matches!(e.state(), ShortcutState::Pressed) {
                crate::app::route_main(&app_h, "/history/");
            }
        },
    );
    let app_k = app.clone();
    bind(
        shortcut_mgr,
        &mut failed,
        s.hotkeys.open_command_palette.as_str(),
        move |_a, _s, e| {
            if matches!(e.state(), ShortcutState::Pressed) {
                crate::app::show_main_window(&app_k);
                let _ = app_k.emit_to("main", "moonlens://hotkey", "open_command_palette");
            }
        },
    );

    for (id, accel) in &s.hotkeys.custom {
        let app_x = app.clone();
        let id = id.clone();
        bind(
            shortcut_mgr,
            &mut failed,
            accel.as_str(),
            move |_a, _s, e| {
                if matches!(e.state(), ShortcutState::Pressed) {
                    let _ = app_x.emit("moonlens://custom-hotkey", &id);
                }
            },
        );
    }
    if failed.is_empty() {
        crate::notices::clear(NOTICE_HOTKEYS);
        Ok(())
    } else {
        let msg = tr!(
            "ショートカット登録失敗（他アプリと重複、または形式不正）：{}",
            "Could not register shortcuts (used by another app, or malformed): {}",
            failed.join(", ")
        );
        // Also shown on the home screen: at start-up nobody is looking
        // at a log.
        crate::notices::push(
            NOTICE_HOTKEYS,
            msg.clone(),
            Some("/settings/#section-hotkeys"),
        );
        Err(anyhow::anyhow!(msg))
    }
}

/// Register one global shortcut. An empty accelerator means "disabled".
fn bind<R, F>(
    mgr: &tauri_plugin_global_shortcut::GlobalShortcut<R>,
    failed: &mut Vec<String>,
    accel: &str,
    handler: F,
) where
    R: tauri::Runtime,
    F: Fn(
            &AppHandle<R>,
            &tauri_plugin_global_shortcut::Shortcut,
            tauri_plugin_global_shortcut::ShortcutEvent,
        ) + Send
        + Sync
        + 'static,
{
    let accel = accel.trim();
    if accel.is_empty() {
        return;
    }
    if let Err(e) = mgr.on_shortcut(accel, handler) {
        tracing::warn!("hotkey {accel} not registered: {e}");
        failed.push(accel.to_string());
    }
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
