//! Persistent storage: history (rusqlite).
//!
//! `Storage::open` uses the app-data path, so tests use [`Storage::open_at`]
//! with a temp file. Unit tests exercise append + query to catch schema
//! regressions.

use std::path::{Path, PathBuf};

use crate::tr;
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

pub struct Storage {
    db: Mutex<Connection>,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub created_at: i64,
    pub kind: String,
    pub source_text: Option<String>,
    pub result_text: Option<String>,
    pub provider: Option<String>,
    pub via_cloud: bool,
    pub image_path: Option<String>,
}

impl Storage {
    pub fn open(app: &AppHandle) -> Result<Self> {
        let data_dir = app.path().app_data_dir().context("no data dir")?;
        std::fs::create_dir_all(&data_dir)?;
        Self::open_at(data_dir.join("moonlens.db"), data_dir)
    }

    /// [`Self::open`], but a history database that cannot be opened
    /// (corrupted, written by a newer MoonLens, …) is moved aside and a
    /// fresh one is created, so a broken history never stops the app from
    /// starting. The user is told through a notice.
    pub fn open_or_recover(app: &AppHandle) -> Result<Self> {
        let data_dir = app.path().app_data_dir().context("no data dir")?;
        std::fs::create_dir_all(&data_dir)?;
        Self::open_at_or_recover(data_dir.join("moonlens.db"), data_dir)
    }

