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
//! - Linux: `$XDG_CONFIG_HOME/autostart/MoonLens.desktop` (`~/.config` by default)
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
    /// Registered and not switched off in the OS (Windows: Task Manager,
    /// Linux: `Hidden=true` in the entry).
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

/// One argument as it is written after `Exec=` in a desktop entry. Two
/// layers of the Desktop Entry Specification apply: the argument is
/// quoted for the command line ("The Exec key"), and the result is then
/// a string value of the file, where a backslash is itself an escape
/// character ("Possible value types"). So `$` is stored as `\\$` and a
/// literal backslash as four.
pub fn desktop_exec_quote(arg: &str) -> String {
    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    for c in arg.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            // Field codes (%f, %u, ...) are expanded even inside quotes.
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted.replace('\\', "\\\\")
}

/// The real login item of this OS, pointing at the running executable.
pub struct OsLoginItem(Backend);

#[cfg(not(target_os = "linux"))]
type Backend = auto_launch::AutoLaunch;
#[cfg(target_os = "linux")]
type Backend = XdgEntry;

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
        Self::build(&command_path(path))
    }

    #[cfg(not(target_os = "linux"))]
    fn build(command: &str) -> Result<Self> {
        let mut builder = auto_launch::AutoLaunchBuilder::new();
        builder
            .set_app_name(ITEM_NAME)
            .set_app_path(command)
            .set_args(&[START_HIDDEN_FLAG]);
        #[cfg(windows)]
        builder.set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
        #[cfg(target_os = "macos")]
        builder.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
        Ok(Self(builder.build().context("building the login item")?))
    }

    #[cfg(target_os = "linux")]
    fn build(command: &str) -> Result<Self> {
        let dir = xdg_autostart_dir(
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            std::env::var_os("HOME").as_deref(),
        )
        .context("no XDG_CONFIG_HOME or HOME for the autostart folder")?;
        Ok(Self(XdgEntry {
            file: dir.join(format!("{ITEM_NAME}.desktop")),
            exec: format!("{command} {START_HIDDEN_FLAG}"),
        }))
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

/// `$XDG_CONFIG_HOME/autostart`, or `~/.config/autostart` when the
/// variable is unset, empty or relative (XDG Base Directory rules). The
/// desktop session reads the same folder, so a relocated config home
/// must be followed or the entry is never seen.
pub fn xdg_autostart_dir(config_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    let base = match config_home.map(Path::new) {
        Some(dir) if dir.is_absolute() => dir.to_path_buf(),
        _ => Path::new(home.filter(|h| !h.is_empty())?).join(".config"),
    };
    Some(base.join("autostart"))
}

/// Whether a desktop session would start this autostart entry. Desktops
/// switch an entry off without deleting it: `Hidden=true` (the
/// specification's "treat as deleted") or GNOME's
/// `X-GNOME-Autostart-enabled=false`.
pub fn desktop_entry_active(text: &str) -> bool {
    !text.lines().any(|line| {
        let Some((key, value)) = line.split_once('=') else {
            return false;
        };
        let value = value.trim();
        match key.trim() {
            "Hidden" => value.eq_ignore_ascii_case("true"),
            "X-GNOME-Autostart-enabled" => value.eq_ignore_ascii_case("false"),
            _ => false,
        }
    })
}

/// XDG autostart entry (Linux).
#[cfg(target_os = "linux")]
pub struct XdgEntry {
    file: PathBuf,
    /// Value of the `Exec` key, already quoted.
    exec: String,
}

#[cfg(target_os = "linux")]
impl XdgEntry {
    fn enable(&self) -> std::io::Result<()> {
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = format!(
            "[Desktop Entry]\nType=Application\nVersion=1.0\nName={ITEM_NAME}\n\
             Exec={}\nStartupNotify=false\nTerminal=false\n",
            self.exec
        );
        std::fs::write(&self.file, text)
    }

    fn disable(&self) -> std::io::Result<()> {
        match std::fs::remove_file(&self.file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    fn is_enabled(&self) -> std::io::Result<bool> {
        match std::fs::read_to_string(&self.file) {
            Ok(text) => Ok(desktop_entry_active(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
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
            let message = if wanted {
                crate::tr!(
                    "autostart:could_not_register_launch_at_login_e",
                    e = format!("{:#}", e)
                )
            } else {
                crate::tr!(
                    "autostart:could_not_remove_launch_at_login_e",
                    e = format!("{:#}", e)
                )
            };
            crate::notices::push(NOTICE_ID, message, Some("/settings/#section-startup"));
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

    /// What a desktop session does with the text after `Exec=`: undo the
    /// string-value escapes, then the command-line quoting.
    fn desktop_exec_read(value: &str) -> String {
        fn unescape(text: &str, keep: impl Fn(char) -> bool) -> String {
            let mut out = String::new();
            let mut chars = text.chars();
            while let Some(c) = chars.next() {
                match (c, chars.clone().next()) {
                    ('\\', Some(next)) if keep(next) => {
                        out.push(next);
                        chars.next();
                    }
                    _ => out.push(c),
                }
            }
            out
        }
        let stored = unescape(value, |c| c == '\\');
        let inner = stored
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .expect("quoted argument");
        unescape(inner, |c| matches!(c, '"' | '`' | '$' | '\\')).replace("%%", "%")
    }

    #[test]
    fn desktop_exec_paths_survive_spaces_and_reserved_characters() {
        assert_eq!(
            desktop_exec_quote("/home/a b/Moon Lens/moonlens"),
            "\"/home/a b/Moon Lens/moonlens\""
        );
        // Quoting backslashes are doubled once more for the file format.
        assert_eq!(desktop_exec_quote("/x/$HOME"), r#""/x/\\$HOME""#);
        assert_eq!(desktop_exec_quote(r"/x/a\b"), r#""/x/a\\\\b""#);
        for path in [
            "/home/a b/Moon Lens/moonlens",
            "/x/$HOME/`a`/\"q\"/m",
            r"/x/back\slash/m",
            "/x/100%/m",
        ] {
            assert_eq!(desktop_exec_read(&desktop_exec_quote(path)), path);
        }
    }

    // Unix paths: `/data/cfg` is not absolute on Windows.
    #[cfg(unix)]
    #[test]
    fn a_relocated_config_home_is_followed() {
        let home = Some(OsStr::new("/home/u"));
        assert_eq!(
            xdg_autostart_dir(Some(OsStr::new("/data/cfg")), home).unwrap(),
            Path::new("/data/cfg/autostart")
        );
        for unusable in [None, Some(OsStr::new("")), Some(OsStr::new("rel/cfg"))] {
            assert_eq!(
                xdg_autostart_dir(unusable, home).unwrap(),
                Path::new("/home/u/.config/autostart")
            );
        }
        assert_eq!(xdg_autostart_dir(None, None), None);
    }

    #[test]
    fn an_entry_the_desktop_switched_off_counts_as_off() {
        let base = "[Desktop Entry]\nType=Application\nExec=\"/m\" --hidden\n";
        assert!(desktop_entry_active(base));
        assert!(!desktop_entry_active(&format!("{base}Hidden=true\n")));
        assert!(!desktop_entry_active(&format!("{base}Hidden = True\n")));
        assert!(desktop_entry_active(&format!("{base}Hidden=false\n")));
        assert!(!desktop_entry_active(&format!(
            "{base}X-GNOME-Autostart-enabled=false\n"
        )));
    }
}
