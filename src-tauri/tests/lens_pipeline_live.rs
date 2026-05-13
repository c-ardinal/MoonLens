//! Live end-to-end test for the Lens pipeline.
//!
//! Loads a real screenshot fixture, runs the OCR provider, then
//! translates every detected line via the same `OllamaTranslator`
//! the Lens watcher uses in production. Prints per-stage timings
//! and all intermediate output so a developer can iterate on the
//! pipeline without launching `tauri:dev` each time.
//!
//! Marked `#[ignore]` because it:
//!   1. Needs `text-detection.rten` / `text-recognition.rten` in the
//!      platform data dir (`$APPDATA/moonlens/models/` on Windows).
//!   2. Needs Ollama running on 127.0.0.1:11434 with the
//!      `gemma4:e2b` model pulled.
//! Run explicitly with:
//!
//! ```bash
//! cd src-tauri
//! cargo test --release --test lens_pipeline_live -- --ignored --nocapture
//! ```

use std::{path::PathBuf, sync::Arc, time::Instant};

use moonlens_lib::providers::{
    ocr_ocrs::OcrsProvider, translate_ollama::OllamaTranslator, Provider, TaskInput, TaskOutput,
};

fn fixture_path() -> PathBuf {
    // CARGO_MANIFEST_DIR is src-tauri/; the fixture lives in the
    // workspace-root `tests/input/` folder.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("tests")
        .join("input")
        .join("english0001.png")
}

#[tokio::test]
#[ignore = "live: requires ocrs models + running Ollama with gemma4:e2b"]
async fn lens_pipeline_on_english_fixture() {
    let path = fixture_path();
    assert!(
        path.exists(),
        "fixture missing at {}\n\
         create it by running the bench or dropping a screenshot there",
        path.display()
    );
    let png = std::fs::read(&path).expect("read fixture");
    println!("\n=== MoonLens live pipeline test ===");
    println!("fixture: {} ({} bytes)", path.display(), png.len());

    // ---------- OCR preload ----------
    let ocr = OcrsProvider::new("ocrs", &serde_json::Value::Null);
    let t = Instant::now();
    ocr.preload()
        .await
        .expect("ocrs preload (check models are installed)");
    println!("\nOCR preload      : {:>5} ms", t.elapsed().as_millis());

    // ---------- OCR run ----------
    let t = Instant::now();
    let ocr_out = ocr
        .run(TaskInput::Ocr {
            image_png: png.clone(),
            languages: vec![],
        })
        .await
        .expect("ocr run");
    println!("OCR run          : {:>5} ms", t.elapsed().as_millis());

    let lines = match ocr_out {
        TaskOutput::Ocr { lines } => lines,
        _ => panic!("unexpected OCR output variant"),
    };
    assert!(!lines.is_empty(), "no OCR lines detected in fixture");
    println!("\nOCR detected {} line(s):", lines.len());
    for (i, line) in lines.iter().enumerate() {
        let [x, y, w, h] = line.bbox;
        println!(
            "  [{i:>2}] bbox=[{x:>4.0},{y:>4.0},{w:>4.0},{h:>4.0}]  text={:?}",
            line.text
        );
    }

    // ---------- Per-line translate, streaming ----------
    println!("\nPer-line translate -> Japanese:");
    for (i, line) in lines.iter().enumerate() {
        let streamed: Arc<parking_lot::Mutex<String>> =
            Arc::new(parking_lot::Mutex::new(String::new()));
        let streamed_sink = streamed.clone();
        let sink: Arc<dyn Fn(String, bool) + Send + Sync> = Arc::new(move |delta, _done| {
            if !delta.is_empty() {
                streamed_sink.lock().push_str(&delta);
            }
        });
        let translator =
            OllamaTranslator::new("ollama.gemma4", &serde_json::Value::Null).with_chunk_sink(sink);
        let t = Instant::now();
        let out = translator
            .run(TaskInput::Translate {
                text: line.text.clone(),
                source_language: "auto".into(),
                target_language: "ja".into(),
            })
            .await
            .unwrap_or_else(|e| panic!("translate line {i} failed: {e}"));
        let elapsed = t.elapsed().as_millis();
        match out {
            TaskOutput::Text(final_text) => {
                let streamed_text = streamed.lock().clone();
                assert!(
                    !final_text.trim().is_empty(),
                    "empty translation for line {i} ({:?})",
                    line.text
                );
                println!(
                    "  [{i:>2}] {:>5} ms | streamed {:>3} chars | final: {}",
                    elapsed,
                    streamed_text.len(),
                    final_text.trim()
                );
            }
            other => panic!("translator returned non-text: {other:?}"),
        }
    }
    println!();
}
