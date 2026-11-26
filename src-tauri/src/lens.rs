//! Window translation (`lens` windows).
//!
//! Window translation is the one on-screen translation feature: the
//! selector picks the target (a dragged range or the whole screen) on a
//! frozen frame and translates it once; switching to continuous reading
//! opens a lens window exactly over that range. A lens window is a
//! frameless, always-on-top, transparent window: its header can be
//! dragged and its edges resized, while the body lets the mouse through
//! to the app underneath (see [`run_passthrough`]). A session can have
//! many lens windows, each independent.
//!
//! A lens reads its region continuously, like a camera lens:
//! [`set_continuous`] runs the change-driven [`Tracker`], so OCR runs
//! only when the pixels change and only text not shown yet goes to the
//! model. While the lens is dragged or resized no capture is taken;
//! reading resumes once it is dropped. With continuous reading switched
//! off the lens is read on request only.
//!
//! The capture flow:
//! 1. The watcher (or the frontend, on request) captures a given lens id.
//! 2. The captured frame must not include the lens UI. Windows and macOS
//!    exclude the window from screen capture (content protection); on
//!    other platforms Rust hides the window for the moment of the grab.
//! 3. Rust resolves the window's client bounds, shrinks them by the
//!    header the renderer tells us about (CSS pixels, scaled to physical
//!    pixels here), and feeds the resulting rect to
//!    [`crate::capture::capture_global_rect`].
//! 4. Pipeline runs as usual; the translation appears in the lens body.
//!
//! Pure geometry and id generation live in [`geometry`] and [`ids`] so
//! they are testable.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use image::DynamicImage;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::{
    capture::CaptureRect,
    lens_drag::{self, DragGate},
    lens_text::{self, Follow},
    pipeline::Pipeline,
    region_watcher::{Frame, Geometry, LiveEvent, Tick, TickEnv, Tracker},
    settings::SettingsStore,
};

/// Event with [`LiveEvent`]s for one lens window.
pub const LENS_LIVE_EVENT: &str = "moonlens://lens-live";

/// Operate mode on (`true`) / off for a lens window (operate hotkey).
pub const LENS_OPERATE_EVENT: &str = "moonlens://lens-operate";

/// Height of the lens header in CSS pixels (fixed in `routes/lens`), used
/// to put the header just above a range when a lens opens over it.
pub const HEADER_CSS: u32 = 32;

/// Width in CSS pixels of the band along the window edges that stays
/// clickable for resizing while the body lets the mouse through.
const EDGE_CSS: f64 = 6.0;

/// Size in CSS pixels of the resize grip in the bottom-right corner.
const GRIP_CSS: f64 = 18.0;

/// How often the pointer position is checked for click-through.
const PASSTHROUGH_POLL_MS: u64 = 40;

/// Shortest pause between readings where the lens must be hidden for a
/// capture (no content protection): a shorter one makes it flicker.
const HIDDEN_CAPTURE_MIN_MS: u64 = 1000;

pub mod geometry {
    //! Pure rect-math for lens windows.

    use serde::{Deserialize, Serialize};

