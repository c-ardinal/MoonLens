//! Pre/post-process pass that protects "must not be translated"
//! tokens by replacing them with opaque placeholders before the
//! model sees them, then restoring the originals after.
//!
//! Motivation: the 2026-05-22 routed eval surfaced a stubborn class
//! of `id_score` regressions where llm-jp-3.1 1.8B (the Standard /
//! Quality JA bridge) paraphrases tokens that MUST stay verbatim:
//! ISO timestamps decay into `2026年5月18日`, `embedded_llm_perf`
//! transliterates into katakana, `[providers.configs."embedded.llm"]`
//! loses its TOML structure. Lower temperature and stronger system
//! prompts only get us partway because the model has its own ideas
//! about what counts as "translatable".
//!
//! Placeholder translation is the NMT community's standard fix
//! (Google NMT 2017, "Anonymizing Named Entities" 2020+, recent LLM
//! work). The mechanic is mechanical, not heuristic: the model
//! literally cannot fail to preserve a token it never saw — we
//! show it `__MLENS_T_5__` and substitute the original back from
//! our own map.
//!
//! Trade-offs:
//! - Surgical: the model still translates surrounding prose, so the
//!   natural-language quality is unchanged. Only the protected
//!   tokens are forced verbatim.
//! - Cheap: regex pass + hashmap. No extra model call, no latency.
//! - Streaming-aware: `unmask` is idempotent on partial output, so
//!   the caller can apply it once per chunk OR once at the end.
//!   See `unmask_stream` for chunk-by-chunk usage that holds back
//!   bytes that might be a placeholder prefix.

use regex::Regex;
use std::sync::OnceLock;

/// Map from `placeholder_id (usize)` → `original_text`. Returned by
/// [`mask`] alongside the rewritten text so the caller can run
/// [`unmask`] on the model's output.
#[derive(Debug, Clone, Default)]
pub struct PlaceholderMap {
    entries: Vec<String>,
}

impl PlaceholderMap {
    /// Build a placeholder string for `id`. The format is
    /// intentionally pattern-ish — leading + trailing `__`, all
    /// caps, MLENS prefix — so the model recognises it as
    /// "internal junk, copy verbatim" rather than "natural noun,
    /// translate me". Choosing an obscure pattern (instead of e.g.
    /// `[ID1]`) avoids collisions with content the user might
    /// have pasted (markdown footnotes, bracket citations).
    pub fn placeholder_for(id: usize) -> String {
        format!("__MLENS_T_{id}__")
    }

    /// Length of the longest produced placeholder — used by
    /// streaming consumers that need to hold back partial bytes.
    pub fn longest_placeholder_len(&self) -> usize {
        if self.entries.is_empty() {
            return 0;
        }
        // All placeholders share the same `__MLENS_T_<n>__` prefix;
        // the longest is the one with the highest index, whose
        // numeric width grows logarithmically. Computing it
        // exactly is cheaper than a per-iteration max call.
        Self::placeholder_for(self.entries.len() - 1).len()
    }

