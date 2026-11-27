//! Reading modes of a window-translation window (`ReadMode`).
//!
//! Subtitles: a line that is being typed out is translated once, when
//! it is complete, and with the lines read before it as context.
//! Interface text: nothing is carried from one reading to the next, and
//! text that keeps changing is shown after the usual wait.
//!
//! Drives the shared [`Tracker`] with stub OCR / translate providers and
//! records what the translator is asked.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use image::{DynamicImage, Rgba, RgbaImage};
use moonlens_lib::{
    pipeline::Pipeline,
    providers::{
        Capability, OcrLine, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
        TranslateContext,
    },
    region_watcher::{Frame, Geometry, LiveEvent, ReadMode, Tick, TickEnv, Tracker},
    settings::{
        schema::{Settings, TaskChain},
        SettingsStore,
    },
    storage::Storage,
};
use parking_lot::Mutex;

/// OCR stub: one line per text set by the test, far enough apart to be
/// blocks of their own.
struct StubOcr(Arc<Mutex<Vec<String>>>);

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
                .lock()
                .iter()
                .enumerate()
                .map(|(i, text)| OcrLine {
                    text: text.clone(),
                    bbox: [10.0, 20.0 + 150.0 * i as f32, 300.0, 20.0],
                    confidence: 1.0,
                })
                .collect(),
        })
    }
}

type Requests = Arc<Mutex<Vec<(String, TranslateContext)>>>;

/// Translator stub that takes context, like the built-in engine, and
/// records every request.
struct Recorder(Requests);

#[async_trait]
impl Provider for Recorder {
    fn id(&self) -> &str {
        "rec"
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Translate]
    }
    fn is_local(&self) -> bool {
        true
    }
    fn takes_translation_context(&self) -> Option<bool> {
        Some(true)
    }
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Translate { text, context, .. } = input else {
            unreachable!()
        };
        self.0.lock().push((text.clone(), context));
        Ok(TaskOutput::Text(format!("[{text}]")))
    }
}

const GEOMETRY: Geometry = [0, 0, 800, 400];

struct Run {
    pipeline: Pipeline,
    tracker: Tracker,
    mode: ReadMode,
    lines: Arc<Mutex<Vec<String>>>,
    requests: Requests,
    target: &'static str,
    app: Option<&'static str>,
    /// Every picture differs from the one before, like a screen whose
    /// text changes.
    pictures: u32,
}

impl Run {
    fn new(mode: ReadMode) -> Self {
        let requests: Requests = Arc::default();
        let mut run = Self::with(mode, Arc::new(Recorder(requests.clone())));
        run.requests = requests;
        run
    }

    fn with(mode: ReadMode, translator: ProviderHandle) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let store = SettingsStore::load_from(tmp.path().join("config.toml")).unwrap();
        let mut s = Settings::default();
        s.providers.ocr = TaskChain {
            chain: vec!["stub_ocr".into()],
            timeout_ms: 1000,
            ..TaskChain::default()
        };
        s.providers.translate = TaskChain {
            chain: vec![translator.id().to_string()],
            timeout_ms: 120_000,
            ..TaskChain::default()
        };
        // No QR provider in the chain: the QR step fails fast and OCR runs.
        s.providers.qr_handler.chain = vec![];
        store.save(s).unwrap();
        std::mem::forget(tmp);

