//! Multilingual cross-tier embedded LLM benchmark runner.
//!
//! Phase 27: extends `bench_all_tiers.rs` to 28 language pairs
//! (Tier A+B+C from `docs/benchmarks/2026-05-12-phase27-*.md`).
//! Each registered tier is loaded ONCE, then exercised against
//! every (source, target) pair to amortise the per-tier cold
//! start (Vulkan shader compile, model mmap, KV alloc).
//!
//! Usage:
//!     cargo run --example bench_multilang_tiers --release \
//!         --features embedded-llm-vulkan \
//!         -- [--samples N] [--app-data PATH] [--write-report PATH] \
//!            [--pairs en->ja,ja->en,...]
//!
//! Defaults match `bench_all_tiers`:
//!   --samples 5
//!   --app-data %APPDATA%\io.moonlens.app (Windows) / XDG / macOS
//!
//! --pairs accepts a comma-separated subset; default is all 28.

use std::{collections::HashMap, fs, path::PathBuf, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use moonlens_lib::{
    model_registry::{ChatTemplate, ModelTier, TIERS},
    providers::{
        translate_embedded_llm::{default_model_dir, EmbeddedLlmTranslator},
        Provider, StreamingChunkSink, TaskInput,
    },
};

/// 5-sample fixture per source language. Each row covers the same
/// 5 conceptual buckets so cross-language comparison stays meaningful:
///   1. greeting        — "Hello, world." class
///   2. idiomatic       — pangram-style action sentence (fox/dog)
///   3. abstract        — "Welcome to the future of translation."
///   4. meta            — sentence about the translation process
///   5. technical       — performance / hardware / runtime
struct LanguageFixture {
    code: &'static str,
    label: &'static str,
    samples: [&'static str; 5],
}

const FIXTURES: &[LanguageFixture] = &[
    LanguageFixture {
        code: "en",
        label: "English",
        samples: [
            "Hello, world.",
            "The quick brown fox jumps over the lazy dog.",
            "Welcome to the future of translation.",
            "This sentence demonstrates how translation handles structure.",
            "Performance varies across hardware and runtime configurations.",
        ],
    },
    LanguageFixture {
        code: "ja",
        label: "Japanese",
        samples: [
            "こんにちは、世界。",
            "素早い茶色の狐が怠惰な犬を飛び越える。",
            "翻訳の未来へようこそ。",
            "この文は翻訳が文構造をどう扱うかを示している。",
            "パフォーマンスはハードウェアやランタイム設定によって異なります。",
        ],
    },
    LanguageFixture {
        code: "zh-CN",
        label: "Chinese (Simplified)",
        samples: [
            "你好,世界。",
            "敏捷的棕色狐狸跳过懒狗。",
            "欢迎来到翻译的未来。",
            "这个句子展示了翻译如何处理结构。",
            "性能因硬件和运行时配置而异。",
        ],
    },
    LanguageFixture {
        code: "zh-TW",
        label: "Chinese (Traditional)",
        samples: [
            "你好,世界。",
            "敏捷的棕色狐狸跳過懶狗。",
            "歡迎來到翻譯的未來。",
            "這個句子展示了翻譯如何處理結構。",
            "效能因硬體和執行階段組態而異。",
        ],
    },
    LanguageFixture {
        code: "ko",
        label: "Korean",
        samples: [
            "안녕, 세상.",
            "빠른 갈색 여우가 게으른 개를 뛰어넘는다.",
            "번역의 미래에 오신 것을 환영합니다.",
            "이 문장은 번역이 구조를 어떻게 다루는지 보여준다.",
            "성능은 하드웨어와 런타임 구성에 따라 달라진다.",
        ],
    },
    LanguageFixture {
        code: "es",
        label: "Spanish",
        samples: [
            "Hola, mundo.",
            "El rápido zorro marrón salta sobre el perro perezoso.",
            "Bienvenido al futuro de la traducción.",
            "Esta oración demuestra cómo la traducción maneja la estructura.",
            "El rendimiento varía según el hardware y la configuración de tiempo de ejecución.",
        ],
    },
    LanguageFixture {
        code: "pt",
        label: "Portuguese",
        samples: [
            "Olá, mundo.",
            "A rápida raposa marrom salta sobre o cão preguiçoso.",
            "Bem-vindo ao futuro da tradução.",
            "Esta frase demonstra como a tradução lida com a estrutura.",
            "O desempenho varia conforme o hardware e as configurações de tempo de execução.",
        ],
    },
    LanguageFixture {
        code: "fr",
        label: "French",
        samples: [
            "Bonjour, le monde.",
            "Le renard brun et rapide saute par-dessus le chien paresseux.",
            "Bienvenue dans l'avenir de la traduction.",
            "Cette phrase démontre comment la traduction gère la structure.",
            "Les performances varient selon le matériel et la configuration d'exécution.",
        ],
    },
    LanguageFixture {
        code: "de",
        label: "German",
        samples: [
            "Hallo, Welt.",
            "Der schnelle braune Fuchs springt über den faulen Hund.",
            "Willkommen in der Zukunft der Übersetzung.",
            "Dieser Satz zeigt, wie eine Übersetzung mit Struktur umgeht.",
            "Die Leistung variiert je nach Hardware und Laufzeitkonfiguration.",
        ],
    },
    LanguageFixture {
        code: "ru",
        label: "Russian",
        samples: [
            "Привет, мир.",
            "Быстрая коричневая лиса перепрыгивает через ленивую собаку.",
            "Добро пожаловать в будущее перевода.",
            "Это предложение показывает, как перевод обрабатывает структуру.",
            "Производительность зависит от оборудования и конфигурации времени выполнения.",
        ],
    },
    LanguageFixture {
        code: "vi",
        label: "Vietnamese",
        samples: [
            "Xin chào, thế giới.",
            "Con cáo nâu nhanh nhẹn nhảy qua con chó lười.",
            "Chào mừng đến với tương lai của dịch thuật.",
            "Câu này minh họa cách bản dịch xử lý cấu trúc.",
            "Hiệu suất thay đổi tùy theo phần cứng và cấu hình thời gian chạy.",
        ],
    },
    LanguageFixture {
        code: "th",
        label: "Thai",
        samples: [
            "สวัสดี โลก",
            "สุนัขจิ้งจอกสีน้ำตาลคล่องแคล่วกระโดดข้ามสุนัขขี้เกียจ",
            "ยินดีต้อนรับสู่อนาคตของการแปลภาษา",
            "ประโยคนี้แสดงให้เห็นว่าการแปลจัดการกับโครงสร้างอย่างไร",
            "ประสิทธิภาพแตกต่างกันไปตามฮาร์ดแวร์และการกำหนดค่ารันไทม์",
        ],
    },
    LanguageFixture {
        code: "id",
        label: "Indonesian",
        samples: [
            "Halo, dunia.",
            "Rubah cokelat yang gesit melompati anjing yang malas.",
            "Selamat datang di masa depan terjemahan.",
            "Kalimat ini menunjukkan bagaimana terjemahan menangani struktur.",
            "Performa bervariasi tergantung pada perangkat keras dan konfigurasi runtime.",
        ],
    },
    LanguageFixture {
        code: "hi",
        label: "Hindi",
        samples: [
            "नमस्ते, दुनिया।",
            "तेज़ भूरी लोमड़ी आलसी कुत्ते के ऊपर से कूदती है।",
            "अनुवाद के भविष्य में आपका स्वागत है।",
            "यह वाक्य दिखाता है कि अनुवाद संरचना को कैसे संभालता है।",
            "प्रदर्शन हार्डवेयर और रनटाइम कॉन्फ़िगरेशन के आधार पर भिन्न होता है।",
        ],
    },
    LanguageFixture {
        code: "ar",
        label: "Arabic",
        samples: [
            "مرحبا، أيها العالم.",
            "الثعلب البني السريع يقفز فوق الكلب الكسول.",
            "مرحبا بكم في مستقبل الترجمة.",
            "هذه الجملة توضح كيف تتعامل الترجمة مع البنية.",
            "يختلف الأداء باختلاف الأجهزة وتكوينات وقت التشغيل.",
        ],
    },
];

/// All 28 (source, target) pairs. Tier A+B+C from
/// docs/benchmarks/2026-05-12-phase27-* — every non-ja language
/// paired both directions with ja.
const PAIRS: &[(&str, &str)] = &[
    // Tier A — core ja-paired
    ("en", "ja"),
    ("ja", "en"),
    ("zh-CN", "ja"),
    ("ja", "zh-CN"),
    ("ko", "ja"),
    ("ja", "ko"),
    // Tier B — high-frequency ja-paired
    ("zh-TW", "ja"),
    ("ja", "zh-TW"),
    ("es", "ja"),
    ("ja", "es"),
    ("pt", "ja"),
    ("ja", "pt"),
    ("fr", "ja"),
    ("ja", "fr"),
    ("de", "ja"),
    ("ja", "de"),
    // Tier C — regional / specialty ja-paired
    ("vi", "ja"),
    ("ja", "vi"),
    ("th", "ja"),
    ("ja", "th"),
    ("id", "ja"),
    ("ja", "id"),
    ("hi", "ja"),
    ("ja", "hi"),
    ("ru", "ja"),
    ("ja", "ru"),
    ("ar", "ja"),
    ("ja", "ar"),
];

const APP_IDENTIFIER: &str = "io.moonlens.app";

#[derive(Debug)]
struct CliArgs {
    samples: u32,
    app_data: PathBuf,
    write_report: Option<PathBuf>,
    pairs_filter: Option<Vec<(String, String)>>,
}

fn parse_args() -> Result<CliArgs> {
    let mut samples = 5u32;
    let mut app_data: Option<PathBuf> = None;
    let mut write_report: Option<PathBuf> = None;
    let mut pairs_filter: Option<Vec<(String, String)>> = None;
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
            "--pairs" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--pairs needs a value"))?;
                let mut out: Vec<(String, String)> = Vec::new();
                for spec in v.split(',') {
                    let (s, t) = spec
                        .split_once("->")
                        .ok_or_else(|| anyhow!("--pairs entry must be `src->tgt`, got `{spec}`"))?;
                    out.push((s.trim().to_string(), t.trim().to_string()));
                }
                pairs_filter = Some(out);
            }
            "-h" | "--help" => {
                eprintln!(
                    "usage: cargo run --example bench_multilang_tiers --release \
                     --features embedded-llm -- \
                     [--samples N] [--app-data PATH] [--write-report PATH] \
                     [--pairs en->ja,ja->en,...]"
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
        pairs_filter,
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
    index: u32,
    output_tokens: u32,
    total_ms: u64,
    prompt_eval_ms: u64,
    decode_ms: u64,
    wall_tps: f64,
    decode_tps: f64,
    source_text: String,
    preview: String,
}

#[derive(Debug, Clone)]
struct PairResult {
    tier_id: String,
    tier_display_name: String,
    template: ChatTemplate,
    source: String,
    target: String,
    samples: Vec<SampleResult>,
    p50_decode: f64,
    mean_decode: f64,
    p50_wall: f64,
    mean_wall: f64,
}

fn fixture_for(code: &str) -> Option<&'static LanguageFixture> {
    FIXTURES.iter().find(|f| f.code == code)
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
    let pairs: Vec<(&str, &str)> = match args.pairs_filter.as_ref() {
        Some(filter) => filter
            .iter()
            .map(|(s, t)| (s.as_str(), t.as_str()))
            .collect(),
        None => PAIRS.iter().copied().collect(),
    };
    // Validate that every (src, tgt) has a known source fixture.
    for (s, _t) in &pairs {
        if fixture_for(s).is_none() {
            return Err(anyhow!(
                "no source fixture registered for `{s}`; \
                 add a LanguageFixture entry first"
            ));
        }
    }

    println!("== bench_multilang_tiers ==");
    println!("samples per (tier,pair): {}", args.samples);
    println!("pairs                    : {}", pairs.len());
    println!("app-data                 : {}", args.app_data.display());
    println!("models dir               : {}", models_dir.display());
    println!();

    if !models_dir.is_dir() {
        return Err(anyhow!(
            "models dir does not exist: {}",
            models_dir.display()
        ));
    }

    // tier outer, pair inner — minimises cold-start cost (Vulkan
    // shader compile + Llama mmap) by reusing the loaded engine
    // across all pair runs for a given tier.
    let mut all_results: Vec<PairResult> = Vec::new();
    for tier in TIERS {
        let primary_path = models_dir.join(tier.primary_filename());
        if !primary_path.is_file() {
            println!(
                "SKIP  {}  ({}) — not downloaded",
                tier.id, tier.display_name
            );
            continue;
        }
        println!("==== {} ({}) ====", tier.id, tier.display_name);
        // Build one translator instance per tier so the worker /
        // KV cache is reused across pairs. Engine spawn happens on
        // first `run_streaming` call (preload below).
        let cfg = serde_json::json!({
            "model_path": primary_path.display().to_string(),
            "repo": tier.repo,
            "filename_remote": tier.primary_filename(),
            "license": tier.license,
            "license_url": tier.license_url,
            "num_predict": 64u64,
        });
        let translator = EmbeddedLlmTranslator::new("bench.multilang", &cfg);
        print!("      preload");
        use std::io::Write as _;
        std::io::stdout().flush().ok();
        let preload_start = Instant::now();
        if let Err(e) = translator.preload().await {
            println!("\n      FAIL preload: {e}");
            continue;
        }
        println!(" — ready in {:.1}s", preload_start.elapsed().as_secs_f64());

        for (src, tgt) in &pairs {
            println!("   -- {src} -> {tgt} --");
            let fixture = fixture_for(src).expect("validated above");
            match bench_one_pair(&translator, tier, fixture, tgt, args.samples).await {
                Ok(result) => {
                    println!(
                        "      decode tok/s: p50={:.1}  mean={:.1}   wall p50={:.1}  mean={:.1}",
                        result.p50_decode, result.mean_decode, result.p50_wall, result.mean_wall
                    );
                    all_results.push(result);
                }
                Err(e) => println!("      FAIL: {e:#}"),
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

async fn bench_one_pair(
    translator: &EmbeddedLlmTranslator,
    tier: &ModelTier,
    fixture: &LanguageFixture,
    target_lang: &str,
    n_samples: u32,
) -> Result<PairResult> {
    let mut samples_out = Vec::with_capacity(n_samples as usize);
    for i in 0..n_samples {
        let text = fixture.samples[(i as usize) % fixture.samples.len()].to_string();
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
                if p.chars().count() < 120 {
                    p.push_str(&delta);
                }
            }
        });

        let started = Instant::now();
        translator
            .run_streaming(
                TaskInput::Translate {
                    text: text.clone(),
                    source_language: fixture.code.into(),
                    target_language: target_lang.into(),
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
            "      sample {:>1}/{}: {:>3} tok / {:>5} ms  d={:>5.1}  w={:>5.1}  | {}",
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
            source_text: text,
            preview: preview_text,
        });
    }
    let (p50_decode, mean_decode) = stats(samples_out.iter().map(|s| s.decode_tps));
    let (p50_wall, mean_wall) = stats(samples_out.iter().map(|s| s.wall_tps));
    Ok(PairResult {
        tier_id: tier.id.into(),
        tier_display_name: tier.display_name.into(),
        template: tier.chat_template,
        source: fixture.code.into(),
        target: target_lang.into(),
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

fn print_summary(results: &[PairResult]) {
    if results.is_empty() {
        println!("No (tier,pair) runs completed.");
        return;
    }
    println!("\n== Summary: per-pair speed leaderboard (top 3 tiers by wall mean) ==");
    // Group results by pair string for the leaderboard.
    let mut by_pair: HashMap<(String, String), Vec<PairResult>> = HashMap::new();
    for r in results {
        by_pair
            .entry((r.source.clone(), r.target.clone()))
            .or_default()
            .push(r.clone());
    }
    let mut pair_keys: Vec<(String, String)> = by_pair.keys().cloned().collect();
    pair_keys.sort();
    for (src, tgt) in &pair_keys {
        let mut group = by_pair[&(src.clone(), tgt.clone())].clone();
        group.sort_by(|a, b| {
            b.mean_wall
                .partial_cmp(&a.mean_wall)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top: Vec<String> = group
            .iter()
            .take(3)
            .map(|r| format!("{}={:.0}", short_name(&r.tier_display_name), r.mean_wall))
            .collect();
        println!("  {src}->{tgt:<5} : {}", top.join("  "));
    }
}

fn short_name(s: &str) -> String {
    // Trim parenthetical detail so leaderboard rows stay short.
    s.split_once(" (")
        .map(|(a, _)| a.to_string())
        .unwrap_or_else(|| s.to_string())
}

fn template_label(t: ChatTemplate) -> &'static str {
    match t {
        ChatTemplate::ChatMl => "ChatML",
        ChatTemplate::ChatMlQwen3NoThink => "ChatML+nt",
        ChatTemplate::Gemma => "Gemma",
    }
}

fn render_markdown(results: &[PairResult], samples: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Multilingual Embedded LLM Bench — {} samples / (tier, pair)\n\n",
        samples
    ));
    out.push_str("Generated by `cargo run --example bench_multilang_tiers`\n\n");

    // ── Overall speed matrix: rows = pair, columns = tier ──
    out.push_str("## Speed matrix (wall mean tok/s)\n\n");
    let mut tiers_seen: Vec<String> = Vec::new();
    let mut tier_label: HashMap<String, String> = HashMap::new();
    for r in results {
        if !tiers_seen.contains(&r.tier_id) {
            tiers_seen.push(r.tier_id.clone());
            tier_label.insert(r.tier_id.clone(), r.tier_display_name.clone());
        }
    }
    let mut by_pair: HashMap<(String, String), HashMap<String, f64>> = HashMap::new();
    for r in results {
        by_pair
            .entry((r.source.clone(), r.target.clone()))
            .or_default()
            .insert(r.tier_id.clone(), r.mean_wall);
    }
    let mut pair_keys: Vec<(String, String)> = by_pair.keys().cloned().collect();
    pair_keys.sort();

    out.push_str("| Pair |");
    for tid in &tiers_seen {
        out.push_str(&format!(" {} |", short_name(&tier_label[tid])));
    }
    out.push('\n');
    out.push_str("|---|");
    for _ in &tiers_seen {
        out.push_str("---:|");
    }
    out.push('\n');
    for (src, tgt) in &pair_keys {
        out.push_str(&format!("| {src}->{tgt} |"));
        for tid in &tiers_seen {
            let v = by_pair[&(src.clone(), tgt.clone())]
                .get(tid)
                .copied()
                .unwrap_or(0.0);
            out.push_str(&format!(" {v:.0} |"));
        }
        out.push('\n');
    }
    out.push('\n');

    // ── Per-pair sample detail ──
    out.push_str("## Per-pair sample detail\n\n");
    for (src, tgt) in &pair_keys {
        out.push_str(&format!("### {src} → {tgt}\n\n"));
        out.push_str("Source fixtures:\n\n");
        let fixture = fixture_for(src).expect("validated");
        for (i, s) in fixture.samples.iter().enumerate() {
            out.push_str(&format!("- {}. `{}`\n", i + 1, s.replace('|', "\\|")));
        }
        out.push('\n');
        // Per-tier sample outputs for this pair.
        for tid in &tiers_seen {
            let r_opt = results
                .iter()
                .find(|r| r.tier_id == *tid && r.source == *src && r.target == *tgt);
            let r = match r_opt {
                Some(r) => r,
                None => continue,
            };
            out.push_str(&format!(
                "#### {} ({})  · {} · decode mean {:.0} · wall mean {:.0}\n\n",
                r.tier_display_name,
                r.tier_id,
                template_label(r.template),
                r.mean_decode,
                r.mean_wall,
            ));
            out.push_str("| # | tokens | total ms | preview |\n");
            out.push_str("|---:|---:|---:|---|\n");
            for s in &r.samples {
                let pv = s.preview.lines().next().unwrap_or("").replace('|', "\\|");
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    s.index, s.output_tokens, s.total_ms, pv
                ));
            }
            out.push('\n');
        }
    }
    out
}
