//! Offline distribution format for the embedded LLM model.
//!
//! Use case: a single admin (or one PC with internet access) downloads
//! the GGUF once, packages it via [`build_package`], drops the result
//! on a fileserver, and every other PC on the restricted-network LAN
//! imports via [`apply_package`] — no HuggingFace round-trip required
//! for the rest of the team. This is the offline-first promise:
//! MoonLens must work on PCs where outbound HTTPS is firewalled.
//!
//! Format
//! ------
//!
//! Single `.moonlens-model` file. Internally a ZIP store with two
//! entries:
//!
//! * `manifest.json` — typed metadata (model id, license, sha256,
//!   exporter version). Validated before the GGUF is touched so a
//!   tampered or wrong-format file is rejected up front.
//! * `<filename>.gguf` — verbatim copy of the GGUF the exporting PC
//!   already verified by running the model successfully.
//!
//! The manifest's sha256 covers the GGUF entry's bytes, NOT the
//! whole zip. That keeps verification cheap (one streaming pass over
//! the largest entry) and means the manifest itself can be inspected
//! without trusting the GGUF.
//!
//! Why ZIP and not a hand-rolled format
//! ------------------------------------
//!
//! The `zip` crate is already a project dependency (used by the
//! plugin loader) and standard tooling (Windows Explorer, macOS
//! Archive Utility, every IT-admin script in existence) can open
//! `.moonlens-model` files for inspection. Compression level is set
//! to `Stored` because GGUFs are already quantised binary and
//! compress under 1 % — paying CPU time for that is a regression on
//! the weak office hardware this feature targets.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

/// Filename for the manifest entry inside the ZIP. Stable so future
/// versions of the importer can still read older packages.
pub const MANIFEST_ENTRY: &str = "manifest.json";

/// Identifies the package shape so a future format change can be
/// rolled out without silently overwriting a model with a malformed
/// file. Bump on incompatible changes; older versions are loaded
/// best-effort.
pub const MANIFEST_FORMAT_VERSION: u32 = 1;

/// Identifies a MoonLens package vs. some random zip the user
/// dragged in. The `kind` field guards against mistaken imports of,
/// say, an OCR plugin archive.
pub const PACKAGE_KIND: &str = "moonlens.embedded_model";

/// File extension the export IPC suggests in the save dialog. Any
/// extension is technically accepted on import (we read the bytes
/// regardless of name), but standardising on `.moonlens-model` makes
/// the file identifiable in shared drives.
pub const PACKAGE_EXTENSION: &str = "moonlens-model";

