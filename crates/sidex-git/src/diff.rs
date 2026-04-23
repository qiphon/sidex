//! Git diff — file diffs, staged diffs, and line-level diff info.

use std::fmt::Write;
use std::path::Path;

use serde::Serialize;

use crate::cmd::run_git;
use crate::error::GitResult;

/// The kind of change on a single line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum LineDiffKind {
    Added,
    Removed,
    Modified,
}

/// A single line-level diff entry (for gutter decorations).
#[derive(Debug, Clone, Serialize)]
pub struct LineDiff {
    pub line_number: usize,
    pub kind: LineDiffKind,
}

/// Get the full diff for a specific file (unstaged changes).
pub fn get_diff(repo_root: &Path, path: &Path) -> GitResult<String> {
    let path_str = path.to_string_lossy();
    let output = run_git(repo_root, &["diff", "--", &path_str])?;
    Ok(output)
}

/// Get the staged diff for a specific file.
pub fn get_diff_staged(repo_root: &Path, path: &Path) -> GitResult<String> {
    let path_str = path.to_string_lossy();
    let output = run_git(repo_root, &["diff", "--staged", "--", &path_str])?;
    Ok(output)
}

/// Parse `git diff` output into line-level diffs for gutter decorations.
pub fn get_line_diffs(repo_root: &Path, path: &Path) -> GitResult<Vec<LineDiff>> {
    let path_str = path.to_string_lossy();
    let output = run_git(
        repo_root,
        &["diff", "--unified=0", "--no-color", "--", &path_str],
    )?;
    Ok(parse_unified_diff(&output))
}

/// Parse a unified diff (with `--unified=0`) into `LineDiff` entries.
fn parse_unified_diff(diff: &str) -> Vec<LineDiff> {
    let mut diffs = Vec::new();

    for line in diff.lines() {
        // Hunk headers: @@ -old_start[,old_count] +new_start[,new_count] @@
        if let Some(hunk) = line.strip_prefix("@@ ") {
            if let Some((removed, added)) = parse_hunk_header(hunk) {
                if removed.count > 0 && added.count == 0 {
                    // Lines were deleted before `added.start`
                    diffs.push(LineDiff {
                        line_number: added.start.max(1),
                        kind: LineDiffKind::Removed,
                    });
                } else if removed.count == 0 && added.count > 0 {
                    for i in 0..added.count {
                        diffs.push(LineDiff {
                            line_number: added.start + i,
                            kind: LineDiffKind::Added,
                        });
                    }
                } else {
                    for i in 0..added.count {
                        diffs.push(LineDiff {
                            line_number: added.start + i,
                            kind: LineDiffKind::Modified,
                        });
                    }
                }
            }
        }
    }

    diffs
}

struct HunkRange {
    start: usize,
    count: usize,
}

fn parse_hunk_header(header: &str) -> Option<(HunkRange, HunkRange)> {
    // Format: "-old_start[,old_count] +new_start[,new_count] @@..."
    let parts: Vec<&str> = header.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let removed = parse_range(parts[0].strip_prefix('-')?)?;
    let added = parse_range(parts[1].strip_prefix('+')?)?;

    Some((removed, added))
}

fn parse_range(s: &str) -> Option<HunkRange> {
    if let Some((start, count)) = s.split_once(',') {
        Some(HunkRange {
            start: start.parse().ok()?,
            count: count.parse().ok()?,
        })
    } else {
        Some(HunkRange {
            start: s.parse().ok()?,
            count: 1,
        })
    }
}

// ── Rich diff types ──────────────────────────────────────────────────────────

/// The kind of a single diff line inside a hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

/// A single line within a [`DiffHunk`].
#[derive(Debug, Clone, Serialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

/// A contiguous hunk of changes between original and modified text.
#[derive(Debug, Clone, Serialize)]
pub struct DiffHunk {
    pub original_start: u32,
    pub original_count: u32,
    pub modified_start: u32,
    pub modified_count: u32,
    pub lines: Vec<DiffLine>,
}

/// Compute structured hunks by diffing two strings line-by-line.
///
/// Uses a simple LCS-based approach: walk both texts, emit context around
/// changed regions (up to 3 lines), and group consecutive changes into hunks.
pub fn compute_hunks(original: &str, modified: &str) -> Vec<DiffHunk> {
    let old_lines: Vec<&str> = original.lines().collect();
    let new_lines: Vec<&str> = modified.lines().collect();

    let lcs = lcs_table(&old_lines, &new_lines);
    let raw = build_raw_diff(&old_lines, &new_lines, &lcs);

    group_into_hunks(&raw, &old_lines, &new_lines, 3)
}

