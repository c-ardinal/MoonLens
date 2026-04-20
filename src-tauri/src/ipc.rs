//! Tauri command handlers exposed to the Svelte frontend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    app::AppState,
    capture::{self, CaptureRect},
    clipboard, hotkey, intent,
    lens::{self, geometry::LensInsets, LensDescriptor},
    overlay,
    pipeline::{CaptureFlowResult, TaskResult},
    plugins::PluginDescriptor,
    providers::{ocr_ocrs, Capability, TaskInput},
    selector,
    settings::schema::Settings,
};

// Individual Tauri commands. App wiring registers them via
// `tauri::generate_handler![...]`; keep that list in sync when adding a
// new command below.

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok((*state.settings.snapshot()).clone())
}

#[tauri::command]
pub async fn set_settings(
    new: Settings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.settings.save(new).map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    // Hotkey bindings might have changed; re-register best-effort and
    // surface per-shortcut failures via the `moonlens://hotkey-failure`
    // event rather than rejecting the whole save.
    let report = hotkey::reapply(&app).map_err(|e| e.to_string())?;
    hotkey::notify_failures(&app, &report);
    Ok(())
}

#[tauri::command]
pub async fn get_settings_schema() -> Result<serde_json::Value, String> {
    Ok(crate::settings::SettingsStore::json_schema())
}

#[tauri::command]
pub async fn list_plugins(state: State<'_, AppState>) -> Result<Vec<PluginDescriptor>, String> {
    Ok(state.plugins.list())
}

#[tauri::command]
pub async fn set_plugin_enabled(
    id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .plugins
        .set_enabled(&id, enabled)
        .map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    Ok(())
}

#[derive(Deserialize)]
pub struct TranslateArgs {
    pub text: String,
    pub source_language: String,
    pub target_language: String,
}

