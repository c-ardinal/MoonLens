//! Translation routing matrix.
//!
//! The user picks **one** of three Tiers (Compact / Standard /
//! Quality); given that choice plus the (source, target) language
//! pair and the host's hardware bucket, this module returns the
//! exact model — or pair of models, for pivot routes — the tool
//! should run inference on.
//!
//! The matrix is defined in `docs/TRANSLATION_ROUTING_MATRIX.md`
//! and was validated by the Phase 27 benches:
//! - `docs/benchmarks/2026-05-12-phase27-multilang-quality-review.md`
//!   (direct, single-step routes)
//! - `docs/benchmarks/2026-05-12-phase27-pivot-quality-review.md`
//!   (two-stage pivot via English as bridge)
//!
//! Design choices:
//!
//! - `en↔ja` is **always** the JA-bridge model — Phase 27 measured
//!   100% safety on JA-Std+ direct, which beats every multilingual
//!   tier we benched.
//! - `ja↔X` (X∉{en, ja}) splits on hardware:
//!   * **GPU**: two-stage pivot (JA bridge ↔ English ↔ Edge multi-
//!     lingual) maxes quality at ~290 ms wall-clock.
//!   * **CPU**: single-step Edge (2 GB) or Edge+ (4 GB) — pivot's
//!     ~3-5 s latency on CPU outweighs the quality gain.
//! - `en↔X` and `X↔Y` use the Edge / Edge+ direct on both hardware
//!   classes — no JA bridge involvement.
//! - Compact tier has no multilingual partner, so non-en/ja routes
//!   degrade to the JA-Compact model (the only one loaded) and the
//!   UI MUST surface a warning that the result will be low-quality.
//!
//! Tier IDs are imported from `model_registry` as constants so
//! renames there flow through here without grep gymnastics; see
//! `model_registry::router_anchor_ids_are_registered` for the
//! cross-check.

use serde::{Deserialize, Serialize};

use crate::model_registry::{
    COMPACT_JA_TIER_ID, QUALITY_MULTI_TIER_ID, STANDARD_JA_BRIDGE_TIER_ID, STANDARD_MULTI_TIER_ID,
};

/// User-facing memory/quality tier. One of three, surfaced as a
/// single dropdown in the settings UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum UserTier {
    /// Smallest footprint (0.6 GB RAM). EN↔JA only; multilingual
    /// translations degrade silently to the same JA-Compact model
    /// because there is no multilingual partner.
    Compact,
    /// Default (4.2 GB RAM). JA bridge + Edge E2B. Recommended
    /// for typical office laptops.
    #[default]
    Standard,
    /// Maximum quality (6.1 GB RAM). JA bridge + Edge+ E4B.
    /// Recommended for workstations with a dGPU.
    Quality,
}

impl UserTier {
    /// Stable string id the IPC layer round-trips through. Lower-
    /// snake_case matches the `serde(rename_all)` above.
    pub fn as_id(self) -> &'static str {
        match self {
            UserTier::Compact => "compact",
            UserTier::Standard => "standard",
            UserTier::Quality => "quality",
        }
    }

    /// Parse an id back into the enum. Returns `None` for unknown
    /// ids — callers should fall back to `UserTier::Standard`.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "compact" => Some(UserTier::Compact),
            "standard" => Some(UserTier::Standard),
            "quality" => Some(UserTier::Quality),
            _ => None,
        }
    }
}

/// Host hardware bucket the inference runtime will run on. Picked
/// from `services::hardware_probe::HostCapabilities::hw_kind` at
/// runtime; tests can pass either variant directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hardware {
    /// No GPU acceleration available — pure CPU inference. Pivot
    /// (two-stage) routes are avoided here because the second
    /// stage's latency stacks on top of the first.
    Cpu,
    /// At least one of {CUDA, Vulkan, Metal} is available. Pivot
    /// routes are preferred where they yield better quality at
    /// acceptable latency.
    Gpu,
}