    /// Global screen-space rect covered by a lens window's **outer**
    /// frame (i.e. what [`WebviewWindow::outer_position`] and
    /// [`WebviewWindow::outer_size`] return in physical pixels).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct LensOuterRect {
        pub x: i32,
        pub y: i32,
        pub w: u32,
        pub h: u32,
    }

    /// Inset applied to the outer rect to reach the region that the lens
    /// actually observes. The renderer decides these values (because it
    /// owns the header / border layout) and reports them via `set_insets`.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub struct LensInsets {
        pub top: u32,
        pub bottom: u32,
        pub left: u32,
        pub right: u32,
    }

    /// Apply insets to the outer rect. Returns `None` if the insets
    /// consume the entire rect (so callers can skip pointless captures).
    pub fn observed_rect(outer: LensOuterRect, insets: LensInsets) -> Option<LensOuterRect> {
        let x = outer.x + insets.left as i32;
        let y = outer.y + insets.top as i32;
        let w = outer.w.saturating_sub(insets.left + insets.right);
        let h = outer.h.saturating_sub(insets.top + insets.bottom);
        if w == 0 || h == 0 {
            None
        } else {
            Some(LensOuterRect { x, y, w, h })
        }
    }

    /// Client rect for a lens that should observe `region` (global
    /// physical pixels): the header goes right above the region. Where
    /// the monitor ends above it (`top_limit`), the header covers the top
    /// of the region instead.
    pub fn window_for_region(region: LensOuterRect, header: u32, top_limit: i32) -> LensOuterRect {
        let y = (region.y - header as i32).max(top_limit.min(region.y));
        let bottom = region.y + region.h as i32;
        LensOuterRect {
            x: region.x,
            y,
            w: region.w,
            h: (bottom - y).max(header as i32 + 1) as u32,
        }
    }

    /// Top-left corner for a `size` window whose header point `grab`
    /// (offset from its corner) sits under the pointer, kept inside
    /// `monitor`.
    pub fn window_at_pointer(
        monitor: LensOuterRect,
        size: (u32, u32),
        pointer: (i32, i32),
        grab: (i32, i32),
    ) -> (i32, i32) {
        let clamp = |v: i32, lo: i32, len: u32, w: u32| v.min(lo + len as i32 - w as i32).max(lo);
        (
            clamp(pointer.0 - grab.0, monitor.x, monitor.w, size.0),
            clamp(pointer.1 - grab.1, monitor.y, monitor.h, size.1),
        )
    }

    /// Top-left corner for a `size` window opened from the window at
    /// `source`: shifted by `step` right and down so both headers stay
    /// reachable, kept inside `monitor`. Where that lands on the source
    /// again (it sits in the bottom-right corner), the window goes to
    /// the monitor's top-left instead.
    pub fn window_beside(
        monitor: LensOuterRect,
        size: (u32, u32),
        source: (i32, i32),
        step: i32,
    ) -> (i32, i32) {
        let at = window_at_pointer(monitor, size, (source.0 + step, source.1 + step), (0, 0));
        if at == source {
            return window_at_pointer(monitor, size, (monitor.x + step, monitor.y + step), (0, 0));
        }
        at
    }

    /// Whether a pointer at (`px`, `py`) should reach the lens window
    /// rather than the app underneath: on the header, on the resize band
    /// along the edges (including the frame outside the client area) or
    /// on the resize grip. `outer` is the window frame, `inner` the
    /// client area; `header`, `edge` and `grip` are physical pixels.
    pub fn hits_controls(
        outer: LensOuterRect,
        inner: LensOuterRect,
        header: u32,
        edge: u32,
        grip: u32,
        px: i32,
        py: i32,
    ) -> bool {
        let inside = |r: LensOuterRect, m: i32| {
            px >= r.x + m && py >= r.y + m && px < r.x + r.w as i32 - m && py < r.y + r.h as i32 - m
        };
        let frame = LensOuterRect {
            x: outer.x.min(inner.x),
            y: outer.y.min(inner.y),
            w: outer.w.max(inner.w),
            h: outer.h.max(inner.h),
        };
        if !inside(frame, 0) && !inside(inner, 0) {
            return false;
        }
        let right = inner.x + inner.w as i32;
        let bottom = inner.y + inner.h as i32;
        let on_grip = px >= right - grip as i32 && py >= bottom - grip as i32;
        py < inner.y + header as i32 || !inside(inner, edge as i32) || on_grip
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn insets_trim_the_header() {
            let outer = LensOuterRect {
                x: 100,
                y: 200,
                w: 400,
                h: 300,
            };
            let insets = LensInsets {
                top: 32,
                bottom: 0,
                left: 0,
                right: 0,
            };
            let observed = observed_rect(outer, insets).unwrap();
            assert_eq!(
                observed,
                LensOuterRect {
                    x: 100,
                    y: 232,
                    w: 400,
                    h: 268
                }
            );
        }

        #[test]
        fn header_goes_above_the_region() {
            let region = LensOuterRect {
                x: 100,
                y: 300,
                w: 400,
                h: 200,
            };
            assert_eq!(
                window_for_region(region, 34, 0),
                LensOuterRect {
                    x: 100,
                    y: 266,
                    w: 400,
                    h: 234
                }
            );
        }

        #[test]
        fn header_covers_the_top_at_the_monitor_edge() {
            // Whole screen: no room above, the header overlaps the top.
            let region = LensOuterRect {
                x: 0,
                y: 0,
                w: 1920,
                h: 1080,
            };
            assert_eq!(window_for_region(region, 34, 0), region);
            // Partly room above.
            let region = LensOuterRect {
                x: 0,
                y: 10,
                w: 300,
                h: 100,
            };
            assert_eq!(
                window_for_region(region, 34, 0),
                LensOuterRect {
                    x: 0,
                    y: 0,
                    w: 300,
                    h: 110
                }
            );
        }

        #[test]
        fn controls_take_the_pointer_only_on_header_edges_and_grip() {
            let inner = LensOuterRect {
                x: 100,
                y: 100,
                w: 400,
                h: 300,
            };
            let outer = LensOuterRect {
                x: 92,
                y: 100,
                w: 416,
                h: 308,
            };
            let hit = |x, y| hits_controls(outer, inner, 34, 6, 18, x, y);
            assert!(hit(300, 110), "header");
            assert!(!hit(300, 250), "body lets the pointer through");
            assert!(hit(102, 250), "left resize band");
            assert!(hit(95, 250), "invisible frame outside the client area");
            assert!(hit(490, 390), "resize grip");
            assert!(!hit(50, 50), "outside the window");
        }

        #[test]
        fn insets_that_consume_the_window_return_none() {
            let outer = LensOuterRect {
                x: 0,
                y: 0,
                w: 100,
                h: 32,
            };
            let insets = LensInsets {
                top: 32,
                bottom: 0,
                left: 0,
                right: 0,
            };
            assert!(observed_rect(outer, insets).is_none());
        }

        #[test]
        fn insets_larger_than_the_window_do_not_panic() {
            let outer = LensOuterRect {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            };
            let insets = LensInsets {
                top: 200,
                bottom: 200,
                left: 200,
                right: 200,
            };
            // saturating_sub keeps the values in bounds; result is None.
            assert!(observed_rect(outer, insets).is_none());
        }
    }
}

pub mod ids {
    //! Lens window id generation. We prefer short collision-resistant ids
    //! so the window label stays short (Tauri uses the label as a DOM
    //! lookup key).

    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Generate a new lens id like `lens-<ms>-<seq>`. Monotonic per
    /// process so two calls in the same millisecond still produce
    /// distinct ids.
    pub fn next() -> String {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("lens-{ms:x}-{seq:x}")
    }

    /// Extract the sequence number from a generated id. Used by tests
    /// to verify monotonicity.
    #[cfg(test)]
    pub fn seq_of(id: &str) -> Option<u64> {
        let tail = id.rsplit('-').next()?;
        u64::from_str_radix(tail, 16).ok()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn generated_ids_are_unique_and_monotonic_within_a_batch() {
            let ids: Vec<String> = (0..16).map(|_| next()).collect();
            let seqs: Vec<u64> = ids.iter().filter_map(|id| seq_of(id)).collect();
            assert_eq!(seqs.len(), ids.len());
            for pair in seqs.windows(2) {
                assert!(pair[0] < pair[1], "ids must be strictly monotonic");
            }
        }

        #[test]
        fn ids_start_with_lens_prefix() {
            let id = next();
            assert!(id.starts_with("lens-"));
        }
    }
}

