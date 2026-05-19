//! Translation **quality** eval, evaluated through the production
//! routing matrix.
//!
//! Unlike `bench_all_tiers.rs` (which iterates every `ModelTier` in
//! the registry as a throughput micro-bench) this example exercises
//! the path real users hit: pick a `UserTier` in settings, the
//! `RoutedTranslator` then decides per-call whether to dispatch
//! Direct (one model) or Pivot (two models, source → English → target)
//! based on the (source, target) language pair and the host's
//! hardware bucket. Quality is scored against the invariants we
//! actually care about for the text-translation UI:
//!
//! - **whitespace preservation** — newlines, leading indentation,
//!   inline 2+-space runs (the user-visible symptom of failure is
//!   `... Z  INFO  embedded_llm_perf` collapsing to a single
//!   alphabet soup).
//! - **identifier preservation** — code identifiers, module paths,
//!   ISO timestamps, log levels, URLs, CLI flags, hex literals must
//!   survive verbatim. Small embedded LLMs love to transliterate
//!   these to katakana when the input is technical.
//! - **did-translate** — output is not byte-identical to input
//!   (catches the trivial "model echoed back the source" failure).
//! - **length sanity** — output is not catastrophically shorter
//!   than the input (catches truncation mid-sentence).
//!
//! Per fixture we also record the **routing decision** the matrix
//! picked (`Direct:<tier_id>` / `Pivot:<s1>→<s2>` / `Unsupported`)
//! so the report shows WHICH model the score belongs to — same
//! UserTier hits different inner tiers per language pair.
//!
//! Usage:
//!     cargo run --example translate_quality_eval --release \
//!         --features embedded-llm-vulkan \
//!         -- [--samples N] [--app-data PATH] [--write-report PATH] \
//!            [--label LABEL] [--user-tier compact|standard|quality] \
//!            [--hardware cpu|gpu] [--fixture ID_OR_CATEGORY]
//!
//! Default: iterate all 3 UserTiers, Gpu hardware, 1 sample per
//! fixture.

