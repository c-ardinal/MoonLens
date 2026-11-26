//! In-memory upgrades for defaults that earlier builds seeded into the
//! user's `config.toml`.
//!
//! The store writes the full bundled `default.toml` to disk on first
//! launch, so changing a default in the bundle alone never reaches
//! existing installs. Each rule here rewrites a value **only when it
//! still equals the exact old default** — anything the user changed by
//! hand is left alone.

use super::schema::Settings;
use crate::providers::translate_ollama;

/// Ollama tag shipped as the default up to v0.1.0-rc.1. It never existed
/// in the Ollama library (Gemma 4 ships as e2b / e4b / 12b / 26b / 31b),
/// so every request 404'd.
pub const LEGACY_OLLAMA_MODEL: &str = "gemma4:7b-instruct-q4_K_M";
/// System prompt shipped up to v0.1.0-rc.1. It did not name the
/// languages and overrode the provider's language-aware prompt.
pub const LEGACY_OLLAMA_SYSTEM_PROMPT: &str =
    "You are a careful translator. Preserve meaning, tone, and formatting. Output only the translation.";
pub const LEGACY_TRANSLATE_TIMEOUT_MS: u32 = 3000;
pub const TRANSLATE_TIMEOUT_MS: u32 = 15000;
/// Reverse image search chain shipped up to v0.1.0: every entry opens a
/// web search engine (Google first). MoonLens stays fully local, so the
/// chain is emptied; plugins can still provide the capability.
pub const LEGACY_REVERSE_IMAGE_SEARCH_CHAIN: [&str; 3] =
    ["browser.google_images", "browser.bing", "browser.yandex"];

/// Speech recognition chain shipped up to v0.1.0: an external whisper.cpp
/// server that had to be started by hand. The engine is built in now.
pub const LEGACY_ASR_CHAIN: [&str; 1] = ["whisper_cpp.large_v3_turbo"];
pub const LEGACY_ASR_TIMEOUT_MS: u32 = 15000;
pub const ASR_TIMEOUT_MS: u32 = 60000;

/// Accent colour shipped as the default while the main window ignored the
/// setting (its palette was fixed to amber). The setting now recolours the
/// main window, so the old default would turn every install violet. The
/// colour picker writes lower-case hex, so a violet the user picked on
/// purpose does not match.
pub const LEGACY_ACCENT: &str = "#8B5CF6";
pub const ACCENT: &str = "#FFB547";

/// Apply every rule; returns true when anything changed.
pub fn migrate_legacy_defaults(s: &mut Settings) -> bool {
    let mut changed = false;
    if s.appearance.accent == LEGACY_ACCENT {
        s.appearance.accent = ACCENT.into();
        changed = true;
    }
    for (id, cfg) in s.providers.configs.iter_mut() {
        let Some(obj) = cfg.as_object_mut() else {
            continue;
        };
        if id.starts_with("ollama.") {
            if obj.get("model").and_then(|v| v.as_str()) == Some(LEGACY_OLLAMA_MODEL) {
                obj.insert("model".into(), translate_ollama::DEFAULT_MODEL.into());
                changed = true;
            }
            if obj.get("system_prompt").and_then(|v| v.as_str())
                == Some(LEGACY_OLLAMA_SYSTEM_PROMPT)
            {
                obj.remove("system_prompt");
                changed = true;
            }
        }
    }
    if s.providers.translate.timeout_ms == LEGACY_TRANSLATE_TIMEOUT_MS {
        s.providers.translate.timeout_ms = TRANSLATE_TIMEOUT_MS;
        changed = true;
    }
    if s.providers.reverse_image_search.chain == LEGACY_REVERSE_IMAGE_SEARCH_CHAIN {
        s.providers.reverse_image_search.chain.clear();
        s.providers.reverse_image_search.allow_cloud = false;
        s.providers.reverse_image_search.allow_network = false;
        changed = true;
    }
    for id in LEGACY_REVERSE_IMAGE_SEARCH_CHAIN {
        changed |= s.providers.configs.remove(id).is_some();
    }
    if s.providers.asr.chain == LEGACY_ASR_CHAIN {
        s.providers.asr.chain = vec![crate::providers::asr_local::LOCAL_ASR_ID.to_string()];
        changed = true;
    }
    if s.providers.asr.timeout_ms == LEGACY_ASR_TIMEOUT_MS {
        s.providers.asr.timeout_ms = ASR_TIMEOUT_MS;
        changed = true;
    }
    changed |= restore_empty_chains(s);
    changed
}

/// A feature with a built-in engine never works with an empty chain
/// (pre-release builds wrote `chain = []` for speech recognition, which
/// made live subtitles report "no provider" even with the model
/// downloaded). Put the bundled default chain back.
fn restore_empty_chains(s: &mut Settings) -> bool {
    let d = super::store::defaults().providers;
    let p = &mut s.providers;
    let mut changed = false;
    for (chain, default) in [
        (&mut p.translate, d.translate),
        (&mut p.ocr, d.ocr),
        (&mut p.asr, d.asr),
        (&mut p.explain, d.explain),
        (&mut p.summarize, d.summarize),
    ] {
        if chain.chain.is_empty() && !default.chain.is_empty() {
            chain.chain = default.chain;
            changed = true;
        }
    }
    changed
}

/// Format version of `config.toml`. Files written before versioning
/// count as 0. Bump it when a rule below must run exactly once.
pub const CONFIG_VERSION: u32 = 1;

