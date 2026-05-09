//! Service layer — business logic separated from Tauri / IPC plumbing.
//!
//! Why this module exists
//! ----------------------
//!
//! Pre-7.x the IPC commands in `crate::ipc` did three jobs in one
//! function body: read the user's settings, talk to the network /
//! filesystem / hardware, and emit Tauri events. That fused
//! presentation, transport, and business logic such that:
//!
//! * download / inference / hardware code paths could not be unit
//!   tested without live HTTP, real GGUFs on disk, and a host with
//!   the target RAM size,
//! * config resolution (read `[providers.configs."embedded.llm"]`,
//!   fall back to `DEFAULT_*` constants) was duplicated 5x and
//!   drifted with each touch,
//! * the only tests that existed were happy-path (`status returns the
//!   default file does not exist`).
//!
//! The service layer carves the work into:
//!
//! * **Adapters** — small traits over the platform side-effects
//!   ([`HardwareProbe`], [`ModelDownloader`], [`ModelStorage`],
//!   [`TranslationEngine`]). Production impls wrap `sysinfo` /
//!   `reqwest` / `std::fs` / `mistralrs::Model`. Test impls live
//!   alongside them and let unit tests script every branch (network
//!   failure mid-download, sha256 mismatch, 2 GB-RAM host, model
//!   load taking 5 s, etc.) without touching the real environment.
//! * **Service** — [`ModelService`] aggregates the adapters and
//!   carries the business logic (resolve config → download or
//!   import → verify → swap → rebuild providers). IPC commands
//!   become thin wrappers (5-10 lines) that just hand the request
//!   through and translate errors to `String`.
//!
//! The IPC layer is therefore a presentation adapter; tests against
//! `ModelService` directly are sufficient to lock down behaviour.

pub mod embedded_config;
pub mod hardware_probe;
pub mod model_downloader;
pub mod model_service;
pub mod model_storage;
pub mod translation_engine;

pub use embedded_config::ResolvedConfig;
pub use hardware_probe::{HardwareProbe, HostCapabilities, SysinfoProbe};
pub use model_downloader::{
    DownloadProgress, DownloadRequest, DownloadResult, HttpModelDownloader, ModelDownloader,
    ModelDownloaderError, ProgressCallback,
};
pub use model_service::{ModelService, ModelServiceError};
pub use model_storage::{FileSystemStorage, ModelFileMeta, ModelStorage};
pub use translation_engine::{EngineError, TranslateRequest, TranslationEngine};

#[cfg(test)]
pub use hardware_probe::FixedProbe;
#[cfg(test)]
pub use model_downloader::ScriptedDownloader;
#[cfg(test)]
pub use model_storage::InMemoryStorage;
#[cfg(test)]
pub use translation_engine::ScriptedEngine;