        let lines: Arc<Mutex<Vec<String>>> = Arc::default();
        let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
        providers.insert("stub_ocr".into(), Arc::new(StubOcr(lines.clone())));
        providers.insert(translator.id().to_string(), translator);
        Self {
            pipeline: Pipeline::with_providers(
                Arc::new(store),
                Arc::new(Storage::open_in_memory().unwrap()),
                providers,
            ),
            tracker: Tracker::new(),
            mode,
            lines,
            requests: Arc::default(),
            target: "ja",
            app: None,
            pictures: 0,
        }
    }

    /// One tick with the screen showing `lines`. `changed`: the picture
    /// differs from the last tick's.
    async fn tick(&mut self, lines: &[&str], changed: bool) -> Tick {
        *self.lines.lock() = lines.iter().map(|l| l.to_string()).collect();
        self.pictures += u32::from(changed);
        let mut img = RgbaImage::from_pixel(800, 400, Rgba([255, 255, 255, 255]));
        for yy in 20..40 {
            for xx in 0..(self.pictures * 60).min(790) {
                img.put_pixel(xx, yy, Rgba([0, 0, 0, 255]));
            }
        }
        let frame = Frame {
            image: DynamicImage::ImageRgba8(img),
            geometry: GEOMETRY,
            visible: None,
        };
        let emit = |_: LiveEvent| {};
        let geometry_now = || Some(GEOMETRY);
        let env = TickEnv {
            pipeline: &self.pipeline,
            target_language: self.target,
            mode: self.mode,
            history_kind: "lens",
            app: self.app,
            emit: &emit,
            geometry_now: &geometry_now,
        };
        self.tracker.tick(frame, false, &env).await.unwrap()
    }

    /// Show `lines` until they are translated (read twice). Returns the
    /// translation.
    async fn show(&mut self, lines: &[&str]) -> String {
        assert_eq!(self.tick(lines, true).await, Tick::Settling);
        match self.tick(lines, false).await {
            Tick::Translated { translation, .. } => translation,
            tick => panic!("{tick:?}"),
        }
    }

    fn moved(&mut self) {
        let emit = |_: LiveEvent| {};
        let geometry_now = || Some(GEOMETRY);
        let env = TickEnv {
            pipeline: &self.pipeline,
            target_language: self.target,
            mode: self.mode,
            history_kind: "lens",
            app: self.app,
            emit: &emit,
            geometry_now: &geometry_now,
        };
        self.tracker.region_moved(GEOMETRY, &env);
    }

    fn requests(&self) -> Vec<(String, TranslateContext)> {
        std::mem::take(&mut *self.requests.lock())
    }
}

/// A line typed out one word per tick, longer than the wait for text
/// that never settles.
const TYPED: [&str; 10] = [
    "The",
    "The north",
    "The north wind",
    "The north wind and",
    "The north wind and the",
    "The north wind and the sun",
    "The north wind and the sun were",
    "The north wind and the sun were arguing",
    "The north wind and the sun were arguing about",
    "The north wind and the sun were arguing about strength.",
];

#[tokio::test]
async fn a_subtitle_is_translated_once_it_is_typed_out_in_full() {
    let mut run = Run::new(ReadMode::Subtitle);
    for text in TYPED {
        assert_eq!(run.tick(&[text], true).await, Tick::Settling, "{text}");
    }
    assert!(run.requests().is_empty(), "translated half-way");

    let tick = run.tick(&[TYPED[9]], false).await;
    assert!(
        matches!(&tick, Tick::Translated { source, .. } if source == TYPED[9]),
        "{tick:?}"
    );
    let requests = run.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, TYPED[9]);
}

#[tokio::test]
async fn interface_text_that_keeps_changing_is_translated_after_the_usual_wait() {
    let mut run = Run::new(ReadMode::Ui);
    let mut translated = None;
    for (i, text) in TYPED.iter().enumerate() {
        if let Tick::Translated { source, .. } = run.tick(&[text], true).await {
            translated = Some((i, source));
            break;
        }
    }
    // Six readings wait, the seventh is shown as it is.
    assert_eq!(translated, Some((6, TYPED[6].to_string())));
}

#[tokio::test]
async fn text_that_changes_without_growing_does_not_wait_longer_as_a_subtitle() {
    let mut run = Run::new(ReadMode::Subtitle);
    let clock = [
        "10:01", "10:02", "10:03", "10:04", "10:05", "10:06", "10:07",
    ];
    for text in &clock[..6] {
        assert_eq!(run.tick(&[text], true).await, Tick::Settling);
    }
    let tick = run.tick(&[clock[6]], true).await;
    assert!(matches!(tick, Tick::Translated { .. }), "{tick:?}");
}

