//! Strongly-typed settings schema.
//!
//! Ranges (`#[schemars(range)]`) match the settings screen and are
//! checked by `settings::validate` (hand-edited values only get a
//! proposed fix; loading never fails on them).
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
    /// Format version of the user file (see `settings::migrate`).
    pub config_version: u32,
    pub appearance: Appearance,
    pub hotkeys: Hotkeys,
    pub overlay: OverlaySettings,
    pub subtitle: SubtitleSettings,
    pub clipboard: ClipboardSettings,
    pub translation: TranslationSettings,
    pub privacy: PrivacySettings,
    pub storage: StorageSettings,
    pub performance: PerformanceSettings,
    pub startup: StartupSettings,
    pub speech: SpeechSettings,
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
    /// Whether the first-run setup (hardware check, recommended models,
    /// bulk download) was finished or skipped.
    pub setup_completed: bool,
    /// Ids of model licences the user explicitly accepted before a
    /// download (see `providers::llm_models::require_consent`). No
    /// duplicates; ids are never removed.
    pub accepted_licenses: Vec<String>,
}

/* ------------------------------- appearance ------------------------------ */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub theme: Theme,
    pub accent: String,
    pub density: Density,
    /// Text size of the main window's body screens (history, settings,
    /// text/image translation, extensions). Home and page titles keep
    /// their size; window translation keeps the source layout.
    pub text_size: TextSize,
    /// Display language of every window, the tray menu and messages:
    /// `auto` (the OS display language when `locales/` has it, otherwise
    /// English), `ja`, or the code of a file in `locales/`.
    pub ui_language: String,
    pub font_family_ui: String,
    pub font_family_mono: String,
    pub reduced_motion: bool,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            accent: "#FFB547".into(),
            density: Density::Normal,
            text_size: TextSize::default(),
            ui_language: default_ui_language(),
            font_family_ui: "Inter Variable".into(),
            font_family_mono: "JetBrains Mono".into(),
            reduced_motion: false,
        }
    }
}

pub fn default_ui_language() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    #[default]
    Dark,
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

/// Body text size of the main window. The pixel font is drawn at 16 dots,
/// so each step is a whole number of CSS px: 16 / 20 / 24 / 28 / 32.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextSize {
    Xsmall,
    Small,
    #[default]
    Medium,
    Large,
    Xlarge,
}

/* --------------------------------- hotkeys -------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Hotkeys {
    /// Open one more window-translation window under the mouse pointer.
    pub capture_region: String,
    /// Unused (the full-screen selector is gone: a maximized window
    /// translates the whole screen); kept so existing configs still load.
    pub capture_window: String,
    /// Window translation: open a window when there is none, otherwise
    /// close them all.
    pub toggle_pin_overlay: String,
    /// Window translation: switch every window between letting the mouse
    /// through (default) and taking it (select / copy the translation).
    pub toggle_window_operate: String,
    /// Window translation: stop reading and translating in every window
    /// (the windows stay), or resume.
    pub toggle_window_pause: String,
    /// Window translation: while held, every window hides its
    /// translations so the original on screen can be read.
    pub hold_window_original: String,
    pub toggle_subtitle: String,
    pub open_command_palette: String,
    pub open_history: String,
    pub reverse_image_search: String,
    /// Free-form additional bindings keyed by plugin/action id.
    pub custom: BTreeMap<String, String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            capture_region: "CommandOrControl+Shift+Space".into(),
            capture_window: "CommandOrControl+Shift+W".into(),
            toggle_pin_overlay: "CommandOrControl+Shift+P".into(),
            toggle_window_operate: "CommandOrControl+Shift+E".into(),
            toggle_window_pause: "CommandOrControl+Shift+X".into(),
            hold_window_original: "CommandOrControl+Shift+O".into(),
            toggle_subtitle: "CommandOrControl+Shift+S".into(),
            open_command_palette: "CommandOrControl+K".into(),
            open_history: "CommandOrControl+Shift+H".into(),
            reverse_image_search: "CommandOrControl+Shift+G".into(),
            custom: BTreeMap::new(),
        }
    }
}

/* -------------------------------- overlay -------------------------------- */

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct OverlaySettings {
    #[schemars(range(min = 240, max = 480))]
    pub max_width_px: u32,
    #[schemars(range(min = 0.5, max = 1.0))]
    pub opacity: f32,
    #[schemars(range(min = 0, max = 40))]
    pub blur_px: u32,
    pub saturation: f32,
    #[schemars(range(min = 10, max = 28))]
    pub font_size_px: u32,
    #[schemars(range(min = 1.1, max = 2.0))]
    pub line_height: f32,
    /// Unused since live translation became continuous window
    /// translation (kept so existing config files still load).
    pub follow_source: bool,
    pub auto_hide_ms: Option<u32>,
    pub position: OverlayPosition,
    /// Translation windows appear in screenshots and recordings
    /// (Windows; they are excluded only while MoonLens reads the
    /// screen). Off: excluded from every capture.
    pub show_in_screenshots: bool,
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
            show_in_screenshots: true,
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
    #[schemars(range(min = 0, max = 3))]
    pub vad_aggressiveness: u8,
    #[schemars(range(min = 1.0, max = 10.0))]
    pub chunk_seconds: f32,
    #[schemars(range(min = 0.0, max = 2.0))]
    pub overlap_seconds: f32,
    pub show_original: bool,
    #[schemars(range(min = 14, max = 48))]
    pub font_size_px: u32,
    /// Translate each sentence again as a whole as soon as it ends.
    pub realtime_correction: bool,
    /// Translate the sentences not yet corrected again when the speech
    /// pauses or the audio ends.
    pub batch_correction: bool,
    /// Remove stock misrecognitions (video sign-offs, subtitle credits,
    /// sound annotations) before they are shown or translated; see
    /// `subtitle_filter`.
    pub block_stock_phrases: bool,
    /// The user's additions to the bundled list of stock phrases. Only
    /// ever changed by the user (settings screen, or approving a
    /// suggestion in the subtitle window).
    pub blocked_phrases: Vec<String>,
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
            realtime_correction: true,
            batch_correction: true,
            block_stock_phrases: true,
            blocked_phrases: Vec::new(),
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

/* ------------------------------- translation ----------------------------- */

/// User preferences applied to every translation, whichever local model
/// runs it (see `providers::glossary`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TranslationSettings {
    /// Register of Japanese output. Other target languages ignore it.
    pub style: JaStyle,
    /// Fixed renderings for terms. An entry is used only when its source
    /// term occurs in the text and its `lang` matches the target.
    pub glossary: Vec<GlossaryEntry>,
    /// Glossaries of single applications: used on top of `glossary`
    /// for what is translated from that application (a window attached
    /// to one of its windows, its audio).
    pub app_glossaries: Vec<AppGlossary>,
}

