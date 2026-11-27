//! Cache of per-paragraph translations.
//!
//! Text that comes back (a menu, an item name, a line scrolled back to)
//! shows its translation at once instead of paying for another model
//! round-trip. Two layers:
//!
//! - memory: bounded, oldest entries evicted first, gone at exit;
//! - disk ([`DiskCache`], SQLite in the app data folder): survives a
//!   restart, so the menus of a game read yesterday cost nothing today.
//!
//! An entry is keyed by scope (the app a window follows; empty = no
//! app), target language, exact source paragraph and the lines it was
//! translated after (empty = none), and carries a
//! fingerprint of everything that shaped its translation (engine setup,
//! style, the glossary terms that occur in the paragraph). A lookup with
//! another fingerprint is a miss, so a changed model or glossary term is
//! never masked by old output, and only the paragraphs a glossary change
//! touches are translated again.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedTranslation {
    pub text: String,
    pub provider: String,
    pub via_cloud: bool,
}

/// What a cached translation is looked up by.
#[derive(Debug, Clone, Copy)]
pub struct Lookup<'a> {
    /// App the text was read from ([`crate::pipeline::CaptureFlowResult::app`]);
    /// empty when the capture belongs to no app.
    pub scope: &'a str,
    pub target: &'a str,
    pub source: &'a str,
    /// Digest of the lines the text was translated after
    /// ([`crate::pipeline::CaptureFlowResult::earlier`]); empty for text
    /// that stands alone. The same words after other lines are another
    /// entry: they can mean something else.
    pub context: &'a str,
    /// See [`fingerprint`].
    pub fingerprint: &'a str,
    /// Settings → storage.keep_translation_cache: read and write the
    /// disk layer too. Off: memory only.
    pub persist: bool,
}

type Key = (String, String, String, String);

impl Lookup<'_> {
    fn key(&self) -> Key {
        (
            self.scope.to_string(),
            self.target.trim().to_ascii_lowercase(),
            self.source.trim().to_string(),
            self.context.to_string(),
        )
    }
}

