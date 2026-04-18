//! Plugin update checker.
//!
//! Each plugin may declare an `update_url` in its manifest. When the
//! user flips `settings.plugins.auto_update_check` on, the host fetches
//! that URL expecting a small JSON document of the shape
//!
//! ```json
//! { "version": "1.2.3", "download_url": "https://…/foo.moonlens-plugin",
//!   "notes": "optional text" }
//! ```
//!
//! If `version` semver-compares as strictly greater than the currently
//! installed version, we raise an [`UpdateAvailable`] entry. The check
//! itself never touches the filesystem; applying the update is a
//! follow-up user action through the installer.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::plugins::manifest::PluginManifest;

/// What a plugin's update endpoint should return.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Aggregated result surfaced to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateAvailable {
    pub plugin_id: String,
    pub installed_version: String,
    pub latest_version: String,
    pub download_url: Option<String>,
    pub notes: Option<String>,
}

/// Pure comparison used by both the async checker and its tests.
/// Returns `Some(UpdateAvailable)` when `remote.version` is strictly
/// greater than `local.plugin.version` under a minimal semver parser.
pub fn decide_update(local: &PluginManifest, remote: &UpdateManifest) -> Option<UpdateAvailable> {
    if !is_newer(&remote.version, &local.plugin.version) {
        return None;
    }
    Some(UpdateAvailable {
        plugin_id: local.plugin.id.clone(),
        installed_version: local.plugin.version.clone(),
        latest_version: remote.version.clone(),
        download_url: remote.download_url.clone(),
        notes: remote.notes.clone(),
    })
}

/// Relaxed semver: supports `major.minor.patch[-pre]`. Missing parts
/// compare as zero. Non-numeric segments sort after numeric ones so
/// `1.0.0-beta` < `1.0.0`.
pub fn is_newer(candidate: &str, baseline: &str) -> bool {
    cmp_versions(candidate, baseline) == std::cmp::Ordering::Greater
}

fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let (a_main, a_pre) = split_pre(a);
    let (b_main, b_pre) = split_pre(b);
    let am = parts(a_main);
    let bm = parts(b_main);
    for i in 0..3 {
        let av = am.get(i).copied().unwrap_or(0);
        let bv = bm.get(i).copied().unwrap_or(0);
        match av.cmp(&bv) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    // Release wins over pre-release.
    match (a_pre, b_pre) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(x), Some(y)) => x.cmp(y),
    }
}

fn split_pre(v: &str) -> (&str, Option<&str>) {
    match v.split_once('-') {
        Some((m, p)) => (m, Some(p)),
        None => (v, None),
    }
}

fn parts(main: &str) -> Vec<u64> {
    main.split('.')
        .filter_map(|s| s.trim().parse::<u64>().ok())
        .collect()
}

/// Fetch `update_url` from a single manifest and compare to the
/// installed version. Exposed for the async checker below and for
/// future manual-refresh UI actions.
pub async fn check_one(
    http: &reqwest::Client,
    manifest: &PluginManifest,
) -> Result<Option<UpdateAvailable>> {
    let url = manifest
        .plugin
        .update_url
        .as_deref()
        .context("plugin has no update_url")?;
    let resp = http
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?;
    let remote: UpdateManifest = resp.json().await.context("parse update manifest")?;
    Ok(decide_update(manifest, &remote))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(version: &str) -> PluginManifest {
        toml::from_str(&format!(
            r#"
            [plugin]
            id = "x"
            name = "X"
            version = "{version}"
            author = ""
            license = ""
            kind = "wasm"
            entry = "x.wasm"
            api_version = "1"
        "#
        ))
        .unwrap()
    }

    #[test]
    fn decide_update_flags_newer_patch() {
        let local = manifest("1.2.3");
        let remote = UpdateManifest {
            version: "1.2.4".into(),
            download_url: Some("https://x".into()),
            notes: None,
        };
        let u = decide_update(&local, &remote).unwrap();
        assert_eq!(u.latest_version, "1.2.4");
        assert_eq!(u.installed_version, "1.2.3");
    }

    #[test]
    fn decide_update_ignores_same_or_older() {
        let local = manifest("1.2.3");
        for v in ["1.2.3", "1.2.2", "0.9.9"] {
            let remote = UpdateManifest {
                version: v.into(),
                download_url: None,
                notes: None,
            };
            assert!(decide_update(&local, &remote).is_none(), "v = {v}");
        }
    }

    #[test]
    fn is_newer_compares_major_minor_patch() {
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
    }

    #[test]
    fn is_newer_treats_missing_parts_as_zero() {
        assert!(is_newer("1.1", "1.0.9"));
        assert!(!is_newer("1", "1.0.0"));
    }

    #[test]
    fn pre_release_is_less_than_release() {
        assert!(is_newer("1.0.0", "1.0.0-beta"));
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
        assert!(is_newer("1.0.0-rc2", "1.0.0-rc1"));
    }

    #[test]
    fn non_numeric_garbage_does_not_panic() {
        // Malformed remote payloads should never crash the checker.
        assert!(!is_newer("banana", "1.0.0"));
        assert!(is_newer("2.0.0", "banana"));
    }
}
