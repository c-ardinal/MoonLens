//! Adapter over the model files directory.
//!
//! The pre-7.x `embedded_model_status` / `download_embedded_model` /
//! `apply_embedded_model_package` IPCs all hand-rolled `std::fs`
//! calls inline. Tests therefore could not exercise:
//!
//! * "what does status look like when the file is half-written?"
//!   (a `.partial` left by an aborted download)
//! * "import overwrites a stale GGUF without leaving the original
//!   in place if the rename fails"
//! * "downloader writes outside the models dir" (regression guard)
//!
//! [`ModelStorage`] gives us a seam: production points it at the
//! Tauri app-data dir, tests use [`InMemoryStorage`] (a HashMap with
//! the same invariants).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("path is not under the storage root: {0}")]
    OutsideRoot(PathBuf),
}

/// Metadata for a single model file. Pre-computed so the IPC layer
/// can answer "is this present, and how big is it?" in one round-trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFileMeta {
    /// Absolute path the file would live at, regardless of presence.
    pub path: PathBuf,
    /// Bytes already written to disk. `0` when absent.
    pub size_bytes: u64,
    /// Whether the file exists.
    pub exists: bool,
}

/// Storage abstraction. All paths supplied to instances of this trait
/// MUST be under [`models_dir`](Self::models_dir) — implementations
/// may reject foreign paths with `StorageError::OutsideRoot`. This
/// is a defence-in-depth against a future bug where the downloader
/// is asked to write to `..\..\..\Windows\System32\evil.gguf`.
#[async_trait]
pub trait ModelStorage: Send + Sync {
    /// Root directory the storage is rooted at. The IPC layer surfaces
    /// this path in error messages.
    fn models_dir(&self) -> &Path;

    /// Resolve a filename to an absolute path under
    /// [`models_dir`](Self::models_dir). Pure function — no IO.
    fn resolve(&self, filename: &str) -> PathBuf {
        self.models_dir().join(filename)
    }

    /// Stat a file by filename. Returns metadata even for absent
    /// files (with `exists = false`) so callers do not have to
    /// branch.
    async fn meta(&self, filename: &str) -> Result<ModelFileMeta, StorageError>;

    /// Make sure the storage root exists. Idempotent — safe to call
    /// from every download.
    async fn ensure_dir(&self) -> Result<(), StorageError>;

    /// Write a stream of bytes into the storage atomically. The
    /// bytes are first written to a sibling `.partial` file and the
    /// final rename only happens after `commit()` returns Ok. On
    /// drop without commit (e.g. panicked download), the partial
    /// file is removed.
    ///
    /// Implementations must verify the dest path is under the
    /// storage root.
    async fn open_write(
        &self,
        filename: &str,
    ) -> Result<Box<dyn ModelWrite + Send>, StorageError>;

    /// Atomically swap a separately-prepared file (e.g. extracted
    /// from a package) into the storage.  `source` must already be
    /// fully written. Used by the package importer where the
    /// streaming hash + write happens before the storage acquires
    /// the file.
    async fn import_from(&self, source: &Path, filename: &str) -> Result<PathBuf, StorageError>;

    /// Remove a file by filename. Returns Ok if the file was absent.
    async fn delete(&self, filename: &str) -> Result<(), StorageError>;
}

/// Streaming writer handle returned by [`ModelStorage::open_write`].
/// The trait is async because the production `tokio::fs::File` write
/// path is async; the test impl synchronously appends to a Vec and
/// just spells the API consistently.
#[async_trait]
pub trait ModelWrite {
    /// Append a chunk to the underlying writer.
    async fn write_all(&mut self, data: &[u8]) -> Result<(), StorageError>;

    /// Atomically swap the partial file to its final destination
    /// and consume the handle. Subsequent writes are not possible.
    async fn commit(self: Box<Self>) -> Result<PathBuf, StorageError>;

    /// Discard the partial file without finalising. Idempotent — a
    /// commit followed by discard is a no-op.
    async fn discard(self: Box<Self>) -> Result<(), StorageError>;
}

/// Production storage backed by [`tokio::fs`].
#[derive(Debug, Clone)]
pub struct FileSystemStorage {
    root: PathBuf,
}

impl FileSystemStorage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

#[async_trait]
impl ModelStorage for FileSystemStorage {
    fn models_dir(&self) -> &Path {
        &self.root
    }

    async fn meta(&self, filename: &str) -> Result<ModelFileMeta, StorageError> {
        let path = self.resolve(filename);
        match tokio::fs::metadata(&path).await {
            Ok(m) if m.is_file() => Ok(ModelFileMeta {
                path,
                size_bytes: m.len(),
                exists: true,
            }),
            _ => Ok(ModelFileMeta {
                path,
                size_bytes: 0,
                exists: false,
            }),
        }
    }

    async fn ensure_dir(&self) -> Result<(), StorageError> {
        tokio::fs::create_dir_all(&self.root).await?;
        Ok(())
    }

