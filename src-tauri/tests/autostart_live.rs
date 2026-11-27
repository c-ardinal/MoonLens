//! The real login item on Linux, written into a throw-away home folder:
//! proves the file a desktop session reads (path quoting, the tray flag)
//! and the follow-the-executable rule of a portable copy.
#![cfg(target_os = "linux")]

use std::path::Path;

use moonlens_lib::autostart::{apply, reconcile, LoginItem, OsLoginItem, StartupStep, ITEM_NAME};

#[test]
fn login_item_follows_a_portable_copy_and_is_removed_on_request() {
    let home = tempfile::tempdir().unwrap();
    // The only test in this binary, so no other thread reads HOME.
    std::env::set_var("HOME", home.path());
    // A config home outside ~/.config, as the desktop session would read.
    std::env::set_var("XDG_CONFIG_HOME", home.path().join("cfg"));
    let entry = home
        .path()
        .join("cfg/autostart")
        .join(format!("{ITEM_NAME}.desktop"));

    // Switched on from the settings screen, from a folder with spaces.
    let first = OsLoginItem::for_path(Path::new("/media/usb stick/Moon Lens/moonlens")).unwrap();
    assert!(!first.is_enabled().unwrap());
    apply(&first, true).unwrap();
    assert!(first.is_enabled().unwrap());
    let text = std::fs::read_to_string(&entry).unwrap();
    assert!(
        text.lines()
            .any(|l| l == "Exec=\"/media/usb stick/Moon Lens/moonlens\" --hidden"),
        "{text}"
    );

    // The folder was moved; the next start rewrites the entry.
    let moved = OsLoginItem::for_path(Path::new("/opt/tools/moonlens")).unwrap();
    assert_eq!(reconcile(&moved, true).unwrap(), StartupStep::Refresh);
    let text = std::fs::read_to_string(&entry).unwrap();
    assert!(
        text.lines()
            .any(|l| l == "Exec=\"/opt/tools/moonlens\" --hidden"),
        "{text}"
    );

    // Switched off in the desktop's own startup list (entry kept, marked
    // hidden): the OS wins and the mark is not overwritten.
    std::fs::write(&entry, format!("{text}Hidden=true\n")).unwrap();
    assert_eq!(reconcile(&moved, true).unwrap(), StartupStep::SettingOff);
    assert!(std::fs::read_to_string(&entry)
        .unwrap()
        .contains("Hidden=true"));

    // Removed there instead: same outcome.
    std::fs::remove_file(&entry).unwrap();
    assert_eq!(reconcile(&moved, true).unwrap(), StartupStep::SettingOff);
    assert!(!entry.exists());

    // Switched off from the settings screen.
    apply(&moved, true).unwrap();
    apply(&moved, false).unwrap();
    assert!(!entry.exists());
}