/// Short stable digest of the parts that shape a translation. Parts are
/// length-prefixed, so `["ab", "c"]` and `["a", "bc"]` differ.
pub fn fingerprint<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update((p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    h.finalize()[..12]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub struct TranslationCache {
    capacity: usize,
    inner: Mutex<Inner>,
    disk: Option<DiskCache>,
}

#[derive(Default)]
struct Inner {
    map: HashMap<Key, (String, CachedTranslation)>,
    order: VecDeque<Key>,
}

impl TranslationCache {
    pub const DEFAULT_CAPACITY: usize = 2000;

    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            inner: Mutex::new(Inner::default()),
            disk: None,
        }
    }

    /// Keep entries on disk too.
    pub fn with_disk(mut self, disk: DiskCache) -> Self {
        self.disk = Some(disk);
        self
    }

    pub fn get(&self, at: Lookup<'_>) -> Option<CachedTranslation> {
        let key = at.key();
        if let Some((fp, value)) = self.inner.lock().map.get(&key) {
            if fp == at.fingerprint {
                return Some(value.clone());
            }
        }
        let hit = self
            .disk
            .as_ref()
            .filter(|_| at.persist)?
            .get(&key, at.fingerprint)?;
        self.remember(key, at.fingerprint, hit.clone());
        Some(hit)
    }

    /// Store a translation. `durable` = worth keeping across restarts:
    /// the answer of the engine the user chose, not of a fallback that
    /// stood in for it.
    pub fn put(&self, at: Lookup<'_>, value: CachedTranslation, durable: bool) {
        if at.source.trim().is_empty() || value.text.trim().is_empty() {
            return;
        }
        let key = at.key();
        if let Some(disk) = self.disk.as_ref().filter(|_| durable && at.persist) {
            disk.put(&key, at.fingerprint, &value);
        }
        self.remember(key, at.fingerprint, value);
    }

    fn remember(&self, key: Key, fingerprint: &str, value: CachedTranslation) {
        let mut inner = self.inner.lock();
        if inner
            .map
            .insert(key.clone(), (fingerprint.to_string(), value))
            .is_none()
        {
            inner.order.push_back(key);
        }
        while inner.order.len() > self.capacity {
            if let Some(old) = inner.order.pop_front() {
                inner.map.remove(&old);
            }
        }
    }

    /// Forget the memory layer (what is on disk stays): a restart.
    #[cfg(test)]
    fn clear_memory(&self) {
        let mut inner = self.inner.lock();
        inner.map.clear();
        inner.order.clear();
    }

    /// Delete cached translations, on disk too: those of one scope, or
    /// all of them. Returns the number of entries removed from disk.
    pub fn clear(&self, scope: Option<&str>) -> Result<usize> {
        {
            let mut inner = self.inner.lock();
            let Inner { map, order } = &mut *inner;
            match scope {
                Some(s) => {
                    map.retain(|k, _| k.0 != s);
                    order.retain(|k| k.0 != s);
                }
                None => {
                    map.clear();
                    order.clear();
                }
            }
        }
        match &self.disk {
            Some(disk) => disk.clear(scope),
            None => Ok(0),
        }
    }

    /// What is stored on disk.
    pub fn usage(&self) -> Result<Usage> {
        match &self.disk {
            Some(disk) => disk.usage(),
            None => Ok(Usage::default()),
        }
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for TranslationCache {
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

/// Size of the disk cache, for the settings page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Usage {
    /// Size of the cache file in bytes.
    pub bytes: u64,
    pub entries: u64,
    /// Per scope, largest first. `scope` is empty for text that belongs
    /// to no app.
    pub scopes: Vec<ScopeUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeUsage {
    pub scope: String,
    pub entries: u64,
    /// Bytes of source text and translations.
    pub bytes: u64,
}

/// The disk layer. Every failure is logged and treated as a miss: a
/// cache must never stop a translation.
pub struct DiskCache {
    db: Mutex<Connection>,
    path: Option<PathBuf>,
    max_entries: u64,
    puts: AtomicU32,
}

impl DiskCache {
    pub const FILE_NAME: &'static str = "translation-cache.db";
    /// Entries kept at most; the least recently used go first. About
    /// 40 MB of menu-sized paragraphs.
    pub const MAX_ENTRIES: u64 = 200_000;
    /// Trim to the limit once per this many writes.
    const TRIM_EVERY: u32 = 256;
    /// A hit refreshes `last_used` only when it is older than this, so
    /// text on screen does not cause a write per look.
    const TOUCH_AFTER_SECS: i64 = 3600;

    /// Open the cache file, recreating it when it cannot be read: its
    /// content can always be translated again.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let db = match Self::connect(path) {
            Ok(db) => db,
            Err(e) => {
                tracing::warn!("translation cache unusable, recreating: {e:#}");
                for ext in ["db", "db-wal", "db-shm", "db-journal"] {
                    let _ = std::fs::remove_file(path.with_extension(ext));
                }
                Self::connect(path)?
            }
        };
        Ok(Self::over(db, Some(path.to_path_buf())))
    }

    pub fn open_in_memory() -> Result<Self> {
        let db = Connection::open_in_memory()?;
        Self::migrate(&db)?;
        Ok(Self::over(db, None))
    }

    fn over(db: Connection, path: Option<PathBuf>) -> Self {
        Self {
            db: Mutex::new(db),
            path,
            max_entries: Self::MAX_ENTRIES,
            puts: AtomicU32::new(0),
        }
    }

    pub fn with_max_entries(mut self, max: u64) -> Self {
        self.max_entries = max.max(1);
        self
    }

    fn connect(path: &Path) -> Result<Connection> {
        let db = Connection::open(path)?;
        Self::migrate(&db)?;
        Ok(db)
    }

    const SCHEMA_VERSION: i64 = 1;

    fn migrate(db: &Connection) -> Result<()> {
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > Self::SCHEMA_VERSION {
            anyhow::bail!("translation cache schema {version} is newer than this build");
        }
        // Losing the last writes in a crash costs a re-translation only.
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS entries (
                scope TEXT NOT NULL,
                target TEXT NOT NULL,
                source TEXT NOT NULL,
                context TEXT NOT NULL,
                fingerprint TEXT NOT NULL,
                text TEXT NOT NULL,
                provider TEXT NOT NULL,
                via_cloud INTEGER NOT NULL,
                last_used INTEGER NOT NULL,
                PRIMARY KEY (scope, target, source, context)
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS entries_last_used ON entries(last_used);",
        )?;
        db.pragma_update(None, "user_version", Self::SCHEMA_VERSION)?;
        Ok(())
    }

    fn get(&self, key: &Key, fingerprint: &str) -> Option<CachedTranslation> {
        let db = self.db.lock();
        let now = now_seconds();
        let row = db
            .query_row(
                "SELECT text, provider, via_cloud, last_used FROM entries
                 WHERE scope = ?1 AND target = ?2 AND source = ?3 AND context = ?4
                   AND fingerprint = ?5",
                params![key.0, key.1, key.2, key.3, fingerprint],
                |r| {
                    Ok((
                        CachedTranslation {
                            text: r.get(0)?,
                            provider: r.get(1)?,
                            via_cloud: r.get::<_, i64>(2)? != 0,
                        },
                        r.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional();
        match row {
            Ok(Some((value, last_used))) => {
                if now - last_used > Self::TOUCH_AFTER_SECS {
                    let _ = db.execute(
                        "UPDATE entries SET last_used = ?5
                         WHERE scope = ?1 AND target = ?2 AND source = ?3 AND context = ?4",
                        params![key.0, key.1, key.2, key.3, now],
                    );
                }
                Some(value)
            }
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("translation cache read failed: {e}");
                None
            }
        }
    }

    fn put(&self, key: &Key, fingerprint: &str, value: &CachedTranslation) {
        let db = self.db.lock();
        let stored = db.execute(
            "INSERT OR REPLACE INTO entries
                (scope, target, source, context, fingerprint, text, provider, via_cloud, last_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                key.0,
                key.1,
                key.2,
                key.3,
                fingerprint,
                value.text,
                value.provider,
                value.via_cloud as i64,
                now_seconds()
            ],
        );
        if let Err(e) = stored {
            tracing::warn!("translation cache write failed: {e}");
            return;
        }
        if self.puts.fetch_add(1, Ordering::Relaxed) % Self::TRIM_EVERY == Self::TRIM_EVERY - 1 {
            if let Err(e) = Self::trim(&db, self.max_entries) {
                tracing::warn!("translation cache trim failed: {e}");
            }
        }
    }

    /// Drop the least recently used entries beyond `max`.
    fn trim(db: &Connection, max: u64) -> rusqlite::Result<usize> {
        let count: i64 = db.query_row("SELECT COUNT(*) FROM entries", [], |r| r.get(0))?;
        let max = max.min(i64::MAX as u64) as i64;
        if count <= max {
            return Ok(0);
        }
        db.execute(
            "DELETE FROM entries WHERE (scope, target, source, context) IN (
                SELECT scope, target, source, context FROM entries
                ORDER BY last_used ASC LIMIT ?1)",
            params![count - max],
        )
    }

    fn clear(&self, scope: Option<&str>) -> Result<usize> {
        let db = self.db.lock();
        let n = match scope {
            Some(s) => db.execute("DELETE FROM entries WHERE scope = ?1", params![s])?,
            None => db.execute("DELETE FROM entries", [])?,
        };
        // Hand the space back to the disk; the page shows the file size.
        if let Err(e) = db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;") {
            tracing::warn!("translation cache vacuum failed: {e}");
        }
        Ok(n)
    }

    fn usage(&self) -> Result<Usage> {
        let db = self.db.lock();
        let mut stmt = db.prepare(
            "SELECT scope, COUNT(*),
                    COALESCE(SUM(LENGTH(CAST(source AS BLOB)) + LENGTH(CAST(text AS BLOB))), 0)
             FROM entries GROUP BY scope ORDER BY 3 DESC, scope",
        )?;
        let scopes = stmt
            .query_map([], |r| {
                Ok(ScopeUsage {
                    scope: r.get(0)?,
                    entries: r.get::<_, i64>(1)? as u64,
                    bytes: r.get::<_, i64>(2)? as u64,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let entries = scopes.iter().map(|s| s.entries).sum();
        let bytes = if entries == 0 {
            // An emptied file still has its schema pages: not worth a
            // number on the settings page.
            0
        } else {
            match &self.path {
                Some(p) => ["db", "db-wal"]
                    .iter()
                    .filter_map(|ext| std::fs::metadata(p.with_extension(ext)).ok())
                    .map(|m| m.len())
                    .sum(),
                None => scopes.iter().map(|s| s.bytes).sum(),
            }
        };
        Ok(Usage {
            bytes,
            entries,
            scopes,
        })
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

    fn v(t: &str) -> CachedTranslation {
        CachedTranslation {
            text: t.into(),
            provider: "p".into(),
            via_cloud: false,
        }
    }

    fn at<'a>(scope: &'a str, target: &'a str, source: &'a str, fp: &'a str) -> Lookup<'a> {
        Lookup {
            scope,
            target,
            source,
            context: "",
            fingerprint: fp,
            persist: true,
        }
    }

    fn on_disk() -> TranslationCache {
        TranslationCache::new(10).with_disk(DiskCache::open_in_memory().unwrap())
    }

    #[test]
    fn keyed_by_target_and_trimmed_source() {
        let c = TranslationCache::new(10);
        c.put(at("", "ja", " Hello ", "f"), v("こんにちは"), true);
        assert_eq!(
            c.get(at("", "JA", "Hello", "f")).unwrap().text,
            "こんにちは"
        );
        assert!(c.get(at("", "en", "Hello", "f")).is_none());
    }

    #[test]
    fn evicts_oldest_beyond_capacity() {
        let c = TranslationCache::new(2);
        c.put(at("", "ja", "a", "f"), v("A"), true);
        c.put(at("", "ja", "b", "f"), v("B"), true);
        c.put(at("", "ja", "a", "f"), v("A2"), true); // update keeps its slot
        c.put(at("", "ja", "c", "f"), v("C"), true);
        assert!(c.get(at("", "ja", "a", "f")).is_none());
        assert_eq!(c.get(at("", "ja", "b", "f")).unwrap().text, "B");
        assert_eq!(c.get(at("", "ja", "c", "f")).unwrap().text, "C");
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn ignores_empty_and_clears() {
        let c = TranslationCache::new(5);
        c.put(at("", "ja", "", "f"), v("x"), true);
        c.put(at("", "ja", "a", "f"), v("  "), true);
        assert!(c.is_empty());
        c.put(at("", "ja", "a", "f"), v("A"), true);
        c.clear(None).unwrap();
        assert!(c.get(at("", "ja", "a", "f")).is_none());
    }

    #[test]
    fn scopes_do_not_share_entries() {
        let c = on_disk();
        c.put(at("game", "ja", "Save", "f"), v("セーブ"), true);
        assert!(c.get(at("", "ja", "Save", "f")).is_none());
        assert!(c.get(at("editor", "ja", "Save", "f")).is_none());
        assert_eq!(c.get(at("game", "ja", "Save", "f")).unwrap().text, "セーブ");
    }

    #[test]
    fn another_fingerprint_is_a_miss_and_replaces_the_entry() {
        let c = on_disk();
        c.put(at("", "ja", "Sword", "old"), v("ソード"), true);
        assert!(c.get(at("", "ja", "Sword", "new")).is_none());
        c.put(at("", "ja", "Sword", "new"), v("剣"), true);
        c.clear_memory();
        assert_eq!(c.get(at("", "ja", "Sword", "new")).unwrap().text, "剣");
        assert!(c.get(at("", "ja", "Sword", "old")).is_none());
        assert_eq!(c.usage().unwrap().entries, 1);
    }

    #[test]
    fn disk_entries_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DiskCache::FILE_NAME);
        {
            let c = TranslationCache::new(10).with_disk(DiskCache::open(&path).unwrap());
            c.put(at("game", "ja", "New Game", "f"), v("ニューゲーム"), true);
            c.put(at("game", "ja", "Fallback", "f"), v("代替"), false);
        }
        let c = TranslationCache::new(10).with_disk(DiskCache::open(&path).unwrap());
        assert_eq!(
            c.get(at("game", "ja", "New Game", "f")).unwrap().text,
            "ニューゲーム"
        );
        // A fallback engine's answer lives for the session only.
        assert!(c.get(at("game", "ja", "Fallback", "f")).is_none());
    }

    #[test]
    fn persist_off_neither_reads_nor_writes_disk() {
        let c = on_disk();
        let memory_only = |source| Lookup {
            persist: false,
            ..at("", "ja", source, "f")
        };
        c.put(at("", "ja", "a", "f"), v("A"), true);
        c.clear_memory();
        assert!(c.get(memory_only("a")).is_none());
        c.put(memory_only("b"), v("B"), true);
        assert_eq!(c.get(memory_only("b")).unwrap().text, "B");
        c.clear_memory();
        assert!(c.get(at("", "ja", "b", "f")).is_none());
        assert_eq!(c.get(at("", "ja", "a", "f")).unwrap().text, "A");
    }

    #[test]
    fn usage_counts_per_scope_and_clear_removes_one_scope() {
        let c = on_disk();
        c.put(at("game", "ja", "abc", "f"), v("あ"), true);
        c.put(at("game", "ja", "de", "f"), v("い"), true);
        c.put(at("", "ja", "x", "f"), v("う"), true);
        let u = c.usage().unwrap();
        assert_eq!(u.entries, 3);
        assert_eq!(
            u.scopes,
            vec![
                ScopeUsage {
                    scope: "game".into(),
                    entries: 2,
                    bytes: 3 + 3 + 2 + 3
                },
                ScopeUsage {
                    scope: "".into(),
                    entries: 1,
                    bytes: 1 + 3
                },
            ]
        );
        assert_eq!(c.clear(Some("game")).unwrap(), 2);
        assert!(c.get(at("game", "ja", "abc", "f")).is_none());
        assert_eq!(c.get(at("", "ja", "x", "f")).unwrap().text, "う");
        assert_eq!(c.clear(None).unwrap(), 1);
        assert_eq!(c.usage().unwrap(), Usage::default());
    }

    #[test]
    fn trims_least_recently_used_beyond_the_limit() {
        let disk = DiskCache::open_in_memory().unwrap().with_max_entries(10);
        for i in 0..DiskCache::TRIM_EVERY {
            disk.put(
                &("".into(), "ja".into(), format!("s{i}"), "".into()),
                "f",
                &v("t"),
            );
        }
        assert_eq!(disk.usage().unwrap().entries, 10);
    }

    #[test]
    fn unreadable_file_is_recreated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DiskCache::FILE_NAME);
        std::fs::write(&path, b"not a database").unwrap();
        let c = TranslationCache::new(10).with_disk(DiskCache::open(&path).unwrap());
        c.put(at("", "ja", "a", "f"), v("A"), true);
        c.clear_memory();
        assert_eq!(c.get(at("", "ja", "a", "f")).unwrap().text, "A");
    }

    #[test]
    fn the_same_text_after_other_lines_is_another_entry() {
        let c = on_disk();
        let after = |context| Lookup {
            context,
            ..at("", "ja", "Left an hour ago.", "f")
        };
        c.put(after(""), v("出発は 1 時間前。"), true);
        c.put(after("tom"), v("彼は 1 時間前に出た。"), true);
        assert!(c.get(after("me")).is_none());
        c.clear_memory();
        assert_eq!(c.get(after("tom")).unwrap().text, "彼は 1 時間前に出た。");
        assert_eq!(c.get(after("")).unwrap().text, "出発は 1 時間前。");
    }

    #[test]
    fn fingerprint_separates_parts() {
        assert_ne!(fingerprint(["ab", "c"]), fingerprint(["a", "bc"]));
        assert_eq!(fingerprint(["a", "b"]), fingerprint(["a", "b"]));
    }
}
