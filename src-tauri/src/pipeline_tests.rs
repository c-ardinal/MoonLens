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
}

/// End-to-end exercises for the streaming entry point. These build a
/// real `Pipeline` (without a Tauri AppHandle) and verify that
/// `run_with_sink` walks the chain, dispatches `run_streaming` rather
/// than `run`, and forwards per-chunk deltas verbatim — the contract
/// the Lens watcher relies on after the Ollama-hardcode unwind.
#[cfg(test)]
mod chain_streaming {
    use crate::pipeline::Pipeline;
    use crate::providers::{
        Capability, Provider, ProviderError, ProviderHandle, StreamingChunkSink, TaskInput,
        TaskOutput,
    };
    use crate::settings::{schema::Settings, SettingsStore};
    use crate::storage::Storage;
    use async_trait::async_trait;
    use parking_lot::Mutex;
    use std::collections::HashMap;
    use std::sync::Arc;

    /// Streaming-aware mock provider. `run_streaming` emits the
    /// canned chunks one-by-one through the supplied sink; `run`
    /// returns the joined output without touching any sink. The
    /// distinction lets a single test prove the pipeline actually
    /// dispatches the streaming entry point.
    struct MockStreamingProvider {
        id: String,
        chunks: Vec<&'static str>,
        run_streaming_called: Arc<Mutex<bool>>,
        run_called: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl Provider for MockStreamingProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> &[Capability] {
            &[Capability::Translate]
        }
        fn is_local(&self) -> bool {
            true
        }
        async fn run(&self, _input: TaskInput) -> Result<TaskOutput, ProviderError> {
            *self.run_called.lock() = true;
            Ok(TaskOutput::Text(self.chunks.concat()))
        }
        async fn run_streaming(
            &self,
            _input: TaskInput,
            sink: StreamingChunkSink,
        ) -> Result<TaskOutput, ProviderError> {
            *self.run_streaming_called.lock() = true;
            let mut acc = String::new();
            let n = self.chunks.len();
            for (i, ch) in self.chunks.iter().enumerate() {
                let done = i + 1 == n;
                sink((*ch).into(), done);
                acc.push_str(ch);
            }
            Ok(TaskOutput::Text(acc))
        }
    }

    /// Helper: build a Pipeline whose translate chain is `chain`,
    /// registered against the supplied provider map. The settings
    /// store is rooted at a tempdir so default.toml is layered in
    /// correctly without touching the user's real config.
    fn pipeline_with_chain(
        chain: Vec<&str>,
        providers: HashMap<String, ProviderHandle>,
    ) -> Arc<Pipeline> {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cfg_path = tmp.path().join("config.toml");
        // Seed the settings file so the chain assertion holds even
        // after `load_from` writes defaults: we re-save the value.
        let mut settings: Settings =
            toml::from_str(include_str!("settings/default.toml")).unwrap();
        settings.providers.translate.chain =
            chain.into_iter().map(|s| s.to_string()).collect();
        // Drop the timeout so our error-path tests do not hang on
        // the real 30s default; 500 ms is plenty for in-process
        // mocks.
        settings.providers.translate.timeout_ms = 500;
        let toml_text = toml::to_string_pretty(&settings).unwrap();
        std::fs::write(&cfg_path, toml_text).unwrap();
        let store = Arc::new(SettingsStore::load_from(cfg_path).expect("settings store"));
        let storage = Arc::new(Storage::open_in_memory().expect("storage"));
        // Leak the tempdir so it outlives the test (not strictly
        // necessary because the Pipeline holds no fd, but avoids
        // surprise during interactive `cargo test --no-capture`).
        std::mem::forget(tmp);
        Arc::new(Pipeline::with_providers(store, storage, providers))
    }

