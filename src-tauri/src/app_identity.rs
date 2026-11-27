//! Which application something on the desktop belongs to, in a form
//! that survives a restart of that application.
//!
//! Window handles and process ids change with every launch; what stays
//! is the program that runs. [`AppIdentity::key`] is derived from the
//! name of its executable alone, the same way on every OS, so whatever
//! is stored per application (window layouts, glossaries, cached
//! translations) finds its application again.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// An application, as it is remembered across launches.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppIdentity {
    /// Stable key: see [`normalize_key`].
    pub key: String,
    /// The executable's name as written, for display.
    pub name: String,
}

/// The key of the executable called `name` (a file name or a path):
/// its base name without a trailing `.exe`, trimmed and lowercased.
/// Empty when `name` names nothing.
pub fn normalize_key(name: &str) -> String {
    display_name(name).to_lowercase()
}

/// Base name of `name` without a trailing `.exe`, as written.
fn display_name(name: &str) -> String {
    let name = name.trim();
    // Both separators on every OS: a Windows path may be read on any.
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    let cut = base.len().saturating_sub(4);
    let stem = match base.get(cut..) {
        Some(ext) if ext.eq_ignore_ascii_case(".exe") => &base[..cut],
        _ => base,
    };
    stem.trim().to_string()
}

impl AppIdentity {
    /// The application whose executable is called `name` (a file name
    /// or a path). `None` when `name` names nothing.
    pub fn from_executable(name: &str) -> Option<Self> {
        let name = display_name(name);
        if name.is_empty() {
            return None;
        }
        Some(Self {
            key: name.to_lowercase(),
            name,
        })
    }

    /// The application the running process `pid` belongs to.
    ///
    /// Read from the path of the executable where the OS gives it: the
    /// process name alone is cut to 15 bytes on Linux. A process whose
    /// path cannot be read (another user's, a protected one) falls back
    /// to its name.
    pub fn of_pid(pid: u32) -> Option<Self> {
        let pid = sysinfo::Pid::from_u32(pid);
        let mut system = sysinfo::System::new();
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            true,
            sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::Always),
        );
        let process = system.process(pid)?;
        process
            .exe()
            .and_then(Path::file_name)
            .and_then(|name| Self::from_executable(&name.to_string_lossy()))
            .or_else(|| Self::from_executable(&process.name().to_string_lossy()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_the_lowercased_base_name_without_exe() {
        for (name, key) in [
            ("Game.exe", "game"),
            ("GAME.EXE", "game"),
            ("game", "game"),
            (r"C:\Program Files\Some Game\Game.exe", "game"),
            ("/usr/lib/firefox/firefox", "firefox"),
            ("  Visual Novel.exe  ", "visual novel"),
            ("Ｎｏｖｅｌ.exe", "ｎｏｖｅｌ"),
            ("ÉDITEUR.exe", "éditeur"),
            ("archive.tar", "archive.tar"),
        ] {
            assert_eq!(normalize_key(name), key, "{name}");
        }
    }

    #[test]
    fn a_name_that_names_nothing_has_no_identity() {
        for name in ["", "   ", ".exe", "dir/", r"C:\dir\ .exe"] {
            assert_eq!(AppIdentity::from_executable(name), None, "{name:?}");
            assert_eq!(normalize_key(name), "", "{name:?}");
        }
    }

    #[test]
    fn display_name_keeps_the_spelling() {
        let app = AppIdentity::from_executable(r"D:\Games\MyGame.EXE").unwrap();
        assert_eq!(app.name, "MyGame");
        assert_eq!(app.key, "mygame");
    }

    #[test]
    fn the_same_program_has_the_same_key_however_it_is_named() {
        let by_path = AppIdentity::from_executable(r"C:\Apps\Player.exe").unwrap();
        let by_stem = AppIdentity::from_executable("player").unwrap();
        assert_eq!(by_path.key, by_stem.key);
    }

    #[test]
    fn this_process_is_identified_by_its_executable() {
        let app = AppIdentity::of_pid(std::process::id()).expect("own process");
        let exe = std::env::current_exe().unwrap();
        let expected = AppIdentity::from_executable(&exe.to_string_lossy()).unwrap();
        assert_eq!(app, expected);
    }

    #[test]
    fn a_process_that_does_not_run_has_no_identity() {
        assert_eq!(AppIdentity::of_pid(u32::MAX - 1), None);
    }
}
