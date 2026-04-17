//! Side-by-side diff model.
//!
//! Holds an original and modified [`Document`] pair, computes line-level diffs
//! using the Myers algorithm from `sidex_text::diff`, and provides
//! character-level inline diffs for modified lines.

use serde::{Deserialize, Serialize};
use sidex_text::Buffer;

use crate::document::Document;

/// A contiguous range of lines `[start, start + count)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    /// First line index (0-based).
    pub start: usize,
    /// Number of lines in the range.
    pub count: usize,
}

impl LineRange {
    pub fn new(start: usize, count: usize) -> Self {
        Self { start, count }
    }

    /// Exclusive end index.
    pub fn end(&self) -> usize {
        self.start + self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// Classification of a diff change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeKind {
    /// Lines exist only in the modified document.
    Added,
    /// Lines exist only in the original document.
    Deleted,
    /// Lines differ between original and modified.
    Modified,
}

/// A single change block between original and modified documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffChange {
    /// Affected range in the original document.
    pub original_range: LineRange,
    /// Affected range in the modified document.
    pub modified_range: LineRange,
    /// Classification of this change.
    pub kind: ChangeKind,
}

/// The complete result of diffing two documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    /// Ordered list of change blocks.
    pub changes: Vec<DiffChange>,
    /// Number of lines in the original document.
    pub original_line_count: usize,
    /// Number of lines in the modified document.
    pub modified_line_count: usize,
}

impl DiffResult {
    pub fn is_identical(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn change_count(&self) -> usize {
        self.changes.len()
    }
}

/// Side-by-side diff editor holding original and modified documents.
pub struct DiffEditor {
    /// The original (left-hand) document.
    pub original: Document,
    /// The modified (right-hand) document.
    pub modified: Document,
    /// Cached diff result; recomputed on demand.
    diff: Option<DiffResult>,
}

impl DiffEditor {
    pub fn new(original: Document, modified: Document) -> Self {
        Self {
            original,
            modified,
            diff: None,
        }
    }

    /// Recomputes (or returns cached) diff between the two documents.
    pub fn diff(&mut self) -> &DiffResult {
        if self.diff.is_none() {
            let result = compute_diff(&self.original.buffer, &self.modified.buffer);
            self.diff = Some(result);
        }
        self.diff.as_ref().expect("diff was just computed")
    }

    /// Forces recomputation next time `diff()` is called.
    pub fn invalidate(&mut self) {
        self.diff = None;
    }

    /// Convenience accessor returning changes by reference.
    pub fn changes(&mut self) -> &[DiffChange] {
        // Compute if needed, then return slice
        if self.diff.is_none() {
            self.invalidate();
            let _ = self.diff();
        }
        &self.diff.as_ref().expect("diff computed").changes
    }
}

// ── Line-level diff computation ──────────────────────────────────────

/// Computes line-level diff between two buffers, returning a [`DiffResult`].
pub fn compute_diff(original: &Buffer, modified: &Buffer) -> DiffResult {
    let orig_lines = buffer_lines(original);
    let mod_lines = buffer_lines(modified);

    let orig_refs: Vec<&str> = orig_lines.iter().map(String::as_str).collect();
    let mod_refs: Vec<&str> = mod_lines.iter().map(String::as_str).collect();

    let raw = sidex_text::diff::compute_line_diff(&orig_refs, &mod_refs);

    let mut changes = Vec::new();
    let mut orig_idx: usize = 0;
    let mut mod_idx: usize = 0;

    // Walk through the LineDiff entries and coalesce adjacent non-equal runs
    // into DiffChange blocks.
    let mut pending: Option<PendingChange> = None;

    for entry in &raw {
        match entry {
            sidex_text::diff::LineDiff::Equal(_) => {
                if let Some(p) = pending.take() {
                    changes.push(p.into_diff_change());
                }
                orig_idx += 1;
                mod_idx += 1;
            }
            sidex_text::diff::LineDiff::Added(_) => {
                let p = pending.get_or_insert_with(|| PendingChange::new(orig_idx, mod_idx));
                p.modified_count += 1;
                mod_idx += 1;
            }
            sidex_text::diff::LineDiff::Removed(_) => {
                let p = pending.get_or_insert_with(|| PendingChange::new(orig_idx, mod_idx));
                p.original_count += 1;
                orig_idx += 1;
            }
            sidex_text::diff::LineDiff::Modified(_, _) => {
                let p = pending.get_or_insert_with(|| PendingChange::new(orig_idx, mod_idx));
                p.original_count += 1;
                p.modified_count += 1;
                orig_idx += 1;
                mod_idx += 1;
            }
        }
    }

    if let Some(p) = pending.take() {
        changes.push(p.into_diff_change());
    }

    DiffResult {
        changes,
        original_line_count: orig_lines.len(),
        modified_line_count: mod_lines.len(),
    }
}

struct PendingChange {
    orig_start: usize,
    mod_start: usize,
    original_count: usize,
    modified_count: usize,
}

impl PendingChange {
    fn new(orig_start: usize, mod_start: usize) -> Self {
        Self {
            orig_start,
            mod_start,
            original_count: 0,
            modified_count: 0,
        }
    }

