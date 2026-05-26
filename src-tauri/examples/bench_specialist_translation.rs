//! Specialist translation-model bench (Hy-MT2, future Opus-MT, NLLB).
//!
//! Compares purpose-built translation models against the current
//! LLM-tier baselines on the same fixture set the routed quality
//! eval uses. Bypasses `RoutedTranslator` — each model gets a bare
//! `EmbeddedLlmTranslator` instance so this measures the model's
//! raw translation capability, not our routing layer's
//! contribution.
//!
//! Why a separate bench: `translate_quality_eval` iterates
//! `UserTier` through production routing. To compare a candidate
//! model against the existing JA-bridge (llm-jp-3.1-1.8b) we'd
//! need to wire the candidate into routing first — a chicken-and-
//! egg situation when the candidate's quality is exactly what
//! we're trying to measure.
//!
//! Usage:
//!     cargo run --example bench_specialist_translation --release \
//!         --features embedded-llm-vulkan \
//!         -- [--app-data PATH] [--samples N] [--write-report PATH]
//!
//! Models benched are hard-coded (see `MODELS` below) so the
//! comparison set is reproducible across runs.

#![allow(dead_code)]

use std::{collections::HashSet, fs, path::PathBuf, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use moonlens_lib::providers::{
    translate_embedded_llm::{default_model_dir, EmbeddedLlmTranslator},
    Provider, StreamingChunkSink, TaskInput,
};

const APP_IDENTIFIER: &str = "io.moonlens.app";

/// One candidate model to bench. The `gguf` field is the basename
/// inside the app's `models/` directory — same convention as
/// `model_registry::TIERS`. `chat_template_hint` is a free-form
/// string the comments use to remember which template we are
/// passing through (we let llama.cpp read the template from the
/// GGUF metadata; this field is documentation only).
struct Candidate {
    label: &'static str,
    gguf: &'static str,
    /// Sampling temperature override. Models recommend different
    /// defaults — Hy-MT2 ships at 0.7, our llm-jp default is 0.2
    /// (lower for faithfulness). We mirror each model's
    /// recommendation rather than imposing one global default.
    temperature: f32,
    top_p: f32,
}

const MODELS: &[Candidate] = &[
    // Baseline — current Standard / Quality JA-bridge.
    Candidate {
        label: "llm-jp-3.1 1.8B (baseline)",
        gguf: "llm-jp-3.1-1.8b-instruct4-Q4_K_M.gguf",
        temperature: 0.2,
        top_p: 0.95,
    },
    // Hy-MT2 sampling sweep — Tencent's published default is
    // 0.7 / 1.0 but our 2026-05-26 first-pass eval showed that
    // generates 10–15× source-length run-on loops on multi-en-to-*
    // fixtures. The lower-temperature variants below mirror the
    // sweep we did for our own tiers (0.2 / 0.3 / 0.4) so we can
    // pick a config that keeps Hy-MT2's identifier-preservation
    // win without the looping tail.
    Candidate {
        label: "Hy-MT2 1.8B @ temp=0.2",
        gguf: "Hy-MT2-1.8B-Q4_K_M.gguf",
        temperature: 0.2,
        top_p: 0.95,
    },
    Candidate {
        label: "Hy-MT2 1.8B @ temp=0.3",
        gguf: "Hy-MT2-1.8B-Q4_K_M.gguf",
        temperature: 0.3,
        top_p: 0.95,
    },
    Candidate {
        label: "Hy-MT2 1.8B @ temp=0.7 (publisher recommended)",
        gguf: "Hy-MT2-1.8B-Q4_K_M.gguf",
        temperature: 0.7,
        top_p: 1.0,
    },
];

/// Identical fixture set to `translate_quality_eval` so the
/// comparison goes head-to-head. Copy-pasted (not imported)
/// because example crates can't share modules cleanly.
struct Fixture {
    id: &'static str,
    category: &'static str,
    source_lang: &'static str,
    target_lang: &'static str,
    text: &'static str,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        id: "log-rust-tracing-multi",
        category: "logs",
        source_lang: "en",
        target_lang: "ja",
        text: "\
2026-05-18T00:51:11.674022Z  INFO embedded_llm_perf: embedded LLM warmup: 38 ms (compiled backend kernels)
2026-05-18T00:51:11.674320Z  INFO embedded_llm_perf: embedded LLM worker ready: n_ctx=4096 n_threads=6 spec_decode=false n_draft=5 gpu_layers=99
2026-05-18T00:52:33.572626Z  INFO embedded_llm_perf: embedded LLM inference done (worker reuse): n_prompt=582 produced=264 elapsed_ms=2425 tok_per_sec=108.9
2026-05-18T00:52:33.573258Z  INFO moonlens_lib::pipeline: pipeline: Translate served by `embedded.llm` in 2552ms (skipped: [])",
    },
    Fixture {
        id: "log-single-line",
        category: "logs",
        source_lang: "en",
        target_lang: "ja",
        text: "2026-05-18T00:51:11Z WARN tauri::window: focus lost on label='main' before show_main route ran",
    },
    Fixture {
        id: "code-rust-signature",
        category: "code",
        source_lang: "en",
        target_lang: "ja",
        text: "\
pub async fn run_translate(args: TranslateArgs) -> Result<TaskResult, ProviderError> {
    let chain = self.chain.read().await;
    chain.translate(args.text, args.source_language, args.target_language).await
}",
    },
    Fixture {
        id: "structured-toml",
        category: "structured",
        source_lang: "en",
        target_lang: "ja",
        text: "\
[providers.configs.\"embedded.llm\"]
repo = \"Qwen/Qwen2.5-1.5B-Instruct-GGUF\"
temperature = 0.4
num_ctx = 8192",
    },
    Fixture {
        id: "markdown-codeblock",
        category: "markdown",
        source_lang: "en",
        target_lang: "ja",
        text: "\
Run the bench with:

```bash
cargo run --example bench_specialist_translation --release
```

The report is written to docs/benchmarks/.",
    },
    Fixture {
        id: "prose-short-greeting",
        category: "prose",
        source_lang: "en",
        target_lang: "ja",
        text: "Hello, world.",
    },
    Fixture {
        id: "prose-idiomatic",
        category: "prose",
        source_lang: "en",
        target_lang: "ja",
        text: "The quick brown fox jumps over the lazy dog.",
    },
    Fixture {
        id: "prose-meta",
        category: "prose",
        source_lang: "en",
        target_lang: "ja",
        text: "Performance varies across hardware and runtime configurations.",
    },
    Fixture {
        id: "long-prose-paragraph",
        category: "long-prose",
        source_lang: "en",
        target_lang: "ja",
        text: "\
Speculative decoding pairs a small drafter model with a larger main model: the drafter predicts a short sequence of candidate tokens autoregressively, then the main model verifies that whole sequence in a single forward pass and accepts the longest matching prefix. The end-to-end latency improves whenever the drafter and the main model agree on most of the tokens, because one expensive forward pass effectively decodes several tokens at once.",
    },
    Fixture {
        id: "reverse-ja-to-en",
        category: "reverse",
        source_lang: "ja",
        target_lang: "en",
        text: "投機的デコードでは、ドラフタモデルが候補トークンを予測し、メインモデルが一括検証して合致した接頭辞をまとめて受理します。",
    },
    Fixture {
        id: "multi-en-to-ko",
        category: "multilingual",
        source_lang: "en",
        target_lang: "ko",
        text: "MoonLens is a privacy-first screen translation tool that runs entirely on your local machine.",
    },
    Fixture {
        id: "multi-en-to-fr",
        category: "multilingual",
        source_lang: "en",
        target_lang: "fr",
        text: "MoonLens is a privacy-first screen translation tool that runs entirely on your local machine.",
    },
];

