//! OCR via the `ocrs` crate.
//!
//! Models are loaded lazily and cached for the life of the process: on
//! first `run` the provider looks for `text-detection.onnx` and
//! `text-recognition.onnx` (or the legacy `.rten` files) under the data dir.
//! If either is missing the provider returns `LowConfidence` so chains can
//! fall through (e.g. to a plugin-provided OCR). The `download_models`
//! helper populates the directory from the upstream S3 bucket and is
//! wired up through a Tauri command so the UI can offer a one-click setup.

use std::path::{Path, PathBuf};
#[cfg(feature = "ocrs-engine")]
use std::sync::Arc;

use async_trait::async_trait;
#[cfg(feature = "ocrs-engine")]
use parking_lot::Mutex;

use super::{Capability, OcrLine, Provider, ProviderError, TaskInput, TaskOutput};

/// Upstream moved the published models from `.rten` to `.onnx`; the old
/// `.rten` objects now return 403, so fresh installs must fetch ONNX.
pub const DETECTION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.onnx";
pub const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.onnx";

pub const DETECTION_MODEL_FILE: &str = "text-detection.onnx";
pub const RECOGNITION_MODEL_FILE: &str = "text-recognition.onnx";
/// Pre-0.2 installs downloaded `.rten` files; keep loading them so
/// existing users do not have to re-download.
const LEGACY_DETECTION_FILE: &str = "text-detection.rten";
const LEGACY_RECOGNITION_FILE: &str = "text-recognition.rten";

/// Bundle identifier from `tauri.conf.json`. Tauri names the app's data
/// folders after it, and installers (the Windows uninstaller's "delete
/// app data" option) clean up exactly those folders.
pub const APP_IDENTIFIER: &str = "io.moonlens.app";

/// Where every model lives: `<local app data>/io.moonlens.app/models`
/// (`%LOCALAPPDATA%` on Windows, so multi-GB files never roam with the
/// profile). Up to v0.1.0 models went to `<data dir>/moonlens/models`,
/// outside the app's folders, so uninstalling left them behind; the first
/// call moves such a folder over.
pub fn default_models_dir() -> Option<PathBuf> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let new = local_data_base()?.join(APP_IDENTIFIER).join("models");
        Some(match legacy_models_dir() {
            Some(old) => adopt_legacy_dir(&old, &new),
            None => new,
        })
    })
    .clone()
}

/// Move `old` to `new` when only `old` exists. Returns the folder to use:
/// `old` if it could not be moved (e.g. another volume), else `new`.
pub fn adopt_legacy_dir(old: &Path, new: &Path) -> PathBuf {
    if new.exists() || !old.is_dir() {
        return new.to_path_buf();
    }
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(old, new) {
        Ok(()) => {
            tracing::info!("moved models from {} to {}", old.display(), new.display());
            // Drop the now empty `moonlens` folder too.
            if let Some(p) = old.parent() {
                let _ = std::fs::remove_dir(p);
            }
            new.to_path_buf()
        }
        Err(e) => {
            tracing::warn!("keeping models in {}: {e}", old.display());
            old.to_path_buf()
        }
    }
}

fn legacy_models_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        local_data_base()
    };
    base.map(|b| b.join("moonlens").join("models"))
}

/// Tauri's `app_local_data_dir` base, without needing an `AppHandle`.
pub fn local_data_base() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    }
}

/// Resolve the detection / recognition model paths in `dir`, preferring
/// ONNX and falling back to legacy `.rten` files.
pub fn model_paths(dir: &Path) -> Option<(PathBuf, PathBuf)> {
    let pick = |new: &str, legacy: &str| {
        let p = dir.join(new);
        if non_empty(&p) {
            return Some(p);
        }
        let l = dir.join(legacy);
        non_empty(&l).then_some(l)
    };
    Some((
        pick(DETECTION_MODEL_FILE, LEGACY_DETECTION_FILE)?,
        pick(RECOGNITION_MODEL_FILE, LEGACY_RECOGNITION_FILE)?,
    ))
}

fn non_empty(p: &Path) -> bool {
    std::fs::metadata(p).map(|m| m.len() > 0).unwrap_or(false)
}

/// How much to enlarge a capture before OCR (`upscale` provider config).
/// Default is no scaling: measured on real screenshots, a 2× upscale of
/// 1× DPI text made recognition slightly *worse* (`automatically` →
/// `automaticallv`) at the same speed, so it is opt-in only.
pub fn upscale_factor(configured: f32) -> f32 {
    if configured > 0.0 {
        configured.clamp(1.0, 4.0)
    } else {
        1.0
    }
}

pub struct OcrsProvider {
    id: String,
    models_dir: PathBuf,
    upscale: f32,
}

