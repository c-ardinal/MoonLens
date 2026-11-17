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

        let mut settings: Settings =
            toml::from_str(DEFAULT_TOML).context("bundled default.toml is invalid")?;
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(raw) => match toml::from_str::<Settings>(&raw) {
                    Ok(user) => merge(&mut settings, user),
                    Err(e) => tracing::warn!("user config invalid, ignoring: {e}"),
                },
                Err(e) => tracing::warn!("cannot read user config: {e}"),
            }
            if super::migrate::migrate_legacy_defaults(&mut settings) {
                tracing::info!("upgraded legacy provider defaults in user config");
            }
        } else {
            // Seed the user file with defaults so the user can edit it.
            let _ = std::fs::write(&config_path, DEFAULT_TOML);
        }

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
        let mut settings: Settings =
            toml::from_str(DEFAULT_TOML).context("bundled default.toml is invalid")?;
        if self.config_path.exists() {
            let raw = std::fs::read_to_string(&self.config_path).context("re-read user config")?;
            match toml::from_str::<Settings>(&raw) {
                Ok(user) => merge(&mut settings, user),
                Err(e) => {
                    tracing::warn!("user config invalid on reload, keeping previous: {e}");
                    return Ok(false);
                }
            }
            super::migrate::migrate_legacy_defaults(&mut settings);
        }
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
        let serialized = toml::to_string_pretty(&new)?;
        std::fs::write(&self.config_path, serialized)?;
        *self.inner.write() = new.clone();
        let _ = self.tx.send(Arc::new(new));
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

/// Shallow merge: any field set in `user` replaces the corresponding field in
/// `base`. For nested structs the user value is taken as a whole; finer merging
/// is done at the TOML level via `#[serde(default)]` on each struct.
fn merge(base: &mut Settings, user: Settings) {
    *base = user;
}