#[tokio::test]
async fn a_subtitle_is_translated_with_the_lines_before_it() {
    let mut run = Run::new(ReadMode::Subtitle);
    run.show(&["Who goes there?"]).await;
    assert!(run.requests()[0].1.history.is_empty());

    run.show(&["It is only the wind."]).await;
    run.show(&["Then close the gate."]).await;
    let requests = run.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].1.history,
        vec![
            (
                "Who goes there?".to_string(),
                "[Who goes there?]".to_string()
            ),
            (
                "It is only the wind.".to_string(),
                "[It is only the wind.]".to_string()
            ),
        ]
    );
    assert_eq!(requests[1].0, "Then close the gate.");
}

#[tokio::test]
async fn a_line_that_comes_back_after_other_lines_is_translated_again() {
    let mut run = Run::new(ReadMode::Subtitle);
    run.show(&["Who goes there?"]).await;
    run.show(&["I see."]).await;
    run.show(&["Then close the gate."]).await;
    run.requests();

    // The same words answer another line now: the translation made
    // after "Who goes there?" is not shown for them.
    run.show(&["I see."]).await;
    let requests = run.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "I see.");
    assert_eq!(
        requests[0].1.history.last().map(|(s, _)| s.as_str()),
        Some("Then close the gate.")
    );
}

#[tokio::test]
async fn a_line_shown_half_way_is_not_context_of_its_own_completion() {
    let mut run = Run::new(ReadMode::Subtitle);
    run.show(&["Who goes there?"]).await;
    // The typing paused long enough to be read twice.
    run.show(&["It is only"]).await;
    run.show(&["It is only the wind."]).await;
    let requests = run.requests();
    assert_eq!(requests[2].0, "It is only the wind.");
    assert_eq!(
        requests[2].1.history,
        vec![(
            "Who goes there?".to_string(),
            "[Who goes there?]".to_string()
        )]
    );

    // And only the complete line is context of the next one.
    run.show(&["Then close the gate."]).await;
    let sources: Vec<String> = run.requests()[0]
        .1
        .history
        .iter()
        .map(|(s, _)| s.clone())
        .collect();
    assert_eq!(sources, ["Who goes there?", "It is only the wind."]);
}

#[tokio::test]
async fn a_moved_window_starts_without_context() {
    let mut run = Run::new(ReadMode::Subtitle);
    run.show(&["Who goes there?"]).await;
    run.moved();
    run.show(&["Settings"]).await;
    let requests = run.requests();
    assert!(requests[1].1.is_empty(), "{:?}", requests[1].1);
}

#[tokio::test]
async fn interface_text_is_translated_without_earlier_readings() {
    let mut run = Run::new(ReadMode::Ui);
    run.show(&["Who goes there?"]).await;
    run.show(&["Save", "Load", "Quit to title"]).await;
    let requests = run.requests();
    assert_eq!(requests.len(), 2);
    // The blocks of one reading still go together: a label alone
    // ("Light", "Resume") is ambiguous.
    assert_eq!(requests[1].0, "Save\n\nLoad\n\nQuit to title");
    assert!(requests[1].1.is_empty(), "{:?}", requests[1].1);
}

#[tokio::test]
async fn switching_the_mode_drops_the_context() {
    let mut run = Run::new(ReadMode::Subtitle);
    run.show(&["Who goes there?"]).await;
    run.mode = ReadMode::Ui;
    run.show(&["Settings"]).await;
    assert!(run.requests()[1].1.is_empty());
    run.mode = ReadMode::Subtitle;
    run.show(&["It is only the wind."]).await;
    let requests = run.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].1.is_empty(), "{:?}", requests[0].1);
}

