//! Live subtitle service.
//!
//! Drives loopback audio capture, slices it into chunks, sends each chunk
//! to whisper.cpp via the ASR provider chain, optionally translates the
//! transcript, and emits the result to the subtitle window. Any number
//! of windows can run, each on its own audio source (the whole PC, one
//! application, or the microphone: a face-to-face conversation,
//! translated in both directions, see `conversation`).

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

use crate::tr;
use crate::{
    app_identity::AppIdentity,
    audio::{
        backend::{AudioFrame, LoopbackStream},
        gate::VoiceGate,
        level, AudioSource,
    },
    clipboard,
    conversation::{Pair, Speaker},
    overlay,
    pipeline::Pipeline,
    providers::{Capability, TaskInput, TaskOutput, TranslateContext},
    settings::SettingsStore,
    storage::Storage,
    subtitle_filter::StockFilter,
    subtitle_record::Recorder,
    subtitle_revise::{Assembler, Languages, Reviser, Revision, Translator},
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
        /// Names the line for a later [`SubtitleEvent::Revised`].
        id: u64,
        original: String,
        translated: String,
        /// Milliseconds from the start of the loop to the end of this
        /// chunk's audio (when its last sample was received).
        at_ms: u64,
        /// Length of the audio sent to recognition.
        duration_ms: u64,
        /// Face-to-face conversation: who said it. The translation is
        /// in the language of the other person.
        #[serde(skip_serializing_if = "Option::is_none")]
        speaker: Option<Speaker>,
    },
    /// A short sentence that keeps coming back: the window asks whether
    /// to block it from now on (see `subtitle_filter`).
    Candidate {
        phrase: String,
    },
    /// A finished sentence, translated again as a whole: it replaces
    /// the lines of the chunks it was recognized in.
    Revised(Revision),
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
    /// No sound, and finished sentences are being translated again.
    Revising,
}

/// What a recognition loop translates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// Subtitles: everything goes into the target language. `app`: key
    /// of the application the audio comes from, when it is one
    /// application's (its glossary is used).
    Subtitle { app: Option<String> },
    /// A face-to-face conversation: each utterance goes into the
    /// language of the other person (`conversation`).
    Conversation,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Subtitle { app: None }
    }
}

impl Mode {
    /// What a window listening to `source` does.
    pub fn of(source: AudioSource) -> Self {
        match source {
            AudioSource::Microphone => Mode::Conversation,
            AudioSource::App { pid } => Mode::Subtitle {
                app: AppIdentity::of_pid(pid).map(|a| a.key),
            },
            AudioSource::System => Mode::Subtitle { app: None },
        }
    }

    fn app(&self) -> Option<&str> {
        match self {
            Mode::Subtitle { app } => app.as_deref(),
            Mode::Conversation => None,
        }
    }
}

/// Label prefix of the subtitle windows (`subtitle-1`, `subtitle-2`, …).
pub const WINDOW_PREFIX: &str = "subtitle-";

/// Title of a subtitle window, in the display language.
pub fn window_title() -> String {
    tr!("subtitle:moonlens_audio_translation")
}

/// Initial size of a window listening to `source` (logical pixels).
/// Subtitles are a strip at the bottom of the screen; a conversation is
/// read by two people and keeps several turns in view.
fn initial_size(source: AudioSource) -> (f64, f64) {
    match source {
        AudioSource::Microphone => (760.0, 440.0),
        _ => (900.0, 190.0),
    }
}

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
    storage: Arc<Storage>,
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

/// Whether [`SubtitleService::run`] starts the first run of a window
/// being opened, or a further one of a window that must still be open.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Window {
    New,
    Open,
}

impl Running {
    fn halt(self) {
        self.stop.notify_waiters();
        self.handle.abort();
    }
}

impl SubtitleService {
    pub fn new(
        settings: Arc<SettingsStore>,
        pipeline: Arc<Pipeline>,
        storage: Arc<Storage>,
    ) -> Self {
        Self {
            settings,
            pipeline,
            storage,
            sessions: Mutex::new(HashMap::new()),
            next_window: AtomicU64::new(1),
            next_run: AtomicU64::new(1),
        }
    }

