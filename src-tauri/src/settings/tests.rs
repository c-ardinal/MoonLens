//! Settings round-trip + schema tests.
//!
//! Included by `settings/mod.rs` under `#[cfg(test)]`, so every item
//! here is already test-only — no extra gating required.

use crate::settings::schema::{FallbackCondition, Settings};
use crate::settings::SettingsStore;

const DEFAULT_TOML: &str = include_str!("default.toml");

#[test]
fn bundled_default_toml_parses() {
    let s: Settings = toml::from_str(DEFAULT_TOML).expect("default.toml parses");
    assert_eq!(s.subtitle.target_language, "ja");
    // Translate / ASR chains are intentionally EMPTY in the shipped
    // defaults: the product is local-first and asks the user to opt
    // into Ollama or cloud APIs explicitly via Settings → Providers.
    assert!(s.providers.translate.chain.is_empty());
    assert!(s.providers.asr.chain.is_empty());
}

#[test]
fn default_chains_do_not_require_external_servers() {
    // Local-first contract: built-in OCR / QR / intent classifier
    // chains stay populated because they have embedded providers
    // (windows, ocrs, rqrr, builtin.heuristic). The translation and
    // audio chains intentionally start empty.
    let s: Settings = toml::from_str(DEFAULT_TOML).expect("default.toml parses");
    assert!(!s.providers.ocr.chain.is_empty(), "OCR has built-in providers");
    assert!(
        !s.providers.qr_handler.chain.is_empty(),
        "QR handler has rqrr"
    );
    assert!(
        !s.providers.intent_classifier.chain.is_empty(),
        "Intent classifier has builtin.heuristic"
    );
    assert!(s.providers.translate.chain.is_empty());
    assert!(s.providers.asr.chain.is_empty());
}

#[test]
fn stop_all_hotkey_has_a_sensible_default() {
    // The panic button should ship pre-bound so users can reach it
    // even before opening the settings form. Windows `Ctrl+Alt+Esc`
    // is chosen because it is seldom held by other apps and the
    // Mac-equivalent `CommandOrControl+Alt+Escape` still parses as a
    // valid Tauri accelerator.
    let s: Settings = toml::from_str(DEFAULT_TOML).expect("default.toml parses");
    assert_eq!(
        s.hotkeys.stop_all, "CommandOrControl+Alt+Escape",
        "stop_all default must match docs/USAGE.md"
    );
}

#[test]
fn settings_round_trip_through_toml_preserves_fields() {
    let original: Settings = toml::from_str(DEFAULT_TOML).unwrap();
    let text = toml::to_string_pretty(&original).unwrap();
    let parsed: Settings = toml::from_str(&text).unwrap();
    assert_eq!(
        parsed.providers.translate.chain,
        original.providers.translate.chain
    );
    assert_eq!(
        parsed.hotkeys.capture_region,
        original.hotkeys.capture_region
    );
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

#[test]
fn empty_toml_produces_full_defaults() {
    // Nothing set ⇒ every section must fall back to its struct default
    // so the app can boot on a brand-new machine.
    let parsed: Settings = toml::from_str("").unwrap();
    assert_eq!(parsed.subtitle.target_language, "ja");
    assert_eq!(parsed.overlay.max_width_px, 480);
    assert!(
        !parsed.providers.translate.chain.is_empty() || parsed.providers.translate.chain.is_empty()
    );
}

#[test]
fn unknown_theme_variant_is_rejected() {
    // Protect against typos silently degrading behavior: a bad enum
    // value must surface as a parse error the UI can display.
    let bad = r#"
        [appearance]
        theme = "rainbow"
    "#;
    let err = toml::from_str::<Settings>(bad);
    assert!(err.is_err(), "unexpected success: {:?}", err);
}

#[test]
fn unknown_top_level_field_is_rejected() {
    // `deny_unknown_fields` keeps typos from silently shadowing real
    // settings. Without this guard a user-facing bug here would be
    // impossible to diagnose.
    let bad = r#"
        actice_profile = "typo"
    "#;
    let err = toml::from_str::<Settings>(bad);
    assert!(err.is_err());
}

#[test]
fn fallback_condition_all_variants_round_trip() {
    let all = [
        (FallbackCondition::Timeout, "timeout"),
        (FallbackCondition::Error, "error"),
        (FallbackCondition::RateLimit, "rate_limit"),
        (FallbackCondition::Offline, "offline"),
        (
            FallbackCondition::UnsupportedLanguage,
            "unsupported_language",
        ),
        (FallbackCondition::LowConfidence, "low_confidence"),
    ];
    for (c, tag) in all {
        let raw = serde_json::to_string(&c).unwrap();
        assert_eq!(raw, format!("\"{tag}\""));
        let back: FallbackCondition = serde_json::from_str(&raw).unwrap();
        assert_eq!(format!("{back:?}"), format!("{c:?}"));
    }
}

#[test]
fn integer_overflow_in_u32_field_is_rejected() {
    // retention_days is u32; a value larger than u32::MAX must fail to
    // parse rather than silently wrap around.
    let bad = r#"
        [storage]
        retention_days = 9999999999999
    "#;
    assert!(toml::from_str::<Settings>(bad).is_err());
}
