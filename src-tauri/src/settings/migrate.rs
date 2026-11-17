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

/// Apply every rule; returns true when anything changed.
pub fn migrate_legacy_defaults(s: &mut Settings) -> bool {
    let mut changed = false;
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
        // Idempotent.
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
        assert!(!migrate_legacy_defaults(&mut s));
        assert_eq!(s.providers.configs["ollama.gemma4"]["model"], "qwen3.5:9b");
        assert_eq!(
            s.providers.configs["ollama.gemma4"]["system_prompt"],
            "my own prompt"
        );
    }

    #[test]
    fn bundled_defaults_need_no_migration() {
        let mut s: Settings = toml::from_str(include_str!("default.toml")).unwrap();
        assert!(!migrate_legacy_defaults(&mut s));
    }
}
