//! Search history, terminal session history, and clipboard history.

use anyhow::{Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;

// ── Search history ──────────────────────────────────────────────────────────

/// A recorded search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistoryEntry {
    pub id: i64,
    pub query: String,
    pub is_regex: bool,
    pub is_case_sensitive: bool,
    pub is_whole_word: bool,
    pub created_at: String,
}

/// Add a search query to history.
pub fn add_search_history(
    db: &Database,
    query: &str,
    is_regex: bool,
    is_case: bool,
    is_word: bool,
) -> Result<()> {
    db.conn()
        .execute(
            "INSERT INTO search_history (query, is_regex, is_case, is_word)
             VALUES (?1, ?2, ?3, ?4)",
            params![query, is_regex, is_case, is_word],
        )
        .context("add search history")?;
    Ok(())
}

/// Get recent search history entries, newest first.
pub fn search_history(db: &Database, limit: usize) -> Result<Vec<SearchHistoryEntry>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, query, is_regex, is_case, is_word, created_at
             FROM search_history ORDER BY created_at DESC LIMIT ?1",
        )
        .context("prepare search_history")?;

    #[allow(clippy::cast_possible_wrap)]
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(SearchHistoryEntry {
                id: row.get(0)?,
                query: row.get(1)?,
                is_regex: row.get(2)?,
                is_case_sensitive: row.get(3)?,
                is_whole_word: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .context("query search_history")?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("read search_history row")?);
    }
    Ok(entries)
}

/// Clear all search history.
pub fn clear_search_history(db: &Database) -> Result<()> {
    db.conn()
        .execute("DELETE FROM search_history", [])
        .context("clear search_history")?;
    Ok(())
}

// ── Terminal session history ────────────────────────────────────────────────

/// A recorded terminal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSession {
    pub id: i64,
    pub title: String,
    pub shell_path: String,
    pub cwd: String,
    pub created_at: String,
    pub closed_at: Option<String>,
}

/// Record a new terminal session.
pub fn add_terminal_session(
    db: &Database,
    title: &str,
    shell_path: &str,
    cwd: &str,
) -> Result<i64> {
    db.conn()
        .execute(
            "INSERT INTO terminal_sessions (title, shell_path, cwd)
             VALUES (?1, ?2, ?3)",
            params![title, shell_path, cwd],
        )
        .context("add terminal_session")?;
    Ok(db.conn().last_insert_rowid())
}

/// Mark a terminal session as closed.
pub fn close_terminal_session(db: &Database, id: i64) -> Result<()> {
    db.conn()
        .execute(
            "UPDATE terminal_sessions SET closed_at = datetime('now') WHERE id = ?1",
            params![id],
        )
        .context("close terminal_session")?;
    Ok(())
}

/// Get recent terminal sessions, newest first.
pub fn terminal_sessions(db: &Database, limit: usize) -> Result<Vec<TerminalSession>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, title, shell_path, cwd, created_at, closed_at
             FROM terminal_sessions ORDER BY created_at DESC LIMIT ?1",
        )
        .context("prepare terminal_sessions")?;

    #[allow(clippy::cast_possible_wrap)]
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(TerminalSession {
                id: row.get(0)?,
                title: row.get(1)?,
                shell_path: row.get(2)?,
                cwd: row.get(3)?,
                created_at: row.get(4)?,
                closed_at: row.get(5)?,
            })
        })
        .context("query terminal_sessions")?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("read terminal_session row")?);
    }
    Ok(entries)
}

// ── Clipboard history ───────────────────────────────────────────────────────

/// A clipboard history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content: String,
    pub created_at: String,
}

/// Add content to clipboard history.
pub fn add_clipboard_entry(db: &Database, content: &str) -> Result<()> {
    db.conn()
        .execute(
            "INSERT INTO clipboard_history (content) VALUES (?1)",
            params![content],
        )
        .context("add clipboard_entry")?;
    Ok(())
}

/// Get recent clipboard entries, newest first.
pub fn clipboard_history(db: &Database, limit: usize) -> Result<Vec<ClipboardEntry>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, content, created_at
             FROM clipboard_history ORDER BY created_at DESC LIMIT ?1",
        )
        .context("prepare clipboard_history")?;

    #[allow(clippy::cast_possible_wrap)]
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .context("query clipboard_history")?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("read clipboard_entry row")?);
    }
    Ok(entries)
}