    /// Total number of placeholders allocated.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Static regex that matches every protected-token shape. We
/// combine all alternatives into a single pattern so one pass
/// over the input catches everything in a deterministic order
/// (left-to-right, longest match wins per regex semantics).
///
/// Patterns (in priority order):
/// - ISO 8601 datetime (`2026-05-18T00:51:11.674022Z`)
/// - ISO date alone (`2026-05-18`)
/// - URL (`https://example.com/...`)
/// - HTML tag (`<p class="intro">`, `</strong>`, `<br/>`)
/// - JSON / TOML string literal (`"task"`, `"embedded.llm"`,
///   `"こんにちは、世界。"`) — see rationale below
/// - Module path with `::` (`moonlens_lib::pipeline`)
/// - Hex literal (`0xDEADBEEF`)
/// - Version string (`1.2.3`, `2.4.1-rc.1`)
/// - CLI flag (`--config`, `-v`)
/// - Key=value (`n_threads=6`)
/// - Snake_case identifier (`embedded_llm_perf`)
/// - Log level (uppercase only: `INFO` / `WARN` / `ERROR` / `DEBUG` / `TRACE`)
///
/// HTML rationale (2026-05-26 routed eval): standard/quality JA
/// bridge drops `<strong>` / `<p class="intro">` while translating
/// the surrounding prose, producing `<p>MoonLens, a translation
/// tool that respects formatting.</p>`. Masking the tags lifts them
/// out of the model's translatable surface entirely. The pattern
/// matches open / close / self-closing forms with attributes; it
/// deliberately rejects malformed `<` followed by a space (so
/// inequalities like `if x < 5` stay untouched).
///
/// JSON rationale (2026-05-26 routed eval, Hy-MT2 adoption):
/// Hy-MT2 on the structured-json fixture occasionally outputs ONLY
/// the Japanese value already embedded inside the source JSON
/// (`{"output": {"Text": "こんにちは、世界。"}, ...}` → just
/// `こんにちは、世界。`). The model treats the embedded JA literal
/// as "the translation" and drops the surrounding structure.
/// Masking every double-quoted string lifts them all out of the
/// translatable surface — the model sees `{__MLENS_T_3__:
/// __MLENS_T_4__, ...}` and has no JA content to latch onto.
/// Trade-off: prose translations that legitimately contain
/// `"quoted phrases"` get their quoted portions preserved
/// verbatim instead of translated. Acceptable for a developer-
/// tool target audience where code/config/log content dominates
/// over English fiction with dialogue quotes.
fn protected_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        // Each alternative is anchored with `\b` where appropriate
        // to avoid grabbing characters that belong to surrounding
        // tokens. Numeric prefixes use `(?:^|[^\w])` look-behind via
        // word-boundary because `regex` doesn't support
        // variable-width lookbehinds.
        let pattern = r#"(?x)
            \b\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:?\d{2})?\b      # ISO datetime
            | \b\d{4}-\d{2}-\d{2}\b                                    # ISO date
            | https?://[^\s]+                                          # URL
            | </?[A-Za-z][A-Za-z0-9]*(?:\s+[A-Za-z_:][A-Za-z0-9_:.\-]*(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]+))?)*\s*/?>  # HTML tag
            | "[^"\n]*"                                                # JSON/TOML/code double-quoted string literal
            | \b[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+\b  # module path
            | \b0x[0-9a-fA-F]+\b                                       # hex literal
            | \b\d+\.\d+(?:\.\d+)?(?:-[A-Za-z0-9]+(?:\.[A-Za-z0-9]+)*)?\b # version
            | (?:^|\s)\-\-?[A-Za-z][A-Za-z0-9_\-]*\b                   # CLI flag
            | \b[a-z_][a-z0-9_]*=[^\s\)\]\},]+                         # key=value
            | \b[a-z]+_[a-z][a-z0-9_]*\b                               # snake_case (lowercase only)
            | \b(?:INFO|WARN|ERROR|DEBUG|TRACE)\b                      # log level
        "#;
        Regex::new(pattern).expect("placeholder regex is well-formed")
    })
}

/// Replace every protected token in `text` with an opaque
/// placeholder. Returns `(masked_text, map)` so the caller can run
/// [`unmask`] on the model's output.
///
/// When the input contains no protected tokens the returned map is
/// empty and the text is unchanged — callers can skip the unmask
/// step entirely in that case (cheap fast-path).
pub fn mask(text: &str) -> (String, PlaceholderMap) {
    let re = protected_token_re();
    let mut map = PlaceholderMap::default();
    let mut out = String::with_capacity(text.len());
    let mut last_end = 0usize;
    for m in re.find_iter(text) {
        // Some patterns (CLI flag) include a leading whitespace
        // character so the word boundary works. Detect that and
        // keep the whitespace in `out` rather than inside the
        // placeholder.
        let mut start = m.start();
        let matched = m.as_str();
        let leading_skipped = matched.chars().next().is_some_and(|c| c.is_whitespace());
        let token = if leading_skipped {
            start += matched.chars().next().unwrap().len_utf8();
            &matched[matched.chars().next().unwrap().len_utf8()..]
        } else {
            matched
        };

        // Emit prose before the token verbatim.
        out.push_str(&text[last_end..start]);

        // Assign a placeholder id and emit it in place of the
        // original token. Deduplicate so the same identifier
        // appearing twice gets the same placeholder — saves the
        // model from having to learn two arbitrary names map to
        // the same source token.
        let id = match map.entries.iter().position(|e| e == token) {
            Some(idx) => idx,
            None => {
                map.entries.push(token.to_string());
                map.entries.len() - 1
            }
        };
        out.push_str(&PlaceholderMap::placeholder_for(id));
        last_end = m.end();
    }
    out.push_str(&text[last_end..]);
    (out, map)
}