    #[tokio::test]
    async fn run_with_sink_dispatches_run_streaming_and_forwards_chunks() {
        let run_streaming_called = Arc::new(Mutex::new(false));
        let run_called = Arc::new(Mutex::new(false));
        let prov = Arc::new(MockStreamingProvider {
            id: "mock.streaming".into(),
            chunks: vec!["hel", "lo ", "world"],
            run_streaming_called: run_streaming_called.clone(),
            run_called: run_called.clone(),
        });
        let mut map: HashMap<String, ProviderHandle> = HashMap::new();
        map.insert("mock.streaming".into(), prov);

        let pipeline = pipeline_with_chain(vec!["mock.streaming"], map);

        let captured: Arc<Mutex<Vec<(String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_for_sink = captured.clone();
        let sink: StreamingChunkSink = Arc::new(move |delta, done| {
            captured_for_sink.lock().push((delta, done));
        });

        let res = pipeline
            .run_with_sink(
                Capability::Translate,
                TaskInput::Translate {
                    text: "hello world".into(),
                    source_language: "en".into(),
                    target_language: "ja".into(),
                },
                sink,
            )
            .await
            .expect("translation should succeed");

        assert!(*run_streaming_called.lock(), "run_streaming should be used");
        assert!(!*run_called.lock(), "non-streaming run should NOT be used");
        let chunks = captured.lock().clone();
        assert_eq!(
            chunks,
            vec![
                ("hel".to_string(), false),
                ("lo ".to_string(), false),
                ("world".to_string(), true),
            ],
            "every delta + the done flag must propagate verbatim"
        );
        match res.output {
            TaskOutput::Text(s) => assert_eq!(s, "hello world"),
            other => panic!("expected Text, got {other:?}"),
        }
        assert_eq!(res.used_provider, "mock.streaming");
    }

    #[tokio::test]
    async fn run_dispatches_non_streaming_when_no_sink_supplied() {
        let run_streaming_called = Arc::new(Mutex::new(false));
        let run_called = Arc::new(Mutex::new(false));
        let prov = Arc::new(MockStreamingProvider {
            id: "mock.both".into(),
            chunks: vec!["a", "b"],
            run_streaming_called: run_streaming_called.clone(),
            run_called: run_called.clone(),
        });
        let mut map: HashMap<String, ProviderHandle> = HashMap::new();
        map.insert("mock.both".into(), prov);

        let pipeline = pipeline_with_chain(vec!["mock.both"], map);

        let res = pipeline
            .run(
                Capability::Translate,
                TaskInput::Translate {
                    text: "hi".into(),
                    source_language: "en".into(),
                    target_language: "ja".into(),
                },
            )
            .await
            .expect("translation should succeed");

        assert!(*run_called.lock(), "run should be used");
        assert!(
            !*run_streaming_called.lock(),
            "streaming entry must stay quiet without a sink"
        );
        match res.output {
            TaskOutput::Text(s) => assert_eq!(s, "ab"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    /// Trivial provider that always errors. Used to prove fallback
    /// works through `run_with_sink`.
    struct AlwaysFail {
        id: String,
    }
    #[async_trait]
    impl Provider for AlwaysFail {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> &[Capability] {
            &[Capability::Translate]
        }
        fn is_local(&self) -> bool {
            true
        }
        async fn run(&self, _input: TaskInput) -> Result<TaskOutput, ProviderError> {
            Err(ProviderError::Other("forced failure".into()))
        }
    }

    /// Pipeline preserves a loaded embedded engine across `rebuild_providers`
    /// when the resolved `model_path` does not change. This is the
    /// production hot-path: every settings save (and every `set_chunk_sink`
    /// call) hits `rebuild_providers`, and without preservation the
    /// multi-hundred-MB GGUF would be dropped + reloaded on every
    /// settings change (multi-second freeze per save).
    #[tokio::test]
    async fn rebuild_preserves_embedded_engine_when_path_unchanged() {
        let pipeline =
            pipeline_with_chain(vec!["embedded.llm"], HashMap::new());

        // Snapshot the cached embedded handle from the FIRST rebuild
        // (triggered inside pipeline_with_chain via SettingsStore::load_from
        // → Pipeline::with_providers, which we then nudge with an explicit
        // rebuild to populate the cache).
        pipeline.rebuild_providers();
        let first = pipeline
            .embedded_cache_for_test()
            .get("embedded.llm")
            .cloned()
            .expect("first rebuild registers embedded.llm");

        // Second rebuild without changing settings MUST hand back the
        // same Arc — preserving any loaded engine the first instance
        // accumulated through ensure_engine.
        pipeline.rebuild_providers();
        let second = pipeline
            .embedded_cache_for_test()
            .get("embedded.llm")
            .cloned()
            .expect("second rebuild still registers embedded.llm");
        assert!(
            Arc::ptr_eq(&first, &second),
            "rebuild MUST reuse the cached EmbeddedLlmTranslator when model_path is unchanged"
        );

        // Now flip the cached path by mutating settings under the
        // store; the next rebuild MUST evict + recreate.
        {
            let store = pipeline.settings_for_test();
            let mut snap = (*store.snapshot()).clone();
            snap.providers.configs.insert(
                "embedded.llm".into(),
                serde_json::json!({"model_path": "C:\\fake\\different.gguf"}),
            );
            store.save(snap).expect("save");
        }
        pipeline.rebuild_providers();
        let third = pipeline
            .embedded_cache_for_test()
            .get("embedded.llm")
            .cloned()
            .expect("third rebuild still registers embedded.llm");
        assert!(
            !Arc::ptr_eq(&first, &third),
            "rebuild MUST drop the cached engine when model_path changes (model swap)"
        );
        assert_ne!(
            first.model_path(),
            third.model_path(),
            "the new instance MUST reflect the updated path"
        );

        // Suppress the unused-import warning when this test is
        // compiled in isolation — Settings is referenced through
        // pipeline_with_chain only.
        let _: fn() -> Settings =
            || -> Settings { toml::from_str("").unwrap_or_default() };
    }

    #[tokio::test]
    async fn run_with_sink_falls_back_through_chain_on_error() {
        // First provider always fails; second provider streams "ok".
        // The pipeline must fall through to the second and the sink
        // must only see the second provider's chunks.
        let captured_called = Arc::new(Mutex::new(false));
        let mut map: HashMap<String, ProviderHandle> = HashMap::new();
        map.insert(
            "first.fail".into(),
            Arc::new(AlwaysFail {
                id: "first.fail".into(),
            }),
        );
        map.insert(
            "second.ok".into(),
            Arc::new(MockStreamingProvider {
                id: "second.ok".into(),
                chunks: vec!["o", "k"],
                run_streaming_called: captured_called.clone(),
                run_called: Arc::new(Mutex::new(false)),
            }),
        );

        let pipeline =
            pipeline_with_chain(vec!["first.fail", "second.ok"], map);

        let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_for_sink = captured.clone();
        let sink: StreamingChunkSink = Arc::new(move |delta, _done| {
            captured_for_sink.lock().push(delta);
        });

        let res = pipeline
            .run_with_sink(
                Capability::Translate,
                TaskInput::Translate {
                    text: "x".into(),
                    source_language: "en".into(),
                    target_language: "ja".into(),
                },
                sink,
            )
            .await
            .expect("fallback should produce a result");

        assert_eq!(res.used_provider, "second.ok");
        assert!(*captured_called.lock(), "second provider must be exercised");
        assert_eq!(
            captured.lock().clone(),
            vec!["o".to_string(), "k".into()],
            "only the successful provider's deltas reach the sink"
        );
        // The first provider's failure shows up in fallback_reasons.
        assert!(
            res.fallback_reasons
                .iter()
                .any(|r| r.contains("first.fail")),
            "fallback reasons should mention the failed provider"
        );
    }
}
