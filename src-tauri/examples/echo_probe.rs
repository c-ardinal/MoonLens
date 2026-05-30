//! Echo-rate probe for the Compact (llm-jp-3-980m) Lens translator.
//!
//! Purpose: measure, data-backed, how often the Compact tier echoes
//! the English source instead of translating it on long lines — the
//! `english0001` idx 2 / idx 3 failure the 2026-05-29 parity audit
//! surfaced. The probe drives the SAME translation entry point the
//! Lens uses per card (`Pipeline::run(Capability::Translate, ...)`,
//! which is exactly what `ChainBackedLensTranslator` calls), so the
//! echo it measures is the echo production produces — only the input
//! source differs (hard-coded fixture lines vs OCR output).
//!
//! It does NOT run OCR / inpaint / render: the OCR output for
//! `english0001.png` is deterministic (see `english0001_eval.json`),
//! so the four card texts are pinned below. That isolates the
//! translation variable cleanly and lets us run many samples per
//! line per temperature without paying the OCR cost each time.
//!
//! Temperature is the swept lever. `config_for_tier` reads
//! `MOONLENS_LENS_COMPACT_TEMP` (production path — no eval-only
//! divergence), so a shell loop can sweep it without rebuilding:
//!
//!   foreach ($t in 0.0,0.1,0.2,0.3,0.4) {
//!     $env:MOONLENS_LENS_COMPACT_TEMP=$t
//!     cargo run --example echo_probe --release
//!   }
//!
//! Env knobs:
//!   - `MOONLENS_LENS_COMPACT_TEMP`  Compact sampling temperature
//!     (0.0 = greedy / deterministic). Unset = tuned default (0.1).
//!   - `MOONLENS_PROBE_SAMPLES`      samples per line (default 8).
//!   - `MOONLENS_EVAL_APP_DATA`      app-data root holding `models/`
//!     (default `%APPDATA%/io.moonlens.app`).

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};

use moonlens_lib::providers::{Capability, TaskInput, TaskOutput};

/// The four card texts PP-OCRv5 emits for `tests/input/english0001.png`,
/// copied verbatim from `tests/runs/lens_eval/english0001_eval.json`.
/// idx 2 is the pure-echo case (translation == source); idx 3 is the
/// partial-echo + hallucination case.
const PROBE_LINES: &[(&str, &str)] = &[
    (
        "idx0",
        "The Elicitation Paragraph (Please Call Stella)\nThe Speech Accent Archive (SAA)により使われている文。",
    ),
    (
        "idx1",
        "Please cal Stella. Ask her to bring these things with her from the store: Six spoons of fresh snow peas, five thick slabs of blue",
    ),
    (
        "idx2",
        "cheese, and maybe a snack for her brother Bob. We also need a smal plastic snake and a big toy frog for the kids. She can scoop",
    ),
    (
        "idx3",
        "these things into three red bags, and we willgo meet her Wednesday at the train station.",
    ),
];

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3000..=0x303F
        | 0x3040..=0x309F
        | 0x30A0..=0x30FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0xFF00..=0xFFEF
        | 0xAC00..=0xD7AF
    )
}

/// Fraction of non-whitespace chars that are CJK (kana / kanji /
/// hangul / fullwidth). A faithful EN→JA translation is CJK-dense;
/// a verbatim English echo is ~0.
fn cjk_ratio(s: &str) -> f32 {
    let non_ws: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    if non_ws.is_empty() {
        return 0.0;
    }
    let cjk = non_ws.iter().filter(|c| is_cjk(**c)).count();
    cjk as f32 / non_ws.len() as f32
}

