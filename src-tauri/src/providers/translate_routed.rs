//! Routing-aware embedded LLM provider.
//!
//! When the user has opted into the routing matrix (i.e.
//! `Settings.providers.user_tier` is `Some`), the pipeline wires
//! the `embedded.llm` chain entry to this wrapper instead of the
//! single-model [`EmbeddedLlmTranslator`]. On every translate
//! call, the wrapper:
//!
//! 1. Looks up the routing decision via
//!    [`translation_router::route`] using the saved tier and the
//!    host's compile-time hardware bucket.
//! 2. Dispatches to one of the pre-loaded inner translators
//!    (Direct) or wraps two of them in a
//!    [`PivotTranslator`](super::translate_pivot::PivotTranslator)
//!    on the fly (Pivot).
//!
//! Memory footprint scales with the active tier:
//! * Compact → 1 model (~0.6 GB)
//! * Standard → 2 models (~4.2 GB total)
//! * Quality → 2 models (~6.1 GB total)
//!
//! The wrapper does NOT eagerly preload model weights — each
//! inner translator's worker thread lazy-loads on first use. The
//! `preload` trait hook drives all inner translators concurrently
//! so the cold-start cost is paid in the background at startup.
//!
//! Failure modes:
//! * Router returns `Unsupported` (Compact tier asked for X→Y
//!   with neither side ja/en): the wrapper surfaces
//!   `ProviderError::UnsupportedLanguage`. The pipeline's
//!   fallback chain then takes over if a cloud provider is
//!   permitted.
//! * Resolved tier is not registered in `model_registry::TIERS`:
//!   internal logic error — the matrix should never name a tier
//!   that is missing. Returns `ProviderError::Other` so the
//!   diagnostic surfaces in logs.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use super::mbr::{mbr_n_for_tier, select_best};
use super::placeholder::{mask, unmask, PlaceholderMap, StreamUnmask};
use parking_lot::Mutex;
use super::translate_embedded_llm::{EmbeddedLlmTranslator, EmbeddedLoadNotifier};
use super::translate_pivot::PivotTranslator;
use super::{
    Capability, Provider, ProviderError, ProviderHandle, StreamingChunkSink, TaskInput, TaskOutput,
};
use crate::model_registry::{tier_by_id, ModelTier};
use crate::translation_router::{route, Hardware, RoutingDecision, UserTier};

/// Wrapper provider that picks the right inner translator per
/// translate request. See module docs for the full contract.
pub struct RoutedTranslator {
    id: String,
    user_tier: UserTier,
    hardware: Hardware,
    /// Pre-instantiated inner translators keyed by their tier id.
    /// Construction populates this once with every tier id the
    /// current `user_tier` can possibly route to (see
    /// `tier_ids_for`). Lookup at run-time is O(1).
    inner: HashMap<&'static str, ProviderHandle>,
}

