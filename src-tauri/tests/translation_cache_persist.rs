//! The translation cache as the pipeline uses it: kept on disk, per
//! app, and never in the way of quality.
//!
//! Stub OCR / translate providers; the translator records every request
//! so each test can tell which paragraphs reached the model.

use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use moonlens_lib::{
    pipeline::{CaptureFlowResult, Pipeline},
    providers::{
        Capability, OcrLine, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
    },
    settings::{
        schema::{GlossaryEntry, Settings, TaskChain},
        SettingsStore,
    },
    storage::Storage,
    translation_cache::{DiskCache, TranslationCache},
};
use parking_lot::Mutex;

/// Two paragraphs: "Iron Sword" and "Save Game".
struct StubOcr;

#[async_trait]
impl Provider for StubOcr {
    fn id(&self) -> &str {
        "stub_ocr"
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Ocr]
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn run(&self, _input: TaskInput) -> Result<TaskOutput, ProviderError> {
        Ok(TaskOutput::Ocr {
            lines: [("Iron Sword", 10.0), ("Save Game", 80.0)]
                .iter()
                .map(|&(text, y)| OcrLine {
                    text: text.into(),
                    bbox: [10.0, y, 200.0, 16.0],
                    confidence: 1.0,
                })
                .collect(),
        })
    }
}

/// Wraps each paragraph in `tag`; fails while `down`.
struct StubTranslator {
    id: &'static str,
    tag: &'static str,
    down: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Provider for StubTranslator {
    fn id(&self) -> &str {
        self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Translate]
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        if self.down.load(Ordering::Relaxed) {
            return Err(ProviderError::Other("down".into()));
        }
        let TaskInput::Translate { text, .. } = input else {
            unreachable!()
        };
        self.requests.lock().push(text.clone());
        let tag = self.tag;
        let out: Vec<String> = text.split("\n\n").map(|p| format!("{tag}{p}")).collect();
        Ok(TaskOutput::Text(out.join("\n\n")))
    }
}

/// One "run of the app": a pipeline over the cache file in `dir`.
struct Run {
    pipe: Pipeline,
    store: Arc<SettingsStore>,
    /// Paragraph batches the main translator was asked for.
    requests: Arc<Mutex<Vec<String>>>,
    main_down: Arc<AtomicBool>,
}

fn start(dir: &Path) -> Run {
    let store = Arc::new(SettingsStore::load_from(dir.join("config.toml")).unwrap());
    if store.snapshot().providers.ocr.chain != ["stub_ocr"] {
        let mut s = Settings::default();
        s.providers.ocr = TaskChain {
            chain: vec!["stub_ocr".into()],
            timeout_ms: 1000,
            ..TaskChain::default()
        };
        s.providers.translate = TaskChain {
            chain: vec!["main".into(), "spare".into()],
            timeout_ms: 1000,
            sticky_session_ms: 0,
            ..TaskChain::default()
        };
        s.providers.qr_handler.chain = vec![];
        s.storage.keep_history = false;
        store.save(s).unwrap();
    }
    let requests: Arc<Mutex<Vec<String>>> = Arc::default();
    let main_down = Arc::new(AtomicBool::new(false));
    let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
    providers.insert("stub_ocr".into(), Arc::new(StubOcr));
    providers.insert(
        "main".into(),
        Arc::new(StubTranslator {
            id: "main",
            tag: "訳:",
            down: main_down.clone(),
            requests: requests.clone(),
        }),
    );
    providers.insert(
        "spare".into(),
        Arc::new(StubTranslator {
            id: "spare",
            tag: "代:",
            down: Arc::default(),
            requests: Arc::default(),
        }),
    );
    let cache = TranslationCache::default()
        .with_disk(DiskCache::open(dir.join(DiskCache::FILE_NAME)).unwrap());
    let pipe = Pipeline::with_providers(
        store.clone(),
        Arc::new(Storage::open_in_memory().unwrap()),
        providers,
    )
    .with_translation_cache(cache);
    Run {
        pipe,
        store,
        requests,
        main_down,
    }
}

impl Run {
    /// Read the screen of `app` and translate it into Japanese.
    async fn translate(&self, app: Option<&str>) -> CaptureFlowResult {
        let mut r = self.pipe.analyze_capture(white_png()).await.unwrap();
        r.app = app.map(str::to_string);
        self.pipe.translate_capture_to(&mut r, "ja").await.unwrap();
        r
    }

    fn requests(&self) -> Vec<String> {
        std::mem::take(&mut *self.requests.lock())
    }

    fn change(&self, f: impl FnOnce(&mut Settings)) {
        let mut s = (*self.store.snapshot()).clone();
        f(&mut s);
        self.store.save(s).unwrap();
    }
}

