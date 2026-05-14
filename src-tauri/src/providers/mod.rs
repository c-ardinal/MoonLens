//! Provider abstraction.
//!
//! Each task (translate, ocr, asr, ...) is served by a chain of providers
//! resolved at runtime. Built-in providers live as submodules; plugin
//! providers are registered by [`crate::plugins`]. Both paths implement the
//! same trait so [`crate::pipeline`] does not care which is which.

pub mod asr_whisper;
pub mod ocr_ocrs;
#[cfg(feature = "paddleocr-engine")]
pub mod ocr_paddle;
#[cfg(target_os = "windows")]
pub mod ocr_windows;
pub mod qr_rqrr;
pub mod search_serpapi;
pub mod translate_deepl;
pub mod translate_embedded_llm;
pub mod translate_gemini;
pub mod translate_ollama;
pub mod translate_pivot;
pub mod translate_routed;

use std::sync::Arc;

/// Streaming token sink invoked once per delta. `done=true` marks the
/// final chunk; aggregated text is still returned from `run_streaming`
/// so non-streaming call sites continue to work unchanged.
pub type StreamingChunkSink = Arc<dyn Fn(String, bool) + Send + Sync>;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Logical capability a provider advertises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Translate,
    Ocr,
    Asr,
    Explain,
    Summarize,
    ReverseImageSearch,
    QrHandler,
    IntentClassifier,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("timeout after {0}ms")]
    Timeout(u32),
    #[error("offline")]
    Offline,
    #[error("rate limited")]
    RateLimit,
    #[error("unsupported language pair: {0} -> {1}")]
    UnsupportedLanguage(String, String),
    #[error("low confidence result")]
    LowConfidence,
    #[error("provider error: {0}")]
    Other(String),
}

/// Input/Output payload types are kept as enum variants so a chain can be
/// driven generically by the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskInput {
    Translate {
        text: String,
        source_language: String,
        target_language: String,
    },
    Ocr {
        image_png: Vec<u8>,
        languages: Vec<String>,
    },
    Asr {
        audio_pcm16_mono_16k: Vec<u8>,
        language_hint: Option<String>,
    },
    Explain {
        prompt: String,
        image_png: Option<Vec<u8>>,
    },
    Summarize {
        text: String,
    },
    ReverseImageSearch {
        image_url: String,
    },
    QrHandler {
        payload: String,
    },
    IntentClassifier {
        image_png: Vec<u8>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskOutput {
    Text(String),
    /// Streaming chunk; the pipeline forwards each to the UI via events.
    TextChunk {
        delta: String,
        done: bool,
    },
    Ocr {
        lines: Vec<OcrLine>,
    },
    Transcript {
        segments: Vec<TranscriptSegment>,
    },
    OpenedExternalUrl(String),
    Intent(Intent),
    Empty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrLine {
    pub text: String,
    pub bbox: [f32; 4],
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_s: f32,
    pub end_s: f32,
    pub text: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Qr,
    Text,
    Photo,
    Mixed,
    Unknown,
}

#[async_trait]
pub trait Provider: Send + Sync {
    /// Unique id used inside chains, e.g. `ollama.gemma4` or `plugin.<id>`.
    fn id(&self) -> &str;
    fn capabilities(&self) -> &[Capability];
    /// True for providers that do not reach the network. Used by the privacy
    /// indicator and `allow_cloud`/`allow_network` gates.
    fn is_local(&self) -> bool;
    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError>;

    /// Streaming variant. Default: invoke `run` and forward the final
    /// `Text` output as a single `done=true` chunk. Providers that
    /// natively stream (Ollama, the embedded LLM runtime) override
    /// this to emit per-token deltas as they arrive. The `sink` is
    /// owned by the caller — providers MUST NOT cache it across calls
    /// because Lens routes a per-line sink (with the line index baked
    /// in) per translation.
    async fn run_streaming(
        &self,
        input: TaskInput,
        sink: StreamingChunkSink,
    ) -> Result<TaskOutput, ProviderError> {
        let out = self.run(input).await?;
        if let TaskOutput::Text(text) = &out {
            sink(text.clone(), true);
        }
        Ok(out)
    }

    /// Optional warm-up hook. Providers that need heavy first-call
    /// initialisation (OCR model load, LLM context allocation) should
    /// populate their caches here so the user does not wait inline.
    /// Default: noop. Called at app startup from [`crate::pipeline::Pipeline::preload_all`].
    async fn preload(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

pub type ProviderHandle = Arc<dyn Provider>;
