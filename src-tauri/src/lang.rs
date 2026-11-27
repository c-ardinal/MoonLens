//! Lightweight, dependency-free language guessing for OCR'd text.
//!
//! The capture flow uses this to skip blocks that are already in the
//! target language (a Japanese screen captured with target `ja` should
//! not round-trip through the translator) and to tell the UI what the
//! source language was. It only has to be right when it says "this is
//! language X"; anything uncertain returns `None` and gets translated.

/// Writing systems we can tell apart by code point alone.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ScriptCounts {
    kana: usize,
    han: usize,
    hangul: usize,
    latin: usize,
    cyrillic: usize,
    other_letters: usize,
}

fn count(text: &str) -> ScriptCounts {
    let mut c = ScriptCounts::default();
    for ch in text.chars() {
        let u = ch as u32;
        match u {
            0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9F => c.kana += 1,
            0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F => c.han += 1,
            0xAC00..=0xD7AF | 0x1100..=0x11FF | 0x3130..=0x318F => c.hangul += 1,
            0x0400..=0x04FF => c.cyrillic += 1,
            _ if ch.is_ascii_alphabetic() => c.latin += 1,
            0x00C0..=0x024F | 0xFF21..=0xFF3A | 0xFF41..=0xFF5A => c.latin += 1,
            _ if ch.is_alphabetic() => c.other_letters += 1,
            _ => {}
        }
    }
    c
}

/// Very common English function words. A Latin-script text in which a
/// good share of the words are on this list is English; short labels
/// with none of them stay undetermined.
const EN_WORDS: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "with", "is", "are", "was",
    "were", "be", "this", "that", "it", "you", "your", "we", "our", "not", "by", "from", "at",
    "as", "can", "will", "have", "has", "if", "all", "more", "about", "what", "how", "my", "do",
];

fn looks_english(text: &str) -> bool {
    let words: Vec<String> = text
        .split(|c: char| !c.is_ascii_alphabetic() && c != '\'')
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect();
    if words.len() < 3 {
        return false;
    }
    let hits = words
        .iter()
        .filter(|w| EN_WORDS.contains(&w.as_str()))
        .count();
    // Accented letters are rare in English and common in its neighbours.
    let accented = text
        .chars()
        .filter(|c| matches!(*c as u32, 0x00C0..=0x024F))
        .count();
    hits * 6 >= words.len() && accented * 50 <= text.len().max(1)
}

/// Best guess at the language of `text` as a BCP-47 primary tag
/// (`ja`, `zh`, `ko`, `en`, `ru`), or `None` when unsure.
pub fn detect(text: &str) -> Option<&'static str> {
    let c = count(text);
    let cjk = c.kana + c.han;
    let total = cjk + c.hangul + c.latin + c.cyrillic + c.other_letters;
    if total == 0 {
        return None;
    }
    // Any kana means Japanese: Chinese never uses it, and Japanese text
    // routinely mixes in Latin brand names.
    if c.kana > 0 && c.kana * 10 >= cjk {
        return Some("ja");
    }
    if c.hangul * 2 >= total {
        return Some("ko");
    }
    if c.han * 2 >= total && c.kana == 0 {
        return Some("zh");
    }
    if c.cyrillic * 2 >= total {
        return Some("ru");
    }
    if c.latin * 10 >= total * 9 && looks_english(text) {
        return Some("en");
    }
    None
}