/// Typed metadata serialised as the package's `manifest.json`. Every
/// field that travels with the GGUF needs to live here — the importer
/// is intentionally cautious and will not infer anything about the
/// model from the filename alone.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageManifest {
    /// Constant package kind. Set to [`PACKAGE_KIND`].
    pub kind: String,
    /// Format version, [`MANIFEST_FORMAT_VERSION`] for currently
    /// produced packages. Loader rejects unknown future versions.
    pub format_version: u32,
    /// Model id (e.g. `embedded.llm`). The importer writes the
    /// GGUF to `<app-data>/moonlens/models/<model_filename>` and
    /// updates `[providers.configs."<model_id>"]` in user.toml so
    /// the right provider picks it up.
    pub model_id: String,
    /// Primary filename to use on the importing PC. For multi-part
    /// tiers this is the first part; subsequent parts live in
    /// [`Self::parts`]. Mirrors the `filename_remote` field the
    /// downloader produces, which keeps "model downloaded on PC A"
    /// and "model imported from package on PC B" producing the
    /// same on-disk layout.
    pub model_filename: String,
    /// Upstream HuggingFace repo and filename the package was
    /// originally built from. Recorded so the importer can update
    /// `repo` / `filename_remote` in user.toml if the user later
    /// wants to re-download instead of re-importing.
    pub source_repo: String,
    pub source_filename: String,
    /// SHA-256 of the **primary** GGUF bytes (`model_filename`).
    /// For multi-part tiers each additional part has its own
    /// sha256 in [`Self::parts`]. Recomputed on import; mismatch
    /// aborts the extraction without touching the user's existing
    /// model.
    pub sha256: String,
    /// Total bytes of the **primary** GGUF entry. Multi-part tiers
    /// surface the sum across parts via the importer's outcome.
    pub size_bytes: u64,
    /// Additional parts beyond `model_filename`. Empty for
    /// single-file tiers (which is the vast majority of packages —
    /// `Vec::is_empty()` round-trips through serde with the
    /// default `#[serde(default)]` so older v1 packages without
    /// this field still parse).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<PackageManifestPart>,
    /// SPDX-style license advert (e.g. `"Apache-2.0"`). Required
    /// because corporate deployments often need a license audit
    /// before importing.
    pub license: Option<String>,
    /// URL to the upstream license text. Lets IT click through to
    /// the canonical source without trusting the package author.
    pub license_url: Option<String>,
    /// `MoonLens` version string the package was built with. Bumped
    /// in lockstep with `Cargo.toml` so the importer can warn on a
    /// version skew (info only — does not block import).
    pub exporter_version: String,
    /// RFC 3339 export timestamp. Surfaced in the import dialog
    /// alongside the model id.
    pub exported_at: String,
    /// Optional human-readable note (the exporting admin can add
    /// "for office floor 5", "QA team build", etc.).
    pub note: Option<String>,
}

/// One additional part in a multi-file GGUF package. Stored
/// separately from the primary so v1 (single-file) consumers can
/// continue to deserialise the manifest without seeing extra fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageManifestPart {
    /// Filename inside the zip and on the importing PC's disk.
    pub filename: String,
    /// SHA-256 of the part's bytes.
    pub sha256: String,
    /// Bytes of the part. Useful for the importer's confirmation
    /// UI so it can render "1.2 GB / 1.5 GB" before extracting.
    pub size_bytes: u64,
}

impl PackageManifest {
    pub fn validate(&self) -> Result<()> {
        if self.kind != PACKAGE_KIND {
            return Err(anyhow!(
                "wrong package kind `{}`, expected `{}`",
                self.kind,
                PACKAGE_KIND
            ));
        }
        if self.format_version > MANIFEST_FORMAT_VERSION {
            return Err(anyhow!(
                "package format version {} is newer than this MoonLens \
                 ({}); upgrade MoonLens to import",
                self.format_version,
                MANIFEST_FORMAT_VERSION
            ));
        }
        if self.model_filename.is_empty() {
            return Err(anyhow!("manifest is missing model_filename"));
        }
        validate_hex_sha256("primary", &self.sha256)?;
        for part in &self.parts {
            if part.filename.is_empty() {
                return Err(anyhow!("manifest part has empty filename"));
            }
            if part.filename == self.model_filename {
                return Err(anyhow!(
                    "manifest lists `{}` as both primary and part",
                    part.filename
                ));
            }
            validate_hex_sha256(&part.filename, &part.sha256)?;
        }
        Ok(())
    }
}

fn validate_hex_sha256(label: &str, sha256: &str) -> Result<()> {
    if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!(
            "manifest {label} sha256 is not a 64-char hex string: {}",
            sha256
        ));
    }
    Ok(())
}

/// Streaming SHA-256 over a file. Used both at export time (to
/// stamp the manifest) and at import time (to verify the package).
pub fn sha256_of_file(path: &Path) -> Result<(String, u64)> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = reader.read(&mut buf).context("read for sha256")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    let digest = hasher.finalize();
    Ok((hex::encode(digest), total))
}

