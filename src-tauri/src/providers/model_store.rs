//! Download, verify and install pinned model files.
//!
//! Shared by the model catalogs ([`super::llm_models`] for llama.cpp,
//! [`super::whisper_models`] for whisper.cpp). Every file is pinned by
//! URL + size + SHA-256, so a model is only ever loaded after its bytes
//! were verified. Downloads stream to a `.part` file next to the target
//! (multi-GB files never sit in RAM), resume with an HTTP Range request
//! after an interruption, and are renamed into place only once the hash
//! matches.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use sha2::{Digest, Sha256};

/// One downloadable file. `id` must be unique across all catalogs: it
/// keys the in-flight download table.
#[derive(Debug, Clone, Copy)]
pub struct PinnedFile {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

pub fn model_path(dir: &Path, m: &PinnedFile) -> PathBuf {
    dir.join(m.file)
}

pub fn part_path(dir: &Path, m: &PinnedFile) -> PathBuf {
    dir.join(format!("{}.part", m.file))
}

/// A model counts as installed when the final file exists with the
/// pinned size. The hash was checked before the rename, so re-hashing
/// gigabytes on every launch is unnecessary.
pub fn is_installed(dir: &Path, m: &PinnedFile) -> bool {
    std::fs::metadata(model_path(dir, m))
        .map(|md| md.len() == m.size)
        .unwrap_or(false)
}

/// Bytes of an interrupted download that a retry will resume from.
pub fn partial_bytes(dir: &Path, m: &PinnedFile) -> u64 {
    std::fs::metadata(part_path(dir, m))
        .map(|md| md.len())
        .unwrap_or(0)
}

/// Cancel flags of downloads in flight, keyed by model id.
fn active() -> &'static Mutex<HashMap<&'static str, Arc<AtomicBool>>> {
    static ACTIVE: OnceLock<Mutex<HashMap<&'static str, Arc<AtomicBool>>>> = OnceLock::new();
    ACTIVE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn is_downloading(id: &str) -> bool {
    active().lock().contains_key(id)
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

/// Headroom left free after a download, so the disk is never filled to
/// the last byte.
pub const FREE_SPACE_MARGIN: u64 = 512 << 20;

/// Bytes a download of `m` still has to fetch (resuming any `.part`).
pub fn bytes_still_needed(dir: &Path, m: &PinnedFile) -> u64 {
    if is_installed(dir, m) {
        return 0;
    }
    m.size.saturating_sub(partial_bytes(dir, m))
}

/// Free space on the volume that holds `dir` (or its nearest existing
/// parent). `None` when it cannot be determined.
pub fn free_space(dir: &Path) -> Option<u64> {
    let mut p = dir;
    loop {
        if p.exists() {
            return fs4::available_space(p).ok();
        }
        p = p.parent()?;
    }
}

/// Delete the installed file and any partial download of `m`.
pub fn remove_files(dir: &Path, m: &PinnedFile) -> anyhow::Result<()> {
    cancel(m.id);
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

/// Download, verify and install `m` into `dir`. Resumes a previous
/// partial download. No-op when already installed.
pub async fn install(
    dir: &Path,
    m: &PinnedFile,
    progress: Option<&ProgressCb>,
) -> anyhow::Result<PathBuf> {
    use futures::StreamExt;

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

    const F: PinnedFile = PinnedFile {
        id: "test-model",
        label: "Test",
        file: "test.bin",
        url: "https://example.invalid/test.bin",
        size: 4096,
        sha256: "00",
    };

    #[test]
    fn installed_requires_exact_size() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_installed(dir.path(), &F));
        std::fs::write(model_path(dir.path(), &F), b"short").unwrap();
        assert!(!is_installed(dir.path(), &F));
        std::fs::write(model_path(dir.path(), &F), vec![0u8; 4096]).unwrap();
        assert!(is_installed(dir.path(), &F));
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
    fn still_needed_accounts_for_partial_downloads() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(bytes_still_needed(dir.path(), &F), F.size);
        std::fs::write(part_path(dir.path(), &F), vec![0u8; 1000]).unwrap();
        assert_eq!(partial_bytes(dir.path(), &F), 1000);
        assert_eq!(bytes_still_needed(dir.path(), &F), F.size - 1000);
        assert!(free_space(&dir.path().join("does/not/exist")).is_some());
    }

    #[test]
    fn remove_files_deletes_model_and_part() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(model_path(dir.path(), &F), b"x").unwrap();
        std::fs::write(part_path(dir.path(), &F), b"x").unwrap();
        remove_files(dir.path(), &F).unwrap();
        assert!(!model_path(dir.path(), &F).exists());
        assert!(!part_path(dir.path(), &F).exists());
        // Nothing left: still fine.
        remove_files(dir.path(), &F).unwrap();
    }
}
