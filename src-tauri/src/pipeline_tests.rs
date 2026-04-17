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
    fn fallback_timeout_matches_condition() {
        let conds = [FallbackCondition::Timeout];
        assert!(should_fallback_for_test(&conds, &ProviderError::Timeout(100)));
        assert!(!should_fallback_for_test(
            &conds,
            &ProviderError::Offline
        ));
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
}