    pub fn open_at_or_recover(db_path: PathBuf, data_dir: PathBuf) -> Result<Self> {
        match Self::open_at(&db_path, &data_dir) {
            Ok(s) => Ok(s),
            Err(e) => {
                tracing::warn!("history database unusable: {e:#}");
                let secs = now_seconds();
                let aside = db_path.with_extension(format!("db.broken-{secs}"));
                std::fs::rename(&db_path, &aside)
                    .with_context(|| format!("moving {} aside", db_path.display()))?;
                for ext in ["db-wal", "db-shm", "db-journal"] {
                    let _ = std::fs::remove_file(db_path.with_extension(ext));
                }
                crate::notices::push(
                    "history-db",
                    tr!(
                        "履歴データベースを読込不可のため再作成。旧ファイル：{}",
                        "The history database could not be read and was recreated. Old file: {}",
                        aside.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    None,
                );
                Self::open_at(&db_path, &data_dir)
            }
        }
    }

    /// Open a storage at an explicit path; used by tests and callers that
    /// want custom locations.
    pub fn open_at(db_path: impl AsRef<Path>, data_dir: impl AsRef<Path>) -> Result<Self> {
        let db = Connection::open(db_path.as_ref())?;
        Self::migrate(&db)?;
        Ok(Self {
            db: Mutex::new(db),
            data_dir: data_dir.as_ref().to_path_buf(),
        })
    }

    /// In-memory storage for tests. Not exposed through the app builder
    /// because the path is ephemeral. Kept `pub` (not `cfg(test)`) so
    /// integration tests in the separate `tests/` crate can use it too.
    pub fn open_in_memory() -> Result<Self> {
        let db = Connection::open_in_memory()?;
        Self::migrate(&db)?;
        Ok(Self {
            db: Mutex::new(db),
            data_dir: PathBuf::new(),
        })
    }

    /// Schema version this build writes (`PRAGMA user_version`).
    pub const SCHEMA_VERSION: i64 = 1;

    fn migrate(db: &Connection) -> Result<()> {
        // Fails fast on a file that is not a SQLite database.
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > Self::SCHEMA_VERSION {
            anyhow::bail!(
                "history database schema {version} is newer than this build ({})",
                Self::SCHEMA_VERSION
            );
        }
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at INTEGER NOT NULL,
                kind TEXT NOT NULL,
                source_text TEXT,
                result_text TEXT,
                provider TEXT,
                via_cloud INTEGER,
                image_path TEXT
             );
             CREATE INDEX IF NOT EXISTS history_created_at
                ON history(created_at DESC);",
        )?;
        // Future schema changes go here, keyed on `version`.
        db.pragma_update(None, "user_version", Self::SCHEMA_VERSION)?;
        Ok(())
    }

    pub fn append(
        &self,
        kind: &str,
        source_text: Option<&str>,
        result_text: Option<&str>,
        provider: Option<&str>,
        via_cloud: bool,
        image_path: Option<&str>,
    ) -> Result<i64> {
        let now = now_seconds();
        let db = self.db.lock();
        db.execute(
            "INSERT INTO history
              (created_at, kind, source_text, result_text, provider, via_cloud, image_path)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                now,
                kind,
                source_text,
                result_text,
                provider,
                via_cloud as i32,
                image_path
            ],
        )?;
        Ok(db.last_insert_rowid())
    }

    pub fn recent(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        let db = self.db.lock();
        // Tie-break by `id` so entries inserted within the same second
        // still return newest-first deterministically.
        let mut stmt = db.prepare(
            "SELECT id, created_at, kind, source_text, result_text, provider, via_cloud, image_path
             FROM history ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit as i64], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                created_at: row.get(1)?,
                kind: row.get(2)?,
                source_text: row.get(3)?,
                result_text: row.get(4)?,
                provider: row.get(5)?,
                via_cloud: row.get::<_, i32>(6)? != 0,
                image_path: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Newest-first entries whose text, provider or kind contains
    /// `needle` (case-insensitive for ASCII), searched over the whole
    /// history rather than only the most recent rows.
    pub fn search(&self, needle: &str, limit: usize) -> Result<Vec<HistoryEntry>> {
        let escaped = needle
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let db = self.db.lock();
        let mut stmt = db.prepare(
            "SELECT id, created_at, kind, source_text, result_text, provider, via_cloud, image_path
             FROM history
             WHERE source_text LIKE ?1 ESCAPE '\\'
                OR result_text LIKE ?1 ESCAPE '\\'
                OR provider LIKE ?1 ESCAPE '\\'
                OR kind LIKE ?1 ESCAPE '\\'
             ORDER BY created_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(rusqlite::params![pattern, limit as i64], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                created_at: row.get(1)?,
                kind: row.get(2)?,
                source_text: row.get(3)?,
                result_text: row.get(4)?,
                provider: row.get(5)?,
                via_cloud: row.get::<_, i32>(6)? != 0,
                image_path: row.get(7)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn prune_older_than(&self, epoch_seconds: i64) -> Result<usize> {
        let db = self.db.lock();
        let n = db.execute("DELETE FROM history WHERE created_at < ?1", [epoch_seconds])?;
        Ok(n)
    }

    /// Delete a single entry by primary key. Returns the number of rows
    /// removed (0 if the id did not exist).
    /// Keep only the newest `max_entries` rows (0 = no limit).
    pub fn prune_to_count(&self, max_entries: u32) -> Result<usize> {
        if max_entries == 0 {
            return Ok(0);
        }
        let db = self.db.lock();
        let n = db.execute(
            "DELETE FROM history WHERE id NOT IN (
               SELECT id FROM history ORDER BY created_at DESC, id DESC LIMIT ?1
             )",
            [max_entries as i64],
        )?;
        Ok(n)
    }

    /// Apply the history retention settings: drop rows older than
    /// `retention_days` (0 = keep forever), then cap at `max_entries`
    /// (0 = no cap). Returns the number of rows removed.
    pub fn apply_retention(&self, retention_days: u32, max_entries: u32) -> Result<usize> {
        let mut removed = 0;
        if retention_days > 0 {
            let cutoff = now_seconds() - i64::from(retention_days) * 86_400;
            removed += self.prune_older_than(cutoff)?;
        }
        removed += self.prune_to_count(max_entries)?;
        Ok(removed)
    }

    pub fn delete(&self, id: i64) -> Result<usize> {
        let db = self.db.lock();
        let n = db.execute("DELETE FROM history WHERE id = ?1", [id])?;
        Ok(n)
    }

    /// Put a deleted row back under its original id and timestamp (undo
    /// of [`Storage::delete`]). Returns 0 when the id is already present.
    pub fn restore(&self, entry: &HistoryEntry) -> Result<usize> {
        let db = self.db.lock();
        let n = db.execute(
            "INSERT OR IGNORE INTO history
              (id, created_at, kind, source_text, result_text, provider, via_cloud, image_path)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                entry.id,
                entry.created_at,
                entry.kind,
                entry.source_text,
                entry.result_text,
                entry.provider,
                entry.via_cloud as i32,
                entry.image_path
            ],
        )?;
        Ok(n)
    }

    /// Drop every history row. Returns the number of rows removed.
    pub fn clear_all(&self) -> Result<usize> {
        let db = self.db.lock();
        let n = db.execute("DELETE FROM history", [])?;
        Ok(n)
    }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_append_then_recent() {
        let s = Storage::open_in_memory().unwrap();
        let id = s
            .append(
                "translate",
                Some("hello"),
                Some("こんにちは"),
                Some("ollama.gemma4"),
                false,
                None,
            )
            .unwrap();
        assert!(id > 0);
        let entries = s.recent(10).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.kind, "translate");
        assert_eq!(e.source_text.as_deref(), Some("hello"));
        assert_eq!(e.result_text.as_deref(), Some("こんにちは"));
        assert_eq!(e.provider.as_deref(), Some("ollama.gemma4"));
        assert!(!e.via_cloud);
    }

    #[test]
    fn recent_returns_newest_first() {
        let s = Storage::open_in_memory().unwrap();
        for i in 0..5 {
            s.append("translate", Some(&format!("s{i}")), None, None, false, None)
                .unwrap();
        }
        let entries = s.recent(3).unwrap();
        assert_eq!(entries.len(), 3);
        let ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
        // Newest inserts have the largest ids; ordering must be desc.
        let mut sorted = ids.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(ids, sorted);
    }

    #[test]
    fn prune_older_than_removes_old_rows() {
        let s = Storage::open_in_memory().unwrap();
        // Insert one row, then drop everything dated before "now + 1s".
        s.append("translate", None, None, None, false, None)
            .unwrap();
        let deleted = s.prune_older_than(now_seconds() + 1).unwrap();
        assert_eq!(deleted, 1);
        assert!(s.recent(10).unwrap().is_empty());
    }

    #[test]
    fn prune_to_count_keeps_the_newest_rows() {
        let s = Storage::open_in_memory().unwrap();
        for i in 0..5 {
            s.append("translate", Some(&i.to_string()), None, None, false, None)
                .unwrap();
        }
        assert_eq!(s.prune_to_count(2).unwrap(), 3);
        let left: Vec<_> = s
            .recent(10)
            .unwrap()
            .into_iter()
            .map(|e| e.source_text.unwrap())
            .collect();
        assert_eq!(left, vec!["4", "3"]);
        // 0 means "no cap".
        assert_eq!(s.prune_to_count(0).unwrap(), 0);
        assert_eq!(s.recent(10).unwrap().len(), 2);
    }

    #[test]
    fn apply_retention_zero_keeps_everything() {
        let s = Storage::open_in_memory().unwrap();
        s.append("translate", Some("a"), None, None, false, None)
            .unwrap();
        assert_eq!(s.apply_retention(0, 0).unwrap(), 0);
        assert_eq!(s.apply_retention(30, 10).unwrap(), 0);
        assert_eq!(s.recent(10).unwrap().len(), 1);
    }

    #[test]
    fn via_cloud_flag_is_preserved() {
        let s = Storage::open_in_memory().unwrap();
        s.append("translate", Some("x"), Some("y"), Some("deepl"), true, None)
            .unwrap();
        let entries = s.recent(1).unwrap();
        assert!(entries[0].via_cloud);
    }

    #[test]
    fn empty_database_recent_returns_empty_vec() {
        let s = Storage::open_in_memory().unwrap();
        assert!(s.recent(10).unwrap().is_empty());
    }

    #[test]
    fn recent_with_zero_limit_returns_empty() {
        let s = Storage::open_in_memory().unwrap();
        s.append("translate", Some("x"), Some("y"), None, false, None)
            .unwrap();
        assert!(s.recent(0).unwrap().is_empty());
    }

    #[test]
    fn recent_limit_larger_than_row_count_returns_all_rows() {
        let s = Storage::open_in_memory().unwrap();
        for i in 0..3 {
            s.append("translate", Some(&i.to_string()), None, None, false, None)
                .unwrap();
        }
        assert_eq!(s.recent(1000).unwrap().len(), 3);
    }

    #[test]
    fn empty_string_is_stored_distinct_from_none() {
        let s = Storage::open_in_memory().unwrap();
        // Empty `Some("")` and `None` mean different things (an empty
        // translation vs. no translation). The schema must preserve that.
        s.append("translate", Some(""), Some(""), None, false, None)
            .unwrap();
        s.append("translate", None, None, None, false, None)
            .unwrap();
        let rows = s.recent(10).unwrap();
        // Newest first → row with Nones first, row with empties second.
        assert!(rows[0].source_text.is_none());
        assert_eq!(rows[1].source_text.as_deref(), Some(""));
    }

    #[test]
    fn unicode_is_preserved_in_all_text_fields() {
        let s = Storage::open_in_memory().unwrap();
        s.append(
            "translate",
            Some("猫が鳴く 🐱"),
            Some("Cat meows 🐱"),
            Some("plugin.🔌"),
            true,
            Some("/tmp/猫.png"),
        )
        .unwrap();
        let e = &s.recent(1).unwrap()[0];
        assert_eq!(e.source_text.as_deref(), Some("猫が鳴く 🐱"));
        assert_eq!(e.result_text.as_deref(), Some("Cat meows 🐱"));
        assert_eq!(e.provider.as_deref(), Some("plugin.🔌"));
        assert_eq!(e.image_path.as_deref(), Some("/tmp/猫.png"));
    }

    #[test]
    fn prune_with_epoch_zero_removes_no_rows() {
        let s = Storage::open_in_memory().unwrap();
        s.append("translate", None, None, None, false, None)
            .unwrap();
        assert_eq!(s.prune_older_than(0).unwrap(), 0);
        assert_eq!(s.recent(10).unwrap().len(), 1);
    }

    #[test]
    fn prune_with_epoch_max_removes_everything() {
        let s = Storage::open_in_memory().unwrap();
        for _ in 0..4 {
            s.append("translate", None, None, None, false, None)
                .unwrap();
        }
        let deleted = s.prune_older_than(i64::MAX).unwrap();
        assert_eq!(deleted, 4);
        assert!(s.recent(10).unwrap().is_empty());
    }

    #[test]
    fn delete_removes_only_the_requested_row() {
        let s = Storage::open_in_memory().unwrap();
        let ids: Vec<i64> = (0..3)
            .map(|i| {
                s.append("translate", Some(&format!("s{i}")), None, None, false, None)
                    .unwrap()
            })
            .collect();
        let removed = s.delete(ids[1]).unwrap();
        assert_eq!(removed, 1);
        let remaining: Vec<i64> = s.recent(10).unwrap().into_iter().map(|e| e.id).collect();
        assert_eq!(remaining.len(), 2);
        assert!(!remaining.contains(&ids[1]));
        assert!(remaining.contains(&ids[0]));
        assert!(remaining.contains(&ids[2]));
    }

    #[test]
    fn restore_puts_a_deleted_row_back_unchanged() {
        let s = Storage::open_in_memory().unwrap();
        for i in 0..3 {
            s.append(
                "translate",
                Some(&format!("s{i}")),
                Some("r"),
                Some("p"),
                false,
                None,
            )
            .unwrap();
        }
        let before = s.recent(10).unwrap();
        let target = before[1].clone();
        assert_eq!(s.delete(target.id).unwrap(), 1);
        assert_eq!(s.restore(&target).unwrap(), 1);
        assert_eq!(s.recent(10).unwrap(), before);
        // 既存 id への復元は無処理（重複挿入・上書きなし）
        assert_eq!(s.restore(&target).unwrap(), 0);
        assert_eq!(s.recent(10).unwrap(), before);
    }

    #[test]
    fn delete_missing_id_is_a_noop() {
        let s = Storage::open_in_memory().unwrap();
        s.append("translate", Some("x"), None, None, false, None)
            .unwrap();
        // Removing a non-existent id must not panic and must not touch
        // any other row.
        assert_eq!(s.delete(9_999_999).unwrap(), 0);
        assert_eq!(s.recent(10).unwrap().len(), 1);
    }

    #[test]
    fn clear_all_empties_the_history() {
        let s = Storage::open_in_memory().unwrap();
        for _ in 0..5 {
            s.append("translate", None, None, None, false, None)
                .unwrap();
        }
        let removed = s.clear_all().unwrap();
        assert_eq!(removed, 5);
        assert!(s.recent(10).unwrap().is_empty());
    }

    #[test]
    fn clear_all_on_empty_db_returns_zero() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(s.clear_all().unwrap(), 0);
    }

    #[test]
    fn search_reaches_beyond_the_recent_window() {
        let s = Storage::open_in_memory().unwrap();
        s.append("translate", Some("needle 100%"), None, None, false, None)
            .unwrap();
        for i in 0..50 {
            s.append(
                "translate",
                Some(&format!("hay {i}")),
                None,
                None,
                false,
                None,
            )
            .unwrap();
        }
        let hits = s.search("NEEDLE", 10).unwrap();
        assert_eq!(hits.len(), 1);
        // LIKE wildcards in the query are literal.
        assert_eq!(s.search("100%", 10).unwrap().len(), 1);
        assert_eq!(s.search("1_0", 10).unwrap().len(), 0);
    }

    #[test]
    fn garbage_file_is_moved_aside_and_recreated() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("moonlens.db");
        std::fs::write(&db, b"this is not a sqlite database at all, just text....").unwrap();
        assert!(Storage::open_at(&db, dir.path()).is_err());
        let s = Storage::open_at_or_recover(db.clone(), dir.path().to_path_buf()).unwrap();
        s.append("translate", Some("x"), None, None, false, None)
            .unwrap();
        let aside = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().contains(".db.broken-"));
        assert!(aside);
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("moonlens.db");
        {
            let c = Connection::open(&db).unwrap();
            c.pragma_update(None, "user_version", Storage::SCHEMA_VERSION + 1)
                .unwrap();
        }
        assert!(Storage::open_at(&db, dir.path()).is_err());
    }

    #[test]
    fn very_long_text_round_trips() {
        // 1 MiB of text: common ceiling for pathological OCR output. The
        // schema must not truncate or reject.
        let big = "あ".repeat(300_000); // ~900 KB in UTF-8
        let s = Storage::open_in_memory().unwrap();
        s.append("translate", Some(&big), None, None, false, None)
            .unwrap();
        let e = &s.recent(1).unwrap()[0];
        assert_eq!(e.source_text.as_ref().map(|s| s.len()), Some(big.len()));
    }
}
