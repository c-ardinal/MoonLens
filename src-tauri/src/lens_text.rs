//! Text view of a lens window shown outside of it: a companion window
//! docked to one side of the lens, or placed freely and kept at the
//! same offset. The companion follows the lens as it moves or resizes
//! ([`Follow`] holds the placement rule).

use anyhow::{anyhow, Context, Result};
use tauri::{
    AppHandle, Emitter, EventTarget, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::follow::{self, HEIGHT_CSS, WIDTH_CSS};
pub use crate::follow::{Dock, Follow};
use crate::lens::{self, LensRegistryHandle};
use crate::tr;

const MIN_WIDTH_CSS: f64 = 200.0;
const MIN_HEIGHT_CSS: f64 = 90.0;

/// What brings a [`follow`] about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    /// The lens window reported a move or resize: the companion goes
    /// along at once, within the same turn of the event loop.
    LensMoved,
    /// The regular poll, which also notices what the user did to the
    /// companion itself.
    Poll,
}

/// Event telling a lens window what the user did in its companion (the
/// renderer's `TextCommand`); sent from here when the companion was
/// closed through the window system.
const COMMAND_EVENT: &str = "moonlens://lens-text-command";

/// Label of the companion window of lens `id` (covered by the `lens-*`
/// capability).
pub fn label(id: &str) -> String {
    format!("{id}-text")
}

/// Show the text view of lens `id` outside of it at `dock`, or close
/// the companion (`None`). Creates the window: call on the main thread.
pub fn set_dock(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    id: &str,
    dock: Option<Dock>,
) -> Result<()> {
    let label = label(id);
    let Some(dock) = dock else {
        registry.set_text(id, None);
        if let Some(win) = app.get_webview_window(&label) {
            // At once, not on request: switching back to an outside
            // view right away must not find the old window.
            win.destroy().ok();
        }
        return Ok(());
    };
    let lens_win = app
        .get_webview_window(id)
        .ok_or_else(|| anyhow!("lens window {id} not found"))?;
    let scale = lens_win.scale_factor().unwrap_or(1.0);
    if registry.with_text(id, |f| f.set_dock(dock)).is_none() {
        // The window comes first: a placement without a window reads
        // as a companion the user closed (see `follow`).
        if app.get_webview_window(&label).is_none() {
            let url = format!("/lens/text/?id={id}");
            WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
                .title(tr!("MoonLens テキスト", "MoonLens Text"))
                .inner_size(WIDTH_CSS, HEIGHT_CSS)
                .min_inner_size(MIN_WIDTH_CSS, MIN_HEIGHT_CSS)
                .decorations(false)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(true)
                // Like the lens: no window reads its own translations back.
                .content_protected(lens::hidden_from_captures(app))
                .visible(false)
                .build()
                .context("create lens text window")?;
        }
        registry.set_text(id, Some(Follow::new(dock, scale)));
    }
    follow(app, registry, id, &lens_win, Cause::Poll, false);
    if let Some(win) = app.get_webview_window(&label) {
        win.show().ok();
        let _ = win.set_always_on_top(true);
    }
    Ok(())
}

/// Keep the companion of lens `id` in place (see [`Follow::step`]) and
/// out of sight while the lens is (`hidden`). Nothing to do without a
/// companion.
pub fn follow(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    id: &str,
    lens_win: &WebviewWindow,
    cause: Cause,
    hidden: bool,
) {
    let Some(win) = app.get_webview_window(&label(id)) else {
        // Closed through the window system (Alt+F4): drop the placement
        // and tell the lens, as the companion's close button does.
        if registry.with_text(id, |_| ()).is_some() {
            registry.set_text(id, None);
            let target = EventTarget::WebviewWindow {
                label: id.to_string(),
            };
            let _ = app.emit_to(
                target,
                COMMAND_EVENT,
                serde_json::json!({ "kind": "close" }),
            );
        }
        return;
    };
    if cause == Cause::Poll && win.is_visible().is_ok_and(|shown| shown == hidden) {
        let _ = if hidden { win.hide() } else { win.show() };
    }
    let Ok(lens_rect) = lens::inner_rect(lens_win) else {
        return;
    };
    // A move event for a lens that is where the companion was placed for
    // (a resize reports both): nothing to do, and no step to count
    // towards the wait for an earlier placement.
    if cause == Cause::LensMoved
        && registry.with_text(id, |f| f.anchor_moved(lens_rect)) != Some(true)
    {
        return;
    }
    let Ok(actual) = lens::inner_rect(&win) else {
        return;
    };
    let monitor = follow::monitor_of(lens_win, lens_rect);
    let Some(Some(target)) = registry.with_text(id, |f| f.step(lens_rect, actual, monitor)) else {
        return;
    };
    follow::move_to(&win, actual, target);
}

/// Close the companion of lens `id`, if any (the lens is closing).
pub fn close(app: &AppHandle, id: &str) {
    if let Some(win) = app.get_webview_window(&label(id)) {
        win.close().ok();
    }
}
