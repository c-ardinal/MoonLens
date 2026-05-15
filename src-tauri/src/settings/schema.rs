//! Strongly-typed settings schema.
//!
//! `#[derive(JsonSchema)]` powers the auto-generated settings UI on the
//! frontend; `#[serde(default)]` lets partial user TOMLs merge cleanly over
//! the embedded defaults.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub appearance: Appearance,
    pub hotkeys: Hotkeys,
    pub overlay: OverlaySettings,
    pub subtitle: SubtitleSettings,
    pub clipboard: ClipboardSettings,
    pub privacy: PrivacySettings,
    pub storage: StorageSettings,
    pub performance: PerformanceSettings,
    pub providers: Providers,
    pub plugins: PluginSettings,
    pub onboarding: OnboardingState,
    /// Active profile name; profiles are additional TOML files that overlay.
    pub active_profile: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct OnboardingState {
    /// Whether the user has completed (or skipped) the first-run permission
    /// walk-through on macOS. Other platforms always store `true` so the
    /// flag is a no-op there.
    pub macos_permissions_reviewed: bool,
    /// Whether the user has been shown the embedded LLM setup
    /// prompt. Stays `false` until the first-run modal is either
    /// dismissed ("後で") or completed (download finished). Once `true`
    /// the modal never reappears, even if the model file is later
    /// removed — at that point the Settings → Providers card is the
    /// authoritative status / download surface.
    pub embedded_model_setup_seen: bool,
}

/* ------------------------------- appearance ------------------------------ */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub theme: Theme,
    pub accent: String,
    pub density: Density,
    pub ui_language: String,
    pub font_family_ui: String,
    pub font_family_mono: String,
    pub reduced_motion: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            accent: "#8B5CF6".into(),
            density: Density::Normal,
            ui_language: "ja".into(),
            font_family_ui: "Inter Variable".into(),
            font_family_mono: "JetBrains Mono".into(),
            reduced_motion: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum Density {
    Compact,
    #[default]
    Normal,
    Comfortable,
}

/* --------------------------------- hotkeys -------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Hotkeys {
    pub capture_region: String,
    pub capture_window: String,
    pub toggle_pin_overlay: String,
    pub toggle_subtitle: String,
    pub open_command_palette: String,
    pub open_history: String,
    pub reverse_image_search: String,
    /// Global "panic" shortcut — tears down every Lens window, halts
    /// the subtitle pipeline, and hides the overlay. Useful when
    /// output floods the screen and the user just wants quiet.
    pub stop_all: String,
    /// Free-form additional bindings keyed by plugin/action id.
    pub custom: BTreeMap<String, String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            capture_region: "CommandOrControl+Shift+Space".into(),
            capture_window: "CommandOrControl+Shift+W".into(),
            toggle_pin_overlay: "CommandOrControl+Shift+P".into(),
            toggle_subtitle: "CommandOrControl+Shift+S".into(),
            open_command_palette: "CommandOrControl+K".into(),
            open_history: "CommandOrControl+Shift+H".into(),
            reverse_image_search: "CommandOrControl+Shift+G".into(),
            stop_all: "CommandOrControl+Alt+Escape".into(),
            custom: BTreeMap::new(),
        }
    }
}

/* -------------------------------- overlay -------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct OverlaySettings {
    pub max_width_px: u32,
    pub opacity: f32,
    pub blur_px: u32,
    pub saturation: f32,
    pub font_size_px: u32,
    pub line_height: f32,
    pub follow_source: bool,
    pub auto_hide_ms: Option<u32>,
    pub position: OverlayPosition,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            max_width_px: 480,
            opacity: 0.92,
            blur_px: 24,
            saturation: 1.8,
            font_size_px: 14,
            line_height: 1.5,
            follow_source: true,
            auto_hide_ms: None,
            position: OverlayPosition::NearSource,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum OverlayPosition {
    #[default]
    NearSource,
    FollowCursor,
    Fixed,
}

/* -------------------------------- subtitle ------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SubtitleSettings {
    pub source_language: String,
    pub target_language: String,
    pub model: String,
    pub fallback_model: String,
    pub vad_aggressiveness: u8,
    pub chunk_seconds: f32,
    pub overlap_seconds: f32,
    pub show_original: bool,
    pub font_size_px: u32,
}

impl Default for SubtitleSettings {
    fn default() -> Self {
        Self {
            source_language: "auto".into(),
            target_language: "ja".into(),
            model: "large-v3-turbo".into(),
            fallback_model: "base".into(),
            vad_aggressiveness: 2,
            chunk_seconds: 3.0,
            overlap_seconds: 0.5,
            show_original: true,
            font_size_px: 22,
        }
    }
}

/* ------------------------------- clipboard ------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ClipboardSettings {
    pub auto_copy_translation: bool,
    pub auto_copy_source: bool,
    pub auto_copy_qr: bool,
    pub auto_copy_transcript: bool,
}

impl Default for ClipboardSettings {
    fn default() -> Self {
        Self {
            auto_copy_translation: true,
            auto_copy_source: false,
            auto_copy_qr: true,
            auto_copy_transcript: false,
        }
    }
}

/* -------------------------------- privacy -------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PrivacySettings {
    /// Global kill switch: if false, any cloud provider is refused regardless
    /// of per-task `allow_cloud`.
    pub allow_cloud_globally: bool,
    pub show_privacy_indicator: bool,
    pub notify_on_fallback_to_cloud: bool,
    pub log_level: LogLevel,
    pub redact_sensitive_in_logs: bool,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            allow_cloud_globally: true,
            show_privacy_indicator: true,
            notify_on_fallback_to_cloud: true,
            log_level: LogLevel::Info,
            redact_sensitive_in_logs: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

/* --------------------------------- storage -------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct StorageSettings {
    pub keep_history: bool,
    pub retention_days: u32,
    pub max_entries: u32,
    pub encrypt_at_rest: bool,
    pub save_capture_images: bool,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            keep_history: true,
            retention_days: 90,
            max_entries: 10_000,
            encrypt_at_rest: false,
            save_capture_images: true,
        }
    }
}

/* ------------------------------- performance ----------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PerformanceSettings {
    pub streaming_debounce_ms: u32,
    pub region_watcher_poll_ms: u32,
    pub max_concurrent_tasks: u32,
    pub cache_ttl_seconds: u32,
    pub use_gpu_if_available: bool,
    /// Lens content-change auto-poll interval in milliseconds.
    /// Lens watchers fire a capture trigger on this cadence so source
    /// text that changes under a stationary window (video subtitles,
    /// chat messages) still re-translates. Hash-compare short-
    /// circuits ticks where nothing changed, so the only cost is a
    /// brief mask + capture every `N` ms when source is static. Set
    /// to 0 to disable periodic polling (event-driven only).
    pub lens_auto_poll_ms: u32,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            streaming_debounce_ms: 50,
            region_watcher_poll_ms: 250,
            max_concurrent_tasks: 4,
            cache_ttl_seconds: 3600,
            use_gpu_if_available: true,
            lens_auto_poll_ms: 1500,
        }
    }
}

/* -------------------------------- providers ------------------------------ */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Providers {
    pub translate: TaskChain,
    pub ocr: TaskChain,
    pub asr: TaskChain,
    pub explain: TaskChain,
    pub summarize: TaskChain,
    pub reverse_image_search: TaskChain,
    pub qr_handler: TaskChain,
    pub intent_classifier: TaskChain,
    /// Per-provider configuration blobs, keyed by the provider id that
    /// appears in a chain (e.g. "ollama.gemma4", "deepl", "plugin.<id>").
    ///
    /// Stored as `serde_json::Value` so we can round-trip through both TOML
    /// (for on-disk persistence) and JSON Schema (for the auto-generated
    /// settings UI). Provider constructors receive a `&serde_json::Value`.
    pub configs: BTreeMap<String, serde_json::Value>,
    /// User-facing translation tier. Drives `RoutedTranslator`
    /// model selection (which 1–2 GGUFs the embedded provider
    /// loads) and the per-language routing matrix.
    ///
    /// `None` is still accepted for backwards-compat readability
    /// (a hand-edited user.toml that omits the field), but the
    /// settings layer treats absence as Standard via
    /// `Providers::default` + the `migrate_legacy_embedded_settings`
    /// migration. The frontend picker exposes only the three
    /// `UserTier` variants — there is no "disabled" option from
    /// the UI side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_tier: Option<crate::translation_router::UserTier>,

    /// Phase 29 glossary feature. When a batch of OCR / audio lines
    /// has at least this many entries, the lens layer asks the
    /// translator chain for a glossary of proper nouns + technical
    /// terms upfront, then injects it into every per-line translate
    /// call so the same source term renders the same way each time.
    /// `0` disables the gate (every translation pays glossary
    /// latency). A large value like `u32::MAX` disables the feature.
    /// Default `5` — see `lens_pipeline::PipelineConfig::default` for
    /// the rationale on the cut-off.
    #[serde(default = "default_glossary_min_lines")]
    pub glossary_min_lines: u32,
}