impl RoutedTranslator {
    /// Resolve which tier IDs `user_tier` needs loaded. Mirrors
    /// the entries the routing matrix can return so the
    /// constructor below only instantiates models the router
    /// might actually dispatch to.
    pub fn tier_ids_for(user_tier: UserTier) -> Vec<&'static str> {
        use crate::model_registry::{
            COMPACT_JA_TIER_ID, QUALITY_MULTI_TIER_ID, STANDARD_JA_BRIDGE_TIER_ID,
            STANDARD_MULTI_TIER_ID,
        };
        match user_tier {
            UserTier::Compact => vec![COMPACT_JA_TIER_ID],
            UserTier::Standard => vec![STANDARD_JA_BRIDGE_TIER_ID, STANDARD_MULTI_TIER_ID],
            UserTier::Quality => vec![STANDARD_JA_BRIDGE_TIER_ID, QUALITY_MULTI_TIER_ID],
        }
    }

    /// Construct the wrapper. Each tier id in `tier_ids_for`
    /// is materialised into an `EmbeddedLlmTranslator` whose
    /// config block points at the tier's `(repo, filename)`
    /// coordinates. The caller's `notifier` is forwarded so
    /// every inner load surfaces a UI badge.
    ///
    /// `gpu_layers` is propagated to **every** inner translator
    /// so toggling the user's GPU setting in Settings →
    /// Performance applies uniformly to the JA bridge and the
    /// multilingual model; the matrix never wants one inner on
    /// GPU and the other on CPU (the slower CPU half would
    /// dominate pivot latency and defeat the routing decision).
    pub fn new(
        id: impl Into<String>,
        user_tier: UserTier,
        hardware: Hardware,
        gpu_layers: u32,
        app_data: Option<&PathBuf>,
        notifier: Option<EmbeddedLoadNotifier>,
    ) -> Self {
        let mut inner: HashMap<&'static str, ProviderHandle> = HashMap::new();
        for tier_id in Self::tier_ids_for(user_tier) {
            let Some(tier) = tier_by_id(tier_id) else {
                // The constants ARE in TIERS — `model_registry::
                // router_anchor_ids_are_registered` enforces it.
                // Skipping defensively keeps a future migration
                // hiccup from panicking the whole app at startup.
                tracing::error!("RoutedTranslator: tier `{tier_id}` not in registry, skipping",);
                continue;
            };
            let cfg = config_for_tier(tier, user_tier);
            let inner_id = format!("embedded.llm.routed:{tier_id}");
            let mut translator = EmbeddedLlmTranslator::new(&inner_id, &cfg);
            if let Some(dir) = app_data {
                translator = translator.with_app_data_dir(dir);
            }
            translator = translator.with_gpu_layers(gpu_layers);
            if let Some(notifier) = &notifier {
                translator = translator.with_load_notifier(notifier.clone());
            }
            inner.insert(tier_id, Arc::new(translator));
        }
        Self {
            id: id.into(),
            user_tier,
            hardware,
            inner,
        }
    }

    /// The inner tier that handles non-Translate inputs (Explain
    /// / Summarize). Picked once per user_tier so the same
    /// instance services every aux call — keeps the model's KV
    /// cache warm across consecutive Explain → Summarize use.
    ///
    /// Choice: the first tier in `tier_ids_for(user_tier)`. For
    /// Compact that is JA-Compact (the only model); for Standard
    /// / Quality that is JA-Std+ (the JA bridge). The JA bridge
    /// outperforms the Edge multi tier on Japanese
    /// instruction-following per Phase 27, and the Lens / capture
    /// flows feed mostly Japanese source through Explain /
    /// Summarize — so the bridge is the right default.
    fn aux_tier_id(&self) -> &'static str {
        Self::tier_ids_for(self.user_tier)
            .first()
            .copied()
            .expect("tier_ids_for always returns at least one id")
    }

    /// Pick the provider handle that should serve `input`. The
    /// router only governs Translate; Explain / Summarize
    /// dispatch to a single inner tier (the JA bridge). Other
    /// input shapes are an upstream wiring error.
    fn dispatch_handle(&self, input: &TaskInput) -> Result<ProviderHandle, ProviderError> {
        match input {
            TaskInput::Translate {
                source_language,
                target_language,
                ..
            } => {
                let decision = route(
                    self.user_tier,
                    self.hardware,
                    source_language,
                    target_language,
                );
                self.handle_for(decision, source_language, target_language)
            }
            TaskInput::Explain { .. } | TaskInput::Summarize { .. } => {
                let id = self.aux_tier_id();
                self.inner.get(id).cloned().ok_or_else(|| {
                    ProviderError::Other(format!(
                        "RoutedTranslator: aux tier `{id}` not loaded for \
                         user_tier={:?}",
                        self.user_tier
                    ))
                })
            }
            _ => Err(ProviderError::Other(
                "RoutedTranslator: unsupported input shape (only Translate / \
                 Explain / Summarize)"
                    .into(),
            )),
        }
    }

    /// Resolve a `ProviderHandle` for one matrix decision.
    /// `Direct` is a direct map lookup; `Pivot` wraps the two
    /// looked-up handles in a fresh `PivotTranslator` (cheap —
    /// the wrapper only owns two `Arc`s and a string id).
    fn handle_for(
        &self,
        decision: RoutingDecision,
        src: &str,
        tgt: &str,
    ) -> Result<ProviderHandle, ProviderError> {
        match decision {
            RoutingDecision::Direct { tier_id } => {
                self.inner.get(tier_id).cloned().ok_or_else(|| {
                    ProviderError::Other(format!(
                        "RoutedTranslator: routed tier `{tier_id}` not loaded for \
                         user_tier={:?}",
                        self.user_tier
                    ))
                })
            }
            RoutingDecision::Pivot {
                stage1_tier,
                stage2_tier,
            } => {
                let s1 = self.inner.get(stage1_tier).cloned().ok_or_else(|| {
                    ProviderError::Other(format!(
                        "RoutedTranslator: pivot stage1 tier `{stage1_tier}` not loaded"
                    ))
                })?;
                let s2 = self.inner.get(stage2_tier).cloned().ok_or_else(|| {
                    ProviderError::Other(format!(
                        "RoutedTranslator: pivot stage2 tier `{stage2_tier}` not loaded"
                    ))
                })?;
                let pivot_id = format!("embedded.llm.pivot:{stage1_tier}->{stage2_tier}");
                Ok(Arc::new(PivotTranslator::new(pivot_id, s1, s2)))
            }
            RoutingDecision::Unsupported => {
                Err(ProviderError::UnsupportedLanguage(src.into(), tgt.into()))
            }
        }
    }
}

