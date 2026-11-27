//! Speech with the voices installed in Windows
//! (`Windows.Media.SpeechSynthesis`), played by a `MediaPlayer`.
//! Synthesis runs on this PC; nothing is sent anywhere.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use windows::core::HSTRING;
use windows::Foundation::TypedEventHandler;
use windows::Media::Core::MediaSource;
use windows::Media::Playback::MediaPlayer;
use windows::Media::SpeechSynthesis::{SpeechSynthesisStream, SpeechSynthesizer, VoiceInformation};

use super::{Engine, Tuning, Voice};

struct Playing {
    player: MediaPlayer,
    /// Set by the player when it reaches the end or fails.
    done: Arc<AtomicBool>,
}

pub struct WindowsEngine {
    playing: Option<Playing>,
}

impl Default for WindowsEngine {
    /// Call on the thread that will use the engine.
    fn default() -> Self {
        // Idempotent; the thread lives as long as the app.
        let _ = unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
            CoInitializeEx(None, COINIT_MULTITHREADED)
        };
        Self { playing: None }
    }
}

fn installed() -> windows::core::Result<Vec<VoiceInformation>> {
    Ok(SpeechSynthesizer::AllVoices()?.into_iter().collect())
}

fn describe(v: &VoiceInformation) -> windows::core::Result<Voice> {
    Ok(Voice {
        id: v.Id()?.to_string(),
        name: v.DisplayName()?.to_string(),
        lang: v.Language()?.to_string(),
    })
}

fn synthesize(text: &str, voice: &Voice, tuning: &Tuning) -> Result<SpeechSynthesisStream> {
    let info = installed()?
        .into_iter()
        .find(|v| v.Id().is_ok_and(|id| id == voice.id))
        .ok_or_else(|| anyhow!("voice {} is no longer installed", voice.name))?;
    let synth = SpeechSynthesizer::new()?;
    synth.SetVoice(&info)?;
    // Windows takes 0.5 to 6.0.
    synth
        .Options()?
        .SetSpeakingRate(f64::from(tuning.rate.clamp(0.5, 6.0)))?;
    Ok(synth
        .SynthesizeTextToStreamAsync(&HSTRING::from(text))?
        .join()?)
}

fn play(text: &str, voice: &Voice, tuning: &Tuning) -> Result<Playing> {
    let stream = synthesize(text, voice, tuning)?;
    let source = MediaSource::CreateFromStream(&stream, &stream.ContentType()?)?;
    let player = MediaPlayer::new()?;
    player.SetVolume(f64::from(tuning.volume.clamp(0.0, 1.0)))?;
    let done = Arc::new(AtomicBool::new(false));
    let ended = done.clone();
    player.MediaEnded(&TypedEventHandler::new(move |_, _| {
        ended.store(true, Ordering::Relaxed);
        Ok(())
    }))?;
    let failed = done.clone();
    player.MediaFailed(&TypedEventHandler::new(move |_, _| {
        failed.store(true, Ordering::Relaxed);
        Ok(())
    }))?;
    player.SetSource(&source)?;
    player.Play()?;
    Ok(Playing { player, done })
}

impl Engine for WindowsEngine {
    fn voices(&mut self) -> Vec<Voice> {
        match installed() {
            Ok(voices) => voices.iter().filter_map(|v| describe(v).ok()).collect(),
            Err(e) => {
                tracing::warn!("speech: installed voices could not be listed: {e}");
                Vec::new()
            }
        }
    }

    fn start(&mut self, text: &str, voice: &Voice, tuning: &Tuning) -> Result<()> {
        self.stop();
        self.playing = Some(play(text, voice, tuning)?);
        Ok(())
    }

    fn playing(&mut self) -> bool {
        self.playing
            .as_ref()
            .is_some_and(|p| !p.done.load(Ordering::Relaxed))
    }

    fn stop(&mut self) {
        if let Some(p) = self.playing.take() {
            let _ = p.player.Pause();
            let _ = p.player.Close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The OS engine itself, on any Windows with at least one voice
    /// (every edition ships one): listing and synthesis work. Playback
    /// needs an audio device, which CI has not.
    #[test]
    fn installed_voice_synthesizes_audio() {
        let mut engine = WindowsEngine::default();
        let voices = engine.voices();
        let Some(voice) = voices.first() else {
            eprintln!("no voice installed; skipped");
            return;
        };
        assert!(!voice.id.is_empty() && !voice.lang.is_empty());
        let tuning = Tuning {
            voice: String::new(),
            rate: 1.5,
            volume: 1.0,
        };
        let stream = synthesize("MoonLens", voice, &tuning).unwrap();
        assert!(stream.Size().unwrap() > 0);
    }
}
