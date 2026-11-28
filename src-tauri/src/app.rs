//! Tauri application wiring.

use std::sync::Arc;

use anyhow::Result;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

use crate::tr;
use crate::{
    hotkey, ipc,
    lens::{LensRegistry, LensRegistryHandle},
    pipeline::Pipeline,
    plugins::PluginRegistry,
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
            ipc::get_default_settings,
            ipc::list_plugins,
            ipc::set_plugin_enabled,
            ipc::install_plugin_from_path,
            ipc::pick_and_install_plugin,
            ipc::check_plugin_updates,
            ipc::run_translate,
            ipc::record_translation,
            ipc::run_ocr,
            ipc::run_intent,
            ipc::run_qr,
            ipc::run_explain,
            ipc::run_reverse_image_search,
            ipc::capture_region_cmd,
            ipc::capture_and_process,
            ipc::translate_image,
            ipc::read_image_file,
            ipc::install_ocr_models,
            ipc::ocr_models_status,
            ipc::ocr_scripts_status,
            ipc::install_ocr_script,
            ipc::delete_ocr_script,
            ipc::list_llm_models,
            ipc::install_llm_model,
            ipc::accept_license,
            ipc::cancel_llm_download,
            ipc::delete_llm_model,
            ipc::select_llm_model,
            ipc::list_asr_models,
            ipc::install_asr_model,
            ipc::cancel_asr_download,
            ipc::delete_asr_model,
            ipc::select_asr_model,
            ipc::show_main,
            ipc::hide_overlay,
            ipc::toggle_subtitle,
            ipc::subtitle_state,
            ipc::stop_subtitle,
            ipc::block_subtitle_phrase,
            ipc::start_subtitle,
            ipc::start_conversation,
            ipc::open_subtitle_window,
            ipc::close_subtitle_window,
            ipc::set_subtitle_source,
            ipc::subtitle_session,
            ipc::list_audio_apps,
            ipc::get_onboarding_status,
            ipc::get_system_profile,
            ipc::complete_setup,
            ipc::mark_macos_permissions_reviewed,
            ipc::open_macos_privacy_pane,
            ipc::macos_permissions_snapshot,
            ipc::macos_request_screen_capture,
            ipc::open_lens,
            ipc::set_lens_language,
            ipc::set_lens_read_mode,
            ipc::set_lens_interactive,
            ipc::set_lens_operate,
            ipc::set_lens_text_dock,
            ipc::close_lens_window,
            ipc::list_lens_windows,
            crate::attach::list_attach_targets,
            crate::attach::list_window_apps,
            crate::attach::attach_window,
            crate::attach::detach_window,
            crate::attach::window_attachment,
            crate::layout_sets::list_layout_sets,
            crate::layout_sets::save_layout_set,
            crate::layout_sets::update_layout_set,
            crate::layout_sets::delete_layout_set,
            crate::layout_sets::put_layout_set,
            crate::layout_sets::restore_layout_set,
            crate::layout_sets::set_lens_view,
            crate::stream::stream_state,
            crate::stream::set_stream,
            crate::stream::set_stream_paint,
            crate::speech::speak,
            crate::speech::stop_speaking,
            crate::speech::list_voices,
            ipc::set_lens_insets,
            ipc::set_lens_continuous,
            ipc::refresh_lens,
            ipc::toggle_lens_pause,
            ipc::trigger_supported,
            ipc::capture_trigger,
            ipc::cancel_trigger_capture,
            ipc::lens_controls,
            ipc::fit_lens,
            ipc::restore_lens,
            ipc::capture_lens_now,
            ipc::list_history,
            ipc::delete_history_entry,
            ipc::restore_history_entry,
            ipc::get_subtitle_record,
            ipc::replace_subtitle_lines,
            ipc::export_subtitle_record,
            ipc::pick_input_file,
            ipc::start_file_subtitle,
            ipc::cancel_file_subtitle,
            ipc::file_subtitle_status,
            ipc::export_file_subtitle,
            ipc::clear_history,
            ipc::translation_cache_usage,
            ipc::clear_translation_cache,
            ipc::list_notices,
            ipc::dismiss_notice,
            ipc::list_config_issues,
            ipc::apply_config_fixes,
            ipc::open_external_url,
            ipc::open_main_route,
            ipc::hide_main,
            ipc::quit_app,
            ipc::data_locations,
            ipc::list_orphan_llm_files,
            ipc::delete_orphan_llm_file,
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
    pub file_subtitle: Arc<crate::file_subtitle::FileSubtitleService>,
    pub lenses: LensRegistryHandle,
    pub speech: Arc<crate::speech::SpeechService>,
    /// Background watcher that re-reads `config.toml` when the user
    /// edits it outside of the app. Held here so its thread stays alive
    /// for the lifetime of the Tauri AppHandle.
    pub _settings_watcher: Watcher,
}

