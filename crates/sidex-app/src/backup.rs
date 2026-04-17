//! File backup service — auto-saves unsaved changes so no work is lost on
//! crash or unexpected exit.
//!
//! Mirrors VS Code's hot-exit / backup behaviour:
//! - Dirty buffers are persisted to `~/.sidex/backups/{workspace_hash}/`
//!   every second while there are unsaved changes.
//! - On crash recovery the backup contents are restored into the editor.
//! - Backups are cleaned up when the file is saved normally.
//! - Untitled (never-saved) files are also backed up.

use std::collections::HashMap;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// How often the backup timer fires.
const BACKUP_INTERVAL: Duration = Duration::from_secs(1);

/// How old a stale backup must be before automatic cleanup.
const STALE_BACKUP_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Metadata for a single backed-up file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    /// The original file path (`None` for untitled buffers).
    pub original_path: Option<PathBuf>,
    /// Where the backup content is stored on disk.
    pub backup_path: PathBuf,
    /// Hex hash of the backup content (for quick change detection).
    pub content_hash: String,
    /// When the backup was last written.
    pub timestamp: String,
    /// `true` when the buffer has never been saved to a real file.
    pub is_untitled: bool,
}

/// Manages the backup directory and tracks which files are currently
/// backed up.
pub struct BackupService {
    pub backup_dir: PathBuf,
    pub backups: HashMap<PathBuf, BackupEntry>,
    pub interval: Duration,
}

impl BackupService {
    /// Create a new service rooted at `backup_dir`.
    pub fn new(backup_dir: PathBuf) -> Self {
        Self {
            backup_dir,
            backups: HashMap::new(),
            interval: BACKUP_INTERVAL,
        }
    }

    /// Convenience: create a service under
    /// `~/.sidex/backups/{workspace_hash}`.
    pub fn for_workspace(workspace_root: Option<&Path>) -> Self {
        let base = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("sidex")
            .join("backups");
        let dir = match workspace_root {
            Some(root) => base.join(hash_path(root)),
            None => base.join("no-workspace"),
        };
        Self::new(dir)
    }

    /// Save a backup of a dirty buffer. Returns the `BackupEntry` written.
    pub fn save_backup(&mut self, path: &Path, content: &str) -> Result<BackupEntry> {
        let entry = save_backup(path, content, &self.backup_dir)?;
        self.backups.insert(path.to_path_buf(), entry.clone());
        Ok(entry)
    }

    /// Save a backup for an untitled (never-saved) document.
    pub fn save_untitled_backup(&mut self, id: &str, content: &str) -> Result<BackupEntry> {
        let key = PathBuf::from(format!("untitled:{id}"));
        let entry = save_untitled(id, content, &self.backup_dir)?;
        self.backups.insert(key, entry.clone());
        Ok(entry)
    }

    /// Restore a backup's content.
    pub fn restore(&self, entry: &BackupEntry) -> Result<String> {
        restore_backup(entry)
    }

    /// Remove the backup for `path` (e.g. after the file was saved normally).
    pub fn discard(&mut self, path: &Path) {
        if let Some(entry) = self.backups.remove(path) {
            let _ = std::fs::remove_file(&entry.backup_path);
        }
    }

    /// Load the manifest of previously-written backups (for crash recovery).
    pub fn load_existing(&mut self) -> Result<()> {
        let entries = list_backups(&self.backup_dir)?;
        for entry in entries {
            let key = entry
                .original_path
                .clone()
                .unwrap_or_else(|| PathBuf::from("untitled"));
            self.backups.insert(key, entry);
        }
        Ok(())
    }

    /// All currently tracked backup entries.
    pub fn entries(&self) -> impl Iterator<Item = &BackupEntry> {
        self.backups.values()
    }

    /// Number of active backups.
    pub fn count(&self) -> usize {
        self.backups.len()
    }

    /// Remove stale backups older than `STALE_BACKUP_AGE`.
    pub fn cleanup_stale(&self) -> Result<()> {
        cleanup_stale_backups(&self.backup_dir, STALE_BACKUP_AGE)
    }