/// Longest run of consecutive ASCII letters — catches the idx 3
/// "shecanscoop…station" partial echo even when Japanese is appended
/// afterwards (overall cjk_ratio would otherwise look healthy).
fn max_latin_run(s: &str) -> usize {
    let mut best = 0usize;
    let mut cur = 0usize;
    for c in s.chars() {
        if c.is_ascii_alphabetic() {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Verdict {
    /// Mostly Japanese, no long Latin run — a real translation.
    Clean,
    /// A long Latin run survived (partial echo / contamination).
    Contaminated,
    /// Near-verbatim English copy.
    Echo,
    /// Provider returned nothing. Production falls back to the source
    /// text on the card, so to the user this looks like an echo too —
    /// but the cause (model emits EOS immediately) differs from a
    /// sampled English copy, so it gets its own bucket.
    Empty,
    /// Provider error (timeout / load failure). A measurement artifact
    /// to exclude from the echo-rate denominator, not a translation
    /// quality signal — but worth surfacing (greedy can loop until the
    /// 180 s chain timeout).
    Error,
}

fn classify(translation: &str) -> Verdict {
    if translation.starts_with("<error:") || translation.starts_with("<non-text:") {
        return Verdict::Error;
    }
    if translation.trim().is_empty() {
        return Verdict::Empty;
    }
    let ratio = cjk_ratio(translation);
    let latin = max_latin_run(translation);
    if ratio < 0.35 {
        Verdict::Echo
    } else if latin >= 20 {
        Verdict::Contaminated
    } else {
        Verdict::Clean
    }
}

fn resolve_app_data_dir() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("MOONLENS_EVAL_APP_DATA") {
        return Ok(PathBuf::from(custom));
    }
    let appdata = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .context("neither MOONLENS_EVAL_APP_DATA, APPDATA, nor HOME is set")?;
    Ok(PathBuf::from(appdata).join("io.moonlens.app"))
}

fn main() -> Result<()> {
    let samples: usize = std::env::var("MOONLENS_PROBE_SAMPLES")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(8);
    let temp_label =
        std::env::var("MOONLENS_LENS_COMPACT_TEMP").unwrap_or_else(|_| "default(0.1)".to_string());

    // Same parity Pipeline lens_eval builds: settings from the live
    // config.toml (copied to a temp file so the probe never writes
    // the live one), in-memory storage, real provider chain. The
    // translate chain + user_tier come straight from the user's
    // config, so this is the production Compact translator.
    let app_data = resolve_app_data_dir().context("resolve app-data dir")?;
    let settings = {
        let live_cfg = app_data.join("config.toml");
        let tmp_cfg = std::env::temp_dir().join("moonlens_echo_probe_config.toml");
        if live_cfg.exists() {
            std::fs::copy(&live_cfg, &tmp_cfg)
                .with_context(|| format!("copy {} for probe", live_cfg.display()))?;
        } else if tmp_cfg.exists() {
            let _ = std::fs::remove_file(&tmp_cfg);
        }
        Arc::new(
            moonlens_lib::settings::SettingsStore::load_from(tmp_cfg)
                .context("load settings for probe pipeline")?,
        )
    };
    let storage = Arc::new(
        moonlens_lib::storage::Storage::open_in_memory().context("open in-memory storage")?,
    );
    let pipeline = Arc::new(moonlens_lib::pipeline::Pipeline::with_providers(
        settings.clone(),
        storage,
        std::collections::HashMap::new(),
    ));
    pipeline.set_app_data_dir(app_data.clone());
    pipeline.rebuild_providers();

    let (source_lang, target_lang) = {
        let snap = settings.snapshot();
        let src = if snap.translation.default_source_language.trim().is_empty() {
            "auto".to_string()
        } else {
            snap.translation.default_source_language.clone()
        };
        (src, snap.translation.default_target_language.clone())
    };

    {
        let snap = settings.snapshot();
        println!(
            "echo_probe: app_data={} translate_chain={:?} user_tier={:?} \
             source={source_lang} target={target_lang}",
            app_data.display(),
            snap.providers.translate.chain,
            snap.providers.user_tier,
        );
    }
    println!("MOONLENS_LENS_COMPACT_TEMP={temp_label}  samples/line={samples}\n");

    // Diagnostic override: `MOONLENS_PROBE_LINE="label::text"` (repeatable
    // via `;;` separator) replaces the pinned english0001 fixtures with
    // ad-hoc lines. Used to isolate WHY idx2 ("cheese, …") empties out —
    // capitalisation, length, leading fragment, etc.
    let custom_lines: Vec<(String, String)> = std::env::var("MOONLENS_PROBE_LINE")
        .ok()
        .map(|raw| {
            raw.split(";;")
                .filter_map(|entry| {
                    let entry = entry.trim();
                    if entry.is_empty() {
                        return None;
                    }
                    match entry.split_once("::") {
                        Some((label, text)) => Some((label.to_string(), text.to_string())),
                        None => Some(("custom".to_string(), entry.to_string())),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let lines: Vec<(String, String)> = if custom_lines.is_empty() {
        PROBE_LINES
            .iter()
            .map(|(l, t)| (l.to_string(), t.to_string()))
            .collect()
    } else {
        custom_lines
    };

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;

    let translate_once = |source: &str| -> (String, u128) {
        let t0 = std::time::Instant::now();
        let out = runtime.block_on(pipeline.run(
            Capability::Translate,
            TaskInput::Translate {
                text: source.to_string(),
                source_language: source_lang.clone(),
                target_language: target_lang.clone(),
                glossary: Vec::new(),
            },
        ));
        let elapsed = t0.elapsed().as_millis();
        let translation = match out {
            Ok(res) => match res.output {
                TaskOutput::Text(t) => t,
                TaskOutput::TextChunk { delta, .. } => delta,
                other => format!("<non-text: {other:?}>"),
            },
            Err(e) => format!("<error: {e}>"),
        };
        (translation, elapsed)
    };

    // Warm the worker (model load + Vulkan init + first-token priming)
    // on a throwaway call so the first MEASURED line is not skewed by
    // cold-start latency / a load-window timeout.
    let (_warm, warm_ms) = translate_once("Hello.");
    println!("warmup done in {warm_ms} ms\n");

    let mut totals = [0usize; 5]; // clean, contam, echo, empty, error
    let mut total_runs = 0usize;

    for (label, source) in lines.iter() {
        let mut counts = [0usize; 5];
        let mut latency_sum = 0u128;
        let mut first_outputs: Vec<String> = Vec::new();
        for _ in 0..samples {
            let (translation, ms) = translate_once(source);
            latency_sum += ms;
            let idx = match classify(&translation) {
                Verdict::Clean => 0,
                Verdict::Contaminated => 1,
                Verdict::Echo => 2,
                Verdict::Empty => 3,
                Verdict::Error => 4,
            };
            counts[idx] += 1;
            totals[idx] += 1;
            if first_outputs.len() < 2 {
                first_outputs.push(translation);
            }
            total_runs += 1;
        }
        let avg_ms = latency_sum / samples.max(1) as u128;
        let src_preview: String = source.chars().take(48).collect();
        println!(
            "{label}: clean={} contam={} echo={} empty={} error={} (n={samples}, avg={avg_ms}ms)  src={src_preview:?}",
            counts[0], counts[1], counts[2], counts[3], counts[4]
        );
        for (i, o) in first_outputs.iter().enumerate() {
            let preview: String = o.chars().take(90).collect();
            println!("    sample[{i}] -> {preview:?}");
        }
    }

    // User-visible failure = echo + empty (source fallback) + contaminated.
    // Errors (timeouts) are a separate axis and excluded from the rate.
    let visible_fail = totals[1] + totals[2] + totals[3];
    let fail_rate = if total_runs > 0 {
        100.0 * visible_fail as f32 / total_runs as f32
    } else {
        0.0
    };
    println!(
        "\nTOTAL (n={total_runs}): clean={} contam={} echo={} empty={} error={}",
        totals[0], totals[1], totals[2], totals[3], totals[4]
    );
    println!(
        "VISIBLE-FAIL (echo+empty+contam) = {visible_fail}/{total_runs} ({fail_rate:.1}%)  temp={temp_label}"
    );

    Ok(())
}
