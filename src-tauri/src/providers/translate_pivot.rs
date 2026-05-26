//! Two-stage pivot translator that bridges arbitrary language pairs
//! through English.
//!
//! When the router (`translation_router::route`) returns a
//! [`RoutingDecision::Pivot`](crate::translation_router::RoutingDecision::Pivot),
//! the pipeline instantiates one of these wrapping the two
//! provider handles named by `stage1_tier` / `stage2_tier`. The
//! wrapper implements the same [`Provider`] trait the pipeline
//! drives, so the higher layers do not need to know that two
//! models are involved.
//!
//! Pipeline:
//!
//! ```text
//! TaskInput::Translate { text, src, tgt }
//!     │
//!     ├─► stage1 provider: Translate(src → en) → intermediate English
//!     │       │
//!     │       └─► strip STOP_SUFFIXES from intermediate (Phase 27 v2 fix)
//!     │
//!     └─► stage2 provider: Translate(en → tgt) → final output
//! ```
//!
//! Phase 27 quality-review (`docs/benchmarks/2026-05-12-phase27-
//! pivot-quality-review.md`) measured this two-stage path
//! producing better translations than either stage's direct
//! attempt at the same pair, with the runtime EOS fix in
//! `translate_embedded_llm.rs` keeping the intermediate clean.

use async_trait::async_trait;

use super::translate_embedded_llm::{
    check_and_strip_stop_suffix, language_name, strip_chat_template_garbage,
};
use super::{
    Capability, Provider, ProviderError, ProviderHandle, StreamingChunkSink, TaskInput, TaskOutput,
};

/// Provider wrapper running `(src → en) → (en → tgt)` through two
/// inner providers. The inner handles are owned via `Arc` so the
/// same `EmbeddedLlmTranslator` instance can appear in multiple
/// pivot configurations (e.g. JA-bridge serves both as Stage 1
/// for ja→X and as Stage 2 for X→ja).
pub struct PivotTranslator {
    id: String,
    stage1: ProviderHandle,
    stage2: ProviderHandle,
}

impl PivotTranslator {
    /// Construct a pivot wrapper with explicit stage assignments.
    /// `id` becomes the provider id chains reference (the routing
    /// logic generates these dynamically per `(tier, src, tgt)`
    /// triple).
    pub fn new(id: impl Into<String>, stage1: ProviderHandle, stage2: ProviderHandle) -> Self {
        Self {
            id: id.into(),
            stage1,
            stage2,
        }
    }
}

/// Wrap the Stage-1 English intermediate with an explicit Stage-2
/// target-language reminder before handing it off.
///
/// Why: the 2026-05-18 routed eval surfaced that gemma-4-e2b/e4b
/// echoes English back when asked to translate technical English
/// intermediates into French / German / Korean. The same model
/// happily translates everyday English to those targets, so the
/// problem is target-language LOCKING under technical inputs, not
/// raw translation capability. Prepending one short directive sentence
/// (in addition to the system prompt and few-shot examples) anchors
/// the model on the destination language without altering the
/// translation header that `build_user_prompt` already produces.
///
/// Stage 1 is unaffected — its target is always English and the
/// directive would be a tautology. The wrapper only fires when the
/// final user-facing target is non-English; for `en` targets the
/// returned text is the cleaned intermediate unchanged.
pub fn wrap_stage2_with_target_hint(intermediate: &str, target_language: &str) -> String {
    let tgt_name = language_name(target_language);
    if tgt_name.eq_ignore_ascii_case("English") {
        return intermediate.to_string();
    }
    format!(
        "(The following English text MUST be rendered in {tgt}. Output {tgt} only — never echo English back.)\n\n{src}",
        tgt = tgt_name,
        src = intermediate,
    )
}

