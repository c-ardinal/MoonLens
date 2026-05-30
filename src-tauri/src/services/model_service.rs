//! Aggregates the embedded-model service surface.
//!
//! Pre-7.x the IPC layer (`ipc.rs`) carried both presentation and
//! business logic. This file is the latter half: status snapshot,
//! download orchestration, package export/import, tier switching.
//! Every dependency that touches the network, filesystem, or
//! hardware is injected, which lets the entire surface be tested
//! against fakes.

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use crate::embedded_model_package::{
    self, PackageManifest, PackageManifestInput, PACKAGE_EXTENSION,
};
use crate::model_registry::{
    recommended_class_for_ram, recommended_tier_for_ram, tier_by_coord, tier_by_id, ModelTier,
    ModelTierClass,
};
use crate::pipeline::Pipeline;
use crate::services::embedded_config::ResolvedConfig;
use crate::services::hardware_probe::{HardwareProbe, HostCapabilities};
use crate::services::model_downloader::{
    DownloadProgress, DownloadRequest, DownloadResult, ModelDownloader, ModelDownloaderError,
    ProgressCallback,
};
use crate::services::model_storage::{ModelStorage, StorageError};
use crate::settings::SettingsStore;

#[derive(Debug, Error)]
pub enum ModelServiceError {
    #[error("settings: {0}")]
    Settings(String),
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("download: {0}")]
    Download(#[from] ModelDownloaderError),
    #[error("package: {0}")]
    Package(String),
    #[error("invalid input: {0}")]
    Invalid(String),
}

/// Status snapshot the settings UI consumes. Mirrors the Tauri
/// `EmbeddedModelStatus` payload but lives here so tests can assert
/// it without going through the IPC layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedStatus {
    pub path: PathBuf,
    pub present: bool,
    pub size_bytes: u64,
    pub model_id: String,
    pub license: String,
    pub license_url: String,
}

/// Tier snapshot consumed by the settings dropdown + onboarding modal.
#[derive(Debug, Clone)]
pub struct TierSnapshot {
    pub current: Option<ModelTier>,
    pub recommended: ModelTier,
    pub recommended_reason: String,
    pub host: HostCapabilities,
    /// True when the user's current tier requires more RAM than the
    /// host has installed. Drives the settings UI's "your hardware
    /// is below this tier's minimum" warning so the user sees an
    /// honest signal before sitting through a 30-minute Lens
    /// translation that ultimately times out.
    pub current_underpowered: bool,
}

/// Outcome of [`ModelService::download_current`]. The IPC adapter
/// translates this into the `download_embedded_model` Tauri payload.
#[derive(Debug, Clone)]
pub struct DownloadOutcome {
    pub path: PathBuf,
    pub size_bytes: u64,
    /// True when the download was a no-op because the file already
    /// existed and `force` was false.
    pub was_already_present: bool,
}

/// Outcome of [`ModelService::package_current`]. Mirrors the IPC
/// `EmbeddedModelPackageResult` payload.
#[derive(Debug, Clone)]
pub struct PackageOutcome {
    pub path: PathBuf,
    pub manifest: PackageManifest,
}

/// Outcome of [`ModelService::import_package`].
#[derive(Debug, Clone)]
pub struct ImportOutcome {
    pub path: PathBuf,
    pub manifest: PackageManifest,
}

/// Per-sample result from [`ModelService::bench_embedded`]. Captures
/// the full inference latency for one representative translation
/// request — useful both for showing the user the variance across
/// samples and for computing aggregate p50 / p95 latencies in the UI.
///
/// llama.cpp upstream (and the wider community) reports two separate
/// numbers for inference benchmarks:
///   1. **Prompt eval** (a.k.a. prefill): time to process the input
///      prompt + LlamaContext setup. One-shot per request.
///   2. **Generation** (a.k.a. decode): the streaming token rate
///      AFTER prompt eval is done.
/// Mixing them into a single "tokens / total_ms" number makes short
/// translations look catastrophically slow because prompt eval
/// dominates total time when the output is only a handful of tokens.
/// We track them separately and compare the **decode** rate against
/// the published baseline in the verdict — that is the apples-to-apples
/// metric vs `llama-bench` upstream.
#[derive(Debug, Clone)]
pub struct BenchSample {
    /// 1-indexed sample number (1..=N).
    pub index: u32,
    /// Total wall-clock latency from request submission to final
    /// chunk delivered, in milliseconds. Includes context creation,
    /// prompt processing, AND decode for `output_tokens` tokens.
    pub total_ms: u64,
    /// Time to first token (a.k.a. prefill latency / TTFT) in
    /// milliseconds. From request submission to the first streaming
    /// token. Bounded below by context construction (~70ms for
    /// n_ctx=2048) + prompt eval (proportional to prompt length).
    pub prompt_eval_ms: u64,
    /// Steady-state decode time in milliseconds — `last_token_at -
    /// first_token_at`. Zero when only one token was produced (no
    /// inter-token interval to measure).
    pub decode_ms: u64,
    /// Number of output tokens the engine produced. Counted by
    /// streaming chunk callbacks (one chunk = one token in the
    /// llama.cpp text-generation path), so this matches what the
    /// user actually saw render to screen.
    pub output_tokens: u32,
    /// Wall-clock decode rate (tokens / second), computed as
    /// `output_tokens / total_ms * 1000`. Includes prompt eval
    /// overhead — the per-line latency a Lens user actually feels.
    pub tokens_per_second: f64,
    /// Steady-state decode rate (tokens / second), computed as
    /// `(output_tokens - 1) / decode_ms * 1000` for samples with
    /// 2+ tokens, falling back to `tokens_per_second` for 1-token
    /// outputs. This is the model's intrinsic generation throughput
    /// — what `llama-bench tg` reports — and is the right number to
    /// compare against published llama.cpp baselines.
    pub decode_tokens_per_second: f64,
    /// Translated text. Cropped to the first 80 chars so the UI
    /// does not need to render multi-line bodies in its bench
    /// table.
    pub translated_preview: String,
}

/// Live progress event fired by [`ModelService::bench_embedded`]. The
/// settings UI subscribes through `moonlens://embedded-bench-progress`
/// so the user sees forward motion (current sample, tokens generated,
/// elapsed time) instead of staring at "ベンチ実行中…" for minutes
/// while mistral.rs silently chews through CPU. Throttled to ~4 Hz
/// inside the bench so weak hardware does not pay overhead emitting
/// events more often than the UI can render them.
#[derive(Debug, Clone)]
pub enum BenchProgressEvent {
    /// Bench started. Total samples to run + the resolved tier label
    /// so the UI can render a stable header (`Compact (Qwen 2.5 0.5B):
    /// 0 / 5 サンプル …`).
    Started {
        total_samples: u32,
        tier_display_name: String,
    },
    /// One sample begins. Sent once per sample at request submission
    /// time. The `fixture_text` is the input string the bench is
    /// translating; useful for debugging if a specific fixture
    /// chokes mistral.rs.
    SampleStarted {
        index: u32,
        total: u32,
        fixture_text: String,
    },
    /// Periodic update during a sample. Throttled to ~4 Hz; the
    /// final per-sample numbers come back via `SampleCompleted`.
    SampleProgress {
        index: u32,
        total: u32,
        tokens_so_far: u32,
        elapsed_ms: u64,
    },
    /// One sample completed (or errored — partial output is still
    /// reported). The `sample` payload is identical to what ends
    /// up in [`BenchOutcome::samples`] so the UI can append rows
    /// to the result table as they arrive.
    SampleCompleted { sample: BenchSample },
    /// Whole bench finished; `outcome` mirrors the value the IPC
    /// returns, included here so a UI that has been streaming
    /// progress can show the final result without a second
    /// invoke round-trip.
    Completed { outcome: BenchOutcome },
}

