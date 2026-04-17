//! Named profiles that overlay on top of the base settings.
//!
//! A profile is just a TOML file in `$config/profiles/<name>.toml` containing
//! any subset of the schema. The active profile is selected by the root
//! `active_profile` field.

use std::path::PathBuf;

use anyhow::Result;
use tauri::{AppHandle, Manager};

pub fn profiles_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_config_dir()?.join("profiles");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn list(app: &AppHandle) -> Result<Vec<String>> {
    let dir = profiles_dir(app)?;
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
            names.push(stem.to_owned());
        }
    }
    names.sort();
    Ok(names)
}
