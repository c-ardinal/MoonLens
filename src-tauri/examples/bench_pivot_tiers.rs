//! Pivot translation benchmark.
//!
//! Phase 27 v2: route X↔ja translations via an `en` intermediate
//! using a fixed two-stage pipeline:
//!   ja→X   : JA-Std+ (llm-jp 3.1 1.8B)  ja→en   →  Edge+ (Gemma 4 E4B)  en→X
//!   X→ja   : Edge+ (Gemma 4 E4B)         X→en   →  JA-Std+ (llm-jp 3.1 1.8B)  en→ja
//!   en→ja  : single step via JA-Std+
//!   ja→en  : single step via JA-Std+
//!
//! Compares against the direct results from
//! `bench_multilang_tiers` (Phase 27 v1). Goal: see whether routing
//! through en rescues the pairs that JA-Compact / JA-Std+ /
//! Edge+ broke down on when used directly.
//!
//! Usage:
//!     cargo run --example bench_pivot_tiers --release \
//!         --features embedded-llm-vulkan \
//!         -- [--samples N] [--write-report PATH] [--pairs en->ja,ja->en,...]
//!
//! The two pivot tiers are loaded once at startup and reused
//! across every pair, so the per-pair overhead is just 1-2
//! decode passes (no model reload).

// See sibling `bench_all_tiers.rs` for the dead_code rationale —
// bench data structures capture every metric for future analysis
// even when the current report renders a subset.
#![allow(dead_code)]

use std::{collections::HashMap, fs, path::{Path, PathBuf}, sync::Arc, time::Instant};

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;

use moonlens_lib::{
    model_registry::{tier_by_id, ModelTier},
    providers::{
        translate_embedded_llm::{default_model_dir, EmbeddedLlmTranslator},
        Provider, StreamingChunkSink, TaskInput,
    },
};

// ---- pivot configuration ----------------------------------------
const JA_BRIDGE_TIER: &str = "llm-jp-3.1-1.8b-instruct4-q4_k_m";
const MULTI_BRIDGE_TIER: &str = "gemma-4-e4b-it-q4_k_m";

