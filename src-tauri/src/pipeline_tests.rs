//! Focused pipeline tests that do not require a running Tauri instance.
//!
//! The real `Pipeline::new` expects `PluginRegistry` and `Storage` backed by
//! Tauri paths, which is not available in unit tests. Instead, we exercise
//! the decision logic: chain ordering, fallback selection, stickiness.

#[cfg(test)]
mod tests {
    use crate::{
        pipeline::{apply_stickiness_for_test, should_fallback_for_test},
        providers::ProviderError,
        settings::schema::FallbackCondition,
    };
    use std::{
        collections::HashMap,
        time::{Duration, Instant},
    };

    #[test]
    fn legacy_seeded_ocr_chain_is_upgraded_but_custom_chains_are_kept() {
        use crate::pipeline::upgrade_ocr_chain_for_test as up;
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // Windows also gets the OS OCR engine between PP-OCR and ocrs.
        let default = if cfg!(windows) {
            v(&["ppocr", "windows", "ocrs"])
        } else {
            v(&["ppocr", "ocrs"])
        };
        assert_eq!(up(v(&["ocrs", "paddle"])), default);
        assert_eq!(up(v(&["ppocr", "ocrs"])), default);
        assert_eq!(up(v(&["ocrs"])), v(&["ocrs"]));
        assert_eq!(up(v(&["plugin.x", "ocrs"])), v(&["plugin.x", "ocrs"]));
    }

