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

    /// Snapshot of every known manifest, regardless of enabled state.
    /// Used by the update checker so it can query `update_url` for each
    /// plugin without locking the registry for the duration of the
    /// network requests.
    pub fn manifests(&self) -> Vec<PluginManifest> {
        self.plugins
            .read()
            .iter()
            .map(|l| l.manifest.clone())
            .collect()
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

    /// Filesystem path of the on-disk directory backing a plugin id.
    /// Used by IPC commands that need to reveal or remove the plugin
    /// without leaking storage layout to the frontend.
    pub fn path_of(&self, id: &str) -> Option<PathBuf> {
        self.plugins
            .read()
            .iter()
            .find(|l| l.manifest.plugin.id == id)
            .map(|l| l.path.clone())
    }

    /// Drop the plugin instance, delete its directory, and clear it
    /// from the registry + settings. Returns an error only when the
    /// directory removal fails — a missing plugin id is treated as
    /// already-gone (no-op).
    pub fn uninstall(&self, id: &str) -> Result<()> {
        let path: Option<PathBuf> = {
            let mut guard = self.plugins.write();
            let pos = guard.iter().position(|l| l.manifest.plugin.id == id);
            match pos {
                Some(i) => {
                    let removed = guard.swap_remove(i);
                    Some(removed.path)
                }
                None => None,
            }
        };
        if let Some(p) = path {
            if p.exists() {
                std::fs::remove_dir_all(&p)
                    .with_context(|| format!("remove plugin dir {}", p.display()))?;
            }
        }
        // Drop the enabled flag from settings so a future scan doesn't
        // resurrect the entry as "enabled by default".
        let snapshot = self.settings.snapshot();
        let mut s = (*snapshot).clone();
        if s.plugins.enabled.remove(id).is_some() {
            self.settings.save(s)?;
        }
        Ok(())
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let snapshot = self.settings.snapshot();
        if enabled && snapshot.plugins.require_signature {
            // Look up the plugin manifest + path up front so we can
            // verify before mutating any state.
            let (manifest, path) = {
                let guard = self.plugins.read();
                let l = guard
                    .iter()
                    .find(|l| l.manifest.plugin.id == id)
                    .ok_or_else(|| anyhow::anyhow!("plugin {id} not found"))?;
                (l.manifest.clone(), l.path.clone())
            };
            verify_plugin_signature(&manifest, &path, &snapshot.plugins.trusted_publishers)?;
        }

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
        let mut s = (*snapshot).clone();
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

fn verify_plugin_signature(
    manifest: &PluginManifest,
    plugin_dir: &Path,
    trusted: &[String],
) -> Result<()> {
    use super::signature;
    let signature_b64 =
        manifest.plugin.signature.as_deref().ok_or_else(|| {
            anyhow::anyhow!("signature required but plugin.toml has no `signature`")
        })?;
    let entry = plugin_dir.join(&manifest.plugin.entry);
    let digest = signature::sha256_file(&entry)?;
    let parsed: Result<Vec<_>> = trusted
        .iter()
        .map(|s| signature::parse_trusted(s))
        .collect();
    let keys = parsed.context("trusted_publishers contains an invalid entry")?;
    if keys.is_empty() {
        anyhow::bail!("require_signature is on but no trusted publishers are configured");
    }
    let publisher = signature::verify_digest(&digest, signature_b64, &keys)
        .context("signature verification failed")?;
    tracing::info!("plugin {} signed by {publisher}", manifest.plugin.id);
    Ok(())
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
