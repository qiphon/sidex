//! File watcher event processing — reacts to file-system changes detected by [`FileWatcher`].
//!
//! When files change on disk the editor needs to decide whether to silently
//! reload, prompt the user (conflict dialog), refresh auxiliary panels, etc.
//! This module provides the decision logic and conflict resolution state.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;

use crate::watcher::{FileEvent, FileEventKind};

// ---------------------------------------------------------------------------
// Reactions
// ---------------------------------------------------------------------------

/// An action the editor should take in response to a file-system event.
#[derive(Debug, Clone, Serialize)]
pub enum FileWatcherReaction {
    ReloadFile {
        path: PathBuf,
    },
    ShowConflictDialog {
        path: PathBuf,
        disk_mtime: SystemTime,
    },
    RefreshFileTree,
    RefreshGitStatus,
    RerunSearch {
        query: String,
    },
    ReloadSettings,
    ReloadExtensions,
}

// ---------------------------------------------------------------------------
// Conflict resolution
// ---------------------------------------------------------------------------

/// A file that was changed both in the editor and on disk.
#[derive(Debug, Clone)]
pub struct FileConflict {
    pub path: PathBuf,
    pub editor_content: String,
    pub disk_content: String,
    pub editor_mtime: SystemTime,
    pub disk_mtime: SystemTime,
}

/// The user's chosen resolution for a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ConflictResolution {
    /// Overwrite the disk version with the editor content.
    KeepEditor,
    /// Reload the file from disk, discarding editor changes.
    ReloadFromDisk,
    /// Open a diff view comparing the two versions.
    Compare,
}

/// Manages pending file conflicts.
#[derive(Debug, Clone, Default)]
pub struct FileConflictResolver {
    pub pending_conflicts: Vec<FileConflict>,
}

impl FileConflictResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_conflict(&mut self, conflict: FileConflict) {
        if !self.has_conflict(&conflict.path) {
            self.pending_conflicts.push(conflict);
        }
    }

    pub fn has_conflict(&self, path: &Path) -> bool {
        self.pending_conflicts.iter().any(|c| c.path == path)
    }

    pub fn resolve(&mut self, path: &Path, resolution: ConflictResolution) -> Option<FileConflict> {
        if let Some(pos) = self.pending_conflicts.iter().position(|c| c.path == path) {
            let conflict = self.pending_conflicts.remove(pos);
            match resolution {
                ConflictResolution::KeepEditor
                | ConflictResolution::Compare
                | ConflictResolution::ReloadFromDisk => Some(conflict),
            }
        } else {
            None
        }
    }

    pub fn resolve_all(&mut self, resolution: ConflictResolution) -> Vec<FileConflict> {
        let all: Vec<PathBuf> = self
            .pending_conflicts
            .iter()
            .map(|c| c.path.clone())
            .collect();
        let mut resolved = Vec::new();
        for path in all {
            if let Some(c) = self.resolve(&path, resolution) {
                resolved.push(c);
            }
        }
        resolved
    }

    pub fn pending_count(&self) -> usize {
        self.pending_conflicts.len()
    }

    pub fn clear(&mut self) {
        self.pending_conflicts.clear();
    }
}

// ---------------------------------------------------------------------------
// Event throttler / debouncer
// ---------------------------------------------------------------------------

/// Deduplicates and throttles rapid file-system events into batched reactions.
pub struct EventThrottler {
    seen: HashSet<PathBuf>,
    last_flush: Instant,
    min_interval: Duration,
    pending: Vec<FileEvent>,
    /// Paths written by our own editor (to ignore echo events).
    own_writes: HashSet<PathBuf>,
    own_write_expiry: Duration,
    own_write_times: Vec<(PathBuf, Instant)>,
}

impl EventThrottler {
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
            last_flush: Instant::now(),
            min_interval: Duration::from_millis(200),
            pending: Vec::new(),
            own_writes: HashSet::new(),
            own_write_expiry: Duration::from_secs(2),
            own_write_times: Vec::new(),
        }
    }

    /// Record that we wrote to `path` ourselves, so we can ignore the echo event.
    pub fn record_own_write(&mut self, path: PathBuf) {
        self.own_writes.insert(path.clone());
        self.own_write_times.push((path, Instant::now()));
    }

    /// Ingest a batch of raw events. Returns `true` if the caller should flush.
    pub fn ingest(&mut self, events: Vec<FileEvent>) -> bool {
        self.expire_own_writes();

        for event in events {
            if self.own_writes.contains(&event.path) {
                continue;
            }
            if self.seen.insert(event.path.clone()) {
                self.pending.push(event);
            }
        }

        self.last_flush.elapsed() >= self.min_interval && !self.pending.is_empty()
    }

    /// Flush pending events and return reactions.
    pub fn flush(
        &mut self,
        dirty_files: &HashSet<PathBuf>,
        auto_reload: bool,
        active_search_query: Option<&str>,
    ) -> Vec<FileWatcherReaction> {
        let events = std::mem::take(&mut self.pending);
        self.seen.clear();
        self.last_flush = Instant::now();

        let mut reactions = Vec::new();
        let mut needs_tree_refresh = false;
        let mut needs_git_refresh = false;

        for event in &events {
            match event.kind {
                FileEventKind::Created | FileEventKind::Deleted | FileEventKind::Renamed => {
                    needs_tree_refresh = true;
                    needs_git_refresh = true;
                }
                FileEventKind::Modified => {
                    needs_git_refresh = true;
                }
            }

            if is_settings_file(&event.path) {
                reactions.push(FileWatcherReaction::ReloadSettings);
                continue;
            }

            if is_extension_file(&event.path) {
                reactions.push(FileWatcherReaction::ReloadExtensions);
                continue;
            }

            match event.kind {
                FileEventKind::Modified => {
                    if dirty_files.contains(&event.path) {
                        let disk_mtime = std::fs::metadata(&event.path)
                            .and_then(|m| m.modified())
                            .unwrap_or(SystemTime::UNIX_EPOCH);
                        reactions.push(FileWatcherReaction::ShowConflictDialog {
                            path: event.path.clone(),
                            disk_mtime,
                        });
                    } else if auto_reload {
                        reactions.push(FileWatcherReaction::ReloadFile {
                            path: event.path.clone(),
                        });
                    }
                }
                FileEventKind::Created | FileEventKind::Deleted | FileEventKind::Renamed => {}
            }
        }

        if needs_tree_refresh {
            reactions.push(FileWatcherReaction::RefreshFileTree);
        }
        if needs_git_refresh {
            reactions.push(FileWatcherReaction::RefreshGitStatus);
        }
        if let Some(query) = active_search_query {
            if !events.is_empty() {
                reactions.push(FileWatcherReaction::RerunSearch {
                    query: query.to_string(),
                });
            }
        }

        reactions
    }

    fn expire_own_writes(&mut self) {
        let now = Instant::now();
        self.own_write_times.retain(|(path, time)| {
            if now.duration_since(*time) > self.own_write_expiry {
                self.own_writes.remove(path);
                false
            } else {
                true
            }
        });
    }
}

