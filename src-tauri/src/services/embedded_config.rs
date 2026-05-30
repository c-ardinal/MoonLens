//! Single source of truth for "what model is the embedded provider
//! pointing at, given the current user.toml".
//!
//! Pre-7.x five different IPC commands open-coded the same merge:
//!
//! ```ignore
//! let snap = state.settings.snapshot();
//! let cfg = snap.providers.configs.get("embedded.llm").cloned().unwrap_or(Null);
//! let repo = cfg.get("repo").and_then(|v| v.as_str()).unwrap_or(DEFAULT_MODEL_REPO);
//! let filename = cfg.get("filename_remote").and_then(|v| v.as_str()).unwrap_or(DEFAULT_MODEL_FILENAME_REMOTE);
//! let model_path = cfg.get("model_path").and_then(|v| v.as_str()).map(PathBuf::from).unwrap_or_else(|| default_model_path(&app_data));
//! // ... license, license_url, ...
//! ```
//!
//! Each duplicate drifted slightly (one used `_remote`, another used
//! `filename`, two had different fallback orderings for `model_path`
//! vs `app_data`). [`ResolvedConfig::from_settings`] is the single
//! resolver — every consumer goes through it, no other code reads
//! the raw JSON.

use std::path::{Path, PathBuf};

use crate::providers::translate_embedded_llm::{
    default_model_path, DEFAULT_MODEL_FILENAME_REMOTE, DEFAULT_MODEL_LICENSE,
    DEFAULT_MODEL_LICENSE_URL, DEFAULT_MODEL_REPO,
};
use crate::settings::schema::Settings;

/// Resolved embedded-model configuration. All fields are populated;
/// no `Option` because the resolver always falls back to the bundled
/// defaults for missing keys. Consumers that want to know whether a
/// value came from user.toml vs the defaults can compare the field
/// to the corresponding `DEFAULT_*` constant directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    /// HuggingFace repo id (`owner/name`).
    pub repo: String,
    /// Primary remote filename inside `repo`. For single-part tiers
    /// this is the only file. For multi-part tiers (e.g. Qwen 2.5 7B
    /// Q4_K_M) this is the first part — additional parts live in
    /// [`Self::filenames_remote_extra`]. Existing user.toml files
    /// that only carry `filename_remote = "..."` continue to work
    /// as single-part configs because `filenames_remote_extra`
    /// defaults to empty.
    pub filename_remote: String,
    /// Additional parts after `filename_remote`. Empty for ordinary
    /// single-file GGUFs.
    pub filenames_remote_extra: Vec<String>,
    /// Absolute on-disk path the **primary** GGUF lives at. Either
    /// an explicit `model_path` override from user.toml or
    /// `<app-data>/moonlens/models/<filename_remote>`. For multi-part
    /// tiers, the additional parts sit alongside under the same
    /// directory (see [`Self::model_paths`]).
    pub model_path: PathBuf,
    /// SPDX-style license advert (e.g. `"Apache-2.0"`).
    pub license: String,
    /// Canonical license URL.
    pub license_url: String,
}

impl ResolvedConfig {
    /// All filenames in download / load order: primary then extras.
    /// Used by the downloader (one HTTP fetch per part) and the
    /// engine (`mistralrs::GgufModelBuilder` accepts the full list).
    pub fn all_filenames(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(1 + self.filenames_remote_extra.len());
        out.push(self.filename_remote.clone());
        out.extend(self.filenames_remote_extra.iter().cloned());
        out
    }

    /// All on-disk paths in the same order as [`Self::all_filenames`].
    /// Multi-part tiers store every part beside the primary in the
    /// resolved storage root (the parent of `model_path`).
    pub fn all_model_paths(&self) -> Vec<PathBuf> {
        let parent = self
            .model_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut out = Vec::with_capacity(1 + self.filenames_remote_extra.len());
        out.push(self.model_path.clone());
        for extra in &self.filenames_remote_extra {
            out.push(parent.join(extra));
        }
        out
    }
}