use std::{collections::HashSet, fs, path::PathBuf, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use moonlens_lib::{
    providers::{
        translate_embedded_llm::default_model_dir, translate_routed::RoutedTranslator, Provider,
        StreamingChunkSink, TaskInput,
    },
    translation_router::{route, Hardware, RoutingDecision, UserTier},
};

const APP_IDENTIFIER: &str = "io.moonlens.app";

#[derive(Debug, Clone)]
struct Fixture {
    /// Stable id used in CLI filtering and in the markdown report.
    id: &'static str,
    /// Human-readable category for grouping.
    category: &'static str,
    /// Source language code passed to `TaskInput::Translate` AND
    /// `translation_router::route`. The router classifies via its
    /// own `LangBucket` (`ja` / `en` / `Other`), so any unambiguous
    /// ISO 639-1 code works.
    source_lang: &'static str,
    /// Target language code.
    target_lang: &'static str,
    /// Source text — kept short enough that even Compact tiers
    /// complete within ~30s. Log / structured / indented fixtures
    /// intentionally include 2+-space runs to stress the inline
    /// whitespace metric.
    text: &'static str,
}

/// Fixture set ordered by category. Each text is hand-picked to
/// stress a specific failure mode the system_prompt directives
/// must mitigate.
const FIXTURES: &[Fixture] = &[
    // -- logs (en→ja) ----------------------------------------------
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
    // -- code (en→ja) ----------------------------------------------
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
        id: "code-cli-help",
        category: "code",
        source_lang: "en",
        target_lang: "ja",
        text: "Usage: moonlens [--profile NAME] [--config PATH] [--verbose] capture --region 0,0,1920,1080",
    },
    // -- structured (en→ja) ----------------------------------------
    Fixture {
        id: "structured-json",
        category: "structured",
        source_lang: "en",
        target_lang: "ja",
        text: "\
{
  \"task\": \"Translate\",
  \"output\": { \"Text\": \"こんにちは、世界。\" },
  \"used_provider\": \"embedded.llm\",
  \"latency_ms\": 1760,
  \"fallback_reasons\": []
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
    // -- markdown (en→ja) ------------------------------------------
    Fixture {
        id: "markdown-bullets",
        category: "markdown",
        source_lang: "en",
        target_lang: "ja",
        text: "\
## Release notes
- Added the standalone translation page (route `/translate/`)
- Fixed `extractText` mis-extraction in `runTranslate` callers
- Updated system_prompt to preserve identifiers and whitespace",
    },
    Fixture {
        id: "markdown-codeblock",
        category: "markdown",
        source_lang: "en",
        target_lang: "ja",
        text: "\
Run the bench with:

```bash
cargo run --example translate_quality_eval --release -- --samples 3
```

The report is written to docs/benchmarks/.",
    },
    // -- html (en→ja) ----------------------------------------------
    Fixture {
        id: "html-tags",
        category: "html",
        source_lang: "en",
        target_lang: "ja",
        text: "<p class=\"intro\">Welcome to <strong>MoonLens</strong>, the translation tool that respects formatting.</p>",
    },
    // -- prose (en→ja) ---------------------------------------------
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
    // -- long-prose (en→ja) ----------------------------------------
    Fixture {
        id: "long-prose-paragraph",
        category: "long-prose",
        source_lang: "en",
        target_lang: "ja",
        text: "\
Speculative decoding pairs a small drafter model with a larger main model: the drafter predicts a short sequence of candidate tokens autoregressively, then the main model verifies that whole sequence in a single forward pass and accepts the longest matching prefix. The end-to-end latency improves whenever the drafter and the main model agree on most of the tokens, because one expensive forward pass effectively decodes several tokens at once.",
    },
    // -- indented (en→ja) ------------------------------------------
    Fixture {
        id: "indented-multiline",
        category: "indented",
        source_lang: "en",
        target_lang: "ja",
        text: "\
The deploy steps are:
    1. Stop the running container
        - Wait for in-flight requests
        - Drain the work queue
    2. Pull the new image
    3. Start the new container with the same volume mounts",
    },
    // -- reverse (ja→en) -------------------------------------------
    // Exercises: Direct via JA bridge (Standard/Quality) or via
    // COMPACT_JA (Compact).
    Fixture {
        id: "reverse-ja-to-en",
        category: "reverse",
        source_lang: "ja",
        target_lang: "en",
        text: "投機的デコードでは、ドラフタモデルが候補トークンを予測し、メインモデルが一括検証して合致した接頭辞をまとめて受理します。",
    },
    // -- multilingual: en → other --------------------------------
    // Exercises: Direct multi (Standard = Gemma E2B / Quality = E4B).
    // Compact has no multi partner — silently degrades to JA-Compact.
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
    // -- pivot stage-1-like inputs (Direct en→fr) ---------------
    // These simulate the English intermediate a Pivot's stage 1
    // produces, so we can tell whether the Pivot wrapper itself
    // is mis-wired or whether gemma-4 multi just echoes
    // technical English when asked to render it in French.
    Fixture {
        id: "debug-tech-en-to-fr-1",
        category: "debug-pivot",
        source_lang: "en",
        target_lang: "fr",
        text: "The speculative decoding method involves combining a small drafter model and a large main model.",
    },
    Fixture {
        id: "debug-tech-en-to-fr-2",
        category: "debug-pivot",
        source_lang: "en",
        target_lang: "fr",
        text: "Speculative decoding pairs a small drafter with a larger main model: the drafter predicts candidate tokens autoregressively.",
    },
    // -- multilingual: ja → other -------------------------------
    // Exercises: Pivot on GPU (JA bridge → multi); Direct multi on
    // CPU; Compact degrades to JA-Compact.
    Fixture {
        id: "pivot-ja-to-fr",
        category: "multilingual",
        source_lang: "ja",
        target_lang: "fr",
        text: "投機的デコードは、小型ドラフタモデルと大型メインモデルを組み合わせる手法です。",
    },
    // -- multilingual: X → Y (neither en/ja) --------------------
    // Exercises: Pivot through English (Standard/Quality GPU);
    // Direct multi on CPU; Compact UNSUPPORTED — should be flagged.
    Fixture {
        id: "pivot-fr-to-de",
        category: "multilingual",
        source_lang: "fr",
        target_lang: "de",
        text: "Le décodage spéculatif associe un petit modèle brouillon à un modèle principal plus grand pour accélérer la génération.",
    },
];

#[derive(Debug)]
struct CliArgs {
    samples: u32,
    app_data: PathBuf,
    write_report: Option<PathBuf>,
    label: Option<String>,
    user_tier_filter: Option<UserTier>,
    hardware: Hardware,
    fixture_filter: Option<String>,
}

fn parse_args() -> Result<CliArgs> {
    let mut samples = 1u32;
    let mut app_data: Option<PathBuf> = None;
    let mut write_report: Option<PathBuf> = None;
    let mut label: Option<String> = None;
    let mut user_tier_filter: Option<UserTier> = None;
    let mut hardware: Hardware = Hardware::Gpu;
    let mut fixture_filter: Option<String> = None;
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
            "--label" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--label needs a value"))?;
                label = Some(v);
            }
            "--user-tier" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--user-tier needs compact|standard|quality"))?;
                user_tier_filter = Some(parse_user_tier(&v)?);
            }
            "--hardware" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--hardware needs cpu|gpu"))?;
                hardware = parse_hardware(&v)?;
            }
            "--fixture" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--fixture needs a value (id or category)"))?;
                fixture_filter = Some(v);
            }
            "-h" | "--help" => {
                eprintln!(
                    "usage: cargo run --example translate_quality_eval --release \
                     --features embedded-llm-vulkan -- \
                     [--samples N] [--app-data PATH] [--write-report PATH] \
                     [--label LABEL] [--user-tier compact|standard|quality] \
                     [--hardware cpu|gpu] [--fixture ID_OR_CATEGORY]"
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
        samples: samples.clamp(1, 10),
        app_data,
        write_report,
        label,
        user_tier_filter,
        hardware,
        fixture_filter,
    })
}

