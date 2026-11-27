//! Windows of other apps on the desktop: which ones there are, and
//! where one of them is right now. What [`crate::attach`] follows.

use serde::Serialize;

use crate::lens::geometry::LensOuterRect as Rect;

/// A top-level window of another app.
#[derive(Debug, Clone, Serialize)]
pub struct ForeignWindow {
    /// The window system's handle (HWND, X11 window id, CGWindowID).
    pub id: u64,
    pub pid: u32,
    pub title: String,
    pub app: String,
    /// Visible bounds on the screen (physical pixels).
    pub rect: Rect,
}

/// Where a window is right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    At(Rect),
    /// Minimized, or not shown at the moment (another virtual desktop).
    Hidden,
    /// Closed.
    Gone,
}

/// Smallest side of a window worth offering (pixels): leaves out the
/// helper windows some apps keep around.
const MIN_SIDE: u32 = 48;

/// The windows of other apps that are on the screen now.
pub fn list() -> Vec<ForeignWindow> {
    let own = std::process::id();
    let Ok(windows) = xcap::Window::all() else {
        return Vec::new();
    };
    windows
        .iter()
        .filter_map(|w| {
            let id = u64::from(w.id().ok()?);
            let pid = w.pid().ok()?;
            if pid == own || w.is_minimized().unwrap_or(false) {
                return None;
            }
            let Probe::At(rect) = probe(id) else {
                return None;
            };
            if rect.w < MIN_SIDE || rect.h < MIN_SIDE {
                return None;
            }
            let title = w.title().unwrap_or_default().trim().to_string();
            let app = w.app_name().unwrap_or_default().trim().to_string();
            if title.is_empty() && app.is_empty() {
                return None;
            }
            Some(ForeignWindow {
                id,
                pid,
                title,
                app,
                rect,
            })
        })
        .collect()
}

/// Where window `id` is right now. Cheap enough to ask many times a
/// second on Windows and X11.
#[cfg(windows)]
pub fn probe(id: u64) -> Probe {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct RECT {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn IsWindow(hwnd: *mut c_void) -> i32;
        fn IsWindowVisible(hwnd: *mut c_void) -> i32;
        fn IsIconic(hwnd: *mut c_void) -> i32;
        fn GetWindowRect(hwnd: *mut c_void, rect: *mut RECT) -> i32;
    }
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmGetWindowAttribute(
            hwnd: *mut c_void,
            attribute: u32,
            value: *mut c_void,
            size: u32,
        ) -> i32;
    }
    const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
    const DWMWA_CLOAKED: u32 = 14;

    let hwnd = id as usize as *mut c_void;
    // SAFETY: the calls take a window handle that may be stale (they
    // then fail) and pointers to locals of the size passed along.
    unsafe {
        if IsWindow(hwnd) == 0 {
            return Probe::Gone;
        }
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
            return Probe::Hidden;
        }
        let mut cloaked = 0u32;
        let got = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            std::mem::size_of::<u32>() as u32,
        );
        if got == 0 && cloaked != 0 {
            return Probe::Hidden;
        }
        // The frame as drawn: `GetWindowRect` adds the invisible resize
        // border of Windows 10 and later.
        let mut r = RECT::default();
        let got = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&raw mut r).cast(),
            std::mem::size_of::<RECT>() as u32,
        );
        if got != 0 && GetWindowRect(hwnd, &raw mut r) == 0 {
            return Probe::Gone;
        }
        if r.right <= r.left || r.bottom <= r.top {
            return Probe::Hidden;
        }
        Probe::At(Rect {
            x: r.left,
            y: r.top,
            w: (r.right - r.left) as u32,
            h: (r.bottom - r.top) as u32,
        })
    }
}

#[cfg(target_os = "linux")]
pub fn probe(id: u64) -> Probe {
    use parking_lot::Mutex;
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{ConnectionExt, MapState};
    use x11rb::rust_connection::RustConnection;

    static X: Mutex<Option<(RustConnection, u32)>> = Mutex::new(None);
    let mut x = X.lock();
    if x.is_none() {
        *x = x11rb::connect(None).ok().map(|(c, screen)| {
            let root = c.setup().roots[screen].root;
            (c, root)
        });
    }
    let Some((c, root)) = x.as_ref() else {
        return Probe::Gone;
    };
    let window = id as u32;
    // All three sent before the first reply is read: one round trip.
    let (Ok(attributes), Ok(size), Ok(origin)) = (
        c.get_window_attributes(window),
        c.get_geometry(window),
        c.translate_coordinates(window, *root, 0, 0),
    ) else {
        // The connection broke: start over next time.
        *x = None;
        return Probe::Gone;
    };
    let (Ok(attributes), Ok(size), Ok(origin)) = (attributes.reply(), size.reply(), origin.reply())
    else {
        return Probe::Gone;
    };
    if attributes.map_state != MapState::VIEWABLE {
        return Probe::Hidden;
    }
    Probe::At(Rect {
        x: i32::from(origin.dst_x),
        y: i32::from(origin.dst_y),
        w: u32::from(size.width),
        h: u32::from(size.height),
    })
}

/// No direct query here: look the window up in the window list.
#[cfg(not(any(windows, target_os = "linux")))]
pub fn probe(id: u64) -> Probe {
    let Ok(windows) = xcap::Window::all() else {
        return Probe::Gone;
    };
    let Some(w) = windows
        .iter()
        .find(|w| w.id().is_ok_and(|i| u64::from(i) == id))
    else {
        return Probe::Gone;
    };
    if w.is_minimized().unwrap_or(false) {
        return Probe::Hidden;
    }
    // The list is in points; the rects here are physical pixels.
    let scale = w
        .current_monitor()
        .and_then(|m| m.scale_factor())
        .map_or(1.0, f64::from);
    let px = |v: f64| (v * scale).round();
    match (w.x(), w.y(), w.width(), w.height()) {
        (Ok(x), Ok(y), Ok(w), Ok(h)) if w > 0 && h > 0 => Probe::At(Rect {
            x: px(f64::from(x)) as i32,
            y: px(f64::from(y)) as i32,
            w: px(f64::from(w)) as u32,
            h: px(f64::from(h)) as u32,
        }),
        _ => Probe::Hidden,
    }
}

/// How often [`probe`] may be asked while the window moves, and while
/// it holds still (milliseconds).
pub const PROBE_MOVING_MS: u64 = if cfg!(any(windows, target_os = "linux")) {
    8
} else {
    33
};
pub const PROBE_IDLE_MS: u64 = if cfg!(any(windows, target_os = "linux")) {
    50
} else {
    150
};
