//! High-level editing controller that composes [`Document`] operations into a
//! single facade with full VS Code parity for every editing command.
//!
//! This module re-exports and adds operations that weren't directly on
//! `Document`, including `remove_duplicate_lines`, `add_cursors_to_line_ends`,
//! and convenience wrappers that pair with the extended [`CursorController`].

use sidex_text::{Buffer, Position};

use crate::document::Document;
use crate::multi_cursor::MultiCursor;
use crate::selection::Selection;
use crate::undo::EditGroup;

/// Aggregated editing controller that wraps a [`Document`] and provides the
/// full set of editing commands with multi-cursor support.
///
/// Thin wrapper — delegates to `Document` for operations that already exist
/// there, and adds the remaining ones.
#[derive(Debug, Clone)]
pub struct EditController {
    pub document: Document,
}

impl EditController {
    /// Creates an `EditController` wrapping a new empty document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            document: Document::new(),
        }
    }

    /// Creates an `EditController` from existing text.
    #[must_use]
    pub fn from_str(text: &str) -> Self {
        Self {
            document: Document::from_str(text),
        }
    }

    /// Creates an `EditController` wrapping an existing document.
    #[must_use]
    pub fn from_document(doc: Document) -> Self {
        Self { document: doc }
    }

    // ── Delegated text accessors ──────────────────────────────────

    #[must_use]
    pub fn text(&self) -> String {
        self.document.text()
    }

    #[must_use]
    pub fn buffer(&self) -> &Buffer {
        &self.document.buffer
    }

    // ── Basic editing ─────────────────────────────────────────────

    /// Inserts text at all cursor positions (multi-cursor aware).
    pub fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.document.type_char(ch);
        }
    }

    /// Deletes one character to the left at all cursors (Backspace).
    pub fn delete_left(&mut self) {
        self.document.delete_left();
    }

    /// Deletes one character to the right at all cursors (Delete).
    pub fn delete_right(&mut self) {
        self.document.delete_right();
    }

    /// Deletes one word to the left at all cursors (Ctrl+Backspace).
    pub fn delete_word_left(&mut self) {
        self.document.delete_word_left();
    }

    /// Deletes one word to the right at all cursors (Ctrl+Delete).
    pub fn delete_word_right(&mut self) {
        self.document.delete_word_right();
    }

    /// Deletes the entire line at each cursor (Ctrl+Shift+K).
    pub fn delete_line(&mut self) {
        self.document.delete_line();
    }

    /// Deletes from cursor to line start.
    pub fn delete_all_left(&mut self) {
        self.document.delete_all_left();
    }

    /// Deletes from cursor to line end.
    pub fn delete_all_right(&mut self) {
        self.document.delete_all_right();
    }

    // ── Cut line (entire line if no selection) ────────────────────

    /// Cuts the entire line when there is no selection. Returns the cut text
    /// (suitable for placing on the clipboard).
    pub fn cut_line(&mut self) -> String {
        let primary = self.document.cursors.primary().selection;
        if primary.is_empty() {
            let line = primary.active.line as usize;
            let content = self.document.buffer.line_content(line);
            let result = format!("{content}\n");
            self.document.delete_line();
            result
        } else {
            let s = self
                .document
                .buffer
                .position_to_offset(primary.start());
            let e = self
                .document
                .buffer
                .position_to_offset(primary.end());
            let text = self.document.buffer.slice(s..e);
            self.document.delete_right();
            text
        }
    }

    // ── Line duplication / movement ───────────────────────────────

    /// Copies the current line up (Shift+Alt+Up).
    pub fn copy_line_up(&mut self) {
        self.document.copy_line_up();
    }

    /// Copies the current line down (Shift+Alt+Down).
    pub fn copy_line_down(&mut self) {
        self.document.copy_line_down();
    }

    /// Moves the current line up (Alt+Up).
    pub fn move_line_up(&mut self) {
        self.document.move_line_up();
    }

    /// Moves the current line down (Alt+Down).
    pub fn move_line_down(&mut self) {
        self.document.move_line_down();
    }

    // ── Indentation ───────────────────────────────────────────────

    /// Indents the current line(s) (Tab or Ctrl+]).
    pub fn indent_line(&mut self) {
        self.document.indent();
    }

    /// Outdents the current line(s) (Shift+Tab or Ctrl+[).
    pub fn outdent_line(&mut self) {
        self.document.outdent();
    }

    // ── Line insertion ────────────────────────────────────────────

    /// Inserts a blank line above the cursor (Ctrl+Shift+Enter).
    pub fn insert_line_above(&mut self) {
        self.document.insert_line_above();
    }

    /// Inserts a blank line below the cursor (Ctrl+Enter).
    pub fn insert_line_below(&mut self) {
        self.document.insert_line_below();
    }

    // ── Comments ──────────────────────────────────────────────────

    /// Toggles a line comment (Ctrl+/).
    pub fn toggle_comment(&mut self, prefix: &str) {
        self.document.toggle_line_comment(prefix);
    }

    /// Toggles a block comment (Shift+Alt+A).
    pub fn toggle_block_comment(&mut self, open: &str, close: &str) {
        self.document.toggle_block_comment(open, close);
    }

    // ── Join lines ────────────────────────────────────────────────

    /// Joins selected lines into one.
    pub fn join_lines(&mut self) {
        self.document.join_lines();
    }

    // ── Transpose ─────────────────────────────────────────────────

    /// Swaps the two characters surrounding the cursor.
    pub fn transpose_characters(&mut self) {
        self.document.transpose_characters();
    }

    // ── Case transforms ───────────────────────────────────────────

    /// Transforms selected text (or word at cursor) to UPPERCASE.
    pub fn transform_to_uppercase(&mut self) {
        self.document.transform_to_uppercase();
    }

    /// Transforms selected text (or word at cursor) to lowercase.
    pub fn transform_to_lowercase(&mut self) {
        self.document.transform_to_lowercase();
    }

    /// Transforms selected text (or word at cursor) to Title Case.
    pub fn transform_to_title_case(&mut self) {
        self.document.transform_to_title_case();
    }

    // ── Sort lines ────────────────────────────────────────────────

    /// Sorts all lines in ascending order.
    pub fn sort_lines_ascending(&mut self) {
        self.document.sort_lines_ascending();
    }

    /// Sorts all lines in descending order.
    pub fn sort_lines_descending(&mut self) {
        self.document.sort_lines_descending();
    }

    // ── Remove duplicate lines ────────────────────────────────────

    /// Removes consecutive duplicate lines from the document.
    pub fn remove_duplicate_lines(&mut self) {
        let before = cursor_selections(&self.document);
        let text = self.document.buffer.text();
        let lines: Vec<&str> = text.split('\n').collect();
        let mut deduped: Vec<&str> = Vec::with_capacity(lines.len());
        let mut prev: Option<&str> = None;
        for line in &lines {
            if prev != Some(line) {
                deduped.push(line);
            }
            prev = Some(line);
        }
        let new_text = deduped.join("\n");
        let len = self.document.buffer.len_chars();
        if len > 0 {
            self.document.buffer.remove(0..len);
        }
        self.document.buffer.insert(0, &new_text);
        let after = cursor_selections(&self.document);
        self.document
            .undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.document.version += 1;
        self.document.is_modified = true;
    }

    // ── Trim trailing whitespace ──────────────────────────────────

    /// Trims trailing whitespace from all lines.
    pub fn trim_trailing_whitespace(&mut self) {
        self.document.trim_trailing_whitespace();
    }

    // ── Add cursors to line ends ──────────────────────────────────

    /// Adds a cursor at the end of each selected line. Useful after
    /// selecting a block of text to edit all line endings at once.
    pub fn add_cursors_to_line_ends(&mut self) {
        let primary = self.document.cursors.primary().selection;
        if primary.is_empty() {
            return;
        }
        let start_line = primary.start().line;
        let end_line = primary.end().line;

        let first_end = self.document.buffer.line_content_len(start_line as usize) as u32;
        self.document.cursors =
            MultiCursor::new(Position::new(start_line, first_end));

        for line in (start_line + 1)..=end_line {
            let end_col = self.document.buffer.line_content_len(line as usize) as u32;
            self.document
                .cursors
                .add_cursor(Position::new(line, end_col));
        }
    }

    // ── Undo / redo ───────────────────────────────────────────────

    /// Undoes the last edit group (Ctrl+Z).
    pub fn undo(&mut self) {
        self.document.undo();
    }

    /// Redoes the last undone edit group (Ctrl+Shift+Z / Ctrl+Y).
    pub fn redo(&mut self) {
        self.document.redo();
    }
}