/// Descriptor returned to the UI so it can render a list of lens windows.
#[derive(Debug, Clone, Serialize)]
pub struct LensDescriptor {
    pub id: String,
    pub title: String,
    pub insets: geometry::LensInsets,
    /// Reads the region continuously (see [`set_continuous`]).
    pub continuous: bool,
    /// Language it translates into; `None` = the configured target.
    pub target_language: Option<String>,
}

struct LensEntry {
    insets: geometry::LensInsets,
    watch: Option<Watch>,
    /// Translate into this language; `None` = the configured target.
    target_language: Option<String>,
    /// The renderer shows controls over the body (text view): the body
    /// takes the mouse too.
    interactive: bool,
    /// Operate mode as the user set it (button or hotkey). Separate from
    /// `interactive`, which the text view also turns on.
    operate: bool,
    /// Text view shown outside the window (see [`crate::lens_text`]).
    text: Option<Follow>,
}

/// Where a lens opens: the range to observe and the language to show.
#[derive(Debug, Clone)]
pub struct Placement {
    /// Monitor-relative physical pixels, like the selector's crops.
    pub rect: CaptureRect,
    pub target_language: Option<String>,
}

/// A running continuous reading of one lens.
struct Watch {
    handle: JoinHandle<()>,
    stop: Arc<tokio::sync::Notify>,
    /// Read now, even when nothing changed (manual refresh).
    kick: Arc<tokio::sync::Notify>,
}

impl Watch {
    fn stop(self) {
        self.stop.notify_one();
        self.handle.abort();
    }
}

#[derive(Default)]
pub struct LensRegistry {
    entries: Mutex<Vec<(String, LensEntry)>>,
}

impl LensRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list(&self) -> Vec<LensDescriptor> {
        self.entries
            .lock()
            .iter()
            .map(|(id, e)| LensDescriptor {
                id: id.clone(),
                title: format!("ウィンドウ翻訳 {}", short(id)),
                insets: e.insets,
                continuous: e.watch.is_some(),
                target_language: e.target_language.clone(),
            })
            .collect()
    }

    pub fn track(&self, id: String) {
        self.track_with(id, None);
    }

    fn track_with(&self, id: String, target_language: Option<String>) {
        self.entries.lock().push((
            id,
            LensEntry {
                // The renderer reports its header once loaded; until then
                // assume the fixed one, or the first reading would
                // include the strip under the header.
                insets: geometry::LensInsets {
                    top: HEADER_CSS,
                    ..Default::default()
                },
                watch: None,
                target_language,
                interactive: false,
                operate: false,
                text: None,
            },
        ));
    }

    pub fn target_language(&self, id: &str) -> Option<String> {
        self.entries
            .lock()
            .iter()
            .find(|(i, _)| i == id)
            .and_then(|(_, e)| e.target_language.clone())
    }

    pub fn set_target_language(&self, id: &str, lang: Option<String>) {
        if let Some((_, e)) = self.entries.lock().iter_mut().find(|(i, _)| i == id) {
            e.target_language = lang;
        }
    }

    pub fn set_interactive(&self, id: &str, on: bool) {
        if let Some((_, e)) = self.entries.lock().iter_mut().find(|(i, _)| i == id) {
            e.interactive = on;
        }
    }

    /// Record a lens's operate mode (see [`toggle_operate`]).
    pub fn set_operate(&self, id: &str, on: bool) {
        if let Some((_, e)) = self.entries.lock().iter_mut().find(|(i, _)| i == id) {
            e.operate = on;
        }
    }

    /// Whether any lens is in operate mode.
    pub fn any_operate(&self) -> bool {
        self.entries.lock().iter().any(|(_, e)| e.operate)
    }

    /// `None` for a lens that is gone.
    fn interactive(&self, id: &str) -> Option<bool> {
        self.entries
            .lock()
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, e)| e.interactive)
    }

    /// Set or drop the placement of the lens's outside text view.
    pub fn set_text(&self, id: &str, follow: Option<Follow>) {
        if let Some((_, e)) = self.entries.lock().iter_mut().find(|(i, _)| i == id) {
            e.text = follow;
        }
    }

    /// Run `f` on the placement of the lens's outside text view. `None`
    /// without one.
    pub fn with_text<R>(&self, id: &str, f: impl FnOnce(&mut Follow) -> R) -> Option<R> {
        let mut entries = self.entries.lock();
        let (_, e) = entries.iter_mut().find(|(i, _)| i == id)?;
        e.text.as_mut().map(f)
    }

    pub fn untrack(&self, id: &str) {
        self.set_watch(id, None);
        self.entries.lock().retain(|(i, _)| i != id);
    }

    /// Replace the lens's continuous reading (stopping the old one).
    /// Returns false for an unknown lens; `watch` is stopped then.
    fn set_watch(&self, id: &str, watch: Option<Watch>) -> bool {
        let mut entries = self.entries.lock();
        let Some((_, e)) = entries.iter_mut().find(|(i, _)| i == id) else {
            drop(entries);
            if let Some(w) = watch {
                w.stop();
            }
            return false;
        };
        let old = std::mem::replace(&mut e.watch, watch);
        drop(entries);
        if let Some(w) = old {
            w.stop();
        }
        true
    }

    /// Ask a continuous reading to read now. False when the lens is not
    /// reading continuously.
    pub fn kick(&self, id: &str) -> bool {
        let entries = self.entries.lock();
        match entries
            .iter()
            .find(|(i, _)| i == id)
            .and_then(|(_, e)| e.watch.as_ref())
        {
            Some(w) => {
                w.kick.notify_one();
                true
            }
            None => false,
        }
    }

    pub fn set_insets(&self, id: &str, insets: geometry::LensInsets) {
        for (i, e) in self.entries.lock().iter_mut() {
            if i == id {
                e.insets = insets;
                return;
            }
        }
    }

    pub fn insets(&self, id: &str) -> Option<geometry::LensInsets> {
        self.entries
            .lock()
            .iter()
            .find(|(i, _)| i == id)
            .map(|(_, e)| e.insets)
    }
}

