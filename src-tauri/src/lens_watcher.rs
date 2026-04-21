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
    /// Mean RGB sampled from a 2 px rim around the bbox. Used as a
    /// flat fill when `bg_gradient` is `None`, and as the fallback
    /// any time the gradient sampler fails.
    pub bg_color: [u8; 3],
    /// Optional linear gradient spec. Emitted when the rim has a
    /// noticeable colour slope along one axis (common for UI cards
    /// with subtle gradients, content over photos, etc.). The lens
    /// UI renders it as `linear-gradient` so the card blends into
    /// the source instead of posting a flat block of solid colour.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_gradient: Option<BgGradient>,
    /// Text colour sampled from the dominant non-bg pixels inside
    /// the bbox (source text's real colour). Falls back to a WCAG
    /// contrast black/white pick when the bbox has too few "text"
    /// pixels to sample reliably.
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

/// Gradient specification emitted to the lens UI when the rim of
/// `bbox` has a noticeable colour slope along one axis. The UI
/// renders `background: linear-gradient(<angle>deg, from, to)`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BgGradient {
    /// CSS `linear-gradient` angle, where 0° is "to top" and
    /// rotates clockwise (CSS convention). 180° = vertical
    /// top-to-bottom, 90° = horizontal left-to-right.
    pub angle: u16,
    pub from: [u8; 3],
    pub to: [u8; 3],
}

