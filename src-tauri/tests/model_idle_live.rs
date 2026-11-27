//! Unused models are freed and come back: the real translation model
//! and the real text recogniser go through [`model_idle::sweep`], the
//! call the app makes every few seconds.
//!
//! Shown per model: memory held, memory given back, and what the next
//! request pays for loading again (the number that decides whether
//! freeing is worth it).
//!
//! `#[ignore]` because the tests need the downloaded models in the
//! app's models folder (see `lens_images_live.rs`): one test per
//! model, so each runs with what is installed. Run with
//!
//! ```sh
//! cd src-tauri
//! cargo test --release --test model_idle_live -- --ignored --nocapture
//! ```

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use moonlens_lib::{
    model_idle::{self, Waiting},
    pipeline::Pipeline,
    plugins::registry::PluginRegistry,
    providers::{Capability, TaskInput, TaskOutput, TranslateContext},
    settings::SettingsStore,
    storage::Storage,
};

fn pipeline() -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins)
}

/// Resident memory of this process in MiB.
fn resident_mib() -> u64 {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(std::process::id());
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map(|p| p.memory()).unwrap_or(0) / (1024 * 1024)
}

async fn translate(pipeline: &Pipeline, text: &str) -> (String, Duration) {
    let t = Instant::now();
    let out = pipeline
        .run(
            Capability::Translate,
            TaskInput::Translate {
                text: text.into(),
                source_language: "en".into(),
                target_language: "ja".into(),
                context: TranslateContext::default(),
            },
        )
        .await
        .expect("translation");
    let TaskOutput::Text(s) = out.output else {
        panic!("translation returned no text");
    };
    (s, t.elapsed())
}

async fn read(pipeline: &Pipeline, png: &[u8]) -> (usize, Duration) {
    let t = Instant::now();
    let out = pipeline
        .run(
            Capability::Ocr,
            TaskInput::Ocr {
                image_png: png.to_vec(),
                languages: vec![],
            },
        )
        .await
        .expect("reading");
    let TaskOutput::Ocr { lines } = out.output else {
        panic!("reading returned no lines");
    };
    (lines.len(), t.elapsed())
}

const LIMIT: Duration = Duration::from_millis(300);

const WINDOW_OPEN: Waiting = Waiting {
    windows: true,
    subtitles: false,
};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: needs the plamo-3-translate-2b model"]
async fn unused_translation_model_is_freed_and_loads_again() {
    let pipeline = pipeline();
    let model = pipeline
        .local_llm_path()
        .expect("built-in translator in the chain");
    assert!(model.is_file(), "model missing: {}", model.display());
    // Different sentences: the translation cache must not answer.
    let texts = [
        "The north wind and the sun argued about which of them was stronger.",
        "A traveller came along the road, wrapped in a warm cloak.",
        "The harder the wind blew, the tighter the traveller held his cloak.",
        "Then the sun shone out warmly, and the traveller took off his cloak.",
        "And so the north wind had to admit that the sun was the stronger.",
    ];

    let base = resident_mib();
    let (first, cold) = translate(&pipeline, texts[0]).await;
    let (_, warm) = translate(&pipeline, texts[1]).await;
    let loaded = resident_mib();
    println!("first {cold:.2?} (with load), loaded {warm:.2?} -> {first}");
    println!("memory: {base} MiB before, {loaded} MiB with the model loaded");

    // An open translation window keeps it, however long nothing ran.
    tokio::time::sleep(LIMIT * 2).await;
    model_idle::sweep(Some(LIMIT), WINDOW_OPEN, Some(&model));
    let kept = resident_mib();
    let (_, still_warm) = translate(&pipeline, texts[2]).await;
    println!("window open: {kept} MiB, next translation {still_warm:.2?}");
    assert!(kept + 500 > loaded, "freed although a window was open");

    // Nothing open and nothing ran for the limit: it is freed.
    tokio::time::sleep(LIMIT * 2).await;
    model_idle::sweep(Some(LIMIT), Waiting::default(), Some(&model));
    let freed = resident_mib();
    println!(
        "unused: {freed} MiB ({} MiB given back)",
        kept.saturating_sub(freed)
    );
    assert!(
        freed + 500 < kept,
        "freeing gave back too little: {kept} MiB -> {freed} MiB"
    );

    // The next request loads again and still answers.
    let (again, reload) = translate(&pipeline, texts[3]).await;
    assert!(!again.trim().is_empty());
    let cost = reload.saturating_sub(warm);
    println!("after freeing: {reload:.2?} (loading again costs about {cost:.2?})");

    // Opening a window loads ahead: by the first reading it is there.
    tokio::time::sleep(LIMIT * 2).await;
    model_idle::sweep(Some(LIMIT), Waiting::default(), Some(&model));
    pipeline.preload_for_reading();
    tokio::time::sleep(cost + Duration::from_secs(2)).await;
    let (_, ahead) = translate(&pipeline, texts[4]).await;
    println!("loaded ahead: next translation {ahead:.2?}");
    assert!(
        ahead < warm + cost / 2,
        "loading ahead did not spare the request the load"
    );

    // Switched off: nothing is freed.
    let before = resident_mib();
    tokio::time::sleep(LIMIT * 2).await;
    model_idle::sweep(model_idle::limit(0), Waiting::default(), Some(&model));
    assert!(resident_mib() + 500 > before, "freed although switched off");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: needs the PP-OCR models"]
async fn unused_text_recogniser_is_freed_and_loads_again() {
    let pipeline = pipeline();
    let png = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/lens/english0002.png"),
    )
    .unwrap();

    let base = resident_mib();
    let (lines, cold) = read(&pipeline, &png).await;
    let (_, warm) = read(&pipeline, &png).await;
    assert!(lines > 0, "nothing read from the fixture");
    println!("first {cold:.2?} (with load), loaded {warm:.2?}");
    println!("memory: {base} MiB before, {} MiB loaded", resident_mib());

    tokio::time::sleep(LIMIT * 2).await;
    model_idle::sweep(Some(LIMIT), WINDOW_OPEN, None);
    let (_, still_warm) = read(&pipeline, &png).await;
    assert!(
        still_warm < (cold + warm) / 2,
        "loaded again although a window was open: {still_warm:.2?}"
    );

    tokio::time::sleep(LIMIT * 2).await;
    model_idle::sweep(Some(LIMIT), Waiting::default(), None);
    println!("unused: {} MiB", resident_mib());

    // Loads again (without measuring the backends a second time) and
    // reads the same.
    let (lines_again, reload) = read(&pipeline, &png).await;
    assert_eq!(lines_again, lines, "reads differently after loading again");
    println!(
        "after freeing: {reload:.2?} (loading again costs about {:.2?})",
        reload.saturating_sub(warm)
    );
}
