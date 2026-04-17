//! Git status — parse `git status --porcelain=v2`.

use std::path::Path;

use serde::Serialize;

use crate::cmd::run_git;
use crate::error::GitResult;

/// High-level status of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Ignored,
    Conflicted,
    Copied,
}

/// One entry from `git status`.
#[derive(Debug, Clone, Serialize)]
pub struct StatusEntry {
    pub path: String,
    pub status: FileStatus,
    pub staged: bool,
}

/// Run `git status --porcelain=v2` and parse the output.
pub fn get_status(repo_root: &Path) -> GitResult<Vec<StatusEntry>> {
    let output = run_git(repo_root, &["status", "--porcelain=v2", "-uall"])?;
    let mut entries = Vec::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }

        match line.as_bytes().first() {
            // Ordinary changed entries: "1 XY ..."
            Some(b'1') => {
                if let Some(entry) = parse_ordinary_entry(line) {
                    entries.push(entry);
                }
            }
            // Renamed/copied entries: "2 XY ..."
            Some(b'2') => {
                if let Some(entry) = parse_rename_entry(line) {
                    entries.push(entry);
                }
            }
            // Unmerged entries: "u XY ..."
            Some(b'u') => {
                if let Some(entry) = parse_unmerged_entry(line) {
                    entries.push(entry);
                }
            }
            // Untracked: "? path"
            Some(b'?') => {
                let path = line.get(2..).unwrap_or("").to_string();
                entries.push(StatusEntry {
                    path,
                    status: FileStatus::Untracked,
                    staged: false,
                });
            }
            // Ignored: "! path"
            Some(b'!') => {
                let path = line.get(2..).unwrap_or("").to_string();
                entries.push(StatusEntry {
                    path,
                    status: FileStatus::Ignored,
                    staged: false,
                });
            }
            _ => {}
        }
    }

    Ok(entries)
}

/// Parse a "1 XY sub mH mI mW hH hI path" line.
fn parse_ordinary_entry(line: &str) -> Option<StatusEntry> {
    let parts: Vec<&str> = line.splitn(9, ' ').collect();
    if parts.len() < 9 {
        return None;
    }
    let xy = parts[1];
    let path = parts[8].to_string();

    let x = xy.as_bytes().first().copied().unwrap_or(b'.');
    let y = xy.as_bytes().get(1).copied().unwrap_or(b'.');

    let (status, staged) = if x == b'.' {
        (char_to_status(y), false)
    } else {
        (char_to_status(x), true)
    };

    Some(StatusEntry {
        path,
        status,
        staged,
    })
}

/// Parse a "2 XY sub mH mI mW hH hI X### origPath\tpath" line.
fn parse_rename_entry(line: &str) -> Option<StatusEntry> {
    let parts: Vec<&str> = line.splitn(10, ' ').collect();
    if parts.len() < 10 {
        return None;
    }
    let xy = parts[1];
    let paths_part = parts[9];

    let path = paths_part
        .split('\t')
        .next_back()
        .unwrap_or(paths_part)
        .to_string();

    let x = xy.as_bytes().first().copied().unwrap_or(b'.');
    let staged = x != b'.';

    Some(StatusEntry {
        path,
        status: FileStatus::Renamed,
        staged,
    })
}

/// Parse a "u XY sub m1 m2 m3 hH path" line.
fn parse_unmerged_entry(line: &str) -> Option<StatusEntry> {
    let parts: Vec<&str> = line.splitn(8, ' ').collect();
    if parts.len() < 8 {
        return None;
    }
    let path = parts[7].to_string();
    Some(StatusEntry {
        path,
        status: FileStatus::Conflicted,
        staged: false,
    })
}

fn char_to_status(c: u8) -> FileStatus {
    match c {
        b'A' => FileStatus::Added,
        b'D' => FileStatus::Deleted,
        b'R' => FileStatus::Renamed,
        b'C' => FileStatus::Copied,
        _ => FileStatus::Modified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["init", "-b", "main"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["config", "user.name", "Test"])
            .output()
            .unwrap();
        tmp
    }

    #[test]
    fn untracked_files_detected() {
        let tmp = init_repo();
        fs::write(tmp.path().join("new.txt"), "hello").unwrap();

        let entries = get_status(tmp.path()).unwrap();
        assert!(!entries.is_empty());
        assert!(entries
            .iter()
            .any(|e| e.path == "new.txt" && e.status == FileStatus::Untracked));
    }

    #[test]
    fn staged_files_detected() {
        let tmp = init_repo();
        fs::write(tmp.path().join("a.txt"), "a").unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["add", "a.txt"])
            .output()
            .unwrap();

        let entries = get_status(tmp.path()).unwrap();
        let entry = entries.iter().find(|e| e.path == "a.txt").unwrap();
        assert!(entry.staged);
        assert_eq!(entry.status, FileStatus::Added);
    }
}