/// Command-line flag for launching straight to the tray (e.g. from a
/// login item). Without it every launch shows the main window, so a user
/// who starts the app always sees it start.
pub const START_HIDDEN_FLAG: &str = "--hidden";

pub fn build() -> Result<()> {
    tauri::Builder::default()
        // Must be the first plugin: a second launch hands over to the
        // running instance and exits before anything else starts.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // A second launch exits and hands over to this (older) process.
            tracing::info!("second launch redirected to the running instance");
            show_main_window(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            crate::notices::attach(&handle);
            crate::i18n::apply(&SettingsStore::peek_ui_language(&handle));
            let settings = Arc::new(SettingsStore::load_or_default(&handle)?);
            crate::apply_log_level(settings.snapshot().privacy.log_level);
            crate::i18n::apply(&settings.snapshot().appearance.ui_language);
            let settings_watcher = Watcher::spawn(settings.clone())?;
            // Wire disk reloads to the parts of the app that need to
            // react (providers + hotkeys). `subscribe` returns a fresh
            // receiver each call so this does not starve any other
            // subscriber.
            // Before subscribing: a login item the OS switched off turns
            // the setting off here, and that save needs no reaction.
            crate::autostart::sync_at_startup(&settings);
            let mut launch_at_login = settings.snapshot().startup.launch_at_login;
            let reload_handle = handle.clone();
            let mut reload_rx = settings.subscribe();
            tauri::async_runtime::spawn(async move {
                while let Ok(snapshot) = reload_rx.recv().await {
                    crate::apply_log_level(snapshot.privacy.log_level);
                    if crate::i18n::apply(&snapshot.appearance.ui_language) {
                        retitle(&reload_handle);
                    }
                    if snapshot.startup.launch_at_login != launch_at_login {
                        launch_at_login = snapshot.startup.launch_at_login;
                        crate::autostart::sync_on_change(launch_at_login);
                    }
                    // Let every window (overlay, subtitle, lens) apply theme
                    // or font changes, whether saved here or edited on disk.
                    let _ = reload_handle.emit("moonlens://settings-changed", &*snapshot);
                    let state = reload_handle.state::<AppState>();
                    crate::lens::apply_capture_visibility(&reload_handle, &state.lenses);
                    if let Err(e) = state.storage.apply_retention(
                        snapshot.storage.retention_days,
                        snapshot.storage.max_entries,
                    ) {
                        tracing::warn!("history retention failed: {e}");
                    }
                    state.inner().pipeline.rebuild_providers();
                    let pipeline = state.inner().pipeline.clone();
                    tauri::async_runtime::spawn(async move { pipeline.warm_up().await });
                    if let Err(e) = hotkey::reapply(&reload_handle) {
                        tracing::warn!("hotkey reapply after settings reload: {e}");
                    }
                }
            });
            let storage = Arc::new(Storage::open_or_recover(&handle)?);
            {
                // History written before the retention settings changed.
                let st = settings.snapshot();
                if let Err(e) =
                    storage.apply_retention(st.storage.retention_days, st.storage.max_entries)
                {
                    tracing::warn!("history retention failed: {e}");
                }
            }
            let plugins = Arc::new(PluginRegistry::load(&handle, settings.clone())?);
            let pipeline = Arc::new(Pipeline::new(
                settings.clone(),
                storage.clone(),
                plugins.clone(),
            ));
            pipeline.preload_local_llm();
            // Forward LLM streaming chunks to the overlay webview so
            // the UI renders tokens in real time.
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
            {
                let pipeline = pipeline.clone();
                tauri::async_runtime::spawn(async move { pipeline.warm_up().await });
            }
            let subtitle = Arc::new(SubtitleService::new(
                settings.clone(),
                pipeline.clone(),
                storage.clone(),
            ));
            let file_subtitle = Arc::new(crate::file_subtitle::FileSubtitleService::new(
                settings.clone(),
                pipeline.clone(),
                storage.clone(),
            ));
            let lenses: LensRegistryHandle = Arc::new(LensRegistry::new());

            crate::model_idle::spawn(
                settings.clone(),
                pipeline.clone(),
                lenses.clone(),
                subtitle.clone(),
            );

            // A hotkey another app already owns must not stop MoonLens
            // from starting; the others still register.
            if let Err(e) = hotkey::register_all(&handle, settings.clone()) {
                tracing::warn!("{e}");
            }
            if let Err(e) = build_tray(&handle) {
                // Some Linux desktops have no tray; the main window and
                // the hotkeys still work.
                tracing::warn!("tray unavailable: {e}");
            }

            app.manage(crate::attach::Handle::default());
            let layout_sets = crate::layout_sets::LayoutSets::open(&storage.data_dir);
            app.manage(layout_sets.clone());
            app.manage(crate::stream::Handle::default());
            app.manage(AppState {
                settings,
                storage,
                plugins,
                pipeline,
                subtitle,
                file_subtitle,
                lenses,
                speech: Arc::new(crate::speech::SpeechService::new(handle.clone())),
                _settings_watcher: settings_watcher,
            });
            crate::layout_sets::watch(&handle, &layout_sets);
            if !std::env::args().any(|a| a == START_HIDDEN_FLAG) {
                show_main_window(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // The main window has no title bar of its own; closing it
            // (Alt+F4, Cmd+W, the close button) hides it to the tray
            // instead of destroying it, so the tray and hotkeys can bring
            // it back. Quit is in the tray menu and the command palette.
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
                // Coming back to the app after a while: the translation
                // model may have been freed (see `model_idle`).
                if let tauri::WindowEvent::Focused(true) = event {
                    if let Some(state) = window.app_handle().try_state::<AppState>() {
                        state.pipeline.preload_local_llm();
                    }
                }
            }
            // A window that is gone must not go on speaking.
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    state.speech.cancel(window.label());
                }
            }
            // A subtitle window closing by itself (Alt+F4; its × button
            // goes through `close_subtitle_window`): stop its recognition.
            if crate::subtitle::is_window(window.label()) {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    let app = window.app_handle();
                    if let Some(state) = app.try_state::<AppState>() {
                        state.subtitle.forget(app, window.label());
                    }
                }
            }
        })
        .invoke_handler(all_commands!())
        .run(tauri::generate_context!())?;
    Ok(())
}

