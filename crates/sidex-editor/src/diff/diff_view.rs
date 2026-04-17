//! Diff view state — scroll synchronization, change navigation, and
//! accept/revert operations.
//!
//! This module manages the *presentation* state layered on top of the
//! diff model. It tracks the current view mode (side-by-side vs. inline),
//! synchronizes scroll positions between the two editor panes, and provides
//! navigation through change hunks. Also provides character-level diff
//! highlighting, gutter decorations, hunk staging, and inline view rendering.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sidex_text::Buffer;

use super::diff_model::{
    compute_diff, compute_inline_diff, ChangeKind, DiffChange, DiffResult, InlineDiffKind,
    InlineDiffPart, LineRange,
};
use crate::document::Document;

/// How the diff is presented to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffViewMode {
    /// Two editors side by side (VS Code default).
    SideBySide,
    /// Interleaved additions/deletions in a single editor.
    Inline,
}

/// Metadata for one side of the diff editor.
#[derive(Debug, Clone)]
pub struct DiffEditorSide {
    pub content: String,
    pub path: Option<PathBuf>,
    pub language: String,
    pub scroll_top: f32,
}

impl DiffEditorSide {
    pub fn new(content: String) -> Self {
        Self {
            content,
            path: None,
            language: String::new(),
            scroll_top: 0.0,
        }
    }

    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = lang.into();
        self
    }
}

/// A character-level diff segment within a single line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharDiffPart {
    pub kind: CharDiffKind,
    pub text: String,
}

/// Classification for character-level diffs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharDiffKind {
    Unchanged,
    Added,
    Removed,
}

/// Gutter decoration for a diff line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffGutterMark {
    Added,
    Removed,
    Modified,
    Empty,
}

/// A single line in the inline diff view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineDiffLine {
    pub line_number_original: Option<u32>,
    pub line_number_modified: Option<u32>,
    pub content: String,
    pub kind: InlineDiffLineKind,
}

/// Classification for inline diff view lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InlineDiffLineKind {
    Unchanged,
    Added,
    Removed,
    Modified,
}

/// Action a user can take on a diff hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffHunkAction {
    RevertToOriginal,
    AcceptModified,
    StageHunk,
    UnstageHunk,
    CopyToLeft,
    CopyToRight,
}

/// Mutable view-layer state for a diff editor session.
pub struct DiffViewState {
    /// The original (left) document.
    pub original: Document,
    /// The modified (right) document.
    pub modified: Document,
    /// Side metadata for the original pane.
    pub original_side: DiffEditorSide,
    /// Side metadata for the modified pane.
    pub modified_side: DiffEditorSide,
    /// Cached diff between the two documents.
    diff: DiffResult,
    /// Current display mode.
    pub mode: DiffViewMode,
    /// Index into `diff.changes` that is currently focused (-1 = none).
    current_change: Option<usize>,
    /// Scroll position of the original pane (in logical pixels).
    pub original_scroll_top: f64,
    /// Scroll position of the modified pane (in logical pixels).
    pub modified_scroll_top: f64,
    /// Whether to ignore whitespace when diffing.
    pub ignore_whitespace: bool,
    /// Whether scroll is synchronized between panes.
    pub scroll_sync: bool,
    /// Whether gutter indicators (+/-) are rendered.
    pub render_indicators: bool,
    /// Whether the original side is editable.
    pub original_editable: bool,
    /// Staged hunk indices (for git integration).
    staged_hunks: Vec<usize>,
}

impl DiffViewState {
    pub fn new(original: Document, modified: Document) -> Self {
        let orig_text = original.text();
        let mod_text = modified.text();
        let diff = compute_diff(&original.buffer, &modified.buffer);
        Self {
            original,
            modified,
            original_side: DiffEditorSide::new(orig_text),
            modified_side: DiffEditorSide::new(mod_text),
            diff,
            mode: DiffViewMode::SideBySide,
            current_change: None,
            original_scroll_top: 0.0,
            modified_scroll_top: 0.0,
            ignore_whitespace: false,
            scroll_sync: true,
            render_indicators: true,
            original_editable: false,
            staged_hunks: Vec::new(),
        }
    }

