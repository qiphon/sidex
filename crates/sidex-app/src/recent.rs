//! Recent files and workspaces — high-level manager used by the Welcome page,
//! **File > Open Recent** menu, and <kbd>Ctrl+R</kbd> quick-open.
//!
//! This module sits on top of [`sidex_db::recent`] and adds:
//! - An in-memory `RecentManager` that batches DB writes.
//! - Richer `RecentItem` type with display labels and `SystemTime`.
//! - Capacity limits and de-duplication.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use sidex_db::Database;

/// Maximum number of recent items kept per category.
const MAX_RECENT_FILES: usize = 50;
const MAX_RECENT_WORKSPACES: usize = 20;

/// A recent file or workspace entry with display metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentItem {
    /// Absolute path.
    pub path: PathBuf,
    /// Human-readable label (typically the file/folder name).
    pub label: String,
    /// The full path as a string (for subtitle display).
    pub description: String,
    /// When this item was last opened.
    pub last_opened: SystemTime,
}

impl RecentItem {
    fn from_path(path: &Path) -> Self {
        let label = path
            .file_name()
            .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        Self {
            path: path.to_path_buf(),
            label,
            description: path.display().to_string(),
            last_opened: SystemTime::now(),
        }
    }
}

/// High-level recent-items manager.
///
/// Wraps the lower-level `sidex_db::recent` functions and maintains an
/// in-memory cache.
pub struct RecentManager {
    files: Vec<RecentItem>,
    workspaces: Vec<RecentItem>,
}

impl RecentManager {
    /// Create an empty manager. Call [`load`](Self::load) afterwards to
    /// hydrate from the database.
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            workspaces: Vec::new(),
        }
    }

    /// Load recent items from the database into memory.
    pub fn load(&mut self, db: &Database) -> Result<()> {
        let db_files = sidex_db::recent_files(db, MAX_RECENT_FILES)?;
        self.files = db_files
            .into_iter()
            .map(|e| RecentItem {
                label: Path::new(&e.path)
                    .file_name()
                    .map_or_else(|| e.path.clone(), |n| n.to_string_lossy().into_owned()),
                description: e.path.clone(),
                path: PathBuf::from(&e.path),
                last_opened: SystemTime::now(),
            })
            .collect();

        let db_ws = sidex_db::recent_workspaces(db, MAX_RECENT_WORKSPACES)?;
        self.workspaces = db_ws
            .into_iter()
            .map(|e| RecentItem {
                label: Path::new(&e.path)
                    .file_name()
                    .map_or_else(|| e.path.clone(), |n| n.to_string_lossy().into_owned()),
                description: e.path.clone(),
                path: PathBuf::from(&e.path),
                last_opened: SystemTime::now(),
            })
            .collect();

        Ok(())
    }

    /// Record a file as recently opened (persists to DB immediately).
    pub fn add_recent_file(&mut self, path: &Path, db: &Database) -> Result<()> {
        let abs = normalize_path(path);
        let abs_str = abs.display().to_string();

        self.files.retain(|i| i.path != abs);
        self.files.insert(0, RecentItem::from_path(&abs));
        self.files.truncate(MAX_RECENT_FILES);

        sidex_db::add_recent_file(db, &abs_str)?;
        Ok(())
    }

    /// Record a workspace folder as recently opened.
    pub fn add_recent_workspace(&mut self, path: &Path, db: &Database) -> Result<()> {
        let abs = normalize_path(path);
        let abs_str = abs.display().to_string();

        self.workspaces.retain(|i| i.path != abs);
        self.workspaces.insert(0, RecentItem::from_path(&abs));
        self.workspaces.truncate(MAX_RECENT_WORKSPACES);

        sidex_db::add_recent_workspace(db, &abs_str)?;
        Ok(())
    }

    /// Return the `limit` most-recently opened files.
    pub fn recent_files(&self, limit: usize) -> &[RecentItem] {
        let end = limit.min(self.files.len());
        &self.files[..end]
    }

    /// Return the `limit` most-recently opened workspaces.
    pub fn recent_workspaces(&self, limit: usize) -> &[RecentItem] {
        let end = limit.min(self.workspaces.len());
        &self.workspaces[..end]
    }

    /// Remove a single file from the recent list.
    pub fn remove_recent_file(&mut self, path: &Path) {
        let abs = normalize_path(path);
        self.files.retain(|i| i.path != abs);
    }

    /// Remove a single workspace from the recent list.
    pub fn remove_recent_workspace(&mut self, path: &Path) {
        let abs = normalize_path(path);
        self.workspaces.retain(|i| i.path != abs);
    }

    /// Clear all recent files and workspaces (in-memory and DB).
    pub fn clear_recent(&mut self, db: &Database) -> Result<()> {
        self.files.clear();
        self.workspaces.clear();
        sidex_db::clear_recent(db)?;
        Ok(())
    }

    /// Total number of recent file entries.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Total number of recent workspace entries.
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// All recent items (files + workspaces) merged and sorted by
    /// recency, suitable for a unified "Open Recent" menu.
    pub fn all_recent(&self, limit: usize) -> Vec<&RecentItem> {
        let mut all: Vec<&RecentItem> = self.files.iter().chain(self.workspaces.iter()).collect();
        all.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));
        all.truncate(limit);
        all
    }
}