/// Output of the router. Either a single tier (direct inference)
/// or two tiers chained through English (pivot inference).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RoutingDecision {
    /// Run the source text directly through one model. The runtime
    /// passes `(src, tgt)` to the named tier's translator and
    /// returns its output verbatim.
    Direct {
        /// Tier id from `model_registry::TIERS`.
        tier_id: &'static str,
    },
    /// Two-stage pivot through English. `stage1_tier` translates
    /// `(src, "en")` and its output is fed to `stage2_tier` as
    /// `("en", tgt)`. The pivot wrapper provider runs the two
    /// stages back-to-back inside a single Translate call.
    Pivot {
        /// First-stage tier id. Translates source → English.
        stage1_tier: &'static str,
        /// Second-stage tier id. Translates English → target.
        stage2_tier: &'static str,
    },
    /// Language pair is outside the matrix for this tier (e.g.
    /// Compact requested X→Y with neither side being ja/en).
    /// Callers MUST surface this as a user-visible error and fall
    /// back to a cloud provider if `allow_cloud` permits.
    Unsupported,
}

/// Normalised language category used by the routing match. We
/// only care about three buckets: Japanese, English, and "other"
/// — the matrix has no per-language nuance beyond those three.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LangBucket {
    Ja,
    En,
    Other,
}

fn classify(lang: &str) -> LangBucket {
    // Strip BCP-47 region suffixes (`ja-JP`, `en-US`) and lower-
    // case before comparing. The IPC layer historically passes
    // both 2-letter ISO codes and full English names; the
    // existing `translate_embedded_llm` already normalises that,
    // and we mirror its acceptance set here.
    let head = lang
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match head.as_str() {
        "ja" | "jpn" | "japanese" => LangBucket::Ja,
        "en" | "eng" | "english" => LangBucket::En,
        _ => LangBucket::Other,
    }
}

/// Resolve an `auto` / empty source-language code into a concrete
/// `en` / `ja` code by inspecting the source TEXT, so the router
/// buckets it correctly instead of dropping it into `Other`.
///
/// Why this matters (2026-06 Lens regression): the Lens / capture
/// flows pass `source_language = "auto"` (the user never picks a
/// source). `classify("auto")` is `Other`, so an English screenshot
/// translated to Japanese took the `Other → ja` GPU **pivot**
/// (stage 1 = Quality multi = Gemma 4 E4B, stage 2 = JA bridge)
/// instead of the `En → Ja` **direct** JA-bridge route. The pivot's
/// stage-1 Gemma 4 model echoes the English source verbatim (it is
/// not a reliable en→en bridge), and the extra stage plus Quality's
/// MBR-N=5 turned one fast bridge call into ~10 slow ones — the
/// "English echo + 25 s" symptom. Detecting `en` up front sends the
/// request straight to the bridge that actually translates it.
///
/// The router only needs En / Ja / Other, so a script heuristic is
/// enough and avoids a detection dependency:
///   - any hiragana / katakana ⇒ `ja` (unambiguously Japanese),
///   - otherwise Latin letters with no other-script letters ⇒ `en`,
///   - anything else ⇒ `None` (keep the caller's original code, so
///     a genuine French/Korean/… source still reaches the multi
///     path rather than being mislabelled English).
pub fn detect_source_lang(text: &str) -> Option<&'static str> {
    let mut kana = 0usize;
    let mut latin = 0usize;
    let mut other_letters = 0usize;
    for ch in text.chars() {
        let u = ch as u32;
        let is_kana = (0x3040..=0x309F).contains(&u) // hiragana
            || (0x30A0..=0x30FF).contains(&u)         // katakana
            || (0xFF66..=0xFF9D).contains(&u); // halfwidth katakana
        if is_kana {
            kana += 1;
        } else if ch.is_ascii_alphabetic() {
            latin += 1;
        } else if ch.is_alphabetic() {
            other_letters += 1;
        }
    }
    // Decide by the PREDOMINANT script, not mere presence — an OCR
    // line that merges an English title with a short Japanese caption
    // (e.g. "The Elicitation Paragraph … により使われている文。") is
    // mostly English and should translate as `en`, not collapse to a
    // same-language no-op because a few kana appeared. Kana-dominant
    // text is `ja`; Latin-dominant text with no third-script letters
    // is `en`; genuinely mixed/other stays `None` so the caller keeps
    // its original code.
    if kana == 0 && latin == 0 {
        return None;
    }
    if kana > latin {
        Some("ja")
    } else if latin > 0 && other_letters == 0 {
        Some("en")
    } else {
        None
    }
}

