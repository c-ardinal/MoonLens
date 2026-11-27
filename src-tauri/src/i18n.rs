//! Display language of everything the backend words itself: tray menu,
//! window titles, error messages, notices and settings diagnostics.
//!
//! Japanese is the source text. [`tr!`] takes the Japanese format string
//! and its English counterpart side by side, so a message cannot be added
//! without both, and `npm run lint:copy` keeps checking the Japanese one:
//!
//! ```ignore
//! tr!("画像データなし", "No image data");
//! tr!("読込失敗：{e}", "Could not read the file: {e}");
//! tr!("残容量不足：必要 {:.1} GB", "Not enough free space: {:.1} GB needed", gb);
//! ```
//!
//! The language follows `appearance.ui_language` (see [`apply`]); the
//! frontend resolves the same setting in `src/lib/i18n`.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::settings::schema::UiLanguage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    Ja,
    En,
}

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// `auto` follows the OS display language: Japanese systems get Japanese,
/// every other system English.
pub fn resolve(setting: UiLanguage, system: Option<&str>) -> Locale {
    match setting {
        UiLanguage::Ja => Locale::Ja,
        UiLanguage::En => Locale::En,
        UiLanguage::Auto => {
            let is_ja = system.is_some_and(|tag| {
                let primary = tag.split(['-', '_']).next().unwrap_or("");
                primary.eq_ignore_ascii_case("ja")
            });
            if is_ja {
                Locale::Ja
            } else {
                Locale::En
            }
        }
    }
}

/// Apply the setting. Returns `true` when the language changed.
pub fn apply(setting: UiLanguage) -> bool {
    let locale = resolve(setting, sys_locale::get_locale().as_deref());
    set(locale)
}

/// Returns `true` when the language changed.
pub fn set(locale: Locale) -> bool {
    let english = locale == Locale::En;
    ENGLISH.swap(english, Ordering::Relaxed) != english
}

pub fn locale() -> Locale {
    if ENGLISH.load(Ordering::Relaxed) {
        Locale::En
    } else {
        Locale::Ja
    }
}

/// Pick between two ready-made texts (catalog entries, static labels).
pub fn pick<'a>(ja: &'a str, en: &'a str) -> &'a str {
    match locale() {
        Locale::Ja => ja,
        Locale::En => en,
    }
}

/// A ready-made text in both languages (catalog notes, recommendation
/// reasons). Serializes as `{ ja, en }`: the frontend picks the language
/// when it renders, so cached catalogs follow a later language switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Text {
    pub ja: &'static str,
    pub en: &'static str,
}

impl Text {
    pub const fn new(ja: &'static str, en: &'static str) -> Self {
        Self { ja, en }
    }

    pub fn get(&self) -> &'static str {
        pick(self.ja, self.en)
    }
}

/// `format!` in the display language: Japanese format string, English
/// format string, then the arguments both use.
#[macro_export]
macro_rules! tr {
    ($ja:literal, $en:literal $(, $($arg:tt)*)?) => {
        match $crate::i18n::locale() {
            $crate::i18n::Locale::Ja => format!($ja $(, $($arg)*)?),
            $crate::i18n::Locale::En => format!($en $(, $($arg)*)?),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_follows_the_system_language() {
        assert_eq!(resolve(UiLanguage::Auto, Some("ja-JP")), Locale::Ja);
        assert_eq!(resolve(UiLanguage::Auto, Some("ja")), Locale::Ja);
        assert_eq!(resolve(UiLanguage::Auto, Some("en-US")), Locale::En);
        assert_eq!(resolve(UiLanguage::Auto, Some("zh_CN")), Locale::En);
        // "jam" is not Japanese: only the primary subtag counts.
        assert_eq!(resolve(UiLanguage::Auto, Some("jam")), Locale::En);
        assert_eq!(resolve(UiLanguage::Auto, None), Locale::En);
    }

    #[test]
    fn text_serializes_both_languages() {
        let v = serde_json::to_value(Text::new("翻訳", "translation")).unwrap();
        assert_eq!(v, serde_json::json!({ "ja": "翻訳", "en": "translation" }));
    }

    #[test]
    fn explicit_choice_wins_over_the_system_language() {
        assert_eq!(resolve(UiLanguage::Ja, Some("en-US")), Locale::Ja);
        assert_eq!(resolve(UiLanguage::En, Some("ja-JP")), Locale::En);
    }
}
