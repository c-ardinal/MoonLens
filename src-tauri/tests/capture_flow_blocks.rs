//! Capture flow → positioned, per-paragraph translations.
//!
//! Drives `Pipeline::run_capture_flow` with stub OCR / translate
//! providers to check the Lens-style overlay contract: OCR lines are
//! grouped into blocks, each block keeps its position, and every block
//! receives its own translation — via the single batched request when
//! the translator preserves paragraphs, or via per-block requests when
//! it does not.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use moonlens_lib::{
    pipeline::Pipeline,
    providers::{
        Capability, Intent, OcrLine, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
    },
    settings::{
        schema::{Settings, TaskChain},
        SettingsStore,
    },
    storage::Storage,
};

/// OCR stub returning `(text, y)` lines, each 16 px tall at x = 10.
struct StubOcr(Vec<(&'static str, f32)>);

const ENGLISH_LINES: &[(&str, f32)] = &[
    ("Hello", 10.0),
    ("world.", 28.0),
    ("Second", 80.0),
    ("paragraph.", 98.0),
];

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
            lines: self
                .0
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

/// Translator that either keeps paragraph breaks (`merge = false`) or
/// collapses them on multi-paragraph input (`merge = true`).
struct StubTranslator {
    merge: bool,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Provider for StubTranslator {
    fn id(&self) -> &str {
        "stub_tr"
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Translate]
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let TaskInput::Translate { text, .. } = input else {
            unreachable!()
        };
        let paragraphs: Vec<String> = text.split("\n\n").map(|p| format!("[{p}]")).collect();
        Ok(TaskOutput::Text(if self.merge {
            paragraphs.join(" ")
        } else {
            paragraphs.join("\n\n")
        }))
    }
}

fn pipeline(merge: bool, calls: Arc<AtomicUsize>) -> Pipeline {
    pipeline_with(ENGLISH_LINES, "en-not-target", merge, calls)
}

fn pipeline_with(
    lines: &[(&'static str, f32)],
    target: &str,
    merge: bool,
    calls: Arc<AtomicUsize>,
) -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::load_from(tmp.path().join("config.toml")).unwrap();
    let mut s = Settings::default();
    s.providers.ocr = TaskChain {
        chain: vec!["stub_ocr".into()],
        timeout_ms: 1000,
        ..TaskChain::default()
    };
    s.providers.translate = TaskChain {
        chain: vec!["stub_tr".into()],
        timeout_ms: 1000,
        ..TaskChain::default()
    };
    // No QR provider in the chain → QR step fails fast and OCR runs.
    s.providers.qr_handler.chain = vec![];
    s.subtitle.target_language = target.into();
    store.save(s).unwrap();
    std::mem::forget(tmp);
    let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
    providers.insert("stub_ocr".into(), Arc::new(StubOcr(lines.to_vec())));
    providers.insert("stub_tr".into(), Arc::new(StubTranslator { merge, calls }));
    Pipeline::with_providers(
        Arc::new(store),
        Arc::new(Storage::open_in_memory().unwrap()),
        providers,
    )
}

fn white_png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

#[tokio::test]
async fn paragraphs_are_translated_in_one_request_and_mapped_back() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline(false, calls.clone());
    let r = pipe.run_capture_flow(white_png(300, 140)).await.unwrap();

    assert!(matches!(r.intent, Intent::Text));
    assert_eq!(r.image_size, Some([300, 140]));
    assert_eq!(
        r.source_text.as_deref(),
        Some("Hello world.\n\nSecond paragraph.")
    );
    assert_eq!(r.blocks.len(), 2);
    assert_eq!(r.blocks[0].source, "Hello world.");
    assert_eq!(r.blocks[0].bbox, [10.0, 10.0, 200.0, 34.0]);
    assert_eq!(r.blocks[0].translation.as_deref(), Some("[Hello world.]"));
    assert_eq!(
        r.blocks[1].translation.as_deref(),
        Some("[Second paragraph.]")
    );
    let colors = r.blocks[0].colors.unwrap();
    assert_eq!(colors.background, [255, 255, 255]);
    assert_eq!(calls.load(Ordering::Relaxed), 1, "one batched request");
}

#[tokio::test]
async fn merged_translation_falls_back_to_per_block_requests() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline(true, calls.clone());
    let r = pipe.run_capture_flow(white_png(300, 140)).await.unwrap();

    assert_eq!(r.blocks.len(), 2);
    assert_eq!(r.blocks[0].translation.as_deref(), Some("[Hello world.]"));
    assert_eq!(
        r.blocks[1].translation.as_deref(),
        Some("[Second paragraph.]")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 3, "batched + one per block");
}

#[tokio::test]
async fn analyze_then_translate_matches_run_capture_flow() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline(false, calls.clone());
    let mut r = pipe.analyze_capture(white_png(300, 140)).await.unwrap();
    assert!(r.needs_translation());
    assert!(r.blocks.iter().all(|b| b.translation.is_none()));
    assert_eq!(calls.load(Ordering::Relaxed), 0);

    pipe.translate_capture(&mut r).await.unwrap();
    assert!(!r.needs_translation());
    assert!(matches!(&r.result.output, TaskOutput::Text(t) if t.contains("[Hello world.]")));
}

