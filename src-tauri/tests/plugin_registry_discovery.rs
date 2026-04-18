//! Integration test for plugin discovery on disk.
//!
//! Populates a temp plugins directory with two plugin manifests and
//! verifies the registry enumerates them, respects enable/disable
//! toggles, and skips malformed directories with a warning instead of
//! crashing the app.

use std::{fs, sync::Arc};

use moonlens_lib::plugins::PluginRegistry;
use moonlens_lib::settings::SettingsStore;

fn write_plugin(dir: &std::path::Path, name: &str, kind: &str) {
    let plugin_dir = dir.join(name);
    fs::create_dir_all(&plugin_dir).unwrap();
    let manifest = format!(
        r#"
        [plugin]
        id = "{name}"
        name = "{name}"
        version = "0.1.0"
        author = ""
        license = "MIT"
        kind = "{kind}"
        entry = "bin/unused"
        api_version = "1"

        [capabilities]
        provides = ["translator"]

        [permissions]
        "#
    );
    fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();
    // Create a 0-byte "wasm" entry for the wasm kind so the loader does
    // not fail trying to read it. (Sidecar plugins will fail to spawn
    // because the binary is empty, but `load` succeeds since it only
    // reads the manifest.)
    let bin = plugin_dir.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("unused"), [] as [u8; 0]).unwrap();
}

#[test]
fn discovers_every_plugin_with_a_valid_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    write_plugin(&plugins_dir, "alpha", "sidecar");
    write_plugin(&plugins_dir, "beta", "sidecar");

    let settings_path = tmp.path().join("config.toml");
    let settings = Arc::new(SettingsStore::load_from(settings_path).unwrap());
    let registry = PluginRegistry::load_from(plugins_dir, settings).unwrap();

    let ids: Vec<String> = registry.list().into_iter().map(|d| d.id).collect();
    assert!(ids.contains(&"alpha".to_string()));
    assert!(ids.contains(&"beta".to_string()));
}

#[test]
fn malformed_plugin_is_skipped_without_breaking_other_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    write_plugin(&plugins_dir, "good", "sidecar");
    // A directory without plugin.toml should be ignored, not panic.
    fs::create_dir_all(plugins_dir.join("corrupt")).unwrap();
    // A plugin with an unparseable manifest should also be skipped.
    fs::create_dir_all(plugins_dir.join("broken")).unwrap();
    fs::write(
        plugins_dir.join("broken/plugin.toml"),
        "not = valid :: toml",
    )
    .unwrap();

    let settings = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let registry = PluginRegistry::load_from(plugins_dir, settings).unwrap();
    let ids: Vec<String> = registry.list().into_iter().map(|d| d.id).collect();
    assert_eq!(ids, vec!["good".to_string()]);
}

#[test]
fn enabled_flag_is_per_plugin_and_persists_across_rescan() {
    let tmp = tempfile::tempdir().unwrap();
    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    write_plugin(&plugins_dir, "a", "sidecar");
    write_plugin(&plugins_dir, "b", "sidecar");

    let settings = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let registry = PluginRegistry::load_from(plugins_dir.clone(), settings.clone()).unwrap();

    // Neither plugin is enabled by default.
    for d in registry.list() {
        assert!(!d.enabled, "{} unexpectedly enabled", d.id);
    }

    // Enabling persists through a rescan because the flag lives in
    // settings, not in the registry's in-memory vector.
    registry.set_enabled("a", true).unwrap();
    registry.rescan().unwrap();
    let enabled: Vec<_> = registry
        .list()
        .into_iter()
        .filter(|d| d.enabled)
        .map(|d| d.id)
        .collect();
    assert_eq!(enabled, vec!["a".to_string()]);
}