// ---- fixtures (same as bench_multilang_tiers) -------------------
struct LanguageFixture {
    code: &'static str,
    samples: [&'static str; 5],
}

const FIXTURES: &[LanguageFixture] = &[
    LanguageFixture {
        code: "en",
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
        samples: [
            "مرحبا، أيها العالم.",
            "الثعلب البني السريع يقفز فوق الكلب الكسول.",
            "مرحبا بكم في مستقبل الترجمة.",
            "هذه الجملة توضح كيف تتعامل الترجمة مع البنية.",
            "يختلف الأداء باختلاف الأجهزة وتكوينات وقت التشغيل.",
        ],
    },
];

const PAIRS: &[(&str, &str)] = &[
    ("en", "ja"),
    ("ja", "en"),
    ("zh-CN", "ja"),
    ("ja", "zh-CN"),
    ("ko", "ja"),
    ("ja", "ko"),
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
                app_data = Some(PathBuf::from(
                    iter.next()
                        .ok_or_else(|| anyhow!("--app-data needs a value"))?,
                ));
            }
            "--write-report" => {
                write_report = Some(PathBuf::from(
                    iter.next()
                        .ok_or_else(|| anyhow!("--write-report needs a value"))?,
                ));
            }
            "--pairs" => {
                let v = iter
                    .next()
                    .ok_or_else(|| anyhow!("--pairs needs a value"))?;
                let mut out = Vec::new();
                for spec in v.split(',') {
                    let (s, t) = spec
                        .split_once("->")
                        .ok_or_else(|| anyhow!("`{spec}` is not `src->tgt`"))?;
                    out.push((s.trim().to_string(), t.trim().to_string()));
                }
                pairs_filter = Some(out);
            }
            "-h" | "--help" => {
                eprintln!("usage: cargo run --example bench_pivot_tiers --release --features embedded-llm -- [--samples N] [--app-data PATH] [--write-report PATH] [--pairs ...]");
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
struct StageResult {
    output_tokens: u32,
    total_ms: u64,
    decode_ms: u64,
    wall_tps: f64,
    decode_tps: f64,
    output_text: String,
}

#[derive(Debug, Clone)]
struct PivotSample {
    index: u32,
    source_text: String,
    /// Path through the pivot: "ja->en->X", "X->en->ja", or "direct"
    /// for en<->ja pairs that don't need a pivot.
    route: String,
    step1: StageResult,
    step2: Option<StageResult>,
}

#[derive(Debug, Clone)]
struct PivotPairResult {
    source: String,
    target: String,
    samples: Vec<PivotSample>,
    p50_total_ms: f64,
    mean_total_ms: f64,
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
        None => PAIRS.to_vec(),
    };
    for (s, _) in &pairs {
        if fixture_for(s).is_none() {
            return Err(anyhow!("no source fixture registered for `{s}`"));
        }
    }

    let ja_bridge_tier = tier_by_id(JA_BRIDGE_TIER)
        .ok_or_else(|| anyhow!("registry missing tier `{JA_BRIDGE_TIER}`"))?;
    let multi_bridge_tier = tier_by_id(MULTI_BRIDGE_TIER)
        .ok_or_else(|| anyhow!("registry missing tier `{MULTI_BRIDGE_TIER}`"))?;
    let ja_path = models_dir.join(ja_bridge_tier.primary_filename());
    let multi_path = models_dir.join(multi_bridge_tier.primary_filename());
    if !ja_path.is_file() {
        return Err(anyhow!("ja bridge GGUF missing: {}", ja_path.display()));
    }
    if !multi_path.is_file() {
        return Err(anyhow!(
            "multi bridge GGUF missing: {}",
            multi_path.display()
        ));
    }

    println!("== bench_pivot_tiers ==");
    println!("samples per pair       : {}", args.samples);
    println!("pairs                  : {}", pairs.len());
    println!(
        "ja-bridge model        : {} @ {}",
        JA_BRIDGE_TIER,
        ja_path.display()
    );
    println!(
        "multi-bridge model     : {} @ {}",
        MULTI_BRIDGE_TIER,
        multi_path.display()
    );
    println!();

    let ja_bridge = make_translator(ja_bridge_tier, &ja_path);
    let multi_bridge = make_translator(multi_bridge_tier, &multi_path);

    print!("preload ja-bridge");
    use std::io::Write as _;
    std::io::stdout().flush().ok();
    let t = Instant::now();
    ja_bridge
        .preload()
        .await
        .map_err(|e| anyhow!("ja-bridge preload: {e}"))?;
    println!(" — ready in {:.1}s", t.elapsed().as_secs_f64());

    print!("preload multi-bridge");
    std::io::stdout().flush().ok();
    let t = Instant::now();
    multi_bridge
        .preload()
        .await
        .map_err(|e| anyhow!("multi-bridge preload: {e}"))?;
    println!(" — ready in {:.1}s", t.elapsed().as_secs_f64());
    println!();

    let mut all_pairs: Vec<PivotPairResult> = Vec::new();
    for (src, tgt) in &pairs {
        let fx = fixture_for(src).expect("validated");
        println!("---- {src} -> {tgt} ----");
        let mut samples_out = Vec::with_capacity(args.samples as usize);
        let mut totals: Vec<f64> = Vec::with_capacity(args.samples as usize);
        for i in 0..args.samples {
            let source_text = fx.samples[(i as usize) % fx.samples.len()].to_string();
            let sample =
                pivot_one_sample(&ja_bridge, &multi_bridge, src, tgt, &source_text, i + 1).await?;
            let total_ms =
                sample.step1.total_ms + sample.step2.as_ref().map(|s| s.total_ms).unwrap_or(0);
            totals.push(total_ms as f64);
            let intermediate = if sample.step2.is_some() {
                format!(
                    " | en: `{}`",
                    sample.step1.output_text.lines().next().unwrap_or("").trim()
                )
            } else {
                String::new()
            };
            let final_text = sample
                .step2
                .as_ref()
                .map(|s| s.output_text.as_str())
                .unwrap_or(sample.step1.output_text.as_str());
            println!(
                "   sample {}/{} [{}]: {} ms total{} | {} -> `{}`",
                i + 1,
                args.samples,
                sample.route,
                total_ms,
                intermediate,
                tgt,
                final_text.lines().next().unwrap_or("").trim()
            );
            samples_out.push(sample);
        }
        totals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p50 = if totals.is_empty() {
            0.0
        } else {
            totals[totals.len() / 2]
        };
        let mean = if totals.is_empty() {
            0.0
        } else {
            totals.iter().sum::<f64>() / totals.len() as f64
        };
        println!("   total ms: p50={:.0}  mean={:.0}", p50, mean);
        all_pairs.push(PivotPairResult {
            source: src.to_string(),
            target: tgt.to_string(),
            samples: samples_out,
            p50_total_ms: p50,
            mean_total_ms: mean,
        });
    }

    if let Some(path) = args.write_report.as_ref() {
        let md = render_markdown(&all_pairs, args.samples);
        fs::write(path, md).with_context(|| format!("write {}", path.display()))?;
        println!("\nreport written to {}", path.display());
    }
    Ok(())
}

fn fixture_for(code: &str) -> Option<&'static LanguageFixture> {
    FIXTURES.iter().find(|f| f.code == code)
}

