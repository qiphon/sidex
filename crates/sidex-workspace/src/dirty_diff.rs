//! Working tree diff provider.
//!
//! Computes diffs between the current buffer content and the last committed
//! version from git HEAD. Used by git gutter decorations, minimap marks,
//! and scroll bar marks. Debounces recomputation to avoid excessive work.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::Serialize;

/// Classification of a single line change in the working tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DirtyDiffKind {
    Added,
    Removed,
    Modified,
}

/// A contiguous hunk of dirty changes.
#[derive(Debug, Clone, Serialize)]
pub struct DiffHunk {
    pub original_start: u32,
    pub original_count: u32,
    pub modified_start: u32,
    pub modified_count: u32,
    pub kind: DirtyDiffKind,
}

/// Per-file dirty diff entry.
#[derive(Debug, Clone)]
pub struct FileDirtyDiff {
    pub hunks: Vec<DiffHunk>,
    pub last_computed: Instant,
}

/// Provider that tracks dirty diffs for open files.
pub struct DirtyDiffProvider {
    /// Per-path dirty diff data.
    pub diffs: HashMap<PathBuf, FileDirtyDiff>,
    /// Debounce interval in milliseconds.
    pub debounce_ms: u64,
}

impl Default for DirtyDiffProvider {
    fn default() -> Self {
        Self {
            diffs: HashMap::new(),
            debounce_ms: 300,
        }
    }
}

impl DirtyDiffProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the debounce interval.
    #[must_use]
    pub fn with_debounce_ms(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Update the dirty diff for a file if the debounce interval has elapsed.
    /// Returns `true` if recomputed.
    pub fn update_if_stale(
        &mut self,
        path: &Path,
        current_content: &str,
        repo_root: &Path,
    ) -> bool {
        let now = Instant::now();
        if let Some(existing) = self.diffs.get(path) {
            #[allow(clippy::cast_possible_truncation)]
            let elapsed = now.duration_since(existing.last_computed).as_millis() as u64;
            if elapsed < self.debounce_ms {
                return false;
            }
        }

        self.recompute(path, current_content, repo_root);
        true
    }

    /// Force recomputation of the dirty diff for a file.
    pub fn recompute(&mut self, path: &Path, current_content: &str, repo_root: &Path) {
        let Ok(original) = get_original_content(path, repo_root) else {
            self.diffs.remove(path);
            return;
        };

        let hunks = compute_dirty_diff(&original, current_content);
        self.diffs.insert(
            path.to_path_buf(),
            FileDirtyDiff {
                hunks,
                last_computed: Instant::now(),
            },
        );
    }

    /// Get cached hunks for a file.
    pub fn hunks(&self, path: &Path) -> &[DiffHunk] {
        self.diffs.get(path).map_or(&[], |d| d.hunks.as_slice())
    }

    /// Remove cached diff data for a file.
    pub fn remove(&mut self, path: &Path) {
        self.diffs.remove(path);
    }

    /// Clear all cached diffs.
    pub fn clear(&mut self) {
        self.diffs.clear();
    }

    /// Returns gutter decoration data: `(line, kind)` pairs for the modified file.
    pub fn gutter_decorations(&self, path: &Path) -> Vec<(u32, DirtyDiffKind)> {
        let mut result = Vec::new();
        for hunk in self.hunks(path) {
            match hunk.kind {
                DirtyDiffKind::Added => {
                    for i in 0..hunk.modified_count {
                        result.push((hunk.modified_start + i, DirtyDiffKind::Added));
                    }
                }
                DirtyDiffKind::Removed => {
                    result.push((hunk.modified_start, DirtyDiffKind::Removed));
                }
                DirtyDiffKind::Modified => {
                    for i in 0..hunk.modified_count {
                        result.push((hunk.modified_start + i, DirtyDiffKind::Modified));
                    }
                }
            }
        }
        result
    }

    /// Returns minimap/scrollbar marks: `(line_fraction, kind)` pairs.
    #[allow(clippy::cast_precision_loss)]
    pub fn scrollbar_marks(&self, path: &Path, total_lines: u32) -> Vec<(f32, DirtyDiffKind)> {
        if total_lines == 0 {
            return Vec::new();
        }
        self.gutter_decorations(path)
            .iter()
            .map(|(line, kind)| (*line as f32 / total_lines as f32, *kind))
            .collect()
    }
}