pub type LensRegistryHandle = Arc<LensRegistry>;

/// Open a lens window over `placement` (or at a default spot), register
/// it and start letting the mouse through its body.
pub fn create(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    placement: Option<Placement>,
    beside: Option<&str>,
) -> Result<String> {
    let id = ids::next();
    let label = id.clone();
    let url = format!("/lens/?id={id}");
    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title(format!("MoonLens {}", short(&id)))
        .inner_size(460.0, 320.0)
        .position(200.0, 200.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        // Keep the lens UI out of its own captures. Hiding the window
        // instead does not work on Windows: `hide()` returns before DWM
        // has faded the window out, so the grab read the lens itself
        // (its own Japanese placeholder) and nothing got translated.
        .content_protected(EXCLUDED_FROM_CAPTURE)
        .visible(false)
        .build()
        .context("create lens window")?;
    let source = beside.and_then(|b| app.get_webview_window(b));
    let target_language = match &placement {
        Some(p) => p.target_language.clone(),
        // Opened from a window: the same language as that window.
        None => beside.and_then(|b| registry.target_language(b)),
    };
    let placed = match (&placement, &source) {
        (Some(p), _) => place(&win, &p.rect),
        (None, Some(source)) => place_beside(&win, source),
        (None, None) => place_at_pointer(app, &win),
    };
    if let Err(e) = placed {
        tracing::warn!("placing lens window: {e:#}");
    }
    win.show().ok();
    let _ = win.set_always_on_top(true);
    registry.track_with(id.clone(), target_language);
    tauri::async_runtime::spawn(run_passthrough(app.clone(), registry.clone(), id.clone()));
    Ok(id)
}

/// Move and size `win` so that its body covers `rect` (monitor-relative
/// physical pixels) and its header sits right above it.
fn place(win: &WebviewWindow, rect: &CaptureRect) -> Result<()> {
    let monitors = xcap::Monitor::all()?;
    let m = monitors
        .get(rect.monitor_index)
        .ok_or_else(|| anyhow!("monitor {} missing", rect.monitor_index))?;
    let (mx, my) = (m.x()?, m.y()?);
    let scale = m.scale_factor().unwrap_or(1.0) as f64;
    let header = (HEADER_CSS as f64 * scale).ceil() as u32;
    let region = geometry::LensOuterRect {
        x: mx + rect.x,
        y: my + rect.y,
        w: rect.w,
        h: rect.h,
    };
    let want = geometry::window_for_region(region, header, my);
    win.set_size(PhysicalSize::new(want.w, want.h))?;
    win.set_position(PhysicalPosition::new(want.x, want.y))?;
    // `set_position` places the outer frame; a frameless window on
    // Windows 11 has an invisible border around its client area. Shift
    // by the difference so the client area lands on the range.
    if let (Ok(inner), Ok(outer)) = (win.inner_position(), win.outer_position()) {
        let (dx, dy) = (inner.x - outer.x, inner.y - outer.y);
        if dx != 0 || dy != 0 {
            win.set_position(PhysicalPosition::new(want.x - dx, want.y - dy))?;
        }
    }
    Ok(())
}

/// Open one more window-translation window under the mouse pointer,
/// off the caller's thread (tray, hotkeys).
pub fn open_new(app: &AppHandle) {
    let app = app.clone();
    let main = app.clone();
    let _ = main.run_on_main_thread(move || {
        let state = app.state::<crate::app::AppState>();
        if let Err(e) = create(&app, &state.lenses, None, None) {
            tracing::warn!("opening lens window: {e:#}");
        }
    });
}

/// Toggle hotkey: open a window when there is none, otherwise close them
/// all. Returns whether a window is open afterwards.
pub fn toggle(app: &AppHandle, registry: &LensRegistryHandle) -> bool {
    let ids: Vec<String> = registry.list().into_iter().map(|d| d.id).collect();
    if ids.is_empty() {
        open_new(app);
        return true;
    }
    for id in ids {
        let _ = close(app, registry, &id);
    }
    false
}

/// Operate-mode hotkey: when any window is in operate mode, switch all of
/// them back to letting the mouse through; otherwise put every window in
/// operate mode (to select and copy translations). Each window applies
/// the mode itself ([`LENS_OPERATE_EVENT`]) and reports the resulting
/// mouse handling (which an open text view also affects). Returns the
/// new mode.
pub fn toggle_operate(app: &AppHandle, registry: &LensRegistryHandle) -> bool {
    let operate = !registry.any_operate();
    for d in registry.list() {
        let _ = app.emit_to(
            tauri::EventTarget::WebviewWindow {
                label: d.id.clone(),
            },
            LENS_OPERATE_EVENT,
            operate,
        );
        // Right away, so a second press in quick succession sees it.
        registry.set_operate(&d.id, operate);
    }
    operate
}

/// Put a new window's header under the mouse pointer (default size),
/// kept on the pointer's monitor.
fn place_at_pointer(app: &AppHandle, win: &WebviewWindow) -> Result<()> {
    let Some((px, py)) = pointer_position(app) else {
        return Ok(());
    };
    let monitor = win.available_monitors()?.into_iter().find(|m| {
        let (p, s) = (m.position(), m.size());
        px >= p.x && py >= p.y && px < p.x + s.width as i32 && py < p.y + s.height as i32
    });
    let Some(m) = monitor else {
        return Ok(());
    };
    let size = win.outer_size()?;
    let scale = m.scale_factor();
    let want = geometry::window_at_pointer(
        geometry::LensOuterRect {
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
        },
        (size.width, size.height),
        (px, py),
        (
            (40.0 * scale) as i32,
            (HEADER_CSS as f64 * scale / 2.0) as i32,
        ),
    );
    win.set_position(PhysicalPosition::new(want.0, want.1))?;
    Ok(())
}

/// Put a new window next to `source` (the window it was opened from):
/// the same size, one header height right and down. A maximized source
/// keeps the default size.
fn place_beside(win: &WebviewWindow, source: &WebviewWindow) -> Result<()> {
    let at = source.outer_position()?;
    let Some(m) = source.current_monitor()? else {
        return Ok(());
    };
    if !source.is_maximized().unwrap_or(false) {
        win.set_size(source.inner_size()?)?;
    }
    let size = win.outer_size()?;
    let want = geometry::window_beside(
        geometry::LensOuterRect {
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
        },
        (size.width, size.height),
        (at.x, at.y),
        (HEADER_CSS as f64 * m.scale_factor()).ceil() as i32,
    );
    win.set_position(PhysicalPosition::new(want.0, want.1))?;
    Ok(())
}

/// Let the mouse through the lens body to the app underneath, but keep
/// the header, the resize edges and the grip clickable. Windows can only
/// ignore the cursor as a whole, so the pointer position decides. The
/// same poll keeps an outside text view next to the lens.
async fn run_passthrough(app: AppHandle, registry: LensRegistryHandle, id: String) {
    let mut applied: Option<bool> = None;
    let mut since_apply = 0u32;
    loop {
        tokio::time::sleep(Duration::from_millis(PASSTHROUGH_POLL_MS)).await;
        let Some(win) = app.get_webview_window(&id) else {
            break;
        };
        let Some(body_interactive) = registry.interactive(&id) else {
            break;
        };
        lens_text::follow(&app, &registry, &id, &win);
        let take = body_interactive || pointer_on_controls(&app, &win, &registry, &id);
        since_apply += 1;
        // Re-apply now and then: hiding and showing the window for a
        // capture (Linux) can reset it.
        if applied != Some(take) || since_apply >= 25 {
            if win.set_ignore_cursor_events(!take).is_ok() {
                applied = Some(take);
            }
            since_apply = 0;
        }
    }
    // The lens is gone, however it was closed: so is its text view.
    lens_text::close(&app, &id);
}

fn pointer_on_controls(
    app: &AppHandle,
    win: &WebviewWindow,
    registry: &LensRegistryHandle,
    id: &str,
) -> bool {
    let (Some(p), Ok(inner), Ok(pos), Ok(size)) = (
        pointer_position(app),
        inner_rect(win),
        win.outer_position(),
        win.outer_size(),
    ) else {
        // Unknown: keep the window usable rather than unreachable.
        return true;
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    let outer = geometry::LensOuterRect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    };
    let header = scale_insets(registry.insets(id).unwrap_or_default(), scale).top;
    let px = |v: f64| (v * scale).ceil() as u32;
    geometry::hits_controls(outer, inner, header, px(EDGE_CSS), px(GRIP_CSS), p.0, p.1)
}

/// Pointer position on the screen (physical pixels).
#[cfg(not(target_os = "linux"))]
fn pointer_position(app: &AppHandle) -> Option<(i32, i32)> {
    let p = app.cursor_position().ok()?;
    Some((p.x.floor() as i32, p.y.floor() as i32))
}

/// Pointer position on the screen (physical pixels). On Linux tao reports
/// the last position its own windows saw, which goes stale as soon as a
/// window lets the mouse through, so ask the X server (Wayland sessions
/// without X fall back to tao).
#[cfg(target_os = "linux")]
fn pointer_position(app: &AppHandle) -> Option<(i32, i32)> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;
    use x11rb::rust_connection::RustConnection;

    static X: Mutex<Option<(RustConnection, u32)>> = Mutex::new(None);
    let mut x = X.lock();
    if x.is_none() {
        *x = x11rb::connect(None).ok().map(|(c, screen)| {
            let root = c.setup().roots[screen].root;
            (c, root)
        });
    }
    let queried = x
        .as_ref()
        .and_then(|(c, root)| c.query_pointer(*root).ok()?.reply().ok());
    match queried {
        Some(r) => Some((r.root_x as i32, r.root_y as i32)),
        None => {
            *x = None;
            let p = app.cursor_position().ok()?;
            Some((p.x.floor() as i32, p.y.floor() as i32))
        }
    }
}

