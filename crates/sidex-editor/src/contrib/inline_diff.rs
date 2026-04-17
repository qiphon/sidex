//! Inline diff decorations for dirty files.
//!
//! When a file has unsaved changes relative to HEAD, this module provides
//! decoration data for:
//! - Deleted text shown with a red background (inline strikethrough)
//! - Added text shown with a green background
//! - Modified lines with a ghost line above showing the old content

use serde::{Deserialize, Serialize};

/// The kind of inline diff decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InlineChangeKind {
    /// Text was added — render with green background.
    Added,
    /// Text was deleted — render as ghost/strikethrough with red background.
    Deleted,
    /// Line was modified — render old content as a ghost line above.
    Modified,
}

/// A single inline diff decoration for the editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineChange {
    /// 1-based line number in the modified document.
    pub line: u32,
    pub kind: InlineChangeKind,
    /// For `Deleted`/`Modified`: the original text content.
    pub old_text: Option<String>,
    /// For `Added`/`Modified`: the new text content.
    pub new_text: Option<String>,
}

/// Full inline diff state for a document.
#[derive(Debug, Clone, Default)]
pub struct InlineDiffState {
    pub changes: Vec<InlineChange>,
    pub enabled: bool,
}

impl InlineDiffState {
    pub fn new() -> Self {
        Self {
            changes: Vec::new(),
            enabled: true,
        }
    }

    /// Toggle inline diff display.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Whether inline diff display is active.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Replace all inline changes with newly computed ones.
    pub fn set_changes(&mut self, changes: Vec<InlineChange>) {
        self.changes = changes;
    }

    /// Clear all inline diff data.
    pub fn clear(&mut self) {
        self.changes.clear();
    }

    /// Get changes for a specific line.
    pub fn changes_at(&self, line: u32) -> Vec<&InlineChange> {
        self.changes.iter().filter(|c| c.line == line).collect()
    }

    /// Get the ghost line text to render above a modified line (if any).
    pub fn ghost_line(&self, line: u32) -> Option<&str> {
        self.changes.iter().find_map(|c| {
            if c.line == line && c.kind == InlineChangeKind::Modified {
                c.old_text.as_deref()
            } else {
                None
            }
        })
    }

    /// Returns all changes visible in `[first_line, first_line + count)`.
    pub fn visible_changes(&self, first_line: u32, count: u32) -> Vec<&InlineChange> {
        if !self.enabled {
            return Vec::new();
        }
        let end = first_line + count;
        self.changes
            .iter()
            .filter(|c| c.line >= first_line && c.line < end)
            .collect()
    }
}

/// Compute inline diff changes by diffing `original` (HEAD) against `modified`.
pub fn compute_inline_changes(original: &str, modified: &str) -> Vec<InlineChange> {
    let old_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = modified.lines().collect();

    let lcs = lcs_table(&old_lines, &new_lines);
    let ops = backtrack(&old_lines, &new_lines, &lcs);

    let mut changes = Vec::new();
    let mut idx = 0;
    while idx < ops.len() {
        match ops[idx] {
            EditOp::Equal => {
                idx += 1;
            }
            EditOp::Insert(new_idx) => {
                changes.push(InlineChange {
                    line: (new_idx + 1) as u32,
                    kind: InlineChangeKind::Added,
                    old_text: None,
                    new_text: Some(new_lines[new_idx].to_string()),
                });
                idx += 1;
            }
            EditOp::Delete(old_idx) => {
                if idx + 1 < ops.len() {
                    if let EditOp::Insert(new_idx) = ops[idx + 1] {
                        changes.push(InlineChange {
                            line: (new_idx + 1) as u32,
                            kind: InlineChangeKind::Modified,
                            old_text: Some(old_lines[old_idx].to_string()),
                            new_text: Some(new_lines[new_idx].to_string()),
                        });
                        idx += 2;
                        continue;
                    }
                }
                let line = if idx > 0 {
                    nearest_new_line(&ops, idx)
                } else {
                    1
                };
                changes.push(InlineChange {
                    line: line as u32,
                    kind: InlineChangeKind::Deleted,
                    old_text: Some(old_lines[old_idx].to_string()),
                    new_text: None,
                });
                idx += 1;
            }
        }
    }

    changes
}

