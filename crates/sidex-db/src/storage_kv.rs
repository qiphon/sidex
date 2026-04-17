//! Flat key-value store ported from the Tauri `storage` commands.
//!
//! This provides a simple `get / set / delete` API without scope prefixes,
//! mirroring the `kv_store` table used by the legacy `StorageDb`.  New code
//! should prefer [`crate::state::StateStore`] (which adds scoping), but this
//! module is provided for backward compatibility and for cases where a single
//! global namespace is sufficient.

use anyhow::{bail, Context, Result};
use rusqlite::params;

use crate::db::Database;

const MAX_KEY_LENGTH: usize = 256;
const MAX_VALUE_LENGTH: usize = 1_048_576; // 1 MiB

/// Flat (unscoped) key-value store backed by the `kv_store` table.
pub struct StorageKv<'db> {
    db: &'db Database,
}

impl<'db> StorageKv<'db> {
    pub fn new(db: &'db Database) -> Self {
        Self { db }
    }

    /// Retrieve a value by key, returning `None` if absent.
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .db
            .conn()
            .prepare_cached("SELECT value FROM kv_store WHERE key = ?1")
            .context("prepare kv get")?;

        let result = stmt
            .query_row(params![key], |row| row.get::<_, String>(0))
            .optional()
            .context("query kv get")?;

        Ok(result)
    }

    /// Set (upsert) a value.  Enforces size limits to prevent resource
    /// exhaustion (CWE-400).
    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        if key.len() > MAX_KEY_LENGTH {
            bail!(
                "key length exceeds maximum of {MAX_KEY_LENGTH} bytes (got {} bytes)",
                key.len()
            );
        }
        if value.len() > MAX_VALUE_LENGTH {
            bail!(
                "value length exceeds maximum of {MAX_VALUE_LENGTH} bytes (got {} bytes)",
                value.len()
            );
        }

        self.db
            .conn()
            .execute(
                "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .with_context(|| format!("kv set key '{key}'"))?;
        Ok(())
    }

    /// Delete a key.
    pub fn delete(&self, key: &str) -> Result<()> {
        self.db
            .conn()
            .execute("DELETE FROM kv_store WHERE key = ?1", params![key])
            .with_context(|| format!("kv delete key '{key}'"))?;
        Ok(())
    }

    /// List all keys, ordered alphabetically.
    pub fn keys(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .db
            .conn()
            .prepare_cached("SELECT key FROM kv_store ORDER BY key")
            .context("prepare kv keys")?;

        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .context("query kv keys")?;

        let mut keys = Vec::new();
        for row in rows {
            keys.push(row.context("read kv key row")?);
        }
        Ok(keys)
    }
}

trait OptionalExt<T> {
    fn optional(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let tmp = tempfile::TempDir::new().unwrap();
        Database::open(&tmp.path().join("test.db")).unwrap()
    }

    #[test]
    fn get_missing_returns_none() {
        let db = test_db();
        let kv = StorageKv::new(&db);
        assert!(kv.get("nope").unwrap().is_none());
    }

    #[test]
    fn set_and_get() {
        let db = test_db();
        let kv = StorageKv::new(&db);
        kv.set("theme", "dark").unwrap();
        assert_eq!(kv.get("theme").unwrap().unwrap(), "dark");
    }

    #[test]
    fn upsert_overwrites() {
        let db = test_db();
        let kv = StorageKv::new(&db);
        kv.set("k", "v1").unwrap();
        kv.set("k", "v2").unwrap();
        assert_eq!(kv.get("k").unwrap().unwrap(), "v2");
    }

    #[test]
    fn delete_key() {
        let db = test_db();
        let kv = StorageKv::new(&db);
        kv.set("k", "v").unwrap();
        kv.delete("k").unwrap();
        assert!(kv.get("k").unwrap().is_none());
    }

    #[test]
    fn list_keys() {
        let db = test_db();
        let kv = StorageKv::new(&db);
        kv.set("b", "2").unwrap();
        kv.set("a", "1").unwrap();
        let keys = kv.keys().unwrap();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn rejects_oversized_key() {
        let db = test_db();
        let kv = StorageKv::new(&db);
        let big_key = "x".repeat(MAX_KEY_LENGTH + 1);
        assert!(kv.set(&big_key, "v").is_err());
    }

    #[test]
    fn rejects_oversized_value() {
        let db = test_db();
        let kv = StorageKv::new(&db);
        let big_val = "x".repeat(MAX_VALUE_LENGTH + 1);
        assert!(kv.set("k", &big_val).is_err());
    }
}
