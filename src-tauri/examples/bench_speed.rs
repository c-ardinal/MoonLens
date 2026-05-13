//! Speed feasibility benchmark for the Lens streaming pipeline.
//!
//! Measures four stages end-to-end on the current host and prints a
//! markdown report. Drives a "go / no-go" decision before committing to
//! the MVP implementation per plan.md Q18 (SLO).
//!
//! Usage:
//!   cargo run --example bench_speed --release -- [--iters N] [--warmup N]
//!       [--models gemma4:e2b,gemma4:e4b] [--write-report]
//!       [--fixtures-dir path]
//!
//! Exit status: 0 on completion, 1 on setup failure.

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use image::DynamicImage;
use serde::{Deserialize, Serialize};

use moonlens_lib::{
    capture,
    providers::{ocr_ocrs::OcrsProvider, Provider, TaskInput, TaskOutput},
};

const DEFAULT_ITERS: usize = 20;
const DEFAULT_WARMUP: usize = 3;
const DEFAULT_MODELS: &[&str] = &["gemma4:e2b"];
const HIDE_SLEEP_MS: u64 = 60;
const OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";

// ---------------------------------------------------------------------------
// CLI + config

#[derive(Debug, Clone)]
struct BenchConfig {
    iters: usize,
    warmup: usize,
    models: Vec<String>,
    write_report: bool,
    fixtures_dir: PathBuf,
}

impl BenchConfig {
    fn parse() -> Result<Self> {
        let mut cfg = BenchConfig {
            iters: DEFAULT_ITERS,
            warmup: DEFAULT_WARMUP,
            models: DEFAULT_MODELS.iter().map(|s| (*s).to_string()).collect(),
            write_report: false,
            fixtures_dir: PathBuf::from("tests/fixtures/bench"),
        };
        let mut args = env::args().skip(1);
        while let Some(a) = args.next() {
            match a.as_str() {
                "--iters" => {
                    cfg.iters = args
                        .next()
                        .context("--iters requires a value")?
                        .parse()
                        .context("--iters parse")?;
                }
                "--warmup" => {
                    cfg.warmup = args
                        .next()
                        .context("--warmup requires a value")?
                        .parse()
                        .context("--warmup parse")?;
                }
                "--models" => {
                    let v = args.next().context("--models requires a value")?;
                    cfg.models = v.split(',').map(|s| s.trim().to_string()).collect();
                }
                "--write-report" => cfg.write_report = true,
                "--fixtures-dir" => {
                    cfg.fixtures_dir =
                        PathBuf::from(args.next().context("--fixtures-dir requires a value")?);
                }
                "--help" | "-h" => {
                    print_help();
                    std::process::exit(0);
                }
                other => return Err(anyhow!("unknown argument: {other}")),
            }
        }
        Ok(cfg)
    }
}

fn print_help() {
    println!("MoonLens speed feasibility benchmark");
    println!();
    println!("Flags:");
    println!("  --iters N           iterations per stage (default 20)");
    println!("  --warmup N          warmup iters excluded from stats (default 3)");
    println!("  --models a,b,c      Ollama models to test (default gemma4:e2b)");
    println!("  --fixtures-dir P    where to load/save fixture PNGs");
    println!("  --write-report      write docs/benchmarks/YYYY-MM-DD-speed-feasibility.md");
}

// ---------------------------------------------------------------------------
// Statistics

#[derive(Debug, Clone, Serialize)]
struct Stats {
    /// Measurement count after warmup.
    n: usize,
    /// All measured durations in milliseconds (sorted ascending).
    samples_ms: Vec<u128>,
    p50_ms: u128,
    p95_ms: u128,
    p99_ms: u128,
    mean_ms: u128,
    min_ms: u128,
    max_ms: u128,
}

