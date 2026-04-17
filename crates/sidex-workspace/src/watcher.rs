//! File system watcher with debouncing and ignore patterns.
//!
//! Wraps [`notify::RecommendedWatcher`] and coalesces rapid changes within a
//! configurable debounce window (default 100 ms). Ignores common build artifact
//! directories (`.git`, `node_modules`, `target`, …).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;

use crate::error::{WorkspaceError, WorkspaceResult};

const DEFAULT_DEBOUNCE_MS: u64 = 100;

static IGNORED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    "__pycache__",
    ".next",
    ".cache",
];

/// Kind of file-system event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FileEventKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// A single observed file-system change.
#[derive(Debug, Clone, Serialize)]
pub struct FileEvent {
    pub path: PathBuf,
    pub kind: FileEventKind,
}

/// File system watcher with built-in debouncing.
pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    _debounce_handle: std::thread::JoinHandle<()>,
    stop: Arc<Mutex<bool>>,
}

impl FileWatcher {
    /// Create a new watcher on `root` with the default debounce window.
    pub fn new(root: &Path) -> WorkspaceResult<Self> {
        Self::with_debounce(root, Duration::from_millis(DEFAULT_DEBOUNCE_MS))
    }

    /// Create a watcher with a custom debounce duration.
    pub fn with_debounce(root: &Path, debounce: Duration) -> WorkspaceResult<Self> {
        Self::build(root, debounce, |_events| {})
    }

    /// Create a watcher that calls `handler` with each debounced batch of events.
    pub fn on_change(
        root: &Path,
        handler: impl Fn(Vec<FileEvent>) + Send + 'static,
    ) -> WorkspaceResult<Self> {
        Self::build(root, Duration::from_millis(DEFAULT_DEBOUNCE_MS), handler)
    }

    fn build(
        root: &Path,
        debounce: Duration,
        handler: impl Fn(Vec<FileEvent>) + Send + 'static,
    ) -> WorkspaceResult<Self> {
        let pending: Arc<Mutex<HashMap<PathBuf, FileEventKind>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_tx = Arc::clone(&pending);
        let stop = Arc::new(Mutex::new(false));
        let stop_rx = Arc::clone(&stop);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let kind = match event.kind {
                        EventKind::Create(_) => FileEventKind::Created,
                        EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                            FileEventKind::Renamed
                        }
                        EventKind::Modify(_) => FileEventKind::Modified,
                        EventKind::Remove(_) => FileEventKind::Deleted,
                        _ => return,
                    };

                    if let Ok(mut map) = pending_tx.lock() {
                        for path in event.paths {
                            if should_ignore(&path) {
                                continue;
                            }
                            map.insert(path, kind);
                        }
                    }
                }
            },
            Config::default(),
        )
        .map_err(WorkspaceError::Watcher)?;

        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(WorkspaceError::Watcher)?;

        let debounce_handle = std::thread::spawn(move || loop {
            std::thread::sleep(debounce);

            if *stop_rx
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
            {
                break;
            }

            let batch: Vec<FileEvent> = {
                let mut map = pending
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if map.is_empty() {
                    continue;
                }
                let events = map
                    .drain()
                    .map(|(path, kind)| FileEvent { path, kind })
                    .collect();
                events
            };

            if !batch.is_empty() {
                handler(batch);
            }
        });

        Ok(Self {
            _watcher: watcher,
            _debounce_handle: debounce_handle,
            stop,
        })
    }

    /// Stop the watcher and its debounce thread.
    pub fn stop(self) {
        if let Ok(mut s) = self.stop.lock() {
            *s = true;
        }
    }
}

fn should_ignore(path: &Path) -> bool {
    for component in path.components() {
        if let Some(name) = component.as_os_str().to_str() {
            if IGNORED_DIRS.contains(&name) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::mpsc;
    use tempfile::TempDir;

    #[test]
    fn watcher_emits_events() {
        let tmp = TempDir::new().unwrap();
        let (tx, rx) = mpsc::channel();

        let watcher = FileWatcher::on_change(tmp.path(), move |events| {
            for e in events {
                let _ = tx.send(e);
            }
        })
        .unwrap();

        // Give the watcher time to initialise.
        std::thread::sleep(Duration::from_millis(200));

        fs::write(tmp.path().join("hello.txt"), "world").unwrap();

        // Wait for debounce.
        let event = rx.recv_timeout(Duration::from_secs(2));
        assert!(event.is_ok(), "expected at least one file event");

        watcher.stop();
    }

    #[test]
    fn ignored_dirs_are_skipped() {
        assert!(should_ignore(Path::new("/foo/.git/objects/abc")));
        assert!(should_ignore(Path::new("/project/node_modules/pkg")));
        assert!(!should_ignore(Path::new("/project/src/main.rs")));
    }
}
