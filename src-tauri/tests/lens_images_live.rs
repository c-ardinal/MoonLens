//! Image-in, translation-out end-to-end test of the capture pipeline the
//! Lens window, range translation and full-screen translation share.
//!
//! Each fixture PNG in `tests/fixtures/lens/` goes through the real
//! providers (PP-OCR on rten, the built-in llama.cpp translator) via
//! [`Pipeline::run_capture_flow`], the same call path as
//! `capture_lens_now` after its screen grab. Asserted per fixture:
//!
//! 1. OCR recall against the ground truth JSON next to the image.
//! 2. Every block that is not already Japanese gets a Japanese
//!    translation (kana present, no long Latin run left over).
//! 3. Expected key terms appear in the joined translation.
//!
//! `#[ignore]` because it needs the downloaded models (about 2 GB) in
//! the app's models folder (`<local data>/io.moonlens.app/models`, the
//! same folder the app downloads to): PP-OCR and
//! `plamo-3-translate-2b`. Run with
//!
//! ```sh
//! cd src-tauri
//! cargo test --release --test lens_images_live -- --ignored --nocapture
//! ```
//!
//! `LENS_FIXTURE=english0001` limits the run to one fixture.

use std::{path::PathBuf, sync::Arc, time::Instant};

use moonlens_lib::{
    pipeline::Pipeline,
    plugins::registry::PluginRegistry,
    providers::{Capability, TaskInput, TaskOutput},
    settings::SettingsStore,
    storage::Storage,
};

struct Case {
    name: &'static str,
    /// Terms of which at least one must appear in the translation, per
    /// group (all groups must match).
    expect: &'static [&'static [&'static str]],
}

const CASES: &[Case] = &[
    Case {
        name: "english0001",
        expect: &[&["ステラ"], &["チーズ"], &["水曜"]],
    },
    Case {
        name: "english0002",
        expect: &[&["北風"], &["太陽"], &["旅人", "旅行者"]],
    },
    Case {
        name: "english0003",
        expect: &[&["虹"], &["雨"]],
    },
    Case {
        name: "english0004",
        expect: &[&["新聞"]],
    },
    Case {
        name: "synth_title_body",
        expect: &[&["狐", "キツネ", "きつね"], &["犬"]],
    },
    Case {
        name: "synth_fr_paragraph",
        expect: &[&["母", "ママ"], &["電報"]],
    },
    Case {
        name: "synth_de_paragraph",
        expect: &[&["グレゴール", "ザムザ"], &["ベッド", "寝台"]],
    },
    // Full-screen captures (browser page, 1080p and 1440p at 150 %):
    // many lines in one image, as in full-screen / large range captures.
    Case {
        name: "screen_fhd",
        expect: &[&["北風"], &["太陽"], &["虹"]],
    },
    Case {
        name: "screen_qhd150",
        expect: &[&["北風"], &["太陽"], &["虹"]],
    },
    // 3840x2160 at 100 %, the reporter's screen: small text, full frame.
    Case {
        name: "screen_uhd100",
        expect: &[&["北風"], &["太陽"], &["虹"]],
    },
    Case {
        name: "synth_ui_buttons",
        expect: &[&["ファイル"], &["ヘルプ"]],
    },
];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lens")
}

fn has_kana(s: &str) -> bool {
    s.chars()
        .any(|c| ('\u{3041}'..='\u{30FF}').contains(&c) || c == 'ー')
}

/// Longest run of ASCII letters / spaces, a sign of untranslated text.
fn longest_latin_run(s: &str) -> usize {
    let (mut best, mut cur) = (0, 0);
    for c in s.chars() {
        if c.is_ascii_alphabetic() || (c == ' ' && cur > 0) {
            cur += 1;
            best = best.max(cur);
        } else {
            cur = 0;
        }
    }
    best
}