/// Primary subtag of a language tag, lower-cased (`zh-Hans` → `zh`).
pub fn primary(tag: &str) -> String {
    tag.split(['-', '_'])
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

/// Tag of easy Japanese (やさしい日本語): Japanese rewritten with common
/// words and short sentences for learners and children. A way of
/// writing Japanese rather than a language of its own, so it is only
/// ever a target. The spelling is PLaMo's own code for it.
pub const EASY_JAPANESE: &str = "ja-easy";

/// True for [`EASY_JAPANESE`] in any case / separator spelling.
pub fn is_easy_japanese(tag: &str) -> bool {
    tag.trim()
        .replace('_', "-")
        .eq_ignore_ascii_case(EASY_JAPANESE)
}

/// True when `text` does not need translating into `target`: it has no
/// letters at all (numbers, prices, symbols) or is confidently already
/// written in that language.
pub fn already_in(text: &str, target: &str) -> bool {
    let c = count(text);
    if c.kana + c.han + c.hangul + c.latin + c.cyrillic + c.other_letters == 0 {
        return true;
    }
    // Ordinary Japanese is exactly what an easy-Japanese target rewrites,
    // kanji-only labels included.
    if is_easy_japanese(target) {
        return false;
    }
    let target = primary(target);
    if detect(text) == Some(target.as_str()) {
        return true;
    }
    // A Japanese reader reads short kanji-only labels (設定, 東京都) fine;
    // longer Chinese text without kana is still translated.
    target == "ja" && c.kana == 0 && c.han > 0 && c.han <= 4 && c.latin == 0
}

/// Dominant language over several texts, weighted by UTF-8 length (a
/// CJK character carries roughly as much as three Latin letters).
pub fn dominant<'a>(texts: impl IntoIterator<Item = &'a str>) -> Option<&'static str> {
    let mut tally: Vec<(&'static str, usize)> = Vec::new();
    for t in texts {
        if let Some(l) = detect(t) {
            let n = t.len();
            match tally.iter_mut().find(|(k, _)| *k == l) {
                Some((_, v)) => *v += n,
                None => tally.push((l, n)),
            }
        }
    }
    tally.into_iter().max_by_key(|(_, n)| *n).map(|(l, _)| l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_cjk_scripts() {
        assert_eq!(detect("これはテストです"), Some("ja"));
        assert_eq!(detect("東京都に住んでいます"), Some("ja"));
        assert_eq!(detect("我们今天去公园散步"), Some("zh"));
        assert_eq!(detect("안녕하세요 반갑습니다"), Some("ko"));
        assert_eq!(detect("Привет, как дела?"), Some("ru"));
    }

    #[test]
    fn japanese_with_latin_brand_names() {
        assert_eq!(detect("Google の検索結果を表示します"), Some("ja"));
    }

    #[test]
    fn english_needs_function_words() {
        assert_eq!(
            detect("The quick brown fox jumps over the lazy dog"),
            Some("en")
        );
        assert_eq!(detect("Settings"), None);
        assert_eq!(detect("Hola, ¿cómo estás? Muy bien gracias"), None);
        assert_eq!(
            detect("Der schnelle braune Fuchs springt über den Hund"),
            None
        );
    }

    #[test]
    fn already_in_target() {
        assert!(already_in("これはテストです", "ja"));
        assert!(already_in("これはテストです", "ja-JP"));
        assert!(!already_in("これはテストです", "en"));
        assert!(already_in("This is a test of the system", "en"));
        assert!(!already_in("This is a test of the system", "ja"));
        // No letters: nothing to translate.
        assert!(already_in("12:30 ¥1,980", "ja"));
        assert!(already_in("12:30 ¥1,980", "en"));
        // Short kanji labels are fine for Japanese readers…
        assert!(already_in("設定", "ja"));
        // …but Chinese sentences are not.
        assert!(!already_in("我们今天去公园散步", "ja"));
        assert!(already_in("我们今天去公园散步", "zh-Hans"));
    }

    #[test]
    fn easy_japanese_rewrites_japanese() {
        assert!(is_easy_japanese("ja-easy"));
        assert!(is_easy_japanese(" JA_Easy "));
        assert!(!is_easy_japanese("ja"));
        assert!(!is_easy_japanese("ja-JP"));
        // Japanese text and kanji labels go to the translator …
        assert!(!already_in("不要不急の外出は控えてください", "ja-easy"));
        assert!(!already_in("設定", "ja-easy"));
        assert!(!already_in("This is a test of the system", "ja-easy"));
        // … text without letters still does not.
        assert!(already_in("12:30 ¥1,980", "ja-easy"));
    }

    #[test]
    fn dominant_weights_by_length() {
        let texts = [
            "OK",
            "これは長い日本語の文章です",
            "Hello there, this is it",
        ];
        assert_eq!(dominant(texts), Some("ja"));
        assert_eq!(dominant(["123"]), None);
    }
}
