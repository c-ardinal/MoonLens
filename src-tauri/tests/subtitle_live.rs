//! End-to-end test of live subtitles with real English speech (a public
//! domain recording of John F. Kennedy's inaugural address, 11 s): the
//! audio goes through the same loop the app runs
//! ([`moonlens_lib::subtitle::run_loop`]) with the real providers (the
//! built-in whisper.cpp recognizer and llama.cpp translator), and the
//! events the subtitle window receives are checked.
//!
//! 1. `english_speech_becomes_japanese_subtitles`: the recording is fed
//!    in at real-time pace, as the capture thread delivers it.
//! 2. `loopback_hears_what_the_pc_plays` (Windows): the recording is
//!    played on the default output device and captured back through the
//!    app's own loopback capture, then recognized. This is the path that
//!    was broken: the capture listened to a microphone instead of the
//!    speakers. Needs an audio output device (CI installs a virtual one).
//!
//! 3. `english_after_a_japanese_aside_stays_english`: an English speech
//!    with one Japanese sentence in the middle. The language is detected
//!    for every chunk, so the English that follows the Japanese must come
//!    out as English again (it used to be decoded as Japanese).
//!
//! 4. `each_window_hears_only_its_application` (Windows): two processes
//!    play at the same time, one English and one Japanese speech. Two
//!    captures, one per process, run their own subtitle loops, as two
//!    subtitle windows do: each must recognize its own application only.
//!
//! 5. `sentence_cut_by_chunks_is_translated_again`: the recording is one
//!    sentence spread over four chunks. Once it ends, the window must be
//!    told to replace the chunk lines with the sentence translated as a
//!    whole.
//!
//! `#[ignore]` because all need the downloaded models (about 2 GB):
//!
//! ```sh
//! cd src-tauri
//! cargo run --release --example fetch_models
//! cargo test --release --test subtitle_live -- --ignored --nocapture --test-threads 1
//! ```
#![cfg(feature = "local-asr")]

use std::{path::PathBuf, sync::Arc, time::Duration};

use moonlens_lib::{
    pipeline::Pipeline,
    plugins::registry::PluginRegistry,
    settings::SettingsStore,
    storage::Storage,
    subtitle::{run_loop, Phase, SubtitleEvent},
};
use parking_lot::Mutex;
use tokio::sync::{mpsc, Notify};

/// The English recording as 16 kHz mono PCM16.
fn speech() -> Vec<i16> {
    fixture("jfk.wav")
}

/// A fixture recording (16 kHz mono PCM16 WAV) as samples.
fn fixture(name: &str) -> Vec<i16> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/audio")
        .join(name);
    let bytes = std::fs::read(path).unwrap();
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("no data chunk")
        + 8;
    bytes[data..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes(*b))
        .collect()
}

type Env = (Arc<SettingsStore>, Arc<Pipeline>);

/// Settings and pipeline with the speech model loaded, as the app has
/// it when the loop starts (`SubtitleService::preflight` preloads it).
/// Loading takes most of a minute on a CI runner; left to the first
/// chunk, it would run into the recognition time limit.
async fn setup() -> Env {
    use moonlens_lib::providers::{asr_local::LocalWhisperProvider, Provider, TaskInput};
    let tmp = tempfile::tempdir().unwrap();
    let config = tmp.path().join("config.toml");
    // A CI runner has no GPU and few cores: a chunk takes 20-45 s there
    // (one second or less on a GPU), close to the default limit of 60 s.
    // With two windows, a translation shares those cores with the other
    // window's recognition and passes its default limit of 15 s.
    std::fs::write(
        &config,
        "[providers.asr]\ntimeout_ms = 180000\n\n[providers.translate]\ntimeout_ms = 120000\n",
    )
    .unwrap();
    let store = Arc::new(SettingsStore::load_from(config).unwrap());
    assert_eq!(store.snapshot().providers.asr.timeout_ms, 180_000);
    assert_eq!(store.snapshot().providers.translate.timeout_ms, 120_000);
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    let pipeline = Pipeline::new(
        store.clone(),
        Arc::new(Storage::open_in_memory().unwrap()),
        plugins,
    );
    let snap = store.snapshot();
    let cfg = snap
        .providers
        .configs
        .get("whisper")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let provider =
        LocalWhisperProvider::new("whisper", &cfg, snap.performance.use_gpu_if_available);
    assert!(
        provider.is_ready(),
        "speech model not downloaded: {}",
        provider.model_path().display()
    );
    let started = std::time::Instant::now();
    provider
        .run(TaskInput::Asr {
            audio_pcm16_mono_16k: vec![0; 32_000],
            language_hint: Some("en".into()),
        })
        .await
        .expect("speech model failed to load");
    println!(
        "speech model ready in {:.1} s",
        started.elapsed().as_secs_f32()
    );
    (store, Arc::new(pipeline))
}