    /// Re-diff after document edits.
    pub fn recompute_diff(&mut self) {
        self.diff = compute_diff(&self.original.buffer, &self.modified.buffer);
        // Clamp current_change to the new range.
        if let Some(idx) = self.current_change {
            if idx >= self.diff.changes.len() {
                self.current_change = if self.diff.changes.is_empty() {
                    None
                } else {
                    Some(self.diff.changes.len() - 1)
                };
            }
        }
    }

    pub fn diff_result(&self) -> &DiffResult {
        &self.diff
    }

    pub fn changes(&self) -> &[DiffChange] {
        &self.diff.changes
    }

    /// Synchronize the scroll position of both panes.
    ///
    /// When one pane scrolls, we adjust the other to keep corresponding
    /// unchanged regions aligned. Insertions/deletions cause the scroll
    /// mapping to diverge, so we compute a piecewise-linear mapping
    /// from original line space to modified line space.
    pub fn synchronized_scroll(&mut self, scroll_top: f64, line_height: f64) {
        if line_height <= 0.0 {
            return;
        }

        let source_line = (scroll_top / line_height).floor();
        let target_line = self.map_original_line_to_modified(source_line as usize);

        self.original_scroll_top = scroll_top;
        #[allow(clippy::cast_precision_loss)]
        {
            self.modified_scroll_top = target_line as f64 * line_height;
        }
    }

    /// Map an original-side line index to the corresponding modified-side line.
    fn map_original_line_to_modified(&self, orig_line: usize) -> usize {
        let mut orig_offset: isize = 0;
        let mut mod_offset: isize = 0;

        for change in &self.diff.changes {
            if change.original_range.start > orig_line {
                break;
            }
            if orig_line < change.original_range.end() {
                // Inside a change — map to the start of the modified range.
                let into = orig_line - change.original_range.start;
                let clamped = into.min(change.modified_range.count.saturating_sub(1));
                return (change.modified_range.start + clamped).max(0);
            }
            orig_offset += change.original_range.count as isize;
            mod_offset += change.modified_range.count as isize;
        }

        let delta = mod_offset - orig_offset;
        #[allow(clippy::cast_sign_loss)]
        let mapped = (orig_line as isize + delta).max(0) as usize;
        mapped
    }

    // ── Change navigation ────────────────────────────────────────

    /// Jump to the next change after the current one.
    pub fn navigate_next_change(&mut self) {
        if self.diff.changes.is_empty() {
            return;
        }
        self.current_change = Some(match self.current_change {
            None => 0,
            Some(idx) => {
                if idx + 1 < self.diff.changes.len() {
                    idx + 1
                } else {
                    0 // wrap around
                }
            }
        });
    }

    /// Jump to the previous change before the current one.
    pub fn navigate_prev_change(&mut self) {
        if self.diff.changes.is_empty() {
            return;
        }
        self.current_change = Some(match self.current_change {
            None | Some(0) => self.diff.changes.len() - 1,
            Some(idx) => idx - 1,
        });
    }

    /// Returns the currently focused change, if any.
    pub fn active_change(&self) -> Option<&DiffChange> {
        self.current_change
            .and_then(|idx| self.diff.changes.get(idx))
    }

    /// Returns the index of the currently focused change.
    pub fn active_change_index(&self) -> Option<usize> {
        self.current_change
    }

    // ── Accept / revert operations ───────────────────────────────

    /// Accept a change from the modified document into the original.
    ///
    /// Replaces the original-side lines with the modified-side lines
    /// for the change at `change_idx`.
    pub fn accept_change(&mut self, change_idx: usize) {
        if change_idx >= self.diff.changes.len() {
            return;
        }
        let change = self.diff.changes[change_idx].clone();
        let replacement = extract_lines(&self.modified.buffer, &change.modified_range);
        replace_lines(&mut self.original, &change.original_range, &replacement);
        self.recompute_diff();
    }

