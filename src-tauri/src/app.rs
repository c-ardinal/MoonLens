//! Tauri application wiring.

use std::sync::Arc;

use anyhow::Result;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

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
            ipc::install_plugin_from_path,
            ipc::run_translate,
            ipc::run_ocr,
            ipc::run_intent,
            ipc::run_qr,
            ipc::run_reverse_image_search,
            ipc::capture_region_cmd,
            ipc::capture_and_process,
            ipc::install_ocr_models,
            ipc::show_main,
            ipc::hide_overlay,
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
            build_tray(&handle)?;

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

fn build_tray(app: &AppHandle) -> Result<()> {
    let open = MenuItem::with_id(app, "open", "MoonLens を開く", true, None::<&str>)?;
    let capture = MenuItem::with_id(app, "capture", "キャプチャ (⌘⇧Space)", true, None::<&str>)?;
    let subtitle = MenuItem::with_id(app, "subtitle", "字幕オーバーレイ", true, None::<&str>)?;
    let plugins = MenuItem::with_id(app, "plugins", "プラグイン", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "履歴", true, None::<&str>)?;
    let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&open, &capture, &subtitle, &plugins, &history, &sep, &quit],
    )?;

    TrayIconBuilder::with_id("moonlens-tray")
        .tooltip("MoonLens")
        .menu(&menu)
        .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
            // Fall back to a 1x1 so the tray still appears on platforms
            // that require a non-null icon.
            tauri::image::Image::new_owned(vec![0, 0, 0, 0], 1, 1)
        }))
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "capture" => fire_capture(app),
            "subtitle" => {
                if let Some(w) = app.get_webview_window("subtitle") {
                    let _ = w.show();
                }
            }
            "plugins" => route_main(app, "/plugins/"),
            "history" => route_main(app, "/history/"),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn route_main(app: &AppHandle, path: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(&format!("location.hash = '#{path}'; location.pathname = '{path}';"));
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn fire_capture(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if let Err(e) = run_capture_from_tray(&app, state.pipeline.clone()).await {
            tracing::warn!("tray capture failed: {e:?}");
        }
    });
}

async fn run_capture_from_tray(app: &AppHandle, pipeline: Arc<Pipeline>) -> Result<()> {
    use crate::capture::{self, CaptureRect};
    use crate::overlay::{self, OverlayEvent};

    overlay::emit(app, OverlayEvent::Processing {
        message: "キャプチャ中…".into(),
    })
    .ok();

    let png = tokio::task::spawn_blocking(|| -> Result<Vec<u8>> {
        let monitors = xcap::Monitor::all()?;
        let monitor = monitors.first().ok_or_else(|| anyhow::anyhow!("no monitor"))?;
        let img = capture::capture_rect(CaptureRect {
            monitor_index: 0,
            x: 0,
            y: 0,
            w: monitor.width(),
            h: monitor.height(),
        })?;
        capture::png_bytes(&img)
    })
    .await??;

    let bundle = pipeline
        .run_capture_flow(png)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    overlay::show_centered(app)?;
    overlay::emit(
        app,
        OverlayEvent::Result {
            source_text: bundle.source_text,
            result: bundle.result,
        },
    )
    .ok();
    Ok(())
}
