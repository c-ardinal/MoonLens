//! Live in-place translation of a pinned region.
//!
//! Pinning a region opens the `live` window exactly over it: a
//! transparent, click-through, always-on-top window that paints each
//! paragraph's translation over the source text, like the selector does
//! on a frozen frame, but on the live screen. The watcher keeps reading
//! the region and updates the window as the content changes.
//!
//! Per tick (`performance.region_watcher_poll_ms`):
//!
//! 1. Capture the region and reduce it to a small greyscale thumbnail.
//!    Unchanged pixels cost nothing more.
//! 2. When pixels change, translations whose source area changed are
//!    hidden at once ([`LiveEvent::Stale`]), so an outdated translation
//!    never sits over new text while OCR runs.
//! 3. OCR the region. When the text is what is already shown (a video
//!    behind a subtitle, a blinking cursor), nothing is re-translated.
//!    New text is translated once it reads the same on two consecutive
//!    ticks, so text that is still being typed out is not translated
//!    half-way; text that keeps changing is translated after
//!    [`SETTLE_LIMIT`] ticks anyway.
//! 4. Blocks seen before come from the translation cache and are shown
//!    immediately; only new paragraphs go to the model.
//!
//! Steps 1–4 live in [`Tracker`], which Lens windows run too (see
//! `lens::set_continuous`).
//!
//! The live window is excluded from screen capture (Windows / macOS), so
//! the watcher reads the original text under it. Where that is not
//! supported (Linux) the window is hidden for the duration of a capture.

use std::sync::Arc;
use std::time::Duration;

use image::{DynamicImage, GrayImage};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize};

use crate::{
    capture::{self, CaptureRect},
    pipeline::{LensBlock, Pipeline},
    providers::{Capability, TaskOutput},
    settings::SettingsStore,
    user_error,
};

/// Label of the live translation window (see `tauri.conf.json`).
pub const LIVE_WINDOW: &str = "live";

/// Event sent to the live window.
pub const LIVE_EVENT: &str = "moonlens://live";

/// Longest side of the thumbnail used for change detection.
const THUMB_MAX_SIDE: u32 = 192;

/// Luma difference above which a thumbnail pixel counts as changed.
const PIXEL_DELTA: u8 = 24;

/// Share of changed thumbnail pixels for the region to count as changed.
/// Small enough for a one-word change in a subtitle line, large enough to
/// ignore compression noise in video.
const REGION_CHANGED: f32 = 0.002;

/// Share of changed pixels inside a block's box that hides its
/// translation before OCR confirms the change.
const BLOCK_CHANGED: f32 = 0.12;

/// Ticks a changing text may take to settle before it is translated
/// anyway (a clock, a counter).
const SETTLE_LIMIT: u32 = 6;

/// What the watcher is currently tracking.
#[derive(Debug, Clone)]
pub struct WatchTarget {
    pub rect: CaptureRect,
    /// Translate into this language; `None` = the configured target.
    pub target_language: Option<String>,
}

/// Messages for the live window.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveEvent {
    /// Blocks to paint, positioned in the window's physical pixels.
    /// `translating` is true while new paragraphs are still with the
    /// model (blocks without a translation are drawn as pending).
    Blocks {
        blocks: Vec<LensBlock>,
        translating: bool,
        /// Engine that translated the new blocks, once known.
        #[serde(skip_serializing_if = "Option::is_none")]
        provider: Option<String>,
        /// The new translations came from a cloud provider.
        #[serde(skip_serializing_if = "std::ops::Not::not")]
        via_cloud: bool,
    },
    /// Hide these blocks (indices into the last `Blocks`): their source
    /// changed and a new reading is on its way.
    Stale { indices: Vec<usize> },
    /// Nothing to show (live translation stopped or the region is empty).
    Clear,
    /// A QR code in the region was decoded; Lens windows show `text`.
    Decoded { text: String },
    /// Reading or translating failed (`code|message`, see `user_error`).
    /// Lens windows show it; the live window ignores it.
    Error { message: String },
}

#[derive(Default)]
pub struct RegionWatcher {
    inner: Mutex<Option<Running>>,
    /// Region of the most recent capture (or pin), so the pin hotkey can
    /// pin "what I just translated" without the overlay being open.
    last: Mutex<Option<WatchTarget>>,
}

struct Running {
    handle: JoinHandle<()>,
    stop: Arc<tokio::sync::Notify>,
}

