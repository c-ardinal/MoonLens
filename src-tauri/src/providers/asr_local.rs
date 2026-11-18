//! Built-in speech recognition for live subtitles: whisper.cpp compiled
//! into the app (see `build.rs`), sharing ggml and its GPU backends with
//! the built-in llama.cpp. No server or external runtime is needed; the
//! model is downloaded from the settings screen
//! ([`super::whisper_models`]).

use std::path::{Path, PathBuf};
#[cfg(feature = "local-asr")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "local-asr")]
use std::sync::Arc;

use async_trait::async_trait;

use super::whisper_models::{self, AsrModel};
use super::{Capability, Provider, ProviderError, TaskInput, TaskOutput, TranscriptSegment};

/// Provider id used in `[providers.asr] chain`.
pub const LOCAL_ASR_ID: &str = "whisper";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecodeParams {
    /// 0 = pick from the core count.
    pub threads: u32,
    /// 1 = greedy. Larger values search more hypotheses: slightly more
    /// accurate, proportionally slower decoding.
    pub beam_size: u32,
    pub use_gpu: bool,
    /// Segments whose no-speech probability exceeds this are dropped
    /// (whisper.cpp also requires a low average log-probability).
    pub no_speech_thold: f32,
}

impl DecodeParams {
    pub fn from_config(cfg: &serde_json::Value, use_gpu: bool) -> Self {
        let u = |k: &str, d: u32| {
            cfg.get(k)
                .and_then(|v| v.as_u64())
                .map(|v| v.min(u32::MAX as u64) as u32)
                .unwrap_or(d)
        };
        Self {
            threads: u("threads", 0).min(64),
            beam_size: u("beam_size", 1).clamp(1, 8),
            use_gpu: use_gpu && cfg.get("use_gpu").and_then(|v| v.as_bool()) != Some(false),
            no_speech_thold: cfg
                .get("no_speech_thold")
                .and_then(|v| v.as_f64())
                .map(|v| v.clamp(0.0, 1.0) as f32)
                .unwrap_or(0.6),
        }
    }
}

pub struct LocalWhisperProvider {
    id: String,
    model: &'static AsrModel,
    path: PathBuf,
    params: DecodeParams,
}

impl LocalWhisperProvider {
    pub fn new(id: impl Into<String>, cfg: &serde_json::Value, use_gpu: bool) -> Self {
        let model = whisper_models::selected(cfg);
        let dir = whisper_models::default_models_dir().unwrap_or_default();
        Self {
            id: id.into(),
            model,
            path: whisper_models::model_path(&dir, model),
            params: DecodeParams::from_config(cfg, use_gpu),
        }
    }

    pub fn model(&self) -> &'static AsrModel {
        self.model
    }

    pub fn model_path(&self) -> &Path {
        &self.path
    }

    pub fn is_ready(&self) -> bool {
        std::fs::metadata(&self.path)
            .map(|m| m.len() == self.model.size)
            .unwrap_or(false)
    }
}

/// Little-endian PCM16 bytes to the f32 samples whisper.cpp expects.
pub fn pcm16_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect()
}

#[async_trait]
impl Provider for LocalWhisperProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Asr]
    }
    fn is_local(&self) -> bool {
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Asr {
            audio_pcm16_mono_16k,
            language_hint,
        } = input
        else {
            return Err(ProviderError::Other("expected Asr input".into()));
        };
        if !self.is_ready() {
            return Err(ProviderError::NotInstalled(self.model.label.to_string()));
        }
        if !super::translate_llama::cpu_supported() {
            return Err(ProviderError::Other(
                "this CPU lacks AVX2/FMA, which the built-in engine needs".into(),
            ));
        }
        #[cfg(feature = "local-asr")]
        {
            let cancel = Arc::new(AtomicBool::new(false));
            // The subtitle loop is aborted (and pipeline timeouts drop
            // this future) without waiting for whisper.cpp; stop the
            // blocking decode too so the model is free for the next call.
            struct CancelOnDrop(Arc<AtomicBool>);
            impl Drop for CancelOnDrop {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::SeqCst);
                }
            }
            let _guard = CancelOnDrop(cancel.clone());
            let path = self.path.clone();
            let params = self.params;
            let samples = pcm16_bytes_to_f32(&audio_pcm16_mono_16k);
            let segments = tokio::task::spawn_blocking(move || {
                engine::transcribe(&path, params, &samples, language_hint.as_deref(), &cancel)
            })
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))??;
            Ok(TaskOutput::Transcript { segments })
        }
        #[cfg(not(feature = "local-asr"))]
        {
            let _ = (audio_pcm16_mono_16k, language_hint);
            Err(ProviderError::Other(
                "this build has no built-in speech recognition".into(),
            ))
        }
    }

    async fn warm_up(&self) -> Result<(), ProviderError> {
        // Loading takes a moment (and ~1 GB of RAM for the default
        // model); do it when subtitles start, not at app launch.
        Ok(())
    }
}

