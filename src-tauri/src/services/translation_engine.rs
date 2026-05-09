//! Adapter over the LLM inference backend.
//!
//! Pre-7.x `EmbeddedLlmTranslator` held a concrete
//! `tokio::sync::Mutex<Option<Arc<mistralrs::Model>>>`. Tests that
//! wanted to assert "Lens hits the engine 4 times when 4 lines come
//! out of OCR" had to either ship a real GGUF in the test bundle
//! (gigabytes of CI artefacts) or skip the check. Neither is fine.
//!
//! [`TranslationEngine`] hides the backend behind a small async
//! contract: ensure the model is ready, run a request, optionally
//! stream tokens to a sink. Production wraps `mistralrs::Model`;
//! tests use [`ScriptedEngine`] which can be programmed to:
//!
//! * succeed with a canned reply (per-call dispatch is asserted),
//! * fail loading the first time and succeed on retry (recovery),
//! * stream a response chunk-by-chunk so the UI's per-token sink is
//!   exercised exactly the way Lens calls it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::providers::StreamingChunkSink;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("model not present: {0}")]
    ModelMissing(PathBuf),
    #[error("load failed: {0}")]
    LoadFailed(String),
    #[error("inference failed: {0}")]
    InferenceFailed(String),
    #[error("input not supported: {0}")]
    UnsupportedInput(String),
}

/// Task-agnostic translation request. The same shape covers
/// translate / explain / summarize because the engine just renders
/// system + user messages — the higher-level Provider impl is what
/// understands that translate has source/target language metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslateRequest {
    pub system_prompt: String,
    /// User-side content, already prompted for the specific task.
    /// e.g. for translate it is `"Target language: ja. Rewrite the
    /// following from en naturally in ja:\n\nHello world"`.
    pub user_prompt: String,
}

/// LLM inference backend contract. Implementations are expected to
/// be cheap to construct — the actual model load lives behind
/// [`ensure_loaded`](Self::ensure_loaded), which the service layer
/// invokes lazily on the first translate request.
#[async_trait]
pub trait TranslationEngine: Send + Sync {
    /// Lazy load. Idempotent — subsequent calls skip the heavy
    /// `mistralrs::Model::build` step when the engine is already
    /// loaded. Long-running on the first call; sub-microsecond
    /// thereafter.
    async fn ensure_loaded(&self) -> Result<(), EngineError>;

    /// Single-shot translation. Returns the aggregated text. Uses
    /// the engine's preferred decoding settings (greedy by default).
    async fn translate(&self, request: TranslateRequest) -> Result<String, EngineError>;

    /// Streaming translation. The sink receives per-token deltas;
    /// the final aggregated string is returned for callers that
    /// want both (Lens uses both — sink for UI streaming, return
    /// value for the history record).
    async fn translate_streaming(
        &self,
        request: TranslateRequest,
        sink: StreamingChunkSink,
    ) -> Result<String, EngineError>;

    /// Best-effort presence check. Tells the settings UI whether the
    /// model file is on disk without forcing a load.
    fn is_model_present(&self) -> bool;

    /// Absolute path the engine resolved to. Surfaced in load
    /// notifications so the UI can show "loading … (foo.gguf)".
    fn model_path(&self) -> &Path;

    /// True after [`ensure_loaded`](Self::ensure_loaded) returned Ok
    /// at least once. The pipeline's preserve-loaded-engine logic
    /// reads this to decide whether to keep an instance across
    /// rebuilds.
    fn is_loaded(&self) -> bool;
}

/// Legacy mistralrs engine. Wraps `mistralrs::Model` behind the
/// trait so the rest of the service layer (and the test suite) does
/// not see the concrete type. Phase 20 moved the production runtime
/// to llama.cpp via `LlamaCppEngine`; this implementation is kept
/// behind the dedicated `embedded-llm-mistralrs` feature so the
/// fallback / A/B comparison path still compiles, but it is NOT in
/// the default feature set.
#[cfg(feature = "embedded-llm-mistralrs")]
pub struct MistralRsEngine {
    model_path: PathBuf,
    /// Optional load-lifecycle notifier. When set, fired around
    /// `Model::build` so the layout shell can render a "loading…"
    /// badge.
    load_notifier: Option<crate::providers::translate_embedded_llm::EmbeddedLoadNotifier>,
    /// Provider id this engine is bound to (used in
    /// `EmbeddedLoadEvent` payloads).
    provider_id: String,
    engine: tokio::sync::Mutex<Option<Arc<mistralrs::Model>>>,
}

#[cfg(feature = "embedded-llm-mistralrs")]
impl MistralRsEngine {
    pub fn new(provider_id: impl Into<String>, model_path: PathBuf) -> Self {
        Self {
            provider_id: provider_id.into(),
            model_path,
            load_notifier: None,
            engine: tokio::sync::Mutex::new(None),
        }
    }