    /// Revert a change by replacing the modified-side lines with the original.
    pub fn revert_change(&mut self, change_idx: usize) {
        if change_idx >= self.diff.changes.len() {
            return;
        }
        let change = self.diff.changes[change_idx].clone();
        let original_text = extract_lines(&self.original.buffer, &change.original_range);
        replace_lines(&mut self.modified, &change.modified_range, &original_text);
        self.recompute_diff();
    }

    // ── View mode ─────────────────────────────────────────────────

    /// Toggle between side-by-side and inline view.
    pub fn toggle_view_mode(&mut self) {
        self.mode = match self.mode {
            DiffViewMode::SideBySide => DiffViewMode::Inline,
            DiffViewMode::Inline => DiffViewMode::SideBySide,
        };
    }

    /// Set whitespace handling and recompute diff.
    pub fn set_ignore_whitespace(&mut self, ignore: bool) {
        if self.ignore_whitespace != ignore {
            self.ignore_whitespace = ignore;
            self.recompute_diff();
        }
    }

    /// Whether the two documents are identical.
    pub fn is_identical(&self) -> bool {
        self.diff.is_identical()
    }

    /// Status label e.g. "3 changes" or "No changes".
    pub fn status_label(&self) -> String {
        let n = self.diff.change_count();
        if n == 0 {
            "No changes".to_string()
        } else if n == 1 {
            "1 change".to_string()
        } else {
            format!("{n} changes")
        }
    }

    // ── Character-level diff ──────────────────────────────────────

    /// Compute character-level diff for a modified-kind change, returning
    /// parts for the original line and the modified line.
    pub fn compute_char_diff(original_line: &str, modified_line: &str) -> Vec<CharDiffPart> {
        let parts = compute_inline_diff(original_line, modified_line);
        let mut result = Vec::new();

        for part in &parts {
            let source = match part.kind {
                InlineDiffKind::Unchanged | InlineDiffKind::Deleted => original_line,
                InlineDiffKind::Added => modified_line,
            };
            let text = if part.range.1 <= source.len() {
                &source[part.range.0..part.range.1]
            } else {
                ""
            };
            let kind = match part.kind {
                InlineDiffKind::Unchanged => CharDiffKind::Unchanged,
                InlineDiffKind::Added => CharDiffKind::Added,
                InlineDiffKind::Deleted => CharDiffKind::Removed,
            };
            result.push(CharDiffPart {
                kind,
                text: text.to_string(),
            });
        }
        result
    }

    // ── Gutter decorations ────────────────────────────────────────

    /// Compute gutter marks for the original side.
    pub fn original_gutter_marks(&self) -> Vec<(usize, DiffGutterMark)> {
        let mut marks = Vec::new();
        for change in &self.diff.changes {
            match change.kind {
                ChangeKind::Deleted => {
                    for line in change.original_range.start..change.original_range.end() {
                        marks.push((line, DiffGutterMark::Removed));
                    }
                }
                ChangeKind::Modified => {
                    for line in change.original_range.start..change.original_range.end() {
                        marks.push((line, DiffGutterMark::Modified));
                    }
                }
                ChangeKind::Added => {
                    marks.push((
                        change.original_range.start,
                        DiffGutterMark::Empty,
                    ));
                }
            }
        }
        marks
    }

    /// Compute gutter marks for the modified side.
    pub fn modified_gutter_marks(&self) -> Vec<(usize, DiffGutterMark)> {
        let mut marks = Vec::new();
        for change in &self.diff.changes {
            match change.kind {
                ChangeKind::Added => {
                    for line in change.modified_range.start..change.modified_range.end() {
                        marks.push((line, DiffGutterMark::Added));
                    }
                }
                ChangeKind::Modified => {
                    for line in change.modified_range.start..change.modified_range.end() {
                        marks.push((line, DiffGutterMark::Modified));
                    }
                }
                ChangeKind::Deleted => {
                    marks.push((
                        change.modified_range.start,
                        DiffGutterMark::Empty,
                    ));
                }
            }
        }
        marks
    }

