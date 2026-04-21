//! Global hotkey registration wired to pipeline actions.
//!
//! Bindings are re-registered whenever settings change so rebind is
//! immediate. Each shortcut either emits a UI intent (the
//! `moonlens://hotkey` event with an action name payload) or — for the
//! capture shortcut — directly drives the full pipeline and pushes
//! overlay events.
//!
//! Per-shortcut registration failures (e.g. the accelerator is already
//! held by another process or input method) do NOT abort startup. They
//! are collected into [`HotkeyRegistrationReport`] so the app can surface
//! them to the user while remaining usable. The user can then rebind the
//! conflicting shortcut from the settings UI.

use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;
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

/// One shortcut that could not be registered. The `id` is the logical
/// action name (e.g. `"capture_region"`) so the frontend can map the
/// failure back to a settings field without parsing the accelerator.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HotkeyFailure {
    pub id: String,
    pub accelerator: String,
    pub error: String,
}

/// Summary of a `register_all` call. Empty when every shortcut bound
/// successfully. Serialized for the `moonlens://hotkey-failure` event.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct HotkeyRegistrationReport {
    pub failed: Vec<HotkeyFailure>,
}

impl HotkeyRegistrationReport {
    pub fn is_empty(&self) -> bool {
        self.failed.is_empty()
    }

    fn push(&mut self, id: impl Into<String>, accelerator: impl Into<String>, error: String) {
        self.failed.push(HotkeyFailure {
            id: id.into(),
            accelerator: accelerator.into(),
            error,
        });
    }
}

/// Event name emitted to the frontend when one or more hotkeys fail to
/// register. Payload is [`HotkeyRegistrationReport`].
pub const HOTKEY_FAILURE_EVENT: &str = "moonlens://hotkey-failure";

pub fn register_all(
    app: &AppHandle,
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
    subtitle: Arc<SubtitleService>,
) -> Result<HotkeyRegistrationReport> {
    let s = settings.snapshot();
    let shortcut_mgr = app.global_shortcut();
    shortcut_mgr.unregister_all().ok();

    let mut report = HotkeyRegistrationReport::default();

    // Capture: open the selector overlay; the renderer invokes
    // capture_and_process once a rectangle is dragged. We keep the prior
    // pipeline handle in scope so the closure is `move`-friendly even when
    // we add a "select-then-capture" shortcut variant later.
    let app_c = app.clone();
    let _pipeline_c = pipeline.clone();
    let capture_region_accel = s.hotkeys.capture_region.clone();
    if let Err(e) =
        shortcut_mgr.on_shortcut(capture_region_accel.as_str(), move |_app, _sc, event| {
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
        })
    {
        tracing::warn!(
            "hotkey 'capture_region' registration failed: accel='{capture_region_accel}' error={e:?}"
        );
        report.push("capture_region", capture_region_accel, e.to_string());
    }

    // Capture window (whole primary monitor) – mirrors the pre-selector
    // behavior and is useful for one-shot full-screen translations.
    let app_w = app.clone();
    let pipeline_w = pipeline.clone();
    let capture_window_accel = s.hotkeys.capture_window.clone();
    if let Err(e) = shortcut_mgr.on_shortcut(capture_window_accel.as_str(), move |_a, _s, e| {
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
    }) {
        tracing::warn!(
            "hotkey 'capture_window' registration failed: accel='{capture_window_accel}' error={e:?}"
        );
        report.push("capture_window", capture_window_accel, e.to_string());
    }

    // Pin overlay toggle.
    let app_p = app.clone();
    let toggle_pin_accel = s.hotkeys.toggle_pin_overlay.clone();
    if let Err(e) = shortcut_mgr.on_shortcut(toggle_pin_accel.as_str(), move |_a, _s, e| {
        if matches!(e.state(), ShortcutState::Pressed) {
            let _ = app_p.emit("moonlens://hotkey", "toggle_pin_overlay");
        }
    }) {
        tracing::warn!(
            "hotkey 'toggle_pin_overlay' registration failed: accel='{toggle_pin_accel}' error={e:?}"
        );
        report.push("toggle_pin_overlay", toggle_pin_accel, e.to_string());
    }

    // Subtitle: toggle the live subtitle service.
    let app_s = app.clone();
    let subtitle_s = subtitle.clone();
    let toggle_subtitle_accel = s.hotkeys.toggle_subtitle.clone();
    if let Err(e) = shortcut_mgr.on_shortcut(toggle_subtitle_accel.as_str(), move |_a, _s, e| {
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
    }) {
        tracing::warn!(
            "hotkey 'toggle_subtitle' registration failed: accel='{toggle_subtitle_accel}' error={e:?}"
        );
        report.push("toggle_subtitle", toggle_subtitle_accel, e.to_string());
    }

    // Stop-all panic button: drives the full shutdown cascade inline
    // so the user gets immediate feedback even if no frontend window is
    // open to receive the intent event. Duplicates the logic from
    // `ipc::stop_all` but keeps the public command as the canonical
    // entry point (this path just reuses AppState handles directly).
    let app_stop = app.clone();
    let stop_all_accel = s.hotkeys.stop_all.clone();
    if let Err(e) = shortcut_mgr.on_shortcut(stop_all_accel.as_str(), move |_a, _s, e| {
        if !matches!(e.state(), ShortcutState::Pressed) {
            return;
        }
        let app = app_stop.clone();
        let state = app.state::<AppState>();
        // Snapshot before teardown so the event payload reflects what
        // was running when the button was hit.
        let lens_ids: Vec<String> = state.lenses.list().into_iter().map(|d| d.id).collect();
        let subtitle_was_running = state.subtitle.is_running();
        let overlay_was_open = app
            .get_webview_window("overlay")
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);
        state.lens_watchers.stop_all();
        let mut lens_count = 0usize;
        for id in &lens_ids {
            match crate::lens::close(&app, &state.lenses, id) {
                Ok(()) => lens_count += 1,
                Err(err) => tracing::warn!("stop_all hotkey: close lens {id} failed: {err:?}"),
            }
        }
        if let Err(err) = state.subtitle.stop(&app) {
            tracing::warn!("stop_all hotkey: stop subtitle failed: {err:?}");
        }
        if let Err(err) = overlay::hide(&app) {
            tracing::warn!("stop_all hotkey: hide overlay failed: {err:?}");
        }
        let payload = serde_json::json!({
            "lens_count": lens_count,
            "subtitle_was_running": subtitle_was_running,
            "overlay_was_open": overlay_was_open,
        });
        let _ = app.emit("moonlens://stop-all", payload);
        tracing::info!(
            "stop_all hotkey fired: {} lens closed, subtitle={}, overlay={}",
            lens_count,
            subtitle_was_running,
            overlay_was_open
        );
    }) {
        tracing::warn!(
            "hotkey 'stop_all' registration failed: accel='{stop_all_accel}' error={e:?}"
        );
        report.push("stop_all", stop_all_accel, e.to_string());
    }

    // All other hotkeys just surface an intent; the frontend routes them.
    let remaining: Vec<(&str, String)> = vec![
        (
            "open_command_palette",
            s.hotkeys.open_command_palette.clone(),
        ),
        ("open_history", s.hotkeys.open_history.clone()),
        (
            "reverse_image_search",
            s.hotkeys.reverse_image_search.clone(),
        ),
    ];
    for (action, accel) in remaining {
        let app_x = app.clone();
        let action_id = action.to_string();
        let action_for_closure = action_id.clone();
        if let Err(e) = shortcut_mgr.on_shortcut(accel.as_str(), move |_a, _s, e| {
            if matches!(e.state(), ShortcutState::Pressed) {
                let _ = app_x.emit("moonlens://hotkey", &action_for_closure);
            }
        }) {
            tracing::warn!("hotkey '{action_id}' registration failed: accel='{accel}' error={e:?}");
            report.push(action_id, accel, e.to_string());
        }
    }

    for (id, accel) in &s.hotkeys.custom {
        let app_x = app.clone();
        let id_owned = id.clone();
        let id_for_closure = id_owned.clone();
        let accel_owned = accel.clone();
        if let Err(e) = shortcut_mgr.on_shortcut(accel_owned.as_str(), move |_a, _s, e| {
            if matches!(e.state(), ShortcutState::Pressed) {
                let _ = app_x.emit("moonlens://custom-hotkey", &id_for_closure);
            }
        }) {
            tracing::warn!(
                "hotkey custom '{id_owned}' registration failed: accel='{accel_owned}' error={e:?}"
            );
            report.push(id_owned, accel_owned, e.to_string());
        }
    }
    Ok(report)
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
/// Propagates the per-shortcut failure report back to the caller so it can
/// be surfaced to the UI (the save command in `ipc::set_settings`).
pub fn reapply(app: &AppHandle) -> Result<HotkeyRegistrationReport> {
    let state = app.state::<AppState>();
    let s = state.inner();
    register_all(
        app,
        s.settings.clone(),
        s.pipeline.clone(),
        s.subtitle.clone(),
    )
}