fn default_glossary_min_lines() -> u32 {
    5
}

impl Default for Providers {
    fn default() -> Self {
        Self {
            translate: TaskChain::default(),
            ocr: TaskChain::default(),
            asr: TaskChain::default(),
            explain: TaskChain::default(),
            summarize: TaskChain::default(),
            reverse_image_search: TaskChain::default(),
            qr_handler: TaskChain::default(),
            intent_classifier: TaskChain::default(),
            configs: BTreeMap::new(),
            // Fresh installs default to Standard. The settings UI
            // shows only the three UserTier variants — every install
            // ends up with one of them. Pre-routing installs that
            // were saved with `user_tier = None` get migrated to a
            // tier derived from the legacy `[providers.configs.
            // "embedded.llm"]` block by
            // `migrate_legacy_embedded_settings`.
            user_tier: Some(crate::translation_router::UserTier::Standard),
            glossary_min_lines: default_glossary_min_lines(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TaskChain {
    pub chain: Vec<String>,
    pub parallel_race: bool,
    pub fallback_on: Vec<FallbackCondition>,
    pub timeout_ms: u32,
    pub allow_cloud: bool,
    pub allow_network: bool,
    pub max_cost_per_request: f32,
    pub sticky_session_ms: u32,
}

impl Default for TaskChain {
    fn default() -> Self {
        Self {
            chain: Vec::new(),
            parallel_race: false,
            fallback_on: vec![
                FallbackCondition::Timeout,
                FallbackCondition::Error,
                FallbackCondition::Offline,
            ],
            timeout_ms: 3000,
            allow_cloud: true,
            allow_network: true,
            max_cost_per_request: 0.01,
            sticky_session_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FallbackCondition {
    Timeout,
    Error,
    RateLimit,
    Offline,
    UnsupportedLanguage,
    LowConfidence,
}

/* -------------------------------- plugins -------------------------------- */

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PluginSettings {
    pub enabled: BTreeMap<String, bool>,
    pub auto_update_check: bool,
    pub require_signature: bool,
    pub trusted_publishers: Vec<String>,
}
