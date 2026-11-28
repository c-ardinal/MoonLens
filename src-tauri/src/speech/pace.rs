//! Speed of read-aloud that keeps up with the original speech.
//!
//! A subtitle window asks for the translation of each piece of speech
//! it heard, with the length of that piece ([`super::Utterance::source_ms`]).
//! Read at one fixed speed, a translation longer to say than its
//! original makes the speech fall further behind with every piece. So
//! each request is spoken at the speed that fits it into the time its
//! original took ([`rate`]), a little faster while speech still trails
//! the subtitles, and never faster than [`MAX_RATE`].
//!
//! How long a text takes at the voice's own speed is learnt from the
//! requests spoken so far ([`Speeds`]): voices differ too much for one
//! fixed figure per language.

use std::collections::HashMap;
use std::time::Duration;

/// Fastest speed set by pacing (1.0 = the voice's own). Faster than
/// this, synthesized speech stops being easy to follow. A faster speed
/// in the settings is kept.
pub const MAX_RATE: f32 = 1.8;

/// Delay behind the subtitles (from the request to the start of its
/// speech) that is left as it is. Beyond it, speech catches up.
pub const LAG_OK: Duration = Duration::from_millis(1500);

/// A request that waited longer than this is dropped when a newer one
/// of the same window waits behind it: speaking it would only keep the
/// speech behind.
pub const STALE: Duration = Duration::from_secs(8);

/// Share of the delay beyond [`LAG_OK`] that one request makes up.
const CATCH_UP: f32 = 0.5;

/// A request is given at least this share of its original's length,
/// however far speech trails.
const MIN_SHARE: f32 = 0.5;

/// Shortest time given to a request, in milliseconds.
const MIN_BUDGET_MS: f32 = 300.0;

/// Weight of the newest measurement in a voice's learnt speed.
const LEARN: f32 = 0.3;

/// Measurements shorter than this say more about the engine's start-up
/// than about the voice.
const MIN_MEASURED: Duration = Duration::from_millis(400);

/// What is counted to estimate how long a text takes: its letters,
/// digits and ideographs (spaces and punctuation are spoken as pauses
/// only).
pub fn units(text: &str) -> usize {
    text.chars().filter(|c| c.is_alphanumeric()).count()
}

/// Units per second of a voice of the language at its own speed,
/// before anything was measured.
pub fn default_speed(lang: &str) -> f32 {
    match super::primary(lang).as_str() {
        // Kana and kanji: about 7 to 8 per second.
        "ja" => 7.5,
        // One syllable per character.
        "zh" | "yue" => 4.5,
        "ko" => 6.0,
        "th" => 9.0,
        // Alphabetic scripts: about 160 words a minute.
        _ => 13.0,
    }
}

/// Learnt speed of each voice, in units per second at speed 1.0.
#[derive(Debug, Default)]
pub struct Speeds(HashMap<String, f32>);

impl Speeds {
    pub fn speed(&self, voice: &str, lang: &str) -> f32 {
        self.0
            .get(voice)
            .copied()
            .unwrap_or_else(|| default_speed(lang))
    }

    /// Milliseconds `units` take at the voice's own speed.
    pub fn natural_ms(&self, voice: &str, lang: &str, units: usize) -> f32 {
        units as f32 * 1000.0 / self.speed(voice, lang)
    }

    /// A request of `units` spoken at `rate` took `took` to the end.
    pub fn learn(&mut self, voice: &str, lang: &str, units: usize, rate: f32, took: Duration) {
        if units == 0 || took < MIN_MEASURED || rate <= 0.0 {
            return;
        }
        let measured = units as f32 / took.as_secs_f32() / rate;
        let seed = default_speed(lang);
        // An engine that stalled or skipped part of the text says
        // nothing about the voice.
        if !(seed / 3.0..=seed * 3.0).contains(&measured) {
            return;
        }
        let known = self.speed(voice, lang);
        let learnt = if self.0.contains_key(voice) {
            known + (measured - known) * LEARN
        } else {
            // The first measurement of a voice replaces the guess.
            measured
        };
        self.0.insert(voice.to_string(), learnt);
    }
}

/// Speed for a request whose text takes `natural_ms` at the voice's own
/// speed, whose original speech lasted `source_ms`, and whose speech
/// starts `lag` after it was asked for. `base` = the speed of the
/// settings, the slowest one used.
pub fn rate(base: f32, natural_ms: f32, source_ms: u64, lag: Duration) -> f32 {
    let source = source_ms as f32;
    let behind = lag.saturating_sub(LAG_OK).as_millis() as f32 * CATCH_UP;
    let budget = (source - behind).max(source * MIN_SHARE).max(MIN_BUDGET_MS);
    let needed = natural_ms / budget;
    needed.max(base).min(MAX_RATE.max(base))
}
