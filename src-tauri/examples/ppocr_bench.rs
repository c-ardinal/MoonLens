//! PP-OCRv5 smoke test / benchmark.
//!
//! ```sh
//! cargo run --example ppocr_bench -- path/to/screenshot.png [det_max_side]
//! ```
//!
//! Uses the multilingual models from the default data dir (install them
//! from the app first) and prints timings, lines and paragraph blocks.

#[cfg(feature = "ocrs-engine")]
fn main() -> anyhow::Result<()> {
    use moonlens_lib::{layout, providers::ocr_ppocr};
    use std::time::Instant;

    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: ppocr_bench <image> [det_max_side]");
    let mut params = ocr_ppocr::Params::default();
    if let Some(s) = args.next() {
        params.det_max_side = s.parse()?;
    }
    let dir = ocr_ppocr::default_models_dir().expect("no data dir");
    let t = Instant::now();
    let engine = ocr_ppocr::engine::PpOcr::load(&dir)?;
    println!("model load: {:?}", t.elapsed());
    let img = image::open(&path)?;
    for run in 0..2 {
        let t = Instant::now();
        let lines = engine.recognize(&img, &params)?;
        println!("run {run}: {} lines in {:?}", lines.len(), t.elapsed());
        if run == 1 {
            for l in &lines {
                println!(
                    "  {:?} {:.2} {}",
                    l.bbox.map(|v| v.round()),
                    l.confidence,
                    l.text
                );
            }
            let rgba = img.to_rgba8();
            for b in layout::group_lines(&lines) {
                let colors = layout::estimate_colors(&rgba, b.bbox);
                let stroke = layout::stroke_ratio(&rgba, b.bbox, colors, b.line_height);
                println!(
                    "BLOCK {:?} {:?} stroke={:?}\n  {}",
                    b.bbox.map(|v| v.round()),
                    b.align,
                    stroke,
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