fn parse_user_tier(s: &str) -> Result<UserTier> {
    match s.to_lowercase().as_str() {
        "compact" => Ok(UserTier::Compact),
        "standard" => Ok(UserTier::Standard),
        "quality" => Ok(UserTier::Quality),
        other => Err(anyhow!("unknown user-tier `{other}` (compact|standard|quality)")),
    }
}

fn parse_hardware(s: &str) -> Result<Hardware> {
    match s.to_lowercase().as_str() {
        "cpu" => Ok(Hardware::Cpu),
        "gpu" => Ok(Hardware::Gpu),
        other => Err(anyhow!("unknown hardware `{other}` (cpu|gpu)")),
    }
}

fn user_tier_label(t: UserTier) -> &'static str {
    match t {
        UserTier::Compact => "Compact",
        UserTier::Standard => "Standard",
        UserTier::Quality => "Quality",
    }
}

fn user_tier_id_string(t: UserTier) -> &'static str {
    match t {
        UserTier::Compact => "compact",
        UserTier::Standard => "standard",
        UserTier::Quality => "quality",
    }
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
    /// Routing decision the matrix chose for this (user_tier, src,
    /// tgt, hardware) tuple. Rendered as `Direct:<tier>`,
    /// `Pivot:<s1>→<s2>`, or `Unsupported`.
    routing: String,
    /// True when the routing decision was `Unsupported` — in that
    /// case the fixture is recorded with empty scores and skipped
    /// quality-wise. Caller renders it as "—" in the table.
    unsupported: bool,
    output_tokens: u32,
    total_ms: u64,
    decode_tps: f64,
    output_text: String,
    ws_score: f64,
    id_score: f64,
    translated: bool,
    length_ratio: f64,
    protected_tokens_total: usize,
    protected_tokens_preserved: usize,
}

#[derive(Debug, Clone)]
struct UserTierResult {
    user_tier: UserTier,
    samples: Vec<SampleResult>,
    /// Mean scores across SUPPORTED samples only. Unsupported rows
    /// are excluded — averaging in zeros would punish Compact for
    /// honest matrix limits.
    mean_ws_score: f64,
    mean_id_score: f64,
    mean_length_ratio: f64,
    translated_ratio: f64,
    mean_decode_tps: f64,
    supported_count: usize,
    unsupported_count: usize,
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

