//! Render a curated set of synthetic input PNGs into
//! `tests/input/synth/`. Each scenario covers a layout pattern the
//! Lens pipeline has historically struggled with — long pure-English
//! paragraphs, heading + body splits, two-column layouts, mixed
//! English+Japanese lines, dense small UI text. Re-run any time the
//! corpus needs refreshing; the generator is deterministic so output
//! diffs surface real changes.
//!
//! Run: `cargo run --release --example gen_synth_inputs`
//! Then: `cargo run --release --example lens_eval` picks them up
//! alongside the existing `tests/input/*.png`.

use std::{fs, path::Path};

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;

fn main() -> Result<()> {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("project root (parent of src-tauri)")?;
    let out_dir = project_root.join("tests").join("input").join("synth");
    fs::create_dir_all(&out_dir).context("mkdir tests/input/synth")?;

    let regular = read_first_font(&[
        "C:/Windows/Fonts/NotoSansJP-Regular.ttf",
        "C:/Windows/Fonts/arial.ttf",
    ])?;
    let bold = read_first_font(&[
        "C:/Windows/Fonts/NotoSansJP-Bold.ttf",
        "C:/Windows/Fonts/arialbd.ttf",
        "C:/Windows/Fonts/NotoSansJP-Regular.ttf",
        "C:/Windows/Fonts/arial.ttf",
    ])?;

    let scenarios: &[Scenario] = &[
        Scenario {
            name: "synth_long_paragraph",
            width: 900,
            height: 260,
            blocks: &[Block {
                x: 24,
                y: 24,
                font_px: 16.0,
                bold: false,
                line_height: 1.5,
                lines: &[
                    "Of all the senses, sight must be the most delightful.",
                    "Few things give us greater pleasure than to be able to see again",
                    "after we have been prevented from doing so for a length of time.",
                    "Yet though sight is so precious a possession, comparatively few",
                    "people make full use of it. They may indeed glance at trees and",
                    "animals and clouds, but they almost never observe them.",
                ],
            }],
        },
        Scenario {
            name: "synth_title_body",
            width: 900,
            height: 220,
            blocks: &[
                Block {
                    x: 24,
                    y: 24,
                    font_px: 26.0,
                    bold: true,
                    line_height: 1.3,
                    lines: &["The Quick Brown Fox"],
                },
                Block {
                    x: 24,
                    y: 80,
                    font_px: 15.0,
                    bold: false,
                    line_height: 1.5,
                    lines: &[
                        "The quick brown fox jumps over the lazy dog. The five",
                        "boxing wizards jump quickly. Pack my box with five dozen",
                        "liquor jugs. Sphinx of black quartz, judge my vow.",
                    ],
                },
            ],
        },
        Scenario {
            name: "synth_two_columns",
            width: 900,
            height: 260,
            blocks: &[
                Block {
                    x: 24,
                    y: 24,
                    font_px: 14.0,
                    bold: false,
                    line_height: 1.5,
                    lines: &[
                        "The North Wind and the Sun were",
                        "disputing which was stronger when",
                        "a traveler came along wrapped in",
                        "a warm cloak. They agreed that the",
                        "one who first succeeded in making",
                        "the traveler take his cloak off",
                        "should be considered stronger than",
                        "the other.",
                    ],
                },
                Block {
                    x: 480,
                    y: 24,
                    font_px: 14.0,
                    bold: false,
                    line_height: 1.5,
                    lines: &[
                        "Then the North Wind blew as hard",
                        "as he could, but the more he blew",
                        "the more closely did the traveler",
                        "fold his cloak around him; and at",
                        "last the North Wind gave up the",
                        "attempt. Then the Sun shined out",
                        "warmly, and the traveler took off",
                        "his cloak.",
                    ],
                },
            ],
        },
        Scenario {
            name: "synth_mixed_jp_en",
            width: 900,
            height: 260,
            blocks: &[
                Block {
                    x: 24,
                    y: 24,
                    font_px: 22.0,
                    bold: true,
                    line_height: 1.3,
                    lines: &["The Rainbow Passage (虹の文章)"],
                },
                Block {
                    x: 24,
                    y: 70,
                    font_px: 14.0,
                    bold: false,
                    line_height: 1.5,
                    lines: &["音声学のサンプルとして広く使われる文章。出典：Wikipedia"],
                },
                Block {
                    x: 24,
                    y: 110,
                    font_px: 14.0,
                    bold: false,
                    line_height: 1.5,
                    lines: &[
                        "When the sunlight strikes raindrops in the air, they act",
                        "as a prism and form a rainbow. The rainbow is a division",
                        "of white light into many beautiful colours.",
                    ],
                },
            ],
        },
        Scenario {
            name: "synth_ui_buttons",
            width: 900,
            height: 100,
            blocks: &[Block {
                x: 24,
                y: 24,
                font_px: 14.0,
                bold: false,
                line_height: 1.0,
                lines: &["File    Edit    View    Insert    Format    Tools    Help"],
            }],
        },
        Scenario {
            name: "synth_small_body",
            width: 900,
            height: 260,
            blocks: &[Block {
                x: 24,
                y: 24,
                font_px: 11.0,
                bold: false,
                line_height: 1.45,
                lines: &[
                    "It was the best of times, it was the worst of times, it was the age of wisdom,",
                    "it was the age of foolishness, it was the epoch of belief, it was the epoch",
                    "of incredulity, it was the season of Light, it was the season of Darkness,",
                    "it was the spring of hope, it was the winter of despair, we had everything",
                    "before us, we had nothing before us, we were all going direct to Heaven,",
                    "we were all going direct the other way — in short, the period was so far",
                    "like the present period, that some of its noisiest authorities insisted on",
                    "its being received, for good or for evil, in the superlative degree of",
                    "comparison only.",
                ],
            }],
        },
        Scenario {
            name: "synth_short_lines",
            width: 900,
            height: 220,
            blocks: &[Block {
                x: 24,
                y: 24,
                font_px: 16.0,
                bold: false,
                line_height: 1.5,
                lines: &[
                    "Save your work.",
                    "Open a new file.",
                    "Close all tabs.",
                    "Search the document.",
                    "Show settings panel.",
                    "Quit the application.",
                ],
            }],
        },
    ];

    for scenario in scenarios {
        let mut img =
            RgbaImage::from_pixel(scenario.width, scenario.height, Rgba([248, 248, 250, 255]));
        for block in scenario.blocks {
            render_block(&mut img, block, &regular, &bold);
        }
        let path = out_dir.join(format!("{}.png", scenario.name));
        img.save(&path)
            .with_context(|| format!("save {}", path.display()))?;
        println!("wrote {}", path.display());
    }

    generate_multilingual_panels(&out_dir)?;
    render_vertical_ja(&out_dir)?;
    render_decorative_ja(&out_dir)?;
    Ok(())
}

