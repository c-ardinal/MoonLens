//! Continuous window translation follows a scroll: the translations
//! shown move with their text at once, only the rows that came into
//! view are read, and only paragraphs not seen before go to the
//! translator.
//!
//! Drives the shared [`Tracker`] with a stub OCR that really reads the
//! test page (so it reports what is in the picture it is given, and how
//! tall that picture is) and a stub translator that records its
//! requests.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use image::{DynamicImage, GrayImage, Luma};
use moonlens_lib::{
    pipeline::{LensBlock, Pipeline},
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

const W: u32 = 400;
const H: u32 = 300;
/// Height of a line, rows between lines of a paragraph and between
/// paragraphs (three lines each).
const LINE: u32 = 20;
const ROW_PITCH: u32 = 26;
const PARAGRAPH_PITCH: u32 = 120;
/// A line is a row of cells: a dark one, ten that spell its number in
/// binary, and a dark one.
const CELL: u32 = 12;
const LEFT: u32 = 20;

fn line_top(i: u32) -> u32 {
    (i / 3) * PARAGRAPH_PITCH + (i % 3) * ROW_PITCH + 10
}

fn page(lines: u32) -> GrayImage {
    let mut img = GrayImage::from_pixel(W, line_top(lines) + PARAGRAPH_PITCH, Luma([255]));
    for i in 0..lines {
        for cell in 0..12 {
            let dark = cell == 0 || cell == 11 || (i >> (cell - 1)) & 1 == 1;
            if !dark {
                continue;
            }
            for y in 0..LINE {
                // Narrower towards the bottom, so the rows of a line
                // differ like those of real glyphs.
                for x in 0..CELL - 2 - y / 4 {
                    img.put_pixel(LEFT + cell * CELL + x, line_top(i) + y, Luma([20]));
                }
            }
        }
    }
    img
}

fn view(page: &GrayImage, top: u32) -> DynamicImage {
    DynamicImage::ImageLuma8(image::imageops::crop_imm(page, 0, top, W, H).to_image())
}

/// Reads the lines of [`page`] in whatever part of it the picture
/// shows. A line cut by the picture's edge reads as `cut`.
struct PageOcr {
    /// Height of every picture read.
    heights: Arc<Mutex<Vec<u32>>>,
    fail_next: Arc<Mutex<bool>>,
}

#[async_trait]
impl Provider for PageOcr {
    fn id(&self) -> &str {
        "page_ocr"
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Ocr]
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Ocr { image_png, .. } = input else {
            unreachable!()
        };
        let img = image::load_from_memory(&image_png).unwrap().to_luma8();
        self.heights.lock().push(img.height());
        if std::mem::take(&mut *self.fail_next.lock()) {
            return Err(ProviderError::Other("engine failure".into()));
        }
        let dark = |cell: u32, y: u32| img.get_pixel(LEFT + cell * CELL + 2, y)[0] < 128;
        let mut lines = Vec::new();
        let mut y = 0;
        while y < img.height() {
            if !dark(0, y) {
                y += 1;
                continue;
            }
            let top = y;
            while y < img.height() && dark(0, y) {
                y += 1;
            }
            let text = if y - top == LINE {
                let n: u32 = (1..=10).map(|c| (dark(c, top) as u32) << (c - 1)).sum();
                format!("w{n}")
            } else {
                "cut".to_string()
            };
            lines.push(OcrLine {
                text,
                bbox: [
                    LEFT as f32,
                    top as f32,
                    (12 * CELL) as f32,
                    (y - top) as f32,
                ],
                confidence: 1.0,
            });
        }
        Ok(TaskOutput::Ocr { lines })
    }
}

/// Wraps every paragraph in `<>` and records the requests.
struct Recorder(Arc<Mutex<Vec<String>>>);

#[async_trait]
impl Provider for Recorder {
    fn id(&self) -> &str {
        "recorder"
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Translate]
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Translate { text, .. } = input else {
            unreachable!()
        };
        self.0.lock().push(text.clone());
        let out: Vec<String> = text.split("\n\n").map(|p| format!("<{p}>")).collect();
        Ok(TaskOutput::Text(out.join("\n\n")))
    }
}

struct Run {
    pipeline: Pipeline,
    tracker: Tracker,
    page: GrayImage,
    heights: Arc<Mutex<Vec<u32>>>,
    fail_next: Arc<Mutex<bool>>,
    requests: Arc<Mutex<Vec<String>>>,
    events: Arc<Mutex<Vec<LiveEvent>>>,
}

impl Run {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let store = SettingsStore::load_from(tmp.path().join("config.toml")).unwrap();
        let mut s = Settings::default();
        s.providers.ocr = TaskChain {
            chain: vec!["page_ocr".into()],
            timeout_ms: 1000,
            ..TaskChain::default()
        };
        s.providers.translate = TaskChain {
            chain: vec!["recorder".into()],
            timeout_ms: 1000,
            ..TaskChain::default()
        };
        s.providers.qr_handler.chain = vec![];
        store.save(s).unwrap();
        std::mem::forget(tmp);

