//! User glossary and Japanese style, turned into prompt pieces for the
//! local translators.
//!
//! Both are applied per request and only where they matter, so a request
//! with no matching term and the default style produces exactly the same
//! prompt as before (no quality change for users who never touch them):
//!
//! - PLaMo 3 Translate takes them natively in PFML3: a `glossary` element
//!   before `input` (`<|plamo:index|>term<|plamo:item|>rendering`) and a
//!   `form` attribute (`plain` / `polite`) on a Japanese `output`.
//! - Chat models (Gemma etc.) get one extra paragraph in the system
//!   prompt. The source text stays alone in the user turn.
//!
//! Only terms that occur in the text are sent: a long glossary would
//! otherwise bloat every prompt and nudge small models into inserting
//! terms that are not there.

use crate::settings::schema::{JaStyle, TranslationSettings};

/// Matched terms sent per request at most (keeps the prompt bounded).
pub const MAX_TERMS_PER_REQUEST: usize = 40;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranslateOptions {
    pub style: JaStyle,
    terms: Vec<Term>,
}

#[derive(Debug, Clone, PartialEq)]
struct Term {
    source: String,
    /// Lowercased source, for case-insensitive matching.
    needle: String,
    target: String,
    /// Normalised language tag; empty = any.
    lang: String,
}

impl TranslateOptions {
    pub fn from_settings(s: &TranslationSettings) -> Self {
        let mut terms: Vec<Term> = s
            .glossary
            .iter()
            .filter_map(|e| {
                let source = sanitize(&e.source);
                let target = sanitize(&e.target);
                if source.is_empty() || target.is_empty() {
                    return None;
                }
                Some(Term {
                    needle: source.to_lowercase(),
                    source,
                    target,
                    lang: norm_lang(&e.lang),
                })
            })
            .collect();
        // Longer terms first, so "Project Manager" wins over "Project"
        // when both match.
        terms.sort_by_key(|t| std::cmp::Reverse(t.source.chars().count()));
        Self {
            style: s.style,
            terms,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.style == JaStyle::Auto && self.terms.is_empty()
    }

    /// Glossary pairs that apply to translating `text` into `target`.
    pub fn terms_for(&self, text: &str, target: &str) -> Vec<(String, String)> {
        if self.terms.is_empty() {
            return Vec::new();
        }
        let target = norm_lang(target);
        let hay = text.to_lowercase();
        let mut out: Vec<(String, String)> = Vec::new();
        for t in &self.terms {
            if out.len() >= MAX_TERMS_PER_REQUEST {
                break;
            }
            if !t.lang.is_empty() && !lang_matches(&t.lang, &target) {
                continue;
            }
            if out.iter().any(|(s, _)| s.to_lowercase() == t.needle) {
                continue;
            }
            if contains_term(&hay, &t.needle) {
                out.push((t.source.clone(), t.target.clone()));
            }
        }
        out
    }

    /// PFML3 `form` value for `target`, when a style is set and the
    /// target is Japanese (the only language PLaMo supports it for).
    pub fn form_for(&self, target: &str) -> Option<&'static str> {
        // Easy Japanese has its own (polite) register; a style on top of
        // it made the real model mix both.
        if primary(&norm_lang(target)) != "ja" || crate::lang::is_easy_japanese(target) {
            return None;
        }
        match self.style {
            JaStyle::Auto => None,
            JaStyle::Polite => Some("polite"),
            JaStyle::Plain => Some("plain"),
        }
    }

    /// Extra system-prompt paragraph for chat models, or `None` when
    /// nothing applies (the prompt then stays byte-identical).
    pub fn chat_instructions(&self, text: &str, target: &str) -> Option<String> {
        let mut parts = Vec::new();
        match self.form_for(target) {
            Some("polite") => parts.push(
                "Write the Japanese in the polite style (desu/masu form) throughout.".to_string(),
            ),
            Some("plain") => parts.push(
                "Write the Japanese in the plain style (da/dearu form) throughout, \
                 never desu/masu."
                    .to_string(),
            ),
            _ => {}
        }
        let terms = self.terms_for(text, target);
        if !terms.is_empty() {
            let list: Vec<String> = terms.iter().map(|(s, t)| format!("- {s} => {t}")).collect();
            parts.push(format!(
                "Glossary: always translate these terms exactly as given \
                 (source => translation), adjusting only inflection where the grammar needs it:\n{}",
                list.join("\n")
            ));
        }
        (!parts.is_empty()).then(|| parts.join("\n"))
    }
}

