//! End-to-end test of scroll following: a real page (a browser
//! picture of an English article, `fixtures/lens/scroll_article.png`)
//! is scrolled under a region in steps and read by the shared
//! [`Tracker`] with the real providers (PP-OCR, the built-in llama.cpp
//! translator).
//!
//! Asserted along the scroll:
//!
//! 1. A step moves the shown translations with their text at once:
//!    the first event of the tick, before anything is read.
//! 2. No step asks the model for anything; the new rows are translated
//!    once the page holds still.
//! 3. Paragraphs shown before keep their translation word for word.
//! 4. What the window shows afterwards is what a fresh reading of the
//!    whole region gives: same paragraphs, same places.
//!
//! `SCROLL_FRAMES=<dir>` reads the region pictures from a folder
//! instead (PNG files in name order, e.g. real captures of a scrolling
//! window); the same checks apply.
//!
//! `#[ignore]` because it needs the downloaded models (about 2 GB), like
//! `lens_images_live`. Run with
//!
//! ```sh
//! cd src-tauri
//! cargo test --release --test lens_scroll_live -- --ignored --nocapture
//! ```

use std::{path::PathBuf, sync::Arc, time::Instant};

use image::DynamicImage;
use moonlens_lib::{
    pipeline::{LensBlock, Pipeline},
    plugins::registry::PluginRegistry,
    region_watcher::{Frame, Geometry, LiveEvent, Tick, TickEnv, Tracker},
    settings::SettingsStore,
    storage::Storage,
};
use parking_lot::Mutex;

/// Region size and the page rows at its top, tick by tick. A repeated
/// value is the page holding still.
const SIZE: (u32, u32) = (860, 620);
const TOPS: &[u32] = &[0, 0, 120, 120, 250, 330, 470, 600, 600, 520, 380, 380];

fn frames() -> Vec<DynamicImage> {
    if let Ok(dir) = std::env::var("SCROLL_FRAMES") {
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "png"))
            .collect();
        files.sort();
        return files.iter().map(|p| image::open(p).unwrap()).collect();
    }
    let page = image::open(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lens/scroll_article.png"),
    )
    .unwrap();
    TOPS.iter()
        .map(|&top| page.crop_imm(0, top, SIZE.0, SIZE.1))
        .collect()
}

fn pipeline() -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins)
}

struct Run<'a> {
    pipeline: &'a Pipeline,
    tracker: Tracker,
    geometry: Geometry,
}

impl Run<'_> {
    async fn tick(&mut self, image: &DynamicImage) -> (Tick, Vec<LiveEvent>, u128) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let emit = move |ev: LiveEvent| sink.lock().push(ev);
        let geometry = self.geometry;
        let geometry_now = move || Some(geometry);
        let env = TickEnv {
            pipeline: self.pipeline,
            target_language: "ja",
            history_kind: "lens",
            emit: &emit,
            geometry_now: &geometry_now,
        };
        let frame = Frame {
            image: image.clone(),
            geometry,
        };
        let t = Instant::now();
        let tick = self.tracker.tick(frame, false, &env).await.unwrap();
        let ms = t.elapsed().as_millis();
        let events = std::mem::take(&mut *events.lock());
        (tick, events, ms)
    }
}

fn blocks_of(events: &[LiveEvent], last: bool) -> Option<Vec<LensBlock>> {
    let mut all = events.iter().filter_map(|e| match e {
        LiveEvent::Blocks { blocks, .. } => Some(blocks.clone()),
        _ => None,
    });
    if last {
        all.last()
    } else {
        all.next()
    }
}

fn head(s: &str) -> String {
    s.replace('\n', " ").chars().take(34).collect()
}

/// The same paragraph in two readings: one may lack the rows above the
/// region, and recognition may differ in a character or two.
fn alike(a: &str, b: &str) -> bool {
    let start = |s: &str| s.chars().take(24).collect::<String>();
    a.contains(b) || b.contains(a) || start(a) == start(b)
}