impl RegionWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_running(&self) -> bool {
        self.inner.lock().is_some()
    }

    pub fn start(
        &self,
        app: AppHandle,
        pipeline: Arc<Pipeline>,
        settings: Arc<SettingsStore>,
        target: WatchTarget,
    ) {
        self.stop_task();
        self.remember(target.clone());
        if let Err(e) = show_live_window(&app, &target.rect) {
            tracing::warn!("showing the live translation window: {e}");
        }
        let stop = Arc::new(tokio::sync::Notify::new());
        let stop_t = stop.clone();
        let handle = tauri::async_runtime::spawn(async move {
            run_watch(app, pipeline, settings, target, stop_t).await;
        });
        *self.inner.lock() = Some(Running { handle, stop });
    }

    /// Record the region of the latest capture as the next pin target.
    pub fn remember(&self, target: WatchTarget) {
        *self.last.lock() = Some(target);
    }

    /// Pin hotkey: stop when running, otherwise pin the last captured
    /// region. Returns the new pinned state, or `None` when there is
    /// nothing to pin yet.
    pub fn toggle(
        &self,
        app: AppHandle,
        pipeline: Arc<Pipeline>,
        settings: Arc<SettingsStore>,
    ) -> Option<bool> {
        if self.is_running() {
            self.stop_with(&app);
            return Some(false);
        }
        let target = self.last.lock().clone()?;
        self.start(app, pipeline, settings, target);
        Some(true)
    }

    /// Stop watching and take the translations off the screen.
    pub fn stop_with(&self, app: &AppHandle) {
        self.stop_task();
        hide_live_window(app);
    }

    /// Stop watching (the live window is left to the caller).
    pub fn stop(&self) {
        self.stop_task();
    }

    fn stop_task(&self) {
        if let Some(r) = self.inner.lock().take() {
            r.stop.notify_waiters();
            r.handle.abort();
        }
    }
}

/// Place the live window over `rect` and show it without taking focus.
fn show_live_window(app: &AppHandle, rect: &CaptureRect) -> anyhow::Result<()> {
    let win = app
        .get_webview_window(LIVE_WINDOW)
        .ok_or_else(|| anyhow::anyhow!("live window missing"))?;
    let monitors = xcap::Monitor::all()?;
    let m = monitors
        .get(rect.monitor_index)
        .ok_or_else(|| anyhow::anyhow!("monitor {} missing", rect.monitor_index))?;
    let (mx, my) = (m.x()?, m.y()?);
    let _ = app.emit_to(LIVE_WINDOW, LIVE_EVENT, LiveEvent::Clear);
    win.set_position(PhysicalPosition::new(mx + rect.x, my + rect.y))?;
    win.set_size(PhysicalSize::new(rect.w, rect.h))?;
    // Keep our own translations out of the watcher's captures (and out of
    // screenshots / recordings) where the OS supports it.
    win.set_content_protected(true)?;
    win.set_ignore_cursor_events(true)?;
    win.set_always_on_top(true)?;
    win.show()?;
    Ok(())
}

fn hide_live_window(app: &AppHandle) {
    let _ = app.emit_to(LIVE_WINDOW, LIVE_EVENT, LiveEvent::Clear);
    if let Some(win) = app.get_webview_window(LIVE_WINDOW) {
        let _ = win.hide();
    }
}

/// Capture the region without the live window in it. Windows and macOS
/// exclude the window from capture (content protection); elsewhere it
/// is hidden for the moment of the capture.
async fn capture_under_window(app: &AppHandle, rect: CaptureRect) -> Option<DynamicImage> {
    let hide = cfg!(not(any(target_os = "windows", target_os = "macos")));
    let win = app.get_webview_window(LIVE_WINDOW);
    let was_visible = hide
        && win
            .as_ref()
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false);
    if was_visible {
        if let Some(w) = &win {
            let _ = w.hide();
        }
        // Let the compositor drop the window before reading the screen.
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    let image = tokio::task::spawn_blocking(move || capture::capture_rect(rect))
        .await
        .ok()
        .and_then(|r| r.ok());
    if was_visible {
        if let Some(w) = &win {
            let _ = w.show();
        }
    }
    image
}

/// Region geometry in screen pixels (`[x, y, w, h]`). A different value
/// means the region moved or was resized.
pub type Geometry = [i32; 4];