        let heights = Arc::new(Mutex::new(Vec::new()));
        let fail_next = Arc::new(Mutex::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
        providers.insert(
            "page_ocr".into(),
            Arc::new(PageOcr {
                heights: heights.clone(),
                fail_next: fail_next.clone(),
            }),
        );
        providers.insert("recorder".into(), Arc::new(Recorder(requests.clone())));
        Self {
            pipeline: Pipeline::with_providers(
                Arc::new(store),
                Arc::new(Storage::open_in_memory().unwrap()),
                providers,
            ),
            tracker: Tracker::new(),
            page: page(30),
            heights,
            fail_next,
            requests,
            events: Arc::default(),
        }
    }

    /// One tick with the page scrolled to `top`.
    async fn tick(&mut self, top: u32) -> Result<Tick, String> {
        const GEOMETRY: Geometry = [0, 0, W as i32, H as i32];
        let events = self.events.clone();
        let emit = move |e: LiveEvent| events.lock().push(e);
        let geometry_now = || Some(GEOMETRY);
        let env = TickEnv {
            pipeline: &self.pipeline,
            target_language: "ja",
            mode: Default::default(),
            history_kind: "lens",
            emit: &emit,
            geometry_now: &geometry_now,
        };
        let frame = Frame {
            image: view(&self.page, top),
            geometry: GEOMETRY,
        };
        self.tracker.tick(frame, false, &env).await
    }

