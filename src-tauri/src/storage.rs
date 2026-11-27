//! Persistent storage: history (rusqlite).
//!
//! `Storage::open` uses the app-data path, so tests use [`Storage::open_at`]
//! with a temp file. Unit tests exercise append + query to catch schema
//! regressions.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};
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

/// One timed line of a live-subtitle record. Offsets count from the start
/// of the recognition run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleLine {
    /// 0-based order inside the record.
    pub seq: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub original: String,
    /// Empty when translation failed or was not needed.
    pub translated: String,
}

/// A live-subtitle run saved to history: the `history` row (kind
/// `subtitle`) holds the joined texts, this holds the timed lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleRecord {
    pub history_id: i64,
    /// Name of the captured device or application.
    pub source: String,
    pub source_language: String,
    pub target_language: String,
    /// Counts the changes to the lines. A writer that replaces all lines
    /// names the revision it read, so it cannot discard lines recorded
    /// meanwhile.
    #[serde(default)]
    pub revision: i64,
    pub lines: Vec<SubtitleLine>,
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
                    format!(
                        "履歴データベースを読込不可のため再作成。旧ファイル：{}",
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
                ON history(created_at DESC);
             -- Additive, so the schema version stays: an older build still
             -- opens the file and simply ignores these tables.
             CREATE TABLE IF NOT EXISTS subtitle_session (
                history_id INTEGER PRIMARY KEY,
                source TEXT NOT NULL,
                source_language TEXT NOT NULL,
                target_language TEXT NOT NULL,
                revision INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS subtitle_line (
                history_id INTEGER NOT NULL,
                seq INTEGER NOT NULL,
                start_ms INTEGER NOT NULL,
                end_ms INTEGER NOT NULL,
                original TEXT NOT NULL,
                translated TEXT NOT NULL,
                PRIMARY KEY (history_id, seq)
             );",
        )?;
        // An older build may have deleted history rows without knowing
        // about their subtitle records.
        Self::purge_subtitle_orphans(db)?;
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
        Self::purge_subtitle_orphans(&db)?;
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
        Self::purge_subtitle_orphans(&db)?;
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
        Ok(self.delete_keeping_record(id)?.0)
    }

    /// [`Storage::delete`], also returning the row's subtitle record as
    /// it was at that moment, for [`Storage::restore_with_record`]. Read
    /// and delete share one lock, so a run still recording cannot add a
    /// line in between that an undo would then lose.
    pub fn delete_keeping_record(&self, id: i64) -> Result<(usize, Option<SubtitleRecord>)> {
        let db = self.db.lock();
        let record = read_record(&db, id)?;
        let n = db.execute("DELETE FROM history WHERE id = ?1", [id])?;
        Self::purge_subtitle_orphans(&db)?;
        Ok((n, record))
    }

    /// Put a deleted row back under its original id and timestamp (undo
    /// of [`Storage::delete`]). Returns 0 when the id is already present.
    pub fn restore(&self, entry: &HistoryEntry) -> Result<usize> {
        self.restore_with_record(entry, None)
    }

    /// [`Storage::restore`] that also puts back the subtitle record of the
    /// row (taken before the delete). Nothing is written for the record
    /// when the id is already present.
    pub fn restore_with_record(
        &self,
        entry: &HistoryEntry,
        record: Option<&SubtitleRecord>,
    ) -> Result<usize> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        let n = tx.execute(
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
        if n > 0 {
            if let Some(r) = record {
                tx.execute(
                    "INSERT OR REPLACE INTO subtitle_session
                      (history_id, source, source_language, target_language, revision)
                      VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        entry.id,
                        r.source,
                        r.source_language,
                        r.target_language,
                        r.revision
                    ],
                )?;
                write_lines(&tx, entry.id, &r.lines)?;
                // `entry` may be older than the record (lines recorded
                // after the list was loaded): the lines decide the texts.
                rebuild_texts(&tx, entry.id)?;
                tx.execute(
                    "UPDATE subtitle_session SET revision = ?2 WHERE history_id = ?1",
                    rusqlite::params![entry.id, r.revision],
                )?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// The history row with this id.
    pub fn entry(&self, id: i64) -> Result<Option<HistoryEntry>> {
        let db = self.db.lock();
        let found = db
            .query_row(
                "SELECT id, created_at, kind, source_text, result_text, provider, via_cloud, image_path
                 FROM history WHERE id = ?1",
                [id],
                |row| {
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
                },
            )
            .optional()?;
        Ok(found)
    }

    /// Start the record of a live-subtitle run: the history row (kind
    /// `subtitle`, no text yet) and its session, in one transaction.
    pub fn begin_subtitle_record(
        &self,
        source: &str,
        source_language: &str,
        target_language: &str,
    ) -> Result<i64> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        tx.execute(
            "INSERT INTO history
              (created_at, kind, source_text, result_text, provider, via_cloud, image_path)
              VALUES (?1, ?2, '', '', NULL, 0, NULL)",
            rusqlite::params![now_seconds(), crate::pipeline::history_kind::SUBTITLE],
        )?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO subtitle_session (history_id, source, source_language, target_language)
              VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![id, source, source_language, target_language],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Append a line after the last one and extend the row's texts.
    /// Returns the line with its assigned `seq`.
    pub fn append_subtitle_line(
        &self,
        id: i64,
        start_ms: i64,
        end_ms: i64,
        original: &str,
        translated: &str,
    ) -> Result<SubtitleLine> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        require_session(&tx, id)?;
        let seq: i64 = tx.query_row(
            "SELECT COALESCE(MAX(seq) + 1, 0) FROM subtitle_line WHERE history_id = ?1",
            [id],
            |r| r.get(0),
        )?;
        let line = SubtitleLine {
            seq,
            start_ms,
            end_ms,
            original: original.to_string(),
            translated: translated.to_string(),
        };
        insert_line(&tx, id, &line)?;
        rebuild_texts(&tx, id)?;
        tx.commit()?;
        Ok(line)
    }

    /// Replace the lines `first_seq..=last_seq` (consecutive) by one line
    /// per entry of `texts` (original, translated). Together the new
    /// lines span from the first start to the last end, each a share in
    /// proportion to the length of its original. No entry: the lines
    /// just go. Later lines keep their order; seqs are renumbered from 0.
    /// `expected_len` is the number of lines the caller believes the
    /// record has: when it differs, the lines were replaced by someone
    /// else, the seqs mean other lines, and nothing is written.
    pub fn revise_subtitle_lines(
        &self,
        id: i64,
        first_seq: i64,
        last_seq: i64,
        texts: &[(&str, &str)],
        expected_len: usize,
    ) -> Result<()> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        require_session(&tx, id)?;
        let lines = read_lines(&tx, id)?;
        if lines.len() != expected_len {
            anyhow::bail!(
                "subtitle record {id} has {} lines, not {expected_len}",
                lines.len()
            );
        }
        let (inside, outside): (Vec<_>, Vec<_>) = lines
            .into_iter()
            .partition(|l| (first_seq..=last_seq).contains(&l.seq));
        let (Some(first), Some(last)) = (inside.first(), inside.last()) else {
            anyhow::bail!("no subtitle lines in {first_seq}..={last_seq}");
        };
        let (start, span) = (first.start_ms, last.end_ms - first.start_ms);
        let weight = |t: &(&str, &str)| t.0.chars().count().max(1) as i64;
        let total: i64 = texts.iter().map(weight).sum();
        let mut out = outside;
        let mut before = 0;
        for text in texts {
            let from = start + span * before / total;
            before += weight(text);
            out.push(SubtitleLine {
                seq: first.seq,
                start_ms: from,
                end_ms: start + span * before / total,
                original: text.0.to_string(),
                translated: text.1.to_string(),
            });
        }
        // Stable: the new lines stay in their order at the first seq.
        out.sort_by_key(|l| l.seq);
        write_lines(&tx, id, &out)?;
        rebuild_texts(&tx, id)?;
        tx.commit()?;
        Ok(())
    }

    /// Replace every line of the record, numbering them from 0 in the
    /// given order, and rebuild the row's texts. `revision` is the one
    /// the caller read the lines at: when the record changed since (a
    /// run still recording appended a line), nothing is written and the
    /// call fails, so no recorded line is lost. Returns the new
    /// revision. Also fails when the row has no record.
    pub fn replace_subtitle_lines(
        &self,
        id: i64,
        revision: i64,
        lines: &[SubtitleLine],
    ) -> Result<i64> {
        let mut db = self.db.lock();
        let tx = db.transaction()?;
        require_session(&tx, id)?;
        if revision_of(&tx, id)? != revision {
            anyhow::bail!("字幕記録の更新を検出（記録中または他操作）。再読込後に再実行");
        }
        write_lines(&tx, id, lines)?;
        rebuild_texts(&tx, id)?;
        let next = revision_of(&tx, id)?;
        tx.commit()?;
        Ok(next)
    }

    /// The subtitle record of a history row, `None` when it has none.
    pub fn subtitle_record(&self, id: i64) -> Result<Option<SubtitleRecord>> {
        read_record(&self.db.lock(), id)
    }

    /// Remove session and line rows whose history row is gone. Every path
    /// that deletes history rows calls this, so none leaves a record behind.
    fn purge_subtitle_orphans(db: &Connection) -> Result<()> {
        db.execute_batch(
            "DELETE FROM subtitle_line WHERE history_id NOT IN (SELECT id FROM history);
             DELETE FROM subtitle_session WHERE history_id NOT IN (SELECT id FROM history);",
        )?;
        Ok(())
    }

    /// Drop every history row. Returns the number of rows removed.
    pub fn clear_all(&self) -> Result<usize> {
        let db = self.db.lock();
        let n = db.execute("DELETE FROM history", [])?;
        Self::purge_subtitle_orphans(&db)?;
        Ok(n)
    }
}

