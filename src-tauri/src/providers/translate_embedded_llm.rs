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
    /// Optional lifecycle hook for the slow GGUF load. Fired right
    /// before and right after `LlamaModel::load_from_file` so the UI
    /// can light up a "ローカル LLM をロード中…" badge during the
    /// multi-second cold-start (which is otherwise invisible —
    /// translation requests just appear to stall).
    load_notifier: Option<EmbeddedLoadNotifier>,
    /// Lazily-initialised llama.cpp model. The first translation
    /// call pays the model-load cost (a few hundred MB of GGUF +
    /// tokenizer init), every subsequent call reuses it. Wrapped in
    /// `tokio::sync::Mutex` so concurrent translates serialise on
    /// load but each request can spawn its own per-request
    /// `LlamaContext` (the context borrows from the model — we keep
    /// the model alive in `Arc` and create / drop contexts inline).
    #[cfg(feature = "embedded-llm")]
    engine: tokio::sync::Mutex<Option<std::sync::Arc<llama_cpp_2::model::LlamaModel>>>,
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
        Self {
            id: id.into(),
            model_path,
            system_prompt,
            max_predict_tokens,
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
fn llama_backend() -> Result<&'static llama_cpp_2::llama_backend::LlamaBackend, ProviderError> {
    use llama_cpp_2::llama_backend::LlamaBackend;
    use std::sync::OnceLock;
    static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();
    if let Some(b) = BACKEND.get() {
        return Ok(b);
    }
    let backend = LlamaBackend::init()
        .map_err(|e| ProviderError::Other(format!("llama backend init: {e}")))?;
    let _ = BACKEND.set(backend);
    BACKEND
        .get()
        .ok_or_else(|| ProviderError::Other("llama backend init race".into()))
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

/// Manually rendered Qwen 2.5 / ChatML prompt. The model itself
/// embeds a Jinja chat template, but we render the prompt by hand
/// so:
///   1. The exact shape is independent of any per-build template
///      bug in the GGUF metadata (we have hit one of those before
///      with split-file Qwen 7B).
///   2. We avoid pulling in Jinja parsing at runtime.
/// Format matches the upstream Qwen 2.5 release notes: `<|im_start|>`
/// / `<|im_end|>` markers, system + user, then `<|im_start|>assistant\n`
/// to prime the response.
#[cfg(feature = "embedded-llm")]
fn render_chatml(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n\
         <|im_start|>user\n{user}<|im_end|>\n\
         <|im_start|>assistant\n",
    )
}

#[cfg(feature = "embedded-llm")]
async fn ensure_engine(
    provider: &EmbeddedLlmTranslator,
) -> Result<std::sync::Arc<llama_cpp_2::model::LlamaModel>, ProviderError> {
    use llama_cpp_2::model::params::LlamaModelParams;
    use llama_cpp_2::model::LlamaModel;

    {
        let guard = provider.engine.lock().await;
        if let Some(eng) = guard.as_ref() {
            return Ok(eng.clone());
        }
    }

    let model_path = provider.model_path.clone();
    tracing::info!(
        "embedded LLM: loading GGUF via llama.cpp ({})",
        model_path.display()
    );

    if let Some(notify) = provider.load_notifier.as_ref() {
        notify(EmbeddedLoadEvent::Started {
            provider_id: provider.id.clone(),
            model_path: model_path.clone(),
        });
    }

    let backend = llama_backend()?;
    let load_path = model_path.clone();
    let load_result = tokio::task::spawn_blocking(move || -> Result<LlamaModel, String> {
        // CPU-only path. n_gpu_layers=0 is explicit because the
        // product spec is "fast on CPU" — GPU offload should be a
        // deliberate setting, not implicit here.
        let params = LlamaModelParams::default().with_n_gpu_layers(0);
        let params = std::pin::Pin::new(&params);
        LlamaModel::load_from_file(backend, &load_path, &params).map_err(|e| format!("{e}"))
    })
    .await
    .map_err(|e| ProviderError::Other(format!("embedded LLM load join: {e}")))?;

    match load_result {
        Ok(model) => {
            let arc = std::sync::Arc::new(model);
            {
                let mut guard = provider.engine.lock().await;
                if guard.is_none() {
                    *guard = Some(arc.clone());
                }
            }
            if let Some(notify) = provider.load_notifier.as_ref() {
                notify(EmbeddedLoadEvent::Finished {
                    provider_id: provider.id.clone(),
                    model_path: model_path.clone(),
                    success: true,
                    error: None,
                });
            }
            Ok(arc)
        }
        Err(msg) => {
            if let Some(notify) = provider.load_notifier.as_ref() {
                notify(EmbeddedLoadEvent::Finished {
                    provider_id: provider.id.clone(),
                    model_path: model_path.clone(),
                    success: false,
                    error: Some(msg.lines().next().unwrap_or(&msg).to_string()),
                });
            }
            Err(ProviderError::Other(format!("embedded LLM load: {msg}")))
        }
    }
}

/// Recommended thread count for the inference batch. `llama.cpp`
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

