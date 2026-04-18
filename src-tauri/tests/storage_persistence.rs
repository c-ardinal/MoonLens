//! File-backed storage round-trips. Unit tests use an in-memory SQLite
//! which cannot catch migration or on-disk format bugs; this suite opens
//! a real file, closes it, reopens it, and verifies data survives.

use moonlens_lib::storage::Storage;

#[test]
fn rows_persist_across_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("moonlens.db");

    let storage = Storage::open_at(&db_path, tmp.path()).unwrap();
    for i in 0..25 {
        storage
            .append(
                "translate",
                Some(&format!("src{i}")),
                Some(&format!("tgt{i}")),
                Some("ollama.gemma4"),
                false,
                None,
            )
            .unwrap();
    }
    drop(storage);

    // Reopen using the same path; migrations run again but data survives.
    let reopened = Storage::open_at(&db_path, tmp.path()).unwrap();
    let rows = reopened.recent(100).unwrap();
    assert_eq!(rows.len(), 25);
    // Newest first; the last appended row has source `src24`.
    assert_eq!(rows[0].source_text.as_deref(), Some("src24"));
}

#[test]
fn migration_is_idempotent_on_repeated_opens() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("moonlens.db");
    // Open/close three times; each open runs CREATE TABLE IF NOT EXISTS.
    for _ in 0..3 {
        let s = Storage::open_at(&db_path, tmp.path()).unwrap();
        s.append("translate", Some("x"), None, None, false, None)
            .unwrap();
        drop(s);
    }
    let final_storage = Storage::open_at(&db_path, tmp.path()).unwrap();
    assert_eq!(final_storage.recent(100).unwrap().len(), 3);
}

#[test]
fn prune_before_reopen_deletes_rows_persistently() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("moonlens.db");
    {
        let s = Storage::open_at(&db_path, tmp.path()).unwrap();
        for _ in 0..5 {
            s.append("translate", Some("x"), None, None, false, None)
                .unwrap();
        }
        // Delete everything by pruning far into the future.
        s.prune_older_than(i64::MAX).unwrap();
    }
    let reopened = Storage::open_at(&db_path, tmp.path()).unwrap();
    assert!(reopened.recent(100).unwrap().is_empty());
}
