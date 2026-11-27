//! Catalog and installer for the GGUF models that the built-in
//! llama.cpp provider ([`super::translate_llama`]) runs.
//!
//! Every entry pins the exact file (URL + size + SHA-256); downloading
//! and verification live in [`super::model_store`].

use crate::i18n::Text;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::model_store::{self, PinnedFile};
pub use super::model_store::{free_space, ProgressCb, FREE_SPACE_MARGIN};

/// How the provider turns a request into a prompt for this model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptStyle {
    /// General instruction model: render the chat template stored in the
    /// GGUF with a system + user message. Handles translate, explain and
    /// summarize.
    ChatTemplate,
    /// PLaMo 3 Translate: hand-written PFML3 prompt, translation only.
    Pfml3,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogModel {
    /// Stable id stored in `[providers.configs.llamacpp] model = "…"`.
    pub id: &'static str,
    pub label: &'static str,
    /// One-line description shown in the model picker.
    pub note: Text,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub license: &'static str,
    pub license_url: &'static str,
    pub style: PromptStyle,
    /// Id of a licence the user must explicitly accept before this model
    /// is downloaded (see [`require_consent`]); `None` when the licence
    /// needs no consent step.
    pub consent: Option<&'static str>,
}

/// Licence id for the PLaMo Community License.
pub const CONSENT_PLAMO: &str = "plamo-community";

/// Translation model used when the config names none (or an unknown
/// id). PLaMo 3 Translate 2B won the in-house benchmark: same chrF as
/// Gemma 4 E4B on WMT22 ja-en / en-ja at ~2.5x the speed, the most
/// natural output on game / UI / signage text, and it cannot be talked
/// out of translating by the text it is given.
pub const DEFAULT_TRANSLATE_MODEL_ID: &str = "plamo-3-translate-2b";
/// General model for explain / summarize (PLaMo only translates).
pub const DEFAULT_ASSIST_MODEL_ID: &str = "gemma-4-e4b";

/// Every model the app can download. Sizes and hashes are the LFS
/// metadata Hugging Face publishes for that exact file.
pub const CATALOG: &[CatalogModel] = &[
    CatalogModel {
        id: "plamo-3-translate-2b",
        label: "PLaMo 3 Translate 2B",
        note: Text::new("models:recommended_high_quality_and_fast"),
        file: "plamo-3-translate-2b-Q4_K_M.gguf",
        url: "https://huggingface.co/pfnet/plamo-3-translate-2b-GGUF/resolve/main/plamo-3-translate-2b-Q4_K_M.gguf",
        size: 1_807_203_040,
        sha256: "1ffd9073afcc70e35ccc942428aa2b73d7aa2b9ea779bf504b775369e0276c13",
        license: "PLaMo Community License",
        license_url: "https://plamo.preferredai.jp/info/plamo-community-license-en",
        style: PromptStyle::Pfml3,
        consent: Some(CONSENT_PLAMO),
    },
    CatalogModel {
        id: "plamo-3-translate-0.8b",
        label: "PLaMo 3 Translate 0.8B",
        note: Text::new("models:fastest_and_lightest_for_low_spec_pcs"),
        file: "plamo-3-translate-0.8b-Q4_K_M.gguf",
        url: "https://huggingface.co/pfnet/plamo-3-translate-0.8b-GGUF/resolve/main/plamo-3-translate-0.8b-Q4_K_M.gguf",
        size: 573_126_848,
        sha256: "70891ec70dc7ab4205745a38307867c2881b99239f20327927633abfe4bc9ec6",
        license: "PLaMo Community License",
        license_url: "https://plamo.preferredai.jp/info/plamo-community-license-en",
        style: PromptStyle::Pfml3,
        consent: Some(CONSENT_PLAMO),
    },
    CatalogModel {
        id: "gemma-4-e4b",
        label: "Gemma 4 E4B",
        note: Text::new("models:general_model_for_explanations_and_summaries"),
        file: "gemma-4-E4B-it-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/gemma-4-E4B-it-GGUF/resolve/main/gemma-4-E4B-it-Q4_K_M.gguf",
        size: 4_977_171_584,
        sha256: "85a896a047553e842f25297ee5b031d64ff30147d9c4af17b1e4b394cd1fab87",
        license: "Apache-2.0",
        license_url: "https://ai.google.dev/gemma/apache_2",
        style: PromptStyle::ChatTemplate,
        consent: None,
    },
    CatalogModel {
        id: "gemma-4-e2b",
        label: "Gemma 4 E2B",
        note: Text::new("models:lightweight_general_model_faster_than_e4b"),
        file: "gemma-4-E2B-it-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-E2B-it-Q4_K_M.gguf",
        size: 3_106_738_272,
        sha256: "740185b21d22ceb83a11c3aa62ad5842ef32c70f6096d756bbee85a1e4ec34b8",
        license: "Apache-2.0",
        license_url: "https://ai.google.dev/gemma/apache_2",
        style: PromptStyle::ChatTemplate,
        consent: None,
    },
];

