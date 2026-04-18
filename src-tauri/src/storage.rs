//! Persistent storage: history (rusqlite).
//!
//! `Storage::open` uses the app-data path, so tests use [`Storage::open_at`]
//! with a temp file. Unit tests exercise append + query to catch schema
//! regressions.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Manager};

pub struct Storage {
    db: Mutex<Connection>,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

    fn migrate(db: &Connection) -> Result<()> {
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
                now, kind, source_text, result_text, provider, via_cloud as i32, image_path
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

    pub fn prune_older_than(&self, epoch_seconds: i64) -> Result<usize> {
        let db = self.db.lock();
        let n = db.execute("DELETE FROM history WHERE created_at < ?1", [epoch_seconds])?;
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
        s.append("translate", None, None, None, false, None).unwrap();
        let deleted = s.prune_older_than(now_seconds() + 1).unwrap();
        assert_eq!(deleted, 1);
        assert!(s.recent(10).unwrap().is_empty());
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
        s.append("translate", None, None, None, false, None).unwrap();
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
        s.append("translate", None, None, None, false, None).unwrap();
        assert_eq!(s.prune_older_than(0).unwrap(), 0);
        assert_eq!(s.recent(10).unwrap().len(), 1);
    }

    #[test]
    fn prune_with_epoch_max_removes_everything() {
        let s = Storage::open_in_memory().unwrap();
        for _ in 0..4 {
            s.append("translate", None, None, None, false, None).unwrap();
        }
        let deleted = s.prune_older_than(i64::MAX).unwrap();
        assert_eq!(deleted, 4);
        assert!(s.recent(10).unwrap().is_empty());
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