impl Stats {
    fn from_samples(mut samples_ms: Vec<u128>) -> Self {
        samples_ms.sort_unstable();
        let n = samples_ms.len();
        if n == 0 {
            return Self {
                n,
                samples_ms,
                p50_ms: 0,
                p95_ms: 0,
                p99_ms: 0,
                mean_ms: 0,
                min_ms: 0,
                max_ms: 0,
            };
        }
        let sum: u128 = samples_ms.iter().sum();
        let mean_ms = sum / n as u128;
        Self {
            n,
            p50_ms: pct(&samples_ms, 0.50),
            p95_ms: pct(&samples_ms, 0.95),
            p99_ms: pct(&samples_ms, 0.99),
            mean_ms,
            min_ms: *samples_ms.first().unwrap(),
            max_ms: *samples_ms.last().unwrap(),
            samples_ms,
        }
    }
}

fn pct(sorted_samples: &[u128], p: f64) -> u128 {
    if sorted_samples.is_empty() {
        return 0;
    }
    // Nearest-rank percentile: ceil(p * n) - 1, clamped to [0, n-1]. Keeps
    // P50 of a 1-element set returning that element, and avoids a spurious
    // index-out-of-range at p = 1.0.
    let n = sorted_samples.len();
    let rank = ((p * n as f64).ceil() as usize)
        .saturating_sub(1)
        .min(n - 1);
    sorted_samples[rank]
}

/// Run a synchronous closure `iters + warmup` times and collect stats.
fn bench_sync<F>(label: &str, cfg: &BenchConfig, mut f: F) -> Stats
where
    F: FnMut() -> Result<Duration>,
{
    eprintln!("  · {label}...");
    let total = cfg.warmup + cfg.iters;
    let mut samples = Vec::with_capacity(cfg.iters);
    for i in 0..total {
        match f() {
            Ok(d) => {
                if i >= cfg.warmup {
                    samples.push(d.as_millis());
                }
            }
            Err(e) => {
                eprintln!("    ✗ iter {i}: {e}");
            }
        }
    }
    Stats::from_samples(samples)
}

/// Async variant of [`bench_sync`]. Sequential runs (no concurrency) so
/// latency measurements reflect a single in-flight request, matching how
/// the Lens pipeline invokes providers.
async fn bench_async<F, Fut>(label: &str, cfg: &BenchConfig, mut f: F) -> Stats
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Duration>>,
{
    eprintln!("  · {label}...");
    let total = cfg.warmup + cfg.iters;
    let mut samples = Vec::with_capacity(cfg.iters);
    for i in 0..total {
        match f().await {
            Ok(d) => {
                if i >= cfg.warmup {
                    samples.push(d.as_millis());
                }
            }
            Err(e) => {
                eprintln!("    ✗ iter {i}: {e}");
            }
        }
    }
    Stats::from_samples(samples)
}

// ---------------------------------------------------------------------------
// Fixtures

#[derive(Debug, Clone)]
struct Fixture {
    name: String,
    png: Vec<u8>,
    width: u32,
    height: u32,
}

