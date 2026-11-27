//! Read-aloud of translations with the speech synthesis of the OS.
//!
//! Fully local: Windows speaks through its installed voices
//! (`Windows.Media.SpeechSynthesis`), macOS through `say`, Linux
//! through Speech Dispatcher. No model of MoonLens' own is loaded, so
//! nothing competes with translation for the GPU or its memory.
//!
//! A window asks for its text to be spoken ([`speak`]); one worker
//! thread, started on first use, speaks the requests in turn
//! ([`Player`]). The backlog is short: speech that falls behind the
//! screen drops what waited longest instead of trailing further.
//!
//! Speech is never played into a capture that would hear it: while a
//! subtitle window listens to the whole PC or the microphone, requests are refused
//! ([`Host::blocked`]), or its recognition would transcribe and
//! translate what MoonLens says, and read that aloud in turn.

mod command;
#[cfg(windows)]
mod winrt;

use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Result;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use crate::app::AppState;

/// Event with a [`Event`], sent to the window that asked.
pub const EVENT: &str = "moonlens://speech";

/// Requests waiting behind the one being spoken. One more arriving
/// drops the oldest: by then the screen shows something else.
pub const MAX_PENDING: usize = 2;

/// Longest text spoken as one request, in characters (the limit of
/// the text translation page).
pub const MAX_CHARS: usize = 5000;

/// How often the worker asks the engine whether it still speaks.
const POLL: Duration = Duration::from_millis(50);

/// A voice installed on this PC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Voice {
    /// Identifies the voice to the engine and in the settings.
    pub id: String,
    pub name: String,
    /// BCP 47 tag ("ja-JP").
    pub lang: String,
}

/// `[speech]` of the settings, as the engine takes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tuning {
    /// Preferred voice id; empty: the first voice of the language.
    pub voice: String,
    /// Speed, 1.0 = the voice's own.
    pub rate: f32,
    /// 0.0 to 1.0.
    pub volume: f32,
}

/// One text to speak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Utterance {
    /// Label of the window that asked.
    pub owner: String,
    pub text: String,
    /// Language of `text` ("ja", "zh-TW").
    pub lang: String,
}

/// Why a request was not spoken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Skip {
    /// A subtitle window listens to the whole PC or the microphone.
    Blocked,
    /// No installed voice speaks the language.
    NoVoice,
    /// The engine failed (see the log).
    Failed,
}

/// What happens to the requests of one window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Started,
    /// Spoken to the end, or stopped.
    Ended,
    Skipped {
        reason: Skip,
        lang: String,
    },
    /// Left the queue unspoken: newer requests took its place.
    Dropped,
}

/// Speaks one text at a time. Runs on the worker thread only.
pub trait Engine {
    fn voices(&mut self) -> Vec<Voice>;
    /// Start speaking; returns at once.
    fn start(&mut self, text: &str, voice: &Voice, tuning: &Tuning) -> Result<()>;
    /// The text given to [`Engine::start`] is still being spoken.
    fn playing(&mut self) -> bool;
    fn stop(&mut self);
}

/// What the player needs from the app.
pub trait Host {
    fn tuning(&self) -> Tuning;
    /// Speech would be heard by a capture of MoonLens' own.
    fn blocked(&self) -> bool;
    fn emit(&self, owner: &str, event: Event);
}

/// Text as it is spoken: one line, at most [`MAX_CHARS`] characters.
/// `None` when there is nothing to pronounce.
pub fn prepare(text: &str) -> Option<String> {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if !line.chars().any(char::is_alphanumeric) {
        return None;
    }
    Some(line.chars().take(MAX_CHARS).collect())
}

fn primary(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Regions whose voices read the script of a Chinese target language.
fn regions(lang: &str) -> &'static [&'static str] {
    match lang.to_ascii_lowercase().as_str() {
        "zh-hans" => &["cn", "sg"],
        "zh-hant" => &["tw", "hk", "mo"],
        _ => &[],
    }
}

