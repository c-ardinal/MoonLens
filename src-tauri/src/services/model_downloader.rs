//! Adapter over the model fetch path.
//!
//! Pre-7.x `download_embedded_model` opened a `reqwest::Client`,
//! streamed bytes into `tokio::fs`, emitted Tauri events, and
//! computed sha256 — all in one IPC body. Tests for "what happens
//! when the network drops at 80 %" needed a live HuggingFace mirror
//! that misbehaved on cue.
//!
//! [`ModelDownloader`] encapsulates "fetch this URL into this storage
//! slot, with progress callbacks and cancellation". Production wraps
//! `reqwest`; tests use [`ScriptedDownloader`] which can be
//! programmed to fail mid-stream, return wrong-length bytes, or
//! satisfy the request from canned fixtures.

use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use serde::Serialize;
use thiserror::Error;

use crate::services::model_storage::{ModelStorage, StorageError};

/// Per-chunk progress payload. Mirrors the Tauri event the IPC layer
/// emits, without the `filename` field (the IPC adapter adds that).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// User-supplied progress sink. Boxed because it crosses the trait
/// boundary; cheap to clone (Arc).
pub type ProgressCallback = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

/// Inputs to a download. Built by the service layer from the
/// `[providers.configs."embedded.llm"]` snapshot — the trait does
/// NOT know about `repo` / `filename_remote`, only an absolute URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadRequest {
    pub url: String,
    /// Filename inside the storage. The downloader passes this to
    /// `storage.open_write` and never composes paths itself.
    pub filename: String,
}

/// Outcome of a successful download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResult {
    /// Final on-disk path (the storage's `resolve(filename)`).
    pub path: std::path::PathBuf,
    /// Bytes written.
    pub size_bytes: u64,
}

#[derive(Debug, Error)]
pub enum ModelDownloaderError {
    #[error("http: {0}")]
    Http(String),
    #[error("server returned status {0}")]
    HttpStatus(u16),
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("cancelled")]
    Cancelled,
}

/// One-shot download. Implementations are expected to:
/// 1. Open a writer via the supplied `ModelStorage` (so paths stay
///    rooted, partial files get atomic-rename semantics, etc.),
/// 2. Stream bytes from the remote source,
/// 3. Fire `on_progress` at a reasonable cadence (≤ 10 Hz),
/// 4. Commit the writer on success, discard on failure.
#[async_trait]
pub trait ModelDownloader: Send + Sync {
    async fn download(
        &self,
        request: DownloadRequest,
        storage: &(dyn ModelStorage + Send + Sync),
        on_progress: Option<ProgressCallback>,
    ) -> Result<DownloadResult, ModelDownloaderError>;
}

/// Production downloader backed by `reqwest`. Throttles progress
/// emission to once every 100 ms so a slow 1 GB download does not
/// flood the IPC channel with per-chunk events.
#[derive(Debug, Clone)]
pub struct HttpModelDownloader {
    http: reqwest::Client,
}

impl HttpModelDownloader {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60 * 60))
            .build()
            .expect("reqwest client builder must succeed with default options");
        Self { http }
    }

    pub fn with_client(http: reqwest::Client) -> Self {
        Self { http }
    }
}

impl Default for HttpModelDownloader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ModelDownloader for HttpModelDownloader {
    async fn download(
        &self,
        request: DownloadRequest,
        storage: &(dyn ModelStorage + Send + Sync),
        on_progress: Option<ProgressCallback>,
    ) -> Result<DownloadResult, ModelDownloaderError> {
        use futures::StreamExt;
        let resp = self
            .http
            .get(&request.url)
            .send()
            .await
            .map_err(|e| ModelDownloaderError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(ModelDownloaderError::HttpStatus(resp.status().as_u16()));
        }
        let total = resp.content_length();

        let mut writer = storage.open_write(&request.filename).await?;
        let mut downloaded: u64 = 0;
        let mut last_emit = std::time::Instant::now();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ModelDownloaderError::Http(e.to_string()))?;
            writer.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            if let Some(cb) = on_progress.as_ref() {
                if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
                    cb(DownloadProgress {
                        downloaded,
                        total,
                    });
                    last_emit = std::time::Instant::now();
                }
            }
        }
        let path = writer.commit().await?;
        if let Some(cb) = on_progress.as_ref() {
            cb(DownloadProgress {
                downloaded,
                total,
            });
        }
        Ok(DownloadResult {
            path,
            size_bytes: downloaded,
        })
    }
}