impl OcrsProvider {
    pub fn new(id: impl Into<String>, config: &serde_json::Value) -> Self {
        let models_dir = config
            .get("models_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .or_else(default_models_dir)
            .unwrap_or_else(|| PathBuf::from("./models"));
        let upscale = config
            .get("upscale")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;
        Self {
            id: id.into(),
            models_dir,
            upscale,
        }
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }
}

/// Process-wide engine cache. Loading the two models takes hundreds of
/// milliseconds, and providers are rebuilt on every settings change, so
/// the cache lives outside the provider and is keyed by model directory.
#[cfg(feature = "ocrs-engine")]
static ENGINE: Mutex<Option<(PathBuf, Arc<ocrs::OcrEngine>)>> = Mutex::new(None);

#[cfg(feature = "ocrs-engine")]
fn engine_for(dir: &Path) -> anyhow::Result<Arc<ocrs::OcrEngine>> {
    let mut guard = ENGINE.lock();
    if let Some((cached_dir, engine)) = guard.as_ref() {
        if cached_dir == dir {
            return Ok(engine.clone());
        }
    }
    let engine = Arc::new(try_load_engine(dir)?);
    *guard = Some((dir.to_path_buf(), engine.clone()));
    Ok(engine)
}

/// Load a fresh engine from `dir` (uncached). Exposed for the
/// `ocr_bench` example.
#[cfg(feature = "ocrs-engine")]
pub fn load_engine(dir: &Path) -> anyhow::Result<ocrs::OcrEngine> {
    try_load_engine(dir)
}

#[cfg(feature = "ocrs-engine")]
fn try_load_engine(dir: &Path) -> anyhow::Result<ocrs::OcrEngine> {
    use ocrs::{OcrEngine, OcrEngineParams};
    let (detection_path, recognition_path) = model_paths(dir)
        .ok_or_else(|| anyhow::anyhow!("ocrs models not installed at {}", dir.display()))?;
    let detection = rten::Model::load_file(&detection_path)
        .map_err(|e| anyhow::anyhow!("load {}: {e}", detection_path.display()))?;
    let recognition = rten::Model::load_file(&recognition_path)
        .map_err(|e| anyhow::anyhow!("load {}: {e}", recognition_path.display()))?;
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection),
        recognition_model: Some(recognition),
        ..Default::default()
    })
    .map_err(|e| anyhow::anyhow!("ocrs init: {e}"))?;
    Ok(engine)
}

/// Run detection + recognition and return lines with bboxes in the
/// coordinate space of the *original* (un-upscaled) image.
#[cfg(feature = "ocrs-engine")]
pub fn recognize(
    engine: &ocrs::OcrEngine,
    img: &image::DynamicImage,
    upscale: f32,
) -> anyhow::Result<Vec<OcrLine>> {
    use ocrs::TextItem;
    let scale = upscale_factor(upscale);
    let rgb = if scale > 1.0 {
        let w = (img.width() as f32 * scale).round() as u32;
        let h = (img.height() as f32 * scale).round() as u32;
        img.resize_exact(w, h, image::imageops::FilterType::Triangle)
            .to_rgb8()
    } else {
        img.to_rgb8()
    };
    let (w, h) = (rgb.width(), rgb.height());
    let source = ocrs::ImageSource::from_bytes(rgb.as_raw(), (w, h))
        .map_err(|e| anyhow::anyhow!("ocrs image source: {e}"))?;
    let input = engine
        .prepare_input(source)
        .map_err(|e| anyhow::anyhow!("ocrs prepare: {e}"))?;
    let words = engine
        .detect_words(&input)
        .map_err(|e| anyhow::anyhow!("ocrs detect: {e}"))?;
    let line_rects = engine.find_text_lines(&input, &words);
    let line_texts = engine
        .recognize_text(&input, &line_rects)
        .map_err(|e| anyhow::anyhow!("ocrs recognize: {e}"))?;
    Ok(line_texts
        .into_iter()
        .flatten()
        .filter_map(|line| {
            let text = line.to_string().trim().to_string();
            if text.is_empty() {
                return None;
            }
            let r = line.bounding_rect();
            let bbox = [
                r.left() as f32 / scale,
                r.top() as f32 / scale,
                r.width() as f32 / scale,
                r.height() as f32 / scale,
            ];
            Some(OcrLine {
                text,
                bbox,
                confidence: 1.0,
            })
        })
        .collect())
}

#[async_trait]
impl Provider for OcrsProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Ocr]
    }
    fn is_local(&self) -> bool {
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Ocr { image_png, .. } = input else {
            return Err(ProviderError::Other("OcrsProvider handles Ocr only".into()));
        };

        #[cfg(feature = "ocrs-engine")]
        {
            if model_paths(&self.models_dir).is_none() {
                return Err(ProviderError::NotInstalled("ocrs".into()));
            }
            let models_dir = self.models_dir.clone();
            let upscale = self.upscale;
            // Model loading and inference are CPU-bound; keep both off
            // the async runtime threads.
            let lines =
                tokio::task::spawn_blocking(move || -> Result<Vec<OcrLine>, ProviderError> {
                    let engine = engine_for(&models_dir).map_err(|e| {
                        tracing::warn!("ocrs models unavailable: {e}");
                        ProviderError::LowConfidence
                    })?;
                    let img = image::load_from_memory(&image_png)
                        .map_err(|e| ProviderError::Other(e.to_string()))?;
                    recognize(&engine, &img, upscale)
                        .map_err(|e| ProviderError::Other(e.to_string()))
                })
                .await
                .map_err(|e| ProviderError::Other(e.to_string()))??;

            if lines.is_empty() {
                return Err(ProviderError::LowConfidence);
            }
            Ok(TaskOutput::Ocr { lines })
        }
        #[cfg(not(feature = "ocrs-engine"))]
        {
            let _ = (image_png, self.upscale);
            Err(ProviderError::LowConfidence)
        }
    }
}

