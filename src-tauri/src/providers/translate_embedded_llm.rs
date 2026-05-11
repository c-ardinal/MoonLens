//! Embedded local LLM translator (no Ollama / external daemon required).
//!
//! The original v0.1 design shipped with `OllamaTranslator` as the
//! only "local" option, which forced every fresh install into a
//! "translation silently fails until you also install + run Ollama"
//! state. That contradicted the stated product spec
//! (`docs/plan.md` § "ローカル優先") because the user never opted
//! into a separate daemon — installing MoonLens should be enough.
//!
//! This provider closes that gap. It loads a quantised LLM GGUF
//! from `<app-data>/moonlens/models/<model>.gguf` and runs inference
//! in-process via the `llama-cpp-2` crate (FFI bindings to llama.cpp),
//! so the only runtime state outside the executable is the model
//! file itself. The model is *not* bundled in the binary — first-run
//! onboarding downloads it from HuggingFace into the app-data
//! directory and reports progress so the user is never surprised
//! by a multi-hundred-megabyte fetch.
//!
//! Runtime history (Phase 20, 2026-05-09): originally backed by
//! `mistralrs` (Pure Rust). The in-app benchmark on user hardware
//! consistently showed mistral.rs CPU paths under 30% of the
//! llama.cpp baseline (5–15 tok/s vs 50–100 tok/s expected for
//! 0.5B Q4_K_M), so the runtime was swapped to `llama-cpp-2`.
//! The Provider trait surface is unchanged — only the inference
//! impl below; everything else (download IPC, settings UI, chain
//! dispatch, migration logic) keeps working as-is.
//!
//! Naming history: the type was originally `EmbeddedLlmTranslator`
//! when Gemma 3 1B was the bundled default. With the 2026-05-09
//! switch to Qwen 2.5 (Apache 2.0) the Gemma-specific name became
//! misleading — the runtime supports any decoder-only GGUF
//! `mistralrs` accepts. The 12.x rename to `EmbeddedLlmTranslator` /
//! `embedded.llm` reflects that. A startup migration rewrites
//! existing user.toml entries (`embedded.llm` → `embedded.llm`)
//! so the rename is invisible to users.
//!
//! When the model is missing, `run` returns
//! `ProviderError::Other("model not downloaded")` so the chain
//! either falls back (Ollama / DeepL / Gemini if the user enabled
//! them) or surfaces the missing-model state in the settings UI
//! ("ローカル LLM を準備中…"). The pipeline's `not registered`
//! / "all providers failed" reporting handles that messaging
//! uniformly.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{Capability, Provider, ProviderError, StreamingChunkSink, TaskInput, TaskOutput};

/// Lifecycle event for the embedded LLM load. Fired around the
/// `mistralrs::Model::build` call so the UI can render a "ローカル
/// LLM をロード中…" badge instead of leaving the user staring at a
/// silent translation request.
#[derive(Debug, Clone)]
pub enum EmbeddedLoadEvent {
    Started {
        provider_id: String,
        model_path: PathBuf,
    },
    Finished {
        provider_id: String,
        model_path: PathBuf,
        success: bool,
        /// Error message when `success == false`. Trimmed to a
        /// single line so the toast does not blow up the layout.
        error: Option<String>,
    },
}

/// Pluggable notifier sink. The Tauri layer installs one that emits
/// `moonlens://embedded-model-loading` events; tests pass a closure
/// that records events into a `Vec`. Optional so headless / CI
/// builds work without one.
pub type EmbeddedLoadNotifier = Arc<dyn Fn(EmbeddedLoadEvent) + Send + Sync>;

/// Default GGUF primary filename. The "primary" caveat is for
/// multi-part tiers (e.g. Qwen 2.5 7B Q4_K_M ships as two LFS
/// objects) — those have additional parts the registry tracks via
/// `ModelTier::filenames`, but `DEFAULT_MODEL_FILE` only cares
/// about the first one because it is the surface used by status
/// labels, package manifests' legacy `model_filename` field, and
/// path resolution where a single file name is needed.
///
/// 2026-05-09: switched from `gemma-3-1b-it-q4_k_m.gguf` to the
/// Qwen 2.5 1.5B-Instruct Q4_K_M variant. The driver was a license
/// review for commercial deployments — Gemma's "Gemma Terms of Use"
/// permits commercial redistribution but obliges every redistributor
/// to ship the license terms and the Prohibited Use Policy.
/// Qwen 2.5 1.5B is **Apache 2.0**, which removes the redistribution
/// paperwork entirely. Architecture remains decoder-only Transformer
/// (compatible with `mistral.rs::GgufModelBuilder`); MADLAD-400, the
/// other Apache-2.0 candidate we evaluated, is T5 encoder-decoder
/// and would require a parallel runtime.
pub const DEFAULT_MODEL_FILE: &str = "qwen2.5-1.5b-instruct-q4_k_m.gguf";

/// HuggingFace repo + file the download IPC fetches. The
/// `Qwen/Qwen2.5-1.5B-Instruct-GGUF` repo carries the canonical
/// quants from the model authors (Apache 2.0). Settings can
/// override both fields per-install — power users who want a
/// different variant (Q3_K_M, Q4_0, IQ3_XS, ...) just point
/// `filename_remote` at the matching file in the same repo.
pub const DEFAULT_MODEL_REPO: &str = "Qwen/Qwen2.5-1.5B-Instruct-GGUF";
pub const DEFAULT_MODEL_FILENAME_REMOTE: &str = "qwen2.5-1.5b-instruct-q4_k_m.gguf";

/// Default extra parts, beyond `DEFAULT_MODEL_FILENAME_REMOTE`.
/// Empty for the bundled Qwen 2.5 1.5B (single-file). Configs that
/// pin a multi-part tier override this via the
/// `filenames_remote_extra` array in user.toml.
pub const DEFAULT_MODEL_FILENAMES_EXTRA: &[&str] = &[];

/// Human-readable license advert. Surfaced in the settings UI so
/// users can verify the license without leaving the app — important
/// for corporate deployments that need to greenlight third-party
/// licenses up front.
pub const DEFAULT_MODEL_LICENSE: &str = "Apache-2.0";
pub const DEFAULT_MODEL_LICENSE_URL: &str =
    "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/blob/main/LICENSE";

/// Resolve the on-disk path for the embedded model. Mirrors the
/// directory scheme used by `crate::settings::store::config_path`
/// (also under `<app-data>/moonlens/`) so a user opening the app-
/// data folder sees `config.toml`, `models/`, `plugins/`, ... as
/// peers.
pub fn default_model_dir(app_data: &Path) -> PathBuf {
    app_data.join("models")
}

pub fn default_model_path(app_data: &Path) -> PathBuf {
    default_model_dir(app_data).join(DEFAULT_MODEL_FILE)
}

