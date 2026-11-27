//! Start at OS login (`startup.launch_at_login`).
//!
//! MoonLens ships as a portable program, so nothing registers it with the
//! OS at install time and the executable may be moved at any moment. The
//! login item therefore always points at the executable that is running
//! now, and every start with the setting on rewrites it: after a move the
//! first manual launch repairs the entry.
//!
//! Per-user only, no administrator rights:
//! - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//! - macOS: `~/Library/LaunchAgents/MoonLens.plist`
//! - Linux: `~/.config/autostart/MoonLens.desktop`
//!
//! A login launch carries [`START_HIDDEN_FLAG`], so the app goes straight
//! to the tray instead of opening the main window at every login.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::app::START_HIDDEN_FLAG;
use crate::settings::SettingsStore;

/// Name of the login item (registry value / file stem). Debug builds use
/// their own name so a development run never replaces the entry of the
/// released program.
pub const ITEM_NAME: &str = if cfg!(debug_assertions) {
    "MoonLens-dev"
} else {
    "MoonLens"
};

/// Notice id for a failed registration (see `notices.rs`).
pub const NOTICE_ID: &str = "autostart";

/// The OS side of the setting. A trait so the decisions below are tested
/// without touching the registry or the home directory.
pub trait LoginItem {
    /// Create the entry, or rewrite it with the current path.
    fn enable(&self) -> Result<()>;
    fn disable(&self) -> Result<()>;
    /// Registered and not switched off in the OS (Windows: Task Manager).
    fn is_enabled(&self) -> Result<bool>;
}

/// What a start does about the login item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupStep {
    /// Setting and OS agree that it is off.
    Keep,
    /// Both on: rewrite the entry so it follows a moved executable.
    Refresh,
    /// Setting off but an entry exists (config reset by hand): remove it.
    Remove,
    /// Setting on but the OS has no active entry: the user switched it off
    /// there (Task Manager, the desktop's startup list) or the config came
    /// from another PC. The OS wins; the setting goes back to off rather
    /// than re-registering against the user's choice.
    SettingOff,
}

pub fn startup_step(wanted: bool, enabled: bool) -> StartupStep {
    match (wanted, enabled) {
        (false, false) => StartupStep::Keep,
        (true, true) => StartupStep::Refresh,
        (false, true) => StartupStep::Remove,
        (true, false) => StartupStep::SettingOff,
    }
}

/// Bring the OS entry in line at start. Returns the step taken; the caller
/// writes the setting back on [`StartupStep::SettingOff`].
pub fn reconcile(item: &dyn LoginItem, wanted: bool) -> Result<StartupStep> {
    let step = startup_step(wanted, item.is_enabled()?);
    match step {
        StartupStep::Refresh => item.enable()?,
        StartupStep::Remove => item.disable()?,
        StartupStep::Keep | StartupStep::SettingOff => {}
    }
    Ok(step)
}

/// The setting was changed by the user: make the OS follow.
pub fn apply(item: &dyn LoginItem, wanted: bool) -> Result<()> {
    if wanted {
        item.enable()
    } else {
        item.disable()
    }
}

/// The file a login launch has to start. An AppImage runs from a mount
/// point that changes on every launch, so the image file itself is the
/// target there (`APPIMAGE`, set by its runtime).
pub fn launch_path(current_exe: &Path, appimage: Option<&OsStr>) -> PathBuf {
    match appimage {
        Some(image) if !image.is_empty() => PathBuf::from(image),
        _ => current_exe.to_path_buf(),
    }
}

/// `path` as the OS entry needs it. Portable copies often live in folders
/// with spaces (`C:\Users\A B\Desktop\MoonLens`), which the entry formats
/// of Windows and Linux split unless the path is quoted.
pub fn command_path(path: &str) -> String {
    if cfg!(windows) {
        format!("\"{path}\"")
    } else if cfg!(target_os = "linux") {
        desktop_exec_quote(path)
    } else {
        // macOS: one <string> of the launch agent's ProgramArguments.
        path.to_string()
    }
}