/// Load the model in the background, e.g. right before the subtitle
/// window opens, so the first chunk does not wait for it.
pub fn preload(path: PathBuf, params: DecodeParams) {
    #[cfg(feature = "local-asr")]
    std::thread::spawn(move || {
        if let Err(e) = engine::load(&path, params.use_gpu) {
            tracing::warn!("whisper preload failed: {e}");
        }
    });
    #[cfg(not(feature = "local-asr"))]
    let _ = (path, params);
}

/// Free the loaded model if it is `path` (before deleting the file).
pub fn unload(path: &Path) {
    #[cfg(feature = "local-asr")]
    engine::unload(Some(path));
    #[cfg(not(feature = "local-asr"))]
    let _ = path;
}

/// Free whatever model is loaded (subtitles stopped). The default model
/// holds ~1 GB, which should not stay resident while unused.
pub fn release() {
    #[cfg(feature = "local-asr")]
    engine::unload(None);
}

#[cfg(feature = "local-asr")]
mod engine {
    use std::ffi::{c_char, c_int, c_void, CStr, CString};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;
    use std::sync::OnceLock;

    use parking_lot::Mutex;

    use super::{DecodeParams, ProviderError, TranscriptSegment};

    // Keep llama-cpp-sys-2 (and with it the ggml that whisper.cpp was
    // compiled against) on the link line.
    use llama_cpp_sys_2 as _;

    #[repr(C)]
    struct WhisperContext {
        _private: [u8; 0],
    }

    extern "C" {
        fn mlw_init(path: *const c_char, use_gpu: bool, flash_attn: bool) -> *mut WhisperContext;
        #[allow(clippy::too_many_arguments)]
        fn mlw_full(
            ctx: *mut WhisperContext,
            samples: *const f32,
            n_samples: c_int,
            language: *const c_char,
            n_threads: c_int,
            beam_size: c_int,
            audio_ctx: c_int,
            prompt: *const c_char,
            no_speech_thold: f32,
            abort_flag: *mut c_void,
        ) -> c_int;
        fn whisper_free(ctx: *mut WhisperContext);
        fn whisper_full_n_segments(ctx: *mut WhisperContext) -> c_int;
        fn whisper_full_get_segment_text(ctx: *mut WhisperContext, i: c_int) -> *const c_char;
        fn whisper_full_get_segment_t0(ctx: *mut WhisperContext, i: c_int) -> i64;
        fn whisper_full_get_segment_t1(ctx: *mut WhisperContext, i: c_int) -> i64;
        fn whisper_full_lang_id(ctx: *mut WhisperContext) -> c_int;
        fn whisper_lang_str(id: c_int) -> *const c_char;
    }

    struct Loaded {
        path: PathBuf,
        use_gpu: bool,
        ctx: *mut WhisperContext,
    }
    // The context is only touched while holding `ENGINE`'s lock.
    unsafe impl Send for Loaded {}

    impl Drop for Loaded {
        fn drop(&mut self) {
            unsafe { whisper_free(self.ctx) };
        }
    }

