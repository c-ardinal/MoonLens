//! Tauri application wiring.

use std::sync::Arc;

use anyhow::Result;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::{
    hotkey, ipc,
    lens::{LensRegistry, LensRegistryHandle},
    lens_watcher::{LensStreamWatchers, LensStreamWatchersHandle},
    pipeline::Pipeline,
    plugins::PluginRegistry,
    region_watcher::RegionWatcher,
    settings::{SettingsStore, Watcher},
    storage::Storage,
    subtitle::SubtitleService,
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
            ipc::pick_and_install_plugin,
            ipc::uninstall_plugin,
            ipc::reveal_plugin_folder,
            ipc::check_plugin_updates,
            ipc::run_translate,
            ipc::run_ocr,
            ipc::run_intent,
            ipc::run_qr,
            ipc::run_explain,
            ipc::run_reverse_image_search,
            ipc::capture_region_cmd,
            ipc::capture_and_process,
            ipc::capture_file,
            ipc::pick_and_capture_file,
            ipc::list_windows,
            ipc::capture_window_by_id,
            ipc::pin_overlay,
            ipc::unpin_overlay,
            ipc::install_ocr_models,
            ipc::show_main,
            ipc::restart_app,
            ipc::hide_overlay,
            ipc::start_region_selection,
            ipc::cancel_region_selection,
            ipc::toggle_subtitle,
            ipc::subtitle_state,
            ipc::stop_subtitle,
            ipc::start_subtitle,
            ipc::test_asr_endpoint,
            ipc::get_onboarding_status,
            ipc::mark_macos_permissions_reviewed,
            ipc::open_macos_privacy_pane,
            ipc::macos_permissions_snapshot,
            ipc::macos_request_screen_capture,
            ipc::new_lens_window,
            ipc::close_lens_window,
            ipc::list_lens_windows,
            ipc::set_lens_insets,
            ipc::set_lens_auto_refresh,
            ipc::capture_lens_now,
            ipc::refresh_lens,
            ipc::list_history,
            ipc::delete_history_entry,
            ipc::clear_history,
            ipc::stop_all,
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
    pub subtitle: Arc<SubtitleService>,
    pub lenses: LensRegistryHandle,
    /// Per-lens streaming change detectors. Each entry is a polling
    /// task that captures the lens rect, hash-compares, and triggers
    /// the OCR + per-line translate flow when the content changes.
    pub lens_watchers: LensStreamWatchersHandle,
    /// Watcher for pinned overlay auto-refresh. Started when the user
    /// pins the overlay on a captured region; idle otherwise.
    pub region_watcher: Arc<RegionWatcher>,
    /// Background watcher that re-reads `config.toml` when the user
    /// edits it outside of the app. Held here so its thread stays alive
    /// for the lifetime of the Tauri AppHandle.
    pub _settings_watcher: Watcher,
}