#[derive(Debug)]
struct CliArgs {
    samples: u32,
    app_data: PathBuf,
    write_report: Option<PathBuf>,
}

fn parse_args() -> Result<CliArgs> {
    let mut samples = 3u32;
    let mut app_data: Option<PathBuf> = None;
    let mut write_report: Option<PathBuf> = None;
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        match flag.as_str() {
            "--samples" => {
                let v = iter.next().ok_or_else(|| anyhow!("--samples needs value"))?;
                samples = v.parse().context("--samples must be a positive integer")?;
            }
            "--app-data" => {
                let v = iter.next().ok_or_else(|| anyhow!("--app-data needs value"))?;
                app_data = Some(PathBuf::from(v));
            }
            "--write-report" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--write-report needs value"))?;
                write_report = Some(PathBuf::from(v));
            }
            "-h" | "--help" => {
                eprintln!(
                    "usage: cargo run --example bench_specialist_translation --release \
                     --features embedded-llm-vulkan -- \
                     [--samples N] [--app-data PATH] [--write-report PATH]"
                );
                std::process::exit(0);
            }
            other => return Err(anyhow!("unknown flag: {other}")),
        }
    }
    let app_data = app_data
        .or_else(default_app_data_dir)
        .ok_or_else(|| anyhow!("could not resolve default app-data dir"))?;
    Ok(CliArgs {
        samples: samples.clamp(1, 10),
        app_data,
        write_report,
    })
}

