//! End-to-end WASM Component plugin execution.
//!
//! The previous `plugin_wasm_host` suite only covered load-time paths
//! (core module rejection, empty component). This suite drops a
//! **real** `.wasm` component into a temp plugins directory, lets the
//! registry pick it up, and invokes its exported provider through the
//! same code path production takes.
//!
//! The fixture (`tests/fixtures/wasm_hello.wasm`) is built by the
//! `tests/fixtures/wasm-hello/` crate via `cargo-component`. See its
//! `Cargo.toml` for the build recipe; the resulting binary is
//! committed so the test runs anywhere without `cargo-component`
//! installed. Regeneration instructions live in the fixture README.

use std::{fs, sync::Arc};

use moonlens_lib::plugins::{manifest::PluginManifest, PluginRegistry};
use moonlens_lib::providers::{TaskInput, TaskOutput};
use moonlens_lib::settings::SettingsStore;

const FIXTURE_WASM: &[u8] = include_bytes!("fixtures/wasm_hello.wasm");

fn prepare_plugin_dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    let plugin_dir = plugins_dir.join("com.moonlens.it.wasm_hello");
    fs::create_dir_all(&plugin_dir).unwrap();

    let manifest = r#"
        [plugin]
        id = "com.moonlens.it.wasm_hello"
        name = "wasm-hello"
        version = "0.1.0"
        author = "MoonLens"
        license = "Apache-2.0"
        kind = "wasm"
        entry = "plugin.wasm"
        api_version = "1"

        [capabilities]
        provides = ["translator"]

        [permissions]
    "#;
    fs::write(plugin_dir.join("plugin.toml"), manifest).unwrap();
    fs::write(plugin_dir.join("plugin.wasm"), FIXTURE_WASM).unwrap();
    let root = tmp.path().to_path_buf();
    (tmp, root)
}

#[tokio::test]
async fn wasm_component_plugin_runs_translate_end_to_end() {
    let (_guard, tmp_root) = prepare_plugin_dir();
    let plugins_dir = tmp_root.join("plugins");

    let settings = Arc::new(SettingsStore::load_from(tmp_root.join("config.toml")).unwrap());
    // Opt the plugin in before scanning so the registry actually
    // instantiates it.
    let mut s = (*settings.snapshot()).clone();
    s.plugins
        .enabled
        .insert("com.moonlens.it.wasm_hello".into(), true);
    settings.save(s).unwrap();

    let registry = PluginRegistry::load_from(plugins_dir, settings).unwrap();
    let handles = registry.providers();
    let (_id, handle) = handles
        .into_iter()
        .find(|(id, _)| id.ends_with(".translator"))
        .expect("wasm-hello should register a translator provider");

    let out = handle
        .run(TaskInput::Translate {
            text: "good morning".into(),
            source_language: "en".into(),
            target_language: "ja".into(),
        })
        .await
        .expect("wasm plugin run succeeds");
    match out {
        TaskOutput::Text(t) => assert_eq!(t, "[wasm-hello] good morning"),
        other => panic!("unexpected output: {other:?}"),
    }
}

#[tokio::test]
async fn wasm_component_plugin_handles_unicode_inputs() {
    let (_guard, tmp_root) = prepare_plugin_dir();
    let plugins_dir = tmp_root.join("plugins");
    let settings = Arc::new(SettingsStore::load_from(tmp_root.join("config.toml")).unwrap());
    let mut s = (*settings.snapshot()).clone();
    s.plugins
        .enabled
        .insert("com.moonlens.it.wasm_hello".into(), true);
    settings.save(s).unwrap();
    let registry = PluginRegistry::load_from(plugins_dir, settings).unwrap();
    let (_id, handle) = registry
        .providers()
        .into_iter()
        .find(|(id, _)| id.ends_with(".translator"))
        .unwrap();
    let out = handle
        .run(TaskInput::Translate {
            text: "猫が 🐱 鳴く".into(),
            source_language: "ja".into(),
            target_language: "en".into(),
        })
        .await
        .unwrap();
    match out {
        TaskOutput::Text(t) => assert_eq!(t, "[wasm-hello] 猫が 🐱 鳴く"),
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn fixture_component_parses_as_a_valid_component_manifest() {
    // Belt-and-braces: the manifest we write during setup must parse
    // against the current PluginManifest schema, or the tests above
    // would fail with a cryptic registry error rather than a schema
    // mismatch.
    let manifest: PluginManifest = toml::from_str(
        r#"
        [plugin]
        id = "com.moonlens.it.wasm_hello"
        name = "wasm-hello"
        version = "0.1.0"
        author = ""
        license = "Apache-2.0"
        kind = "wasm"
        entry = "plugin.wasm"
        api_version = "1"

        [capabilities]
        provides = ["translator"]
    "#,
    )
    .unwrap();
    assert_eq!(manifest.plugin.id, "com.moonlens.it.wasm_hello");
    assert_eq!(manifest.capabilities.provides, vec!["translator"]);
}