/// True when `code` is the "let the system decide" sentinel — empty
/// or `auto` (case-insensitive). Source language defaults to this in
/// the Lens / capture flows.
pub fn is_auto_source(code: &str) -> bool {
    let t = code.trim();
    t.is_empty() || t.eq_ignore_ascii_case("auto")
}

/// Resolve the matrix entry for one translation request.
///
/// See `docs/TRANSLATION_ROUTING_MATRIX.md` for the full table
/// and rationale. The router is a pure function — no allocation,
/// no global state — so it is safe to call from any thread and
/// any async context.
pub fn route(user_tier: UserTier, hw: Hardware, src: &str, tgt: &str) -> RoutingDecision {
    // Degenerate "translate into the same language" case. The
    // pipeline upstream rejects this, but the router stays
    // defensive: pick the cheapest model for the tier so the
    // caller does not crash mid-stream. Comparison MUST happen on
    // the raw strings here — collapsing through `LangBucket` would
    // wrongly treat e.g. fr↔de as same-language because both
    // classify as `Other`.
    if src.eq_ignore_ascii_case(tgt) {
        return cheapest_for(user_tier);
    }
    let src_b = classify(src);
    let tgt_b = classify(tgt);
    route_buckets(user_tier, hw, src_b, tgt_b)
}

fn route_buckets(
    tier: UserTier,
    hw: Hardware,
    src: LangBucket,
    tgt: LangBucket,
) -> RoutingDecision {
    use Hardware::*;
    use LangBucket::*;
    use UserTier::*;

    // en↔ja: one route across every tier × hardware combo.
    if matches!((src, tgt), (En, Ja) | (Ja, En)) {
        return match tier {
            Compact => RoutingDecision::Direct {
                tier_id: COMPACT_JA_TIER_ID,
            },
            Standard => RoutingDecision::Direct {
                tier_id: STANDARD_JA_BRIDGE_TIER_ID,
            },
            // Quality en→ja: the larger Gemma 4 E4B (4B) measurably
            // beats the 1.8B JA bridge on accuracy now that the Gemma 4
            // chat template is fixed — 2026-06 lens_eval evidence on
            // the Stella fixture: "fresh snow peas" → 新鮮なスナップエンドウ
            // (Gemma 4) vs 新なスネイパー (bridge), "Please call Stella"
            // → Stellaさんに電話してください vs ステラに連して (bridge
            // drops the 絡 in 連絡). ja→en keeps the JA-specialised
            // bridge, which handles Japanese SOURCE best (Phase 27).
            Quality if matches!(tgt, Ja) => RoutingDecision::Direct {
                tier_id: QUALITY_MULTI_TIER_ID,
            },
            Quality => RoutingDecision::Direct {
                tier_id: STANDARD_JA_BRIDGE_TIER_ID,
            },
        };
    }

    // Compact has no multilingual partner — every non-en/ja
    // route degrades to the JA-Compact model. The matrix doc
    // marks these as warnings ("品質低い"); the UI is expected
    // to surface that, but the router itself still returns a
    // working decision rather than `Unsupported`.
    if matches!(tier, Compact) {
        if matches!(src, Ja) || matches!(tgt, Ja) || matches!(src, En) || matches!(tgt, En) {
            return RoutingDecision::Direct {
                tier_id: COMPACT_JA_TIER_ID,
            };
        }
        // X↔Y with neither side ja/en is genuinely unsupported.
        return RoutingDecision::Unsupported;
    }

    // Standard / Quality from here on. Pick the tier's multi-
    // lingual model up front so the match arms below stay
    // readable.
    let multi = match tier {
        Standard => STANDARD_MULTI_TIER_ID,
        Quality => QUALITY_MULTI_TIER_ID,
        Compact => unreachable!("Compact handled above"),
    };

    // ja→X (X != en): GPU pivots, CPU goes direct multi.
    if matches!(src, Ja) && matches!(tgt, Other) {
        return match hw {
            Cpu => RoutingDecision::Direct { tier_id: multi },
            Gpu => RoutingDecision::Pivot {
                stage1_tier: STANDARD_JA_BRIDGE_TIER_ID,
                stage2_tier: multi,
            },
        };
    }

    // X→ja (X != en): GPU pivots, CPU goes direct multi.
    if matches!(tgt, Ja) && matches!(src, Other) {
        return match hw {
            Cpu => RoutingDecision::Direct { tier_id: multi },
            Gpu => RoutingDecision::Pivot {
                stage1_tier: multi,
                stage2_tier: STANDARD_JA_BRIDGE_TIER_ID,
            },
        };
    }

    // en→X or X→en: direct multi on either hardware. No JA
    // bridge involvement because the JA bridge's non-en/ja
    // quality is the whole reason we route around it.
    if matches!(src, En) || matches!(tgt, En) {
        return RoutingDecision::Direct { tier_id: multi };
    }

    // X→Y with neither side being ja or en. Standard/Quality
    // CPU stays single-step (no measured upside to pivoting
    // through a language neither end speaks natively); GPU does
    // pivot through English because the Phase 27 multilingual
    // bench confirmed Edge / Edge+ produces clean English from
    // every tested source, and clean English fed back through
    // Edge / Edge+ produces clean target.
    match hw {
        Cpu => RoutingDecision::Direct { tier_id: multi },
        Gpu => RoutingDecision::Pivot {
            stage1_tier: multi,
            stage2_tier: multi,
        },
    }
}

