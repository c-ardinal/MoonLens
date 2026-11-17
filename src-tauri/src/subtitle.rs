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
        // Frames keep arriving while a chunk is being recognized; the
        // capture callback drops what does not fit, so leave room for a
        // long decode (callbacks deliver ~10 ms each).
        let (tx, rx) = mpsc::channel::<AudioFrame>(FRAME_QUEUE);
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
            // Give back the ~1 GB the speech model holds. Off this thread:
            // it waits for an aborted decode to wind down.
            std::thread::spawn(crate::providers::asr_local::release);
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
            let route = (code == "model_missing").then_some("/settings/#section-models");
            crate::notices::push(NOTICE, format!("ライブ字幕：{message}"), route);
            crate::app::route_main(&app, route.unwrap_or("/"));
        });
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
        // Collect whatever arrived during the previous recognition.
        while let Ok(frame) = rx.try_recv() {
            buffer.extend_from_slice(&frame);
        }
        if buffer.len() < chunk_samples {
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
        if overlap_samples > 0 && take >= overlap_samples {
            overlap = buffer[take - overlap_samples..take].to_vec();
        } else {
            overlap.clear();
        }
        buffer.drain(..take);
        // Silence (or near-silence) makes Whisper invent stock phrases
        // such as "Thanks for watching"; skip it.
        if is_silent(&to_send) {
            continue;
        }

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

/// Longest chunk sent to recognition: just under Whisper's 30 s window.
const MAX_WINDOW_SAMPLES: usize = 28 * 16_000;

/// Audio frames buffered between the capture thread and the loop
/// (~40 s at 10 ms per callback).
const FRAME_QUEUE: usize = 4096;

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
