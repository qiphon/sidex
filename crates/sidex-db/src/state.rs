//! Generic key-value state store backed by `SQLite`.
//!
//! Keys are scoped so that different subsystems (global settings, per-workspace
//! data, per-extension data) can coexist without collision.  Typical scopes:
//!
//! - `"global"`
//! - `"workspace:<path>"`
//! - `"extension:<id>"`

use anyhow::{Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::db::Database;

/// Scoped key-value state store (backed by `state_kv` table).
pub struct StateStore<'db> {
    db: &'db Database,
}

impl<'db> StateStore<'db> {
    /// Creates a `StateStore` backed by the given database.
    pub fn new(db: &'db Database) -> Self {
        Self { db }
    }

    /// Retrieves a value for `(scope, key)`, returning `None` if absent.
    pub fn get(&self, scope: &str, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .db
            .conn()
            .prepare_cached("SELECT value FROM state_kv WHERE scope = ?1 AND key = ?2")
            .context("prepare get")?;

        let result = stmt
            .query_row(params![scope, key], |row| row.get::<_, String>(0))
            .optional()
            .context("query get")?;

        Ok(result)
    }

    /// Sets (upserts) the value for `(scope, key)`.
    pub fn set(&self, scope: &str, key: &str, value: &str) -> Result<()> {
        self.db
            .conn()
            .execute(
                "INSERT INTO state_kv (scope, key, value) VALUES (?1, ?2, ?3)
                 ON CONFLICT(scope, key) DO UPDATE SET value = excluded.value",
                params![scope, key, value],
            )
            .context("upsert state")?;
        Ok(())
    }

    /// Deletes the entry for `(scope, key)`.
    pub fn delete(&self, scope: &str, key: &str) -> Result<()> {
        self.db
            .conn()
            .execute(
                "DELETE FROM state_kv WHERE scope = ?1 AND key = ?2",
                params![scope, key],
            )
            .context("delete state")?;
        Ok(())
    }

    /// Returns all keys in the given scope.
    pub fn keys(&self, scope: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .db
            .conn()
            .prepare_cached("SELECT key FROM state_kv WHERE scope = ?1 ORDER BY key")
            .context("prepare keys")?;

        let rows = stmt
            .query_map(params![scope], |row| row.get::<_, String>(0))
            .context("query keys")?;

        let mut keys = Vec::new();
        for row in rows {
            keys.push(row.context("read key row")?);
        }
        Ok(keys)
    }
}

// ── Workspace state ─────────────────────────────────────────────────────────

/// Get a workspace-scoped state value (open files, scroll positions, etc.).
pub fn get_workspace_state(db: &Database, workspace: &str, key: &str) -> Result<Option<Value>> {
    let mut stmt = db
        .conn()
        .prepare_cached("SELECT value FROM workspace_state WHERE workspace = ?1 AND key = ?2")
        .context("prepare get_workspace_state")?;

    let result = stmt
        .query_row(params![workspace, key], |row| row.get::<_, String>(0))
        .optional()
        .context("query get_workspace_state")?;

    match result {
        Some(s) => {
            let v: Value = serde_json::from_str(&s).unwrap_or(Value::String(s));
            Ok(Some(v))
        }
        None => Ok(None),
    }
}

/// Set a workspace-scoped state value.
pub fn set_workspace_state(db: &Database, workspace: &str, key: &str, value: &Value) -> Result<()> {
    let json = serde_json::to_string(value).context("serialize workspace state")?;
    db.conn()
        .execute(
            "INSERT INTO workspace_state (workspace, key, value, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(workspace, key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            params![workspace, key, json],
        )
        .context("upsert workspace_state")?;
    Ok(())
}

/// Delete a workspace-scoped state value.
pub fn delete_workspace_state(db: &Database, workspace: &str, key: &str) -> Result<()> {
    db.conn()
        .execute(
            "DELETE FROM workspace_state WHERE workspace = ?1 AND key = ?2",
            params![workspace, key],
        )
        .context("delete workspace_state")?;
    Ok(())
}

// ── Global state ────────────────────────────────────────────────────────────

/// Get a global state value (recently opened, theme, sidebar state, etc.).
pub fn get_global_state(db: &Database, key: &str) -> Result<Option<Value>> {
    let mut stmt = db
        .conn()
        .prepare_cached("SELECT value FROM global_state WHERE key = ?1")
        .context("prepare get_global_state")?;

    let result = stmt
        .query_row(params![key], |row| row.get::<_, String>(0))
        .optional()
        .context("query get_global_state")?;

    match result {
        Some(s) => {
            let v: Value = serde_json::from_str(&s).unwrap_or(Value::String(s));
            Ok(Some(v))
        }
        None => Ok(None),
    }
}

