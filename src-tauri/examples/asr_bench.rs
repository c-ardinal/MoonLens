//! Speech recognition smoke test / benchmark against real audio.
//!
//! ```sh
//! cargo run --release --example asr_bench -- [--model whisper-small] [--lang ja] a.wav b.wav …
//! ```
//!
//! Runs the built-in whisper.cpp provider exactly as live subtitles do
//! (model from the default data dir: download it from the app's
//! settings first) and prints one line per file: name, audio seconds,
//! recognition seconds, text. WAV files must be 16 kHz mono, PCM16 or
//! float32.

#[cfg(feature = "local-asr")]
fn main() -> anyhow::Result<()> {
    use moonlens_lib::providers::{
        asr_local::LocalWhisperProvider, Provider, TaskInput, TaskOutput,
    };
    use std::time::Instant;

    let mut model = "whisper-large-v3-turbo".to_string();
    let mut lang: Option<String> = None;
    let mut gpu = true;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--model" => model = args.next().expect("--model <id>"),
            "--lang" => lang = args.next(),
            "--cpu" => gpu = false,
            _ => files.push(a),
        }
    }
    assert!(
        !files.is_empty(),
        "usage: asr_bench [--model id] [--lang xx] [--cpu] <wav>…"
    );

    let rt = tokio::runtime::Runtime::new()?;
    let provider =
        LocalWhisperProvider::new("whisper", &serde_json::json!({ "model": model }), gpu);
    anyhow::ensure!(
        provider.is_ready(),
        "{} is not downloaded ({})",
        provider.model().label,
        provider.model_path().display()
    );
    let (mut audio, mut spent) = (0.0f32, 0.0f32);
    for f in &files {
        let pcm = read_wav_pcm16(f)?;
        let secs = pcm.len() as f32 / 32_000.0;
        let t = Instant::now();
        let out = rt.block_on(provider.run(TaskInput::Asr {
            audio_pcm16_mono_16k: pcm,
            language_hint: lang.clone(),
        }))?;
        let el = t.elapsed().as_secs_f32();
        let text = match out {
            TaskOutput::Transcript { segments } => segments
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        };
        // The first file includes the model load.
        if f != &files[0] || files.len() == 1 {
            audio += secs;
            spent += el;
        }
        let name = std::path::Path::new(f)
            .file_name()
            .unwrap()
            .to_string_lossy();
        println!("{name}\t{secs:.2}\t{el:.2}\t{text}");
    }
    if audio > 0.0 {
        eprintln!(
            "real-time factor {:.2} ({spent:.1} s for {audio:.1} s of audio)",
            spent / audio
        );
    }
    Ok(())
}

/// Minimal RIFF reader: 16 kHz mono PCM16 or float32, returned as
/// little-endian PCM16 bytes (the subtitle pipeline's format).
#[cfg(feature = "local-asr")]
fn read_wav_pcm16(path: &str) -> anyhow::Result<Vec<u8>> {
    let b = std::fs::read(path)?;
    anyhow::ensure!(
        b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WAVE",
        "not a WAV file"
    );
    let (mut fmt, mut bits, mut channels, mut rate) = (0u16, 0u16, 0u16, 0u32);
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into()?) as usize;
        let body = &b[i + 8..(i + 8 + len).min(b.len())];
        if id == b"fmt " {
            fmt = u16::from_le_bytes([body[0], body[1]]);
            channels = u16::from_le_bytes([body[2], body[3]]);
            rate = u32::from_le_bytes(body[4..8].try_into()?);
            bits = u16::from_le_bytes([body[14], body[15]]);
        } else if id == b"data" {
            anyhow::ensure!(
                rate == 16_000 && channels == 1,
                "need 16 kHz mono ({rate} Hz, {channels} ch)"
            );
            return Ok(match (fmt, bits) {
                (1, 16) => body.to_vec(),
                (3, 32) => body
                    .chunks_exact(4)
                    .flat_map(|c| {
                        let v = f32::from_le_bytes(c.try_into().unwrap());
                        ((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()
                    })
                    .collect(),
                _ => anyhow::bail!("unsupported WAV format {fmt} / {bits} bit"),
            });
        }
        i += 8 + len + (len & 1);
    }
    anyhow::bail!("no data chunk")
}

#[cfg(not(feature = "local-asr"))]
fn main() {
    eprintln!("built without the local-asr feature");
}