/// Emit [`HOTKEY_FAILURE_EVENT`] if the report contains any failures.
/// No-op when the report is empty so callers can pipe `reapply` output
/// through this helper unconditionally.
pub fn notify_failures(app: &AppHandle, report: &HotkeyRegistrationReport) {
    if report.is_empty() {
        return;
    }
    if let Err(e) = app.emit(HOTKEY_FAILURE_EVENT, report) {
        tracing::warn!("failed to emit hotkey-failure event: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_report_is_empty() {
        let report = HotkeyRegistrationReport::default();
        assert!(report.is_empty());
        assert!(report.failed.is_empty());
    }

    #[test]
    fn push_records_failure_details() {
        let mut report = HotkeyRegistrationReport::default();
        report.push("capture_region", "Ctrl+Shift+Space", "already taken".into());

        assert!(!report.is_empty());
        assert_eq!(report.failed.len(), 1);
        assert_eq!(
            report.failed[0],
            HotkeyFailure {
                id: "capture_region".into(),
                accelerator: "Ctrl+Shift+Space".into(),
                error: "already taken".into(),
            }
        );
    }

    #[test]
    fn push_preserves_insertion_order() {
        let mut report = HotkeyRegistrationReport::default();
        report.push("a", "A", "e1".into());
        report.push("b", "B", "e2".into());
        report.push("c", "C", "e3".into());

        let ids: Vec<_> = report.failed.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn report_serializes_to_expected_shape() {
        let mut report = HotkeyRegistrationReport::default();
        report.push("capture_region", "Ctrl+Shift+Space", "taken".into());

        let value = serde_json::to_value(&report).expect("serde");
        let failed = value.get("failed").and_then(|v| v.as_array()).unwrap();
        assert_eq!(failed.len(), 1);
        let entry = &failed[0];
        assert_eq!(entry["id"], "capture_region");
        assert_eq!(entry["accelerator"], "Ctrl+Shift+Space");
        assert_eq!(entry["error"], "taken");
    }

    #[test]
    fn hotkey_failure_event_name_is_stable() {
        // Frontend listens on this exact string; guard against typos.
        assert_eq!(HOTKEY_FAILURE_EVENT, "moonlens://hotkey-failure");
    }
}