/// One reading of a watched region.
pub struct Frame {
    pub image: DynamicImage,
    pub geometry: Geometry,
}

/// What the caller supplies to [`Tracker::tick`].
pub struct TickEnv<'a> {
    pub pipeline: &'a Pipeline,
    pub target_language: &'a str,
    /// History kind of new translations (`pipeline::history_kind`).
    pub history_kind: &'a str,
    pub emit: &'a (dyn Fn(LiveEvent) + Send + Sync),
    /// Region geometry now. A translation finished after the region moved
    /// is not shown at the old place; the next tick shows it at the new.
    pub geometry_now: &'a (dyn Fn() -> Option<Geometry> + Send + Sync),
}

/// Result of one [`Tracker::tick`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tick {
    /// Pixels unchanged: no OCR, no translation.
    Unchanged,
    /// The region moved or was resized: shown blocks hidden, reading
    /// waits until it holds still.
    Moved,
    /// New text read once; waiting for the same reading again.
    Settling,
    /// Same text as shown (only pixels around it changed): translations
    /// kept, nothing sent to the model.
    SameText,
    /// No text in the region.
    Empty,
    /// A QR code was decoded (no text blocks).
    Decoded { text: String },
    /// New text shown. `requested` is false when every block came from
    /// the cache or is already in the target language.
    Translated {
        source: String,
        translation: String,
        requested: bool,
    },
}

/// What the tracker has on screen.
#[derive(Default)]
struct Shown {
    thumb: Option<Thumb>,
    /// Target language and source text of the shown blocks (or the
    /// decoded QR payload): what a new reading is compared with.
    text: Option<String>,
    blocks: Vec<LensBlock>,
    /// Indices already reported stale since the last `Blocks`.
    stale: Vec<usize>,
}

/// Change-driven reading of one screen region, shared by live
/// translation and Lens windows. Feed it a [`Frame`] per tick; it runs
/// OCR only when pixels change and translates only text not shown yet
/// (see the module docs for the steps).
#[derive(Default)]
pub struct Tracker {
    shown: Shown,
    /// Last frame read, and the text OCR found in it while waiting for
    /// the text to settle.
    last_thumb: Option<Thumb>,
    last_geometry: Option<Geometry>,
    pending_text: Option<String>,
    unsettled: u32,
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// The region moved or was resized: hide every shown block (their
    /// boxes no longer line up) and wait for it to hold still.
    pub fn region_moved(&mut self, geometry: Geometry, env: &TickEnv<'_>) {
        self.last_geometry = Some(geometry);
        self.last_thumb = None;
        self.pending_text = None;
        self.unsettled = 0;
        let newly: Vec<usize> = (0..self.shown.blocks.len())
            .filter(|i| !self.shown.stale.contains(i))
            .collect();
        if !newly.is_empty() {
            self.shown.stale.extend(&newly);
            (env.emit)(LiveEvent::Stale { indices: newly });
        }
    }

