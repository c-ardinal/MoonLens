//! Live subtitle service.
//!
//! Drives loopback audio capture, slices it into chunks, sends each chunk
//! to whisper.cpp via the ASR provider chain, optionally translates the
//! transcript, and emits the result to the `subtitle` window.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

use crate::{
    audio::{
        backend::{AudioFrame, LoopbackStream},
        level,
    },
    clipboard, overlay,
    pipeline::Pipeline,
    providers::{Capability, TaskInput, TaskOutput},
    settings::SettingsStore,
};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubtitleEvent {
    Started {
        /// The device whose audio is captured.
        device: String,
    },
    Stopped,
    /// What the loop is doing, for the window's status display.
    Status {
        phase: Phase,
    },
    Chunk {
        original: String,
        translated: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    /// No sound to recognize: nothing plays, or it is below the gate.
    Waiting,
    Recognizing,
    Translating,
}

/// Input level for the window's waveform (`moonlens://subtitle-level`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LevelEvent {
    /// RMS level per 25 ms of captured audio in dBFS, oldest first.
    /// Empty when the capture device delivers nothing.
    pub bins: Vec<i8>,
    /// Level below which the loop treats audio as silence, in dBFS.
    pub gate_db: f32,
}

pub struct SubtitleService {
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
    inner: Mutex<Option<Inner>>,
}

struct Inner {
    _stream: LoopbackStream,
    handle: JoinHandle<()>,
    stop: Arc<tokio::sync::Notify>,
}