/// Streaming SHA-256 over a `Read`. Used by the importer to verify
/// the GGUF entry while it streams out of the ZIP, without writing
/// it to disk twice.
pub fn sha256_of_reader<R: Read>(mut reader: R) -> Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = reader.read(&mut buf).context("read for sha256")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }
    Ok((hex::encode(hasher.finalize()), total))
}

/// Build a `.moonlens-model` package from an on-disk GGUF.
///
/// `source_gguf` MUST already be a fully downloaded model — the
/// caller is responsible for that (we do not trigger a download
/// here, the IPC layer composes that). For multi-part tiers, the
/// additional parts are looked up under `source_gguf.parent()`
/// keyed by `manifest_template.extra_filenames`. `dest` is
/// overwritten if it exists. Returns the manifest written into the
/// package so the IPC layer can echo it back to the UI for
/// confirmation.
pub fn build_package(
    source_gguf: &Path,
    dest: &Path,
    manifest_template: PackageManifestInput,
) -> Result<PackageManifest> {
    if !source_gguf.is_file() {
        return Err(anyhow!("source GGUF not found: {}", source_gguf.display()));
    }

    // Hash up front so the manifest reflects the bytes we are about
    // to embed. Re-reading the file twice (once for hash, once for
    // copy) is an explicit choice: we want the hash to match the
    // exact bytes that landed in the ZIP, even if a parallel
    // process later edits the source.
    let (sha256, size_bytes) = sha256_of_file(source_gguf)?;

    let parent = source_gguf
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| anyhow!("source GGUF has no parent dir: {}", source_gguf.display()))?;

    // Hash every additional part under the same dir. If a part is
    // missing the build aborts cleanly — half-packaged multi-file
    // GGUFs would be unloadable on the receiving end.
    let mut parts: Vec<(PathBuf, PackageManifestPart)> =
        Vec::with_capacity(manifest_template.extra_filenames.len());
    for extra in &manifest_template.extra_filenames {
        let part_path = parent.join(extra);
        if !part_path.is_file() {
            return Err(anyhow!(
                "multi-part GGUF: missing part `{}` in {}",
                extra,
                parent.display()
            ));
        }
        let (part_sha, part_size) = sha256_of_file(&part_path)?;
        parts.push((
            part_path,
            PackageManifestPart {
                filename: extra.clone(),
                sha256: part_sha,
                size_bytes: part_size,
            },
        ));
    }

    let manifest = PackageManifest {
        kind: PACKAGE_KIND.to_string(),
        format_version: MANIFEST_FORMAT_VERSION,
        model_id: manifest_template.model_id,
        model_filename: manifest_template.model_filename,
        source_repo: manifest_template.source_repo,
        source_filename: manifest_template.source_filename,
        sha256,
        size_bytes,
        parts: parts.iter().map(|(_, p)| p.clone()).collect(),
        license: manifest_template.license,
        license_url: manifest_template.license_url,
        exporter_version: manifest_template.exporter_version,
        exported_at: manifest_template.exported_at,
        note: manifest_template.note,
    };
    manifest.validate()?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create parent dir for {}", dest.display()))?;
    }

    let dest_file = File::create(dest).with_context(|| format!("create {}", dest.display()))?;
    let mut zip = zip::ZipWriter::new(BufWriter::new(dest_file));

    // GGUFs are already quantised → compression level adds CPU time
    // for ~0% size win. `Stored` keeps the package fast to write +
    // fast to extract, which matters on the weak hardware this
    // feature targets.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    let manifest_bytes = serde_json::to_vec_pretty(&manifest).context("serialise manifest")?;
    zip.start_file(MANIFEST_ENTRY, stored)
        .context("start manifest entry")?;
    zip.write_all(&manifest_bytes)
        .context("write manifest entry")?;

    // Stream the primary GGUF first, then each part in declared
    // order. Buffer size is 1 MiB so the disk seek cost is amortised
    // even on slow corporate-laptop SSDs.
    let mut buf = [0u8; 1024 * 1024];
    let stream_into_zip = |zip: &mut zip::ZipWriter<BufWriter<File>>,
                           name: &str,
                           path: &Path,
                           buf: &mut [u8]|
     -> Result<()> {
        zip.start_file(name, stored)
            .with_context(|| format!("start zip entry {}", name))?;
        let mut src =
            BufReader::new(File::open(path).with_context(|| format!("open {}", path.display()))?);
        loop {
            let n = src
                .read(buf)
                .with_context(|| format!("read {}", path.display()))?;
            if n == 0 {
                break;
            }
            zip.write_all(&buf[..n])
                .with_context(|| format!("write zip entry {}", name))?;
        }
        Ok(())
    };

    stream_into_zip(&mut zip, &manifest.model_filename, source_gguf, &mut buf)?;
    for (path, part) in &parts {
        stream_into_zip(&mut zip, &part.filename, path, &mut buf)?;
    }

    zip.finish().context("finalise zip")?;
    Ok(manifest)
}