/// PFML3 `glossary` element (empty string when there are no terms).
pub fn pfml3_glossary(terms: &[(String, String)]) -> String {
    if terms.is_empty() {
        return String::new();
    }
    let mut s = String::from("<|plamo:tag|>glossary");
    for (src, tgt) in terms {
        s.push_str("<|plamo:index|>");
        s.push_str(src);
        s.push_str("<|plamo:item|>");
        s.push_str(tgt);
    }
    s
}

/// PFML3 attribute appended to the `output` element after `lang`.
pub fn pfml3_form(form: Option<&str>) -> String {
    form.map(|f| format!("<|plamo:key|>form<|plamo:val|>{f}"))
        .unwrap_or_default()
}

/// Trim, fold line breaks and neutralise PLaMo control tokens so a term
/// can never break the prompt structure.
fn sanitize(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("<|", "< |")
}

fn norm_lang(code: &str) -> String {
    let c = code.trim().to_ascii_lowercase().replace('_', "-");
    match c.as_str() {
        "zh" | "zh-cn" | "zh-sg" => "zh-hans".into(),
        "zh-tw" | "zh-hk" => "zh-hant".into(),
        _ => c,
    }
}

fn primary(code: &str) -> &str {
    code.split('-').next().unwrap_or(code)
}

/// `ja` matches `ja-JP`; Chinese scripts must match exactly.
fn lang_matches(entry: &str, target: &str) -> bool {
    if entry == target {
        return true;
    }
    if primary(entry) == "zh" || primary(target) == "zh" {
        return false;
    }
    primary(entry) == primary(target)
}

