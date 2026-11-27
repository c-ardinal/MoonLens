//! Reads pictures with the PP-OCR models of one folder and writes what
//! was read as JSON, for comparing model generations on the same
//! pictures.
//!
//! ```sh
//! cargo run --release --example ocr_eval -- <models dir> <out dir> <image>...
//! ```
//!
//! `PPOCR_BACKEND` pins the backend; `PPOCR_DET_THRESHOLD`,
//! `PPOCR_BOX_THRESHOLD`, `PPOCR_UNCLIP_RATIO`, `PPOCR_MIN_CONFIDENCE`
//! and `PPOCR_DET_MAX_SIDE` replace the defaults. Per picture
//! `<out dir>/<name>.json` holds the lines (text, box, confidence) and
//! the time of a reading in which every line is new and the models
//! are warm.

#[cfg(feature = "ocrs-engine")]
fn main() -> anyhow::Result<()> {
    use moonlens_lib::providers::ocr_ppocr::{self, engine};
    use std::path::PathBuf;
    use std::time::Instant;

    let mut args = std::env::args().skip(1);
    let usage = "usage: ocr_eval <models dir> <out dir> <image>...";
    let dir = PathBuf::from(args.next().expect(usage));
    let out = PathBuf::from(args.next().expect(usage));
    std::fs::create_dir_all(&out)?;

    let mut params = ocr_ppocr::Params::default();
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f32>().ok());
    if let Some(v) = env("PPOCR_DET_THRESHOLD") {
        params.det_threshold = v;
    }
    if let Some(v) = env("PPOCR_BOX_THRESHOLD") {
        params.box_threshold = v;
    }
    if let Some(v) = env("PPOCR_UNCLIP_RATIO") {
        params.unclip_ratio = v;
    }
    if let Some(v) = env("PPOCR_MIN_CONFIDENCE") {
        params.min_confidence = v;
    }
    if let Some(v) = env("PPOCR_DET_MAX_SIDE") {
        params.det_max_side = v as u32;
    }
    let want = engine::Want {
        backend: ocr_ppocr::Backend::from_name(&std::env::var("PPOCR_BACKEND").unwrap_or_default()),
        gpu: true,
    };
    let ocr = engine::PpOcr::load(&dir, want)?;
    eprintln!("backend {}", ocr.backend().name());

    for path in args {
        let path = PathBuf::from(path);
        let img = image::open(&path)?;
        // The inverted picture warms the models up without putting the
        // lines of the picture itself into the line cache.
        let mut warm = img.clone();
        warm.invert();
        ocr.recognize(&warm, &params)?;
        let t = Instant::now();
        let lines = ocr.recognize(&img, &params)?;
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let json = serde_json::json!({
            "image": name,
            "ms": ms,
            "lines": lines.iter().map(|l| serde_json::json!({
                "text": l.text, "box": l.bbox, "confidence": l.confidence,
            })).collect::<Vec<_>>(),
        });
        std::fs::write(
            out.join(format!("{name}.json")),
            serde_json::to_string_pretty(&json)?,
        )?;
        eprintln!("{name}: {} lines, {ms:.0} ms", lines.len());
    }
    Ok(())
}

#[cfg(not(feature = "ocrs-engine"))]
fn main() {
    eprintln!("built without the ocrs-engine feature");
}