/// Inputs the IPC layer passes to [`build_package`]. The split
/// keeps the file-system concerns (paths, disk IO) and the metadata
/// concerns (model_id, license) cleanly separated.
#[derive(Debug, Clone)]
pub struct PackageManifestInput {
    pub model_id: String,
    pub model_filename: String,
    /// Additional part filenames (multi-file GGUF). Empty for
    /// single-file packages. The on-disk paths are derived by
    /// joining each name to `source_gguf.parent()` at build time.
    pub extra_filenames: Vec<String>,
    pub source_repo: String,
    pub source_filename: String,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub exporter_version: String,
    pub exported_at: String,
    pub note: Option<String>,
}

/// Inspect a package without committing to import. Reads the
/// manifest (cheap, KB-sized) and validates its shape. The IPC
/// layer uses this for the import confirmation dialog so the user
/// sees model id, size, license, and exporter version BEFORE the
/// big GGUF write touches disk.
pub fn inspect_package(package: &Path) -> Result<PackageManifest> {
    let file =
        File::open(package).with_context(|| format!("open package {}", package.display()))?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).context("open package as zip")?;
    let mut manifest_entry = zip
        .by_name(MANIFEST_ENTRY)
        .context("manifest.json not found in package")?;
    let mut buf = String::new();
    manifest_entry
        .read_to_string(&mut buf)
        .context("read manifest")?;
    let manifest: PackageManifest = serde_json::from_str(&buf).context("parse manifest")?;
    manifest.validate()?;
    Ok(manifest)
}

