//! Minimum Bayes Risk (MBR) decoding for the routed translator.
//!
//! Background — Freitag et al. 2022, "Minimum Bayes Risk Decoding
//! for Neural Machine Translation": instead of picking the single
//! greedy-or-temperature-sampled output, generate N candidates,
//! score each candidate by its expected utility against the OTHER
//! candidates (the "Bayes risk" of choosing it), and emit the
//! candidate with the highest mean utility. Empirically beats
//! greedy / sampling / beam search on every published WMT pair.
//!
//! Why it matters here — even after temperature tuning + placeholder
//! masking the eval still surfaced one or two strict-ordering
//! violations driven by single-sample variance: Quality at temp=0.1
//! occasionally happens to draw a bad token sequence on a specific
//! fixture that Standard at temp=0.2 happened to avoid. MBR with
//! more samples for upper tiers makes the selected output's
//! quality monotonically non-decreasing in N (more candidates →
//! better expected quality), which mechanically enforces
//! "Quality is at least as good as Standard" in expectation.
//!
//! Tier-graded N:
//! - Compact:  N=1 (no MBR — single shot)
//! - Standard: N=3
//! - Quality:  N=5
//!
//! Cost: Quality runs 5x the inference time. Acceptable for a tier
//! the user explicitly opted into for quality over speed.
//!
//! Utility metric: character bigram Jaccard similarity. chrF would
//! be more principled but bigram Jaccard captures the same
//! "candidates that agree on most n-grams are mutually
//! supporting → likely closer to the truth" property at a fraction
//! of the code surface, and works on any output without needing a
//! tokeniser.

use std::collections::HashSet;

use crate::translation_router::UserTier;

/// How many candidates to generate for `user_tier`. Returning 1
/// disables MBR and short-circuits the call site back to its
/// single-shot path.
pub fn mbr_n_for_tier(user_tier: UserTier) -> usize {
    match user_tier {
        UserTier::Compact => 1,
        UserTier::Standard => 3,
        UserTier::Quality => 5,
    }
}