pub fn find(id: &str) -> Option<&'static CatalogModel> {
    CATALOG.iter().find(|m| m.id == id)
}

/// Refuse a download whose licence is not in `accepted`. The error is the
/// licence id the user still has to accept.
pub fn require_consent(m: &CatalogModel, accepted: &[String]) -> Result<(), &'static str> {
    match m.consent {
        Some(license) if !accepted.iter().any(|a| a == license) => Err(license),
        _ => Ok(()),
    }
}

/// Record acceptance of licence `id`. Only ids a catalog entry uses are
/// accepted; repeating an accepted id changes nothing. Returns whether
/// `accepted` changed.
pub fn accept_license(accepted: &mut Vec<String>, id: &str) -> Result<bool, String> {
    if !CATALOG.iter().any(|m| m.consent == Some(id)) {
        return Err(format!("unknown license {id}"));
    }
    if accepted.iter().any(|a| a == id) {
        return Ok(false);
    }
    accepted.push(id.to_string());
    Ok(true)
}

impl CatalogModel {
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

/// Where GGUF files live: `<data dir>/moonlens/models/llm`.
pub fn default_models_dir() -> Option<PathBuf> {
    super::ocr_ocrs::default_models_dir().map(|d| d.join("llm"))
}

pub fn model_path(dir: &Path, m: &CatalogModel) -> PathBuf {
    model_store::model_path(dir, &m.pinned())
}

pub fn is_installed(dir: &Path, m: &CatalogModel) -> bool {
    model_store::is_installed(dir, &m.pinned())
}

/// Bytes a download of `m` still has to fetch (resuming any `.part`).
pub fn bytes_still_needed(dir: &Path, m: &CatalogModel) -> u64 {
    model_store::bytes_still_needed(dir, &m.pinned())
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelStatus {
    #[serde(flatten)]
    pub model: CatalogModel,
    pub installed: bool,
    /// Bytes of an interrupted download that a retry will resume from.
    pub partial_bytes: u64,
    pub downloading: bool,
}

pub fn status(dir: &Path) -> Vec<ModelStatus> {
    CATALOG
        .iter()
        .map(|m| ModelStatus {
            model: m.clone(),
            installed: is_installed(dir, m),
            partial_bytes: model_store::partial_bytes(dir, &m.pinned()),
            downloading: model_store::is_downloading(m.id),
        })
        .collect()
}

/// Ask an in-flight download to stop (the `.part` file is kept).
pub fn cancel(id: &str) -> bool {
    model_store::cancel(id)
}

/// A file in the models folder that no catalog entry owns: a model an
/// earlier version shipped, or a leftover download of one.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OrphanFile {
    pub name: String,
    pub size: u64,
}

fn is_model_file(name: &str) -> bool {
    name.ends_with(".gguf") || name.ends_with(".gguf.part")
}

pub fn orphans(dir: &Path) -> Vec<OrphanFile> {
    let owned: std::collections::HashSet<String> = CATALOG
        .iter()
        .flat_map(|m| [m.file.to_string(), format!("{}.part", m.file)])
        .collect();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut out: Vec<OrphanFile> = rd
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            (is_model_file(&name) && !owned.contains(&name)).then(|| OrphanFile {
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                name,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Delete one file listed by [`orphans`]. Anything else (catalog models,
/// other files, paths) is refused.
pub fn remove_orphan(dir: &Path, name: &str) -> anyhow::Result<()> {
    if !orphans(dir).iter().any(|o| o.name == name) {
        anyhow::bail!("not a leftover model file: {name}");
    }
    let p = dir.join(name);
    super::translate_llama::unload(&p);
    std::fs::remove_file(p)?;
    Ok(())
}

/// Remove an installed model and any partial download.
pub fn remove(dir: &Path, id: &str) -> anyhow::Result<()> {
    let m = find(id).ok_or_else(|| anyhow::anyhow!("unknown model {id}"))?;
    model_store::cancel(id);
    // Drop the loaded weights first: Windows refuses to delete a file
    // that is still memory-mapped.
    super::translate_llama::unload(&model_path(dir, m));
    model_store::remove_files(dir, &m.pinned())
}

/// Download, verify and install `id` into `dir`. Resumes a previous
/// partial download. No-op when already installed.
pub async fn install(
    dir: &Path,
    id: &str,
    progress: Option<&ProgressCb>,
) -> anyhow::Result<PathBuf> {
    let m = find(id).ok_or_else(|| anyhow::anyhow!("unknown model {id}"))?;
    model_store::install(dir, &m.pinned(), progress).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_well_formed() {
        assert_eq!(
            find(DEFAULT_TRANSLATE_MODEL_ID).map(|m| m.style),
            Some(PromptStyle::Pfml3)
        );
        // explain / summarize need a general chat model.
        assert_eq!(
            find(DEFAULT_ASSIST_MODEL_ID).map(|m| m.style),
            Some(PromptStyle::ChatTemplate)
        );
        let mut ids = std::collections::HashSet::new();
        for m in CATALOG {
            assert!(ids.insert(m.id), "duplicate id {}", m.id);
            assert_eq!(m.sha256.len(), 64, "{}", m.id);
            assert!(m.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
            assert!(m.url.starts_with("https://huggingface.co/"), "{}", m.id);
            assert!(m.url.ends_with(m.file), "{}", m.id);
            assert!(m.size > 100 << 20, "{}", m.id);
            assert!(m.url.contains(".gguf") && m.file.ends_with(".gguf"));
        }
    }

    #[test]
    fn plamo_models_require_consent() {
        for m in CATALOG {
            let plamo = m.license == "PLaMo Community License";
            assert_eq!(m.consent == Some(CONSENT_PLAMO), plamo, "{}", m.id);
        }
        assert!(CATALOG.iter().any(|m| m.consent.is_some()));
    }

    #[test]
    fn download_needs_accepted_license() {
        let plamo = find(DEFAULT_TRANSLATE_MODEL_ID).unwrap();
        let gemma = find(DEFAULT_ASSIST_MODEL_ID).unwrap();
        assert_eq!(require_consent(plamo, &[]), Err(CONSENT_PLAMO));
        assert_eq!(
            require_consent(plamo, &["other".into()]),
            Err(CONSENT_PLAMO)
        );
        assert_eq!(require_consent(gemma, &[]), Ok(()));
        assert_eq!(require_consent(plamo, &[CONSENT_PLAMO.into()]), Ok(()));
    }

    #[test]
    fn accepting_is_idempotent_and_rejects_unknown_ids() {
        let mut accepted = vec![];
        assert!(accept_license(&mut accepted, "nope").is_err());
        assert!(accept_license(&mut accepted, "").is_err());
        assert!(accepted.is_empty());
        assert_eq!(accept_license(&mut accepted, CONSENT_PLAMO), Ok(true));
        assert_eq!(accept_license(&mut accepted, CONSENT_PLAMO), Ok(false));
        assert_eq!(accepted, vec![CONSENT_PLAMO.to_string()]);
    }

    #[test]
    fn orphans_are_model_files_the_catalog_does_not_own() {
        let dir = tempfile::tempdir().unwrap();
        let m = find(DEFAULT_TRANSLATE_MODEL_ID).unwrap();
        std::fs::write(model_path(dir.path(), m), b"x").unwrap();
        std::fs::write(model_store::part_path(dir.path(), &m.pinned()), b"x").unwrap();
        std::fs::write(dir.path().join("old-model-Q4_K_M.gguf"), b"xyz").unwrap();
        std::fs::write(dir.path().join("old-model-Q4_K_M.gguf.part"), b"x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        let names: Vec<_> = orphans(dir.path()).into_iter().map(|o| o.name).collect();
        assert_eq!(
            names,
            vec!["old-model-Q4_K_M.gguf", "old-model-Q4_K_M.gguf.part"]
        );
        // Only listed files can be removed.
        assert!(remove_orphan(dir.path(), m.file).is_err());
        assert!(remove_orphan(dir.path(), "notes.txt").is_err());
        assert!(remove_orphan(dir.path(), "../x.gguf").is_err());
        remove_orphan(dir.path(), "old-model-Q4_K_M.gguf").unwrap();
        assert_eq!(orphans(dir.path()).len(), 1);
    }

    #[test]
    fn still_needed_accounts_for_partial_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let m = find(DEFAULT_TRANSLATE_MODEL_ID).unwrap();
        assert_eq!(bytes_still_needed(dir.path(), m), m.size);
        std::fs::write(
            model_store::part_path(dir.path(), &m.pinned()),
            vec![0u8; 1000],
        )
        .unwrap();
        assert_eq!(bytes_still_needed(dir.path(), m), m.size - 1000);
        assert!(free_space(&dir.path().join("does/not/exist")).is_some());
    }

    #[test]
    fn status_lists_every_model() {
        let dir = tempfile::tempdir().unwrap();
        let s = status(dir.path());
        assert_eq!(s.len(), CATALOG.len());
        assert!(s.iter().all(|m| !m.installed && m.partial_bytes == 0));
    }
}