/// Test downloader. Configurable per call to simulate the network
/// scenarios that production has to handle.
#[cfg(test)]
pub use scripted::{ScriptedDownloader, ScriptedResponse};

#[cfg(test)]
pub mod scripted {
    use super::*;
    use parking_lot::Mutex;
    use std::collections::HashMap;

    /// One canned response. Bytes are streamed in `chunk_size`-sized
    /// chunks so tests can reason about the progress cadence; setting
    /// `fail_after` truncates the stream at that byte offset and the
    /// downloader returns a transport error (mimics a flaky network).
    #[derive(Debug, Clone)]
    pub struct ScriptedResponse {
        pub bytes: Vec<u8>,
        pub chunk_size: usize,
        pub fail_after: Option<u64>,
        pub status: u16,
    }

    impl ScriptedResponse {
        pub fn ok(bytes: impl Into<Vec<u8>>) -> Self {
            Self {
                bytes: bytes.into(),
                chunk_size: 1024,
                fail_after: None,
                status: 200,
            }
        }

        pub fn fail_at(bytes: impl Into<Vec<u8>>, fail_after: u64) -> Self {
            Self {
                bytes: bytes.into(),
                chunk_size: 1024,
                fail_after: Some(fail_after),
                status: 200,
            }
        }

        pub fn http_error(status: u16) -> Self {
            Self {
                bytes: Vec::new(),
                chunk_size: 1024,
                fail_after: None,
                status,
            }
        }

        pub fn with_chunk_size(mut self, chunk_size: usize) -> Self {
            self.chunk_size = chunk_size.max(1);
            self
        }
    }

    /// Test-only downloader. Construct with [`ScriptedDownloader::new`],
    /// register canned responses with [`Self::respond_to`], then hand
    /// off to the system under test as `Arc<dyn ModelDownloader>`.
    #[derive(Default)]
    pub struct ScriptedDownloader {
        responses: Mutex<HashMap<String, ScriptedResponse>>,
        progress_log: Mutex<Vec<DownloadProgress>>,
        urls_called: Mutex<Vec<String>>,
    }

    impl ScriptedDownloader {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn respond_to(self, url: impl Into<String>, response: ScriptedResponse) -> Self {
            self.responses.lock().insert(url.into(), response);
            self
        }

        pub fn progress_log(&self) -> Vec<DownloadProgress> {
            self.progress_log.lock().clone()
        }

        pub fn urls_called(&self) -> Vec<String> {
            self.urls_called.lock().clone()
        }
    }

    #[async_trait]
    impl ModelDownloader for ScriptedDownloader {
        async fn download(
            &self,
            request: DownloadRequest,
            storage: &(dyn ModelStorage + Send + Sync),
            on_progress: Option<ProgressCallback>,
        ) -> Result<DownloadResult, ModelDownloaderError> {
            self.urls_called.lock().push(request.url.clone());
            let response = self
                .responses
                .lock()
                .get(&request.url)
                .cloned()
                .ok_or_else(|| {
                    ModelDownloaderError::Http(format!(
                        "no scripted response for {}",
                        request.url
                    ))
                })?;
            if response.status >= 400 {
                return Err(ModelDownloaderError::HttpStatus(response.status));
            }
            let total = Some(response.bytes.len() as u64);
            let mut writer = storage.open_write(&request.filename).await?;
            let mut downloaded: u64 = 0;
            for chunk in response.bytes.chunks(response.chunk_size) {
                if let Some(limit) = response.fail_after {
                    if downloaded >= limit {
                        // discard the partial — production behaviour
                        // when the writer is dropped without commit.
                        writer.discard().await.ok();
                        return Err(ModelDownloaderError::Http(format!(
                            "scripted failure after {} bytes",
                            downloaded
                        )));
                    }
                }
                writer.write_all(chunk).await?;
                downloaded += chunk.len() as u64;
                if let Some(cb) = on_progress.as_ref() {
                    let p = DownloadProgress {
                        downloaded,
                        total,
                    };
                    cb(p);
                    self.progress_log.lock().push(p);
                }
            }
            let path = writer.commit().await?;
            Ok(DownloadResult {
                path,
                size_bytes: downloaded,
            })
        }
    }
}

