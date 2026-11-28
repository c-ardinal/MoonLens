//! Subtitles for a media file with the real models, as the app makes
//! them (download the models from the app's settings, or with the
//! `fetch_models` example, first).
//!
//! ```sh
//! cargo run --release --example file_subtitle -- talk.mp4 [--source en] [--target ja] > talk.ja.srt
//! ```
//!
//! Prints the SRT (original and translation) to stdout; progress, time
//! and the peak memory of the process go to stderr.

use std::sync::Arc;
use std::time::Instant;

use moonlens_lib::{
    file_subtitle::{self, Event, PipelineEngine},
    pipeline::Pipeline,
    plugins::registry::PluginRegistry,
    settings::SettingsStore,
    storage::{Storage, SubtitleLine},
    subtitle_record::{render_srt, ExportContent},
};
use parking_lot::Mutex;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut path = None;
    let (mut source, mut target) = (None, None);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--source" => source = args.next(),
            "--target" => target = args.next(),
            _ => path = Some(std::path::PathBuf::from(a)),
        }
    }
    let path = path.expect("usage: file_subtitle <file> [--source xx] [--target xx]");

    let tmp = tempfile::tempdir()?;
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml"))?);
    let plugins = Arc::new(PluginRegistry::load_from(
        tmp.path().join("plugins"),
        store.clone(),
    )?);
    let pipeline = Arc::new(Pipeline::new(
        store.clone(),
        Arc::new(Storage::open_in_memory()?),
        plugins,
    ));
    let settings = store.snapshot().subtitle.clone();
    let engine = PipelineEngine {
        pipeline,
        source_language: source.unwrap_or(settings.source_language.clone()),
        target_language: target.unwrap_or(settings.target_language.clone()),
    };

    let started = Instant::now();
    let (duration_ms, rx) =
        file_subtitle::open(&path).map_err(|e| anyhow::anyhow!("cannot open: {e:?}"))?;
    eprintln!("audio track: {duration_ms:?} ms");
    let lines = Mutex::new(Vec::new());
    let emit = |event: Event| match event {
        Event::Cue(cue) => {
            let mut lines = lines.lock();
            let seq = lines.len() as i64;
            lines.push(SubtitleLine {
                seq,
                start_ms: cue.start_ms as i64,
                end_ms: cue.end_ms as i64,
                original: cue.original,
                translated: cue.translated,
            });
        }
        Event::Progress { done_ms } => eprintln!(
            "{:>8.1} s done after {:>6.1} s, peak memory {} MB",
            done_ms as f64 / 1000.0,
            started.elapsed().as_secs_f64(),
            peak_rss_mb()
        ),
    };
    file_subtitle::run(rx, &engine, &settings, &emit)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    print!("{}", render_srt(&lines.lock(), ExportContent::Both));
    Ok(())
}
