//! Settings store: loads layered TOML, publishes snapshots, hot-reloads.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use parking_lot::RwLock;
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast;

use super::schema::Settings;

const DEFAULT_TOML: &str = include_str!("default.toml");

pub struct SettingsStore {
    inner: RwLock<Settings>,
    config_path: PathBuf,
    tx: broadcast::Sender<Arc<Settings>>,
}

impl SettingsStore {
    pub fn load_or_default(app: &AppHandle) -> Result<Self> {
        let config_dir = app.path().app_config_dir().context("no app config dir")?;
        std::fs::create_dir_all(&config_dir).ok();
        let config_path = config_dir.join("config.toml");
        Self::load_from(config_path)
    }

    /// Tauri-free entry point: the caller provides the on-disk path.
    /// Integration tests use this to operate on a temp directory without
    /// spinning up a full Tauri runtime.
    pub fn load_from(config_path: PathBuf) -> Result<Self> {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let settings = if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(raw) => layer_user_over_defaults(&raw)
                    .unwrap_or_else(|e| {
                        tracing::warn!("user config invalid, falling back to defaults: {e}");
                        toml::from_str(DEFAULT_TOML).expect("bundled default.toml is invalid")
                    }),
                Err(e) => {
                    tracing::warn!("cannot read user config, using defaults: {e}");
                    toml::from_str(DEFAULT_TOML).context("bundled default.toml is invalid")?
                }
            }
        } else {
            // Seed the user file with defaults so the user can edit it.
            let _ = std::fs::write(&config_path, DEFAULT_TOML);
            toml::from_str(DEFAULT_TOML).context("bundled default.toml is invalid")?
        };

        let (tx, _) = broadcast::channel(16);
        Ok(Self {
            inner: RwLock::new(settings),
            config_path,
            tx,
        })
    }

    pub fn snapshot(&self) -> Arc<Settings> {
        Arc::new(self.inner.read().clone())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Settings>> {
        self.tx.subscribe()
    }

    /// Re-read the on-disk TOML, layer it over the bundled defaults, and
    /// broadcast the new snapshot if anything changed. Used by the
    /// file-watcher for hot reload.
    pub fn reload_from_disk(&self) -> Result<bool> {
        let settings = if self.config_path.exists() {
            let raw =
                std::fs::read_to_string(&self.config_path).context("re-read user config")?;
            match layer_user_over_defaults(&raw) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("user config invalid on reload, keeping previous: {e}");
                    return Ok(false);
                }
            }
        } else {
            toml::from_str(DEFAULT_TOML).context("bundled default.toml is invalid")?
        };
        let changed = {
            let current = self.inner.read();
            toml::to_string(&*current).ok() != toml::to_string(&settings).ok()
        };
        if changed {
            *self.inner.write() = settings.clone();
            let _ = self.tx.send(Arc::new(settings));
        }
        Ok(changed)
    }

    pub fn save(&self, new: Settings) -> Result<()> {
        // CRITICAL ordering: update the in-memory state BEFORE the
        // disk write so that a write failure (read-only fs, locked
        // user.toml, full disk) does not leave the running session
        // out of sync with itself. Pre-13.x the disk write came
        // first via `?`, which silently skipped the in-memory
        // update on failure — the rest of startup then ran against
        // stale state, e.g. the embedded settings migration would
        // appear to succeed in-memory but the Pipeline still saw
        // the pre-migration `embedded.gemma` provider id.
        let serialized = toml::to_string_pretty(&new)?;
        *self.inner.write() = new.clone();
        let _ = self.tx.send(Arc::new(new));
        std::fs::write(&self.config_path, serialized)
            .context("write user config")?;
        Ok(())
    }

    /// JSON Schema for the current `Settings` type, consumed by the frontend
    /// to auto-generate a form.
    pub fn json_schema() -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(Settings)).expect("schema is serializable")
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
}

/// Layer a user-provided TOML on top of the bundled defaults at the
/// VALUE level, not the struct level. The previous shallow merge
/// (`*base = user`) made the user file's stale values shadow
/// every later default-toml change — adding a new `examples`
/// array, bumping `num_predict`, raising `num_ctx`, or any other
/// silent default tweak never reached returning users because
/// their on-disk config replaced the entire `Settings` value.
///
/// The TOML-level merge means: missing keys in the user file fall
/// through to the default, present keys win. So after the bundled
/// `num_predict` jumps from 2048 to 4096, a user whose config
/// never explicitly set `num_predict` automatically picks up the
/// new value; one who DID set it keeps their override.
fn layer_user_over_defaults(user_raw: &str) -> Result<Settings> {
    let default_value: toml::Value =
        toml::from_str(DEFAULT_TOML).context("bundled default.toml is invalid")?;
    let user_value: toml::Value =
        toml::from_str(user_raw).context("parse user config")?;
    let merged = deep_merge_toml(default_value, user_value);
    let merged_str = toml::to_string(&merged).context("re-serialise merged config")?;
    let settings: Settings =
        toml::from_str(&merged_str).context("merged config does not match Settings schema")?;
    Ok(settings)
}

