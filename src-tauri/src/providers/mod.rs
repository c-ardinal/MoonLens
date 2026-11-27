//! Provider abstraction.
//!
//! Each task (translate, ocr, asr, ...) is served by a chain of providers
//! resolved at runtime. Built-in providers live as submodules; plugin
//! providers are registered by [`crate::plugins`]. Both paths implement the
//! same trait so [`crate::pipeline`] does not care which is which.

pub mod asr_local;
pub mod asr_whisper;
pub mod glossary;
pub mod lang;
pub mod llm_models;
pub mod model_store;
pub mod ocr_ocrs;
pub mod ocr_onnx;
pub mod ocr_ppocr;
pub mod ocr_scripts;
#[cfg(windows)]
pub mod ocr_windows;
pub mod qr_rqrr;
pub mod search_serpapi;
pub mod translate_deepl;
pub mod translate_gemini;
pub mod translate_llama;
pub mod translate_ollama;
pub mod whisper_models;

use std::sync::Arc;

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
    /// The provider needs a model (or engine) that is not on disk yet.
    /// The payload names it for the user (e.g. "PLaMo 3 Translate 2B").
    /// A chain always moves on to its next provider after this.
    #[error("not installed: {0}")]
    NotInstalled(String),
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
        /// Text around `text` that helps the translator but must not be
        /// translated. Only providers whose
        /// [`Provider::takes_translation_context`] says so read it.
        #[serde(default, skip_serializing_if = "TranslateContext::is_empty")]
        context: TranslateContext,
    },
    Ocr {
        image_png: Vec<u8>,
        languages: Vec<String>,
    },
    Asr {
        audio_pcm16_mono_16k: Vec<u8>,
        language_hint: Option<String>,
        /// Without a hint: the languages the speech can be in (a
        /// face-to-face conversation has two). An engine that detects
        /// the language chooses among them; empty = any language.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        language_candidates: Vec<String>,
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

/// What a translator may know about the text around a request: the
/// earlier parts of the same capture with their translations, and other
/// text on screen. Short labels ("Save", "Light") are ambiguous on their
/// own; this lets them be read in context while still being translated
/// one part at a time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TranslateContext {
    /// Earlier `(source, translation)` pairs, in reading order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<(String, String)>,
    /// Surrounding text for reference only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// Key of the application the text comes from (see
    /// [`crate::app_identity`]), when it is known: selects that
    /// application's glossary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
}

impl TranslateContext {
    pub fn is_empty(&self) -> bool {
        self.history.is_empty() && self.reference.is_none() && self.app.is_none()
    }

    /// A context that only says which application the text comes from.
    pub fn of_app(app: Option<&str>) -> Self {
        Self {
            app: app.map(str::to_string),
            ..Default::default()
        }
    }
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
        /// The language the speech was decoded in, when the engine
        /// tells (its own detection, or the hint).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        language: Option<String>,
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
    /// Optional: load weights / open connections ahead of the first real
    /// request. Called in the background at startup and after settings
    /// reloads; the default does nothing.
    async fn warm_up(&self) -> Result<(), ProviderError> {
        Ok(())
    }
    /// For translators: `Some(true)` when the provider reads
    /// [`TranslateContext`], so a long capture can be sent in parts
    /// without losing context. `None` when it cannot take requests right
    /// now (e.g. its model is not installed) and the chain will fall
    /// through to the next provider.
    fn takes_translation_context(&self) -> Option<bool> {
        Some(false)
    }
}

pub type ProviderHandle = Arc<dyn Provider>;