/// Load fixture PNGs from `fixtures_dir`, or capture the primary monitor
/// and crop three reference rects if the dir is empty. Returned fixtures
/// are cached on disk so subsequent runs produce identical inputs.
fn prepare_fixtures(dir: &Path) -> Result<Vec<Fixture>> {
    fs::create_dir_all(dir).with_context(|| format!("mkdir {}", dir.display()))?;
    let names = [
        ("small", 600u32, 120u32),
        ("medium", 1000u32, 250u32),
        ("large", 1600u32, 500u32),
    ];

    // Reuse disk fixtures when present — reproducible numbers across runs.
    if names
        .iter()
        .all(|(n, _, _)| dir.join(format!("{n}.png")).exists())
    {
        let mut out = Vec::new();
        for (name, _, _) in names {
            let path = dir.join(format!("{name}.png"));
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            let img = image::load_from_memory(&bytes)?;
            out.push(Fixture {
                name: name.to_string(),
                png: bytes,
                width: img.width(),
                height: img.height(),
            });
        }
        eprintln!("  (loaded {} fixtures from disk)", out.len());
        return Ok(out);
    }

    eprintln!("  (capturing primary monitor as fixtures — ensure text is on screen)");
    let full = capture::capture_primary_full().context("primary monitor capture")?;
    let (fw, fh) = (full.width(), full.height());
    let mut out = Vec::new();
    for (name, w, h) in names {
        // Center the crop horizontally, a third of the way down vertically —
        // subtitles and captions are usually mid-to-lower screen.
        let w = w.min(fw.saturating_sub(2));
        let h = h.min(fh.saturating_sub(2));
        let x = fw.saturating_sub(w) / 2;
        let y = (fh / 3).min(fh.saturating_sub(h));
        let cropped = image::imageops::crop_imm(&full, x, y, w, h).to_image();
        let dynamic = DynamicImage::ImageRgba8(cropped);
        let png = capture::png_bytes(&dynamic)?;
        let path = dir.join(format!("{name}.png"));
        fs::write(&path, &png).with_context(|| format!("write {}", path.display()))?;
        out.push(Fixture {
            name: name.to_string(),
            png,
            width: w,
            height: h,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Stage 1: capture

fn bench_capture(cfg: &BenchConfig) -> Stats {
    bench_sync("capture_primary_full + png_bytes", cfg, || {
        let start = Instant::now();
        let img = capture::capture_primary_full()?;
        let _png = capture::png_bytes(&img)?;
        Ok(start.elapsed())
    })
}

/// Capture a small lens-sized rect from the primary monitor — the
/// realistic Lens case (we never grab the whole screen in production).
fn bench_capture_region(cfg: &BenchConfig) -> Stats {
    bench_sync("capture_global_rect(600x300) + png_bytes", cfg, || {
        let start = Instant::now();
        let img = capture::capture_global_rect(200, 200, 600, 300)?;
        let _png = capture::png_bytes(&img)?;
        Ok(start.elapsed())
    })
}

fn bench_hide_sleep(cfg: &BenchConfig) -> Stats {
    // Isolates the 60ms hide-sleep-show cost currently paid per Lens
    // capture. Real hide/show window ops add OS compositor latency on
    // top; this is the lower bound contribution.
    bench_sync("60ms hide sleep baseline", cfg, || {
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(HIDE_SLEEP_MS));
        Ok(start.elapsed())
    })
}

// ---------------------------------------------------------------------------
// Stage 2: OCR

async fn bench_ocr_provider(cfg: &BenchConfig, fixtures: &[Fixture]) -> Vec<(String, Stats)> {
    let provider = OcrsProvider::new("ocrs", &serde_json::Value::Null);
    let mut out = Vec::new();
    for fx in fixtures {
        let label = format!(
            "OCR (OcrsProvider) — {} ({}x{})",
            fx.name, fx.width, fx.height
        );
        let stats = bench_async(&label, cfg, || {
            let png = fx.png.clone();
            let p = &provider;
            async move {
                let start = Instant::now();
                let res = p
                    .run(TaskInput::Ocr {
                        image_png: png,
                        languages: vec![],
                    })
                    .await;
                let elapsed = start.elapsed();
                match res {
                    Ok(_) => Ok(elapsed),
                    // LowConfidence happens when no text is detected in the
                    // fixture. Still a valid latency sample — the model ran.
                    Err(e) if e.to_string().contains("LowConfidence") => Ok(elapsed),
                    Err(e) => Err(anyhow!(e)),
                }
            }
        })
        .await;
        out.push((fx.name.clone(), stats));
    }
    out
}

// ---------------------------------------------------------------------------
// Stage 3: LLM first-token

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    system: &'a str,
    prompt: String,
    stream: bool,
    /// Ollama accepts an integer number of seconds (or -1 = keep forever)
    /// or a duration string. We send an integer so serde emits `-1` not
    /// `"-1"` — the string form triggers "400 invalid duration".
    keep_alive: i64,
    /// Disable internal reasoning for reasoning-capable models
    /// (gemma4, qwen3, deepseek). Without this, first-token latency
    /// includes multi-second silent thinking passes — not representative
    /// of direct translation workload.
    think: bool,
    options: serde_json::Value,
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    response: String,
    #[serde(default)]
    done: bool,
}

/// Latency from POST /api/generate to the first chunk that carries a
/// non-empty `response` field. Matches what the Lens UI considers
/// "first visible token".
async fn first_token_latency(model: &str, prompt: &str) -> Result<Duration> {
    let client = reqwest::Client::new();
    let url = format!("{OLLAMA_ENDPOINT}/api/generate");
    let body = GenerateRequest {
        model,
        system: "You are a translator. Output only the translation, no \
                 explanation, no alternatives, no commentary.",
        prompt: prompt.to_string(),
        stream: true,
        // -1 keeps the model in VRAM indefinitely so the next request
        // hits a warm context. Essential for reproducible numbers.
        keep_alive: -1,
        // Reasoning-capable models (gemma4, qwen3, ...) run a silent
        // thinking pass by default — adds seconds before the first
        // visible token. Disable for direct translation benchmarks.
        think: false,
        options: serde_json::json!({
            "temperature": 0.2,
            "top_p": 0.9,
            "num_predict": 128,
        }),
    };
    let start = Instant::now();
    let resp = client.post(&url).json(&body).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("ollama returned {}", resp.status()));
    }
    let mut stream = resp.bytes_stream();
    let mut buffer = Vec::<u8>::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buffer.extend_from_slice(&chunk);
        while let Some(nl) = buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=nl).collect();
            if let Ok(s) = std::str::from_utf8(&line) {
                if let Ok(c) = serde_json::from_str::<StreamChunk>(s.trim()) {
                    if !c.response.is_empty() {
                        return Ok(start.elapsed());
                    }
                    if c.done {
                        return Ok(start.elapsed());
                    }
                }
            }
        }
    }
    Err(anyhow!("stream ended without first token"))
}