/// In-process LLM runtime. The actual inference path is gated
/// behind the `embedded-llm` cargo feature so `cargo check` /
/// `cargo build --no-default-features` still compile in
/// CPU-constrained CI environments without pulling in the full
/// `llama-cpp-2` dep tree (which itself bundles llama.cpp's C++
/// build — a multi-minute `cargo build` cold-start cost). When the
/// feature is OFF, the provider compiles to a no-op that always
/// reports "feature disabled" — useful for keeping non-translate
/// tests fast.
pub struct EmbeddedLlmTranslator {
    id: String,
    model_path: PathBuf,
    /// System prompt prepended to every translate / explain /
    /// summarize call. Mirrors the Ollama provider's defaults so
    /// switching between the two does not change translation tone.
    system_prompt: String,
    /// Maximum tokens to generate per call. CRITICAL for production:
    /// without a cap llama.cpp will run the model until it hits the
    /// context limit (which for Qwen 2.5 is 32k tokens) — short
    /// translations would generate hundreds of unrelated tokens and
    /// blow the 30s/180s pipeline timeout. Sourced from the
    /// `num_predict` config key (inherited from default.toml's
    /// 19.x-era 2048 cap, which fits Lens's longest paragraph
    /// translations without going over).
    ///
    /// Pre-19.x bug: this field did not exist; the runtime used its
    /// internal default → sub-translations took 30+ seconds → Lens
    /// timed out every call. Adding the cap is what makes the
    /// embedded LLM path actually usable.
    max_predict_tokens: Option<usize>,
    /// Chat template family the active GGUF expects. ChatML for
    /// Qwen / SmolLM family, Gemma for Google's Gemma variants.
    /// Resolved from the registry tier at construction time —
    /// `EmbeddedLlmTranslator::new` looks up the (repo, filename)
    /// pair via `tier_by_coord` and picks up the template, falling
    /// back to ChatML when unknown.
    chat_template: crate::model_registry::ChatTemplate,
    /// Optional speculative-decoding drafter on disk. When present
    /// the worker loads BOTH this and the main model, then runs
    /// every inference through a spec-decode loop (drafter predicts
    /// N tokens, main model verifies in a single batch). When None
    /// the worker uses the normal autoregressive path.
    drafter_path: Option<PathBuf>,
    /// Number of tokens the drafter predicts per spec-decode round.
    /// Only meaningful when `drafter_path` is Some.
    drafter_n_draft: u32,
    /// Optional lifecycle hook for the slow GGUF load. Fired right
    /// before and right after `LlamaModel::load_from_file` so the UI
    /// can light up a "ローカル LLM をロード中…" badge during the
    /// multi-second cold-start (which is otherwise invisible —
    /// translation requests just appear to stall).
    load_notifier: Option<EmbeddedLoadNotifier>,
    /// Lazily-initialised llama.cpp engine worker. Phase 20.7:
    /// instead of holding `Arc<LlamaModel>` and creating a fresh
    /// `LlamaContext` per request (which paid ~700 ms per call for
    /// context allocation + KV cache + 302 MiB compute buffer
    /// reservation + prompt eval), we now own a dedicated worker
    /// thread per provider that:
    ///   1. Loads the GGUF once at startup.
    ///   2. Creates ONE `LlamaContext` and reuses it across requests.
    ///   3. Calls `clear_kv_cache` between requests to start fresh
    ///      without re-paying allocation costs.
    ///   4. Receives requests via an mpsc channel, sends results via
    ///      a oneshot per request.
    /// This lets us reuse the context (which is `!Send` because it
    /// owns a raw `NonNull<llama_context>`) without unsafe lifetime
    /// gymnastics — the context never leaves the worker thread.
    ///
    /// `tokio::sync::Mutex` is used over `parking_lot` because
    /// `ensure_engine` is async (we await on the worker's startup
    /// signal) and lock acquisition needs to suspend, not spin.
    #[cfg(feature = "embedded-llm")]
    engine: tokio::sync::Mutex<Option<std::sync::Arc<EngineWorkerHandle>>>,
}

impl EmbeddedLlmTranslator {
    pub fn new(id: impl Into<String>, config: &serde_json::Value) -> Self {
        let model_path = config
            .get("model_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                // Best-effort: fall back to a cwd-relative path so the
                // type can be constructed even without an AppHandle
                // (used by unit tests that exercise the registration
                // logic without a real app data dir). Production
                // callers go through `with_app_data_dir` below.
                PathBuf::from("models").join(DEFAULT_MODEL_FILE)
            });
        let system_prompt = config
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or(
                "You are a professional translator. Render the user's text \
                 naturally and idiomatically in the target language — match \
                 the meaning, tone, and register a native reader would expect, \
                 not the word order of the source. Preserve newlines exactly. \
                 Output ONLY the translated text. Never include commentary, \
                 explanations, romanizations, alternative translations, notes \
                 in parentheses, surrounding quotes, or bracketed markers like \
                 `[1]` / `[2]`.",
            )
            .to_string();
        // Read `num_predict` (legacy Ollama-compat key) from config
        // and apply it as the mistral.rs sampler max_len. Default
        // 512 is a translation-shaped cap — most Lens lines fit
        // comfortably under 200 tokens of output, paragraph blocks
        // cap around 400, so 512 leaves headroom while still
        // bounding any model that decides to ramble. Power users
        // can override per-tier via user.toml (the default.toml
        // entry sets 2048 which is also fine — it just allows
        // longer outputs at the cost of unbounded inference time).
        let max_predict_tokens = config
            .get("num_predict")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .or(Some(512));
        // Resolve chat template via the registry. Two layers:
        //   1. `chat_template` config override — power users can pin
        //      a template independent of the registered tier.
        //   2. `(repo, filename_remote)` lookup — the path most
        //      callers take. New tiers register their template once
        //      and the runtime picks it up automatically.
        // Fallback: ChatML, the most common open-weight format.
        let chat_template = config
            .get("chat_template")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "gemma" => Some(crate::model_registry::ChatTemplate::Gemma),
                "chatml" | "chat_ml" => Some(crate::model_registry::ChatTemplate::ChatMl),
                _ => None,
            })
            .or_else(|| {
                let repo = config.get("repo").and_then(|v| v.as_str())?;
                let filename = config.get("filename_remote").and_then(|v| v.as_str())?;
                crate::model_registry::tier_by_coord(repo, filename)
                    .map(|t| t.chat_template)
            })
            .unwrap_or(crate::model_registry::ChatTemplate::ChatMl);
        // Resolve speculative-decoding drafter, if any. Two layers
        // (same precedence as chat_template):
        //   1. Explicit `drafter_path` override in config — power
        //      users with a custom drafter point here directly.
        //   2. Registry lookup by (repo, filename_remote) — picks
        //      up the official drafter when the tier registers one.
        let (drafter_path, drafter_n_draft) = {
            let explicit = config
                .get("drafter_path")
                .and_then(|v| v.as_str())
                .map(PathBuf::from);
            let n_draft = config
                .get("drafter_n_draft")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32);
            if let Some(p) = explicit {
                (Some(p), n_draft.unwrap_or(5))
            } else {
                let from_registry = (|| {
                    let repo = config.get("repo").and_then(|v| v.as_str())?;
                    let filename = config.get("filename_remote").and_then(|v| v.as_str())?;
                    let tier = crate::model_registry::tier_by_coord(repo, filename)?;
                    tier.drafter
                })();
                match from_registry {
                    Some(spec) => {
                        // Drafter sits alongside the main GGUF in the
                        // same models/ dir; the downloader puts it
                        // there at the exact `filename`.
                        let p = model_path
                            .parent()
                            .map(|d| d.join(spec.filename))
                            .unwrap_or_else(|| PathBuf::from(spec.filename));
                        (Some(p), n_draft.unwrap_or(spec.n_draft))
                    }
                    None => (None, n_draft.unwrap_or(5)),
                }
            }
        };
        Self {
            id: id.into(),
            model_path,
            system_prompt,
            max_predict_tokens,
            chat_template,
            drafter_path,
            drafter_n_draft,
            load_notifier: None,
            #[cfg(feature = "embedded-llm")]
            engine: tokio::sync::Mutex::new(None),
        }
    }

    /// Override the model path post-construction. The pipeline calls
    /// this when it knows the Tauri app-data dir at provider rebuild
    /// time, so config-less installs still resolve the model from
    /// `<app-data>/moonlens/models/<file>.gguf`.
    pub fn with_app_data_dir(mut self, app_data: &Path) -> Self {
        if !self.model_path.is_absolute() {
            self.model_path = default_model_path(app_data);
        }
        self
    }

    /// Attach a load lifecycle notifier. Called once per provider
    /// instance from `Pipeline::rebuild_providers` so the same
    /// closure fires for every embedded provider in the chain.
    pub fn with_load_notifier(mut self, notifier: EmbeddedLoadNotifier) -> Self {
        self.load_notifier = Some(notifier);
        self
    }

    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Best-effort presence check. Consulted by the settings UI to
    /// decide whether to surface the "ダウンロード" CTA or the
    /// "準備済み" badge.
    pub fn is_model_present(&self) -> bool {
        self.model_path.is_file()
    }
}

#[async_trait]
impl Provider for EmbeddedLlmTranslator {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &[Capability] {
        &[
            Capability::Translate,
            Capability::Explain,
            Capability::Summarize,
        ]
    }

    fn is_local(&self) -> bool {
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        // Validate input early so the missing-model error path stays
        // single-purpose. The embedded LLM supports the LLM-shaped inputs
        // (Translate, Explain, Summarize); other capabilities should
        // never reach here because the pipeline routes by capability,
        // but be defensive.
        match &input {
            TaskInput::Translate { .. }
            | TaskInput::Explain { .. }
            | TaskInput::Summarize { .. } => {}
            _ => {
                return Err(ProviderError::Other(
                    "EmbeddedLlmTranslator cannot handle this input".into(),
                ));
            }
        }

        if !self.is_model_present() {
            return Err(ProviderError::Other(format!(
                "embedded LLM model not downloaded ({})",
                self.model_path.display()
            )));
        }

        #[cfg(feature = "embedded-llm")]
        {
            run_inference(self, input).await
        }
        #[cfg(not(feature = "embedded-llm"))]
        {
            // The dep tree for mistralrs adds a sizeable tail to the
            // first build, so non-feature builds short-circuit here
            // rather than load a runtime they cannot drive. Phase 2.2
            // gates the feature ON in `default` so a normal `cargo
            // build` produces the embedded path; CI / tests that opt
            // out via `--no-default-features` see this branch.
            let _ = input;
            Err(ProviderError::Other(
                "embedded LLM feature is disabled in this build. \
                 Rebuild with `npm run tauri:build` (default features \
                 include embedded-llm), or pass --features embedded-llm \
                 if you used --no-default-features."
                    .into(),
            ))
        }
    }