    /// One model at a time; whisper.cpp contexts are not re-entrant, so
    /// the lock also serializes decoding.
    fn engine() -> &'static Mutex<Option<Loaded>> {
        static ENGINE: OnceLock<Mutex<Option<Loaded>>> = OnceLock::new();
        ENGINE.get_or_init(|| Mutex::new(None))
    }

    /// Language of the last auto-detected chunk and how many chunks have
    /// reused it. Detection runs the encoder a second time, so with
    /// "auto" it is repeated only every few chunks.
    fn sticky_language() -> &'static Mutex<(Option<String>, u32)> {
        static LANG: OnceLock<Mutex<(Option<String>, u32)>> = OnceLock::new();
        LANG.get_or_init(|| Mutex::new((None, 0)))
    }
    const REDETECT_EVERY: u32 = 6;

    fn open(path: &Path, use_gpu: bool) -> Result<Loaded, ProviderError> {
        let c = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|e| ProviderError::Other(e.to_string()))?;
        // Flash attention is a clear win on GPUs and roughly neutral on
        // CPUs.
        let mut ctx = unsafe { mlw_init(c.as_ptr(), use_gpu, use_gpu) };
        let mut gpu = use_gpu;
        if ctx.is_null() && use_gpu {
            tracing::warn!("whisper: GPU init failed, retrying on the CPU");
            ctx = unsafe { mlw_init(c.as_ptr(), false, false) };
            gpu = false;
        }
        if ctx.is_null() {
            return Err(ProviderError::Other(format!(
                "failed to load speech model {}",
                path.display()
            )));
        }
        tracing::info!("whisper: loaded {} (gpu: {gpu})", path.display());
        Ok(Loaded {
            path: path.to_path_buf(),
            use_gpu,
            ctx,
        })
    }

    fn ensure<'a>(
        slot: &'a mut Option<Loaded>,
        path: &Path,
        use_gpu: bool,
    ) -> Result<&'a mut Loaded, ProviderError> {
        let stale = !matches!(slot, Some(l) if l.path == path && l.use_gpu == use_gpu);
        if stale {
            // Free the old model before loading the new one.
            *slot = None;
            *slot = Some(open(path, use_gpu)?);
        }
        Ok(slot.as_mut().expect("just loaded"))
    }

    pub fn load(path: &Path, use_gpu: bool) -> Result<(), ProviderError> {
        let mut slot = engine().lock();
        ensure(&mut slot, path, use_gpu).map(|_| ())
    }

    pub fn unload(path: Option<&Path>) {
        let mut slot = engine().lock();
        let matches = match path {
            None => true,
            Some(p) => matches!(&*slot, Some(l) if l.path == p),
        };
        if matches {
            *slot = None;
        }
        *sticky_language().lock() = (None, 0);
    }

    fn thread_count(requested: u32) -> i32 {
        if requested > 0 {
            return requested as i32;
        }
        let logical = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        // The encoder is compute-bound; SMT siblings add little.
        let n = if logical >= 8 { logical / 2 } else { logical };
        n.clamp(1, 16) as i32
    }

    pub fn transcribe(
        path: &Path,
        params: DecodeParams,
        samples: &[f32],
        language: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Vec<TranscriptSegment>, ProviderError> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }
        let mut slot = engine().lock();
        let loaded = ensure(&mut slot, path, params.use_gpu)?;

        let auto = language.is_none();
        let lang = match language {
            Some(l) => Some(l.to_string()),
            None => {
                let mut s = sticky_language().lock();
                s.1 += 1;
                if s.1 >= REDETECT_EVERY {
                    s.0 = None;
                }
                s.0.clone()
            }
        };
        let lang_c = lang
            .as_deref()
            .map(|l| CString::new(l).map_err(|e| ProviderError::Other(e.to_string())))
            .transpose()?;

        let rc = unsafe {
            mlw_full(
                loaded.ctx,
                samples.as_ptr(),
                samples.len() as c_int,
                lang_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
                thread_count(params.threads),
                params.beam_size as c_int,
                0,
                std::ptr::null(),
                params.no_speech_thold,
                cancel as *const AtomicBool as *mut c_void,
            )
        };
        if cancel.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(ProviderError::Other("cancelled".into()));
        }
        if rc != 0 {
            return Err(ProviderError::Other(format!("whisper.cpp failed ({rc})")));
        }
        if auto && lang.is_none() {
            let id = unsafe { whisper_full_lang_id(loaded.ctx) };
            let p = unsafe { whisper_lang_str(id) };
            if !p.is_null() {
                let detected = unsafe { CStr::from_ptr(p) }.to_string_lossy().to_string();
                *sticky_language().lock() = (Some(detected), 0);
            }
        }

        let n = unsafe { whisper_full_n_segments(loaded.ctx) };
        let mut out = Vec::with_capacity(n.max(0) as usize);
        for i in 0..n {
            let text = unsafe { whisper_full_get_segment_text(loaded.ctx, i) };
            if text.is_null() {
                continue;
            }
            // Multi-byte characters can be split across tokens at a
            // segment edge; lossy keeps the rest of the text.
            let text = unsafe { CStr::from_ptr(text) }
                .to_string_lossy()
                .trim()
                .to_string();
            if text.is_empty() {
                continue;
            }
            // Timestamps are in units of 10 ms.
            let (t0, t1) = unsafe {
                (
                    whisper_full_get_segment_t0(loaded.ctx, i),
                    whisper_full_get_segment_t1(loaded.ctx, i),
                )
            };
            out.push(TranscriptSegment {
                start_s: t0 as f32 / 100.0,
                end_s: t1 as f32 / 100.0,
                text,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm16_conversion() {
        let bytes: Vec<u8> = [0i16, 16384, -32768, 32767]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let f = pcm16_bytes_to_f32(&bytes);
        assert_eq!(f.len(), 4);
        assert_eq!(f[0], 0.0);
        assert!((f[1] - 0.5).abs() < 1e-6);
        assert_eq!(f[2], -1.0);
        assert!(f[3] < 1.0 && f[3] > 0.999);
        // A trailing odd byte is ignored.
        assert_eq!(pcm16_bytes_to_f32(&[1, 2, 3]).len(), 1);
    }

    #[test]
    fn decode_params_clamp_and_honour_gpu_switch() {
        let p = DecodeParams::from_config(&serde_json::json!({}), true);
        assert_eq!(p.beam_size, 1);
        assert!(p.use_gpu);
        assert!(!DecodeParams::from_config(&serde_json::json!({}), false).use_gpu);
        let p = DecodeParams::from_config(
            &serde_json::json!({"beam_size": 99, "use_gpu": false, "no_speech_thold": 3.0}),
            true,
        );
        assert_eq!(p.beam_size, 8);
        assert!(!p.use_gpu);
        assert_eq!(p.no_speech_thold, 1.0);
    }

    #[tokio::test]
    async fn missing_model_reports_not_installed() {
        let p = LocalWhisperProvider::new(
            LOCAL_ASR_ID,
            &serde_json::json!({"model": "whisper-small"}),
            false,
        );
        if p.is_ready() {
            return; // a developer machine with the model downloaded
        }
        let err = p
            .run(TaskInput::Asr {
                audio_pcm16_mono_16k: vec![0; 3200],
                language_hint: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::NotInstalled(ref m) if m == "Whisper small"));
    }
}