fn cheapest_for(tier: UserTier) -> RoutingDecision {
    let id = match tier {
        UserTier::Compact => COMPACT_JA_TIER_ID,
        UserTier::Standard => STANDARD_JA_BRIDGE_TIER_ID,
        UserTier::Quality => STANDARD_JA_BRIDGE_TIER_ID,
    };
    RoutingDecision::Direct { tier_id: id }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_registry::tier_by_id;

    // ----- helpers -------------------------------------------------

    fn expect_direct(d: &RoutingDecision, expect_id: &str) {
        match d {
            RoutingDecision::Direct { tier_id } => assert_eq!(*tier_id, expect_id),
            other => panic!("expected Direct({expect_id}), got {other:?}"),
        }
    }

    fn expect_pivot(d: &RoutingDecision, stage1: &str, stage2: &str) {
        match d {
            RoutingDecision::Pivot {
                stage1_tier,
                stage2_tier,
            } => {
                assert_eq!(*stage1_tier, stage1, "stage1 mismatch");
                assert_eq!(*stage2_tier, stage2, "stage2 mismatch");
            }
            other => panic!("expected Pivot({stage1} -> {stage2}), got {other:?}"),
        }
    }

    fn expect_unsupported(d: &RoutingDecision) {
        match d {
            RoutingDecision::Unsupported => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    // ----- en↔ja: identical across hw & non-compact tiers ----------

    #[test]
    fn en_ja_compact_uses_ja_compact() {
        for hw in [Hardware::Cpu, Hardware::Gpu] {
            expect_direct(
                &route(UserTier::Compact, hw, "en", "ja"),
                COMPACT_JA_TIER_ID,
            );
            expect_direct(
                &route(UserTier::Compact, hw, "ja", "en"),
                COMPACT_JA_TIER_ID,
            );
        }
    }

    #[test]
    fn en_ja_standard_uses_ja_bridge_both_directions() {
        for hw in [Hardware::Cpu, Hardware::Gpu] {
            expect_direct(
                &route(UserTier::Standard, hw, "en", "ja"),
                STANDARD_JA_BRIDGE_TIER_ID,
            );
            expect_direct(
                &route(UserTier::Standard, hw, "ja", "en"),
                STANDARD_JA_BRIDGE_TIER_ID,
            );
        }
    }

    #[test]
    fn quality_en_to_ja_uses_multi_tier_ja_to_en_uses_bridge() {
        // Quality en→ja routes to the larger Gemma 4 E4B for accuracy;
        // ja→en keeps the JA-specialised bridge (best on JA source).
        for hw in [Hardware::Cpu, Hardware::Gpu] {
            expect_direct(
                &route(UserTier::Quality, hw, "en", "ja"),
                QUALITY_MULTI_TIER_ID,
            );
            expect_direct(
                &route(UserTier::Quality, hw, "ja", "en"),
                STANDARD_JA_BRIDGE_TIER_ID,
            );
        }
    }

    // ----- ja↔X: cpu direct, gpu pivot ----------------------------

    #[test]
    fn standard_ja_to_x_cpu_uses_e2b_direct() {
        expect_direct(
            &route(UserTier::Standard, Hardware::Cpu, "ja", "zh-CN"),
            STANDARD_MULTI_TIER_ID,
        );
    }

    #[test]
    fn standard_ja_to_x_gpu_pivots_through_ja_bridge_and_e2b() {
        expect_pivot(
            &route(UserTier::Standard, Hardware::Gpu, "ja", "zh-CN"),
            STANDARD_JA_BRIDGE_TIER_ID,
            STANDARD_MULTI_TIER_ID,
        );
    }

    #[test]
    fn quality_ja_to_x_gpu_pivots_through_ja_bridge_and_e4b() {
        expect_pivot(
            &route(UserTier::Quality, Hardware::Gpu, "ja", "fr"),
            STANDARD_JA_BRIDGE_TIER_ID,
            QUALITY_MULTI_TIER_ID,
        );
    }

    #[test]
    fn quality_x_to_ja_gpu_pivots_through_e4b_and_ja_bridge() {
        // Reverse direction must reverse the pivot ordering.
        expect_pivot(
            &route(UserTier::Quality, Hardware::Gpu, "fr", "ja"),
            QUALITY_MULTI_TIER_ID,
            STANDARD_JA_BRIDGE_TIER_ID,
        );
    }

    #[test]
    fn standard_x_to_ja_cpu_uses_e2b_direct() {
        expect_direct(
            &route(UserTier::Standard, Hardware::Cpu, "ko", "ja"),
            STANDARD_MULTI_TIER_ID,
        );
    }

    // ----- en↔X: direct multi on both hardware ---------------------

    #[test]
    fn standard_en_to_x_uses_e2b_direct_on_both_hardware() {
        for hw in [Hardware::Cpu, Hardware::Gpu] {
            expect_direct(
                &route(UserTier::Standard, hw, "en", "de"),
                STANDARD_MULTI_TIER_ID,
            );
            expect_direct(
                &route(UserTier::Standard, hw, "de", "en"),
                STANDARD_MULTI_TIER_ID,
            );
        }
    }

    #[test]
    fn quality_en_to_x_uses_e4b_direct_on_both_hardware() {
        for hw in [Hardware::Cpu, Hardware::Gpu] {
            expect_direct(
                &route(UserTier::Quality, hw, "en", "ko"),
                QUALITY_MULTI_TIER_ID,
            );
        }
    }

    // ----- X→Y (neither side ja/en) --------------------------------

    #[test]
    fn standard_x_to_y_cpu_uses_e2b_direct() {
        expect_direct(
            &route(UserTier::Standard, Hardware::Cpu, "fr", "de"),
            STANDARD_MULTI_TIER_ID,
        );
    }

    #[test]
    fn quality_x_to_y_gpu_pivots_through_e4b_twice() {
        expect_pivot(
            &route(UserTier::Quality, Hardware::Gpu, "ko", "vi"),
            QUALITY_MULTI_TIER_ID,
            QUALITY_MULTI_TIER_ID,
        );
    }

    // ----- Compact unsupported / degraded --------------------------

    #[test]
    fn compact_x_to_y_is_unsupported() {
        // Compact has only JA-Compact loaded; X↔Y with neither
        // side being ja/en is the one input shape with no
        // working fallback.
        for hw in [Hardware::Cpu, Hardware::Gpu] {
            expect_unsupported(&route(UserTier::Compact, hw, "fr", "de"));
        }
    }

    #[test]
    fn compact_x_to_ja_degrades_to_compact_ja() {
        // Documented "low quality" path. The router still hands
        // back a working decision; the UI is responsible for
        // surfacing the quality warning.
        for hw in [Hardware::Cpu, Hardware::Gpu] {
            expect_direct(
                &route(UserTier::Compact, hw, "zh-CN", "ja"),
                COMPACT_JA_TIER_ID,
            );
            expect_direct(
                &route(UserTier::Compact, hw, "ja", "zh-CN"),
                COMPACT_JA_TIER_ID,
            );
        }
    }

    #[test]
    fn compact_en_to_x_degrades_to_compact_ja() {
        // en involvement still goes through JA-Compact in
        // Compact tier — that's the only loaded model.
        for hw in [Hardware::Cpu, Hardware::Gpu] {
            expect_direct(
                &route(UserTier::Compact, hw, "en", "de"),
                COMPACT_JA_TIER_ID,
            );
        }
    }

    // ----- language normalisation ----------------------------------

    #[test]
    fn classify_recognises_iso_and_region_and_english_name_aliases() {
        // The IPC historically passes both ISO 2-letter and full
        // English names. Routing must converge to the same
        // decision regardless of which form the upstream chose.
        let decisions = [
            route(UserTier::Standard, Hardware::Gpu, "ja", "fr"),
            route(UserTier::Standard, Hardware::Gpu, "ja-JP", "fr-FR"),
            route(UserTier::Standard, Hardware::Gpu, "Japanese", "fr"),
            route(UserTier::Standard, Hardware::Gpu, "JA", "Fr"),
        ];
        for d in &decisions[1..] {
            assert_eq!(d, &decisions[0]);
        }
    }

    #[test]
    fn classify_treats_unknown_language_as_other() {
        // An unrecognised code should not crash the router or
        // collapse into ja/en — it falls into the "Other" bucket
        // and follows the same matrix as known non-en/ja codes.
        // Picked "xx" (the reserved BCP-47 private-use range
        // bypass) so the test does not depend on any specific
        // real language being absent from the classifier.
        let d = route(UserTier::Standard, Hardware::Gpu, "xx", "ja");
        expect_pivot(&d, STANDARD_MULTI_TIER_ID, STANDARD_JA_BRIDGE_TIER_ID);
    }

    // ----- defensive cases -----------------------------------------

    #[test]
    fn same_language_pair_returns_cheap_direct() {
        // The upstream pipeline rejects src==tgt before reaching
        // the router, but if it ever slips through, the router
        // returns the cheapest direct entry for the tier rather
        // than crashing or returning Unsupported.
        expect_direct(
            &route(UserTier::Standard, Hardware::Gpu, "ja", "ja"),
            STANDARD_JA_BRIDGE_TIER_ID,
        );
        expect_direct(
            &route(UserTier::Compact, Hardware::Cpu, "en", "en"),
            COMPACT_JA_TIER_ID,
        );
    }

    #[test]
    fn every_returned_tier_id_is_registered() {
        // Coverage sweep: enumerate every (tier, hw, src, tgt)
        // shape the router cares about and confirm every model
        // id it returns lives in TIERS. Catches a future PR that
        // adds a new pair shape but forgets to register the
        // multilingual partner.
        let langs = ["ja", "en", "fr", "ko"];
        for &tier in &[UserTier::Compact, UserTier::Standard, UserTier::Quality] {
            for &hw in &[Hardware::Cpu, Hardware::Gpu] {
                for &src in &langs {
                    for &tgt in &langs {
                        match route(tier, hw, src, tgt) {
                            RoutingDecision::Direct { tier_id } => {
                                assert!(
                                    tier_by_id(tier_id).is_some(),
                                    "Direct({tier_id}) not in TIERS for {tier:?}/{hw:?} {src}->{tgt}"
                                );
                            }
                            RoutingDecision::Pivot {
                                stage1_tier,
                                stage2_tier,
                            } => {
                                assert!(tier_by_id(stage1_tier).is_some());
                                assert!(tier_by_id(stage2_tier).is_some());
                            }
                            RoutingDecision::Unsupported => {}
                        }
                    }
                }
            }
        }
    }

    // ----- user tier id round-trip ---------------------------------

    #[test]
    fn user_tier_id_round_trips() {
        for t in [UserTier::Compact, UserTier::Standard, UserTier::Quality] {
            assert_eq!(UserTier::from_id(t.as_id()), Some(t));
        }
        assert!(UserTier::from_id("nope").is_none());
        // Empty string must not match either — the IPC sends a
        // string by serde, and a typo would otherwise silently
        // route to Compact (the alphabetical-first variant) if
        // we did not assert explicit None here.
        assert!(UserTier::from_id("").is_none());
    }

    #[test]
    fn user_tier_default_is_standard() {
        assert_eq!(UserTier::default(), UserTier::Standard);
    }
}