fn in_view(b: &LensBlock, height: f32) -> bool {
    b.bbox[1] >= 12.0 && b.bbox[1] + b.bbox[3] <= height - 12.0
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs the downloaded models"]
async fn a_scrolled_page_keeps_its_translations_and_reads_only_new_rows() {
    let frames = frames();
    let custom_frames = std::env::var("SCROLL_FRAMES").is_ok();
    let pipeline = pipeline();
    pipeline.warm_up().await;
    let geometry: Geometry = [0, 0, frames[0].width() as i32, frames[0].height() as i32];
    let height = frames[0].height() as f32;
    let mut run = Run {
        pipeline: &pipeline,
        tracker: Tracker::new(),
        geometry,
    };

    let mut shown: Vec<LensBlock> = Vec::new();
    let mut requests = 0;
    let mut scroll_ms: Vec<u128> = Vec::new();
    for (i, frame) in frames.iter().enumerate() {
        let still = i > 0 && frames[i - 1].as_bytes() == frame.as_bytes();
        let (tick, events, ms) = run.tick(frame).await;
        println!(
            "[{i:02}] {ms:>5} ms {}",
            match &tick {
                Tick::Translated { requested, .. } => format!("Translated (request: {requested})"),
                other => format!("{other:?}"),
            }
        );
        if i == 0 {
            assert_eq!(tick, Tick::Settling);
            continue;
        }
        if !still {
            // 1. The shown translations go along at once.
            assert!(
                matches!(tick, Tick::Settling | Tick::SameText),
                "a scroll step asked the model: {tick:?}"
            );
            scroll_ms.push(ms);
            assert!(
                matches!(events.first(), Some(LiveEvent::Blocks { .. })),
                "the first event of a scroll step is not the moved blocks: {events:?}"
            );
            let moved = blocks_of(&events, false).unwrap();
            assert_eq!(moved.len(), shown.len());
            // Every block moved by the same distance, or stayed (a
            // part of the picture that does not scroll).
            let (mut distance, mut stayed) = (0.0f32, 0);
            for (old, new) in shown.iter().zip(&moved) {
                assert_eq!(old.source, new.source);
                assert_eq!(old.translation, new.translation, "{}", head(&old.source));
                let dy = new.bbox[1] - old.bbox[1];
                if dy == 0.0 {
                    stayed += 1;
                    continue;
                }
                if distance == 0.0 {
                    distance = dy;
                }
                assert!((distance - dy).abs() < 0.01, "blocks moved apart");
            }
            let hidden: Vec<usize> = events
                .iter()
                .filter_map(|e| match e {
                    LiveEvent::Stale { indices } => Some(indices.clone()),
                    _ => None,
                })
                .flatten()
                .collect();
            println!(
                "       moved {distance} px: {} translations went along, {stayed} stayed, {} hidden",
                moved.len() - stayed - hidden.len(),
                hidden.len()
            );
            // The article has nothing a paragraph could slide under.
            assert!(
                custom_frames || (hidden.is_empty() && stayed == 0),
                "translations hidden or left behind by a scroll: {hidden:?}"
            );
            shown = moved;
            continue;
        }
        // The page holds still: what is new is translated now.
        let Tick::Translated { requested, .. } = &tick else {
            panic!("[{i}] expected a translation: {tick:?}");
        };
        requests += *requested as usize;
        let now = blocks_of(&events, true).unwrap();
        for b in &now {
            println!(
                "       y={:>5.0} h={:>4.0} cache={:<5} | {} → {}",
                b.bbox[1],
                b.bbox[3],
                b.from_cache,
                head(&b.source),
                head(b.translation.as_deref().unwrap_or("-"))
            );
            assert!(b.translation.is_some(), "{}", head(&b.source));
            // 3. A paragraph shown before reads and translates as it did.
            if let Some(before) = shown.iter().find(|o| o.source == b.source) {
                assert_eq!(before.translation, b.translation);
            }
        }
        // 4. A fresh reading of the whole region sees the same.
        let mut fresh = Run {
            pipeline: &pipeline,
            tracker: Tracker::new(),
            geometry,
        };
        fresh.tick(frame).await;
        let (_, events, full_ms) = fresh.tick(frame).await;
        let full = blocks_of(&events, true).unwrap();
        println!("       fresh reading of the whole region: {full_ms} ms (after its first tick)");
        // The detector draws a line's box a few pixels differently from
        // one picture to the next; more so next to a fixed header.
        let near = if custom_frames { 6.0 } else { 3.0 };
        for f in full.iter().filter(|f| in_view(f, height)) {
            // (A paragraph that began above the region is whole here
            // and starts at its first row in view in a fresh reading.)
            let same = now
                .iter()
                .find(|b| alike(&b.source, &f.source))
                .unwrap_or_else(|| {
                    let shown: Vec<&str> = now.iter().map(|b| b.source.as_str()).collect();
                    panic!("[{i}] {:?} missing after the scroll: {shown:#?}", f.source)
                });
            for k in (0..4).filter(|_| same.source == f.source) {
                assert!(
                    (same.bbox[k] - f.bbox[k]).abs() <= near,
                    "[{i}] {} is at {:?}, a fresh reading puts it at {:?}",
                    head(&f.source),
                    same.bbox,
                    f.bbox
                );
            }
        }
        for b in now.iter().filter(|b| in_view(b, height)) {
            assert!(
                full.iter().any(|f| alike(&b.source, &f.source)),
                "[{i}] {:?} is shown but a fresh reading does not see it",
                head(&b.source)
            );
        }
        shown = now;
    }
    scroll_ms.sort_unstable();
    println!(
        "scroll steps: median {} ms, slowest {} ms; model requests after the first: {}",
        scroll_ms[scroll_ms.len() / 2],
        scroll_ms.last().unwrap(),
        requests - 1
    );
}
