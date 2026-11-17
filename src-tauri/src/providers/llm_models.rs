//! Catalog and installer for the GGUF models that the built-in
//! llama.cpp provider ([`super::translate_llama`]) runs.
//!
//! Every entry pins the exact file (URL + size + SHA-256), so a model is
//! only ever loaded after its bytes were verified. Downloads stream to a
//! `.part` file next to the target (multi-GB files never sit in RAM),
//! resume with an HTTP Range request after an interruption, and are
//! renamed into place only once the hash matches.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use serde::Serialize;
use sha2::{Digest, Sha256};

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
    /// One-line description shown in the model picker (Japanese UI).
    pub note: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub license: &'static str,
    pub license_url: &'static str,
    pub style: PromptStyle,
}

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
        note: "翻訳の推奨モデル。高品質で高速。翻訳専用",
        file: "plamo-3-translate-2b-Q4_K_M.gguf",
        url: "https://huggingface.co/pfnet/plamo-3-translate-2b-GGUF/resolve/main/plamo-3-translate-2b-Q4_K_M.gguf",
        size: 1_807_203_040,
        sha256: "1ffd9073afcc70e35ccc942428aa2b73d7aa2b9ea779bf504b775369e0276c13",
        license: "PLaMo Community License",
        license_url: "https://plamo.preferredai.jp/info/plamo-community-license-en",
        style: PromptStyle::Pfml3,
    },
    CatalogModel {
        id: "plamo-3-translate-0.8b",
        label: "PLaMo 3 Translate 0.8B",
        note: "最速・省メモリ。低スペック PC 向け。翻訳専用",
        file: "plamo-3-translate-0.8b-Q4_K_M.gguf",
        url: "https://huggingface.co/pfnet/plamo-3-translate-0.8b-GGUF/resolve/main/plamo-3-translate-0.8b-Q4_K_M.gguf",
        size: 573_126_848,
        sha256: "70891ec70dc7ab4205745a38307867c2881b99239f20327927633abfe4bc9ec6",
        license: "PLaMo Community License",
        license_url: "https://plamo.preferredai.jp/info/plamo-community-license-en",
        style: PromptStyle::Pfml3,
    },
    CatalogModel {
        id: "gemma-4-e4b",
        label: "Gemma 4 E4B",
        note: "説明・要約用の汎用モデル（翻訳にも使えます）",
        file: "gemma-4-E4B-it-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/gemma-4-E4B-it-GGUF/resolve/main/gemma-4-E4B-it-Q4_K_M.gguf",
        size: 4_977_171_584,
        sha256: "85a896a047553e842f25297ee5b031d64ff30147d9c4af17b1e4b394cd1fab87",
        license: "Apache-2.0",
        license_url: "https://ai.google.dev/gemma/apache_2",
        style: PromptStyle::ChatTemplate,
    },
    CatalogModel {
        id: "gemma-4-e2b",
        label: "Gemma 4 E2B",
        note: "軽量な汎用モデル。説明・要約を速く、品質は E4B より下がります",
        file: "gemma-4-E2B-it-Q4_K_M.gguf",
        url: "https://huggingface.co/unsloth/gemma-4-E2B-it-GGUF/resolve/main/gemma-4-E2B-it-Q4_K_M.gguf",
        size: 3_106_738_272,
        sha256: "740185b21d22ceb83a11c3aa62ad5842ef32c70f6096d756bbee85a1e4ec34b8",
        license: "Apache-2.0",
        license_url: "https://ai.google.dev/gemma/apache_2",
        style: PromptStyle::ChatTemplate,
    },
];

pub fn find(id: &str) -> Option<&'static CatalogModel> {
    CATALOG.iter().find(|m| m.id == id)
}

/// Where GGUF files live: `<data dir>/moonlens/models/llm`.
pub fn default_models_dir() -> Option<PathBuf> {
    super::ocr_ocrs::default_models_dir().map(|d| d.join("llm"))
}

pub fn model_path(dir: &Path, m: &CatalogModel) -> PathBuf {
    dir.join(m.file)
}

fn part_path(dir: &Path, m: &CatalogModel) -> PathBuf {
    dir.join(format!("{}.part", m.file))
}

/// A model counts as installed when the final file exists with the
/// pinned size. The hash was checked before the rename, so re-hashing
/// gigabytes on every launch is unnecessary.
pub fn is_installed(dir: &Path, m: &CatalogModel) -> bool {
    std::fs::metadata(model_path(dir, m))
        .map(|md| md.len() == m.size)
        .unwrap_or(false)
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
            partial_bytes: std::fs::metadata(part_path(dir, m))
                .map(|md| md.len())
                .unwrap_or(0),
            downloading: active().lock().contains_key(m.id),
        })
        .collect()
}