impl Default for EditController {
    fn default() -> Self {
        Self::new()
    }
}

fn cursor_selections(doc: &Document) -> Vec<Selection> {
    doc.cursors.cursors().iter().map(|c| c.selection).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_text_basic() {
        let mut ec = EditController::from_str("hello");
        ec.document.cursors = MultiCursor::new(Position::new(0, 5));
        ec.type_text(" world");
        assert_eq!(ec.text(), "hello world");
    }

    #[test]
    fn delete_left_right() {
        let mut ec = EditController::from_str("abcde");
        ec.document.cursors = MultiCursor::new(Position::new(0, 3));
        ec.delete_left();
        assert_eq!(ec.text(), "abde");

        ec.document.cursors = MultiCursor::new(Position::new(0, 2));
        ec.delete_right();
        assert_eq!(ec.text(), "abe");
    }

    #[test]
    fn delete_word_left_right() {
        let mut ec = EditController::from_str("hello world");
        ec.document.cursors = MultiCursor::new(Position::new(0, 5));
        ec.delete_word_left();
        assert_eq!(ec.text(), " world");

        let mut ec2 = EditController::from_str("hello world");
        ec2.document.cursors = MultiCursor::new(Position::new(0, 6));
        ec2.delete_word_right();
        assert_eq!(ec2.text(), "hello ");
    }

    #[test]
    fn delete_line() {
        let mut ec = EditController::from_str("aaa\nbbb\nccc");
        ec.document.cursors = MultiCursor::new(Position::new(1, 0));
        ec.delete_line();
        assert_eq!(ec.text(), "aaa\nccc");
    }

    #[test]
    fn delete_all_left_right() {
        let mut ec = EditController::from_str("hello world");
        ec.document.cursors = MultiCursor::new(Position::new(0, 5));
        ec.delete_all_left();
        assert_eq!(ec.text(), " world");

        let mut ec2 = EditController::from_str("hello world");
        ec2.document.cursors = MultiCursor::new(Position::new(0, 5));
        ec2.delete_all_right();
        assert_eq!(ec2.text(), "hello");
    }

    #[test]
    fn cut_line_no_selection() {
        let mut ec = EditController::from_str("aaa\nbbb\nccc");
        ec.document.cursors = MultiCursor::new(Position::new(1, 0));
        let cut = ec.cut_line();
        assert_eq!(cut, "bbb\n");
        assert_eq!(ec.text(), "aaa\nccc");
    }

    #[test]
    fn copy_line_up_down() {
        let mut ec = EditController::from_str("aaa\nbbb\nccc");
        ec.document.cursors = MultiCursor::new(Position::new(1, 0));
        ec.copy_line_up();
        assert!(ec.text().contains("bbb"));
        assert!(ec.document.buffer.len_lines() >= 4);
    }

    #[test]
    fn move_line_up_down() {
        let mut ec = EditController::from_str("aaa\nbbb\nccc");
        ec.document.cursors = MultiCursor::new(Position::new(1, 0));
        ec.move_line_up();
        let text = ec.text();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines[0], "bbb");
        assert_eq!(lines[1], "aaa");
    }

    #[test]
    fn indent_outdent() {
        let mut ec = EditController::from_str("hello");
        ec.document.cursors = MultiCursor::new(Position::new(0, 0));
        ec.indent_line();
        assert!(ec.text().starts_with("    "));
        ec.outdent_line();
        assert_eq!(ec.text(), "hello");
    }

    #[test]
    fn insert_line_above_below() {
        let mut ec = EditController::from_str("aaa\nbbb");
        ec.document.cursors = MultiCursor::new(Position::new(1, 0));
        ec.insert_line_above();
        assert!(ec.document.buffer.len_lines() >= 3);

        let mut ec2 = EditController::from_str("aaa\nbbb");
        ec2.document.cursors = MultiCursor::new(Position::new(0, 0));
        ec2.insert_line_below();
        assert!(ec2.document.buffer.len_lines() >= 3);
    }

    #[test]
    fn toggle_comment() {
        let mut ec = EditController::from_str("hello");
        ec.document.cursors = MultiCursor::new(Position::new(0, 0));
        ec.toggle_comment("//");
        assert!(ec.text().starts_with("// "));
        ec.toggle_comment("//");
        assert_eq!(ec.text(), "hello");
    }

    #[test]
    fn toggle_block_comment() {
        let mut ec = EditController::from_str("hello");
        ec.document.cursors = MultiCursor::new(Position::new(0, 0));
        ec.document.cursors.set_primary_selection(Selection::new(
            Position::new(0, 0),
            Position::new(0, 5),
        ));
        ec.toggle_block_comment("/*", "*/");
        assert_eq!(ec.text(), "/* hello */");
    }

    #[test]
    fn join_lines() {
        let mut ec = EditController::from_str("hello\nworld");
        ec.document.cursors = MultiCursor::new(Position::new(0, 0));
        ec.join_lines();
        assert_eq!(ec.text(), "hello world");
    }

    #[test]
    fn transpose_characters() {
        let mut ec = EditController::from_str("abcd");
        ec.document.cursors = MultiCursor::new(Position::new(0, 2));
        ec.transpose_characters();
        assert_eq!(ec.text(), "acbd");
    }

    #[test]
    fn transform_uppercase() {
        let mut ec = EditController::from_str("hello");
        ec.document.cursors.set_primary_selection(Selection::new(
            Position::new(0, 0),
            Position::new(0, 5),
        ));
        ec.transform_to_uppercase();
        assert_eq!(ec.text(), "HELLO");
    }

    #[test]
    fn transform_lowercase() {
        let mut ec = EditController::from_str("HELLO");
        ec.document.cursors.set_primary_selection(Selection::new(
            Position::new(0, 0),
            Position::new(0, 5),
        ));
        ec.transform_to_lowercase();
        assert_eq!(ec.text(), "hello");
    }

    #[test]
    fn transform_title_case() {
        let mut ec = EditController::from_str("hello world");
        ec.document.cursors.set_primary_selection(Selection::new(
            Position::new(0, 0),
            Position::new(0, 11),
        ));
        ec.transform_to_title_case();
        assert_eq!(ec.text(), "Hello World");
    }

    #[test]
    fn sort_ascending_descending() {
        let mut ec = EditController::from_str("cherry\napple\nbanana");
        ec.sort_lines_ascending();
        assert_eq!(ec.text(), "apple\nbanana\ncherry");

        ec.sort_lines_descending();
        assert_eq!(ec.text(), "cherry\nbanana\napple");
    }

    #[test]
    fn remove_duplicate_lines() {
        let mut ec = EditController::from_str("aaa\naaa\nbbb\nbbb\nccc");
        ec.remove_duplicate_lines();
        assert_eq!(ec.text(), "aaa\nbbb\nccc");
    }

    #[test]
    fn remove_duplicate_lines_no_dupes() {
        let mut ec = EditController::from_str("aaa\nbbb\nccc");
        ec.remove_duplicate_lines();
        assert_eq!(ec.text(), "aaa\nbbb\nccc");
    }

    #[test]
    fn trim_trailing_whitespace() {
        let mut ec = EditController::from_str("hello   \nworld  ");
        ec.trim_trailing_whitespace();
        assert_eq!(ec.text(), "hello\nworld");
    }

    #[test]
    fn add_cursors_to_line_ends() {
        let mut ec = EditController::from_str("aaa\nbbb\nccc");
        ec.document.cursors.set_primary_selection(Selection::new(
            Position::new(0, 0),
            Position::new(2, 3),
        ));
        ec.add_cursors_to_line_ends();
        assert_eq!(ec.document.cursors.len(), 3);
    }

    #[test]
    fn add_cursors_to_line_ends_empty_selection_noop() {
        let mut ec = EditController::from_str("aaa\nbbb");
        ec.document.cursors = MultiCursor::new(Position::new(0, 1));
        ec.add_cursors_to_line_ends();
        assert_eq!(ec.document.cursors.len(), 1);
    }

    #[test]
    fn undo_redo() {
        let mut ec = EditController::from_str("hello");
        ec.document.cursors = MultiCursor::new(Position::new(0, 5));
        ec.type_text("!");
        assert_eq!(ec.text(), "hello!");
        ec.undo();
        ec.redo();
    }

    #[test]
    fn default_is_empty() {
        let ec = EditController::default();
        assert_eq!(ec.text(), "");
    }
}
