//! Cross-tier embedded LLM benchmark runner.
//!
//! Iterates every tier in `model_registry::TIERS`, runs the same
//! 5-fixture translation set against each that is already on disk,
//! and prints a comparison table. Replaces the click-through-each-tier
//! settings UI workflow with one command.
//!
//! Usage:
//!     cargo run --example bench_all_tiers --release \
//!         --features embedded-llm \
//!         -- [--samples N] [--app-data PATH] [--write-report PATH]
//!
//! Defaults:
//!   --samples 5
//!   --app-data %APPDATA%\io.moonlens.app  (Windows)
//!              ~/.config/io.moonlens.app  (Linux)
//!              ~/Library/Application Support/io.moonlens.app  (macOS)
//!
//! Tiers whose primary GGUF is not present on disk are skipped with a
//! `SKIP` line. Download them via the settings UI first, or pass an
//! `--app-data` pointing at a dir that already has them.

// Bench-style example: the SampleResult / TierResult structs capture
// every per-sample measurement (token count, decode_ms, etc.) for
// future analysis even when the current markdown report only renders
// a subset. `#[allow(dead_code)]` keeps the captured fields without
// clippy noise; removing them would mean re-running the bench when
// we later want to compute the unused metric.
#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use moonlens_lib::{
    model_registry::{ChatTemplate, ModelTier, TIERS},
    providers::{
        translate_embedded_llm::{default_model_dir, EmbeddedLlmTranslator},
        Provider, StreamingChunkSink, TaskInput,
    },
};

const FIXTURES: &[&str] = &[
    "Hello, world.",
    "The quick brown fox jumps over the lazy dog.",
    "Welcome to the future of translation.",
    "This is a benchmark sentence with eleven words exactly.",
    "Performance varies across hardware and runtime configurations.",
];

const APP_IDENTIFIER: &str = "io.moonlens.app";

#[derive(Debug)]
struct CliArgs {
    samples: u32,
    app_data: PathBuf,
    write_report: Option<PathBuf>,
}

fn parse_args() -> Result<CliArgs> {
    let mut samples = 5u32;
    let mut app_data: Option<PathBuf> = None;
    let mut write_report: Option<PathBuf> = None;
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--samples" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--samples needs a value"))?;
                samples = v.parse().context("--samples must be a positive integer")?;
            }
            "--app-data" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--app-data needs a value"))?;
                app_data = Some(PathBuf::from(v));
            }
            "--write-report" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--write-report needs a value"))?;
                write_report = Some(PathBuf::from(v));
            }
            "-h" | "--help" => {
                eprintln!(
                    "usage: cargo run --example bench_all_tiers --release \
                     --features embedded-llm -- \
                     [--samples N] [--app-data PATH] [--write-report PATH]"
                );
                std::process::exit(0);
            }
            other => return Err(anyhow!("unknown flag: {other}")),
        }
    }
    let app_data = app_data
        .or_else(default_app_data_dir)
        .ok_or_else(|| anyhow!("could not resolve default app-data dir; pass --app-data PATH"))?;
    Ok(CliArgs {
        samples: samples.clamp(1, 20),
        app_data,
        write_report,
    })
}

fn default_app_data_dir() -> Option<PathBuf> {
    // Mirror Tauri's `path().app_data_dir()` on each platform without
    // pulling in tauri itself (this example runs without the runtime).
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|p| p.join(APP_IDENTIFIER))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| {
            PathBuf::from(h)
                .join("Library")
                .join("Application Support")
                .join(APP_IDENTIFIER)
        })
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|p| p.join(APP_IDENTIFIER))
    }
}

#[derive(Debug, Clone)]
struct SampleResult {
    index: u32,
    output_tokens: u32,
    total_ms: u64,
    prompt_eval_ms: u64,
    decode_ms: u64,
    wall_tps: f64,
    decode_tps: f64,
    preview: String,
}

