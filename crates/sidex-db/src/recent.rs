//! Recent files and workspaces persistence.

use anyhow::{Context, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::Database;

/// A recently opened file or workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    /// Absolute path to the file or workspace root.
    pub path: String,
    /// ISO-8601 timestamp of the last time this entry was opened.
    pub last_opened: String,
}

/// Records a file as recently opened, upserting the timestamp.
pub fn add_recent_file(db: &Database, path: &str) -> Result<()> {
    db.conn()
        .execute(
            "INSERT INTO recent_files (path, last_opened) VALUES (?1, datetime('now'))
             ON CONFLICT(path) DO UPDATE SET last_opened = datetime('now')",
            params![path],
        )
        .context("add recent file")?;
    Ok(())
}

/// Records a workspace as recently opened, upserting the timestamp.
pub fn add_recent_workspace(db: &Database, path: &str) -> Result<()> {
    db.conn()
        .execute(
            "INSERT INTO recent_workspaces (path, last_opened) VALUES (?1, datetime('now'))
             ON CONFLICT(path) DO UPDATE SET last_opened = datetime('now')",
            params![path],
        )
        .context("add recent workspace")?;
    Ok(())
}

/// Returns the most recently opened files, newest first.
pub fn recent_files(db: &Database, limit: usize) -> Result<Vec<RecentEntry>> {
    query_recent(db, "recent_files", limit)
}

/// Returns the most recently opened workspaces, newest first.
pub fn recent_workspaces(db: &Database, limit: usize) -> Result<Vec<RecentEntry>> {
    query_recent(db, "recent_workspaces", limit)
}

/// Clears all recent file and workspace entries.
pub fn clear_recent(db: &Database) -> Result<()> {
    db.conn()
        .execute_batch("DELETE FROM recent_files; DELETE FROM recent_workspaces;")
        .context("clear recent")?;
    Ok(())
}

fn query_recent(db: &Database, table: &str, limit: usize) -> Result<Vec<RecentEntry>> {
    let sql = format!("SELECT path, last_opened FROM {table} ORDER BY last_opened DESC LIMIT ?1");
    let mut stmt = db.conn().prepare(&sql).context("prepare recent query")?;

    #[allow(clippy::cast_possible_wrap)]
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(RecentEntry {
                path: row.get(0)?,
                last_opened: row.get(1)?,
            })
        })
        .context("query recent")?;

    let mut entries = Vec::new();
    for row in rows {
        entries.push(row.context("read recent row")?);
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

    #[test]
    fn add_and_list_recent_files() {
        let db = test_db();
        add_recent_file(&db, "/a.rs").unwrap();
        add_recent_file(&db, "/b.rs").unwrap();
        let files = recent_files(&db, 10).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn add_and_list_recent_workspaces() {
        let db = test_db();
        add_recent_workspace(&db, "/proj1").unwrap();
        add_recent_workspace(&db, "/proj2").unwrap();
        let ws = recent_workspaces(&db, 10).unwrap();
        assert_eq!(ws.len(), 2);
    }

    #[test]
    fn clear_recent_empties_both() {
        let db = test_db();
        add_recent_file(&db, "/a.rs").unwrap();
        add_recent_workspace(&db, "/proj").unwrap();
        clear_recent(&db).unwrap();
        assert!(recent_files(&db, 10).unwrap().is_empty());
        assert!(recent_workspaces(&db, 10).unwrap().is_empty());
    }

    #[test]
    fn limit_is_respected() {
        let db = test_db();
        for i in 0..20 {
            add_recent_file(&db, &format!("/file{i}.rs")).unwrap();
        }
        assert_eq!(recent_files(&db, 5).unwrap().len(), 5);
    }

    #[test]
    fn duplicate_upserts_timestamp() {
        let db = test_db();
        add_recent_file(&db, "/a.rs").unwrap();
        add_recent_file(&db, "/b.rs").unwrap();
        add_recent_file(&db, "/a.rs").unwrap();
        let files = recent_files(&db, 10).unwrap();
        assert_eq!(files[0].path, "/a.rs");
    }
}
