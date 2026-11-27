//! Display language of everything the backend words itself: tray menu,
//! window titles, error messages, notices and settings diagnostics.
//!
//! Japanese is the source text and the lookup key. The translations live
//! in `locales/<code>.json` at the repository root, one file per language,
//! shared with the frontend (`src/lib/i18n`). `build.rs` embeds every file
//! in that folder, so a new language is a new file and nothing else:
//!
//! ```ignore
//! tr!("画像データなし");
//! tr!("読込失敗：{e}", e = e);
//! tr!("残容量不足：必要 {need} GB", need = format!("{gb:.1}"));
//! ```
//!
//! `{name}` is replaced by the argument of that name. A key may carry a
//! `#context` suffix (`"翻訳#task"`) to tell apart two uses of one source
//! text; the suffix never shows. `npm run lint:i18n` checks that every
//! key used here exists in every language file and that the arguments
//! match the placeholders.
//!
//! The language follows `appearance.ui_language` (see [`apply`]).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

// `LOCALE_FILES: &[(&str, &str)]`: language code and file contents.
include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// Language of the source text.
pub const SOURCE: &str = "ja";
/// Language for systems whose own language has no file.
const FALLBACK: &str = "en";

struct Table {
    code: &'static str,
    entries: HashMap<String, String>,
}

fn tables() -> &'static [Table] {
    static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        LOCALE_FILES
            .iter()
            .map(|(code, raw)| Table {
                code,
                entries: serde_json::from_str(raw)
                    .unwrap_or_else(|e| panic!("locales/{code}.json: {e}")),
            })
            .collect()
    })
}

/// 0: source text; n: `tables()[n - 1]`.
static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn index_of(code: &str) -> Option<usize> {
    if code.eq_ignore_ascii_case(SOURCE) {
        return Some(0);
    }
    tables()
        .iter()
        .position(|t| t.code.eq_ignore_ascii_case(code))
        .map(|i| i + 1)
}

fn code_at(index: usize) -> &'static str {
    match index {
        0 => SOURCE,
        n => tables()[n - 1].code,
    }
}

/// The setting names a language, or `auto` (as does any unknown value):
/// the OS display language when it has a file, English otherwise.
pub fn resolve(setting: &str, system: Option<&str>) -> &'static str {
    if let Some(i) = index_of(setting) {
        return code_at(i);
    }
    let primary = system.and_then(|tag| tag.split(['-', '_']).next());
    primary
        .and_then(index_of)
        .or_else(|| index_of(FALLBACK))
        .map_or(SOURCE, code_at)
}

/// Apply the setting. Returns `true` when the language changed.
pub fn apply(setting: &str) -> bool {
    set(resolve(setting, sys_locale::get_locale().as_deref()))
}

/// Returns `true` when the language changed.
pub fn set(code: &str) -> bool {
    let next = index_of(code).unwrap_or(0);
    CURRENT.swap(next, Ordering::Relaxed) != next
}

/// Code of the display language.
pub fn locale() -> &'static str {
    code_at(CURRENT.load(Ordering::Relaxed))
}

/// The source text without its `#context` suffix.
fn plain(ja: &str) -> &str {
    match ja.rfind('#') {
        Some(i)
            if ja[i + 1..].starts_with(|c: char| c.is_ascii_lowercase())
                && ja[i + 1..]
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-') =>
        {
            &ja[..i]
        }
        _ => ja,
    }
}

fn text_at(index: usize, ja: &'static str) -> &'static str {
    index
        .checked_sub(1)
        .and_then(|i| tables()[i].entries.get(ja))
        .map_or_else(|| plain(ja), String::as_str)
}

/// A ready-made text in the display language.
pub fn text(ja: &'static str) -> &'static str {
    text_at(CURRENT.load(Ordering::Relaxed), ja)
}

fn fill(template: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let named = after.find('}').and_then(|close| {
            let value = args.iter().find(|(name, _)| *name == &after[..close])?;
            Some((close, &value.1))
        });
        match named {
            Some((close, value)) => {
                out.push_str(value);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// [`tr!`] behind the macro.
pub fn render(ja: &'static str, args: &[(&str, String)]) -> String {
    fill(text(ja), args)
}

/// A ready-made text (catalog notes, recommendation reasons). Serializes
/// as its source text: the frontend translates it when it renders, so
/// cached catalogs follow a later language switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct Text(&'static str);

impl Text {
    pub const fn new(ja: &'static str) -> Self {
        Self(ja)
    }

    pub fn get(&self) -> &'static str {
        text(self.0)
    }
}

/// A message in the display language: the Japanese source text, then the
/// values of its `{name}` placeholders as `name = value`.
#[macro_export]
macro_rules! tr {
    ($ja:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::render(
            $ja,
            &[$((stringify!($name), ::std::string::ToString::to_string(&$value))),*],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_system_language() {
        assert_eq!(resolve("auto", Some("ja-JP")), "ja");
        assert_eq!(resolve("auto", Some("ja")), "ja");
        assert_eq!(resolve("auto", Some("en-US")), "en");
        assert_eq!(resolve("auto", Some("zh_CN")), "en");
        // "jam" is not Japanese: only the primary subtag counts.
        assert_eq!(resolve("auto", Some("jam")), "en");
        assert_eq!(resolve("auto", None), "en");
    }

    #[test]
    fn explicit_choice_wins_over_the_system_language() {
        assert_eq!(resolve("ja", Some("en-US")), "ja");
        assert_eq!(resolve("en", Some("ja-JP")), "en");
        // A language without a file behaves as `auto`.
        assert_eq!(resolve("xx", Some("ja-JP")), "ja");
    }

    #[test]
    fn every_language_file_is_loaded() {
        assert!(tables().iter().any(|t| t.code == "en"));
        let en = index_of("en").unwrap();
        assert_eq!(text_at(en, "設定"), "Settings");
        assert_eq!(text_at(0, "設定"), "設定");
    }

    #[test]
    fn context_suffix_never_shows() {
        assert_eq!(text_at(0, "翻訳#task"), "翻訳");
        assert_eq!(text_at(index_of("en").unwrap(), "翻訳#task"), "translation");
        // Unknown text falls back to the source.
        assert_eq!(text_at(index_of("en").unwrap(), "未登録#x"), "未登録");
        assert_eq!(plain("C# 言語"), "C# 言語");
        assert_eq!(plain("C#"), "C#");
    }

    #[test]
    fn placeholders_take_named_values() {
        let args = [("n", "3".to_string()), ("name", "a{n}".to_string())];
        assert_eq!(fill("{name}: {n} / {n}", &args), "a{n}: 3 / 3");
        assert_eq!(fill("{other} {", &args), "{other} {");
    }

    #[test]
    fn text_serializes_as_its_source() {
        let v = serde_json::to_value(Text::new("翻訳#task")).unwrap();
        assert_eq!(v, serde_json::json!("翻訳#task"));
    }
}