    pub fn with_load_notifier(
        mut self,
        notifier: crate::providers::translate_embedded_llm::EmbeddedLoadNotifier,
    ) -> Self {
        self.load_notifier = Some(notifier);
        self
    }

    async fn build_messages(
        &self,
        request: &TranslateRequest,
    ) -> Result<mistralrs::TextMessages, EngineError> {
        use mistralrs::{TextMessageRole, TextMessages};
        let msgs = TextMessages::new()
            .add_message(TextMessageRole::System, request.system_prompt.clone())
            .add_message(TextMessageRole::User, request.user_prompt.clone());
        Ok(msgs)
    }

    async fn load_inner(&self) -> Result<Arc<mistralrs::Model>, EngineError> {
        use crate::providers::translate_embedded_llm::EmbeddedLoadEvent;
        use mistralrs::GgufModelBuilder;

        let dir = self.model_path.parent().ok_or_else(|| {
            EngineError::LoadFailed(format!(
                "model path has no parent: {}",
                self.model_path.display()
            ))
        })?;
        let filename = self
            .model_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| {
                EngineError::LoadFailed(format!(
                    "model path has no filename: {}",
                    self.model_path.display()
                ))
            })?
            .to_string();

        if let Some(notify) = self.load_notifier.as_ref() {
            notify(EmbeddedLoadEvent::Started {
                provider_id: self.provider_id.clone(),
                model_path: self.model_path.clone(),
            });
        }

        let result = GgufModelBuilder::new(dir.to_string_lossy().to_string(), vec![filename])
            .with_logging()
            .build()
            .await;

        match result {
            Ok(model) => {
                let arc = Arc::new(model);
                if let Some(notify) = self.load_notifier.as_ref() {
                    notify(EmbeddedLoadEvent::Finished {
                        provider_id: self.provider_id.clone(),
                        model_path: self.model_path.clone(),
                        success: true,
                        error: None,
                    });
                }
                Ok(arc)
            }
            Err(e) => {
                let msg = format!("{e}");
                if let Some(notify) = self.load_notifier.as_ref() {
                    notify(EmbeddedLoadEvent::Finished {
                        provider_id: self.provider_id.clone(),
                        model_path: self.model_path.clone(),
                        success: false,
                        error: Some(msg.lines().next().unwrap_or(&msg).to_string()),
                    });
                }
                Err(EngineError::LoadFailed(msg))
            }
        }
    }
}

#[cfg(feature = "embedded-llm-mistralrs")]
#[async_trait]
impl TranslationEngine for MistralRsEngine {
    async fn ensure_loaded(&self) -> Result<(), EngineError> {
        if !self.is_model_present() {
            return Err(EngineError::ModelMissing(self.model_path.clone()));
        }
        {
            let guard = self.engine.lock().await;
            if guard.is_some() {
                return Ok(());
            }
        }
        let arc = self.load_inner().await?;
        let mut guard = self.engine.lock().await;
        if guard.is_none() {
            *guard = Some(arc);
        }
        Ok(())
    }

    async fn translate(&self, request: TranslateRequest) -> Result<String, EngineError> {
        self.ensure_loaded().await?;
        let model = {
            let guard = self.engine.lock().await;
            guard
                .as_ref()
                .ok_or_else(|| EngineError::LoadFailed("engine not loaded".into()))?
                .clone()
        };
        let messages = self.build_messages(&request).await?;
        let response = model
            .send_chat_request(messages)
            .await
            .map_err(|e| EngineError::InferenceFailed(format!("{e}")))?;
        let text = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();
        Ok(text)
    }

    async fn translate_streaming(
        &self,
        request: TranslateRequest,
        sink: StreamingChunkSink,
    ) -> Result<String, EngineError> {
        use mistralrs::{ChatCompletionChunkResponse, ChunkChoice, Delta, Response};

        self.ensure_loaded().await?;
        let model = {
            let guard = self.engine.lock().await;
            guard
                .as_ref()
                .ok_or_else(|| EngineError::LoadFailed("engine not loaded".into()))?
                .clone()
        };
        let messages = self.build_messages(&request).await?;
        let mut stream = model
            .stream_chat_request(messages)
            .await
            .map_err(|e| EngineError::InferenceFailed(format!("{e}")))?;
        let mut aggregated = String::new();
        while let Some(chunk) = stream.next().await {
            if let Response::Chunk(ChatCompletionChunkResponse { choices, .. }) = chunk {
                if let Some(ChunkChoice {
                    delta:
                        Delta {
                            content: Some(content),
                            ..
                        },
                    finish_reason,
                    ..
                }) = choices.into_iter().next()
                {
                    if !content.is_empty() {
                        aggregated.push_str(&content);
                        sink(content, finish_reason.is_some());
                    } else if finish_reason.is_some() {
                        sink(String::new(), true);
                    }
                }
            }
        }
        Ok(aggregated)
    }