/// Format hunks as a standard unified diff string.
pub fn format_unified_diff(hunks: &[DiffHunk], original_name: &str, modified_name: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "--- {original_name}");
    let _ = writeln!(out, "+++ {modified_name}");

    for hunk in hunks {
        let _ = writeln!(
            out,
            "@@ -{},{} +{},{} @@",
            hunk.original_start, hunk.original_count, hunk.modified_start, hunk.modified_count
        );
        for line in &hunk.lines {
            let prefix = match line.kind {
                DiffLineKind::Context => ' ',
                DiffLineKind::Added => '+',
                DiffLineKind::Removed => '-',
            };
            out.push(prefix);
            out.push_str(&line.content);
            out.push('\n');
        }
    }
    out
}

/// Apply hunks to the original text to produce the modified text.
pub fn apply_hunks(original: &str, hunks: &[DiffHunk]) -> String {
    let old_lines: Vec<&str> = original.lines().collect();
    let mut result = Vec::new();
    let mut old_idx: usize = 0;

    for hunk in hunks {
        let hunk_start = if hunk.original_start == 0 {
            0
        } else {
            (hunk.original_start - 1) as usize
        };

        while old_idx < hunk_start && old_idx < old_lines.len() {
            result.push(old_lines[old_idx].to_string());
            old_idx += 1;
        }

        for dl in &hunk.lines {
            match dl.kind {
                DiffLineKind::Added | DiffLineKind::Context => {
                    result.push(dl.content.clone());
                }
                DiffLineKind::Removed => {}
            }
        }
        old_idx = hunk_start + hunk.original_count as usize;
    }

    while old_idx < old_lines.len() {
        result.push(old_lines[old_idx].to_string());
        old_idx += 1;
    }

    let mut out = result.join("\n");
    if original.ends_with('\n') || modified_ends_with_newline(&result) {
        out.push('\n');
    }
    out
}

fn modified_ends_with_newline(_lines: &[String]) -> bool {
    false
}

/// Revert a single hunk: apply all hunks *except* `hunk_idx`.
pub fn revert_hunk(original: &str, modified: &str, hunk_idx: usize) -> String {
    let hunks = compute_hunks(original, modified);
    let kept: Vec<DiffHunk> = hunks
        .into_iter()
        .enumerate()
        .filter(|(i, _)| *i != hunk_idx)
        .map(|(_, h)| h)
        .collect();
    apply_hunks(original, &kept)
}

// ── Internal LCS helpers ─────────────────────────────────────────────────────

fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<u32>> {
    let m = a.len();
    let n = b.len();
    let mut table = vec![vec![0u32; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                table[i][j] = table[i - 1][j - 1] + 1;
            } else {
                table[i][j] = table[i - 1][j].max(table[i][j - 1]);
            }
        }
    }
    table
}

#[derive(Clone, Copy)]
enum RawOp {
    Equal(usize, usize),
    Remove(usize),
    Add(usize),
}

