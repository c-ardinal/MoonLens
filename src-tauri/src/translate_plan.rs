//! Splitting a text-heavy capture into translation requests.
//!
//! A local model reads the whole prompt before it writes the first
//! token, so translating a full screen in one request keeps the user
//! waiting for every paragraph to be read first. Instead the first
//! request holds only the first few blocks and later requests are
//! larger. Each request carries the earlier parts and their
//! translations as history (see [`crate::providers::TranslateContext`]),
//! so later parts are translated with the same context as before, and
//! the engine reuses the previous request's KV cache for that history.

use std::ops::Range;

/// Size of the first request, in [`cost`] units (about four units per
/// English word, three per CJK character). Small enough that the first
/// translation starts almost at once, large enough to hold a heading
/// and its first sentence.
pub const FIRST_PART: usize = 160;
/// Size of every later request. Larger parts mean fewer requests while
/// the user is already reading.
pub const LATER_PART: usize = 600;
/// History kept in a request, in [`cost`] units. Older pairs are dropped
/// first when a capture is longer than this.
pub const HISTORY: usize = 6000;
/// Reference text given with a short first part, in [`cost`] units.
pub const REFERENCE: usize = 200;

/// Rough prompt cost of `s`: ASCII characters count 1, others 3, which
/// tracks token counts across Latin and CJK scripts closely enough to
/// size requests.
pub fn cost(s: &str) -> usize {
    s.chars().map(|c| if c.is_ascii() { 1 } else { 3 }).sum()
}

/// Group consecutive items (block costs, in reading order) into
/// requests: the first up to `first`, the rest up to `later`. A block is
/// never split, so a block larger than the limit gets a request of its
/// own.
pub fn plan(costs: &[usize], first: usize, later: usize) -> Vec<Range<usize>> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut size = 0;
    let mut limit = first;
    for (i, &c) in costs.iter().enumerate() {
        if i > start && size + c > limit {
            parts.push(start..i);
            start = i;
            size = 0;
            limit = later;
        }
        size += c;
    }
    if start < costs.len() {
        parts.push(start..costs.len());
    }
    parts
}

/// Drop the oldest pairs until the history fits in `budget`.
pub fn trim_history(history: &mut Vec<(String, String)>, budget: usize) {
    let mut total: usize = history.iter().map(|(s, t)| cost(s) + cost(t)).sum();
    let mut drop = 0;
    while total > budget && drop < history.len() {
        total -= cost(&history[drop].0) + cost(&history[drop].1);
        drop += 1;
    }
    history.drain(..drop);
}

/// Up to `budget` of `texts`, joined by `sep`, cut at a text boundary
/// (the first text is cut at a character if it alone is too long).
pub fn take_reference<'a>(
    texts: impl IntoIterator<Item = &'a str>,
    sep: &str,
    budget: usize,
) -> Option<String> {
    let mut out = String::new();
    let mut used = 0;
    for t in texts {
        let c = cost(t) + if out.is_empty() { 0 } else { cost(sep) };
        if used + c > budget {
            if out.is_empty() {
                let mut acc = 0;
                out = t
                    .chars()
                    .take_while(|ch| {
                        acc += if ch.is_ascii() { 1 } else { 3 };
                        acc <= budget
                    })
                    .collect();
            }
            break;
        }
        if !out.is_empty() {
            out.push_str(sep);
        }
        out.push_str(t);
        used += c;
    }
    (!out.trim().is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_weights_cjk_higher() {
        assert_eq!(cost("Save"), 4);
        assert_eq!(cost("保存"), 6);
    }

    #[test]
    fn first_part_is_small_and_later_parts_larger() {
        let costs = [40, 40, 100, 100, 100, 100, 100, 100, 100, 100];
        assert_eq!(plan(&costs, 160, 400), vec![0..2, 2..6, 6..10]);
    }

    #[test]
    fn oversized_blocks_get_their_own_part() {
        assert_eq!(
            plan(&[500, 10, 900, 10], 160, 400),
            vec![0..1, 1..2, 2..3, 3..4]
        );
        assert_eq!(plan(&[10], 160, 400), vec![0..1]);
        assert!(plan(&[], 160, 400).is_empty());
    }

    #[test]
    fn short_capture_stays_one_request() {
        assert_eq!(plan(&[20, 30, 40], 160, 600), vec![0..3]);
    }

    #[test]
    fn history_drops_oldest_pairs() {
        let mut h = vec![
            ("a".repeat(50), "b".repeat(50)),
            ("c".repeat(50), "d".repeat(50)),
        ];
        trim_history(&mut h, 150);
        assert_eq!(h.len(), 1);
        assert!(h[0].0.starts_with('c'));
        trim_history(&mut h, 0);
        assert!(h.is_empty());
    }

    #[test]
    fn reference_stops_at_block_boundary() {
        assert_eq!(
            take_reference(["Light", "Dark", "System"], "\n\n", 13).as_deref(),
            Some("Light\n\nDark")
        );
        assert_eq!(
            take_reference(["abcdef"], "\n\n", 3).as_deref(),
            Some("abc")
        );
        assert_eq!(take_reference([], "\n\n", 100), None);
    }
}