    /// At least one window is recognizing.
    pub fn is_running(&self) -> bool {
        self.sessions.lock().values().any(|s| s.running.is_some())
    }

    /// A window recognizes everything the PC plays or what the
    /// microphone picks up, which would include speech of MoonLens' own
    /// (see [`crate::speech`]).
    pub fn hears_speech(&self) -> bool {
        self.sessions
            .lock()
            .values()
            .any(|s| s.running.is_some() && !matches!(s.source, AudioSource::App { .. }))
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
                tr!("subtitle:no_speech_recognition_provider_is_set"),
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
                        crate::user_error::coded(
                            "cpu_unsupported",
                            crate::user_error::unsupported_cpu(),
                        )
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
                        tr!(
                            "subtitle:speech_recognition_model_not_downloaded",
                            model = p.model().label
                        ),
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
                tr!(
                    "subtitle:speech_recognition_server_unreachable",
                    servers = unreachable.join(crate::i18n::text("common:list_separator"))
                ),
            )
        }))
    }

    /// Start from the home screen, a shortcut or the tray: resume the
    /// windows that stopped, or open the first one (whole PC). Returns a
    /// coded, user-facing message on failure.
    pub async fn start(&self, app: &AppHandle) -> Result<(), String> {
        self.preflight().await?;
        let failed = |e: anyhow::Error| {
            crate::user_error::coded(
                "error",
                tr!(
                    "common:could_not_start_audio_capture_e",
                    e = format!("{:#}", e)
                ),
            )
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

    /// Start a face-to-face conversation: a window on the microphone,
    /// or the one already open. Returns a coded, user-facing message on
    /// failure.
    pub async fn start_conversation(&self, app: &AppHandle) -> Result<(), String> {
        self.preflight().await?;
        let open = self
            .sessions
            .lock()
            .iter()
            .find(|(_, s)| s.source == AudioSource::Microphone)
            .map(|(label, s)| (label.clone(), s.running.is_some()));
        let started = match open {
            Some((label, true)) => {
                if let Some(win) = app.get_webview_window(&label) {
                    let _ = win.set_focus();
                }
                Ok(())
            }
            Some((label, false)) => self.set_source(app, &label, AudioSource::Microphone),
            None => self.open(app, AudioSource::Microphone, None).map(|_| ()),
        };
        started.map_err(|e| {
            crate::user_error::coded(
                "error",
                tr!(
                    "common:could_not_start_audio_capture_e",
                    e = format!("{:#}", e)
                ),
            )
        })
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
        let (width, height) = initial_size(source);
        let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("/subtitle".into()))
            .title(match source {
                AudioSource::Microphone => tr!("subtitle:moonlens_conversation"),
                _ => window_title(),
            })
            .inner_size(width, height)
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
        if let Err(e) = self.run(app, &label, source, Window::New) {
            let _ = win.destroy();
            return Err(e);
        }
        match beside.and_then(|b| app.get_webview_window(b)) {
            Some(other) => place_beside(&win, &other),
            // Between the two people, not at the edge of the screen.
            None if source == AudioSource::Microphone => {
                let _ = win.center();
            }
            None => place_first(&win),
        }
        win.show().ok();
        Ok(label)
    }

    /// Switch the window `label` to `source` (or restart the same one
    /// after it stopped). The current source keeps running when the new
    /// one cannot be opened.
    pub fn set_source(&self, app: &AppHandle, label: &str, source: AudioSource) -> Result<()> {
        self.run(app, label, source, Window::Open)
    }

    /// Open `source` and start a recognition loop for the window
    /// `label`, replacing its current one.
    fn run(&self, app: &AppHandle, label: &str, source: AudioSource, window: Window) -> Result<()> {
        let missing = || {
            anyhow::anyhow!(tr!(
                "subtitle:audio_translation_window_not_found_label",
                label = label
            ))
        };
        if window == Window::Open && !self.sessions.lock().contains_key(label) {
            return Err(missing());
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
        let stream = LoopbackStream::start_with(source, tap_tx)?;
        let (app_level, level_to) = (app.clone(), label.to_string());
        // A microphone never pauses by itself: the gate passes speech only.
        let gate = source.is_continuous().then(VoiceGate::default);
        tauri::async_runtime::spawn(meter_relay(tap_rx, tx, gate, move |ev| {
            let _ = app_level.emit_to(level_to.as_str(), LEVEL_EVENT, ev);
        }));
        let device = stream.device().to_string();
        let lost = stream.lost();
        tracing::info!("subtitle: {label} started ({source:?}, device: {device})");
        let run = self.next_run.fetch_add(1, Ordering::Relaxed);
        let stop = Arc::new(tokio::sync::Notify::new());
        let stop_in = stop.clone();
        let pipeline = self.pipeline.clone();
        let settings = self.settings.clone();
        let app_emit = app.clone();
        let recorder = Recorder::new(self.storage.clone(), self.settings.clone())
            .conversation(source == AudioSource::Microphone);
        let id = label.to_string();
        let mode = Mode::of(source);
        let started = SubtitleEvent::Started {
            device: device.clone(),
            source,
        };
        // Hold the sessions while the loop is spawned: it must not end
        // (and look itself up) before it is registered.
        let mut sessions = self.sessions.lock();
        // Opening the source took a while: the window may have been
        // closed meanwhile. Starting now would recognize for no window.
        if window == Window::Open && !sessions.contains_key(label) {
            drop(sessions);
            drop(stream);
            return Err(missing());
        }
        let handle = tauri::async_runtime::spawn(async move {
            let emit = {
                let app = app_emit.clone();
                let settings = settings.clone();
                let id = id.clone();
                move |event: SubtitleEvent| {
                    recorder.observe(&event);
                    if let SubtitleEvent::Chunk {
                        original,
                        translated,
                        ..
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
            let gave_up = run_loop(&emit, settings, pipeline, rx, stop_in, mode).await;
            // The loop also ends when the capture closes the frame
            // channel: the application exited or its stream failed.
            let lost = lost.load(Ordering::Relaxed);
            tracing::info!("subtitle: {id} stopped (gave up: {gave_up}, source lost: {lost})");
            // Release the audio stream (and the model, with no other
            // window left), but leave the window up: it shows why the
            // subtitles stopped.
            let current = app_emit
                .state::<crate::app::AppState>()
                .subtitle
                .finish(&id, run)
                .is_some();
            // A run the window has replaced (source switched) or that
            // was closed says nothing more: its window shows the new run.
            if !current {
                return;
            }
            if lost {
                emit(SubtitleEvent::Error {
                    message: tr!("subtitle:the_source_app_exited_or_audio_capture"),
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
        // No longer follows a window of another app.
        if let Some(attachments) = app.try_state::<crate::attach::Handle>() {
            attachments.forget(label);
        }
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
            crate::notices::push(
                NOTICE,
                tr!("common:audio_translation_message", message = message),
                route,
            );
            crate::app::route_main(&app, route.unwrap_or("/"));
        });
    }
}

/// Event name of [`LevelEvent`]s, emitted to the window they belong to.
pub const LEVEL_EVENT: &str = "moonlens://subtitle-level";

/// Event name of [`SubtitleEvent`]s; the payload carries the label of
/// the window it belongs to as `window`.
pub const SUBTITLE_EVENT: &str = "moonlens://subtitle";

/// A [`SubtitleEvent`] with the label of its window.
///
/// The key must not be one an event uses itself: `Chunk` and `Revised`
/// carry the line's `id`, and a flattened struct writes both keys.
#[derive(Serialize, Clone)]
struct Tagged<'a> {
    window: &'a str,
    #[serde(flatten)]
    event: SubtitleEvent,
}

fn emit_event(app: &AppHandle, window: &str, event: SubtitleEvent) {
    let _ = app.emit(SUBTITLE_EVENT, Tagged { window, event });
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
    mode: Mode,
) -> bool {
    let app = mode.app().map(str::to_string);
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
    let loop_started = std::time::Instant::now();
    // When the newest sample in `buffer` was received. Subtitle times
    // come from this, not from the moment a chunk is cut: a chunk cut
    // after a pause would be dated late by the wait for that pause.
    let mut heard_until = Duration::ZERO;
    let mut stock = StockFilter::default();
    let mut lines = Assembler::default();
    let mut reviser = Reviser::new(translator(pipeline.clone(), app.clone()));
    // A sentence approved for blocking after its chunks were shown must
    // not come back as a correction: its lines go (empty text), the
    // start of the next sentence stays.
    let revised = |stock: &mut StockFilter, mut revision: Revision| {
        if stock.blocks_all(&settings.snapshot().subtitle, &revision.original) {
            revision.original.clear();
            revision.translated.clear();
        }
        emit(SubtitleEvent::Revised(revision))
    };
    // The languages of the sentence being recognized. A conversation
    // learns them from each utterance; subtitles read them from the
    // settings on every pass.
    let mut languages = Languages {
        source: String::new(),
        target: String::new(),
    };
    loop {
        let snap = settings.snapshot();
        let realtime = snap.subtitle.realtime_correction;
        let correcting = realtime || snap.subtitle.batch_correction;
        if !correcting {
            lines.clear();
            reviser.clear();
        }
        let conversation = mode == Mode::Conversation;
        if !conversation {
            languages = Languages {
                source: snap.subtitle.source_language.clone(),
                target: snap.subtitle.target_language.clone(),
            };
        }
        let chunk_samples = (snap.subtitle.chunk_seconds * 16_000.0) as usize;
        let overlap_samples = (snap.subtitle.overlap_seconds * 16_000.0) as usize;
        drop(snap);

        // Wait for the next frame, a stop notification, or a pause in
        // the audio: a loopback delivers nothing while nothing plays, so
        // without the pause the end of a sentence would sit in the
        // buffer until sound returns.
        let mut flush = closed;
        // A conversation: the microphone's gate reported a pause (an
        // empty frame). What follows it may be the other person, in the
        // other language, so the audio before it is recognized alone,
        // also when both were queued during a recognition.
        let mut turn_ended = false;
        if !closed {
            tokio::select! {
                _ = stop.notified() => break,
                maybe = rx.recv() => match maybe {
                    Some(frame) if frame.is_empty() => turn_ended = conversation,
                    Some(frame) => {
                        buffer.extend_from_slice(&frame);
                        heard_until = loop_started.elapsed();
                    }
                    None => {
                        closed = true;
                        flush = true;
                    }
                },
                // Also without buffered audio while a sentence is open:
                // the pause is what ends it.
                _ = tokio::time::sleep(FLUSH_AFTER), if !buffer.is_empty() || lines.is_open() => {
                    flush = true;
                }
                // `None`: the work left needed no new translation.
                revision = reviser.next(), if reviser.is_busy() => {
                    if let Some(revision) = revision {
                        revised(&mut stock, revision);
                    }
                    set_phase(idle(&reviser));
                    continue;
                }
            }
        }
        // Collect whatever arrived during the previous recognition.
        while !turn_ended {
            match rx.try_recv() {
                Ok(frame) if frame.is_empty() => turn_ended = conversation,
                Ok(frame) => {
                    buffer.extend_from_slice(&frame);
                    heard_until = loop_started.elapsed();
                }
                Err(_) => break,
            }
        }
        flush |= turn_ended;
        if !heard && !buffer.is_empty() {
            heard = true;
            tracing::info!("subtitle: first audio frames received");
        }
        if buffer.len() < chunk_samples && !(flush && buffer.len() >= MIN_FLUSH_SAMPLES) {
            if flush {
                // Too short to recognize on its own.
                buffer.clear();
                overlap.clear();
                if correcting {
                    pause(&mut lines, &mut reviser, &languages);
                }
                set_phase(idle(&reviser));
            }
            if closed {
                // The audio ended (stopping drops this task instead):
                // finish what the pause above released.
                while let Some(revision) = reviser.next().await {
                    revised(&mut stock, revision);
                }
                set_phase(Phase::Waiting);
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
        let overlapped = !overlap.is_empty();
        let mut to_send = overlap.clone();
        to_send.extend_from_slice(&buffer[..take]);
        let duration_ms = to_send.len() as u64 * 1000 / 16_000;
        // What stays in the buffer was heard after this chunk.
        let left_ms = (buffer.len() - take) as u64 * 1000 / 16_000;
        let at_ms = (heard_until.as_millis() as u64).saturating_sub(left_ms);
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
            if correcting {
                pause(&mut lines, &mut reviser, &languages);
            }
            set_phase(idle(&reviser));
            continue;
        }

        // One inference at a time: recognition and a sentence translation
        // running together slow each other down far more than twofold
        // without a GPU, until the translation runs out of time. A
        // sentence under way finishes first; the next one starts when
        // the loop waits for audio again.
        if let Some(revision) = reviser.settle().await {
            revised(&mut stock, revision);
        }
        set_phase(Phase::Recognizing);
        // Read now, not when the wait for this audio began: a partner
        // language chosen during a silence applies to what is said next.
        let pair = conversation.then(|| Pair::from_settings(&settings.snapshot().subtitle));
        let seconds = to_send.len() as f32 / 16_000.0;
        let started = std::time::Instant::now();
        let pcm_bytes = pcm16_to_bytes(&to_send);
        let recognition = pipeline.run(
            Capability::Asr,
            TaskInput::Asr {
                audio_pcm16_mono_16k: pcm_bytes,
                // A conversation is in one of its two languages, whoever
                // speaks: recognition chooses between them per chunk.
                language_hint: match &pair {
                    None if languages.source != "auto" => Some(languages.source.clone()),
                    _ => None,
                },
                language_candidates: pair.as_ref().map(Pair::candidates).unwrap_or_default(),
            },
        );
        match recognition.await {
            Ok(res) => {
                failures = 0;
                let (segments, heard_in): (Vec<&str>, Option<&str>) = match &res.output {
                    TaskOutput::Transcript { segments, language } => (
                        segments.iter().map(|s| s.text.as_str()).collect(),
                        language.as_deref(),
                    ),
                    TaskOutput::Text(t) => (vec![t.as_str()], None),
                    _ => (Vec::new(), None),
                };
                // Stock misrecognitions go no further: not shown, and
                // not worth a translation.
                let screened = stock.screen(
                    &settings.snapshot().subtitle,
                    &segments,
                    std::time::Instant::now(),
                );
                for phrase in &screened.blocked {
                    tracing::debug!("subtitle: stock phrase removed: {phrase}");
                }
                if let Some(phrase) = screened.candidate {
                    emit(SubtitleEvent::Candidate { phrase });
                }
                let original = screened.text;
                tracing::debug!(
                    "subtitle: {seconds:.1} s of audio recognized in {} ms ({} chars)",
                    started.elapsed().as_millis(),
                    original.chars().count()
                );
                if original.trim().is_empty() {
                    if correcting {
                        pause(&mut lines, &mut reviser, &languages);
                    }
                    set_phase(idle(&reviser));
                    continue;
                }
                // The other person answers: the sentence of the first
                // one ends here, in its own languages.
                let speaker = pair.as_ref().map(|pair| {
                    let turn = pair.turn(heard_in, &original);
                    if correcting && turn.languages != languages {
                        pause(&mut lines, &mut reviser, &languages);
                    }
                    languages = turn.languages;
                    turn.speaker
                });
                set_phase(Phase::Translating);
                let translation = pipeline.run(
                    Capability::Translate,
                    TaskInput::Translate {
                        text: original.clone(),
                        source_language: languages.source.clone(),
                        target_language: languages.target.clone(),
                        context: TranslateContext::of_app(app.as_deref()),
                    },
                );
                let translated = match translation.await {
                    Ok(t) => match t.output {
                        TaskOutput::Text(s) => s,
                        _ => String::new(),
                    },
                    Err(e) => {
                        tracing::warn!("subtitle: translation failed: {e}");
                        String::new()
                    }
                };
                // Approved for blocking while it was being translated
                // (the window asked before translation began): the
                // window dropped its copies, do not bring one back.
                if stock.blocks_all(&settings.snapshot().subtitle, &original) {
                    if correcting {
                        pause(&mut lines, &mut reviser, &languages);
                    }
                    set_phase(idle(&reviser));
                    continue;
                }
                let id = lines.next_id();
                if correcting {
                    if let Some(sentence) = lines.push(id, &original, &translated, overlapped) {
                        reviser.push(sentence, languages.clone(), realtime);
                    }
                }
                emit(SubtitleEvent::Chunk {
                    id,
                    original,
                    translated,
                    at_ms,
                    duration_ms,
                    speaker,
                });
                if correcting && flush {
                    pause(&mut lines, &mut reviser, &languages);
                }
                set_phase(idle(&reviser));
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
                    tr!(
                        "subtitle:stopped_after_repeated_speech_recognition",
                        detail = detail
                    )
                } else {
                    tr!("subtitle:speech_recognition_failed_detail", detail = detail)
                };
                emit(SubtitleEvent::Error { message });
                if gave_up {
                    return true;
                }
                // The chunk's words are lost: the open sentence cannot
                // be completed, and the next chunk overlaps this audio,
                // not the last recognized text.
                if correcting {
                    pause(&mut lines, &mut reviser, &languages);
                }
                // Recognition is over, failed or not: without this the
                // window says "recognizing" until the next chunk.
                set_phase(idle(&reviser));
            }
        }
    }
    false
}

/// What the loop shows between chunks.
fn idle(reviser: &Reviser) -> Phase {
    if reviser.is_busy() {
        Phase::Revising
    } else {
        Phase::Waiting
    }
}

/// The speech paused: the open sentence ends here, and every sentence
/// held back for a pause may now be translated again.
fn pause(lines: &mut Assembler, reviser: &mut Reviser, languages: &Languages) {
    if let Some(sentence) = lines.flush() {
        reviser.push(sentence, languages.clone(), true);
    }
    reviser.release();
}

/// Sentence translation for [`Reviser`] through the provider chain,
/// with the glossary of application `app` when the audio is its.
fn translator(pipeline: Arc<Pipeline>, app: Option<String>) -> Translator {
    Arc::new(move |text, languages: Languages, context| {
        let pipeline = pipeline.clone();
        let context = TranslateContext {
            app: app.clone(),
            ..context
        };
        Box::pin(async move {
            let input = TaskInput::Translate {
                text,
                source_language: languages.source,
                target_language: languages.target,
                context,
            };
            match pipeline.run(Capability::Translate, input).await {
                Ok(res) => match res.output {
                    TaskOutput::Text(s) => Some(s),
                    _ => None,
                },
                Err(e) => {
                    tracing::warn!("subtitle: sentence translation failed: {e}");
                    None
                }
            }
        })
    })
}

/// Passes captured frames on to the subtitle loop and reports their
/// level every [`LEVEL_INTERVAL`]. When the device delivers nothing (a
/// loopback is silent while nothing plays), one event with no bins says
/// so and reporting pauses until audio returns. Ends when the capture
/// stops; the loop then sees its channel close.
///
/// `gate` (a microphone): only what it lets through goes on to the
/// loop, and the reported threshold is the gate's, which follows the
/// room's noise. The level shown is always that of the input.
pub async fn meter_relay(
    mut rx: mpsc::Receiver<AudioFrame>,
    tx: mpsc::Sender<AudioFrame>,
    mut gate: Option<VoiceGate>,
    emit: impl Fn(LevelEvent),
) {
    let gate_db = |gate: &Option<VoiceGate>| {
        level::dbfs(gate.as_ref().map_or(SILENCE_RMS, VoiceGate::threshold))
    };
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
                    let passed = match gate.as_mut() {
                        Some(gate) => gate.push(frame),
                        None => vec![frame],
                    };
                    // A full queue drops the frame, as the capture
                    // callback did before the meter sat in between.
                    for frame in passed {
                        let _ = tx.try_send(frame);
                    }
                }
                None => break,
            },
            _ = tick.tick() => {
                let bins = meter.take();
                if !bins.is_empty() {
                    empty_ticks = 0;
                    idle = false;
                    emit(LevelEvent { bins, gate_db: gate_db(&gate) });
                } else if !idle {
                    empty_ticks += 1;
                    if empty_ticks >= IDLE_TICKS {
                        idle = true;
                        emit(LevelEvent { bins, gate_db: gate_db(&gate) });
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
const SILENCE_RMS: f64 = crate::audio::gate::MIN_THRESHOLD;

pub(crate) fn is_silent(samples: &[i16]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let sum: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum / samples.len() as f64).sqrt() < SILENCE_RMS
}

pub(crate) fn pcm16_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window key that an event also uses is written twice, and the
    /// window then reads one value for both (JSON keeps the last).
    #[test]
    fn window_label_shares_no_key_with_an_event() {
        let events = [
            SubtitleEvent::Chunk {
                id: 7,
                original: "a".into(),
                translated: "b".into(),
                at_ms: 0,
                duration_ms: 0,
                speaker: None,
            },
            SubtitleEvent::Revised(crate::subtitle_revise::Revision {
                replaces: vec![7],
                id: 8,
                original: "a".into(),
                translated: "b".into(),
                rest: None,
            }),
            SubtitleEvent::Stopped,
        ];
        for event in events {
            let json = serde_json::to_string(&Tagged {
                window: "subtitle-2",
                event,
            })
            .unwrap();
            for key in [r#""id":"#, r#""window":"#, r#""kind":"#] {
                assert!(json.matches(key).count() <= 1, "{key} twice in {json}");
            }
            assert!(json.contains(r#""window":"subtitle-2""#), "{json}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn meter_relay_forwards_frames_and_reports_level_then_absence() {
        let (tap_tx, tap_rx) = mpsc::channel::<AudioFrame>(64);
        let (tx, mut rx) = mpsc::channel::<AudioFrame>(64);
        let events = Arc::new(Mutex::new(Vec::<LevelEvent>::new()));
        let sink = events.clone();
        let relay = tokio::spawn(meter_relay(tap_rx, tx, None, move |ev| {
            sink.lock().push(ev)
        }));

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

    /// A microphone: room noise stays out of the loop (which then sees
    /// the pause that ends a sentence), while the level display still
    /// shows it, against the threshold that follows the noise.
    #[tokio::test(start_paused = true)]
    async fn a_gated_relay_passes_speech_only_and_reports_the_gates_threshold() {
        let (tap_tx, tap_rx) = mpsc::channel::<AudioFrame>(512);
        let (tx, mut rx) = mpsc::channel::<AudioFrame>(512);
        let events = Arc::new(Mutex::new(Vec::<LevelEvent>::new()));
        let sink = events.clone();
        let gate = Some(VoiceGate::default());
        let relay = tokio::spawn(meter_relay(tap_rx, tx, gate, move |ev| {
            sink.lock().push(ev)
        }));
        let frame = |level: i16| -> Vec<i16> {
            (0..160)
                .map(|i| if i % 2 == 0 { level } else { -level })
                .collect()
        };

        // One second of hiss above the fixed silence gate (RMS 100).
        for _ in 0..100 {
            tap_tx.send(frame(200)).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(rx.try_recv().is_err(), "noise reached the loop");
        {
            let seen = events.lock();
            let last = seen.last().expect("the level is still reported");
            assert!(last.bins.iter().all(|&db| db == -44), "{:?}", last.bins);
            assert!(last.gate_db > level::dbfs(200.0), "{}", last.gate_db);
        }

        // Speech passes, with the audio right before it.
        for _ in 0..50 {
            tap_tx.send(frame(3000)).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(60)).await;
        let mut passed = 0;
        while let Ok(f) = rx.try_recv() {
            passed += f.len();
        }
        assert!(passed >= 50 * 160, "{passed}");
        drop(tap_tx);
        relay.await.unwrap();
    }

    #[test]
    fn a_microphone_window_is_a_conversation() {
        assert_eq!(Mode::of(AudioSource::Microphone), Mode::Conversation);
        assert_eq!(Mode::of(AudioSource::System), Mode::default());
        assert!(AudioSource::Microphone.is_continuous());
        assert!(!AudioSource::System.is_continuous());
        assert_eq!(
            serde_json::to_value(AudioSource::Microphone).unwrap(),
            serde_json::json!({ "kind": "microphone" })
        );
        let said = SubtitleEvent::Chunk {
            id: 1,
            original: "a".into(),
            translated: "b".into(),
            at_ms: 0,
            duration_ms: 0,
            speaker: Some(Speaker::Partner),
        };
        assert_eq!(
            serde_json::to_value(&said).unwrap()["speaker"],
            serde_json::json!("partner")
        );
    }

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