const MIXED_LINES: &[(&str, f32)] = &[
    ("これは日本語の文章です。", 10.0),
    ("This is a test of the system.", 80.0),
    ("12:30", 140.0),
];

#[tokio::test]
async fn blocks_already_in_target_are_not_translated() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline_with(MIXED_LINES, "ja", false, calls.clone());
    let r = pipe.run_capture_flow(white_png(300, 180)).await.unwrap();

    assert_eq!(r.blocks.len(), 3);
    assert_eq!(r.source_language.as_deref(), Some("ja"));
    assert_eq!(r.target_language.as_deref(), Some("ja"));
    assert!(r.blocks[0].same_language);
    assert_eq!(
        r.blocks[0].translation.as_deref(),
        Some("これは日本語の文章です。")
    );
    assert!(!r.blocks[1].same_language);
    assert_eq!(
        r.blocks[1].translation.as_deref(),
        Some("[This is a test of the system.]")
    );
    assert!(r.blocks[2].same_language, "numbers need no translation");
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    // Only the English block went to the translator.
    assert!(
        matches!(&r.result.output, TaskOutput::Text(t) if t == "[This is a test of the system.]")
    );
}

#[tokio::test]
async fn retranslating_into_another_language_flips_passthrough() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline_with(MIXED_LINES, "ja", false, calls.clone());
    let mut r = pipe.analyze_capture(white_png(300, 180)).await.unwrap();
    assert!(r.blocks[0].same_language);

    pipe.translate_capture_to(&mut r, "en-US").await.unwrap();
    assert_eq!(r.target_language.as_deref(), Some("en-US"));
    assert!(!r.blocks[0].same_language);
    assert_eq!(
        r.blocks[0].translation.as_deref(),
        Some("[これは日本語の文章です。]")
    );
    assert!(r.blocks[1].same_language);
    assert_eq!(
        r.blocks[1].translation.as_deref(),
        Some("This is a test of the system.")
    );
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn nothing_to_translate_skips_the_translator() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline_with(&MIXED_LINES[..1], "ja", false, calls.clone());
    let r = pipe.run_capture_flow(white_png(300, 60)).await.unwrap();

    assert_eq!(r.source_language.as_deref(), Some("ja"));
    assert!(r.blocks.iter().all(|b| b.same_language));
    assert_eq!(r.result.used_provider, "builtin.passthrough");
    assert!(!r.needs_translation());
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn repeated_selection_is_served_from_the_cache() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline(false, calls.clone());
    let first = pipe.run_capture_flow(white_png(300, 140)).await.unwrap();
    assert!(first.blocks.iter().all(|b| !b.from_cache));
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    let again = pipe.run_capture_flow(white_png(300, 140)).await.unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1, "no second round-trip");
    assert!(again.blocks.iter().all(|b| b.from_cache));
    assert_eq!(
        again.blocks[1].translation.as_deref(),
        Some("[Second paragraph.]")
    );
    assert_eq!(again.result.used_provider, "stub_tr");
}

#[tokio::test]
async fn only_uncached_paragraphs_are_requested() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline(false, calls.clone());
    pipe.run_capture_flow(white_png(300, 140)).await.unwrap();

    // Same page scrolled: first paragraph unchanged, second one new.
    let mut r = pipe.analyze_capture(white_png(300, 140)).await.unwrap();
    assert!(
        r.blocks.iter().all(|b| b.from_cache),
        "prefilled at analysis"
    );
    r.blocks[1].source = "Brand new paragraph.".into();
    pipe.translate_capture(&mut r).await.unwrap();

    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert!(r.blocks[0].from_cache);
    assert!(!r.blocks[1].from_cache);
    assert_eq!(
        r.blocks[1].translation.as_deref(),
        Some("[Brand new paragraph.]")
    );
    assert!(matches!(&r.result.output, TaskOutput::Text(t) if t == "[Brand new paragraph.]"));
}

#[tokio::test]
async fn cache_is_per_target_language() {
    let calls = Arc::new(AtomicUsize::new(0));
    let pipe = pipeline(false, calls.clone());
    let mut r = pipe.analyze_capture(white_png(300, 140)).await.unwrap();
    pipe.translate_capture_to(&mut r, "fr").await.unwrap();
    pipe.translate_capture_to(&mut r, "de").await.unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    pipe.translate_capture_to(&mut r, "fr").await.unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    assert!(r.blocks.iter().all(|b| b.from_cache));

    // Settings / provider changes drop the cache.
    pipe.clear_translation_cache();
    pipe.translate_capture_to(&mut r, "fr").await.unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}
