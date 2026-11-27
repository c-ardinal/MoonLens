//! Catalog of the whisper.cpp models that the built-in speech
//! recognition ([`super::asr_local`]) runs for live subtitles.
//!
//! Files come from the whisper.cpp model repository on Hugging Face and
//! are pinned by size + SHA-256; downloading, resuming and verification
//! live in [`super::model_store`].

use crate::i18n::Text;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::model_store::{self, PinnedFile};

#[derive(Debug, Clone, Serialize)]
pub struct AsrModel {
    /// Stable id stored in `[providers.configs.whisper] model = "…"`.
    pub id: &'static str,
    pub label: &'static str,
    /// One-line description shown in the model list (Japanese UI).
    pub note: Text,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub license: &'static str,
    pub license_url: &'static str,
}

/// Model used when the config names none (or an unknown id). Whisper
/// large-v3-turbo has the best Japanese / English accuracy of the
/// models that still decode in near real time on a CPU (see
/// docs/asr-models.md).
pub const DEFAULT_MODEL_ID: &str = "whisper-large-v3-turbo";

pub const CATALOG: &[AsrModel] = &[
    AsrModel {
        id: "whisper-large-v3-turbo",
        label: "Whisper large-v3-turbo",
        note: Text::new(
            "推奨。日英双方で高精度",
            "Recommended. Accurate in Japanese and English",
        ),
        file: "ggml-large-v3-turbo-q8_0.bin",
        url:
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin",
        size: 874_188_075,
        sha256: "317eb69c11673c9de1e1f0d459b253999804ec71ac4c23c17ecf5fbe24e259a1",
        license: "MIT",
        license_url: "https://github.com/openai/whisper/blob/main/LICENSE",
    },
    AsrModel {
        id: "whisper-small",
        label: "Whisper small",
        note: Text::new(
            "軽量・高速。低スペック PC 用。精度は中程度",
            "Light and fast, for low-spec PCs. Medium accuracy",
        ),
        file: "ggml-small-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
        size: 190_085_487,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        license: "MIT",
        license_url: "https://github.com/openai/whisper/blob/main/LICENSE",
    },
];

pub fn find(id: &str) -> Option<&'static AsrModel> {
    CATALOG.iter().find(|m| m.id == id)
}

impl AsrModel {
    pub fn pinned(&self) -> PinnedFile {
        PinnedFile {
            id: self.id,
            label: self.label,
            file: self.file,
            url: self.url,
            size: self.size,
            sha256: self.sha256,
        }
    }
}

/// Where whisper models live: `<models dir>/whisper`.
pub fn default_models_dir() -> Option<PathBuf> {
    super::ocr_ocrs::default_models_dir().map(|d| d.join("whisper"))
}

pub fn model_path(dir: &Path, m: &AsrModel) -> PathBuf {
    model_store::model_path(dir, &m.pinned())
}

pub fn is_installed(dir: &Path, m: &AsrModel) -> bool {
    model_store::is_installed(dir, &m.pinned())
}

/// The catalog entry a provider config selects (`model = "<id>"`),
/// falling back to the default for a missing or unknown id.
pub fn selected(cfg: &serde_json::Value) -> &'static AsrModel {
    cfg.get("model")
        .and_then(|v| v.as_str())
        .and_then(find)
        .unwrap_or_else(|| find(DEFAULT_MODEL_ID).expect("default in catalog"))
}

#[derive(Debug, Clone, Serialize)]
pub struct AsrModelStatus {
    #[serde(flatten)]
    pub model: AsrModel,
    pub installed: bool,
    /// Bytes of an interrupted download that a retry will resume from.
    pub partial_bytes: u64,
    pub downloading: bool,
}

pub fn status(dir: &Path) -> Vec<AsrModelStatus> {
    CATALOG
        .iter()
        .map(|m| AsrModelStatus {
            model: m.clone(),
            installed: is_installed(dir, m),
            partial_bytes: model_store::partial_bytes(dir, &m.pinned()),
            downloading: model_store::is_downloading(m.id),
        })
        .collect()
}

/// Remove an installed model (unloading it first) and any partial
/// download.
pub fn remove(dir: &Path, id: &str) -> anyhow::Result<()> {
    let m = find(id).ok_or_else(|| anyhow::anyhow!("unknown model {id}"))?;
    model_store::cancel(id);
    super::asr_local::unload(&model_path(dir, m));
    model_store::remove_files(dir, &m.pinned())
}

pub async fn install(
    dir: &Path,
    id: &str,
    progress: Option<&model_store::ProgressCb>,
) -> anyhow::Result<PathBuf> {
    let m = find(id).ok_or_else(|| anyhow::anyhow!("unknown model {id}"))?;
    model_store::install(dir, &m.pinned(), progress).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        assert!(find(DEFAULT_MODEL_ID).is_some());
        let mut ids = std::collections::HashSet::new();
        for m in CATALOG {
            assert!(ids.insert(m.id), "duplicate id {}", m.id);
            // Ids share the download table with the LLM catalog.
            assert!(super::super::llm_models::find(m.id).is_none(), "{}", m.id);
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(m.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
            assert!(m.url.starts_with("https://huggingface.co/"), "{}", m.id);
            assert!(m.url.ends_with(m.file), "{}", m.id);
            assert!(m.file.starts_with("ggml-") && m.file.ends_with(".bin"));
            assert!(m.size > 50 << 20, "{}", m.id);
        }
    }

    #[test]
    fn selected_falls_back_to_default() {
        let d = find(DEFAULT_MODEL_ID).unwrap().id;
        assert_eq!(selected(&serde_json::json!({})).id, d);
        assert_eq!(selected(&serde_json::json!({"model": "nope"})).id, d);
        assert_eq!(
            selected(&serde_json::json!({"model": "whisper-small"})).id,
            "whisper-small"
        );
    }

    #[test]
    fn status_lists_every_model() {
        let dir = tempfile::tempdir().unwrap();
        let s = status(dir.path());
        assert_eq!(s.len(), CATALOG.len());
        assert!(s.iter().all(|m| !m.installed && m.partial_bytes == 0));
    }
}