    async fn run_streaming(
        &self,
        input: TaskInput,
        sink: StreamingChunkSink,
    ) -> Result<TaskOutput, ProviderError> {
        if !self.is_model_present() {
            return Err(ProviderError::Other(format!(
                "embedded LLM model not downloaded ({})",
                self.model_path.display()
            )));
        }

        #[cfg(feature = "embedded-llm")]
        {
            run_inference_streaming(self, input, sink).await
        }
        #[cfg(not(feature = "embedded-llm"))]
        {
            let _ = (input, sink);
            Err(ProviderError::Other(
                "embedded LLM feature is disabled in this build. \
                 Rebuild with `npm run tauri:build` (default features \
                 include embedded-llm), or pass --features embedded-llm \
                 if you used --no-default-features."
                    .into(),
            ))
        }
    }

    /// Warm-up hook fired by `Pipeline::preload_all` at app startup,
    /// after every successful download, and after a tier switch
    /// where the new GGUF is already on disk. Triggers
    /// [`ensure_engine`] so the multi-second `mistralrs::GgufModelBuilder::build`
    /// runs in the background while the user is still on the
    /// dashboard / settings page — by the time they reach Lens or
    /// the capture flow, the engine is already loaded and the
    /// translation request is instant.
    ///
    /// Pre-11.x this hook was the trait's default `Ok(())` no-op,
    /// which meant the model lazy-loaded on the FIRST translate
    /// call. On a 7B Q4_K_M tier that loaded as several seconds of
    /// silent UI freeze the moment the user finally tried to use
    /// the tool — exactly the wrong moment.
    ///
    /// Errors here are intentionally swallowed at the trait
    /// boundary (`Pipeline::preload_all` `tracing::debug!`s
    /// failures and moves on) because the model can fail to
    /// preload for legitimate reasons — the file is not yet
    /// downloaded, the user has no GGUF on disk yet, RAM is
    /// constrained on the host. In those cases the run-time path
    /// re-tries the load on the first translate and surfaces a
    /// proper error to the user via the chain.
    async fn preload(&self) -> Result<(), ProviderError> {
        if !self.is_model_present() {
            // Nothing to warm up — the download IPC will trigger
            // preload again once the GGUF lands.
            return Ok(());
        }
        #[cfg(feature = "embedded-llm")]
        {
            ensure_engine(self).await.map(|_| ())
        }
        #[cfg(not(feature = "embedded-llm"))]
        {
            Ok(())
        }
    }
}

/// Process-wide llama.cpp backend. The crate's `LlamaBackend::init`
/// is allowed to run exactly once per process (it returns
/// `BackendAlreadyInitialized` on the second call), so we keep the
/// returned proof-of-init in a static. All `LlamaModel::load_from_file`
/// and `new_context` calls take a `&LlamaBackend`; using a static
/// reference is the canonical pattern from the upstream examples.
#[cfg(feature = "embedded-llm")]
fn llama_backend() -> Result<&'static llama_cpp_2::llama_backend::LlamaBackend, String> {
    use llama_cpp_2::llama_backend::LlamaBackend;
    use std::sync::OnceLock;
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }
    let backend = LlamaBackend::init().map_err(|e| format!("llama backend init: {e}"))?;
    let _ = BACKEND.set(backend);
    BACKEND
        .get()
        .ok_or_else(|| "llama backend init race".to_string())
}

/// Worker-side request. Sent from the async surface
/// ([`run_inference`] / [`run_inference_streaming`]) over an mpsc
/// channel; the worker processes them one at a time on its dedicated
/// OS thread. This is the only way to share a `LlamaContext` (which
/// is `!Send`) across many translate calls — the context never
/// leaves the worker thread.
#[cfg(feature = "embedded-llm")]
struct InferenceRequest {
    /// Pre-rendered ChatML prompt. Built on the calling task before
    /// the request is sent so prompt assembly can use the
    /// EmbeddedLlmTranslator's system_prompt field directly.
    prompt: String,
    /// Token cap for this request. Honors the per-call override
    /// (bench uses 64, production uses 512+).
    max_predict: usize,
    /// Optional per-token sink. `None` for non-streaming calls. The
    /// closure must be `Send` because it crosses the channel into
    /// the worker thread.
    on_token: Option<Box<dyn FnMut(String) + Send>>,
    /// Result channel. The worker sends the aggregated text (or an
    /// error message) back when inference completes.
    response_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
}

/// Caller-facing handle to the engine worker. Keeps the request
/// channel + the worker thread alive. Drops cleanly: when the last
/// `Arc<EngineWorkerHandle>` goes away, the request channel closes,
/// the worker loop exits, and the worker's owned `LlamaContext`
/// + `LlamaModel` are dropped on the worker thread.
#[cfg(feature = "embedded-llm")]
pub struct EngineWorkerHandle {
    request_tx: std::sync::mpsc::Sender<InferenceRequest>,
    /// Kept just to detach the worker thread on shutdown. We do not
    /// `join()` because the worker exits cleanly when the channel
    /// closes; the `JoinHandle` here is purely for the OS-level
    /// reaper. Wrapped in `Option` so `Drop` can `take()` it.
    _join: parking_lot::Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[cfg(feature = "embedded-llm")]
impl EngineWorkerHandle {
    /// Spawn a dedicated worker thread that loads the GGUF, creates
    /// a long-lived `LlamaContext`, and serves inference requests
    /// over an mpsc channel. Awaits the worker's "ready" signal so
    /// callers see the load failure immediately (rather than
    /// per-request).
    async fn spawn(
        provider_id: String,
        model_path: PathBuf,
        drafter_path: Option<PathBuf>,
        drafter_n_draft: u32,
        load_notifier: Option<EmbeddedLoadNotifier>,
    ) -> Result<Self, ProviderError> {
        use std::sync::mpsc;
        use tokio::sync::oneshot;

        let (req_tx, req_rx) = mpsc::channel::<InferenceRequest>();
        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();

        let join = std::thread::Builder::new()
            .name(format!("moonlens-llm-{}", provider_id))
            .spawn(move || {
                worker_main(
                    provider_id,
                    model_path,
                    drafter_path,
                    drafter_n_draft,
                    load_notifier,
                    ready_tx,
                    req_rx,
                );
            })
            .map_err(|e| ProviderError::Other(format!("worker spawn: {e}")))?;

        match ready_rx.await {
            Ok(Ok(())) => Ok(Self {
                request_tx: req_tx,
                _join: parking_lot::Mutex::new(Some(join)),
            }),
            Ok(Err(msg)) => Err(ProviderError::Other(format!("embedded LLM load: {msg}"))),
            Err(_) => Err(ProviderError::Other(
                "embedded LLM worker died before signalling ready".into(),
            )),
        }
    }