    fn into_diff_change(self) -> DiffChange {
        let kind = match (self.original_count, self.modified_count) {
            (0, _) => ChangeKind::Added,
            (_, 0) => ChangeKind::Deleted,
            _ => ChangeKind::Modified,
        };
        DiffChange {
            original_range: LineRange::new(self.orig_start, self.original_count),
            modified_range: LineRange::new(self.mod_start, self.modified_count),
            kind,
        }
    }
}

/// Extract all lines from a buffer (without trailing newline characters).
fn buffer_lines(buf: &Buffer) -> Vec<String> {
    (0..buf.len_lines())
        .map(|i| {
            buf.line_content(i)
                .trim_end_matches(&['\n', '\r'][..])
                .to_string()
        })
        .collect()
}

// ── Inline (character-level) diff ────────────────────────────────────

/// Classification of a character-level diff segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InlineDiffKind {
    Unchanged,
    Added,
    Deleted,
}

/// A segment of an inline diff result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InlineDiffPart {
    /// Byte range `(start, end)` within the respective source string.
    pub range: (usize, usize),
    /// Classification of this segment.
    pub kind: InlineDiffKind,
}

/// Computes character-level diff between two lines.
///
/// Returns parts relative to both the original and modified strings,
/// interleaved: `Deleted` parts refer to the original, `Added` parts
/// refer to the modified string, and `Unchanged` parts refer to both.
pub fn compute_inline_diff(original_line: &str, modified_line: &str) -> Vec<InlineDiffPart> {
    let old_chars: Vec<char> = original_line.chars().collect();
    let new_chars: Vec<char> = modified_line.chars().collect();

    let raw = sidex_text::diff::compute_diff(original_line, modified_line);

    let mut parts = Vec::new();
    let mut orig_char_idx: usize = 0;

    for change in &raw {
        // Unchanged portion before this change
        if change.original_start > orig_char_idx {
            let start_byte = char_offset_to_byte(&old_chars, orig_char_idx);
            let end_byte = char_offset_to_byte(&old_chars, change.original_start);
            parts.push(InlineDiffPart {
                range: (start_byte, end_byte),
                kind: InlineDiffKind::Unchanged,
            });
        }
        orig_char_idx = change.original_start;

        if change.original_length > 0 {
            let start_byte = char_offset_to_byte(&old_chars, orig_char_idx);
            let end_byte = char_offset_to_byte(&old_chars, orig_char_idx + change.original_length);
            parts.push(InlineDiffPart {
                range: (start_byte, end_byte),
                kind: InlineDiffKind::Deleted,
            });
        }

        if change.modified_length > 0 {
            let start_byte = char_offset_to_byte(&new_chars, change.modified_start);
            let end_byte =
                char_offset_to_byte(&new_chars, change.modified_start + change.modified_length);
            parts.push(InlineDiffPart {
                range: (start_byte, end_byte),
                kind: InlineDiffKind::Added,
            });
        }

        orig_char_idx += change.original_length;
    }

    // Trailing unchanged portion
    if orig_char_idx < old_chars.len() {
        let start_byte = char_offset_to_byte(&old_chars, orig_char_idx);
        let end_byte = char_offset_to_byte(&old_chars, old_chars.len());
        parts.push(InlineDiffPart {
            range: (start_byte, end_byte),
            kind: InlineDiffKind::Unchanged,
        });
    }

    parts
}