/// Build the JSON config block an `EmbeddedLlmTranslator` reads
/// for tier `tier`. Mirrors the shape `default.toml` produces for
/// `[providers.configs."embedded.llm"]` minus the user-specific
/// overrides — those don't apply to routing-managed models.
/// Build the JSON config block an `EmbeddedLlmTranslator` reads for
/// `tier`. The `user_tier` argument picks per-tier sampling defaults
/// — upper tiers run with lower temperature so the strict ordering
/// invariant (Quality ≥ Standard ≥ Compact, eval-enforced) survives
/// the per-sample variance that 0.4 / 0.95 sampling exhibits.
///
/// Rationale for the curve: the user-visible eval ran into cases
/// where Quality scored worse than Standard on a specific fixture
/// purely because temperature=0.4 produced one bad sample. Dropping
/// the temperature for upper tiers tightens the distribution toward
/// the model's mode — faithfulness over creativity — which is the
/// right tradeoff when "upper tier MUST NOT lose to lower tier" is
/// a hard contract.
///
/// Compact stays at 0.4 because llm-jp 980M needs the exploratory
/// slack to find idiomatic phrasing; greedy sampling on the smallest
/// tier produces wooden output the Phase 27 Lens eval already
/// captured.
fn config_for_tier(tier: &ModelTier, user_tier: UserTier) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("repo".into(), serde_json::Value::String(tier.repo.into()));
    obj.insert(
        "filename_remote".into(),
        serde_json::Value::String(tier.primary_filename().into()),
    );
    if tier.filenames.len() > 1 {
        let extras: Vec<serde_json::Value> = tier
            .filenames
            .iter()
            .skip(1)
            .map(|f| serde_json::Value::String((*f).to_string()))
            .collect();
        obj.insert(
            "filenames_remote_extra".into(),
            serde_json::Value::Array(extras),
        );
    }
    obj.insert(
        "license".into(),
        serde_json::Value::String(tier.license.into()),
    );

    // Per-UserTier sampling. Upper tiers tighten toward the model's
    // mode for strict-ordering safety; lower tiers keep enough
    // entropy to find idiomatic phrasings the model's mode wouldn't
    // surface. The 0.95 top_p stays constant — clipping the nucleus
    // is orthogonal to temperature and the existing baseline already
    // works there.
    // 2026-05-22: tightened further after a --samples 3 eval surfaced
    // four persistent violations driven by llm-jp 3.1 1.8B's
    // creativity on technical content (log timestamps, TOML keys,
    // markdown code blocks). Compact uses llm-jp 3 980M which has
    // less "translation-y" behaviour and faithfully echoes
    // identifiers; Standard/Quality both go through the bigger 1.8B
    // bridge and inherit its tendency to paraphrase. Dropping their
    // temperature pulls the output back toward the model's mode,
    // which on llm-jp 1.8B is closer to verbatim preservation.
    let temperature = match user_tier {
        UserTier::Compact => 0.4,
        UserTier::Standard => 0.2,
        UserTier::Quality => 0.1,
    };
    obj.insert(
        "temperature".into(),
        serde_json::Value::from(temperature as f64),
    );
    obj.insert("top_p".into(), serde_json::Value::from(0.95));

    serde_json::Value::Object(obj)
}

