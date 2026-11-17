//! MoonLens library entry point.
//!
//! The binary crate delegates to `run()`; keeping the top-level logic in a
//! library makes it easier to embed in tests or alternate frontends.

pub mod app;
pub mod audio;
pub mod capture;
pub mod clipboard;
pub mod hotkey;
pub mod intent;
pub mod ipc;
pub mod lang;
pub mod layout;
pub mod lens;
pub mod macos_permissions;
pub mod notices;
pub mod overlay;
pub mod pipeline;
#[cfg(test)]
mod pipeline_tests;
pub mod plugins;
pub mod providers;
pub mod region_watcher;
pub mod selector;
pub mod settings;
pub mod storage;
pub mod subtitle;
pub mod translation_cache;
pub mod user_error;

use std::sync::OnceLock;

use tracing_subscriber::{fmt, prelude::*, reload, EnvFilter, Registry};

/// Swaps the log filter when Settings → privacy.log_level changes.
static LOG_FILTER: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

pub fn run() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let (filter, handle) = reload::Layer::new(filter);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .init();
    let _ = LOG_FILTER.set(handle);
    app::build().expect("failed to build Tauri app");
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