    /// Submit a request and await the worker's reply. Cheap; serializes
    /// per worker (which is fine — translation requests for the same
    /// provider already serialize through `Pipeline`'s chain).
    async fn submit(
        &self,
        prompt: String,
        max_predict: usize,
        on_token: Option<Box<dyn FnMut(String) + Send>>,
    ) -> Result<String, ProviderError> {
        use tokio::sync::oneshot;
        let (tx, rx) = oneshot::channel();
        let req = InferenceRequest {
            prompt,
            max_predict,
            on_token,
            response_tx: tx,
        };
        self.request_tx
            .send(req)
            .map_err(|_| ProviderError::Other("embedded LLM worker shut down".into()))?;
        match rx.await {
            Ok(Ok(text)) => Ok(text),
            Ok(Err(msg)) => Err(ProviderError::Other(format!("embedded LLM inference: {msg}"))),
            Err(_) => Err(ProviderError::Other(
                "embedded LLM worker dropped response channel".into(),
            )),
        }
    }
}

/// Worker thread main. Owns the model + context for the lifetime of
/// the channel; exits cleanly when the request channel is closed
/// (i.e. the last `EngineWorkerHandle` is dropped).
#[cfg(feature = "embedded-llm")]
fn worker_main(
    provider_id: String,
    model_path: PathBuf,
    drafter_path: Option<PathBuf>,
    drafter_n_draft: u32,
    load_notifier: Option<EmbeddedLoadNotifier>,
    ready_tx: tokio::sync::oneshot::Sender<Result<(), String>>,
    req_rx: std::sync::mpsc::Receiver<InferenceRequest>,
) {
    use llama_cpp_2::context::params::LlamaContextParams;
    use llama_cpp_2::model::params::LlamaModelParams;
    use llama_cpp_2::model::LlamaModel;
    use llama_cpp_2::sampling::LlamaSampler;
    use std::num::NonZeroU32;

    if let Some(n) = load_notifier.as_ref() {
        n(EmbeddedLoadEvent::Started {
            provider_id: provider_id.clone(),
            model_path: model_path.clone(),
        });
    }

    tracing::info!(
        "embedded LLM: loading GGUF via llama.cpp ({})",
        model_path.display()
    );

    let backend = match llama_backend() {
        Ok(b) => b,
        Err(e) => {
            let _ = ready_tx.send(Err(e.clone()));
            if let Some(n) = load_notifier.as_ref() {
                n(EmbeddedLoadEvent::Finished {
                    provider_id: provider_id.clone(),
                    model_path: model_path.clone(),
                    success: false,
                    error: Some(e),
                });
            }
            return;
        }
    };

    // CPU-only path. n_gpu_layers=0 is explicit because the product
    // spec is "fast on CPU" — GPU offload should be a deliberate
    // setting, not implicit.
    let model_params = LlamaModelParams::default().with_n_gpu_layers(0);
    let model_params = std::pin::Pin::new(&model_params);
    let model = match LlamaModel::load_from_file(backend, &model_path, &model_params) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("{e}");
            let _ = ready_tx.send(Err(msg.clone()));
            if let Some(n) = load_notifier.as_ref() {
                n(EmbeddedLoadEvent::Finished {
                    provider_id: provider_id.clone(),
                    model_path: model_path.clone(),
                    success: false,
                    error: Some(msg.lines().next().unwrap_or(&msg).to_string()),
                });
            }
            return;
        }
    };

    // Context size is fixed for the worker's lifetime. 4096 fits
    // every realistic Lens line + the bundled max_predict; longer
    // paragraphs that approach this cap still work because we
    // clear_kv_cache between requests (i.e. the old prompt does not
    // accumulate). If a future tier needs more, adjust here.
    let n_ctx = NonZeroU32::new(4096).expect("4096 is non-zero");
    let n_threads = recommended_threads();
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx))
        .with_n_batch(2048)
        .with_n_threads(n_threads)
        .with_n_threads_batch(n_threads);

    let mut ctx = match model.new_context(backend, ctx_params) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("new_context: {e}");
            let _ = ready_tx.send(Err(msg.clone()));
            if let Some(n) = load_notifier.as_ref() {
                n(EmbeddedLoadEvent::Finished {
                    provider_id: provider_id.clone(),
                    model_path: model_path.clone(),
                    success: false,
                    error: Some(msg),
                });
            }
            return;
        }
    };

    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);

    // Optional speculative-decoding drafter. Loaded once at worker
    // startup; spec_decode_inference uses (model, ctx) as the
    // target/verifier and (drafter_model, drafter_ctx) to predict
    // candidate tokens N at a time. If load fails we degrade to
    // pure autoregressive — spec decode is opt-in, not required.
    let drafter_pair: Option<(LlamaModel, llama_cpp_2::context::LlamaContext<'_>)> =
        drafter_path.as_ref().and_then(|p| {
            tracing::info!(
                target: "embedded_llm_perf",
                "embedded LLM: loading drafter GGUF for spec decode ({})",
                p.display()
            );
            let dp = LlamaModelParams::default().with_n_gpu_layers(0);
            let dp = std::pin::Pin::new(&dp);
            let drafter_model = match LlamaModel::load_from_file(backend, p, &dp) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(
                        target: "embedded_llm_perf",
                        "embedded LLM: drafter load failed, falling back to autoregressive: {e}"
                    );
                    return None;
                }
            };
            let dctx_params = LlamaContextParams::default()
                .with_n_ctx(Some(n_ctx))
                .with_n_batch(2048)
                .with_n_threads(n_threads)
                .with_n_threads_batch(n_threads);
            // SAFETY: LlamaContext borrows from LlamaModel via a
            // lifetime parameter, but in practice only uses the
            // model's raw C pointer (`NonNull<llama_model>`), which
            // is heap-allocated and stable across Rust struct
            // moves. The transmute extends the Rust-level borrow
            // to 'static so we can store both in a tuple; safety
            // relies on the worker thread keeping the tuple alive
            // for the full inference, which it does (the tuple is
            // a local on the worker thread, dropped only on
            // worker exit).
            let drafter_static: &'static LlamaModel = unsafe {
                std::mem::transmute::<&LlamaModel, &'static LlamaModel>(&drafter_model)
            };
            let drafter_ctx = match drafter_static.new_context(backend, dctx_params) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(
                        target: "embedded_llm_perf",
                        "embedded LLM: drafter context create failed, falling back: {e}"
                    );
                    return None;
                }
            };
            tracing::info!(
                target: "embedded_llm_perf",
                "embedded LLM: drafter ready (model + ctx loaded)"
            );
            Some((drafter_model, drafter_ctx))
        });
    let has_drafter = drafter_pair.is_some();

    if let Some(n) = load_notifier.as_ref() {
        n(EmbeddedLoadEvent::Finished {
            provider_id: provider_id.clone(),
            model_path: model_path.clone(),
            success: true,
            error: None,
        });
    }
    let _ = ready_tx.send(Ok(()));
    tracing::info!(
        target: "embedded_llm_perf",
        "embedded LLM worker ready: n_ctx=4096 n_threads={n_threads} \
         spec_decode={has_drafter} n_draft={drafter_n_draft}",
    );

    let mut drafter_pair = drafter_pair;
    // Request loop. `recv()` blocks until a request arrives or the
    // channel is closed (last handle dropped). When the channel
    // closes we exit, dropping ctx + model + sampler in that order.
    while let Ok(req) = req_rx.recv() {
        // Reset KV cache + sampler so the new request starts fresh
        // without re-paying the per-context-creation cost.
        ctx.clear_kv_cache();
        sampler.reset();
        let result = if let Some((drafter_model, drafter_ctx)) = drafter_pair.as_mut() {
            drafter_ctx.clear_kv_cache();
            run_inference_in_context_spec(
                &model,
                &mut ctx,
                &mut sampler,
                drafter_model,
                drafter_ctx,
                drafter_n_draft as usize,
                req.prompt,
                req.max_predict,
                req.on_token,
            )
        } else {
            run_inference_in_context(
                &model,
                &mut ctx,
                &mut sampler,
                req.prompt,
                req.max_predict,
                req.on_token,
            )
        };
        let _ = req.response_tx.send(result);
    }
    tracing::info!("embedded LLM worker exiting (channel closed)");
}

/// Per-request inference body. Runs inside the worker thread, given
/// a borrowed `LlamaContext` that survives across requests.
#[cfg(feature = "embedded-llm")]
fn run_inference_in_context(
    model: &llama_cpp_2::model::LlamaModel,
    ctx: &mut llama_cpp_2::context::LlamaContext<'_>,
    sampler: &mut llama_cpp_2::sampling::LlamaSampler,
    prompt: String,
    max_predict: usize,
    on_token: Option<Box<dyn FnMut(String) + Send>>,
) -> Result<String, String> {
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::AddBos;
    use std::num::NonZeroU16;

    let mut on_token = on_token;
    let prompt_tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|e| format!("tokenize: {e}"))?;
    let n_prompt = prompt_tokens.len();
    if n_prompt == 0 {
        return Err("tokenized prompt was empty".into());
    }

    let batch_capacity = n_prompt.max(512);
    let mut batch = LlamaBatch::new(batch_capacity, 1);
    batch
        .add_sequence(&prompt_tokens, 0, false)
        .map_err(|e| format!("batch add_sequence: {e}"))?;
    ctx.decode(&mut batch)
        .map_err(|e| format!("decode prompt: {e}"))?;

    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut aggregated = String::new();
    let mut n_cur = i32::try_from(n_prompt).map_err(|e| format!("n_cur i32: {e}"))?;
    let mut produced = 0usize;

    let infer_started = std::time::Instant::now();
    while produced < max_predict {
        let token = sampler.sample(ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.is_eog_token(token) {
            break;
        }
        // llama-cpp-2's token_to_piece enumerates a fixed set of
        // token types and returns `UnknownTokenType` for anything
        // outside that list. Newer models (Qwen 3.5, RakutenAI 2.0,
        // EuroLLM, ...) ship tokenisers that include type tags the
        // wrapper does not yet enumerate — but the actual token is
        // still valid in the model's KV cache. Treat the decoder
        // failure as "skip the surface form, keep generating" so
        // the model can recover instead of aborting mid-sentence.
        let piece = model
            .token_to_piece(token, &mut decoder, false, NonZeroU16::new(1))
            .unwrap_or_default();
        if !piece.is_empty() {
            aggregated.push_str(&piece);
            if let Some(cb) = on_token.as_mut() {
                cb(piece);
            }
        }

        batch.clear();
        batch
            .add(token, n_cur, &[0], true)
            .map_err(|e| format!("batch add: {e}"))?;
        ctx.decode(&mut batch)
            .map_err(|e| format!("decode step: {e}"))?;
        n_cur += 1;
        produced += 1;
    }
    let elapsed_ms = infer_started.elapsed().as_millis() as u64;
    let tps = if elapsed_ms > 0 {
        (produced as f64) / (elapsed_ms as f64) * 1000.0
    } else {
        0.0
    };
    tracing::info!(
        target: "embedded_llm_perf",
        "embedded LLM inference done (worker reuse): n_prompt={n_prompt} produced={produced} \
         elapsed_ms={elapsed_ms} tok_per_sec={tps:.1}",
    );
    Ok(aggregated)
}