fn color_distance_sq(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

/// Decide whether the rim has a dominant gradient direction. Returns
/// `Some` when either axis has a colour delta whose squared distance
/// clears `MIN_DELTA_SQ` (~18 units in Euclidean RGB); the axis with
/// the larger delta wins. Returns `None` when the rim is
/// "uniform enough" — the caller then falls back to a flat fill,
/// which is cheaper for the renderer and avoids spurious gradients
/// on solid-colour sources like chat windows.
pub fn detect_rim_gradient(
    top: [u8; 3],
    bottom: [u8; 3],
    left: [u8; 3],
    right: [u8; 3],
) -> Option<BgGradient> {
    const MIN_DELTA_SQ: i32 = 18 * 18;
    let v = color_distance_sq(top, bottom);
    let h = color_distance_sq(left, right);
    if v < MIN_DELTA_SQ && h < MIN_DELTA_SQ {
        return None;
    }
    if v >= h {
        // Vertical gradient. CSS 180deg = top→bottom.
        Some(BgGradient {
            angle: 180,
            from: top,
            to: bottom,
        })
    } else {
        // Horizontal gradient. CSS 90deg = left→right.
        Some(BgGradient {
            angle: 90,
            from: left,
            to: right,
        })
    }
}

/// Sample the mean RGB of ONE edge strip of the rim (top/bottom/
/// left/right of the 2 px border around `bbox`). Small and cheap —
/// each call iterates ≤ 2 × max(bbox_w, bbox_h) pixels.
fn sample_edge(
    img: &image::DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Option<[u8; 3]> {
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);
    let x0 = x0.max(0);
    let y0 = y0.max(0);
    let x1 = x1.min(w - 1);
    let y1 = y1.min(h - 1);
    if x1 < x0 || y1 < y0 {
        return None;
    }
    let mut r: u64 = 0;
    let mut g: u64 = 0;
    let mut b: u64 = 0;
    let mut n: u64 = 0;
    for py in y0..=y1 {
        for px in x0..=x1 {
            let p = rgb.get_pixel(px as u32, py as u32).0;
            r += p[0] as u64;
            g += p[1] as u64;
            b += p[2] as u64;
            n += 1;
        }
    }
    if n == 0 {
        return None;
    }
    Some([(r / n) as u8, (g / n) as u8, (b / n) as u8])
}

/// Sample the 2 px rim around `bbox` as four edge means (top /
/// bottom / left / right) plus an overall mean for the flat-fill
/// fallback. Callers pass the result through [`detect_rim_gradient`]
/// to decide whether to ship a gradient or a solid colour.
pub fn sample_rim_edges(
    img: &image::DynamicImage,
    bbox: [f32; 4],
) -> ([u8; 3], Option<BgGradient>) {
    const RIM: i32 = 2;
    let x = bbox[0] as i32;
    let y = bbox[1] as i32;
    let bw = bbox[2] as i32;
    let bh = bbox[3] as i32;
    let solid = sample_rim_color(img, bbox);
    if bw <= 0 || bh <= 0 {
        return (solid, None);
    }
    let top = sample_edge(img, x - RIM, y - RIM, x + bw + RIM - 1, y - 1)
        .unwrap_or(solid);
    let bottom = sample_edge(img, x - RIM, y + bh, x + bw + RIM - 1, y + bh + RIM - 1)
        .unwrap_or(solid);
    let left = sample_edge(img, x - RIM, y, x - 1, y + bh - 1).unwrap_or(solid);
    let right =
        sample_edge(img, x + bw, y, x + bw + RIM - 1, y + bh - 1).unwrap_or(solid);
    let gradient = detect_rim_gradient(top, bottom, left, right);
    (solid, gradient)
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

/// Does `text` look like OCR noise — unrecognisable glyphs that ocrs
/// produced from non-Latin source or low-quality input? Sending such
/// output to an LLM wastes cycles and returns nonsense translations.
/// Heuristic stack (all bias toward *keeping* a line — false
/// positives hide real short content):
///   - fewer than 3 alphanumeric characters → too little to mean
///     anything useful
///   - non-letter / total ratio above 60 % → mostly punctuation or
///     symbols, almost always misread glyphs
///   - for Latin-script lines ≥ 4 letters, vowel ratio below 15 %
///     → `bcdfgz`-style random consonants, a hallmark of ocrs
///     running on CJK source it cannot parse
pub fn looks_like_ocr_noise(text: &str) -> bool {
    let trimmed = text.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.is_empty() {
        return true;
    }
    let letter_or_digit: Vec<char> = chars
        .iter()
        .copied()
        .filter(|c| c.is_alphanumeric())
        .collect();
    if letter_or_digit.len() < 3 {
        return true;
    }
    let total = chars.len();
    let non_letter = chars.iter().filter(|c| !c.is_alphanumeric() && !c.is_whitespace()).count();
    if total > 0 && (non_letter as f32 / total as f32) > 0.6 {
        return true;
    }
    // Latin-only garbled text: ocrs's usual failure mode when pointed
    // at Japanese/Chinese is to emit mostly-consonant nonsense. CJK
    // text itself is not scored here (it has no Latin vowels but
    // should not be flagged as noise).
    let latin_letters: Vec<char> = chars
        .iter()
        .copied()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    if latin_letters.len() >= 4 && (latin_letters.len() as f32 / total as f32) > 0.6 {
        let vowels = latin_letters
            .iter()
            .filter(|c| matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y'))
            .count();
        let ratio = vowels as f32 / latin_letters.len() as f32;
        if ratio < 0.15 {
            return true;
        }
    }
    false
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

/// A run of OCR lines that belong to the same sentence / paragraph.
/// We group lines here only for **LLM context** — each member still
/// gets its own card on screen at its original bbox; the paragraph
/// only exists so Gemma receives the whole sentence at once and can
/// resolve cross-line anaphora, carry register, etc.
#[derive(Debug, Clone)]
struct Paragraph {
    /// Ordered indices into the OcrLine Vec this paragraph covers.
    /// `members.len() == 1` is the default / single-line case.
    members: Vec<usize>,
    /// Member line texts joined with `\n`. Sent to the LLM verbatim;
    /// the system prompt instructs it to preserve line breaks, which
    /// lets us split the output back to each member card.
    text: String,
}

/// Group OCR lines into paragraphs. Conservative by design: merging
/// unrelated lines corrupts every translation downstream, so every
/// condition below must hold before two consecutive (sorted-by-y)
/// lines share a paragraph:
///   - vertical gap ≤ 0.5 × min line height (real line break)
///   - horizontal range overlap ≥ 60% of min width (same column)
///   - upper line does NOT end with `.!?。！？` (no hard break)
///   - both lines have ≥ 6 characters (avoid grouping UI labels)
fn group_paragraphs(lines: &[OcrLine]) -> Vec<Paragraph> {
    if lines.is_empty() {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..lines.len()).collect();
    order.sort_by(|&a, &b| {
        lines[a].bbox[1]
            .partial_cmp(&lines[b].bbox[1])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let ends_paragraph = |text: &str| -> bool {
        let trimmed = text.trim_end();
        let last = trimmed.chars().last();
        matches!(
            last,
            Some('.') | Some('!') | Some('?') | Some('。') | Some('！') | Some('？')
        )
    };
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for &idx in &order {
        if current.is_empty() {
            current.push(idx);
            continue;
        }
        let prev_idx = *current.last().unwrap();
        let prev = &lines[prev_idx].bbox;
        let curr = &lines[idx].bbox;
        let prev_bottom = prev[1] + prev[3];
        let gap = (curr[1] - prev_bottom).max(0.0);
        let min_h = prev[3].min(curr[3]).max(1.0);
        let close_vertically = gap <= 0.5 * min_h;
        let prev_left = prev[0];
        let prev_right = prev[0] + prev[2];
        let curr_left = curr[0];
        let curr_right = curr[0] + curr[2];
        let overlap = (prev_right.min(curr_right) - prev_left.max(curr_left)).max(0.0);
        let min_w = prev[2].min(curr[2]).max(1.0);
        let horizontally_aligned = overlap / min_w >= 0.6;
        let prev_finished = ends_paragraph(&lines[prev_idx].text);
        let both_substantial =
            lines[prev_idx].text.chars().count() >= 6 && lines[idx].text.chars().count() >= 6;
        if close_vertically && horizontally_aligned && !prev_finished && both_substantial {
            current.push(idx);
        } else {
            groups.push(std::mem::take(&mut current));
            current.push(idx);
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
        .into_iter()
        .map(|members| {
            let text = members
                .iter()
                .map(|&i| lines[i].text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            Paragraph { members, text }
        })
        .collect()
}

/// Distribute a multi-line translation back to its member OCR lines.
/// Happy path: Gemma preserved the newline count and we get a clean
/// 1:1 map. Degraded paths:
///   - too few segments → put the whole translation on the first
///     line, leave the rest empty (better than truncating)
///   - too many segments → keep the 1:1 map for all but the last
///     line, which absorbs every extra segment joined with a space
pub fn split_paragraph_translation(translation: &str, line_count: usize) -> Vec<String> {
    if line_count == 0 {
        return Vec::new();
    }
    let raw: Vec<&str> = translation.split('\n').collect();
    // Drop leading / trailing blanks that some models tack on.
    let trimmed: Vec<&str> = raw
        .iter()
        .map(|s| s.trim_end())
        .collect::<Vec<_>>()
        .into_iter()
        .skip_while(|s| s.is_empty())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .skip_while(|s| s.is_empty())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if trimmed.len() == line_count {
        return trimmed.into_iter().map(|s| s.to_string()).collect();
    }
    if trimmed.len() > line_count {
        let mut out: Vec<String> =
            trimmed.iter().take(line_count - 1).map(|s| s.to_string()).collect();
        let tail = trimmed[(line_count - 1)..].join(" ");
        out.push(tail);
        return out;
    }
    // Too few segments: bundle the entire translation on the first
    // card so the user can at least read it, and leave the others
    // blank. Blank cards will fall back to source text in the
    // frontend's per-line commit.
    let mut out = vec![translation.trim().to_string()];
    out.extend((1..line_count).map(|_| String::new()));
    out
}

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
                // Drop lines that look like OCR noise here rather
                // than downstream — the Lens UI only ever sees
                // "real" lines, so there's no garbage card that
                // needs a translation round-trip to figure out.
                TaskOutput::Ocr { lines } => {
                    let kept: Vec<OcrLine> = lines
                        .into_iter()
                        .filter(|l| {
                            let drop = looks_like_ocr_noise(&l.text);
                            if drop {
                                tracing::debug!(
                                    "lens {lens_id}: dropped noise line {:?}",
                                    l.text
                                );
                            }
                            !drop
                        })
                        .collect();
                    kept
                }
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
                // Sample the rim along all four edges: the solid
                // mean stays as a safe fallback, and when the rim
                // actually changes colour across the bbox we emit a
                // gradient so the card blends into the source
                // instead of looking like a pasted block.
                let (bg, bg_gradient) = match &decoded {
                    Some(img) => sample_rim_edges(img, l.bbox),
                    None => ([128, 128, 128], None),
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
                    bg_gradient,
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

        // Group OCR lines into paragraphs for **LLM context only**.
        // Each OCR line still owns its own card above — the
        // paragraph is the translation unit, not the rendering
        // unit. Gemma sees the full sentence, splits its output
        // back across the member cards by the preserved line
        // breaks (see `translate_paragraph_streaming`).
        let paragraphs = group_paragraphs(&ocr_lines);

        let snapshot = settings.snapshot();
        let target = snapshot.subtitle.target_language.clone();
        let max_concurrent = snapshot.performance.max_concurrent_tasks.max(1) as usize;
        let semaphore = Arc::new(Semaphore::new(max_concurrent));

        // Keep originals around so the skip-same-language path can
        // echo the source text back into each member line's chunk.
        let originals: Vec<String> = ocr_lines.iter().map(|l| l.text.clone()).collect();

        let mut tasks: Vec<JoinHandle<()>> = Vec::with_capacity(paragraphs.len());
        for paragraph in paragraphs.into_iter() {
            if text_is_already_target(&paragraph.text, &target) {
                tracing::info!(
                    "lens paragraph [{:?}]: skipped translation (already {target}) — {:?}",
                    paragraph.members,
                    paragraph.text.chars().take(20).collect::<String>()
                );
                for &line_idx in &paragraph.members {
                    let text = originals
                        .get(line_idx)
                        .cloned()
                        .unwrap_or_default();
                    let _ = app.emit(
                        "moonlens://lens-line-chunk",
                        LensLineChunk {
                            id: lens_id.clone(),
                            flow_id,
                            line_index: line_idx,
                            delta: text,
                            done: true,
                        },
                    );
                }
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
                if let Err(e) = translate_paragraph_streaming(
                    app_c,
                    id_c,
                    flow_id,
                    paragraph,
                    target_c,
                    settings_c,
                )
                .await
                {
                    tracing::info!("lens paragraph translate failed: {e}");
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

/// Translate a paragraph (one or more OCR lines) and route the
/// result back to each member card. Single-line paragraphs stream
/// tokens in real time at their member's `line_index`. Multi-line
/// paragraphs buffer the whole output, split it on `\n`, and emit
/// exactly one `done=true` chunk per member — the UI then commits
/// each card atomically (no partial state while we wait for the
/// final token). This is what lets the LLM translate a sentence
/// with full context without forcing the frontend to merge cards.
async fn translate_paragraph_streaming(
    app: AppHandle,
    lens_id: String,
    flow_id: u64,
    paragraph: Paragraph,
    target_language: String,
    settings: Arc<SettingsStore>,
) -> Result<String, ProviderError> {
    // Single-line paragraphs keep the MVP streaming path — user sees
    // tokens accumulate via chunks, no newline-split bookkeeping.
    if paragraph.members.len() == 1 {
        return translate_line_streaming(
            app,
            lens_id,
            flow_id,
            paragraph.members[0],
            paragraph.text,
            target_language,
            settings,
        )
        .await;
    }

    let snap = settings.snapshot();
    let cfg = snap
        .providers
        .configs
        .get("ollama.gemma4")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    // Buffer the whole translation here; we can only split once the
    // final token arrives (Gemma occasionally reshuffles line breaks
    // mid-stream). Nothing is emitted to the frontend while the
    // stream is running — previous cards from the last flow keep
    // showing until we swap in the new paragraph.
    let buffer = Arc::new(parking_lot::Mutex::new(String::new()));
    let buffer_for_sink = buffer.clone();
    let sink: Arc<dyn Fn(String, bool) + Send + Sync> =
        Arc::new(move |delta, _done| {
            if !delta.is_empty() {
                buffer_for_sink.lock().push_str(&delta);
            }
        });

    let translator =
        OllamaTranslator::new("ollama.gemma4", &cfg).with_chunk_sink(sink);
    let out = translator
        .run(TaskInput::Translate {
            text: paragraph.text.clone(),
            source_language: "auto".into(),
            target_language,
        })
        .await?;

    // Some providers return the aggregated text via `run`; fall back
    // to the buffer if the sink path delivered more detail.
    let full = match out {
        TaskOutput::Text(s) if !s.is_empty() => s,
        _ => buffer.lock().clone(),
    };

    let segments = split_paragraph_translation(&full, paragraph.members.len());
    for (i, &line_idx) in paragraph.members.iter().enumerate() {
        let delta = segments.get(i).cloned().unwrap_or_default();
        let _ = app.emit(
            "moonlens://lens-line-chunk",
            LensLineChunk {
                id: lens_id.clone(),
                flow_id,
                line_index: line_idx,
                delta,
                done: true,
            },
        );
    }
    Ok(full)
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
    fn detect_rim_gradient_returns_none_for_uniform_rim() {
        let c = [200u8, 210, 220];
        assert_eq!(detect_rim_gradient(c, c, c, c), None);
    }

    #[test]
    fn detect_rim_gradient_picks_vertical_when_top_bottom_differ() {
        let top = [30, 30, 30];
        let bottom = [230, 230, 230];
        let side = [130, 130, 130];
        let g = detect_rim_gradient(top, bottom, side, side).expect("gradient");
        assert_eq!(g.angle, 180);
        assert_eq!(g.from, top);
        assert_eq!(g.to, bottom);
    }

    #[test]
    fn detect_rim_gradient_picks_horizontal_when_left_right_differ() {
        let side = [130, 130, 130];
        let left = [20, 10, 10];
        let right = [240, 210, 210];
        let g = detect_rim_gradient(side, side, left, right).expect("gradient");
        assert_eq!(g.angle, 90);
        assert_eq!(g.from, left);
        assert_eq!(g.to, right);
    }

    #[test]
    fn detect_rim_gradient_prefers_larger_delta_axis() {
        // Vertical delta dwarfs horizontal — even if both exceed the
        // threshold we should ship the vertical gradient.
        let top = [10, 10, 10];
        let bottom = [240, 240, 240];
        let left = [100, 100, 100];
        let right = [130, 130, 130];
        let g = detect_rim_gradient(top, bottom, left, right).expect("gradient");
        assert_eq!(g.angle, 180);
    }

    #[test]
    fn split_paragraph_translation_maps_one_to_one() {
        let segs = split_paragraph_translation("第一行\n第二行\n第三行", 3);
        assert_eq!(segs, vec!["第一行", "第二行", "第三行"]);
    }

    #[test]
    fn split_paragraph_translation_absorbs_extra_segments_into_last_line() {
        // Gemma tacked on a spurious newline — merge the tail into
        // the last member card so we don't silently drop content.
        let segs = split_paragraph_translation("A\nB\nC extra\ntrailing", 3);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "A");
        assert_eq!(segs[1], "B");
        assert!(segs[2].contains("C extra") && segs[2].contains("trailing"));
    }

    #[test]
    fn split_paragraph_translation_collapses_onto_first_when_too_few() {
        // Gemma ignored line breaks — keep the entire translation
        // visible on the first card instead of truncating.
        let segs = split_paragraph_translation("One long sentence.", 3);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "One long sentence.");
        assert_eq!(segs[1], "");
        assert_eq!(segs[2], "");
    }

    #[test]
    fn split_paragraph_translation_trims_leading_trailing_blank_lines() {
        let segs = split_paragraph_translation("\n\nA\nB\n\n", 2);
        assert_eq!(segs, vec!["A", "B"]);
    }

    #[test]
    fn group_paragraphs_merges_two_wrapped_lines() {
        let lines = vec![
            OcrLine {
                text: "The quick brown fox jumps".into(),
                bbox: [10.0, 10.0, 200.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "over the lazy dog".into(),
                bbox: [10.0, 32.0, 180.0, 20.0],
                confidence: 1.0,
            },
        ];
        let p = group_paragraphs(&lines);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].members, vec![0, 1]);
        assert!(p[0].text.contains('\n'));
    }

    #[test]
    fn group_paragraphs_splits_on_sentence_end() {
        let lines = vec![
            OcrLine {
                text: "Very interesting.".into(),
                bbox: [10.0, 10.0, 200.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Indeed it is strongly so".into(),
                bbox: [10.0, 32.0, 200.0, 20.0],
                confidence: 1.0,
            },
        ];
        let p = group_paragraphs(&lines);
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn group_paragraphs_keeps_short_ui_labels_separate() {
        // UI labels (< 6 chars) must stay as per-line cards so that
        // menu entries never merge into one translation request.
        let lines = vec![
            OcrLine {
                text: "File".into(),
                bbox: [10.0, 10.0, 60.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Edit".into(),
                bbox: [10.0, 32.0, 60.0, 20.0],
                confidence: 1.0,
            },
        ];
        let p = group_paragraphs(&lines);
        assert_eq!(p.len(), 2);
    }

    #[test]
    fn noise_filter_keeps_normal_english() {
        assert!(!looks_like_ocr_noise("Hello world"));
        assert!(!looks_like_ocr_noise("The quick brown fox jumps."));
        assert!(!looks_like_ocr_noise("On your left"));
    }

    #[test]
    fn noise_filter_keeps_normal_japanese() {
        // CJK lines should pass straight through — Latin-vowel check
        // must not fire on hiragana/kanji.
        assert!(!looks_like_ocr_noise("こんにちは世界"));
        assert!(!looks_like_ocr_noise("日本語のテキストです"));
    }

    #[test]
    fn noise_filter_rejects_very_short_text() {
        assert!(looks_like_ocr_noise("Hi"));
        assert!(looks_like_ocr_noise("!"));
        assert!(looks_like_ocr_noise(""));
    }

    #[test]
    fn noise_filter_rejects_punctuation_heavy_glyphs() {
        // Classic ocrs misread of CJK: long strings of @/#/* noise.
        assert!(looks_like_ocr_noise("@#$%^&*"));
        assert!(looks_like_ocr_noise("...:::;;;"));
    }

    #[test]
    fn noise_filter_rejects_consonant_only_latin() {
        // ocrs aimed at Japanese often emits consonant-heavy garbage.
        assert!(looks_like_ocr_noise("bcdfgzhk"));
        assert!(looks_like_ocr_noise("zxcvbnmsdfg"));
        // But real words with a natural consonant:vowel balance pass.
        assert!(!looks_like_ocr_noise("rhythm music"));
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
