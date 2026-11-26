//! Windows: the Lens window must not appear in its own screen captures.
//!
//! The v0.1.0-rc.1 Lens window hid itself and grabbed the screen 60 ms
//! later; Windows was still fading the window out, so the grab read the
//! lens UI (Japanese text) and nothing was translated. The fix gives the
//! window `content_protected(true)`, which Tauri (tao) maps to
//! `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`.
//!
//! This test puts a solid magenta, topmost window over a screen region
//! and reads that region with MoonLens' own capture function
//! ([`moonlens_lib::capture::capture_global_rect`], the one
//! `lens::capture_lens` uses):
//!
//! 1. Without the affinity the window must be in the capture (otherwise
//!    the test proves nothing, e.g. no desktop session).
//! 2. With `WDA_EXCLUDEFROMCAPTURE` it must not be, while it stays shown.
#![cfg(windows)]

use std::time::{Duration, Instant};

use moonlens_lib::capture::capture_global_rect;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::{CreateSolidBrush, UpdateWindow},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
            RegisterClassW, SetWindowDisplayAffinity, ShowWindow, TranslateMessage, MSG, PM_REMOVE,
            SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE, WNDCLASSW, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
        },
    },
};

const X: i32 = 120;
const Y: i32 = 140;
const W: i32 = 240;
const H: i32 = 160;

unsafe extern "system" fn wnd_proc(h: HWND, m: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    DefWindowProcW(h, m, w, l)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// Pump messages for `ms` so the window paints and DWM composes it.
fn pump(ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Share of pixels in the region that are (close to) magenta.
fn magenta_share() -> f64 {
    let img = capture_global_rect(X, Y, W as u32, H as u32)
        .expect("screen capture")
        .to_rgba8();
    let n = img.pixels().count().max(1);
    let hits = img
        .pixels()
        .filter(|p| p[0] > 200 && p[1] < 60 && p[2] > 200)
        .count();
    hits as f64 / n as f64
}

#[test]
fn excluded_window_is_not_in_the_capture() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let class = wide("MoonLensCaptureProbe");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: GetModuleHandleW(std::ptr::null()),
            lpszClassName: class.as_ptr(),
            // COLORREF is 0x00BBGGRR: magenta.
            hbrBackground: CreateSolidBrush(0x00FF00FF),
            ..std::mem::zeroed()
        };
        assert_ne!(RegisterClassW(&wc), 0, "RegisterClassW");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("probe").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            X,
            Y,
            W,
            H,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            wc.hInstance,
            std::ptr::null(),
        );
        assert!(!hwnd.is_null(), "CreateWindowExW");
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        UpdateWindow(hwnd);
        pump(500);

        let visible = magenta_share();
        println!("magenta share, plain window: {:.1}%", visible * 100.0);
        assert!(
            visible > 0.9,
            "the probe window should be captured before exclusion ({:.1}%); \
             no interactive desktop?",
            visible * 100.0
        );

        assert_ne!(
            SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE),
            0,
            "SetWindowDisplayAffinity (needs Windows 10 2004+)"
        );
        pump(300);
        let excluded = magenta_share();
        println!("magenta share, excluded window: {:.1}%", excluded * 100.0);
        // For the CI log: what one reading of a window-translation
        // region costs next to copying the whole monitor.
        let per_run = |f: &dyn Fn()| {
            let t = Instant::now();
            (0..5).for_each(|_| f());
            t.elapsed().as_secs_f64() * 1000.0 / 5.0
        };
        let region = per_run(&|| {
            capture_global_rect(X, Y, W as u32, H as u32).expect("screen capture");
        });
        let whole = per_run(&|| {
            moonlens_lib::capture::capture_primary_full().expect("screen capture");
        });
        println!("capture: region {W}x{H} {region:.1} ms, whole monitor {whole:.1} ms");
        DestroyWindow(hwnd);
        assert!(
            excluded < 0.01,
            "excluded window still captured ({:.1}%)",
            excluded * 100.0
        );
    }
}
