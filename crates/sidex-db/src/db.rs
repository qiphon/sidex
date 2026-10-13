//! `SQLite` database connection with versioned schema migrations.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::Connection;

/// Current schema version. Bump when adding migrations.
pub const CURRENT_SCHEMA_VERSION: u32 = 4;

/// Wraps a `SQLite` connection and ensures schema migrations run on open.
pub struct Database {
    conn: Connection,
    path: PathBuf,
}

impl Database {
    /// Opens (or creates) a database at `path` and runs migrations.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database at {}", path.display()))?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("failed to set pragmas")?;

        let mut db = Self {
            conn,
            path: path.to_path_buf(),
        };
        db.migrate(CURRENT_SCHEMA_VERSION)?;
        Ok(db)
    }

    /// Opens the database at the default path: `~/.sidex/state.db`.
    pub fn open_default() -> Result<Self> {
        let dir = dirs::home_dir()
            .context("could not determine home directory")?
            .join(".sidex");
        Self::open(&dir.join("state.db"))
    }

    /// Returns a reference to the underlying `rusqlite::Connection`.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Returns the path this database was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the current `user_version` of the database.
    pub fn schema_version(&self) -> Result<u32> {
        let v: u32 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .context("query user_version")?;
        Ok(v)
    }

    /// Run VACUUM to compact the database.
    pub fn vacuum(&self) -> Result<()> {
        self.conn.execute_batch("VACUUM").context("vacuum")?;
        Ok(())
    }

    /// Back up the database to `dest`.
    pub fn backup_to(&self, dest: &Path) -> Result<()> {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut backup_conn =
            Connection::open(dest).with_context(|| format!("open backup at {}", dest.display()))?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut backup_conn)
            .context("create backup object")?;
        backup
            .run_to_completion(100, std::time::Duration::from_millis(10), None)
            .context("run backup")?;
        Ok(())
    }

    /// Run versioned migrations up to `target_version`.
    pub fn migrate(&mut self, target_version: u32) -> Result<()> {
        let current = self.schema_version()?;
        if current >= target_version {
            return Ok(());
        }

        if current < 1 {
            self.migration_v1()?;
        }
        if current < 2 {
            self.migration_v2()?;
        }
        if current < 3 {
            self.migration_v3()?;
        }
        if current < 4 {
            self.migration_v4()?;
        }

        self.conn
            .execute_batch(&format!("PRAGMA user_version = {target_version}"))
            .context("set user_version")?;
        Ok(())
    }

    /// V1: original tables.
    fn migration_v1(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS state_kv (
                    scope   TEXT NOT NULL,
                    key     TEXT NOT NULL,
                    value   TEXT NOT NULL,
                    PRIMARY KEY (scope, key)
                );

                CREATE TABLE IF NOT EXISTS kv_store (
                    key   TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS recent_files (
                    path        TEXT PRIMARY KEY,
                    last_opened TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE IF NOT EXISTS recent_workspaces (
                    path        TEXT PRIMARY KEY,
                    last_opened TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE IF NOT EXISTS window_state (
                    id              INTEGER PRIMARY KEY CHECK (id = 1),
                    x               INTEGER NOT NULL,
                    y               INTEGER NOT NULL,
                    width           INTEGER NOT NULL,
                    height          INTEGER NOT NULL,
                    is_maximized    INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS session_data (
                    id      INTEGER PRIMARY KEY CHECK (id = 1),
                    payload TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS hot_exit (
                    path          TEXT PRIMARY KEY,
                    content       TEXT NOT NULL,
                    cursor_line   INTEGER NOT NULL DEFAULT 0,
                    cursor_column INTEGER NOT NULL DEFAULT 0,
                    is_untitled   INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS telemetry_events (
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_name      TEXT NOT NULL,
                    classification  TEXT NOT NULL,
                    payload         TEXT NOT NULL,
                    timestamp       TEXT NOT NULL
                );
                ",
            )
            .context("migration v1")?;
        Ok(())
    }

    /// V2: workspace state, global state, extension state, search history,
    /// terminal sessions, clipboard history, breakpoints, bookmarks.
    fn migration_v2(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS workspace_state (
                    workspace   TEXT NOT NULL,
                    key         TEXT NOT NULL,
                    value       TEXT NOT NULL,
                    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
                    PRIMARY KEY (workspace, key)
                );

                CREATE TABLE IF NOT EXISTS global_state (
                    key         TEXT PRIMARY KEY,
                    value       TEXT NOT NULL,
                    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE IF NOT EXISTS extension_state (
                    extension_id TEXT NOT NULL,
                    scope        TEXT NOT NULL DEFAULT 'global',
                    key          TEXT NOT NULL,
                    value        TEXT NOT NULL,
                    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
                    PRIMARY KEY (extension_id, scope, key)
                );

                CREATE TABLE IF NOT EXISTS search_history (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    query       TEXT NOT NULL,
                    is_regex    INTEGER NOT NULL DEFAULT 0,
                    is_case     INTEGER NOT NULL DEFAULT 0,
                    is_word     INTEGER NOT NULL DEFAULT 0,
                    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE IF NOT EXISTS terminal_sessions (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    title       TEXT NOT NULL DEFAULT '',
                    shell_path  TEXT NOT NULL DEFAULT '',
                    cwd         TEXT NOT NULL DEFAULT '',
                    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                    closed_at   TEXT
                );

                CREATE TABLE IF NOT EXISTS clipboard_history (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    content     TEXT NOT NULL,
                    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE IF NOT EXISTS breakpoints (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path   TEXT NOT NULL,
                    line        INTEGER NOT NULL,
                    column      INTEGER,
                    condition   TEXT,
                    hit_count   TEXT,
                    log_message TEXT,
                    is_enabled  INTEGER NOT NULL DEFAULT 1,
                    UNIQUE(file_path, line)
                );

                CREATE TABLE IF NOT EXISTS bookmarks (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path   TEXT NOT NULL,
                    line        INTEGER NOT NULL,
                    label       TEXT,
                    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                    UNIQUE(file_path, line)
                );
                ",
            )
            .context("migration v2")?;
        Ok(())
    }

    /// V3: snippets storage, tasks history.
    fn migration_v3(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS snippets (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    name        TEXT NOT NULL,
                    prefix      TEXT NOT NULL,
                    body        TEXT NOT NULL,
                    language    TEXT NOT NULL DEFAULT '',
                    description TEXT NOT NULL DEFAULT '',
                    source      TEXT NOT NULL DEFAULT 'user',
                    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
                );

                CREATE TABLE IF NOT EXISTS tasks_history (
                    id          INTEGER PRIMARY KEY AUTOINCREMENT,
                    task_name   TEXT NOT NULL,
                    task_type   TEXT NOT NULL DEFAULT '',
                    exit_code   INTEGER,
                    started_at  TEXT NOT NULL DEFAULT (datetime('now')),
                    finished_at TEXT
                );

                CREATE INDEX IF NOT EXISTS idx_search_history_created
                    ON search_history(created_at DESC);

                CREATE INDEX IF NOT EXISTS idx_clipboard_history_created
                    ON clipboard_history(created_at DESC);

                CREATE INDEX IF NOT EXISTS idx_breakpoints_file
                    ON breakpoints(file_path);

                CREATE INDEX IF NOT EXISTS idx_extension_state_ext
                    ON extension_state(extension_id);
                ",
            )
            .context("migration v3")?;
        Ok(())
    }

    /// V4: `window_state` stores OS frame geometry only; workbench layout lives in `kv_store`.
    fn migration_v4(&self) -> Result<()> {
        let conn = &self.conn;
        let has_window_state = Self::table_exists(conn, "window_state")?;
        let has_window_state_v4 = Self::table_exists(conn, "window_state_v4")?;

        conn.execute_batch("BEGIN IMMEDIATE")
            .context("begin migration v4")?;

        let migration_result = (|| -> Result<()> {
            if has_window_state_v4 && !has_window_state {
                conn.execute_batch("ALTER TABLE window_state_v4 RENAME TO window_state;")
                    .context("rename window_state_v4 after interrupted drop")?;
                return Ok(());
            }

            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS window_state_v4 (
                    id              INTEGER PRIMARY KEY CHECK (id = 1),
                    x               INTEGER NOT NULL,
                    y               INTEGER NOT NULL,
                    width           INTEGER NOT NULL,
                    height          INTEGER NOT NULL,
                    is_maximized    INTEGER NOT NULL DEFAULT 0
                );
                ",
            )
            .context("create window_state_v4")?;

            if has_window_state {
                conn.execute_batch(
                    "
                    INSERT OR IGNORE INTO window_state_v4 (id, x, y, width, height, is_maximized)
                    SELECT id, x, y, width, height, is_maximized FROM window_state;

                    DROP TABLE window_state;
                    ",
                )
                .context("copy and drop legacy window_state")?;
            }

            if Self::table_exists(conn, "window_state_v4")? {
                conn.execute_batch("ALTER TABLE window_state_v4 RENAME TO window_state;")
                    .context("rename window_state_v4")?;
            }

            Ok(())
        })();

        match migration_result {
            Ok(()) => conn
                .execute_batch("COMMIT")
                .context("commit migration v4")?,
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e).context("migration v4");
            }
        }

        Ok(())
    }

    fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [name],
                |row| row.get(0),
            )
            .context("query sqlite_master")?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();
        assert!(db.path().exists());
    }

    #[test]
    fn migrations_are_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let _db1 = Database::open(&path).unwrap();
        let _db2 = Database::open(&path).unwrap();
    }

    #[test]
    fn schema_version_is_current() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();
        assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn vacuum_succeeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();
        db.vacuum().unwrap();
    }

    #[test]
    fn migration_v4_survives_interrupted_rename() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("test.db");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "
            PRAGMA user_version = 3;
            CREATE TABLE window_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                x INTEGER NOT NULL,
                y INTEGER NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                is_maximized INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO window_state (id, x, y, width, height, is_maximized)
            VALUES (1, 10, 20, 800, 600, 0);
            CREATE TABLE window_state_v4 (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                x INTEGER NOT NULL,
                y INTEGER NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                is_maximized INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO window_state_v4 (id, x, y, width, height, is_maximized)
            VALUES (1, 10, 20, 800, 600, 0);
            DROP TABLE window_state;
            ",
        )
        .unwrap();
        drop(conn);

        let db = Database::open(&path).unwrap();
        assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        let (x, width): (i64, i64) = db
            .conn()
            .query_row(
                "SELECT x, width FROM window_state WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!((x, width), (10, 800));
        assert!(!Database::table_exists(db.conn(), "window_state_v4").unwrap());
    }

    #[test]
    fn backup_and_restore() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();
        db.conn()
            .execute(
                "INSERT INTO global_state (key, value) VALUES ('theme', 'dark')",
                [],
            )
            .unwrap();

        let backup_path = tmp.path().join("backup.db");
        db.backup_to(&backup_path).unwrap();

        let db2 = Database::open(&backup_path).unwrap();
        let val: String = db2
            .conn()
            .query_row(
                "SELECT value FROM global_state WHERE key = 'theme'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(val, "dark");
    }
}