/// Core CPU-bound inference loop. Runs on a `spawn_blocking` worker
/// so the tokio executor stays free during the multi-second decode.
/// Creates a fresh `LlamaContext` per call — Phase 20.7 will replace
/// this with a worker thread that owns a long-lived context.
#[cfg(feature = "embedded-llm")]
fn run_blocking_inference(
    model: std::sync::Arc<llama_cpp_2::model::LlamaModel>,
    backend: &'static llama_cpp_2::llama_backend::LlamaBackend,
    prompt: String,
    max_predict: usize,
    on_token: Option<Box<dyn FnMut(String) + Send>>,
) -> Result<String, String> {
    use llama_cpp_2::context::params::LlamaContextParams;
    use llama_cpp_2::llama_batch::LlamaBatch;
    use llama_cpp_2::model::AddBos;
    use llama_cpp_2::sampling::LlamaSampler;
    use std::num::{NonZeroU16, NonZeroU32};

    let mut on_token = on_token;

    let prompt_tokens = model
        .str_to_token(&prompt, AddBos::Always)
        .map_err(|e| format!("tokenize: {e}"))?;
    let n_prompt = prompt_tokens.len();
    if n_prompt == 0 {
        return Err("tokenized prompt was empty".into());
    }

    // Size the context to comfortably hold prompt + generation. A
    // nominal 2048-token floor avoids oversized allocation for short
    // Lens lines while still fitting paragraph blocks. If the user
    // requests a larger num_predict than fits, we grow the window.
    let needed = n_prompt + max_predict + 16;
    let n_ctx = needed.max(2048).min(8192);
    let n_ctx_u32 = u32::try_from(n_ctx).map_err(|e| format!("n_ctx u32: {e}"))?;

    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(n_ctx_u32))
        .with_n_batch(n_ctx_u32.max(512))
        .with_n_threads(recommended_threads())
        .with_n_threads_batch(recommended_threads());

    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| format!("new_context: {e}"))?;

    let batch_capacity = n_prompt.max(512);
    let mut batch = LlamaBatch::new(batch_capacity, 1);
    batch
        .add_sequence(&prompt_tokens, 0, false)
        .map_err(|e| format!("batch add_sequence: {e}"))?;
    ctx.decode(&mut batch)
        .map_err(|e| format!("decode prompt: {e}"))?;

    // Greedy sampler is the right default for translation — we want
    // the model's most-likely rendering, not a creative variant.
    let mut sampler = LlamaSampler::chain_simple([LlamaSampler::greedy()]);

    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut aggregated = String::new();
    let mut n_cur = i32::try_from(n_prompt).map_err(|e| format!("n_cur i32: {e}"))?;
    let mut produced = 0usize;

    while produced < max_predict {
        let token = sampler.sample(&ctx, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.is_eog_token(token) {
            break;
        }
        let piece = model
            .token_to_piece(token, &mut decoder, false, NonZeroU16::new(1))
            .map_err(|e| format!("token_to_piece: {e}"))?;
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
    Ok(aggregated)
}

#[cfg(feature = "embedded-llm")]
async fn run_inference(
    provider: &EmbeddedLlmTranslator,
    input: TaskInput,
) -> Result<TaskOutput, ProviderError> {
    let model = ensure_engine(provider).await?;
    let user = build_user_prompt(&input)?;
    let prompt = render_chatml(&provider.system_prompt, &user);
    let max_predict = provider.max_predict_tokens.unwrap_or(512);
    let backend = llama_backend()?;
    let aggregated = tokio::task::spawn_blocking(move || {
        run_blocking_inference(model, backend, prompt, max_predict, None)
    })
    .await
    .map_err(|e| ProviderError::Other(format!("embedded LLM inference join: {e}")))?
    .map_err(|e| ProviderError::Other(format!("embedded LLM inference: {e}")))?;
    Ok(TaskOutput::Text(aggregated))
}

#[cfg(feature = "embedded-llm")]
async fn run_inference_streaming(
    provider: &EmbeddedLlmTranslator,
    input: TaskInput,
    sink: StreamingChunkSink,
) -> Result<TaskOutput, ProviderError> {
    let model = ensure_engine(provider).await?;
    let user = build_user_prompt(&input)?;
    let prompt = render_chatml(&provider.system_prompt, &user);
    let max_predict = provider.max_predict_tokens.unwrap_or(512);
    let backend = llama_backend()?;

    // Bridge the blocking inference loop's per-token callback into
    // the async sink. We fire `done=false` per delta and a final
    // `done=true` empty marker once the loop exits, mirroring the
    // semantics other streaming providers (Ollama / Gemini) use.
    let sink_arc = sink.clone();
    let aggregated = tokio::task::spawn_blocking(move || {
        run_blocking_inference(
            model,
            backend,
            prompt,
            max_predict,
            Some(Box::new(move |piece: String| {
                sink_arc(piece, false);
            })),
        )
    })
    .await
    .map_err(|e| ProviderError::Other(format!("embedded LLM stream join: {e}")))?
    .map_err(|e| ProviderError::Other(format!("embedded LLM stream: {e}")))?;
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
