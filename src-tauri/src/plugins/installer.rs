//! Plugin archive installer.
//!
//! A `.moonlens-plugin` file is just a zip that contains a `plugin.toml`
//! at its root plus whatever the plugin ships (Wasm binary, sidecar
//! executable, icons, etc.). The installer extracts the archive into a
//! subdirectory of `plugins/` named after the manifest's `plugin.id` so
//! two plugins with the same `file_stem()` still coexist.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::plugins::manifest::PluginManifest;

/// Extract `archive` into a new subdirectory of `dest_root`. The returned
/// path points at the newly-created plugin root.
///
/// Security note: we reject any archive entry whose destination escapes
/// `dest_root` (classic zip-slip). Absolute paths and `..` components in
/// entry names cause the install to abort before any file is written.
pub fn extract_zip(archive: &Path, dest_root: &Path) -> Result<PathBuf> {
    let file =
        fs::File::open(archive).with_context(|| format!("open archive {}", archive.display()))?;
    let mut zip = zip::ZipArchive::new(file).context("parse zip")?;

    // First pass: find plugin.toml and derive the install folder from
    // its `plugin.id` so directories are stable across archive names.
    let manifest_raw = read_manifest_from_zip(&mut zip)?;
    let manifest: PluginManifest =
        toml::from_str(&manifest_raw).context("plugin.toml is not a valid manifest")?;
    let install_dir = dest_root.join(sanitise_id(&manifest.plugin.id));
    if install_dir.exists() {
        fs::remove_dir_all(&install_dir)
            .with_context(|| format!("clear existing plugin at {}", install_dir.display()))?;
    }
    fs::create_dir_all(&install_dir)?;

    // Second pass: extract every file, rejecting unsafe names.
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.enclosed_name().ok_or_else(|| {
            anyhow!(
                "zip entry rejected for unsafe name: {}",
                entry.name().unwrap_or_default()
            )
        })?;
        if !is_safe_relative(&name) {
            anyhow::bail!("zip entry escapes install dir: {}", name.display());
        }
        // Symlinks could point outside the install dir (or at a file a
        // later entry overwrites through the link); plugins never need them.
        if entry.is_symlink() {
            anyhow::bail!("zip entry is a symlink (not allowed): {}", name.display());
        }
        let out_path = install_dir.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&out_path)?;
        io::copy(&mut entry, &mut out)?;
        #[cfg(unix)]
        {
            // Preserve the executable bit so sidecar binaries remain
            // runnable after install, but only the plain rwx bits:
            // setuid / setgid / sticky and group/other write from an
            // untrusted archive are dropped.
            if let Some(mode) = entry.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&out_path, fs::Permissions::from_mode(safe_mode(mode)))?;
            }
        }
    }
    Ok(install_dir)
}

/// Read `plugin.toml` from the given open archive. Accepts either a
/// top-level entry or one nested under a single directory (common when
/// archives were created with `zip -r foo.moonlens-plugin ./foo`).
fn read_manifest_from_zip<R: io::Read + io::Seek>(zip: &mut zip::ZipArchive<R>) -> Result<String> {
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name()?.into_owned();
        if name == "plugin.toml" || name.ends_with("/plugin.toml") {
            let mut s = String::new();
            entry.read_to_string(&mut s)?;
            return Ok(s);
        }
    }
    Err(anyhow!("archive does not contain plugin.toml at root"))
}

/// Permission bits applied to an extracted file: owner rwx is kept,
/// group/other keep read+execute, everything else (setuid, setgid,
/// sticky, group/other write) is cleared.
#[cfg_attr(not(unix), allow(dead_code))]
fn safe_mode(mode: u32) -> u32 {
    mode & 0o755
}

/// Reject characters that the host filesystem does not like in directory
/// names. Keeps the id visually close to the original by allowing
/// letters, digits, dot, dash, and underscore.
fn sanitise_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn is_safe_relative(p: &Path) -> bool {
    for component in p.components() {
        match component {
            Component::Normal(_) => {}
            _ => return false,
        }
    }
    true
}

// Bring Read into scope for `entry.read_to_string` above.
use std::io::Read;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitise_id_keeps_safe_characters() {
        assert_eq!(sanitise_id("com.example.a-b_c.2"), "com.example.a-b_c.2");
    }

    #[test]
    fn sanitise_id_replaces_path_separators() {
        assert_eq!(sanitise_id("com/evil\\x"), "com_evil_x");
    }

    #[test]
    fn is_safe_relative_accepts_nested_files() {
        assert!(is_safe_relative(Path::new("a/b/c.toml")));
    }

    #[test]
    fn is_safe_relative_rejects_parent_traversal() {
        assert!(!is_safe_relative(Path::new("../etc/passwd")));
        assert!(!is_safe_relative(Path::new("a/../b")));
    }

    #[test]
    fn is_safe_relative_rejects_absolute_paths() {
        // On Unix a leading "/" produces a RootDir component; on Windows
        // a drive letter produces Prefix. Both must be rejected.
        assert!(!is_safe_relative(Path::new("/etc/passwd")));
    }

    #[test]
    fn safe_mode_strips_setuid_setgid_sticky_and_shared_write() {
        assert_eq!(safe_mode(0o4755), 0o755);
        assert_eq!(safe_mode(0o2755), 0o755);
        assert_eq!(safe_mode(0o1777), 0o755);
        assert_eq!(safe_mode(0o100644), 0o644);
        assert_eq!(safe_mode(0o700), 0o700);
    }

    fn write_archive(
        path: &Path,
        build: impl FnOnce(&mut zip::ZipWriter<fs::File>, zip::write::SimpleFileOptions),
    ) {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        w.start_file("plugin.toml", opts).unwrap();
        w.write_all(MANIFEST.as_bytes()).unwrap();
        build(&mut w, opts);
        w.finish().unwrap();
    }

    const MANIFEST: &str = r#"
[plugin]
id = "com.example.t"
name = "t"
version = "0.1.0"
author = "t"
license = "MIT"
kind = "sidecar"
entry = "bin/run"
api_version = "1"

[capabilities]
provides = ["translator"]
consumes = []
"#;

    #[test]
    fn extract_rejects_symlink_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("p.zip");
        write_archive(&archive, |w, opts| {
            w.add_symlink("evil", "/etc/passwd", opts).unwrap();
        });
        let err = extract_zip(&archive, &dir.path().join("out")).unwrap_err();
        assert!(err.to_string().contains("symlink"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn extract_drops_setuid_bit_but_keeps_exec() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("p.zip");
        write_archive(&archive, |w, opts| {
            w.start_file("bin/run", opts.unix_permissions(0o4755))
                .unwrap();
            w.write_all(b"#!/bin/sh\n").unwrap();
        });
        let root = extract_zip(&archive, &dir.path().join("out")).unwrap();
        let mode = fs::metadata(root.join("bin/run"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o7777, 0o755);
    }
}