impl SubtitleService {
    pub fn new(settings: Arc<SettingsStore>, pipeline: Arc<Pipeline>) -> Self {
        Self {
            settings,
            pipeline,
            inner: Mutex::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().is_some()
    }

    /// Make sure speech recognition can run before opening the subtitle
    /// window: the built-in engine needs its model downloaded, an
    /// external whisper.cpp server must answer. Starting without either
    /// would only produce an error every few seconds. Returns a coded,
    /// user-facing message (see `user_error`). On success the built-in
    /// model starts loading in the background.
    pub async fn preflight(&self) -> Result<(), String> {
        use crate::providers::asr_local::{self, LocalWhisperProvider, LOCAL_ASR_ID};

        let s = self.settings.snapshot();
        let chain = &s.providers.asr.chain;
        if chain.is_empty() {
            return Err(crate::user_error::coded(
                "asr_unavailable",
                "音声認識プロバイダー未設定",
            ));
        }
        let empty = serde_json::Value::Object(Default::default());
        let use_gpu = s.performance.use_gpu_if_available;
        let mut first_error = None;
        let mut unreachable = Vec::new();
        for id in chain {
            let cfg = s.providers.configs.get(id).unwrap_or(&empty);
            if id == LOCAL_ASR_ID {
                let p = LocalWhisperProvider::new(id, cfg, use_gpu);
                if !crate::providers::translate_llama::cpu_supported() {
                    first_error.get_or_insert_with(|| {
                        crate::user_error::coded("cpu_unsupported", "非対応 CPU（AVX2・FMA 必須）")
                    });
                    continue;
                }
                if p.is_ready() {
                    asr_local::preload(
                        p.model_path().to_path_buf(),
                        asr_local::DecodeParams::from_config(cfg, use_gpu),
                    );
                    return Ok(());
                }
                first_error.get_or_insert_with(|| {
                    crate::user_error::coded(
                        "model_missing",
                        format!("音声認識モデル未導入：{}", p.model().label),
                    )
                });
            } else if id.starts_with("whisper_cpp.") {
                let endpoint = cfg
                    .get("endpoint")
                    .and_then(|v| v.as_str())
                    .unwrap_or("http://127.0.0.1:9002")
                    .trim_end_matches('/')
                    .to_string();
                let http = reqwest::Client::builder()
                    .timeout(Duration::from_millis(1500))
                    .build()
                    .map_err(|e| e.to_string())?;
                // Any HTTP answer means a server is listening.
                if http.get(&endpoint).send().await.is_ok() {
                    return Ok(());
                }
                unreachable.push(endpoint);
            } else {
                // Plugins and other providers report their own errors.
                return Ok(());
            }
        }
        Err(first_error.unwrap_or_else(|| {
            crate::user_error::coded(
                "asr_unavailable",
                format!("音声認識サーバー未接続：{}", unreachable.join("、")),
            )
        }))
    }

    pub fn start(&self, app: AppHandle) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        // It may have been freed while unused (see `model_idle`).
        self.pipeline.preload_local_llm();
        // Frames keep arriving while a chunk is being recognized; the
        // capture callback drops what does not fit, so leave room for a
        // long decode (callbacks deliver ~10 ms each).
        let (tx, rx) = mpsc::channel::<AudioFrame>(FRAME_QUEUE);
        // The loop does not read frames while a chunk is recognized, so
        // the level for the window's waveform is measured on the way in.
        let (tap_tx, tap_rx) = mpsc::channel::<AudioFrame>(TAP_QUEUE);
        let stream = LoopbackStream::start(tap_tx)?;
        let app_level = app.clone();
        tauri::async_runtime::spawn(meter_relay(tap_rx, tx, move |ev| {
            let _ = app_level.emit_to("subtitle", "moonlens://subtitle-level", ev);
        }));
        let device = stream.device().to_string();
        tracing::info!("subtitle: started (device: {device})");
        if let Some(win) = app.get_webview_window("subtitle") {
            place_once(&win);
            win.show().ok();
        }
        let stop = Arc::new(tokio::sync::Notify::new());
        let stop_in = stop.clone();
        let pipeline = self.pipeline.clone();
        let settings = self.settings.clone();
        let app_emit = app.clone();
        let handle = tauri::async_runtime::spawn(async move {
            let emit = {
                let app = app_emit.clone();
                let settings = settings.clone();
                move |ev: SubtitleEvent| {
                    if let SubtitleEvent::Chunk {
                        original,
                        translated,
                    } = &ev
                    {
                        let text = if translated.is_empty() {
                            original
                        } else {
                            translated
                        };
                        let cfg = settings.snapshot().clipboard.clone();
                        clipboard::auto_copy(&app, clipboard::Kind::Transcript, text, &cfg);
                    }
                    let _ = app.emit("moonlens://subtitle", ev);
                }
            };
            emit(SubtitleEvent::Started { device });
            let gave_up = run_loop(&emit, settings, pipeline, rx, stop_in).await;
            if gave_up {
                // Release the audio stream and the model, but leave the
                // window up: it shows why the subtitles stopped.
                app_emit.state::<crate::app::AppState>().subtitle.release();
            }
            tracing::info!("subtitle: stopped (gave up: {gave_up})");
            emit(SubtitleEvent::Stopped);
        });
        *self.inner.lock() = Some(Inner {
            _stream: stream,
            handle,
            stop,
        });
        Ok(())
    }

    /// Stop capturing and recognizing. The window stays as it is.
    fn release(&self) {
        if let Some(inner) = self.inner.lock().take() {
            inner.stop.notify_waiters();
            inner.handle.abort();
            // Give back the ~1 GB the speech model holds. Off this thread:
            // it waits for an aborted decode to wind down.
            std::thread::spawn(crate::providers::asr_local::release);
        }
    }

    /// Stop and hide the window (also when recognition already gave up
    /// and only the window with its error is left).
    pub fn stop(&self, app: &AppHandle) -> Result<()> {
        let was_running = self.is_running();
        self.release();
        if let Some(win) = app.get_webview_window("subtitle") {
            win.hide().ok();
        }
        // Reset overlay context if any.
        overlay::hide(app).ok();
        if was_running {
            // The aborted loop cannot report it any more.
            let _ = app.emit("moonlens://subtitle", SubtitleEvent::Stopped);
        }
        Ok(())
    }

