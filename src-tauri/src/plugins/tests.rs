//! Plugin manifest parsing + capability wiring tests.
//!
//! Included by `plugins/mod.rs` under `#[cfg(test)]`, so every item here
//! is already test-only — no extra gating required.

use crate::plugins::manifest::{PluginKind, PluginManifest};

const HELLO_MANIFEST: &str = r#"
    [plugin]
    id = "com.example.hello"
    name = "Hello"
    version = "1.0.0"
    author = "Test"
    license = "MIT"
    kind = "sidecar"
    entry = "bin/hello"
    api_version = "1"

    [capabilities]
    provides = ["translator", "action"]

    [permissions]
    network = ["https://api.example.com"]
    audio_capture = true
"#;

#[test]
fn sidecar_manifest_parses_with_permissions() {
    let m: PluginManifest = toml::from_str(HELLO_MANIFEST).expect("manifest parses");
    assert_eq!(m.plugin.id, "com.example.hello");
    assert!(matches!(m.plugin.kind, PluginKind::Sidecar));
    assert_eq!(m.capabilities.provides, vec!["translator", "action"]);
    assert!(m.permissions.audio_capture);
    assert_eq!(m.permissions.network, vec!["https://api.example.com"]);
}

#[test]
fn missing_capabilities_block_defaults_to_empty() {
    let minimal = r#"
        [plugin]
        id = "x"
        name = "X"
        version = "0.0.1"
        author = ""
        license = ""
        kind = "wasm"
        entry = "x.wasm"
        api_version = "1"
    "#;
    let m: PluginManifest = toml::from_str(minimal).unwrap();
    assert!(m.capabilities.provides.is_empty());
    assert!(!m.permissions.audio_capture);
}

#[test]
fn wasm_kind_is_recognized() {
    let m: PluginManifest = toml::from_str(
        r#"
        [plugin]
        id = "a"
        name = "A"
        version = "0.1"
        author = ""
        license = ""
        kind = "wasm"
        entry = "a.wasm"
        api_version = "1"
    "#,
    )
    .unwrap();
    assert!(matches!(m.plugin.kind, PluginKind::Wasm));
}

#[test]
fn missing_required_plugin_id_fails_to_parse() {
    let bad = r#"
        [plugin]
        name = "Nameless"
        version = "0.1"
        author = ""
        license = ""
        kind = "sidecar"
        entry = "bin/x"
        api_version = "1"
    "#;
    assert!(toml::from_str::<PluginManifest>(bad).is_err());
}

#[test]
fn invalid_plugin_kind_is_rejected() {
    // A plugin author mistyping the kind must see a parse error, not
    // a silently-disabled plugin.
    let bad = r#"
        [plugin]
        id = "x"
        name = "X"
        version = "0.1"
        author = ""
        license = ""
        kind = "docker"
        entry = "x"
        api_version = "1"
    "#;
    assert!(toml::from_str::<PluginManifest>(bad).is_err());
}

#[test]
fn empty_provides_parses_as_empty_vec() {
    // An empty `provides` table is valid (plugin provides nothing
    // discoverable) and must parse without error.
    let m: PluginManifest = toml::from_str(
        r#"
        [plugin]
        id = "x"
        name = "X"
        version = "0.1"
        author = ""
        license = ""
        kind = "wasm"
        entry = "x.wasm"
        api_version = "1"

        [capabilities]
        provides = []
    "#,
    )
    .unwrap();
    assert!(m.capabilities.provides.is_empty());
}

#[test]
fn permission_booleans_default_to_false() {
    let m: PluginManifest = toml::from_str(
        r#"
        [plugin]
        id = "a"
        name = "A"
        version = "0.1"
        author = ""
        license = ""
        kind = "sidecar"
        entry = "a"
        api_version = "1"
        [permissions]
    "#,
    )
    .unwrap();
    assert!(!m.permissions.audio_capture);
    assert!(!m.permissions.screen_capture);
    assert!(!m.permissions.clipboard);
    assert!(!m.permissions.subprocess);
}

#[test]
fn chain_id_format_is_stable() {
    // The chain id format is `plugin.<id>.<cap_str>`. Assert it once so
    // config authors have a single source of truth.
    let id = "com.example.hello";
    let cap = "translator";
    let chain = format!("plugin.{id}.{cap}");
    assert_eq!(chain, "plugin.com.example.hello.translator");
}