/// Tray menu in the current display language.
fn tray_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>> {
    let item = |id: &str, text: String| MenuItem::with_id(app, id, text, true, None::<&str>);
    let open = item("open", tr!("app:show_moonlens"))?;
    let capture = item("capture", tr!("common:window_translation"))?;
    let pause = item("pause", tr!("common:window_translation_pause_resume"))?;
    let subtitle = item("subtitle", tr!("common:audio_translation_start_stop"))?;
    let plugins = item("plugins", tr!("common:extensions"))?;
    let history = item("history", tr!("common:history"))?;
    let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = item("quit", tr!("app:quit"))?;
    Ok(Menu::with_items(
        app,
        &[
            &open, &capture, &pause, &subtitle, &plugins, &history, &sep, &quit,
        ],
    )?)
}

/// Re-word the tray menu and the audio translation windows' titles after
/// the display language changed. Window contents follow
/// `moonlens://settings-changed`; translation windows have no visible
/// title (frameless, off the taskbar) and take the language when opened.
fn retitle(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id("moonlens-tray") {
        match tray_menu(app) {
            Ok(menu) => {
                let _ = tray.set_menu(Some(menu));
            }
            Err(e) => tracing::warn!("tray menu rebuild failed: {e}"),
        }
    }
    let title = crate::subtitle::window_title();
    for (label, w) in app.webview_windows() {
        if crate::subtitle::is_window(&label) {
            let _ = w.set_title(&title);
        }
    }
}

fn build_tray(app: &AppHandle) -> Result<()> {
    let menu = tray_menu(app)?;

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
            "capture" => crate::lens::open_new(app),
            "pause" => {
                let state = app.state::<AppState>();
                crate::lens::toggle_pause(app, &state.lenses);
            }
            "subtitle" => {
                let state = app.state::<AppState>();
                state.subtitle.toggle_checked(app);
            }
            "plugins" => route_main(app, "/settings/#section-plugins"),
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

pub(crate) fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Show the main window on `path` (e.g. `/history/` or
/// `/settings/#section-models`). The layout navigates client-side, so the
/// settings screen can still keep unsaved edits.
pub(crate) fn route_main(app: &AppHandle, path: &str) {
    let _ = app.emit_to("main", "moonlens://navigate", path);
    show_main_window(app);
}
