//! Session management — persist and restore window/editor state across launches.
//!
//! Covers:
//! - Saving/restoring which files were open, cursor positions, scroll offsets.
//! - Hot exit: saving unsaved buffers to temp storage so the user never loses
//!   work, even across unexpected exits.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use sidex_db::Database;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Persisted state of a single open file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFileState {
    /// Absolute path (or `None` for untitled files).
    pub path: Option<PathBuf>,
    /// Cursor line (0-based).
    pub cursor_line: u32,
    /// Cursor column (0-based).
    pub cursor_column: u32,
    /// First visible line (scroll position).
    pub scroll_line: u32,
    /// Whether the tab is pinned.
    pub is_pinned: bool,
}

/// Persisted window-level session state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionWindowState {
    /// Workspace folder that was open (if any).
    pub workspace_path: Option<PathBuf>,
    /// All open file tabs.
    pub open_files: Vec<OpenFileState>,
    /// Index of the active file in `open_files`.
    pub active_file_index: usize,
    /// Whether the sidebar was visible.
    pub sidebar_visible: bool,
    /// Width of the sidebar in logical pixels.
    pub sidebar_width: f32,
    /// Whether the bottom panel was visible.
    pub panel_visible: bool,
    /// Height of the bottom panel in logical pixels.
    pub panel_height: f32,
}

impl Default for SessionWindowState {
    fn default() -> Self {
        Self {
            workspace_path: None,
            open_files: Vec::new(),
            active_file_index: 0,
            sidebar_visible: true,
            sidebar_width: 260.0,
            panel_visible: false,
            panel_height: 200.0,
        }
    }
}

/// Data saved for the hot-exit feature: unsaved buffer contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotExitData {
    /// The file path, or a synthetic path like `untitled:1`.
    pub path: String,
    /// Full buffer contents at the time of exit.
    pub content: String,
    /// Cursor line at the time of exit.
    pub cursor_line: u32,
    /// Cursor column at the time of exit.
    pub cursor_column: u32,
    /// `true` if this was an untitled (never-saved) buffer.
    pub is_untitled: bool,
}

// ---------------------------------------------------------------------------
// Session persistence
// ---------------------------------------------------------------------------

/// Save session state (open files, layout) to the database.
pub fn save_session(db: &Database, windows: &[SessionWindowState]) -> Result<()> {
    let json = serde_json::to_string(windows).context("serialise session")?;
    db.conn()
        .execute(
            "INSERT INTO session_data (id, payload) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET payload = excluded.payload",
            params![json],
        )
        .context("save session")?;
    Ok(())
}

/// Restore session state from the database.
pub fn restore_session(db: &Database) -> Result<Vec<SessionWindowState>> {
    let mut stmt = db
        .conn()
        .prepare_cached("SELECT payload FROM session_data WHERE id = 1")
        .context("prepare restore session")?;

    let result: Option<String> = stmt
        .query_row([], |row| row.get(0))
        .optional()
        .context("query session")?;

    match result {
        Some(json) => {
            let states: Vec<SessionWindowState> =
                serde_json::from_str(&json).context("deserialise session")?;
            Ok(states)
        }
        None => Ok(Vec::new()),
    }
}

// ---------------------------------------------------------------------------
// Hot-exit persistence
// ---------------------------------------------------------------------------

/// Save hot-exit data for all dirty buffers.
pub fn save_hot_exit(db: &Database, data: &[HotExitData]) -> Result<()> {
    let tx = db.conn();
    tx.execute("DELETE FROM hot_exit", [])
        .context("clear hot exit")?;

    let mut stmt = tx
        .prepare_cached(
            "INSERT INTO hot_exit (path, content, cursor_line, cursor_column, is_untitled)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .context("prepare hot exit insert")?;

    for entry in data {
        stmt.execute(params![
            entry.path,
            entry.content,
            entry.cursor_line,
            entry.cursor_column,
            entry.is_untitled,
        ])
        .context("insert hot exit entry")?;
    }
    Ok(())
}

/// Restore hot-exit data.
pub fn restore_hot_exit(db: &Database) -> Result<Vec<HotExitData>> {
    let mut stmt = db
        .conn()
        .prepare_cached(
            "SELECT path, content, cursor_line, cursor_column, is_untitled FROM hot_exit",
        )
        .context("prepare restore hot exit")?;

    let rows = stmt
        .query_map([], |row| {
            Ok(HotExitData {
                path: row.get(0)?,
                content: row.get(1)?,
                cursor_line: row.get(2)?,
                cursor_column: row.get(3)?,
                is_untitled: row.get(4)?,
            })
        })
        .context("query hot exit")?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("read hot exit row")?);
    }
    Ok(entries)
}

/// Clear all hot-exit data (called after a clean launch restores everything).
pub fn clear_hot_exit(db: &Database) -> Result<()> {
    db.conn()
        .execute("DELETE FROM hot_exit", [])
        .context("clear hot exit")?;
    Ok(())
}

