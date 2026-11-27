//! Declarative upgrade rules for `config.toml`.
//!
//! When a setting is renamed or retired, or a provider id changes, add
//! one line here; `settings::validate` applies it to every user file at
//! load and tells the user what it changed. Rules are certain by
//! construction (a developer wrote them), so they run without asking.
//! Anything else wrong in the file (typos, wrong types, out-of-range
//! values, providers that are not registered) is only *proposed* to the
//! user; see `validate::check_schema` and `validate::check_providers`.
//!
//! Paths are dotted schema paths (`section.key`); provider ids may
//! contain dots and are matched whole.

/// A key that moved. The old value is kept unless the file already sets
/// the new key.
pub struct Renamed {
    pub from: &'static str,
    pub to: &'static str,
}

/// A key that no longer exists. `note` says why, for the log.
pub struct Removed {
    pub path: &'static str,
    pub note: &'static str,
}

/// A provider id that was replaced. Chains are rewritten; the old id's
/// `[providers.configs.<old>]` blob belonged to the old engine and is
/// dropped (the new id uses its defaults).
pub struct ProviderAlias {
    pub old: &'static str,
    pub new: &'static str,
    /// Limit to these task chains (`providers.<task>`); empty = every
    /// chain. A task-specific entry wins over a general one.
    pub tasks: &'static [&'static str],
}

pub struct Rules {
    pub renamed: &'static [Renamed],
    pub removed: &'static [Removed],
    pub provider_aliases: &'static [ProviderAlias],
}

pub const RULES: Rules = Rules {
    renamed: &[
        // Pre-release builds kept the language pair under `translation`;
        // the subtitle section owns it now and the home screen reads it.
        Renamed {
            from: "translation.default_source_language",
            to: "subtitle.source_language",
        },
        Renamed {
            from: "translation.default_target_language",
            to: "subtitle.target_language",
        },
    ],
    removed: &[
        Removed {
            path: "hotkeys.stop_all",
            note: "pre-release panic shortcut, not carried over",
        },
        Removed {
            path: "performance.lens_auto_poll_ms",
            note: "pre-release lens polling, replaced by region_watcher_poll_ms",
        },
        Removed {
            path: "performance.cache_ttl_seconds",
            note: "cached translations no longer expire; they are replaced when the engine setup or a glossary term changes",
        },
        Removed {
            path: "providers.user_tier",
            note: "pre-release translation router tier",
        },
        Removed {
            path: "providers.glossary_min_lines",
            note: "pre-release lens glossary cut-off",
        },
    ],
    provider_aliases: &[
        // Pre-release OCR engine; PP-OCRv5 on rten replaced it.
        ProviderAlias {
            old: "paddle",
            new: "ppocr",
            tasks: &[],
        },
        // Pre-release embedded LLM; the built-in llama.cpp engine. Its
        // general model serves explain / summarize (PLaMo only translates).
        ProviderAlias {
            old: "embedded.llm",
            new: "llamacpp.assistant",
            tasks: &["explain", "summarize"],
        },
        ProviderAlias {
            old: "embedded.llm",
            new: "llamacpp",
            tasks: &[],
        },
    ],
};
