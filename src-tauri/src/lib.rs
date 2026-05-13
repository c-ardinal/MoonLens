//! MoonLens library entry point.
//!
//! The binary crate delegates to `run()`; keeping the top-level logic in a
//! library makes it easier to embed in tests or alternate frontends.

pub mod app;
pub mod audio;
pub mod capture;
pub mod clipboard;
pub mod embedded_model_package;
pub mod hotkey;
pub mod intent;
pub mod ipc;
pub mod lens;
pub mod lens_inpaint;
pub mod lens_layout;
pub mod lens_pipeline;
pub mod lens_watcher;
pub mod macos_permissions;
pub mod model_registry;
pub mod overlay;
pub mod pipeline;
#[cfg(test)]
mod pipeline_tests;
pub mod plugins;
pub mod providers;
pub mod region_watcher;
pub mod selector;
pub mod services;
pub mod settings;
pub mod storage;
pub mod subtitle;
pub mod translation_router;

use tracing_subscriber::{fmt, EnvFilter};

pub fn run() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
    app::build().expect("failed to build Tauri app");
}
