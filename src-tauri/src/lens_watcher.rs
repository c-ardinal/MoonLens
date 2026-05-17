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

use crate::{
    lens::{self, LensRegistryHandle},
    lens_inpaint,
    lens_pipeline::{
        run_lens_pipeline, CapturedFrame, ChainBackedLensTranslator, LensSink, PipelineConfig,
        PipelineLensOcr, PreCapturedInput,
    },
    pipeline::Pipeline,
    providers::{Capability, OcrLine, ProviderError, TaskInput, TaskOutput},
    region_watcher::{average_hash, hamming},
    settings::SettingsStore,
};
use base64::Engine as _;

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
                            tracing::debug!("lens {id_for_poll}: position-poll trigger ({c:?})");
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
                    let interval_ms = settings_for_auto.snapshot().performance.lens_auto_poll_ms;
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
            run_capture_loop(id_clone, app, pipeline, lenses, settings, stop_t, trigger_t).await;
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

/// Detected horizontal alignment of source text within its
/// containing region. Serialised as the lower-case CSS keyword so
/// the frontend can use the value directly for `text-align`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

impl Default for TextAlign {
    fn default() -> Self {
        TextAlign::Left
    }
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
    /// `[x, y, w, h]` in image pixel coordinates. This is the **tiled**
    /// bbox — expanded vertically to the midpoint with adjacent lines
    /// and horizontally by a small padding so adjacent cards meet
    /// exactly without gap or overlap.
    pub bbox: [f32; 4],
    /// Original OCR bbox **before** tile expansion, also in image
    /// pixel coordinates. The frontend uses the height of this rect
    /// to size the translation font to match the source glyph height
    /// — without it, card height (which includes the tile padding)
    /// makes short translations look absurdly large over small source.
    pub source_bbox: [f32; 4],
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
    /// CSS `font-weight` estimated from the source glyph's stroke
    /// thickness (see [`estimate_font_weight`]). One of
    /// 400 (regular), 600 (semi-bold), or 700 (bold). The frontend
    /// renders the translation at the same weight so a bold source
    /// does not become a thin translation.
    pub font_weight: u16,
    /// Source-derived font size in image pixels — median height of
    /// the OCR member bboxes assigned to this card. Used verbatim
    /// by the frontend (after DPR compensation) so translation
    /// glyphs render at the same visible height as the source. No
    /// cap-height reduction: the earlier `× 0.82` shrink made
    /// translations look systematically smaller than the original.
    pub font_size_px: f32,
    /// Source-derived `line-height` ratio (baseline-to-baseline
    /// distance ÷ glyph height). Measured from the vertical gaps
    /// between the card's OCR members; falls back to 1.25 when the
    /// block has a single member and no gap to sample. Frontend
    /// uses this as the CSS `line-height` so wrapped translation
    /// lines occupy the same vertical rhythm as the source.
    pub line_height: f32,
    /// Number of source OCR lines covered by this card. The
    /// frontend uses this as the target line count: if the wrapped
    /// translation would exceed it, horizontal compression
    /// (`transform: scaleX(<1)`) shrinks the text to fit without
    /// changing font-size, weight, or adding rows — preserving
    /// strict source fidelity for size and stroke.
    pub source_line_count: u32,
    /// Detected horizontal alignment of the source text relative
    /// to its containing layout block (or, when no block info is
    /// available, the column inferred from OCR member bbox
    /// variance). Frontend maps this to CSS `text-align` so a
    /// centered heading stays centered and a right-aligned status
    /// strip stays right-aligned even when the translation is
    /// narrower than the source. Values: `"left"`, `"center"`,
    /// `"right"`.
    pub text_align: TextAlign,
    /// Tight rectangle the inpaint patch (`bg_patch`) covers,
    /// always in the same image-pixel space as `bbox`. Often
    /// strictly NARROWER than `bbox`: `bbox` is widened toward
    /// the layout block edge in the direction of `text_align` so
    /// the rendered translation has slack to actually align,
    /// while `inpaint_bbox` stays tight to the source glyphs so
    /// the bg-patch does not erase neighbouring content. Frontend
    /// uses the offset (`inpaint_bbox - bbox`) to position the
    /// bg-patch image inside the card.
    pub inpaint_bbox: [f32; 4],
    /// Base64-encoded PNG of the inpainted source region, sized to
    /// match `source_bbox`. The frontend draws it as the card
    /// background so the card blends into the surrounding image
    /// even on complex (gradient / photo) surfaces. `None` when the
    /// captured frame could not be decoded; frontend falls back to
    /// the `bg_color` / `bg_gradient` flat fill in that case.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg_patch: Option<String>,
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
/// renders `background: linear-gradient(<angle>deg, stop0, stop1,
/// ...)` using however many stops the sampler produces. Keeping
/// `from` / `to` for backward compatibility — they mirror the first
/// and last entries of `stops`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BgGradient {
    /// CSS `linear-gradient` angle, where 0° is "to top" and
    /// rotates clockwise (CSS convention). 180° = vertical
    /// top-to-bottom, 90° = horizontal left-to-right.
    pub angle: u16,
    /// Start colour. Alias of `stops[0]` — kept so older frontends
    /// continue to render without the `stops` array.
    pub from: [u8; 3],
    /// End colour. Alias of `stops[n-1]`.
    pub to: [u8; 3],
    /// Equally-spaced colour stops along the axis (2..=5). First and
    /// last match `from` / `to`; additional stops capture non-linear
    /// gradients (spotlights, vignettes, banded backgrounds).
    pub stops: Vec<[u8; 3]>,
}

fn color_distance_sq(a: [u8; 3], b: [u8; 3]) -> i32 {
    let dr = a[0] as i32 - b[0] as i32;
    let dg = a[1] as i32 - b[1] as i32;
    let db = a[2] as i32 - b[2] as i32;
    dr * dr + dg * dg + db * db
}

fn average_rgb(a: [u8; 3], b: [u8; 3]) -> [u8; 3] {
    [
        ((a[0] as u16 + b[0] as u16) / 2) as u8,
        ((a[1] as u16 + b[1] as u16) / 2) as u8,
        ((a[2] as u16 + b[2] as u16) / 2) as u8,
    ]
}

/// Decide whether the rim has a dominant gradient direction from the
/// four corner samples (`tl` / `tr` / `bl` / `br`) and four edge
/// midpoint samples. Tries four candidate directions (vertical,
/// horizontal, ↘ diagonal, ↗ anti-diagonal) and picks the one whose
/// endpoint-pair has the largest Euclidean colour delta; returns
/// `None` when no candidate clears the minimum-delta threshold
/// (solid fills take the flat-colour path). Covers the common cases
/// we previously missed: diagonal window-chrome gradients and any
/// oblique background where the edge-mean approach chose the wrong
/// axis.
pub fn detect_rim_gradient(
    top: [u8; 3],
    bottom: [u8; 3],
    left: [u8; 3],
    right: [u8; 3],
    tl: [u8; 3],
    tr: [u8; 3],
    bl: [u8; 3],
    br: [u8; 3],
) -> Option<BgGradient> {
    const MIN_DELTA_SQ: i32 = 18 * 18;

    // Candidate gradients. Edge pair uses edge midpoint samples for
    // the cardinal axes; corner pair drives the diagonals so that
    // "top-left dark → bottom-right light" style lighting is caught.
    let candidates: [(u16, [u8; 3], [u8; 3]); 4] = [
        // Vertical: top → bottom. CSS 180deg = to bottom.
        (180, top, bottom),
        // Horizontal: left → right. CSS 90deg = to right.
        (90, left, right),
        // ↘ diagonal: top-left → bottom-right. CSS 135deg =
        // to bottom-right.
        (135, tl, br),
        // ↗ anti-diagonal: bottom-left → top-right. CSS 45deg =
        // to top-right.
        (45, bl, tr),
    ];

    let best = candidates
        .iter()
        .max_by_key(|(_, a, b)| color_distance_sq(*a, *b))?;

    if color_distance_sq(best.1, best.2) < MIN_DELTA_SQ {
        return None;
    }
    Some(BgGradient {
        angle: best.0,
        from: best.1,
        to: best.2,
        stops: vec![best.1, best.2],
    })
}

#[cfg(test)]
/// Backward-compat test helper: cardinal-only caller that mixes
/// corner samples in via simple averages. Keeps older tests
/// readable without repeating the 8-arg signature everywhere.
fn detect_rim_gradient_cardinals(
    top: [u8; 3],
    bottom: [u8; 3],
    left: [u8; 3],
    right: [u8; 3],
) -> Option<BgGradient> {
    detect_rim_gradient(
        top,
        bottom,
        left,
        right,
        average_rgb(top, left),
        average_rgb(top, right),
        average_rgb(bottom, left),
        average_rgb(bottom, right),
    )
}

/// Sample the mean RGB of ONE edge strip of the rim (top/bottom/
/// left/right of the 2 px border around `bbox`). Small and cheap —
/// each call iterates ≤ 2 × max(bbox_w, bbox_h) pixels.
fn sample_edge(img: &image::DynamicImage, x0: i32, y0: i32, x1: i32, y1: i32) -> Option<[u8; 3]> {
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

/// Refine a detected gradient by **resampling the actual bbox
/// interior** along the picked axis. Rim samples alone can lie
/// (adjacent UI chrome, neighbouring OCR lines poison them), so
/// once we know the direction we pull the stop colours from
/// pixels that were really under the lens card — excluding glyph
/// pixels via the distance-from-bg filter. Returns `None` when
/// the inside sampler can't find enough background pixels; the
/// caller keeps the rim-based 2-stop gradient in that case.
fn refine_gradient_from_bbox(
    img: &image::DynamicImage,
    bbox: [f32; 4],
    rim_start: [u8; 3],
    rim_end: [u8; 3],
    axis: u16,
) -> Option<Vec<[u8; 3]>> {
    // 5 stops gives enough resolution for spotlights / vignettes
    // without overwhelming CSS. Diagonal axes (45° / 135°) skip
    // this refinement — the corner-based 2-stop is already accurate
    // because corners ARE positions, not edge averages.
    const STOPS: usize = 5;
    if axis != 90 && axis != 180 {
        return None;
    }
    let rgb = img.to_rgb8();
    let (iw, ih) = (rgb.width() as i32, rgb.height() as i32);
    let x = bbox[0] as i32;
    let y = bbox[1] as i32;
    let bw = bbox[2] as i32;
    let bh = bbox[3] as i32;
    if bw < 10 || bh < 10 {
        return None;
    }
    // Filter width relative to the *local* bg estimate, interpolated
    // between the two rim samples. Much tighter than a single global
    // bg_estimate because the bg itself varies along the axis. 70
    // units tolerates JPEG / compositor noise while still rejecting
    // glyph strokes (which differ from the local bg by 100+ units).
    const BG_RADIUS_SQ: i32 = 70 * 70;

    let mut stops: Vec<[u8; 3]> = Vec::with_capacity(STOPS);
    for i in 0..STOPS {
        let t = i as f32 / (STOPS as f32 - 1.0);
        let (sx, sy, ex, ey) = if axis == 180 {
            // Vertical: horizontal strip at t along y.
            let band_h = (bh / (STOPS as i32 * 2)).max(1);
            let cy = y + ((bh as f32 - 1.0) * t) as i32;
            (x, cy - band_h, x + bw - 1, cy + band_h)
        } else {
            // Horizontal: vertical strip at t along x.
            let band_w = (bw / (STOPS as i32 * 2)).max(1);
            let cx = x + ((bw as f32 - 1.0) * t) as i32;
            (cx - band_w, y, cx + band_w, y + bh - 1)
        };
        let sx = sx.max(0);
        let sy = sy.max(0);
        let ex = ex.min(iw - 1);
        let ey = ey.min(ih - 1);
        if sx > ex || sy > ey {
            return None;
        }

        // Interpolate a local bg estimate so wide-range gradients
        // don't collapse under a single global threshold.
        let local_bg = [
            (rim_start[0] as f32 + (rim_end[0] as f32 - rim_start[0] as f32) * t) as i32,
            (rim_start[1] as f32 + (rim_end[1] as f32 - rim_start[1] as f32) * t) as i32,
            (rim_start[2] as f32 + (rim_end[2] as f32 - rim_start[2] as f32) * t) as i32,
        ];

        let mut r: u64 = 0;
        let mut g: u64 = 0;
        let mut b: u64 = 0;
        let mut n: u64 = 0;
        for py in sy..=ey {
            for px in sx..=ex {
                let p = rgb.get_pixel(px as u32, py as u32).0;
                let dr = p[0] as i32 - local_bg[0];
                let dg = p[1] as i32 - local_bg[1];
                let db = p[2] as i32 - local_bg[2];
                if dr * dr + dg * dg + db * db < BG_RADIUS_SQ {
                    r += p[0] as u64;
                    g += p[1] as u64;
                    b += p[2] as u64;
                    n += 1;
                }
            }
        }
        // Require at least 8 bg pixels in this band; otherwise the
        // sample is unreliable (glyph ink dominates the row). Fall
        // back to the interpolated rim estimate so the gradient is
        // still smooth across bands.
        if n < 8 {
            stops.push([
                local_bg[0].clamp(0, 255) as u8,
                local_bg[1].clamp(0, 255) as u8,
                local_bg[2].clamp(0, 255) as u8,
            ]);
        } else {
            stops.push([(r / n) as u8, (g / n) as u8, (b / n) as u8]);
        }
    }
    Some(stops)
}

/// Is the bbox surround "clean enough" for a linear gradient? Linear
/// gradients have the algebraic property `tl + br ≈ tr + bl`; real
/// photos / patterns violate it wildly, and shipping a linear
/// gradient over a patterned source produces exactly the
/// "wrong-colour block" artefact the user reported.
fn rim_is_clean_linear_gradient(tl: [u8; 3], tr: [u8; 3], bl: [u8; 3], br: [u8; 3]) -> bool {
    // Midpoint predicted by a true linear gradient along either
    // diagonal should be the average of the opposite corners.
    let diag_mid = average_rgb(tl, br);
    let anti_mid = average_rgb(tr, bl);
    // Allow ~22 units in Euclidean RGB (504 sq) before we call the
    // surround "complex". Slightly above MIN_DELTA_SQ so cases that
    // ARE clean but noisy still qualify.
    color_distance_sq(diag_mid, anti_mid) < 22 * 22
}

/// Sample the 2 px rim around `bbox` as four edge midpoints (top /
/// bottom / left / right) and four corner patches (tl / tr / bl /
/// br), plus an overall mean for the flat-fill fallback. The
/// corners drive the diagonal gradient candidates in
/// [`detect_rim_gradient`], which is what lets the card follow a
/// diagonal light source instead of guessing the wrong axis.
pub fn sample_rim_edges(
    img: &image::DynamicImage,
    bbox: [f32; 4],
) -> ([u8; 3], Option<BgGradient>) {
    const RIM: i32 = 2;
    // Patch size for corner sampling — a small square just outside
    // the corner of the bbox. Larger than the rim so we get a
    // stable colour even when the source gradient is noisy.
    const CORNER: i32 = 4;
    let x = bbox[0] as i32;
    let y = bbox[1] as i32;
    let bw = bbox[2] as i32;
    let bh = bbox[3] as i32;
    let solid = sample_rim_color(img, bbox);
    if bw <= 0 || bh <= 0 {
        return (solid, None);
    }

    // Edge midpoint strips — narrower than before so a long bbox
    // does not average out a gradient into a flat mean.
    let mid_w = (bw / 3).max(1);
    let mid_h = (bh / 3).max(1);
    let cx = x + bw / 2;
    let cy = y + bh / 2;
    let top = sample_edge(img, cx - mid_w / 2, y - RIM, cx + mid_w / 2, y - 1).unwrap_or(solid);
    let bottom = sample_edge(
        img,
        cx - mid_w / 2,
        y + bh,
        cx + mid_w / 2,
        y + bh + RIM - 1,
    )
    .unwrap_or(solid);
    let left = sample_edge(img, x - RIM, cy - mid_h / 2, x - 1, cy + mid_h / 2).unwrap_or(solid);
    let right = sample_edge(
        img,
        x + bw,
        cy - mid_h / 2,
        x + bw + RIM - 1,
        cy + mid_h / 2,
    )
    .unwrap_or(solid);

    // Corner patches just outside each bbox corner — these detect
    // diagonal lighting that the edge midpoints alone cannot.
    let tl = sample_edge(img, x - CORNER, y - CORNER, x - 1, y - 1).unwrap_or(solid);
    let tr = sample_edge(img, x + bw, y - CORNER, x + bw + CORNER - 1, y - 1).unwrap_or(solid);
    let bl = sample_edge(img, x - CORNER, y + bh, x - 1, y + bh + CORNER - 1).unwrap_or(solid);
    let br = sample_edge(
        img,
        x + bw,
        y + bh,
        x + bw + CORNER - 1,
        y + bh + CORNER - 1,
    )
    .unwrap_or(solid);

    // Complex source (photo, pattern, checker) fails the linear-
    // gradient symmetry test. Shipping a 2-colour gradient over a
    // textured background makes the card look painted-on at the
    // wrong colour — a solid fill is the correct downgrade.
    if !rim_is_clean_linear_gradient(tl, tr, bl, br) {
        return (solid, None);
    }

    let mut gradient = detect_rim_gradient(top, bottom, left, right, tl, tr, bl, br);
    // When the rim detector picked a cardinal axis, we can do
    // better: resample the inside of the bbox for multi-stop
    // gradients that capture vignettes and non-linear fades. The
    // refinement silently no-ops for diagonal axes (corner-based
    // sampling is already accurate there). Pass the correct pair
    // of edge-mean samples as the local-bg anchors — 180° runs
    // top→bottom, 90° runs left→right.
    if let Some(g) = gradient.as_mut() {
        let (rim_start, rim_end) = match g.angle {
            180 => (top, bottom),
            90 => (left, right),
            _ => (g.from, g.to),
        };
        if let Some(inner_stops) = refine_gradient_from_bbox(img, bbox, rim_start, rim_end, g.angle)
        {
            g.from = *inner_stops.first().unwrap();
            g.to = *inner_stops.last().unwrap();
            g.stops = inner_stops;
        }
    }
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
    [(r_sum / n) as u8, (g_sum / n) as u8, (b_sum / n) as u8]
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
    let mean = [(r_sum / n) as u8, (g_sum / n) as u8, (b_sum / n) as u8];
    let dr = mean[0] as i32 - br;
    let dg = mean[1] as i32 - bgg;
    let db = mean[2] as i32 - bb;
    if dr * dr + dg * dg + db * db < TEXT_THRESHOLD_SQ {
        return contrast_text(bg);
    }
    mean
}

/// Estimate the source text's font weight by measuring stroke
/// thickness inside `bbox`. OCR does not report weight, so we sample
/// it directly from the pixels the user sees.
///
/// Algorithm (per-row minimum run, more robust to mixed-width glyphs):
///  1. Mark every pixel whose colour distance to `bg` exceeds the
///     `TEXT_THRESHOLD_SQ` floor as a "text" pixel.
///  2. For each row containing text pixels, collect horizontal runs
///     and record the MINIMUM run width in that row — a vertical
///     stroke appears as a narrow run and dominates over the wide
///     horizontal bars of letters like `T`, `E`, so the min tracks
///     stroke width rather than glyph width.
///  3. Crop to the vertical band that actually contains text pixels
///     so the glyph_h denominator reflects real cap + descender
///     span, not the OCR bbox's padding.
///  4. Compute `median(min_runs) / glyph_h`. Empirically calibrated
///     against web / document screenshots:
///       - < 0.11 → 400 (regular)
///       - < 0.17 → 600 (semi-bold)
///       - ≥ 0.17 → 700 (bold)
///
/// Returns `400` whenever sampling is unreliable (empty / noisy bbox,
/// no text pixels, degenerate glyph height).
fn estimate_font_weight(img: &image::DynamicImage, bbox: [f32; 4], bg: [u8; 3]) -> u16 {
    const DEFAULT_WEIGHT: u16 = 400;
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);
    let x = bbox[0] as i32;
    let y = bbox[1] as i32;
    let bw = bbox[2] as i32;
    let bh = bbox[3] as i32;
    if bw <= 2 || bh <= 2 {
        return DEFAULT_WEIGHT;
    }
    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + bw).min(w);
    let y1 = (y + bh).min(h);
    if x1 - x0 < 3 || y1 - y0 < 3 {
        return DEFAULT_WEIGHT;
    }

    const TEXT_THRESHOLD_SQ: i32 = 60 * 60;
    let (br, bgg, bb) = (bg[0] as i32, bg[1] as i32, bg[2] as i32);

    let is_text = |px: i32, py: i32| -> bool {
        let p = rgb.get_pixel(px as u32, py as u32).0;
        let dr = p[0] as i32 - br;
        let dg = p[1] as i32 - bgg;
        let db = p[2] as i32 - bb;
        dr * dr + dg * dg + db * db >= TEXT_THRESHOLD_SQ
    };

    let mut first_text_row: Option<i32> = None;
    let mut last_text_row: i32 = y0;
    let mut per_row_min: Vec<i32> = Vec::new();
    for py in y0..y1 {
        let mut row_min: Option<i32> = None;
        let mut run: i32 = 0;
        let mut row_has_text = false;
        for px in x0..x1 {
            if is_text(px, py) {
                row_has_text = true;
                run += 1;
            } else if run > 0 {
                row_min = Some(row_min.map_or(run, |m| m.min(run)));
                run = 0;
            }
        }
        if run > 0 {
            row_min = Some(row_min.map_or(run, |m| m.min(run)));
        }
        if row_has_text {
            if first_text_row.is_none() {
                first_text_row = Some(py);
            }
            last_text_row = py;
            if let Some(m) = row_min {
                per_row_min.push(m);
            }
        }
    }

    let Some(first) = first_text_row else {
        return DEFAULT_WEIGHT;
    };
    let glyph_h = (last_text_row - first + 1).max(1) as f32;
    if per_row_min.is_empty() || glyph_h < 4.0 {
        return DEFAULT_WEIGHT;
    }

    per_row_min.sort_unstable();
    let median_min = per_row_min[per_row_min.len() / 2] as f32;
    let ratio = median_min / glyph_h;

    if ratio < 0.11 {
        400
    } else if ratio < 0.17 {
        600
    } else {
        700
    }
}