pub fn build() -> Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = Arc::new(SettingsStore::load_or_default(&handle)?);
            let settings_watcher = Watcher::spawn(settings.clone())?;
            // Wire disk reloads to the parts of the app that need to
            // react (providers + hotkeys). `subscribe` returns a fresh
            // receiver each call so this does not starve any other
            // subscriber.
            let reload_handle = handle.clone();
            let mut reload_rx = settings.subscribe();
            tauri::async_runtime::spawn(async move {
                while let Ok(_snapshot) = reload_rx.recv().await {
                    let state = reload_handle.state::<AppState>();
                    state.inner().pipeline.rebuild_providers();
                    match hotkey::reapply(&reload_handle) {
                        Ok(report) => hotkey::notify_failures(&reload_handle, &report),
                        Err(e) => {
                            tracing::warn!("hotkey reapply after settings reload: {e}");
                        }
                    }
                }
            });
            let storage = Arc::new(Storage::open(&handle)?);
            let plugins = Arc::new(PluginRegistry::load(&handle, settings.clone())?);
            let pipeline = Arc::new(Pipeline::new(
                settings.clone(),
                storage.clone(),
                plugins.clone(),
            ));
            // Forward Ollama streaming chunks to the overlay webview
            // so the one-shot capture flow can render tokens live.
            // Lens windows use a different path: the lens_watcher
            // instantiates its own per-line OllamaTranslator with a
            // sink that emits `moonlens://lens-line-chunk` tagged by
            // line_index, so parallel line translations never collide.
            {
                let emit_handle = handle.clone();
                pipeline.set_chunk_sink(Some(Arc::new(move |delta, done| {
                    let _ = tauri::Emitter::emit(
                        &emit_handle,
                        "moonlens://overlay",
                        crate::overlay::OverlayEvent::Chunk { delta, done },
                    );
                })));
            }
            // Kick off provider warm-up in the background. OCR model
            // load (~60MB rten files + tensor init) can take several
            // seconds on first use; doing it at startup means the first
            // lens capture usually lands on a warm engine.
            {
                let pipeline_preload = pipeline.clone();
                tauri::async_runtime::spawn(async move {
                    pipeline_preload.preload_all().await;
                });
            }

            let subtitle = Arc::new(SubtitleService::new(settings.clone(), pipeline.clone()));
            let lenses: LensRegistryHandle = Arc::new(LensRegistry::new());
            let lens_watchers: LensStreamWatchersHandle = Arc::new(LensStreamWatchers::new());
            let region_watcher = Arc::new(RegionWatcher::new());

            // Register hotkeys best-effort: an accelerator held by another
            // process (IME, PowerToys, etc.) must NOT abort startup.
            // Collect failures and emit a UI event so the user can rebind
            // the offending shortcut from the settings page.
            let hotkey_report = hotkey::register_all(
                &handle,
                settings.clone(),
                pipeline.clone(),
                subtitle.clone(),
            )?;
            hotkey::notify_failures(&handle, &hotkey_report);
            build_tray(&handle)?;
            install_main_window_close_to_tray(&handle);

            app.manage(AppState {
                settings,
                storage,
                plugins,
                pipeline,
                subtitle,
                lenses,
                lens_watchers,
                region_watcher,
                _settings_watcher: settings_watcher,
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
    let lens = MenuItem::with_id(
        app,
        "new_lens",
        "Lens ウィンドウを新規作成",
        true,
        None::<&str>,
    )?;
    let subtitle = MenuItem::with_id(app, "subtitle", "字幕オーバーレイ", true, None::<&str>)?;
    let plugins = MenuItem::with_id(app, "plugins", "プラグイン", true, None::<&str>)?;
    let history = MenuItem::with_id(app, "history", "履歴", true, None::<&str>)?;
    let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open, &capture, &lens, &subtitle, &plugins, &history, &sep, &quit,
        ],
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
            "capture" => {
                if let Err(e) = crate::selector::show(app) {
                    tracing::warn!("selector show failed: {e:?}");
                }
            }
            "new_lens" => {
                let state = app.state::<AppState>();
                match crate::lens::create(app, &state.lenses) {
                    Ok(id) => {
                        // IMPORTANT: every creation path must start
                        // the watcher AND hook window events so move /
                        // resize triggers the re-capture flow. The
                        // position-polling fallback inside the watcher
                        // is a safety net for compositor paths that
                        // drop `Moved` events.
                        state.lens_watchers.start(
                            id.clone(),
                            app.clone(),
                            state.pipeline.clone(),
                            state.lenses.clone(),
                            state.settings.clone(),
                        );
                        if let Some(win) = app.get_webview_window(&id) {
                            let id_for_events = id.clone();
                            let watchers = state.lens_watchers.clone();
                            win.on_window_event(move |event| {
                                if matches!(
                                    event,
                                    tauri::WindowEvent::Moved(_)
                                        | tauri::WindowEvent::Resized(_)
                                ) {
                                    watchers.trigger(&id_for_events);
                                }
                            });
                        }
                    }
                    Err(e) => tracing::warn!("lens create failed: {e:?}"),
                }
            }
            "subtitle" => {
                let state = app.state::<AppState>();
                if let Err(e) = state.subtitle.toggle(app) {
                    tracing::warn!("subtitle toggle failed: {e:?}");
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
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Convert the main window's close button into "hide to tray".
///
/// The main window is created with `decorations: false`, and the
/// chromeless close button on the frontend calls `window.close()`. By
/// default that destroys the window — after which the tray menu's
/// "MoonLens を開く" silently no-ops because `get_webview_window("main")`
/// returns `None`. Intercepting `CloseRequested` here keeps the window
/// alive (just hidden) so subsequent tray clicks can re-show it.
///
/// The "終了" tray item still exits cleanly via `app.exit(0)`, which is
/// the only path that actually tears the process down.
fn install_main_window_close_to_tray(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let hidden = main.clone();
    main.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hidden.hide();
        }
    });
}

fn route_main(app: &AppHandle, path: &str) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.eval(format!(
            "location.hash = '#{path}'; location.pathname = '{path}';"
        ));
        let _ = w.show();
        let _ = w.set_focus();
    }
}