/// The voice for `lang` (a target language: "ja", "zh-Hant"): the
/// preferred one when it speaks that language, else one of the same
/// tag or script, else the first of the language.
pub fn pick_voice<'a>(voices: &'a [Voice], lang: &str, preferred: &str) -> Option<&'a Voice> {
    let same_language = |v: &&Voice| primary(&v.lang) == primary(lang);
    let same_tag = |v: &&Voice| {
        let tag = v.lang.replace('_', "-").to_ascii_lowercase();
        let region = tag.rsplit('-').next().unwrap_or_default();
        tag == lang.to_ascii_lowercase() || regions(lang).contains(&region)
    };
    voices
        .iter()
        .filter(same_language)
        .find(|v| !preferred.is_empty() && v.id == preferred)
        .or_else(|| voices.iter().filter(same_language).find(same_tag))
        .or_else(|| voices.iter().find(same_language))
}

/// The queue of requests and the one being spoken.
pub struct Player<E, H> {
    engine: E,
    host: H,
    pending: VecDeque<Utterance>,
    current: Option<Utterance>,
    /// Asked of the engine once: installed voices rarely change, and
    /// listing them may start a process.
    voices: Option<Vec<Voice>>,
}

impl<E: Engine, H: Host> Player<E, H> {
    pub fn new(engine: E, host: H) -> Self {
        Self {
            engine,
            host,
            pending: VecDeque::new(),
            current: None,
            voices: None,
        }
    }

    /// Nothing is spoken or waiting.
    pub fn idle(&self) -> bool {
        self.current.is_none() && self.pending.is_empty()
    }

    /// Installed voices, asked of the engine again (the settings
    /// screen: a voice may have been installed meanwhile).
    pub fn voices(&mut self) -> Vec<Voice> {
        let voices = self.engine.voices();
        self.voices = Some(voices.clone());
        voices
    }

    pub fn say(&mut self, utterance: Utterance) {
        let Some(text) = prepare(&utterance.text) else {
            return;
        };
        if self.host.blocked() {
            self.skip(&utterance, Skip::Blocked);
            return;
        }
        self.pending.push_back(Utterance { text, ..utterance });
        while self.pending.len() > MAX_PENDING {
            if let Some(old) = self.pending.pop_front() {
                self.host.emit(&old.owner, Event::Dropped);
            }
        }
        self.advance();
    }

    /// Drop what `owner` asked for, spoken or waiting.
    pub fn cancel(&mut self, owner: &str) {
        self.pending.retain(|u| u.owner != owner);
        if self.current.as_ref().is_some_and(|u| u.owner == owner) {
            self.halt();
            self.advance();
        }
    }

    /// Called every [`POLL`] while not idle.
    pub fn tick(&mut self) {
        if self.host.blocked() {
            let waiting: Vec<Utterance> = self.pending.drain(..).collect();
            let spoken = self.current.clone();
            self.halt();
            let mut told: Vec<&str> = Vec::new();
            for u in spoken.iter().chain(&waiting) {
                if !told.contains(&u.owner.as_str()) {
                    told.push(&u.owner);
                    self.skip(u, Skip::Blocked);
                }
            }
            return;
        }
        if self.current.is_some() && !self.engine.playing() {
            self.finish();
        }
        self.advance();
    }

    fn skip(&self, u: &Utterance, reason: Skip) {
        let lang = u.lang.clone();
        self.host.emit(&u.owner, Event::Skipped { reason, lang });
    }

    fn finish(&mut self) {
        if let Some(u) = self.current.take() {
            self.host.emit(&u.owner, Event::Ended);
        }
    }

    fn halt(&mut self) {
        if self.current.is_some() {
            self.engine.stop();
            self.finish();
        }
    }

    fn advance(&mut self) {
        while self.current.is_none() {
            let Some(u) = self.pending.pop_front() else {
                return;
            };
            let tuning = self.host.tuning();
            if self.voices.is_none() {
                self.voices = Some(self.engine.voices());
            }
            let voices = self.voices.as_deref().unwrap_or_default();
            let Some(voice) = pick_voice(voices, &u.lang, &tuning.voice).cloned() else {
                // Installed since the list was read?
                self.voices = None;
                self.skip(&u, Skip::NoVoice);
                continue;
            };
            match self.engine.start(&u.text, &voice, &tuning) {
                Ok(()) => {
                    self.host.emit(&u.owner, Event::Started);
                    self.current = Some(u);
                }
                Err(e) => {
                    tracing::warn!("speech: {} ({}) failed: {e:#}", voice.name, voice.lang);
                    self.skip(&u, Skip::Failed);
                }
            }
        }
    }
}