fn default_app_data_dir() -> Option<PathBuf> {
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
    fixture_id: String,
    category: String,
    output_text: String,
    output_tokens: u32,
    total_ms: u64,
    decode_tps: f64,
    ws_score: f64,
    id_score: f64,
    translated: bool,
    length_ratio: f64,
    protected_tokens_preserved: usize,
    protected_tokens_total: usize,
}

#[derive(Debug, Clone)]
struct ModelResult {
    label: String,
    gguf: String,
    samples: Vec<SampleResult>,
    mean_ws: f64,
    mean_id: f64,
    translated_ratio: f64,
    mean_decode_tps: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("embedded_llm_perf=info")),
        )
        .with_writer(std::io::stderr)
        .try_init();

    let args = parse_args()?;
    let models_dir = default_model_dir(&args.app_data);

    println!("== bench_specialist_translation ==");
    println!("samples / fixture: {}", args.samples);
    println!("models dir       : {}", models_dir.display());
    println!("fixtures         : {}", FIXTURES.len());
    println!("candidates       : {}", MODELS.len());
    println!();

    let mut all: Vec<ModelResult> = Vec::new();
    for cand in MODELS {
        let path = models_dir.join(cand.gguf);
        if !path.is_file() {
            println!("SKIP  {}  ({}) — file missing: {}", cand.label, cand.gguf, path.display());
            continue;
        }
        println!("---- {} ({}) ----", cand.label, cand.gguf);
        match bench_one_model(cand, &path, args.samples).await {
            Ok(r) => {
                println!(
                    "      mean: ws={:.2}  id={:.2}  translated={:.0}%  decode={:.1} tok/s",
                    r.mean_ws,
                    r.mean_id,
                    r.translated_ratio * 100.0,
                    r.mean_decode_tps
                );
                all.push(r);
            }
            Err(e) => println!("      FAIL: {e:#}"),
        }
        println!();
    }

    print_summary(&all);

    if let Some(p) = args.write_report.as_ref() {
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).ok();
            }
        }
        fs::write(p, render_markdown(&all, &args))
            .with_context(|| format!("write {}", p.display()))?;
        println!("\nreport written to {}", p.display());
    }
    Ok(())
}

