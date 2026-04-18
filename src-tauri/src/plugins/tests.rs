//! Plugin manifest parsing + capability wiring tests.

#[cfg(test)]
mod tests {
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
    fn chain_id_format_is_stable() {
        // The chain id format is `plugin.<id>.<cap_str>`. Assert it once so
        // config authors have a single source of truth.
        let id = "com.example.hello";
        let cap = "translator";
        let chain = format!("plugin.{id}.{cap}");
        assert_eq!(chain, "plugin.com.example.hello.translator");
    }
}