impl Default for RecentManager {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let tmp = tempfile::TempDir::new().unwrap();
        Database::open(&tmp.path().join("test.db")).unwrap()
    }

    #[test]
    fn add_and_list_files() {
        let db = test_db();
        let mut mgr = RecentManager::new();
        mgr.add_recent_file(Path::new("/a.rs"), &db).unwrap();
        mgr.add_recent_file(Path::new("/b.rs"), &db).unwrap();
        assert_eq!(mgr.file_count(), 2);
        assert_eq!(mgr.recent_files(10)[0].path, PathBuf::from("/b.rs"));
    }

    #[test]
    fn add_and_list_workspaces() {
        let db = test_db();
        let mut mgr = RecentManager::new();
        mgr.add_recent_workspace(Path::new("/proj1"), &db).unwrap();
        mgr.add_recent_workspace(Path::new("/proj2"), &db).unwrap();
        assert_eq!(mgr.workspace_count(), 2);
    }

    #[test]
    fn duplicate_moves_to_front() {
        let db = test_db();
        let mut mgr = RecentManager::new();
        mgr.add_recent_file(Path::new("/a.rs"), &db).unwrap();
        mgr.add_recent_file(Path::new("/b.rs"), &db).unwrap();
        mgr.add_recent_file(Path::new("/a.rs"), &db).unwrap();
        assert_eq!(mgr.file_count(), 2);
        assert_eq!(mgr.recent_files(10)[0].path, PathBuf::from("/a.rs"));
    }

    #[test]
    fn clear_empties_both() {
        let db = test_db();
        let mut mgr = RecentManager::new();
        mgr.add_recent_file(Path::new("/a.rs"), &db).unwrap();
        mgr.add_recent_workspace(Path::new("/proj"), &db).unwrap();
        mgr.clear_recent(&db).unwrap();
        assert_eq!(mgr.file_count(), 0);
        assert_eq!(mgr.workspace_count(), 0);
    }

    #[test]
    fn limit_is_respected() {
        let db = test_db();
        let mut mgr = RecentManager::new();
        for i in 0..60 {
            mgr.add_recent_file(&PathBuf::from(format!("/file{i}.rs")), &db)
                .unwrap();
        }
        assert_eq!(mgr.file_count(), MAX_RECENT_FILES);
    }

    #[test]
    fn remove_recent_file() {
        let db = test_db();
        let mut mgr = RecentManager::new();
        mgr.add_recent_file(Path::new("/a.rs"), &db).unwrap();
        mgr.add_recent_file(Path::new("/b.rs"), &db).unwrap();
        mgr.remove_recent_file(Path::new("/a.rs"));
        assert_eq!(mgr.file_count(), 1);
    }

    #[test]
    fn all_recent_merges() {
        let db = test_db();
        let mut mgr = RecentManager::new();
        mgr.add_recent_file(Path::new("/a.rs"), &db).unwrap();
        mgr.add_recent_workspace(Path::new("/proj"), &db).unwrap();
        let all = mgr.all_recent(10);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn recent_item_label() {
        let item = RecentItem::from_path(Path::new("/home/user/project/main.rs"));
        assert_eq!(item.label, "main.rs");
    }

    #[test]
    fn load_from_db() {
        let db = test_db();
        sidex_db::add_recent_file(&db, "/x.rs").unwrap();
        sidex_db::add_recent_workspace(&db, "/proj").unwrap();
        let mut mgr = RecentManager::new();
        mgr.load(&db).unwrap();
        assert_eq!(mgr.file_count(), 1);
        assert_eq!(mgr.workspace_count(), 1);
    }
}