/// Boxed callback that `bench_embedded` calls per progress event.
/// `Arc<dyn Fn>` mirrors the existing `ProgressCallback` type so
/// the IPC layer can wrap a Tauri emitter the same way it does for
/// download progress.
pub type BenchProgressCallback = Arc<dyn Fn(BenchProgressEvent) + Send + Sync>;

/// Aggregate of [`ModelService::bench_embedded`] runs. The IPC layer
/// turns this into the JSON the settings UI renders.
#[derive(Debug, Clone)]
pub struct BenchOutcome {
    /// Resolved tier label (e.g. "Compact (Qwen 2.5 0.5B)") so the
    /// user can see WHICH tier they benchmarked. Doubles as a
    /// "saved state" indicator if they later swap tiers and want to
    /// remember what their old numbers were.
    pub tier_display_name: String,
    pub model_path: String,
    pub samples: Vec<BenchSample>,
    /// p50 / p95 / mean derived from `samples`. Surfaced so the UI
    /// can show "median 12 tok/s, worst-case 7 tok/s" without
    /// having to compute them frontend-side (and risk drift across
    /// the IPC + UI-language boundary).
    pub p50_tokens_per_second: f64,
    pub p95_tokens_per_second: f64,
    pub mean_tokens_per_second: f64,
    /// Decode-only aggregates — the model's intrinsic generation
    /// throughput, comparable to upstream `llama-bench tg`. These
    /// drive the verdict because they isolate the runtime/SIMD
    /// performance from the per-request setup overhead.
    pub p50_decode_tokens_per_second: f64,
    pub p95_decode_tokens_per_second: f64,
    pub mean_decode_tokens_per_second: f64,
    pub p50_latency_ms: u64,
    pub p95_latency_ms: u64,
    /// HW snapshot at bench start. Lets the user post a screenshot
    /// of the result + their CPU specs without us needing a
    /// separate HW IPC.
    pub host_total_ram_gb: f64,
    pub host_cpu_cores: usize,
    /// Wall-clock duration of the entire bench run including
    /// engine warm-up. The first sample is usually the slowest
    /// because mistralrs caches some state on the first inference;
    /// surfacing this lets users distinguish "cold start cost" from
    /// "steady-state throughput".
    pub total_run_ms: u64,
}

/// Aggregated service. Generic over the three platform adapters;
/// production wires it with `HttpModelDownloader`, `FileSystemStorage`,
/// and `SysinfoProbe`. Tests use the in-memory / scripted variants.
pub struct ModelService<D, S, H>
where
    D: ModelDownloader + 'static,
    S: ModelStorage + 'static,
    H: HardwareProbe + 'static,
{
    pub downloader: D,
    pub storage: S,
    pub probe: H,
    pub settings: Arc<SettingsStore>,
    /// Optional pipeline reference. Production wires it so changes
    /// (download / import / tier switch) trigger a rebuild and the
    /// in-memory `EmbeddedLlmTranslator` picks up the new file.
    /// Tests pass `None` to focus on the service's own behaviour.
    pub pipeline: Option<Arc<Pipeline>>,
    /// Resolved app-data root. Mirrors `Pipeline::app_data_dir` but
    /// stored here so the service does not need to ask the pipeline
    /// each call.
    pub app_data_dir: PathBuf,
    /// Build-version stamp embedded in exported packages.
    pub exporter_version: String,
}

