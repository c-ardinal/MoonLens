//! Per-lens streaming change detector and OCR/translate pipeline.
//!
//! When a [`crate::lens`] window is created we spawn one of these
//! watchers. Each iteration:
//!   1. Capture the observed rect with [`crate::lens::capture_lens`]
//!      (hides the lens window to avoid self-capture).
//!   2. Hash the image and short-circuit if unchanged since last poll
//!      (cheap [`crate::region_watcher::average_hash`] + Hamming
//!      distance check).
//!   3. On change, run OCR, publish the per-line axis-aligned bboxes
//!      to the lens UI via `moonlens://lens-ocr-lines`, and spawn N
//!      concurrent translate tasks (one per line).
//!   4. Each translate task streams tokens back through
//!      `moonlens://lens-line-chunk` tagged with its `line_index`.
//!   5. When every line is done the watcher emits
//!      `moonlens://lens-flow-done` so the UI can finalise (hide
//!      spinners, etc).
//!
//! The in-place overlay rendering (positioned cards, background fill,
//! font-fit) is a frontend concern — the backend only publishes the
//! raw bboxes + streaming tokens.

use std::{collections::HashMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{async_runtime::JoinHandle, AppHandle, Emitter, Manager};
use tokio::sync::Semaphore;

use crate::{
    lens::{self, LensRegistryHandle},
    pipeline::Pipeline,
    providers::{
        translate_ollama::OllamaTranslator, Capability, OcrLine, Provider, ProviderError,
        TaskInput, TaskOutput,
    },
    region_watcher::{average_hash, hamming},
    settings::SettingsStore,
};

/// Polling cadence.
pub const POLL_INTERVAL_MS: u64 = 500;

/// Minimum Hamming distance in the 64-bit average hash that counts
/// as "source changed". 8 was the original value shared with
/// `region_watcher`, but ocrs' own rasterisation + font antialiasing
/// causes 6-10 bits of jitter between truly-identical frames under
/// some compositors. 20 is still well under the ~32 bits you'd see
/// for an obviously different screen (e.g. a different paragraph)
/// and kills the auto-poll → retranslate infinite loop.
pub const HASH_DIFF_THRESHOLD: u32 = 20;

/// Registry of running per-lens capture tasks.
///
/// The MVP is event-driven rather than polling: idle lenses never
/// trigger a capture (so the source app is never briefly masked) and
/// the only time the watcher runs is when `trigger` is called —
/// either by the user (window moved/resized, manual refresh) or by
/// `start` itself for the initial grab.
#[derive(Default)]
pub struct LensStreamWatchers {
    inner: Mutex<HashMap<String, Running>>,
    flow_counter: Mutex<u64>,
}

struct Running {
    handle: JoinHandle<()>,
    stop: Arc<tokio::sync::Notify>,
    /// Notified whenever the watcher should run another capture pass
    /// — window move/resize, manual refresh, etc.
    trigger: Arc<tokio::sync::Notify>,
}

impl LensStreamWatchers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn running_count(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_running(&self, lens_id: &str) -> bool {
        self.inner.lock().contains_key(lens_id)
    }

    pub fn start(
        &self,
        lens_id: String,
        app: AppHandle,
        pipeline: Arc<Pipeline>,
        lenses: LensRegistryHandle,
        settings: Arc<SettingsStore>,
    ) {
        self.stop(&lens_id);
        let stop = Arc::new(tokio::sync::Notify::new());
        let trigger = Arc::new(tokio::sync::Notify::new());
        // Pre-notify so the first `notified().await` returns
        // immediately and the lens fires its initial capture without
        // waiting for user input.
        trigger.notify_one();
        let stop_t = stop.clone();
        let trigger_t = trigger.clone();
        let id_clone = lens_id.clone();
        tracing::info!("lens watcher start: {lens_id}");
        // Also spawn a lightweight background task that polls the
        // window's outer_position every 250 ms and fires a trigger if
        // it moved. This is a belt-and-braces backup for the
        // `on_window_event` hook — on Tauri 2 / Windows, frameless
        // transparent webviews sometimes swallow `Moved` events, so
        // position polling guarantees we still notice the drag.
        // Polling the position is visually invisible (no capture, no
        // DOM change) so it does not re-introduce the blink that the
        // no-polling capture design eliminates.
        {
            let app_for_poll = app.clone();
            let id_for_poll = lens_id.clone();
            let trigger_for_poll = trigger.clone();
            let stop_for_poll = stop.clone();
            tauri::async_runtime::spawn(async move {
                let mut last_pos: Option<(i32, i32, u32, u32)> = None;
                loop {
                    tokio::select! {
                        _ = stop_for_poll.notified() => break,
                        _ = tokio::time::sleep(Duration::from_millis(250)) => {}
                    }
                    let Some(win) = app_for_poll.get_webview_window(&id_for_poll) else {
                        break;
                    };
                    let pos = win.outer_position().ok();
                    let size = win.outer_size().ok();
                    let cur = match (pos, size) {
                        (Some(p), Some(s)) => Some((p.x, p.y, s.width, s.height)),
                        _ => None,
                    };
                    if let Some(c) = cur {
                        if last_pos.map(|l| l != c).unwrap_or(false) {
                            trigger_for_poll.notify_one();
                            tracing::debug!(
                                "lens {id_for_poll}: position-poll trigger ({c:?})"
                            );
                        }
                        last_pos = Some(c);
                    }
                }
            });
        }

        // Content-change auto-poll. The capture loop already hash-
        // compares every trigger and bails out when nothing changed,
        // so firing a periodic trigger is safe — it costs at most one
        // brief mask + capture per tick when source is static, and
        // auto-refreshes the translation the moment source text
        // actually changes (video subtitles, chat messages, etc.).
        // User-configurable via `performance.lens_auto_poll_ms`; 0
        // disables periodic polling entirely (event-driven only).
        {
            let settings_for_auto = settings.clone();
            let trigger_for_auto = trigger.clone();
            let stop_for_auto = stop.clone();
            let id_for_auto = lens_id.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let interval_ms = settings_for_auto
                        .snapshot()
                        .performance
                        .lens_auto_poll_ms;
                    if interval_ms == 0 {
                        // Re-check every 1 s — the user can turn
                        // auto-poll on from settings without needing a
                        // restart.
                        tokio::select! {
                            _ = stop_for_auto.notified() => break,
                            _ = tokio::time::sleep(Duration::from_millis(1000)) => continue,
                        }
                    }
                    tokio::select! {
                        _ = stop_for_auto.notified() => break,
                        _ = tokio::time::sleep(Duration::from_millis(interval_ms as u64)) => {}
                    }
                    trigger_for_auto.notify_one();
                    tracing::trace!("lens {id_for_auto}: auto-poll trigger");
                }
            });
        }
        let handle = tauri::async_runtime::spawn(async move {
            run_capture_loop(id_clone, app, pipeline, lenses, settings, stop_t, trigger_t)
                .await;
        });
        self.inner.lock().insert(
            lens_id,
            Running {
                handle,
                stop,
                trigger,
            },
        );
    }

    pub fn stop(&self, lens_id: &str) {
        if let Some(r) = self.inner.lock().remove(lens_id) {
            r.stop.notify_waiters();
            r.handle.abort();
        }
    }

    pub fn stop_all(&self) {
        let mut map = self.inner.lock();
        for (_, r) in map.drain() {
            r.stop.notify_waiters();
            r.handle.abort();
        }
    }

    /// Ask the watcher for `lens_id` to run another capture pass.
    /// Idempotent: if a capture is already queued or in progress the
    /// extra signal coalesces into the next wait via Notify.
    pub fn trigger(&self, lens_id: &str) {
        let present = {
            let guard = self.inner.lock();
            match guard.get(lens_id) {
                Some(r) => {
                    r.trigger.notify_one();
                    true
                }
                None => false,
            }
        };
        tracing::debug!("lens {lens_id}: trigger (registered={present})");
    }

    fn next_flow_id(&self) -> u64 {
        let mut g = self.flow_counter.lock();
        *g = g.wrapping_add(1);
        *g
    }
}

