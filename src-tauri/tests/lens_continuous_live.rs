//! End-to-end test of continuous Lens reading: a sequence of changing
//! screen images goes through the shared change-driven [`Tracker`] (the
//! same code the Lens window and live translation run each tick) with the
//! real providers (PP-OCR on rten, the built-in llama.cpp translator).
//!
//! Asserted along the sequence:
//!
//! 1. A first reading is translated (Japanese, key terms present).
//! 2. An unchanged frame costs nothing: no OCR, no event.
//! 3. New text is translated once it reads the same twice.
//! 4. Text seen before comes from the cache: no model request.
//! 5. When only part of the region changes, only that part is hidden and
//!    sent to the model; the rest keeps its translation.
//! 6. Moving the lens hides every box; re-reading the same text shows the
//!    kept translations at the new place without a model request.
//!
//! `#[ignore]` because it needs the downloaded models (about 2 GB), like
//! `lens_images_live`. Run with
//!
//! ```sh
//! cd src-tauri
//! cargo test --release --test lens_continuous_live -- --ignored --nocapture
//! ```

use std::{path::PathBuf, sync::Arc, time::Instant};

use image::{DynamicImage, GenericImage, Rgba, RgbaImage};
use moonlens_lib::{
    pipeline::Pipeline,
    plugins::registry::PluginRegistry,
    region_watcher::{Frame, Geometry, LiveEvent, ReadMode, Tick, TickEnv, Tracker},
    settings::SettingsStore,
    storage::Storage,
};
use parking_lot::Mutex;

fn fixture(name: &str) -> DynamicImage {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lens")
        .join(format!("{name}.png"));
    image::open(p).unwrap()
}

/// `top` above `bottom` on one white 920x880 canvas: a screen where a
/// region keeps its size while part of its content changes.
fn screen(top: &str, bottom: Option<&str>) -> DynamicImage {
    let mut canvas = RgbaImage::from_pixel(920, 880, Rgba([255, 255, 255, 255]));
    canvas.copy_from(&fixture(top).to_rgba8(), 0, 0).unwrap();
    if let Some(b) = bottom {
        canvas.copy_from(&fixture(b).to_rgba8(), 0, 300).unwrap();
    }
    DynamicImage::ImageRgba8(canvas)
}

fn pipeline() -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins)
}

struct Run {
    pipeline: Pipeline,
    tracker: Tracker,
    events: Arc<Mutex<Vec<LiveEvent>>>,
    geometry: Arc<Mutex<Geometry>>,
    target: &'static str,
}

impl Run {
    async fn tick(&mut self, step: &str, image: DynamicImage) -> (Tick, Vec<LiveEvent>, u128) {
        let events = self.events.clone();
        let emit = move |ev: LiveEvent| events.lock().push(ev);
        let geo = self.geometry.clone();
        let geometry_now = move || Some(*geo.lock());
        let env = TickEnv {
            pipeline: &self.pipeline,
            target_language: self.target,
            mode: ReadMode::default(),
            history_kind: "lens",
            app: None,
            emit: &emit,
            geometry_now: &geometry_now,
        };
        let geometry = *self.geometry.lock();
        let t = Instant::now();
        let tick = self
            .tracker
            .tick(
                Frame {
                    image,
                    geometry,
                    visible: None,
                },
                false,
                &env,
            )
            .await
            .unwrap_or_else(|e| panic!("{step}: {e}"));
        let ms = t.elapsed().as_millis();
        let evs = std::mem::take(&mut *self.events.lock());
        println!("[{step}] {ms} ms → {}", describe(&tick));
        for ev in &evs {
            println!("    event {}", describe_event(ev));
            if let LiveEvent::Blocks {
                blocks,
                translating: false,
                ..
            } = ev
            {
                for b in blocks {
                    println!(
                        "      y={:>4.0} cache={} same={} | {} → {}",
                        b.bbox[1],
                        b.from_cache,
                        b.same_language,
                        b.source
                            .replace('\n', " ")
                            .chars()
                            .take(50)
                            .collect::<String>(),
                        b.translation
                            .as_deref()
                            .unwrap_or("-")
                            .replace('\n', " ")
                            .chars()
                            .take(40)
                            .collect::<String>()
                    );
                }
            }
        }
        (tick, evs, ms)
    }

    fn moved(&mut self, to: Geometry) -> Vec<LiveEvent> {
        *self.geometry.lock() = to;
        let events = self.events.clone();
        let emit = move |ev: LiveEvent| events.lock().push(ev);
        let geometry_now = move || Some(to);
        let env = TickEnv {
            pipeline: &self.pipeline,
            target_language: "ja",
            mode: ReadMode::default(),
            history_kind: "lens",
            app: None,
            emit: &emit,
            geometry_now: &geometry_now,
        };
        self.tracker.region_moved(to, &env);
        std::mem::take(&mut *self.events.lock())
    }
}