    // ── Inline view rendering ─────────────────────────────────────

    /// Build the interleaved line list for inline diff mode.
    #[allow(clippy::cast_possible_truncation)]
    pub fn build_inline_lines(&self) -> Vec<InlineDiffLine> {
        let orig_lines = buffer_lines_vec(&self.original.buffer);
        let mod_lines = buffer_lines_vec(&self.modified.buffer);
        let mut result = Vec::new();
        let mut orig_idx: usize = 0;
        let mut mod_idx: usize = 0;

        for change in &self.diff.changes {
            while orig_idx < change.original_range.start && mod_idx < change.modified_range.start {
                let content = if orig_idx < orig_lines.len() {
                    orig_lines[orig_idx].clone()
                } else {
                    String::new()
                };
                result.push(InlineDiffLine {
                    line_number_original: Some((orig_idx + 1) as u32),
                    line_number_modified: Some((mod_idx + 1) as u32),
                    content,
                    kind: InlineDiffLineKind::Unchanged,
                });
                orig_idx += 1;
                mod_idx += 1;
            }

            for i in change.original_range.start..change.original_range.end() {
                let content = if i < orig_lines.len() {
                    orig_lines[i].clone()
                } else {
                    String::new()
                };
                result.push(InlineDiffLine {
                    line_number_original: Some((i + 1) as u32),
                    line_number_modified: None,
                    content,
                    kind: InlineDiffLineKind::Removed,
                });
            }
            for i in change.modified_range.start..change.modified_range.end() {
                let content = if i < mod_lines.len() {
                    mod_lines[i].clone()
                } else {
                    String::new()
                };
                result.push(InlineDiffLine {
                    line_number_original: None,
                    line_number_modified: Some((i + 1) as u32),
                    content,
                    kind: InlineDiffLineKind::Added,
                });
            }

            orig_idx = change.original_range.end();
            mod_idx = change.modified_range.end();
        }

        while orig_idx < orig_lines.len() || mod_idx < mod_lines.len() {
            let content = if orig_idx < orig_lines.len() {
                orig_lines[orig_idx].clone()
            } else if mod_idx < mod_lines.len() {
                mod_lines[mod_idx].clone()
            } else {
                break;
            };
            result.push(InlineDiffLine {
                line_number_original: if orig_idx < orig_lines.len() {
                    Some((orig_idx + 1) as u32)
                } else {
                    None
                },
                line_number_modified: if mod_idx < mod_lines.len() {
                    Some((mod_idx + 1) as u32)
                } else {
                    None
                },
                content,
                kind: InlineDiffLineKind::Unchanged,
            });
            orig_idx += 1;
            mod_idx += 1;
        }
        result
    }

    // ── Hunk staging (git integration) ────────────────────────────

    /// Mark a hunk as staged.
    pub fn stage_hunk(&mut self, change_idx: usize) {
        if change_idx < self.diff.changes.len() && !self.staged_hunks.contains(&change_idx) {
            self.staged_hunks.push(change_idx);
            self.staged_hunks.sort_unstable();
        }
    }

    /// Unstage a previously staged hunk.
    pub fn unstage_hunk(&mut self, change_idx: usize) {
        self.staged_hunks.retain(|&i| i != change_idx);
    }

    /// Whether a hunk is staged.
    pub fn is_hunk_staged(&self, change_idx: usize) -> bool {
        self.staged_hunks.contains(&change_idx)
    }

    /// Returns indices of all staged hunks.
    pub fn staged_hunk_indices(&self) -> &[usize] {
        &self.staged_hunks
    }

    // ── Copy operations ───────────────────────────────────────────