/// Strip Stage-1 artifacts before handing the intermediate
/// English text to Stage 2. Phase 27 v2 evidence: when this
/// cleanup is omitted, an LLM-emitted `<|im_end|>` or `|im_end|>`
/// substring at the end of Stage-1 output gets dutifully
/// translated by Stage 2, producing visible garbage like
/// "Hello, world 名無し" → "你好，世界 名無し".
///
/// Currently strips:
/// - One trailing occurrence of any `STOP_SUFFIXES` entry (see
///   the runtime side for the canonical list).
/// - Surrounding ASCII / Unicode whitespace.
///
/// Kept narrow on purpose: a more aggressive cleanup (e.g.
/// stripping every Japanese token from supposedly-English Stage-1
/// output) risks deleting legitimate proper nouns (人名, 商標).
/// The runtime EOS fix in Phase 27 v4 already handles the only
/// pattern this function was specifically built for.
pub fn clean_pivot_intermediate(text: &str) -> String {
    let mut s = text.trim().to_string();
    // Loop until no more suffix matches. In practice a single
    // pass suffices because the runtime emits at most one EOS
    // marker, but the loop costs nothing for the common case
    // (early-exits on the first iteration that finds no match).
    while check_and_strip_stop_suffix(&mut s) {
        // Trim again — the suffix may have been preceded by
        // whitespace that becomes the new trailing run.
        s = s.trim().to_string();
    }
    // Also catch malformed chat-template variants (Hy-MT2-style
    // byte-level leaks like `<|im_end(){`). The suffix matcher
    // above only catches well-formed terminators; the
    // prefix-anywhere scrubber catches the rest.
    if strip_chat_template_garbage(&mut s, 0) {
        s = s.trim().to_string();
    }
    s
}

#[async_trait]
impl Provider for PivotTranslator {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &[Capability] {
        // Pivot only translates. Explain / Summarize go through a
        // single-model path because there is no second-language
        // bridge concept for them.
        &[Capability::Translate]
    }

    fn is_local(&self) -> bool {
        // Composition of locals is local. If either stage were
        // ever a cloud provider this would have to flip, but the
        // router only emits Pivot when both stages are tiers from
        // the local registry.
        self.stage1.is_local() && self.stage2.is_local()
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let (text, source_language, target_language) = match input {
            TaskInput::Translate {
                text,
                source_language,
                target_language,
                ..
            } => (text, source_language, target_language),
            _ => {
                return Err(ProviderError::Other(
                    "PivotTranslator only supports Translate input".into(),
                ));
            }
        };

        // Stage 1: source → English.
        let stage1_out = self
            .stage1
            .run(TaskInput::Translate {
                text,
                source_language,
                target_language: "en".to_string(),
                glossary: Vec::new(),
            })
            .await?;
        let intermediate = match stage1_out {
            TaskOutput::Text(t) => t,
            TaskOutput::TextChunk { delta, .. } => delta,
            other => {
                return Err(ProviderError::Other(format!(
                    "PivotTranslator stage1 returned non-text output: {other:?}",
                )));
            }
        };
        let cleaned = clean_pivot_intermediate(&intermediate);
        if cleaned.is_empty() {
            return Err(ProviderError::LowConfidence);
        }

        // Stage 2: English → target. The intermediate is wrapped
        // with a one-line target-language reminder (see
        // `wrap_stage2_with_target_hint`) to keep small multilingual
        // models like gemma-4-e2b/e4b from echoing English back on
        // technical inputs.
        let stage2_text = wrap_stage2_with_target_hint(&cleaned, &target_language);
        self.stage2
            .run(TaskInput::Translate {
                text: stage2_text,
                source_language: "en".to_string(),
                target_language,
                glossary: Vec::new(),
            })
            .await
    }

