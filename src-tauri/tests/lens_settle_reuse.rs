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
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    region_watcher::{Frame, Geometry, LiveEvent, ReadMode, Tick, TickEnv, Tracker},
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
    /// Fail the next run (an engine error, not "no text").
    fail_next: Arc<AtomicBool>,
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
        if self.fail_next.swap(false, Ordering::Relaxed) {
            return Err(ProviderError::Other("engine failure".into()));
        }
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
    fail_next: Arc<AtomicBool>,
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
        let fail_next = Arc::new(AtomicBool::new(false));
        let translations = Arc::new(AtomicUsize::new(0));
        let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
        providers.insert(
            "stub_ocr".into(),
            Arc::new(StubOcr {
                text: text.clone(),
                runs: ocr_runs.clone(),
                fail_next: fail_next.clone(),
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
            fail_next,
            translations,
        }
    }

    async fn tick(&mut self, image: DynamicImage, force: bool) -> Tick {
        self.try_tick(image, force).await.unwrap()
    }

    async fn try_tick(&mut self, image: DynamicImage, force: bool) -> Result<Tick, String> {
        const GEOMETRY: Geometry = [0, 0, 800, 60];
        let emit = |_: LiveEvent| {};
        let geometry_now = || Some(GEOMETRY);
        let env = TickEnv {
            pipeline: &self.pipeline,
            target_language: "ja",
            mode: ReadMode::default(),
            history_kind: "lens",
            app: None,
            emit: &emit,
            geometry_now: &geometry_now,
        };
        self.tracker
            .tick(
                Frame {
                    image,
                    geometry: GEOMETRY,
                    visible: None,
                },
                force,
                &env,
            )
            .await
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

/// One more character typed into a large region changes too few pixels
/// for the change detection, which ignores video noise. The picture is
/// not the same one, though, so it must be read again.
#[tokio::test]
async fn a_change_below_the_detection_threshold_is_still_read_again() {
    let mut run = Run::new();

    assert_eq!(run.tick(picture(100), false).await, Tick::Settling);
    let mut more = picture(100).to_rgba8();
    more.put_pixel(700, 30, Rgba([0, 0, 0, 255]));
    *run.text.lock() = "The north wind blows!";
    assert_eq!(
        run.tick(DynamicImage::ImageRgba8(more.clone()), false)
            .await,
        Tick::Settling
    );
    assert_eq!(run.ocr_runs(), 2);

    let tick = run.tick(DynamicImage::ImageRgba8(more), false).await;
    assert!(
        matches!(&tick, Tick::Translated { source, .. } if source == "The north wind blows!"),
        "{tick:?}"
    );
    assert_eq!(run.ocr_runs(), 2);
}

/// A reading must never stand in for a picture it was not read from,
/// also when reading the newer picture failed.
#[tokio::test]
async fn a_failed_read_of_a_new_picture_is_retried_not_replaced_by_the_old_reading() {
    let mut run = Run::new();

    assert_eq!(run.tick(picture(100), false).await, Tick::Settling);
    *run.text.lock() = "The north wind blows hard tonight.";
    run.fail_next.store(true, Ordering::Relaxed);
    assert!(run.try_tick(picture(300), false).await.is_err());
    assert_eq!(run.ocr_runs(), 2);

    // The new picture holds still: it is read now, not skipped.
    assert_eq!(run.tick(picture(300), false).await, Tick::Settling);
    assert_eq!(run.ocr_runs(), 3);
    let tick = run.tick(picture(300), false).await;
    assert!(
        matches!(&tick, Tick::Translated { source, .. }
            if source == "The north wind blows hard tonight."),
        "{tick:?}"
    );
    assert_eq!(run.ocr_runs(), 3);
    assert_eq!(run.translations.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn a_manual_refresh_always_reads_the_screen() {
    let mut run = Run::new();

    assert_eq!(run.tick(picture(100), false).await, Tick::Settling);
    let tick = run.tick(picture(100), true).await;
    assert!(matches!(tick, Tick::Translated { .. }), "{tick:?}");
    assert_eq!(run.ocr_runs(), 2);
}