/// Progress callback signature for [`install_models_with_progress`].
/// `total` is `None` when the server did not advertise Content-Length.
pub type ProgressCb = dyn Fn(&str, u64, Option<u64>) + Send + Sync;

/// Download models into `models_dir`. Called by the Tauri command
/// `install_ocr_models` on user request so first launch stays offline.
///
/// When a `progress` callback is supplied, it is invoked each time a
/// new chunk of bytes arrives so the UI can render a progress bar.
pub async fn install_models_with_progress(
    models_dir: &Path,
    progress: Option<&ProgressCb>,
) -> anyhow::Result<()> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    std::fs::create_dir_all(models_dir)?;
    let client = reqwest::Client::new();
    for (url, filename) in [
        (DETECTION_MODEL_URL, DETECTION_MODEL_FILE),
        (RECOGNITION_MODEL_URL, RECOGNITION_MODEL_FILE),
    ] {
        let path = models_dir.join(filename);
        if path.exists()
            && std::fs::metadata(&path)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
            if let Some(cb) = progress {
                let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                cb(filename, size, Some(size));
            }
            continue;
        }
        let resp = client.get(url).send().await?.error_for_status()?;
        let total = resp.content_length();
        let mut stream = resp.bytes_stream();
        // Write atomically to a tempfile then rename, so a cancelled
        // download does not leave a truncated .rten for the engine to
        // try to load on the next launch.
        let tmp_path = path.with_extension("part");
        let mut file = tokio::fs::File::create(&tmp_path).await?;
        let mut downloaded: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if let Some(cb) = progress {
                cb(filename, downloaded, total);
            }
        }
        file.flush().await?;
        drop(file);
        tokio::fs::rename(&tmp_path, &path).await?;
    }
    Ok(())
}

/// Convenience wrapper used by non-UI callers. Equivalent to
/// `install_models_with_progress(dir, None)`.
pub async fn install_models(models_dir: &Path) -> anyhow::Result<()> {
    install_models_with_progress(models_dir, None).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_models_folder_is_moved_once() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("moonlens/models");
        let new = tmp.path().join("io.moonlens.app/models");
        std::fs::create_dir_all(old.join("llm")).unwrap();
        std::fs::write(old.join("llm/a.gguf"), b"x").unwrap();
        assert_eq!(adopt_legacy_dir(&old, &new), new);
        assert!(new.join("llm/a.gguf").exists());
        assert!(!tmp.path().join("moonlens").exists());
        // Second run: nothing left to move.
        assert_eq!(adopt_legacy_dir(&old, &new), new);
    }

    #[test]
    fn existing_new_folder_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let old = tmp.path().join("old");
        let new = tmp.path().join("new");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        assert_eq!(adopt_legacy_dir(&old, &new), new);
        assert!(old.exists());
    }

    #[test]
    fn upscale_is_opt_in_and_clamped() {
        assert_eq!(upscale_factor(0.0), 1.0);
        assert_eq!(upscale_factor(2.0), 2.0);
        assert_eq!(upscale_factor(9.0), 4.0);
        assert_eq!(upscale_factor(0.5), 1.0);
    }

    #[test]
    fn model_paths_prefer_onnx_and_accept_legacy_rten() {
        let dir = tempfile::tempdir().unwrap();
        assert!(model_paths(dir.path()).is_none());
        std::fs::write(dir.path().join("text-detection.rten"), b"x").unwrap();
        std::fs::write(dir.path().join("text-recognition.rten"), b"x").unwrap();
        let (d, r) = model_paths(dir.path()).unwrap();
        assert!(d.ends_with("text-detection.rten"));
        assert!(r.ends_with("text-recognition.rten"));
        std::fs::write(dir.path().join(DETECTION_MODEL_FILE), b"x").unwrap();
        let (d, _) = model_paths(dir.path()).unwrap();
        assert!(d.ends_with(DETECTION_MODEL_FILE));
        // Empty files (interrupted downloads) are ignored.
        std::fs::write(dir.path().join(RECOGNITION_MODEL_FILE), b"").unwrap();
        let (_, r) = model_paths(dir.path()).unwrap();
        assert!(r.ends_with("text-recognition.rten"));
    }
}
