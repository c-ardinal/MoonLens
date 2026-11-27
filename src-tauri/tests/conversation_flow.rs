//! Face-to-face conversation through the real subtitle loop, with
//! scripted speech recognition and a recording translator: which
//! language each utterance is translated into, what the window is told
//! about the speaker, and what speech recognition is asked for.

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
    conversation::Speaker,
    pipeline::Pipeline,
    providers::{
        Capability, Provider, ProviderError, ProviderHandle, TaskInput, TaskOutput,
        TranscriptSegment,
    },
    settings::{schema::Settings, SettingsStore},
    storage::Storage,
    subtitle::{run_loop, Mode, Phase, SubtitleEvent},
};
use tokio::sync::mpsc;

/// One recognized chunk: the language the engine reports (if it does)
/// and the text.
type Heard = (Option<&'static str>, &'static str);

/// What speech recognition was asked for.
#[derive(Debug, Clone, PartialEq)]
struct Asked {
    hint: Option<String>,
    candidates: Vec<String>,
}

struct ScriptedAsr {
    script: Vec<Heard>,
    calls: Arc<AtomicUsize>,
    asked: Arc<Mutex<Vec<Asked>>>,
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
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Asr {
            language_hint,
            language_candidates,
            ..
        } = input
        else {
            return Err(ProviderError::Other("not speech".into()));
        };
        self.asked.lock().unwrap().push(Asked {
            hint: language_hint,
            candidates: language_candidates,
        });
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let (language, text) = self.script[n];
        Ok(TaskOutput::Transcript {
            segments: vec![TranscriptSegment {
                start_s: 0.0,
                end_s: 1.0,
                text: text.to_string(),
            }],
            language: language.map(str::to_string),
        })
    }
}

/// A translation request: text, source and target language.
type Request = (String, String, String);

struct RecordingTranslator {
    requests: Arc<Mutex<Vec<Request>>>,
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
        let TaskInput::Translate {
            text,
            source_language,
            target_language,
            ..
        } = input
        else {
            return Err(ProviderError::Other("not a translation".into()));
        };
        let out = format!("[{target_language}] {text}");
        self.requests
            .lock()
            .unwrap()
            .push((text, source_language, target_language));
        Ok(TaskOutput::Text(out))
    }
}

struct Outcome {
    events: Vec<SubtitleEvent>,
    requests: Vec<Request>,
    asked: Vec<Asked>,
}

impl Outcome {
    /// Shown lines: speaker, original, translation.
    fn lines(&self) -> Vec<(Option<Speaker>, &str, &str)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                SubtitleEvent::Chunk {
                    speaker,
                    original,
                    translated,
                    ..
                } => Some((*speaker, original.as_str(), translated.as_str())),
                _ => None,
            })
            .collect()
    }

    /// Sentences translated again as a whole: lines replaced, text,
    /// translation.
    fn corrections(&self) -> Vec<(&[u64], &str, &str)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                SubtitleEvent::Revised(r) => Some((
                    r.replaces.as_slice(),
                    r.original.as_str(),
                    r.translated.as_str(),
                )),
                _ => None,
            })
            .collect()
    }
}