fn white_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(300, 140, image::Rgba([255, 255, 255, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn texts(r: &CaptureFlowResult) -> Vec<&str> {
    r.blocks
        .iter()
        .map(|b| b.translation.as_deref().unwrap_or("-"))
        .collect()
}

fn term(source: &str, target: &str) -> GlossaryEntry {
    GlossaryEntry {
        source: source.into(),
        target: target.into(),
        ..GlossaryEntry::default()
    }
}

#[tokio::test]
async fn text_seen_before_a_restart_needs_no_request() {
    let dir = tempfile::tempdir().unwrap();
    let first = start(dir.path());
    let r = first.translate(Some("game")).await;
    assert_eq!(texts(&r), ["訳:Iron Sword", "訳:Save Game"]);
    assert_eq!(first.requests().len(), 1);
    drop(first);

    let again = start(dir.path());
    let r = again.translate(Some("game")).await;
    assert_eq!(texts(&r), ["訳:Iron Sword", "訳:Save Game"]);
    assert!(r.blocks.iter().all(|b| b.from_cache));
    assert!(again.requests().is_empty(), "served from disk");
    assert_eq!(r.result.used_provider, "main");
}

#[tokio::test]
async fn each_app_has_its_own_entries() {
    let dir = tempfile::tempdir().unwrap();
    let run = start(dir.path());
    run.translate(Some("game")).await;
    assert_eq!(run.requests().len(), 1);

    // The same words in another app, or in no app, are translated anew.
    run.translate(Some("editor")).await;
    run.translate(None).await;
    assert_eq!(run.requests().len(), 2);

    let usage = run.pipe.translation_cache_usage().unwrap();
    let mut scopes: Vec<(&str, u64)> = usage
        .scopes
        .iter()
        .map(|s| (s.scope.as_str(), s.entries))
        .collect();
    scopes.sort();
    assert_eq!(scopes, [("", 2), ("editor", 2), ("game", 2)]);

    // Clearing one app leaves the others.
    assert_eq!(run.pipe.clear_translation_cache(Some("game")).unwrap(), 2);
    run.translate(Some("editor")).await;
    assert!(run.requests().is_empty());
    run.translate(Some("game")).await;
    assert_eq!(run.requests().len(), 1);
}

#[tokio::test]
async fn a_glossary_change_retranslates_only_the_paragraphs_it_touches() {
    let dir = tempfile::tempdir().unwrap();
    let run = start(dir.path());
    run.translate(None).await;
    run.requests();

    // A term that occurs in the first paragraph only.
    run.change(|s| s.translation.glossary = vec![term("Sword", "剣")]);
    let r = run.translate(None).await;
    assert_eq!(run.requests(), ["Iron Sword"]);
    assert!(!r.blocks[0].from_cache);
    assert!(r.blocks[1].from_cache);

    // Its rendering changed: again that paragraph only.
    run.change(|s| s.translation.glossary = vec![term("Sword", "刀")]);
    run.translate(None).await;
    assert_eq!(run.requests(), ["Iron Sword"]);

    // A term that occurs nowhere on screen costs nothing.
    run.change(|s| s.translation.glossary.push(term("Shield", "盾")));
    run.translate(None).await;
    assert!(run.requests().is_empty());

    // Removed: the paragraph goes back to the model. The translation made
    // under the old glossary is not brought back, after a restart either.
    run.change(|s| s.translation.glossary.clear());
    drop(run);
    let run = start(dir.path());
    run.translate(None).await;
    assert_eq!(run.requests(), ["Iron Sword"]);
}

#[tokio::test]
async fn another_engine_setup_or_style_is_not_served_old_output() {
    let dir = tempfile::tempdir().unwrap();
    let run = start(dir.path());
    run.translate(None).await;
    run.requests();

    run.change(|s| {
        s.providers
            .configs
            .insert("main".into(), serde_json::json!({ "model": "other" }));
    });
    run.translate(None).await;
    assert_eq!(run.requests().len(), 1, "engine config changed");

    run.change(|s| s.translation.style = moonlens_lib::settings::schema::JaStyle::Polite);
    run.translate(None).await;
    assert_eq!(run.requests().len(), 1, "style changed");

    // Settings that do not shape the translation keep the cache.
    run.change(|s| s.performance.max_concurrent_tasks = 2);
    run.translate(None).await;
    assert!(run.requests().is_empty());
}

#[tokio::test]
async fn a_stand_in_engines_answer_is_not_kept_past_the_session() {
    let dir = tempfile::tempdir().unwrap();
    let run = start(dir.path());
    run.main_down.store(true, Ordering::Relaxed);
    let r = run.translate(None).await;
    assert_eq!(texts(&r), ["代:Iron Sword", "代:Save Game"]);
    // Within the session the answer is reused …
    let r = run.translate(None).await;
    assert!(r.blocks.iter().all(|b| b.from_cache));
    assert_eq!(run.pipe.translation_cache_usage().unwrap().entries, 0);
    drop(run);

    // … after a restart the chosen engine translates the text.
    let run = start(dir.path());
    let r = run.translate(None).await;
    assert_eq!(texts(&r), ["訳:Iron Sword", "訳:Save Game"]);
}

#[tokio::test]
async fn keeping_translations_can_be_turned_off() {
    let dir = tempfile::tempdir().unwrap();
    let run = start(dir.path());
    run.change(|s| s.storage.keep_translation_cache = false);
    run.translate(None).await;
    assert_eq!(run.requests().len(), 1);
    // Still instant within the session, nothing on disk.
    run.translate(None).await;
    assert!(run.requests().is_empty());
    assert_eq!(run.pipe.translation_cache_usage().unwrap().entries, 0);
    drop(run);

    let run = start(dir.path());
    run.translate(None).await;
    assert_eq!(run.requests().len(), 1);
}