    println!("== translate_quality_eval (routed) ==");
    if let Some(l) = args.label.as_ref() {
        println!("label             : {l}");
    }
    println!("samples / fixture : {}", args.samples);
    println!("hardware          : {:?}", args.hardware);
    println!("app-data          : {}", args.app_data.display());
    println!("models dir        : {}", models_dir.display());

    let fixtures = select_fixtures(args.fixture_filter.as_deref());
    println!("fixtures          : {} active", fixtures.len());
    println!();

    if !models_dir.is_dir() {
        return Err(anyhow!(
            "models dir does not exist: {}\nDownload at least one tier via the settings UI first.",
            models_dir.display()
        ));
    }

    let user_tiers: &[UserTier] = match args.user_tier_filter {
        Some(t) => match t {
            UserTier::Compact => &[UserTier::Compact],
            UserTier::Standard => &[UserTier::Standard],
            UserTier::Quality => &[UserTier::Quality],
        },
        None => &[UserTier::Compact, UserTier::Standard, UserTier::Quality],
    };

    let mut all_results: Vec<UserTierResult> = Vec::new();
    for &user_tier in user_tiers {
        println!("---- UserTier::{:?} ----", user_tier);
        match eval_user_tier(user_tier, args.hardware, &args.app_data, &fixtures, args.samples).await {
            Ok(result) => {
                println!(
                    "      mean: ws={:.2}  id={:.2}  len_ratio={:.2}  translated={:.0}%  decode={:.1} tok/s  ({} supported, {} unsupported)",
                    result.mean_ws_score,
                    result.mean_id_score,
                    result.mean_length_ratio,
                    result.translated_ratio * 100.0,
                    result.mean_decode_tps,
                    result.supported_count,
                    result.unsupported_count,
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
        let md = render_markdown(&all_results, &args, &fixtures);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).ok();
            }
        }
        fs::write(path, md).with_context(|| format!("write {}", path.display()))?;
        println!("\nreport written to {}", path.display());
    }
    Ok(())
}

fn select_fixtures(filter: Option<&str>) -> Vec<&'static Fixture> {
    match filter {
        None => FIXTURES.iter().collect(),
        Some(f) => FIXTURES
            .iter()
            .filter(|fx| fx.id == f || fx.category == f)
            .collect(),
    }
}

