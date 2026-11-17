//! OCR smoke test / benchmark against a real image.
//!
//! ```sh
//! cargo run --example ocr_bench -- path/to/screenshot.png [upscale]
//! ```
//!
//! Loads the ocrs models from the default data dir (install them from
//! the app first), runs the same recognize + layout code the capture
//! flow uses, and prints timings, lines and paragraph blocks.

#[cfg(feature = "ocrs-engine")]
fn main() -> anyhow::Result<()> {
    use moonlens_lib::{layout, providers::ocr_ocrs};
    use std::time::Instant;

    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: ocr_bench <image> [upscale]");
    let upscale: f32 = args.next().map(|s| s.parse().unwrap()).unwrap_or(0.0);
    let dir = ocr_ocrs::default_models_dir().expect("no data dir");

    let t = Instant::now();
    let engine = ocr_ocrs::load_engine(&dir)?;
    println!("model load: {:?}", t.elapsed());

    let img = image::open(&path)?;
    for run in 0..2 {
        let t = Instant::now();
        let lines = ocr_ocrs::recognize(&engine, &img, upscale)?;
        println!("run {run}: {} lines in {:?}", lines.len(), t.elapsed());
        if run == 1 {
            for l in &lines {
                println!("  {:?} {}", l.bbox.map(|v| v.round()), l.text);
            }
            let rgba = img.to_rgba8();
            for b in layout::group_lines(&lines) {
                let c = layout::estimate_colors(&rgba, b.bbox);
                println!(
                    "BLOCK {:?} lh={} bg={:?} fg={:?}\n  {}",
                    b.bbox.map(|v| v.round()),
                    b.line_height,
                    c.background,
                    c.foreground,
                    b.text
                );
            }
        }
    }
    Ok(())
}

#[cfg(not(feature = "ocrs-engine"))]
fn main() {
    eprintln!("build with the `ocrs-engine` feature");
}