/// Speculative-decoding inference. Runs the same translation request
/// as `run_inference_in_context` but uses a smaller "drafter" model
/// to predict candidate tokens that the main model verifies in one
/// batch — accepting N tokens in a single decode when the drafter
/// matches the main model's argmax. With a vocab-compatible drafter
/// (e.g. Google's gemma-4-*-it-assistant), greedy spec decode is
/// mathematically equivalent to plain greedy: the output sequence is
/// identical, only the wall-clock time shrinks.
///
/// Both contexts must use the same tokeniser. Both KV caches are
/// kept in sync — they always contain the same prefix of tokens.
/// The algorithm:
///   1. Prefill both contexts with `prompt`.
///   2. Sample first token from target (no drafting needed).
///   3. Feed the first token into BOTH contexts.
///   4. Loop:
///      a. Drafter generates `n_draft` candidate tokens
///         autoregressively (each step: decode 1 token → argmax).
///      b. Target batch-decodes the N candidates in ONE call,
///         producing per-position logits.
///      c. For each candidate position i, compare target's argmax
///         to the drafter's pick. Accept tokens until the first
///         mismatch.
///      d. On mismatch at position k: output the target's correction
///         (which may differ from drafter's pick), then rollback
///         both KV caches past position k+1 in the new tokens.
///      e. If all N accepted: emit one "free" bonus token from
///         target's last logits (it has been computed during the
///         verify batch and costs nothing extra).
#[cfg(feature = "embedded-llm")]
fn run_inference_in_context_spec(
    model: &llama_cpp_2::model::LlamaModel,
    ctx: &mut llama_cpp_2::context::LlamaContext<'_>,
    sampler: &mut llama_cpp_2::sampling::LlamaSampler,
    drafter_model: &llama_cpp_2::model::LlamaModel,
    drafter_ctx: &mut llama_cpp_2::context::LlamaContext<'_>,
    n_draft: usize,
    prompt: String,
    max_predict: usize,
    on_token: Option<Box<dyn FnMut(String) + Send>>,
) -> Result<String, String> {
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::AddBos;
    use llama_cpp_2::token::LlamaToken;
    use std::num::NonZeroU16;

    let mut on_token = on_token;
    let prompt_tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|e| format!("tokenize: {e}"))?;
    let n_prompt = prompt_tokens.len();
    if n_prompt == 0 {
        return Err("tokenized prompt was empty".into());
    }
    // Tokenise with drafter's tokeniser too — they SHOULD be
    // identical for a paired (main, drafter) but we don't enforce it
    // at type level; if the drafter's tokeniser drifts the spec
    // decode will produce mismatches on every token and effectively
    // degrade to autoregressive (just slower because we waste
    // drafter compute). The right pair (gemma-4-it + gemma-4-it-
    // assistant) shares vocab by design.
    let drafter_prompt_tokens = drafter_model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|e| format!("drafter tokenize: {e}"))?;
    if drafter_prompt_tokens.len() != n_prompt {
        return Err(format!(
            "drafter tokenizer mismatch: drafter produced {} tokens vs target {}",
            drafter_prompt_tokens.len(),
            n_prompt
        ));
    }

    // === Phase 1: Prefill both contexts ===
    let batch_capacity = n_prompt.max(512);
    let mut t_batch = LlamaBatch::new(batch_capacity, 1);
    t_batch
        .add_sequence(&prompt_tokens, 0, false)
        .map_err(|e| format!("target prefill add_sequence: {e}"))?;
    ctx.decode(&mut t_batch)
        .map_err(|e| format!("target prefill decode: {e}"))?;
    let mut d_batch = LlamaBatch::new(batch_capacity, 1);
    d_batch
        .add_sequence(&drafter_prompt_tokens, 0, false)
        .map_err(|e| format!("drafter prefill add_sequence: {e}"))?;
    drafter_ctx
        .decode(&mut d_batch)
        .map_err(|e| format!("drafter prefill decode: {e}"))?;

    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut aggregated = String::new();
    let mut n_target_pos = i32::try_from(n_prompt).map_err(|e| format!("n_pos i32: {e}"))?;
    let mut n_drafter_pos = n_target_pos;
    let mut produced = 0usize;

    // Helper: emit a token (decode, push, sink callback). Returns
    // true if we should stop (EOG or budget exhausted).
    let mut emit = |aggregated: &mut String,
                    produced: &mut usize,
                    decoder: &mut encoding_rs::Decoder,
                    on_token: &mut Option<Box<dyn FnMut(String) + Send>>,
                    tok: LlamaToken|
     -> Result<bool, String> {
        if model.is_eog_token(tok) {
            return Ok(true);
        }
        // Same wrapper-enum recovery as the autoregressive path:
        // emit empty when the token type is not in llama-cpp-2's
        // enum; the model state still includes the token.
        let piece = model
            .token_to_piece(tok, decoder, false, NonZeroU16::new(1))
            .unwrap_or_default();
        if !piece.is_empty() {
            aggregated.push_str(&piece);
            if let Some(cb) = on_token.as_mut() {
                cb(piece);
            }
        }
        *produced += 1;
        Ok(*produced >= max_predict)
    };

    // === Phase 2: Sample first token from target's prefill logits ===
    let first = sampler.sample(ctx, t_batch.n_tokens() - 1);
    sampler.accept(first);
    if emit(
        &mut aggregated,
        &mut produced,
        &mut decoder,
        &mut on_token,
        first,
    )? {
        return Ok(aggregated);
    }
    // Feed `first` into both contexts so their KV caches stay aligned.
    let mut b = LlamaBatch::new(1, 1);
    b.add(first, n_target_pos, &[0], true)
        .map_err(|e| format!("target advance add: {e}"))?;
    ctx.decode(&mut b)
        .map_err(|e| format!("target advance decode: {e}"))?;
    n_target_pos += 1;
    let mut b = LlamaBatch::new(1, 1);
    b.add(first, n_drafter_pos, &[0], true)
        .map_err(|e| format!("drafter advance add: {e}"))?;
    drafter_ctx
        .decode(&mut b)
        .map_err(|e| format!("drafter advance decode: {e}"))?;
    n_drafter_pos += 1;

    // === Phase 3: Spec decode loop ===
    let infer_started = std::time::Instant::now();
    let mut total_drafted = 0usize;
    let mut total_accepted = 0usize;
    while produced < max_predict {
        // Step 1: Drafter speculatively generates N candidates.
        // Each is its own greedy argmax over the drafter's logits.
        // We use a separate "drafter sampler" so the main sampler's
        // accept history stays clean (we re-feed accepted tokens
        // back into it later).
        let mut drafts: Vec<LlamaToken> = Vec::with_capacity(n_draft);
        let drafter_start = n_drafter_pos;
        let mut drafter_sampler = llama_cpp_2::sampling::LlamaSampler::chain_simple([
            llama_cpp_2::sampling::LlamaSampler::greedy(),
        ]);
        for _ in 0..n_draft {
            // Drafter's last decode left logits at position end of
            // its KV cache; sample from there.
            let tok = drafter_sampler.sample(drafter_ctx, -1);
            drafts.push(tok);
            let mut b = LlamaBatch::new(1, 1);
            b.add(tok, n_drafter_pos, &[0], true)
                .map_err(|e| format!("draft step add: {e}"))?;
            drafter_ctx
                .decode(&mut b)
                .map_err(|e| format!("draft step decode: {e}"))?;
            n_drafter_pos += 1;
        }
        total_drafted += drafts.len();

        // Step 2: BEFORE running verify batch, capture target's
        // prediction for the position drafts[0] occupies. The
        // current ctx state (after [prompt, first, accepted_so_far])
        // has logits at slot -1 that show target's next-token
        // distribution — i.e. what it would predict for drafts[0].
        // We must capture this NOW because the verify batch will
        // overwrite the logits buffer.
        let target_pred_for_drafts_0: LlamaToken = {
            let last = ctx.get_logits();
            argmax_logits(last)
        };

        // Step 3: Target batch-verifies ALL N drafts in one decode.
        // Each position carries logits=true so we can read per-
        // position predictions afterwards.
        let mut v_batch = LlamaBatch::new(n_draft.max(1), 1);
        for (i, &t) in drafts.iter().enumerate() {
            v_batch
                .add(t, n_target_pos + i as i32, &[0], true)
                .map_err(|e| format!("verify add: {e}"))?;
        }
        ctx.decode(&mut v_batch)
            .map_err(|e| format!("verify decode: {e}"))?;
        // After this decode, target's KV cache has the drafts
        // appended; we may need to rewind on mismatch. Position i
        // in the verify batch produced logits showing "next-token
        // prediction GIVEN drafts[0..=i] consumed" — i.e. what
        // target would emit after drafts[i].

        let mut n_accepted_this_round = 0usize;
        let mut mismatch_at: Option<usize> = None;
        let mut correction: Option<LlamaToken> = None;
        for i in 0..drafts.len() {
            // To verify drafts[i], we need target's prediction at
            // the position BEFORE consuming drafts[i]:
            //   - i == 0 → captured above as target_pred_for_drafts_0
            //   - i >= 1 → verify batch slot (i-1) = "logits after
            //              consuming drafts[i-1]" = next-token
            //              prediction for drafts[i].
            let target_pred = if i == 0 {
                target_pred_for_drafts_0
            } else {
                let arr = ctx.get_logits_ith((i - 1) as i32);
                argmax_logits(arr)
            };
            if drafts[i] == target_pred {
                // Accept drafts[i].
                if emit(
                    &mut aggregated,
                    &mut produced,
                    &mut decoder,
                    &mut on_token,
                    drafts[i],
                )? {
                    sampler.accept(drafts[i]);
                    n_accepted_this_round += 1;
                    n_target_pos += n_accepted_this_round as i32;
                    n_drafter_pos = drafter_start + n_accepted_this_round as i32;
                    total_accepted += n_accepted_this_round;
                    log_spec_perf(infer_started, produced, total_drafted, total_accepted, n_prompt);
                    return Ok(aggregated);
                }
                sampler.accept(drafts[i]);
                n_accepted_this_round += 1;
            } else {
                // Mismatch: emit target's correction instead.
                if emit(
                    &mut aggregated,
                    &mut produced,
                    &mut decoder,
                    &mut on_token,
                    target_pred,
                )? {
                    sampler.accept(target_pred);
                    mismatch_at = Some(i);
                    correction = Some(target_pred);
                    n_accepted_this_round += 1;
                    // Don't update positions yet — we'll roll back.
                    total_accepted += n_accepted_this_round;
                    log_spec_perf(infer_started, produced, total_drafted, total_accepted, n_prompt);
                    return Ok(aggregated);
                }
                sampler.accept(target_pred);
                mismatch_at = Some(i);
                correction = Some(target_pred);
                n_accepted_this_round += 1;
                break;
            }
        }
        total_accepted += n_accepted_this_round;

        // Step 4: Sync KV caches based on accept/reject outcome.
        if let Some(k) = mismatch_at {
            // Output ends with drafts[0..k] (accepted, k items)
            // plus target_pred (correction, 1 item). Target KV
            // has [prompt, first, drafts[0..N-1]] = n_target_pos +
            // N positions. We want to keep [prompt, first,
            // drafts[0..k-1], correction] = n_target_pos + k + 1
            // positions (1-indexed positions correspond to
            // n_target_pos + k for the correction slot).
            //
            // Remove target KV from `n_target_pos + k` onward.
            ctx.clear_kv_cache_seq(Some(0), Some((n_target_pos + k as i32) as u32), None)
                .map_err(|e| format!("target kv rollback: {e:?}"))?;
            let mut b = LlamaBatch::new(1, 1);
            b.add(correction.unwrap(), n_target_pos + k as i32, &[0], true)
                .map_err(|e| format!("correction add: {e}"))?;
            ctx.decode(&mut b)
                .map_err(|e| format!("correction decode: {e}"))?;
            n_target_pos = n_target_pos + k as i32 + 1;

            // Drafter KV similarly: keep [prompt, first,
            // drafts[0..k-1], correction] = drafter_start + k + 1.
            drafter_ctx
                .clear_kv_cache_seq(Some(0), Some((drafter_start + k as i32) as u32), None)
                .map_err(|e| format!("drafter kv rollback: {e:?}"))?;
            let mut b = LlamaBatch::new(1, 1);
            b.add(correction.unwrap(), drafter_start + k as i32, &[0], true)
                .map_err(|e| format!("drafter correction add: {e}"))?;
            drafter_ctx
                .decode(&mut b)
                .map_err(|e| format!("drafter correction decode: {e}"))?;
            n_drafter_pos = drafter_start + k as i32 + 1;
        } else {
            // All N drafts accepted. Target's verify batch left
            // logits at slot N-1 = "next-token prediction after
            // consuming drafts[N-1]". That's a "free bonus" token
            // we can emit without an extra decode pass on target.
            n_target_pos += drafts.len() as i32;
            let bonus_logits = ctx.get_logits_ith((drafts.len() - 1) as i32);
            let bonus = argmax_logits(bonus_logits);
            // Feed bonus into target so its KV cache keeps the
            // sequence we just emitted.
            let mut b = LlamaBatch::new(1, 1);
            b.add(bonus, n_target_pos, &[0], true)
                .map_err(|e| format!("bonus target add: {e}"))?;
            ctx.decode(&mut b)
                .map_err(|e| format!("bonus target decode: {e}"))?;
            n_target_pos += 1;
            // Feed bonus into drafter too so both KV caches stay
            // aligned for the next spec round.
            let mut b = LlamaBatch::new(1, 1);
            b.add(bonus, n_drafter_pos, &[0], true)
                .map_err(|e| format!("bonus drafter add: {e}"))?;
            drafter_ctx
                .decode(&mut b)
                .map_err(|e| format!("bonus drafter decode: {e}"))?;
            n_drafter_pos += 1;
            // Emit bonus AFTER updating positions so the early-
            // return path sees consistent state.
            if emit(
                &mut aggregated,
                &mut produced,
                &mut decoder,
                &mut on_token,
                bonus,
            )? {
                sampler.accept(bonus);
                log_spec_perf(infer_started, produced, total_drafted, total_accepted, n_prompt);
                return Ok(aggregated);
            }
            sampler.accept(bonus);
        }
    }
    log_spec_perf(infer_started, produced, total_drafted, total_accepted, n_prompt);
    Ok(aggregated)
}

