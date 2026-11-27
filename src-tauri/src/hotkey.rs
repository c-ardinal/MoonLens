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

use crate::lens_control::HoldSource;
use crate::settings::schema::{Hotkeys, Triggers};
use crate::tr;
use crate::{app::AppState, settings::SettingsStore};

/// Notice id for shortcuts that could not be registered.
pub const NOTICE_HOTKEYS: &str = "hotkeys";

/// What a shortcut or an extra trigger (`trigger_watch.rs`) does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Window translation: open one more window under the mouse pointer.
    /// (`capture_window` is no longer bound: a maximized window is the
    /// full-screen translation.)
    OpenWindow,
    /// Window translation: open a window when there is none, otherwise
    /// close them all.
    ToggleWindows,
    /// Window translation: mouse through the windows / operate them.
    ToggleOperate,
    /// Window translation: pause / resume every window.
    TogglePause,
    /// Window translation: the original instead of the translations
    /// while the key is held.
    HoldOriginal,
    /// Subtitle: toggle the live subtitle service.
    ToggleSubtitle,
    OpenHistory,
    OpenCommandPalette,
}

impl Action {
    /// Run the action for a press (`pressed`) or a release of `source`.
    /// Only [`Action::HoldOriginal`] reacts to the release.
    pub fn run(self, app: &AppHandle, source: HoldSource, pressed: bool) {
        let state = app.state::<AppState>();
        if self == Action::HoldOriginal {
            crate::lens::hold_original(app, &state.lenses, source, pressed);
            return;
        }
        if !pressed {
            return;
        }
        match self {
            Action::OpenWindow => crate::lens::open_new(app),
            Action::ToggleWindows => {
                crate::lens::toggle(app, &state.lenses);
            }
            Action::ToggleOperate => {
                crate::lens::toggle_operate(app, &state.lenses);
            }
            Action::TogglePause => {
                crate::lens::toggle_pause(app, &state.lenses);
            }
            Action::HoldOriginal => {}
            Action::ToggleSubtitle => state.subtitle.toggle_checked(app),
            // History and the command palette live in the main window:
            // bring it up first, or the event lands in a hidden window
            // and nothing happens. (Reverse image search is not bound: it
            // sends the image to a web service, and MoonLens stays fully
            // local.)
            Action::OpenHistory => crate::app::route_main(app, "/history/"),
            Action::OpenCommandPalette => {
                crate::app::show_main_window(app);
                let _ = app.emit_to("main", "moonlens://hotkey", "open_command_palette");
            }
        }
    }
}

/// The shortcut of every action.
fn shortcuts(h: &Hotkeys) -> [(Action, &str); 8] {
    [
        (Action::OpenWindow, &h.capture_region),
        (Action::ToggleWindows, &h.toggle_pin_overlay),
        (Action::ToggleOperate, &h.toggle_window_operate),
        (Action::TogglePause, &h.toggle_window_pause),
        (Action::HoldOriginal, &h.hold_window_original),
        (Action::ToggleSubtitle, &h.toggle_subtitle),
        (Action::OpenHistory, &h.open_history),
        (Action::OpenCommandPalette, &h.open_command_palette),
    ]
}

/// The extra trigger of every action (see `trigger.rs` for the text form).
pub fn triggers(t: &Triggers) -> [(Action, &str); 8] {
    [
        (Action::OpenWindow, &t.capture_region),
        (Action::ToggleWindows, &t.toggle_pin_overlay),
        (Action::ToggleOperate, &t.toggle_window_operate),
        (Action::TogglePause, &t.toggle_window_pause),
        (Action::HoldOriginal, &t.hold_window_original),
        (Action::ToggleSubtitle, &t.toggle_subtitle),
        (Action::OpenHistory, &t.open_history),
        (Action::OpenCommandPalette, &t.open_command_palette),
    ]
}

pub fn register_all(app: &AppHandle, settings: Arc<SettingsStore>) -> Result<()> {
    let s = settings.snapshot();
    let shortcut_mgr = app.global_shortcut();
    shortcut_mgr.unregister_all().ok();
    // A held show-original key loses its binding, and its release with it.
    if let Some(state) = app.try_state::<AppState>() {
        crate::lens::cancel_hold(app, &state.lenses);
    }
    // One bad or duplicate binding must not take every other hotkey down
    // with it: register each one independently and report the failures.
    let mut failed: Vec<String> = Vec::new();

    for (action, accel) in shortcuts(&s.hotkeys) {
        let app_a = app.clone();
        bind(shortcut_mgr, &mut failed, accel, move |_app, _sc, event| {
            let pressed = matches!(event.state(), ShortcutState::Pressed);
            action.run(&app_a, HoldSource::Shortcut, pressed);
        });
    }

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
    failed.extend(crate::trigger_watch::apply(app, &s.triggers));
    if failed.is_empty() {
        crate::notices::clear(NOTICE_HOTKEYS);
        Ok(())
    } else {
        let msg = tr!(
            "hotkey:could_not_register_shortcuts_used_by_another",
            keys = failed.join(", ")
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
    register_all(app, state.settings.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shortcut_action_has_an_extra_trigger() {
        let actions = |pairs: [(Action, &str); 8]| pairs.map(|(a, _)| a);
        assert_eq!(
            actions(shortcuts(&Hotkeys::default())),
            actions(triggers(&Triggers::default()))
        );
    }

    #[test]
    fn extra_triggers_are_off_by_default() {
        let t = Triggers::default();
        assert!(triggers(&t).iter().all(|(_, text)| text.is_empty()));
    }
}
