//! Tauri command handlers exposed to the Svelte frontend.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::{
    app::AppState,
    capture::{self, CaptureRect},
    hotkey, intent, overlay,
    pipeline::{CaptureFlowResult, TaskResult},
    plugins::PluginDescriptor,
    providers::{ocr_ocrs, Capability, TaskInput},
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
    // Hotkey bindings might have changed; re-register atomically.
    hotkey::reapply(&app).map_err(|e| e.to_string())?;
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

#[tauri::command]
pub async fn run_reverse_image_search(
    image_url: String,
    state: State<'_, AppState>,
) -> Result<TaskResult, String> {
    state
        .pipeline
        .run(
            Capability::ReverseImageSearch,
            TaskInput::ReverseImageSearch { image_url },
        )
        .await
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
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
pub async fn capture_region_cmd(args: RegionArgs, _app: AppHandle) -> Result<CapturedRegion, String> {
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

    let png = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let img = match args {
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

    let bundle = state
        .pipeline
        .run_capture_flow(png)
        .await
        .map_err(|e| e.to_string())?;

    overlay::show_centered(&app).ok();
    overlay::emit(
        &app,
        overlay::OverlayEvent::Result {
            source_text: bundle.source_text.clone(),
            result: bundle.result.clone(),
        },
    )
    .ok();
    Ok(bundle)
}

#[tauri::command]
pub async fn install_ocr_models(app: AppHandle) -> Result<String, String> {
    let dir = ocr_ocrs::default_models_dir().ok_or("no data dir")?;
    ocr_ocrs::install_models(&dir).await.map_err(|e| e.to_string())?;
    let _ = app; // reserved for future progress events
    Ok(dir.display().to_string())
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
    // The installer accepts either a directory laid out like an installed
    // plugin or a `.moonlens-plugin` zip. Directories are copied; zips are
    // left for a future enhancement (pulls in a `zip` crate).
    if src.is_dir() {
        let name = src
            .file_name()
            .ok_or("invalid archive path")?
            .to_string_lossy()
            .to_string();
        let target = dest.join(&name);
        copy_dir(&src, &target).map_err(|e| e.to_string())?;
        state.plugins.rescan().map_err(|e| e.to_string())?;
        state.pipeline.rebuild_providers();
        Ok(target.display().to_string())
    } else {
        Err("zip archives not yet supported; copy the plugin directory manually".into())
    }
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