#[cfg(feature = "embedded-llm")]
fn argmax_logits(logits: &[f32]) -> llama_cpp_2::token::LlamaToken {
    use llama_cpp_2::token::LlamaToken;
    let (idx, _) = logits
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((0, &0.0));
    LlamaToken(idx as i32)
}

#[cfg(feature = "embedded-llm")]
fn log_spec_perf(
    started: std::time::Instant,
    produced: usize,
    drafted: usize,
    accepted: usize,
    n_prompt: usize,
) {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let tps = if elapsed_ms > 0 {
        (produced as f64) / (elapsed_ms as f64) * 1000.0
    } else {
        0.0
    };
    let acc_pct = if drafted > 0 {
        100.0 * (accepted as f64) / (drafted as f64)
    } else {
        0.0
    };
    tracing::info!(
        target: "embedded_llm_perf",
        "embedded LLM spec-decode done: n_prompt={n_prompt} produced={produced} \
         elapsed_ms={elapsed_ms} tok_per_sec={tps:.1} \
         drafted={drafted} accepted={accepted} acceptance={acc_pct:.0}%",
    );
}

/// User-prompt builder shared by the single-shot and streaming
/// paths. Keeps the prompt format in one place so the two inference
/// flavours never drift.
#[cfg(feature = "embedded-llm")]
fn build_user_prompt(input: &TaskInput) -> Result<String, ProviderError> {
    match input {
        TaskInput::Translate {
            text,
            source_language,
            target_language,
        } => {
            let from_clause = if source_language.trim().is_empty()
                || source_language.eq_ignore_ascii_case("auto")
            {
                String::new()
            } else {
                format!(" from {source_language}")
            };
            Ok(format!(
                "Target language: {tgt}. Rewrite the following{from} naturally in {tgt}:\n\n{src}",
                tgt = target_language,
                from = from_clause,
                src = text,
            ))
        }
        TaskInput::Explain { prompt, .. } => Ok(prompt.clone()),
        TaskInput::Summarize { text } => Ok(format!("Summarize concisely:\n\n{text}")),
        _ => Err(ProviderError::Other(
            "EmbeddedLlmTranslator cannot handle this input".into(),
        )),
    }
}