pub fn close(app: &AppHandle, registry: &LensRegistryHandle, id: &str) -> Result<()> {
    lens_text::close(app, id);
    if let Some(win) = app.get_webview_window(id) {
        win.close().ok();
    }
    registry.untrack(id);
    Ok(())
}

/// Resolve the client (webview) rect of a lens window in physical
/// coords. Not the outer rect: on Windows 11 a frameless window's outer
/// rect includes an invisible resize border, which shifted the captured
/// region against the header inset the renderer reports.
pub fn inner_rect(win: &WebviewWindow) -> Result<geometry::LensOuterRect> {
    let pos = win.inner_position()?;
    let size = win.inner_size()?;
    Ok(geometry::LensOuterRect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    })
}

/// Platforms where content protection removes the window from screen
/// captures (Windows 10 2004+: `WDA_EXCLUDEFROMCAPTURE`; macOS:
/// `NSWindowSharingNone`). Elsewhere the window is hidden for the grab.
pub const EXCLUDED_FROM_CAPTURE: bool = cfg!(any(target_os = "windows", target_os = "macos"));

/// Convert the renderer's CSS-pixel insets to physical pixels. The outer
/// rect is physical, so at 150 % scaling unscaled insets left part of the
/// header inside the captured region.
pub fn scale_insets(insets: geometry::LensInsets, scale: f64) -> geometry::LensInsets {
    let s = |v: u32| (v as f64 * scale).ceil() as u32;
    geometry::LensInsets {
        top: s(insets.top),
        bottom: s(insets.bottom),
        left: s(insets.left),
        right: s(insets.right),
    }
}