async fn eval_user_tier(
    user_tier: UserTier,
    hardware: Hardware,
    app_data: &PathBuf,
    fixtures: &[&Fixture],
    n_samples: u32,
) -> Result<UserTierResult> {
    let translator = RoutedTranslator::new(
        format!("quality.eval.{}", user_tier_id_string(user_tier)),
        user_tier,
        hardware,
        // gpu_layers=99 mirrors the in-app default for `embedded-llm-vulkan`
        // builds. CPU runs ignore this value internally.
        99,
        Some(app_data),
        None,
    );

    print!("      preload");
    use std::io::Write as _;
    std::io::stdout().flush().ok();
    let preload_start = Instant::now();
    translator
        .preload()
        .await
        .map_err(|e| anyhow!("preload failed: {e}"))?;
    println!(" — ready in {:.1}s", preload_start.elapsed().as_secs_f64());

    let mut samples_out: Vec<SampleResult> = Vec::new();
    for fx in fixtures {
        let decision = route(user_tier, hardware, fx.source_lang, fx.target_lang);
        let routing_label = format_routing(&decision);

        for sample_idx in 0..n_samples {
            if matches!(decision, RoutingDecision::Unsupported) {
                println!(
                    "      {:<28} #{}/{}  [UNSUPPORTED — {}]",
                    fx.id,
                    sample_idx + 1,
                    n_samples,
                    routing_label,
                );
                samples_out.push(SampleResult {
                    fixture_id: fx.id.into(),
                    category: fx.category.into(),
                    routing: routing_label.clone(),
                    unsupported: true,
                    output_tokens: 0,
                    total_ms: 0,
                    decode_tps: 0.0,
                    output_text: String::new(),
                    ws_score: 0.0,
                    id_score: 0.0,
                    translated: false,
                    length_ratio: 0.0,
                    protected_tokens_total: 0,
                    protected_tokens_preserved: 0,
                });
                continue;
            }

            let token_count = Arc::new(Mutex::new(0u32));
            let collected = Arc::new(Mutex::new(String::new()));
            let first_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
            let last_at: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));

            let tc = token_count.clone();
            let buf = collected.clone();
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
                    buf.lock().push_str(&delta);
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
                .map_err(|e| {
                    anyhow!(
                        "inference failed on fixture {} sample {}: {e}",
                        fx.id,
                        sample_idx + 1
                    )
                })?;
            let total_ms = started.elapsed().as_millis() as u64;

            let n_tokens = *token_count.lock();
            let output_text = collected.lock().clone();
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

            let scores = score_translation(fx.text, &output_text);
            println!(
                "      {:<28} #{}/{}  [{}]  ws={:.2}  id={:.2} ({}/{})  len_ratio={:.2}  translated={}  | {:.0} tok/s",
                fx.id,
                sample_idx + 1,
                n_samples,
                routing_label,
                scores.ws_score,
                scores.id_score,
                scores.protected_tokens_preserved,
                scores.protected_tokens_total,
                scores.length_ratio,
                if scores.translated { "yes" } else { "NO" },
                decode_tps,
            );

            samples_out.push(SampleResult {
                fixture_id: fx.id.into(),
                category: fx.category.into(),
                routing: routing_label.clone(),
                unsupported: false,
                output_tokens: n_tokens,
                total_ms,
                decode_tps,
                output_text,
                ws_score: scores.ws_score,
                id_score: scores.id_score,
                translated: scores.translated,
                length_ratio: scores.length_ratio,
                protected_tokens_total: scores.protected_tokens_total,
                protected_tokens_preserved: scores.protected_tokens_preserved,
            });
        }
    }

    // Aggregate over supported samples only.
    let supported: Vec<&SampleResult> = samples_out.iter().filter(|s| !s.unsupported).collect();
    let mean_ws_score = mean(supported.iter().map(|s| s.ws_score));
    let mean_id_score = mean(supported.iter().map(|s| s.id_score));
    let mean_length_ratio = mean(supported.iter().map(|s| s.length_ratio));
    let translated_ratio = if supported.is_empty() {
        0.0
    } else {
        supported.iter().filter(|s| s.translated).count() as f64 / supported.len() as f64
    };
    let mean_decode_tps = mean(supported.iter().map(|s| s.decode_tps));
    let unsupported_count = samples_out.len() - supported.len();

    Ok(UserTierResult {
        user_tier,
        supported_count: supported.len(),
        unsupported_count,
        samples: samples_out,
        mean_ws_score,
        mean_id_score,
        mean_length_ratio,
        translated_ratio,
        mean_decode_tps,
    })
}

fn format_routing(d: &RoutingDecision) -> String {
    match d {
        RoutingDecision::Direct { tier_id } => format!("Direct:{}", tier_id),
        RoutingDecision::Pivot {
            stage1_tier,
            stage2_tier,
        } => format!("Pivot:{}→{}", stage1_tier, stage2_tier),
        RoutingDecision::Unsupported => "Unsupported".to_string(),
    }
}

fn mean<'a, I, T>(iter: I) -> f64
where
    I: Iterator<Item = T>,
    T: AsF64,
{
    let mut count = 0usize;
    let mut sum = 0.0f64;
    for v in iter {
        sum += v.as_f64();
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / (count as f64)
    }
}

trait AsF64 {
    fn as_f64(&self) -> f64;
}

impl AsF64 for f64 {
    fn as_f64(&self) -> f64 {
        *self
    }
}

impl AsF64 for &f64 {
    fn as_f64(&self) -> f64 {
        **self
    }
}