/// Convenience: check if there is any hot-exit data pending.
pub fn has_hot_exit_data(db: &Database) -> bool {
    db.conn()
        .query_row("SELECT COUNT(*) FROM hot_exit", [], |row| row.get::<_, i64>(0))
        .map(|count| count > 0)
        .unwrap_or(false)
}

/// Build `HotExitData` entries from the application's in-memory document list.
///
/// Only includes dirty (modified) buffers. This is the function the shutdown
/// path should call before `save_hot_exit`.
pub fn collect_hot_exit_entries<'a, I>(documents: I) -> Vec<HotExitData>
where
    I: IntoIterator<Item = (&'a Option<PathBuf>, &'a str, u32, u32, bool)>,
{
    documents
        .into_iter()
        .map(|(path, content, line, col, dirty)| {
            let (p, untitled) = match path {
                Some(p) => (p.display().to_string(), false),
                None => (format!("untitled:{line}"), true),
            };
            HotExitData {
                path: p,
                content: content.to_owned(),
                cursor_line: line,
                cursor_column: col,
                is_untitled: untitled && dirty,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

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

// Suppress unused-import warning when the `Path` import is only used
// indirectly via `PathBuf`.
const _: () = {
    fn _use_path(_: &Path) {}
};

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let tmp = tempfile::TempDir::new().unwrap();
        Database::open(&tmp.path().join("test.db")).unwrap()
    }

    #[test]
    fn roundtrip_session() {
        let db = test_db();
        let states = vec![SessionWindowState {
            workspace_path: Some(PathBuf::from("/projects/sidex")),
            open_files: vec![
                OpenFileState {
                    path: Some(PathBuf::from("/projects/sidex/main.rs")),
                    cursor_line: 42,
                    cursor_column: 10,
                    scroll_line: 30,
                    is_pinned: true,
                },
                OpenFileState {
                    path: None,
                    cursor_line: 0,
                    cursor_column: 0,
                    scroll_line: 0,
                    is_pinned: false,
                },
            ],
            active_file_index: 0,
            sidebar_visible: true,
            sidebar_width: 300.0,
            panel_visible: true,
            panel_height: 250.0,
        }];
        save_session(&db, &states).unwrap();
        let restored = restore_session(&db).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(
            restored[0].workspace_path.as_deref(),
            Some(Path::new("/projects/sidex"))
        );
        assert_eq!(restored[0].open_files.len(), 2);
        assert_eq!(restored[0].open_files[0].cursor_line, 42);
        assert!(restored[0].open_files[0].is_pinned);
    }

    #[test]
    fn empty_session() {
        let db = test_db();
        let restored = restore_session(&db).unwrap();
        assert!(restored.is_empty());
    }

    #[test]
    fn session_overwrites() {
        let db = test_db();
        let s1 = vec![SessionWindowState::default()];
        save_session(&db, &s1).unwrap();
        let s2 = vec![
            SessionWindowState::default(),
            SessionWindowState::default(),
        ];
        save_session(&db, &s2).unwrap();
        let restored = restore_session(&db).unwrap();
        assert_eq!(restored.len(), 2);
    }

    #[test]
    fn hot_exit_roundtrip() {
        let db = test_db();
        let data = vec![
            HotExitData {
                path: "/tmp/a.rs".into(),
                content: "fn main() {}".into(),
                cursor_line: 1,
                cursor_column: 0,
                is_untitled: false,
            },
            HotExitData {
                path: "untitled:1".into(),
                content: "hello world".into(),
                cursor_line: 0,
                cursor_column: 5,
                is_untitled: true,
            },
        ];
        save_hot_exit(&db, &data).unwrap();
        assert!(has_hot_exit_data(&db));

        let restored = restore_hot_exit(&db).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].path, "/tmp/a.rs");
        assert!(!restored[0].is_untitled);
        assert!(restored[1].is_untitled);
    }

    #[test]
    fn clear_hot_exit_works() {
        let db = test_db();
        let data = vec![HotExitData {
            path: "/x.rs".into(),
            content: "x".into(),
            cursor_line: 0,
            cursor_column: 0,
            is_untitled: false,
        }];
        save_hot_exit(&db, &data).unwrap();
        clear_hot_exit(&db).unwrap();
        assert!(!has_hot_exit_data(&db));
    }

    #[test]
    fn hot_exit_save_clears_previous() {
        let db = test_db();
        let d1 = vec![HotExitData {
            path: "/a.rs".into(),
            content: "a".into(),
            cursor_line: 0,
            cursor_column: 0,
            is_untitled: false,
        }];
        save_hot_exit(&db, &d1).unwrap();
        let d2 = vec![HotExitData {
            path: "/b.rs".into(),
            content: "b".into(),
            cursor_line: 0,
            cursor_column: 0,
            is_untitled: false,
        }];
        save_hot_exit(&db, &d2).unwrap();
        let restored = restore_hot_exit(&db).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].path, "/b.rs");
    }
}