/// Screen rect the lens observes now (physical pixels): the client
/// area minus the renderer's header.
pub fn observed(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    id: &str,
) -> Result<geometry::LensOuterRect> {
    let win = app
        .get_webview_window(id)
        .ok_or_else(|| anyhow!("lens window {id} not found"))?;
    let outer = inner_rect(&win)?;
    let scale = win.scale_factor().unwrap_or(1.0);
    let insets = scale_insets(registry.insets(id).unwrap_or_default(), scale);
    geometry::observed_rect(outer, insets).ok_or_else(|| anyhow!("lens observation rect is empty"))
}

fn as_geometry(r: geometry::LensOuterRect) -> Geometry {
    [r.x, r.y, r.w as i32, r.h as i32]
}

/// Capture the observed rect without the lens window in the frame.
pub fn grab(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    id: &str,
) -> Result<(DynamicImage, geometry::LensOuterRect)> {
    use crate::capture;

    let observed = observed(app, registry, id)?;
    if EXCLUDED_FROM_CAPTURE {
        let image = capture::capture_global_rect(observed.x, observed.y, observed.w, observed.h)?;
        return Ok((image, observed));
    }
    let win = app
        .get_webview_window(id)
        .ok_or_else(|| anyhow!("lens window {id} not found"))?;
    // No content protection: hide the window for the moment of the grab,
    // with a brief pause so the compositor unmaps it first.
    win.hide().ok();
    std::thread::sleep(std::time::Duration::from_millis(60));
    let result = capture::capture_global_rect(observed.x, observed.y, observed.w, observed.h);
    win.show().ok();
    Ok((result?, observed))
}

/// Compute the observed rect and capture it without the lens window in
/// the frame. Returns the captured PNG.
pub fn capture_lens(app: &AppHandle, registry: &LensRegistryHandle, id: &str) -> Result<Vec<u8>> {
    let (image, _) = grab(app, registry, id)?;
    crate::capture::png_bytes(&image)
}

/// Switch continuous reading of a lens on or off.
pub fn set_continuous(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    pipeline: Arc<Pipeline>,
    settings: Arc<SettingsStore>,
    id: &str,
    on: bool,
) {
    if !on {
        registry.set_watch(id, None);
        return;
    }
    let stop = Arc::new(tokio::sync::Notify::new());
    let kick = Arc::new(tokio::sync::Notify::new());
    let handle = tauri::async_runtime::spawn(run_watch(
        app.clone(),
        registry.clone(),
        pipeline,
        settings,
        id.to_string(),
        stop.clone(),
        kick.clone(),
    ));
    registry.set_watch(id, Some(Watch { handle, stop, kick }));
}