#[derive(Debug, Clone)]
struct TierResult {
    tier_id: String,
    tier_display_name: String,
    template: ChatTemplate,
    samples: Vec<SampleResult>,
    p50_decode: f64,
    mean_decode: f64,
    p50_wall: f64,
    mean_wall: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Surface the worker's `embedded_llm_perf` tracing target so the
    // spec-decode acceptance rate + per-call decode rate are visible.
    // RUST_LOG can still override (e.g. RUST_LOG=debug for more).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("embedded_llm_perf=info")),
        )
        .with_writer(std::io::stderr)
        .try_init();

    let args = parse_args()?;
    let models_dir = default_model_dir(&args.app_data);
    println!("== bench_all_tiers ==");
    println!("samples per tier : {}", args.samples);
    println!("app-data         : {}", args.app_data.display());
    println!("models dir       : {}", models_dir.display());
    println!();

    if !models_dir.is_dir() {
        return Err(anyhow!(
            "models dir does not exist: {}\nDownload at least one tier via the settings UI first.",
            models_dir.display()
        ));
    }

    let mut all_results: Vec<TierResult> = Vec::new();
    for tier in TIERS {
        let primary_path = models_dir.join(tier.primary_filename());
        if !primary_path.is_file() {
            println!(
                "SKIP  {}  ({}) — not downloaded",
                tier.id, tier.display_name
            );
            continue;
        }

        let drafter_status = match tier.drafter {
            Some(d) => {
                let dp = models_dir.join(d.filename);
                if dp.is_file() {
                    format!(" [spec decode: {} n_draft={}]", d.filename, d.n_draft)
                } else {
                    format!(" [drafter MISSING, falling back: {}]", d.filename)
                }
            }
            None => String::new(),
        };
        println!(
            "---- {} ({}){} ----",
            tier.id, tier.display_name, drafter_status
        );
        match bench_one_tier(tier, &primary_path, args.samples).await {
            Ok(result) => {
                println!(
                    "      decode tok/s: p50={:.1}  mean={:.1}",
                    result.p50_decode, result.mean_decode
                );
                println!(
                    "      wall   tok/s: p50={:.1}  mean={:.1}",
                    result.p50_wall, result.mean_wall
                );
                all_results.push(result);
            }
            Err(e) => {
                println!("      FAIL: {e:#}");
            }
        }
        println!();
    }

    print_summary(&all_results);

    if let Some(path) = args.write_report.as_ref() {
        let md = render_markdown(&all_results, args.samples);
        fs::write(path, md).with_context(|| format!("write {}", path.display()))?;
        println!("\nreport written to {}", path.display());
    }
    Ok(())
}

async fn bench_one_tier(tier: &ModelTier, model_path: &Path, n_samples: u32) -> Result<TierResult> {
    let cfg = serde_json::json!({
        "model_path": model_path.display().to_string(),
        "repo": tier.repo,
        "filename_remote": tier.primary_filename(),
        "license": tier.license,
        "license_url": tier.license_url,
        // Cap output the same way the in-app bench does (64 tok/sample)
        // so the numbers compare apples-to-apples vs the UI-driven path.
        "num_predict": 64u64,
    });
    let translator = EmbeddedLlmTranslator::new("bench.cli", &cfg);

    print!("      preload");
    use std::io::Write as _;
    std::io::stdout().flush().ok();
    let preload_start = Instant::now();
    translator
        .preload()
        .await
        .map_err(|e| anyhow!("preload failed: {e}"))?;
    println!(" — ready in {:.1}s", preload_start.elapsed().as_secs_f64());

    let mut samples_out = Vec::with_capacity(n_samples as usize);
    for i in 0..n_samples {
        let text = FIXTURES[(i as usize) % FIXTURES.len()].to_string();
        let token_count = Arc::new(Mutex::new(0u32));
        let preview = Arc::new(Mutex::new(String::new()));
        let first_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let last_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

        let tc = token_count.clone();
        let pv = preview.clone();
        let fa = first_at.clone();
        let la = last_at.clone();
        let sink: StreamingChunkSink = Arc::new(move |delta, _done| {
            if !delta.is_empty() {
                let now = Instant::now();
                *tc.lock() += 1;
                {
                    let mut f = fa.lock();
                    if f.is_none() {
                        *f = Some(now);
                    }
                }
                *la.lock() = Some(now);
                let mut p = pv.lock();
                if p.chars().count() < 80 {
                    p.push_str(&delta);
                }
            }
        });

        let started = Instant::now();
        let _ = translator
            .run_streaming(
                TaskInput::Translate {
                    text: text.clone(),
                    source_language: "en".into(),
                    target_language: "ja".into(),
                    glossary: Vec::new(),
                },
                sink,
            )
            .await
            .map_err(|e| anyhow!("inference failed on sample {}: {e}", i + 1))?;
        let total_ms = started.elapsed().as_millis() as u64;

        let n_tokens = *token_count.lock();
        let preview_text = preview.lock().clone();
        let f = *first_at.lock();
        let l = *last_at.lock();
        let prompt_eval_ms = f
            .map(|t| t.duration_since(started).as_millis() as u64)
            .unwrap_or(total_ms);
        let decode_ms = match (f, l) {
            (Some(f), Some(l)) if l > f => l.duration_since(f).as_millis() as u64,
            _ => 0,
        };
        let wall_tps = if total_ms > 0 {
            (n_tokens as f64) / (total_ms as f64) * 1000.0
        } else {
            0.0
        };
        let decode_tps = if n_tokens >= 2 && decode_ms > 0 {
            ((n_tokens - 1) as f64) / (decode_ms as f64) * 1000.0
        } else {
            wall_tps
        };
        println!(
            "      sample {:>2}/{}: {:>3} tok / {:>5} ms  decode={:>5.1}  wall={:>5.1}  | {}",
            i + 1,
            n_samples,
            n_tokens,
            total_ms,
            decode_tps,
            wall_tps,
            preview_text.lines().next().unwrap_or("").trim(),
        );
        samples_out.push(SampleResult {
            index: i + 1,
            output_tokens: n_tokens,
            total_ms,
            prompt_eval_ms,
            decode_ms,
            wall_tps,
            decode_tps,
            preview: preview_text,
        });
    }

    let (p50_decode, mean_decode) = stats(samples_out.iter().map(|s| s.decode_tps));
    let (p50_wall, mean_wall) = stats(samples_out.iter().map(|s| s.wall_tps));
    Ok(TierResult {
        tier_id: tier.id.into(),
        tier_display_name: tier.display_name.into(),
        template: tier.chat_template,
        samples: samples_out,
        p50_decode,
        mean_decode,
        p50_wall,
        mean_wall,
    })
}