/// Clear all clipboard history.
pub fn clear_clipboard_history(db: &Database) -> Result<()> {
    db.conn()
        .execute("DELETE FROM clipboard_history", [])
        .context("clear clipboard_history")?;
    Ok(())
}

// ── Breakpoints ─────────────────────────────────────────────────────────────

/// A persisted breakpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    pub id: i64,
    pub file_path: String,
    pub line: u32,
    pub column: Option<u32>,
    pub condition: Option<String>,
    pub hit_count: Option<String>,
    pub log_message: Option<String>,
    pub is_enabled: bool,
}

/// Add or update a breakpoint.
pub fn upsert_breakpoint(
    db: &Database,
    file_path: &str,
    line: u32,
    condition: Option<&str>,
    hit_count: Option<&str>,
    log_message: Option<&str>,
    is_enabled: bool,
) -> Result<()> {
    db.conn()
        .execute(
            "INSERT INTO breakpoints (file_path, line, condition, hit_count, log_message, is_enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(file_path, line) DO UPDATE SET
                condition = excluded.condition,
                hit_count = excluded.hit_count,
                log_message = excluded.log_message,
                is_enabled = excluded.is_enabled",
            params![file_path, line, condition, hit_count, log_message, is_enabled],
        )
        .context("upsert breakpoint")?;
    Ok(())
}

/// Get all breakpoints for a file.
pub fn breakpoints_for_file(db: &Database, file_path: &str) -> Result<Vec<Breakpoint>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, file_path, line, column, condition, hit_count, log_message, is_enabled
             FROM breakpoints WHERE file_path = ?1 ORDER BY line",
        )
        .context("prepare breakpoints_for_file")?;

    let rows = stmt
        .query_map(params![file_path], |row| {
            Ok(Breakpoint {
                id: row.get(0)?,
                file_path: row.get(1)?,
                line: row.get(2)?,
                column: row.get(3)?,
                condition: row.get(4)?,
                hit_count: row.get(5)?,
                log_message: row.get(6)?,
                is_enabled: row.get(7)?,
            })
        })
        .context("query breakpoints_for_file")?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("read breakpoint row")?);
    }
    Ok(entries)
}

/// Get all breakpoints across all files.
pub fn all_breakpoints(db: &Database) -> Result<Vec<Breakpoint>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, file_path, line, column, condition, hit_count, log_message, is_enabled
             FROM breakpoints ORDER BY file_path, line",
        )
        .context("prepare all_breakpoints")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Breakpoint {
                id: row.get(0)?,
                file_path: row.get(1)?,
                line: row.get(2)?,
                column: row.get(3)?,
                condition: row.get(4)?,
                hit_count: row.get(5)?,
                log_message: row.get(6)?,
                is_enabled: row.get(7)?,
            })
        })
        .context("query all_breakpoints")?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("read breakpoint row")?);
    }
    Ok(entries)
}

/// Remove a breakpoint.
pub fn remove_breakpoint(db: &Database, file_path: &str, line: u32) -> Result<()> {
    db.conn()
        .execute(
            "DELETE FROM breakpoints WHERE file_path = ?1 AND line = ?2",
            params![file_path, line],
        )
        .context("remove breakpoint")?;
    Ok(())
}

/// Remove all breakpoints.
pub fn clear_breakpoints(db: &Database) -> Result<()> {
    db.conn()
        .execute("DELETE FROM breakpoints", [])
        .context("clear breakpoints")?;
    Ok(())
}

// ── Bookmarks ───────────────────────────────────────────────────────────────

/// A persisted bookmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: i64,
    pub file_path: String,
    pub line: u32,
    pub label: Option<String>,
    pub created_at: String,
}

