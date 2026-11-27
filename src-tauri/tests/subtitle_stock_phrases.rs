//! Stock misrecognitions in live subtitles, through the real subtitle
//! loop with scripted speech recognition and a recording translator:
//! what the window is told, and what reaches translation.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use async_trait::async_trait;
use moonlens_lib::{
    pipeline::Pipeline,
    providers::{
        Capability, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
        TranscriptSegment,
    },
    settings::{schema::Settings, SettingsStore},
    storage::Storage,
    subtitle::{run_loop, SubtitleEvent},
};
use tokio::sync::mpsc;

/// Speech recognition that answers its n-th call with the n-th script
/// entry (one segment per string).
struct ScriptedAsr {
    script: Vec<Vec<&'static str>>,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Provider for ScriptedAsr {
    fn id(&self) -> &str {
        "asr"
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Asr]
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn run(&self, _input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let segments = self.script[n]
            .iter()
            .map(|text| TranscriptSegment {
                start_s: 0.0,
                end_s: 1.0,
                text: text.to_string(),
            })
            .collect();
        Ok(TaskOutput::Transcript { segments })
    }
}

/// A translator that records what it was asked to translate.
struct RecordingTranslator {
    asked: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl Provider for RecordingTranslator {
    fn id(&self) -> &str {
        "tr"
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Translate]
    }
    fn is_local(&self) -> bool {
        true
    }
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Translate { text, .. } = input else {
            return Err(ProviderError::Other("not a translation".into()));
        };
        self.asked.lock().unwrap().push(text.clone());
        Ok(TaskOutput::Text(format!("訳：{text}")))
    }
}

struct Outcome {
    events: Vec<SubtitleEvent>,
    /// Texts sent to translation, in order.
    translated: Vec<String>,
    settings: Arc<SettingsStore>,
}

impl Outcome {
    fn shown(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter_map(|e| match e {
                SubtitleEvent::Chunk { original, .. } => Some(original.as_str()),
                _ => None,
            })
            .collect()
    }

    fn candidates(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter_map(|e| match e {
                SubtitleEvent::Candidate { phrase } => Some(phrase.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// Run the subtitle loop over one audible chunk per script entry.
/// `between` runs after each chunk is done (to change settings mid-way).
async fn subtitles(
    configure: impl FnOnce(&mut Settings),
    script: Vec<Vec<&'static str>>,
    between: impl Fn(usize, &SettingsStore),
) -> Outcome {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::load_from(tmp.path().join("config.toml")).unwrap();
    let mut s = Settings::default();
    s.providers.asr.chain = vec!["asr".into()];
    s.providers.translate.chain = vec!["tr".into()];
    s.subtitle.source_language = "en".into();
    s.subtitle.overlap_seconds = 0.0;
    configure(&mut s);
    let chunk_samples = (s.subtitle.chunk_seconds * 16_000.0) as usize;
    store.save(s).unwrap();
    let settings = Arc::new(store);

    let calls = Arc::new(AtomicUsize::new(0));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let chunks = script.len();
    let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
    providers.insert(
        "asr".into(),
        Arc::new(ScriptedAsr {
            script,
            calls: calls.clone(),
        }),
    );
    providers.insert(
        "tr".into(),
        Arc::new(RecordingTranslator {
            asked: asked.clone(),
        }),
    );
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let pipeline = Arc::new(Pipeline::with_providers(
        settings.clone(),
        storage,
        providers,
    ));

    let events = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = mpsc::channel::<Vec<i16>>(8);
    let task = {
        let (events, settings) = (events.clone(), settings.clone());
        tokio::spawn(async move {
            let emit = move |ev: SubtitleEvent| events.lock().unwrap().push(ev);
            let stop = Arc::new(tokio::sync::Notify::new());
            run_loop(&emit, settings, pipeline, rx, stop).await
        })
    };
    // Audible, so the silence gate lets it through.
    let tone: Vec<i16> = (0..chunk_samples)
        .map(|i| ((i as f32 * 0.05).sin() * 3000.0) as i16)
        .collect();
    for n in 0..chunks {
        tx.send(tone.clone()).await.unwrap();
        // The loop is back to waiting once it reports so after the call.
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let done = calls.load(Ordering::SeqCst) > n
                    && matches!(
                        events.lock().unwrap().last(),
                        Some(SubtitleEvent::Status {
                            phase: moonlens_lib::subtitle::Phase::Waiting
                        })
                    );
                if done {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("chunk processed");
        between(n, &settings);
    }
    drop(tx);
    let gave_up = task.await.unwrap();
    assert!(!gave_up);
    let events = events.lock().unwrap().clone();
    let translated = asked.lock().unwrap().clone();
    std::mem::forget(tmp);
    Outcome {
        events,
        translated,
        settings,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn stock_phrases_are_neither_shown_nor_translated() {
    let out = subtitles(
        |_| {},
        vec![
            vec!["The meeting starts at nine."],
            vec!["Thanks for watching!"],
            vec!["[Music]"],
            vec!["Bring the report.", "Subtitles by the Amara.org community"],
            vec!["ご視聴ありがとうございました。"],
        ],
        |_, _| {},
    )
    .await;
    assert_eq!(
        out.shown(),
        ["The meeting starts at nine.", "Bring the report."]
    );
    // Translation ran for the two real lines only.
    assert_eq!(
        out.translated,
        ["The meeting starts at nine.", "Bring the report."]
    );
    assert!(out.candidates().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn switched_off_everything_is_shown() {
    let out = subtitles(
        |s| s.subtitle.block_stock_phrases = false,
        vec![vec!["Thanks for watching!"], vec!["[Music]"]],
        |_, _| {},
    )
    .await;
    assert_eq!(out.shown(), ["Thanks for watching!", "[Music]"]);
    assert_eq!(out.translated.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_repeated_sentence_is_suggested_and_blocked_only_after_approval() {
    let out = subtitles(
        |_| {},
        vec![
            vec!["Thank you."],
            vec!["Thank you."],
            vec!["Thank you."],
            // Suggested by now, but still shown: nobody approved it.
            vec!["Thank you."],
            // Approved after the 4th chunk (see below).
            vec!["Thank you."],
            vec!["Thank you for coming."],
        ],
        |n, settings| {
            if n == 3 {
                // What `block_subtitle_phrase` does on approval.
                let mut next = (*settings.snapshot()).clone();
                assert!(moonlens_lib::subtitle_filter::add_phrase(
                    &mut next.subtitle.blocked_phrases,
                    "Thank you."
                ));
                settings.save(next).unwrap();
            }
        },
    )
    .await;
    assert_eq!(out.candidates(), ["Thank you."], "asked once");
    assert_eq!(
        out.shown(),
        [
            "Thank you.",
            "Thank you.",
            "Thank you.",
            "Thank you.",
            "Thank you for coming."
        ]
    );
    assert_eq!(
        out.translated.len(),
        5,
        "the blocked chunk is not translated"
    );
    assert_eq!(
        out.settings.snapshot().subtitle.blocked_phrases,
        ["Thank you."]
    );
}