/// Cancel flags of downloads in flight, keyed by model id.
fn active() -> &'static Mutex<HashMap<&'static str, Arc<AtomicBool>>> {
    static ACTIVE: OnceLock<Mutex<HashMap<&'static str, Arc<AtomicBool>>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Ask an in-flight download to stop. The `.part` file is kept so the
/// next attempt resumes. Returns false when nothing was downloading.
pub fn cancel(id: &str) -> bool {
    match active().lock().get(id) {
        Some(flag) => {
            flag.store(true, Ordering::SeqCst);
            true
        }
        None => false,
    }
}

/// Remove an installed model and any partial download.
pub fn remove(dir: &Path, id: &str) -> anyhow::Result<()> {
    let m = find(id).ok_or_else(|| anyhow::anyhow!("unknown model {id}"))?;
    cancel(id);
    // Drop the loaded weights first: Windows refuses to delete a file
    // that is still memory-mapped.
    super::translate_llama::unload(&model_path(dir, m));
    for p in [model_path(dir, m), part_path(dir, m)] {
        match std::fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// `progress(model_id, downloaded_bytes, total_bytes)`.
pub type ProgressCb = dyn Fn(&str, u64, u64) + Send + Sync;

/// Download, verify and install `id` into `dir`. Resumes a previous
/// partial download. No-op when already installed.
pub async fn install(
    dir: &Path,
    id: &str,
    progress: Option<&ProgressCb>,
) -> anyhow::Result<PathBuf> {
    use futures::StreamExt;

    let m = find(id).ok_or_else(|| anyhow::anyhow!("unknown model {id}"))?;
    let target = model_path(dir, m);
    if is_installed(dir, m) {
        return Ok(target);
    }
    std::fs::create_dir_all(dir)?;

    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut act = active().lock();
        if act.contains_key(m.id) {
            anyhow::bail!("{} is already downloading", m.label);
        }
        act.insert(m.id, cancel_flag.clone());
    }
    // Deregister however this function exits.
    struct Done(&'static str);
    impl Drop for Done {
        fn drop(&mut self) {
            active().lock().remove(self.0);
        }
    }
    let _done = Done(m.id);

    let part = part_path(dir, m);
    // Hash whatever an earlier attempt left behind so the final digest
    // covers the whole file.
    let (mut hasher, mut have) = {
        let part = part.clone();
        let size = m.size;
        tokio::task::spawn_blocking(move || hash_existing(&part, size)).await??
    };

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()?;
    let mut req = client.get(m.url);
    if have > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let resp = req.send().await?.error_for_status()?;
    if have > 0 && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        // Server ignored the range: start over.
        hasher = Sha256::new();
        have = 0;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(have == 0)
        .append(have > 0)
        .open(&part)?;

    let mut stream = resp.bytes_stream();
    let mut last_report = 0u64;
    while let Some(chunk) = stream.next().await {
        if cancel_flag.load(Ordering::SeqCst) {
            anyhow::bail!("cancelled");
        }
        let chunk = chunk?;
        have += chunk.len() as u64;
        if have > m.size {
            drop(file);
            let _ = std::fs::remove_file(&part);
            anyhow::bail!("{}: server sent more data than expected", m.label);
        }
        hasher.update(&chunk);
        file.write_all(&chunk)?;
        if let Some(cb) = progress {
            // ~every 4 MB is plenty for a progress bar.
            if have - last_report >= 4 << 20 || have == m.size {
                last_report = have;
                cb(m.id, have, m.size);
            }
        }
    }
    file.flush()?;
    drop(file);

    if have != m.size {
        anyhow::bail!(
            "{}: download incomplete ({have} of {} bytes); retry to resume",
            m.label,
            m.size
        );
    }
    let digest = hex(&hasher.finalize());
    if digest != m.sha256 {
        let _ = std::fs::remove_file(&part);
        anyhow::bail!("{} failed its integrity check", m.label);
    }
    std::fs::rename(&part, &target)?;
    Ok(target)
}

/// Feed an existing partial file into a fresh hasher. A part file that is
/// larger than the model is corrupt and gets discarded.
fn hash_existing(part: &Path, size: u64) -> std::io::Result<(Sha256, u64)> {
    let mut hasher = Sha256::new();
    let mut f = match std::fs::File::open(part) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((hasher, 0)),
        Err(e) => return Err(e),
    };
    if f.metadata()?.len() > size {
        drop(f);
        std::fs::remove_file(part)?;
        return Ok((hasher, 0));
    }
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let r = f.read(&mut buf)?;
        if r == 0 {
            break;
        }
        hasher.update(&buf[..r]);
        n += r as u64;
    }
    Ok((hasher, n))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
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
    fn installed_requires_exact_size() {
        let dir = tempfile::tempdir().unwrap();
        let m = find(DEFAULT_TRANSLATE_MODEL_ID).unwrap();
        assert!(!is_installed(dir.path(), m));
        std::fs::write(model_path(dir.path(), m), b"short").unwrap();
        assert!(!is_installed(dir.path(), m));
    }

    #[test]
    fn hash_existing_resumes_digest() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.part");
        std::fs::write(&p, b"hello ").unwrap();
        let (mut h, n) = hash_existing(&p, 100).unwrap();
        assert_eq!(n, 6);
        h.update(b"world");
        assert_eq!(hex(&h.finalize()), hex(&Sha256::digest(b"hello world")));
    }

    #[test]
    fn oversized_part_is_discarded() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.part");
        std::fs::write(&p, b"0123456789").unwrap();
        let (_, n) = hash_existing(&p, 4).unwrap();
        assert_eq!(n, 0);
        assert!(!p.exists());
    }

    #[test]
    fn missing_part_starts_from_zero() {
        let dir = tempfile::tempdir().unwrap();
        let (_, n) = hash_existing(&dir.path().join("nope.part"), 4).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn status_lists_every_model() {
        let dir = tempfile::tempdir().unwrap();
        let s = status(dir.path());
        assert_eq!(s.len(), CATALOG.len());
        assert!(s.iter().all(|m| !m.installed && m.partial_bytes == 0));
    }
}
