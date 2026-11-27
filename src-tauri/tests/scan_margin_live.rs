//! A translation window whose frame cuts the text, read with and
//! without the scan margin, through the real PP-OCR engine.
//!
//! The window sits a few pixels inside the paragraph of
//! `english0002.png`: its frame cuts the first and last letters of
//! every row and the top of the first row. The line above the
//! paragraph (a source note) lies outside the window, inside the margin.
//!
//! Asserted:
//!
//! 1. Read to the frame only, words at the frame are lost or misread.
//! 2. Read with the margin, every word of the paragraph is recognised.
//! 3. The line next to the window is not part of the reading.
//! 4. Every position stays inside the window.
//!
//! `#[ignore]`: needs the PP-OCR models in the app's models folder. With
//! `SCAN_MARGIN_TRANSLATE=1` both readings are translated as well (needs
//! `plamo-3-translate-2b`) and printed for comparison.
//!
//! ```sh
//! cd src-tauri
//! cargo test --release --test scan_margin_live -- --ignored --nocapture
//! ```

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use moonlens_lib::{
    capture::{self, rect::GlobalRect},
    pipeline::{CaptureFlowResult, Pipeline},
    plugins::registry::PluginRegistry,
    scan_margin,
    settings::SettingsStore,
    storage::Storage,
};

/// The window, in pixels of the fixture image (the "screen").
const WINDOW: GlobalRect = GlobalRect {
    x: 34,
    y: 112,
    w: 832,
    h: 120,
};

/// `performance.scan_margin_px` default, at 100 % scaling.
const MARGIN: u32 = 32;

fn pipeline() -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins)
}

fn words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

/// Words of `truth` missing from `read`.
fn missing(truth: &str, read: &str) -> Vec<String> {
    let have: HashSet<String> = words(read).into_iter().collect();
    let mut gone: Vec<String> = words(truth)
        .into_iter()
        .filter(|w| !have.contains(w))
        .collect();
    gone.dedup();
    gone
}

fn text(r: &CaptureFlowResult) -> String {
    r.blocks
        .iter()
        .map(|b| b.source.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: needs the PP-OCR models"]
async fn text_cut_by_the_frame_is_read_whole_with_the_margin() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lens");
    let screen = image::open(dir.join("english0002.png")).unwrap();
    let truth: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("english0002.json")).unwrap()).unwrap();
    let paragraph = truth["lines"][3].as_str().unwrap();
    let pipeline = pipeline();

    // What a monitor does to a capture rect: clip it to the screen.
    let grab = |r: GlobalRect| {
        let x0 = r.x.max(0) as u32;
        let y0 = r.y.max(0) as u32;
        let x1 = ((r.x + r.w as i32) as u32).min(screen.width());
        let y1 = ((r.y + r.h as i32) as u32).min(screen.height());
        let covered = GlobalRect {
            x: x0 as i32,
            y: y0 as i32,
            w: x1 - x0,
            h: y1 - y0,
        };
        let png = capture::png_bytes(&screen.crop_imm(x0, y0, x1 - x0, y1 - y0)).unwrap();
        (png, covered)
    };

    let (png, _) = grab(WINDOW);
    let mut exact = pipeline.analyze_capture(png).await.unwrap();

    let (png, covered) = grab(scan_margin::widened(WINDOW, MARGIN));
    let visible = scan_margin::visible_in(WINDOW, covered);
    let mut wide = pipeline
        .analyze_capture_in(png, Some(visible))
        .await
        .unwrap();

    let (exact_missing, wide_missing) = (
        missing(paragraph, &text(&exact)),
        missing(paragraph, &text(&wide)),
    );
    println!("--- frame only ---\n{}", text(&exact));
    println!("missing: {exact_missing:?}");
    println!("--- with a {MARGIN} px margin ---\n{}", text(&wide));
    println!("missing: {wide_missing:?}");

    assert!(
        !exact_missing.is_empty(),
        "the fixture window no longer cuts any word"
    );
    assert!(
        wide_missing.is_empty(),
        "words lost with the margin: {wide_missing:?}"
    );
    assert!(
        !text(&wide).contains("Wikipedia"),
        "the line above the window was read into it"
    );
    for b in &wide.blocks {
        for [x, y, w, h] in b.lines.iter().chain([&b.bbox]).copied() {
            assert!(
                x >= 0.0 && y >= 0.0 && x + w <= WINDOW.w as f32 && y + h <= WINDOW.h as f32,
                "outside the window: {:?}",
                [x, y, w, h]
            );
        }
    }

    if std::env::var_os("SCAN_MARGIN_TRANSLATE").is_some() {
        pipeline.translate_capture(&mut exact).await.unwrap();
        pipeline.translate_capture(&mut wide).await.unwrap();
        let shown = |r: &CaptureFlowResult| {
            r.blocks
                .iter()
                .filter_map(|b| b.translation.clone())
                .collect::<Vec<_>>()
                .join("\n")
        };
        println!("--- translation, frame only ---\n{}", shown(&exact));
        println!("--- translation, with the margin ---\n{}", shown(&wide));
    }
}
