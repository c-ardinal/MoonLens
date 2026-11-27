//! Live subtitle service.
//!
//! Drives loopback audio capture, slices it into chunks, sends each chunk
//! to whisper.cpp via the ASR provider chain, optionally translates the
//! transcript, and emits the result to the subtitle window. Any number
//! of windows can run, each on its own audio source (the whole PC or one
//! application).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::mpsc;

use crate::{
    audio::{
        backend::{AudioFrame, LoopbackStream},
        AudioSource,
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
        /// The device or application whose audio is captured.
        device: String,
        source: AudioSource,
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

/// Label prefix of the subtitle windows (`subtitle-1`, `subtitle-2`, …).
pub const WINDOW_PREFIX: &str = "subtitle-";

/// `label` is a subtitle window.
pub fn is_window(label: &str) -> bool {
    label.starts_with(WINDOW_PREFIX)
}

/// What one subtitle window listens to and whether it is recognizing,
/// for a window that (re)loads.
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub source: AudioSource,
    /// Name of the captured device or application.
    pub device: String,
    pub running: bool,
}

/// The subtitle windows: each has its own audio source, capture stream
/// and recognition loop. All share the speech and translation models.
pub struct SubtitleService {
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
    /// By window label.
    sessions: Mutex<HashMap<String, Session>>,
    next_window: AtomicU64,
    next_run: AtomicU64,
}

struct Session {
    source: AudioSource,
    device: String,
    /// `None` once recognition gave up or the application went away;
    /// the window stays and shows why.
    running: Option<Running>,
}

struct Running {
    /// Tells a loop that ends by itself whether it is still the
    /// window's current one (the source may have been switched).
    run: u64,
    _stream: LoopbackStream,
    handle: JoinHandle<()>,
    stop: Arc<tokio::sync::Notify>,
}

impl Running {
    fn halt(self) {
        self.stop.notify_waiters();
        self.handle.abort();
    }
}

impl SubtitleService {
    pub fn new(settings: Arc<SettingsStore>, pipeline: Arc<Pipeline>) -> Self {
        Self {
            settings,
            pipeline,
            sessions: Mutex::new(HashMap::new()),
            next_window: AtomicU64::new(1),
            next_run: AtomicU64::new(1),
        }
    }

    /// At least one window is recognizing.
    pub fn is_running(&self) -> bool {
        self.sessions.lock().values().any(|s| s.running.is_some())
    }

    /// At least one subtitle window is open (recognizing or showing why
    /// it stopped).
    pub fn has_windows(&self) -> bool {
        !self.sessions.lock().is_empty()
    }

    pub fn session(&self, label: &str) -> Option<SessionInfo> {
        self.sessions.lock().get(label).map(|s| SessionInfo {
            source: s.source,
            device: s.device.clone(),
            running: s.running.is_some(),
        })
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

    /// Start from the home screen, a shortcut or the tray: resume the
    /// windows that stopped, or open the first one (whole PC). Returns a
    /// coded, user-facing message on failure.
    pub async fn start(&self, app: &AppHandle) -> Result<(), String> {
        self.preflight().await?;
        let failed = |e: anyhow::Error| {
            crate::user_error::coded("error", format!("音声取得の開始失敗：{e:#}"))
        };
        let stopped: Vec<(String, AudioSource)> = self
            .sessions
            .lock()
            .iter()
            .filter(|(_, s)| s.running.is_none())
            .map(|(label, s)| (label.clone(), s.source))
            .collect();
        if self.has_windows() {
            for (label, source) in stopped {
                self.set_source(app, &label, source).map_err(failed)?;
            }
            return Ok(());
        }
        self.open(app, AudioSource::System, None)
            .map(|_| ())
            .map_err(failed)
    }

    /// Open one more subtitle window listening to `source`, next to the
    /// window `beside` when given. Returns its label.
    pub fn open(
        &self,
        app: &AppHandle,
        source: AudioSource,
        beside: Option<&str>,
    ) -> Result<String> {
        let label = format!(
            "{WINDOW_PREFIX}{}",
            self.next_window.fetch_add(1, Ordering::Relaxed)
        );
        let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("/subtitle".into()))
            .title("MoonLens 音声翻訳")
            .inner_size(900.0, 190.0)
            .min_inner_size(220.0, 96.0)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(true)
            .focused(false)
            .visible(false)
            .build()
            .context("create subtitle window")?;
        if let Err(e) = self.run(app, &label, source) {
            let _ = win.destroy();
            return Err(e);
        }
        match beside.and_then(|b| app.get_webview_window(b)) {
            Some(other) => place_beside(&win, &other),
            None => place_first(&win),
        }
        win.show().ok();
        Ok(label)
    }

    /// Switch the window `label` to `source` (or restart the same one
    /// after it stopped). The current source keeps running when the new
    /// one cannot be opened.
    pub fn set_source(&self, app: &AppHandle, label: &str, source: AudioSource) -> Result<()> {
        anyhow::ensure!(
            self.sessions.lock().contains_key(label),
            "音声翻訳ウィンドウ未検出：{label}"
        );
        self.run(app, label, source)
    }

    /// Open `source` and start a recognition loop for the window
    /// `label`, replacing its current one.
    fn run(&self, app: &AppHandle, label: &str, source: AudioSource) -> Result<()> {
        // Frames keep arriving while a chunk is being recognized; the
        // capture callback drops what does not fit, so leave room for a
        // long decode (callbacks deliver ~10 ms each).
        let (tx, rx) = mpsc::channel::<AudioFrame>(FRAME_QUEUE);
        let stream = LoopbackStream::start_with(source, tx)?;
        let device = stream.device().to_string();
        let lost = stream.lost();
        tracing::info!("subtitle: {label} started ({source:?}, device: {device})");
        let run = self.next_run.fetch_add(1, Ordering::Relaxed);
        let stop = Arc::new(tokio::sync::Notify::new());
        let stop_in = stop.clone();
        let pipeline = self.pipeline.clone();
        let settings = self.settings.clone();
        let app_emit = app.clone();
        let id = label.to_string();
        let started = SubtitleEvent::Started {
            device: device.clone(),
            source,
        };
        // Hold the sessions while the loop is spawned: it must not end
        // (and look itself up) before it is registered.
        let mut sessions = self.sessions.lock();
        let handle = tauri::async_runtime::spawn(async move {
            let emit = {
                let app = app_emit.clone();
                let settings = settings.clone();
                let id = id.clone();
                move |event: SubtitleEvent| {
                    if let SubtitleEvent::Chunk {
                        original,
                        translated,
                    } = &event
                    {
                        let text = if translated.is_empty() {
                            original
                        } else {
                            translated
                        };
                        let cfg = settings.snapshot().clipboard.clone();
                        clipboard::auto_copy(&app, clipboard::Kind::Transcript, text, &cfg);
                    }
                    emit_event(&app, &id, event);
                }
            };
            emit(started);
            let gave_up = run_loop(&emit, settings, pipeline, rx, stop_in).await;
            // The loop also ends when the capture closes the frame
            // channel: the application exited or its stream failed.
            let lost = lost.load(Ordering::Relaxed);
            tracing::info!("subtitle: {id} stopped (gave up: {gave_up}, source lost: {lost})");
            // Release the audio stream (and the model, with no other
            // window left), but leave the window up: it shows why the
            // subtitles stopped.
            let ended = app_emit
                .state::<crate::app::AppState>()
                .subtitle
                .finish(&id, run);
            if lost && ended.is_some() {
                emit(SubtitleEvent::Error {
                    message: "音源アプリ終了、または音声取得失敗".into(),
                });
            }
            emit(SubtitleEvent::Stopped);
        });
        let previous = sessions.insert(
            label.to_string(),
            Session {
                source,
                device,
                running: Some(Running {
                    run,
                    _stream: stream,
                    handle,
                    stop,
                }),
            },
        );
        drop(sessions);
        if let Some(old) = previous.and_then(|s| s.running) {
            old.halt();
        }
        Ok(())
    }

    /// A loop ended by itself: drop its stream, unless the window has
    /// moved on to another run or is gone.
    fn finish(&self, label: &str, run: u64) -> Option<()> {
        let ended = {
            let mut sessions = self.sessions.lock();
            let session = sessions.get_mut(label)?;
            if session.running.as_ref()?.run != run {
                return None;
            }
            session.running.take()
        };
        // Joins the capture thread; not under the lock.
        drop(ended);
        self.release_model_when_idle();
        Some(())
    }

    /// Give back the ~1 GB the speech model holds once no window
    /// recognizes any more. Off this thread: it waits for an aborted
    /// decode to wind down.
    fn release_model_when_idle(&self) {
        if !self.is_running() {
            std::thread::spawn(crate::providers::asr_local::release);
        }
    }

    /// The window `label` is closing (× button, Esc, Alt+F4): stop its
    /// recognition and forget it.
    pub fn forget(&self, app: &AppHandle, label: &str) {
        let Some(session) = self.sessions.lock().remove(label) else {
            return;
        };
        if let Some(running) = session.running {
            running.halt();
        }
        self.release_model_when_idle();
        // The aborted loop cannot report it any more.
        emit_event(app, label, SubtitleEvent::Stopped);
    }

    /// Stop and close one subtitle window.
    pub fn close(&self, app: &AppHandle, label: &str) {
        self.forget(app, label);
        if let Some(win) = app.get_webview_window(label) {
            let _ = win.destroy();
        }
    }

    /// Stop and close every subtitle window.
    pub fn stop(&self, app: &AppHandle) {
        let labels: Vec<String> = self.sessions.lock().keys().cloned().collect();
        for label in labels {
            self.close(app, &label);
        }
        // Reset overlay context if any.
        overlay::hide(app).ok();
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
            if this.has_windows() {
                this.stop(&app);
                return;
            }
            let failure = match this.start(&app).await {
                Ok(()) => {
                    crate::notices::clear(NOTICE);
                    return;
                }
                Err(coded) => coded,
            };
            let (code, message) = failure.split_once('|').unwrap_or(("error", &failure));
            let route = (code == "model_missing").then_some("/settings/#section-model-asr");
            crate::notices::push(NOTICE, format!("音声翻訳：{message}"), route);
            crate::app::route_main(&app, route.unwrap_or("/"));
        });
    }
}