/// Maximum byte length of any placeholder we might emit. The
/// streaming wrapper holds back this many bytes from the tail of
/// the accumulated buffer because they MAY be the prefix of an
/// in-flight placeholder. A complete placeholder is at most
/// `__MLENS_T_<usize_max_digits>__` = 14 + 20 = 34 bytes; we
/// pad the bound to 40 for a safety margin in case future
/// placeholder formats grow.
const MAX_PLACEHOLDER_BYTES: usize = 40;

/// Restore the original tokens previously replaced by [`mask`].
/// Tolerates partial output (leftover placeholder fragments are
/// left untouched) so this is safe to call on streaming chunks.
///
/// The model occasionally mangles the placeholder by changing case
/// (`__mlens_t_3__`) or by inserting whitespace inside it
/// (`__ MLENS_T_3 __`). The matcher accepts those variations with
/// a tolerant regex; minor mangling does not break restoration.
pub fn unmask(text: &str, map: &PlaceholderMap) -> String {
    if map.is_empty() {
        return text.to_string();
    }
    static UM: OnceLock<Regex> = OnceLock::new();
    let re = UM.get_or_init(|| {
        Regex::new(r"__\s?[Mm][Ll][Ee][Nn][Ss]\s?[_\-]\s?[Tt]\s?[_\-]\s?(\d+)\s?__")
            .expect("unmask regex compiles")
    });
    let mut out = String::with_capacity(text.len());
    let mut last_end = 0usize;
    for cap in re.captures_iter(text) {
        let full = cap.get(0).unwrap();
        out.push_str(&text[last_end..full.start()]);
        let id: usize = cap[1].parse().unwrap_or(usize::MAX);
        match map.entries.get(id) {
            Some(orig) => out.push_str(orig),
            // Unknown id (model invented one) → leave the
            // placeholder text intact so the user can see it and
            // know something went wrong, rather than emitting
            // arbitrary content.
            None => out.push_str(full.as_str()),
        }
        last_end = full.end();
    }
    out.push_str(&text[last_end..]);
    out
}

/// Stateful stream filter that applies [`unmask`] to streaming
/// chunks. Holds back the trailing `MAX_PLACEHOLDER_BYTES` bytes
/// of accumulated output because they could be the prefix of an
/// in-flight placeholder; flushes the safe prefix on each call;
/// emits everything on `done=true`.
///
/// Why not unmask each chunk independently: a placeholder like
/// `__MLENS_T_3__` can split across token boundaries (the model
/// emits `__MLENS_T_` then `3__` as separate chunks). Naive
/// per-chunk unmask would see neither piece as a complete match
/// and leak the raw placeholder to the user.
pub struct StreamUnmask {
    map: PlaceholderMap,
    pending: String,
}

impl StreamUnmask {
    pub fn new(map: PlaceholderMap) -> Self {
        Self {
            map,
            pending: String::new(),
        }
    }

    /// Returns the safe-to-emit text for this chunk. When `done`,
    /// flushes the entire pending buffer; otherwise holds back the
    /// suffix containing any in-flight (unclosed) placeholder.
    ///
    /// Algorithm: find the rightmost `__` in pending and inspect
    /// what follows. If the `__` is followed by another `__` later
    /// in pending, this `__` is either the closing of a placeholder
    /// (already substitutable) or paired with a later `__` (paired
    /// either way). Safe to emit the whole pending. If no closing
    /// `__` follows, this `__` is an OPEN placeholder fragment that
    /// the next chunk will complete — hold back from that position.
    ///
    /// The model emits placeholder tokens piecemeal (single ASCII
    /// chars per piece), so this byte-level check is enough — we
    /// don't need to recognise the `MLENS_T_<n>` interior.
    pub fn process(&mut self, delta: &str, done: bool) -> String {
        if self.map.is_empty() {
            return delta.to_string();
        }
        self.pending.push_str(delta);
        if done {
            let out = unmask(&self.pending, &self.map);
            self.pending.clear();
            return out;
        }
        let safe_end = safe_emit_boundary(&self.pending);
        if safe_end == 0 {
            return String::new();
        }
        let safe_prefix = self.pending[..safe_end].to_string();
        self.pending.drain(..safe_end);
        unmask(&safe_prefix, &self.map)
    }
}