enum Command {
    Say(Utterance),
    Cancel(String),
    Voices(mpsc::Sender<Vec<Voice>>),
}

/// Handle to the worker thread.
pub struct SpeechService {
    app: AppHandle,
    worker: Mutex<Option<mpsc::Sender<Command>>>,
}

struct AppHost(AppHandle);

impl Host for AppHost {
    fn tuning(&self) -> Tuning {
        let s = self.0.state::<AppState>().settings.snapshot();
        Tuning {
            voice: s.speech.voice.clone(),
            rate: s.speech.rate,
            volume: s.speech.volume_percent.min(100) as f32 / 100.0,
        }
    }

    fn blocked(&self) -> bool {
        self.0.state::<AppState>().subtitle.hears_speech()
    }

    fn emit(&self, owner: &str, event: Event) {
        let _ = self.0.emit_to(owner, EVENT, event);
    }
}

#[cfg(windows)]
fn engine() -> impl Engine {
    winrt::WindowsEngine::default()
}

#[cfg(not(windows))]
fn engine() -> impl Engine {
    command::CommandEngine::default()
}

impl SpeechService {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            worker: Mutex::new(None),
        }
    }

    fn send(&self, command: Command) {
        let mut worker = self.worker.lock();
        let command = match worker.as_ref() {
            None => command,
            Some(tx) => match tx.send(command) {
                Ok(()) => return,
                // The worker is gone (its engine panicked): start another.
                Err(mpsc::SendError(command)) => command,
            },
        };
        let (tx, rx) = mpsc::channel();
        let app = self.app.clone();
        let spawned = std::thread::Builder::new()
            .name("speech".into())
            .spawn(move || run(Player::new(engine(), AppHost(app)), rx));
        match spawned {
            Ok(_) => {
                let _ = tx.send(command);
                *worker = Some(tx);
            }
            Err(e) => tracing::warn!("speech: worker thread failed to start: {e}"),
        }
    }

    pub fn say(&self, utterance: Utterance) {
        self.send(Command::Say(utterance));
    }

    pub fn cancel(&self, owner: &str) {
        // Nothing was ever spoken: no worker to start for it.
        if self.worker.lock().is_some() {
            self.send(Command::Cancel(owner.to_string()));
        }
    }

    /// Blocks until the engine has listed its voices.
    pub fn voices(&self) -> Vec<Voice> {
        let (tx, rx) = mpsc::channel();
        self.send(Command::Voices(tx));
        rx.recv().unwrap_or_default()
    }
}

fn run<E: Engine, H: Host>(mut player: Player<E, H>, rx: mpsc::Receiver<Command>) {
    loop {
        let command = if player.idle() {
            rx.recv().ok()
        } else {
            match rx.recv_timeout(POLL) {
                Ok(c) => Some(c),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        };
        match command {
            Some(Command::Say(u)) => player.say(u),
            Some(Command::Cancel(owner)) => player.cancel(&owner),
            Some(Command::Voices(reply)) => {
                let _ = reply.send(player.voices());
            }
            None if player.idle() => return,
            None => {}
        }
        player.tick();
    }
}

/// Speak `text` (in the language `lang`) for the calling window. What
/// becomes of it arrives as [`EVENT`].
#[tauri::command]
pub async fn speak(
    text: String,
    lang: String,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.speech.say(Utterance {
        owner: window.label().to_string(),
        text,
        lang,
    });
    Ok(())
}

/// Stop what the calling window asked to be spoken.
#[tauri::command]
pub async fn stop_speaking(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.speech.cancel(window.label());
    Ok(())
}

/// Voices installed on this PC.
#[tauri::command]
pub async fn list_voices(state: State<'_, AppState>) -> Result<Vec<Voice>, String> {
    let speech = state.speech.clone();
    tauri::async_runtime::spawn_blocking(move || speech.voices())
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
