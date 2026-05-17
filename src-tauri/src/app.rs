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
    services::{FileSystemStorage, HttpModelDownloader, ModelService, SysinfoProbe},
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
            #[cfg(feature = "paddleocr-engine")]
            ipc::install_paddleocr_models,
            #[cfg(feature = "paddleocr-engine")]
            ipc::paddleocr_models_status,
            ipc::embedded_model_status,
            ipc::download_embedded_model,
            ipc::package_embedded_model,
            ipc::inspect_embedded_model_package,
            ipc::apply_embedded_model_package,
            ipc::pick_and_package_embedded_model,
            ipc::pick_and_apply_embedded_model_package,
            ipc::list_model_tiers,
            ipc::model_tier_status,
            ipc::set_model_tier,
            ipc::get_user_tier,
            ipc::set_user_tier,
            ipc::clear_user_tier,
            ipc::preview_routing,
            ipc::routed_model_status,
            ipc::user_tier_sizes,
            ipc::download_routed_model,
            ipc::bench_embedded_translate,
            ipc::show_main,
            ipc::show_main_route,
            ipc::hide_launcher,
            ipc::add_history_entry,
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
            ipc::mark_embedded_model_setup_seen,
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

/// Concrete `ModelService` type with the production adapters
/// pre-applied. Kept aliased so `AppState` and IPC commands can refer
/// to it without spelling the three trait params each time.
pub type AppModelService = ModelService<HttpModelDownloader, FileSystemStorage, SysinfoProbe>;

