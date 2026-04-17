//! Local file history — tracks file saves over time.
//!
//! Stores snapshots in `.sidex/history/` with automatic cleanup: at most 50
//! versions per file and nothing older than 30 days.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

const MAX_VERSIONS_PER_FILE: usize = 50;
const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30 days

/// A single historical snapshot of a file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub timestamp: u64,
    pub size: usize,
    pub path: PathBuf,
}

/// Tracks file saves and manages the `.sidex/history/` directory.
pub struct LocalHistory {
    root: PathBuf,
}

impl LocalHistory {
    /// Create a `LocalHistory` rooted at a workspace directory.
    /// Snapshots are stored under `<root>/.sidex/history/`.
    pub fn new(workspace_root: &Path) -> Self {
        Self {
            root: workspace_root.join(".sidex").join("history"),
        }
    }

    fn history_dir_for(&self, file_path: &Path) -> PathBuf {
        let hash = path_hash(file_path);
        self.root.join(hash)
    }

    /// Save a snapshot of the given file contents.
    pub fn save_version(&self, file_path: &Path, content: &str) -> Result<(), String> {
        let dir = self.history_dir_for(file_path);
        std::fs::create_dir_all(&dir).map_err(|e| format!("create history dir: {e}"))?;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry_path = dir.join(format!("{now}.snapshot"));

        let meta = serde_json::json!({
            "original_path": file_path.to_string_lossy(),
            "timestamp": now,
            "size": content.len(),
        });
        let meta_path = dir.join(format!("{now}.meta.json"));

        std::fs::write(&entry_path, content).map_err(|e| format!("write snapshot: {e}"))?;
        std::fs::write(
            &meta_path,
            serde_json::to_string_pretty(&meta).unwrap_or_default(),
        )
        .map_err(|e| format!("write meta: {e}"))?;

        self.cleanup(&dir)?;

        Ok(())
    }

    /// List all history entries for a file, newest first.
    pub fn get_versions(&self, file_path: &Path) -> Result<Vec<HistoryEntry>, String> {
        let dir = self.history_dir_for(file_path);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        let read_dir = std::fs::read_dir(&dir).map_err(|e| format!("read history dir: {e}"))?;

        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.ends_with(".meta.json") {
                continue;
            }

            let meta_content = match std::fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let meta: serde_json::Value = match serde_json::from_str(&meta_content) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let timestamp = meta.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
            let size = meta.get("size").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let original = meta
                .get("original_path")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            entries.push(HistoryEntry {
                timestamp,
                size,
                path: PathBuf::from(original),
            });
        }

        entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(entries)
    }

    /// Retrieve the content of a specific history version.
    pub fn get_version_content(
        &self,
        file_path: &Path,
        entry: &HistoryEntry,
    ) -> Result<String, String> {
        let dir = self.history_dir_for(file_path);
        let snapshot_path = dir.join(format!("{}.snapshot", entry.timestamp));
        std::fs::read_to_string(&snapshot_path).map_err(|e| format!("read snapshot: {e}"))
    }

    fn cleanup(&self, dir: &Path) -> Result<(), String> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let cutoff = now.saturating_sub(MAX_AGE.as_secs());

        let mut timestamps: Vec<u64> = Vec::new();
        let read_dir = match std::fs::read_dir(dir) {
            Ok(d) => d,
            Err(_) => return Ok(()),
        };

        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if let Some(ts_str) = name_str.strip_suffix(".snapshot") {
                if let Ok(ts) = ts_str.parse::<u64>() {
                    timestamps.push(ts);
                }
            }
        }

        timestamps.sort_unstable();

        // Remove entries older than cutoff
        for &ts in &timestamps {
            if ts < cutoff {
                let _ = std::fs::remove_file(dir.join(format!("{ts}.snapshot")));
                let _ = std::fs::remove_file(dir.join(format!("{ts}.meta.json")));
            }
        }

        // Keep only the latest MAX_VERSIONS_PER_FILE
        let remaining: Vec<u64> = timestamps.into_iter().filter(|&ts| ts >= cutoff).collect();
        if remaining.len() > MAX_VERSIONS_PER_FILE {
            let to_remove = remaining.len() - MAX_VERSIONS_PER_FILE;
            for &ts in &remaining[..to_remove] {
                let _ = std::fs::remove_file(dir.join(format!("{ts}.snapshot")));
                let _ = std::fs::remove_file(dir.join(format!("{ts}.meta.json")));
            }
        }

        Ok(())
    }
}

fn path_hash(path: &Path) -> String {
    let s = path.to_string_lossy();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn save_and_list_versions() {
        let tmp = TempDir::new().unwrap();
        let history = LocalHistory::new(tmp.path());
        let file = PathBuf::from("/src/main.rs");

        history.save_version(&file, "fn main() {}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        history
            .save_version(&file, "fn main() { println!(\"hi\"); }")
            .unwrap();

        let versions = history.get_versions(&file).unwrap();
        assert_eq!(versions.len(), 2);
        assert!(versions[0].timestamp >= versions[1].timestamp);
    }

    #[test]
    fn get_version_content() {
        let tmp = TempDir::new().unwrap();
        let history = LocalHistory::new(tmp.path());
        let file = PathBuf::from("/src/lib.rs");

        history.save_version(&file, "content_v1").unwrap();
        let versions = history.get_versions(&file).unwrap();
        let content = history.get_version_content(&file, &versions[0]).unwrap();
        assert_eq!(content, "content_v1");
    }

    #[test]
    fn empty_history() {
        let tmp = TempDir::new().unwrap();
        let history = LocalHistory::new(tmp.path());
        let file = PathBuf::from("/nonexistent.rs");
        let versions = history.get_versions(&file).unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn path_hash_deterministic() {
        let a = path_hash(Path::new("/foo/bar.rs"));
        let b = path_hash(Path::new("/foo/bar.rs"));
        assert_eq!(a, b);
    }

    #[test]
    fn path_hash_different() {
        let a = path_hash(Path::new("/foo/bar.rs"));
        let b = path_hash(Path::new("/foo/baz.rs"));
        assert_ne!(a, b);
    }
}