/// Run the subtitle loop over `rx` until the channel closes; the events
/// it emitted.
async fn subtitles(env: Env, rx: mpsc::Receiver<Vec<i16>>) -> Vec<SubtitleEvent> {
    subtitles_notifying(env, rx, Arc::new(Notify::new())).await
}

/// [`subtitles`], signalling `idle` each time the loop has finished a
/// chunk and waits for audio again.
async fn subtitles_notifying(
    env: Env,
    rx: mpsc::Receiver<Vec<i16>>,
    idle: Arc<Notify>,
) -> Vec<SubtitleEvent> {
    let (settings, pipeline) = env;
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let working = std::sync::atomic::AtomicBool::new(false);
    let emit = move |ev: SubtitleEvent| {
        use std::sync::atomic::Ordering::Relaxed;
        println!("{ev:?}");
        match ev {
            SubtitleEvent::Status {
                phase: Phase::Recognizing,
            } => working.store(true, Relaxed),
            // Once per chunk: correcting a sentence also ends in waiting.
            SubtitleEvent::Status {
                phase: Phase::Waiting | Phase::Revising,
            } if working.swap(false, Relaxed) => idle.notify_one(),
            _ => {}
        }
        sink.lock().push(ev);
    };
    let gave_up = run_loop(&emit, settings, pipeline, rx, Arc::new(Notify::new())).await;
    assert!(!gave_up, "recognition gave up");
    let mut events = events.lock();
    std::mem::take(&mut *events)
}

fn has_japanese(text: &str) -> bool {
    text.chars()
        .any(|c| ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c))
}