impl ResolvedConfig {
    /// Resolve the embedded provider's effective config.
    /// Preconditions: `app_data` is the directory `Pipeline::set_app_data_dir`
    /// was called with (i.e. `<platform-app-data>/moonlens/`).
    pub fn from_settings(settings: &Settings, app_data: &Path) -> Self {
        // 2026-05-28: when the user has opted into a routing tier,
        // the embedded-model surface (status badge, tier snapshot,
        // download target) MUST reflect the tier's primary model —
        // not the legacy `embedded.llm` config block. That block can
        // still carry a stale single-model filename from before tier
        // routing existed (a leftover Qwen-7B Quality entry, say),
        // which the `RoutedTranslator` never actually loads under a
        // Compact tier. Reading the tier here keeps the settings
        // status ("準備済み 626 MB / llm-jp-3 980M") and the runtime
        // dispatch in agreement instead of advertising a 4.36 GB Qwen
        // that no translate call will ever touch.
        if let Some(user_tier) = settings.providers.user_tier {
            if let Some(tier) =
                crate::providers::translate_routed::RoutedTranslator::tier_ids_for(user_tier)
                    .first()
                    .and_then(|id| crate::model_registry::tier_by_id(id))
            {
                return Self::from_tier(tier, app_data);
            }
        }
        let cfg = settings
            .providers
            .configs
            .get("embedded.llm")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        Self::from_json(&cfg, app_data)
    }

    /// Resolve directly from a registry tier. Used by
    /// [`from_settings`] when a routing `user_tier` is active so the
    /// embedded-model surface tracks the tier's primary model rather
    /// than the legacy `embedded.llm` config. Multi-part tiers (e.g.
    /// the split Quality GGUF) flow their extra parts through
    /// `filenames_remote_extra` so `present`/`size` aggregation still
    /// sees every file.
    fn from_tier(tier: &crate::model_registry::ModelTier, app_data: &Path) -> Self {
        let primary = tier.primary_filename().to_string();
        let filenames_remote_extra = tier
            .filenames
            .iter()
            .skip(1)
            .map(|s| (*s).to_string())
            .collect();
        let model_path = app_data.join("models").join(&primary);
        Self {
            repo: tier.repo.to_string(),
            filename_remote: primary,
            filenames_remote_extra,
            model_path,
            license: tier.license.to_string(),
            license_url: tier.license_url.to_string(),
        }
    }