    /// Toggle from a global shortcut or the tray menu, where no window
    /// can show a returned error: run the same check as the start button
    /// and, when it fails, leave a notice on the home screen and bring up
    /// the page that fixes it (the model downloads, usually).
    pub fn toggle_checked(self: &Arc<Self>, app: &AppHandle) {
        const NOTICE: &str = "subtitle_unavailable";
        let this = self.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if this.is_running() {
                let _ = this.stop(&app);
                return;
            }
            let failure = match this.preflight().await {
                Ok(()) => match this.start(app.clone()) {
                    Ok(()) => {
                        crate::notices::clear(NOTICE);
                        return;
                    }
                    Err(e) => {
                        crate::user_error::coded("error", format!("音声取得の開始失敗：{e:#}"))
                    }
                },
                Err(coded) => coded,
            };
            let (code, message) = failure.split_once('|').unwrap_or(("error", &failure));
            let route = (code == "model_missing").then_some("/settings/#section-model-asr");
            crate::notices::push(NOTICE, format!("ライブ字幕：{message}"), route);
            crate::app::route_main(&app, route.unwrap_or("/"));
        });
    }
}

/// First show: bottom centre of the primary screen, where subtitles
/// are expected. Later shows keep wherever the user moved the window.
fn place_once(win: &tauri::WebviewWindow) {
    static PLACED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if PLACED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let (Ok(Some(monitor)), Ok(size)) = (win.primary_monitor(), win.outer_size()) else {
        return;
    };
    let (origin, screen) = (monitor.position(), monitor.size());
    let margin = (96.0 * monitor.scale_factor()) as i32;
    let x = origin.x + (screen.width as i32 - size.width as i32) / 2;
    let y = origin.y + screen.height as i32 - size.height as i32 - margin;
    let _ = win.set_position(tauri::PhysicalPosition::new(x, y.max(origin.y)));
}