/// Test-only re-export of the layered loader. Used by
/// `model_registry::tests` to construct a `Settings` that matches a
/// realistic user.toml without going through `SettingsStore::load_from`.
#[cfg(test)]
pub fn layer_user_over_defaults_for_test(
    defaults: &str,
    user_raw: &str,
) -> Result<Settings> {
    let default_value: toml::Value =
        toml::from_str(defaults).context("default.toml parse for test")?;
    let user_value: toml::Value =
        toml::from_str(user_raw).context("user toml parse for test")?;
    let merged = deep_merge_toml(default_value, user_value);
    let merged_str = toml::to_string(&merged).context("re-serialise merged config")?;
    let settings: Settings =
        toml::from_str(&merged_str).context("merged config does not match Settings schema")?;
    Ok(settings)
}

/// Recursive deep merge: tables are merged key-by-key, anything
/// else has the user value win. `inline` arrays are NOT spliced —
/// a chain or examples list set in the user file is taken whole
/// (otherwise the user could not REPLACE a default array, only
/// append to it).
fn deep_merge_toml(base: toml::Value, user: toml::Value) -> toml::Value {
    match (base, user) {
        (toml::Value::Table(mut base_tbl), toml::Value::Table(user_tbl)) => {
            for (k, user_v) in user_tbl {
                let merged_v = match base_tbl.remove(&k) {
                    Some(base_v) => deep_merge_toml(base_v, user_v),
                    None => user_v,
                };
                base_tbl.insert(k, merged_v);
            }
            toml::Value::Table(base_tbl)
        }
        (_, user_v) => user_v,
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    #[test]
    fn deep_merge_keeps_default_when_user_omits_field() {
        // Simulated default with two scalar fields.
        let default: toml::Value = toml::from_str(
            r#"
            num_predict = 4096
            num_ctx = 8192
            "#,
        )
        .unwrap();
        // User only overrides one — the other must come from default.
        let user: toml::Value = toml::from_str(
            r#"
            num_predict = 256
            "#,
        )
        .unwrap();
        let merged = deep_merge_toml(default, user);
        let merged_tbl = merged.as_table().unwrap();
        assert_eq!(merged_tbl["num_predict"].as_integer(), Some(256));
        assert_eq!(merged_tbl["num_ctx"].as_integer(), Some(8192));
    }

    #[test]
    fn deep_merge_recurses_into_nested_tables() {
        let default: toml::Value = toml::from_str(
            r#"
            [providers.configs."ollama.gemma4"]
            num_predict = 4096
            num_ctx = 8192
            model = "gemma4:e2b"
            "#,
        )
        .unwrap();
        let user: toml::Value = toml::from_str(
            r#"
            [providers.configs."ollama.gemma4"]
            model = "gemma4:e4b"
            "#,
        )
        .unwrap();
        let merged = deep_merge_toml(default, user);
        let cfg = &merged["providers"]["configs"]["ollama.gemma4"];
        assert_eq!(cfg["num_predict"].as_integer(), Some(4096));
        assert_eq!(cfg["num_ctx"].as_integer(), Some(8192));
        assert_eq!(cfg["model"].as_str(), Some("gemma4:e4b"));
    }

    #[test]
    fn layer_user_over_defaults_picks_up_new_default_fields() {
        // Stale user config from before `examples` was added: the
        // top-level translator block exists but lacks the new
        // few-shot array. The deep merge must surface the bundled
        // examples without losing the user's explicit overrides.
        let user_raw = r#"
[providers.configs."ollama.gemma4"]
model = "gemma4:e2b"
num_predict = 256
"#;
        let merged = layer_user_over_defaults(user_raw).expect("merge");
        let cfg = merged
            .providers
            .configs
            .get("ollama.gemma4")
            .expect("ollama.gemma4 present");
        // User's explicit num_predict wins.
        assert_eq!(cfg.get("num_predict").and_then(|v| v.as_u64()), Some(256));
        // num_ctx wasn't in user file → default 8192 surfaces.
        assert_eq!(cfg.get("num_ctx").and_then(|v| v.as_u64()), Some(8192));
        // examples array wasn't in user file → default array surfaces.
        let examples = cfg.get("examples").and_then(|v| v.as_array()).unwrap();
        assert!(!examples.is_empty(), "default examples must be inherited");
    }
}