async fn bench_llm(cfg: &BenchConfig) -> Vec<(String, Stats)> {
    // Short, realistic translate prompt — ~30 input tokens after tokenization.
    let prompt = "Translate the following English text to Japanese.\n\n\
                  Hello, how are you today? I hope you are doing well.";
    let mut out = Vec::new();
    for model in &cfg.models {
        let label = format!("LLM first-token — {model}");
        // Pre-warm by firing one untimed request.
        eprintln!("  · pre-warming {model}...");
        if let Err(e) = first_token_latency(model, prompt).await {
            eprintln!("    ! {model} warmup failed: {e}. Skipping.");
            continue;
        }
        let stats = bench_async(&label, cfg, || {
            let m = model.clone();
            let p = prompt.to_string();
            async move { first_token_latency(&m, &p).await }
        })
        .await;
        out.push((model.clone(), stats));
    }
    out
}

// ---------------------------------------------------------------------------
// Stage 4: end-to-end

async fn bench_e2e(cfg: &BenchConfig, fixtures: &[Fixture], model: &str) -> Vec<(String, Stats)> {
    let provider = OcrsProvider::new("ocrs", &serde_json::Value::Null);
    // Pre-warm so first-iter model load does not pollute results.
    let _ = provider
        .run(TaskInput::Ocr {
            image_png: fixtures[0].png.clone(),
            languages: vec![],
        })
        .await;
    let _ = first_token_latency(model, "warmup").await;

    let mut out = Vec::new();
    for fx in fixtures {
        let label = format!("E2E — {} + {}", fx.name, model);
        let stats = bench_async(&label, cfg, || {
            let png = fx.png.clone();
            let p = &provider;
            let m = model.to_string();
            async move {
                let start = Instant::now();
                // hide-sleep-show placeholder: we measure the 60ms sleep
                // only, since actually hiding a window requires a Tauri
                // app context.
                tokio::time::sleep(Duration::from_millis(HIDE_SLEEP_MS)).await;

                // capture is run once in the setup phase in practice (we
                // already have the fixture PNG), so do not re-capture
                // here — the capture stat is measured separately.

                // OCR
                let ocr = p
                    .run(TaskInput::Ocr {
                        image_png: png,
                        languages: vec![],
                    })
                    .await;
                let text = match ocr {
                    Ok(TaskOutput::Ocr { lines }) => lines
                        .into_iter()
                        .map(|l| l.text)
                        .collect::<Vec<_>>()
                        .join(" "),
                    _ => String::new(),
                };
                // LLM first token
                let prompt = if text.trim().is_empty() {
                    "Translate to Japanese: Hello world".to_string()
                } else {
                    format!("Translate to Japanese.\n\n{text}")
                };
                let _ = first_token_latency(&m, &prompt).await?;
                Ok(start.elapsed())
            }
        })
        .await;
        out.push((fx.name.clone(), stats));
    }
    out
}

