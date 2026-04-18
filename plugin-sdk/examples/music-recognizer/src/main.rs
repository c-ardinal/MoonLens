//! Music recognizer sidecar plugin for MoonLens.
//!
//! Accepts a chunk of loopback audio (16 kHz mono PCM16) via the `Asr`
//! task input, wraps it as a WAV, and POSTs it to the AudD recognize
//! endpoint. The recognized track metadata comes back as a single
//! `Text` task output formatted as
//! `<artist> — <title> (<album>)`.
//!
//! The expected `plugin.toml`:
//!
//! ```toml
//! [permissions]
//! network = ["https://api.audd.io"]
//! audio_capture = true
//! env = ["AUDD_API_TOKEN"]
//! ```
//!
//! The plugin looks at `AUDD_API_TOKEN` at startup; an empty token falls
//! back to the public free-tier rate limits.

mod wav;

use anyhow::Result;
use moonlens_plugin_sdk::{Capability, Provider, ProviderError, TaskInput, TaskOutput};
use serde::Deserialize;

pub struct MusicRecognizer {
    token: String,
}

impl MusicRecognizer {
    pub fn from_env() -> Self {
        Self {
            token: std::env::var("AUDD_API_TOKEN").unwrap_or_default(),
        }
    }
}

impl Provider for MusicRecognizer {
    fn capabilities(&self) -> &[Capability] {
        &[Capability::Asr]
    }

    fn run(&self, input: TaskInput) -> Result<TaskOutput, ProviderError> {
        let TaskInput::Asr { audio_pcm16_mono_16k, .. } = input else {
            return Err(ProviderError::Other(
                "music-recognizer only accepts Asr input".into(),
            ));
        };
        if audio_pcm16_mono_16k.is_empty() {
            return Err(ProviderError::Other("empty audio chunk".into()));
        }
        let samples = bytes_to_i16_le(&audio_pcm16_mono_16k);
        let wav = wav::encode_pcm16_mono_16k(&samples);
        recognize(&self.token, &wav).map(TaskOutput::Text)
    }
}

#[derive(Deserialize)]
struct AudDResponse {
    status: String,
    result: Option<AudDTrack>,
}

#[derive(Deserialize)]
struct AudDTrack {
    artist: Option<String>,
    title: Option<String>,
    album: Option<String>,
}

/// Send a WAV to AudD and format the first match.
fn recognize(token: &str, wav: &[u8]) -> Result<String, ProviderError> {
    // Build a multipart body by hand so we do not pull in a heavy dep.
    let boundary = "moonlens-boundary-ebc14c2b";
    let mut body: Vec<u8> = Vec::with_capacity(wav.len() + 512);
    let header = |name: &str| {
        format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n")
    };
    body.extend_from_slice(header("api_token").as_bytes());
    body.extend_from_slice(token.as_bytes());
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"chunk.wav\"\r\nContent-Type: audio/wav\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(wav);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    let resp = ureq::post("https://api.audd.io/")
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .timeout(std::time::Duration::from_secs(12))
        .send_bytes(&body)
        .map_err(|e| match e {
            ureq::Error::Transport(t) => {
                let msg = t.to_string();
                if msg.contains("dns") || msg.contains("network") {
                    ProviderError::Offline
                } else {
                    ProviderError::Other(msg)
                }
            }
            other => ProviderError::Other(other.to_string()),
        })?;
    let body: AudDResponse = resp
        .into_json()
        .map_err(|e| ProviderError::Other(e.to_string()))?;
    if body.status != "success" {
        return Err(ProviderError::Other(format!(
            "AudD status: {}",
            body.status
        )));
    }
    let track = body.result.ok_or(ProviderError::LowConfidence)?;
    Ok(format_track(&track))
}

/// Decode a little-endian PCM16 byte buffer into samples. Truncates the
/// tail if the input length is odd.
pub fn bytes_to_i16_le(bytes: &[u8]) -> Vec<i16> {
    let pairs = bytes.len() / 2;
    let mut out = Vec::with_capacity(pairs);
    for i in 0..pairs {
        out.push(i16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]));
    }
    out
}

fn format_track(t: &AudDTrack) -> String {
    let artist = t.artist.clone().unwrap_or_else(|| "?".into());
    let title = t.title.clone().unwrap_or_else(|| "?".into());
    match t.album.as_deref() {
        Some(album) if !album.is_empty() => format!("{artist} — {title} ({album})"),
        _ => format!("{artist} — {title}"),
    }
}

fn main() -> anyhow::Result<()> {
    moonlens_plugin_sdk::run_sidecar(MusicRecognizer::from_env())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_track_includes_album_when_present() {
        let t = AudDTrack {
            artist: Some("Miles Davis".into()),
            title: Some("So What".into()),
            album: Some("Kind of Blue".into()),
        };
        assert_eq!(format_track(&t), "Miles Davis — So What (Kind of Blue)");
    }

    #[test]
    fn format_track_drops_empty_album() {
        let t = AudDTrack {
            artist: Some("Artist".into()),
            title: Some("Title".into()),
            album: Some(String::new()),
        };
        assert_eq!(format_track(&t), "Artist — Title");
    }

    #[test]
    fn format_track_tolerates_missing_fields() {
        let t = AudDTrack {
            artist: None,
            title: None,
            album: None,
        };
        assert_eq!(format_track(&t), "? — ?");
    }

    #[test]
    fn empty_audio_short_circuits() {
        let p = MusicRecognizer {
            token: String::new(),
        };
        let res = p.run(TaskInput::Asr {
            audio_pcm16_mono_16k: vec![],
            language_hint: None,
        });
        assert!(matches!(res, Err(ProviderError::Other(_))));
    }

    #[test]
    fn bytes_to_i16_decodes_little_endian_samples() {
        // 0x1234 → 0x34 0x12; -1 (0xFFFF) → 0xFF 0xFF.
        let bytes = vec![0x34, 0x12, 0xFF, 0xFF];
        assert_eq!(bytes_to_i16_le(&bytes), vec![0x1234, -1]);
    }

    #[test]
    fn bytes_to_i16_drops_trailing_odd_byte() {
        let bytes = vec![0x01, 0x00, 0x02];
        assert_eq!(bytes_to_i16_le(&bytes), vec![1]);
    }

    #[test]
    fn non_asr_input_is_rejected() {
        let p = MusicRecognizer {
            token: String::new(),
        };
        let res = p.run(TaskInput::Translate {
            text: "hi".into(),
            source_language: "en".into(),
            target_language: "ja".into(),
        });
        assert!(matches!(res, Err(ProviderError::Other(_))));
    }
}