    async fn open_write(
        &self,
        filename: &str,
    ) -> Result<Box<dyn ModelWrite + Send>, StorageError> {
        if filename.contains(['/', '\\']) {
            return Err(StorageError::OutsideRoot(PathBuf::from(filename)));
        }
        self.ensure_dir().await?;
        let final_path = self.resolve(filename);
        let temp_path = final_path.with_extension("gguf.partial");
        let file = tokio::fs::File::create(&temp_path).await?;
        Ok(Box::new(FsModelWrite {
            file: Some(tokio::io::BufWriter::new(file)),
            temp_path,
            final_path,
        }))
    }

    async fn import_from(&self, source: &Path, filename: &str) -> Result<PathBuf, StorageError> {
        if filename.contains(['/', '\\']) {
            return Err(StorageError::OutsideRoot(PathBuf::from(filename)));
        }
        self.ensure_dir().await?;
        let final_path = self.resolve(filename);
        let _ = tokio::fs::remove_file(&final_path).await;
        tokio::fs::rename(source, &final_path).await?;
        Ok(final_path)
    }

    async fn delete(&self, filename: &str) -> Result<(), StorageError> {
        let path = self.resolve(filename);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

struct FsModelWrite {
    /// `Option` so `commit`/`discard` can take ownership and flush
    /// without leaving a half-state.
    file: Option<tokio::io::BufWriter<tokio::fs::File>>,
    temp_path: PathBuf,
    final_path: PathBuf,
}

#[async_trait]
impl ModelWrite for FsModelWrite {
    async fn write_all(&mut self, data: &[u8]) -> Result<(), StorageError> {
        use tokio::io::AsyncWriteExt;
        let file = self
            .file
            .as_mut()
            .expect("write_all after commit/discard");
        file.write_all(data).await?;
        Ok(())
    }

    async fn commit(mut self: Box<Self>) -> Result<PathBuf, StorageError> {
        use tokio::io::AsyncWriteExt;
        if let Some(mut file) = self.file.take() {
            file.flush().await?;
            drop(file);
        }
        let _ = tokio::fs::remove_file(&self.final_path).await;
        tokio::fs::rename(&self.temp_path, &self.final_path).await?;
        Ok(self.final_path.clone())
    }

    async fn discard(mut self: Box<Self>) -> Result<(), StorageError> {
        if let Some(file) = self.file.take() {
            drop(file);
        }
        match tokio::fs::remove_file(&self.temp_path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

impl Drop for FsModelWrite {
    fn drop(&mut self) {
        // If the caller drops without committing, sweep the
        // partial file so it does not haunt the next download.
        if self.file.is_some() {
            let _ = std::fs::remove_file(&self.temp_path);
        }
    }
}

#[cfg(test)]
mod in_memory {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// In-memory storage backed by a HashMap. Tests construct one
    /// per test so state never leaks. Mirrors the real impl's
    /// invariants:
    /// * `open_write` produces a partial; only `commit` makes it
    ///   visible to `meta`,
    /// * `discard` and `Drop` both clean up the partial,
    /// * `import_from` reads from the real filesystem (used by the
    ///   package import test where the package extractor wrote a
    ///   tempfile via `tempfile::tempdir`).
    #[derive(Default, Clone)]
    pub struct InMemoryStorage {
        files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        root: PathBuf,
    }

    impl InMemoryStorage {
        pub fn new() -> Self {
            Self {
                files: Arc::new(Mutex::new(HashMap::new())),
                root: PathBuf::from("/in-memory/moonlens/models"),
            }
        }

        pub fn with_file(self, filename: &str, data: Vec<u8>) -> Self {
            self.files.lock().insert(filename.to_string(), data);
            self
        }

        pub fn snapshot(&self) -> HashMap<String, Vec<u8>> {
            self.files.lock().clone()
        }
    }

    #[async_trait]
    impl ModelStorage for InMemoryStorage {
        fn models_dir(&self) -> &Path {
            &self.root
        }

        async fn meta(&self, filename: &str) -> Result<ModelFileMeta, StorageError> {
            let map = self.files.lock();
            match map.get(filename) {
                Some(bytes) => Ok(ModelFileMeta {
                    path: self.resolve(filename),
                    size_bytes: bytes.len() as u64,
                    exists: true,
                }),
                None => Ok(ModelFileMeta {
                    path: self.resolve(filename),
                    size_bytes: 0,
                    exists: false,
                }),
            }
        }

        async fn ensure_dir(&self) -> Result<(), StorageError> {
            Ok(())
        }

        async fn open_write(
            &self,
            filename: &str,
        ) -> Result<Box<dyn ModelWrite + Send>, StorageError> {
            if filename.contains(['/', '\\']) {
                return Err(StorageError::OutsideRoot(PathBuf::from(filename)));
            }
            Ok(Box::new(InMemoryWrite {
                files: self.files.clone(),
                filename: filename.to_string(),
                buffer: Vec::new(),
                committed: false,
                final_path: self.resolve(filename),
            }))
        }

        async fn import_from(
            &self,
            source: &Path,
            filename: &str,
        ) -> Result<PathBuf, StorageError> {
            if filename.contains(['/', '\\']) {
                return Err(StorageError::OutsideRoot(PathBuf::from(filename)));
            }
            let bytes = tokio::fs::read(source).await?;
            self.files.lock().insert(filename.to_string(), bytes);
            Ok(self.resolve(filename))
        }

        async fn delete(&self, filename: &str) -> Result<(), StorageError> {
            self.files.lock().remove(filename);
            Ok(())
        }
    }

    struct InMemoryWrite {
        files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        filename: String,
        buffer: Vec<u8>,
        committed: bool,
        final_path: PathBuf,
    }

    #[async_trait]
    impl ModelWrite for InMemoryWrite {
        async fn write_all(&mut self, data: &[u8]) -> Result<(), StorageError> {
            self.buffer.extend_from_slice(data);
            Ok(())
        }

        async fn commit(mut self: Box<Self>) -> Result<PathBuf, StorageError> {
            self.files
                .lock()
                .insert(std::mem::take(&mut self.filename), std::mem::take(&mut self.buffer));
            self.committed = true;
            Ok(self.final_path.clone())
        }

        async fn discard(self: Box<Self>) -> Result<(), StorageError> {
            // The buffer is dropped; the file is never inserted into
            // the map, so the partial is invisible to `meta`.
            Ok(())
        }
    }
}

#[cfg(test)]
pub use in_memory::InMemoryStorage;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::model_storage::InMemoryStorage;

    #[tokio::test]
    async fn meta_for_absent_file_returns_exists_false() {
        let s = InMemoryStorage::new();
        let m = s.meta("ghost.gguf").await.unwrap();
        assert!(!m.exists);
        assert_eq!(m.size_bytes, 0);
    }

    #[tokio::test]
    async fn write_then_commit_makes_meta_observable() {
        let s = InMemoryStorage::new();
        let mut w = s.open_write("model.gguf").await.unwrap();
        w.write_all(b"abc").await.unwrap();
        w.write_all(b"def").await.unwrap();
        w.commit().await.unwrap();
        let m = s.meta("model.gguf").await.unwrap();
        assert!(m.exists);
        assert_eq!(m.size_bytes, 6);
    }

    #[tokio::test]
    async fn discard_keeps_meta_invisible() {
        let s = InMemoryStorage::new();
        let mut w = s.open_write("model.gguf").await.unwrap();
        w.write_all(b"oops").await.unwrap();
        w.discard().await.unwrap();
        let m = s.meta("model.gguf").await.unwrap();
        assert!(!m.exists);
    }

    #[tokio::test]
    async fn open_write_rejects_path_separators() {
        let s = InMemoryStorage::new();
        match s.open_write("../evil.gguf").await {
            Err(StorageError::OutsideRoot(_)) => {}
            other => panic!("expected OutsideRoot, got {:?}", other.err()),
        }
        match s.open_write("subdir\\file.gguf").await {
            Err(StorageError::OutsideRoot(_)) => {}
            other => panic!("expected OutsideRoot, got {:?}", other.err()),
        }
    }

    #[tokio::test]
    async fn import_from_real_file_writes_into_storage() {
        let s = InMemoryStorage::new();
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("source.gguf");
        tokio::fs::write(&src, b"package contents").await.unwrap();
        let final_path = s.import_from(&src, "model.gguf").await.unwrap();
        assert_eq!(final_path, s.resolve("model.gguf"));
        let snap = s.snapshot();
        assert_eq!(snap.get("model.gguf").map(|v| v.as_slice()), Some(b"package contents".as_ref()));
    }

    #[tokio::test]
    async fn delete_is_idempotent_for_absent_files() {
        let s = InMemoryStorage::new();
        s.delete("ghost.gguf").await.unwrap();
        s.delete("ghost.gguf").await.unwrap();
    }

    #[tokio::test]
    async fn fs_storage_round_trip_through_real_disk() {
        // Sanity check the production impl. Uses tempfile so the
        // test does not pollute the user's actual app-data dir.
        let tmp = tempfile::tempdir().unwrap();
        let s = FileSystemStorage::new(tmp.path().to_path_buf());
        s.ensure_dir().await.unwrap();
        let mut w = s.open_write("model.gguf").await.unwrap();
        w.write_all(b"hello").await.unwrap();
        let path = w.commit().await.unwrap();
        assert!(path.is_file());
        let m = s.meta("model.gguf").await.unwrap();
        assert!(m.exists);
        assert_eq!(m.size_bytes, 5);
        s.delete("model.gguf").await.unwrap();
        assert!(!s.meta("model.gguf").await.unwrap().exists);
    }

    #[tokio::test]
    async fn fs_storage_discard_removes_partial_file() {
        let tmp = tempfile::tempdir().unwrap();
        let s = FileSystemStorage::new(tmp.path().to_path_buf());
        let mut w = s.open_write("model.gguf").await.unwrap();
        w.write_all(b"junk").await.unwrap();
        w.discard().await.unwrap();
        // No final file, no partial file.
        assert!(!s.meta("model.gguf").await.unwrap().exists);
        let partial = tmp.path().join("model.gguf.partial");
        assert!(!partial.exists(), "partial file must be cleaned up on discard");
    }
}