async fn bench_one_model(cand: &Candidate, model_path: &PathBuf, n_samples: u32) -> Result<ModelResult> {
    // Build a bare EmbeddedLlmTranslator pointing at this GGUF.
    // Sampling temperature/top_p are passed through the config —
    // the existing code path reads them in `EmbeddedLlmTranslator::new`.
    let cfg = serde_json::json!({
        "model_path": model_path.display().to_string(),
        "temperature": cand.temperature as f64,
        "top_p": cand.top_p as f64,
        // Tightened from 512 → 256 because the first pass at
        // Hy-MT2 default temperature showed runaway loops eating
        // the full 512 budget on multi-en-to-* fixtures. 256 is
        // still enough for our longest legitimate translation
        // (long-prose-paragraph at ~300 source chars produces
        // ~150-200 JA chars, well under the limit) so this fails
        // fast instead of timing out.
        "num_predict": 256u64,
    });
    let translator = EmbeddedLlmTranslator::new("bench.specialist", &cfg);

    print!("      preload");
    use std::io::Write as _;
    std::io::stdout().flush().ok();
    let start = Instant::now();
    translator
        .preload()
        .await
        .map_err(|e| anyhow!("preload failed: {e}"))?;
    println!(" — ready in {:.1}s", start.elapsed().as_secs_f64());

    let mut out_samples: Vec<SampleResult> = Vec::new();
    for fx in FIXTURES {
        for sample_idx in 0..n_samples {
            let count = Arc::new(Mutex::new(0u32));
            let buf = Arc::new(Mutex::new(String::new()));
            let first_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
            let last_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
            let c = count.clone();
            let b = buf.clone();
            let fa = first_at.clone();
            let la = last_at.clone();
            let sink: StreamingChunkSink = Arc::new(move |delta, _done| {
                if !delta.is_empty() {
                    let now = Instant::now();
                    *c.lock() += 1;
                    if fa.lock().is_none() {
                        *fa.lock() = Some(now);
                    }
                    *la.lock() = Some(now);
                    b.lock().push_str(&delta);
                }
            });
            let started = Instant::now();
            let _ = translator
                .run_streaming(
                    TaskInput::Translate {
                        text: fx.text.into(),
                        source_language: fx.source_lang.into(),
                        target_language: fx.target_lang.into(),
                        glossary: Vec::new(),
                    },
                    sink,
                )
                .await
                .map_err(|e| anyhow!("inference {} #{}: {e}", fx.id, sample_idx + 1))?;
            let total_ms = started.elapsed().as_millis() as u64;
            let n_tokens = *count.lock();
            let output_text = buf.lock().clone();
            let f = *first_at.lock();
            let l = *last_at.lock();
            let decode_ms = match (f, l) {
                (Some(f), Some(l)) if l > f => l.duration_since(f).as_millis() as u64,
                _ => 0,
            };
            let decode_tps = if n_tokens >= 2 && decode_ms > 0 {
                ((n_tokens - 1) as f64) / (decode_ms as f64) * 1000.0
            } else if total_ms > 0 {
                (n_tokens as f64) / (total_ms as f64) * 1000.0
            } else {
                0.0
            };
            let s = score(fx.text, &output_text);
            println!(
                "      {:<28} #{}/{}  ws={:.2}  id={:.2} ({}/{})  len_ratio={:.2}  translated={}  | {:.0} tok/s",
                fx.id,
                sample_idx + 1,
                n_samples,
                s.ws_score,
                s.id_score,
                s.preserved,
                s.total,
                s.length_ratio,
                if s.translated { "yes" } else { "NO" },
                decode_tps,
            );
            out_samples.push(SampleResult {
                fixture_id: fx.id.into(),
                category: fx.category.into(),
                output_text,
                output_tokens: n_tokens,
                total_ms,
                decode_tps,
                ws_score: s.ws_score,
                id_score: s.id_score,
                translated: s.translated,
                length_ratio: s.length_ratio,
                protected_tokens_preserved: s.preserved,
                protected_tokens_total: s.total,
            });
        }
    }
    let mean_ws = mean(out_samples.iter().map(|s| s.ws_score));
    let mean_id = mean(out_samples.iter().map(|s| s.id_score));
    let translated_ratio = out_samples.iter().filter(|s| s.translated).count() as f64
        / out_samples.len().max(1) as f64;
    let mean_decode_tps = mean(out_samples.iter().map(|s| s.decode_tps));
    Ok(ModelResult {
        label: cand.label.into(),
        gguf: cand.gguf.into(),
        samples: out_samples,
        mean_ws,
        mean_id,
        translated_ratio,
        mean_decode_tps,
    })
}

