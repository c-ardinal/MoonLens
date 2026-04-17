//! OCR via the `ocrs` crate.
//!
//! Models are loaded lazily: on first `run` the provider looks for
//! `text-detection.rten` and `text-recognition.rten` under `$APPDATA/models/`.
//! If either is missing the provider returns `LowConfidence` so chains can
//! fall through (e.g. to a plugin-provided OCR). The `download_models`
//! helper populates the directory from the upstream S3 bucket and is
//! wired up through a Tauri command so the UI can offer a one-click setup.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
    /// Cached engine once the models have been loaded. The engine is not
    /// `Send + Sync` friendly in older versions, so we guard with a mutex.
    #[cfg(feature = "ocrs-engine")]
    engine: OnceLock<Mutex<Option<ocrs::OcrEngine>>>,
}

impl OcrsProvider {
    pub fn new(id: impl Into<String>, _config: &serde_json::Value) -> Self {
        let models_dir = default_models_dir().unwrap_or_else(|| PathBuf::from("./models"));
        Self {
            id: id.into(),
            models_dir,
            #[cfg(feature = "ocrs-engine")]
            engine: OnceLock::new(),
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

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Ocr { image_png, .. } = input else {
            return Err(ProviderError::Other("OcrsProvider handles Ocr only".into()));
        };

        #[cfg(feature = "ocrs-engine")]
        {
            // Get or lazily initialize the engine.
            let cell = self.engine.get_or_init(|| Mutex::new(None));
            {
                let mut guard = cell.lock();
                if guard.is_none() {
                    match try_load_engine(&self.models_dir) {
                        Ok(e) => *guard = Some(e),
                        Err(e) => {
                            tracing::warn!("ocrs models unavailable: {e}");
                            return Err(ProviderError::LowConfidence);
                        }
                    }
                }
            }

            // Inference is CPU-bound; move off the async runtime thread.
            let models_dir = self.models_dir.clone();
            let image_png_owned = image_png.clone();
            let lines = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<OcrLine>> {
                let engine = try_load_engine(&models_dir)?;
                let img = image::load_from_memory(&image_png_owned)?;
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
                Ok(line_texts
                    .into_iter()
                    .filter_map(|line_opt| {
                        let line = line_opt?;
                        let text = line.to_string();
                        if text.trim().is_empty() {
                            return None;
                        }
                        // The detector rotated-rects do not expose an axis
                        // aligned bbox through a stable trait in the version
                        // we pin to. The overlay only needs rects for layout
                        // hints, so a zeroed placeholder is fine for now.
                        Some(OcrLine {
                            text,
                            bbox: [0.0, 0.0, 0.0, 0.0],
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

/// Download models into `models_dir`. Called by the Tauri command
/// `install_ocr_models` on user request so first launch stays offline.
pub async fn install_models(models_dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(models_dir)?;
    let client = reqwest::Client::new();
    for (url, filename) in [
        (DETECTION_MODEL_URL, "text-detection.rten"),
        (RECOGNITION_MODEL_URL, "text-recognition.rten"),
    ] {
        let path = models_dir.join(filename);
        if path.exists() && std::fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false) {
            continue;
        }
        let resp = client.get(url).send().await?.error_for_status()?;
        let bytes = resp.bytes().await?;
        std::fs::write(&path, &bytes)?;
    }
    Ok(())
}