#[tauri::command]
pub async fn run_translate(
    args: TranslateArgs,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::Translate,
            TaskInput::Translate {
                text: args.text,
                source_language: args.source_language,
                target_language: args.target_language,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_ocr(
    image_png: Vec<u8>,
    languages: Vec<String>,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::Ocr,
            TaskInput::Ocr {
                image_png,
                languages,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_intent(image_png: Vec<u8>) -> Result<String, String> {
    let img = image::load_from_memory(&image_png).map_err(|e| e.to_string())?;
    Ok(format!("{:?}", intent::classify(&img)))
}

#[tauri::command]
pub async fn run_qr(image_png: Vec<u8>, state: State<'_, AppState>) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::QrHandler,
            TaskInput::IntentClassifier { image_png },
        )
        .await
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct ExplainArgs {
    pub prompt: String,
    pub image_png: Option<Vec<u8>>,
}

#[tauri::command]
pub async fn run_explain(
    args: ExplainArgs,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::Explain,
            TaskInput::Explain {
                prompt: args.prompt,
                image_png: args.image_png,
            },
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_reverse_image_search(
    image_url: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    let result = state
        .pipeline
        .run(
            Capability::ReverseImageSearch,
            TaskInput::ReverseImageSearch { image_url },
        )
        .await
        .map_err(|e| e.to_string())?;
    // The provider returns the URL that should be visited — actually
    // open it in the default browser so the user sees results instead
    // of just a raw string in the overlay.
    if let crate::providers::TaskOutput::OpenedExternalUrl(url) = &result.output {
        tauri_plugin_opener::OpenerExt::opener(&app)
            .open_url(url, None::<&str>)
            .map_err(|e| e.to_string())?;
    }
    Ok(result)
}

#[derive(Deserialize, Clone)]
pub struct RegionArgs {
    pub monitor_index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Serialize)]
pub struct CapturedRegion {
    pub image_png: Vec<u8>,
}

#[tauri::command]
pub async fn capture_region_cmd(
    args: RegionArgs,
    _app: AppHandle,
) -> Result<CapturedRegion, String> {
    let img = tokio::task::spawn_blocking(move || {
        capture::capture_rect(CaptureRect {
            monitor_index: args.monitor_index,
            x: args.x,
            y: args.y,
            w: args.w,
            h: args.h,
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let image_png = capture::png_bytes(&img).map_err(|e| e.to_string())?;
    Ok(CapturedRegion { image_png })
}

/// End-to-end: capture a region (or the whole primary monitor if `args`
/// is `None`), run intent → OCR → translate, and return the full flow
/// result. Emits overlay events so the floating panel updates live.
/// Start the region watcher on the last captured rect so subsequent
/// pixel changes refresh the overlay automatically. Matches the
/// "pinned overlay" UX. Idempotent: calling it twice just replaces
/// the target.
#[tauri::command]
pub async fn pin_overlay(
    args: RegionArgs,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target = crate::region_watcher::WatchTarget {
        rect: CaptureRect {
            monitor_index: args.monitor_index,
            x: args.x,
            y: args.y,
            w: args.w,
            h: args.h,
        },
    };
    state
        .region_watcher
        .start(app, state.pipeline.clone(), state.settings.clone(), target);
    Ok(())
}

#[tauri::command]
pub async fn unpin_overlay(state: State<'_, AppState>) -> Result<(), String> {
    state.region_watcher.stop();
    Ok(())
}

#[tauri::command]
pub async fn capture_and_process(
    args: Option<RegionArgs>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CaptureFlowResult, String> {
    overlay::emit(
        &app,
        overlay::OverlayEvent::Processing {
            message: "キャプチャ中…".into(),
        },
    )
    .ok();

    let args_for_capture = args.clone();
    let png = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let img = match args_for_capture {
            Some(a) => capture::capture_rect(CaptureRect {
                monitor_index: a.monitor_index,
                x: a.x,
                y: a.y,
                w: a.w,
                h: a.h,
            })?,
            None => capture::capture_primary_full()?,
        };
        capture::png_bytes(&img)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let mut bundle = state
        .pipeline
        .run_capture_flow(png)
        .await
        .map_err(|e| e.to_string())?;
    // Record the rect we actually captured so the overlay can pin the
    // region watcher onto it. Full-monitor captures resolve to the
    // primary monitor's bounds.
    bundle.source_rect = Some(match args {
        Some(a) => crate::pipeline::CaptureFlowRect {
            monitor_index: a.monitor_index,
            x: a.x,
            y: a.y,
            w: a.w,
            h: a.h,
        },
        None => {
            let monitors = xcap::Monitor::all().map_err(|e| e.to_string())?;
            let m = monitors.first().ok_or("no monitor")?;
            crate::pipeline::CaptureFlowRect {
                monitor_index: 0,
                x: 0,
                y: 0,
                w: m.width().map_err(|e| e.to_string())?,
                h: m.height().map_err(|e| e.to_string())?,
            }
        }
    });

    // Honour the clipboard policy: translation for text captures, QR
    // payload for QR captures. Source text is only copied when the user
    // opted in (default off). All of this is no-op safe when the user
    // disabled the relevant setting.
    let clipboard_cfg = state.settings.snapshot().clipboard.clone();
    if let Some(src) = &bundle.source_text {
        clipboard::auto_copy(&app, clipboard::Kind::Source, src, &clipboard_cfg);
    }
    match &bundle.result.output {
        crate::providers::TaskOutput::Text(t) => {
            let kind = match bundle.intent {
                crate::providers::Intent::Qr => clipboard::Kind::Qr,
                _ => clipboard::Kind::Translation,
            };
            clipboard::auto_copy(&app, kind, t, &clipboard_cfg);
        }
        crate::providers::TaskOutput::Ocr { lines } => {
            let joined = lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            clipboard::auto_copy(&app, clipboard::Kind::Source, &joined, &clipboard_cfg);
        }
        _ => {}
    }

    overlay::show_centered(&app).ok();
    overlay::emit(
        &app,
        overlay::OverlayEvent::Result {
            source_text: bundle.source_text.clone(),
            intent: Some(bundle.intent),
            result: bundle.result.clone(),
            source_rect: bundle.source_rect.clone(),
        },
    )
    .ok();
    Ok(bundle)
}

#[derive(serde::Serialize, Clone)]
pub struct ModelProgress {
    pub filename: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    /// 0.0..=1.0 when the server advertised Content-Length, else null.
    /// Pre-computed so the UI does not have to guard against division
    /// by zero in a template.
    pub fraction: Option<f32>,
}

#[tauri::command]
pub async fn install_ocr_models(app: AppHandle) -> Result<String, String> {
    let dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    let app_for_cb = app.clone();
    let cb: Box<ocr_ocrs::ProgressCb> = Box::new(move |filename, downloaded, total| {
        let fraction = total.and_then(|t| {
            if t == 0 {
                None
            } else {
                Some((downloaded as f32 / t as f32).clamp(0.0, 1.0))
            }
        });
        let payload = ModelProgress {
            filename: filename.to_string(),
            downloaded,
            total,
            fraction,
        };
        let _ = app_for_cb.emit("moonlens://ocr-model-progress", payload);
    });
    ocr_ocrs::install_models_with_progress(&dir, Some(&*cb))
        .await
        .map_err(|e| e.to_string())?;
    Ok(dir.display().to_string())
}

/// Pick a `.moonlens-plugin` archive (or a plugin directory) with the
/// platform's native file picker and install it. Returns the new plugin
/// root on success; returns Ok(None) when the user cancels the dialog.
/// Query each plugin's `update_url` concurrently and return the subset
/// that advertises a newer version than installed.
#[tauri::command]
pub async fn check_plugin_updates(
    state: State<'_, AppState>,
) -> Result<Vec<crate::plugins::updates::UpdateAvailable>, String> {
    let manifests = state.plugins.manifests();
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let futures = manifests.iter().filter_map(|m| {
        m.plugin.update_url.as_ref().map(|_| {
            let m = m.clone();
            let http = http.clone();
            async move { crate::plugins::updates::check_one(&http, &m).await }
        })
    });
    let results = futures::future::join_all(futures).await;
    let mut updates = Vec::new();
    for res in results {
        match res {
            Ok(Some(u)) => updates.push(u),
            Ok(None) => {}
            Err(e) => tracing::warn!("update check failed: {e}"),
        }
    }
    Ok(updates)
}

#[tauri::command]
pub async fn pick_and_install_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dialog = app.dialog().clone();
    let chosen = tokio::task::spawn_blocking(move || {
        dialog
            .file()
            .add_filter("MoonLens Plugin", &["moonlens-plugin", "zip"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(file) = chosen else { return Ok(None) };
    let path = file
        .into_path()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let installed = install_plugin_from_path(path, app, state).await?;
    Ok(Some(installed))
}

#[tauri::command]
pub async fn install_plugin_from_path(
    archive_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let dest = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("plugins");
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let src = PathBuf::from(&archive_path);
    let target = if src.is_dir() {
        let name = src
            .file_name()
            .ok_or("invalid archive path")?
            .to_string_lossy()
            .to_string();
        let target = dest.join(&name);
        copy_dir(&src, &target).map_err(|e| e.to_string())?;
        target
    } else if src.is_file() {
        crate::plugins::installer::extract_zip(&src, &dest).map_err(|e| e.to_string())?
    } else {
        return Err("archive_path is neither a file nor a directory".into());
    };
    state.plugins.rescan().map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
    Ok(target.display().to_string())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn show_main(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_overlay(app: AppHandle) -> Result<(), String> {
    overlay::hide(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_history(
    limit: Option<usize>,
    query: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::storage::HistoryEntry>, String> {
    let cap = limit.unwrap_or(200);
    let all = state.storage.recent(cap).map_err(|e| e.to_string())?;
    let needle = query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase);
    let filtered = match needle {
        None => all,
        Some(q) => all
            .into_iter()
            .filter(|e| {
                e.source_text
                    .as_deref()
                    .map(|s| s.to_lowercase().contains(&q))
                    .unwrap_or(false)
                    || e.result_text
                        .as_deref()
                        .map(|s| s.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || e.provider
                        .as_deref()
                        .map(|s| s.to_lowercase().contains(&q))
                        .unwrap_or(false)
                    || e.kind.to_lowercase().contains(&q)
            })
            .collect(),
    };
    Ok(filtered)
}

#[tauri::command]
pub async fn delete_history_entry(id: i64, state: State<'_, AppState>) -> Result<usize, String> {
    // Return the affected row count so the UI can distinguish "no such
    // id" (0) from an actual removal (1) without a second round-trip.
    state.storage.delete(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<usize, String> {
    state.storage.clear_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_region_selection(app: AppHandle) -> Result<(), String> {
    selector::show(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_region_selection(app: AppHandle) -> Result<(), String> {
    selector::hide(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    state.subtitle.toggle(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn subtitle_state(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.subtitle.is_running())
}

#[tauri::command]
pub async fn stop_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.subtitle.stop(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_subtitle(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.subtitle.start(app).map_err(|e| e.to_string())
}

/// Describes which first-run guidance the UI should show. Exposed as a
/// single struct so the frontend can decide in one round-trip.
#[derive(serde::Serialize)]
pub struct OnboardingStatus {
    pub platform: &'static str,
    pub needs_macos_permissions_review: bool,
    /// Live per-permission snapshot. The modal renders each status as
    /// a badge next to its System Settings link.
    pub macos_permissions: crate::macos_permissions::PermissionReport,
}

#[tauri::command]
pub async fn get_onboarding_status(state: State<'_, AppState>) -> Result<OnboardingStatus, String> {
    let snapshot = state.settings.snapshot();
    let platform = std::env::consts::OS;
    let perms = crate::macos_permissions::snapshot();
    // Surface the modal only when we are on macOS AND the user has
    // not dismissed it AND at least one detectable permission is
    // still denied. Fully-granted sessions skip onboarding entirely.
    let needs_macos_permissions_review = cfg!(target_os = "macos")
        && !snapshot.onboarding.macos_permissions_reviewed
        && (matches!(
            perms.screen_capture,
            crate::macos_permissions::PermissionStatus::Denied
        ) || matches!(
            perms.accessibility,
            crate::macos_permissions::PermissionStatus::Denied
        ));
    Ok(OnboardingStatus {
        platform: platform_label(platform),
        needs_macos_permissions_review,
        macos_permissions: perms,
    })
}

/// Re-query the live permission snapshot. The onboarding UI calls this
/// after the user visits a System Settings pane so the badges flip
/// green without requiring an app restart.
#[tauri::command]
pub async fn macos_permissions_snapshot(
) -> Result<crate::macos_permissions::PermissionReport, String> {
    Ok(crate::macos_permissions::snapshot())
}

/// Trigger the macOS "Allow MoonLens to record your screen?" prompt.
/// No-op on non-macOS targets; returns the post-prompt status.
#[tauri::command]
pub async fn macos_request_screen_capture(
) -> Result<crate::macos_permissions::PermissionStatus, String> {
    Ok(crate::macos_permissions::request_screen_capture())
}

#[tauri::command]
pub async fn mark_macos_permissions_reviewed(state: State<'_, AppState>) -> Result<(), String> {
    let mut s = (*state.settings.snapshot()).clone();
    s.onboarding.macos_permissions_reviewed = true;
    state.settings.save(s).map_err(|e| e.to_string())
}

/// Open a specific macOS privacy pane via the `x-apple.systempreferences`
/// URL scheme. No-op on other platforms (returns Ok).
#[tauri::command]
pub async fn open_macos_privacy_pane(app: AppHandle, pane: String) -> Result<(), String> {
    let url = match pane.as_str() {
        "screen_capture" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
        }
        "accessibility" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        "microphone" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
        }
        "input_monitoring" => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
        }
        other => return Err(format!("unknown pane: {other}")),
    };
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

fn platform_label(os: &str) -> &'static str {
    match os {
        "macos" => "macos",
        "windows" => "windows",
        "linux" => "linux",
        _ => "other",
    }
}

#[tauri::command]
pub async fn new_lens_window(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    lens::create(&app, &state.lenses).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn close_lens_window(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    lens::close(&app, &state.lenses, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_lens_windows(state: State<'_, AppState>) -> Result<Vec<LensDescriptor>, String> {
    Ok(state.lenses.list())
}

#[tauri::command]
pub async fn set_lens_insets(
    id: String,
    top: u32,
    bottom: u32,
    left: u32,
    right: u32,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lenses.set_insets(
        &id,
        LensInsets {
            top,
            bottom,
            left,
            right,
        },
    );
    Ok(())
}

#[tauri::command]
pub async fn set_lens_auto_refresh(
    id: String,
    ms: Option<u32>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lenses.set_auto_refresh(&id, ms);
    Ok(())
}

/// Capture the observed rect of a lens window and run the full pipeline.
/// Emits overlay events to the regular overlay window so the result can
/// be displayed either inside the lens body (via the lens page) or over
/// it (legacy overlay).
#[tauri::command]
pub async fn capture_lens_now(
    id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CaptureFlowResult, String> {
    let lenses = state.lenses.clone();
    let pipeline = state.pipeline.clone();
    let app_clone = app.clone();
    let id_clone = id.clone();
    let png =
        tokio::task::spawn_blocking(move || lens::capture_lens(&app_clone, &lenses, &id_clone))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
    let bundle = pipeline
        .run_capture_flow(png)
        .await
        .map_err(|e| e.to_string())?;
    // Lens captures must obey the same clipboard policy as the
    // regular ⌘⇧Space flow. Without this the user's auto_copy settings
    // would apply to ad-hoc captures but silently drop lens results.
    let clipboard_cfg = state.settings.snapshot().clipboard.clone();
    if let Some(src) = &bundle.source_text {
        clipboard::auto_copy(&app, clipboard::Kind::Source, src, &clipboard_cfg);
    }
    if let crate::providers::TaskOutput::Text(t) = &bundle.result.output {
        let kind = match bundle.intent {
            crate::providers::Intent::Qr => clipboard::Kind::Qr,
            _ => clipboard::Kind::Translation,
        };
        clipboard::auto_copy(&app, kind, t, &clipboard_cfg);
    }
    // Push the result to the lens-specific event channel so the lens
    // renderer updates in place; also emit the generic overlay event so
    // the floating panel can mirror it if opened.
    let _ = app.emit_to(
        tauri::EventTarget::WebviewWindow { label: id.clone() },
        "moonlens://lens-result",
        bundle.clone(),
    );
    overlay::emit(
        &app,
        overlay::OverlayEvent::Result {
            source_text: bundle.source_text.clone(),
            intent: Some(bundle.intent),
            result: bundle.result.clone(),
            source_rect: bundle.source_rect.clone(),
        },
    )
    .ok();
    Ok(bundle)
}
