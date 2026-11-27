//! Display language of everything the backend words itself: tray menu,
//! window titles, error messages, notices and settings diagnostics.
//!
//! The texts live in `locales/<code>.json` at the repository root, one
//! file per language (Japanese included), shared with the frontend
//! (`src/lib/i18n`). The source refers to a text by its key. `build.rs`
//! embeds every file in that folder, so a new language is a new file and
//! nothing else:
//!
//! ```ignore
//! tr!("ipc:no_image_data");
//! tr!("ipc:read_failed", e = e);
//! ```
//!
//! `{name}` in a text is replaced by the argument of that name. A key
//! missing from the display language falls back to Japanese.
//! `npm run lint:i18n` checks that every key used here exists in every
//! language file and that the arguments match the placeholders.
//!
//! The language follows `appearance.ui_language` (see [`apply`]).

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

// `LOCALE_FILES: &[(&str, &str)]`: language code and file contents.
include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// Reference language: what a key missing elsewhere falls back to.
pub const SOURCE: &str = "ja";
/// Language for systems whose own language has no file.
const FALLBACK: &str = "en";

struct Table {
    code: &'static str,
    entries: HashMap<String, String>,
}

/// Every language file, the reference language first.
fn tables() -> &'static [Table] {
    static TABLES: OnceLock<Vec<Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        let mut tables: Vec<Table> = LOCALE_FILES
            .iter()
            .map(|(code, raw)| Table {
                code,
                entries: serde_json::from_str(raw)
                    .unwrap_or_else(|e| panic!("locales/{code}.json: {e}")),
            })
            .collect();
        tables.sort_by_key(|t| t.code != SOURCE);
        assert_eq!(tables[0].code, SOURCE, "locales/{SOURCE}.json is missing");
        tables
    })
}

/// Index into [`tables`].
static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn index_of(code: &str) -> Option<usize> {
    tables()
        .iter()
        .position(|t| t.code.eq_ignore_ascii_case(code))
}

/// The setting names a language, or `auto` (as does any unknown value):
/// the OS display language when it has a file, English otherwise.
pub fn resolve(setting: &str, system: Option<&str>) -> &'static str {
    let primary = system.and_then(|tag| tag.split(['-', '_']).next());
    let index = index_of(setting)
        .or_else(|| primary.and_then(index_of))
        .or_else(|| index_of(FALLBACK))
        .unwrap_or(0);
    tables()[index].code
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
    tables()[CURRENT.load(Ordering::Relaxed)].code
}

fn text_at(index: usize, key: &'static str) -> &'static str {
    let tables = tables();
    tables[index]
        .entries
        .get(key)
        .or_else(|| tables[0].entries.get(key))
        .map_or(key, String::as_str)
}

/// A ready-made text in the display language.
pub fn text(key: &'static str) -> &'static str {
    text_at(CURRENT.load(Ordering::Relaxed), key)
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
pub fn render(key: &'static str, args: &[(&str, String)]) -> String {
    fill(text(key), args)
}

/// A ready-made text (catalog notes, recommendation reasons). Serializes
/// as its key: the frontend looks it up when it renders, so cached
/// catalogs follow a later language switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct Text(&'static str);

impl Text {
    pub const fn new(key: &'static str) -> Self {
        Self(key)
    }

    pub fn get(&self) -> &'static str {
        text(self.0)
    }
}

/// A message in the display language: the key of its text, then the
/// values of the text's `{name}` placeholders as `name = value`.
#[macro_export]
macro_rules! tr {
    ($key:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::render(
            $key,
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
        let (ja, en) = (index_of("ja").unwrap(), index_of("en").unwrap());
        assert_eq!(ja, 0);
        assert_eq!(text_at(ja, "common:settings"), "設定");
        assert_eq!(text_at(en, "common:settings"), "Settings");
    }

    #[test]
    fn unknown_key_shows_as_is() {
        assert_eq!(
            text_at(index_of("en").unwrap(), "no:such_key"),
            "no:such_key"
        );
    }

    #[test]
    fn placeholders_take_named_values() {
        let args = [("n", "3".to_string()), ("name", "a{n}".to_string())];
        assert_eq!(fill("{name}: {n} / {n}", &args), "a{n}: 3 / 3");
        assert_eq!(fill("{other} {", &args), "{other} {");
    }

    #[test]
    fn text_serializes_as_its_key() {
        let v = serde_json::to_value(Text::new("common:translation_task")).unwrap();
        assert_eq!(v, serde_json::json!("common:translation_task"));
    }
}