    fn is_model_present(&self) -> bool {
        self.model_path.is_file()
    }

    fn model_path(&self) -> &Path {
        &self.model_path
    }

    fn is_loaded(&self) -> bool {
        self.engine.try_lock().map(|g| g.is_some()).unwrap_or(false)
    }
}

#[cfg(feature = "embedded-llm-mistralrs")]
use futures::StreamExt as _;

/// Test engine. Records every request, returns canned responses, and
/// streams them in slices so the per-chunk sink path is exercised.
#[cfg(test)]
pub use scripted::ScriptedEngine;

#[cfg(test)]
mod scripted {
    use super::*;
    use parking_lot::Mutex;

    /// Behaviour script. Each entry corresponds to one (translate or
    /// translate_streaming) call. When the script is exhausted the
    /// engine returns a fallback (configurable via `default_response`).
    #[derive(Debug, Clone)]
    pub struct ScriptedReply {
        /// Aggregated text the call returns.
        pub text: String,
        /// Slice into N chunks for `translate_streaming`. Sized so
        /// the test can assert chunk delivery cadence.
        pub chunk_count: usize,
        /// If Some, the call fails with this error instead of
        /// returning text.
        pub error: Option<String>,
    }

    impl ScriptedReply {
        pub fn ok(text: impl Into<String>) -> Self {
            Self {
                text: text.into(),
                chunk_count: 1,
                error: None,
            }
        }

        pub fn ok_chunks(text: impl Into<String>, chunk_count: usize) -> Self {
            Self {
                text: text.into(),
                chunk_count: chunk_count.max(1),
                error: None,
            }
        }

        pub fn err(message: impl Into<String>) -> Self {
            Self {
                text: String::new(),
                chunk_count: 1,
                error: Some(message.into()),
            }
        }
    }

    /// In-memory engine. Construct, queue replies, and observe
    /// every call by inspecting `requests()` after the test runs.
    pub struct ScriptedEngine {
        model_path: PathBuf,
        present: Mutex<bool>,
        loaded: Mutex<bool>,
        load_attempts: Mutex<u32>,
        fail_load_first_n: Mutex<u32>,
        replies: Mutex<Vec<ScriptedReply>>,
        default_reply: Mutex<ScriptedReply>,
        requests: Mutex<Vec<TranslateRequest>>,
    }

    impl ScriptedEngine {
        pub fn new(model_path: PathBuf) -> Self {
            Self {
                model_path,
                present: Mutex::new(true),
                loaded: Mutex::new(false),
                load_attempts: Mutex::new(0),
                fail_load_first_n: Mutex::new(0),
                replies: Mutex::new(Vec::new()),
                default_reply: Mutex::new(ScriptedReply::ok("[default scripted reply]")),
                requests: Mutex::new(Vec::new()),
            }
        }

        pub fn with_present(self, present: bool) -> Self {
            *self.present.lock() = present;
            self
        }

        pub fn with_replies(self, replies: Vec<ScriptedReply>) -> Self {
            *self.replies.lock() = replies;
            self
        }

        pub fn with_default_reply(self, reply: ScriptedReply) -> Self {
            *self.default_reply.lock() = reply;
            self
        }

        pub fn fail_load_for_first_n_attempts(self, n: u32) -> Self {
            *self.fail_load_first_n.lock() = n;
            self
        }

        pub fn requests(&self) -> Vec<TranslateRequest> {
            self.requests.lock().clone()
        }

        pub fn load_attempts(&self) -> u32 {
            *self.load_attempts.lock()
        }

        fn next_reply(&self) -> ScriptedReply {
            let mut q = self.replies.lock();
            if q.is_empty() {
                self.default_reply.lock().clone()
            } else {
                q.remove(0)
            }
        }
    }

    #[async_trait]
    impl TranslationEngine for ScriptedEngine {
        async fn ensure_loaded(&self) -> Result<(), EngineError> {
            if !*self.present.lock() {
                return Err(EngineError::ModelMissing(self.model_path.clone()));
            }
            *self.load_attempts.lock() += 1;
            let attempt = *self.load_attempts.lock();
            let fail_n = *self.fail_load_first_n.lock();
            if attempt <= fail_n {
                return Err(EngineError::LoadFailed(format!(
                    "scripted load failure on attempt {attempt}"
                )));
            }
            *self.loaded.lock() = true;
            Ok(())
        }

        async fn translate(&self, request: TranslateRequest) -> Result<String, EngineError> {
            self.ensure_loaded().await?;
            self.requests.lock().push(request);
            let reply = self.next_reply();
            if let Some(err) = reply.error {
                return Err(EngineError::InferenceFailed(err));
            }
            Ok(reply.text)
        }

