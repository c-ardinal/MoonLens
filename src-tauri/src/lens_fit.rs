//! Fitting a window-translation window to the part of its range where
//! text appears (the subtitle band).
//!
//! The renderer observes the readings of a continuously reading lens for
//! a while and works out the band (`src/lib/lens-fit.ts`); [`fit`] then
//! moves and resizes the window so that its body covers exactly that
//! band. [`restore`] puts the window back where it was.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::lens::{
    self,
    geometry::{observed_rect, LensInsets, LensOuterRect},
    LensRegistryHandle,
};

/// Smallest body a fitted window keeps, in CSS pixels: the header
/// controls stay reachable and one line of text still fits.
const MIN_BODY_CSS: (f64, f64) = (200.0, 48.0);

/// Where a window was before [`fit`], for [`restore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Before {
    /// Client rect in physical screen pixels.
    pub rect: LensOuterRect,
    pub maximized: bool,
}

/// Client rect for a window that should observe `band`, a rect inside
/// the range it observes now (`[x, y, w, h]`, physical pixels from the
/// range's top-left corner). The band is kept inside the range and at
/// least `min` in size, growing around its centre where it is smaller.
/// `None` when the window observes nothing now.
pub fn window_for_band(
    client: LensOuterRect,
    insets: LensInsets,
    band: [i32; 4],
    min: (u32, u32),
) -> Option<LensOuterRect> {
    let range = observed_rect(client, insets)?;
    // One axis: clamp `[at, at + len)` into `[0, limit)` at `min` or more.
    let axis = |at: i32, len: i32, limit: u32, min: u32| -> (i32, u32) {
        let limit = limit as i32;
        let want = len.max(min as i32).min(limit);
        let centre = at + len / 2;
        let start = (centre - want / 2).clamp(0, limit - want);
        (start, want as u32)
    };
    let (x, w) = axis(band[0], band[2].max(0), range.w, min.0);
    let (y, h) = axis(band[1], band[3].max(0), range.h, min.1);
    Some(LensOuterRect {
        x: range.x + x - insets.left as i32,
        y: range.y + y - insets.top as i32,
        w: w + insets.left + insets.right,
        h: h + insets.top + insets.bottom,
    })
}

fn window(app: &AppHandle, id: &str) -> Result<WebviewWindow> {
    app.get_webview_window(id)
        .ok_or_else(|| anyhow!("lens window {id} not found"))
}

/// Move and size `win` so that its client area is `rect`.
fn set_client_rect(win: &WebviewWindow, rect: LensOuterRect) -> Result<()> {
    win.set_size(PhysicalSize::new(rect.w, rect.h))?;
    win.set_position(PhysicalPosition::new(rect.x, rect.y))?;
    // `set_position` places the outer frame; a frameless window on
    // Windows 11 has an invisible border around its client area.
    if let (Ok(inner), Ok(outer)) = (win.inner_position(), win.outer_position()) {
        let (dx, dy) = (inner.x - outer.x, inner.y - outer.y);
        if dx != 0 || dy != 0 {
            win.set_position(PhysicalPosition::new(rect.x - dx, rect.y - dy))?;
        }
    }
    Ok(())
}

/// Shrink the lens `id` to `band` (see [`window_for_band`]). A maximized
/// window leaves that state. Returns where the window was.
pub fn fit(
    app: &AppHandle,
    registry: &LensRegistryHandle,
    id: &str,
    band: [i32; 4],
) -> Result<Before> {
    let win = window(app, id)?;
    let client = lens::inner_rect(&win)?;
    let scale = win.scale_factor().unwrap_or(1.0);
    let insets = lens::scale_insets(registry.insets(id).unwrap_or_default(), scale);
    let min = (
        (MIN_BODY_CSS.0 * scale).ceil() as u32,
        (MIN_BODY_CSS.1 * scale).ceil() as u32,
    );
    let want = window_for_band(client, insets, band, min)
        .ok_or_else(|| anyhow!("lens observation rect is empty"))?;
    let maximized = win.is_maximized().unwrap_or(false);
    if maximized {
        win.unmaximize()?;
    }
    set_client_rect(&win, want)?;
    Ok(Before {
        rect: client,
        maximized,
    })
}

/// Put the lens `id` back where it was before [`fit`].
pub fn restore(app: &AppHandle, id: &str, before: Before) -> Result<()> {
    let win = window(app, id)?;
    if before.maximized {
        win.maximize()?;
        return Ok(());
    }
    set_client_rect(&win, before.rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT: LensOuterRect = LensOuterRect {
        x: 100,
        y: 50,
        w: 1920,
        h: 1112,
    };
    const INSETS: LensInsets = LensInsets {
        top: 32,
        bottom: 0,
        left: 0,
        right: 0,
    };
    const MIN: (u32, u32) = (200, 48);

    #[test]
    fn body_lands_on_the_band_with_the_header_above() {
        let want = window_for_band(CLIENT, INSETS, [500, 900, 900, 120], MIN).unwrap();
        assert_eq!(
            want,
            LensOuterRect {
                x: 600,
                y: 50 + 900,
                w: 900,
                h: 120 + 32,
            }
        );
        // What the fitted window observes is the band, in screen pixels.
        assert_eq!(
            observed_rect(want, INSETS),
            Some(LensOuterRect {
                x: 600,
                y: 50 + 32 + 900,
                w: 900,
                h: 120,
            })
        );
    }

    #[test]
    fn band_is_kept_inside_the_range() {
        let want = window_for_band(CLIENT, INSETS, [-50, 1000, 3000, 500], MIN).unwrap();
        let seen = observed_rect(want, INSETS).unwrap();
        assert_eq!((seen.x, seen.w), (100, 1920));
        assert_eq!(seen.y + seen.h as i32, 50 + 1112);
    }

    #[test]
    fn small_band_grows_around_its_centre_to_the_minimum() {
        let want = window_for_band(CLIENT, INSETS, [1000, 500, 40, 20], MIN).unwrap();
        let seen = observed_rect(want, INSETS).unwrap();
        assert_eq!((seen.w, seen.h), MIN);
        assert_eq!(seen.x + 100, 100 + 1000 + 20);
        assert_eq!(seen.y + 24, 50 + 32 + 500 + 10);
    }

    #[test]
    fn range_smaller_than_the_minimum_is_kept_whole() {
        let client = LensOuterRect {
            x: 0,
            y: 0,
            w: 120,
            h: 62,
        };
        let want = window_for_band(client, INSETS, [10, 5, 50, 10], MIN).unwrap();
        assert_eq!(want, client);
    }

    #[test]
    fn collapsed_window_has_no_band() {
        let client = LensOuterRect {
            x: 0,
            y: 0,
            w: 300,
            h: 32,
        };
        assert_eq!(window_for_band(client, INSETS, [0, 0, 10, 10], MIN), None);
    }
}