// ---------------------------------------------------------------------------
// Reporting

#[derive(Serialize)]
struct Report {
    generated_at: String,
    host: String,
    os: String,
    config: BenchConfigSerde,
    fixtures: Vec<FixtureMeta>,
    capture_full: Stats,
    capture_region: Stats,
    hide_sleep: Stats,
    ocr_provider: Vec<(String, Stats)>,
    llm_first_token: Vec<(String, Stats)>,
    e2e: Vec<(String, Stats)>,
    verdict: Verdict,
}

#[derive(Serialize)]
struct BenchConfigSerde {
    iters: usize,
    warmup: usize,
    models: Vec<String>,
}

#[derive(Serialize)]
struct FixtureMeta {
    name: String,
    width: u32,
    height: u32,
    png_bytes: usize,
}

#[derive(Serialize, Debug)]
struct Verdict {
    category: &'static str,
    e2e_p50_small_ms: u128,
    e2e_p50_medium_ms: u128,
    ocr_p50_small_ms: u128,
    recommendation: String,
}

fn make_verdict(ocr: &[(String, Stats)], e2e: &[(String, Stats)]) -> Verdict {
    let ocr_small = ocr
        .iter()
        .find(|(n, _)| n == "small")
        .map(|(_, s)| s.p50_ms)
        .unwrap_or(0);
    let e2e_small = e2e
        .iter()
        .find(|(n, _)| n == "small")
        .map(|(_, s)| s.p50_ms)
        .unwrap_or(0);
    let e2e_medium = e2e
        .iter()
        .find(|(n, _)| n == "medium")
        .map(|(_, s)| s.p50_ms)
        .unwrap_or(0);
    let (category, rec) = if e2e_small == 0 {
        (
            "unknown",
            "E2E 未計測。ollama or OCR の setup を確認してください。".to_string(),
        )
    } else if e2e_small < 600 {
        (
            "alpha",
            "続行 OK。現行の ocrs + Ollama + xcap 構成で MVP 着手可能。".to_string(),
        )
    } else if e2e_small < 1200 {
        (
            "beta",
            "続行前提、ただし S2 で DXGI + OCR 差し替え必須。ocrs 単独では SLO <0.5s は届かず。"
                .to_string(),
        )
    } else {
        (
            "gamma",
            "SLO 達成困難。抜本再設計を検討（OCR engine 差し替え、モデル軽量化、アーキ再考）。"
                .to_string(),
        )
    };
    Verdict {
        category,
        e2e_p50_small_ms: e2e_small,
        e2e_p50_medium_ms: e2e_medium,
        ocr_p50_small_ms: ocr_small,
        recommendation: rec,
    }
}