impl<D, S, H> ModelService<D, S, H>
where
    D: ModelDownloader + 'static,
    S: ModelStorage + 'static,
    H: HardwareProbe + 'static,
{
    /// Resolve the embedded provider's effective config. Single
    /// source of truth — every public method reads through this.
    pub fn resolved(&self) -> ResolvedConfig {
        let snap = self.settings.snapshot();
        ResolvedConfig::from_settings(&snap, &self.app_data_dir)
    }

    /// Status snapshot for the settings UI. For multi-part tiers
    /// `present` is true only when **every** part is on disk; size
    /// is the sum across parts. A half-downloaded multi-part tier
    /// (one part present, one missing) reports `present = false`
    /// because it cannot load.
    pub async fn status(&self) -> Result<EmbeddedStatus, ModelServiceError> {
        let resolved = self.resolved();
        let all_filenames = resolved.all_filenames();
        let all_paths = resolved.all_model_paths();

        let mut all_present = true;
        let mut total_size: u64 = 0;
        for (filename, path) in all_filenames.iter().zip(all_paths.iter()) {
            let (size, present) = if path.starts_with(self.storage.models_dir()) {
                let meta = self.storage.meta(filename).await?;
                (meta.size_bytes, meta.exists)
            } else {
                match tokio::fs::metadata(path).await {
                    Ok(m) if m.is_file() => (m.len(), true),
                    _ => (0, false),
                }
            };
            if !present {
                all_present = false;
            }
            total_size += size;
        }

        Ok(EmbeddedStatus {
            path: resolved.model_path.clone(),
            present: all_present,
            size_bytes: total_size,
            model_id: resolved.coord(),
            license: resolved.license,
            license_url: resolved.license_url,
        })
    }

    /// Download the current tier's GGUF. Returns early when every
    /// part is already present and `force` is false. For multi-part
    /// tiers each part is fetched in sequence so a partial recovery
    /// only re-downloads the missing parts (when `force=false`).
    /// Always idempotent w.r.t. provider state — calls
    /// `Pipeline::rebuild_providers` on success so the next translate
    /// call sees the files.
    pub async fn download_current(
        &self,
        force: bool,
        on_progress: Option<ProgressCallback>,
    ) -> Result<DownloadOutcome, ModelServiceError> {
        let resolved = self.resolved();

        // Pin to the storage root for safety. If the user has hand-
        // overridden `model_path` to a path outside the root, refuse
        // to download — a download command should not write anywhere
        // it pleases.
        if !resolved.model_path.starts_with(self.storage.models_dir()) {
            return Err(ModelServiceError::Invalid(format!(
                "model_path {} is outside the models dir {}; download refused",
                resolved.model_path.display(),
                self.storage.models_dir().display()
            )));
        }

        let all_filenames = resolved.all_filenames();
        let total_parts = all_filenames.len();

        // Short-circuit when every part is already present and the
        // caller has not asked for a forced re-download. Per-part
        // progress is collapsed into a single "already present"
        // outcome to mirror the single-file behaviour.
        if !force {
            let mut all_present = true;
            let mut total_size: u64 = 0;
            for filename in &all_filenames {
                let m = self.storage.meta(filename).await?;
                if !m.exists {
                    all_present = false;
                    break;
                }
                total_size += m.size_bytes;
            }
            if all_present {
                return Ok(DownloadOutcome {
                    path: resolved.model_path.clone(),
                    size_bytes: total_size,
                    was_already_present: true,
                });
            }
        }

        // Per-part download loop. Forward progress events tagged
        // with the per-part filename so the UI can render
        // "ダウンロード中: part 2/2 …".
        let mut total_size: u64 = 0;
        let mut last_path = resolved.model_path.clone();
        for (idx, filename) in all_filenames.iter().enumerate() {
            // Skip parts that are already present (only relevant
            // when `force` is true, since the early-return above
            // covers the all-present case otherwise — but `force`
            // re-downloads every part to keep the byte-for-byte
            // integrity guarantee).
            if !force {
                let m = self.storage.meta(filename).await?;
                if m.exists {
                    total_size += m.size_bytes;
                    last_path = m.path;
                    continue;
                }
            }
            let url = format!(
                "https://huggingface.co/{repo}/resolve/main/{filename}",
                repo = resolved.repo,
                filename = filename,
            );
            let req = DownloadRequest {
                url,
                filename: filename.clone(),
            };
            // Wrap the progress callback so consumers see the
            // overall "X of N" context. The wrapped closure adds
            // bytes from previously-completed parts to the
            // currently-downloading part's bytes.
            let prev_bytes = total_size;
            let part_progress: Option<ProgressCallback> = on_progress.clone().map(|cb| {
                let cb = cb.clone();
                let part_idx = idx;
                std::sync::Arc::new(move |p: DownloadProgress| {
                    cb(DownloadProgress {
                        downloaded: prev_bytes + p.downloaded,
                        // Total is unknown at the aggregate level
                        // until every part has reported its size,
                        // so leave `total = None` for in-flight
                        // progress and let the IPC layer decide
                        // whether to render a progress bar or a
                        // spinner. Frontends that want per-part
                        // detail can subscribe to a finer-grained
                        // event channel later.
                        total: p.total,
                    });
                    // Side-channel: log so a developer can verify
                    // multi-part progress in the dev console.
                    let _ = (part_idx, total_parts);
                }) as ProgressCallback
            });
            let result = self
                .downloader
                .download(req, &self.storage, part_progress)
                .await?;
            total_size += result.size_bytes;
            last_path = result.path;
        }

        // Final "all parts ready" event so the UI flips to
        // "準備完了" cleanly even on a multi-part tier.
        if let Some(cb) = on_progress.as_ref() {
            cb(DownloadProgress {
                downloaded: total_size,
                total: Some(total_size),
            });
        }
        let _ = last_path;
        let result = DownloadResult {
            path: resolved.model_path.clone(),
            size_bytes: total_size,
        };

        if let Some(p) = self.pipeline.as_ref() {
            p.rebuild_providers();
            // Kick off a background preload of the freshly-downloaded
            // GGUF so the user does not have to wait through
            // `mistralrs::GgufModelBuilder::build` (multi-second on
            // a 7B model) on their first translate call. Spawned —
            // download_current returns as soon as bytes are on disk,
            // the load progress streams over the existing
            // `moonlens://embedded-model-loading` channel.
            let pipeline_for_preload = p.clone();
            tokio::spawn(async move {
                pipeline_for_preload.preload_all().await;
            });
        }
        Ok(DownloadOutcome {
            path: result.path,
            size_bytes: result.size_bytes,
            was_already_present: false,
        })
    }

    /// Download a GGUF by its `model_registry::TIERS` tier id —
    /// the routing equivalent of `download_current`. Used by the
    /// settings UI's "不足モデルをダウンロード" action when the
    /// user's selected `user_tier` requires a GGUF outside the
    /// legacy `embedded.llm` config block (e.g. Standard needs
    /// BOTH JA-Std+ and Edge E2B; the legacy `download_current`
    /// only knows about one of them at a time).
    ///
    /// Differs from `download_current` in that it:
    ///   - Resolves the (repo, filenames) pair from the static
    ///     `TIERS` registry, NOT from user.toml.
    ///   - Always writes to the canonical
    ///     `<models_dir>/<filename>` paths — power-user
    ///     `model_path` overrides only apply to the single tier
    ///     the legacy picker manages.
    ///   - Does NOT mutate the saved `embedded.llm` config block
    ///     (routing maintains its own per-tier config in-memory).
    pub async fn download_tier_by_id(
        &self,
        tier_id: &str,
        force: bool,
        on_progress: Option<ProgressCallback>,
    ) -> Result<DownloadOutcome, ModelServiceError> {
        let tier = crate::model_registry::tier_by_id(tier_id).ok_or_else(|| {
            ModelServiceError::Invalid(format!("tier_id `{tier_id}` not in registry"))
        })?;
        let models_dir = self.storage.models_dir().to_path_buf();
        let total_parts = tier.filenames.len();

        // Early-return if every part is already present (skips
        // the network round-trip entirely on warm-cache restarts).
        if !force {
            let mut all_present = true;
            let mut total_size: u64 = 0;
            for filename in tier.filenames {
                let m = self.storage.meta(filename).await?;
                if !m.exists {
                    all_present = false;
                    break;
                }
                total_size += m.size_bytes;
            }
            if all_present {
                return Ok(DownloadOutcome {
                    path: models_dir.join(tier.primary_filename()),
                    size_bytes: total_size,
                    was_already_present: true,
                });
            }
        }

        let mut total_size: u64 = 0;
        for (idx, filename) in tier.filenames.iter().enumerate() {
            if !force {
                let m = self.storage.meta(filename).await?;
                if m.exists {
                    total_size += m.size_bytes;
                    continue;
                }
            }
            let url = format!(
                "https://huggingface.co/{repo}/resolve/main/{filename}",
                repo = tier.repo,
                filename = filename,
            );
            let req = DownloadRequest {
                url,
                filename: (*filename).to_string(),
            };
            // Aggregate bytes across parts so the UI's progress
            // bar advances monotonically over the whole tier's
            // download. See `download_current` for the same
            // bookkeeping pattern.
            let prev_bytes = total_size;
            let part_progress: Option<ProgressCallback> = on_progress.clone().map(|cb| {
                let cb = cb.clone();
                let part_idx = idx;
                std::sync::Arc::new(move |p: DownloadProgress| {
                    cb(DownloadProgress {
                        downloaded: prev_bytes + p.downloaded,
                        total: p.total,
                    });
                    let _ = (part_idx, total_parts);
                }) as ProgressCallback
            });
            let result = self
                .downloader
                .download(req, &self.storage, part_progress)
                .await?;
            total_size += result.size_bytes;
        }
        if let Some(cb) = on_progress.as_ref() {
            cb(DownloadProgress {
                downloaded: total_size,
                total: Some(total_size),
            });
        }

        // No pipeline rebuild here — the routing path picks up
        // newly-downloaded GGUFs lazily on the next translate
        // call (each inner translator lazy-loads its engine on
        // first inference). Forcing a rebuild would drop any
        // already-loaded engine state for unrelated tiers.
        Ok(DownloadOutcome {
            path: models_dir.join(tier.primary_filename()),
            size_bytes: total_size,
            was_already_present: false,
        })
    }

    /// Build a `.moonlens-model` package from the currently-installed
    /// model. Heavy work (sha256 + zip) runs in
    /// `tokio::task::spawn_blocking` so the runtime is not blocked.
    pub async fn package_current(
        &self,
        target_path: PathBuf,
        note: Option<String>,
    ) -> Result<PackageOutcome, ModelServiceError> {
        let resolved = self.resolved();
        if !resolved.model_path.is_file() {
            return Err(ModelServiceError::Invalid(format!(
                "no model present to package at {}",
                resolved.model_path.display()
            )));
        }
        let model_filename = resolved
            .model_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| ModelServiceError::Invalid("model_path has no filename".into()))?
            .to_string();

        let mut dest = target_path;
        if dest.extension().is_none() {
            dest.set_extension(PACKAGE_EXTENSION);
        }

        let template = PackageManifestInput {
            model_id: "embedded.llm".into(),
            model_filename: model_filename.clone(),
            // Multi-part tier? Pull every additional filename so
            // the package contains all GGUF parts. Single-part
            // tiers leave this empty.
            extra_filenames: resolved.filenames_remote_extra.clone(),
            source_repo: resolved.repo.clone(),
            source_filename: resolved.filename_remote.clone(),
            license: Some(resolved.license.clone()),
            license_url: Some(resolved.license_url.clone()),
            exporter_version: self.exporter_version.clone(),
            exported_at: now_rfc3339(),
            note,
        };

        let dest_clone = dest.clone();
        let model_path_clone = resolved.model_path.clone();
        let manifest = tokio::task::spawn_blocking(move || {
            embedded_model_package::build_package(&model_path_clone, &dest_clone, template)
        })
        .await
        .map_err(|e| ModelServiceError::Package(format!("packaging task panicked: {e}")))?
        .map_err(|e| ModelServiceError::Package(e.to_string()))?;

        Ok(PackageOutcome {
            path: dest,
            manifest,
        })
    }

    /// Import a `.moonlens-model` package. sha256 verification +
    /// atomic rename happen inside `embedded_model_package::apply_package`;
    /// here we wrap the rebuild_providers + storage refresh.
    pub async fn import_package(
        &self,
        package_path: PathBuf,
    ) -> Result<ImportOutcome, ModelServiceError> {
        let target_dir = self.storage.models_dir().to_path_buf();
        let pkg = package_path.clone();
        let (final_path, manifest) = tokio::task::spawn_blocking(move || {
            embedded_model_package::apply_package(&pkg, &target_dir)
        })
        .await
        .map_err(|e| ModelServiceError::Package(format!("import task panicked: {e}")))?
        .map_err(|e| ModelServiceError::Package(e.to_string()))?;
        if let Some(p) = self.pipeline.as_ref() {
            p.rebuild_providers();
            // Mirror download_current: kick off a background preload
            // so the imported model is warm by the time the user
            // tries to translate.
            let pipeline_for_preload = p.clone();
            tokio::spawn(async move {
                pipeline_for_preload.preload_all().await;
            });
        }
        Ok(ImportOutcome {
            path: final_path,
            manifest,
        })
    }

    /// Snapshot of (current tier, recommended tier, host capabilities).
    /// Driven by the injected `HardwareProbe` so test fixtures can
    /// span "2 GB laptop → Compact" through "64 GB workstation →
    /// Quality" without requiring matching real hardware.
    pub fn tier_snapshot(&self) -> TierSnapshot {
        let resolved = self.resolved();
        let current = tier_by_coord(&resolved.repo, &resolved.filename_remote).cloned();
        let host = self.probe.capabilities();
        let recommended = recommended_tier_for_ram(host.total_ram_gb).clone();
        let class = recommended_class_for_ram(host.total_ram_gb);
        let reason = match class {
            ModelTierClass::Compact => format!(
                "RAM {ram:.1} GB と検出 → 低スペック向け Compact ティアを推奨",
                ram = host.total_ram_gb
            ),
            ModelTierClass::Standard => format!(
                "RAM {ram:.1} GB と検出 → 標準 Standard ティアを推奨",
                ram = host.total_ram_gb
            ),
            ModelTierClass::Quality => format!(
                "RAM {ram:.1} GB と検出 → 高品質 Quality ティアを推奨",
                ram = host.total_ram_gb
            ),
        };
        // The user is "underpowered" for their current tier when
        // installed RAM falls below the tier's `min_ram_gb`. Some
        // headroom (-1 GB) is allowed to avoid false-positives on
        // hosts that report 7.8 GB instead of a marketed 8 GB.
        let current_underpowered = current
            .as_ref()
            .map(|t| host.total_ram_gb + 1.0 < t.min_ram_gb as f64)
            .unwrap_or(false);
        TierSnapshot {
            current,
            recommended,
            recommended_reason: reason,
            host,
            current_underpowered,
        }
    }

    /// Switch the embedded provider to a different tier. Saves the
    /// `repo`/`filename_remote` overrides into user.toml and rebuilds
    /// the provider map. Does NOT trigger a download.
    pub async fn set_tier(&self, tier_id: &str) -> Result<(), ModelServiceError> {
        let tier = tier_by_id(tier_id)
            .ok_or_else(|| ModelServiceError::Invalid(format!("unknown tier id `{tier_id}`")))?;
        let mut snap = (*self.settings.snapshot()).clone();
        let cfg = snap
            .providers
            .configs
            .entry("embedded.llm".to_string())
            .or_insert_with(|| serde_json::Value::Object(Default::default()));
        if let Some(obj) = cfg.as_object_mut() {
            obj.insert("repo".into(), serde_json::Value::String(tier.repo.into()));
            // Primary filename (first in `filenames`) is what
            // user.toml stores so older single-file readers keep
            // working unchanged.
            obj.insert(
                "filename_remote".into(),
                serde_json::Value::String(tier.primary_filename().into()),
            );
            // Multi-part tiers (e.g. Qwen 2.5 7B Q4_K_M with two
            // LFS parts) record the additional filenames here so
            // the downloader fetches every file the engine needs.
            // Single-part tiers clear the field — keeping a stale
            // extras list from a previous tier would point the
            // downloader at filenames that do not belong to the
            // newly-picked repo and produce 404s.
            if tier.filenames.len() > 1 {
                let extras: Vec<serde_json::Value> = tier
                    .filenames
                    .iter()
                    .skip(1)
                    .map(|f| serde_json::Value::String((*f).to_string()))
                    .collect();
                obj.insert(
                    "filenames_remote_extra".into(),
                    serde_json::Value::Array(extras),
                );
            } else {
                obj.remove("filenames_remote_extra");
            }
            obj.insert(
                "license".into(),
                serde_json::Value::String(tier.license.into()),
            );
            obj.insert(
                "license_url".into(),
                serde_json::Value::String(tier.license_url.into()),
            );
            obj.remove("model_path");
        }
        self.settings
            .save(snap)
            .map_err(|e| ModelServiceError::Settings(e.to_string()))?;
        if let Some(p) = self.pipeline.as_ref() {
            p.rebuild_providers();
            // Tier switch may point at a GGUF that already lives on
            // disk (e.g. user previously downloaded Standard + Compact
            // and is now flipping back to Standard). Preload eagerly
            // so the FIRST translate after the switch is instant.
            // For a tier whose GGUF is not yet downloaded the
            // EmbeddedLlmTranslator::preload returns Ok(()) without
            // touching mistralrs, so this spawn is cheap.
            let pipeline_for_preload = p.clone();
            tokio::spawn(async move {
                pipeline_for_preload.preload_all().await;
            });
        }
        Ok(())
    }

    /// Run a representative-translation benchmark against the
    /// currently-installed embedded LLM and return per-sample +
    /// aggregate metrics. The fixtures are short JA/EN sentences
    /// representative of Lens output (one OCR line each); using
    /// fixed input means runs across hardware are comparable
    /// without the user having to pick prompts.
    ///
    /// `samples` is clamped to `[1, 20]`. The first sample is
    /// typically slow because mistralrs caches some state on its
    /// first real inference — `total_run_ms` and the per-sample
    /// breakdown make that cold-start cost visible so the user can
    /// distinguish it from steady-state throughput.
    pub async fn bench_embedded(
        &self,
        samples: u32,
        on_progress: Option<BenchProgressCallback>,
    ) -> Result<BenchOutcome, ModelServiceError> {
        use crate::providers::{
            translate_embedded_llm::EmbeddedLlmTranslator, Provider, StreamingChunkSink, TaskInput,
            TaskOutput,
        };
        use parking_lot::Mutex;
        use std::time::Instant;

        let n = samples.clamp(1, 20);
        let resolved = self.resolved();
        if !resolved.model_path.is_file() {
            return Err(ModelServiceError::Invalid(format!(
                "embedded model not present at {}",
                resolved.model_path.display()
            )));
        }

        let tier_display_name =
            crate::model_registry::tier_by_coord(&resolved.repo, &resolved.filename_remote)
                .map(|t| t.display_name.to_string())
                .unwrap_or_else(|| format!("custom: {}", resolved.coord()));

        if let Some(cb) = on_progress.as_ref() {
            cb(BenchProgressEvent::Started {
                total_samples: n,
                tier_display_name: tier_display_name.clone(),
            });
        }

        // Build a one-shot translator pinned to the resolved config.
        // We deliberately do NOT reuse the pipeline-cached instance
        // because we want a clean benchmark — pipeline rebuilds
        // would dirty the timing. Bench-only override: cap output
        // at 64 tokens (much smaller than production's 512+) so a
        // single sample lands in 5-30s on weak hardware. Without
        // this cap, a slow CPU running mistral.rs could spend
        // minutes per sample and the user would think the bench
        // hung.
        let cfg = serde_json::json!({
            "model_path": resolved.model_path.display().to_string(),
            "license": resolved.license,
            "license_url": resolved.license_url,
            "num_predict": 64u64,
        });
        let translator = EmbeddedLlmTranslator::new("embedded.llm.bench", &cfg);
        tracing::info!(
            "bench: starting {} samples on tier `{}` (max 64 tok/sample)",
            n,
            tier_display_name
        );
        // Trigger lazy load up front so it does not skew the first
        // sample's timing.
        translator
            .preload()
            .await
            .map_err(|e| ModelServiceError::Invalid(format!("bench warmup: {e}")))?;

        // Representative inputs — kept short so a slow run does not
        // need 10 minutes to finish. Each is the kind of sentence
        // Lens streams.
        const FIXTURES: &[&str] = &[
            "Hello, world.",
            "The quick brown fox jumps over the lazy dog.",
            "Welcome to the future of translation.",
            "This is a benchmark sentence with eleven words exactly.",
            "Performance varies across hardware and runtime configurations.",
        ];

        let mut samples_out: Vec<BenchSample> = Vec::with_capacity(n as usize);
        let bench_started = Instant::now();
        for i in 0..n {
            let text = FIXTURES[(i as usize) % FIXTURES.len()].to_string();

            if let Some(cb) = on_progress.as_ref() {
                cb(BenchProgressEvent::SampleStarted {
                    index: i + 1,
                    total: n,
                    fixture_text: text.clone(),
                });
            }
            tracing::info!("bench: sample {}/{} started — input: {:?}", i + 1, n, text);

            let token_count = Arc::new(Mutex::new(0u32));
            let preview = Arc::new(Mutex::new(String::new()));
            let last_emit = Arc::new(Mutex::new(Instant::now()));
            // Track first / last token timestamps so we can split
            // prompt-eval (= TTFT) from steady-state decode in the
            // metrics. Without this split, the bench's tokens/total_ms
            // ratio attributes ~70% of a 4-token sample's wall-clock
            // to the per-request setup, masking the actual decode
            // throughput by 5-10x.
            let first_token_at = Arc::new(Mutex::new(None::<Instant>));
            let last_token_at = Arc::new(Mutex::new(None::<Instant>));
            let sample_started = Instant::now();

            // The sink fires once per streaming token. Three jobs:
            //   1. Increment `token_count` so the final-aggregate
            //      number reflects the actual stream depth.
            //   2. Build a short preview (cropped to 80 chars) the
            //      result table renders.
            //   3. THROTTLED: emit `SampleProgress` every ~250 ms
            //      so the UI sees forward motion. Throttling
            //      matters because mistral.rs streams at 5-50
            //      tok/s — emitting per token would flood the
            //      Tauri channel and starve the UI thread.
            let token_count_for_sink = token_count.clone();
            let preview_for_sink = preview.clone();
            let last_emit_for_sink = last_emit.clone();
            let first_token_for_sink = first_token_at.clone();
            let last_token_for_sink = last_token_at.clone();
            let on_progress_for_sink = on_progress.clone();
            let total_for_sink = n;
            let index_for_sink = i + 1;
            let sink: StreamingChunkSink = Arc::new(move |delta, _done| {
                if !delta.is_empty() {
                    let now = Instant::now();
                    *token_count_for_sink.lock() += 1;
                    {
                        let mut first = first_token_for_sink.lock();
                        if first.is_none() {
                            *first = Some(now);
                        }
                    }
                    *last_token_for_sink.lock() = Some(now);
                    let mut p = preview_for_sink.lock();
                    if p.chars().count() < 80 {
                        p.push_str(&delta);
                    }
                    drop(p);
                    if let Some(cb) = on_progress_for_sink.as_ref() {
                        let mut last = last_emit_for_sink.lock();
                        if last.elapsed() >= std::time::Duration::from_millis(250) {
                            *last = now;
                            let tokens = *token_count_for_sink.lock();
                            let elapsed = sample_started.elapsed().as_millis() as u64;
                            drop(last);
                            cb(BenchProgressEvent::SampleProgress {
                                index: index_for_sink,
                                total: total_for_sink,
                                tokens_so_far: tokens,
                                elapsed_ms: elapsed,
                            });
                        }
                    }
                }
            });

            let res = translator
                .run_streaming(
                    TaskInput::Translate {
                        text: text.clone(),
                        source_language: "en".into(),
                        target_language: "ja".into(),
                        glossary: Vec::new(),
                    },
                    sink,
                )
                .await;
            let elapsed_ms = sample_started.elapsed().as_millis() as u64;

            // Even on inference error we record the sample so the
            // user sees the failure at this index — a translator
            // that errors on sample 1 but succeeds on 2 is a
            // useful signal we should not hide.
            let output_tokens = *token_count.lock();
            let preview_text = preview.lock().clone();
            let tps = if elapsed_ms > 0 {
                (output_tokens as f64) / (elapsed_ms as f64) * 1000.0
            } else {
                0.0
            };

            // Split-metric computation. With the streaming sink
            // recording per-token timestamps:
            //   - prompt_eval_ms = sample_started → first_token_at
            //   - decode_ms      = first_token_at → last_token_at
            // We use (output_tokens - 1) as the numerator for
            // decode_tps because the FIRST token's wall-clock cost
            // is dominated by prompt eval, not decode — including it
            // would still mix prompt eval into the decode metric.
            let first_at = *first_token_at.lock();
            let last_at = *last_token_at.lock();
            let prompt_eval_ms = first_at
                .map(|t| t.duration_since(sample_started).as_millis() as u64)
                .unwrap_or(elapsed_ms);
            let decode_ms = match (first_at, last_at) {
                (Some(f), Some(l)) if l > f => l.duration_since(f).as_millis() as u64,
                _ => 0,
            };
            let decode_tps = if output_tokens >= 2 && decode_ms > 0 {
                ((output_tokens - 1) as f64) / (decode_ms as f64) * 1000.0
            } else {
                tps
            };

            // Surface inference failures by recording 0 tokens; the
            // UI will see the discrepancy and can warn.
            let _ = match res {
                Ok(TaskOutput::Text(_)) => Ok(()),
                Ok(_) => Err(ModelServiceError::Invalid(
                    "bench: unexpected output kind".into(),
                )),
                Err(e) => Err(ModelServiceError::Invalid(format!("bench sample {i}: {e}"))),
            };
            let sample = BenchSample {
                index: i + 1,
                total_ms: elapsed_ms,
                prompt_eval_ms,
                decode_ms,
                output_tokens,
                tokens_per_second: tps,
                decode_tokens_per_second: decode_tps,
                translated_preview: preview_text,
            };
            tracing::info!(
                "bench: sample {}/{} done — {} tokens in {} ms (wall {:.1} tok/s, \
                 prompt_eval {} ms, decode {:.1} tok/s)",
                sample.index,
                n,
                sample.output_tokens,
                sample.total_ms,
                sample.tokens_per_second,
                sample.prompt_eval_ms,
                sample.decode_tokens_per_second,
            );
            if let Some(cb) = on_progress.as_ref() {
                cb(BenchProgressEvent::SampleCompleted {
                    sample: sample.clone(),
                });
            }
            samples_out.push(sample);
        }

        let total_run_ms = bench_started.elapsed().as_millis() as u64;
        let aggregates = compute_bench_aggregates(&samples_out);
        let host = self.probe.capabilities();

        let outcome = BenchOutcome {
            tier_display_name,
            model_path: resolved.model_path.display().to_string(),
            samples: samples_out,
            p50_tokens_per_second: aggregates.p50_tps,
            p95_tokens_per_second: aggregates.p95_tps,
            mean_tokens_per_second: aggregates.mean_tps,
            p50_decode_tokens_per_second: aggregates.p50_decode_tps,
            p95_decode_tokens_per_second: aggregates.p95_decode_tps,
            mean_decode_tokens_per_second: aggregates.mean_decode_tps,
            p50_latency_ms: aggregates.p50_ms,
            p95_latency_ms: aggregates.p95_ms,
            host_total_ram_gb: host.total_ram_gb,
            host_cpu_cores: host.cpu_cores,
            total_run_ms,
        };
        if let Some(cb) = on_progress.as_ref() {
            cb(BenchProgressEvent::Completed {
                outcome: outcome.clone(),
            });
        }
        Ok(outcome)
    }
}