impl Default for EventThrottler {
    fn default() -> Self {
        Self::new()
    }
}

fn is_settings_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    matches!(name, "settings.json" | "keybindings.json" | ".editorconfig")
}

fn is_extension_file(path: &Path) -> bool {
    path.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|s| s == ".vscode" || s == "extensions")
    }) && path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e == "json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_resolver_add_and_resolve() {
        let mut resolver = FileConflictResolver::new();
        let conflict = FileConflict {
            path: PathBuf::from("/test.rs"),
            editor_content: "editor".into(),
            disk_content: "disk".into(),
            editor_mtime: SystemTime::now(),
            disk_mtime: SystemTime::now(),
        };
        resolver.add_conflict(conflict);
        assert_eq!(resolver.pending_count(), 1);
        assert!(resolver.has_conflict(Path::new("/test.rs")));

        let resolved = resolver.resolve(Path::new("/test.rs"), ConflictResolution::ReloadFromDisk);
        assert!(resolved.is_some());
        assert_eq!(resolver.pending_count(), 0);
    }

    #[test]
    fn conflict_resolver_no_duplicates() {
        let mut resolver = FileConflictResolver::new();
        let make = || FileConflict {
            path: PathBuf::from("/dup.rs"),
            editor_content: "a".into(),
            disk_content: "b".into(),
            editor_mtime: SystemTime::now(),
            disk_mtime: SystemTime::now(),
        };
        resolver.add_conflict(make());
        resolver.add_conflict(make());
        assert_eq!(resolver.pending_count(), 1);
    }

    #[test]
    fn throttler_ignores_own_writes() {
        let mut throttler = EventThrottler::new();
        let path = PathBuf::from("/edited.rs");
        throttler.record_own_write(path.clone());

        let events = vec![FileEvent {
            path,
            kind: FileEventKind::Modified,
        }];
        throttler.ingest(events);
        let reactions = throttler.flush(&HashSet::new(), true, None);
        assert!(reactions.is_empty(), "own writes should be filtered out");
    }

    #[test]
    fn throttler_deduplicates() {
        let mut throttler = EventThrottler::new();
        let path = PathBuf::from("/file.rs");
        let events = vec![
            FileEvent {
                path: path.clone(),
                kind: FileEventKind::Modified,
            },
            FileEvent {
                path: path.clone(),
                kind: FileEventKind::Modified,
            },
        ];
        throttler.ingest(events);
        let reactions = throttler.flush(&HashSet::new(), true, None);
        let reload_count = reactions
            .iter()
            .filter(|r| matches!(r, FileWatcherReaction::ReloadFile { .. }))
            .count();
        assert_eq!(reload_count, 1);
    }

    #[test]
    fn dirty_file_triggers_conflict() {
        let mut throttler = EventThrottler::new();
        let path = PathBuf::from("/dirty.rs");
        let events = vec![FileEvent {
            path: path.clone(),
            kind: FileEventKind::Modified,
        }];
        throttler.ingest(events);

        let mut dirty = HashSet::new();
        dirty.insert(path);
        let reactions = throttler.flush(&dirty, true, None);
        assert!(reactions
            .iter()
            .any(|r| matches!(r, FileWatcherReaction::ShowConflictDialog { .. })));
    }

    #[test]
    fn settings_file_detected() {
        assert!(is_settings_file(Path::new(
            "/workspace/.vscode/settings.json"
        )));
        assert!(!is_settings_file(Path::new("/workspace/src/main.rs")));
    }

    #[test]
    fn created_file_refreshes_tree() {
        let mut throttler = EventThrottler::new();
        let events = vec![FileEvent {
            path: PathBuf::from("/new_file.rs"),
            kind: FileEventKind::Created,
        }];
        throttler.ingest(events);
        let reactions = throttler.flush(&HashSet::new(), true, None);
        assert!(reactions
            .iter()
            .any(|r| matches!(r, FileWatcherReaction::RefreshFileTree)));
    }
}