#[derive(Debug, Clone)]
struct Scores {
    ws_score: f64,
    id_score: f64,
    translated: bool,
    length_ratio: f64,
    protected_tokens_total: usize,
    protected_tokens_preserved: usize,
}

fn score_translation(input: &str, output: &str) -> Scores {
    let translated = output.trim() != input.trim() && !output.trim().is_empty();
    let length_ratio = if input.is_empty() {
        0.0
    } else {
        (output.chars().count() as f64) / (input.chars().count() as f64)
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
        protected_tokens_total: total,
        protected_tokens_preserved: preserved,
    }
}

/// Three independent whitespace signals averaged:
/// - newline count match
/// - per-line leading indentation match
/// - inline 2+-space-run preservation
fn whitespace_score(input: &str, output: &str) -> f64 {
    if input.is_empty() {
        return 0.0;
    }
    let in_newlines = input.matches('\n').count();
    let out_newlines = output.matches('\n').count();
    let newline_score = if in_newlines == 0 && out_newlines == 0 {
        1.0
    } else if in_newlines == 0 {
        0.0
    } else {
        let diff = (in_newlines as i64 - out_newlines as i64).unsigned_abs() as f64;
        ((in_newlines as f64 + 1.0 - diff) / (in_newlines as f64 + 1.0)).max(0.0)
    };

    let in_lines: Vec<&str> = input.split('\n').collect();
    let out_lines: Vec<&str> = output.split('\n').collect();
    let pair_count = in_lines.len().min(out_lines.len());
    let denom = in_lines.len().max(out_lines.len());
    let indent_score = if denom == 0 {
        0.0
    } else {
        let mut hits = 0usize;
        for i in 0..pair_count {
            let in_indent = leading_ws(in_lines[i]);
            let out_indent = leading_ws(out_lines[i]);
            if in_indent == out_indent {
                hits += 1;
            }
        }
        hits as f64 / denom as f64
    };

    let in_runs = inline_ws_runs(input);
    let out_runs = inline_ws_runs(output);
    let run_score = if in_runs == 0 {
        1.0
    } else {
        (out_runs.min(in_runs) as f64) / (out_runs.max(in_runs) as f64)
    };

    (newline_score + indent_score + run_score) / 3.0
}

fn leading_ws(s: &str) -> &str {
    let end = s.find(|c: char| !c.is_whitespace()).unwrap_or(s.len());
    &s[..end]
}