fn make_translator(tier: &ModelTier, model_path: &Path) -> EmbeddedLlmTranslator {
    let cfg = serde_json::json!({
        "model_path": model_path.display().to_string(),
        "repo": tier.repo,
        "filename_remote": tier.primary_filename(),
        "license": tier.license,
        "license_url": tier.license_url,
        "num_predict": 64u64,
    });
    EmbeddedLlmTranslator::new("bench.pivot", &cfg)
}

async fn pivot_one_sample(
    ja_bridge: &EmbeddedLlmTranslator,
    multi_bridge: &EmbeddedLlmTranslator,
    src: &str,
    tgt: &str,
    source_text: &str,
    index: u32,
) -> Result<PivotSample> {
    let route = if (src == "en" && tgt == "ja") || (src == "ja" && tgt == "en") {
        "direct-ja-bridge"
    } else if src == "ja" {
        "ja->en->X"
    } else if tgt == "ja" {
        "X->en->ja"
    } else {
        return Err(anyhow!("unsupported route {src}->{tgt}"));
    };

    match route {
        "direct-ja-bridge" => {
            let s = run_one_step(ja_bridge, src, tgt, source_text).await?;
            Ok(PivotSample {
                index,
                source_text: source_text.to_string(),
                route: route.to_string(),
                step1: s,
                step2: None,
            })
        }
        "ja->en->X" => {
            // ja -> en via ja-bridge (llm-jp), then en -> X via multi-bridge (Gemma 4)
            let s1 = run_one_step(ja_bridge, "ja", "en", source_text).await?;
            let intermediate = clean_text(&s1.output_text);
            let s2 = run_one_step(multi_bridge, "en", tgt, &intermediate).await?;
            Ok(PivotSample {
                index,
                source_text: source_text.to_string(),
                route: route.to_string(),
                step1: s1,
                step2: Some(s2),
            })
        }
        "X->en->ja" => {
            // X -> en via multi-bridge (Gemma 4), then en -> ja via ja-bridge (llm-jp)
            let s1 = run_one_step(multi_bridge, src, "en", source_text).await?;
            let intermediate = clean_text(&s1.output_text);
            let s2 = run_one_step(ja_bridge, "en", "ja", &intermediate).await?;
            Ok(PivotSample {
                index,
                source_text: source_text.to_string(),
                route: route.to_string(),
                step1: s1,
                step2: Some(s2),
            })
        }
        _ => unreachable!(),
    }
}

