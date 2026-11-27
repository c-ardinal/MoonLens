//! Cached translations and the lines a capture continues.
//!
//! A subtitle is translated with the subtitles before it
//! ([`CaptureFlowResult::earlier`]); the same words after other lines
//! can mean something else, so a cached translation is used again only
//! after the same lines. A block that simply stays on screen
//! ([`CaptureFlowResult::on_screen`]) keeps its translation.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use moonlens_lib::{
    pipeline::{CaptureFlowResult, LensBlock, Pipeline, TaskResult},
    providers::{
        Capability, Intent, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
        TranslateContext,
    },
    settings::{
        schema::{Settings, TaskChain},
        SettingsStore,
    },
    storage::Storage,
};
use parking_lot::Mutex;

type Seen = Arc<Mutex<Vec<(String, TranslateContext)>>>;

/// Translates each paragraph to itself tagged with the number of
/// history entries it was given, so a test can tell under which context
/// a shown translation was made.
struct Recorder {
    context: bool,
    seen: Seen,
}

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
        Some(self.context)
    }
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Translate { text, context, .. } = input else {
            unreachable!()
        };
        let after = context.history.len();
        self.seen.lock().push((text.clone(), context));
        let parts: Vec<String> = text.split("\n\n").map(|p| format!("{p}@{after}")).collect();
        Ok(TaskOutput::Text(parts.join("\n\n")))
    }
}

fn pipeline(context: bool, seen: Seen) -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::load_from(tmp.path().join("config.toml")).unwrap();
    let mut s = Settings::default();
    s.providers.translate = TaskChain {
        chain: vec!["rec".into()],
        timeout_ms: 1000,
        ..TaskChain::default()
    };
    s.storage.keep_history = false;
    store.save(s).unwrap();
    std::mem::forget(tmp);
    let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
    providers.insert("rec".into(), Arc::new(Recorder { context, seen }));
    Pipeline::with_providers(
        Arc::new(store),
        Arc::new(Storage::open_in_memory().unwrap()),
        providers,
    )
}

fn capture(texts: &[&str], earlier: &[(&str, &str)]) -> CaptureFlowResult {
    CaptureFlowResult {
        intent: Intent::Text,
        source_text: Some(texts.join("\n\n")),
        result: TaskResult {
            task: "ocr".into(),
            output: TaskOutput::Empty,
            used_provider: String::new(),
            via_cloud: false,
            latency_ms: 0,
            fallback_reasons: vec![],
        },
        source_rect: None,
        blocks: texts
            .iter()
            .enumerate()
            .map(|(i, t)| LensBlock {
                bbox: [0.0, i as f32 * 40.0, 600.0, 30.0],
                line_height: 16.0,
                source: t.to_string(),
                translation: None,
                colors: None,
                same_language: false,
                from_cache: false,
                align: Default::default(),
                bold: false,
                lines: Vec::new(),
            })
            .collect(),
        image_size: None,
        source_language: None,
        target_language: None,
        app: None,
        earlier: earlier
            .iter()
            .map(|(s, t)| (s.to_string(), t.to_string()))
            .collect(),
        on_screen: Vec::new(),
    }
}

fn shown(b: &CaptureFlowResult) -> Vec<&str> {
    b.blocks
        .iter()
        .map(|b| b.translation.as_deref().unwrap_or("-"))
        .collect()
}

const LINE: &str = "Left an hour ago.";
const ABOUT_HIM: &[(&str, &str)] = &[("Where is Tom?", "トムはどこ？")];
const ABOUT_ME: &[(&str, &str)] = &[
    ("You look tired.", "疲れているね。"),
    ("When did you leave the office?", "いつ退社したの？"),
];

#[tokio::test]
async fn earlier_lines_reach_the_translator_as_history() {
    let seen: Seen = Arc::default();
    let p = pipeline(true, seen.clone());
    let mut b = capture(&[LINE], ABOUT_HIM);
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    let seen = seen.lock();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0].1.history,
        [("Where is Tom?".to_string(), "トムはどこ？".to_string())]
    );
}

#[tokio::test]
async fn the_same_words_after_other_lines_are_translated_again() {
    let seen: Seen = Arc::default();
    let p = pipeline(true, seen.clone());

    let mut b = capture(&[LINE], ABOUT_HIM);
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    assert_eq!(shown(&b), ["Left an hour ago.@1"]);

    // Another conversation: not the translation made about Tom.
    let mut b = capture(&[LINE], ABOUT_ME);
    p.prepare_translation(&mut b, "ja");
    assert!(
        b.blocks[0].needs_request(),
        "nothing shown ahead of the model"
    );
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    assert_eq!(shown(&b), ["Left an hour ago.@2"]);
    assert!(!b.blocks[0].from_cache);

    // Alone (a menu, another window): not a translation made in a
    // conversation.
    let mut b = capture(&[LINE], &[]);
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    assert_eq!(shown(&b), ["Left an hour ago.@0"]);
    assert_eq!(seen.lock().len(), 3);

    // The same lines before it again (the scene replayed): instant.
    let mut b = capture(&[LINE], ABOUT_ME);
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    assert_eq!(shown(&b), ["Left an hour ago.@2"]);
    assert!(b.blocks[0].from_cache);
    assert_eq!(seen.lock().len(), 3);
}

#[tokio::test]
async fn a_block_that_stays_on_screen_keeps_its_translation() {
    let seen: Seen = Arc::default();
    let p = pipeline(true, seen.clone());

    let mut first = capture(&["Chapter 3", LINE], &[]);
    p.translate_capture_to(&mut first, "ja").await.unwrap();
    assert_eq!(seen.lock().len(), 1);

    // The next reading: the heading is still there, the line under it
    // is new. What was read before is now context.
    let whole = (
        first.source_text.clone().unwrap(),
        shown(&first).join("\n\n"),
    );
    let mut next = capture(&["Chapter 3", "He never came back."], &[]);
    next.earlier = vec![whole];
    next.on_screen = first
        .blocks
        .iter()
        .map(|b| (b.source.clone(), b.translation.clone().unwrap()))
        .collect();
    p.prepare_translation(&mut next, "ja");
    assert_eq!(next.blocks[0].translation.as_deref(), Some("Chapter 3@0"));
    assert!(!next.blocks[0].needs_request());
    assert!(next.blocks[1].needs_request());

    p.translate_capture_to(&mut next, "ja").await.unwrap();
    assert_eq!(shown(&next), ["Chapter 3@0", "He never came back.@1"]);
    let seen = seen.lock();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[1].0, "He never came back.", "only the new block");
}

#[tokio::test]
async fn a_translator_that_reads_no_context_keeps_one_translation_per_text() {
    let seen: Seen = Arc::default();
    let p = pipeline(false, seen.clone());
    let mut b = capture(&[LINE], ABOUT_HIM);
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    assert!(seen.lock()[0].1.is_empty());

    let mut b = capture(&[LINE], ABOUT_ME);
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    assert!(b.blocks[0].from_cache);
    assert_eq!(seen.lock().len(), 1);
}