/// Extract a package's GGUF(s) into `target_dir`. Verifies every
/// part's SHA-256 against the manifest before renaming any temp file
/// into place, so a corrupted package never overwrites a working
/// model — even partially. Returns the final on-disk path of the
/// **primary** GGUF (multi-part tiers also write their additional
/// parts beside it under the same `target_dir`).
pub fn apply_package(package: &Path, target_dir: &Path) -> Result<(PathBuf, PackageManifest)> {
    let manifest = inspect_package(package)?;

    std::fs::create_dir_all(target_dir)
        .with_context(|| format!("create target dir {}", target_dir.display()))?;

    // First pass: extract every entry into a `.import.partial`
    // sibling and verify its sha256. We commit nothing until every
    // part has been verified — half-imports would leave the model
    // dir in an unloadable state.
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new();
    fn cleanup_partials(staged: &[(PathBuf, PathBuf)]) {
        for (tmp, _) in staged {
            let _ = std::fs::remove_file(tmp);
        }
    }
    let entries: Vec<(String, String, u64)> = std::iter::once((
        manifest.model_filename.clone(),
        manifest.sha256.clone(),
        manifest.size_bytes,
    ))
    .chain(
        manifest
            .parts
            .iter()
            .map(|p| (p.filename.clone(), p.sha256.clone(), p.size_bytes)),
    )
    .collect();

    let file =
        File::open(package).with_context(|| format!("re-open package {}", package.display()))?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).context("re-open package as zip")?;

    for (entry_name, expected_sha, expected_size) in &entries {
        let final_path = target_dir.join(entry_name);
        let tmp_path = final_path.with_extension("gguf.import.partial");
        let mut entry = zip.by_name(entry_name).with_context(|| {
            format!(
                "package missing GGUF entry `{}` declared in manifest",
                entry_name
            )
        })?;

        let mut tmp = BufWriter::new(
            File::create(&tmp_path)
                .with_context(|| format!("create temp {}", tmp_path.display()))?,
        );
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 1024 * 1024];
        let mut total: u64 = 0;
        loop {
            let n = entry
                .read(&mut buf)
                .with_context(|| format!("read entry `{}`", entry_name))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            tmp.write_all(&buf[..n])
                .with_context(|| format!("write temp `{}`", tmp_path.display()))?;
            total += n as u64;
        }
        tmp.flush()
            .with_context(|| format!("flush `{}`", tmp_path.display()))?;
        drop(tmp);

        let computed = hex::encode(hasher.finalize());
        if &computed != expected_sha {
            let _ = std::fs::remove_file(&tmp_path);
            cleanup_partials(&staged);
            return Err(anyhow!(
                "package entry `{}` sha256 mismatch (manifest {}, got {})",
                entry_name,
                expected_sha,
                computed
            ));
        }
        if total != *expected_size {
            let _ = std::fs::remove_file(&tmp_path);
            cleanup_partials(&staged);
            return Err(anyhow!(
                "package entry `{}` size mismatch (manifest {}, got {})",
                entry_name,
                expected_size,
                total
            ));
        }
        staged.push((tmp_path, final_path));
    }

    // Second pass: every part verified — commit by atomic rename.
    // On Windows `rename` fails if `final_path` already exists, so
    // remove first.
    for (tmp_path, final_path) in &staged {
        let _ = std::fs::remove_file(final_path);
        std::fs::rename(tmp_path, final_path)
            .with_context(|| format!("rename to {}", final_path.display()))?;
    }

    let primary_final = target_dir.join(&manifest.model_filename);
    Ok((primary_final, manifest))
}

/// Backwards-compat shim around `hex` so the file does not pull a
/// new dependency just for one helper. `hex::encode` is already
/// available transitively through `sha2` / `tauri` / `zip` deps so
/// this module is keeping the existing `hex` crate import surface
/// minimal.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        const TBL: &[u8; 16] = b"0123456789abcdef";
        let bytes = bytes.as_ref();
        let mut out = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            out.push(TBL[(b >> 4) as usize] as char);
            out.push(TBL[(b & 0x0f) as usize] as char);
        }
        out
    }
}