/// The glossary of one application.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct AppGlossary {
    /// Key of the application (`app_identity::normalize_key`): the name
    /// of its executable, lowercased, without `.exe`.
    pub app: String,
    /// Name shown for the application.
    pub name: String,
    /// Where its entries and the common ones name the same term, these
    /// win.
    pub glossary: Vec<GlossaryEntry>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JaStyle {
    /// Leave the register to the model (follows the source).
    #[default]
    Auto,
    /// です・ます調
    Polite,
    /// だ・である調
    Plain,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct GlossaryEntry {
    /// Term as it appears in the source text.
    pub source: String,
    /// Rendering to use in the translation.
    pub target: String,
    /// Target language the rendering is written in (e.g. "ja");
    /// empty = any target language.
    pub lang: String,
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
            allow_cloud_globally: false,
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
    /// Keep translations on disk so text seen before shows at once after
    /// a restart too. Off: they are kept until the app exits.
    pub keep_translation_cache: bool,
    #[schemars(range(min = 0, max = 3650))]
    pub retention_days: u32,
    #[schemars(range(min = 0, max = 1000000))]
    pub max_entries: u32,
    pub encrypt_at_rest: bool,
    pub save_capture_images: bool,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            keep_history: true,
            keep_translation_cache: true,
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
    #[schemars(range(min = 100, max = 2000))]
    pub region_watcher_poll_ms: u32,
    /// Width (CSS pixels) a translation window reads beyond its frame
    /// on every side, so text the frame cuts is read whole. Translations
    /// stay inside the window. 0: read the window only.
    #[schemars(range(min = 0, max = 128))]
    pub scan_margin_px: u32,
    #[schemars(range(min = 1, max = 8))]
    pub max_concurrent_tasks: u32,
    pub use_gpu_if_available: bool,
    /// Minutes without use after which loaded models are freed
    /// (0 = keep them loaded).
    #[schemars(range(min = 0, max = 240))]
    pub model_idle_unload_minutes: u32,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            streaming_debounce_ms: 50,
            region_watcher_poll_ms: 250,
            scan_margin_px: 32,
            max_concurrent_tasks: 4,
            use_gpu_if_available: true,
            model_idle_unload_minutes: 10,
        }
    }
}

/* --------------------------------- startup ------------------------------- */

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct StartupSettings {
    /// Start MoonLens (to the tray) when the user logs in to the OS.
    /// Mirrored to a per-user login item by `autostart.rs`.
    pub launch_at_login: bool,
}

/* ---------------------------------- speech -------------------------------- */

/// Read-aloud of translations with the voices of the OS (`speech`).
/// Which windows read aloud is chosen per window, not here.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SpeechSettings {
    /// Id of the preferred voice, used for text in its language. Empty:
    /// the first installed voice of the language.
    pub voice: String,
    /// Speed, 1.0 = the voice's own.
    #[schemars(range(min = 0.5, max = 2.0))]
    pub rate: f32,
    #[schemars(range(min = 0, max = 100))]
    pub volume_percent: u32,
}

impl Default for SpeechSettings {
    fn default() -> Self {
        Self {
            voice: String::new(),
            rate: 1.0,
            volume_percent: 100,
        }
    }
}

/* -------------------------------- providers ------------------------------ */

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TaskChain {
    pub chain: Vec<String>,
    pub parallel_race: bool,
    pub fallback_on: Vec<FallbackCondition>,
    #[schemars(range(min = 100, max = 3600000))]
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