    /// Copy original-side lines for a change to the modified side.
    pub fn copy_to_modified(&mut self, change_idx: usize) {
        self.revert_change(change_idx);
    }

    /// Copy modified-side lines for a change to the original side.
    pub fn copy_to_original(&mut self, change_idx: usize) {
        self.accept_change(change_idx);
    }

    // ── Navigation: line number for a change ──────────────────────

    /// Returns the original-side line number for the given change.
    pub fn change_original_line(&self, change_idx: usize) -> Option<usize> {
        self.diff
            .changes
            .get(change_idx)
            .map(|c| c.original_range.start)
    }

    /// Returns the modified-side line number for the given change.
    pub fn change_modified_line(&self, change_idx: usize) -> Option<usize> {
        self.diff
            .changes
            .get(change_idx)
            .map(|c| c.modified_range.start)
    }

    /// Find the change index containing or nearest to the given original line.
    pub fn change_at_original_line(&self, line: usize) -> Option<usize> {
        self.diff.changes.iter().position(|c| {
            line >= c.original_range.start && line < c.original_range.end()
        }).or_else(|| {
            self.diff.changes.iter().position(|c| {
                c.original_range.start >= line
            })
        })
    }
}

/// Extract text for a line range from a buffer.
fn extract_lines(buf: &Buffer, range: &LineRange) -> String {
    if range.is_empty() {
        return String::new();
    }
    let mut result = String::new();
    for i in range.start..range.end() {
        if i < buf.len_lines() {
            let line: std::borrow::Cow<'_, str> = buf.line(i);
            result.push_str(&line);
        }
    }
    result
}

/// Replace lines `[range.start, range.end())` in a document with `replacement`.
fn replace_lines(doc: &mut Document, range: &LineRange, replacement: &str) {
    let start_offset = if range.start < doc.buffer.len_lines() {
        doc.buffer.line_to_char(range.start)
    } else {
        doc.buffer.len_chars()
    };

    let end_offset = if range.end() < doc.buffer.len_lines() {
        doc.buffer.line_to_char(range.end())
    } else {
        doc.buffer.len_chars()
    };

    doc.buffer.replace(start_offset..end_offset, replacement);
}

/// Extract all lines from a buffer as owned strings.
fn buffer_lines_vec(buf: &Buffer) -> Vec<String> {
    (0..buf.len_lines())
        .map(|i| {
            buf.line_content(i)
                .trim_end_matches(&['\n', '\r'][..])
                .to_string()
        })
        .collect()
}