fn mean<I: Iterator<Item = f64>>(iter: I) -> f64 {
    let v: Vec<f64> = iter.collect();
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

struct Scores {
    ws_score: f64,
    id_score: f64,
    translated: bool,
    length_ratio: f64,
    preserved: usize,
    total: usize,
}

fn score(input: &str, output: &str) -> Scores {
    let translated = output.trim() != input.trim() && !output.trim().is_empty();
    let length_ratio = if input.is_empty() {
        0.0
    } else {
        output.chars().count() as f64 / input.chars().count() as f64
    };
    let ws_score = whitespace_score(input, output);
    let (preserved, total) = identifier_preservation(input, output);
    let id_score = if total == 0 {
        1.0
    } else {
        preserved as f64 / total as f64
    };
    Scores {
        ws_score,
        id_score,
        translated,
        length_ratio,
        preserved,
        total,
    }
}

fn whitespace_score(input: &str, output: &str) -> f64 {
    if input.is_empty() {
        return 0.0;
    }
    let in_n = input.matches('\n').count();
    let out_n = output.matches('\n').count();
    let newline = if in_n == 0 && out_n == 0 {
        1.0
    } else if in_n == 0 {
        0.0
    } else {
        let d = (in_n as i64 - out_n as i64).unsigned_abs() as f64;
        ((in_n as f64 + 1.0 - d) / (in_n as f64 + 1.0)).max(0.0)
    };
    let in_lines: Vec<&str> = input.split('\n').collect();
    let out_lines: Vec<&str> = output.split('\n').collect();
    let pairs = in_lines.len().min(out_lines.len());
    let denom = in_lines.len().max(out_lines.len());
    let indent = if denom == 0 {
        0.0
    } else {
        let mut h = 0usize;
        for i in 0..pairs {
            if leading_ws(in_lines[i]) == leading_ws(out_lines[i]) {
                h += 1;
            }
        }
        h as f64 / denom as f64
    };
    let runs_in = inline_runs(input);
    let runs_out = inline_runs(output);
    let runs = if runs_in == 0 {
        1.0
    } else {
        (runs_out.min(runs_in) as f64) / (runs_out.max(runs_in) as f64)
    };
    (newline + indent + runs) / 3.0
}

fn leading_ws(s: &str) -> &str {
    let e = s.find(|c: char| !c.is_whitespace()).unwrap_or(s.len());
    &s[..e]
}

fn inline_runs(s: &str) -> usize {
    let mut c = 0usize;
    for line in s.split('\n') {
        let t = line.trim_start();
        let mut chars = t.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == ' ' || ch == '\t' {
                if let Some(&n) = chars.peek() {
                    if n == ' ' || n == '\t' {
                        c += 1;
                        while let Some(&n2) = chars.peek() {
                            if n2 == ' ' || n2 == '\t' {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    c
}

fn identifier_preservation(input: &str, output: &str) -> (usize, usize) {
    let tokens = extract_protected_tokens(input);
    if tokens.is_empty() {
        return (0, 0);
    }
    let mut p = 0;
    for tok in &tokens {
        if output.contains(tok.as_str()) {
            p += 1;
        }
    }
    (p, tokens.len())
}

fn extract_protected_tokens(input: &str) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    for tok in input.split_whitespace() {
        let t = tok.trim_matches(|c: char| {
            matches!(
                c,
                ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}'
            )
        });
        if t.is_empty() {
            continue;
        }
        if is_protected(t) {
            set.insert(t.into());
        }
    }
    let mut out: Vec<String> = set.into_iter().collect();
    out.sort();
    out
}

fn is_protected(tok: &str) -> bool {
    if tok.is_empty() {
        return false;
    }
    if matches!(tok, "INFO" | "WARN" | "ERROR" | "DEBUG" | "TRACE") {
        return true;
    }
    if tok.starts_with("http://") || tok.starts_with("https://") {
        return true;
    }
    if tok.starts_with("--") && tok.len() > 2 {
        return true;
    }
    if tok.contains("::") {
        return true;
    }
    if iso_timestamp(tok) {
        return true;
    }
    if tok.starts_with("0x") && tok.len() > 2 && tok[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    if snake_case(tok) {
        return true;
    }
    if let Some((k, _)) = tok.split_once('=') {
        if snake_case(k) || k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return true;
        }
    }
    if version_like(tok) {
        return true;
    }
    false
}

fn iso_timestamp(tok: &str) -> bool {
    if tok.len() < 11 {
        return false;
    }
    let b = tok.as_bytes();
    b[0..4].iter().all(|c| c.is_ascii_digit())
        && b[4] == b'-'
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[7] == b'-'
        && b[8..10].iter().all(|c| c.is_ascii_digit())
        && b[10] == b'T'
}

fn snake_case(tok: &str) -> bool {
    if !tok.contains('_') {
        return false;
    }
    let parts: Vec<&str> = tok.split('_').collect();
    if parts.len() < 2 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().next().is_some_and(|c| c.is_ascii_alphanumeric()))
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn version_like(tok: &str) -> bool {
    if !tok.contains('.') {
        return false;
    }
    let mut groups = 0usize;
    for p in tok.split(|c: char| !c.is_ascii_alphanumeric()) {
        if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) {
            groups += 1;
        }
    }
    groups >= 2
}

fn print_summary(results: &[ModelResult]) {
    if results.is_empty() {
        println!("No results.");
        return;
    }
    println!("== Summary (sorted by mean id_score) ==");
    let mut sorted = results.to_vec();
    sorted.sort_by(|a, b| b.mean_id.partial_cmp(&a.mean_id).unwrap_or(std::cmp::Ordering::Equal));
    println!("{:<60} {:>8} {:>8} {:>12} {:>12}", "model", "ws", "id", "translated", "decode tps");
    println!("{}", "-".repeat(105));
    for r in &sorted {
        let label: String = r.label.chars().take(60).collect();
        println!(
            "{:<60} {:>8.2} {:>8.2} {:>11.0}% {:>12.1}",
            label, r.mean_ws, r.mean_id, r.translated_ratio * 100.0, r.mean_decode_tps
        );
    }
}

fn render_markdown(results: &[ModelResult], args: &CliArgs) -> String {
    let mut out = String::new();
    out.push_str("# Specialist Translation Model Bench\n\n");
    out.push_str(&format!("**Samples per fixture**: {}\n\n", args.samples));
    out.push_str(&format!("**Fixtures evaluated**: {}\n\n", FIXTURES.len()));
    out.push_str("## Ranking (sorted by id_score)\n\n");
    out.push_str("| Model | ws | id | translated | decode tok/s |\n|---|---:|---:|---:|---:|\n");
    let mut sorted = results.to_vec();
    sorted.sort_by(|a, b| b.mean_id.partial_cmp(&a.mean_id).unwrap_or(std::cmp::Ordering::Equal));
    for r in &sorted {
        out.push_str(&format!(
            "| {} | {:.2} | {:.2} | {:.0}% | {:.1} |\n",
            r.label, r.mean_ws, r.mean_id, r.translated_ratio * 100.0, r.mean_decode_tps
        ));
    }
    out.push('\n');
    out.push_str("## Per-fixture detail\n\n");
    for r in &sorted {
        out.push_str(&format!("### {}\n\n", r.label));
        out.push_str("| Fixture | Category | ws | id (kept/total) | len_ratio | translated | preview |\n|---|---|---:|---:|---:|:-:|---|\n");
        for s in &r.samples {
            let preview = s
                .output_text
                .lines()
                .next()
                .unwrap_or("")
                .replace('|', "\\|")
                .chars()
                .take(80)
                .collect::<String>();
            out.push_str(&format!(
                "| {} | {} | {:.2} | {:.2} ({}/{}) | {:.2} | {} | `{}` |\n",
                s.fixture_id,
                s.category,
                s.ws_score,
                s.id_score,
                s.protected_tokens_preserved,
                s.protected_tokens_total,
                s.length_ratio,
                if s.translated { "yes" } else { "**no**" },
                preview,
            ));
        }
        out.push('\n');
    }
    out
}
