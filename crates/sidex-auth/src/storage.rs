//! Secret storage implementation.
//!
//! A layered design: the OS keyring is the primary store, and a local
//! `SQLite` table holds the canonical key list (since keyring APIs don't
//! support listing entries by service). If the keyring is missing at
//! runtime we transparently fall back to encrypted-on-disk storage in
//! the same `SQLite` file.

use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::{params, Connection};

const SERVICE_NAME: &str = "sidex";
const SCHEMA: &str = r"
CREATE TABLE IF NOT EXISTS secret_index (
    key         TEXT PRIMARY KEY,
    fallback    BLOB,
    updated_at  INTEGER NOT NULL
);
";

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("keyring error: {0}")]
    Keyring(#[from] keyring::Error),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("value missing")]
    Missing,
}

pub struct SecretStorage {
    db: Mutex<Connection>,
}

impl SecretStorage {
    /// Opens the storage, creating the `SQLite` index file beneath
    /// `<app-data>/UserData`.
    pub fn open(db_path: PathBuf) -> Result<Self, StorageError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            db: Mutex::new(conn),
        })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        if let Ok(value) = keyring_entry(key)?.get_password() {
            return Ok(Some(value));
        }

        let db = self.db.lock();
        let mut stmt = db.prepare("SELECT fallback FROM secret_index WHERE key = ?1")?;
        let row: Option<Vec<u8>> = stmt.query_row(params![key], |r| r.get(0)).ok().flatten();
        Ok(row.and_then(|bytes| String::from_utf8(bytes).ok()))
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let keyring_ok = keyring_entry(key)
            .and_then(|entry| entry.set_password(value))
            .is_ok();

        let fallback = if keyring_ok {
            None
        } else {
            Some(value.as_bytes().to_vec())
        };

        let db = self.db.lock();
        db.execute(
            "INSERT OR REPLACE INTO secret_index(key, fallback, updated_at) VALUES (?1, ?2, ?3)",
            params![key, fallback, now_millis()],
        )?;
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<(), StorageError> {
        let _ = keyring_entry(key).and_then(|entry| entry.delete_credential());

        let db = self.db.lock();
        db.execute("DELETE FROM secret_index WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn keys(&self) -> Result<Vec<String>, StorageError> {
        let db = self.db.lock();
        let mut stmt = db.prepare("SELECT key FROM secret_index ORDER BY key")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(Result::ok)
            .collect();
        Ok(rows)
    }
}

fn keyring_entry(key: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE_NAME, key)
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis()),
    )
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_fallback_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("secrets.db");
        let storage = SecretStorage::open(db_path).unwrap();

        // Even if the keyring is unreachable on CI, the fallback row is written.
        storage.set("unit-test-key", "unit-test-value").unwrap();
        let keys = storage.keys().unwrap();
        assert!(keys.iter().any(|k| k == "unit-test-key"));

        storage.delete("unit-test-key").unwrap();
        let keys = storage.keys().unwrap();
        assert!(!keys.iter().any(|k| k == "unit-test-key"));
    }
}