fn nearest_new_line(ops: &[EditOp], pos: usize) -> usize {
    for i in (0..pos).rev() {
        match ops[i] {
            EditOp::Equal | EditOp::Insert(_) => {
                let mut line = 1;
                for op in &ops[..=i] {
                    if matches!(op, EditOp::Equal | EditOp::Insert(_)) {
                        line += 1;
                    }
                }
                return line;
            }
            _ => {}
        }
    }
    1
}

// ── LCS internals ────────────────────────────────────────────────────────────

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

#[derive(Clone, Copy)]
enum EditOp {
    Equal,
    Insert(usize),
    Delete(usize),
}

fn backtrack(old: &[&str], new: &[&str], lcs: &[Vec<u32>]) -> Vec<EditOp> {
    let mut ops = Vec::new();
    let mut i = old.len();
    let mut j = new.len();

    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            ops.push(EditOp::Equal);
            i -= 1;
            j -= 1;
        } else if lcs[i - 1][j] >= lcs[i][j - 1] {
            ops.push(EditOp::Delete(i - 1));
            i -= 1;
        } else {
            ops.push(EditOp::Insert(j - 1));
            j -= 1;
        }
    }
    while i > 0 {
        ops.push(EditOp::Delete(i - 1));
        i -= 1;
    }
    while j > 0 {
        ops.push(EditOp::Insert(j - 1));
        j -= 1;
    }
    ops.reverse();
    ops
}

/// Rendering colors for inline diff overlays.
#[derive(Debug, Clone, Copy)]
pub struct InlineDiffColors {
    /// Background for added text (default: translucent green).
    pub added_bg: [f32; 4],
    /// Background for deleted text / ghost line (default: translucent red).
    pub deleted_bg: [f32; 4],
    /// Text color for ghost lines (dimmed).
    pub ghost_text: [f32; 4],
}

impl Default for InlineDiffColors {
    fn default() -> Self {
        Self {
            added_bg: [0.6, 0.8, 0.3, 0.13],
            deleted_bg: [1.0, 0.0, 0.0, 0.13],
            ghost_text: [0.7, 0.3, 0.3, 0.5],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_changes_empty() {
        let changes = compute_inline_changes("a\nb\nc", "a\nb\nc");
        assert!(changes.is_empty());
    }

    #[test]
    fn detect_added_line() {
        let changes = compute_inline_changes("a\nc", "a\nb\nc");
        assert!(changes.iter().any(|c| c.kind == InlineChangeKind::Added));
    }

    #[test]
    fn detect_deleted_line() {
        let changes = compute_inline_changes("a\nb\nc", "a\nc");
        assert!(changes.iter().any(|c| c.kind == InlineChangeKind::Deleted));
    }

    #[test]
    fn detect_modified_line() {
        let changes = compute_inline_changes("a\nb\nc", "a\nx\nc");
        assert!(
            changes.iter().any(|c| c.kind == InlineChangeKind::Modified)
        );
    }

    #[test]
    fn ghost_line_for_modified() {
        let changes = compute_inline_changes("a\nb\nc", "a\nx\nc");
        let mut state = InlineDiffState::new();
        state.set_changes(changes);
        let modified_line = state.changes.iter().find(|c| c.kind == InlineChangeKind::Modified);
        if let Some(mc) = modified_line {
            assert!(state.ghost_line(mc.line).is_some());
        }
    }

    #[test]
    fn visible_changes_filter() {
        let mut state = InlineDiffState::new();
        state.set_changes(vec![
            InlineChange {
                line: 1,
                kind: InlineChangeKind::Added,
                old_text: None,
                new_text: Some("x".into()),
            },
            InlineChange {
                line: 50,
                kind: InlineChangeKind::Deleted,
                old_text: Some("y".into()),
                new_text: None,
            },
        ]);
        let vis = state.visible_changes(1, 10);
        assert_eq!(vis.len(), 1);
    }

    #[test]
    fn toggle_disables_visible() {
        let mut state = InlineDiffState::new();
        state.set_changes(vec![InlineChange {
            line: 1,
            kind: InlineChangeKind::Added,
            old_text: None,
            new_text: Some("x".into()),
        }]);
        state.toggle();
        assert!(state.visible_changes(1, 10).is_empty());
    }
}