pub type LensStreamWatchersHandle = Arc<LensStreamWatchers>;

/// Emitted when a new flow is about to replace whatever the lens UI
/// currently has on screen.
#[derive(Debug, Clone, Serialize)]
pub struct LensFlowStart {
    pub id: String,
    pub flow_id: u64,
}

/// Emitted once per flow, immediately after OCR completes. Gives the
/// UI enough to draw placeholder cards at bbox positions before any
/// translation arrives.
#[derive(Debug, Clone, Serialize)]
pub struct LensOcrLines {
    pub id: String,
    pub flow_id: u64,
    /// Dimensions of the captured image in pixels. Bboxes are in the
    /// same space; the UI divides by these to get 0..1 normalized
    /// positions for CSS percentage layout.
    pub image_w: u32,
    pub image_h: u32,
    pub lines: Vec<LensOcrLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LensOcrLine {
    pub line_index: usize,
    pub text: String,
    /// `[x, y, w, h]` in image pixel coordinates.
    pub bbox: [f32; 4],
    /// Mean RGB sampled from a 2 px rim around the bbox — i.e. the
    /// background colour of the source area the card will sit over.
    /// Rendering the card with this background makes the translation
    /// look like an in-place replacement rather than a dark overlay.
    pub bg_color: [u8; 3],
    /// Contrast text colour chosen from bg luminance: white for dark
    /// backgrounds, black for light ones. A full WCAG contrast
    /// optimiser lives in plan.md S3; for MVP the luma flip is
    /// enough for legibility on real-world screens.
    pub fg_color: [u8; 3],
}

/// A single streaming token for a specific line. The UI filters by
/// `id`, `flow_id`, and `line_index` to route it to the right card.
#[derive(Debug, Clone, Serialize)]
pub struct LensLineChunk {
    pub id: String,
    pub flow_id: u64,
    pub line_index: usize,
    pub delta: String,
    pub done: bool,
}

/// Emitted after every line of a flow has reported `done` (or failed).
/// The UI uses this to hide the "receiving" spinner; it is not
/// required for correctness because per-line `done=true` events
/// already close out each card individually.
#[derive(Debug, Clone, Serialize)]
pub struct LensFlowDone {
    pub id: String,
    pub flow_id: u64,
    pub line_count: usize,
}

/// Pre/post-capture signal. Tells the lens UI to drop card opacity
/// to zero so the screen-grab sees through the transparent window
/// to the real source. This replaces window-level hide/show and its
/// perceivable blink.
#[derive(Debug, Clone, Serialize)]
pub struct LensMask {
    pub id: String,
    pub masked: bool,
}

/// Mean RGB of a 2 px rim immediately surrounding `bbox`. Used to
/// colour the in-place translation card so it blends with the source
/// area instead of floating on a dark panel.
fn sample_rim_color(img: &image::DynamicImage, bbox: [f32; 4]) -> [u8; 3] {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);
    let rim: i32 = 2;
    let x = bbox[0] as i32;
    let y = bbox[1] as i32;
    let bw = bbox[2] as i32;
    let bh = bbox[3] as i32;
    if bw <= 0 || bh <= 0 {
        return [128, 128, 128];
    }
    let mut r_sum: u64 = 0;
    let mut g_sum: u64 = 0;
    let mut b_sum: u64 = 0;
    let mut n: u64 = 0;
    let mut add = |x0: i32, y0: i32, x1: i32, y1: i32| {
        let (x0, x1) = (x0.max(0), x1.min(w - 1));
        let (y0, y1) = (y0.max(0), y1.min(h - 1));
        if x1 < x0 || y1 < y0 {
            return;
        }
        for py in y0..=y1 {
            for px in x0..=x1 {
                let p = rgb.get_pixel(px as u32, py as u32).0;
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                n += 1;
            }
        }
    };
    // Top / bottom / left / right rim strips outside the bbox. Using
    // strips (not corners only) keeps the sample robust for long
    // subtitle-style bboxes where top-bottom dominates.
    add(x - rim, y - rim, x + bw + rim - 1, y - 1);
    add(x - rim, y + bh, x + bw + rim - 1, y + bh + rim - 1);
    add(x - rim, y, x - 1, y + bh - 1);
    add(x + bw, y, x + bw + rim - 1, y + bh - 1);
    if n == 0 {
        return [128, 128, 128];
    }
    [
        (r_sum / n) as u8,
        (g_sum / n) as u8,
        (b_sum / n) as u8,
    ]
}