/// Render single-language paragraph panels for the OCR bench (ocr_bench.rs).
/// Covers JA-only, ZH-CN, KO, FR, DE — the languages we want to measure once
/// `ocrs` (Latin-trained) is taken off its English-dominant home turf.
fn generate_multilingual_panels(out_dir: &Path) -> Result<()> {
    let noto_jp = read_first_font(&["C:/Windows/Fonts/NotoSansJP-Regular.ttf"])?;
    let malgun_ko = read_first_font(&["C:/Windows/Fonts/malgun.ttf"])?;
    let yahei_zh = read_ttc_font(&[("C:/Windows/Fonts/msyh.ttc", 0)])?;
    let latin = read_first_font(&[
        "C:/Windows/Fonts/arial.ttf",
        "C:/Windows/Fonts/NotoSansJP-Regular.ttf",
    ])?;

    let panels: &[MultiPanel] = &[
        MultiPanel {
            name: "synth_ja_paragraph",
            width: 880,
            height: 220,
            font_px: 18.0,
            line_height: 1.6,
            lang_label: "ja",
            lines: &[
                "吾輩は猫である。名前はまだ無い。",
                "どこで生れたかとんと見当がつかぬ。",
                "何でも薄暗いじめじめした所でニャーニャー泣いていた事だけは記憶している。",
                "吾輩はここで始めて人間というものを見た。",
            ],
        },
        MultiPanel {
            name: "synth_zh_paragraph",
            width: 880,
            height: 220,
            font_px: 18.0,
            line_height: 1.6,
            lang_label: "zh",
            lines: &[
                "春眠不觉晓，处处闻啼鸟。",
                "夜来风雨声，花落知多少。",
                "举头望明月，低头思故乡。",
                "床前明月光，疑是地上霜。",
            ],
        },
        MultiPanel {
            name: "synth_ko_paragraph",
            width: 880,
            height: 220,
            font_px: 18.0,
            line_height: 1.6,
            lang_label: "ko",
            lines: &[
                "동해 물과 백두산이 마르고 닳도록",
                "하느님이 보우하사 우리나라 만세",
                "무궁화 삼천리 화려 강산",
                "대한 사람 대한으로 길이 보전하세",
            ],
        },
        MultiPanel {
            name: "synth_fr_paragraph",
            width: 880,
            height: 220,
            font_px: 18.0,
            line_height: 1.6,
            lang_label: "fr",
            lines: &[
                "Aujourd'hui, maman est morte. Ou peut-être hier, je ne sais pas.",
                "J'ai reçu un télégramme de l'asile : « Mère décédée. »",
                "Enterrement demain. Sentiments distingués.",
                "Cela ne veut rien dire. C'était peut-être hier.",
            ],
        },
        MultiPanel {
            name: "synth_de_paragraph",
            width: 880,
            height: 220,
            font_px: 18.0,
            line_height: 1.6,
            lang_label: "de",
            lines: &[
                "Als Gregor Samsa eines Morgens aus unruhigen Träumen erwachte,",
                "fand er sich in seinem Bett zu einem ungeheuren Ungeziefer verwandelt.",
                "Er lag auf seinem panzerartig harten Rücken und sah,",
                "wenn er den Kopf ein wenig hob, seinen gewölbten, braunen Bauch.",
            ],
        },
    ];

    for panel in panels {
        let font = match panel.lang_label {
            "ja" => &noto_jp,
            "zh" => &yahei_zh,
            "ko" => &malgun_ko,
            _ => &latin,
        };
        let mut img =
            RgbaImage::from_pixel(panel.width, panel.height, Rgba([248, 248, 250, 255]));
        let scale = PxScale::from(panel.font_px);
        let scaled = font.as_scaled(scale);
        let ink = Rgba([34, 34, 38, 255]);
        let line_advance = (panel.font_px * panel.line_height).round() as i32;
        let baseline_offset = scaled.ascent().round() as i32;
        let x = 24i32;
        let y0 = 24i32;
        for (i, line) in panel.lines.iter().enumerate() {
            let y = y0 + baseline_offset - scaled.ascent().round() as i32
                + (i as i32) * line_advance;
            draw_text_mut(&mut img, ink, x, y, scale, font, line);
        }
        let path = out_dir.join(format!("{}.png", panel.name));
        img.save(&path)
            .with_context(|| format!("save {}", path.display()))?;
        println!("wrote {}", path.display());
    }
    Ok(())
}