fn describe(t: &Tick) -> String {
    match t {
        Tick::Translated {
            translation,
            requested,
            ..
        } => format!(
            "Translated (model request: {requested}): {}",
            translation
                .replace('\n', " ")
                .chars()
                .take(160)
                .collect::<String>()
        ),
        other => format!("{other:?}"),
    }
}

fn describe_event(ev: &LiveEvent) -> String {
    match ev {
        LiveEvent::Blocks {
            blocks,
            translating,
            ..
        } => format!(
            "Blocks x{} translating={translating} cached={} pending={}",
            blocks.len(),
            blocks.iter().filter(|b| b.from_cache).count(),
            blocks
                .iter()
                .filter(|b| b.translation.is_none() && !b.same_language)
                .count()
        ),
        other => format!("{other:?}"),
    }
}

fn translation_of(t: &Tick) -> &str {
    match t {
        Tick::Translated { translation, .. } => translation,
        other => panic!("expected a translation, got {other:?}"),
    }
}

fn requested(t: &Tick) -> bool {
    matches!(
        t,
        Tick::Translated {
            requested: true,
            ..
        }
    )
}

fn assert_terms(step: &str, text: &str, groups: &[&[&str]]) {
    for g in groups {
        assert!(
            g.iter().any(|t| text.contains(t)),
            "{step}: none of {g:?} in translation: {text}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: needs the PP-OCR and plamo-3-translate-2b models"]
async fn lens_reads_continuously_and_retranslates_only_changes() {
    let mut run = Run {
        pipeline: pipeline(),
        tracker: Tracker::new(),
        events: Arc::default(),
        geometry: Arc::new(Mutex::new([100, 100, 920, 880])),
        target: "ja",
    };
    let wind = || screen("english0002", None);
    let rain = || screen("english0003", None);

    // 1. First reading: settle once, then translate.
    let (t, _, _) = run.tick("1a wind first", wind()).await;
    assert_eq!(t, Tick::Settling);
    let (t, ev, _) = run.tick("1b wind again", wind()).await;
    assert!(requested(&t), "first text goes to the model");
    if let (Ok(path), Some(LiveEvent::Blocks { blocks, .. })) =
        (std::env::var("LENS_DUMP"), ev.last())
    {
        // Blocks for a renderer check of the Lens page.
        std::fs::write(path, serde_json::to_vec(blocks).unwrap()).unwrap();
    }
    assert_terms("1b", translation_of(&t), &[&["北風"], &["太陽"]]);
    assert!(matches!(
        ev.last(),
        Some(LiveEvent::Blocks {
            translating: false,
            ..
        })
    ));

    // 2. Unchanged frames: no OCR, no events.
    for i in 0..3 {
        let (t, ev, ms) = run.tick(&format!("2 unchanged #{i}"), wind()).await;
        assert_eq!(t, Tick::Unchanged);
        assert!(ev.is_empty());
        assert!(ms < 200, "an unchanged frame must stay cheap ({ms} ms)");
    }

    // 3. The screen changes to another text.
    let (t, ev, _) = run.tick("3a rain first", rain()).await;
    assert_eq!(t, Tick::Settling);
    assert!(
        ev.iter().any(|e| matches!(e, LiveEvent::Stale { .. })),
        "old translations are hidden at once"
    );
    let (t, _, _) = run.tick("3b rain again", rain()).await;
    assert!(requested(&t));
    assert_terms("3b", translation_of(&t), &[&["虹"], &["雨"]]);

    // 4. Back to the first text: cache, no model request.
    run.tick("4a wind first", wind()).await;
    let (t, _, ms) = run.tick("4b wind again", wind()).await;
    assert!(!requested(&t), "text seen before must come from the cache");
    assert_terms("4b", translation_of(&t), &[&["北風"], &["太陽"]]);
    println!("    cached re-translation in {ms} ms (OCR only)");

    // 5. Only the lower part changes.
    run.tick("5a wind+rain", screen("english0002", Some("english0003")))
        .await;
    // Stacked on one canvas OCR may group the lower paragraphs
    // differently, so some blocks can be new here.
    run.tick("5b wind+rain", screen("english0002", Some("english0003")))
        .await;
    let (t, ev, _) = run
        .tick(
            "5c wind+fox",
            screen("english0002", Some("synth_title_body")),
        )
        .await;
    assert_eq!(t, Tick::Settling);
    let stale: Vec<usize> = ev
        .iter()
        .flat_map(|e| match e {
            LiveEvent::Stale { indices } => indices.clone(),
            _ => vec![],
        })
        .collect();
    let (t, ev, _) = run
        .tick(
            "5d wind+fox",
            screen("english0002", Some("synth_title_body")),
        )
        .await;
    assert!(requested(&t), "the new lower paragraph goes to the model");
    assert_terms(
        "5d",
        translation_of(&t),
        &[&["北風"], &["狐", "キツネ", "きつね"], &["犬"]],
    );
    let Some(LiveEvent::Blocks { blocks, .. }) = ev.first() else {
        panic!("blocks shown before the model answers: {ev:?}")
    };
    let top: Vec<_> = blocks.iter().filter(|b| b.bbox[1] < 290.0).collect();
    assert!(!top.is_empty());
    assert!(
        top.iter()
            .all(|b| (b.from_cache || b.same_language) && b.translation.is_some()),
        "the unchanged upper paragraph keeps its translation without a request"
    );
    assert!(!stale.is_empty(), "the changed lower part was hidden");
    assert!(
        stale.iter().all(|&i| i > 0),
        "the unchanged upper paragraph was not hidden: {stale:?}"
    );

    // 6. The lens moves over the same text.
    let ev = run.moved([140, 160, 920, 880]);
    assert!(matches!(ev.as_slice(), [LiveEvent::Stale { .. }]), "{ev:?}");
    let (t, ev, _) = run
        .tick(
            "6 moved, same text",
            screen("english0002", Some("synth_title_body")),
        )
        .await;
    assert_eq!(t, Tick::SameText);
    // The region is read again at the new place (`Reading` on and off).
    let shown: Vec<&LiveEvent> = ev
        .iter()
        .filter(|e| !matches!(e, LiveEvent::Reading { .. }))
        .collect();
    assert!(
        matches!(
            shown.as_slice(),
            [LiveEvent::Blocks {
                translating: false,
                ..
            }]
        ),
        "kept translations shown again at the new place: {ev:?}"
    );

    // 7. The target language changes with the screen unchanged: read and
    //    translated again into the new language.
    run.target = "zh";
    let screen_now = || screen("english0002", Some("synth_title_body"));
    let (t, _, _) = run.tick("7a target zh", screen_now()).await;
    assert_eq!(
        t,
        Tick::Settling,
        "a new target language is not 'unchanged'"
    );
    let (t, _, _) = run.tick("7b target zh", screen_now()).await;
    assert!(requested(&t), "nothing cached in the new language");
    let zh = translation_of(&t);
    assert!(
        !zh.contains("ました") && zh.contains("狐"),
        "translated into Chinese, not the Japanese kept: {zh}"
    );
}

/// The text of `synth_title_body` in white on a picture: a diagonal
/// colour gradient, moved by `shift` pixels (a video behind a caption).
fn on_picture(shift: u32) -> DynamicImage {
    let text = fixture("synth_title_body").to_rgba8();
    let img = RgbaImage::from_fn(text.width(), text.height(), |x, y| {
        let t = (x + y + shift) % 256;
        let v = (if t < 128 { t } else { 255 - t } * 2) as f32;
        let back = [v / 2.0, 40.0 + v / 3.0, 150.0 - v / 2.0];
        // Black-on-white glyphs become white ink, edges kept smooth.
        let ink = 1.0 - text.get_pixel(x, y)[0] as f32 / 255.0;
        let c = back.map(|b| (b + (255.0 - b) * ink) as u8);
        Rgba([c[0], c[1], c[2], 255])
    });
    DynamicImage::ImageRgba8(img)
}

fn erased(ev: &[LiveEvent]) -> Option<usize> {
    ev.iter().rev().find_map(|e| match e {
        LiveEvent::Blocks { blocks, .. } => {
            Some(blocks.iter().filter(|b| b.erase.is_some()).count())
        }
        _ => None,
    })
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: needs the PP-OCR and plamo-3-translate-2b models"]
async fn text_on_a_picture_is_erased_only_while_the_picture_holds_still() {
    let size = fixture("synth_title_body");
    let mut run = Run {
        pipeline: pipeline(),
        tracker: Tracker::new(),
        events: Arc::default(),
        geometry: Arc::new(Mutex::new([
            0,
            0,
            size.width() as i32,
            size.height() as i32,
        ])),
        target: "ja",
    };

    // A still picture: the text is erased pixel by pixel.
    let (t, _, _) = run.tick("1a picture first", on_picture(0)).await;
    assert_eq!(t, Tick::Settling);
    let (t, ev, _) = run.tick("1b picture again", on_picture(0)).await;
    assert!(requested(&t));
    assert!(
        erased(&ev).is_some_and(|n| n > 0),
        "text on a still picture gets the erase picture"
    );

    // The picture moves behind the text, tick after tick: whatever is
    // shown from now on is the one-colour box. (Text recognition may or
    // may not read the same text again over the moved picture; both
    // paths have to drop the still erase picture.)
    let mut shown = None;
    for step in 1..=4 {
        let (_, ev, _) = run
            .tick(&format!("2 picture moved #{step}"), on_picture(step * 64))
            .await;
        shown = erased(&ev).or(shown);
    }
    assert_eq!(shown, Some(0), "no still picture over a moving one");
}