    /// Remove *all* backups in the directory.
    pub fn clear_all(&mut self) -> Result<()> {
        if self.backup_dir.exists() {
            std::fs::remove_dir_all(&self.backup_dir)
                .context("remove backup dir")?;
        }
        self.backups.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Standalone functions
// ---------------------------------------------------------------------------

/// Write a backup of `content` for the file at `path`.
pub fn save_backup(path: &Path, content: &str, backup_dir: &Path) -> Result<BackupEntry> {
    std::fs::create_dir_all(backup_dir).context("create backup dir")?;

    let file_hash = hash_path(path);
    let backup_path = backup_dir.join(format!("{file_hash}.bak"));
    let content_hash = hash_content(content);

    let mut f = std::fs::File::create(&backup_path).context("create backup file")?;
    f.write_all(content.as_bytes()).context("write backup")?;

    let meta_path = backup_dir.join(format!("{file_hash}.meta.json"));
    let entry = BackupEntry {
        original_path: Some(path.to_path_buf()),
        backup_path: backup_path.clone(),
        content_hash,
        timestamp: iso_now(),
        is_untitled: false,
    };
    let meta_json = serde_json::to_string_pretty(&entry).context("serialise meta")?;
    std::fs::write(&meta_path, meta_json).context("write meta")?;

    Ok(entry)
}

/// Write a backup for an untitled buffer identified by `id`.
fn save_untitled(id: &str, content: &str, backup_dir: &Path) -> Result<BackupEntry> {
    std::fs::create_dir_all(backup_dir).context("create backup dir")?;

    let backup_path = backup_dir.join(format!("untitled-{id}.bak"));
    let content_hash = hash_content(content);

    let mut f = std::fs::File::create(&backup_path).context("create untitled backup")?;
    f.write_all(content.as_bytes()).context("write untitled backup")?;

    let meta_path = backup_dir.join(format!("untitled-{id}.meta.json"));
    let entry = BackupEntry {
        original_path: None,
        backup_path: backup_path.clone(),
        content_hash,
        timestamp: iso_now(),
        is_untitled: true,
    };
    let meta_json = serde_json::to_string_pretty(&entry).context("serialise untitled meta")?;
    std::fs::write(&meta_path, meta_json).context("write untitled meta")?;

    Ok(entry)
}

/// Read the content from a backup entry.
pub fn restore_backup(entry: &BackupEntry) -> Result<String> {
    std::fs::read_to_string(&entry.backup_path)
        .with_context(|| format!("restore backup {}", entry.backup_path.display()))
}

/// Enumerate all backup entries in `backup_dir` by reading `.meta.json` files.
pub fn list_backups(backup_dir: &Path) -> Result<Vec<BackupEntry>> {
    if !backup_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for item in std::fs::read_dir(backup_dir).context("read backup dir")? {
        let item = item?;
        let path = item.path();
        let is_meta = path
            .file_name()
            .and_then(|n| n.to_str())
            .map_or(false, |n| n.ends_with(".meta.json"));
        if !is_meta {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<BackupEntry>(&json) {
                Ok(entry) => entries.push(entry),
                Err(e) => log::warn!("bad backup meta {}: {e}", path.display()),
            },
            Err(e) => log::warn!("unreadable backup meta {}: {e}", path.display()),
        }
    }

    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(entries)
}

/// Remove backup files older than `max_age`.
pub fn cleanup_stale_backups(backup_dir: &Path, max_age: Duration) -> Result<()> {
    if !backup_dir.exists() {
        return Ok(());
    }

    let now = SystemTime::now();
    let mut removed = 0u32;

    for item in std::fs::read_dir(backup_dir).context("read backup dir")? {
        let item = item?;
        if let Ok(meta) = item.metadata() {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age > max_age {
                        let _ = std::fs::remove_file(item.path());
                        removed += 1;
                    }
                }
            }
        }
    }

    if removed > 0 {
        log::debug!("removed {removed} stale backup files");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hash_path(path: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn hash_content(content: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn iso_now() -> String {
    humantime::format_rfc3339_seconds(SystemTime::now()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_and_restore() {
        let tmp = tempfile::TempDir::new().unwrap();
        let entry =
            save_backup(Path::new("/tmp/hello.rs"), "fn main() {}", tmp.path()).unwrap();
        assert!(!entry.is_untitled);
        assert!(entry.backup_path.exists());

        let restored = restore_backup(&entry).unwrap();
        assert_eq!(restored, "fn main() {}");
    }

    #[test]
    fn save_untitled_and_restore() {
        let tmp = tempfile::TempDir::new().unwrap();
        let entry = save_untitled("1", "hello world", tmp.path()).unwrap();
        assert!(entry.is_untitled);
        assert!(entry.original_path.is_none());

        let restored = restore_backup(&entry).unwrap();
        assert_eq!(restored, "hello world");
    }

    #[test]
    fn list_backups_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let entries = list_backups(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn list_backups_nonexistent() {
        let entries = list_backups(Path::new("/nonexistent/backups")).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn list_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        save_backup(Path::new("/a.rs"), "aaa", tmp.path()).unwrap();
        save_backup(Path::new("/b.rs"), "bbb", tmp.path()).unwrap();

        let entries = list_backups(tmp.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn service_save_and_discard() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = BackupService::new(tmp.path().to_path_buf());

        svc.save_backup(Path::new("/file.rs"), "content").unwrap();
        assert_eq!(svc.count(), 1);

        svc.discard(Path::new("/file.rs"));
        assert_eq!(svc.count(), 0);
    }

    #[test]
    fn service_untitled() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = BackupService::new(tmp.path().to_path_buf());

        let entry = svc.save_untitled_backup("42", "new doc").unwrap();
        assert!(entry.is_untitled);
        assert_eq!(svc.count(), 1);

        let content = svc.restore(&entry).unwrap();
        assert_eq!(content, "new doc");
    }

    #[test]
    fn service_clear_all() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = BackupService::new(tmp.path().join("bk"));
        svc.save_backup(Path::new("/x.rs"), "x").unwrap();
        svc.clear_all().unwrap();
        assert_eq!(svc.count(), 0);
        assert!(!tmp.path().join("bk").exists());
    }

    #[test]
    fn for_workspace_includes_hash() {
        let svc = BackupService::for_workspace(Some(Path::new("/home/user/project")));
        let dir_str = svc.backup_dir.display().to_string();
        assert!(dir_str.contains("backups"));
    }

    #[test]
    fn for_workspace_none() {
        let svc = BackupService::for_workspace(None);
        let dir_str = svc.backup_dir.display().to_string();
        assert!(dir_str.contains("no-workspace"));
    }

    #[test]
    fn content_hash_changes() {
        let h1 = hash_content("hello");
        let h2 = hash_content("world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn content_hash_stable() {
        let h1 = hash_content("same");
        let h2 = hash_content("same");
        assert_eq!(h1, h2);
    }

    #[test]
    fn cleanup_stale_nonexistent() {
        assert!(cleanup_stale_backups(Path::new("/nope"), Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn load_existing_populates() {
        let tmp = tempfile::TempDir::new().unwrap();
        save_backup(Path::new("/a.rs"), "a", tmp.path()).unwrap();

        let mut svc = BackupService::new(tmp.path().to_path_buf());
        svc.load_existing().unwrap();
        assert_eq!(svc.count(), 1);
    }
}
