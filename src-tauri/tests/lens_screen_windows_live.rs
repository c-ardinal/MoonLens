//! Windows: screen in, translation out, on a real desktop.
//!
//! `lens_images_live` feeds PNG files straight into the pipeline, so it
//! cannot catch a screen grab that comes back blank or garbled on
//! Windows. This test shows each fixture image in a topmost window and
//! reads it back the way the app does:
//!
//! - range / Lens path: [`capture::capture_global_rect`] over the window,
//!   the call `lens::capture_lens` makes;
//! - range selector path: `xcap::Monitor::capture_image` of the whole
//!   monitor, cropped to the window, as `selector::show_with` /
//!   `take_crop` do.
//!
//! Each capture is compared with the source pixels, run through every
//! OCR engine on its own (PP-OCR, Windows.Media.Ocr, ocrs) and then
//! through [`Pipeline::run_capture_flow`] with the default settings
//! (OCR chain + built-in translator). Captures and `report.md` go to
//! `MOONLENS_E2E_OUT` (CI uploads them).
//!
//! `#[ignore]`: needs an interactive desktop and the downloaded models
//! (`cargo run --example fetch_models`). Run with
//!
//! ```sh
//! cargo test --release --test lens_screen_windows_live -- --ignored --nocapture
//! ```
#![cfg(windows)]

use std::{
    fmt::Write as _,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use image::{DynamicImage, RgbaImage};
use moonlens_lib::{
    capture,
    pipeline::Pipeline,
    plugins::registry::PluginRegistry,
    providers::{ocr_windows, Capability, TaskInput, TaskOutput},
    settings::SettingsStore,
    storage::Storage,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, EndPaint, StretchDIBits, UpdateWindow, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, PAINTSTRUCT, SRCCOPY,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
            RegisterClassW, ShowWindow, TranslateMessage, MSG, PM_REMOVE, SW_SHOWNOACTIVATE,
            WM_PAINT, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
            WS_VISIBLE,
        },
    },
};

/// Window origin on the primary monitor.
const X0: i32 = 40;
const Y0: i32 = 40;

struct Case {
    name: &'static str,
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
        name: "synth_ui_buttons",
        expect: &[&["ファイル"], &["ヘルプ"]],
    },
];

/// Pixels the probe window paints: (width, height, BGRA rows top-down).
static PIXELS: Mutex<(i32, i32, Vec<u8>)> = Mutex::new((0, 0, Vec::new()));

