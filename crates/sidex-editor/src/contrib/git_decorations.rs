//! Git gutter decorations — colored bars and triangles in the editor gutter
//! showing added, modified, and deleted lines relative to HEAD.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The kind of change for a single line in the gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineChangeKind {
    /// Green bar — line was added.
    Added,
    /// Blue bar — line was modified.
    Modified,
    /// Red triangle — lines were deleted at this position.
    Deleted,
}

/// A change indicator for a single editor line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineChange {
    pub line: u32,
    pub kind: LineChangeKind,
}

/// Computed git gutter decorations for an entire document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitDecorations {
    pub line_changes: Vec<LineChange>,
}

impl GitDecorations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the change kind for a specific line, if any.
    pub fn change_at(&self, line: u32) -> Option<LineChangeKind> {
        self.line_changes
            .iter()
            .find(|c| c.line == line)
            .map(|c| c.kind)
    }

    /// Returns only the changes visible in `[first_line, first_line + count)`.
    pub fn visible_changes(&self, first_line: u32, count: u32) -> Vec<&LineChange> {
        let end = first_line + count;
        self.line_changes
            .iter()
            .filter(|c| c.line >= first_line && c.line < end)
            .collect()
    }

    /// Returns the number of changes by kind.
    pub fn summary(&self) -> DecorationSummary {
        let mut added = 0u32;
        let mut modified = 0u32;
        let mut deleted = 0u32;
        for c in &self.line_changes {
            match c.kind {
                LineChangeKind::Added => added += 1,
                LineChangeKind::Modified => modified += 1,
                LineChangeKind::Deleted => deleted += 1,
            }
        }
        DecorationSummary {
            added,
            modified,
            deleted,
        }
    }
}

/// Summary counts of git gutter decorations.
#[derive(Debug, Clone, Copy, Default)]
pub struct DecorationSummary {
    pub added: u32,
    pub modified: u32,
    pub deleted: u32,
}

impl DecorationSummary {
    pub fn total(&self) -> u32 {
        self.added + self.modified + self.deleted
    }
}

/// Compute gutter decorations by diffing `original` (HEAD) against `modified`
/// (working tree) line by line.
pub fn compute_git_decorations(original: &str, modified: &str) -> GitDecorations {
    let old_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = modified.lines().collect();

    let lcs = lcs_table(&old_lines, &new_lines);
    let ops = backtrack(&old_lines, &new_lines, &lcs);

    let mut line_changes = Vec::new();
    collapse_ops_to_changes(&ops, &mut line_changes);

    GitDecorations { line_changes }
}

// ── LCS diff internals ──────────────────────────────────────────────────────

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffOp {
    Equal,
    Insert(u32),
    Delete(u32),
}

fn backtrack(old: &[&str], new: &[&str], lcs: &[Vec<u32>]) -> Vec<DiffOp> {
    let mut ops = Vec::new();
    let mut i = old.len();
    let mut j = new.len();

    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            ops.push(DiffOp::Equal);
            i -= 1;
            j -= 1;
        } else if lcs[i - 1][j] >= lcs[i][j - 1] {
            ops.push(DiffOp::Delete(i as u32));
            i -= 1;
        } else {
            ops.push(DiffOp::Insert(j as u32));
            j -= 1;
        }
    }
    while i > 0 {
        ops.push(DiffOp::Delete(i as u32));
        i -= 1;
    }
    while j > 0 {
        ops.push(DiffOp::Insert(j as u32));
        j -= 1;
    }
    ops.reverse();
    ops
}