/// Convert a char index to a byte offset in the original string.
fn char_offset_to_byte(chars: &[char], char_idx: usize) -> usize {
    chars[..char_idx].iter().map(|c| c.len_utf8()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_identical_buffers() {
        let buf = Buffer::from_str("hello\nworld\n");
        let result = compute_diff(&buf, &buf);
        assert!(result.is_identical());
    }

    #[test]
    fn diff_added_lines() {
        let orig = Buffer::from_str("aaa\nccc\n");
        let modi = Buffer::from_str("aaa\nbbb\nccc\n");
        let result = compute_diff(&orig, &modi);
        assert_eq!(result.change_count(), 1);
        let c = &result.changes[0];
        assert_eq!(c.kind, ChangeKind::Added);
        assert_eq!(c.modified_range.count, 1);
        assert_eq!(c.original_range.count, 0);
    }

    #[test]
    fn diff_deleted_lines() {
        let orig = Buffer::from_str("aaa\nbbb\nccc\n");
        let modi = Buffer::from_str("aaa\nccc\n");
        let result = compute_diff(&orig, &modi);
        assert_eq!(result.change_count(), 1);
        let c = &result.changes[0];
        assert_eq!(c.kind, ChangeKind::Deleted);
        assert_eq!(c.original_range.count, 1);
        assert_eq!(c.modified_range.count, 0);
    }

    #[test]
    fn diff_modified_lines() {
        let orig = Buffer::from_str("aaa\nbbb\nccc\n");
        let modi = Buffer::from_str("aaa\nBBB\nccc\n");
        let result = compute_diff(&orig, &modi);
        assert_eq!(result.change_count(), 1);
        let c = &result.changes[0];
        assert_eq!(c.kind, ChangeKind::Modified);
    }

    #[test]
    fn diff_empty_buffers() {
        let orig = Buffer::from_str("");
        let modi = Buffer::from_str("");
        let result = compute_diff(&orig, &modi);
        assert!(result.is_identical());
    }

    #[test]
    fn diff_one_empty() {
        let orig = Buffer::from_str("");
        let modi = Buffer::from_str("hello\n");
        let result = compute_diff(&orig, &modi);
        assert!(!result.is_identical());
    }

    #[test]
    fn inline_diff_identical() {
        let parts = compute_inline_diff("hello world", "hello world");
        assert!(parts.iter().all(|p| p.kind == InlineDiffKind::Unchanged));
    }

    #[test]
    fn inline_diff_insertion() {
        let parts = compute_inline_diff("ac", "abc");
        assert!(parts.iter().any(|p| p.kind == InlineDiffKind::Added));
    }

    #[test]
    fn inline_diff_deletion() {
        let parts = compute_inline_diff("abc", "ac");
        assert!(parts.iter().any(|p| p.kind == InlineDiffKind::Deleted));
    }

    #[test]
    fn inline_diff_replacement() {
        let parts = compute_inline_diff("hello", "hullo");
        let has_deleted = parts.iter().any(|p| p.kind == InlineDiffKind::Deleted);
        let has_added = parts.iter().any(|p| p.kind == InlineDiffKind::Added);
        assert!(has_deleted);
        assert!(has_added);
    }

    #[test]
    fn inline_diff_empty_original() {
        let parts = compute_inline_diff("", "hello");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].kind, InlineDiffKind::Added);
    }

    #[test]
    fn inline_diff_empty_modified() {
        let parts = compute_inline_diff("hello", "");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].kind, InlineDiffKind::Deleted);
    }

    #[test]
    fn line_range_basics() {
        let lr = LineRange::new(5, 3);
        assert_eq!(lr.start, 5);
        assert_eq!(lr.count, 3);
        assert_eq!(lr.end(), 8);
        assert!(!lr.is_empty());

        let empty = LineRange::new(0, 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn diff_editor_caches_and_invalidates() {
        let orig = Document::from_str("aaa\nbbb\n");
        let modi = Document::from_str("aaa\nccc\n");
        let mut editor = DiffEditor::new(orig, modi);

        let result = editor.diff();
        assert_eq!(result.change_count(), 1);

        editor.invalidate();
        let result2 = editor.diff();
        assert_eq!(result2.change_count(), 1);
    }
}
