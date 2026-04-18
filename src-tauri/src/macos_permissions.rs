//! macOS permission probes.
//!
//! Live permission detection via lightweight C externs against
//! CoreGraphics and ApplicationServices — no Objective-C runtime
//! required. Non-macOS builds compile the module to a no-op so the
//! rest of the app can call the functions unconditionally.
//!
//! ## Covered permissions
//!
//! - **Screen Recording** — `CGPreflightScreenCaptureAccess` returns
//!   whether we are already allowed to capture the display. The
//!   matching `CGRequestScreenCaptureAccess` triggers the one-time
//!   system prompt; subsequent calls after grant are no-ops.
//! - **Accessibility** — `AXIsProcessTrusted` reports whether our
//!   bundle id is listed in System Settings ▸ Privacy ▸ Accessibility.
//!   We do not programmatically toggle the trust flag because that
//!   requires a privileged daemon, but we can at least reflect state.
//!
//! Microphone and Input Monitoring use AVFoundation / HID APIs that
//! do require the Objective-C runtime, so the onboarding UI keeps
//! "open the pane" links for those two — reflected as `Unknown`
//! here rather than a misleading `Granted`/`Denied`.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionStatus {
    /// The permission is currently granted to the app.
    Granted,
    /// The permission has been explicitly denied, or has never been
    /// asked for, and the app cannot access the resource.
    Denied,
    /// The platform exposes no free-of-Obj-C probe for this permission
    /// (currently microphone and input monitoring). The UI renders a
    /// neutral status for these and points the user at the relevant
    /// System Settings pane.
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionReport {
    pub screen_capture: PermissionStatus,
    pub accessibility: PermissionStatus,
    pub microphone: PermissionStatus,
    pub input_monitoring: PermissionStatus,
}

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

/// Current permission snapshot. On non-macOS builds every field is
/// `Granted` because the app has no opt-in gate on those platforms.
pub fn snapshot() -> PermissionReport {
    #[cfg(target_os = "macos")]
    {
        PermissionReport {
            screen_capture: if unsafe { CGPreflightScreenCaptureAccess() } {
                PermissionStatus::Granted
            } else {
                PermissionStatus::Denied
            },
            accessibility: if unsafe { AXIsProcessTrusted() } {
                PermissionStatus::Granted
            } else {
                PermissionStatus::Denied
            },
            microphone: PermissionStatus::Unknown,
            input_monitoring: PermissionStatus::Unknown,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionReport {
            screen_capture: PermissionStatus::Granted,
            accessibility: PermissionStatus::Granted,
            microphone: PermissionStatus::Granted,
            input_monitoring: PermissionStatus::Granted,
        }
    }
}

/// Fire the macOS "Allow MoonLens to record this computer's screen?"
/// prompt. No-op on other platforms. Returns the post-request state so
/// the UI can react without polling.
pub fn request_screen_capture() -> PermissionStatus {
    #[cfg(target_os = "macos")]
    {
        let granted = unsafe { CGRequestScreenCaptureAccess() };
        if granted {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        PermissionStatus::Granted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_never_returns_unknown_for_detectable_permissions_on_macos() {
        // On macOS the two permissions we actually detect must be
        // either Granted or Denied — never Unknown — because Unknown
        // is reserved for the two we cannot probe without Obj-C.
        let s = snapshot();
        #[cfg(target_os = "macos")]
        {
            assert!(matches!(
                s.screen_capture,
                PermissionStatus::Granted | PermissionStatus::Denied
            ));
            assert!(matches!(
                s.accessibility,
                PermissionStatus::Granted | PermissionStatus::Denied
            ));
            assert_eq!(s.microphone, PermissionStatus::Unknown);
            assert_eq!(s.input_monitoring, PermissionStatus::Unknown);
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Non-macOS hosts have no opt-in gates; every field is
            // reported as Granted so the onboarding modal does not
            // spuriously appear.
            assert_eq!(s.screen_capture, PermissionStatus::Granted);
            assert_eq!(s.accessibility, PermissionStatus::Granted);
            assert_eq!(s.microphone, PermissionStatus::Granted);
            assert_eq!(s.input_monitoring, PermissionStatus::Granted);
        }
    }

    #[test]
    fn permission_status_serializes_as_snake_case() {
        let g = serde_json::to_string(&PermissionStatus::Granted).unwrap();
        let d = serde_json::to_string(&PermissionStatus::Denied).unwrap();
        let u = serde_json::to_string(&PermissionStatus::Unknown).unwrap();
        assert_eq!(g, "\"granted\"");
        assert_eq!(d, "\"denied\"");
        assert_eq!(u, "\"unknown\"");
    }
}