        async fn translate_streaming(
            &self,
            request: TranslateRequest,
            sink: StreamingChunkSink,
        ) -> Result<String, EngineError> {
            self.ensure_loaded().await?;
            self.requests.lock().push(request);
            let reply = self.next_reply();
            if let Some(err) = reply.error {
                return Err(EngineError::InferenceFailed(err));
            }
            // Slice the canned text into ~chunk_count pieces so the
            // streaming sink fires the documented number of times.
            let chars: Vec<char> = reply.text.chars().collect();
            let chunks = if chars.is_empty() {
                vec![String::new()]
            } else {
                let n = reply.chunk_count.max(1);
                let chunk_size = chars.len().div_ceil(n);
                chars
                    .chunks(chunk_size)
                    .map(|c| c.iter().collect::<String>())
                    .collect()
            };
            let mut acc = String::new();
            let last = chunks.len();
            for (i, chunk) in chunks.into_iter().enumerate() {
                acc.push_str(&chunk);
                let done = i + 1 == last;
                sink(chunk, done);
            }
            Ok(acc)
        }

        fn is_model_present(&self) -> bool {
            *self.present.lock()
        }

        fn model_path(&self) -> &Path {
            &self.model_path
        }

        fn is_loaded(&self) -> bool {
            *self.loaded.lock()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::translation_engine::scripted::ScriptedReply;

    #[tokio::test]
    async fn scripted_engine_returns_canned_reply() {
        let eng = ScriptedEngine::new(PathBuf::from("/x/m.gguf"))
            .with_replies(vec![ScriptedReply::ok("こんにちは")]);
        let out = eng
            .translate(TranslateRequest {
                system_prompt: "sys".into(),
                user_prompt: "user".into(),
            })
            .await
            .unwrap();
        assert_eq!(out, "こんにちは");
        assert_eq!(eng.requests().len(), 1);
    }

    #[tokio::test]
    async fn scripted_engine_streams_in_expected_chunks() {
        let eng = ScriptedEngine::new(PathBuf::from("/x/m.gguf"))
            .with_replies(vec![ScriptedReply::ok_chunks("abcdef", 3)]);
        let chunks: Arc<parking_lot::Mutex<Vec<(String, bool)>>> =
            Arc::new(parking_lot::Mutex::new(Vec::new()));
        let chunks_for_sink = chunks.clone();
        let sink: StreamingChunkSink = Arc::new(move |delta, done| {
            chunks_for_sink.lock().push((delta, done));
        });
        let out = eng
            .translate_streaming(
                TranslateRequest {
                    system_prompt: "sys".into(),
                    user_prompt: "user".into(),
                },
                sink,
            )
            .await
            .unwrap();
        assert_eq!(out, "abcdef");
        let log = chunks.lock().clone();
        assert_eq!(log.len(), 3);
        assert_eq!(log.last().unwrap().1, true, "final chunk must mark done");
        let combined: String = log.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(combined, "abcdef");
    }

    #[tokio::test]
    async fn missing_model_yields_typed_error() {
        let eng = ScriptedEngine::new(PathBuf::from("/x/missing.gguf")).with_present(false);
        let err = eng
            .translate(TranslateRequest {
                system_prompt: "sys".into(),
                user_prompt: "user".into(),
            })
            .await
            .expect_err("must fail");
        match err {
            EngineError::ModelMissing(p) => assert_eq!(p, PathBuf::from("/x/missing.gguf")),
            _ => panic!("expected ModelMissing, got {err:?}"),
        }
    }

    #[tokio::test]
    async fn ensure_loaded_recovers_after_transient_failures() {
        // Production path: first load can OOM on weak HW, subsequent
        // attempts succeed once unrelated processes free memory. The
        // scripted engine simulates that.
        let eng = ScriptedEngine::new(PathBuf::from("/x/m.gguf"))
            .fail_load_for_first_n_attempts(2);
        // First two calls fail.
        for _ in 0..2 {
            let err = eng.ensure_loaded().await.expect_err("must fail");
            assert!(matches!(err, EngineError::LoadFailed(_)));
        }
        // Third succeeds.
        eng.ensure_loaded().await.unwrap();
        assert!(eng.is_loaded());
    }

    #[tokio::test]
    async fn scripted_inference_error_propagates() {
        let eng = ScriptedEngine::new(PathBuf::from("/x/m.gguf"))
            .with_replies(vec![ScriptedReply::err("inference panicked")]);
        let err = eng
            .translate(TranslateRequest {
                system_prompt: "sys".into(),
                user_prompt: "user".into(),
            })
            .await
            .expect_err("must fail");
        match err {
            EngineError::InferenceFailed(msg) => assert!(msg.contains("inference panicked")),
            _ => panic!("expected InferenceFailed, got {err:?}"),
        }
    }
}
