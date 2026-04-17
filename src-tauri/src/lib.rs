//! MoonLens library entry point.
//!
//! The binary crate delegates to `run()`; keeping the top-level logic in a
//! library makes it easier to embed in tests or alternate frontends.

pub mod app;
pub mod audio;
pub mod capture;
pub mod hotkey;
pub mod intent;
pub mod ipc;
pub mod overlay;
pub mod pipeline;
pub mod plugins;
pub mod providers;
pub mod settings;
pub mod storage;

use tracing_subscriber::{fmt, EnvFilter};

pub fn run() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
    app::build().expect("failed to build Tauri app");
}