/// Robust paragraph weight: estimate weight per member line and
/// take the median. Single-member paragraphs collapse to a single
/// `estimate_font_weight` call (no overhead beyond the original
/// path). Multi-member paragraphs that survived the height-
/// discontinuity split are still allowed to contain mixed weights
/// (e.g., a quoted bold word inside body text); the median picks
/// the dominant style without one outlier flipping the entire card
/// into the wrong bucket.
fn estimate_paragraph_font_weight(
    img: &image::DynamicImage,
    members: &[usize],
    rendered: &[OcrLine],
    bg: [u8; 3],
) -> u16 {
    if members.is_empty() {
        return 400;
    }
    if members.len() == 1 {
        return estimate_font_weight(img, rendered[members[0]].bbox, bg);
    }
    let mut weights: Vec<u16> = members
        .iter()
        .map(|&m| estimate_font_weight(img, rendered[m].bbox, bg))
        .collect();
    weights.sort_unstable();
    weights[weights.len() / 2]
}

/// Split a paragraph sub-group whenever adjacent member line
/// heights differ by more than the configured ratio. Members come
/// in reading order (top → bottom). The split keeps consecutive
/// similar-height runs together — that preserves the body
/// paragraph's wrap context while peeling off the bold heading the
/// column-continuity grouper accidentally swept in.
///
/// The threshold (1.6×) was chosen empirically against the iter5
/// fixture set: title→body transitions sit in the 1.8–2.4× range,
/// while body-internal variation (descender outliers, italic runs)
/// stays under 1.3×. Bumping below 1.5× starts shattering normal
/// paragraphs; bumping above 1.7× lets title+body slip through
/// again.
fn split_subgroup_by_height_discontinuity(
    members: &[usize],
    rendered: &[OcrLine],
) -> Vec<Vec<usize>> {
    if members.len() <= 1 {
        return vec![members.to_vec()];
    }
    const HEIGHT_RATIO_THRESHOLD: f32 = 1.6;
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for (i, &m) in members.iter().enumerate() {
        if i == 0 {
            current.push(m);
            continue;
        }
        let prev = members[i - 1];
        let prev_h = rendered[prev].bbox[3].max(1.0);
        let curr_h = rendered[m].bbox[3].max(1.0);
        let ratio = if curr_h >= prev_h {
            curr_h / prev_h
        } else {
            prev_h / curr_h
        };
        if ratio >= HEIGHT_RATIO_THRESHOLD {
            // Found a discontinuity — flush the accumulated run
            // and start fresh. The current member becomes the seed
            // of the next group.
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
        }
        current.push(m);
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Compute the inpaint / card region for a single OCR line.
/// Returns `[x, y, w, h]` in image pixel coordinates. Pads around
/// `source_bbox` to capture glyph descenders / ascenders and
/// trailing-character OCR misses, while refusing to overrun any
/// skipped (target-language) line on the same visual row. Same-row
/// non-skipped neighbours are allowed to overlap — both cards
/// inpaint with compatible background samples so the overlap is
/// benign, and the wider zone catches OCR fragment residues.
///
/// This is the single source of truth shared by the production
/// `compute_lens_cards` pipeline and the offline `lens_eval`
/// evaluator — drift between the two was the cause of the "eval
/// looks clean but production leaves source visible" regression.
pub fn compute_card_region(
    ocr_lines: &[OcrLine],
    skip_flags: &[bool],
    self_idx: usize,
    image_w: u32,
    image_h: u32,
) -> [f32; 4] {
    let source = ocr_lines[self_idx].bbox;
    let [sx, sy, sw, sh] = source;
    // Modest padding: enough for descender / ascender overshoot
    // (sh * 0.35) vertically and a handful of glyph widths
    // horizontally. Earlier experiments used sh * 2.5 horizontally
    // to sweep over OCR fragment misses, but that extended the
    // card far past the actual text and the inpaint patch showed
    // up as a visible rectangle overhanging unrelated image area.
    let pad_v = (sh * 0.35).clamp(2.0, 8.0);
    let pad_h = (sh * 0.6).clamp(4.0, 14.0);
    let center_y = sy + sh / 2.0;

    let mut left_boundary = 0.0_f32;
    let mut right_boundary = image_w as f32;
    for (i, other) in ocr_lines.iter().enumerate() {
        if i == self_idx {
            continue;
        }
        if !skip_flags.get(i).copied().unwrap_or(false) {
            continue;
        }
        let other_cy = other.bbox[1] + other.bbox[3] / 2.0;
        if (other_cy - center_y).abs() > sh * 0.5 {
            continue;
        }
        let other_l = other.bbox[0];
        let other_r = other.bbox[0] + other.bbox[2];
        if other_r <= sx {
            let mid = (other_r + sx) / 2.0;
            if mid > left_boundary {
                left_boundary = mid;
            }
        } else if other_l >= sx + sw {
            let mid = (other_l + sx + sw) / 2.0;
            if mid < right_boundary {
                right_boundary = mid;
            }
        }
    }

    let x0 = (sx - pad_h).max(left_boundary).max(0.0);
    let y0 = (sy - pad_v).max(0.0);
    let x1 = (sx + sw + pad_h).min(right_boundary).min(image_w as f32);
    let y1 = (sy + sh + pad_v).min(image_h as f32);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    [x0, y0, w, h]
}

/// Re-encode `png_bytes` as a 2×-upscaled (Lanczos3) PNG for OCR.
/// The frontend still works with the original-resolution image; we
/// divide bboxes by 2 after OCR returns. Returns the original bytes
/// on decode/encode failure so the pipeline degrades gracefully.
pub fn preprocess_png_for_ocr(png_bytes: &[u8], scale: u32) -> anyhow::Result<Vec<u8>> {
    let img = image::load_from_memory(png_bytes)?;
    let (w, h) = (img.width(), img.height());
    let upscaled = img.resize_exact(
        w.saturating_mul(scale),
        h.saturating_mul(scale),
        image::imageops::FilterType::Lanczos3,
    );
    let mut out = Vec::with_capacity(png_bytes.len() * (scale as usize).pow(2));
    upscaled.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

/// Demote OCR lines that `text_is_already_target` classified as
/// target-language when, on closer inspection, they are single-kanji
/// OCR misreads of Latin words sandwiched between English cards on
/// the same visual row. Leaving them as "skip" creates a boundary
/// that blocks downstream horizontal-padding from covering the
/// misread pixels; demoting lets the adjacent cards' inpainting
/// region span the whole row.
pub fn demote_isolated_single_kanji(ocr_lines: &[OcrLine], skip_flags: &mut [bool]) {
    for i in 0..ocr_lines.len() {
        if !skip_flags[i] {
            continue;
        }
        let text = ocr_lines[i].text.trim();
        let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        if chars.len() > 1 {
            continue;
        }
        let Some(&c) = chars.first() else {
            continue;
        };
        let u = c as u32;
        let is_kanji = (0x3400..=0x4DBF).contains(&u) || (0x4E00..=0x9FFF).contains(&u);
        let is_kana = (0x3040..=0x309F).contains(&u)
            || (0x30A0..=0x30FF).contains(&u)
            || (0xFF65..=0xFF9F).contains(&u);
        if !is_kanji || is_kana {
            continue;
        }
        let my = ocr_lines[i].bbox[1] + ocr_lines[i].bbox[3] / 2.0;
        let sh = ocr_lines[i].bbox[3];
        let mut sandwiched = false;
        for j in 0..ocr_lines.len() {
            if j == i || skip_flags[j] {
                continue;
            }
            let oy = ocr_lines[j].bbox[1] + ocr_lines[j].bbox[3] / 2.0;
            if (oy - my).abs() < sh * 0.8 {
                sandwiched = true;
                break;
            }
        }
        if sandwiched {
            skip_flags[i] = false;
        }
    }
}

/// Single entry point that converts raw OCR output into the
/// `(lines, skip_flags)` pair every downstream stage consumes. The
/// production `run_capture_flow` and the offline `lens_eval` binary
/// both go through this function so a line that the real Lens drops
/// can never sneak into the evaluator (and vice versa).
///
/// Pipeline:
///  1. Drop noise-looking lines entirely — decorative runs,
///     single-character fragments, Latin gibberish from ocrs misreads.
///     These never become tiling neighbours because they are not
///     real text.
///  2. Flag lines that already read as the target language so the
///     card pipeline preserves them verbatim (`skip = true`).
///  3. Demote single-kanji lines that are sandwiched between English
///     so the sandwich heuristic promotes them back to translatable.
///
/// Callers must have already un-scaled bboxes into capture-image
/// pixel space (both call sites do their own 2× undo) — this
/// function only reasons about text semantics, not geometry.
pub fn preprocess_ocr_lines(
    raw_lines: Vec<OcrLine>,
    target_language: &str,
) -> (Vec<OcrLine>, Vec<bool>) {
    let filtered: Vec<OcrLine> = raw_lines
        .into_iter()
        .filter(|l| !looks_like_ocr_noise(&l.text))
        .collect();
    let mut skip_flags: Vec<bool> = filtered
        .iter()
        .map(|l| text_is_already_target(&l.text, target_language))
        .collect();
    demote_isolated_single_kanji(&filtered, &mut skip_flags);
    (filtered, skip_flags)
}

/// Run the full lens-card pipeline (tile bbox + rim-gradient / fg /
/// font-weight sampling) against a vector of raw OCR lines.
///
/// Pure — no Tauri state, no I/O. The production watcher calls it
/// with the freshly captured frame; the offline `lens_eval` example
/// binary reuses it to generate evaluation overlays from `tests/input/`
/// images. Keeping the logic in one place means the evaluator can
/// never drift from what ships to the user.
///
/// * `ocr_lines` — raw lines from the OCR provider.
/// * `skip_flags` — one flag per line; `true` means "leave the source
///   visible, do not emit a card". Skipped lines are still used as
///   tiling neighbours so the surrounding cards do not overrun them.
/// * `image_decoded` — the captured PNG decoded as an image. `None`
///   disables colour / weight sampling (every card falls back to
///   mid-grey background and weight 400).
/// * `image_w` / `image_h` — pixel size of the capture, used to clamp
///   tiled rects at the frame boundary.
pub fn compute_lens_cards(
    ocr_lines: &[OcrLine],
    skip_flags: &[bool],
    image_decoded: Option<&image::DynamicImage>,
    image_w: u32,
    image_h: u32,
) -> Vec<LensOcrLine> {
    debug_assert_eq!(ocr_lines.len(), skip_flags.len());

    let initial_rendered: Vec<OcrLine> = ocr_lines
        .iter()
        .zip(skip_flags.iter())
        .filter_map(|(l, &skip)| if skip { None } else { Some(l.clone()) })
        .collect();
    if initial_rendered.is_empty() {
        return Vec::new();
    }

    // Two-step grouping informed by the document-analysis literature:
    //
    //  1. **Layout = noise filter + block bbox reference.** Block
    //     detection finds where TEXT lives versus background. We
    //     keep an OCR line only if it sits inside SOME detected
    //     block (drops stray digits on photos / ocrs hallucinations
    //     in margins) and we record the containing block per
    //     paragraph so the alignment detector has a reference
    //     rectangle to compare against.
    //  2. **Paragraph grouping = text-based rules.** `group_paragraphs`
    //     applies established heuristics (vertical gap ≤ 1.2× line
    //     height, horizontal column overlap, label-marker break)
    //     directly on OCR bboxes. RLSA's classic Cv = 1.5×
    //     line-spacing rule (Wong/Casey/Wahl 1982) assumes a
    //     blank line between paragraphs; modern compact UI / book
    //     layouts violate that, so we cannot rely on dilation
    //     kernels alone to separate paragraphs.
    let (rendered, blocks): (Vec<OcrLine>, Vec<crate::lens_layout::LayoutBlock>) =
        match image_decoded {
            Some(img) => {
                let line_height_hint = median_height_or_fallback(&initial_rendered);
                let detected =
                    crate::lens_layout::detect_text_blocks_for_lines(img, line_height_hint);
                if detected.is_empty() {
                    (initial_rendered, Vec::new())
                } else {
                    let inside: Vec<OcrLine> = initial_rendered
                        .iter()
                        .filter(|line| ocr_line_inside_any_block(line.bbox, &detected, 0.5))
                        .cloned()
                        .collect();
                    if inside.is_empty() {
                        (initial_rendered, detected)
                    } else {
                        (inside, detected)
                    }
                }
            }
            None => (initial_rendered, Vec::new()),
        };
    let mut paragraphs = group_paragraphs(&rendered);
    // Assign each paragraph the layout block that contains the
    // bulk of its members. Used downstream for source-alignment
    // detection (centred / right-aligned headings against a wide
    // background block).
    if !blocks.is_empty() {
        for paragraph in &mut paragraphs {
            paragraph.block_bbox = paragraph_containing_block(paragraph, &rendered, &blocks);
        }
    }

    let mut lens_lines: Vec<LensOcrLine> = Vec::with_capacity(paragraphs.len());
    for paragraph in paragraphs {
        // Split the paragraph into sub-groups at any skipped
        // (target-language) OCR line that vertically falls between
        // consecutive members. Without this split a mixed source
        // (English paragraph with embedded Japanese) would render
        // one card whose bbox spans across the Japanese line — the
        // inpaint step would then erase the Japanese that should
        // have been left visible. Each sub-group becomes an
        // independent card with its own bbox, text, and translation.
        let initial_sub_groups = split_paragraph_by_skipped_intrusions(
            &paragraph.members,
            &rendered,
            ocr_lines,
            skip_flags,
        );
        // Further split each sub-group at sharp font-size
        // discontinuities. A "title (bold, 28px) + body (regular,
        // 14px)" pair that `group_paragraphs` merged on column
        // continuity would otherwise produce ONE card whose single
        // estimated font-weight ends up bolding the body, and whose
        // single line-height grossly mis-sizes one of the two. The
        // split here is intentionally conservative (>1.6× height
        // ratio) so genuine paragraph-internal variation — italics,
        // small caps, the occasional ascender/descender outlier —
        // still flows through as a single translation unit.
        let mut sub_groups: Vec<Vec<usize>> = Vec::with_capacity(initial_sub_groups.len());
        for group in initial_sub_groups {
            sub_groups.extend(split_subgroup_by_height_discontinuity(&group, &rendered));
        }
        let has_single_subgroup = sub_groups.len() == 1;

        for members in sub_groups {
            if members.is_empty() {
                continue;
            }
            let mut min_x = f32::INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            let mut joined_parts: Vec<String> = Vec::with_capacity(members.len());
            for &m in &members {
                let bbox = rendered[m].bbox;
                min_x = min_x.min(bbox[0]);
                min_y = min_y.min(bbox[1]);
                max_x = max_x.max(bbox[0] + bbox[2]);
                max_y = max_y.max(bbox[1] + bbox[3]);
                joined_parts.push(rendered[m].text.clone());
            }
            if min_x >= max_x || min_y >= max_y {
                continue;
            }
            let union_bbox = [min_x, min_y, max_x - min_x, max_y - min_y];
            // `source_bbox` is always the OCR-text union: it must
            // be tight to the actual glyphs because everything
            // downstream (font-size estimation, inpaint area, the
            // bg-patch image dimensions) is keyed off it. The
            // layout block bbox stays separate (consumed by
            // `expand_card_for_alignment` only) — using it here
            // was a hold-over from the old paragraph-per-block
            // model where each paragraph covered the whole block,
            // and it ballooned `source_bbox` to nearly the full
            // image once the new `group_paragraphs`-based
            // splitting started producing multiple paragraphs per
            // block.
            let _ = has_single_subgroup;
            let source_bbox = union_bbox;

            let anchor_bbox = rendered[members[0]].bbox;
            let (bg, bg_gradient) = match image_decoded {
                Some(img) => sample_rim_edges(img, anchor_bbox),
                None => ([128, 128, 128], None),
            };
            let fg = match image_decoded {
                Some(img) => sample_text_color(img, anchor_bbox, bg),
                None => contrast_text(bg),
            };
            // Estimate weight per member and take the median. Using
            // only `members[0]` (the prior behaviour) was wrong when
            // the first OCR line happened to be a different weight
            // than the rest — typical for "label: value" rows where
            // the label is semibold but the value is regular, and
            // also after the height-discontinuity split above when
            // the residue of a misgrouped paragraph still contains
            // mixed weights. The median is robust to one or two
            // outlier rows without needing per-line CSS output.
            let font_weight = match image_decoded {
                Some(img) => estimate_paragraph_font_weight(img, &members, &rendered, bg),
                None => 400,
            };

            let inpaint_bbox =
                paragraph_card_region(source_bbox, ocr_lines, skip_flags, image_w, image_h);
            let bg_patch = match image_decoded {
                Some(img) => encode_inpaint_patch(img, inpaint_bbox, bg, fg),
                None => None,
            };

            let median_h = median_member_line_height(&members, &rendered);
            let font_size_px = median_h;
            let line_height = source_line_height_ratio(&members, &rendered);
            let source_line_count = members.len() as u32;
            let text_align = detect_text_align(&members, &rendered, paragraph.block_bbox);
            // Card render bbox = inpaint bbox extended toward the
            // layout block in the direction OPPOSITE the alignment.
            // Right-aligned text grows the card LEFTWARD so the
            // translation can be wider than the source while still
            // ending at the source's right edge; left-aligned grows
            // RIGHTWARD; centered grows on both sides. Without this
            // slack, CSS `text-align` had nothing to operate on
            // (card width == text width) and alignment was
            // visually a no-op. The inpaint patch stays anchored
            // to `inpaint_bbox` so neighbouring text never gets
            // erased.
            let card_region = expand_card_for_alignment(
                inpaint_bbox,
                text_align,
                paragraph.block_bbox,
                image_w,
                image_h,
            );

            lens_lines.push(LensOcrLine {
                line_index: lens_lines.len(),
                text: joined_parts.join("\n"),
                bbox: card_region,
                source_bbox,
                bg_color: bg,
                bg_gradient,
                fg_color: fg,
                font_weight,
                font_size_px,
                line_height,
                source_line_count,
                text_align,
                inpaint_bbox,
                bg_patch,
            });
        }
    }
    lens_lines
}

/// Does any skipped (target-native) OCR line's bbox overlap
/// `block`? Used to decide whether the layout-detected block bbox
/// can stand in for the sub-group's source rect — if target-
/// language text sits inside the block (same row, adjacent column),
/// using the block rect would cause the card's inpaint to erase it.
pub(crate) fn block_overlaps_skipped(
    block: [f32; 4],
    ocr_lines: &[OcrLine],
    skip_flags: &[bool],
) -> bool {
    let bx1 = block[0];
    let by1 = block[1];
    let bx2 = block[0] + block[2];
    let by2 = block[1] + block[3];
    for (i, &skip) in skip_flags.iter().enumerate() {
        if !skip {
            continue;
        }
        let sb = ocr_lines[i].bbox;
        let sx1 = sb[0];
        let sy1 = sb[1];
        let sx2 = sb[0] + sb[2];
        let sy2 = sb[1] + sb[3];
        // Any non-degenerate overlap counts.
        if sx2 > bx1 && sx1 < bx2 && sy2 > by1 && sy1 < by2 {
            return true;
        }
    }
    false
}

/// Split a paragraph's member list into contiguous sub-groups so
/// that no sub-group spans across a skipped (target-language) OCR
/// line. Members are first y-ordered; any skipped OCR bbox that
/// sits between two consecutive members AND horizontally overlaps
/// the gap-spanning rect of those members becomes a break point.
///
/// Returns one Vec per sub-group in reading order. Empty paragraphs
/// return an empty outer Vec; paragraphs with no skipped
/// intrusions return a single-element Vec with the original order.
pub(crate) fn split_paragraph_by_skipped_intrusions(
    members: &[usize],
    rendered: &[OcrLine],
    ocr_lines: &[OcrLine],
    skip_flags: &[bool],
) -> Vec<Vec<usize>> {
    if members.is_empty() {
        return Vec::new();
    }
    let mut ordered: Vec<usize> = members.to_vec();
    ordered.sort_by(|&a, &b| {
        rendered[a].bbox[1]
            .partial_cmp(&rendered[b].bbox[1])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut sub_groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    for &m in &ordered {
        if let Some(&prev) = current.last() {
            let prev_bot = rendered[prev].bbox[1] + rendered[prev].bbox[3];
            let curr_top = rendered[m].bbox[1];
            if curr_top > prev_bot {
                // Look for a skipped line that falls inside the gap
                // and horizontally overlaps the span covered by the
                // prev+curr pair. If found, split here so the next
                // sub-group starts with `m`.
                let span_l = rendered[prev].bbox[0].min(rendered[m].bbox[0]);
                let span_r = (rendered[prev].bbox[0] + rendered[prev].bbox[2])
                    .max(rendered[m].bbox[0] + rendered[m].bbox[2]);
                let mut split = false;
                for (i, &skip) in skip_flags.iter().enumerate() {
                    if !skip {
                        continue;
                    }
                    let s_top = ocr_lines[i].bbox[1];
                    let s_bot = s_top + ocr_lines[i].bbox[3];
                    let s_l = ocr_lines[i].bbox[0];
                    let s_r = s_l + ocr_lines[i].bbox[2];
                    // Inside the vertical gap?
                    if s_top < prev_bot || s_bot > curr_top {
                        continue;
                    }
                    // Horizontally overlaps the pair's span?
                    if s_r <= span_l || s_l >= span_r {
                        continue;
                    }
                    split = true;
                    break;
                }
                if split {
                    sub_groups.push(std::mem::take(&mut current));
                }
            }
        }
        current.push(m);
    }
    if !current.is_empty() {
        sub_groups.push(current);
    }
    sub_groups
}

/// Median bbox height across the OCR members of a paragraph.
/// Picking the median (rather than mean) keeps a single mis-detected
/// outlier line from inflating the font size and overflowing the
/// card. Returns `1.0` as a safe default for empty member lists.
/// Median of all OCR line heights, used to size the layout
/// detector's vertical kernel. Falls back to a generic body-text
/// figure when there are no lines yet (very early in a flow or on
/// a fully-noise capture).
pub(crate) fn median_height_or_fallback(rendered: &[OcrLine]) -> f32 {
    if rendered.is_empty() {
        return 12.0;
    }
    let mut heights: Vec<f32> = rendered.iter().map(|l| l.bbox[3]).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    heights[heights.len() / 2].max(4.0)
}

/// True when at least `min_coverage` (0..1) of `bbox`'s area falls
/// inside ANY of the layout `blocks`. Used as the noise filter for
/// OCR output: a line whose bbox does not overlap any detected
/// text block is almost always a hallucination on a photo or
/// decorative background — drop it before paragraph grouping.
pub(crate) fn ocr_line_inside_any_block(
    bbox: [f32; 4],
    blocks: &[crate::lens_layout::LayoutBlock],
    min_coverage: f32,
) -> bool {
    blocks
        .iter()
        .any(|b| coverage(bbox, b.bbox) >= min_coverage)
}

pub(crate) fn median_member_line_height(members: &[usize], rendered: &[OcrLine]) -> f32 {
    if members.is_empty() {
        return 1.0;
    }
    let mut heights: Vec<f32> = members.iter().map(|&m| rendered[m].bbox[3]).collect();
    heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    heights[heights.len() / 2].max(1.0)
}

/// Extend the card's render rectangle horizontally toward the
/// layout-block edge OPPOSITE the detected text alignment. The
/// resulting bbox is wider than the source text, giving CSS
/// `text-align` meaningful slack to position the translation:
///
/// * `Left`  — keep the left edge anchored at the source's left,
///             grow the right edge to the block's right edge.
/// * `Right` — keep the right edge anchored at the source's
///             right, grow the left edge to the block's left edge.
/// * `Center` — grow on both sides to the block's full extent.
///
/// Vertical extent is unchanged: the inpaint patch height already
/// matches the source glyph row, and growing vertically would
/// risk overlapping rows above or below.
///
/// When no block info is available, returns the inpaint bbox
/// unchanged — there is no reference rectangle to grow into.
pub(crate) fn expand_card_for_alignment(
    inpaint_bbox: [f32; 4],
    align: TextAlign,
    block: Option<[f32; 4]>,
    image_w: u32,
    image_h: u32,
) -> [f32; 4] {
    let Some(block) = block else {
        return inpaint_bbox;
    };
    let inpaint_left = inpaint_bbox[0];
    let inpaint_right = inpaint_bbox[0] + inpaint_bbox[2];
    let inpaint_w = inpaint_bbox[2].max(0.0);
    let block_left = block[0].max(0.0);
    let block_right = (block[0] + block[2]).min(image_w as f32);
    // Bound the slack: extending the card all the way to the block
    // edge is fine for short headings (one OCR line, plenty of empty
    // column to grow into) but ruins long body paragraphs where the
    // source already nearly fills its column — the translation then
    // wraps differently from the source and spills past the original
    // glyph row. We cap horizontal growth at `MAX_SLACK_RATIO` of the
    // source width: enough room for typical EN→JA expansion (~25%
    // char-count delta on body text) without overrunning the source
    // layout. The block edge stays the outer cap so we never grow
    // beyond what the block could legitimately hold.
    const MAX_SLACK_RATIO: f32 = 0.25;
    let slack = inpaint_w * MAX_SLACK_RATIO;
    let (new_left, new_right) = match align {
        TextAlign::Left => (
            inpaint_left,
            (inpaint_right + slack).min(block_right.max(inpaint_right)),
        ),
        TextAlign::Right => (
            (inpaint_left - slack).max(block_left.min(inpaint_left)),
            inpaint_right,
        ),
        TextAlign::Center => {
            let half = slack * 0.5;
            (
                (inpaint_left - half).max(block_left.min(inpaint_left)),
                (inpaint_right + half).min(block_right.max(inpaint_right)),
            )
        }
    };
    let y = inpaint_bbox[1].max(0.0);
    let h = inpaint_bbox[3].min(image_h as f32 - y);
    let w = (new_right - new_left).max(0.0);
    [new_left, y, w, h]
}

/// Find the layout block that contains the most of the
/// paragraph's members (by total intersection area). Returns
/// `None` when the paragraph does not meaningfully overlap any
/// block — typical for paragraphs assembled from the OCR-only
/// fallback path. Used by the alignment detector so single-line
/// cards have a reference rectangle wider than the text itself.
pub(crate) fn paragraph_containing_block(
    paragraph: &Paragraph,
    rendered: &[OcrLine],
    blocks: &[crate::lens_layout::LayoutBlock],
) -> Option<[f32; 4]> {
    if paragraph.members.is_empty() || blocks.is_empty() {
        return None;
    }
    let mut best: Option<(usize, f32)> = None;
    for (bi, block) in blocks.iter().enumerate() {
        let mut total = 0.0_f32;
        for &m in &paragraph.members {
            total += rectangle_intersection_area(rendered[m].bbox, block.bbox);
        }
        if total > 0.0 && best.map(|(_, s)| total > s).unwrap_or(true) {
            best = Some((bi, total));
        }
    }
    best.map(|(bi, _)| blocks[bi].bbox)
}

fn rectangle_intersection_area(a: [f32; 4], b: [f32; 4]) -> f32 {
    let ax1 = a[0];
    let ay1 = a[1];
    let ax2 = a[0] + a[2];
    let ay2 = a[1] + a[3];
    let bx1 = b[0];
    let by1 = b[1];
    let bx2 = b[0] + b[2];
    let by2 = b[1] + b[3];
    let ix1 = ax1.max(bx1);
    let iy1 = ay1.max(by1);
    let ix2 = ax2.min(bx2);
    let iy2 = ay2.min(by2);
    if ix2 <= ix1 || iy2 <= iy1 {
        0.0
    } else {
        (ix2 - ix1) * (iy2 - iy1)
    }
}

/// Detect the horizontal alignment of the source text relative
/// to its containing layout block (or its own column when no
/// block info is available).
///
/// Two regimes:
/// 1. **Multi-line members**: variance of left and right edges
///    distinguishes the three alignments. `text-align: left`
///    fixes the LEFT edge, the right edge varies as lines wrap;
///    `text-align: right` does the opposite; `text-align: center`
///    varies on both edges (each line centres around the column
///    midpoint). Wrapped justified text fixes both edges — we
///    map that to `Left` since CSS `justify` is rare and looks
///    fine as left.
/// 2. **Single-line members**: with no spread to measure, fall
///    back on the layout block bbox (when present). The text's
///    left/right gaps to the block's edges decide alignment:
///    text touching left edge → `Left`, touching right →
///    `Right`, gaps roughly equal → `Center`, otherwise default
///    to `Left` (the safe assumption — no false centering).
pub(crate) fn detect_text_align(
    members: &[usize],
    rendered: &[OcrLine],
    block_bbox: Option<[f32; 4]>,
) -> TextAlign {
    if members.is_empty() {
        return TextAlign::Left;
    }
    let glyph_h = rendered[members[0]].bbox[3].max(1.0);
    // Tolerance for "edge alignment" in px — about a glyph
    // height. OCR bbox edges drift by sub-pixel amounts even
    // for visually flush text, so an exact match is too strict.
    let tol = glyph_h * 0.6;

    if members.len() >= 2 {
        let lefts: Vec<f32> = members.iter().map(|&i| rendered[i].bbox[0]).collect();
        let rights: Vec<f32> = members
            .iter()
            .map(|&i| rendered[i].bbox[0] + rendered[i].bbox[2])
            .collect();
        let left_spread = max_minus_min(&lefts);
        let right_spread = max_minus_min(&rights);
        let left_aligned = left_spread <= tol;
        let right_aligned = right_spread <= tol;
        match (left_aligned, right_aligned) {
            // Both edges fixed → either justified body or a tight
            // block where every line happens to be the same width.
            // Render as left — looks identical for monospace and
            // matches the most common case.
            (true, true) => return TextAlign::Left,
            (true, false) => return TextAlign::Left,
            (false, true) => return TextAlign::Right,
            // Neither edge is fixed → centred or random. Centre is
            // the sensible default for the disorganised case.
            (false, false) => return TextAlign::Center,
        }
    }

    // Single-line: compare to the layout block. Without one, we
    // cannot tell — leave at the default (Left).
    let Some(block) = block_bbox else {
        return TextAlign::Left;
    };
    let text_bbox = rendered[members[0]].bbox;
    let block_w = block[2].max(1.0);
    let left_gap = (text_bbox[0] - block[0]).max(0.0);
    let right_gap = (block[0] + block[2] - (text_bbox[0] + text_bbox[2])).max(0.0);
    // Edge-touch threshold: 5% of block width. Generous enough
    // to absorb anti-alias / OCR rounding without falsely
    // flagging clearly-indented text.
    let edge_tol = (block_w * 0.05).max(tol);
    let left_touch = left_gap <= edge_tol;
    let right_touch = right_gap <= edge_tol;
    match (left_touch, right_touch) {
        // Touching both edges → text fills the block; default
        // left is safe.
        (true, true) => TextAlign::Left,
        (true, false) => TextAlign::Left,
        (false, true) => TextAlign::Right,
        // Touching neither — but if the gaps are approximately
        // equal, the text is centred in the block. Otherwise
        // default left rather than guess.
        (false, false) => {
            let gap_diff = (left_gap - right_gap).abs();
            if gap_diff <= edge_tol {
                TextAlign::Center
            } else {
                TextAlign::Left
            }
        }
    }
}

fn max_minus_min(values: &[f32]) -> f32 {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in values {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    if min.is_finite() && max.is_finite() {
        max - min
    } else {
        0.0
    }
}

/// Estimate the source text's CSS `line-height` ratio (baseline-to-
/// baseline distance ÷ glyph height) from the vertical spacing of
/// the paragraph's OCR members. Real body text usually lands between
/// 1.3 and 1.6; our previous hard-coded 1.15 made wrapped
/// translations feel cramped relative to the source. Members are
/// y-sorted first so the gaps make sense when OCR returned them in
/// reading order but with occasional out-of-sequence rows.
pub(crate) fn source_line_height_ratio(members: &[usize], rendered: &[OcrLine]) -> f32 {
    // Single-member or empty paragraphs have no gap to measure —
    // return a generic web-safe default tight enough to not balloon
    // a one-liner's card height.
    if members.len() < 2 {
        return 1.25;
    }
    let mut sorted: Vec<usize> = members.to_vec();
    sorted.sort_by(|&a, &b| {
        rendered[a].bbox[1]
            .partial_cmp(&rendered[b].bbox[1])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut ratios: Vec<f32> = Vec::with_capacity(sorted.len() - 1);
    for pair in sorted.windows(2) {
        let a = rendered[pair[0]].bbox;
        let b = rendered[pair[1]].bbox;
        let top_to_top = (b[1] - a[1]).max(1.0);
        let glyph_h = a[3].max(1.0);
        let ratio = top_to_top / glyph_h;
        // Clamp to sane bounds — occasional OCR bbox noise can push
        // a pair way off (e.g. two rows overlapping due to a misread
        // diacritic line). 1.0 floor prevents the CSS line-height
        // collapsing below the glyph itself.
        ratios.push(ratio.clamp(1.0, 2.2));
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ratios[ratios.len() / 2]
}

/// Paragraph-aware variant of `compute_card_region`. Pads the block
/// bbox and clamps against EVERY OTHER OCR line (skipped or not)
/// that overlaps the paragraph along the orthogonal axis. The
/// distinction matters because the card's `pad_v` extension and
/// the inpaint that follows can erase pixels of any neighbour —
/// not only target-native text — when the next paragraph or a
/// table-row label sits within `pad_v` pixels of the source.
///
/// Skipped (target-native) lines fed in via `skip_flags` are
/// excluded from the paragraph itself but kept in `ocr_lines`
/// precisely so this clamp can see them. Non-skipped lines
/// belonging to OTHER paragraphs also need to be respected;
/// previously they were ignored, so two adjacent English
/// paragraphs would each pad into the other's last/first line and
/// the inpaint would clip a row of glyphs from both.
///
/// The function NEVER clamps against the paragraph's own member
/// lines: those sit inside `[sy, sy + sh]` and would shrink the
/// region to zero if treated as outside neighbours. We detect
/// own-membership cheaply by checking whether the candidate's
/// centre lies inside the source rect on both axes.
fn paragraph_card_region(
    source: [f32; 4],
    ocr_lines: &[OcrLine],
    _skip_flags: &[bool],
    image_w: u32,
    image_h: u32,
) -> [f32; 4] {
    let [sx, sy, sw, sh] = source;
    let pad_v = 4.0_f32;
    let pad_h = (sh * 0.05).clamp(6.0, 16.0);

    let mut left_boundary = 0.0_f32;
    let mut right_boundary = image_w as f32;
    let mut top_boundary = 0.0_f32;
    let mut bot_boundary = image_h as f32;
    for other in ocr_lines.iter() {
        let other_top = other.bbox[1];
        let other_bottom = other.bbox[1] + other.bbox[3];
        let other_l = other.bbox[0];
        let other_r = other.bbox[0] + other.bbox[2];
        // Skip own-paragraph members: their centre falls inside the
        // source rect, and clamping against them would collapse the
        // region. The half-pixel slack absorbs OCR rounding so a
        // boundary-touching line on the rim still counts as a
        // neighbour.
        let cx = (other_l + other_r) / 2.0;
        let cy = (other_top + other_bottom) / 2.0;
        let inside_self =
            cx > sx + 0.5 && cx < sx + sw - 0.5 && cy > sy + 0.5 && cy < sy + sh - 0.5;
        if inside_self {
            continue;
        }
        let vertical_overlap = other_bottom > sy && other_top < sy + sh;
        let horizontal_overlap = other_r > sx && other_l < sx + sw;
        if vertical_overlap {
            if other_r <= sx {
                let mid = (other_r + sx) / 2.0;
                if mid > left_boundary {
                    left_boundary = mid;
                }
            } else if other_l >= sx + sw {
                let mid = (other_l + sx + sw) / 2.0;
                if mid < right_boundary {
                    right_boundary = mid;
                }
            }
        }
        if horizontal_overlap {
            if other_bottom <= sy {
                let mid = (other_bottom + sy) / 2.0;
                if mid > top_boundary {
                    top_boundary = mid;
                }
            } else if other_top >= sy + sh {
                let mid = (other_top + sy + sh) / 2.0;
                if mid < bot_boundary {
                    bot_boundary = mid;
                }
            }
        }
    }

    let x0 = (sx - pad_h).max(left_boundary).max(0.0);
    let y0 = (sy - pad_v).max(top_boundary).max(0.0);
    let x1 = (sx + sw + pad_h).min(right_boundary).min(image_w as f32);
    let y1 = (sy + sh + pad_v).min(bot_boundary).min(image_h as f32);
    let w = (x1 - x0).max(0.0);
    let h = (y1 - y0).max(0.0);
    [x0, y0, w, h]
}

/// Median bbox height across the OCR members of a paragraph. Picking
/// the median (rather than mean) means one mis-detected outlier with
/// Inpaint `bbox` from `img`, PNG-encode the result, and base64
/// encode the bytes so the payload round-trips cleanly through
/// Tauri's JSON IPC. Returns `None` on any failure — the frontend
/// then falls back to the flat / gradient fill.
fn encode_inpaint_patch(
    img: &image::DynamicImage,
    bbox: [f32; 4],
    bg: [u8; 3],
    fg: [u8; 3],
) -> Option<String> {
    let rgb = img.to_rgb8();
    let patch = lens_inpaint::inpaint_text_in_bbox(&rgb, bbox, bg, fg);
    if patch.width() < 1 || patch.height() < 1 {
        return None;
    }
    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(patch)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(&png))
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
/// Top-50 English letter digrams (Mayzner corpus, rounded).
/// Cover roughly 50% of all consecutive letter pairs in natural
/// English prose, so real text scores ≥ 0.20 here in almost every
/// case; OCR gibberish extracted from CJK source usually scores
/// under 0.10 because it combines letters that never adjacent in
/// English (e.g. `bz`, `dh`, `zk`).
const COMMON_ENGLISH_DIGRAMS: &[[u8; 2]] = &[
    *b"th", *b"he", *b"in", *b"er", *b"an", *b"re", *b"nd", *b"at", *b"on", *b"nt", *b"ha", *b"es",
    *b"st", *b"en", *b"ed", *b"to", *b"it", *b"ou", *b"ea", *b"hi", *b"is", *b"or", *b"ti", *b"as",
    *b"te", *b"et", *b"ng", *b"of", *b"al", *b"de", *b"se", *b"le", *b"sa", *b"si", *b"ar", *b"ve",
    *b"ra", *b"ld", *b"ur", *b"me", *b"ne", *b"ce", *b"ic", *b"co", *b"ma", *b"om", *b"ll", *b"li",
    *b"ho", *b"we",
];

/// Very short stopword list — enough to rescue "the", "and", "of"
/// patterns and any text that has real English in it. Includes
/// common UI verbs so "save", "delete", "cancel" etc. aren't
/// wrongly flagged as gibberish.
const COMMON_ENGLISH_WORDS: &[&str] = &[
    "the", "and", "of", "to", "in", "is", "it", "for", "on", "with", "this", "that", "you", "he",
    "she", "we", "they", "be", "are", "was", "were", "has", "have", "not", "but", "from", "or",
    "an", "as", "at", "by", "if", "so", "do", "did", "done", "will", "can", "could", "would",
    "should", "may", "might", "must", "all", "any", "some", "no", "yes", "one", "two", "new",
    "now", "how", "who", "what", "when", "where", "why", "open", "close", "save", "load", "edit",
    "delete", "cancel", "ok", "yes", "no", "help", "about", "file", "settings", "menu", "hello",
    "world", "please", "thank", "thanks", "name", "user", "time",
];

/// Fraction of consecutive alphabetic pairs that match one of the
/// common English digrams. Returns 0.0 when fewer than 3 pairs can
/// be formed from the text — too short to judge.
fn english_digram_density(text: &str) -> f32 {
    let bytes = text.as_bytes();
    let mut matches: u32 = 0;
    let mut total: u32 = 0;
    for win in bytes.windows(2) {
        if win[0].is_ascii_alphabetic() && win[1].is_ascii_alphabetic() {
            total += 1;
            let pair = [win[0].to_ascii_lowercase(), win[1].to_ascii_lowercase()];
            if COMMON_ENGLISH_DIGRAMS.iter().any(|p| *p == pair) {
                matches += 1;
            }
        }
    }
    if total < 3 {
        return 0.0;
    }
    matches as f32 / total as f32
}

/// Does `text` contain at least one stopword from
/// [`COMMON_ENGLISH_WORDS`]? Case-insensitive, splits on any
/// non-letter boundary so "the_button" still matches.
fn has_common_english_word(text: &str) -> bool {
    for raw in text.split(|c: char| !c.is_ascii_alphabetic()) {
        if raw.is_empty() {
            continue;
        }
        let lower = raw.to_ascii_lowercase();
        if COMMON_ENGLISH_WORDS.iter().any(|w| *w == lower.as_str()) {
            return true;
        }
    }
    false
}

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
    let non_letter = chars
        .iter()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace())
        .count();
    if total > 0 && (non_letter as f32 / total as f32) > 0.6 {
        return true;
    }
    // Long runs of punctuation/symbols (>= 4 consecutive) are the
    // fingerprint of ocrs misreading UI chrome: rules, separators,
    // tab characters rendered as `------`, `:::::`, `||||`, etc.
    // A length-4 window is permissive enough to keep ellipses
    // (`...`) and common emoticons while rejecting decorative runs.
    if chars
        .windows(4)
        .any(|w| w.iter().all(|c| !c.is_alphanumeric() && !c.is_whitespace()))
    {
        return true;
    }
    // Latin-script gibberish filter. ocrs' failure mode on
    // Japanese / Chinese source is a Latin-looking alphanumeric
    // string that passes surface checks but fails three linguistic
    // signals simultaneously. Each signal alone is too brittle
    // (false positives on genuinely short English), but ALL three
    // failing at once is a strong gibberish signal.
    let latin_letters: Vec<char> = chars
        .iter()
        .copied()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    let latin_dominant =
        latin_letters.len() >= 4 && (latin_letters.len() as f32 / total as f32) > 0.55;
    if latin_dominant {
        // Signal 1: vowel ratio far from English's ~38%. Real
        // English sits in [0.25, 0.60]; outside that range means
        // either consonant soup ("bcdfgz") or vowel spam ("aeioo").
        let vowels = latin_letters
            .iter()
            .filter(|c| matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y'))
            .count();
        let vowel_ratio = vowels as f32 / latin_letters.len() as f32;
        let vowels_off = vowel_ratio < 0.20 || vowel_ratio > 0.70;
        // Signal 2: English digram density is below the gibberish
        // threshold. Natural prose scores > 0.20; Japanese-source
        // gibberish regularly scores under 0.10.
        let digrams_off = english_digram_density(trimmed) < 0.12;
        // Signal 3: no stopword from the common list is present.
        // `the` / `and` / `of` / simple UI verbs — the floor any
        // real English sentence clears.
        let no_english_word = !has_common_english_word(trimmed);

        // Reject when *every* linguistic indicator fails. This
        // keeps short proper names and stopword-less technical
        // labels (e.g. "MoonLens") intact while catching the
        // "aBcDe oKo" style garbage Japanese-through-ocrs produces.
        if vowels_off && digrams_off && no_english_word {
            return true;
        }
        // Even when digrams say "could be English", the extreme
        // vowel imbalance ("bcdfgzhk", "aeeeee") is categorical
        // gibberish — keep the original strong filter for it.
        if vowel_ratio < 0.15 && no_english_word {
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
    // Full character view (before the `is_alphabetic` filter) because
    // several Japanese signals are punctuation / compat forms that
    // don't pass the letter filter.
    let all_chars: Vec<char> = text.chars().collect();

    match target.as_str() {
        "ja" | "ja-jp" | "jpn" | "japanese" => {
            // Count Japanese-signal characters anywhere in Unicode:
            //   hiragana / katakana / halfwidth katakana / CJK
            //   symbols-and-punct / kanji.
            let jp_chars = all_chars
                .iter()
                .filter(|c| {
                    let u = **c as u32;
                    (0x3040..=0x309F).contains(&u)       // Hiragana
                        || (0x30A0..=0x30FF).contains(&u) // Katakana
                        || (0xFF65..=0xFF9F).contains(&u) // Halfwidth Katakana
                        || (0x3000..=0x303F).contains(&u) // CJK Symbols + Punct
                        || (0x3400..=0x4DBF).contains(&u) // CJK Ext-A
                        || (0x4E00..=0x9FFF).contains(&u) // CJK Unified
                })
                .count();
            let latin_letters = all_chars.iter().filter(|c| c.is_ascii_alphabetic()).count();
            // No Japanese at all → definitely needs translation.
            if jp_chars == 0 {
                return false;
            }
            // Pure Japanese (no Latin letters) → already target.
            // Covers single-kanji UI labels (`戻`, `次`), kana
            // strings, and Japanese-punctuation-only sequences
            // (`「」`).
            if latin_letters == 0 {
                return true;
            }
            // Mixed input: only treat as already-target when
            // Japanese dominates (≥ 70% of meaningful chars).
            // Under that threshold the Latin portion is substantive
            // enough that the user expects it translated — the old
            // "any kana → skip" rule dropped English translations
            // whenever an English line was paragraph-merged with a
            // Japanese line.
            let total = jp_chars + latin_letters;
            (jp_chars as f32) / (total as f32) >= 0.70
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
/// only exists so the LLM receives the whole sentence at once and can
/// resolve cross-line anaphora, carry register, etc.
#[derive(Debug, Clone)]
pub struct Paragraph {
    /// Ordered indices into the OcrLine Vec this paragraph covers.
    /// `members.len() == 1` is the default / single-line case.
    pub members: Vec<usize>,
    /// Member line texts joined with `\n`. Sent to the LLM verbatim;
    /// the system prompt instructs it to preserve line breaks, which
    /// lets us split the output back to each member card.
    pub text: String,
    /// Optional layout-block bbox this paragraph is pinned to. When
    /// set, downstream card-placement ignores the OCR union rect
    /// and uses this bbox — the block is the authoritative paragraph
    /// extent because it was derived geometrically, free of any
    /// OCR-bbox drift. `None` for paragraphs that came from the
    /// text-only fallback grouping.
    pub block_bbox: Option<[f32; 4]>,
}

/// Assign every surviving OCR line to the layout block it sits
/// inside, group by block, and return one [`Paragraph`] per block
/// that collected any text. Lines whose bbox is less than
/// `min_coverage` inside every block are dropped as noise (OCR
/// hallucination on blank canvas).
///
/// Coverage is measured as `area(intersection) / area(ocr_bbox)` —
/// IoU would understate the match because a block bbox is much
/// larger than a single OCR line. A line 80% of whose area lives
/// inside a block is a confident match even if IoU is small.
pub fn group_by_layout_blocks(
    lines: &[OcrLine],
    blocks: &[crate::lens_layout::LayoutBlock],
    min_coverage: f32,
) -> Vec<Paragraph> {
    if lines.is_empty() || blocks.is_empty() {
        return Vec::new();
    }
    // Index into `blocks` or `None` for each OCR line.
    let mut assignment: Vec<Option<usize>> = Vec::with_capacity(lines.len());
    for line in lines {
        let mut best: Option<(usize, f32)> = None;
        for (bi, block) in blocks.iter().enumerate() {
            let cov = coverage(line.bbox, block.bbox);
            if cov >= min_coverage && best.map(|(_, c)| cov > c).unwrap_or(true) {
                best = Some((bi, cov));
            }
        }
        assignment.push(best.map(|(bi, _)| bi));
    }

    // Group line indices by their assigned block, preserve reading
    // order (by y then x) so the joined text reads naturally.
    let mut groups: Vec<(usize, Vec<usize>)> = (0..blocks.len()).map(|i| (i, Vec::new())).collect();
    for (li, &bi) in assignment.iter().enumerate() {
        if let Some(b) = bi {
            groups[b].1.push(li);
        }
    }
    let mut paragraphs = Vec::new();
    for (bi, mut members) in groups {
        if members.is_empty() {
            continue;
        }
        members.sort_by(|&a, &b| {
            let ay = lines[a].bbox[1];
            let by = lines[b].bbox[1];
            match ay.partial_cmp(&by) {
                Some(std::cmp::Ordering::Equal) | None => {
                    let ax = lines[a].bbox[0];
                    let bx = lines[b].bbox[0];
                    ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
                }
                Some(ord) => ord,
            }
        });
        let text = members
            .iter()
            .map(|&m| lines[m].text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        paragraphs.push(Paragraph {
            members,
            text,
            block_bbox: Some(blocks[bi].bbox),
        });
    }
    paragraphs
}

/// Fraction of `a`'s area that falls inside `b`. Zero when disjoint
/// or when `a` is degenerate. Used by [`group_by_layout_blocks`] to
/// test whether an OCR line is "inside" a layout block even when
/// the block is vastly larger (small IoU but large coverage).
pub(crate) fn coverage(a: [f32; 4], b: [f32; 4]) -> f32 {
    let ax1 = a[0];
    let ay1 = a[1];
    let ax2 = a[0] + a[2];
    let ay2 = a[1] + a[3];
    let bx1 = b[0];
    let by1 = b[1];
    let bx2 = b[0] + b[2];
    let by2 = b[1] + b[3];
    let ix1 = ax1.max(bx1);
    let iy1 = ay1.max(by1);
    let ix2 = ax2.min(bx2);
    let iy2 = ay2.min(by2);
    if ix2 <= ix1 || iy2 <= iy1 {
        return 0.0;
    }
    let inter = (ix2 - ix1) * (iy2 - iy1);
    let area_a = (a[2] * a[3]).max(0.0);
    if area_a <= 0.0 {
        0.0
    } else {
        inter / area_a
    }
}

/// Group OCR lines into paragraphs. Two consecutive (sorted-by-y)
/// lines share a paragraph when all of these hold:
///   - vertical gap ≤ 1.2 × min line height (covers typical 1.5-2×
///     line-height body text — `0.5 ×` was too tight and split
///     normally-spaced paragraphs into per-line translations, which
///     loses cross-line context like "blue\ncheese")
///   - horizontal range overlap ≥ 60% of min width (same column)
///   - upper line does NOT end with `.!?。！？` (no hard break)
///   - both lines have ≥ 6 characters (avoid grouping UI labels)
pub fn group_paragraphs(lines: &[OcrLine]) -> Vec<Paragraph> {
    if lines.is_empty() {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..lines.len()).collect();
    order.sort_by(|&a, &b| {
        let ay = lines[a].bbox[1];
        let by = lines[b].bbox[1];
        ay.partial_cmp(&by)
            .unwrap_or(std::cmp::Ordering::Equal)
            // Stable left-to-right ordering for same-row items so
            // reading flow stays intact when OCR splits a visual
            // line into multiple cards (common when a misread kanji
            // sits between English words).
            .then_with(|| {
                lines[a].bbox[0]
                    .partial_cmp(&lines[b].bbox[0])
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    // Paragraph boundary detection. The dominant signal is `:`
    // (and full-width `：`) — the canonical end-of-LABEL marker
    // in tabular UI ("Brand :", "Case Material :"). Without
    // splitting on them, every product-spec row collapsed into
    // one mega-card whose translation buried the table's column
    // structure (regression on tests/input/04ea5e..._original.png).
    //
    // Sentence-end punctuation (`.`, `!`, `?`, `。`, `！`, `？`)
    // is ambiguous on its own — a body paragraph ending a
    // sentence mid-flow looks identical to a tabular label whose
    // colon WinRT OCR misread as a period. We disambiguate by
    // length: a SHORT line ending in `.!?` is almost always a
    // label or single-sentence UI item, while a LONG one is a
    // wrapped body sentence that should keep merging with the
    // next line. The 30-char floor was picked from the test
    // corpus (every Rainbow Passage line clears 50+ chars; every
    // 04ea spec row is under 25).
    const SHORT_LABEL_LIMIT: usize = 30;
    let label_terminator =
        |c: char| -> bool { matches!(c, ':' | '：' | '.' | '。' | '!' | '?' | '！' | '？') };
    let ends_paragraph = |text: &str| -> bool {
        let trimmed = text.trim_end();
        let last = match trimmed.chars().last() {
            Some(c) => c,
            None => return false,
        };
        // Always treat a colon ending as a paragraph break — the
        // OCR can rarely confuse it for anything else.
        if matches!(last, ':' | '：') {
            return true;
        }
        // For period-ish endings, only break when the line is
        // short enough to plausibly be a label (or OCR-misread
        // label). Long body sentences keep merging.
        if matches!(last, '.' | '。' | '!' | '?' | '！' | '？')
            && trimmed.chars().count() <= SHORT_LABEL_LIMIT
        {
            return true;
        }
        false
    };
    let curr_is_label = |text: &str| -> bool {
        let trimmed = text.trim_end();
        let last = match trimmed.chars().last() {
            Some(c) => c,
            None => return false,
        };
        label_terminator(last) && trimmed.chars().count() <= SHORT_LABEL_LIMIT
    };

    // Group-level bounds so merge decisions consider the UNION of
    // every member's bbox rather than just the last one added.
    // Prevents the common failure where a paragraph ends up split
    // across columns (line N covers left half, line N+1 covers
    // right half, last-member-only overlap check rejects N+1).
    struct PendingGroup {
        members: Vec<usize>,
        min_x: f32,
        max_x: f32,
        max_bottom: f32,
        last_substantial: Option<usize>,
    }
    impl PendingGroup {
        fn new() -> Self {
            Self {
                members: Vec::new(),
                min_x: f32::INFINITY,
                max_x: f32::NEG_INFINITY,
                max_bottom: f32::NEG_INFINITY,
                last_substantial: None,
            }
        }
        fn push(&mut self, idx: usize, bbox: [f32; 4], is_substantial: bool) {
            self.members.push(idx);
            self.min_x = self.min_x.min(bbox[0]);
            self.max_x = self.max_x.max(bbox[0] + bbox[2]);
            self.max_bottom = self.max_bottom.max(bbox[1] + bbox[3]);
            if is_substantial {
                self.last_substantial = Some(idx);
            }
        }
    }

    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current = PendingGroup::new();
    for &idx in &order {
        let curr = lines[idx].bbox;
        let curr_h = curr[3].max(1.0);
        let curr_len = lines[idx].text.chars().count();
        let is_substantial = curr_len >= 6;

        if current.members.is_empty() {
            current.push(idx, curr, is_substantial);
            continue;
        }

        // Same-row join: when curr's vertical centre sits within
        // half a line-height of current's bottom AND it is
        // horizontally close enough to be a continuation of the
        // visual line, merge it unconditionally (OCR split an
        // English line at a misread glyph). The horizontal gap
        // check is essential — without it a far-right OCR
        // fragment ("UltraNixie" branding inside a watch image)
        // got dragged into a paragraph anchored at the far-left
        // ("Brand") just because both happened to land on the
        // same y-row, smearing the card's source rect across
        // half the screen.
        let prev_cy = current.max_bottom - curr_h / 2.0;
        let curr_cy = curr[1] + curr_h / 2.0;
        let close_y = (prev_cy - curr_cy).abs() < curr_h * 0.5;
        let h_gap = if curr[0] >= current.max_x {
            curr[0] - current.max_x
        } else if curr[0] + curr[2] <= current.min_x {
            current.min_x - (curr[0] + curr[2])
        } else {
            // Already overlapping → effectively zero gap.
            0.0
        };
        // OCR splits within a single logical line rarely leave a
        // gap larger than two glyph widths; anything beyond that
        // is a separate item on the same visual row.
        let close_x = h_gap < curr_h * 2.0;
        let same_row = close_y && close_x;
        if same_row {
            current.push(idx, curr, is_substantial);
            continue;
        }

        // Vertical merge into the next row. Gap compared against the
        // smaller glyph height — 1.2× covers typical 1.5-2× line-
        // height body text.
        let gap = (curr[1] - current.max_bottom).max(0.0);
        let group_h = (current.max_bottom
            - current
                .members
                .first()
                .map(|&i| lines[i].bbox[1])
                .unwrap_or(current.max_bottom))
        .max(curr_h);
        let min_h = group_h.min(curr_h).max(curr_h);
        let close_vertically = gap <= 1.2 * min_h;

        // Title / subtitle / body transitions usually coincide with
        // a font-size change — a 15 px subtitle below an 18 px title
        // visually reads as a new paragraph. Require the anchor
        // line's height to match curr within 15 % before merging so
        // the grouping respects those breaks.
        let anchor_h = current
            .last_substantial
            .map(|i| lines[i].bbox[3])
            .unwrap_or(curr_h);
        let height_ratio = anchor_h.min(curr_h) / anchor_h.max(curr_h).max(1.0);
        let similar_height = height_ratio > 0.85;

        // Column-width similarity: a narrow subtitle (≈224 px) above
        // a full-width body (≈833 px) belongs to a different scope
        // even if vertical gap would otherwise merge them. Compared
        // against the anchor's width (last substantial member) so a
        // paragraph's internal line-to-line width variation — body
        // continuation lines are often shorter than the first line —
        // still passes at 0.3, but "tight heading above a wide body"
        // gets caught.
        let anchor_w = current
            .last_substantial
            .map(|i| lines[i].bbox[2])
            .unwrap_or(curr[2]);
        let width_ratio = anchor_w.min(curr[2]) / anchor_w.max(curr[2]).max(1.0);
        let similar_width = width_ratio > 0.3;

        // Horizontal alignment uses the GROUP's full x-range so a
        // wide paragraph consistently accepts narrower follow-up
        // lines (continuation of the same column).
        let group_w = (current.max_x - current.min_x).max(1.0);
        let curr_left = curr[0];
        let curr_right = curr[0] + curr[2];
        let overlap = (current.max_x.min(curr_right) - current.min_x.max(curr_left)).max(0.0);
        let min_w = group_w.min(curr[2]).max(1.0);
        let horizontally_aligned = overlap / min_w >= 0.6;

        let prev_finished = current
            .last_substantial
            .map(|i| ends_paragraph(&lines[i].text))
            .unwrap_or(false);
        let curr_label = curr_is_label(&lines[idx].text);

        // Short lines (< 6 chars) can join an existing substantial
        // paragraph without anchoring merges — they ride along as
        // context but don't carry the sentence forward. Two short
        // lines in a row stay separate (UI menu labels like
        // `File` / `Edit` must each get their own card).
        //
        // For substantial-to-substantial merges, `prev_finished`
        // and `curr_label` are evaluated against the LABEL-marker
        // set only (`:` / `：`), not the sentence-terminator set
        // (`.!?` etc.). That keeps body paragraphs (which sentence-
        // end mid-flow) merged while splitting tabular UI rows
        // around their `Brand :` / `Model :` / `Case Material :`
        // separators. See US8565474 for the canonical
        // continuation-vs-beginning rule (left-margin alignment
        // is the dominant signal); we extend that with `:` because
        // tabular labels are the failure mode the patent's
        // alignment-only check does not catch on dense product
        // spec sheets.
        let merge = if is_substantial {
            current.last_substantial.is_some()
                && close_vertically
                && horizontally_aligned
                && similar_height
                && similar_width
                && !prev_finished
                && !curr_label
        } else if current.last_substantial.is_some() {
            // Short lines must STILL be horizontally aligned with
            // their target paragraph. Without this check a 5-char
            // OCR fragment ("Brand", "Mode") on the spec list
            // dragged itself into a totally unrelated paragraph
            // sitting at a different x — the watch image's
            // "UltraNixie" branding ended up merged with the
            // bottom-left "Brand" label, smearing the card across
            // half the image. The horizontally_aligned floor was
            // missing only on this branch in earlier revisions
            // because the comment said short lines "ride along"
            // their paragraph; in practice they should still ride
            // along the SAME COLUMN.
            close_vertically && horizontally_aligned && !prev_finished && !curr_label
        } else {
            false
        };

        if merge {
            current.push(idx, curr, is_substantial);
        } else {
            groups.push(std::mem::take(&mut current.members));
            current = PendingGroup::new();
            current.push(idx, curr, is_substantial);
        }
    }
    if !current.members.is_empty() {
        groups.push(current.members);
    }
    groups
        .into_iter()
        .map(|members| {
            let text = members
                .iter()
                .map(|&i| lines[i].text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            Paragraph {
                members,
                text,
                block_bbox: None,
            }
        })
        .collect()
}

/// Build an LLM-ready prompt body that numbers each source line.
/// The numeric prefix `[1]`, `[2]`, … survives the LLM's phrasing
/// better than a bare `\n` and gives us a regex-parseable anchor
/// even if the model adds or removes internal line breaks. The
/// preceding instruction is delivered via the translator's user
/// prompt; here we just format the body.
pub fn build_numbered_paragraph(members_text: &[String]) -> String {
    members_text
        .iter()
        .enumerate()
        .map(|(i, t)| format!("[{}] {}", i + 1, t))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Distribute a multi-line translation back to its member OCR lines.
/// Tries the numbered-marker format first (robust against LLM
/// rewrapping), then falls back to `\n` splitting, then to the
/// "everything on the first card" safety net.
///
/// Numbered path: extracts every `[N]` tagged segment and places it
/// in member slot `N-1`. Missing numbers leave their slot empty,
/// letting the frontend show the source text so the user can tell
/// something's off.
pub fn split_paragraph_translation(translation: &str, line_count: usize) -> Vec<String> {
    if line_count == 0 {
        return Vec::new();
    }

    // 1. Numbered-marker parse. LLMs usually echo the `[N]`
    //    prefix; any line whose `[N]` matches a valid member
    //    index wins its slot.
    if let Some(mapped) = parse_numbered_segments(translation, line_count) {
        return mapped;
    }

    // 2. Plain newline fallback (covers the LLM stripping the `[N]`
    //    prefixes but keeping the line count).
    let trimmed: Vec<&str> = translation
        .split('\n')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if trimmed.len() == line_count {
        return trimmed.into_iter().map(|s| s.to_string()).collect();
    }
    if trimmed.len() > line_count {
        let mut out: Vec<String> = trimmed
            .iter()
            .take(line_count - 1)
            .map(|s| s.to_string())
            .collect();
        let tail = trimmed[(line_count - 1)..].join(" ");
        out.push(tail);
        return out;
    }

    // 3. The LLM collapsed the whole paragraph into one sentence.
    //    Put everything on the first card so the user has a
    //    complete translation visible; subsequent cards stay
    //    empty and the frontend falls back to source text.
    let mut out = vec![translation.trim().to_string()];
    out.extend((1..line_count).map(|_| String::new()));
    out
}

/// Parse `[1] … [2] … [3] …` style numbered output into a slot
/// vector of length `line_count`. Returns `None` when fewer than
/// half of the expected markers are present — in that case the
/// caller falls through to plain-\n splitting.
///
/// Markers are matched anywhere on a line (LLMs sometimes write
/// `**[1]** …` or `[1]:` variants). Duplicate numbers keep the
/// last occurrence; out-of-range numbers are ignored.
fn parse_numbered_segments(text: &str, line_count: usize) -> Option<Vec<String>> {
    let mut slots: Vec<Option<String>> = vec![None; line_count];
    let mut matches = 0usize;
    // Split into chunks *at* every `[N]` marker so one marker's
    // payload can span multiple raw lines (LLMs sometimes break
    // a long translation in the middle of a numbered item).
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut current: Option<(usize, usize)> = None; // (slot_index, start_byte)
    while i < bytes.len() {
        if bytes[i] == b'[' {
            // Parse `[N]` where N is a positive integer.
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b']' {
                let num_str = std::str::from_utf8(&bytes[i + 1..j]).unwrap_or("");
                if let Ok(n) = num_str.parse::<usize>() {
                    if n >= 1 && n <= line_count {
                        // Flush previous slot before we switch.
                        if let Some((idx, start)) = current {
                            let seg = text[start..i].trim().to_string();
                            if !seg.is_empty() {
                                if slots[idx].is_none() {
                                    matches += 1;
                                }
                                slots[idx] = Some(seg);
                            }
                        }
                        current = Some((n - 1, j + 1));
                        i = j + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    if let Some((idx, start)) = current {
        let seg = text[start..].trim().to_string();
        if !seg.is_empty() {
            if slots[idx].is_none() {
                matches += 1;
            }
            slots[idx] = Some(seg);
        }
    }

    // Require a majority of slots filled before we trust the
    // numbered parse — otherwise the LLM probably dropped the format.
    if matches * 2 < line_count {
        return None;
    }
    Some(
        slots
            .into_iter()
            .map(|s| {
                s.unwrap_or_default()
                    .trim()
                    .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
                    .to_string()
            })
            .collect(),
    )
}

/// Production `LensSink` — forwards every pipeline event to its
/// matching Tauri channel. Lives here because the channel names
/// (`moonlens://...`) are coupled to the Lens frontend; putting
/// them next to the only struct that uses them makes the coupling
/// obvious and keeps `lens_pipeline.rs` UI-framework-agnostic.
///
/// Also persists each completed flow into the translation history so
/// the Lens management page can list "最近の翻訳" without needing the
/// frontend to listen on the lens streaming channels itself. Flows
/// produce one history row per run (joined source + translation) with
/// `kind = "lens"`. Provider attribution is intentionally left blank:
/// translations now flow through the configured chain (embedded
/// the embedded LLM, Ollama, plugins, cloud APIs) and a single flow can hit
/// fallbacks across providers, so naming any one would be
/// misleading. The Settings → History detail view surfaces the
/// `kind = "lens"` filter; users who need per-provider stats look at
/// the non-Lens history rows.
pub struct TauriLensSink {
    app: AppHandle,
    lens_id: String,
    pipeline: Arc<Pipeline>,
    flows: parking_lot::Mutex<std::collections::HashMap<u64, FlowAccumulator>>,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct FlowAccumulator {
    /// Source-text per OCR line, indexed by `line_index`.
    sources: std::collections::BTreeMap<usize, String>,
    /// Translated text per OCR line. Streaming chunks append; the
    /// final committed value lands when `done=true` arrives.
    translations: std::collections::BTreeMap<usize, String>,
}

impl FlowAccumulator {
    /// Record (or overwrite) the source-text per OCR line. Called
    /// once per flow when the OCR cards arrive, so we wipe whatever
    /// stale sources were buffered first.
    pub(crate) fn record_sources(&mut self, lines: &[(usize, String)]) {
        self.sources.clear();
        for (idx, text) in lines {
            self.sources.insert(*idx, text.clone());
        }
    }

    /// Append a translation chunk for a line. Empty deltas are
    /// ignored so streaming heartbeats don't pollute the buffer.
    pub(crate) fn append_translation_chunk(&mut self, line_index: usize, delta: &str) {
        if delta.is_empty() {
            return;
        }
        self.translations
            .entry(line_index)
            .or_default()
            .push_str(delta);
    }

    /// Drain into a `(joined_source, joined_translation)` pair. Lines
    /// are joined in `line_index` order and separated by newlines so
    /// the history detail pane can display them as paragraphs.
    pub(crate) fn finalize(self) -> (String, String) {
        let source = self.sources.into_values().collect::<Vec<_>>().join("\n");
        let translation = self
            .translations
            .into_values()
            .collect::<Vec<_>>()
            .join("\n");
        (source, translation)
    }
}

impl TauriLensSink {
    pub fn new(app: AppHandle, lens_id: String, pipeline: Arc<Pipeline>) -> Self {
        Self {
            app,
            lens_id,
            pipeline,
            flows: parking_lot::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl LensSink for TauriLensSink {
    fn on_flow_start(&self, flow_id: u64) {
        // Reset any stale accumulator for this flow_id (defensive: the
        // watcher reuses ids only across distinct lens windows but a
        // hot-reload during dev can leave residue).
        self.flows
            .lock()
            .insert(flow_id, FlowAccumulator::default());
        let _ = self.app.emit(
            "moonlens://lens-flow-start",
            LensFlowStart {
                id: self.lens_id.clone(),
                flow_id,
            },
        );
    }
    fn on_cards_ready(&self, flow_id: u64, image_w: u32, image_h: u32, cards: &[LensOcrLine]) {
        {
            let mut guard = self.flows.lock();
            let acc = guard.entry(flow_id).or_default();
            let pairs: Vec<(usize, String)> = cards
                .iter()
                .map(|c| (c.line_index, c.text.clone()))
                .collect();
            acc.record_sources(&pairs);
        }
        let _ = self.app.emit(
            "moonlens://lens-ocr-lines",
            LensOcrLines {
                id: self.lens_id.clone(),
                flow_id,
                image_w,
                image_h,
                lines: cards.to_vec(),
            },
        );
    }
    fn on_line_chunk(&self, flow_id: u64, line_index: usize, delta: String, done: bool) {
        {
            let mut guard = self.flows.lock();
            let acc = guard.entry(flow_id).or_default();
            acc.append_translation_chunk(line_index, &delta);
        }
        let _ = self.app.emit(
            "moonlens://lens-line-chunk",
            LensLineChunk {
                id: self.lens_id.clone(),
                flow_id,
                line_index,
                delta,
                done,
            },
        );
    }
    fn on_flow_done(&self, flow_id: u64, line_count: usize) {
        let acc = self.flows.lock().remove(&flow_id).unwrap_or_default();
        let (source, translation) = acc.finalize();
        if !source.trim().is_empty() || !translation.trim().is_empty() {
            self.pipeline.record_external_history(
                "lens",
                if source.trim().is_empty() {
                    None
                } else {
                    Some(source.as_str())
                },
                if translation.trim().is_empty() {
                    None
                } else {
                    Some(translation.as_str())
                },
                None,
                false,
            );
        }
        let _ = self.app.emit(
            "moonlens://lens-flow-done",
            LensFlowDone {
                id: self.lens_id.clone(),
                flow_id,
                line_count,
            },
        );
    }
    fn on_mask(&self, _flow_id: u64, masked: bool) {
        let _ = self.app.emit(
            "moonlens://lens-mask",
            LensMask {
                id: self.lens_id.clone(),
                masked,
            },
        );
    }
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

        // Everything below this line is the shared pipeline — exactly
        // the same `run_lens_pipeline` the offline evaluator drives.
        // Capture specifics (hash dedup, mask signalling, window-
        // alive checks) stay up here because they are only
        // meaningful when we are literally watching a live window;
        // the pipeline itself never observes them.
        let snapshot = settings.snapshot();
        // The lens translation pair is the GLOBAL default
        // (`translation.default_*`), NOT the subtitle pair.
        // Previously we read `subtitle.target_language`, which
        // silently bound the visual translation language to the
        // user's audio-transcription target — a counter-intuitive
        // cross-feature dependency. The new top-level field
        // decouples the two so each workflow can be set
        // independently.  Source is similarly honoured so the user
        // can pin (e.g.) "en" to skip provider script detection
        // when they already know the source.
        let config = PipelineConfig {
            source_language: snapshot
                .translation
                .default_source_language
                .clone(),
            target_language: snapshot
                .translation
                .default_target_language
                .clone(),
            max_concurrent_translate: snapshot.performance.max_concurrent_tasks.max(1) as usize,
            // 3× upscale recovers small body-text recognition (see
            // PipelineConfig::default for the rationale).
            ocr_scale: 3,
            glossary_min_lines: snapshot.providers.glossary_min_lines as usize,
        };

        let mut input = PreCapturedInput::new(CapturedFrame {
            png,
            width: image_w,
            height: image_h,
        });
        let sink: Arc<dyn LensSink> = Arc::new(TauriLensSink::new(
            app.clone(),
            lens_id.clone(),
            pipeline.clone(),
        ));
        let ocr = Arc::new(PipelineLensOcr::new(pipeline.clone()));
        // The Lens used to instantiate `OllamaTranslator` directly here,
        // which silently bound the entire feature to a running Ollama
        // daemon regardless of `providers.translate.chain`. Routing
        // through the Pipeline lets the chain (embedded LLM, plugins,
        // cloud APIs, ...) drive the Lens just like the one-shot
        // capture flow — the user's settings finally win.
        let translator = Arc::new(ChainBackedLensTranslator::new(pipeline.clone()));

        if let Err(e) =
            run_lens_pipeline(&mut input, sink.clone(), ocr, translator, &config, flow_id).await
        {
            tracing::warn!("lens {lens_id}: pipeline run failed: {e}");
        }

        // Tauri 2 ships chunk events (`lens-line-chunk`) and the
        // terminator (`lens-flow-done`) over *different* named
        // channels, and the runtime does not guarantee ordering
        // across channels. Without a grace period the terminator
        // (emitted inside the pipeline) can race ahead of the very
        // last `done=true` chunk and the frontend commits a
        // truncated translation. 80 ms is longer than the inter-
        // channel jitter we have observed (< 10 ms) and short
        // enough to feel instant. Kept here (outside the pipeline)
        // because it is a Tauri-channel quirk, not a pipeline
        // concern.
        tokio::time::sleep(Duration::from_millis(80)).await;
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
#[allow(dead_code)]
async fn translate_paragraph_streaming(
    app: AppHandle,
    lens_id: String,
    flow_id: u64,
    paragraph: Paragraph,
    source_language: String,
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
            source_language,
            target_language,
            settings,
        )
        .await;
    }

    // Rebuild the paragraph input with explicit `[1]`/`[2]`/...
    // markers so the LLM's output stays parseable even if it
    // reshuffles internal newlines. The system prompt still asks
    // for newlines, but the markers are the belt.
    let members_text: Vec<String> = paragraph.text.split('\n').map(|s| s.to_string()).collect();
    let members_text = if members_text.len() == paragraph.members.len() {
        members_text
    } else {
        // Defensive: group_paragraphs always joins with '\n', so
        // this mismatch should never fire, but log so we notice.
        tracing::warn!(
            "paragraph text had {} segments, members {} — falling back to raw text",
            members_text.len(),
            paragraph.members.len()
        );
        vec![paragraph.text.clone()]
    };
    let numbered = build_numbered_paragraph(&members_text);

    // Buffer the whole translation here; we can only split once the
    // final token arrives (the LLM occasionally reshuffles line breaks
    // mid-stream). Nothing is emitted to the frontend while the
    // stream is running — previous cards from the last flow keep
    // showing until we swap in the new paragraph.
    let buffer = Arc::new(parking_lot::Mutex::new(String::new()));
    let buffer_for_sink = buffer.clone();
    let sink: Arc<dyn Fn(String, bool) + Send + Sync> = Arc::new(move |delta, _done| {
        if !delta.is_empty() {
            buffer_for_sink.lock().push_str(&delta);
        }
    });

    let pipeline = app.state::<crate::app::AppState>().pipeline.clone();
    let res = pipeline
        .run_with_sink(
            Capability::Translate,
            TaskInput::Translate {
                text: numbered,
                source_language: if source_language.trim().is_empty() {
                    "auto".into()
                } else {
                    source_language.clone()
                },
                target_language: target_language.clone(),
                glossary: Vec::new(),
            },
            sink,
        )
        .await?;

    // Pipeline returns the aggregated text in `output`; fall back
    // to the buffer if the sink path delivered more detail.
    let full = match res.output {
        TaskOutput::Text(s) if !s.is_empty() => s,
        _ => buffer.lock().clone(),
    };

    let segments = split_paragraph_translation(&full, paragraph.members.len());

    // Recovery path: if the numbered/newline split leaves most slots
    // empty, the paragraph-at-once strategy failed. Retry one line
    // at a time — costs 2× the LLM calls for this flow, but always
    // produces usable output. Threshold: more than half of the
    // segments empty.
    let empty_count = segments.iter().filter(|s| s.trim().is_empty()).count();
    let use_retry = empty_count * 2 > paragraph.members.len();

    if use_retry {
        tracing::info!(
            "paragraph translate split yielded {}/{} empty segments, retrying per-line",
            empty_count,
            paragraph.members.len()
        );
        for (i, &line_idx) in paragraph.members.iter().enumerate() {
            let line_text = members_text.get(i).cloned().unwrap_or_default();
            if let Err(e) = translate_line_streaming(
                app.clone(),
                lens_id.clone(),
                flow_id,
                line_idx,
                line_text,
                source_language.clone(),
                target_language.clone(),
                settings.clone(),
            )
            .await
            {
                tracing::info!("lens paragraph retry line {line_idx} failed: {e}");
            }
        }
        return Ok(full);
    }

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
/// Translate a single card's joined block text and stream chunks
/// back tagged with this card's `line_index`. Called from the watch
/// loop once per card; concurrency is bounded by the pool semaphore
/// in the caller so the runtime never exceeds the configured
/// parallel-translation budget.
async fn translate_line_streaming(
    app: AppHandle,
    lens_id: String,
    flow_id: u64,
    line_index: usize,
    text: String,
    source_language: String,
    target_language: String,
    _settings: Arc<SettingsStore>,
) -> Result<String, ProviderError> {
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

    let pipeline = app.state::<crate::app::AppState>().pipeline.clone();
    let res = pipeline
        .run_with_sink(
            Capability::Translate,
            TaskInput::Translate {
                text,
                source_language: if source_language.trim().is_empty() {
                    "auto".into()
                } else {
                    source_language
                },
                target_language,
                glossary: Vec::new(),
            },
            sink,
        )
        .await?;
    match res.output {
        TaskOutput::Text(s) => Ok(s),
        _ => Err(ProviderError::Other(
            "translator returned unexpected output".into(),
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
    fn flow_accumulator_records_sources_in_line_index_order() {
        let mut acc = FlowAccumulator::default();
        // Provide lines out of order — the BTreeMap normalises sort.
        acc.record_sources(&[
            (2, "second".into()),
            (0, "first".into()),
            (1, "middle".into()),
        ]);
        let (src, translation) = acc.finalize();
        assert_eq!(src, "first\nmiddle\nsecond");
        assert_eq!(translation, "");
    }

    #[test]
    fn flow_accumulator_record_sources_overwrites_stale_buffer() {
        let mut acc = FlowAccumulator::default();
        acc.record_sources(&[(0, "stale".into())]);
        // Re-recording wipes whatever was there (e.g. a previous flow's
        // sources surviving an early termination).
        acc.record_sources(&[(0, "fresh".into())]);
        assert_eq!(acc.finalize().0, "fresh");
    }

    #[test]
    fn flow_accumulator_appends_translation_chunks() {
        let mut acc = FlowAccumulator::default();
        acc.record_sources(&[(0, "hello".into()), (1, "world".into())]);
        // Streaming arrives as repeated deltas; non-empty chunks
        // concatenate into the per-line buffer.
        acc.append_translation_chunk(0, "こん");
        acc.append_translation_chunk(0, "にちは");
        acc.append_translation_chunk(1, "");
        acc.append_translation_chunk(1, "世界");
        let (_src, translation) = acc.finalize();
        assert_eq!(translation, "こんにちは\n世界");
    }

    #[test]
    fn flow_accumulator_keeps_partial_translations_on_missing_lines() {
        let mut acc = FlowAccumulator::default();
        acc.record_sources(&[(0, "a".into()), (1, "b".into()), (2, "c".into())]);
        // Only line 1 streams a translation; the other lines stay
        // empty in the joined output (single newline between non-
        // empty entries instead of three blank lines).
        acc.append_translation_chunk(1, "ビー");
        let (src, translation) = acc.finalize();
        assert_eq!(src, "a\nb\nc");
        assert_eq!(translation, "ビー");
    }

    #[test]
    fn flow_accumulator_empty_after_default() {
        let acc = FlowAccumulator::default();
        let (src, translation) = acc.finalize();
        assert!(src.is_empty());
        assert!(translation.is_empty());
    }

    #[test]
    fn detect_rim_gradient_returns_none_for_uniform_rim() {
        let c = [200u8, 210, 220];
        assert_eq!(detect_rim_gradient_cardinals(c, c, c, c), None);
    }

    #[test]
    fn detect_rim_gradient_picks_vertical_when_top_bottom_differ() {
        let top = [30, 30, 30];
        let bottom = [230, 230, 230];
        let side = [130, 130, 130];
        let g = detect_rim_gradient_cardinals(top, bottom, side, side).expect("gradient");
        assert_eq!(g.angle, 180);
        assert_eq!(g.from, top);
        assert_eq!(g.to, bottom);
    }

    #[test]
    fn detect_rim_gradient_picks_horizontal_when_left_right_differ() {
        let side = [130, 130, 130];
        let left = [20, 10, 10];
        let right = [240, 210, 210];
        let g = detect_rim_gradient_cardinals(side, side, left, right).expect("gradient");
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
        let g = detect_rim_gradient_cardinals(top, bottom, left, right).expect("gradient");
        assert_eq!(g.angle, 180);
    }

    #[test]
    fn detect_rim_gradient_picks_diagonal_when_corners_dominate() {
        // Diagonal lighting: top-left dark → bottom-right light.
        // The four edge midpoints sit roughly at mid-grey (averaging
        // the diagonal out) but the corner samples preserve the
        // full delta. The 8-arg detector must prefer the diagonal.
        let mid = [128u8, 128, 128];
        let tl = [20, 20, 20];
        let br = [235, 235, 235];
        let tr_bl = [128, 128, 128]; // roughly mid for the anti-diagonal
        let g = detect_rim_gradient(mid, mid, mid, mid, tl, tr_bl, tr_bl, br).expect("gradient");
        assert_eq!(g.angle, 135);
        assert_eq!(g.from, tl);
        assert_eq!(g.to, br);
    }

    #[test]
    fn rim_is_clean_linear_gradient_accepts_uniform_rim() {
        let c = [180u8, 180, 180];
        assert!(rim_is_clean_linear_gradient(c, c, c, c));
    }

    #[test]
    fn rim_is_clean_linear_gradient_accepts_true_diagonal() {
        // Linear gradient TL(0) → BR(255) with midpoints at 128.
        let tl = [0u8, 0, 0];
        let br = [255u8, 255, 255];
        let mid = [128u8, 128, 128];
        assert!(rim_is_clean_linear_gradient(tl, mid, mid, br));
    }

    #[test]
    fn rim_is_clean_linear_gradient_rejects_checkered_pattern() {
        // Two dark + two light corners that do NOT line up as a
        // linear gradient (e.g. diagonal checker). The sum test
        // `tl+br ≈ tr+bl` should reject this.
        let dark = [20u8, 20, 20];
        let light = [230u8, 230, 230];
        assert!(!rim_is_clean_linear_gradient(dark, light, light, dark));
    }

    #[test]
    fn refine_gradient_from_bbox_produces_multi_stop_for_vertical() {
        use image::{DynamicImage, Rgb, RgbImage};
        // Build a 40×80 image with a vertical gradient from dark at
        // the top to light at the bottom, plus a dark glyph stripe
        // running horizontally in the middle that must be ignored.
        let (w, h) = (40u32, 80u32);
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            let t = y as f32 / (h as f32 - 1.0);
            let v = (40.0 + t * 200.0) as u8;
            for x in 0..w {
                if y >= 35 && y < 45 {
                    // "glyph" — far from gradient bg, should be
                    // filtered out by the text threshold.
                    img.put_pixel(x, y, Rgb([20, 20, 20]));
                } else {
                    img.put_pixel(x, y, Rgb([v, v, v]));
                }
            }
        }
        let dynamic = DynamicImage::ImageRgb8(img);
        // Simulate the real call path: rim_start at y=0 (darkest),
        // rim_end at y=max (lightest). These act as per-stop bg
        // anchors so the filter tolerates a 200-unit gradient span.
        let rim_start = [40u8, 40, 40];
        let rim_end = [240u8, 240, 240];
        let stops =
            refine_gradient_from_bbox(&dynamic, [0.0, 0.0, 40.0, 80.0], rim_start, rim_end, 180)
                .expect("stops sampled");
        assert_eq!(stops.len(), 5);
        // First stop should be dark, last should be bright.
        assert!(stops[0][0] < 90, "top stop: {:?}", stops[0]);
        assert!(stops[4][0] > 200, "bottom stop: {:?}", stops[4]);
    }

    #[test]
    fn detect_rim_gradient_picks_anti_diagonal() {
        // Top-right dark → bottom-left light.
        let mid = [128u8, 128, 128];
        let tr = [15, 15, 15];
        let bl = [240, 240, 240];
        let g = detect_rim_gradient(mid, mid, mid, mid, mid, tr, bl, mid).expect("gradient");
        assert_eq!(g.angle, 45);
        assert_eq!(g.from, bl);
        assert_eq!(g.to, tr);
    }

    fn make_stroke_image(glyph_h: u32, stroke_w: u32) -> image::DynamicImage {
        // Build a small bbox-sized image with a uniform light
        // background and a dark vertical stripe of the requested
        // thickness at x in [8 .. 8+stroke_w). Pads 2 rows above and
        // below the glyph so the empty-row crop actually kicks in.
        use image::{DynamicImage, Rgb, RgbImage};
        let w = 40u32;
        let h = glyph_h + 4;
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgb([230, 230, 230]));
            }
        }
        let stripe_x0 = 8u32;
        let stripe_x1 = stripe_x0 + stroke_w;
        for y in 2..(2 + glyph_h) {
            for x in stripe_x0..stripe_x1 {
                img.put_pixel(x, y, Rgb([20, 20, 20]));
            }
        }
        DynamicImage::ImageRgb8(img)
    }

    #[test]
    fn estimate_font_weight_regular_for_thin_strokes() {
        // 20 px tall glyph with a 1 px stroke → ratio 0.05 → 400.
        let img = make_stroke_image(20, 1);
        let bg = [230u8, 230, 230];
        let w = estimate_font_weight(&img, [0.0, 0.0, 40.0, 24.0], bg);
        assert_eq!(w, 400, "1 px stroke should map to regular weight");
    }

    #[test]
    fn estimate_font_weight_regular_for_typical_body_text() {
        // 20 px tall glyph with a 2 px stroke → ratio 0.10 → 400.
        // Mirrors typical ~10-12pt body copy on screen (Arial
        // Regular at 96dpi has ~1.2-1.5 px strokes).
        let img = make_stroke_image(20, 2);
        let bg = [230u8, 230, 230];
        let w = estimate_font_weight(&img, [0.0, 0.0, 40.0, 24.0], bg);
        assert_eq!(w, 400, "2 px / 20 px ratio stays in regular band");
    }

    #[test]
    fn estimate_font_weight_semi_for_medium_strokes() {
        // 20 px tall glyph with a 3 px stroke → ratio 0.15 → 600.
        let img = make_stroke_image(20, 3);
        let bg = [230u8, 230, 230];
        let w = estimate_font_weight(&img, [0.0, 0.0, 40.0, 24.0], bg);
        assert_eq!(w, 600, "3 px stroke should map to semi-bold");
    }

    #[test]
    fn estimate_font_weight_bold_for_thick_strokes() {
        // 20 px tall glyph with a 4 px stroke → ratio 0.20 → 700.
        let img = make_stroke_image(20, 4);
        let bg = [230u8, 230, 230];
        let w = estimate_font_weight(&img, [0.0, 0.0, 40.0, 24.0], bg);
        assert_eq!(w, 700, "4 px stroke should map to bold");
    }

    #[test]
    fn estimate_font_weight_defaults_when_bbox_degenerate() {
        use image::{DynamicImage, Rgb, RgbImage};
        let mut img = RgbImage::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                img.put_pixel(x, y, Rgb([255, 255, 255]));
            }
        }
        let bg = [255u8, 255, 255];
        // 1x1 bbox falls below the 3x3 floor.
        let w = estimate_font_weight(&DynamicImage::ImageRgb8(img), [0.0, 0.0, 1.0, 1.0], bg);
        assert_eq!(w, 400);
    }

    #[test]
    fn estimate_font_weight_defaults_when_no_text_pixels() {
        use image::{DynamicImage, Rgb, RgbImage};
        let mut img = RgbImage::new(30, 30);
        for y in 0..30 {
            for x in 0..30 {
                img.put_pixel(x, y, Rgb([240, 240, 240]));
            }
        }
        let bg = [240u8, 240, 240];
        let w = estimate_font_weight(&DynamicImage::ImageRgb8(img), [0.0, 0.0, 30.0, 30.0], bg);
        assert_eq!(w, 400, "empty bbox → default");
    }

    #[test]
    fn split_paragraph_translation_maps_numbered_output_one_to_one() {
        let segs = split_paragraph_translation("[1] 第一行\n[2] 第二行\n[3] 第三行", 3);
        assert_eq!(segs, vec!["第一行", "第二行", "第三行"]);
    }

    #[test]
    fn split_paragraph_translation_numbered_survives_wrapped_payloads() {
        // One marker's payload spans two raw output lines — the
        // parser must treat everything up to the next `[N]` as one
        // segment.
        let segs = split_paragraph_translation("[1] first line\ncontinued\n[2] second line", 2);
        assert_eq!(segs.len(), 2);
        assert!(segs[0].contains("first line") && segs[0].contains("continued"));
        assert_eq!(segs[1], "second line");
    }

    #[test]
    fn split_paragraph_translation_numbered_falls_back_when_markers_missing() {
        // The LLM stripped the markers but kept the line count — the
        // plain-\n path should still hand back three clean slots.
        let segs = split_paragraph_translation("A\nB\nC", 3);
        assert_eq!(segs, vec!["A", "B", "C"]);
    }

    #[test]
    fn split_paragraph_translation_falls_back_on_missing_marker() {
        // Only the second marker was preserved — the numbered path
        // requires a majority of slots, so we drop to the plain-\n
        // fallback.
        let segs = split_paragraph_translation("just text\n[2] mid\nmore", 3);
        // Plain-\n path sees three lines → 1:1 map, markers intact
        // because we only strip `[N]` for matched segments.
        assert_eq!(segs.len(), 3);
    }

    #[test]
    fn split_paragraph_translation_absorbs_extra_segments_into_last_line() {
        // The LLM ignored the `[N]` format AND emitted too many
        // newlines — plain-\n fallback absorbs the tail.
        let segs = split_paragraph_translation("A\nB\nC extra\ntrailing", 3);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "A");
        assert_eq!(segs[1], "B");
        assert!(segs[2].contains("C extra") && segs[2].contains("trailing"));
    }

    #[test]
    fn split_paragraph_translation_collapses_onto_first_when_too_few() {
        // The LLM ignored both line breaks AND markers — keep the
        // entire translation visible on the first card instead of
        // truncating.
        let segs = split_paragraph_translation("One long sentence.", 3);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], "One long sentence.");
        assert_eq!(segs[1], "");
        assert_eq!(segs[2], "");
    }

    #[test]
    fn split_paragraph_translation_numbered_handles_colon_variant() {
        // LLMs sometimes write `[1]: foo` — the colon should be
        // stripped from the slot content.
        let segs = split_paragraph_translation("[1]: first\n[2]: second", 2);
        assert_eq!(segs, vec!["first", "second"]);
    }

    #[test]
    fn build_numbered_paragraph_formats_correctly() {
        let text: Vec<String> = vec!["hello".into(), "world".into()];
        let out = build_numbered_paragraph(&text);
        assert_eq!(out, "[1] hello\n[2] world");
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
    fn group_paragraphs_splits_tabular_rows_on_label_colon() {
        // Product-spec layouts ("Brand :", "Model :",
        // "Case Material :", ...) stack labels and values
        // vertically with similar widths, similar x-positions, and
        // tight line spacing — the exact pattern that body-text
        // grouping merges. The colon ending is the dominant
        // signal that this is tabular data, not a wrapped sentence.
        // Each label and value should land in its own paragraph so
        // the inpaint and the translation respect the table's row
        // structure rather than spilling one mega-card across
        // every cell.
        let lines = vec![
            OcrLine {
                text: "Brand :".into(),
                bbox: [10.0, 10.0, 80.0, 16.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "UltraNixie".into(),
                bbox: [10.0, 32.0, 100.0, 16.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Model :".into(),
                bbox: [10.0, 54.0, 80.0, 16.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "UNW21N16SMVIODL".into(),
                bbox: [10.0, 76.0, 160.0, 16.0],
                confidence: 1.0,
            },
        ];
        let p = group_paragraphs(&lines);
        assert_eq!(
            p.len(),
            4,
            "every table row should be its own paragraph, got {}",
            p.len()
        );
    }

    #[test]
    fn group_paragraphs_merges_through_sentence_end_in_body_text() {
        // Body paragraphs from books, articles and Wikipedia
        // routinely end a sentence mid-flow ("...delightful.")
        // and continue on the next line. The dominant signal is
        // line LENGTH: long lines that happen to end with `.`
        // are wrapped body, while short lines that end with `.`
        // are usually labels (or `:` misread by OCR). The 30-char
        // floor in `ends_paragraph` is what keeps real body text
        // together; both lines below clear that threshold.
        let lines = vec![
            OcrLine {
                text: "Of all the senses, sight must be the most delightful.".into(),
                bbox: [10.0, 10.0, 480.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Few things give us greater pleasure than to see again.".into(),
                bbox: [10.0, 32.0, 480.0, 20.0],
                confidence: 1.0,
            },
        ];
        let p = group_paragraphs(&lines);
        assert_eq!(
            p.len(),
            1,
            "tightly-stacked long lines should stay together as body"
        );
        assert_eq!(p[0].members, vec![0, 1]);
    }

    #[test]
    fn detect_text_align_picks_right_when_lines_share_right_edge() {
        // A right-aligned column: every line ends at the same
        // pixel, but the left edge varies because line widths
        // differ. The classic signature for `text-align: right`.
        let rendered = vec![
            OcrLine {
                text: "short".into(),
                bbox: [60.0, 10.0, 40.0, 16.0], // right edge 100
                confidence: 1.0,
            },
            OcrLine {
                text: "longer line".into(),
                bbox: [10.0, 30.0, 90.0, 16.0], // right edge 100
                confidence: 1.0,
            },
        ];
        let align = detect_text_align(&[0, 1], &rendered, None);
        assert_eq!(align, TextAlign::Right);
    }

    #[test]
    fn detect_text_align_picks_center_when_both_edges_vary() {
        // Centred lines: both left and right edges shift. The
        // canonical signature for `text-align: center`.
        let rendered = vec![
            OcrLine {
                text: "Title".into(),
                bbox: [40.0, 10.0, 30.0, 16.0], // x in [40, 70]
                confidence: 1.0,
            },
            OcrLine {
                text: "Subtitle".into(),
                bbox: [25.0, 30.0, 60.0, 16.0], // x in [25, 85]
                confidence: 1.0,
            },
        ];
        let align = detect_text_align(&[0, 1], &rendered, None);
        assert_eq!(align, TextAlign::Center);
    }

    #[test]
    fn detect_text_align_uses_block_for_single_line_card() {
        use crate::lens_layout::{LayoutBlock, Polarity};
        // Single-line text near the right edge of a wider layout
        // block — the multi-line variance check has nothing to
        // work with, so detection falls back on the block bbox
        // gap.
        let rendered = vec![OcrLine {
            text: "Status".into(),
            bbox: [180.0, 10.0, 60.0, 16.0], // right edge 240
            confidence: 1.0,
        }];
        let block = [0.0, 5.0, 250.0, 26.0]; // right edge 250 → small gap
        let align = detect_text_align(&[0], &rendered, Some(block));
        assert_eq!(align, TextAlign::Right);
    }

    #[test]
    fn group_paragraphs_does_not_cross_merge_short_lines_across_columns() {
        // The 04ea regression: a 5-char "Brand" (non-substantial)
        // OCR detection in the spec list got merged with a
        // horizontally-distant "UltraNixie" branding from the
        // watch image because the non-substantial branch skipped
        // the horizontal-alignment check. Short lines must stay in
        // their own column; otherwise card bboxes balloon and the
        // overlay no longer follows the source layout.
        let lines = vec![
            OcrLine {
                text: "UltraNixie".into(),
                bbox: [300.0, 100.0, 100.0, 16.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Brand".into(), // 5 chars → non-substantial
                bbox: [10.0, 100.0, 60.0, 16.0],
                confidence: 1.0,
            },
        ];
        let p = group_paragraphs(&lines);
        assert_eq!(
            p.len(),
            2,
            "non-substantial line in a different column must NOT merge"
        );
    }

    #[test]
    fn group_paragraphs_splits_short_sentence_endings_as_labels() {
        // Short lines ending with `.!?` are almost always labels
        // (or labels whose `:` was OCR'd as `.`). Examples:
        //   `Brand .`        ← OCR misread of `Brand :`
        //   `Manufacturing Process .` (23 chars)
        //   `Standby Time .` (14 chars)
        // The 30-char floor flags these as paragraph boundaries
        // even when no colon survived the OCR.
        let lines = vec![
            OcrLine {
                text: "Manufacturing Process .".into(), // 23 chars
                bbox: [10.0, 10.0, 200.0, 16.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Precision Die Casting".into(),
                bbox: [10.0, 32.0, 200.0, 16.0],
                confidence: 1.0,
            },
        ];
        let p = group_paragraphs(&lines);
        assert_eq!(
            p.len(),
            2,
            "short label-like line ending in `.` should split"
        );
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
    fn group_by_layout_blocks_assigns_lines_by_coverage() {
        use crate::lens_layout::{LayoutBlock, Polarity};

        // Block 1 covers the top half; block 2 covers a small region
        // in the bottom right. Three OCR lines: two sit inside
        // block 1, one sits inside block 2.
        let blocks = vec![
            LayoutBlock {
                bbox: [0.0, 0.0, 200.0, 50.0],
                polarity: Polarity::Dark,
            },
            LayoutBlock {
                bbox: [150.0, 60.0, 40.0, 20.0],
                polarity: Polarity::Dark,
            },
        ];
        let lines = vec![
            OcrLine {
                text: "Line A".into(),
                bbox: [10.0, 5.0, 60.0, 10.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Line B".into(),
                bbox: [10.0, 20.0, 60.0, 10.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "tiny".into(),
                bbox: [155.0, 65.0, 30.0, 10.0],
                confidence: 1.0,
            },
        ];
        let paragraphs = group_by_layout_blocks(&lines, &blocks, 0.5);
        assert_eq!(paragraphs.len(), 2);
        let top = paragraphs.iter().find(|p| p.members == vec![0, 1]).unwrap();
        assert!(
            top.block_bbox.is_some(),
            "block-based paragraph has block_bbox"
        );
        assert_eq!(top.text, "Line A\nLine B");
        let bottom = paragraphs.iter().find(|p| p.members == vec![2]).unwrap();
        assert_eq!(bottom.text, "tiny");
    }

    #[test]
    fn group_by_layout_blocks_drops_ocr_outside_any_block() {
        use crate::lens_layout::{LayoutBlock, Polarity};

        // One block in the top-left. A wild OCR line floats in empty
        // space with no overlapping block — it must be dropped
        // rather than associated with the nearest block or becoming
        // a stray card.
        let blocks = vec![LayoutBlock {
            bbox: [0.0, 0.0, 100.0, 30.0],
            polarity: Polarity::Dark,
        }];
        let lines = vec![
            OcrLine {
                text: "real text".into(),
                bbox: [10.0, 5.0, 60.0, 10.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "hallucination".into(),
                bbox: [200.0, 200.0, 40.0, 10.0],
                confidence: 1.0,
            },
        ];
        let paragraphs = group_by_layout_blocks(&lines, &blocks, 0.5);
        assert_eq!(paragraphs.len(), 1, "only the real line gets a paragraph");
        assert_eq!(paragraphs[0].members, vec![0]);
    }

    #[test]
    fn coverage_is_one_when_a_is_fully_inside_b() {
        // Small bbox fully inside a bigger one — IoU would be ~0.1,
        // but coverage (area / a_area) should be 1.0.
        let a = [10.0, 10.0, 20.0, 20.0];
        let b = [0.0, 0.0, 200.0, 100.0];
        assert!((coverage(a, b) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn coverage_is_zero_when_disjoint() {
        assert_eq!(
            coverage([0.0, 0.0, 10.0, 10.0], [50.0, 50.0, 10.0, 10.0]),
            0.0
        );
    }

    #[test]
    fn coverage_handles_partial_overlap() {
        // Half of A sits inside B.
        let v = coverage([0.0, 0.0, 20.0, 20.0], [10.0, 0.0, 20.0, 20.0]);
        assert!((v - 0.5).abs() < 1e-4, "expected 0.5, got {v}");
    }

    #[test]
    fn split_paragraph_leaves_homogeneous_members_untouched() {
        // Three English lines, no skipped intruders → single sub-group,
        // original order preserved.
        let rendered = vec![
            OcrLine {
                text: "line A".into(),
                bbox: [10.0, 10.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "line B".into(),
                bbox: [10.0, 40.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "line C".into(),
                bbox: [10.0, 70.0, 100.0, 20.0],
                confidence: 1.0,
            },
        ];
        let skip_flags = vec![false, false, false];
        let groups =
            split_paragraph_by_skipped_intrusions(&[0, 1, 2], &rendered, &rendered, &skip_flags);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![0, 1, 2]);
    }

    #[test]
    fn split_paragraph_splits_at_target_native_intrusion() {
        // English 1 at y=10, Japanese (skip) at y=40, English 2 at y=70
        // — the Japanese intrusion must break the paragraph into
        // two sub-groups so downstream inpainting never erases it.
        let rendered = vec![
            OcrLine {
                text: "Eng 1".into(),
                bbox: [10.0, 10.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Eng 2".into(),
                bbox: [10.0, 70.0, 100.0, 20.0],
                confidence: 1.0,
            },
        ];
        // ocr_lines includes the skipped Japanese line too — skip
        // flags align with ocr_lines' indexing, not rendered's.
        let ocr_lines = vec![
            OcrLine {
                text: "Eng 1".into(),
                bbox: [10.0, 10.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "日本語".into(),
                bbox: [10.0, 40.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Eng 2".into(),
                bbox: [10.0, 70.0, 100.0, 20.0],
                confidence: 1.0,
            },
        ];
        let skip_flags = vec![false, true, false];
        let groups =
            split_paragraph_by_skipped_intrusions(&[0, 1], &rendered, &ocr_lines, &skip_flags);
        assert_eq!(
            groups.len(),
            2,
            "Japanese intrusion should split paragraph into two sub-groups"
        );
        assert_eq!(groups[0], vec![0]);
        assert_eq!(groups[1], vec![1]);
    }

    #[test]
    fn split_paragraph_ignores_horizontally_disjoint_skipped_lines() {
        // A skipped line sits vertically between the English members
        // but in a completely different column (horizontally disjoint).
        // That must NOT trigger a split — the intrusion only matters
        // if it occupies the same column as the paragraph.
        let rendered = vec![
            OcrLine {
                text: "Eng 1".into(),
                bbox: [10.0, 10.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "Eng 2".into(),
                bbox: [10.0, 70.0, 100.0, 20.0],
                confidence: 1.0,
            },
        ];
        let ocr_lines = vec![
            OcrLine {
                text: "Eng 1".into(),
                bbox: [10.0, 10.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "日本語".into(),
                bbox: [400.0, 40.0, 80.0, 20.0], // off in a different column
                confidence: 1.0,
            },
            OcrLine {
                text: "Eng 2".into(),
                bbox: [10.0, 70.0, 100.0, 20.0],
                confidence: 1.0,
            },
        ];
        let skip_flags = vec![false, true, false];
        let groups =
            split_paragraph_by_skipped_intrusions(&[0, 1], &rendered, &ocr_lines, &skip_flags);
        assert_eq!(groups.len(), 1, "disjoint column must not trigger split");
        assert_eq!(groups[0], vec![0, 1]);
    }

    #[test]
    fn source_line_height_ratio_returns_fallback_for_single_member() {
        let lines = vec![OcrLine {
            text: "solo".into(),
            bbox: [0.0, 10.0, 100.0, 20.0],
            confidence: 1.0,
        }];
        assert!((source_line_height_ratio(&[0], &lines) - 1.25).abs() < 1e-4);
    }

    #[test]
    fn source_line_height_ratio_measures_top_to_top_gap() {
        // Two lines, glyph height 20, top-to-top distance 30 →
        // line-height ratio 1.5. Tight body text.
        let lines = vec![
            OcrLine {
                text: "a".into(),
                bbox: [0.0, 10.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "b".into(),
                bbox: [0.0, 40.0, 100.0, 20.0],
                confidence: 1.0,
            },
        ];
        let ratio = source_line_height_ratio(&[0, 1], &lines);
        assert!((ratio - 1.5).abs() < 1e-4, "expected 1.5 got {ratio}");
    }

    #[test]
    fn source_line_height_ratio_clamps_outliers() {
        // One normal pair, one pathological pair with 3× ratio.
        // Median of [1.5, 2.2(clamped from 3.0)] = 1.85.
        let lines = vec![
            OcrLine {
                text: "a".into(),
                bbox: [0.0, 10.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "b".into(),
                bbox: [0.0, 40.0, 100.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "c".into(),
                bbox: [0.0, 100.0, 100.0, 20.0],
                confidence: 1.0,
            },
        ];
        let ratio = source_line_height_ratio(&[0, 1, 2], &lines);
        assert!(
            (1.0..=2.2).contains(&ratio),
            "clamped ratio out of bounds: {ratio}"
        );
    }

    #[test]
    fn block_overlaps_skipped_detects_same_row_japanese() {
        // Block covers a mixed "出典：Wikipedia ページ" line. A
        // skipped Japanese OCR line sits inside that block. The
        // helper must flag it so the caller falls back to the OCR
        // union bbox instead of engulfing the Japanese.
        let ocr_lines = vec![
            OcrLine {
                text: "Wikipedia".into(),
                bbox: [60.0, 70.0, 80.0, 22.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "出典：".into(),
                bbox: [10.0, 70.0, 40.0, 22.0],
                confidence: 1.0,
            },
        ];
        let skip_flags = vec![false, true];
        let block = [10.0, 70.0, 171.0, 22.0];
        assert!(block_overlaps_skipped(block, &ocr_lines, &skip_flags));
    }

    #[test]
    fn block_overlaps_skipped_returns_false_for_pure_latin_block() {
        // No skipped lines intersect the block → block_bbox is safe
        // to use as source_bbox directly.
        let ocr_lines = vec![OcrLine {
            text: "Hello".into(),
            bbox: [10.0, 10.0, 100.0, 22.0],
            confidence: 1.0,
        }];
        let skip_flags = vec![false];
        let block = [10.0, 10.0, 100.0, 22.0];
        assert!(!block_overlaps_skipped(block, &ocr_lines, &skip_flags));
    }

    #[test]
    fn preprocess_drops_noise_tokens_like_stray_digits() {
        // english0004 regression: OCR picked up a 17×17 px "0" bullet
        // that slipped past every downstream check because eval did
        // not run `looks_like_ocr_noise`. Both paths now flow through
        // `preprocess_ocr_lines`, so a single-digit fragment must
        // disappear before the translator ever sees it.
        let raw = vec![
            OcrLine {
                text: "0".into(),
                bbox: [15.0, 14.0, 17.0, 17.0],
                confidence: 0.9,
            },
            OcrLine {
                text: "Hello world".into(),
                bbox: [0.0, 40.0, 100.0, 20.0],
                confidence: 1.0,
            },
        ];
        let (lines, flags) = preprocess_ocr_lines(raw, "ja");
        assert_eq!(lines.len(), 1, "noise '0' must be dropped");
        assert_eq!(lines[0].text, "Hello world");
        assert_eq!(flags.len(), 1);
        assert!(!flags[0], "non-target-native line keeps skip=false");
    }

    #[test]
    fn preprocess_flags_target_native_lines() {
        let raw = vec![
            OcrLine {
                text: "Hello".into(),
                bbox: [0.0, 0.0, 80.0, 20.0],
                confidence: 1.0,
            },
            OcrLine {
                text: "こんにちは".into(),
                bbox: [0.0, 30.0, 80.0, 20.0],
                confidence: 1.0,
            },
        ];
        let (lines, flags) = preprocess_ocr_lines(raw, "ja");
        assert_eq!(lines.len(), 2);
        assert_eq!(
            flags,
            vec![false, true],
            "JA line must be skip-flagged when target is ja"
        );
    }

    #[test]
    fn preprocess_is_lossless_for_clean_input() {
        // No noise, no target-native — pipeline is a pass-through.
        let raw = vec![OcrLine {
            text: "Regular sentence goes here.".into(),
            bbox: [0.0, 0.0, 200.0, 20.0],
            confidence: 1.0,
        }];
        let (lines, flags) = preprocess_ocr_lines(raw.clone(), "ja");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, raw[0].text);
        assert_eq!(flags, vec![false]);
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
    fn noise_filter_rejects_decorative_separator_runs() {
        // UI chrome decorations misread as text — long dash / dot
        // runs embedded in otherwise-alphanumeric content.
        assert!(looks_like_ocr_noise("abc----def"));
        assert!(looks_like_ocr_noise("foo::::bar"));
        assert!(looks_like_ocr_noise("xxx||||yyy"));
    }

    #[test]
    fn noise_filter_keeps_short_punctuation_clusters() {
        // Ellipsis and common emoticon-style punctuation should pass.
        assert!(!looks_like_ocr_noise("Hello... world"));
        assert!(!looks_like_ocr_noise("wait, really?!"));
    }

    #[test]
    fn noise_filter_rejects_japanese_ocr_artefacts() {
        // Clear-gibberish cases ocrs produces on CJK source:
        // consonant-heavy strings with no English digrams and no
        // stopwords. Mixed-case + vowel-imbalanced patterns that
        // happen to contain a common letter pair (e.g. "aloo"
        // contains "al") remain a weakness of this heuristic —
        // only a proper language-id provider (plan.md v0.2) can
        // kill those definitively. Until then we ship the strong
        // cases and let borderline Latin slip through.
        assert!(looks_like_ocr_noise("dbHzC qXpR"));
        assert!(looks_like_ocr_noise("kxpq zlmn"));
        assert!(looks_like_ocr_noise("bcdfgh jklmn"));
        assert!(looks_like_ocr_noise("zxvbnp qwrst"));
    }

    #[test]
    fn noise_filter_keeps_real_english_phrases() {
        // Sanity: signals must NOT reject short real English. The
        // stopword list is the floor every natural sentence clears.
        assert!(!looks_like_ocr_noise("Hello world"));
        assert!(!looks_like_ocr_noise("Save the file"));
        assert!(!looks_like_ocr_noise("Click to continue"));
        assert!(!looks_like_ocr_noise("Open settings"));
        assert!(!looks_like_ocr_noise("The quick brown fox"));
    }

    #[test]
    fn noise_filter_keeps_stopword_free_technical_text() {
        // Proper nouns and technical words without stopwords should
        // still pass when their digram density looks natural.
        assert!(!looks_like_ocr_noise("JavaScript"));
        assert!(!looks_like_ocr_noise("rendering complete"));
        assert!(!looks_like_ocr_noise("process running"));
    }

    #[test]
    fn noise_filter_rejects_extreme_vowel_ratio() {
        // Both consonant soup and vowel spam.
        assert!(looks_like_ocr_noise("bcdfgzhk"));
        assert!(looks_like_ocr_noise("aeeeeeeo"));
    }

    #[test]
    fn english_digram_density_scores_reasonable() {
        // Natural English ≥ 0.20; random gibberish < 0.10.
        assert!(english_digram_density("the quick brown fox") >= 0.20);
        assert!(english_digram_density("xkqvzbpjfw") < 0.10);
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
    fn text_is_already_target_single_kanji_ja_label_skips() {
        // Single-kanji UI labels (戻 / 次 / 始 / 終 / ...) are
        // meaningful in Japanese and should NOT be force-translated
        // into bizarre LLM rewordings.
        assert!(text_is_already_target("日", "ja"));
        assert!(text_is_already_target("戻", "ja"));
        assert!(text_is_already_target("日本", "ja"));
    }

    #[test]
    fn text_is_already_target_halfwidth_katakana_skips() {
        // Halfwidth katakana is unambiguously Japanese.
        assert!(text_is_already_target("ｶﾀｶﾅ", "ja"));
    }

    #[test]
    fn text_is_already_target_japanese_punctuation_skips() {
        // Mixed Japanese punctuation + kanji.
        assert!(text_is_already_target("「日本語」", "ja"));
        assert!(text_is_already_target("これは、テスト。", "ja"));
    }

    #[test]
    fn text_is_already_target_mixed_jp_and_latin_translates() {
        // Mixed content: latin dominates so treat as "needs
        // translation" — this prevents the regression where a
        // Japanese line merged into an English paragraph caused the
        // whole paragraph to be skipped, leaving English
        // untranslated.
        assert!(!text_is_already_target("JavaScript五", "ja"));
        // Hello (5 latin) + こんにちは (5 kana) = 50% each, below
        // the 70% JP dominance threshold → translate.
        assert!(!text_is_already_target("Hello こんにちは", "ja"));
        // Short English mixed with a single kanji — translate so
        // the English part actually gets rendered in JA.
        assert!(!text_is_already_target("Open the 窓", "ja"));
    }

    #[test]
    fn text_is_already_target_ja_dominant_mix_skips() {
        // Mostly Japanese with a trailing English loanword: treat
        // as already-target so we don't paraphrase the JP part.
        // 長い日本語の文章 (8 JP chars) + OK (2 latin) = 80% JP
        // → skip.
        assert!(text_is_already_target("長い日本語の文章OK", "ja"));
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
        assert!(
            fg[0] > 180 && fg[1] < 80 && fg[2] < 80,
            "expected red-ish, got {fg:?}"
        );
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

    /// Helper for the split-by-height tests below. Each `(y, h)` pair
    /// becomes one synthetic OCR line whose width is fixed and whose
    /// text is empty — the splitter only consults `bbox[3]` so the
    /// other fields don't matter.
    fn make_ocr_lines_for_heights(heights: &[(f32, f32)]) -> Vec<OcrLine> {
        heights
            .iter()
            .map(|(y, h)| OcrLine {
                text: String::new(),
                bbox: [0.0, *y, 100.0, *h],
                confidence: 1.0,
            })
            .collect()
    }

    #[test]
    fn split_subgroup_by_height_isolates_title_from_body() {
        // Heading (h=32) directly above body lines (h=14) — the kind
        // of layout that would otherwise produce one card with the
        // body coming out bold.
        let lines = make_ocr_lines_for_heights(&[
            (0.0, 32.0),
            (40.0, 14.0),
            (58.0, 14.0),
            (76.0, 14.0),
        ]);
        let groups = split_subgroup_by_height_discontinuity(&[0, 1, 2, 3], &lines);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![0]);
        assert_eq!(groups[1], vec![1, 2, 3]);
    }

    #[test]
    fn split_subgroup_by_height_keeps_uniform_paragraph() {
        // All four lines within 10% of each other — should NOT
        // split. Standard body paragraph.
        let lines = make_ocr_lines_for_heights(&[
            (0.0, 14.0),
            (18.0, 14.5),
            (36.5, 14.0),
            (54.5, 13.5),
        ]);
        let groups = split_subgroup_by_height_discontinuity(&[0, 1, 2, 3], &lines);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![0, 1, 2, 3]);
    }

    #[test]
    fn split_subgroup_by_height_handles_single_member() {
        let lines = make_ocr_lines_for_heights(&[(0.0, 20.0)]);
        let groups = split_subgroup_by_height_discontinuity(&[0], &lines);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![0]);
    }

    #[test]
    fn split_subgroup_by_height_handles_empty() {
        let lines = make_ocr_lines_for_heights(&[]);
        let groups = split_subgroup_by_height_discontinuity(&[], &lines);
        assert_eq!(groups, vec![Vec::<usize>::new()]);
    }

    #[test]
    fn split_subgroup_by_height_below_threshold_does_not_split() {
        // 1.5× — just below the 1.6× threshold. Should stay merged.
        let lines = make_ocr_lines_for_heights(&[(0.0, 14.0), (18.0, 21.0)]);
        let groups = split_subgroup_by_height_discontinuity(&[0, 1], &lines);
        assert_eq!(groups.len(), 1);
    }

    #[test]
    fn expand_card_for_alignment_caps_slack_for_long_body() {
        // Long body paragraph (e.g. 600px wide) inside a 1000px-wide
        // block. With `text_align: Left`, the OLD behaviour grew the
        // card to the full block right edge (1000px), making the
        // card 400px wider than the source. The cap ensures growth
        // is bounded to 25% of source width = 150px.
        let inpaint = [50.0_f32, 100.0, 600.0, 30.0];
        let block = Some([20.0_f32, 80.0, 1000.0, 400.0]);
        let card = expand_card_for_alignment(inpaint, TextAlign::Left, block, 1920, 1080);
        let card_right = card[0] + card[2];
        let inpaint_right = inpaint[0] + inpaint[2]; // 650
        let slack = card_right - inpaint_right;
        assert!(
            slack >= 0.0 && slack <= 600.0 * 0.25 + 0.5,
            "left-aligned card slack {slack} exceeded 25% cap (source width 600px)"
        );
        // Left edge stays anchored at the source's left.
        assert!((card[0] - inpaint[0]).abs() < 0.5);
    }

    #[test]
    fn expand_card_for_alignment_centers_slack_for_centered_text() {
        let inpaint = [400.0_f32, 50.0, 200.0, 28.0];
        let block = Some([0.0_f32, 0.0, 1000.0, 200.0]);
        let card = expand_card_for_alignment(inpaint, TextAlign::Center, block, 1920, 1080);
        let left_slack = inpaint[0] - card[0];
        let right_slack = (card[0] + card[2]) - (inpaint[0] + inpaint[2]);
        assert!(left_slack > 0.0 && right_slack > 0.0);
        // Center alignment should grow about equally on both sides.
        assert!(
            (left_slack - right_slack).abs() < 0.5,
            "center card slack unbalanced: left={left_slack}, right={right_slack}"
        );
        // Total growth still capped at 25% of source width.
        assert!(left_slack + right_slack <= 200.0 * 0.25 + 0.5);
    }
}