/// Collapse raw diff ops into line-level gutter changes.
///
/// Adjacent insert+delete pairs become Modified; isolated inserts are Added;
/// isolated deletes become a single Deleted marker at the line boundary.
fn collapse_ops_to_changes(ops: &[DiffOp], out: &mut Vec<LineChange>) {
    let mut seen_inserts: HashMap<u32, bool> = HashMap::new();
    let mut seen_deletes: Vec<u32> = Vec::new();

    let mut idx = 0;
    while idx < ops.len() {
        match ops[idx] {
            DiffOp::Equal => {
                flush_pending(&mut seen_inserts, &mut seen_deletes, out);
                idx += 1;
            }
            DiffOp::Insert(line) => {
                seen_inserts.insert(line, false);
                idx += 1;
            }
            DiffOp::Delete(_line) => {
                seen_deletes.push(
                    seen_inserts
                        .keys()
                        .copied()
                        .next()
                        .unwrap_or(_line),
                );
                if let Some(first_ins) = seen_inserts.keys().copied().next() {
                    seen_inserts.insert(first_ins, true);
                }
                idx += 1;
            }
        }
    }
    flush_pending(&mut seen_inserts, &mut seen_deletes, out);
}

fn flush_pending(
    inserts: &mut HashMap<u32, bool>,
    deletes: &mut Vec<u32>,
    out: &mut Vec<LineChange>,
) {
    let has_deletes = !deletes.is_empty();
    let has_inserts = !inserts.is_empty();

    if has_inserts && has_deletes {
        for (&line, paired) in inserts.iter() {
            let kind = if *paired {
                LineChangeKind::Modified
            } else {
                LineChangeKind::Added
            };
            out.push(LineChange { line, kind });
        }
        let unpaired_deletes = deletes.len().saturating_sub(inserts.len());
        if unpaired_deletes > 0 {
            let marker_line = inserts.keys().copied().min().unwrap_or(1);
            out.push(LineChange {
                line: marker_line,
                kind: LineChangeKind::Deleted,
            });
        }
    } else if has_inserts {
        for &line in inserts.keys() {
            out.push(LineChange {
                line,
                kind: LineChangeKind::Added,
            });
        }
    } else if has_deletes {
        let marker = deletes.first().copied().unwrap_or(1);
        out.push(LineChange {
            line: marker,
            kind: LineChangeKind::Deleted,
        });
    }

    inserts.clear();
    deletes.clear();
}

/// Gutter rendering dimensions for git decorations.
#[derive(Debug, Clone, Copy)]
pub struct GitGutterStyle {
    /// Width of the colored bar in pixels (default 3).
    pub bar_width: f32,
    /// Height of the deleted triangle in pixels (default 6).
    pub triangle_height: f32,
}

impl Default for GitGutterStyle {
    fn default() -> Self {
        Self {
            bar_width: 3.0,
            triangle_height: 6.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_changes() {
        let decs = compute_git_decorations("a\nb\nc", "a\nb\nc");
        assert!(decs.line_changes.is_empty());
    }

    #[test]
    fn added_lines() {
        let decs = compute_git_decorations("a\nc", "a\nb\nc");
        assert!(
            decs.line_changes
                .iter()
                .any(|c| c.kind == LineChangeKind::Added)
        );
    }

    #[test]
    fn deleted_lines() {
        let decs = compute_git_decorations("a\nb\nc", "a\nc");
        assert!(
            decs.line_changes
                .iter()
                .any(|c| c.kind == LineChangeKind::Deleted)
        );
    }

    #[test]
    fn modified_lines() {
        let decs = compute_git_decorations("a\nb\nc", "a\nx\nc");
        let kinds: Vec<_> = decs.line_changes.iter().map(|c| c.kind).collect();
        assert!(
            kinds.contains(&LineChangeKind::Modified)
                || (kinds.contains(&LineChangeKind::Added)
                    && kinds.contains(&LineChangeKind::Deleted))
        );
    }

    #[test]
    fn summary_counts() {
        let decs = compute_git_decorations("a\nb\nc", "a\nx\ny\nc");
        let s = decs.summary();
        assert!(s.total() > 0);
    }

    #[test]
    fn visible_changes_filter() {
        let decs = GitDecorations {
            line_changes: vec![
                LineChange {
                    line: 1,
                    kind: LineChangeKind::Added,
                },
                LineChange {
                    line: 5,
                    kind: LineChangeKind::Modified,
                },
                LineChange {
                    line: 100,
                    kind: LineChangeKind::Deleted,
                },
            ],
        };
        let vis = decs.visible_changes(1, 10);
        assert_eq!(vis.len(), 2);
    }
}