fn format_report(r: &Report) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    writeln!(
        out,
        "# MoonLens Speed Feasibility Benchmark — {}",
        r.generated_at
    )
    .ok();
    writeln!(out).ok();
    writeln!(out, "## Environment").ok();
    writeln!(out, "- Host: `{}`", r.host).ok();
    writeln!(out, "- OS: `{}`", r.os).ok();
    writeln!(
        out,
        "- Iterations: {} (warmup {})",
        r.config.iters, r.config.warmup
    )
    .ok();
    writeln!(out, "- Models: `{}`", r.config.models.join(", ")).ok();
    writeln!(out).ok();

    writeln!(out, "## Fixtures").ok();
    writeln!(out, "| name | size | png bytes |").ok();
    writeln!(out, "|---|---|---|").ok();
    for f in &r.fixtures {
        writeln!(
            out,
            "| {} | {}x{} | {} |",
            f.name, f.width, f.height, f.png_bytes
        )
        .ok();
    }
    writeln!(out).ok();

    writeln!(out, "## 1. Screen Capture").ok();
    write_stats_table(
        &mut out,
        &[
            ("capture_primary_full+png (entire monitor)", &r.capture_full),
            (
                "capture_global_rect(600x300)+png (lens-sized)",
                &r.capture_region,
            ),
        ],
    );

    writeln!(out, "## 2. Hide-sleep baseline (60ms constant)").ok();
    write_stats_table(&mut out, &[("thread::sleep(60ms)", &r.hide_sleep)]);

    writeln!(out, "## 3. OCR latency (OcrsProvider)").ok();
    let ocr_refs: Vec<(String, &Stats)> =
        r.ocr_provider.iter().map(|(n, s)| (n.clone(), s)).collect();
    let ocr_refs_slice: Vec<(&str, &Stats)> =
        ocr_refs.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    write_stats_table(&mut out, &ocr_refs_slice);

    writeln!(out, "## 4. LLM first-token (Ollama)").ok();
    let llm_refs: Vec<(String, &Stats)> = r
        .llm_first_token
        .iter()
        .map(|(n, s)| (n.clone(), s))
        .collect();
    let llm_refs_slice: Vec<(&str, &Stats)> =
        llm_refs.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    write_stats_table(&mut out, &llm_refs_slice);

    writeln!(out, "## 5. End-to-end (hide-sleep + OCR + LLM first-token)").ok();
    let e2e_refs: Vec<(String, &Stats)> = r.e2e.iter().map(|(n, s)| (n.clone(), s)).collect();
    let e2e_refs_slice: Vec<(&str, &Stats)> =
        e2e_refs.iter().map(|(n, s)| (n.as_str(), *s)).collect();
    write_stats_table(&mut out, &e2e_refs_slice);

    writeln!(out, "## Verdict").ok();
    writeln!(out, "- category: **{}**", r.verdict.category).ok();
    writeln!(out, "- E2E P50 (small): {}ms", r.verdict.e2e_p50_small_ms).ok();
    writeln!(out, "- E2E P50 (medium): {}ms", r.verdict.e2e_p50_medium_ms).ok();
    writeln!(out, "- OCR P50 (small): {}ms", r.verdict.ocr_p50_small_ms).ok();
    writeln!(out).ok();
    writeln!(out, "**Recommendation**: {}", r.verdict.recommendation).ok();
    writeln!(out).ok();

    writeln!(out, "## Decision rule (plan.md Q18)").ok();
    writeln!(
        out,
        "- `alpha`: E2E P50 < 600ms → ocrs + Ollama で MVP 着手"
    )
    .ok();
    writeln!(
        out,
        "- `beta`: 600-1200ms → 続行、S2 で DXGI + OCR 差し替え必須"
    )
    .ok();
    writeln!(out, "- `gamma`: > 1200ms → 抜本再設計").ok();

    out
}

