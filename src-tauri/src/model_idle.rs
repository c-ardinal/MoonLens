//! Frees models nobody uses.
//!
//! The translation model, the general model (explain / summarize) and
//! the text recogniser stay loaded after their first use, which holds
//! gigabytes of (graphics) memory while MoonLens sits in the tray.
//! This module frees each of them once it went unused for
//! `performance.model_idle_unload_minutes`; the next request loads it
//! again.
//!
//! "Used" is more than "running": an open translation window or running
//! subtitles wait for the screen or the sound to change, and must not
//! pay a reload when it does. While either is open, what it needs
//! counts as used. Opening one loads its models ahead of the first
//! reading (see [`crate::pipeline::Pipeline::preload_for_reading`]).
//!
//! The speech model is not handled here: it is freed as soon as
//! subtitles stop (see [`crate::subtitle`]).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::lens::LensRegistryHandle;
use crate::pipeline::Pipeline;
use crate::providers::{ocr_ocrs, ocr_ppocr, translate_llama};
use crate::settings::SettingsStore;
use crate::subtitle::SubtitleService;

/// How often the models are looked at. The setting counts in minutes,
/// so a model is freed at most this much later than asked.
const TICK: Duration = Duration::from_secs(20);

/// What is open and waiting for work right now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Waiting {
    /// Translation windows: need the text recogniser and the
    /// translation model.
    pub windows: bool,
    /// Subtitles: need the translation model.
    pub subtitles: bool,
}

/// The setting as a duration; `None` keeps models loaded.
pub fn limit(minutes: u32) -> Option<Duration> {
    (minutes > 0).then(|| Duration::from_secs(u64::from(minutes) * 60))
}

/// One look at the models: what `waiting` needs counts as used now,
/// then whatever went unused for `limit` is freed.
pub fn sweep(limit: Option<Duration>, waiting: Waiting, translation_model: Option<&Path>) {
    if waiting.windows || waiting.subtitles {
        if let Some(path) = translation_model {
            translate_llama::touch(path);
        }
    }
    if waiting.windows {
        ocr_ppocr::touch();
        ocr_ocrs::touch();
    }
    let Some(limit) = limit else {
        return;
    };
    for path in translate_llama::unload_idle(limit) {
        tracing::info!("freed unused model {}", path.display());
    }
    if ocr_ppocr::unload_idle(limit) {
        tracing::info!("freed unused text recogniser (PP-OCR)");
    }
    if ocr_ocrs::unload_idle(limit) {
        tracing::info!("freed unused text recogniser (ocrs)");
    }
}

/// Look at the models every [`TICK`] for the rest of the run.
pub fn spawn(
    settings: Arc<SettingsStore>,
    pipeline: Arc<Pipeline>,
    lenses: LensRegistryHandle,
    subtitle: Arc<SubtitleService>,
) {
    let run = move || loop {
        std::thread::sleep(TICK);
        let minutes = settings.snapshot().performance.model_idle_unload_minutes;
        let waiting = Waiting {
            windows: !lenses.list().is_empty(),
            subtitles: subtitle.is_running(),
        };
        // Freeing waits for nothing, but dropping gigabytes takes a
        // moment: a thread of its own, not the async runtime.
        sweep(
            limit(minutes),
            waiting,
            pipeline.local_llm_path().as_deref(),
        );
    };
    if let Err(e) = std::thread::Builder::new()
        .name("model-idle".into())
        .spawn(run)
    {
        tracing::warn!("unused models will stay loaded: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_minutes_keeps_models_loaded() {
        assert_eq!(limit(0), None);
        assert_eq!(limit(10), Some(Duration::from_secs(600)));
    }
}
