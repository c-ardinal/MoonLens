//! Per-request glossary extraction.
//!
//! Before translating a batch of OCR / audio lines, we ask the
//! embedded LLM to scan the whole batch and emit a small glossary of
//! proper nouns and technical terms with their target-language
//! renderings. The pipeline then injects that glossary as a
//! system-prompt fragment so every line in the batch uses the *same*
//! translation for the same source term.
//!
//! Why this exists
//!  The per-line translation path treats each line in isolation.
//!  When the same proper noun ("The Speech Accent Archive") appears
//!  three times across a single image, the LLM may render it three
//!  different ways with no continuity. The glossary pass adds one
//!  extra LLM call upfront and locks the renderings.
//!
//! Background: <https://zenn.dev/7shi/scraps/da63f326aed58d>
//! ("ローカル LLM による翻訳実験" — `trtools term`).

use std::collections::HashSet;

/// One glossary entry — a source term and the agreed-upon target
/// rendering. Both sides are post-trim, non-empty UTF-8.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlossaryEntry {
    pub source: String,
    pub target: String,
}

/// Build the user message we send to the LLM. The system prompt is
/// expected to live separately (set via the inference call).
///
/// The shape is deliberately Latin/CJK-agnostic: `src=tgt` per line,
/// no JSON, no quotes. Small models follow this pattern reliably;
/// large models do too but tend to add commentary which the parser
/// then strips.
pub fn build_prompt(lines: &[&str], src_lang: &str, tgt_lang: &str) -> String {
    let mut s = String::with_capacity(256 + lines.iter().map(|l| l.len() + 4).sum::<usize>());
    s.push_str("Extract a translation glossary for the source text below.\n\n");
    s.push_str(&format!(
        "Source language: {src_lang}\nTarget language: {tgt_lang}\n\n"
    ));
    s.push_str(
        "List proper nouns, named entities, and recurring technical terms \
         that should be rendered the SAME way every time they appear in the \
         translation. Common words, function words, and pronouns do NOT \
         belong in this glossary.\n\n",
    );
    s.push_str("Format: one pair per line, with `=` between source and target.\n");
    s.push_str("Example:\n");
    s.push_str("Speech Accent Archive = スピーチアクセントアーカイブ\n");
    s.push_str("SAA = SAA\n\n");
    s.push_str("Rules:\n");
    s.push_str("- Output only the `src=tgt` lines, no commentary.\n");
    s.push_str("- If no glossary-worthy terms exist, output an empty response.\n");
    s.push_str("- Do not invent terms that are not in the source.\n");
    s.push_str("- Keep source terms exactly as they appear (case, punctuation).\n\n");
    s.push_str("Source text:\n---\n");
    for line in lines {
        s.push_str(line.trim());
        s.push('\n');
    }
    s.push_str("---\n");
    s
}

/// System prompt for the glossary pass. Kept separate from
/// `build_prompt` so the inference layer can apply chat-template
/// system / user role tagging cleanly.
pub const SYSTEM_PROMPT: &str =
    "You are a glossary extractor for a translation pipeline. \
     You read source text and emit only `source_term=target_translation` lines. \
     You never invent terms not present in the source. \
     You never add commentary or markdown formatting around the output.";

/// Parse the LLM response into clean glossary entries.
///
/// Tolerates:
///  - Leading / trailing commentary lines (e.g. "Here is the glossary:")
///  - Triple-backtick fences if the model decides to wrap output
///  - Surrounding quotes / asterisks on either side of `=`
///  - Multiple `=` characters (we split on the first only)
///  - Blank lines, indentation, leading bullet markers
///
/// Rejects:
///  - Lines without `=`
///  - Entries with empty source or target after trimming
///  - Duplicate source terms (keep first occurrence to be stable)
///  - Source terms that are NOT present in `source_text` when supplied
///    (anti-hallucination guard — pass `None` to disable).
pub fn parse_response(raw: &str, source_text: Option<&str>) -> Vec<GlossaryEntry> {
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let lowered_source = source_text.map(|s| s.to_lowercase());

    for line in raw.lines() {
        let line = strip_decorations(line);
        if line.is_empty() {
            continue;
        }
        let (src, tgt) = match line.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        let src = strip_decorations(src);
        let tgt = strip_decorations(tgt);
        if src.is_empty() || tgt.is_empty() {
            continue;
        }
        if let Some(haystack) = &lowered_source {
            if !haystack.contains(&src.to_lowercase()) {
                continue;
            }
        }
        let key = src.to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        out.push(GlossaryEntry {
            source: src.to_string(),
            target: tgt.to_string(),
        });
    }
    out
}