/// Get the HEAD version of a file from git.
pub fn get_original_content(path: &Path, repo_root: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(repo_root).unwrap_or(path);
    let relative_str = relative.to_string_lossy();

    let output = Command::new("git")
        .args(["show", &format!("HEAD:{relative_str}")])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git show failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Compute dirty diff hunks between the original (HEAD) and current content.
#[allow(clippy::cast_possible_truncation)]
pub fn compute_dirty_diff(original: &str, current: &str) -> Vec<DiffHunk> {
    let old_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = current.lines().collect();

    let lcs = lcs_table(&old_lines, &new_lines);
    let ops = build_edit_ops(&old_lines, &new_lines, &lcs);

    let mut hunks = Vec::new();
    let mut i = 0;
    while i < ops.len() {
        if let EditOp::Equal = ops[i] {
            i += 1;
        } else {
            let start = i;
            let old_start = count_old_before(&ops, start);
            let new_start = count_new_before(&ops, start);
            let mut old_count = 0u32;
            let mut new_count = 0u32;

            while i < ops.len() && !matches!(ops[i], EditOp::Equal) {
                match ops[i] {
                    EditOp::Delete => old_count += 1,
                    EditOp::Insert => new_count += 1,
                    EditOp::Equal => unreachable!(),
                }
                i += 1;
            }

            let kind = match (old_count, new_count) {
                (0, _) => DirtyDiffKind::Added,
                (_, 0) => DirtyDiffKind::Removed,
                _ => DirtyDiffKind::Modified,
            };

            hunks.push(DiffHunk {
                original_start: (old_start + 1) as u32,
                original_count: old_count,
                modified_start: (new_start + 1) as u32,
                modified_count: new_count,
                kind,
            });

            let _ = old_start;
            let _ = new_start;
        }
    }

    hunks
}

fn count_old_before(ops: &[EditOp], pos: usize) -> usize {
    ops[..pos]
        .iter()
        .filter(|o| matches!(o, EditOp::Equal | EditOp::Delete))
        .count()
}

fn count_new_before(ops: &[EditOp], pos: usize) -> usize {
    ops[..pos]
        .iter()
        .filter(|o| matches!(o, EditOp::Equal | EditOp::Insert))
        .count()
}

#[derive(Clone, Copy)]
enum EditOp {
    Equal,
    Insert,
    Delete,
}

#[allow(clippy::many_single_char_names)]
fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<u32>> {
    let m = a.len();
    let n = b.len();
    let mut t = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            t[i][j] = if a[i - 1] == b[j - 1] {
                t[i - 1][j - 1] + 1
            } else {
                t[i - 1][j].max(t[i][j - 1])
            };
        }
    }
    t
}

fn build_edit_ops(old: &[&str], new: &[&str], lcs: &[Vec<u32>]) -> Vec<EditOp> {
    let mut ops = Vec::new();
    let mut i = old.len();
    let mut j = new.len();

    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            ops.push(EditOp::Equal);
            i -= 1;
            j -= 1;
        } else if lcs[i - 1][j] >= lcs[i][j - 1] {
            ops.push(EditOp::Delete);
            i -= 1;
        } else {
            ops.push(EditOp::Insert);
            j -= 1;
        }
    }
    while i > 0 {
        ops.push(EditOp::Delete);
        i -= 1;
    }
    while j > 0 {
        ops.push(EditOp::Insert);
        j -= 1;
    }
    ops.reverse();
    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_changes() {
        let hunks = compute_dirty_diff("a\nb\nc", "a\nb\nc");
        assert!(hunks.is_empty());
    }

    #[test]
    fn detect_added_lines() {
        let hunks = compute_dirty_diff("a\nc", "a\nb\nc");
        assert!(!hunks.is_empty());
        assert!(hunks.iter().any(|h| h.kind == DirtyDiffKind::Added));
    }

    #[test]
    fn detect_removed_lines() {
        let hunks = compute_dirty_diff("a\nb\nc", "a\nc");
        assert!(!hunks.is_empty());
        assert!(hunks.iter().any(|h| h.kind == DirtyDiffKind::Removed));
    }

    #[test]
    fn detect_modified_lines() {
        let hunks = compute_dirty_diff("a\nb\nc", "a\nx\nc");
        assert!(!hunks.is_empty());
        assert!(hunks.iter().any(|h| h.kind == DirtyDiffKind::Modified));
    }

    #[test]
    fn provider_basics() {
        let provider = DirtyDiffProvider::new();
        let path = PathBuf::from("/tmp/test.rs");
        assert!(provider.hunks(&path).is_empty());
    }

    #[test]
    fn gutter_decorations_for_added() {
        let mut provider = DirtyDiffProvider::new();
        let path = PathBuf::from("/tmp/test.rs");
        provider.diffs.insert(
            path.clone(),
            FileDirtyDiff {
                hunks: vec![DiffHunk {
                    original_start: 2,
                    original_count: 0,
                    modified_start: 2,
                    modified_count: 3,
                    kind: DirtyDiffKind::Added,
                }],
                last_computed: Instant::now(),
            },
        );
        let decorations = provider.gutter_decorations(&path);
        assert_eq!(decorations.len(), 3);
        assert!(decorations.iter().all(|(_, k)| *k == DirtyDiffKind::Added));
    }

    #[test]
    fn scrollbar_marks_basic() {
        let mut provider = DirtyDiffProvider::new();
        let path = PathBuf::from("/tmp/test.rs");
        provider.diffs.insert(
            path.clone(),
            FileDirtyDiff {
                hunks: vec![DiffHunk {
                    original_start: 5,
                    original_count: 1,
                    modified_start: 5,
                    modified_count: 1,
                    kind: DirtyDiffKind::Modified,
                }],
                last_computed: Instant::now(),
            },
        );
        let marks = provider.scrollbar_marks(&path, 100);
        assert_eq!(marks.len(), 1);
    }

    #[test]
    fn clear_and_remove() {
        let mut provider = DirtyDiffProvider::new();
        let path = PathBuf::from("/tmp/test.rs");
        provider.diffs.insert(
            path.clone(),
            FileDirtyDiff {
                hunks: Vec::new(),
                last_computed: Instant::now(),
            },
        );
        assert!(provider.diffs.contains_key(&path));
        provider.remove(&path);
        assert!(!provider.diffs.contains_key(&path));

        provider.diffs.insert(
            path.clone(),
            FileDirtyDiff {
                hunks: Vec::new(),
                last_computed: Instant::now(),
            },
        );
        provider.clear();
        assert!(provider.diffs.is_empty());
    }
}