/// Continuous reading of one lens: every `region_watcher_poll_ms` (or on
/// a kick) check where the lens is, and once it holds still read the
/// region through the shared [`Tracker`].
async fn run_watch(
    app: AppHandle,
    registry: LensRegistryHandle,
    pipeline: Arc<Pipeline>,
    settings: Arc<SettingsStore>,
    id: String,
    stop: Arc<tokio::sync::Notify>,
    kick: Arc<tokio::sync::Notify>,
) {
    let target = tauri::EventTarget::WebviewWindow { label: id.clone() };
    let emit_app = app.clone();
    let emit_target = target.clone();
    let emit = move |ev: LiveEvent| {
        let _ = emit_app.emit_to(emit_target.clone(), LENS_LIVE_EVENT, ev);
    };
    let (geo_app, geo_registry, geo_id) = (app.clone(), registry.clone(), id.clone());
    let geometry_now = move || {
        observed(&geo_app, &geo_registry, &geo_id)
            .ok()
            .map(as_geometry)
    };

    let mut tracker = Tracker::new();
    let mut last: Option<Geometry> = None;
    let mut gate = DragGate::default();
    // The first reading happens at once, as on a manual refresh.
    let mut first = true;
    // New text waits to be confirmed and reading it took an interval
    // or longer: the screen had that long to change, so the confirming
    // capture follows without another pause. Only once in a row, so a
    // region that never settles is not read back to back.
    let mut confirm_now = false;
    loop {
        let mut manual = std::mem::take(&mut first);
        let confirming = std::mem::take(&mut confirm_now) && EXCLUDED_FROM_CAPTURE;
        let mut interval = settings
            .snapshot()
            .performance
            .region_watcher_poll_ms
            .max(50) as u64;
        if !EXCLUDED_FROM_CAPTURE {
            interval = interval.max(HIDDEN_CAPTURE_MIN_MS);
        }
        if !manual {
            // Mid-drag nothing is captured: poll fast to catch the drop.
            let pause = if gate.moving() {
                lens_drag::MOVING_POLL_MS
            } else if confirming {
                0
            } else {
                interval
            };
            tokio::select! {
                _ = stop.notified() => break,
                _ = kick.notified() => manual = true,
                _ = tokio::time::sleep(Duration::from_millis(pause)) => {}
            }
        }
        let target_language = registry
            .target_language(&id)
            .unwrap_or_else(|| settings.snapshot().subtitle.target_language.clone());
        let env = TickEnv {
            pipeline: &pipeline,
            target_language: &target_language,
            history_kind: crate::pipeline::history_kind::LENS,
            emit: &emit,
            geometry_now: &geometry_now,
        };

        // Where the lens is now. No capture while it is being dragged
        // or resized (and no hiding it mid-drag where that is needed):
        // reading waits for the drop, however long the hand pauses.
        let now = match observed(&app, &registry, &id) {
            Ok(r) => as_geometry(r),
            Err(_) if app.get_webview_window(&id).is_none() => break,
            Err(_) => continue, // collapsed to the header
        };
        let moved = last.is_some_and(|g| g != now);
        last = Some(now);
        if moved {
            tracker.region_moved(now, &env);
        }
        let was_moving = gate.moving();
        if !gate.allow(moved, lens_drag::button_down()) {
            continue;
        }
        // Dropped: read the new spot at once, like a manual refresh.
        manual |= was_moving;

        let (a, r, i) = (app.clone(), registry.clone(), id.clone());
        let started = std::time::Instant::now();
        let grabbed = tokio::task::spawn_blocking(move || grab(&a, &r, &i)).await;
        let capture_ms = started.elapsed().as_millis();
        let (image, rect) = match grabbed {
            Ok(Ok(g)) => g,
            Ok(Err(e)) => {
                tracing::debug!("lens {id} capture: {e}");
                continue;
            }
            Err(_) => break,
        };
        let frame = Frame {
            image,
            geometry: as_geometry(rect),
        };
        let (width, height) = (frame.image.width(), frame.image.height());
        let started = std::time::Instant::now();
        let ticked = tracker.tick(frame, manual, &env).await;
        let tick = started.elapsed();
        if !matches!(ticked, Ok(Tick::Unchanged | Tick::Moved)) {
            // Where a reading's time went, next to the OCR and
            // translation lines of the pipeline.
            tracing::info!(
                "window translation {width}x{height}: capture {capture_ms} ms, reading {} ms ({})",
                tick.as_millis(),
                match &ticked {
                    Ok(Tick::Settling) => "new text, confirming",
                    Ok(Tick::SameText) => "same text",
                    Ok(Tick::Empty) => "no text",
                    Ok(Tick::Decoded { .. }) => "QR code",
                    Ok(Tick::Translated {
                        requested: true, ..
                    }) => "translated",
                    Ok(Tick::Translated { .. }) => "shown without a request",
                    _ => "failed",
                }
            );
        }
        confirm_now = !confirming
            && matches!(ticked, Ok(Tick::Settling))
            && tick >= Duration::from_millis(interval);
        match ticked {
            Ok(Tick::Translated {
                source,
                translation,
                ..
            }) => {
                // The clipboard policy of the regular capture flow, once
                // per new text (not on every reading).
                let cfg = settings.snapshot().clipboard.clone();
                crate::clipboard::auto_copy(&app, crate::clipboard::Kind::Source, &source, &cfg);
                crate::clipboard::auto_copy(
                    &app,
                    crate::clipboard::Kind::Translation,
                    &translation,
                    &cfg,
                );
            }
            Ok(Tick::Decoded { text }) => {
                let cfg = settings.snapshot().clipboard.clone();
                crate::clipboard::auto_copy(&app, crate::clipboard::Kind::Qr, &text, &cfg);
            }
            Ok(_) => {}
            Err(message) => {
                tracing::debug!("lens {id}: {message}");
                let _ = app.emit_to(
                    target.clone(),
                    LENS_LIVE_EVENT,
                    LiveEvent::Error { message },
                );
            }
        }
    }
}

/// Update the layout insets reported by the lens renderer. The renderer
/// calls this any time the header height changes (e.g. when the user
/// hides the toolbar).
pub fn set_insets(registry: &LensRegistryHandle, id: &str, insets: geometry::LensInsets) {
    registry.set_insets(id, insets);
}

fn short(id: &str) -> String {
    // Pull the trailing sequence segment for human-friendly display.
    id.rsplit('-').next().unwrap_or(id).to_string()
}