    async fn run_streaming(
        &self,
        input: TaskInput,
        sink: StreamingChunkSink,
    ) -> Result<TaskOutput, ProviderError> {
        let (text, source_language, target_language) = match input {
            TaskInput::Translate {
                text,
                source_language,
                target_language,
                ..
            } => (text, source_language, target_language),
            _ => {
                return Err(ProviderError::Other(
                    "PivotTranslator only supports Translate input".into(),
                ));
            }
        };

        // Stage 1 is run non-streaming: per-token streaming of an
        // intermediate language the user did not ask for would
        // flicker into the overlay and immediately be replaced by
        // Stage 2's output. Far less confusing to wait for the
        // English intermediate and stream only Stage 2.
        let stage1_out = self
            .stage1
            .run(TaskInput::Translate {
                text,
                source_language,
                target_language: "en".to_string(),
                glossary: Vec::new(),
            })
            .await?;
        let intermediate = match stage1_out {
            TaskOutput::Text(t) => t,
            TaskOutput::TextChunk { delta, .. } => delta,
            other => {
                return Err(ProviderError::Other(format!(
                    "PivotTranslator stage1 returned non-text output: {other:?}",
                )));
            }
        };
        let cleaned = clean_pivot_intermediate(&intermediate);
        if cleaned.is_empty() {
            return Err(ProviderError::LowConfidence);
        }

        let stage2_text = wrap_stage2_with_target_hint(&cleaned, &target_language);
        self.stage2
            .run_streaming(
                TaskInput::Translate {
                    text: stage2_text,
                    source_language: "en".to_string(),
                    target_language,
                    glossary: Vec::new(),
                },
                sink,
            )
            .await
    }