/// Application-wide shared state. Kept as `Arc` fields so handles can be
/// cheaply cloned into async tasks and Tauri commands.
pub struct AppState {
    pub settings: Arc<SettingsStore>,
    pub storage: Arc<Storage>,
    pub plugins: Arc<PluginRegistry>,
    pub pipeline: Arc<Pipeline>,
    /// Embedded-model service. Aggregates downloader / storage /
    /// hardware probe behind a single API the IPC layer thinly
    /// wraps. Tests instantiate `ModelService` with mock adapters
    /// instead — the production wiring here uses HTTP + real FS +
    /// sysinfo.
    pub models: Arc<AppModelService>,
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
            // One-shot legacy-settings migration. Bundled migrations
            // applied here:
            //   1. 12.x rename: `embedded.gemma` → `embedded.llm`.
            //      The provider id was originally named after the
            //      Gemma model family; the 2026-05-09 switch to Qwen
            //      2.5 made it misleading, so it was generalised to
            //      `embedded.llm`.
            //   2. 9.x synthetic-filename rewrite for the multi-part
            //      7B Q4_K_M tier — the pre-9.x registry recorded a
            //      single non-existent filename and downloads 404'd.
            // Both migrations run before the pipeline materialises
            // the embedded provider so the next translate / download
            // sees the corrected coord without the user having to
            // re-pick the tier from the dropdown.
            {
                let mut snap = (*settings.snapshot()).clone();
                if crate::model_registry::migrate_legacy_embedded_settings(&mut snap) {
                    if let Err(e) = settings.save(snap) {
                        tracing::warn!("embedded settings migration: save failed: {e}");
                    } else {
                        tracing::info!("embedded settings migration: rewrote legacy id / filename");
                    }
                }
            }
            let plugins = Arc::new(PluginRegistry::load(&handle, settings.clone())?);
            let pipeline = Arc::new(Pipeline::new(
                settings.clone(),
                storage.clone(),
                plugins.clone(),
            ));
            // Resolve `<app-data>/moonlens/` once so the embedded
            // LLM provider knows where to look up `models/<file>.gguf`
            // without hitting the Tauri API on every translate call.
            // `path().app_data_dir()` returns the OS-specific folder
            // (`%APPDATA%\moonlens` on Windows, `~/.config/moonlens`
            // on Linux, `~/Library/Application Support/moonlens` on
            // macOS) so the resolved path matches `config.toml`'s
            // existing layout.
            if let Ok(dir) = handle.path().app_data_dir() {
                pipeline.set_app_data_dir(dir);
            }
            // Forward streaming translation chunks to the overlay
            // webview so the one-shot capture flow can render tokens
            // live. The sink is provider-agnostic now: any
            // `Provider::run_streaming` implementation that emits
            // deltas (Ollama, embedded LLM, plugins, ...) lights up
            // this path. Lens windows use a parallel path: the
            // lens_watcher invokes `Pipeline::run_with_sink` with a
            // per-line sink that emits `moonlens://lens-line-chunk`
            // tagged by line_index, so concurrent line translations
            // never collide.
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
            // Forward embedded model load lifecycle events to a
            // dedicated channel so the layout shell can render a
            // "ローカル LLM をロード中…" badge during the cold-start
            // (and clear it on completion or failure). Without this,
            // the multi-second `Model::build` looks like an unexplained
            // freeze on the first translate call and the first call
            // after every model swap.
            {
                let emit_handle = handle.clone();
                pipeline.set_embedded_load_notifier(Some(std::sync::Arc::new(move |event| {
                    use crate::providers::translate_embedded_llm::EmbeddedLoadEvent;
                    let payload = match event {
                        EmbeddedLoadEvent::Started {
                            provider_id,
                            model_path,
                        } => serde_json::json!({
                            "phase": "started",
                            "provider_id": provider_id,
                            "model_path": model_path.display().to_string(),
                        }),
                        EmbeddedLoadEvent::Finished {
                            provider_id,
                            model_path,
                            success,
                            error,
                        } => serde_json::json!({
                            "phase": "finished",
                            "provider_id": provider_id,
                            "model_path": model_path.display().to_string(),
                            "success": success,
                            "error": error,
                        }),
                    };
                    let _ = tauri::Emitter::emit(
                        &emit_handle,
                        "moonlens://embedded-model-loading",
                        payload,
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

            // Build the embedded-model service with the production
            // adapters. Tests build the same struct with mock
            // adapters; the IPC layer treats the two identically
            // because everything goes through the trait.
            let app_data_dir = handle
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            let models = Arc::new(ModelService {
                downloader: HttpModelDownloader::new(),
                storage: FileSystemStorage::new(app_data_dir.join("models")),
                probe: SysinfoProbe,
                settings: settings.clone(),
                pipeline: Some(pipeline.clone()),
                app_data_dir,
                exporter_version: env!("CARGO_PKG_VERSION").to_string(),
            });

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
            build_tray(&handle, settings.clone())?;
            install_main_window_close_to_tray(&handle);

            app.manage(AppState {
                settings,
                storage,
                plugins,
                pipeline,
                models,
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

fn build_tray(app: &AppHandle, settings: Arc<SettingsStore>) -> Result<()> {
    // Match the Claude Design 2026.05 tray spec: ordered action groups
    // separated by hairlines. Group 1 is quick actions (capture / lens /
    // subtitle / clipboard), group 2 is navigation into the main app,
    // group 3 is the destructive quit.
    //
    // Accelerator labels are built from the LIVE settings snapshot so
    // a user who has remapped (e.g.) the capture shortcut sees their
    // own keys in the tray menu — not a hard-coded `⌘⇧Space` that
    // would silently lie. Items without a bound shortcut omit the
    // accelerator string entirely; an empty label after a tab would
    // render an awkward trailing whitespace block.
    let snap = settings.snapshot();
    let label = |text: &str, accel: &str| -> String {
        let formatted = format_accelerator(accel);
        if formatted.is_empty() {
            text.to_string()
        } else {
            format!("{text}\t{formatted}")
        }
    };

    let open = MenuItem::with_id(app, "open", "MoonLens を開く", true, None::<&str>)?;
    let header_sep = tauri::menu::PredefinedMenuItem::separator(app)?;

    // Quick actions — first group.
    let capture = MenuItem::with_id(
        app,
        "capture",
        label("キャプチャ翻訳", &snap.hotkeys.capture_region),
        true,
        None::<&str>,
    )?;
    let lens = MenuItem::with_id(
        app,
        "new_lens",
        label("Lensウィンドウを開く", &snap.hotkeys.new_lens),
        true,
        None::<&str>,
    )?;
    let subtitle = MenuItem::with_id(
        app,
        "subtitle",
        label("音声字幕を開始", &snap.hotkeys.toggle_subtitle),
        true,
        None::<&str>,
    )?;
    let launcher = MenuItem::with_id(
        app,
        "launcher",
        label("クリップボードを翻訳", &snap.hotkeys.open_command_palette),
        true,
        None::<&str>,
    )?;
    let actions_sep = tauri::menu::PredefinedMenuItem::separator(app)?;

    // Navigation — second group.
    let history = MenuItem::with_id(
        app,
        "history",
        label("履歴を開く", &snap.hotkeys.open_history),
        true,
        None::<&str>,
    )?;
    let plugins = MenuItem::with_id(app, "plugins", "プラグイン", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(
        app,
        "settings",
        label("設定…", &snap.hotkeys.open_settings),
        true,
        None::<&str>,
    )?;
    let nav_sep = tauri::menu::PredefinedMenuItem::separator(app)?;

    // Destructive — third group.
    let quit = MenuItem::with_id(app, "quit", "MoonLens を終了", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &open,
            &header_sep,
            &capture,
            &lens,
            &subtitle,
            &launcher,
            &actions_sep,
            &history,
            &plugins,
            &settings_item,
            &nav_sep,
            &quit,
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
                                    tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
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
            "launcher" => {
                if let Err(e) = crate::launcher::show(app) {
                    tracing::warn!("launcher show failed: {e:?}");
                }
            }
            "plugins" => route_main(app, "/plugins/"),
            "history" => route_main(app, "/history/"),
            "settings" => route_main(app, "/settings/"),
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

/// Convert a Tauri accelerator string like `"CommandOrControl+Shift+Space"`
/// into the compact glyph form (`"⌘⇧Space"`) used by the tray and
/// command palette. Mirrors `src/lib/hotkey.ts::formatAccelerator` so
/// the same shortcut renders identically in the tray, the in-app
/// command palette, and the keyboard hint chips throughout the
/// sidebar / home / launcher.
///
/// Returns an empty string for empty / whitespace input so the tray
/// label builder can omit the accelerator column rather than render
/// an awkward trailing tab.
fn format_accelerator(accel: &str) -> String {
    let trimmed = accel.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    trimmed
        .split('+')
        .map(|part| match part {
            "CommandOrControl" | "Cmd" | "Command" | "Meta" | "Super" => "⌘",
            "Control" | "Ctrl" => "⌃",
            "Option" | "Alt" => "⌥",
            "Shift" => "⇧",
            "Escape" => "Esc",
            "Return" | "Enter" => "↵",
            "Tab" => "⇥",
            "Backspace" => "⌫",
            "ArrowUp" => "↑",
            "ArrowDown" => "↓",
            "ArrowLeft" => "←",
            "ArrowRight" => "→",
            other => other,
        })
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_accelerator_mirrors_frontend_format() {
        // Matches the JS test in src/lib/hotkey.test.ts so the tray
        // and the in-app chip can never visually drift.
        assert_eq!(format_accelerator("CommandOrControl+Shift+Space"), "⌘⇧Space");
        assert_eq!(format_accelerator("Control+K"), "⌃K");
        assert_eq!(format_accelerator("Shift+F1"), "⇧F1");
        assert_eq!(format_accelerator("Control+ArrowLeft"), "⌃←");
    }

    #[test]
    fn format_accelerator_empty_inputs_collapse() {
        assert_eq!(format_accelerator(""), "");
        assert_eq!(format_accelerator("   "), "");
        assert_eq!(format_accelerator("\t\n"), "");
    }
}