/// Set a global state value.
pub fn set_global_state(db: &Database, key: &str, value: &Value) -> Result<()> {
    let json = serde_json::to_string(value).context("serialize global state")?;
    db.conn()
        .execute(
            "INSERT INTO global_state (key, value, updated_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            params![key, json],
        )
        .context("upsert global_state")?;
    Ok(())
}

/// Delete a global state value.
pub fn delete_global_state(db: &Database, key: &str) -> Result<()> {
    db.conn()
        .execute("DELETE FROM global_state WHERE key = ?1", params![key])
        .context("delete global_state")?;
    Ok(())
}

// ── Extension state ─────────────────────────────────────────────────────────

/// Scope for extension state storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateScope {
    Global,
    Workspace(String),
}

impl StateScope {
    fn scope_key(&self) -> String {
        match self {
            StateScope::Global => "global".to_owned(),
            StateScope::Workspace(ws) => format!("workspace:{ws}"),
        }
    }
}

/// Get an extension's stored state.
pub fn get_extension_state(
    db: &Database,
    extension_id: &str,
    key: &str,
    scope: &StateScope,
) -> Result<Option<Value>> {
    let scope_key = scope.scope_key();
    let mut stmt = db
        .conn()
        .prepare_cached(
            "SELECT value FROM extension_state WHERE extension_id = ?1 AND scope = ?2 AND key = ?3",
        )
        .context("prepare get_extension_state")?;

    let result = stmt
        .query_row(params![extension_id, scope_key, key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .context("query get_extension_state")?;

    match result {
        Some(s) => {
            let v: Value = serde_json::from_str(&s).unwrap_or(Value::String(s));
            Ok(Some(v))
        }
        None => Ok(None),
    }
}

/// Set an extension's state.
pub fn set_extension_state(
    db: &Database,
    extension_id: &str,
    key: &str,
    value: &Value,
    scope: &StateScope,
) -> Result<()> {
    let scope_key = scope.scope_key();
    let json = serde_json::to_string(value).context("serialize extension state")?;
    db.conn()
        .execute(
            "INSERT INTO extension_state (extension_id, scope, key, value, updated_at)
             VALUES (?1, ?2, ?3, ?4, datetime('now'))
             ON CONFLICT(extension_id, scope, key)
             DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            params![extension_id, scope_key, key, json],
        )
        .context("upsert extension_state")?;
    Ok(())
}

/// Delete an extension's state entry.
pub fn delete_extension_state(
    db: &Database,
    extension_id: &str,
    key: &str,
    scope: &StateScope,
) -> Result<()> {
    let scope_key = scope.scope_key();
    db.conn()
        .execute(
            "DELETE FROM extension_state WHERE extension_id = ?1 AND scope = ?2 AND key = ?3",
            params![extension_id, scope_key, key],
        )
        .context("delete extension_state")?;
    Ok(())
}

/// List all keys stored by an extension in a given scope.
pub fn extension_state_keys(
    db: &Database,
    extension_id: &str,
    scope: &StateScope,
) -> Result<Vec<String>> {
    let scope_key = scope.scope_key();
    let mut stmt = db
        .conn()
        .prepare_cached(
            "SELECT key FROM extension_state WHERE extension_id = ?1 AND scope = ?2 ORDER BY key",
        )
        .context("prepare extension_state_keys")?;

    let rows = stmt
        .query_map(params![extension_id, scope_key], |row| {
            row.get::<_, String>(0)
        })
        .context("query extension_state_keys")?;

    let mut keys = Vec::new();
    for row in rows {
        keys.push(row.context("read extension state key")?);
    }
    Ok(keys)
}

/// Extension trait on [`rusqlite::Statement`] results for optional single-row queries.
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
    use serde_json::json;

    fn test_db() -> Database {
        let tmp = tempfile::TempDir::new().unwrap();
        Database::open(&tmp.path().join("test.db")).unwrap()
    }

    // ── Legacy StateStore tests ─────────────────────────────────────────

    #[test]
    fn get_missing_returns_none() {
        let db = test_db();
        let store = StateStore::new(&db);
        assert!(store.get("global", "missing").unwrap().is_none());
    }

    #[test]
    fn set_and_get() {
        let db = test_db();
        let store = StateStore::new(&db);
        store.set("global", "theme", "dark").unwrap();
        assert_eq!(store.get("global", "theme").unwrap().unwrap(), "dark");
    }

    #[test]
    fn upsert_overwrites() {
        let db = test_db();
        let store = StateStore::new(&db);
        store.set("global", "k", "v1").unwrap();
        store.set("global", "k", "v2").unwrap();
        assert_eq!(store.get("global", "k").unwrap().unwrap(), "v2");
    }

    #[test]
    fn delete_key() {
        let db = test_db();
        let store = StateStore::new(&db);
        store.set("global", "k", "v").unwrap();
        store.delete("global", "k").unwrap();
        assert!(store.get("global", "k").unwrap().is_none());
    }

    #[test]
    fn keys_in_scope() {
        let db = test_db();
        let store = StateStore::new(&db);
        store.set("ws:/proj", "a", "1").unwrap();
        store.set("ws:/proj", "b", "2").unwrap();
        store.set("global", "c", "3").unwrap();
        let keys = store.keys("ws:/proj").unwrap();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn scopes_are_isolated() {
        let db = test_db();
        let store = StateStore::new(&db);
        store.set("global", "k", "global_v").unwrap();
        store.set("extension:foo", "k", "ext_v").unwrap();
        assert_eq!(store.get("global", "k").unwrap().unwrap(), "global_v");
        assert_eq!(store.get("extension:foo", "k").unwrap().unwrap(), "ext_v");
    }

    // ── Workspace state tests ───────────────────────────────────────────

    #[test]
    fn workspace_state_roundtrip() {
        let db = test_db();
        let val = json!({"openFiles": ["/a.rs", "/b.rs"], "scrollTop": 42});
        set_workspace_state(&db, "/proj", "editor.state", &val).unwrap();
        let loaded = get_workspace_state(&db, "/proj", "editor.state")
            .unwrap()
            .unwrap();
        assert_eq!(loaded, val);
    }

    #[test]
    fn workspace_state_missing() {
        let db = test_db();
        assert!(get_workspace_state(&db, "/proj", "nope").unwrap().is_none());
    }

    #[test]
    fn workspace_state_delete() {
        let db = test_db();
        set_workspace_state(&db, "/proj", "k", &json!("v")).unwrap();
        delete_workspace_state(&db, "/proj", "k").unwrap();
        assert!(get_workspace_state(&db, "/proj", "k").unwrap().is_none());
    }

    // ── Global state tests ──────────────────────────────────────────────

    #[test]
    fn global_state_roundtrip() {
        let db = test_db();
        set_global_state(&db, "theme.selection", &json!("Dark Modern")).unwrap();
        let loaded = get_global_state(&db, "theme.selection").unwrap().unwrap();
        assert_eq!(loaded, json!("Dark Modern"));
    }

    #[test]
    fn global_state_upsert() {
        let db = test_db();
        set_global_state(&db, "key", &json!(1)).unwrap();
        set_global_state(&db, "key", &json!(2)).unwrap();
        assert_eq!(get_global_state(&db, "key").unwrap().unwrap(), json!(2));
    }

    #[test]
    fn global_state_delete() {
        let db = test_db();
        set_global_state(&db, "key", &json!("val")).unwrap();
        delete_global_state(&db, "key").unwrap();
        assert!(get_global_state(&db, "key").unwrap().is_none());
    }

    // ── Extension state tests ───────────────────────────────────────────

    #[test]
    fn extension_state_global_scope() {
        let db = test_db();
        let scope = StateScope::Global;
        set_extension_state(
            &db,
            "ext.rust-analyzer",
            "config",
            &json!({"key": "val"}),
            &scope,
        )
        .unwrap();
        let loaded = get_extension_state(&db, "ext.rust-analyzer", "config", &scope)
            .unwrap()
            .unwrap();
        assert_eq!(loaded, json!({"key": "val"}));
    }

    #[test]
    fn extension_state_workspace_scope() {
        let db = test_db();
        let scope = StateScope::Workspace("/my/project".to_owned());
        set_extension_state(&db, "ext.prettier", "cache", &json!([1, 2, 3]), &scope).unwrap();
        let loaded = get_extension_state(&db, "ext.prettier", "cache", &scope)
            .unwrap()
            .unwrap();
        assert_eq!(loaded, json!([1, 2, 3]));

        let global =
            get_extension_state(&db, "ext.prettier", "cache", &StateScope::Global).unwrap();
        assert!(global.is_none());
    }

    #[test]
    fn extension_state_keys_listed() {
        let db = test_db();
        let scope = StateScope::Global;
        set_extension_state(&db, "ext.a", "k1", &json!(1), &scope).unwrap();
        set_extension_state(&db, "ext.a", "k2", &json!(2), &scope).unwrap();
        let keys = extension_state_keys(&db, "ext.a", &scope).unwrap();
        assert_eq!(keys, vec!["k1", "k2"]);
    }

    #[test]
    fn extension_state_delete() {
        let db = test_db();
        let scope = StateScope::Global;
        set_extension_state(&db, "ext.a", "key", &json!("v"), &scope).unwrap();
        delete_extension_state(&db, "ext.a", "key", &scope).unwrap();
        assert!(get_extension_state(&db, "ext.a", "key", &scope)
            .unwrap()
            .is_none());
    }
}