    async fn preload(&self) -> Result<(), ProviderError> {
        // Warm BOTH stages so the first real translation never
        // pays a cold-load. The Pipeline already swallows
        // individual preload failures, so propagating either
        // error is the right behaviour — a missing model in
        // either stage means this pivot route is non-functional
        // and the caller needs to know up front.
        self.stage1.preload().await?;
        self.stage2.preload().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// Mock provider that records every input it was called with
    /// and replies with a canned `TaskOutput`. Used in pivot
    /// tests where we want to verify the wrapper threaded the
    /// English intermediate correctly without standing up two
    /// real `EmbeddedLlmTranslator` instances.
    struct MockTranslator {
        id: String,
        reply: TaskOutput,
        calls: Mutex<Vec<TaskInput>>,
        run_count: AtomicUsize,
    }

    impl MockTranslator {
        fn new(id: &str, reply_text: &str) -> Self {
            Self {
                id: id.into(),
                reply: TaskOutput::Text(reply_text.into()),
                calls: Mutex::new(Vec::new()),
                run_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl Provider for MockTranslator {
        fn id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> &[Capability] {
            &[Capability::Translate]
        }
        fn is_local(&self) -> bool {
            true
        }
        async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
            self.calls.lock().unwrap().push(input);
            self.run_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.reply.clone())
        }
    }

    fn translate_input(text: &str, src: &str, tgt: &str) -> TaskInput {
        TaskInput::Translate {
            text: text.into(),
            source_language: src.into(),
            target_language: tgt.into(),
            glossary: Vec::new(),
        }
    }

    fn extract_text(out: &TaskOutput) -> &str {
        match out {
            TaskOutput::Text(t) => t,
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn pivot_threads_english_intermediate_through_two_stages() {
        let stage1 = Arc::new(MockTranslator::new("s1", "Hello, world."));
        let stage2 = Arc::new(MockTranslator::new("s2", "你好，世界"));
        let pivot = PivotTranslator::new("pivot.test", stage1.clone(), stage2.clone());

        let out = pivot
            .run(translate_input("こんにちは", "ja", "zh-CN"))
            .await
            .unwrap();
        assert_eq!(extract_text(&out), "你好，世界");

        // Stage 1 should have seen ja→en exactly once.
        let s1_calls = stage1.calls.lock().unwrap();
        assert_eq!(s1_calls.len(), 1);
        match &s1_calls[0] {
            TaskInput::Translate {
                text,
                source_language,
                target_language,
                ..
            } => {
                assert_eq!(text, "こんにちは");
                assert_eq!(source_language, "ja");
                assert_eq!(target_language, "en");
            }
            other => panic!("unexpected stage1 input: {other:?}"),
        }

        // Stage 2 should have seen en→zh-CN with the cleaned
        // intermediate as input. The wrapper now prepends a one-
        // line target-language reminder to keep small multilingual
        // models from echoing English back — check both the
        // directive and the cleaned source are present.
        let s2_calls = stage2.calls.lock().unwrap();
        assert_eq!(s2_calls.len(), 1);
        match &s2_calls[0] {
            TaskInput::Translate {
                text,
                source_language,
                target_language,
                ..
            } => {
                assert!(
                    text.contains("rendered in Chinese"),
                    "Stage 2 input missing target-language reminder: {text}"
                );
                assert!(
                    text.ends_with("Hello, world."),
                    "Stage 2 input does not end with intermediate text: {text}"
                );
                assert_eq!(source_language, "en");
                assert_eq!(target_language, "zh-CN");
            }
            other => panic!("unexpected stage2 input: {other:?}"),
        }
    }

    #[tokio::test]
    async fn pivot_strips_stop_suffix_from_intermediate() {
        // Regression for the Phase 27 v2 bug: Stage 1 emits an
        // EOS marker as literal text, Stage 2 translates the
        // marker verbatim into the target language. The wrapper
        // MUST strip the marker before forwarding.
        let cases = [
            ("Hello, world.<|im_end|>", "Hello, world."),
            ("Hello, world.|im_end|>", "Hello, world."),
            ("Hello, world.<end_of_turn>", "Hello, world."),
            ("   Hello, world.   <|im_end|>   ", "Hello, world."),
        ];
        for (raw_intermediate, expected_clean) in cases {
            let stage1 = Arc::new(MockTranslator::new("s1", raw_intermediate));
            let stage2 = Arc::new(MockTranslator::new("s2", "TARGET"));
            let pivot = PivotTranslator::new("pivot.test", stage1.clone(), stage2.clone());

            pivot
                .run(translate_input("dummy", "ja", "fr"))
                .await
                .unwrap();

            let s2_calls = stage2.calls.lock().unwrap();
            assert_eq!(s2_calls.len(), 1);
            match &s2_calls[0] {
                TaskInput::Translate { text, .. } => {
                    assert!(
                        text.ends_with(expected_clean),
                        "Stage 2 received uncleaned intermediate; got {text}, expected suffix {expected_clean}",
                    );
                }
                other => panic!("unexpected stage2 input: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn pivot_rejects_non_translate_inputs() {
        let stage1 = Arc::new(MockTranslator::new("s1", ""));
        let stage2 = Arc::new(MockTranslator::new("s2", ""));
        let pivot = PivotTranslator::new("pivot.test", stage1.clone(), stage2.clone());

        let err = pivot
            .run(TaskInput::Summarize {
                text: "anything".into(),
            })
            .await
            .unwrap_err();
        match err {
            ProviderError::Other(msg) => assert!(msg.contains("Translate")),
            other => panic!("expected Other error, got {other:?}"),
        }
        // Neither stage should have been called.
        assert_eq!(stage1.run_count.load(Ordering::SeqCst), 0);
        assert_eq!(stage2.run_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn pivot_treats_empty_intermediate_as_low_confidence() {
        // Stage 1 produced nothing usable (e.g. the model emitted
        // only an EOS marker). Bailing as LowConfidence lets the
        // chain fall back to a cloud provider if the user has
        // one configured, rather than crashing or feeding empty
        // text into Stage 2.
        let stage1 = Arc::new(MockTranslator::new("s1", "<|im_end|>"));
        let stage2 = Arc::new(MockTranslator::new("s2", "should not run"));
        let pivot = PivotTranslator::new("pivot.test", stage1.clone(), stage2.clone());

        let err = pivot
            .run(translate_input("anything", "ja", "fr"))
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::LowConfidence));
        // Stage 2 must not be called when Stage 1 fails.
        assert_eq!(stage2.run_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn wrap_stage2_with_target_hint_prepends_language_reminder() {
        let wrapped = wrap_stage2_with_target_hint("Hello, world.", "fr");
        assert!(
            wrapped.contains("MUST be rendered in French"),
            "missing target-language reminder: {wrapped}",
        );
        assert!(
            wrapped.ends_with("Hello, world."),
            "intermediate must remain at the end: {wrapped}",
        );
    }

    #[test]
    fn wrap_stage2_with_target_hint_skips_english_target() {
        // Stage 2 with target=English is a no-op direction (the
        // intermediate IS English). The directive would either
        // confuse the model or trigger a redundant pass — leave
        // the text alone.
        let wrapped = wrap_stage2_with_target_hint("Hello, world.", "en");
        assert_eq!(wrapped, "Hello, world.");
        let wrapped_full = wrap_stage2_with_target_hint("Hello, world.", "English");
        assert_eq!(wrapped_full, "Hello, world.");
    }

    #[test]
    fn wrap_stage2_with_target_hint_resolves_iso_codes_to_full_names() {
        // Models like gemma-4-e2b reliably respect `Korean` /
        // `German` / `Chinese` directives but not bare `ko` / `de` /
        // `zh`. The wrapper relies on `language_name` for the
        // expansion — sanity-check the wiring.
        for (code, name) in [
            ("ko", "Korean"),
            ("de", "German"),
            ("zh", "Chinese"),
            ("ja", "Japanese"),
        ] {
            let wrapped = wrap_stage2_with_target_hint("x", code);
            assert!(
                wrapped.contains(name),
                "code {code} should resolve to {name}: {wrapped}"
            );
        }
    }

    #[tokio::test]
    async fn pivot_is_local_when_both_stages_local() {
        let stage1 = Arc::new(MockTranslator::new("s1", ""));
        let stage2 = Arc::new(MockTranslator::new("s2", ""));
        let pivot = PivotTranslator::new("pivot.test", stage1, stage2);
        assert!(pivot.is_local());
    }

    #[tokio::test]
    async fn streaming_runs_stage1_non_streaming_then_streams_stage2() {
        use std::sync::atomic::AtomicUsize;
        let stage1 = Arc::new(MockTranslator::new("s1", "Hello, world."));
        let stage2 = Arc::new(MockTranslator::new("s2", "Hola, mundo"));
        let pivot = PivotTranslator::new("pivot.test", stage1.clone(), stage2.clone());

        let chunk_count = Arc::new(AtomicUsize::new(0));
        let cc = chunk_count.clone();
        let sink: StreamingChunkSink = Arc::new(move |_chunk, done| {
            if done {
                cc.fetch_add(1, Ordering::SeqCst);
            }
        });

        pivot
            .run_streaming(translate_input("こんにちは", "ja", "es"), sink)
            .await
            .unwrap();

        // The default `run_streaming` impl from the Provider trait
        // forwards the whole reply as a single `done=true` chunk
        // (because the mock does not override run_streaming).
        assert_eq!(chunk_count.load(Ordering::SeqCst), 1);
        assert_eq!(stage1.run_count.load(Ordering::SeqCst), 1);
        assert_eq!(stage2.run_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn clean_pivot_intermediate_handles_all_stop_suffixes() {
        // Direct test of the helper — exhaustive over the
        // STOP_SUFFIXES list at the time of writing.
        assert_eq!(clean_pivot_intermediate("text<|im_end|>"), "text");
        assert_eq!(clean_pivot_intermediate("text|im_end|>"), "text");
        assert_eq!(clean_pivot_intermediate("text<end_of_turn>"), "text");
        assert_eq!(clean_pivot_intermediate("  text  "), "text");
        assert_eq!(clean_pivot_intermediate(""), "");
        // Mid-string chat-template prefix is also stripped (along
        // with anything after it). The 2026-05-26 evidence: when
        // the marker leaks mid-stream from Hy-MT2-style byte-level
        // tokenisation, what follows is reliably garbage (the
        // model has fallen out of distribution); preserving it
        // would re-introduce the very bug we are trying to fix.
        // The trade-off is acceptable for natural-language
        // translation, where literal `<|im_end|>` as content is
        // vanishingly rare.
        assert_eq!(
            clean_pivot_intermediate("text<|im_end|>more"),
            "text"
        );
    }
}