/// The subtitle loop: audio frames in, [`SubtitleEvent`]s out. Runs
/// until `stop` fires or the frame channel closes (what is still
/// buffered is recognized first). Returns `true` when it gave up after
/// repeated recognition failures.
pub async fn run_loop(
    emit: &(impl Fn(SubtitleEvent) + Sync),
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
    mut rx: mpsc::Receiver<AudioFrame>,
    stop: Arc<tokio::sync::Notify>,
) -> bool {
    let mut failures = 0u32;
    let mut buffer: Vec<i16> = Vec::with_capacity(16_000 * 4);
    let mut overlap: Vec<i16> = Vec::new();
    let mut phase = Phase::Waiting;
    let mut set_phase = |next: Phase| {
        if phase != next {
            phase = next;
            emit(SubtitleEvent::Status { phase });
        }
    };
    let mut heard = false;
    let mut closed = false;
    loop {
        let snap = settings.snapshot();
        let chunk_samples = (snap.subtitle.chunk_seconds * 16_000.0) as usize;
        let overlap_samples = (snap.subtitle.overlap_seconds * 16_000.0) as usize;
        let target_lang = snap.subtitle.target_language.clone();
        let source_lang = snap.subtitle.source_language.clone();
        drop(snap);

        // Wait for the next frame, a stop notification, or a pause in
        // the audio: a loopback delivers nothing while nothing plays, so
        // without the pause the end of a sentence would sit in the
        // buffer until sound returns.
        let mut flush = closed;
        if !closed {
            tokio::select! {
                _ = stop.notified() => break,
                maybe = rx.recv() => match maybe {
                    Some(frame) => buffer.extend_from_slice(&frame),
                    None => {
                        closed = true;
                        flush = true;
                    }
                },
                _ = tokio::time::sleep(FLUSH_AFTER), if !buffer.is_empty() => flush = true,
            }
        }
        // Collect whatever arrived during the previous recognition.
        while let Ok(frame) = rx.try_recv() {
            buffer.extend_from_slice(&frame);
        }
        if !heard && !buffer.is_empty() {
            heard = true;
            tracing::info!("subtitle: first audio frames received");
        }
        if buffer.len() < chunk_samples && !(flush && buffer.len() >= MIN_FLUSH_SAMPLES) {
            if flush {
                // Too short to recognize on its own.
                buffer.clear();
                overlap.clear();
                set_phase(Phase::Waiting);
            }
            if closed {
                break;
            }
            continue;
        }
        // Whisper decodes a fixed 30 s window, so a longer chunk costs no
        // more than a short one. When recognition falls behind, take the
        // whole backlog at once instead of queueing chunk after chunk.
        let take = buffer
            .len()
            .min(MAX_WINDOW_SAMPLES.saturating_sub(overlap.len()));
        // Build a chunk that includes a small lookback for context.
        let mut to_send = overlap.clone();
        to_send.extend_from_slice(&buffer[..take]);
        // Preserve the tail for the next iteration's overlap.
        if overlap_samples > 0 && take >= overlap_samples && !flush {
            overlap = buffer[take - overlap_samples..take].to_vec();
        } else {
            overlap.clear();
        }
        buffer.drain(..take);
        // Silence (or near-silence) makes Whisper invent stock phrases
        // such as "Thanks for watching"; skip it.
        if is_silent(&to_send) {
            set_phase(Phase::Waiting);
            continue;
        }

        set_phase(Phase::Recognizing);
        let seconds = to_send.len() as f32 / 16_000.0;
        let started = std::time::Instant::now();
        let pcm_bytes = pcm16_to_bytes(&to_send);
        match pipeline
            .run(
                Capability::Asr,
                TaskInput::Asr {
                    audio_pcm16_mono_16k: pcm_bytes,
                    language_hint: if source_lang == "auto" {
                        None
                    } else {
                        Some(source_lang.clone())
                    },
                },
            )
            .await
        {
            Ok(res) => {
                failures = 0;
                let original = match &res.output {
                    TaskOutput::Transcript { segments } => segments
                        .iter()
                        .map(|s| s.text.trim())
                        .collect::<Vec<_>>()
                        .join(" "),
                    TaskOutput::Text(t) => t.clone(),
                    _ => String::new(),
                };
                tracing::debug!(
                    "subtitle: {seconds:.1} s of audio recognized in {} ms ({} chars)",
                    started.elapsed().as_millis(),
                    original.chars().count()
                );
                if original.trim().is_empty() {
                    set_phase(Phase::Waiting);
                    continue;
                }
                set_phase(Phase::Translating);
                let translated = match pipeline
                    .run(
                        Capability::Translate,
                        TaskInput::Translate {
                            text: original.clone(),
                            source_language: source_lang.clone(),
                            target_language: target_lang.clone(),
                            context: Default::default(),
                        },
                    )
                    .await
                {
                    Ok(t) => match t.output {
                        TaskOutput::Text(s) => s,
                        _ => String::new(),
                    },
                    Err(e) => {
                        tracing::warn!("subtitle: translation failed: {e}");
                        String::new()
                    }
                };
                emit(SubtitleEvent::Chunk {
                    original,
                    translated,
                });
                set_phase(Phase::Waiting);
                // Avoid bursting work when chunks arrive too quickly.
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(e) => {
                failures += 1;
                let gave_up = failures >= MAX_CONSECUTIVE_FAILURES;
                tracing::warn!("subtitle: recognition failed ({failures}): {e}");
                let detail = crate::user_error::provider(&e, Capability::Asr);
                let detail = detail.split_once('|').map_or(detail.as_str(), |(_, m)| m);
                let message = if gave_up {
                    format!("音声認識の連続失敗により停止：{detail}")
                } else {
                    format!("音声認識失敗：{detail}")
                };
                emit(SubtitleEvent::Error { message });
                if gave_up {
                    return true;
                }
            }
        }
    }
    false
}

/// Passes captured frames on to the subtitle loop and reports their
/// level every [`LEVEL_INTERVAL`]. When the device delivers nothing (a
/// loopback is silent while nothing plays), one event with no bins says
/// so and reporting pauses until audio returns. Ends when the capture
/// stops; the loop then sees its channel close.
pub async fn meter_relay(
    mut rx: mpsc::Receiver<AudioFrame>,
    tx: mpsc::Sender<AudioFrame>,
    emit: impl Fn(LevelEvent),
) {
    let gate_db = level::dbfs(SILENCE_RMS);
    let mut meter = level::Meter::default();
    let mut tick = tokio::time::interval(LEVEL_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut empty_ticks = 0u32;
    let mut idle = false;
    loop {
        tokio::select! {
            maybe = rx.recv() => match maybe {
                Some(frame) => {
                    meter.push(&frame);
                    // A full queue drops the frame, as the capture
                    // callback did before the meter sat in between.
                    let _ = tx.try_send(frame);
                }
                None => break,
            },
            _ = tick.tick() => {
                let bins = meter.take();
                if !bins.is_empty() {
                    empty_ticks = 0;
                    idle = false;
                    emit(LevelEvent { bins, gate_db });
                } else if !idle {
                    empty_ticks += 1;
                    if empty_ticks >= IDLE_TICKS {
                        idle = true;
                        emit(LevelEvent { bins, gate_db });
                    }
                }
            }
        }
    }
}

/// How often the input level is sent to the window.
const LEVEL_INTERVAL: Duration = Duration::from_millis(50);

/// Level reports without audio before the input counts as absent.
const IDLE_TICKS: u32 = 3;

/// Frames buffered between the capture thread and the meter, which
/// reads them at once.
const TAP_QUEUE: usize = 256;

/// Stop after this many chunks in a row fail, instead of repeating the
/// same error every few seconds.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;

/// Longest chunk sent to recognition: just under Whisper's 30 s window.
const MAX_WINDOW_SAMPLES: usize = 28 * 16_000;

/// Audio frames buffered between the capture thread and the loop
/// (~40 s at 10 ms per callback).
const FRAME_QUEUE: usize = 4096;

/// A pause this long in the incoming audio sends what is buffered to
/// recognition without waiting for a full chunk.
const FLUSH_AFTER: Duration = Duration::from_millis(700);

/// Shortest buffered audio worth recognizing on a pause (0.5 s).
const MIN_FLUSH_SAMPLES: usize = 8_000;

/// RMS level below which a chunk counts as silence (about -50 dBFS).
const SILENCE_RMS: f64 = 100.0;

fn is_silent(samples: &[i16]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let sum: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum / samples.len() as f64).sqrt() < SILENCE_RMS
}

fn pcm16_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn meter_relay_forwards_frames_and_reports_level_then_absence() {
        let (tap_tx, tap_rx) = mpsc::channel::<AudioFrame>(64);
        let (tx, mut rx) = mpsc::channel::<AudioFrame>(64);
        let events = Arc::new(Mutex::new(Vec::<LevelEvent>::new()));
        let sink = events.clone();
        let relay = tokio::spawn(meter_relay(tap_rx, tx, move |ev| sink.lock().push(ev)));

        // 50 ms of a steady tone in 10 ms frames, as a capture delivers.
        let frame: Vec<i16> = (0..160)
            .map(|i| if i % 2 == 0 { 1000 } else { -1000 })
            .collect();
        for _ in 0..5 {
            tap_tx.send(frame.clone()).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
        let mut forwarded = 0;
        while rx.try_recv().is_ok() {
            forwarded += 1;
        }
        assert_eq!(forwarded, 5);
        {
            let seen = events.lock();
            let bins: Vec<i8> = seen.iter().flat_map(|e| e.bins.clone()).collect();
            assert_eq!(bins, vec![-30; 2]);
            assert!((seen[0].gate_db + 50.3).abs() < 0.1);
        }

        // Nothing plays: one empty report, then no more.
        tokio::time::sleep(Duration::from_millis(1000)).await;
        {
            let seen = events.lock();
            let empty = seen.iter().filter(|e| e.bins.is_empty()).count();
            assert_eq!(empty, 1);
            assert!(seen.last().unwrap().bins.is_empty());
        }

        // Audio returns, then the capture stops.
        for _ in 0..5 {
            tap_tx.send(frame.clone()).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(!events.lock().last().unwrap().bins.is_empty());
        drop(tap_tx);
        relay.await.unwrap();
        while rx.try_recv().is_ok() {}
        assert!(rx.recv().await.is_none());
    }

    #[test]
    fn silence_gate() {
        assert!(is_silent(&[]));
        assert!(is_silent(&vec![0i16; 16_000]));
        // Faint hiss stays below the gate.
        let hiss: Vec<i16> = (0..16_000)
            .map(|i| if i % 2 == 0 { 40 } else { -40 })
            .collect();
        assert!(is_silent(&hiss));
        // Ordinary speech levels are far above it.
        let tone: Vec<i16> = (0..16_000)
            .map(|i| ((i as f32 * 0.05).sin() * 3000.0) as i16)
            .collect();
        assert!(!is_silent(&tone));
    }
}