/// One-shot upgrades for files older than [`CONFIG_VERSION`]. Unlike
/// [`migrate_legacy_defaults`] these may change a value the user could
/// also have chosen, so they run only once per file.
pub fn migrate_versioned(s: &mut Settings, from: u32) -> bool {
    let mut changed = false;
    if from < 1 {
        // Up to v0.1.0 every install got a full copy of the defaults, with
        // the global cloud switch on and no screen offering it. MoonLens
        // is fully local: turn it off once. Users may turn it back on.
        if s.privacy.allow_cloud_globally {
            s.privacy.allow_cloud_globally = false;
            changed = true;
        }
    }
    if s.config_version != CONFIG_VERSION {
        s.config_version = CONFIG_VERSION;
        changed = true;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy() -> Settings {
        let mut s: Settings = toml::from_str(include_str!("default.toml")).unwrap();
        let cfg = s
            .providers
            .configs
            .get_mut("ollama.gemma4")
            .unwrap()
            .as_object_mut()
            .unwrap();
        cfg.insert("model".into(), LEGACY_OLLAMA_MODEL.into());
        cfg.insert("system_prompt".into(), LEGACY_OLLAMA_SYSTEM_PROMPT.into());
        s.providers.translate.timeout_ms = LEGACY_TRANSLATE_TIMEOUT_MS;
        s.providers.reverse_image_search.chain = LEGACY_REVERSE_IMAGE_SEARCH_CHAIN
            .iter()
            .map(|x| x.to_string())
            .collect();
        s.privacy.allow_cloud_globally = true;
        s.providers.asr.chain = LEGACY_ASR_CHAIN.iter().map(|x| x.to_string()).collect();
        s.providers.asr.timeout_ms = LEGACY_ASR_TIMEOUT_MS;
        s
    }

    #[test]
    fn legacy_defaults_are_upgraded() {
        let mut s = legacy();
        assert!(migrate_legacy_defaults(&mut s));
        let o = &s.providers.configs["ollama.gemma4"];
        assert_eq!(o["model"], translate_ollama::DEFAULT_MODEL);
        assert!(o.get("system_prompt").is_none());
        assert_eq!(s.providers.translate.timeout_ms, TRANSLATE_TIMEOUT_MS);
        assert!(s.providers.reverse_image_search.chain.is_empty());
        assert_eq!(s.providers.asr.chain, ["whisper"]);
        assert_eq!(s.providers.asr.timeout_ms, ASR_TIMEOUT_MS);
        assert!(migrate_versioned(&mut s, 0));
        assert!(!s.privacy.allow_cloud_globally);
        assert_eq!(s.config_version, CONFIG_VERSION);
        // Idempotent.
        assert!(!migrate_versioned(&mut s, CONFIG_VERSION));
        assert!(!migrate_legacy_defaults(&mut s));
    }

    #[test]
    fn user_choices_are_left_alone() {
        let mut s = legacy();
        let cfg = s
            .providers
            .configs
            .get_mut("ollama.gemma4")
            .unwrap()
            .as_object_mut()
            .unwrap();
        cfg.insert("model".into(), "qwen3.5:9b".into());
        cfg.insert("system_prompt".into(), "my own prompt".into());
        s.providers.translate.timeout_ms = 4000;
        s.providers.reverse_image_search.chain = vec!["plugin.my_search".into()];
        s.providers.asr.chain = vec!["whisper_cpp.large_v3_turbo".into(), "whisper".into()];
        s.providers.asr.timeout_ms = 20000;
        assert!(!migrate_legacy_defaults(&mut s));
        assert_eq!(s.providers.configs["ollama.gemma4"]["model"], "qwen3.5:9b");
        assert_eq!(
            s.providers.configs["ollama.gemma4"]["system_prompt"],
            "my own prompt"
        );
    }

    #[test]
    fn empty_chains_of_built_in_features_get_the_default_back() {
        let mut s: Settings = toml::from_str(include_str!("default.toml")).unwrap();
        s.providers.asr.chain.clear();
        s.providers.translate.chain.clear();
        s.providers.reverse_image_search.chain.clear();
        assert!(migrate_legacy_defaults(&mut s));
        assert_eq!(s.providers.asr.chain, ["whisper"]);
        assert_eq!(s.providers.translate.chain, ["llamacpp"]);
        // No built-in engine: stays empty.
        assert!(s.providers.reverse_image_search.chain.is_empty());
        assert!(!migrate_legacy_defaults(&mut s));
    }

    #[test]
    fn bundled_defaults_need_no_migration() {
        let mut s: Settings = toml::from_str(include_str!("default.toml")).unwrap();
        assert!(!migrate_legacy_defaults(&mut s));
        assert!(!migrate_versioned(&mut s, CONFIG_VERSION));
    }

    #[test]
    fn current_files_keep_a_deliberate_cloud_switch() {
        let mut s: Settings = toml::from_str(include_str!("default.toml")).unwrap();
        s.privacy.allow_cloud_globally = true;
        migrate_versioned(&mut s, CONFIG_VERSION);
        assert!(s.privacy.allow_cloud_globally);
    }

    #[test]
    fn legacy_default_accent_becomes_amber_and_picked_colours_stay() {
        let mut s = Settings::default();
        s.appearance.accent = LEGACY_ACCENT.into();
        assert!(migrate_legacy_defaults(&mut s));
        assert_eq!(s.appearance.accent, ACCENT);
        // The colour picker writes lower-case hex.
        s.appearance.accent = "#8b5cf6".into();
        assert!(!migrate_legacy_defaults(&mut s));
        assert_eq!(s.appearance.accent, "#8b5cf6");
    }
}
