//! MoonLens library entry point.
//!
//! The binary crate delegates to `run()`; keeping the top-level logic in a
//! library makes it easier to embed in tests or alternate frontends.

pub mod app;
pub mod app_identity;
pub mod attach;
pub mod audio;
pub mod audio_apps;
pub mod capture;
pub mod clipboard;
pub mod follow;
pub mod foreign_window;
pub mod gpu;
pub mod hotkey;
pub mod intent;
pub mod ipc;
pub mod lang;
pub mod layout;
pub mod lens;
pub mod lens_control;
pub mod lens_drag;
pub mod lens_fit;
pub mod lens_shield;
pub mod lens_text;
pub mod macos_permissions;
pub mod notices;
pub mod overlay;
pub mod pipeline;
#[cfg(test)]
mod pipeline_tests;
pub mod plugins;
pub mod providers;
pub mod region_watcher;
pub mod scan_margin;
pub mod settings;
pub mod setup;
pub mod storage;
pub mod subtitle;
pub mod subtitle_filter;
pub mod subtitle_record;
pub mod subtitle_revise;
pub mod translate_plan;
pub mod translation_cache;
pub mod user_error;
#[cfg(windows)]
mod webview2;

use std::sync::OnceLock;

use tracing_subscriber::{fmt, prelude::*, reload, EnvFilter, Registry};

/// Swaps the log filter when Settings → privacy.log_level changes.
static LOG_FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

pub fn run() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (filter, handle) = reload::Layer::new(filter);
    // Release builds on Windows have no console, so stdout alone loses
    // every log line. Also write to `<local data>/io.moonlens.app/logs`.
    let file_layer = open_log_file().map(|f| {
        fmt::layer()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(f))
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(file_layer)
        .init();
    let _ = LOG_FILTER.set(handle);
    // First line of every run, so a log shows which build wrote it.
    tracing::info!(
        "MoonLens {} start (pid {}, exe {:?})",
        build_version(),
        std::process::id(),
        std::env::current_exe().unwrap_or_default()
    );
    // Release builds abort on panic; record it in the log file first.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info}");
        default_hook(info);
    }));
    #[cfg(windows)]
    if let Err(e) = tauri::webview_version() {
        tracing::error!("WebView2 runtime not found: {e}");
        webview2::explain_missing();
        return;
    }
    app::build().expect("failed to build Tauri app");
}

/// Package version plus the commit it was built from, e.g.
/// `0.1.0 (1a2b3c4d5)`.
pub fn build_version() -> String {
    match env!("MOONLENS_COMMIT") {
        "" => env!("CARGO_PKG_VERSION").to_string(),
        c => format!("{} ({c})", env!("CARGO_PKG_VERSION")),
    }
}

/// Log file size that triggers rotation at start-up (one old file kept).
const LOG_ROTATE_BYTES: u64 = 5 * 1024 * 1024;

/// `<local data>/io.moonlens.app/logs` (`%LOCALAPPDATA%` on Windows).
pub fn log_dir() -> Option<std::path::PathBuf> {
    providers::ocr_ocrs::local_data_base()
        .map(|b| b.join(providers::ocr_ocrs::APP_IDENTIFIER).join("logs"))
}

/// Open `moonlens.log` for appending, moving a file over
/// [`LOG_ROTATE_BYTES`] to `moonlens.log.1` first. `None` when the
/// folder cannot be created: logging then stays on stdout only.
fn open_log_file() -> Option<std::fs::File> {
    let dir = log_dir()?;
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("moonlens.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > LOG_ROTATE_BYTES) {
        let _ = std::fs::rename(&path, dir.join("moonlens.log.1"));
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()
}

/// Apply the configured log level. `RUST_LOG`, when set, still wins so
/// developers keep their own filter.
pub fn apply_log_level(level: settings::schema::LogLevel) {
    if std::env::var_os("RUST_LOG").is_some() {
        return;
    }
    let directive = match level {
        settings::schema::LogLevel::Trace => "trace",
        settings::schema::LogLevel::Debug => "debug",
        settings::schema::LogLevel::Info => "info",
        settings::schema::LogLevel::Warn => "warn",
        settings::schema::LogLevel::Error => "error",
    };
    if let Some(handle) = LOG_FILTER.get() {
        let _ = handle.modify(|f| *f = EnvFilter::new(directive));
    }
}
