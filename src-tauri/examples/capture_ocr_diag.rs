//! Capture + OCR diagnostics on a real desktop.
//!
//! Grabs every monitor and a centred region of the first one with the
//! same capture functions the app uses, saves the PNGs, and runs the
//! intent classifier and the configured OCR chain (default settings,
//! models from the app's models folder) on each. Prints image stats,
//! OCR timing, provider, fallback reasons and the first lines found.
//!
//! ```sh
//! cd src-tauri
//! cargo run --release --no-default-features --features ocrs-engine \
//!     --example capture_ocr_diag -- <out-dir> [extra.png ...]
//! ```

use std::{path::PathBuf, sync::Arc, time::Instant};

use image::{DynamicImage, GenericImageView};
use moonlens_lib::{
    capture,
    pipeline::Pipeline,
    plugins::registry::PluginRegistry,
    providers::{ocr_ocrs, Capability, TaskInput, TaskOutput},
    settings::SettingsStore,
    storage::Storage,
};

fn stats(img: &DynamicImage) -> (f64, f64) {
    let g = img.to_luma8();
    let n = (g.width() * g.height()).max(1) as f64;
    let (mut s, mut s2) = (0.0, 0.0);
    for p in g.pixels() {
        let v = p[0] as f64;
        s += v;
        s2 += v * v;
    }
    let mean = s / n;
    (mean, (s2 / n - mean * mean).max(0.0).sqrt())
}

async fn probe(pipeline: &Pipeline, name: &str, img: &DynamicImage, out: &std::path::Path) {
    let (w, h) = img.dimensions();
    let (mean, std) = stats(img);
    let path = out.join(format!("{name}.png"));
    let png = capture::png_bytes(img).expect("png");
    std::fs::write(&path, &png).expect("write png");
    println!(
        "\n## {name}: {w}x{h}, luma mean {mean:.1} std {std:.1}, intent {:?} -> {}",
        moonlens_lib::intent::classify(img),
        path.display()
    );
    let t = Instant::now();
    match pipeline
        .run(
            Capability::Ocr,
            TaskInput::Ocr {
                image_png: png,
                languages: vec![],
            },
        )
        .await
    {
        Ok(r) => {
            let lines = match &r.output {
                TaskOutput::Ocr { lines } => lines.clone(),
                _ => vec![],
            };
            println!(
                "OCR ok in {} ms via {} (fallbacks {:?}): {} lines",
                t.elapsed().as_millis(),
                r.used_provider,
                r.fallback_reasons,
                lines.len()
            );
            for l in lines.iter().take(8) {
                println!("  {:?}", l.text);
            }
        }
        Err(e) => println!("OCR FAILED after {} ms: {e}", t.elapsed().as_millis()),
    }
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "capture-diag".into()));
    std::fs::create_dir_all(&out).unwrap();
    println!("models dir: {:?}", ocr_ocrs::default_models_dir());

    let tmp = std::env::temp_dir().join(format!("moonlens-diag-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    // MOONLENS_DIAG_CONFIG=<config.toml>: run with a copy of the user's
    // real settings instead of the defaults (the original is not touched).
    if let Some(cfg) = std::env::var_os("MOONLENS_DIAG_CONFIG") {
        std::fs::copy(&cfg, tmp.join("config.toml")).expect("copy config");
        println!("using a copy of {cfg:?}");
    }
    let store = Arc::new(SettingsStore::load_from(tmp.join("config.toml")).unwrap());
    let plugins = Arc::new(PluginRegistry::load_from(tmp.join("plugins"), store.clone()).unwrap());
    let snap = store.snapshot();
    println!(
        "ocr chain {:?} timeout {} ms; translate chain {:?}",
        snap.providers.ocr.chain, snap.providers.ocr.timeout_ms, snap.providers.translate.chain
    );
    let pipeline = Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins);

    match xcap::Monitor::all() {
        Ok(monitors) => {
            for (i, m) in monitors.iter().enumerate() {
                println!(
                    "monitor {i}: name {:?} origin ({:?},{:?}) size {:?}x{:?} scale {:?} primary {:?}",
                    m.name().ok(),
                    m.x().ok(),
                    m.y().ok(),
                    m.width().ok(),
                    m.height().ok(),
                    m.scale_factor().ok(),
                    m.is_primary().ok()
                );
                match m.capture_image() {
                    Ok(img) => {
                        probe(
                            &pipeline,
                            &format!("monitor{i}"),
                            &DynamicImage::ImageRgba8(img),
                            &out,
                        )
                        .await
                    }
                    Err(e) => println!("monitor {i}: capture FAILED: {e}"),
                }
            }
            if let Some(m) = monitors.first() {
                let (x, y) = (m.x().unwrap_or(0), m.y().unwrap_or(0));
                let (w, h) = (m.width().unwrap_or(0), m.height().unwrap_or(0));
                let (rw, rh) = (w.min(1200), h.min(600));
                let rx = x + ((w - rw) / 2) as i32;
                let ry = y + ((h - rh) / 2) as i32;
                match capture::capture_global_rect(rx, ry, rw, rh) {
                    Ok(img) => probe(&pipeline, "region-center", &img, &out).await,
                    Err(e) => println!("region capture FAILED: {e}"),
                }
            }
        }
        Err(e) => println!("list monitors FAILED: {e}"),
    }

    for extra in args {
        match image::open(&extra) {
            Ok(img) => {
                let name = format!(
                    "file-{}",
                    PathBuf::from(&extra)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                );
                probe(&pipeline, &name, &img, &out).await
            }
            Err(e) => println!("{extra}: open FAILED: {e}"),
        }
    }
}