/// Strip EOS markers and any trailing garbage from the
/// intermediate output before feeding it into the next stage.
/// Phase 27 pivot v1 showed that JA-Std+ (llm-jp) leaks the
/// literal text `|im_end|>` (sometimes missing the leading `<`)
/// because its EOS token is not being respected by the runtime;
/// the leaked marker confuses the Step 2 model. Edge+ leaks
/// `<end_of_turn>` / `<end_of_of_turn>` cleanly.
///
/// We truncate at the FIRST occurrence of either family of
/// markers so garbage tokens after the actual translation
/// (e.g. "名無し" appearing after "Hello, world.") are also
/// dropped.
fn clean_text(s: &str) -> String {
    let mut cut = s.len();
    for needle in [
        "<|im_end|>",
        "|im_end|>",
        "<|im_end",
        "|im_end",
        "<end_of_of_turn>",
        "<end_of_turn>",
        "<end_of",
    ] {
        if let Some(p) = s.find(needle) {
            if p < cut {
                cut = p;
            }
        }
    }
    s[..cut].trim().to_string()
}

async fn run_one_step(
    translator: &EmbeddedLlmTranslator,
    src: &str,
    tgt: &str,
    text: &str,
) -> Result<StageResult> {
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
            pv.lock().push_str(&delta);
        }
    });

    let started = Instant::now();
    translator
        .run_streaming(
            TaskInput::Translate {
                text: text.to_string(),
                source_language: src.into(),
                target_language: tgt.into(),
                glossary: Vec::new(),
            },
            sink,
        )
        .await
        .map_err(|e| anyhow!("step inference failed: {e}"))?;
    let total_ms = started.elapsed().as_millis() as u64;

    let n_tokens = *token_count.lock();
    let preview_text = preview.lock().clone();
    let f = *first_at.lock();
    let l = *last_at.lock();
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
    Ok(StageResult {
        output_tokens: n_tokens,
        total_ms,
        decode_ms,
        wall_tps,
        decode_tps,
        output_text: preview_text,
    })
}

fn render_markdown(results: &[PivotPairResult], samples: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# Pivot Embedded LLM Bench — {} samples / pair\n\n",
        samples
    ));
    out.push_str("Generated by `cargo run --example bench_pivot_tiers`. \
        Pivot pipeline: **ja→X = JA-Std+ ja→en + Edge+ en→X**, \
        **X→ja = Edge+ X→en + JA-Std+ en→ja**, \
        **en↔ja direct via JA-Std+**.\n\n");
    out.push_str("## Per-pair latency summary\n\n");
    out.push_str("| Pair | Route | total ms p50 | total ms mean |\n");
    out.push_str("|---|---|---:|---:|\n");
    for r in results {
        let route = r
            .samples
            .first()
            .map(|s| s.route.clone())
            .unwrap_or_else(|| "?".to_string());
        out.push_str(&format!(
            "| {} -> {} | {} | {:.0} | {:.0} |\n",
            r.source, r.target, route, r.p50_total_ms, r.mean_total_ms
        ));
    }
    out.push_str("\n## Per-pair sample detail\n\n");
    for r in results {
        out.push_str(&format!("### {} -> {}\n\n", r.source, r.target));
        out.push_str(
            "| # | route | step1 ms | step1 output (en intermediate) | step2 ms | final output |\n",
        );
        out.push_str("|---:|---|---:|---|---:|---|\n");
        for s in &r.samples {
            let s1_out = s
                .step1
                .output_text
                .lines()
                .next()
                .unwrap_or("")
                .replace('|', "\\|");
            let (s2_ms, s2_out) = match s.step2.as_ref() {
                Some(s2) => (
                    s2.total_ms.to_string(),
                    s2.output_text
                        .lines()
                        .next()
                        .unwrap_or("")
                        .replace('|', "\\|"),
                ),
                None => ("-".to_string(), "(single step)".to_string()),
            };
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                s.index, s.route, s.step1.total_ms, s1_out, s2_ms, s2_out
            ));
        }
        out.push('\n');
    }
    out
}

// silence "unused" warnings when the multilang structures evolve
#[allow(dead_code)]
fn _unused() -> Option<HashMap<String, String>> {
    None
}
