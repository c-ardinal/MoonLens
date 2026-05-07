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
        let mut img = RgbaImage::from_pixel(
            scenario.width,
            scenario.height,
            Rgba([248, 248, 250, 255]),
        );
        for block in scenario.blocks {
            render_block(&mut img, block, &regular, &bold);
        }
        let path = out_dir.join(format!("{}.png", scenario.name));
        img.save(&path)
            .with_context(|| format!("save {}", path.display()))?;
        println!("wrote {}", path.display());
    }
    Ok(())
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