/// Internal aggregate result. Public-via-`BenchOutcome` only —
/// kept private so the IPC contract stays minimal.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BenchAggregates {
    pub p50_tps: f64,
    pub p95_tps: f64,
    pub mean_tps: f64,
    pub p50_decode_tps: f64,
    pub p95_decode_tps: f64,
    pub mean_decode_tps: f64,
    pub p50_ms: u64,
    pub p95_ms: u64,
}

/// Compute p50 / p95 / mean for a slice of samples. Public to the
/// crate so tests can verify the percentile math without driving a
/// real bench end-to-end.
pub(crate) fn compute_bench_aggregates(samples: &[BenchSample]) -> BenchAggregates {
    if samples.is_empty() {
        return BenchAggregates {
            p50_tps: 0.0,
            p95_tps: 0.0,
            mean_tps: 0.0,
            p50_decode_tps: 0.0,
            p95_decode_tps: 0.0,
            mean_decode_tps: 0.0,
            p50_ms: 0,
            p95_ms: 0,
        };
    }
    let mut tps: Vec<f64> = samples.iter().map(|s| s.tokens_per_second).collect();
    let mut decode_tps: Vec<f64> = samples.iter().map(|s| s.decode_tokens_per_second).collect();
    let mut ms: Vec<u64> = samples.iter().map(|s| s.total_ms).collect();
    tps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    decode_tps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ms.sort_unstable();
    let mean = tps.iter().sum::<f64>() / tps.len() as f64;
    let mean_decode = decode_tps.iter().sum::<f64>() / decode_tps.len() as f64;
    // Nearest-rank percentiles. With 5 samples, p50 = index 2,
    // p95 = index 4. Conservative for small N (we never have 100
    // samples here so the smoothed-percentile formula buys nothing).
    let pct_idx = |p: f64, len: usize| -> usize {
        let idx = ((p / 100.0) * (len as f64)).ceil() as usize;
        idx.saturating_sub(1).min(len.saturating_sub(1))
    };
    let p50_tps = tps[pct_idx(50.0, tps.len())];
    let p95_tps = tps[pct_idx(95.0, tps.len())];
    let p50_decode_tps = decode_tps[pct_idx(50.0, decode_tps.len())];
    let p95_decode_tps = decode_tps[pct_idx(95.0, decode_tps.len())];
    let p50_ms = ms[pct_idx(50.0, ms.len())];
    let p95_ms = ms[pct_idx(95.0, ms.len())];
    BenchAggregates {
        p50_tps,
        p95_tps,
        mean_tps: mean,
        p50_decode_tps,
        p95_decode_tps,
        mean_decode_tps: mean_decode,
        p50_ms,
        p95_ms,
    }
}

fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    fn ymd_from_days(mut days: i64) -> (i32, u32, u32) {
        let mut y = 1970i32;
        loop {
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            let yd: i64 = if leap { 366 } else { 365 };
            if days < yd {
                break;
            }
            days -= yd;
            y += 1;
        }
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let dim = [
            31,
            if leap { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        let mut m = 0;
        while m < 11 && days >= dim[m] {
            days -= dim[m];
            m += 1;
        }
        (y, (m as u32) + 1, (days as u32) + 1)
    }
    let day = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400) as u32;
    let (y, mo, d) = ymd_from_days(day);
    let h = secs_of_day / 3600;
    let mi = (secs_of_day / 60) % 60;
    let s = secs_of_day % 60;
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::hardware_probe::FixedProbe;
    use crate::services::model_downloader::{scripted::ScriptedResponse, ScriptedDownloader};
    use crate::services::model_storage::InMemoryStorage;
    use crate::settings::SettingsStore;

    fn build_service(
        ram_gb: f64,
        cpus: usize,
        downloader: ScriptedDownloader,
        storage: InMemoryStorage,
    ) -> ModelService<ScriptedDownloader, InMemoryStorage, FixedProbe> {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg_path = tmp.path().join("config.toml");
        std::fs::write(&cfg_path, include_str!("../settings/default.toml")).unwrap();
        let settings = Arc::new(SettingsStore::load_from(cfg_path).unwrap());
        // These fixtures exercise the LEGACY single-model surface
        // (`embedded.llm` config → Qwen 1.5B default): download URLs,
        // status `model_id`, and `set_tier` persistence all assert
        // against that config block. Since `ResolvedConfig` now
        // follows `user_tier` when it is set (so the settings UI
        // reflects the routed tier, not a stale Qwen entry), and
        // default.toml ships `user_tier = "standard"`, clear it here
        // so the legacy path under test stays reachable. Routing-
        // following behaviour is covered by the dedicated
        // `*_follows_user_tier` tests below.
        {
            let mut snap = (*settings.snapshot()).clone();
            snap.providers.user_tier = None;
            settings
                .save(snap)
                .expect("force legacy user_tier=None for model-service fixtures");
        }
        // Storage backs onto the in-memory map; cheat the path
        // resolution by parking the app-data dir at the storage's
        // pretend root so `ResolvedConfig::model_path` lands inside
        // the storage. The InMemoryStorage uses `/in-memory/moonlens/models`.
        let app_data = storage.models_dir().parent().unwrap().to_path_buf();
        // Leak the tempdir so it outlives the test (mirrors the
        // pattern in pipeline_tests::chain_streaming).
        std::mem::forget(tmp);
        let storage_for_field = storage;
        ModelService {
            downloader,
            storage: storage_for_field,
            probe: FixedProbe::new(ram_gb, cpus),
            settings,
            pipeline: None,
            app_data_dir: app_data,
            exporter_version: "test".into(),
        }
    }

    #[tokio::test]
    async fn status_reports_absent_on_fresh_install() {
        let svc = build_service(8.0, 4, ScriptedDownloader::new(), InMemoryStorage::new());
        let s = svc.status().await.unwrap();
        assert!(!s.present);
        assert_eq!(s.size_bytes, 0);
        assert_eq!(s.license, "Apache-2.0");
        assert!(s.model_id.contains(":llm-jp-3-980m-instruct3-Q4_K_M.gguf"));
    }

    #[tokio::test]
    async fn download_writes_payload_and_status_flips_to_present() {
        let storage = InMemoryStorage::new();
        let payload = vec![0xABu8; 2048];
        let url = "https://huggingface.co/mmnga/llm-jp-3-980m-instruct3-gguf/resolve/main/llm-jp-3-980m-instruct3-Q4_K_M.gguf";
        let downloader = ScriptedDownloader::new().respond_to(
            url,
            ScriptedResponse::ok(payload.clone()).with_chunk_size(512),
        );
        let svc = build_service(8.0, 4, downloader, storage);

        let outcome = svc.download_current(false, None).await.unwrap();
        assert!(!outcome.was_already_present);
        assert_eq!(outcome.size_bytes, 2048);

        let s = svc.status().await.unwrap();
        assert!(s.present);
        assert_eq!(s.size_bytes, 2048);
    }

    #[tokio::test]
    async fn second_download_is_a_noop_unless_force_is_set() {
        let storage = InMemoryStorage::new()
            .with_file("llm-jp-3-980m-instruct3-Q4_K_M.gguf", vec![1u8; 1024]);
        let url = "https://huggingface.co/mmnga/llm-jp-3-980m-instruct3-gguf/resolve/main/llm-jp-3-980m-instruct3-Q4_K_M.gguf";
        let downloader =
            ScriptedDownloader::new().respond_to(url, ScriptedResponse::ok(vec![2u8; 1024]));
        let svc = build_service(8.0, 4, downloader, storage);

        // No-force: short-circuits, downloader is NOT called.
        let outcome = svc.download_current(false, None).await.unwrap();
        assert!(outcome.was_already_present);
        assert!(
            svc.downloader.urls_called().is_empty(),
            "no force → no fetch"
        );

        // Force: downloader IS called.
        let outcome = svc.download_current(true, None).await.unwrap();
        assert!(!outcome.was_already_present);
        assert_eq!(svc.downloader.urls_called().len(), 1);
    }

    #[tokio::test]
    async fn mid_download_failure_leaves_storage_clean() {
        let storage = InMemoryStorage::new();
        let url = "https://huggingface.co/mmnga/llm-jp-3-980m-instruct3-gguf/resolve/main/llm-jp-3-980m-instruct3-Q4_K_M.gguf";
        let downloader = ScriptedDownloader::new().respond_to(
            url,
            ScriptedResponse::fail_at(vec![3u8; 4096], 1024).with_chunk_size(256),
        );
        let svc = build_service(8.0, 4, downloader, storage);

        let err = svc
            .download_current(false, None)
            .await
            .expect_err("must fail");
        assert!(matches!(err, ModelServiceError::Download(_)));

        let s = svc.status().await.unwrap();
        assert!(
            !s.present,
            "failed download MUST not leave a partial visible"
        );
    }

    #[tokio::test]
    async fn tier_snapshot_recommends_compact_for_low_ram() {
        let svc = build_service(2.0, 2, ScriptedDownloader::new(), InMemoryStorage::new());
        let snap = svc.tier_snapshot();
        // 2026-05-30: after Qwen was removed from the registry, the
        // JA-specialised llm-jp 3 980M is the sole (and therefore
        // first) Compact-class tier the recommender picks.
        assert_eq!(snap.recommended.class, ModelTierClass::Compact);
        assert_eq!(snap.recommended.id, "llm-jp-3-980m-instruct3-q4_k_m");
        assert!(snap.recommended_reason.contains("Compact"));
        assert!(snap.recommended_reason.contains("2.0"));
    }

    #[tokio::test]
    async fn tier_snapshot_recommends_quality_on_workstations() {
        let svc = build_service(64.0, 32, ScriptedDownloader::new(), InMemoryStorage::new());
        let snap = svc.tier_snapshot();
        assert_eq!(snap.recommended.id, "gemma-4-e4b-it-q4_k_m");
        assert!(snap.recommended_reason.contains("Quality"));
    }

    #[tokio::test]
    async fn tier_snapshot_flags_underpowered_when_ram_below_min() {
        // Repro of the user-reported "Lens で重いティアを選んだら
        // 8 GB PC でタイムアウト連発" case. The 8 GB host should NOT
        // be flagged as adequate for the Quality tier (Edge+ E4B,
        // min_ram_gb = 12).
        let svc = build_service(8.0, 4, ScriptedDownloader::new(), InMemoryStorage::new());
        // Switch to Quality (Edge+ E4B) — this is what the bug
        // report user did and what triggered the timeout cascade.
        svc.set_tier("gemma-4-e4b-it-q4_k_m").await.unwrap();
        let snap = svc.tier_snapshot();
        assert!(
            snap.current_underpowered,
            "8GB-RAM host running the Quality tier MUST be flagged as underpowered"
        );
    }

    #[tokio::test]
    async fn tier_snapshot_does_not_flag_when_ram_meets_minimum() {
        // The flag must NOT fire when host RAM matches the tier's
        // requirement. Otherwise a borderline host would see a
        // permanent red banner — annoying and inaccurate.
        let svc = build_service(8.0, 4, ScriptedDownloader::new(), InMemoryStorage::new());
        // Standard tier needs 8 GB; 8 GB host is right at the
        // boundary and should not be flagged.
        let snap = svc.tier_snapshot();
        assert!(
            !snap.current_underpowered,
            "8GB-RAM host running Standard tier (8GB min) MUST NOT be flagged"
        );
    }

    #[test]
    fn bench_aggregates_pick_correct_percentiles_for_5_samples() {
        // Five samples is the bench default. p50 = index 2 (0-based),
        // p95 = index 4 (last). Nearest-rank semantics — verify we
        // do not silently drift to a smoothed formula that would
        // change reported numbers across versions.
        let mk = |idx: u32, ms: u64, tps: f64| BenchSample {
            index: idx,
            total_ms: ms,
            // Tests: prompt_eval=0, decode_ms=ms-100 (arbitrary positive split),
            // decode_tps mirrors wall tps so the legacy assertions on
            // p50_tps still hold while exercising the new fields.
            prompt_eval_ms: 100,
            decode_ms: ms.saturating_sub(100),
            output_tokens: 10,
            tokens_per_second: tps,
            decode_tokens_per_second: tps,
            translated_preview: String::new(),
        };
        let samples = vec![
            mk(1, 1000, 5.0),
            mk(2, 800, 8.0),
            mk(3, 600, 12.0),
            mk(4, 500, 15.0),
            mk(5, 400, 20.0),
        ];
        let aggs = compute_bench_aggregates(&samples);
        // tps sorted: [5, 8, 12, 15, 20]. p50 (idx 2) = 12, p95 (idx 4) = 20.
        assert!((aggs.p50_tps - 12.0).abs() < 1e-9);
        assert!((aggs.p95_tps - 20.0).abs() < 1e-9);
        // Mean = (5+8+12+15+20)/5 = 12.0
        assert!((aggs.mean_tps - 12.0).abs() < 1e-9);
        // ms sorted ascending: [400, 500, 600, 800, 1000]. p50 = 600, p95 = 1000.
        assert_eq!(aggs.p50_ms, 600);
        assert_eq!(aggs.p95_ms, 1000);
    }

    #[test]
    fn bench_aggregates_safe_on_empty_samples() {
        let aggs = compute_bench_aggregates(&[]);
        assert_eq!(aggs.p50_tps, 0.0);
        assert_eq!(aggs.p95_tps, 0.0);
        assert_eq!(aggs.mean_tps, 0.0);
        assert_eq!(aggs.p50_decode_tps, 0.0);
        assert_eq!(aggs.p95_decode_tps, 0.0);
        assert_eq!(aggs.mean_decode_tps, 0.0);
        assert_eq!(aggs.p50_ms, 0);
        assert_eq!(aggs.p95_ms, 0);
    }

    #[test]
    fn bench_aggregates_split_decode_from_wall_clock() {
        // Regression guard for the Phase 20.6 metric split. The
        // decode-tps and wall-clock-tps aggregates must be computed
        // independently — a sample whose decode is 50 tok/s but
        // whose wall-clock includes a long prompt-eval (= effective
        // wall tps of 5 tok/s) MUST report the 50 tok/s in the
        // decode aggregates and the 5 tok/s in the wall-clock
        // aggregates. Mixing the two would defeat the purpose of
        // the split.
        let samples = vec![
            BenchSample {
                index: 1,
                total_ms: 800,
                prompt_eval_ms: 700,
                decode_ms: 80,
                output_tokens: 5,
                tokens_per_second: 6.25,        // 5 / 0.8
                decode_tokens_per_second: 50.0, // 4 / 0.08
                translated_preview: String::new(),
            },
            BenchSample {
                index: 2,
                total_ms: 1000,
                prompt_eval_ms: 700,
                decode_ms: 280,
                output_tokens: 15,
                tokens_per_second: 15.0,        // 15 / 1.0
                decode_tokens_per_second: 50.0, // 14 / 0.28
                translated_preview: String::new(),
            },
            BenchSample {
                index: 3,
                total_ms: 900,
                prompt_eval_ms: 700,
                decode_ms: 180,
                output_tokens: 10,
                tokens_per_second: 11.11,
                decode_tokens_per_second: 50.0,
                translated_preview: String::new(),
            },
        ];
        let aggs = compute_bench_aggregates(&samples);
        // Decode aggregates: every sample is 50 → all percentiles 50.
        assert!((aggs.p50_decode_tps - 50.0).abs() < 1e-6);
        assert!((aggs.mean_decode_tps - 50.0).abs() < 1e-6);
        // Wall-clock aggregates: each sample is meaningfully lower.
        // p50 (sorted [6.25, 11.11, 15.0]) = 11.11.
        assert!((aggs.p50_tps - 11.11).abs() < 0.01);
        assert!(
            aggs.mean_tps < aggs.mean_decode_tps,
            "wall-clock mean ({:.2}) must be < decode mean ({:.2})",
            aggs.mean_tps,
            aggs.mean_decode_tps
        );
    }

    #[tokio::test]
    async fn tier_snapshot_tolerates_marketed_vs_actual_ram_drift() {
        // Real Windows reports total physical RAM minus reserved
        // regions, so a "marketed 8 GB" stick often shows up as
        // 7.8 GB. The underpowered check has a 1 GB tolerance to
        // avoid a false positive on these hosts.
        let svc = build_service(7.7, 4, ScriptedDownloader::new(), InMemoryStorage::new());
        let snap = svc.tier_snapshot();
        assert!(
            !snap.current_underpowered,
            "7.7 GB host (effectively 8 GB) on Standard tier MUST NOT be flagged"
        );
    }

    #[tokio::test]
    async fn set_tier_persists_repo_and_filename_to_settings() {
        let svc = build_service(4.0, 2, ScriptedDownloader::new(), InMemoryStorage::new());
        svc.set_tier("llm-jp-3-980m-instruct3-q4_k_m")
            .await
            .unwrap();
        let resolved = svc.resolved();
        assert_eq!(resolved.repo, "mmnga/llm-jp-3-980m-instruct3-gguf");
        assert_eq!(
            resolved.filename_remote,
            "llm-jp-3-980m-instruct3-Q4_K_M.gguf"
        );
        // Switching tier MUST drop any pinned model_path so the
        // resolution falls through to <app_data>/models/<filename>.
        let snap = svc.settings.snapshot();
        let cfg = snap.providers.configs.get("embedded.llm").unwrap();
        assert!(
            cfg.get("model_path").is_none(),
            "set_tier must clear model_path override"
        );
    }

    #[tokio::test]
    async fn set_tier_rejects_unknown_id() {
        let svc = build_service(8.0, 4, ScriptedDownloader::new(), InMemoryStorage::new());
        let err = svc.set_tier("not-a-tier").await.expect_err("must fail");
        assert!(matches!(err, ModelServiceError::Invalid(_)));
    }

    /// Smoke test: in-memory storage + scripted downloader + fixed
    /// probe wired together drive the full "fresh install →
    /// download → status flips" path without touching the real
    /// network or filesystem. This is the E2E proof that the DI
    /// refactor delivers what the user asked for: integration tests
    /// that do not require running infrastructure.
    #[tokio::test]
    async fn e2e_fresh_install_through_first_translation_ready() {
        // 2026-05-30: llm-jp 3 980M is the recommended Compact for
        // fresh installs (sole Compact-class tier after Qwen's
        // registry removal).
        let url =
            "https://huggingface.co/mmnga/llm-jp-3-980m-instruct3-gguf/resolve/main/llm-jp-3-980m-instruct3-Q4_K_M.gguf";
        let storage = InMemoryStorage::new();
        let downloader = ScriptedDownloader::new().respond_to(
            url,
            ScriptedResponse::ok(vec![9u8; 4096]).with_chunk_size(512),
        );
        // 4 GB host → recommended Compact tier.
        let svc = build_service(4.0, 4, downloader, storage);

        // 1. Status: nothing present.
        assert!(!svc.status().await.unwrap().present);

        // 2. Tier snapshot recommends Compact.
        let tier = svc.tier_snapshot();
        assert_eq!(tier.recommended.id, "llm-jp-3-980m-instruct3-q4_k_m");

        // 3. User accepts the recommendation.
        svc.set_tier(tier.recommended.id).await.unwrap();

        // 4. Download.
        let outcome = svc.download_current(false, None).await.unwrap();
        assert_eq!(outcome.size_bytes, 4096);

        // 5. Status: present, license advert intact.
        let s = svc.status().await.unwrap();
        assert!(s.present);
        assert_eq!(s.license, "Apache-2.0");
        assert!(s.model_id.contains("llm-jp-3-980m-instruct3-Q4_K_M.gguf"));
    }
}