/// Event name of [`SubtitleEvent`]s; the payload carries the label of
/// the window it belongs to as `id`.
pub const SUBTITLE_EVENT: &str = "moonlens://subtitle";

fn emit_event(app: &AppHandle, id: &str, event: SubtitleEvent) {
    #[derive(Serialize, Clone)]
    struct Tagged<'a> {
        id: &'a str,
        #[serde(flatten)]
        event: SubtitleEvent,
    }
    let _ = app.emit(SUBTITLE_EVENT, Tagged { id, event });
}

/// The first window: bottom centre of the primary screen, where
/// subtitles are expected.
fn place_first(win: &tauri::WebviewWindow) {
    let (Ok(Some(monitor)), Ok(size)) = (win.primary_monitor(), win.outer_size()) else {
        return;
    };
    let (origin, screen) = (monitor.position(), monitor.size());
    let margin = (96.0 * monitor.scale_factor()) as i32;
    let x = origin.x + (screen.width as i32 - size.width as i32) / 2;
    let y = origin.y + screen.height as i32 - size.height as i32 - margin;
    let _ = win.set_position(tauri::PhysicalPosition::new(x, y.max(origin.y)));
}

/// A window opened from `other`: the same size, right above it (below
/// when there is no room above).
fn place_beside(win: &tauri::WebviewWindow, other: &tauri::WebviewWindow) {
    let (Ok(at), Ok(size)) = (other.outer_position(), other.outer_size()) else {
        return place_first(win);
    };
    let scale = other.scale_factor().unwrap_or(1.0);
    let gap = (8.0 * scale) as i32;
    let top = other
        .current_monitor()
        .ok()
        .flatten()
        .map_or(0, |m| m.position().y);
    let y = beside_y(at.y, size.height as i32, gap, top);
    let _ = win.set_size(size);
    let _ = win.set_position(tauri::PhysicalPosition::new(at.x, y));
}

/// Top edge of a window of `height` placed above the one at `y`, or
/// below it when that would leave the screen (whose top is `top`).
fn beside_y(y: i32, height: i32, gap: i32, top: i32) -> i32 {
    let above = y - height - gap;
    if above >= top {
        above
    } else {
        y + height + gap
    }
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

    #[test]
    fn a_new_window_goes_above_its_source_or_below_at_the_screen_top() {
        assert_eq!(beside_y(800, 190, 8, 0), 602);
        assert_eq!(beside_y(100, 190, 8, 0), 298);
        // A screen above the primary one has a negative top.
        assert_eq!(beside_y(-900, 190, 8, -1080), -702);
    }

    #[test]
    fn events_carry_the_window_and_the_source() {
        let started = SubtitleEvent::Started {
            device: "vlc".into(),
            source: AudioSource::App { pid: 42 },
        };
        assert_eq!(
            serde_json::to_value(&started).unwrap(),
            serde_json::json!({
                "kind": "started",
                "device": "vlc",
                "source": { "kind": "app", "pid": 42 },
            })
        );
        assert!(is_window("subtitle-3"));
        assert!(!is_window("main") && !is_window("lens-1"));
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