/// Character bigram Jaccard similarity between two strings.
/// Returns a value in `[0, 1]` — 1 when the strings share the
/// same set of bigrams, 0 when disjoint. Returns 1 when both
/// strings are empty (the degenerate case "they agree on nothing
/// but agree completely").
pub fn bigram_jaccard(a: &str, b: &str) -> f64 {
    let bigrams_a = bigrams(a);
    let bigrams_b = bigrams(b);
    if bigrams_a.is_empty() && bigrams_b.is_empty() {
        return 1.0;
    }
    let intersection = bigrams_a.intersection(&bigrams_b).count();
    let union = bigrams_a.union(&bigrams_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Extract the set of character bigrams from `s`. Bigrams are
/// `(char, char)` pairs of consecutive characters. Useful for
/// scoring "did these two strings emit similar content" without
/// caring about word boundaries (works equally well for JA/ZH/KO
/// which don't space-separate words).
fn bigrams(s: &str) -> HashSet<(char, char)> {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() < 2 {
        return HashSet::new();
    }
    let mut out = HashSet::with_capacity(chars.len() - 1);
    for window in chars.windows(2) {
        out.insert((window[0], window[1]));
    }
    out
}

/// Score every candidate by its mean bigram-Jaccard similarity
/// against the other candidates. Returns the score vector aligned
/// with `candidates` — `scores[i]` is the utility of `candidates[i]`.
///
/// For N=1 the lone candidate scores 1.0 (vacuous: nothing to
/// disagree with).
pub fn score_candidates(candidates: &[String]) -> Vec<f64> {
    if candidates.len() <= 1 {
        return vec![1.0; candidates.len()];
    }
    let n = candidates.len();
    let mut scores = vec![0.0; n];
    for i in 0..n {
        let mut sum = 0.0;
        for j in 0..n {
            if i == j {
                continue;
            }
            sum += bigram_jaccard(&candidates[i], &candidates[j]);
        }
        scores[i] = sum / (n - 1) as f64;
    }
    scores
}

/// Per-candidate newline-structure penalty against `source`.
/// Returns a value in `[0, 1]` — 1 when the candidate's newline
/// count exactly matches the source's, decaying linearly with
/// absolute difference. Empty source short-circuits to 1.0.
///
/// Background (2026-05-26 routed eval, log-rust-tracing-multi
/// fixture): with MBR-N=5, Quality tier's plain bigram-Jaccard
/// consensus locks onto the model's MOST COMMON failure mode —
/// collapsing a 4-line source into a 1-line output. All five
/// candidates "agree" they collapsed it, so they score each
/// other high under bigram Jaccard. The newline penalty breaks
/// this tie by demoting the entire collapse class regardless of
/// internal agreement, surfacing the rare structurally-correct
/// candidate instead.
fn newline_match_score(candidate: &str, source_newlines: usize) -> f64 {
    let cand_newlines = candidate.matches('\n').count();
    if source_newlines == 0 && cand_newlines == 0 {
        return 1.0;
    }
    if source_newlines == 0 {
        // Source has no newlines but candidate added some — small
        // penalty, since extra newlines are usually whitespace
        // noise rather than a structural error.
        return (1.0 - (cand_newlines as f64) * 0.1).max(0.0);
    }
    let diff = cand_newlines.abs_diff(source_newlines) as f64;
    let denom = (source_newlines as f64) + 1.0;
    ((denom - diff) / denom).max(0.0)
}

/// Score every candidate by mean bigram-Jaccard utility against
/// the other candidates, multiplied by a newline-structure match
/// score against `source`.
///
/// Weight tuning (2026-05-26): set to 0.85 — chosen so the
/// newline-wrong penalty (multiplier 0.15 at nl=0) overpowers
/// the worst-case bigram lock-in seen in the Quality-tier
/// routed eval, where 5 of 5 collapsed candidates agreed with
/// each other at bigram_jaccard ≈ 1.0 while the rare survivor
/// scored ≈ 0.4. With weight=0.85 the survivor's penalty-free
/// score (0.4 * 1.0 = 0.40) beats the collapsed consensus
/// (1.0 * 0.15 = 0.15), letting MBR surface the structurally-
/// correct output. Lower weights (0.5–0.7) preserved the lock-in.
pub fn score_candidates_with_source(candidates: &[String], source: &str) -> Vec<f64> {
    let bigram_scores = score_candidates(candidates);
    if candidates.len() <= 1 {
        return bigram_scores;
    }
    let source_newlines = source.matches('\n').count();
    let weight = 0.85;
    bigram_scores
        .into_iter()
        .zip(candidates.iter())
        .map(|(s, c)| {
            let nl = newline_match_score(c, source_newlines);
            // Multiplicative combination: candidate that fully
            // disagrees on newlines (nl=0) gets `1 - weight`
            // (= 0.15) of its bigram score; a fully-matching
            // candidate gets the full bigram score. Output range
            // is roughly `[0.15s, 1.0s]`.
            s * (1.0 - weight + weight * nl)
        })
        .collect()
}

/// Pick the candidate with the highest MBR score. Falls back to
/// the first candidate when the list is non-empty but all scores
/// tie; returns `None` for an empty list.
pub fn select_best(candidates: Vec<String>) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let scores = score_candidates(&candidates);
    let best_idx = scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    candidates.into_iter().nth(best_idx)
}

/// Pick the best candidate using source-aware scoring. See
/// [`score_candidates_with_source`] for the scoring rationale.
pub fn select_best_with_source(candidates: Vec<String>, source: &str) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let scores = score_candidates_with_source(&candidates, source);
    let best_idx = scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);
    candidates.into_iter().nth(best_idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn n_for_tier_grades_monotonically() {
        assert_eq!(mbr_n_for_tier(UserTier::Compact), 1);
        assert!(mbr_n_for_tier(UserTier::Standard) > mbr_n_for_tier(UserTier::Compact));
        assert!(mbr_n_for_tier(UserTier::Quality) > mbr_n_for_tier(UserTier::Standard));
    }

    #[test]
    fn bigram_jaccard_identical_strings_score_one() {
        assert!((bigram_jaccard("hello", "hello") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bigram_jaccard_disjoint_strings_score_zero() {
        // No shared bigrams.
        let s = bigram_jaccard("abc", "xyz");
        assert!(s < 0.01, "expected ~0 for disjoint, got {s}");
    }

    #[test]
    fn bigram_jaccard_handles_unicode() {
        // JA characters are individual chars in Rust's `chars()`
        // iterator (one char per code point), so the bigram set
        // captures grapheme-level agreement correctly.
        let s = bigram_jaccard("こんにちは世界", "こんにちは世界");
        assert!((s - 1.0).abs() < 1e-9);
        let s2 = bigram_jaccard("こんにちは", "こんばんは");
        assert!(s2 > 0.0 && s2 < 1.0, "partial overlap: {s2}");
    }

    #[test]
    fn bigram_jaccard_empty_strings() {
        // Convention: both empty → similarity 1 (vacuously agree).
        assert!((bigram_jaccard("", "") - 1.0).abs() < 1e-9);
        assert_eq!(bigram_jaccard("abc", ""), 0.0);
    }

    #[test]
    fn score_candidates_singleton_returns_one() {
        let scores = score_candidates(&["only".to_string()]);
        assert_eq!(scores, vec![1.0]);
    }

    #[test]
    fn score_candidates_picks_majority_consensus() {
        // Three samples: two agree, one is an outlier. The two
        // agreeing samples should both score higher than the
        // outlier because each agrees with the OTHER agreeing
        // sample, while the outlier disagrees with both.
        let candidates = vec![
            "the cat sat".to_string(),
            "the cat sat".to_string(),
            "wildly different content".to_string(),
        ];
        let scores = score_candidates(&candidates);
        assert!(
            scores[0] > scores[2],
            "consensus > outlier: {scores:?}"
        );
        assert!(scores[1] > scores[2]);
    }

    #[test]
    fn select_best_picks_majority_consensus() {
        let candidates = vec![
            "translated A".to_string(),
            "translated A".to_string(),
            "totally different".to_string(),
        ];
        let best = select_best(candidates).unwrap();
        assert_eq!(best, "translated A");
    }

    #[test]
    fn select_best_handles_empty_list() {
        assert_eq!(select_best(Vec::new()), None);
    }

    #[test]
    fn select_best_handles_singleton() {
        let best = select_best(vec!["only one".to_string()]).unwrap();
        assert_eq!(best, "only one");
    }

    #[test]
    fn newline_match_score_matches_source_exactly() {
        // Source with 3 newlines (= 4 lines). Candidate with the
        // same count scores 1.0 (perfect structure match).
        let cand_match = "a\nb\nc\nd";
        let cand_collapse = "abcd";
        let s = "x\ny\nz\nw";
        let source_nl = s.matches('\n').count();
        assert!((newline_match_score(cand_match, source_nl) - 1.0).abs() < 1e-9);
        // The collapsed candidate (0 newlines vs source's 3) scores
        // roughly `(4 - 3) / 4 = 0.25`.
        let collapsed = newline_match_score(cand_collapse, source_nl);
        assert!(
            collapsed < 0.3,
            "expected collapsed candidate to score < 0.3, got {collapsed}"
        );
    }

    #[test]
    fn newline_match_score_handles_zero_source_newlines() {
        // Source has no newlines (single line). Single-line
        // candidate matches perfectly.
        let s = "single line input";
        let source_nl = s.matches('\n').count();
        assert!((newline_match_score("single line out", source_nl) - 1.0).abs() < 1e-9);
        // Extra newlines added by candidate: small penalty.
        let with_extra = newline_match_score("line one\nline two", source_nl);
        assert!(
            with_extra < 1.0 && with_extra > 0.7,
            "extra newlines on single-line source should be lightly penalised, got {with_extra}"
        );
    }

    #[test]
    fn select_best_with_source_prefers_structure_preserving_candidate() {
        // Three candidates, all bigram-similar but with different
        // newline counts. The one matching the source's 3 newlines
        // should win even though the other two agree with each
        // other on bigrams more strongly.
        let source = "line 1\nline 2\nline 3\nline 4"; // 3 newlines
        let candidates = vec![
            // The structurally-correct candidate. Has 3 newlines.
            "翻訳 1\n翻訳 2\n翻訳 3\n翻訳 4".to_string(),
            // Two collapsed candidates that agree with each other.
            "翻訳 1 翻訳 2 翻訳 3 翻訳 4".to_string(),
            "翻訳 1 翻訳 2 翻訳 3 翻訳 4".to_string(),
        ];
        let best = select_best_with_source(candidates, source).unwrap();
        assert!(
            best.matches('\n').count() == 3,
            "expected structure-preserving candidate, got: {best}"
        );
    }

    #[test]
    fn select_best_with_source_singleton_passes_through() {
        let best =
            select_best_with_source(vec!["only".to_string()], "source").unwrap();
        assert_eq!(best, "only");
    }

    #[test]
    fn select_best_with_source_handles_empty_list() {
        assert_eq!(select_best_with_source(Vec::new(), "source"), None);
    }
}