fn read_record(db: &Connection, id: i64) -> Result<Option<SubtitleRecord>> {
    let session = db
        .query_row(
            "SELECT source, source_language, target_language, revision
             FROM subtitle_session WHERE history_id = ?1",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((source, source_language, target_language, revision)) = session else {
        return Ok(None);
    };
    Ok(Some(SubtitleRecord {
        history_id: id,
        source,
        source_language,
        target_language,
        revision,
        lines: read_lines(db, id)?,
    }))
}

fn revision_of(db: &Connection, id: i64) -> Result<i64> {
    Ok(db.query_row(
        "SELECT revision FROM subtitle_session WHERE history_id = ?1",
        [id],
        |r| r.get(0),
    )?)
}

fn require_session(db: &Connection, id: i64) -> Result<()> {
    let found: Option<i64> = db
        .query_row(
            "SELECT history_id FROM subtitle_session WHERE history_id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    found
        .map(|_| ())
        .with_context(|| format!("history entry {id} has no subtitle record"))
}

fn read_lines(db: &Connection, id: i64) -> Result<Vec<SubtitleLine>> {
    let mut stmt = db.prepare(
        "SELECT seq, start_ms, end_ms, original, translated
         FROM subtitle_line WHERE history_id = ?1 ORDER BY seq",
    )?;
    let rows = stmt.query_map([id], |r| {
        Ok(SubtitleLine {
            seq: r.get(0)?,
            start_ms: r.get(1)?,
            end_ms: r.get(2)?,
            original: r.get(3)?,
            translated: r.get(4)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn insert_line(db: &Connection, id: i64, line: &SubtitleLine) -> Result<()> {
    db.execute(
        "INSERT INTO subtitle_line (history_id, seq, start_ms, end_ms, original, translated)
          VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            id,
            line.seq,
            line.start_ms,
            line.end_ms,
            line.original,
            line.translated
        ],
    )?;
    Ok(())
}

/// Replace all lines of `id`, numbering them from 0 in the given order.
fn write_lines(db: &Connection, id: i64, lines: &[SubtitleLine]) -> Result<()> {
    db.execute("DELETE FROM subtitle_line WHERE history_id = ?1", [id])?;
    for (seq, line) in lines.iter().enumerate() {
        insert_line(
            db,
            id,
            &SubtitleLine {
                seq: seq as i64,
                ..line.clone()
            },
        )?;
    }
    Ok(())
}

/// The history row's texts: originals and non-empty translations, one
/// per line, in line order.
fn rebuild_texts(db: &Connection, id: i64) -> Result<()> {
    let lines = read_lines(db, id)?;
    let join = |texts: Vec<&str>| texts.join("\n");
    let source = join(
        lines
            .iter()
            .map(|l| l.original.as_str())
            .filter(|t| !t.is_empty())
            .collect(),
    );
    let result = join(
        lines
            .iter()
            .map(|l| l.translated.as_str())
            .filter(|t| !t.is_empty())
            .collect(),
    );
    db.execute(
        "UPDATE history SET source_text = ?2, result_text = ?3 WHERE id = ?1",
        rusqlite::params![id, source, result],
    )?;
    // Every change to the lines ends here: count it.
    db.execute(
        "UPDATE subtitle_session SET revision = revision + 1 WHERE history_id = ?1",
        [id],
    )?;
    Ok(())
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

    fn line(
        seq: i64,
        start_ms: i64,
        end_ms: i64,
        original: &str,
        translated: &str,
    ) -> SubtitleLine {
        SubtitleLine {
            seq,
            start_ms,
            end_ms,
            original: original.into(),
            translated: translated.into(),
        }
    }

    fn record_with_three_lines(s: &Storage) -> i64 {
        let id = s.begin_subtitle_record("スピーカー", "auto", "ja").unwrap();
        s.append_subtitle_line(id, 0, 1000, "one", "一").unwrap();
        s.append_subtitle_line(id, 1000, 2500, "two", "").unwrap();
        s.append_subtitle_line(id, 2500, 4000, "three", "三")
            .unwrap();
        id
    }

    fn table_count(s: &Storage, table: &str) -> i64 {
        s.db.lock()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn subtitle_record_round_trips_with_history_row() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        let row = s.entry(id).unwrap().unwrap();
        assert_eq!(row.kind, "subtitle");
        assert_eq!(row.source_text.as_deref(), Some("one\ntwo\nthree"));
        // The empty translation is not joined.
        assert_eq!(row.result_text.as_deref(), Some("一\n三"));
        assert_eq!(s.recent(10).unwrap().len(), 1);
        let rec = s.subtitle_record(id).unwrap().unwrap();
        assert_eq!(rec.history_id, id);
        assert_eq!(rec.source, "スピーカー");
        assert_eq!(rec.source_language, "auto");
        assert_eq!(rec.target_language, "ja");
        assert_eq!(
            rec.lines,
            vec![
                line(0, 0, 1000, "one", "一"),
                line(1, 1000, 2500, "two", ""),
                line(2, 2500, 4000, "three", "三"),
            ]
        );
    }

    #[test]
    fn begun_record_is_empty_and_ordinary_rows_have_none() {
        let s = Storage::open_in_memory().unwrap();
        let id = s.begin_subtitle_record("mic", "en", "ja").unwrap();
        assert!(s.subtitle_record(id).unwrap().unwrap().lines.is_empty());
        let plain = s
            .append("translate", Some("x"), None, None, false, None)
            .unwrap();
        assert!(s.subtitle_record(plain).unwrap().is_none());
        assert!(s.subtitle_record(9_999).unwrap().is_none());
    }

    #[test]
    fn append_to_a_missing_record_fails() {
        let s = Storage::open_in_memory().unwrap();
        let plain = s
            .append("translate", Some("x"), None, None, false, None)
            .unwrap();
        assert!(s.append_subtitle_line(plain, 0, 1, "a", "b").is_err());
        assert!(s.append_subtitle_line(404, 0, 1, "a", "b").is_err());
    }

    #[test]
    fn merging_consecutive_lines_spans_them_and_keeps_order() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        s.revise_subtitle_lines(id, 0, 1, &[("one two", "一二")], 3)
            .unwrap();
        let rec = s.subtitle_record(id).unwrap().unwrap();
        assert_eq!(
            rec.lines,
            vec![
                line(0, 0, 2500, "one two", "一二"),
                line(1, 2500, 4000, "three", "三"),
            ]
        );
        let row = s.entry(id).unwrap().unwrap();
        assert_eq!(row.source_text.as_deref(), Some("one two\nthree"));
        assert_eq!(row.result_text.as_deref(), Some("一二\n三"));
        // A later append continues after the renumbered lines.
        let next = s
            .append_subtitle_line(id, 4000, 5000, "four", "四")
            .unwrap();
        assert_eq!(next.seq, 2);
    }

    #[test]
    fn merging_the_middle_or_a_missing_range() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        assert!(s.revise_subtitle_lines(id, 5, 6, &[("x", "y")], 3).is_err());
        s.revise_subtitle_lines(id, 1, 1, &[("TWO", "二")], 3)
            .unwrap();
        // The lines are not the ones the caller knows.
        assert!(s.revise_subtitle_lines(id, 1, 1, &[("x", "y")], 2).is_err());
        let rec = s.subtitle_record(id).unwrap().unwrap();
        assert_eq!(rec.lines[1], line(1, 1000, 2500, "TWO", "二"));
        assert_eq!(rec.lines.len(), 3);
    }

    #[test]
    fn revising_into_two_lines_shares_the_time_and_into_none_removes() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        s.revise_subtitle_lines(id, 0, 1, &[("abcd", "ア"), ("e", "")], 3)
            .unwrap();
        let rec = s.subtitle_record(id).unwrap().unwrap();
        assert_eq!(
            rec.lines,
            vec![
                line(0, 0, 2000, "abcd", "ア"),
                line(1, 2000, 2500, "e", ""),
                line(2, 2500, 4000, "three", "三"),
            ]
        );
        s.revise_subtitle_lines(id, 0, 1, &[], 3).unwrap();
        let rec = s.subtitle_record(id).unwrap().unwrap();
        assert_eq!(rec.lines, vec![line(0, 2500, 4000, "three", "三")]);
        let row = s.entry(id).unwrap().unwrap();
        assert_eq!(row.source_text.as_deref(), Some("three"));
    }

    #[test]
    fn replacing_lines_renumbers_and_rebuilds_texts() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        let new = vec![
            line(7, 0, 500, "alpha", "ア"),
            line(3, 500, 900, "beta", ""),
        ];
        let read = s.subtitle_record(id).unwrap().unwrap().revision;
        let written = s.replace_subtitle_lines(id, read, &new).unwrap();
        let rec = s.subtitle_record(id).unwrap().unwrap();
        assert_eq!(rec.revision, written);
        assert_eq!(
            rec.lines,
            vec![
                line(0, 0, 500, "alpha", "ア"),
                line(1, 500, 900, "beta", "")
            ]
        );
        let row = s.entry(id).unwrap().unwrap();
        assert_eq!(row.source_text.as_deref(), Some("alpha\nbeta"));
        assert_eq!(row.result_text.as_deref(), Some("ア"));
        // Replacing with nothing empties the record but keeps it.
        s.replace_subtitle_lines(id, written, &[]).unwrap();
        assert!(s.subtitle_record(id).unwrap().unwrap().lines.is_empty());
    }

    #[test]
    fn deleting_hands_back_the_record_as_it_was_for_undo() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        let entry = s.entry(id).unwrap().unwrap();
        let (n, record) = s.delete_keeping_record(id).unwrap();
        assert_eq!(n, 1);
        assert_eq!(record.as_ref().unwrap().lines.len(), 3);
        assert!(s.subtitle_record(id).unwrap().is_none());
        s.restore_with_record(&entry, record.as_ref()).unwrap();
        assert_eq!(s.subtitle_record(id).unwrap(), record);
        // A row without a record, and a missing row.
        let plain = s
            .append("text", Some("a"), None, None, false, None)
            .unwrap();
        assert_eq!(s.delete_keeping_record(plain).unwrap(), (1, None));
        assert_eq!(s.delete_keeping_record(plain).unwrap(), (0, None));
    }

    #[test]
    fn restoring_with_an_older_row_takes_the_texts_from_the_lines() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        // The list was loaded, then one more line was recorded.
        let stale = s.entry(id).unwrap().unwrap();
        s.append_subtitle_line(id, 4000, 5000, "four", "四")
            .unwrap();
        let current = s.entry(id).unwrap().unwrap();
        let (_, record) = s.delete_keeping_record(id).unwrap();
        s.restore_with_record(&stale, record.as_ref()).unwrap();
        assert_eq!(s.entry(id).unwrap().unwrap(), current);
        assert_eq!(s.subtitle_record(id).unwrap(), record);
        assert_eq!(s.search("four", 10).unwrap().len(), 1);
    }

    #[test]
    fn replacing_lines_read_before_a_new_line_was_recorded_changes_nothing() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        let read = s.subtitle_record(id).unwrap().unwrap();
        // The run is still recording while the lines are re-translated.
        s.append_subtitle_line(id, 4000, 5000, "four", "四")
            .unwrap();
        assert!(s
            .replace_subtitle_lines(id, read.revision, &read.lines[..1])
            .is_err());
        assert_eq!(s.subtitle_record(id).unwrap().unwrap().lines.len(), 4);
    }

    #[test]
    fn replacing_lines_without_a_record_fails_and_writes_nothing() {
        let s = Storage::open_in_memory().unwrap();
        let plain = s
            .append("translate", Some("x"), Some("y"), None, false, None)
            .unwrap();
        assert!(s
            .replace_subtitle_lines(plain, 0, &[line(0, 0, 1, "a", "b")])
            .is_err());
        assert_eq!(table_count(&s, "subtitle_line"), 0);
        let row = s.entry(plain).unwrap().unwrap();
        assert_eq!(row.source_text.as_deref(), Some("x"));
    }

    #[test]
    fn every_delete_path_removes_the_record() {
        // delete
        let s = Storage::open_in_memory().unwrap();
        let keep = record_with_three_lines(&s);
        let gone = record_with_three_lines(&s);
        s.delete(gone).unwrap();
        assert!(s.subtitle_record(gone).unwrap().is_none());
        assert!(s.subtitle_record(keep).unwrap().is_some());
        assert_eq!(table_count(&s, "subtitle_session"), 1);
        assert_eq!(table_count(&s, "subtitle_line"), 3);

        // prune_to_count
        let s = Storage::open_in_memory().unwrap();
        let old = record_with_three_lines(&s);
        let new = record_with_three_lines(&s);
        s.prune_to_count(1).unwrap();
        assert!(s.subtitle_record(old).unwrap().is_none());
        assert!(s.subtitle_record(new).unwrap().is_some());
        assert_eq!(table_count(&s, "subtitle_line"), 3);

        // prune_older_than
        let s = Storage::open_in_memory().unwrap();
        record_with_three_lines(&s);
        s.prune_older_than(now_seconds() + 1).unwrap();
        assert_eq!(table_count(&s, "subtitle_session"), 0);
        assert_eq!(table_count(&s, "subtitle_line"), 0);

        // clear_all
        let s = Storage::open_in_memory().unwrap();
        record_with_three_lines(&s);
        record_with_three_lines(&s);
        s.clear_all().unwrap();
        assert_eq!(table_count(&s, "subtitle_session"), 0);
        assert_eq!(table_count(&s, "subtitle_line"), 0);
    }

    #[test]
    fn restore_with_record_puts_everything_back() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        let entry = s.entry(id).unwrap().unwrap();
        let record = s.subtitle_record(id).unwrap().unwrap();
        s.delete(id).unwrap();
        assert_eq!(s.restore_with_record(&entry, Some(&record)).unwrap(), 1);
        assert_eq!(s.entry(id).unwrap().unwrap(), entry);
        assert_eq!(s.subtitle_record(id).unwrap().unwrap(), record);
        // Already present: nothing is written again.
        let read = s.subtitle_record(id).unwrap().unwrap().revision;
        s.replace_subtitle_lines(id, read, &[]).unwrap();
        assert_eq!(s.restore_with_record(&entry, Some(&record)).unwrap(), 0);
        assert!(s.subtitle_record(id).unwrap().unwrap().lines.is_empty());
    }

    #[test]
    fn restore_without_a_record_leaves_no_session() {
        let s = Storage::open_in_memory().unwrap();
        let id = record_with_three_lines(&s);
        let entry = s.entry(id).unwrap().unwrap();
        s.delete(id).unwrap();
        assert_eq!(s.restore(&entry).unwrap(), 1);
        assert!(s.subtitle_record(id).unwrap().is_none());
    }

    #[test]
    fn opening_purges_records_of_rows_an_older_build_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("moonlens.db");
        {
            let s = Storage::open_at(&path, dir.path()).unwrap();
            let id = record_with_three_lines(&s);
            // What an older build does: delete the row only.
            s.db.lock()
                .execute("DELETE FROM history WHERE id = ?1", [id])
                .unwrap();
            assert_eq!(table_count(&s, "subtitle_line"), 3);
        }
        let s = Storage::open_at(&path, dir.path()).unwrap();
        assert_eq!(table_count(&s, "subtitle_session"), 0);
        assert_eq!(table_count(&s, "subtitle_line"), 0);
    }

    #[test]
    fn subtitle_tables_do_not_change_the_schema_version() {
        let s = Storage::open_in_memory().unwrap();
        let v: i64 =
            s.db.lock()
                .query_row("PRAGMA user_version", [], |r| r.get(0))
                .unwrap();
        assert_eq!(v, 1);
    }
}
