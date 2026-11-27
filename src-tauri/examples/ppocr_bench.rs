//! PP-OCR smoke test / benchmark.
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
    // PPOCR_BACKEND=rten|onnx-cpu|directml pins the backend; without
    // it the fastest one that loads is chosen, as in the app.
    let want = ocr_ppocr::engine::Want {
        backend: ocr_ppocr::Backend::from_name(&std::env::var("PPOCR_BACKEND").unwrap_or_default()),
        gpu: true,
    };
    let engine = ocr_ppocr::engine::PpOcr::load(&dir, want)?;
    println!(
        "model load: {:?}, backend {}",
        t.elapsed(),
        engine.backend().name()
    );
    let img = image::open(&path)?;
    // Run 0 recognises every line; run 1 reads the same picture again,
    // so its lines come from the line cache and the time is detection
    // alone. Run 2 has a band of the picture changed: only the lines in
    // that band are recognised again. Run 3 inverts the whole picture:
    // every line is new again, now with the models warm.
    let mut first: Option<Vec<(String, [i32; 4])>> = None;
    for run in 0..4 {
        let mut frame = img.clone();
        if run == 2 {
            let (w, h) = (frame.width(), frame.height());
            let band = image::imageops::crop_imm(&img, 0, h / 2, w, h / 8).to_image();
            let mut band = image::DynamicImage::ImageRgba8(band);
            band.invert();
            image::imageops::replace(&mut frame, &band, 0, (h / 2) as i64);
        }
        if run == 3 {
            frame.invert();
        }
        let t = Instant::now();
        let lines = engine.recognize(&frame, &params)?;
        println!("run {run}: {} lines in {:?}", lines.len(), t.elapsed());
        let read: Vec<(String, [i32; 4])> = lines
            .iter()
            .map(|l| (l.text.clone(), l.bbox.map(|v| v.round() as i32)))
            .collect();
        match (&first, run) {
            (None, _) => first = Some(read),
            (Some(f), 1) => assert_eq!(f, &read, "cached lines differ from the first reading"),
            _ => {}
        }
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