/// Linearise one sRGB channel (0..=255) to linear light 0..=1 using
/// the piecewise curve from the sRGB spec.
fn srgb_to_linear(c: u8) -> f32 {
    let f = c as f32 / 255.0;
    if f <= 0.040_45 {
        f / 12.92
    } else {
        ((f + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance per WCAG 2.x (Y component). Returns 0..=1 where
/// 0 is pure black and 1 is pure white in linear light.
fn relative_luminance(bg: [u8; 3]) -> f32 {
    let r = srgb_to_linear(bg[0]);
    let g = srgb_to_linear(bg[1]);
    let b = srgb_to_linear(bg[2]);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Pick a text colour that contrasts with `bg`. Uses the WCAG
/// relative-luminance threshold (~0.179 ~ sRGB 128 grey) and falls
/// back to pure black / white for maximum contrast. Used as a
/// fallback when the source-text-color sampler can't find any
/// non-background pixels to sample (empty bbox, uniform noise).
fn contrast_text(bg: [u8; 3]) -> [u8; 3] {
    if relative_luminance(bg) > 0.179 {
        [10, 10, 10]
    } else {
        [245, 245, 245]
    }
}

/// Sample the dominant non-background colour *inside* the bbox —
/// i.e. the actual source-text colour. This is what the user sees
/// on screen; picking it for the card text makes the translation
/// feel like a swap, not a black/white label pasted on top.
///
/// Algorithm: iterate every pixel of the bbox; a pixel counts as
/// "text" when its distance from `bg` in RGB space is greater than
/// `text_threshold`. Return the mean of the text pixels. When no
/// pixel clears the threshold (noise-only bbox, uniform region)
/// fall back to [`contrast_text`] so the card is never unreadable.
fn sample_text_color(img: &image::DynamicImage, bbox: [f32; 4], bg: [u8; 3]) -> [u8; 3] {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);
    let x = bbox[0] as i32;
    let y = bbox[1] as i32;
    let bw = bbox[2] as i32;
    let bh = bbox[3] as i32;
    if bw <= 0 || bh <= 0 {
        return contrast_text(bg);
    }
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + bw).min(w);
    let y1 = (y + bh).min(h);
    if x1 <= x0 || y1 <= y0 {
        return contrast_text(bg);
    }

    // Threshold = 60 in Euclidean RGB — picks up antialiased glyph
    // edges while ignoring JPEG noise or minor gradients in the
    // background. 60^2 = 3600 lets us compare squared distances and
    // skip the sqrt per pixel.
    const TEXT_THRESHOLD_SQ: i32 = 60 * 60;
    let (br, bgg, bb) = (bg[0] as i32, bg[1] as i32, bg[2] as i32);

    let mut r_sum: u64 = 0;
    let mut g_sum: u64 = 0;
    let mut b_sum: u64 = 0;
    let mut n: u64 = 0;
    for py in y0..y1 {
        for px in x0..x1 {
            let p = rgb.get_pixel(px as u32, py as u32).0;
            let dr = p[0] as i32 - br;
            let dg = p[1] as i32 - bgg;
            let db = p[2] as i32 - bb;
            if dr * dr + dg * dg + db * db >= TEXT_THRESHOLD_SQ {
                r_sum += p[0] as u64;
                g_sum += p[1] as u64;
                b_sum += p[2] as u64;
                n += 1;
            }
        }
    }

    // Require at least 2% of the bbox pixels to look like "text"
    // before we trust the sampled colour. For tiny bboxes where
    // that floor is unreachable the contrast fallback is safer.
    let area = ((x1 - x0) as u64) * ((y1 - y0) as u64);
    let min_text_pixels = (area / 50).max(8);
    if n < min_text_pixels {
        return contrast_text(bg);
    }

    // Check the sampled mean actually reads against bg — pathological
    // cases (e.g. a gradient bg) can produce a "text" mean that sits
    // right on top of the bg. Fall back when contrast is insufficient.
    let mean = [
        (r_sum / n) as u8,
        (g_sum / n) as u8,
        (b_sum / n) as u8,
    ];
    let dr = mean[0] as i32 - br;
    let dg = mean[1] as i32 - bgg;
    let db = mean[2] as i32 - bb;
    if dr * dr + dg * dg + db * db < TEXT_THRESHOLD_SQ {
        return contrast_text(bg);
    }
    mean
}

/// Does `text` appear to already be written in `target_language`? Used
/// to skip the translation round-trip for OCR output that is already
/// in the user's target tongue (e.g. OCR detects Japanese while the
/// target is `ja`). Heuristic-only — defers to a proper language-id
/// provider in plan.md's v0.2 roadmap.
pub fn text_is_already_target(text: &str, target_language: &str) -> bool {
    let target = target_language.trim().to_ascii_lowercase();
    if target.is_empty() || target == "auto" {
        return false;
    }
    // Strip whitespace / punctuation for the detection pass — single
    // kanji in an otherwise English line is still worth translating.
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.is_empty() {
        return false;
    }
    match target.as_str() {
        "ja" | "ja-jp" | "jpn" | "japanese" => {
            // Any hiragana or katakana is a strong Japanese signal.
            let has_kana = letters.iter().any(|c| {
                let u = *c as u32;
                (0x3040..=0x309F).contains(&u) || (0x30A0..=0x30FF).contains(&u)
            });
            if has_kana {
                return true;
            }
            // Pure-kanji runs are ambiguous (could be Chinese). Treat
            // as Japanese when the line is ≥ 2 chars and all letters
            // are CJK — the `never` misclassification bias would strand
            // Japanese subtitles that happen to be kanji-only.
            let all_cjk = letters.iter().all(|c| {
                let u = *c as u32;
                (0x3400..=0x4DBF).contains(&u)
                    || (0x4E00..=0x9FFF).contains(&u)
                    || (0xF900..=0xFAFF).contains(&u)
            });
            all_cjk && letters.len() >= 2
        }
        "en" | "en-us" | "en-gb" | "eng" | "english" => {
            // English = every letter is ASCII. Numbers / punctuation
            // are stripped before we get here so "123" doesn't falsely
            // match as English.
            letters.iter().all(|c| c.is_ascii_alphabetic())
        }
        "zh" | "zh-cn" | "zh-tw" | "chi" | "chinese" => {
            // Chinese = all CJK kanji-ish, NO hiragana/katakana.
            let has_kana = letters.iter().any(|c| {
                let u = *c as u32;
                (0x3040..=0x309F).contains(&u) || (0x30A0..=0x30FF).contains(&u)
            });
            if has_kana {
                return false;
            }
            letters.iter().all(|c| {
                let u = *c as u32;
                (0x3400..=0x4DBF).contains(&u) || (0x4E00..=0x9FFF).contains(&u)
            })
        }
        "ko" | "kor" | "korean" => letters.iter().any(|c| {
            let u = *c as u32;
            (0xAC00..=0xD7A3).contains(&u) || (0x1100..=0x11FF).contains(&u)
        }),
        _ => false,
    }
}

// Paragraph grouping was attempted 2026-04-21 but reverted: merging
// OCR lines into a union-bbox card rescaled every card to the
// combined paragraph height, which made single-line translations of
// multi-line paragraphs look absurdly small. Cross-line LLM context
// is a plan.md S3 concern (streaming_translate plugin point). Each
// OCR line now translates independently, matching MVP behaviour.

async fn run_capture_loop(
    lens_id: String,
    app: AppHandle,
    pipeline: Arc<Pipeline>,
    lenses: LensRegistryHandle,
    settings: Arc<SettingsStore>,
    stop: Arc<tokio::sync::Notify>,
    trigger: Arc<tokio::sync::Notify>,
) {
    let mut last_hash: Option<u64> = None;
    loop {
        // Block until the next trigger (move, resize, manual refresh
        // or the initial pre-notified kick). When idle, the watcher
        // spends zero CPU and there is no mask/blink at all.
        tokio::select! {
            _ = stop.notified() => break,
            _ = trigger.notified() => {}
        }
        // Debounce a burst of move/resize events so a long drag only
        // fires one capture at the end.
        tokio::time::sleep(Duration::from_millis(180)).await;

        // Window gone → registry still has the id until IPC close
        // runs; bail out so we do not leak a task.
        if app.get_webview_window(&lens_id).is_none() {
            break;
        }

        // Windows: `WDA_EXCLUDEFROMCAPTURE` on the lens window means
        // xcap's screen grab already sees straight through us to the
        // source — no masking dance required, no blink, ever.
        //
        // Other platforms still need the MVP mask fallback: emit the
        // opacity-0 signal, wait long enough for the compositor to
        // propagate the CSS class flip, capture, then restore.
        #[cfg(not(target_os = "windows"))]
        {
            let _ = app.emit(
                "moonlens://lens-mask",
                LensMask {
                    id: lens_id.clone(),
                    masked: true,
                },
            );
            tokio::time::sleep(Duration::from_millis(120)).await;
        }

        let lens_id_c = lens_id.clone();
        let app_c = app.clone();
        let lenses_c = lenses.clone();
        let captured =
            tokio::task::spawn_blocking(move || -> anyhow::Result<(Vec<u8>, u64, u32, u32)> {
                let png = lens::capture_lens(&app_c, &lenses_c, &lens_id_c)?;
                let img = image::load_from_memory(&png)?;
                let hash = average_hash(&img);
                Ok((png, hash, img.width(), img.height()))
            })
            .await;

        // Restore card rendering immediately after the grab. No-op
        // on Windows where the mask signal is never sent.
        #[cfg(not(target_os = "windows"))]
        {
            let _ = app.emit(
                "moonlens://lens-mask",
                LensMask {
                    id: lens_id.clone(),
                    masked: false,
                },
            );
        }

        let (png, hash, image_w, image_h) = match captured {
            Ok(Ok(tuple)) => tuple,
            Ok(Err(e)) => {
                tracing::debug!("lens {lens_id}: capture failed: {e}");
                continue;
            }
            Err(e) => {
                tracing::debug!("lens {lens_id}: capture task join: {e}");
                continue;
            }
        };

        let changed = match last_hash {
            None => true,
            Some(prev) => hamming(prev, hash) >= HASH_DIFF_THRESHOLD,
        };
        if !changed {
            continue;
        }
        last_hash = Some(hash);
        tracing::info!(
            "lens {lens_id}: change detected, captured {}x{} ({} bytes PNG)",
            image_w,
            image_h,
            png.len()
        );

        let state = app.state::<crate::app::AppState>();
        let flow_id = state.lens_watchers.next_flow_id();

        let _ = app.emit(
            "moonlens://lens-flow-start",
            LensFlowStart {
                id: lens_id.clone(),
                flow_id,
            },
        );

        // Keep a copy of the PNG for rim colour sampling after OCR —
        // the TaskInput takes ownership below. A 50-100 KB clone per
        // poll is cheaper than re-capturing or round-tripping through
        // the OCR provider.
        let png_for_colors = png.clone();

        // Run OCR standalone so we can expose bboxes before translation
        // starts. The chain config still applies (plugin OCRs can take
        // over), we just skip the translate step that `run_capture_flow`
        // would bake in.
        let ocr_start = std::time::Instant::now();
        let ocr_result = pipeline
            .run(
                Capability::Ocr,
                TaskInput::Ocr {
                    image_png: png,
                    languages: vec![],
                },
            )
            .await;
        tracing::info!(
            "lens {lens_id}: OCR stage returned in {}ms (ok={})",
            ocr_start.elapsed().as_millis(),
            ocr_result.is_ok()
        );

        let ocr_lines: Vec<OcrLine> = match ocr_result {
            Ok(r) => match r.output {
                TaskOutput::Ocr { lines } => lines,
                _ => Vec::new(),
            },
            Err(e) => {
                tracing::info!("lens {lens_id}: OCR returned no result ({e})");
                // Still emit an empty `lens-ocr-lines` + `flow-done` so
                // the UI can exit its streaming state rather than stay
                // stuck on the placeholder forever when OCR repeatedly
                // finds no text (or returns LowConfidence on noise).
                let _ = app.emit(
                    "moonlens://lens-ocr-lines",
                    LensOcrLines {
                        id: lens_id.clone(),
                        flow_id,
                        image_w,
                        image_h,
                        lines: Vec::new(),
                    },
                );
                let _ = app.emit(
                    "moonlens://lens-flow-done",
                    LensFlowDone {
                        id: lens_id.clone(),
                        flow_id,
                        line_count: 0,
                    },
                );
                continue;
            }
        };
        tracing::info!(
            "lens {lens_id}: OCR detected {} line(s) (flow={flow_id})",
            ocr_lines.len()
        );

        // Decode the captured frame so we can sample background /
        // foreground colours around each bbox. Failure here is
        // non-fatal — we fall back to a neutral grey fill so the
        // card still renders.
        let decoded = image::load_from_memory(&png_for_colors).ok();

        // Per-line translation (MVP behaviour). Paragraph grouping
        // was reverted because it rescaled cards to the union bbox
        // and threw off font-fit for translations shorter than the
        // source. Cross-line context is a plan.md S3 concern
        // (`translate_stream` plugin point) — not worth the visual
        // regression in v0.1.
        let mut by_y: Vec<usize> = (0..ocr_lines.len()).collect();
        by_y.sort_by(|&a, &b| {
            ocr_lines[a].bbox[1]
                .partial_cmp(&ocr_lines[b].bbox[1])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let position_in_sort: std::collections::HashMap<usize, usize> =
            by_y.iter().enumerate().map(|(pos, &idx)| (idx, pos)).collect();

        let lens_lines: Vec<LensOcrLine> = ocr_lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let bg = match &decoded {
                    Some(img) => sample_rim_color(img, l.bbox),
                    None => [128, 128, 128],
                };
                // Match the source text colour instead of forcing
                // black/white. Sampling is scoped to the ORIGINAL
                // bbox (pre-tile) so the card expansion below does
                // not dilute the sample.
                let fg = match &decoded {
                    Some(img) => sample_text_color(img, l.bbox, bg),
                    None => contrast_text(bg),
                };
                let [x, _y, w, _h] = l.bbox;
                let orig_top = l.bbox[1];
                let orig_bottom = l.bbox[1] + l.bbox[3];
                let ex: f32 = 8.0;
                let pos = position_in_sort.get(&i).copied().unwrap_or(0);
                let top_divider = if pos > 0 {
                    let prev = &ocr_lines[by_y[pos - 1]].bbox;
                    let prev_bottom = prev[1] + prev[3];
                    (prev_bottom + orig_top) * 0.5
                } else {
                    (orig_top - 6.0).max(0.0)
                };
                let bot_divider = if pos + 1 < by_y.len() {
                    let nxt = &ocr_lines[by_y[pos + 1]].bbox;
                    let next_top = nxt[1];
                    (orig_bottom + next_top) * 0.5
                } else {
                    (orig_bottom + 6.0).min(image_h as f32)
                };
                let ny = top_divider.clamp(0.0, image_h as f32);
                let bottom = bot_divider.clamp(ny + 1.0, image_h as f32);
                let nh = bottom - ny;
                let nx = (x - ex).max(0.0);
                let nw = (w + ex * 2.0).min((image_w as f32) - nx);
                LensOcrLine {
                    line_index: i,
                    text: l.text.clone(),
                    bbox: [nx, ny, nw, nh],
                    bg_color: bg,
                    fg_color: fg,
                }
            })
            .collect();

        let line_count = lens_lines.len();
        let ocr_payload = LensOcrLines {
            id: lens_id.clone(),
            flow_id,
            image_w,
            image_h,
            lines: lens_lines,
        };
        let _ = app.emit("moonlens://lens-ocr-lines", ocr_payload);

        if ocr_lines.is_empty() {
            let _ = app.emit(
                "moonlens://lens-flow-done",
                LensFlowDone {
                    id: lens_id.clone(),
                    flow_id,
                    line_count: 0,
                },
            );
            continue;
        }

        // Concurrent translate tasks, bounded by user's
        // `performance.max_concurrent_translations` (default 4).
        let snapshot = settings.snapshot();
        let target = snapshot.subtitle.target_language.clone();
        let max_concurrent = snapshot.performance.max_concurrent_tasks.max(1) as usize;
        let semaphore = Arc::new(Semaphore::new(max_concurrent));

        let mut tasks: Vec<JoinHandle<()>> = Vec::with_capacity(line_count);
        for (i, line) in ocr_lines.into_iter().enumerate() {
            if text_is_already_target(&line.text, &target) {
                tracing::info!(
                    "lens line {i}: skipped translation (already {target}) — {:?}",
                    line.text.chars().take(20).collect::<String>()
                );
                let _ = app.emit(
                    "moonlens://lens-line-chunk",
                    LensLineChunk {
                        id: lens_id.clone(),
                        flow_id,
                        line_index: i,
                        delta: line.text.clone(),
                        done: true,
                    },
                );
                continue;
            }
            let sem = semaphore.clone();
            let app_c = app.clone();
            let id_c = lens_id.clone();
            let settings_c = settings.clone();
            let target_c = target.clone();
            let task = tauri::async_runtime::spawn(async move {
                let _permit = match sem.acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => return,
                };
                if let Err(e) = translate_line_streaming(
                    app_c,
                    id_c,
                    flow_id,
                    i,
                    line.text,
                    target_c,
                    settings_c,
                )
                .await
                {
                    tracing::info!("lens line {i} translate failed: {e}");
                }
            });
            tasks.push(task);
        }
        for t in tasks {
            let _ = t.await;
        }

        // Tauri 2 ships chunk events (`lens-line-chunk`) and the
        // terminator (`lens-flow-done`) over *different* named
        // channels, and the runtime does not guarantee ordering
        // across channels. Without a grace period the terminator
        // can race ahead of the very last `done=true` chunk and
        // the frontend commits a truncated translation. 80 ms is
        // far longer than the inter-channel jitter we've observed
        // (< 10 ms) and short enough to feel instant.
        tokio::time::sleep(Duration::from_millis(80)).await;

        let _ = app.emit(
            "moonlens://lens-flow-done",
            LensFlowDone {
                id: lens_id.clone(),
                flow_id,
                line_count,
            },
        );
    }
}

/// Translate a single line and stream tokens through
/// `moonlens://lens-line-chunk`. Bypasses the generic provider chain
/// — per-line cloud fallback is a plan.md S3 nice-to-have that the MVP
/// explicitly skips (each OCR flow fires N translate calls; racing the
/// chain per line would multiply cloud-rate-limit risk).
async fn translate_line_streaming(
    app: AppHandle,
    lens_id: String,
    flow_id: u64,
    line_index: usize,
    text: String,
    target_language: String,
    settings: Arc<SettingsStore>,
) -> Result<String, ProviderError> {
    let snap = settings.snapshot();
    let cfg = snap
        .providers
        .configs
        .get("ollama.gemma4")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let lens_id_sink = lens_id.clone();
    let app_sink = app.clone();
    let sink: Arc<dyn Fn(String, bool) + Send + Sync> = Arc::new(move |delta, done| {
        let _ = app_sink.emit(
            "moonlens://lens-line-chunk",
            LensLineChunk {
                id: lens_id_sink.clone(),
                flow_id,
                line_index,
                delta,
                done,
            },
        );
    });

    let translator = OllamaTranslator::new("ollama.gemma4", &cfg).with_chunk_sink(sink);
    let out = translator
        .run(TaskInput::Translate {
            text,
            source_language: "auto".into(),
            target_language,
        })
        .await?;
    match out {
        TaskOutput::Text(s) => Ok(s),
        _ => Err(ProviderError::Other(
            "ollama returned unexpected output".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_watchers_start_empty() {
        let w = LensStreamWatchers::new();
        assert_eq!(w.running_count(), 0);
        assert!(!w.is_running("missing"));
    }

    #[test]
    fn stop_on_unknown_id_is_a_noop() {
        let w = LensStreamWatchers::new();
        w.stop("missing");
        assert_eq!(w.running_count(), 0);
    }

    #[test]
    fn flow_ids_are_monotonic_and_unique() {
        let w = LensStreamWatchers::new();
        let a = w.next_flow_id();
        let b = w.next_flow_id();
        let c = w.next_flow_id();
        assert!(a < b && b < c);
    }

    #[test]
    fn stop_all_drains_registry_without_panicking() {
        let w = LensStreamWatchers::new();
        w.stop_all();
        assert_eq!(w.running_count(), 0);
    }

    #[test]
    fn text_is_already_target_detects_hiragana_katakana_for_ja() {
        assert!(text_is_already_target("こんにちは", "ja"));
        assert!(text_is_already_target("カタカナ", "ja"));
        assert!(text_is_already_target("漢字だけ", "ja"));
    }

    #[test]
    fn text_is_already_target_skips_pure_english_for_ja() {
        assert!(!text_is_already_target("Hello world", "ja"));
        assert!(!text_is_already_target("It was quite interesting.", "ja"));
    }

    #[test]
    fn text_is_already_target_auto_never_matches() {
        // "auto" target means "translate everything" — detection must
        // opt out so nothing is ever silently skipped.
        assert!(!text_is_already_target("こんにちは", "auto"));
        assert!(!text_is_already_target("Hello", ""));
    }

    #[test]
    fn text_is_already_target_requires_min_length_for_kanji_only() {
        // A single kanji is too ambiguous (could be a Chinese word)
        // to short-circuit on. Need at least two CJK chars.
        assert!(!text_is_already_target("日", "ja"));
        assert!(text_is_already_target("日本", "ja"));
    }

    #[test]
    fn text_is_already_target_korean_and_chinese() {
        assert!(text_is_already_target("안녕하세요", "ko"));
        assert!(text_is_already_target("你好世界", "zh"));
        // Japanese kana must NOT be classified as Chinese.
        assert!(!text_is_already_target("こんにちは", "zh"));
    }

    #[test]
    fn contrast_text_picks_black_on_light_bg() {
        assert_eq!(contrast_text([255, 255, 255]), [10, 10, 10]);
        assert_eq!(contrast_text([240, 230, 200]), [10, 10, 10]);
    }

    #[test]
    fn contrast_text_picks_white_on_dark_bg() {
        assert_eq!(contrast_text([0, 0, 0]), [245, 245, 245]);
        assert_eq!(contrast_text([30, 30, 70]), [245, 245, 245]);
    }

    #[test]
    fn relative_luminance_matches_wcag_anchors() {
        // Reference values from the WCAG spec.
        assert!((relative_luminance([0, 0, 0])).abs() < 1e-5);
        assert!((relative_luminance([255, 255, 255]) - 1.0).abs() < 1e-5);
        // Pure red should be about 0.2126 (R coefficient).
        let r = relative_luminance([255, 0, 0]);
        assert!((r - 0.2126).abs() < 1e-3, "expected ~0.2126, got {r}");
    }

    #[test]
    fn sample_text_color_picks_dominant_non_bg_color() {
        use image::{DynamicImage, Rgb, RgbImage};
        // 20x20 white image with a vertical red stripe in the middle.
        let mut img = RgbImage::new(20, 20);
        for y in 0..20 {
            for x in 0..20 {
                let colour = if x >= 8 && x <= 12 {
                    Rgb([230, 40, 40]) // red glyph
                } else {
                    Rgb([250, 250, 250]) // white bg
                };
                img.put_pixel(x, y, colour);
            }
        }
        let dynamic = DynamicImage::ImageRgb8(img);
        let bg = [250u8, 250, 250];
        let fg = sample_text_color(&dynamic, [0.0, 0.0, 20.0, 20.0], bg);
        // Sampler should land somewhere in the red ballpark, not the
        // white bg and not fall back to the contrast pick (#0a0a0a).
        assert!(fg[0] > 180 && fg[1] < 80 && fg[2] < 80, "expected red-ish, got {fg:?}");
    }

    #[test]
    fn sample_text_color_falls_back_when_no_text_pixels() {
        use image::{DynamicImage, Rgb, RgbImage};
        // Solid white bbox — no "text" pixels to sample.
        let mut img = RgbImage::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                img.put_pixel(x, y, Rgb([240, 240, 240]));
            }
        }
        let dynamic = DynamicImage::ImageRgb8(img);
        let fg = sample_text_color(&dynamic, [0.0, 0.0, 10.0, 10.0], [240, 240, 240]);
        // Should use contrast_text for [240,240,240] → dark text.
        assert_eq!(fg, [10, 10, 10]);
    }

    #[test]
    fn flow_ids_wrap_without_panicking() {
        // Runtime contract: saturating vs wrapping is an implementation
        // detail, but it must not panic either way. Pin it so a future
        // refactor that switches to saturating_add does not silently
        // stall the flow_id stream.
        let w = LensStreamWatchers::new();
        // Force the counter close to u64::MAX so next_flow_id has to
        // wrap. We reach into the field directly because this is the
        // test module.
        *w.flow_counter.lock() = u64::MAX - 1;
        let a = w.next_flow_id(); // u64::MAX
        let b = w.next_flow_id(); // wraps to 0
        assert_eq!(a, u64::MAX);
        assert_eq!(b, 0);
    }
}