fn inline_ws_runs(s: &str) -> usize {
    let mut count = 0usize;
    for line in s.split('\n') {
        let trimmed = line.trim_start();
        let mut chars = trimmed.chars().peekable();
        while let Some(c) = chars.next() {
            if c == ' ' || c == '\t' {
                if let Some(&n) = chars.peek() {
                    if n == ' ' || n == '\t' {
                        count += 1;
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
    count
}

fn identifier_preservation(input: &str, output: &str) -> (usize, usize) {
    let tokens = extract_protected_tokens(input);
    if tokens.is_empty() {
        return (0, 0);
    }
    let mut preserved = 0usize;
    for tok in &tokens {
        if output.contains(tok.as_str()) {
            preserved += 1;
        }
    }
    (preserved, tokens.len())
}

fn extract_protected_tokens(input: &str) -> Vec<String> {
    let mut set: HashSet<String> = HashSet::new();
    for tok in input.split_whitespace() {
        let trimmed = tok.trim_matches(|c: char| {
            matches!(
                c,
                ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}'
            )
        });
        if trimmed.is_empty() {
            continue;
        }
        if is_protected_token(trimmed) {
            set.insert(trimmed.to_string());
        }
    }
    let mut out: Vec<String> = set.into_iter().collect();
    out.sort();
    out
}

fn is_protected_token(tok: &str) -> bool {
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
    if iso_timestamp_like(tok) {
        return true;
    }
    if tok.starts_with("0x")
        && tok.len() > 2
        && tok[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return true;
    }
    if has_snake_case_shape(tok) {
        return true;
    }
    if let Some((k, _)) = tok.split_once('=') {
        if has_snake_case_shape(k) || k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return true;
        }
    }
    if looks_like_version(tok) {
        return true;
    }
    false
}

fn iso_timestamp_like(tok: &str) -> bool {
    if tok.len() < 11 {
        return false;
    }
    let bytes = tok.as_bytes();
    bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(|b| b.is_ascii_digit())
        && bytes[10] == b'T'
}

fn has_snake_case_shape(tok: &str) -> bool {
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
        && parts
            .iter()
            .all(|p| p.chars().all(|c| c.is_ascii_alphanumeric()))
}

fn looks_like_version(tok: &str) -> bool {
    if !tok.contains('.') {
        return false;
    }
    let mut digit_groups = 0usize;
    for part in tok.split(|c: char| !c.is_ascii_alphanumeric()) {
        if !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()) {
            digit_groups += 1;
        }
    }
    digit_groups >= 2
}

fn print_summary(results: &[UserTierResult]) {
    if results.is_empty() {
        println!("No user tiers evaluated.");
        return;
    }
    println!("== Summary (sorted by mean id_score, best first) ==");
    let mut sorted = results.to_vec();
    sorted.sort_by(|a, b| {
        b.mean_id_score
            .partial_cmp(&a.mean_id_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    println!(
        "{:<14} {:>8} {:>8} {:>10} {:>12} {:>10} {:>14}",
        "user_tier", "ws", "id", "len_ratio", "translated", "decode tps", "support/total"
    );
    println!("{}", "-".repeat(100));
    for r in &sorted {
        let total = r.supported_count + r.unsupported_count;
        println!(
            "{:<14} {:>8.2} {:>8.2} {:>10.2} {:>11.0}% {:>10.1} {:>10}/{:<3}",
            user_tier_label(r.user_tier),
            r.mean_ws_score,
            r.mean_id_score,
            r.mean_length_ratio,
            r.translated_ratio * 100.0,
            r.mean_decode_tps,
            r.supported_count,
            total,
        );
    }
}

fn render_markdown(
    results: &[UserTierResult],
    args: &CliArgs,
    fixtures: &[&Fixture],
) -> String {
    let mut out = String::new();
    out.push_str("# Translate Quality Eval (routed)\n\n");
    if let Some(l) = args.label.as_ref() {
        out.push_str(&format!("**Label**: {l}\n\n"));
    }
    out.push_str(&format!("**Samples per fixture**: {}\n\n", args.samples));
    out.push_str(&format!("**Hardware**: {:?}\n\n", args.hardware));
    out.push_str(&format!("**Fixtures evaluated**: {}\n\n", fixtures.len()));

    out.push_str("## UserTier ranking (sorted by id_score, supported-only mean)\n\n");
    out.push_str(
        "| UserTier | ws | id | len_ratio | translated | decode tok/s | supported/total |\n",
    );
    out.push_str("|---|---:|---:|---:|---:|---:|---:|\n");
    let mut sorted = results.to_vec();
    sorted.sort_by(|a, b| {
        b.mean_id_score
            .partial_cmp(&a.mean_id_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for r in &sorted {
        let total = r.supported_count + r.unsupported_count;
        out.push_str(&format!(
            "| {} | {:.2} | {:.2} | {:.2} | {:.0}% | {:.1} | {}/{} |\n",
            user_tier_label(r.user_tier),
            r.mean_ws_score,
            r.mean_id_score,
            r.mean_length_ratio,
            r.translated_ratio * 100.0,
            r.mean_decode_tps,
            r.supported_count,
            total,
        ));
    }
    out.push('\n');

    out.push_str("## Per-UserTier × fixture detail\n\n");
    for r in &sorted {
        out.push_str(&format!("### UserTier::{}\n\n", user_tier_label(r.user_tier)));
        out.push_str(
            "| Fixture | Category | Routing | ws | id (kept/total) | len_ratio | translated | output preview |\n",
        );
        out.push_str("|---|---|---|---:|---:|---:|:-:|---|\n");
        for s in &r.samples {
            if s.unsupported {
                out.push_str(&format!(
                    "| {} | {} | `{}` | — | — | — | — | _Unsupported_ |\n",
                    s.fixture_id, s.category, s.routing,
                ));
                continue;
            }
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
                "| {} | {} | `{}` | {:.2} | {:.2} ({}/{}) | {:.2} | {} | `{}` |\n",
                s.fixture_id,
                s.category,
                s.routing,
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

    out.push_str("## Fixture catalogue\n\n");
    out.push_str("| Id | Category | Direction | Length (chars) |\n");
    out.push_str("|---|---|---|---:|\n");
    for fx in fixtures {
        out.push_str(&format!(
            "| {} | {} | {} → {} | {} |\n",
            fx.id,
            fx.category,
            fx.source_lang,
            fx.target_lang,
            fx.text.chars().count(),
        ));
    }
    out.push('\n');

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_tokens_cover_log_line() {
        let toks = extract_protected_tokens(
            "2026-05-18T00:51:11.674022Z  INFO embedded_llm_perf: embedded LLM warmup: 38 ms",
        );
        assert!(
            toks.iter().any(|t| t.starts_with("2026-05-18T")),
            "ISO timestamp not detected: {toks:?}"
        );
        assert!(
            toks.iter().any(|t| t == "INFO"),
            "INFO log level not detected: {toks:?}"
        );
        assert!(
            toks.iter().any(|t| t.starts_with("embedded_llm_perf")),
            "snake_case identifier not detected: {toks:?}"
        );
    }

    #[test]
    fn protected_tokens_dedup_repeats() {
        let toks = extract_protected_tokens("INFO INFO INFO embedded_llm_perf");
        let info_count = toks.iter().filter(|t| t.as_str() == "INFO").count();
        assert_eq!(info_count, 1, "INFO should dedup to one entry: {toks:?}");
    }

    #[test]
    fn whitespace_score_full_when_identical() {
        let s = "a\n  b\n    c";
        assert!((whitespace_score(s, s) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn whitespace_score_zero_on_full_collapse() {
        let input = "a  b\n  c  d\n    e";
        let output = "abcde";
        let score = whitespace_score(input, output);
        assert!(score < 0.5, "expected collapse score < 0.5, got {score}");
    }

    #[test]
    fn inline_runs_detected_only_for_multi_space() {
        assert_eq!(inline_ws_runs("hello world"), 0);
        assert_eq!(inline_ws_runs("hello  world"), 1);
        assert_eq!(inline_ws_runs("a   b   c"), 2);
        assert_eq!(inline_ws_runs("  indented"), 0);
        assert_eq!(inline_ws_runs("a  b\n  c  d"), 2);
    }

    #[test]
    fn whitespace_score_penalises_inline_collapse() {
        let input = "2026-05-18T00:51:11Z  INFO  embedded_llm_perf  warmup";
        let output = "2026-05-18T00:51:11ZINFOembedded_llm_perfwarmup";
        let score = whitespace_score(input, output);
        assert!(
            score <= 0.75,
            "expected inline-collapse score ≤ 0.75, got {score}"
        );
    }

    #[test]
    fn module_path_detected() {
        assert!(is_protected_token("moonlens_lib::pipeline"));
        assert!(!is_protected_token("hello"));
    }

    #[test]
    fn key_value_pair_detected() {
        assert!(is_protected_token("n_threads=6"));
        assert!(is_protected_token("gpu_layers=99"));
    }

    #[test]
    fn version_string_detected() {
        assert!(looks_like_version("1.5.0"));
        assert!(looks_like_version("2.4.1"));
        assert!(!looks_like_version("hello.world"));
    }

    #[test]
    fn iso_timestamp_detected() {
        assert!(iso_timestamp_like("2026-05-18T00:51:11.674022Z"));
        assert!(!iso_timestamp_like("2026-05-18"));
        assert!(!iso_timestamp_like("hello"));
    }

    #[test]
    fn parse_user_tier_known_values() {
        assert!(matches!(parse_user_tier("compact").unwrap(), UserTier::Compact));
        assert!(matches!(parse_user_tier("STANDARD").unwrap(), UserTier::Standard));
        assert!(matches!(parse_user_tier("Quality").unwrap(), UserTier::Quality));
        assert!(parse_user_tier("invalid").is_err());
    }
}