fn write_stats_table(out: &mut String, rows: &[(&str, &Stats)]) {
    use std::fmt::Write as _;
    writeln!(out, "| label | n | min | P50 | P95 | P99 | max | mean |").ok();
    writeln!(out, "|---|---:|---:|---:|---:|---:|---:|---:|").ok();
    for (label, s) in rows {
        writeln!(
            out,
            "| {} | {} | {}ms | **{}ms** | {}ms | {}ms | {}ms | {}ms |",
            label, s.n, s.min_ms, s.p50_ms, s.p95_ms, s.p99_ms, s.max_ms, s.mean_ms
        )
        .ok();
    }
    writeln!(out).ok();
}

// ---------------------------------------------------------------------------
// main

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    let cfg = BenchConfig::parse()?;
    eprintln!(
        "MoonLens speed bench: iters={}, warmup={}, models={:?}",
        cfg.iters, cfg.warmup, cfg.models
    );

    eprintln!(
        "[1/5] preparing fixtures in {}...",
        cfg.fixtures_dir.display()
    );
    let fixtures = prepare_fixtures(&cfg.fixtures_dir)?;

    eprintln!("[2/5] capture + hide-sleep...");
    let capture_full = bench_capture(&cfg);
    let capture_region = bench_capture_region(&cfg);
    let hide_sleep = bench_hide_sleep(&cfg);

    eprintln!("[3/5] OCR...");
    let ocr_provider = bench_ocr_provider(&cfg, &fixtures).await;

    eprintln!("[4/5] LLM first-token...");
    let llm_first_token = bench_llm(&cfg).await;

    eprintln!("[5/5] end-to-end...");
    // Use first available model (already warmed) to bound total runtime.
    let e2e_model = llm_first_token
        .first()
        .map(|(m, _)| m.clone())
        .unwrap_or_else(|| cfg.models[0].clone());
    let e2e = bench_e2e(&cfg, &fixtures, &e2e_model).await;

    let verdict = make_verdict(&ocr_provider, &e2e);
    let report = Report {
        generated_at: format_date_time(),
        host: std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string()),
        os: format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        config: BenchConfigSerde {
            iters: cfg.iters,
            warmup: cfg.warmup,
            models: cfg.models.clone(),
        },
        fixtures: fixtures
            .iter()
            .map(|f| FixtureMeta {
                name: f.name.clone(),
                width: f.width,
                height: f.height,
                png_bytes: f.png.len(),
            })
            .collect(),
        capture_full,
        capture_region,
        hide_sleep,
        ocr_provider,
        llm_first_token,
        e2e,
        verdict,
    };

    let md = format_report(&report);
    println!("\n{md}");

    if cfg.write_report {
        let out_dir = PathBuf::from("../docs/benchmarks");
        fs::create_dir_all(&out_dir).ok();
        let filename = format!("{}-speed-feasibility.md", today_yyyy_mm_dd());
        let path = out_dir.join(&filename);
        fs::write(&path, &md).with_context(|| format!("write {}", path.display()))?;
        eprintln!("report written to {}", path.display());
    }
    Ok(())
}

/// Current local date as YYYY-MM-DD. Uses SystemTime so we do not pull
/// in chrono just for a timestamp; loses sub-day granularity but that is
/// all the filename needs.
fn today_yyyy_mm_dd() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 86400 seconds per day. This is UTC — good enough for filenames.
    let days_since_epoch = ts / 86_400;
    let (y, m, d) = civil_from_days(days_since_epoch as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn format_date_time() -> String {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = ts / 86_400;
    let secs_of_day = ts % 86_400;
    let (y, mo, d) = civil_from_days(days as i64);
    let h = secs_of_day / 3600;
    let mi = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// Howard Hinnant's date algorithm, converts days-since-1970-01-01 to
/// (year, month, day). Avoids pulling in chrono.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 {
        z / 146_097
    } else {
        (z - 146_096) / 146_097
    };
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}