    /// Blocks of the latest `Blocks` event and the indices hidden
    /// since.
    fn shown(&self) -> (Vec<LensBlock>, Vec<usize>) {
        let events = self.events.lock();
        let at = events
            .iter()
            .rposition(|e| matches!(e, LiveEvent::Blocks { .. }))
            .expect("no blocks shown");
        let LiveEvent::Blocks { blocks, .. } = &events[at] else {
            unreachable!()
        };
        let hidden = events[at..]
            .iter()
            .filter_map(|e| match e {
                LiveEvent::Stale { indices } => Some(indices.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        (blocks.clone(), hidden)
    }

    /// Show the page at the top: three paragraphs, the third without
    /// its last line (it is below the region).
    async fn shown_at_the_top() -> Self {
        let mut run = Self::new();
        assert_eq!(run.tick(0).await, Ok(Tick::Settling));
        let tick = run.tick(0).await.unwrap();
        assert!(
            matches!(&tick, Tick::Translated { source, .. }
                if source == "w0 w1 w2\n\nw3 w4 w5\n\nw6 w7"),
            "{tick:?}"
        );
        assert_eq!(*run.heights.lock(), vec![H]);
        assert_eq!(run.requests.lock().len(), 1);
        run
    }
}

fn find<'a>(blocks: &'a [LensBlock], source: &str) -> &'a LensBlock {
    blocks
        .iter()
        .find(|b| b.source == source)
        .unwrap_or_else(|| panic!("{source:?} not in {blocks:#?}"))
}

#[tokio::test]
async fn translations_move_with_a_scroll_before_anything_is_read() {
    let mut run = Run::shown_at_the_top().await;
    let (before, _) = run.shown();
    let was = find(&before, "w3 w4 w5").bbox;
    run.events.lock().clear();

    assert_eq!(run.tick(60).await, Ok(Tick::Settling));

    // The first thing the window hears: the same blocks, 60 rows up,
    // translations kept, nothing hidden.
    let events = run.events.lock();
    let LiveEvent::Blocks { blocks, .. } = &events[0] else {
        panic!("{:?}", events[0]);
    };
    let moved = find(blocks, "w3 w4 w5");
    assert_eq!(moved.bbox, [was[0], was[1] - 60.0, was[2], was[3]]);
    assert_eq!(moved.translation.as_deref(), Some("<w3 w4 w5>"));
    assert_eq!(moved.lines[0][1], before[1].lines[0][1] - 60.0);
    // The first paragraph is half out of view and stays shown.
    assert_eq!(
        find(blocks, "w0 w1 w2").translation.as_deref(),
        Some("<w0 w1 w2>")
    );
    assert!(
        !events.iter().any(|e| matches!(e, LiveEvent::Stale { .. })),
        "{events:?}"
    );
}

#[tokio::test]
async fn only_the_rows_that_came_into_view_are_read_and_translated() {
    let mut run = Run::shown_at_the_top().await;

    assert_eq!(run.tick(60).await, Ok(Tick::Settling));
    // The second reading saw a part of the region only.
    let heights = run.heights.lock().clone();
    assert_eq!(heights.len(), 2);
    assert!(heights[1] <= H / 2, "{heights:?}");
    // Nothing is translated while the scroll may go on.
    assert_eq!(run.requests.lock().len(), 1);

    // The picture holds still: the new text is translated, without
    // another reading.
    let tick = run.tick(60).await.unwrap();
    assert!(
        matches!(&tick, Tick::Translated { source, requested: true, .. }
            if source == "w0 w1 w2\n\nw3 w4 w5\n\nw6 w7 w8"),
        "{tick:?}"
    );
    assert_eq!(run.heights.lock().len(), 2);
    // Only what was not shown before went to the translator. The first
    // paragraph lost two lines to the scroll and still reads whole.
    let requests = run.requests.lock().clone();
    assert_eq!(requests[1..], ["w6 w7 w8".to_string()]);
    let (blocks, hidden) = run.shown();
    assert!(hidden.is_empty());
    let first = find(&blocks, "w0 w1 w2");
    assert_eq!(first.translation.as_deref(), Some("<w0 w1 w2>"));
    assert_eq!(first.bbox[1], 10.0 - 60.0);
    assert_eq!(
        find(&blocks, "w6 w7 w8").translation.as_deref(),
        Some("<w6 w7 w8>")
    );
}

#[tokio::test]
async fn a_long_scroll_in_steps_never_asks_for_a_paragraph_twice() {
    let mut run = Run::shown_at_the_top().await;
    for top in [40, 90, 150, 220, 300, 390, 480] {
        assert_eq!(run.tick(top).await, Ok(Tick::Settling), "at {top}");
    }
    assert_eq!(run.requests.lock().len(), 1, "translated mid-scroll");
    let tick = run.tick(480).await.unwrap();
    assert!(matches!(tick, Tick::Translated { .. }), "{tick:?}");
    // Region rows 480..780: paragraphs 4 (lines 12..14, rows 490..562),
    // 5 (610..682) and the first two lines of 6 (730..776).
    let (blocks, _) = run.shown();
    let sources: Vec<&str> = blocks.iter().map(|b| b.source.as_str()).collect();
    assert_eq!(sources, ["w12 w13 w14", "w15 w16 w17", "w18 w19"]);
    assert!(
        blocks.iter().all(|b| b.translation.is_some()),
        "{blocks:#?}"
    );
    assert_eq!(find(&blocks, "w15 w16 w17").bbox[1], 610.0 - 480.0);
    // Every reading after the first saw a part of the region.
    let heights = run.heights.lock().clone();
    assert!(heights[1..].iter().all(|h| *h < H), "{heights:?}");

    // And back to the top: paragraphs shown before come without a
    // request, wherever they are.
    for top in [400, 300, 200, 100, 0] {
        run.tick(top).await.unwrap();
    }
    run.tick(0).await.unwrap();
    let (blocks, hidden) = run.shown();
    assert!(hidden.is_empty());
    assert_eq!(
        find(&blocks, "w0 w1 w2").translation.as_deref(),
        Some("<w0 w1 w2>")
    );
    assert_eq!(find(&blocks, "w0 w1 w2").bbox[1], 10.0);
    let requests = run.requests.lock().clone();
    for (i, request) in requests.iter().enumerate() {
        for paragraph in request.split("\n\n") {
            let again = requests[i + 1..]
                .iter()
                .any(|later| later.split("\n\n").any(|p| p == paragraph));
            assert!(!again, "{paragraph:?} was requested twice: {requests:#?}");
        }
    }
}

#[tokio::test]
async fn rows_that_could_not_be_read_are_read_with_the_whole_region_next() {
    let mut run = Run::shown_at_the_top().await;
    *run.fail_next.lock() = true;
    assert!(run.tick(60).await.is_err());
    // Scrolled on: nothing is known for sure, so everything is read
    // (the last line is cut by the edge of the region, as in any full
    // reading).
    assert_eq!(run.tick(100).await, Ok(Tick::Settling));
    assert_eq!(run.heights.lock().last(), Some(&H));
    let tick = run.tick(100).await.unwrap();
    assert!(
        matches!(&tick, Tick::Translated { source, .. }
            if source == "w3 w4 w5\n\nw6 w7 w8\n\nw9\n\ncut"),
        "{tick:?}"
    );
}

#[tokio::test]
async fn rows_that_could_not_be_read_are_read_again_once_the_scroll_stops() {
    let mut run = Run::shown_at_the_top().await;
    *run.fail_next.lock() = true;
    assert!(run.tick(60).await.is_err());
    // The picture holds still from here on: the reading is not given up.
    assert_eq!(run.tick(60).await, Ok(Tick::Settling));
    assert_eq!(run.heights.lock().last(), Some(&H));
    assert!(matches!(run.tick(60).await, Ok(Tick::Translated { .. })));
}

#[tokio::test]
async fn other_text_in_the_region_is_read_in_full_as_before() {
    let mut run = Run::shown_at_the_top().await;
    // A jump to another part of the page: no row is where a scroll of
    // the shown rows would put it.
    assert_eq!(run.tick(840).await, Ok(Tick::Settling));
    assert_eq!(run.heights.lock().last(), Some(&H));
}