/// Strip the kinds of decorations small LLMs love to add around
/// glossary entries: triple-backtick fences, bullet markers, bold
/// markers, surrounding quotes, leading numbering, trailing
/// punctuation that snuck in from the prose.
fn strip_decorations(s: &str) -> &str {
    let t = s.trim();
    let t = t.trim_start_matches("```").trim_end_matches("```");
    let t = t.trim_start_matches([' ', '\t', '-', '*', '•', '・']);
    let t = t.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')');
    let t = t.trim_matches([' ', '\t', '"', '\'', '*', '`']);
    t.trim()
}

/// Render a glossary as the system-prompt fragment we splice into
/// the per-line translation system prompt. Returns an empty string
/// when the glossary is empty so callers can append it
/// unconditionally without producing a stray separator.
pub fn render_for_system_prompt(glossary: &[GlossaryEntry]) -> String {
    if glossary.is_empty() {
        return String::new();
    }
    let mut s = String::from("\n\nUse this glossary verbatim for terms that appear in the source:\n");
    for e in glossary {
        s.push_str("- ");
        s.push_str(&e.source);
        s.push_str(" → ");
        s.push_str(&e.target);
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_includes_all_lines() {
        let lines = ["The Speech Accent Archive (SAA)", "Please call Stella."];
        let prompt = build_prompt(&lines, "en", "ja");
        assert!(prompt.contains("The Speech Accent Archive"));
        assert!(prompt.contains("Please call Stella"));
        assert!(prompt.contains("Source language: en"));
        assert!(prompt.contains("Target language: ja"));
    }

    #[test]
    fn parse_clean_response() {
        let raw = "Speech Accent Archive = スピーチアクセントアーカイブ\n\
                   SAA = SAA\n\
                   Stella = ステラ\n";
        let out = parse_response(raw, None);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].source, "Speech Accent Archive");
        assert_eq!(out[0].target, "スピーチアクセントアーカイブ");
        assert_eq!(out[2].source, "Stella");
    }

    #[test]
    fn parse_strips_commentary_lines() {
        let raw = "Here is the glossary:\n\n\
                   Speech Accent Archive = スピーチアクセントアーカイブ\n\
                   That's all I found.\n";
        let out = parse_response(raw, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, "Speech Accent Archive");
    }

    #[test]
    fn parse_handles_bullet_decorations() {
        let raw = "- SAA = SAA\n\
                   * Wikipedia = Wikipedia\n\
                   1. Bob = ボブ\n\
                   2) Stella = ステラ\n";
        let out = parse_response(raw, None);
        let sources: Vec<&str> = out.iter().map(|e| e.source.as_str()).collect();
        assert_eq!(sources, vec!["SAA", "Wikipedia", "Bob", "Stella"]);
    }

    #[test]
    fn parse_strips_surrounding_quotes_and_bold() {
        let raw = "\"SAA\" = \"SAA\"\n\
                   **Wikipedia** = **ウィキペディア**\n";
        let out = parse_response(raw, None);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].source, "SAA");
        assert_eq!(out[1].target, "ウィキペディア");
    }

    #[test]
    fn parse_skips_duplicates_keeping_first() {
        let raw = "SAA = SAA\n\
                   saa = エス・エー・エー\n";
        let out = parse_response(raw, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].target, "SAA");
    }

    #[test]
    fn parse_rejects_empty_sides() {
        let raw = " = nothing\n\
                   empty = \n\
                   = \n";
        let out = parse_response(raw, None);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_handles_multiple_equals() {
        // "x = y = z" should split on first `=` so source="x" target="y = z"
        let raw = "E = mc^2 = energy formula\n";
        let out = parse_response(raw, None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, "E");
        assert_eq!(out[0].target, "mc^2 = energy formula");
    }

    #[test]
    fn parse_drops_hallucinated_terms_when_source_provided() {
        let raw = "Speech Accent Archive = スピーチアクセントアーカイブ\n\
                   InventedCorp = 架空法人\n";
        let source_text = "The Speech Accent Archive (SAA) is a database.";
        let out = parse_response(raw, Some(source_text));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].source, "Speech Accent Archive");
    }

    #[test]
    fn parse_strips_triple_backtick_fences() {
        let raw = "```\nSAA = SAA\n```\n";
        let out = parse_response(raw, None);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn parse_empty_when_response_blank() {
        assert!(parse_response("", None).is_empty());
        assert!(parse_response("\n\n   \n", None).is_empty());
    }

    #[test]
    fn render_for_system_prompt_empty_glossary_returns_empty() {
        assert!(render_for_system_prompt(&[]).is_empty());
    }

    #[test]
    fn render_for_system_prompt_formats_entries() {
        let entries = vec![
            GlossaryEntry {
                source: "SAA".into(),
                target: "SAA".into(),
            },
            GlossaryEntry {
                source: "Stella".into(),
                target: "ステラ".into(),
            },
        ];
        let rendered = render_for_system_prompt(&entries);
        assert!(rendered.contains("SAA → SAA"));
        assert!(rendered.contains("Stella → ステラ"));
        assert!(rendered.starts_with("\n\nUse this glossary"));
    }
}