fn build_raw_diff<'a>(old: &[&'a str], new: &[&'a str], lcs: &[Vec<u32>]) -> Vec<RawOp> {
    let mut ops = Vec::new();
    let mut i = old.len();
    let mut j = new.len();

    while i > 0 && j > 0 {
        if old[i - 1] == new[j - 1] {
            ops.push(RawOp::Equal(i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if lcs[i - 1][j] >= lcs[i][j - 1] {
            ops.push(RawOp::Remove(i - 1));
            i -= 1;
        } else {
            ops.push(RawOp::Add(j - 1));
            j -= 1;
        }
    }
    while i > 0 {
        ops.push(RawOp::Remove(i - 1));
        i -= 1;
    }
    while j > 0 {
        ops.push(RawOp::Add(j - 1));
        j -= 1;
    }
    ops.reverse();
    ops
}

#[allow(clippy::cast_possible_truncation)]
fn group_into_hunks(ops: &[RawOp], old: &[&str], new: &[&str], context: usize) -> Vec<DiffHunk> {
    let mut hunks: Vec<DiffHunk> = Vec::new();

    let change_ranges = find_change_ranges(ops, context);

    for (start, end) in change_ranges {
        let mut lines = Vec::new();
        let mut o_start = u32::MAX;
        let mut o_count = 0u32;
        let mut m_start = u32::MAX;
        let mut m_count = 0u32;

        for op in ops.iter().skip(start).take(end - start) {
            match *op {
                RawOp::Equal(oi, ni) => {
                    if o_start == u32::MAX {
                        o_start = (oi + 1) as u32;
                        m_start = (ni + 1) as u32;
                    }
                    o_count += 1;
                    m_count += 1;
                    lines.push(DiffLine {
                        kind: DiffLineKind::Context,
                        content: old[oi].to_string(),
                    });
                }
                RawOp::Remove(oi) => {
                    if o_start == u32::MAX {
                        o_start = (oi + 1) as u32;
                        m_start = infer_mod_start(ops, start, end) as u32;
                    }
                    o_count += 1;
                    lines.push(DiffLine {
                        kind: DiffLineKind::Removed,
                        content: old[oi].to_string(),
                    });
                }
                RawOp::Add(ni) => {
                    if o_start == u32::MAX {
                        o_start = infer_orig_start(ops, start, end) as u32;
                        m_start = (ni + 1) as u32;
                    }
                    m_count += 1;
                    lines.push(DiffLine {
                        kind: DiffLineKind::Added,
                        content: new[ni].to_string(),
                    });
                }
            }
        }

        if o_start == u32::MAX {
            o_start = 1;
        }
        if m_start == u32::MAX {
            m_start = 1;
        }

        hunks.push(DiffHunk {
            original_start: o_start,
            original_count: o_count,
            modified_start: m_start,
            modified_count: m_count,
            lines,
        });
    }

    hunks
}

fn find_change_ranges(ops: &[RawOp], context: usize) -> Vec<(usize, usize)> {
    let mut changes: Vec<usize> = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        if !matches!(op, RawOp::Equal(_, _)) {
            changes.push(i);
        }
    }

    if changes.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut group_start = changes[0].saturating_sub(context);
    let mut group_end = (changes[0] + 1 + context).min(ops.len());

    for &ci in &changes[1..] {
        let cs = ci.saturating_sub(context);
        let ce = (ci + 1 + context).min(ops.len());
        if cs <= group_end {
            group_end = ce;
        } else {
            ranges.push((group_start, group_end));
            group_start = cs;
            group_end = ce;
        }
    }
    ranges.push((group_start, group_end));
    ranges
}

fn infer_orig_start(ops: &[RawOp], start: usize, end: usize) -> usize {
    for op in ops.iter().skip(start).take(end - start) {
        match *op {
            RawOp::Equal(oi, _) | RawOp::Remove(oi) => return oi + 1,
            RawOp::Add(_) => {}
        }
    }
    1
}

fn infer_mod_start(ops: &[RawOp], start: usize, end: usize) -> usize {
    for op in ops.iter().skip(start).take(end - start) {
        match *op {
            RawOp::Equal(_, ni) | RawOp::Add(ni) => return ni + 1,
            RawOp::Remove(_) => {}
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_added_lines() {
        let diff = "@@ -0,0 +1,3 @@\n+a\n+b\n+c\n";
        let diffs = parse_unified_diff(diff);
        assert_eq!(diffs.len(), 3);
        assert!(diffs.iter().all(|d| d.kind == LineDiffKind::Added));
        assert_eq!(diffs[0].line_number, 1);
        assert_eq!(diffs[2].line_number, 3);
    }

    #[test]
    fn parse_removed_lines() {
        let diff = "@@ -5,2 +5,0 @@\n-old1\n-old2\n";
        let diffs = parse_unified_diff(diff);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].kind, LineDiffKind::Removed);
        assert_eq!(diffs[0].line_number, 5);
    }

    #[test]
    fn parse_modified_lines() {
        let diff = "@@ -10,2 +10,2 @@\n-old\n-old2\n+new\n+new2\n";
        let diffs = parse_unified_diff(diff);
        assert_eq!(diffs.len(), 2);
        assert!(diffs.iter().all(|d| d.kind == LineDiffKind::Modified));
    }

    #[test]
    fn compute_hunks_added() {
        let hunks = compute_hunks("a\nb\nc\n", "a\nb\nx\nc\n");
        assert!(!hunks.is_empty());
        let has_add = hunks
            .iter()
            .flat_map(|h| &h.lines)
            .any(|l| l.kind == DiffLineKind::Added && l.content == "x");
        assert!(has_add);
    }

    #[test]
    fn compute_hunks_removed() {
        let hunks = compute_hunks("a\nb\nc\n", "a\nc\n");
        assert!(!hunks.is_empty());
        let has_remove = hunks
            .iter()
            .flat_map(|h| &h.lines)
            .any(|l| l.kind == DiffLineKind::Removed && l.content == "b");
        assert!(has_remove);
    }

    #[test]
    fn format_unified_roundtrip() {
        let orig = "a\nb\nc\n";
        let modified = "a\nx\nc\n";
        let hunks = compute_hunks(orig, modified);
        let formatted = format_unified_diff(&hunks, "a.txt", "b.txt");
        assert!(formatted.contains("--- a.txt"));
        assert!(formatted.contains("+++ b.txt"));
        assert!(formatted.contains("@@"));
    }

    #[test]
    fn apply_hunks_produces_modified() {
        let orig = "a\nb\nc";
        let modified = "a\nx\nc";
        let hunks = compute_hunks(orig, modified);
        let result = apply_hunks(orig, &hunks);
        assert_eq!(result.trim(), modified.trim());
    }

    #[test]
    fn revert_hunk_removes_change() {
        let orig = "a\nb\nc";
        let modified = "a\nx\nc";
        let result = revert_hunk(orig, modified, 0);
        assert_eq!(result.trim(), orig.trim());
    }
}
