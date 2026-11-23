//! Long captures are translated in parts, each with the earlier parts as
//! history, when the translator reads `TranslateContext`.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use moonlens_lib::{
    layout::Align,
    pipeline::{CaptureFlowResult, LensBlock, Pipeline, TaskResult, TranslationProgress},
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

/// Wraps each paragraph in brackets. `context` decides whether it claims
/// to read `TranslateContext`; `merge_from` makes multi-paragraph
/// requests whose text contains it come back as one paragraph.
struct Recorder {
    context: bool,
    merge_from: Option<&'static str>,
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
        self.seen.lock().push((text.clone(), context));
        let parts: Vec<String> = text.split("\n\n").map(|p| format!("[{p}]")).collect();
        let merge = self.merge_from.is_some_and(|m| text.contains(m)) && parts.len() > 1;
        Ok(TaskOutput::Text(parts.join(if merge {
            " "
        } else {
            "\n\n"
        })))
    }
}

fn pipeline(context: bool, merge_from: Option<&'static str>, seen: Seen) -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::load_from(tmp.path().join("config.toml")).unwrap();
    let mut s = Settings::default();
    s.providers.translate = TaskChain {
        chain: vec!["rec".into()],
        timeout_ms: 1000,
        ..TaskChain::default()
    };
    s.subtitle.target_language = "xx".into();
    store.save(s).unwrap();
    std::mem::forget(tmp);
    let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
    providers.insert(
        "rec".into(),
        Arc::new(Recorder {
            context,
            merge_from,
            seen,
        }),
    );
    Pipeline::with_providers(
        Arc::new(store),
        Arc::new(Storage::open_in_memory().unwrap()),
        providers,
    )
}

fn bundle(texts: &[String]) -> CaptureFlowResult {
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
            .map(|t| LensBlock {
                bbox: [0.0; 4],
                line_height: 16.0,
                source: t.clone(),
                translation: None,
                colors: None,
                same_language: false,
                from_cache: false,
                align: Align::Left,
                bold: false,
                lines: vec![],
            })
            .collect(),
        image_size: Some([100, 100]),
        source_language: None,
        target_language: None,
    }
}

/// A heading plus paragraphs long enough to need several parts.
fn article() -> Vec<String> {
    let mut v = vec!["Heading".to_string()];
    for i in 0..8 {
        v.push(format!("Paragraph {i} {}", "word ".repeat(20)));
    }
    v
}

#[tokio::test]
async fn long_capture_goes_out_in_parts_with_history() {
    let seen: Seen = Arc::default();
    let p = pipeline(true, None, seen.clone());
    let texts = article();
    let mut b = bundle(&texts);
    let events: Arc<Mutex<Vec<TranslationProgress>>> = Arc::default();
    let ev = events.clone();
    let progress = move |e: &TranslationProgress| ev.lock().push(e.clone());
    p.translate_capture_with_progress(&mut b, "ja", Some(&progress))
        .await
        .unwrap();

    let seen = seen.lock();
    assert!(
        seen.len() >= 3,
        "expected several requests, got {}",
        seen.len()
    );
    // The first part is small: the heading and the first paragraph.
    assert_eq!(seen[0].0, texts[..2].join("\n\n"));
    assert!(seen[0].1.history.is_empty());
    // It is read with the text after it as reference, and every request
    // carries the same reference so the prompt prefix stays the same.
    let reference = seen[0].1.reference.clone().expect("reference context");
    assert!(reference.starts_with("Paragraph 1"));
    assert!(seen
        .iter()
        .all(|(_, c)| c.reference.as_ref() == Some(&reference)));
    // Each later request has every earlier part with its translation.
    for k in 1..seen.len() {
        assert_eq!(seen[k].1.history.len(), k);
        assert_eq!(seen[k].1.history[k - 1].0, seen[k - 1].0);
    }
    // Every block got its own translation.
    for (blk, src) in b.blocks.iter().zip(&texts) {
        assert_eq!(
            blk.translation.as_deref(),
            Some(format!("[{src}]").as_str())
        );
    }
    // One progress event per request, streaming into consecutive blocks.
    let events = events.lock();
    assert_eq!(events.len(), seen.len());
    let streamed: Vec<usize> = events.iter().flat_map(|e| e.streaming.clone()).collect();
    assert_eq!(streamed, (0..texts.len()).collect::<Vec<_>>());
    // Blocks of earlier parts are already translated when the next starts.
    assert!(events[1].blocks[0].translation.is_some());
    assert!(events[1].blocks[events[1].streaming[0]]
        .translation
        .is_none());
    match &b.result.output {
        TaskOutput::Text(t) => assert_eq!(t.split("\n\n").count(), texts.len()),
        o => panic!("unexpected output {o:?}"),
    }
}

