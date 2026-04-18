//! Clipboard auto-copy policy.
//!
//! The settings UI exposes four independent toggles for automatic
//! copying: translation, source text, QR payload, and live subtitle
//! transcript. Rather than scatter that policy across IPC handlers, we
//! centralise it in [`auto_copy`] and let each call site say *which*
//! kind of content it just produced.

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::settings::schema::ClipboardSettings;

/// Semantic tag for each thing we might want to put on the clipboard.
/// Matches the user-facing setting names one-to-one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Translation,
    Source,
    Qr,
    Transcript,
}

/// Decide whether the given [`Kind`] should be copied given the user's
/// settings. Exposed independently so call sites and tests can share it.
pub fn should_copy(kind: Kind, s: &ClipboardSettings) -> bool {
    match kind {
        Kind::Translation => s.auto_copy_translation,
        Kind::Source => s.auto_copy_source,
        Kind::Qr => s.auto_copy_qr,
        Kind::Transcript => s.auto_copy_transcript,
    }
}

/// Honour the clipboard policy for `kind`. No-op when the content is
/// empty or when the relevant setting is off.
pub fn auto_copy(app: &AppHandle, kind: Kind, content: &str, s: &ClipboardSettings) {
    if content.is_empty() || !should_copy(kind, s) {
        return;
    }
    if let Err(e) = app.clipboard().write_text(content.to_string()) {
        tracing::warn!("clipboard write failed for {kind:?}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> ClipboardSettings {
        ClipboardSettings::default()
    }

    #[test]
    fn defaults_copy_translation_and_qr_only() {
        // Capture the default policy so future changes are deliberate.
        let s = base();
        assert!(should_copy(Kind::Translation, &s));
        assert!(should_copy(Kind::Qr, &s));
        assert!(!should_copy(Kind::Source, &s));
        assert!(!should_copy(Kind::Transcript, &s));
    }

    #[test]
    fn disabling_a_single_kind_only_affects_that_kind() {
        let mut s = base();
        s.auto_copy_translation = false;
        assert!(!should_copy(Kind::Translation, &s));
        assert!(should_copy(Kind::Qr, &s));
    }

    #[test]
    fn enabling_source_and_transcript_is_independent() {
        let mut s = base();
        s.auto_copy_source = true;
        s.auto_copy_transcript = true;
        assert!(should_copy(Kind::Source, &s));
        assert!(should_copy(Kind::Transcript, &s));
        // Unchanged kinds keep their defaults.
        assert!(should_copy(Kind::Translation, &s));
    }
}