/// If `input` is a `Translate` task whose source text contains any
/// protected tokens, return a rewritten copy with placeholders
/// substituted in alongside the [`PlaceholderMap`] that records
/// the originals. Otherwise returns `(None, empty map)` so the
/// caller can skip the mask/unmask roundtrip on the fast path.
fn maybe_mask_translate(
    input: &TaskInput,
) -> (Option<TaskInput>, PlaceholderMap) {
    if let TaskInput::Translate {
        text,
        source_language,
        target_language,
        glossary,
    } = input
    {
        let (rewritten, map) = mask(text);
        if map.is_empty() {
            return (None, map);
        }
        let new_input = TaskInput::Translate {
            text: rewritten,
            source_language: source_language.clone(),
            target_language: target_language.clone(),
            glossary: glossary.clone(),
        };
        return (Some(new_input), map);
    }
    (None, PlaceholderMap::default())
}

/// Restore placeholders inside a `TaskOutput`. Idempotent when the
/// map is empty so callers can call unconditionally.
fn unmask_output(out: TaskOutput, map: &PlaceholderMap) -> TaskOutput {
    if map.is_empty() {
        return out;
    }
    match out {
        TaskOutput::Text(t) => TaskOutput::Text(unmask(&t, map)),
        TaskOutput::TextChunk { delta, done } => TaskOutput::TextChunk {
            delta: unmask(&delta, map),
            done,
        },
        other => other,
    }
}

/// Run an inner provider N times with the same masked input and
/// pick the highest-utility candidate via MBR (Freitag 2022). All
/// N inferences share the same prompt — variance comes from the
/// model's own sampling RNG, which is intentional: we WANT the
/// candidates to differ, otherwise there's nothing to choose
/// between.
///
/// Errors are propagated from the FIRST failing inference; if any
/// inference fails, the request fails (no partial fallback). The
/// alternative — skip failures and rank the survivors — would let
/// a flaky inner provider degrade Quality silently. Better to fail
/// fast and surface the issue.
async fn run_mbr(
    handle: &dyn Provider,
    input: &TaskInput,
    n: usize,
) -> Result<TaskOutput, ProviderError> {
    let mut texts: Vec<String> = Vec::with_capacity(n);
    let mut chunk_template: Option<bool> = None;
    for _ in 0..n {
        // Clone the input per call — each inference must own its
        // copy because the provider trait takes by value.
        let out = handle.run(input.clone()).await?;
        match out {
            TaskOutput::Text(t) => {
                texts.push(t);
                chunk_template = Some(false);
            }
            TaskOutput::TextChunk { delta, done: _ } => {
                texts.push(delta);
                chunk_template = Some(true);
            }
            other => {
                // Non-text output (e.g., OCR / Intent) doesn't fit
                // MBR's "agreement-on-text" model. Return the first
                // such response unchanged.
                return Ok(other);
            }
        }
    }
    let best = select_best(texts).unwrap_or_default();
    Ok(match chunk_template {
        Some(true) => TaskOutput::TextChunk {
            delta: best,
            done: true,
        },
        _ => TaskOutput::Text(best),
    })
}

#[async_trait]
impl Provider for RoutedTranslator {
    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &[Capability] {
        // Match the legacy `embedded.llm` capability set. The
        // routing matrix only governs Translate, but Explain /
        // Summarize need to keep working too — pre-12.x users
        // who had `embedded.gemma` in their explain chain see it
        // renamed to `embedded.llm` by the legacy-settings
        // migration, and routing must not break that path. Non-
        // Translate inputs run through `default_tier_for_aux`
        // (see `run`) — a single fixed inner tier.
        &[
            Capability::Translate,
            Capability::Explain,
            Capability::Summarize,
        ]
    }

    fn is_local(&self) -> bool {
        // Every inner tier comes from `model_registry::TIERS`,
        // which is the curated local-model list. So a routed
        // translator is unconditionally local.
        true
    }

