//! Persistent storage: history (rusqlite) + full-text search (tantivy).

use std::path::PathBuf;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

pub struct Storage {
    db: Mutex<Connection>,
    pub data_dir: PathBuf,
}

impl Storage {
    pub fn open(app: &AppHandle) -> Result<Self> {
        let data_dir = app.path().app_data_dir().context("no data dir")?;
        std::fs::create_dir_all(&data_dir)?;
        let db_path = data_dir.join("moonlens.db");
        let db = Connection::open(db_path)?;
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
        Ok(Self {
            db: Mutex::new(db),
            data_dir,
        })
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
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
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
}