/// What the modes do to real translations. Prints, per mode, a dialogue
/// whose lines depend on the lines before them and a menu of short
/// labels:
/// `MOONLENS_TEST_GGUF=/path/plamo-3-translate-2b-Q4_K_M.gguf
///  cargo test --test lens_read_mode -- --ignored --nocapture`
#[cfg(feature = "local-llm")]
#[tokio::test]
#[ignore]
async fn real_model_translations_per_mode() {
    let Ok(path) = std::env::var("MOONLENS_TEST_GGUF") else {
        return;
    };
    let dialogue = [
        "船長はどこだ？",
        "一時間前に出て行ったよ。",
        "あれを持って行ったのか？",
        "ああ、箱ごと全部だ。",
        "なら追いかけるしかないな。",
    ];
    let menu = ["Save", "Load", "Light", "Dark", "Resume", "Quit to title"];
    let hud = [
        "Press E to open",
        "The old miller swears he saw lights in the mine last night.",
        "Back",
        "Party chat: brb, need a drink",
        "Iron Ore x12",
        "Free look",
    ];
    for mode in [ReadMode::Subtitle, ReadMode::Ui] {
        let translator = moonlens_lib::providers::translate_llama::LlamaTranslator::new(
            "llamacpp",
            &serde_json::json!({"model_path": path, "prompt_style": "pfml3"}),
            false,
        );
        let mut run = Run::with(mode, Arc::new(translator));
        run.target = "en";
        println!("{mode:?}: dialogue");
        for line in dialogue {
            let started = std::time::Instant::now();
            let translation = run.show(&[line]).await;
            println!(
                "  {line} → {translation} ({} ms)",
                started.elapsed().as_millis()
            );
            assert!(!translation.trim().is_empty());
        }
        run.moved();
        run.target = "ja";
        let started = std::time::Instant::now();
        let translation = run.show(&menu).await;
        println!(
            "{mode:?}: menu ({} ms)\n  {}",
            started.elapsed().as_millis(),
            translation.replace("\n\n", " | ")
        );
        assert_eq!(translation.split("\n\n").count(), menu.len());
        run.moved();
        let started = std::time::Instant::now();
        let translation = run.show(&hud).await;
        println!(
            "{mode:?}: mixed screen ({} ms)\n  {}",
            started.elapsed().as_millis(),
            translation.replace("\n\n", " | ")
        );
        assert_eq!(translation.split("\n\n").count(), hud.len());
    }
}

#[tokio::test]
async fn another_target_language_starts_without_context() {
    let mut run = Run::new(ReadMode::Subtitle);
    run.show(&["Who goes there?"]).await;
    // Translations into the old language are no examples for the new one.
    run.target = "fr";
    run.show(&["It is only the wind."]).await;
    let requests = run.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].1.is_empty(), "{:?}", requests[1].1);

    run.show(&["Then close the gate."]).await;
    let requests = run.requests();
    assert_eq!(requests[0].1.history.len(), 1);
    assert_eq!(requests[0].1.history[0].0, "It is only the wind.");
}

#[tokio::test]
async fn text_that_stays_on_screen_is_translated_again_for_another_app() {
    let mut run = Run::new(ReadMode::Ui);
    run.show(&["Options"]).await;
    run.requests();

    // The window now follows another application showing the same
    // words: what was made for the first one is not kept.
    run.app = Some("game");
    assert_eq!(run.tick(&["Options"], false).await, Tick::Settling);
    assert!(matches!(
        run.tick(&["Options"], false).await,
        Tick::Translated { .. }
    ));
    let requests = run.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].0, "Options");

    // Still the same application: nothing to do.
    assert_eq!(run.tick(&["Options"], false).await, Tick::Unchanged);
}

#[tokio::test]
async fn text_that_stays_on_screen_is_translated_again_in_another_language() {
    let mut run = Run::new(ReadMode::Ui);
    run.show(&["Options"]).await;
    run.requests();

    // Same language: what is shown is kept.
    run.show(&["Options", "Quit"]).await;
    let requests = run.requests();
    assert_eq!(requests.len(), 1, "{requests:?}");
    assert_eq!(requests[0].0, "Quit");

    // Another language: nothing made for the old one is shown again.
    run.target = "fr";
    run.show(&["Options", "Quit", "Credits"]).await;
    let requests = run.requests();
    let asked: Vec<&str> = requests.iter().flat_map(|(t, _)| t.split("\n\n")).collect();
    assert_eq!(asked, ["Options", "Quit", "Credits"]);
}