/// Compute a diff result directly from two strings (convenience for callers
/// that don't have `Buffer` instances yet).
pub fn compute_diff_from_strings(original: &str, modified: &str, _ignore_whitespace: bool) -> DiffResult {
    let orig_buf = Buffer::from_str(original);
    let mod_buf = Buffer::from_str(modified);
    compute_diff(&orig_buf, &mod_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(orig: &str, modi: &str) -> DiffViewState {
        DiffViewState::new(Document::from_str(orig), Document::from_str(modi))
    }

    #[test]
    fn initial_state() {
        let state = make_state("aaa\nbbb\n", "aaa\nccc\n");
        assert_eq!(state.mode, DiffViewMode::SideBySide);
        assert_eq!(state.changes().len(), 1);
    }

    #[test]
    fn navigate_changes() {
        let mut state = make_state("aaa\nbbb\nccc\n", "aaa\nXXX\nccc\nDDD\n");
        let n = state.changes().len();
        assert!(n >= 1);

        assert!(state.active_change().is_none());
        state.navigate_next_change();
        assert_eq!(state.active_change_index(), Some(0));
        state.navigate_next_change();
        if n > 1 {
            assert_eq!(state.active_change_index(), Some(1));
        }
    }

    #[test]
    fn navigate_wraps_around() {
        let mut state = make_state("aaa\n", "bbb\n");
        state.navigate_next_change();
        assert_eq!(state.active_change_index(), Some(0));
        // Next wraps to 0
        state.navigate_next_change();
        assert_eq!(state.active_change_index(), Some(0));
    }

    #[test]
    fn navigate_prev_from_none() {
        let mut state = make_state("a\n", "b\n");
        state.navigate_prev_change();
        assert_eq!(state.active_change_index(), Some(state.changes().len() - 1));
    }

    #[test]
    fn accept_change_applies() {
        let mut state = make_state("aaa\nbbb\nccc\n", "aaa\nXXX\nccc\n");
        assert_eq!(state.changes().len(), 1);
        state.accept_change(0);
        // After accepting, the original should match the modified
        assert!(state.diff_result().is_identical());
    }

    #[test]
    fn revert_change_applies() {
        let mut state = make_state("aaa\nbbb\nccc\n", "aaa\nXXX\nccc\n");
        state.revert_change(0);
        assert!(state.diff_result().is_identical());
    }

    #[test]
    fn synchronized_scroll_trivial() {
        let mut state = make_state("aaa\nbbb\nccc\n", "aaa\nbbb\nccc\n");
        state.synchronized_scroll(20.0, 20.0);
        assert!((state.modified_scroll_top - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn toggle_view_mode() {
        let mut state = make_state("a\n", "b\n");
        assert_eq!(state.mode, DiffViewMode::SideBySide);
        state.toggle_view_mode();
        assert_eq!(state.mode, DiffViewMode::Inline);
        state.toggle_view_mode();
        assert_eq!(state.mode, DiffViewMode::SideBySide);
    }

    #[test]
    fn status_label_no_changes() {
        let state = make_state("aaa\n", "aaa\n");
        assert_eq!(state.status_label(), "No changes");
    }

    #[test]
    fn status_label_with_changes() {
        let state = make_state("aaa\nbbb\n", "aaa\nccc\n");
        assert_eq!(state.status_label(), "1 change");
    }

    #[test]
    fn char_diff_basic() {
        let parts = DiffViewState::compute_char_diff("hello", "hullo");
        assert!(parts.iter().any(|p| p.kind == CharDiffKind::Removed));
        assert!(parts.iter().any(|p| p.kind == CharDiffKind::Added));
    }

    #[test]
    fn gutter_marks_added() {
        let state = make_state("aaa\nccc\n", "aaa\nbbb\nccc\n");
        let marks = state.modified_gutter_marks();
        assert!(marks.iter().any(|(_, m)| *m == DiffGutterMark::Added));
    }

    #[test]
    fn gutter_marks_removed() {
        let state = make_state("aaa\nbbb\nccc\n", "aaa\nccc\n");
        let marks = state.original_gutter_marks();
        assert!(marks.iter().any(|(_, m)| *m == DiffGutterMark::Removed));
    }

    #[test]
    fn inline_lines_basic() {
        let state = make_state("aaa\nbbb\nccc\n", "aaa\nXXX\nccc\n");
        let lines = state.build_inline_lines();
        assert!(lines.iter().any(|l| l.kind == InlineDiffLineKind::Removed));
        assert!(lines.iter().any(|l| l.kind == InlineDiffLineKind::Added));
    }

    #[test]
    fn stage_hunk() {
        let mut state = make_state("aaa\nbbb\n", "aaa\nccc\n");
        assert!(!state.is_hunk_staged(0));
        state.stage_hunk(0);
        assert!(state.is_hunk_staged(0));
        state.unstage_hunk(0);
        assert!(!state.is_hunk_staged(0));
    }

    #[test]
    fn is_identical() {
        let state = make_state("aaa\n", "aaa\n");
        assert!(state.is_identical());
        let state2 = make_state("aaa\n", "bbb\n");
        assert!(!state2.is_identical());
    }

    #[test]
    fn compute_diff_from_strings_works() {
        let result = compute_diff_from_strings("aaa\nbbb\n", "aaa\nccc\n", false);
        assert_eq!(result.change_count(), 1);
    }
}