    /// Variant that takes the raw JSON config blob directly. Exposed so
    /// callers that already have the snapshot's `configs` map can
    /// avoid re-fetching.
    pub fn from_json(cfg: &serde_json::Value, app_data: &Path) -> Self {
        let repo = cfg
            .get("repo")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_MODEL_REPO)
            .to_string();
        let filename_remote = cfg
            .get("filename_remote")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_MODEL_FILENAME_REMOTE)
            .to_string();
        // Resolve the extras list. Two equivalent sources are
        // supported so power users can pick whichever feels natural:
        // 1. An explicit `filenames_remote_extra` array in user.toml.
        // 2. The active tier in `model_registry::TIERS` matched by
        //    `(repo, filename_remote)` — extras come from there
        //    automatically. This is what makes "I picked Quality
        //    (7B)" work without forcing the user to hand-edit
        //    `filenames_remote_extra` for the second part.
        let extras_explicit: Option<Vec<String>> = cfg
            .get("filenames_remote_extra")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });
        let filenames_remote_extra = extras_explicit.unwrap_or_else(|| {
            crate::model_registry::tier_by_coord(&repo, &filename_remote)
                .map(|t| {
                    t.filenames
                        .iter()
                        .skip(1)
                        .map(|s| (*s).to_string())
                        .collect()
                })
                .unwrap_or_default()
        });
        // Bug fix landed in 7.1: previously the model_path fallback
        // resolved to `<app_data>/models/<DEFAULT_MODEL_FILE>` even
        // when the user had overridden `filename_remote`. That left
        // a tier switch from Standard → Compact pointing at the OLD
        // GGUF on disk while the downloader fetched the NEW one,
        // diverging tier UI from runtime state. Now we honour the
        // override: the path tracks `filename_remote`, and only an
        // explicit `model_path` config wins over both.
        let model_path = cfg
            .get("model_path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if filename_remote == DEFAULT_MODEL_FILENAME_REMOTE {
                    default_model_path(app_data)
                } else {
                    app_data.join("models").join(&filename_remote)
                }
            });
        let license = cfg
            .get("license")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_MODEL_LICENSE)
            .to_string();
        let license_url = cfg
            .get("license_url")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_MODEL_LICENSE_URL)
            .to_string();
        Self {
            repo,
            filename_remote,
            filenames_remote_extra,
            model_path,
            license,
            license_url,
        }
    }

    /// Stable `<repo>:<filename>` coord, matching
    /// `ModelTier::coord` so a `ResolvedConfig` round-trips through
    /// `model_registry::tier_by_coord` cleanly.
    pub fn coord(&self) -> String {
        format!("{}:{}", self.repo, self.filename_remote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Settings with no routing tier selected, so `from_settings`
    /// exercises the LEGACY `embedded.llm` config-resolution path
    /// these tests target. default.toml ships `user_tier =
    /// "standard"`; clearing it keeps the resolver reading the
    /// config block instead of following the tier (which the
    /// dedicated `*_when_user_tier_*` tests cover separately).
    fn empty_settings() -> Settings {
        let mut s: Settings = toml::from_str(include_str!("../settings/default.toml")).unwrap();
        s.providers.user_tier = None;
        s
    }

    /// Variant that keeps default.toml's `user_tier = "standard"`
    /// so the tier-following branch of `from_settings` is reachable.
    fn settings_with_tier(tier: crate::translation_router::UserTier) -> Settings {
        let mut s: Settings = toml::from_str(include_str!("../settings/default.toml")).unwrap();
        s.providers.user_tier = Some(tier);
        s
    }

    #[test]
    fn resolves_to_defaults_when_user_toml_is_silent() {
        // Synthesise a Settings whose `embedded.llm` config is
        // empty. The resolver must produce all five fields from the
        // bundled defaults rather than panic / return blanks.
        let mut s = empty_settings();
        s.providers.configs.remove("embedded.llm");
        let r = ResolvedConfig::from_settings(&s, Path::new("C:\\moonlens"));
        assert_eq!(r.repo, DEFAULT_MODEL_REPO);
        assert_eq!(r.filename_remote, DEFAULT_MODEL_FILENAME_REMOTE);
        assert_eq!(
            r.model_path,
            Path::new("C:\\moonlens\\models").join(DEFAULT_MODEL_FILENAME_REMOTE)
        );
        assert_eq!(r.license, DEFAULT_MODEL_LICENSE);
    }

    #[test]
    fn user_overrides_take_precedence_over_defaults() {
        let mut s = empty_settings();
        s.providers.configs.insert(
            "embedded.llm".into(),
            serde_json::json!({
                "repo": "myorg/custom-finetune-GGUF",
                "filename_remote": "custom-finetune-q4_k_m.gguf",
                "license": "Apache-2.0",
                "license_url": "https://example.test/LICENSE",
            }),
        );
        let r = ResolvedConfig::from_settings(&s, Path::new("/tmp/moonlens"));
        assert_eq!(r.repo, "myorg/custom-finetune-GGUF");
        assert_eq!(r.filename_remote, "custom-finetune-q4_k_m.gguf");
        // `model_path` was not overridden → resolves to <app_data>/models/<filename>.
        assert_eq!(
            r.model_path,
            Path::new("/tmp/moonlens/models").join("custom-finetune-q4_k_m.gguf")
        );
    }

    #[test]
    fn explicit_model_path_pins_an_absolute_location() {
        let mut s = empty_settings();
        s.providers.configs.insert(
            "embedded.llm".into(),
            serde_json::json!({"model_path": "C:\\custom\\model.gguf"}),
        );
        let r = ResolvedConfig::from_settings(&s, Path::new("C:\\moonlens"));
        // Even though the user did not override `repo`/`filename_remote`,
        // `model_path` MUST win — they may have moved the GGUF to a
        // shared drive without re-downloading.
        assert_eq!(r.model_path.to_str(), Some("C:\\custom\\model.gguf"));
        assert_eq!(r.repo, DEFAULT_MODEL_REPO);
    }

    #[test]
    fn coord_round_trips_through_registry_lookup() {
        use crate::model_registry::tier_by_coord;
        let s = empty_settings();
        let r = ResolvedConfig::from_settings(&s, Path::new("/x"));
        let coord = r.coord();
        let (repo, filename) = coord.split_once(':').unwrap();
        // The default config must point at a registered tier so the
        // settings UI can show the matching dropdown entry.
        assert!(
            tier_by_coord(repo, filename).is_some(),
            "default coord {} not registered in model_registry",
            coord
        );
    }

    #[test]
    fn from_json_handles_null_blob_without_panic() {
        // The IPC code paths used to call `.unwrap_or(Null)` and
        // then index into the resulting JSON — the Null branch is
        // exercised on a fresh install where the user.toml has no
        // `[providers.configs."embedded.llm"]` block at all.
        let r = ResolvedConfig::from_json(&serde_json::Value::Null, Path::new("/x"));
        assert_eq!(r.repo, DEFAULT_MODEL_REPO);
        assert_eq!(r.license, DEFAULT_MODEL_LICENSE);
    }

    #[test]
    fn resolver_is_cheap_to_clone_for_per_call_use() {
        // Sanity: ResolvedConfig is `Clone` so consumers don't have
        // to pass `&ResolvedConfig` around. If a future change makes
        // it !Clone, IPC commands will not compile and the author
        // will see this test guarding the contract.
        let s = empty_settings();
        let r = ResolvedConfig::from_settings(&s, Path::new("/x"));
        let r2 = r.clone();
        assert_eq!(r, r2);
    }

    #[test]
    fn follows_user_tier_compact_over_embedded_config() {
        use crate::translation_router::UserTier;
        // Repro of the 2026-05-28 report: a stale `embedded.llm`
        // config still points at the split Qwen-7B Quality model
        // (from a prior tier selection), but the user has switched
        // to the Compact routing tier. The resolver MUST follow the
        // tier (llm-jp-3 980M), NOT the dead Qwen config — otherwise
        // the settings UI advertises a 4.36 GB model no translate
        // call will ever load.
        let mut s = settings_with_tier(UserTier::Compact);
        s.providers.configs.insert(
            "embedded.llm".into(),
            serde_json::json!({
                "repo": "Qwen/Qwen2.5-7B-Instruct-GGUF",
                "filename_remote": "qwen2.5-7b-instruct-q4_k_m-00001-of-00002.gguf",
                "filenames_remote_extra": ["qwen2.5-7b-instruct-q4_k_m-00002-of-00002.gguf"],
                "license": "Apache-2.0",
            }),
        );
        let r = ResolvedConfig::from_settings(&s, Path::new("/tmp/moonlens"));
        assert_eq!(
            r.filename_remote, "llm-jp-3-980m-instruct3-Q4_K_M.gguf",
            "Compact tier must resolve to llm-jp-3 980M, not the stale Qwen config"
        );
        assert!(
            !r.coord().contains("qwen2.5-7b"),
            "stale Qwen config must NOT leak through when a tier is active; got {}",
            r.coord()
        );
        // The model_path tracks the tier filename inside app-data.
        assert_eq!(
            r.model_path,
            Path::new("/tmp/moonlens/models").join("llm-jp-3-980m-instruct3-Q4_K_M.gguf")
        );
    }

    #[test]
    fn follows_user_tier_standard_to_ja_bridge() {
        use crate::translation_router::UserTier;
        // Standard's primary (aux) tier is the JA bridge
        // (hy-mt2-1.8b). The legacy config is irrelevant.
        let s = settings_with_tier(UserTier::Standard);
        let r = ResolvedConfig::from_settings(&s, Path::new("/x"));
        assert!(
            r.coord().to_lowercase().contains("hy-mt2-1.8b"),
            "Standard tier should resolve to the JA bridge hy-mt2-1.8b; got {}",
            r.coord()
        );
    }

    #[test]
    fn tier_following_coord_is_registry_resolvable() {
        use crate::model_registry::tier_by_coord;
        use crate::translation_router::UserTier;
        // Every tier-followed coord must round-trip through the
        // registry so the settings dropdown can highlight the
        // matching entry rather than rendering "カスタム".
        for tier in [UserTier::Compact, UserTier::Standard, UserTier::Quality] {
            let s = settings_with_tier(tier);
            let r = ResolvedConfig::from_settings(&s, Path::new("/x"));
            let coord = r.coord();
            let (repo, filename) = coord.split_once(':').unwrap();
            assert!(
                tier_by_coord(repo, filename).is_some(),
                "tier {tier:?} coord {coord} not registered in model_registry",
            );
        }
    }
}