#[tokio::test]
async fn translators_without_context_get_one_request() {
    let seen: Seen = Arc::default();
    let p = pipeline(false, None, seen.clone());
    let texts = article();
    let mut b = bundle(&texts);
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    let seen = seen.lock();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].1.is_empty());
    assert!(b.blocks.iter().all(|b| b.translation.is_some()));
}

#[tokio::test]
async fn merged_part_is_retried_per_block_with_context() {
    let seen: Seen = Arc::default();
    // The second part comes back as one paragraph.
    let p = pipeline(true, Some("Paragraph 2"), seen.clone());
    let texts = article();
    let mut b = bundle(&texts);
    p.translate_capture_to(&mut b, "ja").await.unwrap();
    let seen = seen.lock();
    let retries: Vec<&(String, TranslateContext)> = seen
        .iter()
        .filter(|(t, _)| !t.contains("\n\n") && t.starts_with("Paragraph"))
        .collect();
    assert!(!retries.is_empty());
    // Retries still see the first part, and each other in order.
    let first = &retries[0].1;
    assert_eq!(first.history[0].0, texts[..2].join("\n\n"));
    assert!(first.reference.is_some());
    for w in retries.windows(2) {
        assert_eq!(w[1].1.history.last().unwrap().0, w[0].0);
    }
    for (blk, src) in b.blocks.iter().zip(&texts) {
        assert_eq!(
            blk.translation.as_deref(),
            Some(format!("[{src}]").as_str())
        );
    }
}

/// Real-model check of the timings in docs/translation-latency.md:
/// `MOONLENS_TEST_GGUF=/path/plamo-3-translate-2b-Q4_K_M.gguf
///  cargo test --test translate_in_parts -- --ignored --nocapture`
#[tokio::test]
#[ignore]
async fn real_model_first_part_arrives_early() {
    let Ok(path) = std::env::var("MOONLENS_TEST_GGUF") else {
        return;
    };
    let screens: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../bench/latency/screens.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::load_from(tmp.path().join("config.toml")).unwrap();
    let mut s = Settings::default();
    s.providers.translate = TaskChain {
        chain: vec!["llamacpp".into()],
        timeout_ms: 60_000,
        ..TaskChain::default()
    };
    store.save(s).unwrap();
    let first_delta: Arc<Mutex<Option<std::time::Instant>>> = Arc::default();
    let fd = first_delta.clone();
    let tr = moonlens_lib::providers::translate_llama::LlamaTranslator::new(
        "llamacpp",
        &serde_json::json!({"model_path": path, "prompt_style": "pfml3"}),
        false,
    )
    .with_chunk_sink(Arc::new(move |d: String, _| {
        if !d.is_empty() {
            fd.lock().get_or_insert_with(std::time::Instant::now);
        }
    }));
    let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
    // MOONLENS_TEST_WHOLE=1 measures the old behaviour: one request.
    if std::env::var("MOONLENS_TEST_WHOLE").is_ok() {
        providers.insert("llamacpp".into(), Arc::new(WithoutContext(tr)));
    } else {
        providers.insert("llamacpp".into(), Arc::new(tr));
    }
    let p = Pipeline::with_providers(
        Arc::new(store),
        Arc::new(Storage::open_in_memory().unwrap()),
        providers,
    );
    // Load the model first so only translation is timed.
    let mut warm = bundle(&["Hello.".to_string()]);
    p.translate_capture_to(&mut warm, "ja").await.unwrap();
    for sc in screens.as_array().unwrap() {
        let texts: Vec<String> = sc["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        p.clear_translation_cache();
        *first_delta.lock() = None;
        let mut b = bundle(&texts);
        let t0 = std::time::Instant::now();
        let parts: Arc<Mutex<Vec<f32>>> = Arc::default();
        let pc = parts.clone();
        let progress = move |_: &TranslationProgress| pc.lock().push(t0.elapsed().as_secs_f32());
        p.translate_capture_with_progress(&mut b, sc["tgt"].as_str().unwrap(), Some(&progress))
            .await
            .unwrap();
        let first = first_delta.lock().map(|t| (t - t0).as_secs_f32());
        eprintln!(
            "{}: first token {:.2}s, parts started at {:?}, total {:.2}s",
            sc["id"],
            first.unwrap_or(-1.0),
            parts.lock(),
            t0.elapsed().as_secs_f32()
        );
        assert!(b.blocks.iter().all(|b| b.translation.is_some()));
    }
}

/// A translator that hides its context support, so the pipeline sends
/// one request as before.
struct WithoutContext<P>(P);

#[async_trait]
impl<P: Provider> Provider for WithoutContext<P> {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn capabilities(&self) -> &[Capability] {
        self.0.capabilities()
    }
    fn is_local(&self) -> bool {
        true
    }
    fn takes_translation_context(&self) -> Option<bool> {
        Some(false)
    }
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        self.0.run(input).await
    }
}