    /// Process one reading. `force` reads and shows the region even when
    /// nothing changed and skips the wait for text to settle (a manual
    /// refresh).
    pub async fn tick(
        &mut self,
        frame: Frame,
        force: bool,
        env: &TickEnv<'_>,
    ) -> Result<Tick, String> {
        if self.last_geometry.is_some_and(|g| g != frame.geometry) {
            self.region_moved(frame.geometry, env);
            return Ok(Tick::Moved);
        }
        self.last_geometry = Some(frame.geometry);
        let thumb = Thumb::of(&frame.image);
        let changed = self
            .last_thumb
            .as_ref()
            .map_or(true, |t| t.changed_share(&thumb, None) >= REGION_CHANGED);
        self.last_thumb = Some(thumb.clone());
        // Nothing changed since the last tick and nothing waits to settle.
        // The target language changed in settings: read and translate again.
        let language_changed = self
            .shown
            .text
            .as_deref()
            .is_some_and(|k| k.split('\u{1}').next() != Some(env.target_language));
        if !changed
            && !force
            && !language_changed
            && self.pending_text.is_none()
            && self.shown.stale.is_empty()
        {
            return Ok(Tick::Unchanged);
        }

        // Hide translations whose source area changed, before OCR.
        if let Some(base) = &self.shown.thumb {
            let newly: Vec<usize> = self
                .shown
                .blocks
                .iter()
                .enumerate()
                .filter(|(i, b)| {
                    !self.shown.stale.contains(i)
                        && base.changed_share(&thumb, Some(b.bbox)) >= BLOCK_CHANGED
                })
                .map(|(i, _)| i)
                .collect();
            if !newly.is_empty() {
                self.shown.stale.extend(&newly);
                (env.emit)(LiveEvent::Stale { indices: newly });
            }
        }

        let png = capture::png_bytes(&frame.image).map_err(|e| e.to_string())?;
        let mut bundle = env
            .pipeline
            .analyze_capture(png)
            .await
            .map_err(|e| user_error::provider(&e, Capability::Ocr))?;
        // The region moved while OCR ran: these boxes belong to the old
        // place. Hide what is shown and read again once it holds still.
        if let Some(now) = (env.geometry_now)().filter(|g| *g != frame.geometry) {
            self.region_moved(now, env);
            return Ok(Tick::Moved);
        }
        let text = source_key(&bundle.blocks);
        // A decoded QR code has no blocks; its payload is the reading.
        let decoded = match (&bundle.result.output, bundle.blocks.is_empty()) {
            (TaskOutput::Text(t), true) if !t.trim().is_empty() => Some(t.clone()),
            _ => None,
        };
        // What decides "same as shown": the text read, and the language
        // it is shown in (a changed target language translates again).
        let key = format!(
            "{}\u{1}{}",
            env.target_language,
            decoded.as_deref().map_or(text.as_str(), |d| d)
        );

        // A refresh retries blocks left untranslated (a failed request).
        let incomplete = self
            .shown
            .blocks
            .iter()
            .any(|b| !b.same_language && b.translation.is_none());
        if self.shown.text.as_deref() == Some(key.as_str()) && !(force && incomplete) {
            // Same text as on screen (only the background changed, or
            // the region moved over the same text): show what was hidden
            // again, at the new positions.
            self.pending_text = None;
            self.unsettled = 0;
            self.shown.thumb = Some(thumb);
            if let (Some(text), true) = (&decoded, force) {
                (env.emit)(LiveEvent::Decoded { text: text.clone() });
            } else if !self.shown.stale.is_empty() || force {
                self.shown.stale.clear();
                let blocks = carry_translations(&self.shown.blocks, bundle.blocks);
                self.shown.blocks = blocks.clone();
                (env.emit)(LiveEvent::Blocks {
                    blocks,
                    translating: false,
                    provider: None,
                    via_cloud: false,
                });
            }
            return Ok(Tick::SameText);
        }

        // New text: wait for it to read the same twice (text being typed
        // out), unless it has been changing for too long.
        let settled = force || self.pending_text.as_deref() == Some(key.as_str());
        if !settled && self.unsettled < SETTLE_LIMIT {
            self.pending_text = Some(key);
            self.unsettled += 1;
            return Ok(Tick::Settling);
        }
        self.pending_text = None;
        self.unsettled = 0;

        if bundle.blocks.is_empty() {
            self.shown = Shown {
                thumb: Some(thumb),
                text: Some(key),
                ..Shown::default()
            };
            if let Some(text) = decoded {
                // Take earlier boxes off (the live window shows no QR text).
                (env.emit)(LiveEvent::Clear);
                (env.emit)(LiveEvent::Decoded { text: text.clone() });
                return Ok(Tick::Decoded { text });
            }
            (env.emit)(LiveEvent::Clear);
            return Ok(Tick::Empty);
        }

        // Cached and same-language blocks are ready now; show them while
        // the rest is translated.
        env.pipeline
            .prepare_translation(&mut bundle, env.target_language);
        let requested =
            bundle.needs_translation() && bundle.blocks.iter().any(|b| b.needs_request());
        self.shown = Shown {
            thumb: Some(thumb),
            text: Some(key),
            blocks: bundle.blocks.clone(),
            stale: Vec::new(),
        };
        (env.emit)(LiveEvent::Blocks {
            blocks: bundle.blocks.clone(),
            translating: requested,
            provider: None,
            via_cloud: false,
        });
        if requested {
            let result = env
                .pipeline
                .translate_capture_as(&mut bundle, env.target_language, None, env.history_kind)
                .await;
            self.shown.blocks = bundle.blocks.clone();
            if let Err(e) = result {
                // Keep what is readable; the text is tried again once it
                // changes or on a manual refresh.
                self.shown.text = None;
                (env.emit)(LiveEvent::Blocks {
                    blocks: bundle.blocks,
                    translating: false,
                    provider: None,
                    via_cloud: false,
                });
                return Err(user_error::provider(&e, Capability::Translate));
            }
            // The region moved while the model ran: the boxes belong to
            // the old place. The next tick shows them at the new one.
            if (env.geometry_now)() == Some(frame.geometry) {
                (env.emit)(LiveEvent::Blocks {
                    blocks: bundle.blocks.clone(),
                    translating: false,
                    provider: Some(bundle.result.used_provider.clone()),
                    via_cloud: bundle.result.via_cloud,
                });
            }
        }
        let translation = bundle
            .blocks
            .iter()
            .map(|b| {
                b.translation
                    .as_deref()
                    .unwrap_or(if b.same_language { &b.source } else { "" })
                    .trim()
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        Ok(Tick::Translated {
            source: text,
            translation,
            requested,
        })
    }
}

async fn run_watch(
    app: AppHandle,
    pipeline: Arc<Pipeline>,
    settings: Arc<SettingsStore>,
    target: WatchTarget,
    stop: Arc<tokio::sync::Notify>,
) {
    let mut tracker = Tracker::new();
    let geometry = [
        target.rect.x,
        target.rect.y,
        target.rect.w as i32,
        target.rect.h as i32,
    ];
    let emit_app = app.clone();
    let emit = move |ev: LiveEvent| {
        let _ = emit_app.emit_to(LIVE_WINDOW, LIVE_EVENT, ev);
    };
    let geometry_now = move || Some(geometry);
    let mut first = true;
    loop {
        let snap = settings.snapshot();
        if !snap.overlay.follow_source {
            break;
        }
        let interval = snap.performance.region_watcher_poll_ms.max(50) as u64;
        let target_language = target
            .target_language
            .clone()
            .unwrap_or_else(|| snap.subtitle.target_language.clone());
        drop(snap);

        // The first reading happens at once; later ones every interval.
        if !first {
            tokio::select! {
                _ = stop.notified() => break,
                _ = tokio::time::sleep(Duration::from_millis(interval)) => {}
            }
        }
        first = false;

        let Some(image) = capture_under_window(&app, target.rect).await else {
            continue;
        };
        let env = TickEnv {
            pipeline: &pipeline,
            target_language: &target_language,
            history_kind: crate::pipeline::history_kind::LIVE,
            emit: &emit,
            geometry_now: &geometry_now,
        };
        if let Err(e) = tracker.tick(Frame { image, geometry }, false, &env).await {
            tracing::debug!("live translation: {e}");
        }
    }
    // Live translation was switched off in settings.
    hide_live_window(&app);
    let _ = app.emit("moonlens://pin-state", false);
}

/// Text of all blocks in order: what decides whether the screen says
/// something new.
fn source_key(blocks: &[LensBlock]) -> String {
    blocks
        .iter()
        .map(|b| b.source.trim())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// New readings of the same text: keep the translations, take the new
/// positions and colours.
fn carry_translations(old: &[LensBlock], new: Vec<LensBlock>) -> Vec<LensBlock> {
    new.into_iter()
        .map(|mut b| {
            if let Some(o) = old.iter().find(|o| o.source == b.source) {
                b.translation = o.translation.clone();
                b.same_language = o.same_language;
                b.from_cache = o.from_cache;
            }
            b
        })
        .collect()
}

/// Small greyscale copy of a capture for change detection.
#[derive(Clone)]
struct Thumb {
    img: GrayImage,
    /// Thumbnail pixels per capture pixel.
    scale: f32,
}

impl Thumb {
    fn of(image: &DynamicImage) -> Self {
        let (w, h) = (image.width().max(1), image.height().max(1));
        let scale = (THUMB_MAX_SIDE as f32 / w.max(h) as f32).min(1.0);
        let tw = ((w as f32 * scale).round() as u32).max(1);
        let th = ((h as f32 * scale).round() as u32).max(1);
        let img = image
            .resize_exact(tw, th, image::imageops::FilterType::Triangle)
            .to_luma8();
        Self { img, scale }
    }

    /// Share of pixels that differ from `other`, over the whole image or
    /// inside `bbox` (`[x, y, w, h]` in capture pixels). A different size
    /// (the region was resized) counts as fully changed.
    fn changed_share(&self, other: &Thumb, bbox: Option<[f32; 4]>) -> f32 {
        if self.img.dimensions() != other.img.dimensions() {
            return 1.0;
        }
        let (w, h) = self.img.dimensions();
        let (x0, y0, x1, y1) = match bbox {
            None => (0, 0, w, h),
            Some([x, y, bw, bh]) => {
                let s = self.scale;
                let x0 = ((x * s).floor().max(0.0) as u32).min(w);
                let y0 = ((y * s).floor().max(0.0) as u32).min(h);
                let x1 = (((x + bw) * s).ceil().max(0.0) as u32).clamp(x0, w);
                let y1 = (((y + bh) * s).ceil().max(0.0) as u32).clamp(y0, h);
                (x0, y0, x1, y1)
            }
        };
        let total = (x1 - x0) * (y1 - y0);
        if total == 0 {
            return 0.0;
        }
        let mut changed = 0u32;
        for y in y0..y1 {
            for x in x0..x1 {
                let a = self.img.get_pixel(x, y)[0];
                let b = other.img.get_pixel(x, y)[0];
                if a.abs_diff(b) > PIXEL_DELTA {
                    changed += 1;
                }
            }
        }
        changed as f32 / total as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn solid(w: u32, h: u32, v: u8) -> DynamicImage {
        DynamicImage::ImageRgba8(ImageBuffer::from_pixel(w, h, Rgba([v, v, v, 255])))
    }

    /// White image with a dark "word" drawn at `x`.
    fn with_word(x: u32) -> DynamicImage {
        let mut img = ImageBuffer::from_pixel(800, 60, Rgba([255, 255, 255, 255]));
        for yy in 20..40 {
            for xx in x..x + 60 {
                img.put_pixel(xx, yy, Rgba([0, 0, 0, 255]));
            }
        }
        DynamicImage::ImageRgba8(img)
    }

    fn block(source: &str, bbox: [f32; 4]) -> LensBlock {
        LensBlock {
            bbox,
            line_height: 20.0,
            source: source.into(),
            translation: None,
            colors: None,
            same_language: false,
            from_cache: false,
            align: Default::default(),
            bold: false,
            lines: vec![],
        }
    }

    #[test]
    fn identical_frames_do_not_count_as_changed() {
        let a = Thumb::of(&with_word(100));
        let b = Thumb::of(&with_word(100));
        assert_eq!(a.changed_share(&b, None), 0.0);
    }

    #[test]
    fn a_new_word_in_a_subtitle_line_is_a_change() {
        let a = Thumb::of(&with_word(100));
        let b = Thumb::of(&with_word(400));
        assert!(a.changed_share(&b, None) >= REGION_CHANGED);
    }

    #[test]
    fn faint_noise_is_not_a_change() {
        let a = Thumb::of(&solid(800, 60, 120));
        let b = Thumb::of(&solid(800, 60, 130));
        assert_eq!(a.changed_share(&b, None), 0.0);
    }

    #[test]
    fn change_is_measured_inside_a_block_only() {
        let a = Thumb::of(&with_word(100));
        let b = Thumb::of(&with_word(400));
        // The word moved away from x=100 and into x=400.
        assert!(a.changed_share(&b, Some([90.0, 15.0, 80.0, 30.0])) >= BLOCK_CHANGED);
        assert!(a.changed_share(&b, Some([600.0, 15.0, 80.0, 30.0])) < BLOCK_CHANGED);
    }

    #[test]
    fn a_resized_region_counts_as_fully_changed() {
        let a = Thumb::of(&solid(800, 60, 0));
        let b = Thumb::of(&solid(400, 60, 0));
        assert_eq!(a.changed_share(&b, None), 1.0);
    }

    #[test]
    fn rereading_the_same_text_keeps_translations_at_new_positions() {
        let mut old = vec![block("Hello", [0.0, 0.0, 50.0, 20.0])];
        old[0].translation = Some("こんにちは".into());
        let new = vec![block("Hello", [10.0, 40.0, 50.0, 20.0])];
        let out = carry_translations(&old, new);
        assert_eq!(out[0].translation.as_deref(), Some("こんにちは"));
        assert_eq!(out[0].bbox, [10.0, 40.0, 50.0, 20.0]);
    }

    #[test]
    fn source_key_ignores_surrounding_whitespace() {
        let a = vec![block(" Hello ", [0.0; 4]), block("World", [0.0; 4])];
        let b = vec![block("Hello", [0.0; 4]), block("World\n", [0.0; 4])];
        assert_eq!(source_key(&a), source_key(&b));
    }
}
