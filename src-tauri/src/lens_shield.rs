//! Capture shield of the translation windows.
//!
//! A translation window must not read its own translations back, so it
//! has to be missing from the frames MoonLens captures. On Windows that
//! is done with `WDA_EXCLUDEFROMCAPTURE`, which removes a window from
//! *every* screen capture, the user's screenshots and recordings
//! included. Leaving it on for good made the windows impossible to
//! screenshot.
//!
//! [`excluded`] therefore switches the exclusion on only around one
//! capture and off again right after it. The windows stay excluded for
//! good only where the user asked for that ([`persist`]) or where the
//! platform gives no cheap switch (macOS, set at window creation).
//!
//! [`capture_tool_active`] tells when a screenshot tool shows its
//! still of the screen: reading that still would take the frozen
//! picture (translations included) for new screen content.

use parking_lot::Mutex;

/// Native window handle (`HWND` on Windows).
pub type Handle = isize;

/// One capture at a time: a second window finishing its capture must
/// not switch the exclusion off under a capture still running.
static GRAB: Mutex<()> = Mutex::new(());

/// Run `capture` with `windows` missing from the captured frame.
/// Afterwards they are capturable again, unless `keep` is set.
pub fn excluded<T>(windows: &[Handle], keep: bool, capture: impl FnOnce() -> T) -> T {
    let _one = GRAB.lock();
    sys::set(windows, true);
    let out = capture();
    if !keep {
        sys::set(windows, false);
    }
    out
}

/// Leave `windows` out of every screen capture (`excluded`), or let
/// screenshots and recordings show them again.
pub fn persist(windows: &[Handle], excluded: bool) {
    let _one = GRAB.lock();
    sys::set(windows, excluded);
}

/// Programs whose capture overlay shows a still of the screen.
const CAPTURE_TOOLS: [&str; 9] = [
    "screenclippinghost.exe",
    "snippingtool.exe",
    "screensketch.exe",
    "sharex.exe",
    "greenshot.exe",
    "lightshot.exe",
    "snipaste.exe",
    "flameshot.exe",
    "picpick.exe",
];

/// Whether the program at `path` is a known screenshot tool.
pub fn is_capture_tool(path: &str) -> bool {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    CAPTURE_TOOLS.iter().any(|t| name.eq_ignore_ascii_case(t))
}

/// Whether a screenshot tool's overlay is in front right now. Always
/// `false` where the platform is not covered.
pub fn capture_tool_active() -> bool {
    sys::capture_tool_active()
}

#[cfg(windows)]
mod sys {
    use super::Handle;

    #[repr(C)]
    #[derive(Default, Clone, Copy, PartialEq)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn SetWindowDisplayAffinity(hwnd: Handle, affinity: u32) -> i32;
        fn GetWindowDisplayAffinity(hwnd: Handle, affinity: *mut u32) -> i32;
        fn GetForegroundWindow() -> Handle;
        fn GetWindowLongW(hwnd: Handle, index: i32) -> i32;
        fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
        fn GetWindowThreadProcessId(hwnd: Handle, pid: *mut u32) -> u32;
        fn MonitorFromWindow(hwnd: Handle, flags: u32) -> isize;
        fn GetMonitorInfoW(monitor: isize, info: *mut MonitorInfo) -> i32;
    }
    #[link(name = "dwmapi")]
    extern "system" {
        fn DwmFlush() -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn QueryFullProcessImageNameW(
            process: isize,
            flags: u32,
            name: *mut u16,
            size: *mut u32,
        ) -> i32;
        fn CloseHandle(handle: isize) -> i32;
        fn GetCurrentProcessId() -> u32;
    }

    const WDA_NONE: u32 = 0;
    const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;

    pub fn set(windows: &[Handle], excluded: bool) {
        let want = if excluded {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };
        let mut changed = false;
        for &hwnd in windows {
            let mut now = u32::MAX;
            // SAFETY: `now` outlives the call; a stale handle only
            // makes the calls fail.
            unsafe {
                GetWindowDisplayAffinity(hwnd, &mut now);
                if now != want {
                    changed |= SetWindowDisplayAffinity(hwnd, want) != 0;
                }
            }
        }
        // The compositor applies the change with its next frame; a
        // capture taken before that would still show the windows.
        if changed && excluded {
            // SAFETY: no arguments.
            unsafe { DwmFlush() };
        }
    }

    pub fn capture_tool_active() -> bool {
        const GWL_EXSTYLE: i32 = -20;
        const WS_EX_TOPMOST: i32 = 0x8;
        const MONITOR_DEFAULTTONEAREST: u32 = 2;
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        // SAFETY: every pointer refers to a local that outlives its
        // call; the process handle is closed before returning.
        unsafe {
            let front = GetForegroundWindow();
            if front == 0 {
                return false;
            }
            let mut pid = 0;
            GetWindowThreadProcessId(front, &mut pid);
            if pid == 0 || pid == GetCurrentProcessId() {
                return false;
            }
            // The overlay lies over everything; the tool's editor is
            // an ordinary window the translation goes on under.
            let topmost = GetWindowLongW(front, GWL_EXSTYLE) & WS_EX_TOPMOST != 0;
            if !topmost && !fills_monitor(front, MONITOR_DEFAULTTONEAREST) {
                return false;
            }
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process == 0 {
                return false;
            }
            let mut name = [0u16; 1024];
            let mut len = name.len() as u32;
            let named = QueryFullProcessImageNameW(process, 0, name.as_mut_ptr(), &mut len) != 0;
            CloseHandle(process);
            named && super::is_capture_tool(&String::from_utf16_lossy(&name[..len as usize]))
        }
    }

    unsafe fn fills_monitor(hwnd: Handle, nearest: u32) -> bool {
        let mut window = Rect::default();
        let mut info = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            monitor: Rect::default(),
            work: Rect::default(),
            flags: 0,
        };
        GetWindowRect(hwnd, &mut window) != 0
            && GetMonitorInfoW(MonitorFromWindow(hwnd, nearest), &mut info) != 0
            && window.left <= info.monitor.left
            && window.top <= info.monitor.top
            && window.right >= info.monitor.right
            && window.bottom >= info.monitor.bottom
    }
}

#[cfg(not(windows))]
mod sys {
    use super::Handle;

    pub fn set(_windows: &[Handle], _excluded: bool) {}

    pub fn capture_tool_active() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_tools_are_known_by_program_name() {
        assert!(is_capture_tool(
            r"C:\Program Files\WindowsApps\Microsoft.ScreenSketch_8wekyb3d8bbwe\SnippingTool\SnippingTool.exe"
        ));
        assert!(is_capture_tool(
            r"C:\Windows\SystemApps\MicrosoftWindows.Client.CBS_cw5n1h2txyewy\ScreenClippingHost.exe"
        ));
        assert!(is_capture_tool("ShareX.exe"));
    }

    #[test]
    fn other_programs_do_not_pause_reading() {
        assert!(!is_capture_tool(r"C:\Games\game.exe"));
        assert!(!is_capture_tool(r"C:\Tools\NotSnippingTool.exe.bak"));
        assert!(!is_capture_tool(""));
    }

    #[test]
    fn the_capture_runs_once_and_its_result_is_returned() {
        let mut runs = 0;
        let out = excluded(&[], false, || {
            runs += 1;
            7
        });
        assert_eq!((out, runs), (7, 1));
    }
}
