//! OCR via the `ocrs` crate.
//!
//! Models are loaded lazily: on first `run` the provider looks for
//! `text-detection.rten` and `text-recognition.rten` under `$APPDATA/models/`.
//! If either is missing the provider returns `LowConfidence` so chains can
//! fall through (e.g. to a plugin-provided OCR). The `download_models`
//! helper populates the directory from the upstream S3 bucket and is
//! wired up through a Tauri command so the UI can offer a one-click setup.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;

use super::{Capability, OcrLine, Provider, ProviderError, TaskInput, TaskOutput};

pub const DETECTION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten";
pub const RECOGNITION_MODEL_URL: &str =
    "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten";

pub fn default_models_dir() -> Option<PathBuf> {
    dirs_like("moonlens/models")
}

/// Minimal, dependency-free XDG-ish data dir lookup. Good enough for the
/// places we want to drop ML models (not user configuration).
fn dirs_like(suffix: &str) -> Option<PathBuf> {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.map(|b| b.join(suffix))
}

pub struct OcrsProvider {
    id: String,
    models_dir: PathBuf,
    /// Shared cached engine. Arc<Mutex<Option<OcrEngine>>> so a clone
    /// of the Arc can be moved into the `spawn_blocking` closure and
    /// the engine loaded exactly once per process — the previous
    /// OnceLock design kept a cache the inference path never looked
    /// at, which caused a fresh `rten::Model::load_file` call on
    /// every OCR (~1-3s per call, blowing the 5 s chain timeout).
    #[cfg(feature = "ocrs-engine")]
    engine: Arc<Mutex<Option<ocrs::OcrEngine>>>,
}

impl OcrsProvider {
    pub fn new(id: impl Into<String>, _config: &serde_json::Value) -> Self {
        let models_dir = default_models_dir().unwrap_or_else(|| PathBuf::from("./models"));
        Self {
            id: id.into(),
            models_dir,
            #[cfg(feature = "ocrs-engine")]
            engine: Arc::new(Mutex::new(None)),
        }
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }
}

#[cfg(feature = "ocrs-engine")]
fn try_load_engine(dir: &Path) -> anyhow::Result<ocrs::OcrEngine> {
    use ocrs::{OcrEngine, OcrEngineParams};
    let detection_path = dir.join("text-detection.rten");
    let recognition_path = dir.join("text-recognition.rten");
    if !detection_path.exists() || !recognition_path.exists() {
        anyhow::bail!("ocrs models not installed at {}", dir.display());
    }
    let detection = rten::Model::load_file(&detection_path)?;
    let recognition = rten::Model::load_file(&recognition_path)?;
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection),
        recognition_model: Some(recognition),
        ..Default::default()
    })
    .map_err(|e| anyhow::anyhow!("ocrs init: {e}"))?;
    Ok(engine)
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

    async fn preload(&self) -> Result<(), ProviderError> {
        #[cfg(feature = "ocrs-engine")]
        {
            let engine_cache = self.engine.clone();
            let models_dir = self.models_dir.clone();
            tokio::task::spawn_blocking(move || -> Result<(), ProviderError> {
                let mut guard = engine_cache.lock();
                if guard.is_none() {
                    let start = std::time::Instant::now();
                    tracing::info!("ocrs engine preload starting from {}", models_dir.display());
                    match try_load_engine(&models_dir) {
                        Ok(e) => {
                            *guard = Some(e);
                            tracing::info!(
                                "ocrs engine preloaded in {}ms",
                                start.elapsed().as_millis()
                            );
                        }
                        Err(e) => {
                            tracing::warn!("ocrs preload failed: {e}");
                            return Err(ProviderError::Other(e.to_string()));
                        }
                    }
                } else {
                    tracing::debug!("ocrs engine already cached, skipping preload");
                }
                Ok(())
            })
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))??;
        }
        Ok(())
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Ocr { image_png, .. } = input else {
            return Err(ProviderError::Other("OcrsProvider handles Ocr only".into()));
        };

        #[cfg(feature = "ocrs-engine")]
        {
            // Inference is CPU-bound; move off the async runtime thread.
            // The engine mutex is shared, so the first OCR pays the
            // model-load cost once and every subsequent call reuses
            // the already-loaded engine.
            let engine_cache = self.engine.clone();
            let models_dir = self.models_dir.clone();
            let image_png_owned = image_png.clone();
            let lines = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<OcrLine>> {
                let lock_start = std::time::Instant::now();
                let mut guard = engine_cache.lock();
                let lock_ms = lock_start.elapsed().as_millis();
                if guard.is_none() {
                    let load_start = std::time::Instant::now();
                    tracing::info!("ocrs engine cold load in run() from {}", models_dir.display());
                    *guard = Some(try_load_engine(&models_dir)?);
                    tracing::info!(
                        "ocrs engine loaded (in run) in {}ms",
                        load_start.elapsed().as_millis()
                    );
                } else if lock_ms > 100 {
                    tracing::info!("ocrs engine mutex wait: {lock_ms}ms");
                }
                let engine = guard.as_ref().expect("engine just ensured above");
                let decode_start = std::time::Instant::now();
                let img = image::load_from_memory(&image_png_owned)?;
                tracing::debug!(
                    "ocrs: decoded {}x{} PNG ({} bytes) in {}ms",
                    img.width(),
                    img.height(),
                    image_png_owned.len(),
                    decode_start.elapsed().as_millis()
                );
                let rgb = img.to_rgb8();
                let (w, h) = (rgb.width() as usize, rgb.height() as usize);
                let source = ocrs::ImageSource::from_bytes(rgb.as_raw(), (w as u32, h as u32))
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
                let infer_start = std::time::Instant::now();
                tracing::debug!("ocrs: detect_words start on {}x{}", w, h);
                // Scope re-entry marker so any infer panic is visible
                let _ = infer_start;
                Ok(line_texts
                    .into_iter()
                    .zip(line_rects.into_iter())
                    .filter_map(|(line_opt, word_rects)| {
                        let line = line_opt?;
                        let text = line.to_string();
                        if text.trim().is_empty() {
                            return None;
                        }
                        // Axis-aligned bbox = union of every rotated-rect
                        // corner across the line's word rects. The overlay
                        // maps bbox back to its CSS cell on render, so the
                        // numbers here are image-pixel space.
                        let mut min_x = f32::MAX;
                        let mut min_y = f32::MAX;
                        let mut max_x = f32::MIN;
                        let mut max_y = f32::MIN;
                        for rr in &word_rects {
                            for c in rr.corners() {
                                min_x = min_x.min(c.x);
                                min_y = min_y.min(c.y);
                                max_x = max_x.max(c.x);
                                max_y = max_y.max(c.y);
                            }
                        }
                        let bbox = if min_x > max_x {
                            [0.0, 0.0, 0.0, 0.0]
                        } else {
                            [min_x, min_y, max_x - min_x, max_y - min_y]
                        };
                        Some(OcrLine {
                            text,
                            bbox,
                            confidence: 1.0,
                        })
                    })
                    .collect())
            })
            .await
            .map_err(|e| ProviderError::Other(e.to_string()))?
            .map_err(|e| ProviderError::Other(e.to_string()))?;

            if lines.is_empty() {
                return Err(ProviderError::LowConfidence);
            }
            Ok(TaskOutput::Ocr { lines })
        }
        #[cfg(not(feature = "ocrs-engine"))]
        {
            let _ = image_png;
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
        (DETECTION_MODEL_URL, "text-detection.rten"),
        (RECOGNITION_MODEL_URL, "text-recognition.rten"),
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
        let tmp_path = path.with_extension("rten.part");
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