/// Toggle a bookmark: add if absent, remove if present.
pub fn toggle_bookmark(db: &Database, file_path: &str, line: u32, label: Option<&str>) -> Result<bool> {
    let existing: bool = db
        .conn()
        .query_row(
            "SELECT COUNT(*) > 0 FROM bookmarks WHERE file_path = ?1 AND line = ?2",
            params![file_path, line],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if existing {
        db.conn()
            .execute(
                "DELETE FROM bookmarks WHERE file_path = ?1 AND line = ?2",
                params![file_path, line],
            )
            .context("remove bookmark")?;
        Ok(false)
    } else {
        db.conn()
            .execute(
                "INSERT INTO bookmarks (file_path, line, label) VALUES (?1, ?2, ?3)",
                params![file_path, line, label],
            )
            .context("add bookmark")?;
        Ok(true)
    }
}

/// Get all bookmarks for a file.
pub fn bookmarks_for_file(db: &Database, file_path: &str) -> Result<Vec<Bookmark>> {
    let mut stmt = db
        .conn()
        .prepare(
            "SELECT id, file_path, line, label, created_at
             FROM bookmarks WHERE file_path = ?1 ORDER BY line",
        )
        .context("prepare bookmarks_for_file")?;

    let rows = stmt
        .query_map(params![file_path], |row| {
            Ok(Bookmark {
                id: row.get(0)?,
                file_path: row.get(1)?,
                line: row.get(2)?,
                label: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .context("query bookmarks_for_file")?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("read bookmark row")?);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let tmp = tempfile::TempDir::new().unwrap();
        Database::open(&tmp.path().join("test.db")).unwrap()
    }

    // Search history
    #[test]
    fn search_history_add_and_list() {
        let db = test_db();
        add_search_history(&db, "fn main", false, false, false).unwrap();
        add_search_history(&db, "todo!", true, true, false).unwrap();
        let history = search_history(&db, 10).unwrap();
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn search_history_clear() {
        let db = test_db();
        add_search_history(&db, "query", false, false, false).unwrap();
        clear_search_history(&db).unwrap();
        assert!(search_history(&db, 10).unwrap().is_empty());
    }

    // Terminal sessions
    #[test]
    fn terminal_session_lifecycle() {
        let db = test_db();
        let id = add_terminal_session(&db, "bash", "/bin/bash", "/home/user").unwrap();
        assert!(id > 0);
        let sessions = terminal_sessions(&db, 10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].closed_at.is_none());

        close_terminal_session(&db, id).unwrap();
        let sessions2 = terminal_sessions(&db, 10).unwrap();
        assert!(sessions2[0].closed_at.is_some());
    }

    // Clipboard history
    #[test]
    fn clipboard_add_and_list() {
        let db = test_db();
        add_clipboard_entry(&db, "hello world").unwrap();
        add_clipboard_entry(&db, "fn main()").unwrap();
        let entries = clipboard_history(&db, 10).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn clipboard_clear() {
        let db = test_db();
        add_clipboard_entry(&db, "data").unwrap();
        clear_clipboard_history(&db).unwrap();
        assert!(clipboard_history(&db, 10).unwrap().is_empty());
    }

    // Breakpoints
    #[test]
    fn breakpoint_crud() {
        let db = test_db();
        upsert_breakpoint(&db, "/main.rs", 10, None, None, None, true).unwrap();
        upsert_breakpoint(&db, "/main.rs", 20, Some("x > 5"), None, None, true).unwrap();

        let bps = breakpoints_for_file(&db, "/main.rs").unwrap();
        assert_eq!(bps.len(), 2);
        assert_eq!(bps[0].line, 10);
        assert_eq!(bps[1].condition.as_deref(), Some("x > 5"));

        remove_breakpoint(&db, "/main.rs", 10).unwrap();
        assert_eq!(breakpoints_for_file(&db, "/main.rs").unwrap().len(), 1);

        clear_breakpoints(&db).unwrap();
        assert!(all_breakpoints(&db).unwrap().is_empty());
    }

    #[test]
    fn breakpoint_upsert_updates() {
        let db = test_db();
        upsert_breakpoint(&db, "/a.rs", 5, None, None, None, true).unwrap();
        upsert_breakpoint(&db, "/a.rs", 5, Some("cond"), None, None, false).unwrap();
        let bps = breakpoints_for_file(&db, "/a.rs").unwrap();
        assert_eq!(bps.len(), 1);
        assert_eq!(bps[0].condition.as_deref(), Some("cond"));
        assert!(!bps[0].is_enabled);
    }

    // Bookmarks
    #[test]
    fn bookmark_toggle() {
        let db = test_db();
        let added = toggle_bookmark(&db, "/main.rs", 10, Some("important")).unwrap();
        assert!(added);
        let bms = bookmarks_for_file(&db, "/main.rs").unwrap();
        assert_eq!(bms.len(), 1);
        assert_eq!(bms[0].label.as_deref(), Some("important"));

        let removed = toggle_bookmark(&db, "/main.rs", 10, None).unwrap();
        assert!(!removed);
        assert!(bookmarks_for_file(&db, "/main.rs").unwrap().is_empty());
    }
}