/// The window must have been told the speech and its translation.
fn assert_subtitles(events: &[SubtitleEvent]) {
    let (mut original, mut translated) = (String::new(), String::new());
    for ev in events {
        match ev {
            SubtitleEvent::Chunk {
                original: o,
                translated: t,
                ..
            } => {
                original.push_str(&o.to_lowercase());
                original.push(' ');
                translated.push_str(t);
            }
            SubtitleEvent::Error { message } => panic!("error event: {message}"),
            _ => {}
        }
    }
    println!("original: {original}\ntranslated: {translated}");
    assert!(
        original.contains("country"),
        "speech not recognized: {original:?}"
    );
    assert!(
        has_japanese(&translated) && translated.contains('国'),
        "not translated to Japanese: {translated:?}"
    );
    for phase in [Phase::Recognizing, Phase::Translating, Phase::Waiting] {
        assert!(
            events
                .iter()
                .any(|e| matches!(e, SubtitleEvent::Status { phase: p } if *p == phase)),
            "no {phase:?} status"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the downloaded models"]
async fn english_speech_becomes_japanese_subtitles() {
    let env = setup().await;
    let (tx, rx) = mpsc::channel::<Vec<i16>>(4096);
    // 10 ms frames at real-time pace, like the capture callback.
    tokio::spawn(async move {
        let pcm = speech();
        let mut tick = tokio::time::interval(Duration::from_millis(10));
        for frame in pcm.chunks(160) {
            tick.tick().await;
            if tx.send(frame.to_vec()).await.is_err() {
                break;
            }
        }
    });
    assert_subtitles(&subtitles(env, rx).await);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the downloaded models"]
async fn english_after_a_japanese_aside_stays_english() {
    let env = setup().await;
    let chunk = (env.0.snapshot().subtitle.chunk_seconds * 16_000.0) as usize;
    // English, one Japanese sentence, English again, 0.3 s apart.
    let pause = vec![0i16; 4_800];
    let pcm = [
        speech(),
        pause.clone(),
        fixture("ja-moldova.wav"),
        pause,
        speech(),
    ]
    .concat();
    let (tx, rx) = mpsc::channel::<Vec<i16>>(4096);
    let idle = Arc::new(Notify::new());
    let fed = idle.clone();
    // One chunk at a time, the next when the previous one is done: the
    // chunks a PC that keeps up with the audio (a GPU) recognizes. A CI
    // runner takes many times the length of a chunk; at real-time pace
    // the loop would merge the backlog into one long window there.
    tokio::spawn(async move {
        for piece in pcm.chunks(chunk) {
            for frame in piece.chunks(160) {
                if tx.send(frame.to_vec()).await.is_err() {
                    return;
                }
            }
            if piece.len() == chunk {
                let _ = tokio::time::timeout(Duration::from_secs(600), fed.notified()).await;
            }
        }
    });
    let events = subtitles_notifying(env, rx, idle).await;

    let chunks: Vec<(&str, &str)> = events
        .iter()
        .filter_map(|ev| match ev {
            SubtitleEvent::Chunk {
                original,
                translated,
                ..
            } => Some((original.as_str(), translated.as_str())),
            SubtitleEvent::Error { message } => panic!("error event: {message}"),
            _ => None,
        })
        .collect();
    let first_ja = chunks
        .iter()
        .position(|(o, _)| has_japanese(o))
        .expect("the Japanese sentence was not recognized as Japanese");
    let last_ja = chunks.iter().rposition(|(o, _)| has_japanese(o)).unwrap();
    let join = |part: &[(&str, &str)]| {
        part.iter()
            .map(|(o, _)| o.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let japanese = join(&chunks[first_ja..=last_ja]);
    assert!(
        japanese.contains("宗教") || japanese.contains("キリスト"),
        "Japanese sentence not recognized: {japanese:?}"
    );
    let before = join(&chunks[..first_ja]);
    assert!(
        before.contains("country"),
        "English before the Japanese not recognized: {before:?}"
    );
    // The point of the test: the speech after the Japanese sentence is
    // heard as English again, and translated.
    let after = &chunks[last_ja + 1..];
    let english = join(after);
    assert!(
        english.contains("country"),
        "English after the Japanese not recognized as English: {english:?}"
    );
    let translated: String = after.iter().map(|(_, t)| *t).collect();
    assert!(
        has_japanese(&translated) && translated.contains('国'),
        "English after the Japanese not translated: {translated:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs the downloaded models"]
async fn sentence_cut_by_chunks_is_translated_again() {
    let env = setup().await;
    let snap = env.0.snapshot();
    assert!(snap.subtitle.realtime_correction && snap.subtitle.batch_correction);
    let chunk = (snap.subtitle.chunk_seconds * 16_000.0) as usize;
    let pcm = speech();
    let (tx, rx) = mpsc::channel::<Vec<i16>>(4096);
    let idle = Arc::new(Notify::new());
    let fed = idle.clone();
    // One chunk at a time, as in the test above.
    tokio::spawn(async move {
        for piece in pcm.chunks(chunk) {
            for frame in piece.chunks(160) {
                if tx.send(frame.to_vec()).await.is_err() {
                    return;
                }
            }
            if piece.len() == chunk {
                let _ = tokio::time::timeout(Duration::from_secs(600), fed.notified()).await;
            }
        }
    });
    let events = subtitles_notifying(env, rx, idle).await;

    // What the window shows: chunk lines, replaced as revisions arrive.
    let mut lines: Vec<(u64, String, String)> = Vec::new();
    let mut shown = std::collections::HashMap::new();
    let mut chunks = 0;
    let mut revisions = Vec::new();
    for ev in &events {
        match ev {
            SubtitleEvent::Chunk {
                id,
                original,
                translated,
            } => {
                chunks += 1;
                shown.insert(*id, translated.clone());
                lines.push((*id, original.clone(), translated.clone()));
            }
            SubtitleEvent::Revised(r) => {
                let at = lines
                    .iter()
                    .position(|(id, ..)| r.replaces.contains(id))
                    .expect("revision of lines never shown");
                for id in &r.replaces {
                    assert!(
                        lines.iter().any(|(shown, ..)| shown == id),
                        "line {id} replaced but not shown"
                    );
                }
                // Not the fallback of a failed translation: the old
                // fragments side by side.
                let fragments: Vec<&str> = r
                    .replaces
                    .iter()
                    .filter_map(|id| shown.get(id).map(String::as_str))
                    .collect();
                assert_ne!(
                    r.translated,
                    fragments.join(" "),
                    "the sentence was not translated again"
                );
                lines.retain(|(id, ..)| !r.replaces.contains(id));
                lines.insert(at, (r.id, r.original.clone(), r.translated.clone()));
                if let Some(rest) = &r.rest {
                    let line = (rest.id, rest.original.clone(), rest.translated.clone());
                    lines.insert(at + 1, line);
                }
                revisions.push(r.clone());
            }
            SubtitleEvent::Error { message } => panic!("error event: {message}"),
            _ => {}
        }
    }
    println!("chunks: {chunks}\nlines after correction: {lines:#?}");
    assert!(chunks >= 3, "the sentence was not cut: {chunks} chunk(s)");
    assert!(!revisions.is_empty(), "no sentence was translated again");
    let replaced: usize = revisions.iter().map(|r| r.replaces.len()).sum();
    assert!(replaced >= chunks, "chunk lines left: {lines:?}");
    assert!(
        lines.len() < chunks,
        "fragments were not joined into sentences: {lines:?}"
    );
    let original = lines
        .iter()
        .map(|(_, o, _)| o.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        original.matches("country").count(),
        2,
        "words lost or doubled when joining the chunks: {original:?}"
    );
    // The first sentence, recognized in three chunks, is one line. (Its
    // wording is the recognizer's: a chunk cut mid-phrase can be misheard,
    // and translating again does not recognize again.)
    assert!(
        lines.iter().any(|(_, o, _)| {
            let o = o.to_lowercase();
            o.contains("fellow americans") && o.contains("can do for you")
        }),
        "first sentence not joined: {lines:?}"
    );
    for (_, o, t) in &lines {
        assert!(has_japanese(t), "{o:?} not translated: {t:?}");
    }
    let translated: String = lines.iter().map(|(.., t)| t.as_str()).collect();
    assert!(translated.contains('国'), "{translated:?}");
    assert!(
        events.iter().any(|e| matches!(
            e,
            SubtitleEvent::Status {
                phase: Phase::Revising
            }
        )),
        "no revising status"
    );
    // The loop ends idle, after the last correction.
    assert!(matches!(
        events.last(),
        Some(SubtitleEvent::Status {
            phase: Phase::Waiting
        })
    ));
}

#[cfg(all(windows, feature = "audio"))]
mod loopback {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use moonlens_lib::audio::backend::LoopbackStream;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Play `pcm` (16 kHz mono) on the default output device; the stream
    /// to keep alive while it plays.
    fn play(pcm: Vec<i16>) -> cpal::Stream {
        let device = cpal::default_host()
            .default_output_device()
            .expect("no output device");
        let config = device.default_output_config().unwrap();
        println!(
            "playing on \"{}\" ({config:?})",
            device.name().unwrap_or_default()
        );
        assert_eq!(
            config.sample_format(),
            cpal::SampleFormat::F32,
            "the shared-mode mix format is float"
        );
        let channels = config.channels() as usize;
        let step = 16_000.0 / config.sample_rate().0 as f64;
        let played = AtomicUsize::new(0);
        let stream = device
            .build_output_stream(
                &config.into(),
                move |out: &mut [f32], _| {
                    for frame in out.chunks_mut(channels) {
                        let n = played.fetch_add(1, Ordering::Relaxed);
                        let at = n as f64 * step;
                        let i = at as usize;
                        let v = match (pcm.get(i), pcm.get(i + 1)) {
                            (Some(&a), Some(&b)) => {
                                let f = (at - i as f64) as f32;
                                (a as f32 + (b as f32 - a as f32) * f) / 32768.0
                            }
                            _ => 0.0,
                        };
                        frame.fill(v);
                    }
                },
                |e| eprintln!("playback error: {e}"),
                None,
            )
            .unwrap();
        stream.play().unwrap();
        stream
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs the downloaded models and an audio output device"]
    async fn loopback_hears_what_the_pc_plays() {
        if cpal::default_host().default_output_device().is_none() {
            assert!(
                std::env::var_os("MOONLENS_REQUIRE_AUDIO").is_none(),
                "no audio output device"
            );
            println!("skipped: no audio output device");
            return;
        }
        let env = setup().await;
        let pcm = speech();
        let seconds = pcm.len() as u64 / 16_000;
        let (tx, rx) = mpsc::channel::<Vec<i16>>(4096);
        let capture = LoopbackStream::start(tx).expect("loopback capture failed to start");
        println!("capturing \"{}\"", capture.device());
        let playback = play(pcm);
        // Recognition runs while the recording still plays, as in the app.
        let run = tokio::spawn(subtitles(env, rx));
        tokio::time::sleep(Duration::from_secs(seconds + 2)).await;
        drop(playback);
        // Closes the frame channel: the loop recognizes the rest and ends.
        tokio::task::spawn_blocking(move || drop(capture))
            .await
            .unwrap();
        assert_subtitles(&run.await.unwrap());
    }

    /// A process playing a recording; killed when the test ends early.
    struct Player(std::process::Child);

    impl Drop for Player {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Start a process that waits `delay` seconds and then plays the
    /// fixture `name` `times` times on the default output device.
    fn player(name: &str, delay: u32, times: u32) -> Player {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/audio")
            .join(name);
        let script = format!(
            "Start-Sleep -Seconds {delay}; \
             $p = New-Object System.Media.SoundPlayer '{}'; \
             1..{times} | ForEach-Object {{ $p.PlaySync() }}",
            path.display()
        );
        let child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .spawn()
            .expect("powershell did not start");
        Player(child)
    }

    fn originals(events: &[SubtitleEvent]) -> String {
        events
            .iter()
            .filter_map(|ev| match ev {
                SubtitleEvent::Chunk { original, .. } => Some(original.to_lowercase()),
                SubtitleEvent::Error { message } => panic!("error event: {message}"),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs the downloaded models and an audio output device"]
    async fn each_window_hears_only_its_application() {
        use moonlens_lib::audio::AudioSource;
        use moonlens_lib::audio_apps;

        if cpal::default_host().default_output_device().is_none() {
            assert!(
                std::env::var_os("MOONLENS_REQUIRE_AUDIO").is_none(),
                "no audio output device"
            );
            println!("skipped: no audio output device");
            return;
        }
        let env = setup().await;
        // Both start playing after the captures are up. The Japanese
        // recording is shorter: repeated, it covers the English one.
        let english = player("jfk.wav", 6, 1);
        let japanese = player("ja-moldova.wav", 6, 3);
        let (en_pid, ja_pid) = (english.0.id(), japanese.0.id());

        let (en_tx, en_rx) = mpsc::channel::<Vec<i16>>(4096);
        let (ja_tx, ja_rx) = mpsc::channel::<Vec<i16>>(4096);
        let en_capture = LoopbackStream::start_with(AudioSource::App { pid: en_pid }, en_tx)
            .expect("capture of the English player failed to start");
        let ja_capture = LoopbackStream::start_with(AudioSource::App { pid: ja_pid }, ja_tx)
            .expect("capture of the Japanese player failed to start");
        println!(
            "capturing \"{}\" (PID {en_pid}) and \"{}\" (PID {ja_pid})",
            en_capture.device(),
            ja_capture.device()
        );
        assert_eq!(en_capture.device(), "powershell");
        let (en_lost, ja_lost) = (en_capture.lost(), ja_capture.lost());

        // Two windows: two loops on the shared models.
        let en_run = tokio::spawn(subtitles(env.clone(), en_rx));
        let ja_run = tokio::spawn(subtitles(env, ja_rx));

        // While they play, the source menu lists both as playing.
        let mut listed = Vec::new();
        for _ in 0..40 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            // The players are processes of this test, which the app's
            // own list leaves out as "itself".
            let found = tokio::task::spawn_blocking(|| audio_apps::list_except(None))
                .await
                .unwrap();
            assert!(found.supported);
            listed = found.apps;
            let playing = |pid| listed.iter().any(|a| a.pid == pid && a.playing);
            if playing(en_pid) && playing(ja_pid) {
                break;
            }
        }
        println!("audio applications: {listed:?}");
        for pid in [en_pid, ja_pid] {
            let app = listed
                .iter()
                .find(|a| a.pid == pid)
                .unwrap_or_else(|| panic!("player {pid} not listed"));
            assert!(app.playing, "player {pid} not listed as playing");
            assert_eq!(app.name, "powershell");
        }

        // The players exit when done; each capture notices, closes its
        // frame channel, and the loop recognizes the rest and ends.
        let en_events = tokio::time::timeout(Duration::from_secs(900), en_run)
            .await
            .expect("the English capture did not end with its application")
            .unwrap();
        let ja_events = tokio::time::timeout(Duration::from_secs(900), ja_run)
            .await
            .expect("the Japanese capture did not end with its application")
            .unwrap();
        use std::sync::atomic::Ordering::Relaxed;
        assert!(en_lost.load(Relaxed) && ja_lost.load(Relaxed));
        tokio::task::spawn_blocking(move || drop((en_capture, ja_capture)))
            .await
            .unwrap();

        assert_subtitles(&en_events);
        let (en, ja) = (originals(&en_events), originals(&ja_events));
        println!("English window: {en}\nJapanese window: {ja}");
        assert!(
            !has_japanese(&en),
            "the English window heard the Japanese player: {en:?}"
        );
        assert!(
            ja.contains("宗教") || ja.contains("キリスト"),
            "the Japanese window did not recognize its player: {ja:?}"
        );
        assert!(
            !ja.contains("country"),
            "the Japanese window heard the English player: {ja:?}"
        );
    }
}
