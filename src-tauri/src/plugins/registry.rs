//! Plugin discovery, enable/disable state, and provider registration.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use parking_lot::RwLock;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::providers::ProviderHandle;
use crate::settings::SettingsStore;

use super::{manifest::PluginManifest, sidecar_host::SidecarPlugin, wasm_host::WasmPlugin};

/// Lightweight view returned to the UI for listing/toggling plugins.
#[derive(Debug, Clone, Serialize)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub license: String,
    pub kind: String,
    pub enabled: bool,
    pub provides: Vec<String>,
    pub permissions_summary: Vec<String>,
    pub path: PathBuf,
}

struct Loaded {
    manifest: PluginManifest,
    path: PathBuf,
    enabled: bool,
    instance: Option<Instance>,
}

enum Instance {
    Wasm(WasmPlugin),
    Sidecar(SidecarPlugin),
}

pub struct PluginRegistry {
    settings: Arc<SettingsStore>,
    plugins_dir: PathBuf,
    plugins: RwLock<Vec<Loaded>>,
}

impl PluginRegistry {
    pub fn load(app: &AppHandle, settings: Arc<SettingsStore>) -> Result<Self> {
        let plugins_dir = app.path().app_data_dir()?.join("plugins");
        Self::load_from(plugins_dir, settings)
    }

    /// Tauri-free constructor: caller supplies the plugins directory.
    /// Used by integration tests to point at a temp folder.
    pub fn load_from(plugins_dir: PathBuf, settings: Arc<SettingsStore>) -> Result<Self> {
        std::fs::create_dir_all(&plugins_dir)?;
        let me = Self {
            settings: settings.clone(),
            plugins_dir,
            plugins: RwLock::new(Vec::new()),
        };
        me.rescan()?;
        Ok(me)
    }

    pub fn rescan(&self) -> Result<()> {
        let mut loaded = Vec::new();
        for entry in std::fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            match load_one(&path, &self.settings) {
                Ok(l) => loaded.push(l),
                Err(e) => tracing::warn!("skip plugin at {}: {e}", path.display()),
            }
        }
        *self.plugins.write() = loaded;
        Ok(())
    }

    pub fn list(&self) -> Vec<PluginDescriptor> {
        self.plugins
            .read()
            .iter()
            .map(|l| PluginDescriptor {
                id: l.manifest.plugin.id.clone(),
                name: l.manifest.plugin.name.clone(),
                version: l.manifest.plugin.version.clone(),
                author: l.manifest.plugin.author.clone(),
                license: l.manifest.plugin.license.clone(),
                kind: format!("{:?}", l.manifest.plugin.kind).to_lowercase(),
                enabled: l.enabled,
                provides: l.manifest.capabilities.provides.clone(),
                permissions_summary: summarize_permissions(&l.manifest.permissions),
                path: l.path.clone(),
            })
            .collect()
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let mut guard = self.plugins.write();
        if let Some(l) = guard.iter_mut().find(|l| l.manifest.plugin.id == id) {
            l.enabled = enabled;
            if enabled && l.instance.is_none() {
                l.instance = instantiate(&l.manifest, &l.path).ok();
            }
            if !enabled {
                l.instance = None;
            }
        }
        let mut s = (*self.settings.snapshot()).clone();
        s.plugins.enabled.insert(id.to_string(), enabled);
        self.settings.save(s)?;
        Ok(())
    }

    /// Aggregated provider handles from every enabled plugin, keyed by the
    /// chain id (`plugin.<id>.<cap>`).
    pub fn providers(&self) -> Vec<(String, ProviderHandle)> {
        let mut out = Vec::new();
        for l in self.plugins.read().iter() {
            if !l.enabled {
                continue;
            }
            let Some(inst) = &l.instance else { continue };
            let id = &l.manifest.plugin.id;
            match inst {
                Instance::Wasm(w) => {
                    for h in w.provider_handles(id) {
                        out.push(h);
                    }
                }
                Instance::Sidecar(s) => {
                    for h in s.provider_handles(id) {
                        out.push(h);
                    }
                }
            }
        }
        out
    }
}

fn load_one(dir: &Path, settings: &SettingsStore) -> Result<Loaded> {
    let manifest_path = dir.join("plugin.toml");
    let raw = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("missing plugin.toml at {}", manifest_path.display()))?;
    let manifest: PluginManifest = toml::from_str(&raw)?;
    let enabled = settings
        .snapshot()
        .plugins
        .enabled
        .get(&manifest.plugin.id)
        .copied()
        .unwrap_or(false);
    let instance = if enabled {
        instantiate(&manifest, dir).ok()
    } else {
        None
    };
    Ok(Loaded {
        manifest,
        path: dir.to_path_buf(),
        enabled,
        instance,
    })
}

fn instantiate(manifest: &PluginManifest, dir: &Path) -> Result<Instance> {
    match manifest.plugin.kind {
        super::PluginKind::Wasm => Ok(Instance::Wasm(WasmPlugin::load(manifest, dir)?)),
        super::PluginKind::Sidecar => Ok(Instance::Sidecar(SidecarPlugin::spawn(manifest, dir)?)),
    }
}

fn summarize_permissions(p: &super::manifest::Permissions) -> Vec<String> {
    let mut s = Vec::new();
    if !p.network.is_empty() {
        s.push(format!("network: {}", p.network.join(", ")));
    }
    if !p.fs_read.is_empty() {
        s.push(format!("fs_read: {}", p.fs_read.join(", ")));
    }
    if !p.fs_write.is_empty() {
        s.push(format!("fs_write: {}", p.fs_write.join(", ")));
    }
    if p.audio_capture {
        s.push("audio_capture".into());
    }
    if p.screen_capture {
        s.push("screen_capture".into());
    }
    if p.clipboard {
        s.push("clipboard".into());
    }
    if p.subprocess {
        s.push("subprocess".into());
    }
    s
}
