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
    audio::backend::{AudioFrame, LoopbackStream},
    clipboard, overlay,
    pipeline::Pipeline,
    providers::{Capability, TaskInput, TaskOutput},
    settings::SettingsStore,
};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubtitleEvent {
    Started,
    Stopped,
    Chunk {
        original: String,
        translated: String,
    },
    Error {
        message: String,
    },
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
    /// window. The built-in chain talks to a whisper.cpp server; when
    /// none answers, starting would only produce an error every few
    /// seconds. Returns a coded, user-facing message (see `user_error`).
    pub async fn preflight(&self) -> Result<(), String> {
        let s = self.settings.snapshot();
        let chain = &s.providers.asr.chain;
        if chain.is_empty() {
            return Err(crate::user_error::coded(
                "asr_unavailable",
                "音声認識プロバイダー未設定",
            ));
        }
        // Plugins and other providers are trusted to report their own
        // errors; only whisper.cpp servers can be probed up front.
        if !chain.iter().all(|id| id.starts_with("whisper_cpp.")) {
            return Ok(());
        }
        let http = reqwest::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
            .map_err(|e| e.to_string())?;
        let mut tried = Vec::new();
        for id in chain {
            let endpoint = s
                .providers
                .configs
                .get(id)
                .and_then(|c| c.get("endpoint"))
                .and_then(|v| v.as_str())
                .unwrap_or("http://127.0.0.1:9002")
                .trim_end_matches('/')
                .to_string();
            // Any HTTP answer means a server is listening.
            if http.get(&endpoint).send().await.is_ok() {
                return Ok(());
            }
            tried.push(endpoint);
        }
        Err(crate::user_error::coded(
            "asr_unavailable",
            format!("音声認識サーバー未接続：{}", tried.join("、")),
        ))
    }

    pub fn start(&self, app: AppHandle) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        let (tx, rx) = mpsc::channel::<AudioFrame>(32);
        let stream = LoopbackStream::start(tx)?;
        if let Some(win) = app.get_webview_window("subtitle") {
            win.show().ok();
        }
        let stop = Arc::new(tokio::sync::Notify::new());
        let stop_in = stop.clone();
        let pipeline = self.pipeline.clone();
        let settings = self.settings.clone();
        let app_emit = app.clone();
        let handle = tauri::async_runtime::spawn(async move {
            let _ = app_emit.emit("moonlens://subtitle", SubtitleEvent::Started);
            let gave_up = run_loop(app_emit.clone(), settings, pipeline, rx, stop_in).await;
            if gave_up {
                // Close the window and release the audio stream too.
                let state = app_emit.state::<crate::app::AppState>();
                let _ = state.subtitle.stop(&app_emit);
            }
            let _ = app_emit.emit("moonlens://subtitle", SubtitleEvent::Stopped);
        });
        *self.inner.lock() = Some(Inner {
            _stream: stream,
            handle,
            stop,
        });
        Ok(())
    }

    pub fn stop(&self, app: &AppHandle) -> Result<()> {
        if let Some(inner) = self.inner.lock().take() {
            inner.stop.notify_waiters();
            inner.handle.abort();
            if let Some(win) = app.get_webview_window("subtitle") {
                win.hide().ok();
            }
            // Reset overlay context if any.
            overlay::hide(app).ok();
        }
        Ok(())
    }

    pub fn toggle(&self, app: &AppHandle) -> Result<bool> {
        if self.is_running() {
            self.stop(app)?;
            Ok(false)
        } else {
            self.start(app.clone())?;
            Ok(true)
        }
    }
}

async fn run_loop(
    app: AppHandle,
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
    mut rx: mpsc::Receiver<AudioFrame>,
    stop: Arc<tokio::sync::Notify>,
) -> bool {
    let mut failures = 0u32;
    let mut buffer: Vec<i16> = Vec::with_capacity(16_000 * 4);
    let mut overlap: Vec<i16> = Vec::new();
    loop {
        let snap = settings.snapshot();
        let chunk_samples = (snap.subtitle.chunk_seconds * 16_000.0) as usize;
        let overlap_samples = (snap.subtitle.overlap_seconds * 16_000.0) as usize;
        let target_lang = snap.subtitle.target_language.clone();
        let source_lang = snap.subtitle.source_language.clone();
        drop(snap);

        // Wait for either the next frame or a stop notification.
        tokio::select! {
            _ = stop.notified() => break,
            maybe = rx.recv() => match maybe {
                Some(frame) => buffer.extend_from_slice(&frame),
                None => break,
            },
        }
        if buffer.len() < chunk_samples {
            continue;
        }
        // Build a chunk that includes a small lookback for context.
        let mut to_send = overlap.clone();
        to_send.extend_from_slice(&buffer[..chunk_samples]);
        // Preserve the tail for the next iteration's overlap.
        if overlap_samples > 0 && chunk_samples >= overlap_samples {
            overlap = buffer[chunk_samples - overlap_samples..chunk_samples].to_vec();
        } else {
            overlap.clear();
        }
        buffer.drain(..chunk_samples);

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
                if original.trim().is_empty() {
                    continue;
                }
                let translated = match pipeline
                    .run(
                        Capability::Translate,
                        TaskInput::Translate {
                            text: original.clone(),
                            source_language: source_lang.clone(),
                            target_language: target_lang.clone(),
                        },
                    )
                    .await
                {
                    Ok(t) => match t.output {
                        TaskOutput::Text(s) => s,
                        _ => String::new(),
                    },
                    Err(_) => String::new(),
                };
                let clipboard_cfg = settings.snapshot().clipboard.clone();
                let copy_text = if !translated.is_empty() {
                    translated.as_str()
                } else {
                    original.as_str()
                };
                clipboard::auto_copy(&app, clipboard::Kind::Transcript, copy_text, &clipboard_cfg);
                let _ = app.emit(
                    "moonlens://subtitle",
                    SubtitleEvent::Chunk {
                        original,
                        translated,
                    },
                );
                // Avoid bursting work when chunks arrive too quickly.
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Err(e) => {
                failures += 1;
                let gave_up = failures >= MAX_CONSECUTIVE_FAILURES;
                let detail = crate::user_error::provider(&e, Capability::Asr);
                let detail = detail.split_once('|').map_or(detail.as_str(), |(_, m)| m);
                let message = if gave_up {
                    format!("音声認識の連続失敗により停止：{detail}")
                } else {
                    format!("音声認識失敗：{detail}")
                };
                let _ = app.emit("moonlens://subtitle", SubtitleEvent::Error { message });
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

fn pcm16_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}
