//! Does text shown on screen help speech recognition? Runs the built-in
//! whisper.cpp recognizer over a list of jobs, each a recording plus an
//! optional vocabulary prompt, and writes what it recognized.
//!
//! ```sh
//! cargo run --release --example asr_bias_bench -- jobs.jsonl out.jsonl [--chunk-seconds 3] [--model id] [--cpu] [--fetch]
//! ```
//!
//! `--fetch` downloads the speech model first when it is missing (the
//! app's own download, into the app's models folder).
//!
//! One job per line: `{"key", "wav", "condition", "language"?, "prompt"?}`
//! (`wav` 16 kHz mono PCM16, relative to the jobs file). One result per
//! line: `{"key", "condition", "text", "audio_s", "spent_s"}`. Jobs whose
//! result is already in the output are skipped, so an interrupted run
//! resumes. `bench/asr-bias/` builds the jobs and scores the results.
//!
//! `--chunk-seconds` cuts each recording the way the live subtitle loop
//! does on a machine that keeps up (chunks with a 0.5 s lookback, each
//! recognized on its own with the same prompt) instead of recognizing it
//! whole.

#[cfg(feature = "local-asr")]
fn main() -> anyhow::Result<()> {
    use moonlens_lib::providers::{
        asr_local::{pcm16_bytes_to_f32, transcribe_with_prompt, DecodeParams},
        whisper_models,
    };
    use std::collections::HashSet;
    use std::io::Write;
    use std::path::Path;
    use std::time::Instant;

    #[derive(serde::Deserialize)]
    struct Job {
        key: String,
        wav: String,
        condition: String,
        language: Option<String>,
        prompt: Option<String>,
    }

    let mut model = whisper_models::DEFAULT_MODEL_ID.to_string();
    let mut gpu = true;
    let mut chunk_seconds = 0.0f32;
    let mut fetch = false;
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--model" => model = args.next().expect("--model <id>"),
            "--chunk-seconds" => {
                chunk_seconds = args.next().expect("--chunk-seconds <s>").parse()?
            }
            "--cpu" => gpu = false,
            "--fetch" => fetch = true,
            _ => paths.push(a),
        }
    }
    anyhow::ensure!(
        paths.len() == 2,
        "usage: asr_bias_bench <jobs.jsonl> <out.jsonl> [--chunk-seconds s] [--model id] [--cpu] [--fetch]"
    );
    let (jobs_path, out_path) = (Path::new(&paths[0]), Path::new(&paths[1]));
    let base = jobs_path.parent().unwrap_or(Path::new("."));

    let cfg = serde_json::json!({ "model": model });
    let asr_model = whisper_models::selected(&cfg);
    let dir = whisper_models::default_models_dir().expect("no data dir");
    let model_path = whisper_models::model_path(&dir, asr_model);
    if fetch && !whisper_models::is_installed(&dir, asr_model) {
        eprintln!("downloading {} to {}", asr_model.label, dir.display());
        tokio::runtime::Runtime::new()?.block_on(whisper_models::install(
            &dir,
            asr_model.id,
            None,
        ))?;
    }
    anyhow::ensure!(
        whisper_models::is_installed(&dir, asr_model),
        "{} is not downloaded ({}); pass --fetch",
        asr_model.label,
        model_path.display()
    );
    let params = DecodeParams::from_config(&cfg, gpu);

    let earlier = std::fs::read_to_string(out_path).unwrap_or_default();
    let done: HashSet<(String, String)> = earlier
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .map(|v| {
            (
                v["key"].as_str().unwrap_or_default().to_string(),
                v["condition"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_path)?;
    // A run stopped mid-write leaves half a line; keep it apart from
    // what follows (the scorer skips it, the job runs again).
    if !earlier.is_empty() && !earlier.ends_with('\n') {
        writeln!(out)?;
    }

    let jobs: Vec<Job> = std::fs::read_to_string(jobs_path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let total = jobs.len();
    for (n, job) in jobs.into_iter().enumerate() {
        if done.contains(&(job.key.clone(), job.condition.clone())) {
            continue;
        }
        let samples = pcm16_bytes_to_f32(&read_wav_pcm16(&base.join(&job.wav))?);
        let started = Instant::now();
        let mut texts = Vec::new();
        for piece in pieces(&samples, chunk_seconds) {
            let segments = transcribe_with_prompt(
                &model_path,
                params,
                piece,
                job.language.as_deref(),
                job.prompt.as_deref(),
            )?;
            texts.extend(segments.into_iter().map(|s| s.text));
        }
        let spent = started.elapsed().as_secs_f32();
        let text = texts.join(" ");
        eprintln!(
            "[{}/{total}] {} {} {spent:.1}s {text}",
            n + 1,
            job.key,
            job.condition
        );
        writeln!(
            out,
            "{}",
            serde_json::json!({
                "key": job.key,
                "condition": job.condition,
                "text": text,
                "audio_s": samples.len() as f32 / 16_000.0,
                "spent_s": spent,
            })
        )?;
    }
    Ok(())
}

/// The recording whole (`chunk_seconds` 0), or cut like the subtitle
/// loop: chunks of `chunk_seconds`, each preceded by the last 0.5 s of
/// the one before; a rest shorter than 0.5 s is dropped, as there.
#[cfg(feature = "local-asr")]
fn pieces(samples: &[f32], chunk_seconds: f32) -> Vec<&[f32]> {
    const OVERLAP: usize = 8_000;
    const MIN_FLUSH: usize = 8_000;
    let chunk = (chunk_seconds * 16_000.0) as usize;
    if chunk == 0 {
        return vec![samples];
    }
    let mut out = Vec::new();
    let mut at = 0;
    while at < samples.len() {
        let end = (at + chunk).min(samples.len());
        if end - at < chunk && end - at < MIN_FLUSH {
            break;
        }
        out.push(&samples[at.saturating_sub(OVERLAP)..end]);
        at = end;
    }
    out
}

/// Minimal RIFF reader: 16 kHz mono PCM16, as little-endian bytes.
#[cfg(feature = "local-asr")]
fn read_wav_pcm16(path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    use anyhow::Context;
    let b = std::fs::read(path).with_context(|| path.display().to_string())?;
    anyhow::ensure!(
        b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WAVE",
        "not a WAV file: {}",
        path.display()
    );
    let (mut fmt, mut bits, mut channels, mut rate) = (0u16, 0u16, 0u16, 0u32);
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32::from_le_bytes(b[i + 4..i + 8].try_into()?) as usize;
        let body = &b[i + 8..(i + 8 + len).min(b.len())];
        if id == b"fmt " && body.len() >= 16 {
            fmt = u16::from_le_bytes([body[0], body[1]]);
            channels = u16::from_le_bytes([body[2], body[3]]);
            rate = u32::from_le_bytes(body[4..8].try_into()?);
            bits = u16::from_le_bytes([body[14], body[15]]);
        } else if id == b"data" {
            anyhow::ensure!(
                fmt == 1 && bits == 16 && rate == 16_000 && channels == 1,
                "need 16 kHz mono PCM16: {}",
                path.display()
            );
            return Ok(body.to_vec());
        }
        i += 8 + len + (len & 1);
    }
    anyhow::bail!("no data chunk: {}", path.display())
}

#[cfg(not(feature = "local-asr"))]
fn main() {
    eprintln!("built without the local-asr feature");
}
