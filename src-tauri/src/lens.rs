//! Lens Window mode.
//!
//! A lens window is a frameless, always-on-top, mostly-transparent Tauri
//! window that the user moves and resizes to mark an on-screen region. The
//! region updates automatically whenever the window geometry changes (via
//! the renderer). A session can have many lens windows, each independent.
//!
//! By default a lens reads its region continuously, like a camera lens:
//! [`start_watch`] runs the same change-driven [`Tracker`] as live
//! translation, so OCR runs only when the pixels change and only text not
//! shown yet goes to the model. While the lens is dragged or resized no
//! capture is taken; reading resumes once it holds still. With continuous
//! reading switched off the lens is read on request only.
//!
//! The capture flow:
//! 1. The watcher (or the frontend, on request) captures a given lens id.
//! 2. The captured frame must not include the lens UI. Windows and macOS
//!    exclude the window from screen capture (content protection); on
//!    other platforms Rust hides the window for the moment of the grab.
//! 3. Rust resolves the window's outer bounds, shrinks them by the title
//!    bar / border offsets the renderer tells us about (CSS pixels, scaled
//!    to physical pixels here), and feeds the
//!    resulting rect to [`crate::capture::capture_global_rect`].
//! 4. Pipeline runs as usual; the translation appears in the lens body.
//!
//! The actual Tauri window creation lives in [`create`]; pure geometry
//! and id generation live in [`geometry`] and [`ids`] so they are
//! testable.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use image::DynamicImage;
use parking_lot::Mutex;
use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{
    pipeline::Pipeline,
    region_watcher::{Frame, Geometry, LiveEvent, Tick, TickEnv, Tracker},
    settings::SettingsStore,
};

/// Event with [`LiveEvent`]s for one lens window.
pub const LENS_LIVE_EVENT: &str = "moonlens://lens-live";

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
    /// Reads the region continuously (see [`start_watch`]).
    pub continuous: bool,
}

struct LensEntry {
    insets: geometry::LensInsets,
    watch: Option<Watch>,
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
                title: format!("Lens {}", short(id)),
                insets: e.insets,
                continuous: e.watch.is_some(),
            })
            .collect()
    }

    pub fn track(&self, id: String) {
        self.entries.lock().push((
            id,
            LensEntry {
                insets: geometry::LensInsets::default(),
                watch: None,
            },
        ));
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

/// Create a new lens window at a sensible default position/size and
/// register it in the shared registry.
pub fn create(app: &AppHandle, registry: &LensRegistryHandle) -> Result<String> {
    let id = ids::next();
    let label = id.clone();
    let url = format!("/lens/?id={id}");
    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title(format!("MoonLens Lens {}", short(&id)))
        .inner_size(460.0, 320.0)
        .position(200.0, 200.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        // Keep the lens UI out of its own captures. Hiding the window
        // instead does not work on Windows: `hide()` returns before DWM
        // has faded the window out, so the grab read the lens itself
        // (its own Japanese placeholder) and nothing got translated.
        .content_protected(EXCLUDED_FROM_CAPTURE)
        .visible(true)
        .build()
        .context("create lens window")?;
    // Give the window a fractional opacity fallback for platforms without
    // true transparency (handled in CSS; the flag here makes intent clear).
    let _ = win.set_always_on_top(true);
    registry.track(id.clone());
    Ok(id)
}

pub fn close(app: &AppHandle, registry: &LensRegistryHandle, id: &str) -> Result<()> {
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
const EXCLUDED_FROM_CAPTURE: bool = cfg!(any(target_os = "windows", target_os = "macos"));

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
    // The first reading happens at once, as on a manual refresh.
    let mut first = true;
    loop {
        let mut manual = std::mem::take(&mut first);
        if !manual {
            let mut interval = settings
                .snapshot()
                .performance
                .region_watcher_poll_ms
                .max(50) as u64;
            if !EXCLUDED_FROM_CAPTURE {
                interval = interval.max(HIDDEN_CAPTURE_MIN_MS);
            }
            tokio::select! {
                _ = stop.notified() => break,
                _ = kick.notified() => manual = true,
                _ = tokio::time::sleep(Duration::from_millis(interval)) => {}
            }
        }
        let target_language = settings.snapshot().subtitle.target_language.clone();
        let env = TickEnv {
            pipeline: &pipeline,
            target_language: &target_language,
            history_kind: crate::pipeline::history_kind::LENS,
            emit: &emit,
            geometry_now: &geometry_now,
        };

        // Where the lens is now. No capture while it is being dragged
        // or resized (and no hiding it mid-drag where that is needed).
        let now = match observed(&app, &registry, &id) {
            Ok(r) => as_geometry(r),
            Err(_) if app.get_webview_window(&id).is_none() => break,
            Err(_) => continue, // collapsed to the header
        };
        if last.is_some_and(|g| g != now) {
            last = Some(now);
            tracker.region_moved(now, &env);
            continue;
        }
        last = Some(now);

        let (a, r, i) = (app.clone(), registry.clone(), id.clone());
        let grabbed = tokio::task::spawn_blocking(move || grab(&a, &r, &i)).await;
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
        match tracker.tick(frame, manual, &env).await {
            Ok(Tick::Translated {
                source,
                translation,
                requested: true,
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
        assert_eq!(r.insets("b").unwrap().top, 0);
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
