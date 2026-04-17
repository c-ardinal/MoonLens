//! Tauri application wiring.

use std::sync::Arc;

use anyhow::Result;
use tauri::Manager;

use crate::{
    hotkey, ipc, pipeline::Pipeline, plugins::PluginRegistry, settings::SettingsStore,
    storage::Storage,
};

macro_rules! all_commands {
    () => {
        tauri::generate_handler![
            ipc::get_settings,
            ipc::set_settings,
            ipc::get_settings_schema,
            ipc::list_plugins,
            ipc::set_plugin_enabled,
            ipc::run_translate,
            ipc::run_ocr,
            ipc::run_intent,
            ipc::run_qr,
            ipc::run_reverse_image_search,
            ipc::capture_region_cmd,
        ]
    };
}

/// Application-wide shared state. Kept as `Arc` fields so handles can be
/// cheaply cloned into async tasks and Tauri commands.
pub struct AppState {
    pub settings: Arc<SettingsStore>,
    pub storage: Arc<Storage>,
    pub plugins: Arc<PluginRegistry>,
    pub pipeline: Arc<Pipeline>,
}

pub fn build() -> Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = Arc::new(SettingsStore::load_or_default(&handle)?);
            let storage = Arc::new(Storage::open(&handle)?);
            let plugins = Arc::new(PluginRegistry::load(&handle, settings.clone())?);
            let pipeline = Arc::new(Pipeline::new(
                settings.clone(),
                storage.clone(),
                plugins.clone(),
            ));

            hotkey::register_all(&handle, settings.clone(), pipeline.clone())?;

            app.manage(AppState {
                settings,
                storage,
                plugins,
                pipeline,
            });
            Ok(())
        })
        .invoke_handler(all_commands!())
        .run(tauri::generate_context!())?;
    Ok(())
}