// Suppress warnings about the unused `Stream` / `Bytes` imports —
// they are referenced indirectly via the `bytes_stream` consumption
// in the production impl, but get flagged on test-only builds.
#[allow(dead_code)]
fn _module_keeps_symbols_referenced() {
    let _: Option<Bytes> = None;
    fn _shape<S: Stream<Item = ()>>(_: S) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::model_storage::InMemoryStorage;

    #[tokio::test]
    async fn happy_path_writes_full_payload_into_storage() {
        let storage = InMemoryStorage::new();
        let dl = ScriptedDownloader::new()
            .respond_to("https://example.test/m.gguf", scripted::ScriptedResponse::ok(vec![1u8; 4096]));
        let req = DownloadRequest {
            url: "https://example.test/m.gguf".into(),
            filename: "m.gguf".into(),
        };
        let res = dl.download(req, &storage, None).await.unwrap();
        assert_eq!(res.size_bytes, 4096);
        let snap = storage.snapshot();
        assert_eq!(snap.get("m.gguf").map(|v| v.len()), Some(4096));
    }

    #[tokio::test]
    async fn progress_callback_fires_with_expected_total() {
        let storage = InMemoryStorage::new();
        let dl = ScriptedDownloader::new().respond_to(
            "https://example.test/m.gguf",
            scripted::ScriptedResponse::ok(vec![7u8; 4 * 1024]).with_chunk_size(1024),
        );
        let captured: std::sync::Arc<parking_lot::Mutex<Vec<DownloadProgress>>> =
            std::sync::Arc::new(parking_lot::Mutex::new(Vec::new()));
        let captured_for_cb = captured.clone();
        let cb: ProgressCallback = std::sync::Arc::new(move |p| {
            captured_for_cb.lock().push(p);
        });
        let req = DownloadRequest {
            url: "https://example.test/m.gguf".into(),
            filename: "m.gguf".into(),
        };
        dl.download(req, &storage, Some(cb)).await.unwrap();
        let log = captured.lock().clone();
        assert!(log.len() >= 4, "expected at least one event per chunk");
        let last = log.last().unwrap();
        assert_eq!(last.downloaded, 4 * 1024);
        assert_eq!(last.total, Some(4 * 1024));
    }

    #[tokio::test]
    async fn mid_stream_failure_discards_partial_and_returns_error() {
        let storage = InMemoryStorage::new();
        let dl = ScriptedDownloader::new().respond_to(
            "https://example.test/m.gguf",
            scripted::ScriptedResponse::fail_at(vec![5u8; 8 * 1024], 4 * 1024)
                .with_chunk_size(1024),
        );
        let req = DownloadRequest {
            url: "https://example.test/m.gguf".into(),
            filename: "m.gguf".into(),
        };
        let err = dl.download(req, &storage, None).await.expect_err("must fail");
        assert!(matches!(err, ModelDownloaderError::Http(_)));
        // Storage MUST not show a partial file — discard cleaned up.
        assert!(!storage.meta("m.gguf").await.unwrap().exists);
    }

    #[tokio::test]
    async fn http_404_propagates_as_status_error() {
        let storage = InMemoryStorage::new();
        let dl = ScriptedDownloader::new().respond_to(
            "https://example.test/missing.gguf",
            scripted::ScriptedResponse::http_error(404),
        );
        let req = DownloadRequest {
            url: "https://example.test/missing.gguf".into(),
            filename: "m.gguf".into(),
        };
        let err = dl.download(req, &storage, None).await.expect_err("must fail");
        assert!(matches!(err, ModelDownloaderError::HttpStatus(404)));
    }

    #[tokio::test]
    async fn unconfigured_url_yields_a_clear_error() {
        let storage = InMemoryStorage::new();
        let dl = ScriptedDownloader::new();
        let req = DownloadRequest {
            url: "https://example.test/never-registered".into(),
            filename: "m.gguf".into(),
        };
        let err = dl.download(req, &storage, None).await.expect_err("must fail");
        match err {
            ModelDownloaderError::Http(msg) => assert!(msg.contains("no scripted response")),
            _ => panic!("expected Http error, got {err:?}"),
        }
    }
}