// `sha256_of_reader` is exported for future use cases (e.g. plugin
// signature verification reusing the streaming hasher); keep it
// referenced here so cargo's dead-code lint does not flag it when
// it has no in-tree caller yet.
#[allow(dead_code)]
fn _module_keeps_symbols_referenced() {
    let _ = sha256_of_reader::<&[u8]>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_fake_gguf(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).expect("write fake gguf");
        path
    }

    fn sample_template() -> PackageManifestInput {
        PackageManifestInput {
            model_id: "embedded.llm".into(),
            model_filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf".into(),
            extra_filenames: Vec::new(),
            source_repo: "Qwen/Qwen2.5-1.5B-Instruct-GGUF".into(),
            source_filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf".into(),
            license: Some("Apache-2.0".into()),
            license_url: Some("https://example.test/LICENSE".into()),
            exporter_version: "0.1.0".into(),
            exported_at: "2026-05-09T12:00:00Z".into(),
            note: Some("for floor-5 office".into()),
        }
    }

    #[test]
    fn round_trip_export_then_apply_recovers_identical_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let payload = b"This stands in for a real GGUF.".repeat(1024);
        let src = write_fake_gguf(tmp.path(), "model.gguf", &payload);
        let pkg = tmp.path().join("bundle.moonlens-model");
        let manifest = build_package(&src, &pkg, sample_template()).expect("export");

        // Inspect-only must not touch disk besides reading the zip.
        let inspected = inspect_package(&pkg).expect("inspect");
        assert_eq!(inspected, manifest);
        assert_eq!(inspected.size_bytes, payload.len() as u64);

        let target = tmp.path().join("imported");
        let (final_path, applied_manifest) = apply_package(&pkg, &target).expect("apply");
        assert_eq!(applied_manifest, manifest);
        let recovered = std::fs::read(&final_path).expect("read final");
        assert_eq!(
            recovered, payload,
            "extracted GGUF must match the source byte-for-byte"
        );
    }

    #[test]
    fn apply_rejects_corrupt_package_without_overwriting_existing_model() {
        let tmp = tempfile::tempdir().unwrap();
        let payload = b"original".repeat(64);
        let src = write_fake_gguf(tmp.path(), "model.gguf", &payload);
        let pkg = tmp.path().join("bundle.moonlens-model");
        build_package(&src, &pkg, sample_template()).unwrap();

        // Tamper: rewrite the manifest with a wrong sha256, then
        // re-zip. We do this by reading the zip, swapping the
        // manifest, and writing a new zip.
        let tampered = tmp.path().join("tampered.moonlens-model");
        {
            let in_file = File::open(&pkg).unwrap();
            let mut in_zip = zip::ZipArchive::new(BufReader::new(in_file)).unwrap();
            let out_file = File::create(&tampered).unwrap();
            let mut out_zip = zip::ZipWriter::new(BufWriter::new(out_file));
            let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            for i in 0..in_zip.len() {
                let mut entry = in_zip.by_index(i).unwrap();
                if entry.name() == MANIFEST_ENTRY {
                    let mut m: PackageManifest = serde_json::from_reader(&mut entry).unwrap();
                    m.sha256 = "0".repeat(64);
                    let bytes = serde_json::to_vec_pretty(&m).unwrap();
                    out_zip.start_file(MANIFEST_ENTRY, stored).unwrap();
                    out_zip.write_all(&bytes).unwrap();
                } else {
                    let name = entry.name().to_string();
                    out_zip.start_file(&name, stored).unwrap();
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf).unwrap();
                    out_zip.write_all(&buf).unwrap();
                }
            }
            out_zip.finish().unwrap();
        }

        // Drop a "victim" file in the target dir — it must NOT be
        // touched when the import fails.
        let target = tmp.path().join("imported");
        std::fs::create_dir_all(&target).unwrap();
        let victim = target.join("qwen2.5-1.5b-instruct-q4_k_m.gguf");
        std::fs::write(&victim, b"victim contents").unwrap();

        let err = apply_package(&tampered, &target).expect_err("must reject tampered package");
        assert!(
            err.to_string().contains("sha256 mismatch"),
            "error must mention sha256: {err}"
        );
        let still_there = std::fs::read(&victim).unwrap();
        assert_eq!(
            still_there, b"victim contents",
            "existing model MUST not be overwritten when verification fails"
        );
    }

    #[test]
    fn manifest_validation_rejects_wrong_kind() {
        let mut m = sample_template();
        let stripped = PackageManifest {
            kind: "some.other.format".into(),
            format_version: 1,
            model_id: m.model_id.clone(),
            model_filename: std::mem::take(&mut m.model_filename),
            source_repo: std::mem::take(&mut m.source_repo),
            source_filename: std::mem::take(&mut m.source_filename),
            sha256: "a".repeat(64),
            size_bytes: 1,
            parts: Vec::new(),
            license: m.license.clone(),
            license_url: m.license_url.clone(),
            exporter_version: m.exporter_version.clone(),
            exported_at: m.exported_at.clone(),
            note: m.note.clone(),
        };
        let err = stripped.validate().expect_err("wrong kind");
        assert!(err.to_string().contains("wrong package kind"));
    }

    #[test]
    fn manifest_validation_rejects_future_format_version() {
        let m = PackageManifest {
            kind: PACKAGE_KIND.to_string(),
            format_version: MANIFEST_FORMAT_VERSION + 1,
            model_id: "x".into(),
            model_filename: "x.gguf".into(),
            source_repo: "x".into(),
            source_filename: "x".into(),
            sha256: "a".repeat(64),
            size_bytes: 1,
            parts: Vec::new(),
            license: None,
            license_url: None,
            exporter_version: "0".into(),
            exported_at: "0".into(),
            note: None,
        };
        let err = m.validate().expect_err("future version");
        assert!(err.to_string().contains("newer than this MoonLens"));
    }

    #[test]
    fn manifest_validation_rejects_malformed_sha256() {
        let m = PackageManifest {
            kind: PACKAGE_KIND.to_string(),
            format_version: 1,
            model_id: "x".into(),
            model_filename: "x.gguf".into(),
            source_repo: "x".into(),
            source_filename: "x".into(),
            sha256: "not-hex".into(),
            size_bytes: 1,
            parts: Vec::new(),
            license: None,
            license_url: None,
            exporter_version: "0".into(),
            exported_at: "0".into(),
            note: None,
        };
        let err = m.validate().expect_err("bad sha256");
        assert!(err.to_string().contains("sha256"));
    }

    #[test]
    fn multi_part_round_trip_preserves_every_part() {
        // Build a package containing a primary GGUF + 2 extra
        // parts (mirrors the Qwen 2.5 7B Q4_K_M layout). Verify
        // every part round-trips byte-for-byte and the manifest
        // records each part's sha256 + size.
        let tmp = tempfile::tempdir().unwrap();
        let primary = write_fake_gguf(
            tmp.path(),
            "qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf",
            &b"PRIMARY".repeat(512),
        );
        write_fake_gguf(
            tmp.path(),
            "qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf",
            &b"PART2".repeat(700),
        );
        let pkg = tmp.path().join("bundle.moonlens-model");
        let mut tmpl = sample_template();
        tmpl.model_filename = "qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf".into();
        tmpl.extra_filenames = vec!["qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf".into()];
        let manifest = build_package(&primary, &pkg, tmpl).expect("build");
        assert_eq!(manifest.parts.len(), 1);
        assert_eq!(
            manifest.parts[0].filename,
            "qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf"
        );

        let target = tmp.path().join("imported");
        let (final_path, applied) = apply_package(&pkg, &target).expect("apply");
        assert!(final_path.ends_with("qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf"));
        let recovered_part2 =
            std::fs::read(target.join("qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf"))
                .expect("part 2 was extracted");
        assert_eq!(recovered_part2, b"PART2".repeat(700));
        assert_eq!(applied.parts.len(), 1);
    }

    #[test]
    fn multi_part_build_fails_when_a_part_is_missing() {
        // Defensive: if the user packages a model whose extras file
        // is gone (perhaps a half-finished download), the build
        // must error rather than ship an incomplete package.
        let tmp = tempfile::tempdir().unwrap();
        let primary = write_fake_gguf(tmp.path(), "primary.gguf", b"PRIMARY");
        let pkg = tmp.path().join("incomplete.moonlens-model");
        let mut tmpl = sample_template();
        tmpl.model_filename = "primary.gguf".into();
        tmpl.extra_filenames = vec!["missing-part-2.gguf".into()];
        let err = build_package(&primary, &pkg, tmpl).expect_err("must fail");
        assert!(
            err.to_string().contains("missing part"),
            "error must mention missing part: {err}"
        );
        assert!(
            !pkg.exists(),
            "incomplete package must not be written to disk"
        );
    }
}