    #[test]
    fn legacy_ollama_chains_move_to_builtin_engine() {
        use crate::pipeline::upgrade_llm_chain_for_test as up;
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            up(v(&["ollama.gemma4", "deepl", "gemini"]), "llamacpp"),
            v(&["llamacpp"])
        );
        assert_eq!(
            up(v(&["ollama.gemma4", "gemini"]), "llamacpp.assistant"),
            v(&["llamacpp.assistant"])
        );
        // A chain the user edited keeps Ollama.
        assert_eq!(
            up(v(&["ollama.gemma4", "deepl"]), "llamacpp"),
            v(&["ollama.gemma4", "deepl"])
        );
        assert_eq!(up(v(&["gemini"]), "llamacpp"), v(&["gemini"]));
    }

    #[test]
    fn fallback_timeout_matches_condition() {
        let conds = [FallbackCondition::Timeout];
        assert!(should_fallback_for_test(
            &conds,
            &ProviderError::Timeout(100)
        ));
        assert!(!should_fallback_for_test(&conds, &ProviderError::Offline));
    }

    #[test]
    fn fallback_offline_matches_condition() {
        let conds = [FallbackCondition::Offline];
        assert!(should_fallback_for_test(&conds, &ProviderError::Offline));
        assert!(!should_fallback_for_test(
            &conds,
            &ProviderError::Timeout(0)
        ));
    }

    #[test]
    fn fallback_low_confidence_matches_condition() {
        let conds = [FallbackCondition::LowConfidence];
        assert!(should_fallback_for_test(
            &conds,
            &ProviderError::LowConfidence
        ));
    }

    #[test]
    fn stickiness_moves_preferred_provider_to_front() {
        let chain = vec!["a".to_string(), "b".into(), "c".into()];
        let mut map = HashMap::new();
        map.insert(
            "Translate".into(),
            ("b".to_string(), Instant::now() + Duration::from_secs(30)),
        );
        let ordered = apply_stickiness_for_test("Translate", &map, &chain);
        assert_eq!(ordered, vec!["b".to_string(), "a".into(), "c".into()]);
    }

    #[test]
    fn stickiness_expired_falls_back_to_chain_order() {
        let chain = vec!["a".to_string(), "b".into()];
        let mut map = HashMap::new();
        map.insert(
            "Translate".into(),
            ("b".into(), Instant::now() - Duration::from_secs(1)),
        );
        let ordered = apply_stickiness_for_test("Translate", &map, &chain);
        assert_eq!(ordered, vec!["a".to_string(), "b".into()]);
    }

    #[test]
    fn stickiness_with_empty_chain_returns_empty() {
        let map = HashMap::new();
        let ordered = apply_stickiness_for_test("Translate", &map, &[]);
        assert!(ordered.is_empty());
    }

    #[test]
    fn stickiness_for_capability_without_entry_keeps_chain_as_is() {
        // Sticky entries for OTHER capabilities must not reorder Translate.
        let chain = vec!["a".to_string(), "b".into(), "c".into()];
        let mut map = HashMap::new();
        map.insert(
            "Ocr".into(),
            ("c".into(), Instant::now() + Duration::from_secs(30)),
        );
        let ordered = apply_stickiness_for_test("Translate", &map, &chain);
        assert_eq!(ordered, chain);
    }

    #[test]
    fn stickiness_with_preferred_not_in_chain_still_tries_it_first() {
        // The current policy is "trust the sticky pick"; documenting it as
        // a test so any behaviour change is deliberate.
        let chain = vec!["a".to_string(), "b".into()];
        let mut map = HashMap::new();
        map.insert(
            "Translate".into(),
            ("ghost".into(), Instant::now() + Duration::from_secs(30)),
        );
        let ordered = apply_stickiness_for_test("Translate", &map, &chain);
        assert_eq!(ordered[0], "ghost");
        // The rest is the chain verbatim, since "ghost" was not in it.
        assert_eq!(&ordered[1..], &chain[..]);
    }

    #[test]
    fn fallback_no_conditions_means_bail_immediately() {
        // Empty conditions → any error short-circuits the chain. Capture
        // the behaviour so optimizing `should_fallback` does not silently
        // change it.
        assert!(!should_fallback_for_test(&[], &ProviderError::Offline));
        assert!(!should_fallback_for_test(&[], &ProviderError::Timeout(0)));
        assert!(!should_fallback_for_test(
            &[],
            &ProviderError::LowConfidence
        ));
    }

    #[test]
    fn fallback_error_condition_matches_only_other_variant() {
        // `Error` is intentionally scoped to `ProviderError::Other` only
        // (timeouts and offline have their own categories). Verify the
        // scoping in isolation so we do not accidentally broaden it.
        let conds = [FallbackCondition::Error];
        assert!(should_fallback_for_test(
            &conds,
            &ProviderError::Other("x".into())
        ));
        assert!(!should_fallback_for_test(
            &conds,
            &ProviderError::Timeout(10)
        ));
        assert!(!should_fallback_for_test(&conds, &ProviderError::Offline));
    }

    #[test]
    fn fallback_unsupported_language_matches_condition() {
        let conds = [FallbackCondition::UnsupportedLanguage];
        assert!(should_fallback_for_test(
            &conds,
            &ProviderError::UnsupportedLanguage("en".into(), "zz".into()),
        ));
    }

    #[test]
    fn fallback_combined_conditions_or_match() {
        let conds = [FallbackCondition::Timeout, FallbackCondition::Offline];
        assert!(should_fallback_for_test(&conds, &ProviderError::Timeout(1)));
        assert!(should_fallback_for_test(&conds, &ProviderError::Offline));
        assert!(!should_fallback_for_test(
            &conds,
            &ProviderError::LowConfidence
        ));
    }

    fn translate(text: &str) -> crate::providers::TaskInput {
        crate::providers::TaskInput::Translate {
            text: text.into(),
            source_language: "en".into(),
            target_language: "ja".into(),
            context: Default::default(),
        }
    }

    #[test]
    fn local_translation_timeout_grows_with_text_length() {
        use crate::pipeline::effective_timeout;
        // Short text: the chain timeout is the floor.
        assert_eq!(
            effective_timeout(15_000, &translate("hello"), true),
            Duration::from_millis(15_000)
        );
        // 1,000 characters on a local engine: 40 s.
        let long = "a".repeat(1_000);
        assert_eq!(
            effective_timeout(15_000, &translate(&long), true),
            Duration::from_millis(40_000)
        );
        // Cloud providers keep the fixed timeout.
        assert_eq!(
            effective_timeout(15_000, &translate(&long), false),
            Duration::from_millis(15_000)
        );
    }
}
