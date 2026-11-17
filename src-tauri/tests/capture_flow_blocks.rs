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
        let l = |text: &str, y: f32| OcrLine {
            text: text.into(),
            bbox: [10.0, y, 200.0, 16.0],
            confidence: 1.0,
        };
        Ok(TaskOutput::Ocr {
            lines: vec![
                l("Hello", 10.0),
                l("world.", 28.0),
                l("Second", 80.0),
                l("paragraph.", 98.0),
            ],
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
    store.save(s).unwrap();
    std::mem::forget(tmp);
    let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
    providers.insert("stub_ocr".into(), Arc::new(StubOcr));
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