struct MultiPanel {
    name: &'static str,
    width: u32,
    height: u32,
    font_px: f32,
    line_height: f32,
    lang_label: &'static str,
    lines: &'static [&'static str],
}

/// Render a tategaki (vertical) Japanese panel: each character is
/// painted into its own row, advancing top-to-bottom along the
/// column. Column advances right-to-left. No native CJK vertical
/// layout in ab_glyph — this is the manual approximation that ocrs
/// / PaddleOCR / manga-ocr all "see" as well as a real CJK reader.
fn render_vertical_ja(out_dir: &Path) -> Result<()> {
    let font = read_first_font(&["C:/Windows/Fonts/NotoSansJP-Regular.ttf"])?;
    let columns: &[&[char]] = &[
        &['吾', '輩', 'は', '猫', 'で', 'あ', 'る', '。'],
        &['名', '前', 'は', 'ま', 'だ', '無', 'い', '。'],
        &['ど', 'こ', 'で', '生', 'れ', 'た', 'か'],
        &['と', 'ん', 'と', '見', '当', 'が', 'つ', 'か', 'ぬ', '。'],
    ];
    let font_px: f32 = 22.0;
    let line_height: f32 = 1.4;
    let col_advance = (font_px * 1.7_f32).round() as i32; // a touch of column air
    let row_advance = (font_px * line_height).round() as i32;
    let width: u32 = (columns.len() as i32 * col_advance + 48) as u32;
    let max_chars = columns.iter().map(|c| c.len()).max().unwrap_or(8) as i32;
    let height: u32 = (max_chars * row_advance + 48) as u32;
    let mut img = RgbaImage::from_pixel(width, height, Rgba([248, 248, 250, 255]));
    let scale = PxScale::from(font_px);
    let scaled = font.as_scaled(scale);
    let ink = Rgba([34, 34, 38, 255]);
    let ascent = scaled.ascent().round() as i32;
    let right_margin = 24i32;
    for (col_idx, col) in columns.iter().enumerate() {
        // Tategaki advances RIGHT-TO-LEFT. col 0 is rightmost.
        let x = width as i32 - right_margin - (col_idx as i32 + 1) * col_advance;
        for (row_idx, ch) in col.iter().enumerate() {
            let y = 24 + (row_idx as i32) * row_advance + ascent
                - scaled.ascent().round() as i32;
            draw_text_mut(&mut img, ink, x, y, scale, &font, &ch.to_string());
        }
    }
    let path = out_dir.join("synth_ja_vertical.png");
    img.save(&path)
        .with_context(|| format!("save {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Decorative JA: large bold title + smaller body. Mimics the game
/// UI / poster look where manga-ocr's training data should help.
fn render_decorative_ja(out_dir: &Path) -> Result<()> {
    let regular = read_first_font(&["C:/Windows/Fonts/NotoSansJP-Regular.ttf"])?;
    let bold = read_first_font(&[
        "C:/Windows/Fonts/NotoSansJP-Bold.ttf",
        "C:/Windows/Fonts/NotoSansJP-Regular.ttf",
    ])?;
    let width = 720u32;
    let height = 260u32;
    let mut img = RgbaImage::from_pixel(width, height, Rgba([248, 248, 250, 255]));
    let title_scale = PxScale::from(44.0);
    let body_scale = PxScale::from(20.0);
    let ink = Rgba([34, 34, 38, 255]);
    draw_text_mut(&mut img, ink, 32, 24, title_scale, &bold, "新章「月光」開幕！");
    let body_lines = [
        "主人公が立ち上がり、",
        "世界は再び動き始める。",
        "今こそ、運命を選び取れ。",
    ];
    let body_line_h = 28i32;
    for (i, line) in body_lines.iter().enumerate() {
        let y = 100 + (i as i32) * body_line_h;
        draw_text_mut(&mut img, ink, 32, y, body_scale, &regular, line);
    }
    let path = out_dir.join("synth_ja_decorative.png");
    img.save(&path)
        .with_context(|| format!("save {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

fn read_ttc_font(candidates: &[(&str, u32)]) -> Result<FontVec> {
    for (p, idx) in candidates {
        if let Ok(bytes) = fs::read(p) {
            if let Ok(font) = FontVec::try_from_vec_and_index(bytes, *idx) {
                return Ok(font);
            }
        }
    }
    anyhow::bail!("no usable TTC font in {:?}", candidates)
}

struct Scenario {
    name: &'static str,
    width: u32,
    height: u32,
    blocks: &'static [Block],
}

struct Block {
    x: u32,
    y: u32,
    font_px: f32,
    bold: bool,
    line_height: f32,
    lines: &'static [&'static str],
}

/// Render `block` onto `img`. The y advance per line is
/// `font_px × line_height`, matching the rule the lens pipeline
/// uses to read source line spacing — that way the synthesized
/// images have predictable `line_height` ratios for the eval to
/// detect.
fn render_block(img: &mut RgbaImage, block: &Block, regular: &FontVec, bold: &FontVec) {
    let font = if block.bold { bold } else { regular };
    let scale = PxScale::from(block.font_px);
    let scaled = font.as_scaled(scale);
    let ink = Rgba([34, 34, 38, 255]);
    let line_advance = (block.font_px * block.line_height).round() as i32;
    // ab_glyph anchors at the top of the glyph cell; nudge a little so
    // descenders do not clip the top edge of the synthesized canvas.
    let baseline_offset = scaled.ascent().round() as i32;
    for (i, line) in block.lines.iter().enumerate() {
        let y = block.y as i32 + baseline_offset - scaled.ascent().round() as i32
            + (i as i32) * line_advance;
        draw_text_mut(img, ink, block.x as i32, y, scale, font, line);
    }
}

fn read_first_font(paths: &[&str]) -> Result<FontVec> {
    for p in paths {
        if let Ok(bytes) = fs::read(p) {
            if let Ok(font) = FontVec::try_from_vec(bytes) {
                return Ok(font);
            }
        }
    }
    anyhow::bail!("no usable font in {:?}", paths)
}