unsafe extern "system" fn wnd_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if m == WM_PAINT {
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let hdc = BeginPaint(h, &mut ps);
        let px = PIXELS.lock().unwrap();
        let (pw, ph) = (px.0, px.1);
        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: pw,
            biHeight: -ph, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        StretchDIBits(
            hdc,
            0,
            0,
            pw,
            ph,
            0,
            0,
            pw,
            ph,
            px.2.as_ptr().cast(),
            &bmi,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        EndPaint(h, &ps);
        return 0;
    }
    DefWindowProcW(h, m, w, l)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn pump(ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Show `img` in a topmost popup at (X0, Y0); returns its HWND.
fn show(img: &RgbaImage) -> HWND {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut bgra = img.as_raw().clone();
    for p in bgra.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    *PIXELS.lock().unwrap() = (w, h, bgra);
    unsafe {
        let class = wide("MoonLensScreenProbe");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: GetModuleHandleW(std::ptr::null()),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        // Fails harmlessly once the class exists.
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("probe").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            X0,
            Y0,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            wc.hInstance,
            std::ptr::null(),
        );
        assert!(!hwnd.is_null(), "CreateWindowExW");
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        UpdateWindow(hwnd);
        hwnd
    }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lens")
}

fn out_dir() -> PathBuf {
    let d = std::env::var_os("MOONLENS_E2E_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("moonlens-e2e"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Mean absolute RGB difference (0..255) between two same-size images.
fn mean_abs_diff(a: &RgbaImage, b: &RgbaImage) -> f64 {
    if a.dimensions() != b.dimensions() {
        return f64::MAX;
    }
    let mut sum = 0u64;
    for (p, q) in a.pixels().zip(b.pixels()) {
        sum += p.0[..3]
            .iter()
            .zip(&q.0[..3])
            .map(|(x, y)| u64::from(x.abs_diff(*y)))
            .sum::<u64>();
    }
    sum as f64 / (a.width() as f64 * a.height() as f64 * 3.0)
}

fn has_kana(s: &str) -> bool {
    s.chars()
        .any(|c| ('\u{3041}'..='\u{30FF}').contains(&c) || c == 'ー')
}

fn recall(truth: &[String], ocr: &str) -> f64 {
    let norm = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| w.to_lowercase())
            .collect()
    };
    let have: std::collections::HashSet<String> = norm(ocr).into_iter().collect();
    let want = norm(&truth.join(" "));
    if want.is_empty() {
        return 1.0;
    }
    want.iter().filter(|w| have.contains(*w)).count() as f64 / want.len() as f64
}

/// A pipeline on default settings, plus `extra_toml` merged on top.
fn pipeline(extra_toml: &str) -> Pipeline {
    let tmp = tempfile::tempdir().unwrap();
    let cfg = tmp.path().join("config.toml");
    if !extra_toml.is_empty() {
        std::fs::write(&cfg, extra_toml).unwrap();
    }
    let store = Arc::new(SettingsStore::load_from(cfg).unwrap());
    let plugins =
        Arc::new(PluginRegistry::load_from(tmp.path().join("plugins"), store.clone()).unwrap());
    std::mem::forget(tmp);
    Pipeline::new(store, Arc::new(Storage::open_in_memory().unwrap()), plugins)
}

const OCR_ENGINES: [&str; 3] = ["ppocr", "windows", "ocrs"];

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: needs a desktop session and the PP-OCR / plamo-3-translate-2b models"]
async fn screen_captures_are_recognised_and_translated() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    // Stage times of every Windows OCR call, so a stalled call shows the
    // stage it never left.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(
            "warn,moonlens_lib::providers::ocr_windows=debug",
        ))
        .with_writer(std::io::stderr)
        .try_init();
    let out = out_dir();
    let only = std::env::var("LENS_FIXTURE").ok();
    // MOONLENS_E2E_CONFIG: run the capture flow on a copy of a real
    // config.toml instead of the defaults.
    let default = pipeline(
        &std::env::var_os("MOONLENS_E2E_CONFIG")
            .map(|p| std::fs::read_to_string(p).unwrap())
            .unwrap_or_default(),
    );
    let single: Vec<(&str, Pipeline)> = OCR_ENGINES
        .iter()
        .map(|id| {
            (
                *id,
                pipeline(&format!("[providers.ocr]\nchain = [\"{id}\"]\n")),
            )
        })
        .collect();

    let mut report = String::from("# MoonLens Windows screen E2E\n\n");
    if let Ok(ms) = xcap::Monitor::all() {
        for m in ms {
            let _ = writeln!(
                report,
                "- monitor {:?}: {:?}x{:?} at ({:?},{:?}), scale {:?}",
                m.name().ok(),
                m.width().ok(),
                m.height().ok(),
                m.x().ok(),
                m.y().ok(),
                m.scale_factor().ok()
            );
        }
    }
    let mut failures = Vec::new();

    // Windows.Media.Ocr pages its language data in from disk as new text
    // touches it. On a freshly started runner the first reading of each
    // text is several times slower than a repeat, and one slow disk read
    // there pushed a single call past the chain's 5 s limit. Read every
    // fixture once up front, without a limit, so the checks below time
    // the engine and not the runner's cold disk. The cold times stay in
    // the report.
    let _ = writeln!(report, "\n## Windows OCR, first reading (no limit)\n");
    for case in CASES {
        if only.as_deref().is_some_and(|o| o != case.name) {
            continue;
        }
        let png = std::fs::read(fixtures_dir().join(format!("{}.png", case.name))).unwrap();
        let t = Instant::now();
        let read = tokio::task::spawn_blocking(move || ocr_windows::run_ocr(&png, None))
            .await
            .unwrap();
        let _ = writeln!(
            report,
            "- {}: {} ms, {}",
            case.name,
            t.elapsed().as_millis(),
            match read {
                Ok(lines) => format!("{} lines", lines.len()),
                Err(e) => format!("FAILED: {e}"),
            }
        );
    }

    for case in CASES {
        if only.as_deref().is_some_and(|o| o != case.name) {
            continue;
        }
        let src = image::open(fixtures_dir().join(format!("{}.png", case.name)))
            .unwrap()
            .to_rgba8();
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
        let (w, h) = src.dimensions();

        let hwnd = show(&src);
        pump(600);

        // Range / Lens path.
        let range = capture::capture_global_rect(X0, Y0, w, h)
            .map(|i| i.to_rgba8())
            .map_err(|e| e.to_string());
        // Range selector path: full monitor, then crop.
        let selector = xcap::Monitor::from_point(X0, Y0)
            .and_then(|m| {
                let (mx, my) = (m.x()?, m.y()?);
                let full = m.capture_image()?;
                Ok(
                    image::imageops::crop_imm(&full, (X0 - mx) as u32, (Y0 - my) as u32, w, h)
                        .to_image(),
                )
            })
            .map_err(|e| e.to_string());
        unsafe {
            DestroyWindow(hwnd);
        }
        pump(100);

        let _ = writeln!(report, "\n## {} ({w}x{h})\n", case.name);
        for (path, cap) in [("range", range), ("selector", selector)] {
            let label = format!("{}/{path}", case.name);
            let img = match cap {
                Ok(i) => i,
                Err(e) => {
                    failures.push(format!("{label}: capture failed: {e}"));
                    let _ = writeln!(report, "- {path}: capture FAILED: {e}");
                    continue;
                }
            };
            let dimg = DynamicImage::ImageRgba8(img.clone());
            let png = capture::png_bytes(&dimg).unwrap();
            std::fs::write(out.join(format!("{}-{path}.png", case.name)), &png).unwrap();
            let diff = mean_abs_diff(&img, &src);
            let stats = moonlens_lib::intent::image_stats(&dimg);
            let _ = writeln!(
                report,
                "- {path}: {}x{}, pixel diff vs source {diff:.1}, luma {:?}, intent {:?}",
                img.width(),
                img.height(),
                stats.map(|s| (s.mean.round(), s.std.round())),
                moonlens_lib::intent::classify(&dimg)
            );
            if diff > 8.0 {
                failures.push(format!(
                    "{label}: capture differs from the screen (diff {diff:.1})"
                ));
            }

            // Each OCR engine on its own.
            for (id, p) in &single {
                let t = Instant::now();
                let r = p
                    .run(
                        Capability::Ocr,
                        TaskInput::Ocr {
                            image_png: png.clone(),
                            languages: vec![],
                        },
                    )
                    .await;
                let ms = t.elapsed().as_millis();
                match r {
                    Ok(r) => {
                        let text = match &r.output {
                            TaskOutput::Ocr { lines } => lines
                                .iter()
                                .map(|l| l.text.as_str())
                                .collect::<Vec<_>>()
                                .join(" / "),
                            _ => String::new(),
                        };
                        let rc = recall(&truth, &text);
                        let _ = writeln!(
                            report,
                            "  - OCR {id}: {ms} ms, recall {:.0}%: {}",
                            rc * 100.0,
                            text.chars().take(160).collect::<String>()
                        );
                        if *id != "ocrs" && rc < 0.9 {
                            failures.push(format!("{label}: OCR {id} recall {:.0}%", rc * 100.0));
                        }
                    }
                    Err(e) => {
                        let _ = writeln!(report, "  - OCR {id}: FAILED after {ms} ms: {e}");
                        if *id != "ocrs" {
                            failures.push(format!("{label}: OCR {id} failed: {e}"));
                        }
                    }
                }
            }

            // The app's capture flow on default settings.
            let t = Instant::now();
            match default.run_capture_flow(png).await {
                Err(e) => {
                    let _ = writeln!(report, "  - flow: FAILED: {e}");
                    failures.push(format!("{label}: capture flow failed: {e}"));
                }
                Ok(b) => {
                    let translation = match &b.result.output {
                        TaskOutput::Text(t) => t.clone(),
                        other => format!("{other:?}"),
                    };
                    let _ = writeln!(
                        report,
                        "  - flow: {} ms, intent {:?}, provider {}, {} blocks\n    - src: {}\n    - ja: {}",
                        t.elapsed().as_millis(),
                        b.intent,
                        b.result.used_provider,
                        b.blocks.len(),
                        b.source_text.clone().unwrap_or_default().replace('\n', " "),
                        translation.replace('\n', " ")
                    );
                    if b.blocks.is_empty() {
                        failures.push(format!("{label}: no text blocks ({:?})", b.result.output));
                    }
                    if !has_kana(&translation) {
                        failures.push(format!("{label}: no Japanese in {translation:?}"));
                    }
                    for group in case.expect {
                        if !group.iter().any(|w| translation.contains(w)) {
                            failures.push(format!("{label}: none of {group:?} in translation"));
                        }
                    }
                }
            }
        }
    }

    let _ = writeln!(report, "\n## Failures\n");
    for f in &failures {
        let _ = writeln!(report, "- {f}");
    }
    std::fs::write(out.join("report.md"), &report).unwrap();
    println!("{report}");
    assert!(failures.is_empty(), "failures:\n{}", failures.join("\n"));
}
