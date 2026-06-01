use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tauri::State;

// SECURITY: Define resource limits to prevent DoS via unbounded storage (CWE-400)
/// Maximum key length: 256 bytes (sufficient for typical storage keys)
const MAX_KEY_LENGTH: usize = 256;
/// Maximum value length: 1 MB (prevents memory exhaustion while allowing reasonable data)
const MAX_VALUE_LENGTH: usize = 1_048_576;

/// Escape `%`, `_`, and `\` for SQL `LIKE ? ESCAPE '\'` prefix patterns.
///
/// SQLite `LIKE` is case-sensitive for ASCII by default; storage scopes use
/// distinct literal prefixes (e.g. `app/`, `profile:`) so cross-scope leakage
/// is not expected. Example: `escape_like_prefix("a_b")` → `a\_b%`.
fn escape_like_prefix(prefix: &str) -> String {
    let mut pattern = String::with_capacity(prefix.len() + 1);
    for ch in prefix.chars() {
        match ch {
            '%' | '_' | '\\' => {
                pattern.push('\\');
                pattern.push(ch);
            }
            other => pattern.push(other),
        }
    }
    pattern.push('%');
    pattern
}

pub struct StorageDb {
    conn: Mutex<Connection>,
}

impl StorageDb {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn =
            Connection::open(db_path).map_err(|e| format!("Failed to open database: {e}"))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv_store (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .map_err(|e| format!("Failed to create table: {e}"))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn get(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT value FROM kv_store WHERE key = ?1")
            .map_err(|e| e.to_string())?;
        Ok(stmt.query_row([key], |row| row.get::<_, String>(0)).ok())
    }

    pub fn list_by_prefix(&self, prefix: &str) -> Result<Vec<(String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let pattern = escape_like_prefix(prefix);
        let mut stmt = conn
            .prepare("SELECT key, value FROM kv_store WHERE key LIKE ?1 ESCAPE '\\' ORDER BY key")
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map([&pattern], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row.map_err(|e| e.to_string())?);
        }
        Ok(items)
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        // SECURITY: Enforce size limits to prevent resource exhaustion (CWE-400)
        if key.len() > MAX_KEY_LENGTH {
            return Err(format!(
                "key length exceeds maximum of {} bytes (got {} bytes)",
                MAX_KEY_LENGTH,
                key.len()
            ));
        }
        if value.len() > MAX_VALUE_LENGTH {
            return Err(format!(
                "value length exceeds maximum of {} bytes (got {} bytes)",
                MAX_VALUE_LENGTH,
                value.len()
            ));
        }

        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?1, ?2)",
            [key, value],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn storage_get(
    state: State<'_, Arc<StorageDb>>,
    key: String,
) -> Result<Option<String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT value FROM kv_store WHERE key = ?1")
        .map_err(|e| format!("Failed to prepare query: {e}"))?;

    let result = stmt.query_row([&key], |row| row.get::<_, String>(0)).ok();

    Ok(result)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn storage_set(
    state: State<'_, Arc<StorageDb>>,
    key: String,
    value: String,
) -> Result<(), String> {
    // SECURITY: Enforce size limits to prevent resource exhaustion (CWE-400)
    if key.len() > MAX_KEY_LENGTH {
        return Err(format!(
            "key length exceeds maximum of {} bytes (got {} bytes)",
            MAX_KEY_LENGTH,
            key.len()
        ));
    }
    if value.len() > MAX_VALUE_LENGTH {
        return Err(format!(
            "value length exceeds maximum of {} bytes (got {} bytes)",
            MAX_VALUE_LENGTH,
            value.len()
        ));
    }

    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?1, ?2)",
        [&key, &value],
    )
    .map_err(|e| format!("Failed to set key '{key}': {e}"))?;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn storage_list(
    state: State<'_, Arc<StorageDb>>,
    prefix: String,
) -> Result<Vec<(String, String)>, String> {
    state.list_by_prefix(&prefix)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn storage_delete(state: State<'_, Arc<StorageDb>>, key: String) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM kv_store WHERE key = ?1", [&key])
        .map_err(|e| format!("Failed to delete key '{key}': {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_like_prefix_leaves_plain_text() {
        assert_eq!(escape_like_prefix("app/profile:"), "app/profile:%");
    }

    #[test]
    fn escape_like_prefix_escapes_metacharacters() {
        assert_eq!(escape_like_prefix("a_b"), r"a\_b%");
        assert_eq!(escape_like_prefix("50%"), r"50\%%");
        assert_eq!(escape_like_prefix(r"key\name"), r"key\\name%");
    }
}