/// Convenience helper for test entry points.
#[cfg(test)]
pub(crate) fn new_registry_for_test() -> LensRegistryHandle {
    Arc::new(LensRegistry::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use geometry::LensInsets;

    #[test]
    fn insets_scale_to_physical_pixels() {
        let css = LensInsets {
            top: 33,
            bottom: 0,
            left: 1,
            right: 2,
        };
        assert_eq!(scale_insets(css, 1.0), css);
        assert_eq!(
            scale_insets(css, 1.5),
            LensInsets {
                top: 50,
                bottom: 0,
                left: 2,
                right: 3
            }
        );
    }

    #[test]
    fn track_untrack_round_trip() {
        let r = new_registry_for_test();
        r.track("lens-1".into());
        r.track("lens-2".into());
        assert_eq!(r.list().len(), 2);
        r.untrack("lens-1");
        let remaining: Vec<String> = r.list().into_iter().map(|d| d.id).collect();
        assert_eq!(remaining, vec!["lens-2".to_string()]);
    }

    #[test]
    fn insets_are_remembered_per_lens() {
        let r = new_registry_for_test();
        r.track("a".into());
        r.track("b".into());
        r.set_insets(
            "a",
            LensInsets {
                top: 30,
                ..Default::default()
            },
        );
        assert_eq!(r.insets("a").unwrap().top, 30);
        // Untouched: the fixed header until the renderer reports.
        assert_eq!(r.insets("b").unwrap().top, HEADER_CSS);
    }

    #[test]
    fn lenses_start_without_continuous_reading() {
        // The renderer switches it on; a lens the renderer never reached
        // (dev server down) must not read the screen on its own.
        let r = new_registry_for_test();
        r.track("a".into());
        assert!(!r.list()[0].continuous);
        assert!(!r.kick("a"));
        assert!(!r.kick("missing"));
    }

    #[test]
    fn per_lens_language_and_interaction() {
        let r = new_registry_for_test();
        r.track_with("a".into(), Some("en".into()));
        r.track("b".into());
        assert_eq!(r.target_language("a").as_deref(), Some("en"));
        assert_eq!(r.target_language("b"), None);
        r.set_target_language("b", Some("ko".into()));
        assert_eq!(r.list()[1].target_language.as_deref(), Some("ko"));
        // Every window lets the mouse through until it is operated.
        assert_eq!(r.interactive("a"), Some(false));
        assert!(!r.any_operate());
        // The text view takes the mouse without being operate mode.
        r.set_interactive("a", true);
        assert_eq!(r.interactive("a"), Some(true));
        assert!(!r.any_operate());
        r.set_operate("b", true);
        assert!(r.any_operate());
        assert_eq!(r.interactive("b"), Some(false));
        // Gone: the click-through loop stops.
        r.untrack("a");
        assert_eq!(r.interactive("a"), None);
    }

    #[test]
    fn a_new_window_opens_under_the_pointer_inside_the_monitor() {
        let m = geometry::LensOuterRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        assert_eq!(
            geometry::window_at_pointer(m, (460, 320), (500, 400), (40, 17)),
            (460, 383)
        );
        // Near the bottom-right corner: pushed back onto the monitor.
        assert_eq!(
            geometry::window_at_pointer(m, (460, 320), (1910, 1070), (40, 17)),
            (1460, 760)
        );
        // Second monitor to the left (negative origin).
        let left = geometry::LensOuterRect {
            x: -1280,
            y: 0,
            w: 1280,
            h: 1024,
        };
        assert_eq!(
            geometry::window_at_pointer(left, (460, 320), (-1275, 5), (40, 17)),
            (-1280, 0)
        );
    }

    #[test]
    fn a_window_opened_from_a_window_is_shifted_and_stays_on_the_monitor() {
        let m = geometry::LensOuterRect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        };
        assert_eq!(
            geometry::window_beside(m, (460, 320), (500, 400), 34),
            (534, 434)
        );
        // At the right edge: only moves down.
        assert_eq!(
            geometry::window_beside(m, (460, 320), (1460, 400), 34),
            (1460, 434)
        );
        // In the bottom-right corner: starts over at the top-left.
        assert_eq!(
            geometry::window_beside(m, (460, 320), (1460, 760), 34),
            (34, 34)
        );
    }

    #[test]
    fn unknown_id_returns_none_insets() {
        let r = new_registry_for_test();
        assert!(r.insets("missing").is_none());
    }

    #[test]
    fn short_strips_everything_before_the_last_dash() {
        assert_eq!(short("lens-ffee-7"), "7");
    }

    #[test]
    fn short_returns_id_verbatim_if_no_dash() {
        // Lens ids always have dashes in production, but we want the
        // helper to be defensive for robustness under weird inputs.
        assert_eq!(short("noseparator"), "noseparator");
    }

    #[test]
    fn list_preserves_tracking_order() {
        let r = new_registry_for_test();
        r.track("a".into());
        r.track("b".into());
        r.track("c".into());
        let ids: Vec<String> = r.list().into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn setting_insets_on_missing_id_is_a_noop() {
        // Should not panic and should not create an entry.
        let r = new_registry_for_test();
        r.set_insets(
            "missing",
            LensInsets {
                top: 10,
                ..Default::default()
            },
        );
        assert_eq!(r.list().len(), 0);
    }

    #[test]
    fn untracking_missing_id_is_a_noop() {
        let r = new_registry_for_test();
        r.untrack("missing"); // must not panic
        r.track("a".into());
        r.untrack("a");
        r.untrack("a"); // twice in a row is fine
        assert!(r.list().is_empty());
    }

    #[test]
    fn observed_rect_with_zero_size_outer_returns_none() {
        let outer = geometry::LensOuterRect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        };
        assert!(geometry::observed_rect(outer, geometry::LensInsets::default()).is_none());
    }

    #[test]
    fn observed_rect_with_zero_insets_equals_outer() {
        let outer = geometry::LensOuterRect {
            x: 10,
            y: 20,
            w: 300,
            h: 200,
        };
        let observed = geometry::observed_rect(outer, geometry::LensInsets::default()).unwrap();
        assert_eq!(observed, outer);
    }

    #[test]
    fn observed_rect_trims_all_four_sides() {
        let outer = geometry::LensOuterRect {
            x: 0,
            y: 0,
            w: 400,
            h: 300,
        };
        let insets = geometry::LensInsets {
            top: 30,
            bottom: 20,
            left: 10,
            right: 15,
        };
        let observed = geometry::observed_rect(outer, insets).unwrap();
        assert_eq!(
            observed,
            geometry::LensOuterRect {
                x: 10,
                y: 30,
                w: 375,
                h: 250
            }
        );
    }

    #[test]
    fn duplicate_track_does_allow_duplicates_callers_must_dedupe() {
        // Document the current contract: track() is append-only and does
        // NOT dedupe. The caller (app::build()) must ensure unique ids;
        // this test pins the behaviour so a future refactor that adds
        // dedup is a conscious decision, not an accident.
        let r = new_registry_for_test();
        r.track("a".into());
        r.track("a".into());
        assert_eq!(r.list().len(), 2);
    }
}
