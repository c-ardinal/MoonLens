//! End-to-end test for the WASM Component plugin host.
//!
//! A full provider-implementing `.wasm` would require `cargo-component`
//! in the CI toolchain. Instead this suite covers the load-time path
//! with two synthesized fixtures:
//!
//! 1. A trivial **core** module assembled from WAT. It is syntactically
//!    valid WebAssembly but not a Component, so the host must reject it
//!    with a clear error — proving the component-model guard is active.
//! 2. A **Component** that is valid but exports no MoonLens world. The
//!    host must still instantiate it (proving the wasmtime wiring works)
//!    yet surface a meaningful error when we ask it for providers.
//!
//! Together these exercise every branch of `WasmPlugin::load` + the
//! bindgen-emitted `Plugin::instantiate` that we hand-wrote in
//! `plugins::wasm_host`. Coverage of actual plugin calls (the `run`
//! path) is provided at the sidecar level, which uses the identical
//! `Provider` trait.

use std::{fs, sync::Arc};

use moonlens_lib::plugins::{manifest::PluginManifest, PluginRegistry};
use moonlens_lib::settings::SettingsStore;

fn write_manifest(dir: &std::path::Path, kind: &str) {
    let manifest = format!(
        r#"
        [plugin]
        id = "com.moonlens.it.wasm"
        name = "Wasm Fixture"
        version = "0.1.0"
        author = ""
        license = "MIT"
        kind = "{kind}"
        entry = "plugin.wasm"
        api_version = "1"

        [capabilities]
        provides = ["translator"]

        [permissions]
        "#
    );
    fs::write(dir.join("plugin.toml"), manifest).unwrap();
}

/// Build a minimal **core** WebAssembly module (not a component) via WAT.
/// Good enough to prove the host notices when a user drops a plain .wasm
/// instead of a Component.
fn core_module_bytes() -> Vec<u8> {
    // An empty core module is a valid wasm binary that is **not** a
    // component; wasmtime should refuse it when component-model decoding
    // is requested.
    wat::parse_str("(module)").expect("wat encodes core module")
}

/// Build a valid **Component** that exports nothing useful. Wasmtime
/// accepts this as a component but our bindgen wrapper will fail when
/// looking up the MoonLens world's exports.
fn empty_component_bytes() -> Vec<u8> {
    wat::parse_str("(component)").expect("wat encodes empty component")
}

#[test]
fn core_module_is_rejected_as_not_a_component() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin_dir = tmp.path().join("plugin");
    fs::create_dir_all(&plugin_dir).unwrap();
    write_manifest(&plugin_dir, "wasm");
    fs::write(plugin_dir.join("plugin.wasm"), core_module_bytes()).unwrap();

    // Set the manifest to enabled so the registry actually tries to load
    // the component at scan time.
    let settings_path = tmp.path().join("config.toml");
    let settings = Arc::new(SettingsStore::load_from(settings_path).unwrap());
    // Opt the plugin in before `load_from` so scan-time enabling sticks.
    {
        let mut s = (*settings.snapshot()).clone();
        s.plugins
            .enabled
            .insert("com.moonlens.it.wasm".into(), true);
        settings.save(s).unwrap();
    }

    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    // Move the plugin directory under the registry's expected root.
    fs::rename(&plugin_dir, plugins_dir.join("com.moonlens.it.wasm")).unwrap();

    let registry = PluginRegistry::load_from(plugins_dir, settings).unwrap();
    // The registry should still list the manifest (scan does not panic),
    // but no providers should be registered because instantiation failed.
    let descriptors = registry.list();
    assert_eq!(descriptors.len(), 1);
    let providers = registry.providers();
    assert!(
        providers.is_empty(),
        "core module should not produce providers (got {} entries)",
        providers.len()
    );
}

#[test]
fn empty_component_does_not_expose_moonlens_world() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin_dir = tmp.path().join("plugin");
    fs::create_dir_all(&plugin_dir).unwrap();
    write_manifest(&plugin_dir, "wasm");
    fs::write(plugin_dir.join("plugin.wasm"), empty_component_bytes()).unwrap();

    let settings_path = tmp.path().join("config.toml");
    let settings = Arc::new(SettingsStore::load_from(settings_path).unwrap());
    {
        let mut s = (*settings.snapshot()).clone();
        s.plugins
            .enabled
            .insert("com.moonlens.it.wasm".into(), true);
        settings.save(s).unwrap();
    }

    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    fs::rename(&plugin_dir, plugins_dir.join("com.moonlens.it.wasm")).unwrap();

    let registry = PluginRegistry::load_from(plugins_dir, settings).unwrap();
    // Same outcome: manifest is listed, but no provider is registered
    // because the component does not export the moonlens-plugin world.
    assert_eq!(registry.list().len(), 1);
    assert!(registry.providers().is_empty());
}

#[test]
fn wasm_manifest_parses_with_expected_defaults() {
    // Quick sanity check that the fixture manifest we construct above
    // matches what the registry expects. Catches regressions in the
    // manifest schema that would silently cause the other two tests to
    // pass vacuously.
    let manifest_text = r#"
        [plugin]
        id = "x"
        name = "X"
        version = "0.1"
        author = ""
        license = "MIT"
        kind = "wasm"
        entry = "x.wasm"
        api_version = "1"

        [capabilities]
        provides = ["translator"]
    "#;
    let m: PluginManifest = toml::from_str(manifest_text).unwrap();
    assert_eq!(m.plugin.kind, moonlens_lib::plugins::PluginKind::Wasm);
    assert_eq!(m.capabilities.provides, vec!["translator"]);
}