/// Render the (system + user) → final prompt for the model's chat
/// template family. We render by hand instead of using the GGUF's
/// embedded Jinja so:
///   1. The exact shape is independent of any per-build template
///      bug in the GGUF metadata (we have hit one of those before
///      with split-file Qwen 7B).
///   2. We avoid pulling in Jinja parsing at runtime.
///   3. We can audit the prompt format from one place per family
///      without chasing model-specific quirks.
#[cfg(feature = "embedded-llm")]
fn render_prompt(
    template: crate::model_registry::ChatTemplate,
    system: &str,
    user: &str,
) -> String {
    use crate::model_registry::ChatTemplate;
    match template {
        ChatTemplate::ChatMl => render_chatml(system, user),
        ChatTemplate::ChatMlQwen3NoThink => render_chatml_qwen3_no_think(system, user),
        ChatTemplate::Gemma => render_gemma(system, user),
    }
}

/// ChatML — used by Qwen 2.5 / Qwen 3 / SmolLM 2 / many other
/// open-weight families. Format matches the upstream Qwen release
/// notes: `<|im_start|>` / `<|im_end|>` markers wrapping each turn,
/// `<|im_start|>assistant\n` to prime the response.
#[cfg(feature = "embedded-llm")]
fn render_chatml(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n\
         <|im_start|>user\n{user}<|im_end|>\n\
         <|im_start|>assistant\n",
    )
}

/// ChatML for Qwen 3 with reasoning mode disabled. Identical to
/// `render_chatml` except the assistant primer pre-emits empty
/// `<think></think>` tags. Qwen 3 ships with deliberative reasoning
/// on by default — without this primer the model spends the entire
/// 64-token bench budget thinking out loud and never reaches the
/// translation. See HF Qwen 3 release notes for the
/// `enable_thinking=False` flag (this is the manual-render
/// equivalent: pre-filling the think turn with empty content tells
/// the model "no thinking required, proceed to answer").
#[cfg(feature = "embedded-llm")]
fn render_chatml_qwen3_no_think(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n\
         <|im_start|>user\n{user}<|im_end|>\n\
         <|im_start|>assistant\n<think>\n\n</think>\n\n",
    )
}

/// Gemma — used by Gemma 2 / 3 / 4 (incl. nano e2b/e4b).
/// `<start_of_turn>role\ncontent<end_of_turn>` markers, no system
/// role on the wire. The Gemma chat template inlines the system
/// prompt into the first user turn (separated by a blank line) and
/// primes the response with `<start_of_turn>model\n`.
#[cfg(feature = "embedded-llm")]
fn render_gemma(system: &str, user: &str) -> String {
    let user_block = if system.trim().is_empty() {
        user.to_string()
    } else {
        format!("{system}\n\n{user}")
    };
    format!(
        "<start_of_turn>user\n{user_block}<end_of_turn>\n\
         <start_of_turn>model\n",
    )
}

/// Resolve (or lazily spawn) the per-translator worker thread that
/// holds the long-lived `LlamaContext`. Phase 20.7: replaces the
/// old `Arc<LlamaModel>` field with a worker handle. The first call
/// pays the model-load cost (sent to a background thread, so the
/// tokio executor stays free); every subsequent call returns the
/// cached handle in microseconds.
#[cfg(feature = "embedded-llm")]
async fn ensure_engine(
    provider: &EmbeddedLlmTranslator,
) -> Result<std::sync::Arc<EngineWorkerHandle>, ProviderError> {
    {
        let guard = provider.engine.lock().await;
        if let Some(eng) = guard.as_ref() {
            return Ok(eng.clone());
        }
    }

    // Resolve the drafter path: only pass it to the worker if the
    // file actually exists on disk. A registry-declared drafter that
    // the user hasn't downloaded yet would otherwise fail the worker
    // load and prevent normal inference. Spec decode is an opt-in
    // optimisation — gracefully degrade to autoregressive when the
    // drafter file is missing.
    let drafter_present = provider
        .drafter_path
        .as_ref()
        .map(|p| p.is_file())
        .unwrap_or(false);
    let drafter_path = if drafter_present {
        provider.drafter_path.clone()
    } else {
        None
    };
    let handle = EngineWorkerHandle::spawn(
        provider.id.clone(),
        provider.model_path.clone(),
        drafter_path,
        provider.drafter_n_draft,
        provider.load_notifier.clone(),
    )
    .await?;
    let arc = std::sync::Arc::new(handle);
    let mut guard = provider.engine.lock().await;
    // Race: another caller may have inserted a handle while we were
    // spawning. Keep theirs if so — both are equivalent; ours simply
    // gets dropped, which closes its channel and exits its worker.
    if let Some(eng) = guard.as_ref() {
        return Ok(eng.clone());
    }
    *guard = Some(arc.clone());
    Ok(arc)
}

/// Recommended thread count for the inference worker. `llama.cpp`
/// generally peaks around 4-8 threads on small models; hand it the
/// lesser of (logical cores, 8) to keep latency stable on larger
/// desktops without oversubscribing tiny laptops.
#[cfg(feature = "embedded-llm")]
fn recommended_threads() -> i32 {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let capped = n.min(8).max(1);
    i32::try_from(capped).unwrap_or(4)
}

#[cfg(feature = "embedded-llm")]
async fn run_inference(
    provider: &EmbeddedLlmTranslator,
    input: TaskInput,
) -> Result<TaskOutput, ProviderError> {
    let worker = ensure_engine(provider).await?;
    let user = build_user_prompt(&input)?;
    let prompt = render_prompt(provider.chat_template, &provider.system_prompt, &user);
    let max_predict = provider.max_predict_tokens.unwrap_or(512);
    let aggregated = worker.submit(prompt, max_predict, None).await?;
    Ok(TaskOutput::Text(aggregated))
}

