//! The translation cache with the real engines (PP-OCR, the built-in
//! llama.cpp translator), over a restart:
//!
//! 1. A fixture image is read and translated: the model runs.
//! 2. A new pipeline over the same data folder (the app started again)
//!    reads the same image: every paragraph comes from disk, the model
//!    is not asked, and the text is the same as in the first run.
//! 3. A glossary term is added: the paragraphs that contain it go back
//!    to the model and follow the term; the others stay cached.
//!
//! `#[ignore]`: needs the downloaded models, like `lens_images_live`.
//!
//! ```sh
//! cd src-tauri
//! cargo test --release --test translation_cache_live -- --ignored --nocapture
//! ```

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use moonlens_lib::{
    pipeline::{CaptureFlowResult, Pipeline},
    plugins::registry::PluginRegistry,
    settings::{schema::GlossaryEntry, SettingsStore},
    storage::Storage,
};

const TERM: &str = "Wind";
const RENDERING: &str = "疾風";

fn start(dir: &Path) -> (Pipeline, Arc<SettingsStore>) {
    let store = Arc::new(SettingsStore::load_from(dir.join("config.toml")).unwrap());
    let plugins = Arc::new(PluginRegistry::load_from(dir.join("plugins"), store.clone()).unwrap());
    let storage = Arc::new(Storage::open_at(dir.join("moonlens.db"), dir).unwrap());
    (Pipeline::new(store.clone(), storage, plugins), store)
}

async fn translate(pipe: &Pipeline, png: &[u8]) -> (CaptureFlowResult, u128) {
    let mut r = pipe.analyze_capture(png.to_vec()).await.unwrap();
    r.app = Some("game".into());
    let t = Instant::now();
    pipe.translate_capture_to(&mut r, "ja").await.unwrap();
    (r, t.elapsed().as_millis())
}

fn texts(r: &CaptureFlowResult) -> Vec<String> {
    r.blocks
        .iter()
        .map(|b| b.translation.clone().unwrap_or_default())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the downloaded OCR and translation models"]
async fn cached_translations_survive_a_restart_and_follow_the_glossary() {
    let png = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lens/english0002.png"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();

    // 1. First run: the model translates.
    let (pipe, _) = start(dir.path());
    let (first, ms) = translate(&pipe, &png).await;
    assert!(!first.blocks.is_empty());
    assert!(first.blocks.iter().all(|b| b.translation.is_some()));
    assert!(first.blocks.iter().any(|b| !b.from_cache));
    println!(
        "first run: {} blocks translated in {ms} ms via {}",
        first.blocks.len(),
        first.result.used_provider
    );
    let usage = pipe.translation_cache_usage().unwrap();
    println!("on disk: {} entries, {} bytes", usage.entries, usage.bytes);
    assert_eq!(usage.entries as usize, first.blocks.len());
    assert_eq!(usage.scopes[0].scope, "game");
    drop(pipe);

    // 2. After a restart: from disk, no model request, same text.
    let (pipe, store) = start(dir.path());
    let (again, ms) = translate(&pipe, &png).await;
    println!("after restart: {} blocks in {ms} ms", again.blocks.len());
    assert!(again.blocks.iter().all(|b| b.from_cache));
    assert_eq!(texts(&again), texts(&first));
    assert_eq!(again.result.latency_ms, 0);
    assert_eq!(again.result.used_provider, first.result.used_provider);

    // 3. A new glossary term: only the paragraphs holding it are
    //    translated again, and they follow it.
    let mut s = (*store.snapshot()).clone();
    s.translation.glossary = vec![GlossaryEntry {
        source: TERM.into(),
        target: RENDERING.into(),
        ..GlossaryEntry::default()
    }];
    store.save(s).unwrap();
    pipe.rebuild_providers();
    let (termed, ms) = translate(&pipe, &png).await;
    println!("after the glossary change: {ms} ms");
    let mut touched = 0;
    for (b, before) in termed.blocks.iter().zip(texts(&first)) {
        let holds_term = b
            .source
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w == TERM.to_lowercase());
        let now = b.translation.as_deref().unwrap_or_default();
        println!("  cache={} | {} → {now}", b.from_cache, b.source);
        if holds_term {
            touched += 1;
            assert!(!b.from_cache, "translated again under the new glossary");
            assert!(now.contains(RENDERING), "follows the glossary: {now}");
        } else {
            assert!(b.from_cache);
            assert_eq!(now, before);
        }
    }
    assert!(touched > 0, "the fixture holds the term");
}