/// Share of ground-truth word tokens (letters only, lowercase) found in
/// the OCR text. Japanese lines count by character.
fn recall(truth: &[String], ocr: &str) -> f64 {
    let norm = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .flat_map(|w| {
                if w.is_ascii() {
                    vec![w.to_ascii_lowercase()]
                } else {
                    w.chars().map(|c| c.to_string()).collect()
                }
            })
            .collect()
    };
    let have: std::collections::HashSet<String> = norm(ocr).into_iter().collect();
    let want = norm(&truth.join(" "));
    if want.is_empty() {
        return 1.0;
    }
    want.iter().filter(|w| have.contains(*w)).count() as f64 / want.len() as f64
}

fn pipeline() -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    // Default settings: OCR chain starts with PP-OCR, translation uses
    // the built-in engine with the default model, target language ja.
    let store = Arc::new(SettingsStore::load_from(tmp.path().join("config.toml")).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: needs the PP-OCR and plamo-3-translate-2b models"]
async fn fixtures_are_recognised_and_translated() {
    let only = std::env::var("LENS_FIXTURE").ok();
    let pipeline = pipeline();
    let mut failures = Vec::new();

    for case in CASES {
        if only.as_deref().is_some_and(|o| o != case.name) {
            continue;
        }
        let png = std::fs::read(fixtures_dir().join(format!("{}.png", case.name))).unwrap();
        let gt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(fixtures_dir().join(format!("{}.json", case.name))).unwrap(),
        )
        .unwrap();
        let truth: Vec<String> = gt["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect();

        // OCR alone first, through the configured chain and its timeout,
        // so a slow or failing engine shows up here instead of as an
        // empty result further down.
        let t = Instant::now();
        match pipeline
            .run(
                Capability::Ocr,
                TaskInput::Ocr {
                    image_png: png.clone(),
                    languages: vec![],
                },
            )
            .await
        {
            Ok(r) => println!(
                "\n[{}] OCR {} ms via {} (fallbacks: {:?})",
                case.name,
                t.elapsed().as_millis(),
                r.used_provider,
                r.fallback_reasons
            ),
            Err(e) => failures.push(format!(
                "{}: OCR failed after {} ms: {e}",
                case.name,
                t.elapsed().as_millis()
            )),
        }

        let t = Instant::now();
        let bundle = match pipeline.run_capture_flow(png).await {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!("{}: pipeline error: {e}", case.name));
                continue;
            }
        };
        let ms = t.elapsed().as_millis();
        let source = bundle.source_text.clone().unwrap_or_default();
        let translation = match &bundle.result.output {
            TaskOutput::Text(t) => t.clone(),
            other => format!("{other:?}"),
        };

        println!(
            "\n=== {} ({ms} ms, provider {}) ===",
            case.name, bundle.result.used_provider
        );
        for b in &bundle.blocks {
            println!("- src: {}", b.source.replace('\n', " "));
            println!(
                "  ja{}: {}",
                if b.same_language {
                    " (same language)"
                } else {
                    ""
                },
                b.translation
                    .as_deref()
                    .unwrap_or("<none>")
                    .replace('\n', " ")
            );
        }

        let r = recall(&truth, &source);
        println!("OCR recall {:.1}%", r * 100.0);
        if r < 0.9 {
            failures.push(format!("{}: OCR recall {:.1}% < 90%", case.name, r * 100.0));
        }
        if bundle.blocks.is_empty() {
            failures.push(format!("{}: no text blocks", case.name));
        }
        for (i, b) in bundle.blocks.iter().enumerate() {
            if b.same_language {
                continue;
            }
            match &b.translation {
                None => failures.push(format!("{}: block {i} not translated", case.name)),
                Some(t) if !has_kana(t) => {
                    failures.push(format!("{}: block {i} has no Japanese: {t:?}", case.name))
                }
                // Names / acronyms stay Latin; whole phrases must not.
                Some(t) if longest_latin_run(t) > 24 => failures.push(format!(
                    "{}: block {i} keeps untranslated text: {t:?}",
                    case.name
                )),
                _ => {}
            }
        }
        for group in case.expect {
            if !group.iter().any(|w| translation.contains(w)) {
                failures.push(format!("{}: none of {group:?} in translation", case.name));
            }
        }
    }

    assert!(failures.is_empty(), "failures:\n{}", failures.join("\n"));
}