#[cfg(feature = "embedded-llm")]
async fn run_inference_streaming(
    provider: &EmbeddedLlmTranslator,
    input: TaskInput,
    sink: StreamingChunkSink,
) -> Result<TaskOutput, ProviderError> {
    let worker = ensure_engine(provider).await?;
    let user = build_user_prompt(&input)?;
    let prompt = render_prompt(provider.chat_template, &provider.system_prompt, &user);
    let max_predict = provider.max_predict_tokens.unwrap_or(512);

    // Bridge the worker's per-token callback into the async sink.
    // We fire `done=false` per delta and a final `done=true` empty
    // marker once the worker returns, mirroring the semantics
    // other streaming providers (Ollama / Gemini) use.
    let sink_arc = sink.clone();
    let on_token: Box<dyn FnMut(String) + Send> = Box::new(move |piece: String| {
        sink_arc(piece, false);
    });
    let aggregated = worker.submit(prompt, max_predict, Some(on_token)).await?;
    sink(String::new(), true);
    Ok(TaskOutput::Text(aggregated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_relative_models_path_when_no_config() {
        let p = EmbeddedLlmTranslator::new("embedded.llm", &serde_json::Value::Null);
        // Without a config and without `with_app_data_dir`, the path
        // is relative — that's fine for unit tests but `is_model_present`
        // will return false unless someone runs from a directory with
        // `models/<file>.gguf`. The pipeline always calls
        // `with_app_data_dir` so production never sees this path.
        assert!(p.model_path().ends_with(DEFAULT_MODEL_FILE));
        assert!(!p.is_model_present());
    }

    #[test]
    fn explicit_model_path_overrides_default() {
        let cfg = serde_json::json!({"model_path": "C:\\models\\custom.gguf"});
        let p = EmbeddedLlmTranslator::new("embedded.llm", &cfg);
        assert_eq!(p.model_path().to_str(), Some("C:\\models\\custom.gguf"));
    }

    #[test]
    fn with_app_data_dir_resolves_relative_default_path() {
        let p = EmbeddedLlmTranslator::new("embedded.llm", &serde_json::Value::Null)
            .with_app_data_dir(Path::new("C:\\Users\\test\\AppData\\Roaming\\moonlens"));
        assert_eq!(
            p.model_path(),
            Path::new("C:\\Users\\test\\AppData\\Roaming\\moonlens\\models")
                .join(DEFAULT_MODEL_FILE)
        );
    }

    #[test]
    fn with_app_data_dir_keeps_absolute_user_path() {
        let cfg = serde_json::json!({"model_path": "C:\\override\\file.gguf"});
        let p = EmbeddedLlmTranslator::new("embedded.llm", &cfg)
            .with_app_data_dir(Path::new("C:\\Users\\test\\AppData\\Roaming\\moonlens"));
        assert_eq!(p.model_path().to_str(), Some("C:\\override\\file.gguf"));
    }

    #[tokio::test]
    async fn run_without_model_reports_missing_state() {
        let p = EmbeddedLlmTranslator::new("embedded.llm", &serde_json::Value::Null);
        let res = p
            .run(TaskInput::Translate {
                text: "hi".into(),
                source_language: "en".into(),
                target_language: "ja".into(),
            })
            .await;
        let err = res.expect_err("should fail without model file");
        assert!(
            err.to_string().contains("model not downloaded"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn run_streaming_without_model_reports_missing_state() {
        let p = EmbeddedLlmTranslator::new("embedded.llm", &serde_json::Value::Null);
        let sink: StreamingChunkSink = std::sync::Arc::new(|_, _| {});
        let res = p
            .run_streaming(
                TaskInput::Translate {
                    text: "hi".into(),
                    source_language: "en".into(),
                    target_language: "ja".into(),
                },
                sink,
            )
            .await;
        let err = res.expect_err("should fail without model file");
        assert!(
            err.to_string().contains("model not downloaded"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn preload_is_a_noop_when_model_file_is_missing() {
        // Critical regression guard for the 11.x preload work:
        // preload MUST short-circuit cleanly when there is no GGUF
        // on disk so app startup does not error / hang waiting on
        // a file that has not been downloaded yet. The download
        // path triggers preload again once bytes land.
        let p = EmbeddedLlmTranslator::new(
            "embedded.llm",
            &serde_json::json!({"model_path": "C:\\does-not-exist\\nope.gguf"}),
        );
        // Use the Provider trait method explicitly so this exercise
        // matches what `Pipeline::preload_all` calls in production.
        let res = <EmbeddedLlmTranslator as Provider>::preload(&p).await;
        assert!(
            res.is_ok(),
            "preload must swallow missing-model state, got: {res:?}"
        );
    }

    // ---- llama.cpp inference helpers (Phase 20) ----------------
    //
    // The `run_blocking_inference` path needs a real GGUF file to
    // exercise end-to-end (we cannot ship one in CI without an LFS
    // payload), but the prompt-building helpers are pure and
    // belong with the rest of the regression net so a future tweak
    // to the system prompt or ChatML template does not silently
    // change every translation.

    #[cfg(feature = "embedded-llm")]
    #[test]
    fn build_user_prompt_translate_includes_target_language_and_text() {
        let prompt = build_user_prompt(&TaskInput::Translate {
            text: "Hello".into(),
            source_language: "en".into(),
            target_language: "ja".into(),
        })
        .unwrap();
        assert!(prompt.contains("Target language: ja"), "got: {prompt}");
        assert!(prompt.contains("from en"), "got: {prompt}");
        assert!(prompt.ends_with("Hello"), "got: {prompt}");
    }

    #[cfg(feature = "embedded-llm")]
    #[test]
    fn build_user_prompt_translate_omits_from_clause_when_auto() {
        // Source-language=auto / empty must NOT inject "from auto"
        // into the rendered prompt — it would steer the model into
        // Latin-script output even when the source is, e.g., Japanese.
        for source in ["", "  ", "auto", "AUTO"] {
            let prompt = build_user_prompt(&TaskInput::Translate {
                text: "x".into(),
                source_language: source.into(),
                target_language: "ja".into(),
            })
            .unwrap();
            assert!(!prompt.contains(" from "), "source={source:?} prompt={prompt}");
        }
    }

    #[cfg(feature = "embedded-llm")]
    #[test]
    fn build_user_prompt_summarize_uses_concise_directive() {
        let prompt = build_user_prompt(&TaskInput::Summarize {
            text: "long text".into(),
        })
        .unwrap();
        assert!(prompt.starts_with("Summarize concisely"), "got: {prompt}");
        assert!(prompt.ends_with("long text"), "got: {prompt}");
    }

    #[cfg(feature = "embedded-llm")]
    #[test]
    fn render_gemma_inlines_system_into_first_user_turn() {
        // Gemma's chat template has no system role on the wire —
        // the system prompt MUST be merged into the first user turn,
        // separated by a blank line. Skipping this would leave the
        // model with no system instruction at all.
        let s = render_gemma("you are helpful", "translate this");
        assert!(
            s.starts_with("<start_of_turn>user\nyou are helpful\n\ntranslate this<end_of_turn>"),
            "system+user merge missing or malformed: {s}"
        );
        assert!(s.ends_with("<start_of_turn>model\n"), "got: {s}");
    }

    #[cfg(feature = "embedded-llm")]
    #[test]
    fn render_gemma_skips_blank_system_prefix() {
        // When the system prompt is empty/whitespace, the Gemma
        // user turn should NOT begin with two blank lines (the
        // model treats those as a literal part of the turn and
        // sometimes echoes them back).
        let s = render_gemma("", "x");
        assert!(s.starts_with("<start_of_turn>user\nx<end_of_turn>"), "got: {s}");
    }

    #[cfg(feature = "embedded-llm")]
    #[test]
    fn render_prompt_dispatches_by_template_family() {
        use crate::model_registry::ChatTemplate;
        let chatml = render_prompt(ChatTemplate::ChatMl, "sys", "hi");
        assert!(chatml.contains("<|im_start|>system\nsys<|im_end|>"));
        let gemma = render_prompt(ChatTemplate::Gemma, "sys", "hi");
        assert!(gemma.contains("<start_of_turn>user\nsys\n\nhi<end_of_turn>"));
        let qwen3 = render_prompt(ChatTemplate::ChatMlQwen3NoThink, "sys", "hi");
        assert!(qwen3.contains("<|im_start|>system\nsys<|im_end|>"));
        // The two formats must NOT bleed into each other.
        assert!(!chatml.contains("<start_of_turn>"), "chatml leaked Gemma markers: {chatml}");
        assert!(!gemma.contains("<|im_start|>"), "gemma leaked ChatML markers: {gemma}");
    }

    #[cfg(feature = "embedded-llm")]
    #[test]
    fn render_chatml_qwen3_no_think_pre_emits_empty_think_tags() {
        // Critical regression guard: Qwen 3 thinking mode is on by
        // default and burns the entire token budget on
        // `<think>...</think>` blocks. The no-think render works
        // by pre-filling those tags with empty content, telling the
        // model "thinking already done, proceed to answer". If a
        // future change drops this, every Qwen 3 tier silently
        // regresses to ~10% effective decode throughput.
        let s = render_chatml_qwen3_no_think("sys", "translate");
        assert!(s.contains("<|im_start|>assistant\n<think>\n\n</think>\n\n"),
            "no-think primer missing or malformed: {s}");
        // Sanity: empty think pair is between user end and answer.
        let user_end = s.find("<|im_start|>user\ntranslate<|im_end|>")
            .expect("user turn missing");
        let asst_start = s.find("<|im_start|>assistant").expect("asst start missing");
        assert!(user_end < asst_start, "user must come before assistant");
    }

    #[cfg(feature = "embedded-llm")]
    #[test]
    fn render_chatml_emits_qwen_25_compatible_markers() {
        // The runtime renders the ChatML prompt by hand so the
        // shape must NOT drift from what Qwen 2.5 expects. If a
        // future refactor swaps to a different chat template
        // mechanism, this test pins the contract.
        let s = render_chatml("you are helpful", "translate this");
        // System block.
        assert!(s.contains("<|im_start|>system\nyou are helpful<|im_end|>"));
        // User block.
        assert!(s.contains("<|im_start|>user\ntranslate this<|im_end|>"));
        // Assistant primer (no `<|im_end|>` — the model fills it in).
        assert!(s.ends_with("<|im_start|>assistant\n"));
    }

    #[tokio::test]
    async fn run_rejects_unsupported_input_kinds() {
        // Even with a "present" model path the type system rejects
        // OCR / Asr / QrHandler shapes — those inputs come from
        // separate capabilities entirely. Construct without a real
        // file but force the missing-model branch second by checking
        // the input branch first.
        let p = EmbeddedLlmTranslator::new("embedded.llm", &serde_json::Value::Null);
        let res = p
            .run(TaskInput::QrHandler {
                payload: "x".into(),
            })
            .await;
        // The early input-shape check fires before the file check,
        // so the message should mention the input rejection rather
        // than the missing model.
        let err = res.expect_err("should fail for unsupported input");
        assert!(
            err.to_string().contains("cannot handle this input"),
            "got: {err}"
        );
    }
}