/// Case-insensitive occurrence. Terms made of Latin letters / digits must
/// stand as whole words, so "AI" does not match inside "MAIL"; CJK terms
/// match anywhere (no spaces between words).
fn contains_term(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let wordy = |c: char| c.is_ascii_alphanumeric();
    let check_bounds =
        needle.chars().next().is_some_and(wordy) || needle.chars().last().is_some_and(wordy);
    if !check_bounds {
        return hay.contains(needle);
    }
    let mut from = 0;
    while let Some(i) = hay[from..].find(needle) {
        let start = from + i;
        let end = start + needle.len();
        let before_ok = !needle.chars().next().is_some_and(wordy)
            || !hay[..start].chars().next_back().is_some_and(wordy);
        let after_ok = !needle.chars().last().is_some_and(wordy)
            || !hay[end..].chars().next().is_some_and(wordy);
        if before_ok && after_ok {
            return true;
        }
        from = start + hay[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::schema::GlossaryEntry;

    fn opts(style: JaStyle, g: &[(&str, &str, &str)]) -> TranslateOptions {
        TranslateOptions::from_settings(&TranslationSettings {
            style,
            glossary: g
                .iter()
                .map(|(s, t, l)| GlossaryEntry {
                    source: s.to_string(),
                    target: t.to_string(),
                    lang: l.to_string(),
                })
                .collect(),
        })
    }

    #[test]
    fn defaults_add_nothing() {
        let o = TranslateOptions::from_settings(&TranslationSettings::default());
        assert!(o.is_empty());
        assert_eq!(o.chat_instructions("Hello", "ja"), None);
        assert_eq!(o.form_for("ja"), None);
        assert_eq!(pfml3_glossary(&o.terms_for("Hello", "ja")), "");
    }

    #[test]
    fn only_terms_present_in_the_text_are_used() {
        let o = opts(
            JaStyle::Auto,
            &[("Dashboard", "管理画面", "ja"), ("Owner", "担当者", "ja")],
        );
        assert_eq!(
            o.terms_for("Open the dashboard.", "ja"),
            vec![("Dashboard".to_string(), "管理画面".to_string())]
        );
    }

    #[test]
    fn latin_terms_match_whole_words_cjk_anywhere() {
        let o = opts(
            JaStyle::Auto,
            &[("AI", "人工知能", ""), ("管理画面", "dashboard", "en")],
        );
        assert!(o.terms_for("Check your MAIL", "ja").is_empty());
        assert_eq!(o.terms_for("New AI tools", "ja").len(), 1);
        assert_eq!(o.terms_for("AI-powered", "ja").len(), 1);
        assert_eq!(o.terms_for("プロジェクトの管理画面を開く", "en").len(), 1);
    }

    #[test]
    fn language_filter() {
        let o = opts(
            JaStyle::Auto,
            &[
                ("Dashboard", "管理画面", "ja"),
                ("Dashboard", "仪表板", "zh-Hans"),
            ],
        );
        assert_eq!(o.terms_for("Dashboard", "ja-JP")[0].1, "管理画面");
        assert_eq!(o.terms_for("Dashboard", "zh-CN")[0].1, "仪表板");
        assert!(o.terms_for("Dashboard", "zh-Hant").is_empty());
        assert!(o.terms_for("Dashboard", "fr").is_empty());
    }

    #[test]
    fn longer_terms_win_and_duplicates_are_dropped() {
        let o = opts(
            JaStyle::Auto,
            &[
                ("Project", "案件", ""),
                ("Project Manager", "PM", ""),
                ("project", "計画", ""),
            ],
        );
        let t = o.terms_for("Ask the project manager", "ja");
        assert_eq!(
            t,
            vec![
                ("Project Manager".to_string(), "PM".to_string()),
                ("Project".to_string(), "案件".to_string()),
            ]
        );
    }

    #[test]
    fn style_applies_to_japanese_only() {
        let o = opts(JaStyle::Polite, &[]);
        assert_eq!(o.form_for("ja"), Some("polite"));
        assert_eq!(o.form_for("en"), None);
        assert!(o
            .chat_instructions("x", "ja")
            .unwrap()
            .contains("desu/masu"));
        assert_eq!(o.chat_instructions("x", "en"), None);
        assert_eq!(opts(JaStyle::Plain, &[]).form_for("ja"), Some("plain"));
    }

    #[test]
    fn easy_japanese_uses_the_japanese_glossary_without_a_style() {
        let o = opts(JaStyle::Plain, &[("Owner", "担当者", "ja")]);
        assert_eq!(o.form_for("ja-easy"), None);
        assert_eq!(
            o.terms_for("Ask the owner.", "ja-easy"),
            vec![("Owner".to_string(), "担当者".to_string())]
        );
        let chat = o.chat_instructions("Ask the owner.", "ja-easy").unwrap();
        assert!(chat.contains("担当者") && !chat.contains("desu/masu"));
    }

    #[test]
    fn pfml3_pieces_follow_the_model_card() {
        let terms = vec![
            ("管理画面".to_string(), "dashboard".to_string()),
            ("プロジェクト".to_string(), "project".to_string()),
        ];
        assert_eq!(
            pfml3_glossary(&terms),
            "<|plamo:tag|>glossary<|plamo:index|>管理画面<|plamo:item|>dashboard\
             <|plamo:index|>プロジェクト<|plamo:item|>project"
        );
        assert_eq!(
            pfml3_form(Some("plain")),
            "<|plamo:key|>form<|plamo:val|>plain"
        );
    }

    #[test]
    fn terms_cannot_inject_control_tokens() {
        let o = opts(JaStyle::Auto, &[("a<|plamo:tag|>b", "x\ny", "")]);
        let t = o.terms_for("a< |plamo:tag|>b", "ja");
        assert_eq!(t.len(), 1);
        assert!(!pfml3_glossary(&t).contains("a<|plamo:tag"));
        assert_eq!(t[0].1, "x y");
    }

    #[test]
    fn caps_terms_per_request() {
        let g: Vec<(String, String, String)> = (0..100)
            .map(|i| (format!("w{i}"), format!("t{i}"), String::new()))
            .collect();
        let text: String = (0..100).map(|i| format!("w{i} ")).collect();
        let o = TranslateOptions::from_settings(&TranslationSettings {
            style: JaStyle::Auto,
            glossary: g
                .into_iter()
                .map(|(source, target, lang)| GlossaryEntry {
                    source,
                    target,
                    lang,
                })
                .collect(),
        });
        assert_eq!(o.terms_for(&text, "ja").len(), MAX_TERMS_PER_REQUEST);
    }
}