fn stats(iter: impl Iterator<Item = f64>) -> (f64, f64) {
    let mut v: Vec<f64> = iter.collect();
    if v.is_empty() {
        return (0.0, 0.0);
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = v[v.len() / 2];
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    (p50, mean)
}

fn print_summary(results: &[TierResult]) {
    if results.is_empty() {
        println!("No tiers benched. Download at least one tier first via the settings UI.");
        return;
    }
    println!("== Summary (sorted by mean decode tok/s, fastest first) ==");
    let mut sorted = results.to_vec();
    sorted.sort_by(|a, b| {
        b.mean_decode
            .partial_cmp(&a.mean_decode)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    println!(
        "{:<32} {:<8} {:>9} {:>9} {:>9} {:>9}",
        "tier", "tmpl", "decode p50", "decode mean", "wall p50", "wall mean"
    );
    println!("{}", "-".repeat(80));
    for r in &sorted {
        println!(
            "{:<32} {:<8} {:>9.1} {:>9.1} {:>9.1} {:>9.1}",
            truncate(&r.tier_display_name, 32),
            template_label(r.template),
            r.p50_decode,
            r.mean_decode,
            r.p50_wall,
            r.mean_wall,
        );
    }
}

fn template_label(t: ChatTemplate) -> &'static str {
    match t {
        ChatTemplate::ChatMl => "ChatML",
        ChatTemplate::ChatMlQwen3NoThink => "ChatML+nt",
        ChatTemplate::Gemma => "Gemma",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn render_markdown(results: &[TierResult], samples: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Embedded LLM Bench — {} samples / tier\n\n",
        samples
    ));
    out.push_str("Generated by `cargo run --example bench_all_tiers`\n\n");
    out.push_str("| Tier | Template | decode p50 | decode mean | wall p50 | wall mean |\n");
    out.push_str("|---|---|---:|---:|---:|---:|\n");
    let mut sorted = results.to_vec();
    sorted.sort_by(|a, b| {
        b.mean_decode
            .partial_cmp(&a.mean_decode)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for r in &sorted {
        out.push_str(&format!(
            "| {} | {} | {:.1} | {:.1} | {:.1} | {:.1} |\n",
            r.tier_display_name,
            template_label(r.template),
            r.p50_decode,
            r.mean_decode,
            r.p50_wall,
            r.mean_wall,
        ));
    }
    out.push_str("\n## Per-sample detail\n\n");
    for r in &sorted {
        out.push_str(&format!("### {} ({})\n\n", r.tier_display_name, r.tier_id));
        out.push_str(
            "| # | tokens | total ms | prompt eval ms | decode ms | decode tok/s | wall tok/s | preview |\n",
        );
        out.push_str("|---:|---:|---:|---:|---:|---:|---:|---|\n");
        for s in &r.samples {
            // Crop preview to a single line to keep the markdown
            // table parseable; the GGUF can emit literal newlines
            // mid-translation that would otherwise break rendering.
            let preview_one_line = s.preview.lines().next().unwrap_or("").replace('|', "\\|");
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {:.1} | {:.1} | {} |\n",
                s.index,
                s.output_tokens,
                s.total_ms,
                s.prompt_eval_ms,
                s.decode_ms,
                s.decode_tps,
                s.wall_tps,
                preview_one_line,
            ));
        }
        out.push('\n');
    }
    out
}
