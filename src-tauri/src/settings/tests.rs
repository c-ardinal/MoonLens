//! Settings round-trip + schema tests.

#[cfg(test)]
mod tests {
    use crate::settings::schema::{FallbackCondition, Settings};
    use crate::settings::SettingsStore;

    const DEFAULT_TOML: &str = include_str!("default.toml");

    #[test]
    fn bundled_default_toml_parses() {
        let s: Settings = toml::from_str(DEFAULT_TOML).expect("default.toml parses");
        assert_eq!(s.subtitle.target_language, "ja");
        assert!(!s.providers.translate.chain.is_empty());
        assert_eq!(s.providers.translate.chain[0], "ollama.gemma4");
    }

    #[test]
    fn settings_round_trip_through_toml_preserves_fields() {
        let original: Settings = toml::from_str(DEFAULT_TOML).unwrap();
        let text = toml::to_string_pretty(&original).unwrap();
        let parsed: Settings = toml::from_str(&text).unwrap();
        assert_eq!(parsed.providers.translate.chain, original.providers.translate.chain);
        assert_eq!(parsed.hotkeys.capture_region, original.hotkeys.capture_region);
    }

    #[test]
    fn settings_round_trip_through_json_schema_generator_does_not_panic() {
        // The schema is produced via schemars; accessing it exercises the
        // `JsonSchema` derivations on every settings struct.
        let schema = SettingsStore::json_schema();
        let obj = schema.as_object().expect("schema is an object");
        assert!(obj.contains_key("definitions") || obj.contains_key("properties"));
    }

    #[test]
    fn missing_user_sections_fall_back_to_defaults() {
        // A partial user override should still parse because every struct
        // uses `#[serde(default)]`.
        let partial = r#"
            active_profile = "dev"
            [appearance]
            theme = "dark"
        "#;
        let parsed: Settings = toml::from_str(partial).unwrap();
        assert_eq!(parsed.active_profile.as_deref(), Some("dev"));
        assert!(matches!(
            parsed.appearance.theme,
            crate::settings::schema::Theme::Dark
        ));
        // Untouched sections still hold defaults.
        assert!(!parsed.providers.translate.fallback_on.is_empty());
    }

    #[test]
    fn fallback_condition_serializes_as_snake_case() {
        let c = FallbackCondition::LowConfidence;
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"low_confidence\"");
    }
}