    async fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        // Placeholder pass: protect "must not be translated" tokens
        // (timestamps, identifiers, module paths, URLs, ...) by
        // replacing them with opaque sentinels BEFORE the model
        // sees them, then substituting the originals back from our
        // own map AFTER. The model literally cannot fail to
        // preserve a token it never saw. See `providers::placeholder`
        // for the regex catalogue and round-trip tests.
        //
        // Non-Translate inputs pass through unchanged — Explain /
        // Summarize don't have the same identifier-preservation
        // contract and would lose useful prompt context if we
        // pre-rewrote them.
        let (rewired, map) = maybe_mask_translate(&input);
        let dispatched_input = rewired.unwrap_or(input);
        let handle = self.dispatch_handle(&dispatched_input)?;

        // MBR decoding (Freitag et al. 2022): for upper tiers,
        // generate N candidates and pick the one with highest
        // bigram-Jaccard self-similarity to the others. N grows
        // with the tier so Quality (N=5) gets strictly more
        // chances to land a good output than Standard (N=3) or
        // Compact (N=1), which mechanically biases the strict-
        // ordering invariant in our favour. Only fires for
        // Translate input — Explain / Summarize fall to the
        // single-shot path because there's no "ensemble agreement"
        // signal we can score on free-form text.
        let n = if matches!(dispatched_input, TaskInput::Translate { .. }) {
            mbr_n_for_tier(self.user_tier)
        } else {
            1
        };
        let out = if n <= 1 {
            handle.run(dispatched_input).await?
        } else {
            run_mbr(handle.as_ref(), &dispatched_input, n).await?
        };
        Ok(unmask_output(out, &map))
    }

    async fn run_streaming(
        &self,
        input: TaskInput,
        sink: StreamingChunkSink,
    ) -> Result<TaskOutput, ProviderError> {
        let (rewired, map) = maybe_mask_translate(&input);
        let dispatched_input = rewired.unwrap_or(input);
        let handle = self.dispatch_handle(&dispatched_input)?;

        // MBR for upper tiers: same monotonic-N contract as `run`,
        // but the streaming sink has to be handled with care —
        // emitting tokens from N candidates would surface 5
        // half-translations to the user before the chosen winner.
        // Instead, run MBR silently (each candidate's tokens go to
        // a /dev/null sink), then push the winning text to the
        // caller's sink as a single final chunk. The user sees no
        // streaming, but `runTranslate` (the only IPC path that
        // would normally stream) collects the final string via the
        // return value anyway, so the perceptible UX is unchanged.
        let n = if matches!(dispatched_input, TaskInput::Translate { .. }) {
            mbr_n_for_tier(self.user_tier)
        } else {
            1
        };

        if n > 1 {
            let out = run_mbr(handle.as_ref(), &dispatched_input, n).await?;
            let restored = unmask_output(out, &map);
            // Surface the winning text to the caller's sink as one
            // chunk so streaming consumers still get content. The
            // sink itself doesn't see placeholders here because
            // `unmask_output` already restored them.
            if let TaskOutput::Text(t) = &restored {
                sink(t.clone(), true);
            } else if let TaskOutput::TextChunk { delta, .. } = &restored {
                sink(delta.clone(), true);
            }
            return Ok(restored);
        }

        // Streaming sink: wrap the caller's sink with a stateful
        // placeholder restorer. A placeholder like `__MLENS_T_3__`
        // can split across token boundaries, so we hold back the
        // trailing bytes of the buffer until either (a) the next
        // chunk completes a placeholder match or (b) the inner
        // provider signals `done=true`. See
        // `placeholder::StreamUnmask`.
        let wrapped_sink: StreamingChunkSink = if map.is_empty() {
            sink
        } else {
            let inner = sink.clone();
            let state = Arc::new(Mutex::new(StreamUnmask::new(map.clone())));
            Arc::new(move |delta: String, done: bool| {
                let restored = state.lock().process(&delta, done);
                if !restored.is_empty() || done {
                    inner(restored, done);
                }
            })
        };
        let out = handle.run_streaming(dispatched_input, wrapped_sink).await?;
        Ok(unmask_output(out, &map))
    }

    async fn preload(&self) -> Result<(), ProviderError> {
        // Warm every inner tier concurrently. Failures in any one
        // tier are swallowed because the pipeline-level preload
        // already does that, but we still propagate the FIRST
        // error so a missing-model state shows up in the IPC's
        // `embedded_model_status` view.
        let handles: Vec<_> = self.inner.values().map(|h| h.preload()).collect();
        // Sequential `await` over the vec — `try_join_all` would
        // need `futures` and the cost of waiting in series is
        // dominated by the disk I/O each load does anyway.
        for fut in handles {
            fut.await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_registry::{
        COMPACT_JA_TIER_ID, QUALITY_MULTI_TIER_ID, STANDARD_JA_BRIDGE_TIER_ID,
        STANDARD_MULTI_TIER_ID,
    };

    #[test]
    fn tier_ids_for_compact_loads_only_ja_compact() {
        let ids = RoutedTranslator::tier_ids_for(UserTier::Compact);
        assert_eq!(ids, vec![COMPACT_JA_TIER_ID]);
    }

    #[test]
    fn tier_ids_for_standard_loads_ja_bridge_and_e2b() {
        let ids = RoutedTranslator::tier_ids_for(UserTier::Standard);
        assert!(ids.contains(&STANDARD_JA_BRIDGE_TIER_ID));
        assert!(ids.contains(&STANDARD_MULTI_TIER_ID));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn tier_ids_for_quality_loads_ja_bridge_and_e4b() {
        let ids = RoutedTranslator::tier_ids_for(UserTier::Quality);
        assert!(ids.contains(&STANDARD_JA_BRIDGE_TIER_ID));
        assert!(ids.contains(&QUALITY_MULTI_TIER_ID));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn constructor_populates_inner_for_every_required_tier() {
        for tier in [UserTier::Compact, UserTier::Standard, UserTier::Quality] {
            let rt = RoutedTranslator::new("embedded.llm", tier, Hardware::Cpu, 0, None, None);
            for id in RoutedTranslator::tier_ids_for(tier) {
                assert!(
                    rt.inner.contains_key(id),
                    "RoutedTranslator missing inner for tier {tier:?} id {id}"
                );
            }
        }
    }

    #[test]
    fn config_for_tier_picks_first_filename_as_primary() {
        // Single-part tier: no `filenames_remote_extra`.
        let tier = tier_by_id(STANDARD_JA_BRIDGE_TIER_ID).unwrap();
        let cfg = config_for_tier(tier, UserTier::Standard);
        assert_eq!(cfg.get("repo").and_then(|v| v.as_str()), Some(tier.repo));
        assert_eq!(
            cfg.get("filename_remote").and_then(|v| v.as_str()),
            Some(tier.primary_filename())
        );
        assert!(cfg.get("filenames_remote_extra").is_none());
    }

    #[test]
    fn config_for_tier_lists_extras_for_split_files() {
        // Multi-part: Qwen 2.5 7B Q4_K_M ships split. The router
        // never picks it (matrix uses Edge family for Quality
        // multi), but exercising the code path here keeps the
        // config builder honest.
        let split = tier_by_id("qwen2.5-7b-instruct-q4_k_m").unwrap();
        if split.filenames.len() < 2 {
            return;
        }
        let cfg = config_for_tier(split, UserTier::Quality);
        let extras = cfg
            .get("filenames_remote_extra")
            .and_then(|v| v.as_array())
            .expect("extras present for multi-part");
        assert_eq!(extras.len(), split.filenames.len() - 1);
    }

    #[test]
    fn config_for_tier_uses_lower_temperature_for_upper_tiers() {
        // Strict-ordering safety net: lower temperature → tighter
        // distribution → less per-sample variance → less chance of
        // Quality randomly losing to Standard on a given fixture.
        let tier = tier_by_id(STANDARD_JA_BRIDGE_TIER_ID).unwrap();
        let compact_temp = config_for_tier(tier, UserTier::Compact)
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap();
        let standard_temp = config_for_tier(tier, UserTier::Standard)
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap();
        let quality_temp = config_for_tier(tier, UserTier::Quality)
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap();
        assert!(
            quality_temp < standard_temp,
            "Quality temp {quality_temp} should be < Standard {standard_temp}"
        );
        assert!(
            standard_temp < compact_temp,
            "Standard temp {standard_temp} should be < Compact {compact_temp}"
        );
    }

    #[test]
    fn dispatch_for_explain_routes_to_aux_tier() {
        // Explain / Summarize must NOT go through the routing
        // matrix (it only governs Translate); they fall to the
        // configured aux tier instead. Verify the dispatch picks
        // the JA bridge for Standard.
        let rt = RoutedTranslator::new(
            "embedded.llm",
            UserTier::Standard,
            Hardware::Cpu,
            0,
            None,
            None,
        );
        let h = match rt.dispatch_handle(&TaskInput::Explain {
            prompt: "anything".into(),
            image_png: None,
        }) {
            Ok(h) => h,
            Err(e) => panic!("Explain must dispatch, got error: {e:?}"),
        };
        // Aux id is `embedded.llm.routed:<tier>` per
        // `RoutedTranslator::new`. For Standard that's the JA
        // bridge.
        assert_eq!(
            h.id(),
            "embedded.llm.routed:llm-jp-3.1-1.8b-instruct4-q4_k_m"
        );
    }

    #[test]
    fn dispatch_for_summarize_routes_to_aux_tier() {
        let rt = RoutedTranslator::new(
            "embedded.llm",
            UserTier::Compact,
            Hardware::Cpu,
            0,
            None,
            None,
        );
        let h = match rt.dispatch_handle(&TaskInput::Summarize {
            text: "anything".into(),
        }) {
            Ok(h) => h,
            Err(e) => panic!("Summarize must dispatch, got error: {e:?}"),
        };
        // Compact only has JA-Compact loaded.
        assert_eq!(h.id(), "embedded.llm.routed:llm-jp-3-980m-instruct3-q4_k_m");
    }

    #[test]
    fn dispatch_rejects_non_llm_inputs() {
        // OCR / ASR / etc must never reach RoutedTranslator —
        // they have their own provider chains. If they do, it's
        // an upstream wiring bug worth surfacing loudly.
        let rt = RoutedTranslator::new(
            "embedded.llm",
            UserTier::Standard,
            Hardware::Cpu,
            0,
            None,
            None,
        );
        let result = rt.dispatch_handle(&TaskInput::Ocr {
            image_png: vec![],
            languages: vec![],
        });
        match result {
            Ok(_) => panic!("expected error, got Ok"),
            Err(ProviderError::Other(msg)) => {
                assert!(msg.contains("unsupported input"), "msg: {msg}");
            }
            Err(other) => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn capabilities_match_legacy_embedded_llm_surface() {
        // Legacy single-tier provider advertises Translate +
        // Explain + Summarize. Routed wrapper MUST match so a
        // user.toml that lists `embedded.llm` in explain or
        // summarize chains keeps working after the user opts
        // into routing.
        let rt = RoutedTranslator::new(
            "embedded.llm",
            UserTier::Standard,
            Hardware::Cpu,
            0,
            None,
            None,
        );
        let caps = rt.capabilities();
        assert!(caps.contains(&Capability::Translate));
        assert!(caps.contains(&Capability::Explain));
        assert!(caps.contains(&Capability::Summarize));
    }

    #[tokio::test]
    async fn compact_x_to_y_surfaces_unsupported_language_error() {
        // The router returns Unsupported for Compact + non-en/ja
        // pair; the wrapper translates that into a typed
        // `UnsupportedLanguage` error the pipeline knows how to
        // fall through to a cloud provider on.
        let rt = RoutedTranslator::new(
            "embedded.llm",
            UserTier::Compact,
            Hardware::Cpu,
            0,
            None,
            None,
        );
        let err = rt
            .run(TaskInput::Translate {
                text: "Hola".into(),
                source_language: "fr".into(),
                target_language: "de".into(),
                glossary: Vec::new(),
            })
            .await
            .unwrap_err();
        match err {
            ProviderError::UnsupportedLanguage(s, t) => {
                assert_eq!(s, "fr");
                assert_eq!(t, "de");
            }
            other => panic!("expected UnsupportedLanguage, got {other:?}"),
        }
    }
}