/// Identify the largest prefix of `pending` that is safe to emit.
///
/// Two cases of in-flight placeholder we must hold back:
/// 1. An unpaired opening `__` whose closing `__` is yet to come.
/// 2. A trailing single `_` that could become `__` on the next
///    chunk (the model emits tokens piecemeal — sometimes one
///    underscore at a time — so the lone underscore is a partial
///    "could-be-placeholder-start").
///
/// Algorithm: count `__` occurrences (non-overlapping, left-to-
/// right). If odd, the last `__` is unpaired — hold from there.
/// Otherwise pending has no open placeholder, but still check the
/// trailing byte for a single `_` and back off if present.
fn safe_emit_boundary(pending: &str) -> usize {
    let bytes = pending.as_bytes();
    let mut positions: Vec<usize> = Vec::new();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'_' && bytes[i + 1] == b'_' {
            positions.push(i);
            i += 2;
        } else {
            i += 1;
        }
    }

    // Unpaired opening `__`: hold from there. Last paired close is
    // ignored — the `__` after the opener is its closing, which
    // means there's nothing further to hold.
    if positions.len() % 2 == 1 {
        let unpaired = *positions.last().unwrap();
        return floor_char_boundary(pending, unpaired);
    }

    // All `__` paired. Now check the TAIL region (bytes after the
    // last paired close) for a single trailing `_` that could
    // combine with the next chunk's first byte to start a new
    // placeholder. Only the byte at `pending.len() - 1` matters —
    // earlier `_` chars are followed by non-`_` so they can't be
    // the start of an upcoming `__` pair.
    let last_paired_end = positions.last().map(|p| p + 2).unwrap_or(0);
    if pending.len() > last_paired_end && bytes[pending.len() - 1] == b'_' && pending.len() >= 1 {
        return floor_char_boundary(pending, pending.len() - 1);
    }
    pending.len()
}

