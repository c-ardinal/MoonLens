//! Continuous window translation reads new text twice before it is
//! translated (text still being typed out must not be translated
//! half-way). When the picture did not change between the two readings,
//! the second one must not run OCR again: the same pixels read the same.
//!
//! Drives the shared [`Tracker`] with stub OCR / translate providers and
//! counts the OCR runs.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use async_trait::async_trait;
use image::{DynamicImage, Rgba, RgbaImage};
use moonlens_lib::{
    pipeline::Pipeline,
    providers::{
        Capability, OcrLine, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
    },
    region_watcher::{Frame, Geometry, LiveEvent, Tick, TickEnv, Tracker},
    settings::{
        schema::{Settings, TaskChain},
        SettingsStore,
    },
    storage::Storage,
};
use parking_lot::Mutex;

/// OCR stub: returns the text set by the test and counts its runs.
struct StubOcr {
    text: Arc<Mutex<&'static str>>,
    runs: Arc<AtomicUsize>,
}

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
        self.runs.fetch_add(1, Ordering::Relaxed);
        Ok(TaskOutput::Ocr {
            lines: vec![OcrLine {
                text: (*self.text.lock()).into(),
                bbox: [10.0, 20.0, 300.0, 20.0],
                confidence: 1.0,
            }],
        })
    }
}

struct StubTranslator(Arc<AtomicUsize>);

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
        self.0.fetch_add(1, Ordering::Relaxed);
        let TaskInput::Translate { text, .. } = input else {
            unreachable!()
        };
        Ok(TaskOutput::Text(format!("[{text}]")))
    }
}

/// White 800x60 image with a dark bar at `x` standing in for a word.
fn picture(x: u32) -> DynamicImage {
    let mut img = RgbaImage::from_pixel(800, 60, Rgba([255, 255, 255, 255]));
    for yy in 20..40 {
        for xx in x..x + 60 {
            img.put_pixel(xx, yy, Rgba([0, 0, 0, 255]));
        }
    }
    DynamicImage::ImageRgba8(img)
}

struct Run {
    pipeline: Pipeline,
    tracker: Tracker,
    text: Arc<Mutex<&'static str>>,
    ocr_runs: Arc<AtomicUsize>,
    translations: Arc<AtomicUsize>,
}

impl Run {
    fn new() -> Self {
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
        // No QR provider in the chain: the QR step fails fast and OCR runs.
        s.providers.qr_handler.chain = vec![];
        store.save(s).unwrap();
        std::mem::forget(tmp);

        let text = Arc::new(Mutex::new("The north wind blows."));
        let ocr_runs = Arc::new(AtomicUsize::new(0));
        let translations = Arc::new(AtomicUsize::new(0));
        let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
        providers.insert(
            "stub_ocr".into(),
            Arc::new(StubOcr {
                text: text.clone(),
                runs: ocr_runs.clone(),
            }),
        );
        providers.insert(
            "stub_tr".into(),
            Arc::new(StubTranslator(translations.clone())),
        );
        Self {
            pipeline: Pipeline::with_providers(
                Arc::new(store),
                Arc::new(Storage::open_in_memory().unwrap()),
                providers,
            ),
            tracker: Tracker::new(),
            text,
            ocr_runs,
            translations,
        }
    }

    async fn tick(&mut self, image: DynamicImage, force: bool) -> Tick {
        const GEOMETRY: Geometry = [0, 0, 800, 60];
        let emit = |_: LiveEvent| {};
        let geometry_now = || Some(GEOMETRY);
        let env = TickEnv {
            pipeline: &self.pipeline,
            target_language: "ja",
            history_kind: "lens",
            emit: &emit,
            geometry_now: &geometry_now,
        };
        self.tracker
            .tick(
                Frame {
                    image,
                    geometry: GEOMETRY,
                },
                force,
                &env,
            )
            .await
            .unwrap()
    }

    fn ocr_runs(&self) -> usize {
        self.ocr_runs.load(Ordering::Relaxed)
    }
}

#[tokio::test]
async fn an_unchanged_picture_is_confirmed_without_a_second_ocr_run() {
    let mut run = Run::new();

    assert_eq!(run.tick(picture(100), false).await, Tick::Settling);
    assert_eq!(run.ocr_runs(), 1);

    // Same picture on the next tick: the first reading stands.
    let tick = run.tick(picture(100), false).await;
    assert!(
        matches!(&tick, Tick::Translated { source, translation, requested: true }
            if source == "The north wind blows." && translation == "[The north wind blows.]"),
        "{tick:?}"
    );
    assert_eq!(run.ocr_runs(), 1, "the same picture was read twice");
    assert_eq!(run.translations.load(Ordering::Relaxed), 1);

    // And nothing more happens while the screen stays as it is.
    assert_eq!(run.tick(picture(100), false).await, Tick::Unchanged);
    assert_eq!(run.ocr_runs(), 1);
}

#[tokio::test]
async fn a_picture_that_changed_while_settling_is_read_again() {
    let mut run = Run::new();

    assert_eq!(run.tick(picture(100), false).await, Tick::Settling);
    // The text is still being typed out: more of it on the next tick.
    *run.text.lock() = "The north wind blows hard tonight.";
    assert_eq!(run.tick(picture(300), false).await, Tick::Settling);
    assert_eq!(run.ocr_runs(), 2);
    assert_eq!(run.translations.load(Ordering::Relaxed), 0);

    // Now it holds still: the longer text is translated, read once.
    let tick = run.tick(picture(300), false).await;
    assert!(
        matches!(&tick, Tick::Translated { source, .. }
            if source == "The north wind blows hard tonight."),
        "{tick:?}"
    );
    assert_eq!(run.ocr_runs(), 2);
}

#[tokio::test]
async fn a_manual_refresh_always_reads_the_screen() {
    let mut run = Run::new();

    assert_eq!(run.tick(picture(100), false).await, Tick::Settling);
    let tick = run.tick(picture(100), true).await;
    assert!(matches!(tick, Tick::Translated { .. }), "{tick:?}");
    assert_eq!(run.ocr_runs(), 2);
}
