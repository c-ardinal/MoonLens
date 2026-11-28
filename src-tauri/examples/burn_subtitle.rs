//! Burn subtitles into a copy of a video, as the app does, and print
//! how long it took (prototype measurement tool).
//!
//! ```sh
//! cargo run --release --example burn_subtitle -- in.mp4 --srt in.ja.srt [--out out.mp4]
//! cargo run --release --example burn_subtitle -- in.mp4 --synthetic [--encoder libx264] [--both]
//! ```
//!
//! `--synthetic` makes a two-line Japanese subtitle every 4 s over the
//! whole video (denser than speech, so a heavy case). The ffmpeg used
//! is the one the app would find (`MOONLENS_FFMPEG`, next to the exe,
//! or the `PATH`). Prints one JSON line with the result to stdout.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use moonlens_lib::{
    storage::SubtitleLine,
    subtitle_burn::{self, Encoder, Job},
    subtitle_record::ExportContent,
};

/// SRT blocks: number, `00:00:01,000 --> 00:00:02,500`, text lines.
fn parse_srt(text: &str) -> Vec<SubtitleLine> {
    let time = |s: &str| -> Option<i64> {
        let (hms, ms) = s.trim().split_once(',')?;
        let mut p = hms.split(':').map(|v| v.parse::<i64>().ok());
        let (h, m, s) = (p.next()??, p.next()??, p.next()??);
        Some(((h * 60 + m) * 60 + s) * 1000 + ms.parse::<i64>().ok()?)
    };
    let mut lines = Vec::new();
    for block in text.replace('\r', "").split("\n\n") {
        let mut rows = block.lines().skip_while(|l| l.trim().is_empty());
        let _number = rows.next();
        let Some((a, b)) = rows.next().and_then(|t| t.split_once("-->")) else {
            continue;
        };
        let (Some(start_ms), Some(end_ms)) = (time(a), time(b)) else {
            continue;
        };
        let text: Vec<&str> = rows.collect();
        // Two text rows: the app's "both" export (original, translation).
        let (original, translated) = match text.as_slice() {
            [o, t, ..] => (o.to_string(), t.to_string()),
            [t] => (String::new(), t.to_string()),
            [] => continue,
        };
        lines.push(SubtitleLine {
            seq: lines.len() as i64,
            start_ms,
            end_ms,
            original,
            translated,
        });
    }
    lines
}

fn synthetic(duration_ms: u64) -> Vec<SubtitleLine> {
    let texts = [
        "本日の会議では、来期の開発計画と予算配分について議論します。",
        "まず、前回の課題として挙がった性能問題の対策状況を確認しましょう。",
        "新しい翻訳エンジンは、従来比で約二倍の速度を達成しました。",
        "ただし、長時間の動画ではメモリー使用量の検証が必要です。",
    ];
    (0..duration_ms / 4_000)
        .map(|i| SubtitleLine {
            seq: i as i64,
            start_ms: (i * 4_000) as i64,
            end_ms: (i * 4_000 + 3_500) as i64,
            original: format!("Line {i}: the quick brown fox jumps over the lazy dog."),
            translated: texts[i as usize % texts.len()].to_string(),
        })
        .collect()
}

/// Peak resident memory of this process in MB (Linux; 0 elsewhere).
fn peak_rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
            line.split_whitespace().nth(1)?.parse::<u64>().ok()
        })
        .map_or(0, |kb| kb / 1024)
}

fn main() -> anyhow::Result<()> {
    let mut input = None;
    let (mut srt, mut out, mut encoder) = (None, None, None);
    let (mut synth, mut content) = (false, ExportContent::Translated);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--srt" => srt = args.next().map(PathBuf::from),
            "--out" => out = args.next().map(PathBuf::from),
            "--synthetic" => synth = true,
            "--both" => content = ExportContent::Both,
            "--encoder" => {
                let name = args.next().unwrap_or_default();
                encoder = Encoder::ORDER.into_iter().find(|e| e.codec() == name);
                anyhow::ensure!(encoder.is_some(), "unknown encoder {name}");
            }
            _ => input = Some(PathBuf::from(a)),
        }
    }
    let input = input.expect("usage: burn_subtitle <video> (--srt <file> | --synthetic)");
    let ffmpeg = subtitle_burn::locate_ffmpeg().ok_or_else(|| anyhow::anyhow!("no ffmpeg"))?;
    let probe = subtitle_burn::probe(&ffmpeg, &input)?;
    let lines = match (&srt, synth) {
        (Some(p), _) => parse_srt(&std::fs::read_to_string(p)?),
        (None, true) => synthetic(probe.duration_ms.unwrap_or(60_000)),
        (None, false) => anyhow::bail!("give --srt <file> or --synthetic"),
    };
    let out = out.unwrap_or_else(|| {
        input.with_file_name(format!(
            "{}.burnt.{}",
            input.file_stem().unwrap_or_default().to_string_lossy(),
            subtitle_burn::output_extension(&input)
        ))
    });
    let picked = Instant::now();
    let encoder = match encoder {
        Some(e) => e,
        None => subtitle_burn::pick_encoder(&ffmpeg, probe.width, probe.height)
            .ok_or_else(|| anyhow::anyhow!("no encoder"))?,
    };
    eprintln!(
        "{}x{} {:?} ms, {} cues, encoder {} (picked in {} ms)",
        probe.width,
        probe.height,
        probe.duration_ms,
        lines.len(),
        encoder.codec(),
        picked.elapsed().as_millis()
    );
    let total = probe.duration_ms.unwrap_or(0).max(1);
    let mut next_report = 0;
    let started = Instant::now();
    let report = subtitle_burn::burn(
        &Job {
            ffmpeg: &ffmpeg,
            input: &input,
            output: &out,
            lines: &lines,
            content,
            language: "ja",
            encoder: Some(encoder),
        },
        |ms| {
            if ms >= next_report {
                eprintln!(
                    "{:>5.1}% {:>7.1} s elapsed",
                    ms as f64 * 100.0 / total as f64,
                    started.elapsed().as_secs_f64()
                );
                next_report = ms + total / 20;
            }
        },
        &AtomicBool::new(false),
    )?;
    let secs = report.elapsed_ms as f64 / 1000.0;
    let media = report.duration_ms.unwrap_or(0) as f64 / 1000.0;
    println!(
        "{{\"input\":{:?},\"width\":{},\"height\":{},\"media_s\":{media:.1},\"elapsed_s\":{secs:.1},\
         \"speed\":{:.2},\"encoder\":\"{}\",\"cues\":{},\"input_mb\":{},\"output_mb\":{},\"tool_peak_mb\":{}}}",
        input.display().to_string(),
        report.width,
        report.height,
        media / secs.max(0.001),
        report.encoder.codec(),
        lines.len(),
        std::fs::metadata(&input)?.len() / 1_000_000,
        std::fs::metadata(&out)?.len() / 1_000_000,
        peak_rss_mb(),
    );
    Ok(())
}
