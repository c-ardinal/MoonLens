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
//! `#[ignore]` because both need the downloaded models (about 2 GB):
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

/// The recording as 16 kHz mono PCM16.
fn speech() -> Vec<i16> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/audio/jfk.wav");
    let bytes = std::fs::read(path).unwrap();
    let data = bytes
        .windows(4)
        .position(|w| w == b"data")
        .expect("no data chunk")
        + 8;
    bytes[data..]
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect()
}

fn setup() -> (Arc<SettingsStore>, Arc<Pipeline>) {
    let tmp = tempfile::tempdir().unwrap();
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    let pipeline = Pipeline::new(
        store.clone(),
        Arc::new(Storage::open_in_memory().unwrap()),
        plugins,
    );
    (store, Arc::new(pipeline))
}

/// Run the subtitle loop over `rx` until the channel closes; the events
/// it emitted.
async fn subtitles(rx: mpsc::Receiver<Vec<i16>>) -> Vec<SubtitleEvent> {
    let (settings, pipeline) = setup();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let emit = move |ev: SubtitleEvent| {
        println!("{ev:?}");
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
    assert_subtitles(&subtitles(rx).await);
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
        let pcm = speech();
        let seconds = pcm.len() as u64 / 16_000;
        let (tx, rx) = mpsc::channel::<Vec<i16>>(4096);
        let capture = LoopbackStream::start(tx).expect("loopback capture failed to start");
        println!("capturing \"{}\"", capture.device());
        let playback = play(pcm);
        // Recognition runs while the recording still plays, as in the app.
        let run = tokio::spawn(subtitles(rx));
        tokio::time::sleep(Duration::from_secs(seconds + 2)).await;
        drop(playback);
        // Closes the frame channel: the loop recognizes the rest and ends.
        tokio::task::spawn_blocking(move || drop(capture))
            .await
            .unwrap();
        assert_subtitles(&run.await.unwrap());
    }
}