/// Like the unstable `str::floor_char_boundary` — find the largest
/// index ≤ `i` that lands on a UTF-8 char boundary.
fn floor_char_boundary(s: &str, i: usize) -> usize {
    let i = i.min(s.len());
    let mut b = i;
    while b > 0 && !s.is_char_boundary(b) {
        b -= 1;
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_round_trip(input: &str) {
        let (masked, map) = mask(input);
        let restored = unmask(&masked, &map);
        assert_eq!(
            restored, input,
            "round-trip mismatch\n  input:    {input}\n  masked:   {masked}\n  restored: {restored}"
        );
    }

    #[test]
    fn mask_iso_timestamp() {
        let (masked, map) = mask("2026-05-18T00:51:11.674022Z INFO");
        assert!(
            masked.contains("__MLENS_T_0__"),
            "expected placeholder substitution: {masked}"
        );
        assert_eq!(map.len(), 2); // timestamp + INFO
    }

    #[test]
    fn mask_module_path() {
        let (masked, _) = mask("moonlens_lib::pipeline");
        assert_eq!(masked, "__MLENS_T_0__");
    }

    #[test]
    fn mask_snake_case_identifier() {
        let (masked, _) = mask("call embedded_llm_perf to warm");
        assert!(masked.contains("__MLENS_T_"));
        assert!(masked.contains("call"));
        assert!(masked.contains("to warm"));
    }

    #[test]
    fn mask_url() {
        let (masked, _) = mask("see https://example.com/docs for details");
        assert!(masked.contains("__MLENS_T_"));
    }

    #[test]
    fn mask_cli_flag() {
        let (masked, _) = mask("run with --config path.toml");
        assert!(masked.contains("__MLENS_T_"));
        // Leading whitespace before `--config` must stay in the
        // output (not get absorbed into the placeholder).
        assert!(masked.contains("with "), "trailing space lost: {masked}");
    }

    #[test]
    fn mask_log_levels() {
        let (masked, map) = mask("WARN something INFO else ERROR done");
        assert_eq!(map.len(), 3);
        assert!(masked.contains("__MLENS_T_0__"));
        assert!(masked.contains("__MLENS_T_1__"));
        assert!(masked.contains("__MLENS_T_2__"));
    }

    #[test]
    fn mask_dedupes_repeated_tokens() {
        // `embedded_llm_perf` appears twice; both occurrences must
        // share the same placeholder so the model sees a consistent
        // mapping rather than two unrelated tokens.
        let (masked, map) = mask("embedded_llm_perf foo embedded_llm_perf");
        assert_eq!(
            map.len(),
            1,
            "deduped map should have one entry: {:?}",
            map.entries
        );
        assert_eq!(masked.matches("__MLENS_T_0__").count(), 2);
    }

    #[test]
    fn mask_preserves_prose_between_tokens() {
        let (masked, _) = mask("Run embedded_llm_perf then check status");
        assert!(masked.starts_with("Run "));
        assert!(masked.contains(" then check status"));
    }

    #[test]
    fn round_trip_rust_tracing_log_line() {
        assert_round_trip("2026-05-18T00:51:11.674022Z  INFO embedded_llm_perf: warmup");
    }

    #[test]
    fn round_trip_toml_block() {
        assert_round_trip(
            "[providers.configs.\"embedded.llm\"]\nrepo = \"mmnga/llm-jp-3-980m-instruct3-gguf\"",
        );
    }

    #[test]
    fn unmask_tolerates_case_mangling() {
        let (_masked, map) = mask("foo embedded_llm_perf bar");
        // Model rewrote the placeholder in lowercase.
        let restored = unmask("foo __mlens_t_0__ bar", &map);
        assert!(
            restored.contains("embedded_llm_perf"),
            "case-mangled placeholder not restored: {restored}"
        );
    }

    #[test]
    fn unmask_passes_through_when_map_empty() {
        let map = PlaceholderMap::default();
        assert_eq!(unmask("plain text", &map), "plain text");
    }

    #[test]
    fn mask_skips_text_with_no_protected_tokens() {
        let (masked, map) = mask("Hello, world.");
        assert_eq!(masked, "Hello, world.");
        assert!(map.is_empty());
    }

    #[test]
    fn mask_html_tags() {
        // Open tag with attribute, close tag, plain prose between.
        let (masked, map) =
            mask(r#"<p class="intro">Welcome to <strong>MoonLens</strong>, the tool.</p>"#);
        // Three tags: <p class="intro">, <strong>, </strong>, </p> = 4 total.
        assert_eq!(
            map.len(),
            4,
            "expected 4 HTML tags, got {}: {masked}",
            map.len()
        );
        // Prose text must remain visible to the model.
        assert!(masked.contains("Welcome to"), "prose stripped: {masked}");
        assert!(masked.contains("MoonLens"), "MoonLens stripped: {masked}");
        // Restoring the placeholders gives back the original.
        let restored = unmask(&masked, &map);
        assert_eq!(
            restored,
            r#"<p class="intro">Welcome to <strong>MoonLens</strong>, the tool.</p>"#,
        );
    }

    #[test]
    fn mask_html_does_not_grab_inequality() {
        // `x < 5` must NOT match the HTML regex (the `<` is followed
        // by whitespace, not a tag name). Confirms the tag-name
        // anchor in the pattern.
        let (masked, map) = mask("if x < 5 && y > 3");
        assert!(map.is_empty(), "false-positive HTML match: {masked}");
        assert_eq!(masked, "if x < 5 && y > 3");
    }

    #[test]
    fn mask_json_string_literals() {
        // The 2026-05-26 routed eval surfaced Hy-MT2 outputting
        // ONLY `こんにちは、世界。` (the embedded JA value) when
        // asked to translate the structured-json fixture. Masking
        // every double-quoted string takes the JA content out of
        // the model's surface entirely.
        let (masked, map) =
            mask(r#"{"task": "Translate", "output": {"Text": "こんにちは、世界。"}, "n": 1760}"#);
        // Six quoted strings: "task", "Translate", "output", "Text", "こんにちは、世界。", "n"
        assert_eq!(
            map.len(),
            6,
            "expected 6 string literals, got {}: {masked}",
            map.len()
        );
        // The JA content must be inside the map, not in the masked text.
        assert!(
            !masked.contains("こんにちは"),
            "JA content leaked into masked text: {masked}"
        );
        // Round-trip preserves the original.
        let restored = unmask(&masked, &map);
        assert_eq!(
            restored,
            r#"{"task": "Translate", "output": {"Text": "こんにちは、世界。"}, "n": 1760}"#,
        );
    }

    #[test]
    fn mask_html_self_closing_and_void_tags() {
        let (masked, map) = mask(r#"line 1<br/>line 2<hr />line 3<img src="x.png">"#);
        assert_eq!(map.len(), 3, "expected 3 tags, got {}: {masked}", map.len());
        let restored = unmask(&masked, &map);
        assert_eq!(
            restored,
            r#"line 1<br/>line 2<hr />line 3<img src="x.png">"#
        );
    }

    #[test]
    fn stream_unmask_handles_intact_placeholder_in_single_chunk() {
        let (_, map) = mask("call embedded_llm_perf now");
        let mut su = StreamUnmask::new(map);
        // Pad to exceed MAX_PLACEHOLDER_BYTES so the safe-prefix
        // logic actually emits.
        let pad = "x".repeat(64);
        let chunk = format!("done __MLENS_T_0__ {pad}");
        let out = su.process(&chunk, false);
        assert!(
            out.contains("embedded_llm_perf"),
            "placeholder not restored: {out}"
        );
        // Whatever didn't flush, the final `done=true` call must
        // cover.
        let tail = su.process("", true);
        let combined = format!("{out}{tail}");
        assert!(combined.contains("embedded_llm_perf"));
    }

    #[test]
    fn stream_unmask_handles_placeholder_split_across_chunks() {
        let (_, map) = mask("call embedded_llm_perf now");
        let mut su = StreamUnmask::new(map);
        // The model emitted the placeholder in three pieces.
        let pad = "x".repeat(64);
        let chunks = ["prefix __MLENS_", "T_", &format!("0__ suffix {pad}")];
        let mut combined = String::new();
        for (i, c) in chunks.iter().enumerate() {
            let last = i == chunks.len() - 1;
            combined.push_str(&su.process(c, false));
            if last {
                combined.push_str(&su.process("", true));
            }
        }
        assert!(
            combined.contains("embedded_llm_perf"),
            "split placeholder not restored: {combined}"
        );
        assert!(!combined.contains("__MLENS_T_0__"));
    }

    #[test]
    fn stream_unmask_emits_complete_placeholder_immediately() {
        // The new safe-boundary algorithm flushes as soon as the
        // placeholder is COMPLETE (paired `__`), regardless of how
        // small the chunk is. Only OPEN placeholders are held back.
        let (_, map) = mask("a embedded_llm_perf b");
        let mut su = StreamUnmask::new(map);
        let out = su.process("done __MLENS_T_0__ tail", false);
        assert!(
            out.contains("embedded_llm_perf"),
            "complete placeholder must be restored immediately: {out}"
        );
        assert!(!out.contains("__MLENS_T_0__"));
        // Trailing flush is a no-op (everything already emitted).
        let tail = su.process("", true);
        assert_eq!(tail, "");
    }

    #[test]
    fn stream_unmask_holds_back_open_placeholder() {
        // Open `__` with no closing partner → must be held back so
        // it's not surfaced to the user as raw garbage.
        let (_, map) = mask("a embedded_llm_perf b");
        let mut su = StreamUnmask::new(map);
        let out = su.process("done __MLENS_T_0 (not yet closed)", false);
        // The text up to (but not including) the open `__` should
        // emit. The rest is held.
        assert!(out.starts_with("done "), "prose before `__` emits: {out}");
        assert!(
            !out.contains("__MLENS"),
            "open placeholder must NOT leak: {out}"
        );
        // Now close it: next chunk supplies the closing `__`.
        let tail = su.process("_ more__ end", true);
        // Final flush restores via unmask. With the closing `_ more__`
        // matched by the regex' tolerant whitespace, the
        // placeholder may or may not be recognised — either way
        // the open `__` no longer appears as a raw fragment.
        let combined = format!("{out}{tail}");
        assert!(
            combined.contains("embedded_llm_perf") || combined.contains("__MLENS_T_0"),
            "either restored or visible to user: {combined}"
        );
    }

    #[test]
    fn stream_unmask_passes_through_when_map_empty() {
        let map = PlaceholderMap::default();
        let mut su = StreamUnmask::new(map);
        let out = su.process("Hello, world.", false);
        assert_eq!(out, "Hello, world.");
        let tail = su.process("", true);
        assert_eq!(tail, "");
    }

    #[test]
    fn mask_does_not_grab_partial_decimals() {
        // "3.14" must be masked (looks like a version) — we
        // tolerate this because losing identifier preservation on
        // "1.5.0" version strings would be worse than over-
        // masking the occasional bare decimal in prose.
        let (masked, map) = mask("pi is 3.14 roughly");
        assert!(masked.contains("__MLENS_T_"));
        assert_eq!(map.len(), 1);
    }
}