/// Run the loop in `mode` over one audible chunk per script entry.
/// `between` runs after each chunk is done.
async fn talk(
    mode: Mode,
    configure: impl FnOnce(&mut Settings),
    script: Vec<Heard>,
    between: impl Fn(usize, &SettingsStore),
) -> Outcome {
    let tmp = tempfile::tempdir().unwrap();
    let store = SettingsStore::load_from(tmp.path().join("config.toml")).unwrap();
    let mut s = Settings::default();
    s.providers.asr.chain = vec!["asr".into()];
    s.providers.translate.chain = vec!["tr".into()];
    s.subtitle.target_language = "ja".into();
    s.subtitle.conversation_language = "en".into();
    s.subtitle.overlap_seconds = 0.0;
    s.subtitle.block_stock_phrases = false;
    s.subtitle.realtime_correction = false;
    s.subtitle.batch_correction = false;
    configure(&mut s);
    let chunk_samples = (s.subtitle.chunk_seconds * 16_000.0) as usize;
    store.save(s).unwrap();
    let settings = Arc::new(store);

    let calls = Arc::new(AtomicUsize::new(0));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let chunks = script.len();
    let mut providers: HashMap<String, ProviderHandle> = HashMap::new();
    providers.insert(
        "asr".into(),
        Arc::new(ScriptedAsr {
            script,
            calls: calls.clone(),
            asked: asked.clone(),
        }),
    );
    providers.insert(
        "tr".into(),
        Arc::new(RecordingTranslator {
            requests: requests.clone(),
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
            run_loop(&emit, settings, pipeline, rx, stop, mode).await
        })
    };
    let tone: Vec<i16> = (0..chunk_samples)
        .map(|i| ((i as f32 * 0.05).sin() * 3000.0) as i16)
        .collect();
    for n in 0..chunks {
        tx.send(tone.clone()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let done = calls.load(Ordering::SeqCst) > n
                    && matches!(
                        events.lock().unwrap().last(),
                        Some(SubtitleEvent::Status {
                            phase: Phase::Waiting
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
    assert!(!task.await.unwrap());
    let events = events.lock().unwrap().clone();
    let requests = requests.lock().unwrap().clone();
    let asked = asked.lock().unwrap().clone();
    Outcome {
        events,
        requests,
        asked,
    }
}

fn request(text: &str, source: &str, target: &str) -> Request {
    (text.into(), source.into(), target.into())
}

#[tokio::test(flavor = "multi_thread")]
async fn each_utterance_is_translated_into_the_language_of_the_other_person() {
    let out = talk(
        Mode::Conversation,
        |_| {},
        vec![
            (Some("ja"), "駅はどこですか。"),
            (Some("en"), "It is over there."),
            (Some("ja"), "ありがとうございます。"),
        ],
        |_, _| {},
    )
    .await;
    assert_eq!(
        out.requests,
        [
            request("駅はどこですか。", "ja", "en"),
            request("It is over there.", "en", "ja"),
            request("ありがとうございます。", "ja", "en"),
        ]
    );
    assert_eq!(
        out.lines(),
        [
            (
                Some(Speaker::Own),
                "駅はどこですか。",
                "[en] 駅はどこですか。"
            ),
            (
                Some(Speaker::Partner),
                "It is over there.",
                "[ja] It is over there."
            ),
            (
                Some(Speaker::Own),
                "ありがとうございます。",
                "[en] ありがとうございます。"
            ),
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn recognition_chooses_between_the_two_languages_only() {
    let out = talk(
        Mode::Conversation,
        |s| {
            // The spoken language of the subtitles does not apply.
            s.subtitle.source_language = "fr".into();
            s.subtitle.conversation_language = "zh-Hans".into();
        },
        vec![(Some("zh"), "车站在哪里？")],
        |_, _| {},
    )
    .await;
    assert_eq!(
        out.asked,
        [Asked {
            hint: None,
            candidates: vec!["ja".into(), "zh".into()],
        }]
    );
    // The translator is told the language as it was chosen.
    assert_eq!(out.requests, [request("车站在哪里？", "zh-Hans", "ja")]);
}

#[tokio::test(flavor = "multi_thread")]
async fn subtitles_keep_their_one_direction_and_name_no_speaker() {
    let out = talk(
        Mode::default(),
        |s| s.subtitle.source_language = "en".into(),
        vec![(Some("en"), "Good morning."), (Some("ja"), "おはよう。")],
        |_, _| {},
    )
    .await;
    assert!(out.asked.iter().all(|a| *a
        == Asked {
            hint: Some("en".into()),
            candidates: Vec::new(),
        }));
    assert!(out.requests.iter().all(|(_, s, t)| s == "en" && t == "ja"));
    assert!(out.lines().iter().all(|(speaker, ..)| speaker.is_none()));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_engine_that_reports_no_language_is_judged_by_the_writing() {
    let out = talk(
        Mode::Conversation,
        |_| {},
        vec![
            (None, "駅はどこですか。"),
            (None, "It is over there, on the left."),
        ],
        |_, _| {},
    )
    .await;
    let speakers: Vec<_> = out.lines().iter().map(|(s, ..)| *s).collect();
    assert_eq!(speakers, [Some(Speaker::Own), Some(Speaker::Partner)]);
    assert_eq!(out.requests[0].2, "en");
    assert_eq!(out.requests[1].2, "ja");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_of_speaker_ends_the_sentence_in_its_own_languages() {
    let out = talk(
        Mode::Conversation,
        |s| {
            s.subtitle.realtime_correction = true;
            s.subtitle.batch_correction = true;
        },
        vec![
            // One sentence over two chunks, cut off by the answer.
            (Some("ja"), "駅は"),
            (Some("ja"), "どこですか"),
            (Some("en"), "It is over there."),
        ],
        |_, _| {},
    )
    .await;
    // The user's sentence is corrected as a whole, into English, and
    // without the answer; the answer goes into Japanese.
    let own = out
        .corrections()
        .into_iter()
        .find(|(replaces, ..)| replaces.contains(&1))
        .expect("the first sentence is corrected");
    assert_eq!(own.0, [1, 2]);
    assert!(own.1.contains("駅は") && own.1.contains("どこですか"));
    assert!(!own.1.contains("over there"), "{}", own.1);
    assert!(own.2.starts_with("[en] "), "{}", own.2);
    for (text, source, target) in &out.requests {
        let japanese = text.contains('駅') || text.contains("どこ");
        let expected = if japanese { ("ja", "en") } else { ("en", "ja") };
        assert_eq!((source.as_str(), target.as_str()), expected, "{text}");
        assert!(
            !(japanese && text.contains("over")),
            "both speakers in one request: {text}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_partner_language_applies_from_the_next_utterance() {
    let out = talk(
        Mode::Conversation,
        |_| {},
        vec![(Some("ja"), "こんにちは。"), (Some("ja"), "こんにちは。")],
        |n, settings| {
            if n == 0 {
                let mut next = (*settings.snapshot()).clone();
                next.subtitle.conversation_language = "ko".into();
                settings.save(next).unwrap();
            }
        },
    )
    .await;
    assert_eq!(out.requests[0].2, "en");
    assert_eq!(out.requests[1].2, "ko");
    assert_eq!(out.asked[1].candidates, ["ja", "ko"]);
}
