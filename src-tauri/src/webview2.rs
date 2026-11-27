//! Start-up check for the Microsoft Edge WebView2 runtime (Windows).
//!
//! MoonLens ships as a portable zip, with no installer to put the
//! runtime in place. Windows 11 includes it; on a Windows 10 PC without
//! it the app would otherwise exit without showing anything.

use windows::core::{w, PCWSTR};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDOK, MB_ICONWARNING, MB_OKCANCEL, MB_SETFOREGROUND,
};

const DOWNLOAD_PAGE: &str = "https://developer.microsoft.com/microsoft-edge/webview2/";

const MESSAGE: PCWSTR = w!(
    "Microsoft Edge WebView2 ランタイムが未導入のため、MoonLens を起動不可。\n\n\
     Microsoft の配布ページから「エバーグリーン ブートストラップ」を導入後、MoonLens を再起動。\n\n\
     ［OK］で配布ページを表示。"
);

/// Tell the user the runtime is missing; "OK" opens Microsoft's download
/// page in the default browser.
pub fn explain_missing() {
    // SAFETY: both strings are static, NUL-terminated wide strings.
    let choice = unsafe {
        MessageBoxW(
            None,
            MESSAGE,
            w!("MoonLens"),
            MB_OKCANCEL | MB_ICONWARNING | MB_SETFOREGROUND,
        )
    };
    if choice == IDOK {
        let _ = std::process::Command::new("explorer")
            .arg(DOWNLOAD_PAGE)
            .spawn();
    }
}