/// Quote one argument for the `Exec` key of a desktop entry
/// (Desktop Entry Specification, "The Exec key").
pub fn desktop_exec_quote(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            // Field codes (%f, %u, ...) are expanded even inside quotes.
            '%' => out.push_str("%%"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The real login item of this OS, pointing at the running executable.
pub struct OsLoginItem(auto_launch::AutoLaunch);

impl OsLoginItem {
    pub fn for_running_app() -> Result<Self> {
        let exe = std::env::current_exe().context("locating the running executable")?;
        let appimage = std::env::var_os("APPIMAGE");
        Self::for_path(&launch_path(&exe, appimage.as_deref()))
    }

    pub fn for_path(path: &Path) -> Result<Self> {
        let path = path
            .to_str()
            .with_context(|| format!("executable path is not UTF-8: {}", path.display()))?;
        let mut builder = auto_launch::AutoLaunchBuilder::new();
        builder
            .set_app_name(ITEM_NAME)
            .set_app_path(&command_path(path))
            .set_args(&[START_HIDDEN_FLAG]);
        #[cfg(windows)]
        builder.set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
        #[cfg(target_os = "macos")]
        builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
        #[cfg(target_os = "linux")]
        builder.set_linux_launch_mode(auto_launch::LinuxLaunchMode::XdgAutostart);
        Ok(Self(builder.build().context("building the login item")?))
    }
}

impl LoginItem for OsLoginItem {
    fn enable(&self) -> Result<()> {
        self.0.enable().context("registering the login item")
    }

    fn disable(&self) -> Result<()> {
        self.0.disable().context("removing the login item")
    }

    fn is_enabled(&self) -> Result<bool> {
        self.0.is_enabled().context("reading the login item")
    }
}

/// Start-up pass: repair, remove or give way to the OS (see
/// [`StartupStep`]). Never fails the launch; problems are only logged
/// because the user did not ask for anything at this point.
pub fn sync_at_startup(settings: &SettingsStore) {
    let wanted = settings.snapshot().startup.launch_at_login;
    let step = OsLoginItem::for_running_app().and_then(|item| reconcile(&item, wanted));
    match step {
        Ok(StartupStep::SettingOff) => {
            tracing::info!("login item is off in the OS; turning the setting off");
            let mut next = (*settings.snapshot()).clone();
            next.startup.launch_at_login = false;
            if let Err(e) = settings.save(next) {
                tracing::warn!("saving launch_at_login = false: {e:#}");
            }
        }
        Ok(step) => tracing::debug!("login item at start: {step:?}"),
        Err(e) => tracing::warn!("login item check at start: {e:#}"),
    }
}

/// The setting changed while running: register or remove, and tell the
/// user when the OS refused (home screen notice with a link to the
/// setting).
pub fn sync_on_change(wanted: bool) {
    match OsLoginItem::for_running_app().and_then(|item| apply(&item, wanted)) {
        Ok(()) => {
            tracing::info!(
                "login item {}",
                if wanted { "registered" } else { "removed" }
            );
            crate::notices::clear(NOTICE_ID);
        }
        Err(e) => {
            let what = if wanted { "登録" } else { "解除" };
            crate::notices::push(
                NOTICE_ID,
                format!("自動起動の{what}失敗：{e:#}"),
                Some("/settings/#section-startup"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct Fake {
        enabled: Cell<bool>,
        calls: RefCell<Vec<&'static str>>,
    }

    impl Fake {
        fn with(enabled: bool) -> Self {
            let f = Self::default();
            f.enabled.set(enabled);
            f
        }
    }

    impl LoginItem for Fake {
        fn enable(&self) -> Result<()> {
            self.calls.borrow_mut().push("enable");
            self.enabled.set(true);
            Ok(())
        }
        fn disable(&self) -> Result<()> {
            self.calls.borrow_mut().push("disable");
            self.enabled.set(false);
            Ok(())
        }
        fn is_enabled(&self) -> Result<bool> {
            Ok(self.enabled.get())
        }
    }

    #[test]
    fn start_rewrites_an_active_entry_so_it_follows_a_moved_exe() {
        let item = Fake::with(true);
        assert_eq!(reconcile(&item, true).unwrap(), StartupStep::Refresh);
        assert_eq!(*item.calls.borrow(), ["enable"]);
    }

    #[test]
    fn start_does_not_re_register_what_the_os_switched_off() {
        let item = Fake::with(false);
        assert_eq!(reconcile(&item, true).unwrap(), StartupStep::SettingOff);
        assert!(item.calls.borrow().is_empty());
        assert!(!item.enabled.get());
    }

    #[test]
    fn start_removes_a_leftover_entry_when_the_setting_is_off() {
        let item = Fake::with(true);
        assert_eq!(reconcile(&item, false).unwrap(), StartupStep::Remove);
        assert!(!item.enabled.get());
    }

    #[test]
    fn start_leaves_the_os_alone_when_off_on_both_sides() {
        let item = Fake::with(false);
        assert_eq!(reconcile(&item, false).unwrap(), StartupStep::Keep);
        assert!(item.calls.borrow().is_empty());
    }

    #[test]
    fn a_changed_setting_is_applied_as_is() {
        let item = Fake::with(false);
        apply(&item, true).unwrap();
        assert!(item.enabled.get());
        apply(&item, false).unwrap();
        assert!(!item.enabled.get());
    }

    #[test]
    fn an_appimage_registers_the_image_not_its_mount_point() {
        let exe = Path::new("/tmp/.mount_MoonLeAbC/usr/bin/moonlens");
        let image = OsStr::new("/home/u/Apps/MoonLens.AppImage");
        assert_eq!(
            launch_path(exe, Some(image)),
            Path::new("/home/u/Apps/MoonLens.AppImage")
        );
        assert_eq!(launch_path(exe, None), exe);
        assert_eq!(launch_path(exe, Some(OsStr::new(""))), exe);
    }

    #[test]
    fn desktop_exec_paths_survive_spaces_and_reserved_characters() {
        assert_eq!(
            desktop_exec_quote("/home/a b/Moon Lens/moonlens"),
            "\"/home/a b/Moon Lens/moonlens\""
        );
        assert_eq!(desktop_exec_quote("/x/$HOME/`a`"), "\"/x/\\$HOME/\\`a\\`\"");
        assert_eq!(desktop_exec_quote("/x/100%/m"), "\"/x/100%%/m\"");
    }
}
