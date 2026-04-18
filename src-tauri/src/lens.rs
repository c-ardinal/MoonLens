//! Lens Window mode.
//!
//! A lens window is a frameless, always-on-top, mostly-transparent Tauri
//! window that the user moves and resizes to mark an on-screen region. The
//! region updates automatically whenever the window geometry changes (via
//! the renderer) or on an interval configured by the user. A session can
//! have many lens windows, each independent.
//!
//! The capture flow:
//! 1. Frontend (or a change-detection timer) requests a capture for a
//!    given lens id.
//! 2. Rust hides the lens window briefly so the captured frame does not
//!    include the lens UI.
//! 3. Rust resolves the window's outer bounds, shrinks them by the title
//!    bar / border offsets the renderer tells us about, and feeds the
//!    resulting rect to [`crate::capture::capture_global_rect`].
//! 4. Pipeline runs as usual; the translation appears in the lens body.
//!
//! The actual Tauri window creation lives in [`create`]; pure geometry
//! and id generation live in [`geometry`] and [`ids`] so they are
//! testable.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

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
        let w = outer
            .w
            .saturating_sub(insets.left + insets.right);
        let h = outer
            .h
            .saturating_sub(insets.top + insets.bottom);
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
            let outer = LensOuterRect { x: 100, y: 200, w: 400, h: 300 };
            let insets = LensInsets { top: 32, bottom: 0, left: 0, right: 0 };
            let observed = observed_rect(outer, insets).unwrap();
            assert_eq!(observed, LensOuterRect { x: 100, y: 232, w: 400, h: 268 });
        }

        #[test]
        fn insets_that_consume_the_window_return_none() {
            let outer = LensOuterRect { x: 0, y: 0, w: 100, h: 32 };
            let insets = LensInsets { top: 32, bottom: 0, left: 0, right: 0 };
            assert!(observed_rect(outer, insets).is_none());
        }

        #[test]
        fn insets_larger_than_the_window_do_not_panic() {
            let outer = LensOuterRect { x: 0, y: 0, w: 100, h: 100 };
            let insets = LensInsets { top: 200, bottom: 200, left: 200, right: 200 };
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
    pub auto_refresh_ms: Option<u32>,
}

struct LensEntry {
    insets: geometry::LensInsets,
    auto_refresh_ms: Option<u32>,
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
                auto_refresh_ms: e.auto_refresh_ms,
            })
            .collect()
    }

    pub fn track(&self, id: String) {
        self.entries.lock().push((
            id,
            LensEntry {
                insets: geometry::LensInsets::default(),
                auto_refresh_ms: None,
            },
        ));
    }

    pub fn untrack(&self, id: &str) {
        self.entries.lock().retain(|(i, _)| i != id);
    }

    pub fn set_insets(&self, id: &str, insets: geometry::LensInsets) {
        for (i, e) in self.entries.lock().iter_mut() {
            if i == id {
                e.insets = insets;
                return;
            }
        }
    }

    pub fn set_auto_refresh(&self, id: &str, ms: Option<u32>) {
        for (i, e) in self.entries.lock().iter_mut() {
            if i == id {
                e.auto_refresh_ms = ms;
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

/// Resolve the outer rect (in physical coords) of a lens window.
pub fn outer_rect(win: &WebviewWindow) -> Result<geometry::LensOuterRect> {
    let pos = win.outer_position()?;
    let size = win.outer_size()?;
    Ok(geometry::LensOuterRect {
        x: pos.x,
        y: pos.y,
        w: size.width,
        h: size.height,
    })
}

/// Compute the observed rect, hide the window, capture, then show again.
/// Returns the captured PNG.
pub fn capture_lens(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    id: &str,
) -> Result<Vec<u8>> {
    use crate::capture;

    let win = app
        .get_webview_window(id)
        .ok_or_else(|| anyhow!("lens window {id} not found"))?;
    let outer = outer_rect(&win)?;
    let insets = registry.insets(id).unwrap_or_default();
    let observed = geometry::observed_rect(outer, insets)
        .ok_or_else(|| anyhow!("lens observation rect is empty"))?;

    // Temporarily hide the window so it does not appear in the capture.
    win.hide().ok();
    // Brief pause so the compositor actually unmaps the window before
    // the screen grab runs; otherwise we capture ourselves.
    std::thread::sleep(std::time::Duration::from_millis(60));
    let result = capture::capture_global_rect(observed.x, observed.y, observed.w, observed.h);
    win.show().ok();
    let image = result?;
    capture::png_bytes(&image)
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
        r.set_insets("a", LensInsets { top: 30, ..Default::default() });
        assert_eq!(r.insets("a").unwrap().top, 30);
        assert_eq!(r.insets("b").unwrap().top, 0);
    }

    #[test]
    fn auto_refresh_is_independent_per_lens() {
        let r = new_registry_for_test();
        r.track("a".into());
        r.track("b".into());
        r.set_auto_refresh("b", Some(1000));
        let list = r.list();
        let a = list.iter().find(|d| d.id == "a").unwrap();
        let b = list.iter().find(|d| d.id == "b").unwrap();
        assert!(a.auto_refresh_ms.is_none());
        assert_eq!(b.auto_refresh_ms, Some(1000));
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
        r.set_insets("missing", LensInsets { top: 10, ..Default::default() });
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
        let outer = geometry::LensOuterRect { x: 0, y: 0, w: 0, h: 0 };
        assert!(geometry::observed_rect(outer, geometry::LensInsets::default()).is_none());
    }

    #[test]
    fn observed_rect_with_zero_insets_equals_outer() {
        let outer = geometry::LensOuterRect { x: 10, y: 20, w: 300, h: 200 };
        let observed = geometry::observed_rect(outer, geometry::LensInsets::default()).unwrap();
        assert_eq!(observed, outer);
    }

    #[test]
    fn observed_rect_trims_all_four_sides() {
        let outer = geometry::LensOuterRect { x: 0, y: 0, w: 400, h: 300 };
        let insets = geometry::LensInsets { top: 30, bottom: 20, left: 10, right: 15 };
        let observed = geometry::observed_rect(outer, insets).unwrap();
        assert_eq!(
            observed,
            geometry::LensOuterRect { x: 10, y: 30, w: 375, h: 250 }
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
