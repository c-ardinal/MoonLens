//! Run the real capture pipeline (OCR → layout → translation) on image
//! files and print each `CaptureFlowResult` as JSON, the same payload
//! the overlays render. Used to check the in-place layout end to end.
//!
//! ```sh
//! cd src-tauri
//! cargo run --release --example capture_flow_dump -- a.png b.png > out.json
//! ```

use std::sync::Arc;

use moonlens_lib::{
    pipeline::Pipeline, plugins::registry::PluginRegistry, settings::SettingsStore,
    storage::Storage,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `RUST_LOG=moonlens_lib=debug` prints the pipeline's timings to stderr.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init();
    let tmp = tempfile::tempdir()?;
    // `MOONLENS_TARGET=fr` translates into another language than the default.
    if let Ok(lang) = std::env::var("MOONLENS_TARGET") {
        std::fs::write(
            tmp.path().join("config.toml"),
            format!("[subtitle]\ntarget_language = \"{lang}\"\n"),
        )?;
    }
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml"))?);
    let plugins = Arc::new(PluginRegistry::load_from(
        tmp.path().join("plugins"),
        store.clone(),
    )?);
    let pipeline = Pipeline::new(store, Arc::new(Storage::open_in_memory()?), plugins);
    let mut out = serde_json::Map::new();
    for path in std::env::args().skip(1) {
        let png = std::fs::read(&path)?;
        let bundle = pipeline.run_capture_flow(png).await?;
        out.insert(path, serde_json::to_value(&bundle)?);
    }
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
