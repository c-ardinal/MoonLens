//! Tauri command handlers exposed to the Svelte frontend.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::{
    app::AppState,
    capture::{self, CaptureRect},
    intent,
    pipeline::TaskResult,
    plugins::PluginDescriptor,
    providers::{Capability, TaskInput},
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
pub async fn set_settings(new: Settings, state: State<'_, AppState>) -> Result<(), String> {
    state.settings.save(new).map_err(|e| e.to_string())?;
    state.pipeline.rebuild_providers();
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
    let img = capture::capture_rect(CaptureRect {
        monitor_index: args.monitor_index,
        x: args.x,
        y: args.y,
        w: args.w,
        h: args.h,
    })
    .map_err(|e| e.to_string())?;
    let image_png = capture::png_bytes(&img).map_err(|e| e.to_string())?;
    Ok(CapturedRegion { image_png })
}
