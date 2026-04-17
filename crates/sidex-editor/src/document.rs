use sidex_text::{Buffer, LineEnding, Position};

use crate::multi_cursor::MultiCursor;
use crate::selection::Selection;
use crate::undo::{EditGroup, UndoRedoStack};
use crate::word::{find_word_end, find_word_start};

const OPEN_BRACKETS: &[char] = &['(', '[', '{'];
const CLOSE_BRACKETS: &[char] = &[')', ']', '}'];
const QUOTE_CHARS: &[char] = &['"', '\'', '`'];
const INDENT_AFTER: &[char] = &['{', ':', '(', '['];
const OUTDENT_BEFORE: &[char] = &['}', ')', ']'];

/// Auto-closing pair definition (open, close).
const AUTO_CLOSE_PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];
/// Surrounding pairs including quotes.
const SURROUND_PAIRS: &[(char, char)] = &[
    ('(', ')'),
    ('[', ']'),
    ('{', '}'),
    ('"', '"'),
    ('\'', '\''),
    ('`', '`'),
];

fn matching_close_bracket(open: char) -> Option<char> {
    OPEN_BRACKETS
        .iter()
        .zip(CLOSE_BRACKETS.iter())
        .find(|(&o, _)| o == open)
        .map(|(_, &c)| c)
}

fn matching_open_bracket(close: char) -> Option<char> {
    CLOSE_BRACKETS
        .iter()
        .zip(OPEN_BRACKETS.iter())
        .find(|(&c, _)| c == close)
        .map(|(_, &o)| o)
}

fn is_quote(ch: char) -> bool {
    QUOTE_CHARS.contains(&ch)
}

fn surround_close(ch: char) -> Option<char> {
    SURROUND_PAIRS
        .iter()
        .find(|(o, _)| *o == ch)
        .map(|(_, c)| *c)
}

/// Editor configuration for type operations (mirrors VS Code `CursorConfiguration`).
#[derive(Debug, Clone)]
pub struct EditorConfig {
    pub tab_size: u32,
    pub insert_spaces: bool,
    pub auto_closing_brackets: AutoClosingStrategy,
    pub auto_closing_quotes: AutoClosingStrategy,
    pub auto_closing_delete: AutoClosingEditStrategy,
    pub auto_surround: AutoSurroundStrategy,
    pub auto_indent: AutoIndentStrategy,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            auto_closing_brackets: AutoClosingStrategy::LanguageDefined,
            auto_closing_quotes: AutoClosingStrategy::LanguageDefined,
            auto_closing_delete: AutoClosingEditStrategy::Auto,
            auto_surround: AutoSurroundStrategy::LanguageDefined,
            auto_indent: AutoIndentStrategy::Full,
        }
    }
}

impl EditorConfig {
    fn indent_str(&self) -> String {
        if self.insert_spaces {
            " ".repeat(self.tab_size as usize)
        } else {
            "\t".to_string()
        }
    }

    fn normalize_indentation(&self, indent: &str) -> String {
        if self.insert_spaces {
            indent.replace('\t', &" ".repeat(self.tab_size as usize))
        } else {
            let spaces = " ".repeat(self.tab_size as usize);
            indent.replace(&spaces, "\t")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoClosingStrategy {
    Always,
    LanguageDefined,
    BeforeWhitespace,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoClosingEditStrategy {
    Always,
    Auto,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoSurroundStrategy {
    LanguageDefined,
    Quotes,
    Brackets,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoIndentStrategy {
    None,
    Keep,
    Brackets,
    Full,
}

/// Tracks ranges that were auto-closed so we know when to delete pairs.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct AutoClosedRange {
    open_line: u32,
    open_col: u32,
    close_line: u32,
    close_col: u32,
}

/// The edit operation type, used for undo grouping decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditOperationType {
    Other,
    Typing,
    DeletingLeft,
    DeletingRight,
}

/// Result of a composition (IME) operation.
#[derive(Debug, Clone)]
pub struct CompositionOutcome {
    pub inserted_text: String,
    pub deleted_text: String,
    pub replaced_range_start: usize,
    pub replaced_range_end: usize,
}

/// The top-level document type tying buffer, cursors, and undo together.
#[derive(Debug, Clone)]
pub struct Document {
    /// The underlying text buffer.
    pub buffer: Buffer,
    /// Multi-cursor state.
    pub cursors: MultiCursor,
    /// Undo/redo history.
    pub undo_stack: UndoRedoStack,
    /// Monotonically increasing version for dirty detection.
    pub version: u64,
    /// Whether the document differs from its last saved state.
    pub is_modified: bool,
    /// The line ending style for this document.
    pub line_ending: LineEnding,
    /// Whether word wrap is enabled.
    pub word_wrap: bool,
    /// Editor configuration for type operations.
    pub config: EditorConfig,
    /// Previous edit operation type for undo grouping.
    prev_edit_type: EditOperationType,
    /// Ranges that were auto-closed (for bracket-pair delete).
    auto_closed: Vec<AutoClosedRange>,
    /// Whether we are currently in an IME composition.
    pub is_composing: bool,
}

impl Document {
    /// Creates a new empty document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            cursors: MultiCursor::new(Position::ZERO),
            undo_stack: UndoRedoStack::new(),
            version: 0,
            is_modified: false,
            line_ending: LineEnding::Lf,
            word_wrap: false,
            config: EditorConfig::default(),
            prev_edit_type: EditOperationType::Other,
            auto_closed: Vec::new(),
            is_composing: false,
        }
    }

    /// Creates a document from the given text.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        let le = sidex_text::detect_line_ending(text);
        let normalized = sidex_text::normalize_line_endings(text, LineEnding::Lf);
        Self {
            buffer: Buffer::from_str(&normalized),
            cursors: MultiCursor::new(Position::ZERO),
            undo_stack: UndoRedoStack::new(),
            version: 0,
            is_modified: false,
            line_ending: le,
            word_wrap: false,
            config: EditorConfig::default(),
            prev_edit_type: EditOperationType::Other,
            auto_closed: Vec::new(),
            is_composing: false,
        }
    }

    /// Returns the full document text.
    #[must_use]
    pub fn text(&self) -> String {
        self.buffer.text()
    }

    fn bump_version(&mut self) {
        self.version += 1;
        self.is_modified = true;
    }

    fn cursor_selections(&self) -> Vec<Selection> {
        self.cursors.cursors().iter().map(|c| c.selection).collect()
    }

    // ── Core multi-cursor edit helper ─────────────────────────────

    /// Applies an edit to each cursor in reverse-sorted order, repositioning
    /// cursors after each insert. The callback returns `(start_off, end_off,
    /// text_to_insert, chars_from_start_to_new_cursor)`.
    fn apply_at_cursors(
        &mut self,
        f: impl Fn(&Buffer, Selection) -> (usize, usize, String, usize),
    ) -> (Vec<Selection>, Vec<Selection>) {
        let before = self.cursor_selections();
        let mut sels: Vec<(usize, Selection)> = self
            .cursors
            .cursors()
            .iter()
            .enumerate()
            .map(|(i, c)| (i, c.selection))
            .collect();
        sels.sort_by_key(|(_, s)| std::cmp::Reverse(s.start()));

        let mut new_positions: Vec<(usize, usize)> = Vec::with_capacity(sels.len());
        for (idx, sel) in &sels {
            let (start_off, end_off, text, cursor_offset_from_start) = f(&self.buffer, *sel);
            if start_off < end_off {
                self.buffer.remove(start_off..end_off);
            }
            if !text.is_empty() {
                self.buffer.insert(start_off, &text);
            }
            let new_off = (start_off + cursor_offset_from_start).min(self.buffer.len_chars());
            new_positions.push((*idx, new_off));
        }

        new_positions.sort_by_key(|(idx, _)| *idx);
        let cursors = self.cursors.cursors_mut();
        for (idx, off) in &new_positions {
            if *idx < cursors.len() {
                let pos = self.buffer.offset_to_position(*off);
                cursors[*idx].selection = Selection::caret(pos);
                cursors[*idx].preferred_column = None;
            }
        }
        self.cursors.merge_overlapping();

        let after = self.cursor_selections();
        (before, after)
    }

    // ── TYPE CHAR DISPATCHER (VS Code typeWithInterceptors) ───────

    /// Main entry point for typing a character, applying all interceptors
    /// in VS Code's order: Enter, auto-indent, auto-close overtype,
    /// auto-close open, surround selection, electric char, simple char.
    pub fn type_char(&mut self, ch: char) {
        if self.is_composing {
            self.insert_text(&ch.to_string());
            return;
        }

        if ch == '\n' {
            self.new_line_with_indent();
            return;
        }

        if ch == '\t' {
            self.tab();
            return;
        }

        // Auto-closing overtype: if the char to type is a closing bracket/quote
        // and the char right after cursor is the same, just skip over it.
        if self.try_auto_close_overtype(ch) {
            return;
        }

        // Auto-closing open: if typing an opening bracket/quote, auto-insert close.
        if self.try_auto_close_open(ch) {
            return;
        }

        // Auto-surround: if selection exists and typing a surround char, wrap.
        if self.try_surround_selection(ch) {
            return;
        }

        // Electric character: auto-outdent on closing bracket matching.
        if self.try_electric_char(ch) {
            return;
        }

        // Simple character type.
        self.simple_type_char(ch);
    }

    fn simple_type_char(&mut self, ch: char) {
        let s = ch.to_string();
        let (before, after) = self.apply_at_cursors(|buf, sel| {
            let start_off = buf.position_to_offset(sel.start());
            let end_off = buf.position_to_offset(sel.end());
            (start_off, end_off, s.clone(), 1)
        });
        self.undo_stack.push(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Typing;
        self.bump_version();
    }

    // ── AUTO-CLOSING OVERTYPE ─────────────────────────────────────

    /// When cursor is right before a closing bracket/quote that was auto-inserted,
    /// just move the cursor past it instead of inserting a duplicate.
    fn try_auto_close_overtype(&mut self, ch: char) -> bool {
        if self.config.auto_closing_brackets == AutoClosingStrategy::Never
            && self.config.auto_closing_quotes == AutoClosingStrategy::Never
        {
            return false;
        }

        let is_close_bracket = CLOSE_BRACKETS.contains(&ch);
        let is_close_quote = is_quote(ch);
        if !is_close_bracket && !is_close_quote {
            return false;
        }

        let all_can_overtype = self.cursors.cursors().iter().all(|c| {
            if !c.selection.is_empty() {
                return false;
            }
            let pos = c.position();
            let line_content = self.buffer.line_content(pos.line as usize);
            let chars: Vec<char> = line_content.chars().collect();
            let col = pos.column as usize;
            col < chars.len() && chars[col] == ch
        });

        if !all_can_overtype {
            return false;
        }

        let before = self.cursor_selections();
        for cursor in self.cursors.cursors_mut() {
            let pos = cursor.position();
            cursor.selection = Selection::caret(Position::new(pos.line, pos.column + 1));
            cursor.preferred_column = None;
        }
        self.cursors.merge_overlapping();
        let after = self.cursor_selections();
        self.undo_stack.push(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Typing;
        self.bump_version();
        true
    }

    // ── AUTO-CLOSING OPEN ─────────────────────────────────────────

    /// When typing an opening bracket or quote, auto-insert the closing
    /// character and place cursor between them.
    fn try_auto_close_open(&mut self, ch: char) -> bool {
        let is_open_bracket = OPEN_BRACKETS.contains(&ch);
        let is_open_quote = is_quote(ch);

        if !is_open_bracket && !is_open_quote {
            return false;
        }

        if is_open_bracket && self.config.auto_closing_brackets == AutoClosingStrategy::Never {
            return false;
        }
        if is_open_quote && self.config.auto_closing_quotes == AutoClosingStrategy::Never {
            return false;
        }

        let close = if is_open_bracket {
            match matching_close_bracket(ch) {
                Some(c) => c,
                None => return false,
            }
        } else {
            ch
        };

        let all_empty = self
            .cursors
            .cursors()
            .iter()
            .all(|c| c.selection.is_empty());
        if !all_empty {
            return false;
        }

        let should_auto_close = self.cursors.cursors().iter().all(|c| {
            let pos = c.position();
            let line_content = self.buffer.line_content(pos.line as usize);
            let chars: Vec<char> = line_content.chars().collect();
            let col = pos.column as usize;

            if is_open_quote
                && self.config.auto_closing_quotes != AutoClosingStrategy::Always
                && col > 0
                && chars[col - 1].is_alphanumeric()
            {
                return false;
            }

            if col < chars.len() {
                let after = chars[col];
                after.is_whitespace()
                    || CLOSE_BRACKETS.contains(&after)
                    || after == ';'
                    || after == ','
            } else {
                true
            }
        });

        if !should_auto_close {
            return false;
        }

        let pair = format!("{ch}{close}");
        let (before, after) = self.apply_at_cursors(|buf, sel| {
            let start_off = buf.position_to_offset(sel.start());
            let end_off = buf.position_to_offset(sel.end());
            (start_off, end_off, pair.clone(), 1)
        });

        for cursor in self.cursors.cursors() {
            let pos = cursor.position();
            self.auto_closed.push(AutoClosedRange {
                open_line: pos.line,
                open_col: pos.column.saturating_sub(1),
                close_line: pos.line,
                close_col: pos.column,
            });
        }

        self.undo_stack.push(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Typing;
        self.bump_version();
        true
    }

    // ── AUTO-SURROUND ─────────────────────────────────────────────

    /// When text is selected and you type a surround character, wrap
    /// the selection in the pair.
    fn try_surround_selection(&mut self, ch: char) -> bool {
        if self.config.auto_surround == AutoSurroundStrategy::Never {
            return false;
        }

        let Some(close) = surround_close(ch) else {
            return false;
        };

        let any_has_selection = self
            .cursors
            .cursors()
            .iter()
            .any(|c| !c.selection.is_empty());
        if !any_has_selection {
            return false;
        }

        let all_non_empty = self
            .cursors
            .cursors()
            .iter()
            .all(|c| !c.selection.is_empty());
        if !all_non_empty {
            return false;
        }

        let before = self.cursor_selections();
        let mut sels: Vec<Selection> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let end_off = self.buffer.position_to_offset(sel.end());
            let start_off = self.buffer.position_to_offset(sel.start());
            self.buffer.insert(end_off, &close.to_string());
            self.buffer.insert(start_off, &ch.to_string());
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Other;
        self.bump_version();
        true
    }

    // ── ELECTRIC CHARACTER ────────────────────────────────────────

    /// On typing a closing bracket, re-indent the line to match the opening
    /// bracket's line indentation.
    fn try_electric_char(&mut self, ch: char) -> bool {
        if !OUTDENT_BEFORE.contains(&ch) {
            return false;
        }
        if self.config.auto_indent == AutoIndentStrategy::None {
            return false;
        }
        if self.cursors.len() != 1 {
            return false;
        }

        let pos = self.cursors.primary().position();
        let line_content = self.buffer.line_content(pos.line as usize);
        let before_cursor: String = line_content.chars().take(pos.column as usize).collect();

        if !before_cursor.trim().is_empty() {
            return false;
        }

        if let Some(open) = matching_open_bracket(ch) {
            let text = self.buffer.text();
            let chars: Vec<char> = text.chars().collect();
            let cursor_off = self.buffer.position_to_offset(pos);

            if let Some(match_pos) = find_matching_open_from(&chars, cursor_off, open, ch) {
                let match_position = self.buffer.offset_to_position(match_pos);
                let match_line_content = self.buffer.line_content(match_position.line as usize);
                let match_indent: String = match_line_content
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                let normalized = self.config.normalize_indentation(&match_indent);

                let before = self.cursor_selections();
                let line_start = self.buffer.line_to_char(pos.line as usize);
                let ws_end = line_start + before_cursor.len();
                self.buffer.remove(line_start..ws_end);
                let insert_text = format!("{normalized}{ch}");
                self.buffer.insert(line_start, &insert_text);

                let new_col = (normalized.len() + 1) as u32;
                self.cursors.cursors_mut()[0].selection =
                    Selection::caret(Position::new(pos.line, new_col));
                self.cursors.cursors_mut()[0].preferred_column = None;

                let after = self.cursor_selections();
                self.undo_stack
                    .push_barrier(EditGroup::empty(before, after));
                self.prev_edit_type = EditOperationType::Typing;
                self.bump_version();
                return true;
            }
        }
        false
    }

    /// Inserts text at all cursor positions with proper multi-cursor repositioning.
    pub fn insert_text(&mut self, text: &str) {
        let text_owned = text.to_string();
        let text_char_len = text.chars().count();
        let (before, after) = self.apply_at_cursors(|buf, sel| {
            let start_off = buf.position_to_offset(sel.start());
            let end_off = buf.position_to_offset(sel.end());
            (start_off, end_off, text_owned.clone(), text_char_len)
        });
        self.undo_stack.push(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Typing;
        self.bump_version();
    }

    /// Deletes one character to the left at all cursors (Backspace).
    /// Handles: bracket pair deletion, tab-stop unindent, then normal delete.
    pub fn delete_left(&mut self) {
        if self.try_auto_close_pair_delete() {
            return;
        }
        if self.try_backspace_unindent() {
            return;
        }
        self.delete_left_simple();
    }

    /// Simple single-char backspace without special handling.
    fn delete_left_simple(&mut self) {
        let before = self.cursor_selections();
        let mut sels: Vec<Selection> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            if sel.is_empty() {
                let off = self.buffer.position_to_offset(sel.active);
                if off > 0 {
                    self.buffer.remove((off - 1)..off);
                }
            } else {
                let s = self.buffer.position_to_offset(sel.start());
                let e = self.buffer.position_to_offset(sel.end());
                self.buffer.remove(s..e);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack.push(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::DeletingLeft;
        self.bump_version();
    }

    // ── BACKSPACE: bracket pair deletion ──────────────────────────

    /// If cursor is between matching auto-closed brackets like `(|)`,
    /// delete both characters.
    fn try_auto_close_pair_delete(&mut self) -> bool {
        if self.config.auto_closing_delete == AutoClosingEditStrategy::Never {
            return false;
        }

        let all_between_pairs = self.cursors.cursors().iter().all(|c| {
            if !c.selection.is_empty() {
                return false;
            }
            let pos = c.position();
            if pos.column == 0 {
                return false;
            }
            let content = self.buffer.line_content(pos.line as usize);
            let chars: Vec<char> = content.chars().collect();
            let col = pos.column as usize;
            if col == 0 || col >= chars.len() {
                return false;
            }
            let before = chars[col - 1];
            let after = chars[col];
            AUTO_CLOSE_PAIRS
                .iter()
                .any(|(o, c)| *o == before && *c == after)
                || (is_quote(before) && before == after)
        });

        if !all_between_pairs {
            return false;
        }

        let before = self.cursor_selections();
        let mut sels: Vec<Selection> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let pos = sel.active;
            let off = self.buffer.position_to_offset(pos);
            self.buffer.remove((off - 1)..(off + 1));
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::DeletingLeft;
        self.bump_version();
        true
    }

    // ── BACKSPACE: tab-stop unindent ──────────────────────────────

    /// When cursor is within leading whitespace, delete back to the previous
    /// tab stop instead of one character.
    fn try_backspace_unindent(&mut self) -> bool {
        if !self.config.insert_spaces {
            return false;
        }

        let tab_size = self.config.tab_size as usize;
        let all_in_indent = self.cursors.cursors().iter().all(|c| {
            if !c.selection.is_empty() {
                return false;
            }
            let pos = c.position();
            if pos.column == 0 {
                return false;
            }
            let content = self.buffer.line_content(pos.line as usize);
            let col = pos.column as usize;
            let leading: usize = content.chars().take_while(|ch| *ch == ' ').count();
            col <= leading && col > 0
        });

        if !all_in_indent {
            return false;
        }

        let before = self.cursor_selections();
        let mut sels: Vec<Selection> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let pos = sel.active;
            let col = pos.column as usize;
            let prev_tab_stop = (col.saturating_sub(1)) / tab_size * tab_size;
            let remove_count = col - prev_tab_stop;
            let off = self.buffer.position_to_offset(pos);
            let start = off - remove_count;
            self.buffer.remove(start..off);
        }

        let after = self.cursor_selections();
        self.undo_stack.push(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::DeletingLeft;
        self.bump_version();
        true
    }

    /// Deletes one character to the right at all cursors (Delete).
    pub fn delete_right(&mut self) {
        let before = self.cursor_selections();
        let mut selections: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        selections.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &selections {
            if sel.is_empty() {
                let off = self.buffer.position_to_offset(sel.active);
                if off < self.buffer.len_chars() {
                    self.buffer.remove(off..(off + 1));
                }
            } else {
                let s = self.buffer.position_to_offset(sel.start());
                let e = self.buffer.position_to_offset(sel.end());
                self.buffer.remove(s..e);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack.push(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Deletes one word to the left at all cursors (Ctrl+Backspace).
    pub fn delete_word_left(&mut self) {
        let before = self.cursor_selections();
        let mut selections: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        selections.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &selections {
            let pos = if sel.is_empty() {
                sel.active
            } else {
                sel.start()
            };
            let word_start = find_word_start(&self.buffer, pos);
            let end = if sel.is_empty() { pos } else { sel.end() };
            let s = self.buffer.position_to_offset(word_start);
            let e = self.buffer.position_to_offset(end);
            if s < e {
                self.buffer.remove(s..e);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Deletes one word to the right at all cursors (Ctrl+Delete).
    pub fn delete_word_right(&mut self) {
        let before = self.cursor_selections();
        let mut selections: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        selections.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &selections {
            let pos = if sel.is_empty() {
                sel.active
            } else {
                sel.end()
            };
            let word_end = find_word_end(&self.buffer, pos);
            let start = if sel.is_empty() { pos } else { sel.start() };
            let s = self.buffer.position_to_offset(start);
            let e = self.buffer.position_to_offset(word_end);
            if s < e {
                self.buffer.remove(s..e);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Deletes the entire line at each cursor.
    pub fn delete_line(&mut self) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for &line in lines.iter().rev() {
            let line_idx = line as usize;
            if line_idx >= self.buffer.len_lines() {
                continue;
            }
            let start = self.buffer.line_to_char(line_idx);
            let end = if line_idx + 1 < self.buffer.len_lines() {
                self.buffer.line_to_char(line_idx + 1)
            } else {
                self.buffer.len_chars()
            };
            if start < end {
                self.buffer.remove(start..end);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Inserts a newline at all cursor positions.
    pub fn new_line(&mut self) {
        self.insert_text("\n");
    }

    /// Inserts a tab or indents the selection at each cursor.
    /// - Empty selection on whitespace-only line: indent to proper level
    /// - Empty selection: insert to next tab stop
    /// - Multi-line selection: indent all selected lines
    /// - Single-line selection (not whole line): insert to next tab stop
    pub fn tab(&mut self) {
        let has_multiline = self.cursors.cursors().iter().any(|c| {
            let sel = c.selection;
            !sel.is_empty() && sel.start().line != sel.end().line
        });

        if has_multiline {
            self.indent();
            return;
        }

        let tab_text = self.config.indent_str();
        let tab_size = self.config.tab_size as usize;

        let (before, after) = self.apply_at_cursors(|buf, sel| {
            let start_off = buf.position_to_offset(sel.start());
            let end_off = buf.position_to_offset(sel.end());
            let col = sel.start().column as usize;
            let spaces_to_next = tab_size - (col % tab_size);
            let insert = if tab_text == "\t" {
                "\t".to_string()
            } else {
                " ".repeat(spaces_to_next)
            };
            let insert_len = insert.chars().count();
            (start_off, end_off, insert, insert_len)
        });

        self.undo_stack.push(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Typing;
        self.bump_version();
    }

    /// Inserts a tab/spaces-based indent at each cursor's line(s).
    pub fn indent(&mut self) {
        let indent_str = self.config.indent_str();
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .flat_map(|c| {
                let start = c.selection.start().line;
                let end = c.selection.end().line;
                (start..=end).collect::<Vec<_>>()
            })
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for &line in lines.iter().rev() {
            let line_start = self.buffer.line_to_char(line as usize);
            self.buffer.insert(line_start, &indent_str);
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Other;
        self.bump_version();
    }

    /// Removes one level of indentation at each cursor (Shift+Tab).
    pub fn outdent(&mut self) {
        let before = self.cursor_selections();
        let tab_size = self.config.tab_size as usize;
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .flat_map(|c| {
                let start = c.selection.start().line;
                let end = c.selection.end().line;
                (start..=end).collect::<Vec<_>>()
            })
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for &line in lines.iter().rev() {
            let content = self.buffer.line_content(line as usize);
            let first_char = content.chars().next();
            let spaces: usize = if first_char == Some('\t') {
                1
            } else {
                content
                    .chars()
                    .take(tab_size)
                    .take_while(|c| *c == ' ')
                    .count()
            };
            if spaces > 0 {
                let start = self.buffer.line_to_char(line as usize);
                self.buffer.remove(start..(start + spaces));
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Other;
        self.bump_version();
    }

    /// Undoes the last edit group and restores cursor state.
    pub fn undo(&mut self) {
        if let Some(group) = self.undo_stack.undo() {
            for (_fwd, inv) in group.edits.iter().rev() {
                self.buffer.apply_edit(inv);
            }
            self.restore_cursors(&group.cursor_before);
            self.bump_version();
        }
    }

    /// Redoes the last undone edit group.
    pub fn redo(&mut self) {
        if let Some(group) = self.undo_stack.redo() {
            for (fwd, _inv) in &group.edits {
                self.buffer.apply_edit(fwd);
            }
            self.restore_cursors(&group.cursor_after);
            self.bump_version();
        }
    }

    fn restore_cursors(&mut self, selections: &[Selection]) {
        if selections.is_empty() {
            return;
        }
        self.cursors = MultiCursor::new(selections[0].active);
        if !selections[0].is_empty() {
            self.cursors.set_primary_selection(selections[0]);
        }
        for sel in selections.iter().skip(1) {
            self.cursors.add_cursor(sel.active);
        }
    }

    /// Selects the entire document.
    pub fn select_all(&mut self) {
        let last_line = (self.buffer.len_lines() - 1) as u32;
        let last_col = self.buffer.line_content_len(last_line as usize) as u32;
        let end = Position::new(last_line, last_col);
        self.cursors = MultiCursor::new(Position::ZERO);
        self.cursors
            .set_primary_selection(Selection::new(Position::ZERO, end));
    }

    /// Selects the current line at each cursor.
    pub fn select_line(&mut self) {
        let line_selections: Vec<_> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| {
                let line = c.position().line;
                let start = Position::new(line, 0);
                let end_col = self.buffer.line_content_len(line as usize) as u32;
                Selection::new(start, Position::new(line, end_col))
            })
            .collect();

        if let Some(first) = line_selections.first() {
            self.cursors = MultiCursor::new(first.active);
            self.cursors.set_primary_selection(*first);
            for sel in line_selections.iter().skip(1) {
                self.cursors.add_cursor(sel.active);
            }
        }
    }

    /// Duplicates the line at each cursor.
    pub fn duplicate_line(&mut self) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for (offset, &line) in lines.iter().enumerate() {
            let actual_line = (line as usize) + offset;
            let content = self.buffer.line_content(actual_line);
            let insert_pos = if actual_line + 1 < self.buffer.len_lines() {
                self.buffer.line_to_char(actual_line + 1)
            } else {
                let end = self.buffer.len_chars();
                self.buffer.insert(end, "\n");
                end + 1
            };
            let insert_text = format!("{content}\n");
            self.buffer.insert(insert_pos, &insert_text);
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Moves the line at each cursor up by one line.
    pub fn move_line_up(&mut self) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for &line in &lines {
            if line == 0 {
                continue;
            }
            let curr_start = self.buffer.line_to_char(line as usize);
            let curr_end = if (line as usize) + 1 < self.buffer.len_lines() {
                self.buffer.line_to_char((line as usize) + 1)
            } else {
                self.buffer.len_chars()
            };
            let curr_text = self.buffer.slice(curr_start..curr_end);

            let prev_start = self.buffer.line_to_char((line as usize) - 1);
            let prev_text = self.buffer.slice(prev_start..curr_start);

            self.buffer.remove(prev_start..curr_end);
            self.buffer.insert(prev_start, &curr_text);
            self.buffer
                .insert(prev_start + curr_text.chars().count(), &prev_text);
        }

        self.cursors.move_all_up(&self.buffer, false);

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Moves the line at each cursor down by one line.
    pub fn move_line_down(&mut self) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        let last_line = (self.buffer.len_lines() - 1) as u32;
        for &line in lines.iter().rev() {
            if line >= last_line {
                continue;
            }
            let curr_start = self.buffer.line_to_char(line as usize);
            let next_start = self.buffer.line_to_char((line as usize) + 1);
            let next_end = if (line as usize) + 2 < self.buffer.len_lines() {
                self.buffer.line_to_char((line as usize) + 2)
            } else {
                self.buffer.len_chars()
            };

            let curr_text = self.buffer.slice(curr_start..next_start);
            let next_text = self.buffer.slice(next_start..next_end);

            self.buffer.remove(curr_start..next_end);
            self.buffer.insert(curr_start, &next_text);
            self.buffer
                .insert(curr_start + next_text.chars().count(), &curr_text);
        }

        self.cursors.move_all_down(&self.buffer, false);

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Toggles line comments with the given prefix at each cursor's line.
    pub fn toggle_line_comment(&mut self, comment_prefix: &str) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        let prefix_with_space = format!("{comment_prefix} ");
        let all_commented = lines.iter().all(|&line| {
            let content = self.buffer.line_content(line as usize);
            let trimmed = content.trim_start();
            trimmed.starts_with(&prefix_with_space) || trimmed.starts_with(comment_prefix)
        });

        for &line in lines.iter().rev() {
            let content = self.buffer.line_content(line as usize);
            let line_start = self.buffer.line_to_char(line as usize);

            if all_commented {
                let leading_ws = content.len() - content.trim_start().len();
                let trimmed = content.trim_start();
                let remove_len = if trimmed.starts_with(&prefix_with_space) {
                    prefix_with_space.len()
                } else {
                    comment_prefix.len()
                };
                let start = line_start + leading_ws;
                self.buffer.remove(start..(start + remove_len));
            } else {
                self.buffer.insert(line_start, &prefix_with_space);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Auto-close / auto-surround (legacy API) ────────────────────

    // ── PASTE WITH AUTO-INDENT ────────────────────────────────────

    /// Pastes text at all cursor positions, adjusting indentation of pasted
    /// lines to match the target context (VS Code `PasteOperation`).
    pub fn paste_with_indent(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let lines: Vec<&str> = text.split('\n').collect();
        if lines.len() <= 1 {
            self.insert_text(text);
            return;
        }

        let multi_cursor_count = self.cursors.len();
        let paste_lines: Vec<&str> = text.split('\n').collect();
        if multi_cursor_count > 1 && paste_lines.len() == multi_cursor_count {
            let before = self.cursor_selections();
            let mut indexed: Vec<(usize, Selection)> = self
                .cursors
                .cursors()
                .iter()
                .enumerate()
                .map(|(i, c)| (i, c.selection))
                .collect();
            indexed.sort_by_key(|(_, s)| s.start());

            let paste_texts: Vec<String> = indexed
                .iter()
                .enumerate()
                .map(|(paste_idx, _)| paste_lines[paste_idx].to_string())
                .collect();

            indexed.reverse();
            for (i, (orig_idx, sel)) in indexed.iter().enumerate() {
                let paste_idx = paste_texts.len() - 1 - i;
                let start_off = self.buffer.position_to_offset(sel.start());
                let end_off = self.buffer.position_to_offset(sel.end());
                if start_off < end_off {
                    self.buffer.remove(start_off..end_off);
                }
                self.buffer.insert(start_off, &paste_texts[paste_idx]);
                let _ = orig_idx;
            }

            let after = self.cursor_selections();
            self.undo_stack
                .push_barrier(EditGroup::empty(before, after));
            self.prev_edit_type = EditOperationType::Other;
            self.bump_version();
            return;
        }

        let first_line = lines[0];
        let rest_lines = &lines[1..];

        let pasted_indent: String = if rest_lines.is_empty() {
            String::new()
        } else {
            let min_indent = rest_lines
                .iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.chars().take_while(|c| c.is_whitespace()).count())
                .min()
                .unwrap_or(0);
            " ".repeat(min_indent)
        };

        let before = self.cursor_selections();
        let mut sels: Vec<Selection> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let pos = sel.start();
            let context_line = self.buffer.line_content(pos.line as usize);
            let context_indent: String = context_line
                .chars()
                .take_while(|c| c.is_whitespace())
                .collect();
            let target_indent = self.config.normalize_indentation(&context_indent);

            let start_off = self.buffer.position_to_offset(sel.start());
            let end_off = self.buffer.position_to_offset(sel.end());
            if start_off < end_off {
                self.buffer.remove(start_off..end_off);
            }

            let mut result = first_line.to_string();
            for rest_line in rest_lines {
                result.push('\n');
                if rest_line.trim().is_empty() {
                    result.push_str(rest_line);
                } else {
                    let stripped = if rest_line.starts_with(&pasted_indent) {
                        &rest_line[pasted_indent.len()..]
                    } else {
                        rest_line.trim_start()
                    };
                    result.push_str(&target_indent);
                    result.push_str(stripped);
                }
            }
            self.buffer.insert(start_off, &result);
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Other;
        self.bump_version();
    }

    // ── COMPOSITION / IME INPUT ───────────────────────────────────

    /// Starts a composition (IME) session.
    pub fn composition_start(&mut self) {
        self.is_composing = true;
    }

    /// Ends a composition (IME) session. The composed text has already been
    /// inserted via `insert_text` or `composition_type` during the session.
    pub fn composition_end(&mut self) {
        self.is_composing = false;
    }

    /// Types text during composition, replacing `replace_prev_chars` characters
    /// before the cursor and `replace_next_chars` after.
    pub fn composition_type(
        &mut self,
        text: &str,
        replace_prev_chars: usize,
        replace_next_chars: usize,
    ) {
        let text_owned = text.to_string();
        let text_len = text.chars().count();
        let before = self.cursor_selections();
        let mut sels: Vec<Selection> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            if !sel.is_empty() {
                continue;
            }
            let pos = sel.active;
            let off = self.buffer.position_to_offset(pos);
            let start = off.saturating_sub(replace_prev_chars);
            let end = (off + replace_next_chars).min(self.buffer.len_chars());
            if start < end {
                self.buffer.remove(start..end);
            }
            self.buffer.insert(start, &text_owned);
        }

        let after = self.cursor_selections();
        self.undo_stack.push(EditGroup::empty(before, after));
        let _ = text_len;
        self.bump_version();
    }

    // ── Auto-close / auto-surround (legacy direct-call API) ──────

    /// Types an opening bracket and auto-inserts the matching close bracket,
    /// placing the cursor between them.
    pub fn auto_close_bracket(&mut self, open: char) {
        if let Some(close) = matching_close_bracket(open) {
            let pair = format!("{open}{close}");
            let before = self.cursor_selections();
            let mut sels: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
            sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

            for sel in &sels {
                let start_off = self.buffer.position_to_offset(sel.start());
                let end_off = self.buffer.position_to_offset(sel.end());
                if start_off < end_off {
                    self.buffer.remove(start_off..end_off);
                }
                self.buffer.insert(start_off, &pair);
            }

            let after = self.cursor_selections();
            self.undo_stack.push(EditGroup::empty(before, after));
            self.bump_version();
        }
    }

    /// Types a quote character and auto-inserts a matching closing quote,
    /// placing the cursor between them.
    pub fn auto_close_quote(&mut self, quote: char) {
        if !QUOTE_CHARS.contains(&quote) {
            self.insert_text(&quote.to_string());
            return;
        }
        let pair = format!("{quote}{quote}");
        let before = self.cursor_selections();
        let mut sels: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let start_off = self.buffer.position_to_offset(sel.start());
            let end_off = self.buffer.position_to_offset(sel.end());
            if start_off < end_off {
                self.buffer.remove(start_off..end_off);
            }
            self.buffer.insert(start_off, &pair);
        }

        let after = self.cursor_selections();
        self.undo_stack.push(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// When text is selected, surrounds it with the given open/close pair.
    pub fn auto_surround(&mut self, open: char, close: char) {
        let before = self.cursor_selections();
        let mut sels: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            if sel.is_empty() {
                continue;
            }
            let end_off = self.buffer.position_to_offset(sel.end());
            let start_off = self.buffer.position_to_offset(sel.start());
            self.buffer.insert(end_off, &close.to_string());
            self.buffer.insert(start_off, &open.to_string());
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Newline with auto-indent ──────────────────────────────────

    /// Inserts a newline, matching the indentation of the current line,
    /// and adding an extra indent level after `{`, `:`, `(`, `[`.
    /// Also handles the `IndentOutdent` case: `{|}` → `{\n  |\n}`.
    pub fn new_line_with_indent(&mut self) {
        let before = self.cursor_selections();
        let indent_str = self.config.indent_str();
        let mut sels: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let pos = sel.active;
            let content = self.buffer.line_content(pos.line as usize);
            let leading: String = content.chars().take_while(|c| c.is_whitespace()).collect();
            let normalized_leading = self.config.normalize_indentation(&leading);

            let char_before = if pos.column > 0 {
                content.chars().nth((pos.column - 1) as usize)
            } else {
                None
            };
            let char_after = content.chars().nth(pos.column as usize);

            let is_indent_after = char_before.is_some_and(|c| INDENT_AFTER.contains(&c));

            let is_indent_outdent = is_indent_after
                && char_after.is_some_and(|c| OUTDENT_BEFORE.contains(&c))
                && char_before
                    .and_then(matching_close_bracket)
                    .is_some_and(|close| char_after == Some(close));

            let start_off = self.buffer.position_to_offset(sel.start());
            let end_off = self.buffer.position_to_offset(sel.end());
            if start_off < end_off {
                self.buffer.remove(start_off..end_off);
            }

            if is_indent_outdent {
                let increased = format!("{normalized_leading}{indent_str}");
                let insert = format!("\n{increased}\n{normalized_leading}");
                self.buffer.insert(start_off, &insert);
            } else if is_indent_after {
                let insert = format!("\n{normalized_leading}{indent_str}");
                self.buffer.insert(start_off, &insert);
            } else {
                let insert = format!("\n{normalized_leading}");
                self.buffer.insert(start_off, &insert);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack.push(EditGroup::empty(before, after));
        self.prev_edit_type = EditOperationType::Typing;
        self.bump_version();
    }

    // ── Transpose ─────────────────────────────────────────────────

    /// Swaps the two characters surrounding the cursor.
    pub fn transpose_characters(&mut self) {
        let before = self.cursor_selections();
        let pos = self.cursors.primary().position();
        let line_len = self.buffer.line_content_len(pos.line as usize);
        if pos.column == 0 || line_len < 2 {
            return;
        }

        let col = (pos.column as usize).min(line_len);
        let (a_col, b_col) = if col >= line_len {
            (col - 2, col - 1)
        } else {
            (col - 1, col)
        };

        let line_start = self.buffer.line_to_char(pos.line as usize);
        let a_off = line_start + a_col;
        let b_off = line_start + b_col;
        let a_ch = self.buffer.slice(a_off..(a_off + 1));
        let b_ch = self.buffer.slice(b_off..(b_off + 1));

        self.buffer.remove(a_off..(b_off + 1));
        let swapped = format!("{b_ch}{a_ch}");
        self.buffer.insert(a_off, &swapped);

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Swaps the current line with the line above.
    pub fn transpose_lines(&mut self) {
        let pos = self.cursors.primary().position();
        if pos.line == 0 {
            return;
        }
        self.move_line_up();
    }

    // ── Join lines ────────────────────────────────────────────────

    /// Joins the current line with the next line, replacing the newline
    /// with a single space.
    pub fn join_lines(&mut self) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        let mut offset = 0i64;
        for &line in &lines {
            let actual = (i64::from(line) - offset) as usize;
            if actual + 1 >= self.buffer.len_lines() {
                continue;
            }
            let curr_content = self.buffer.line_content(actual);
            let next_content = self.buffer.line_content(actual + 1);
            let trimmed_next = next_content.trim_start();

            let curr_end = self.buffer.line_to_char(actual) + curr_content.len();
            let next_line_start = self.buffer.line_to_char(actual + 1);
            let next_content_end = next_line_start + next_content.len();
            let end = if actual + 2 < self.buffer.len_lines() {
                self.buffer.line_to_char(actual + 2)
            } else {
                self.buffer.len_chars()
            };

            self.buffer
                .remove(curr_end..end.min(self.buffer.len_chars()));
            let join_text = if trimmed_next.is_empty() {
                String::new()
            } else {
                format!(" {trimmed_next}")
            };
            self.buffer.insert(curr_end, &join_text);
            let _ = next_content_end;
            offset += 1;
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Sort lines ────────────────────────────────────────────────

    /// Sorts all lines in the document in ascending order.
    pub fn sort_lines_ascending(&mut self) {
        let before = self.cursor_selections();
        let text = self.buffer.text();
        let mut lines: Vec<&str> = text.split('\n').collect();
        lines.sort_unstable();
        let sorted = lines.join("\n");
        let len = self.buffer.len_chars();
        if len > 0 {
            self.buffer.remove(0..len);
        }
        self.buffer.insert(0, &sorted);

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Sorts all lines in the document in descending order.
    pub fn sort_lines_descending(&mut self) {
        let before = self.cursor_selections();
        let text = self.buffer.text();
        let mut lines: Vec<&str> = text.split('\n').collect();
        lines.sort_unstable();
        lines.reverse();
        let sorted = lines.join("\n");
        let len = self.buffer.len_chars();
        if len > 0 {
            self.buffer.remove(0..len);
        }
        self.buffer.insert(0, &sorted);

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Delete to line boundaries ─────────────────────────────────

    /// Deletes from the cursor to the start of the line.
    pub fn delete_all_left(&mut self) {
        let before = self.cursor_selections();
        let mut sels: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let pos = sel.active;
            let line_start = self.buffer.line_to_char(pos.line as usize);
            let cursor_off = self.buffer.position_to_offset(pos);
            if line_start < cursor_off {
                self.buffer.remove(line_start..cursor_off);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Deletes from the cursor to the end of the line.
    pub fn delete_all_right(&mut self) {
        let before = self.cursor_selections();
        let mut sels: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let pos = sel.active;
            let content_len = self.buffer.line_content_len(pos.line as usize);
            let line_start = self.buffer.line_to_char(pos.line as usize);
            let cursor_off = self.buffer.position_to_offset(pos);
            let line_content_end = line_start + content_len;
            if cursor_off < line_content_end {
                self.buffer.remove(cursor_off..line_content_end);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Block comments ────────────────────────────────────────────

    /// Toggles block comments (`/* */`-style) around each cursor's selection
    /// or current line.
    pub fn toggle_block_comment(&mut self, open: &str, close: &str) {
        let before = self.cursor_selections();
        let mut sels: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let start_off = self.buffer.position_to_offset(sel.start());
            let end_off = if sel.is_empty() {
                let line_len = self.buffer.line_content_len(sel.active.line as usize);
                let line_start = self.buffer.line_to_char(sel.active.line as usize);
                line_start + line_len
            } else {
                self.buffer.position_to_offset(sel.end())
            };

            let text = self.buffer.slice(start_off..end_off);
            let trimmed = text.trim();
            if trimmed.starts_with(open) && trimmed.ends_with(close) {
                let inner_start = text.find(open).unwrap_or(0);
                let inner_end = text.rfind(close).unwrap_or(text.len());
                let inner = &text[(inner_start + open.len())..inner_end];
                let inner = inner.trim().to_string();
                self.buffer.remove(start_off..end_off);
                self.buffer.insert(start_off, &inner);
            } else {
                let wrapped = format!("{open} {text} {close}");
                self.buffer.remove(start_off..end_off);
                self.buffer.insert(start_off, &wrapped);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Multi-cursor from selection ───────────────────────────────

    /// Adds a cursor at the start of each line in the current selection
    /// (Ctrl+Shift+L behavior for selections spanning multiple lines).
    pub fn add_cursor_at_each_selection_line(&mut self) {
        let primary = self.cursors.primary().selection;
        if primary.is_empty() {
            return;
        }
        let start_line = primary.start().line;
        let end_line = primary.end().line;
        if start_line == end_line {
            return;
        }
        self.cursors = MultiCursor::new(Position::new(start_line, 0));
        for line in (start_line + 1)..=end_line {
            self.cursors.add_cursor(Position::new(line, 0));
        }
    }

    // ── Text transforms ───────────────────────────────────────────

    /// Transforms selected text (or the word at cursor) to UPPERCASE.
    pub fn transform_to_uppercase(&mut self) {
        self.transform_text(str::to_uppercase);
    }

    /// Transforms selected text (or the word at cursor) to lowercase.
    pub fn transform_to_lowercase(&mut self) {
        self.transform_text(str::to_lowercase);
    }

    /// Transforms selected text (or the word at cursor) to Title Case.
    pub fn transform_to_title_case(&mut self) {
        self.transform_text(|s| {
            s.split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            let upper: String = first.to_uppercase().collect();
                            let rest: String = chars.as_str().to_lowercase();
                            format!("{upper}{rest}")
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        });
    }

    fn transform_text(&mut self, f: impl Fn(&str) -> String) {
        let before = self.cursor_selections();
        let mut sels: Vec<_> = self.cursors.cursors().iter().map(|c| c.selection).collect();
        sels.sort_by_key(|s| std::cmp::Reverse(s.start()));

        for sel in &sels {
            let (start_off, end_off) = if sel.is_empty() {
                let word_range = crate::word::word_at(&self.buffer, sel.active);
                (
                    self.buffer.position_to_offset(word_range.start),
                    self.buffer.position_to_offset(word_range.end),
                )
            } else {
                (
                    self.buffer.position_to_offset(sel.start()),
                    self.buffer.position_to_offset(sel.end()),
                )
            };
            if start_off < end_off {
                let text = self.buffer.slice(start_off..end_off);
                let transformed = f(&text);
                self.buffer.remove(start_off..end_off);
                self.buffer.insert(start_off, &transformed);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Whitespace ────────────────────────────────────────────────

    /// Trims trailing whitespace from all lines.
    pub fn trim_trailing_whitespace(&mut self) {
        let before = self.cursor_selections();
        let line_count = self.buffer.len_lines();

        for line_idx in (0..line_count).rev() {
            let content = self.buffer.line_content(line_idx);
            let trimmed = content.trim_end();
            let trailing = content.len() - trimmed.len();
            if trailing > 0 {
                let line_start = self.buffer.line_to_char(line_idx);
                let trim_start = line_start + trimmed.chars().count();
                let trim_end = line_start + content.chars().count();
                self.buffer.remove(trim_start..trim_end);
            }
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Insert lines above/below ──────────────────────────────────

    /// Inserts a blank line above the cursor (Ctrl+Shift+Enter).
    pub fn insert_line_above(&mut self) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for (offset, &line) in lines.iter().enumerate() {
            let actual = (line as usize) + offset;
            let insert_off = self.buffer.line_to_char(actual);
            self.buffer.insert(insert_off, "\n");
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Inserts a blank line below the cursor (Ctrl+Enter).
    pub fn insert_line_below(&mut self) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for (offset, &line) in lines.iter().enumerate() {
            let actual = (line as usize) + offset;
            let insert_off = if actual + 1 < self.buffer.len_lines() {
                self.buffer.line_to_char(actual + 1)
            } else {
                let end = self.buffer.len_chars();
                self.buffer.insert(end, "\n");
                end + 1
            };
            self.buffer.insert(insert_off, "\n");
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    // ── Copy line up/down ─────────────────────────────────────────

    /// Copies the current line up (Alt+Shift+Up).
    pub fn copy_line_up(&mut self) {
        let before = self.cursor_selections();
        let mut lines: Vec<u32> = self
            .cursors
            .cursors()
            .iter()
            .map(|c| c.position().line)
            .collect();
        lines.sort_unstable();
        lines.dedup();

        for &line in &lines {
            let content = self.buffer.line_content(line as usize);
            let line_start = self.buffer.line_to_char(line as usize);
            let insert_text = format!("{content}\n");
            self.buffer.insert(line_start, &insert_text);
        }

        let after = self.cursor_selections();
        self.undo_stack
            .push_barrier(EditGroup::empty(before, after));
        self.bump_version();
    }

    /// Copies the current line down (Alt+Shift+Down).
    pub fn copy_line_down(&mut self) {
        self.duplicate_line();
    }

    // ── Smart select (bracket matching) ───────────────────────────

    /// Expands the selection to the enclosing bracket/block pair.
    pub fn smart_select_grow(&mut self) {
        let sel = self.cursors.primary().selection;
        let start_off = self.buffer.position_to_offset(sel.start());
        let end_off = self.buffer.position_to_offset(sel.end());
        let text = self.buffer.text();
        let chars: Vec<char> = text.chars().collect();
        let total = chars.len();

        if let Some((new_start, new_end)) =
            find_enclosing_brackets(&chars, start_off, end_off, total)
        {
            let new_start_pos = self.buffer.offset_to_position(new_start);
            let new_end_pos = self.buffer.offset_to_position(new_end);
            self.cursors
                .set_primary_selection(Selection::new(new_start_pos, new_end_pos));
        }
    }

    /// Shrinks the selection by moving inward to the next inner bracket pair.
    pub fn smart_select_shrink(&mut self) {
        let sel = self.cursors.primary().selection;
        if sel.is_empty() {
            return;
        }
        let start_off = self.buffer.position_to_offset(sel.start());
        let end_off = self.buffer.position_to_offset(sel.end());
        let text = self.buffer.text();
        let chars: Vec<char> = text.chars().collect();

        if let Some((new_start, new_end)) = find_inner_brackets(&chars, start_off, end_off) {
            let new_start_pos = self.buffer.offset_to_position(new_start);
            let new_end_pos = self.buffer.offset_to_position(new_end);
            self.cursors
                .set_primary_selection(Selection::new(new_start_pos, new_end_pos));
        } else {
            let mid = usize::midpoint(start_off, end_off);
            let mid_pos = self.buffer.offset_to_position(mid);
            self.cursors
                .set_primary_selection(Selection::caret(mid_pos));
        }
    }

    // ── Word wrap toggle ──────────────────────────────────────────

    /// Toggles word wrap on/off.
    pub fn toggle_word_wrap(&mut self) {
        self.word_wrap = !self.word_wrap;
    }

    // ── Folding ───────────────────────────────────────────────────

    /// Returns line ranges to fold at the given indentation level.
    /// Each tuple is `(start_line, end_line)` inclusive of lines to hide.
    pub fn fold_at_level(&self, level: u32) -> Vec<(u32, u32)> {
        let target_indent = (level as usize) * 4;
        let line_count = self.buffer.len_lines();
        let mut regions = Vec::new();
        let mut i = 0;
        while i < line_count {
            let content = self.buffer.line_content(i);
            let indent: usize = content.chars().take_while(|c| *c == ' ').count();
            if indent == target_indent && !content.trim().is_empty() {
                let fold_start = i + 1;
                let mut fold_end = fold_start;
                while fold_end < line_count {
                    let next_content = self.buffer.line_content(fold_end);
                    let next_indent: usize = next_content.chars().take_while(|c| *c == ' ').count();
                    if next_indent <= target_indent && !next_content.trim().is_empty() {
                        break;
                    }
                    fold_end += 1;
                }
                if fold_end > fold_start {
                    regions.push((fold_start as u32, (fold_end - 1) as u32));
                    i = fold_end;
                    continue;
                }
            }
            i += 1;
        }
        regions
    }

    /// Returns an empty list, indicating all fold regions should be unfolded.
    pub fn unfold_all(&self) -> Vec<(u32, u32)> {
        Vec::new()
    }
}

fn find_enclosing_brackets(
    chars: &[char],
    start: usize,
    end: usize,
    total: usize,
) -> Option<(usize, usize)> {
    let mut open_pos = if start > 0 { start - 1 } else { return None };
    loop {
        if OPEN_BRACKETS.contains(&chars[open_pos]) {
            let close_idx = find_matching_close(chars, open_pos, total)?;
            if close_idx >= end {
                return Some((open_pos, close_idx + 1));
            }
        }
        if open_pos == 0 {
            return None;
        }
        open_pos -= 1;
    }
}

fn find_inner_brackets(chars: &[char], start: usize, end: usize) -> Option<(usize, usize)> {
    for i in start..end {
        if OPEN_BRACKETS.contains(&chars[i]) {
            if let Some(close) = find_matching_close(chars, i, chars.len()) {
                if close < end {
                    return Some((i + 1, close));
                }
            }
        }
    }
    None
}

fn find_matching_close(chars: &[char], open_pos: usize, total: usize) -> Option<usize> {
    let open = chars[open_pos];
    let close = matching_close_bracket(open)?;
    let mut depth = 1i32;
    for (i, &ch) in chars.iter().enumerate().take(total).skip(open_pos + 1) {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

/// Searches backward from `pos` for a matching open bracket.
fn find_matching_open_from(chars: &[char], pos: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 1i32;
    let mut i = pos;
    while i > 0 {
        i -= 1;
        if chars[i] == close {
            depth += 1;
        } else if chars[i] == open {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
    }
    None
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document() {
        let doc = Document::new();
        assert_eq!(doc.text(), "");
        assert_eq!(doc.version, 0);
        assert!(!doc.is_modified);
    }

    #[test]
    fn from_str() {
        let doc = Document::from_str("hello\nworld");
        assert_eq!(doc.text(), "hello\nworld");
        assert_eq!(doc.buffer.len_lines(), 2);
    }

    #[test]
    fn insert_text() {
        let mut doc = Document::new();
        doc.insert_text("hello");
        assert_eq!(doc.text(), "hello");
        assert!(doc.is_modified);
        assert!(doc.version > 0);
    }

    #[test]
    fn delete_left_basic() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 5));
        doc.delete_left();
        assert_eq!(doc.text(), "hell");
    }

    #[test]
    fn delete_right_basic() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.delete_right();
        assert_eq!(doc.text(), "ello");
    }

    #[test]
    fn delete_line() {
        let mut doc = Document::from_str("line1\nline2\nline3");
        doc.cursors = MultiCursor::new(Position::new(1, 0));
        doc.delete_line();
        assert_eq!(doc.text(), "line1\nline3");
    }

    #[test]
    fn new_line() {
        let mut doc = Document::from_str("ab");
        doc.cursors = MultiCursor::new(Position::new(0, 1));
        doc.new_line();
        assert!(doc.text().contains('\n'));
    }

    #[test]
    fn select_all() {
        let mut doc = Document::from_str("hello\nworld");
        doc.select_all();
        let sel = doc.cursors.primary().selection;
        assert_eq!(sel.start(), Position::ZERO);
        assert_eq!(sel.end(), Position::new(1, 5));
    }

    #[test]
    fn toggle_line_comment_add() {
        let mut doc = Document::from_str("hello\nworld");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.toggle_line_comment("//");
        assert!(doc.text().starts_with("// hello"));
    }

    #[test]
    fn toggle_line_comment_remove() {
        let mut doc = Document::from_str("// hello\n// world");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.toggle_line_comment("//");
        assert_eq!(doc.buffer.line_content(0), "hello");
    }

    #[test]
    fn outdent() {
        let mut doc = Document::from_str("    hello");
        doc.cursors = MultiCursor::new(Position::new(0, 4));
        doc.outdent();
        assert_eq!(doc.text(), "hello");
    }

    #[test]
    fn version_increments() {
        let mut doc = Document::new();
        assert_eq!(doc.version, 0);
        doc.insert_text("a");
        let v1 = doc.version;
        doc.insert_text("b");
        assert!(doc.version > v1);
    }

    #[test]
    fn default_is_empty() {
        let doc = Document::default();
        assert_eq!(doc.text(), "");
    }

    // ── New command tests ─────────────────────────────────────────

    #[test]
    fn auto_close_bracket() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 5));
        doc.auto_close_bracket('(');
        assert_eq!(doc.text(), "hello()");
    }

    #[test]
    fn auto_close_bracket_curly() {
        let mut doc = Document::from_str("fn main");
        doc.cursors = MultiCursor::new(Position::new(0, 7));
        doc.auto_close_bracket('{');
        assert_eq!(doc.text(), "fn main{}");
    }

    #[test]
    fn auto_close_quote() {
        let mut doc = Document::from_str("let x = ");
        doc.cursors = MultiCursor::new(Position::new(0, 8));
        doc.auto_close_quote('"');
        assert_eq!(doc.text(), "let x = \"\"");
    }

    #[test]
    fn auto_surround() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 5)));
        doc.auto_surround('(', ')');
        assert_eq!(doc.text(), "(hello)");
    }

    #[test]
    fn auto_surround_empty_selection_is_noop() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 2));
        doc.auto_surround('[', ']');
        assert_eq!(doc.text(), "hello");
    }

    #[test]
    fn new_line_with_indent_basic() {
        let mut doc = Document::from_str("    hello");
        doc.cursors = MultiCursor::new(Position::new(0, 9));
        doc.new_line_with_indent();
        let text = doc.text();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].starts_with("    "));
    }

    #[test]
    fn new_line_with_indent_after_brace() {
        let mut doc = Document::from_str("fn main() {");
        doc.cursors = MultiCursor::new(Position::new(0, 11));
        doc.new_line_with_indent();
        let text = doc.text();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].starts_with("    "));
    }

    #[test]
    fn transpose_characters() {
        let mut doc = Document::from_str("abcd");
        doc.cursors = MultiCursor::new(Position::new(0, 2));
        doc.transpose_characters();
        assert_eq!(doc.text(), "acbd");
    }

    #[test]
    fn transpose_characters_at_end() {
        let mut doc = Document::from_str("abcd");
        doc.cursors = MultiCursor::new(Position::new(0, 4));
        doc.transpose_characters();
        assert_eq!(doc.text(), "abdc");
    }

    #[test]
    fn join_lines() {
        let mut doc = Document::from_str("hello\nworld");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.join_lines();
        assert_eq!(doc.text(), "hello world");
    }

    #[test]
    fn join_lines_trims_leading_whitespace() {
        let mut doc = Document::from_str("hello\n    world");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.join_lines();
        assert_eq!(doc.text(), "hello world");
    }

    #[test]
    fn sort_lines_ascending() {
        let mut doc = Document::from_str("cherry\napple\nbanana");
        doc.sort_lines_ascending();
        assert_eq!(doc.text(), "apple\nbanana\ncherry");
    }

    #[test]
    fn sort_lines_descending() {
        let mut doc = Document::from_str("apple\nbanana\ncherry");
        doc.sort_lines_descending();
        assert_eq!(doc.text(), "cherry\nbanana\napple");
    }

    #[test]
    fn delete_all_left() {
        let mut doc = Document::from_str("hello world");
        doc.cursors = MultiCursor::new(Position::new(0, 5));
        doc.delete_all_left();
        assert_eq!(doc.text(), " world");
    }

    #[test]
    fn delete_all_right() {
        let mut doc = Document::from_str("hello world");
        doc.cursors = MultiCursor::new(Position::new(0, 5));
        doc.delete_all_right();
        assert_eq!(doc.text(), "hello");
    }

    #[test]
    fn toggle_block_comment_add() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 5)));
        doc.toggle_block_comment("/*", "*/");
        assert_eq!(doc.text(), "/* hello */");
    }

    #[test]
    fn toggle_block_comment_remove() {
        let mut doc = Document::from_str("/* hello */");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 11)));
        doc.toggle_block_comment("/*", "*/");
        assert_eq!(doc.text(), "hello");
    }

    #[test]
    fn transform_to_uppercase() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 5)));
        doc.transform_to_uppercase();
        assert_eq!(doc.text(), "HELLO");
    }

    #[test]
    fn transform_to_lowercase() {
        let mut doc = Document::from_str("HELLO");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 5)));
        doc.transform_to_lowercase();
        assert_eq!(doc.text(), "hello");
    }

    #[test]
    fn transform_to_title_case() {
        let mut doc = Document::from_str("hello world");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 11)));
        doc.transform_to_title_case();
        assert_eq!(doc.text(), "Hello World");
    }

    #[test]
    fn trim_trailing_whitespace() {
        let mut doc = Document::from_str("hello   \nworld  ");
        doc.trim_trailing_whitespace();
        assert_eq!(doc.text(), "hello\nworld");
    }

    #[test]
    fn insert_line_above() {
        let mut doc = Document::from_str("line1\nline2");
        doc.cursors = MultiCursor::new(Position::new(1, 0));
        doc.insert_line_above();
        assert_eq!(doc.buffer.len_lines(), 3);
        assert_eq!(doc.buffer.line_content(0), "line1");
        assert_eq!(doc.buffer.line_content(1), "");
    }

    #[test]
    fn insert_line_below() {
        let mut doc = Document::from_str("line1\nline2");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.insert_line_below();
        assert!(doc.buffer.len_lines() >= 3);
    }

    #[test]
    fn copy_line_up() {
        let mut doc = Document::from_str("aaa\nbbb\nccc");
        doc.cursors = MultiCursor::new(Position::new(1, 0));
        doc.copy_line_up();
        assert_eq!(doc.buffer.line_content(1), "bbb");
        assert_eq!(doc.buffer.line_content(2), "bbb");
    }

    #[test]
    fn copy_line_down() {
        let mut doc = Document::from_str("aaa\nbbb\nccc");
        doc.cursors = MultiCursor::new(Position::new(1, 0));
        doc.copy_line_down();
        let text = doc.text();
        let lines: Vec<_> = text.lines().collect();
        assert!(lines.len() >= 4);
    }

    #[test]
    fn smart_select_grow() {
        let mut doc = Document::from_str("fn main() { hello }");
        doc.cursors = MultiCursor::new(Position::new(0, 14));
        doc.smart_select_grow();
        let sel = doc.cursors.primary().selection;
        assert!(sel.start() <= Position::new(0, 11));
    }

    #[test]
    fn toggle_word_wrap() {
        let mut doc = Document::new();
        assert!(!doc.word_wrap);
        doc.toggle_word_wrap();
        assert!(doc.word_wrap);
        doc.toggle_word_wrap();
        assert!(!doc.word_wrap);
    }

    #[test]
    fn fold_at_level() {
        let doc = Document::from_str("top\n    nested\n    nested2\ntop2");
        let regions = doc.fold_at_level(0);
        assert!(!regions.is_empty());
    }

    #[test]
    fn unfold_all_empty() {
        let doc = Document::from_str("anything");
        assert!(doc.unfold_all().is_empty());
    }

    #[test]
    fn add_cursor_at_each_selection_line() {
        let mut doc = Document::from_str("aaa\nbbb\nccc\nddd");
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(2, 3)));
        doc.add_cursor_at_each_selection_line();
        assert_eq!(doc.cursors.len(), 3);
    }

    // ── type_char dispatcher tests ────────────────────────────────

    #[test]
    fn type_char_auto_close_bracket() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 5));
        doc.type_char('(');
        assert_eq!(doc.text(), "hello()");
        assert_eq!(doc.cursors.primary().position(), Position::new(0, 6));
    }

    #[test]
    fn type_char_auto_close_square_bracket() {
        let mut doc = Document::from_str("arr");
        doc.cursors = MultiCursor::new(Position::new(0, 3));
        doc.type_char('[');
        assert_eq!(doc.text(), "arr[]");
        assert_eq!(doc.cursors.primary().position(), Position::new(0, 4));
    }

    #[test]
    fn type_char_auto_close_curly() {
        let mut doc = Document::from_str("fn main()");
        doc.cursors = MultiCursor::new(Position::new(0, 9));
        doc.type_char(' ');
        doc.type_char('{');
        assert_eq!(doc.text(), "fn main() {}");
    }

    #[test]
    fn type_char_auto_close_quote_double() {
        let mut doc = Document::from_str("let x = ");
        doc.cursors = MultiCursor::new(Position::new(0, 8));
        doc.type_char('"');
        assert_eq!(doc.text(), "let x = \"\"");
        assert_eq!(doc.cursors.primary().position(), Position::new(0, 9));
    }

    #[test]
    fn type_char_auto_close_quote_single() {
        let mut doc = Document::from_str("let x = ");
        doc.cursors = MultiCursor::new(Position::new(0, 8));
        doc.type_char('\'');
        assert_eq!(doc.text(), "let x = ''");
    }

    #[test]
    fn type_char_auto_close_backtick() {
        let mut doc = Document::from_str("let s = ");
        doc.cursors = MultiCursor::new(Position::new(0, 8));
        doc.type_char('`');
        assert_eq!(doc.text(), "let s = ``");
    }

    #[test]
    fn type_char_overtype_closing_bracket() {
        let mut doc = Document::from_str("hello()");
        doc.cursors = MultiCursor::new(Position::new(0, 6));
        doc.type_char(')');
        assert_eq!(doc.text(), "hello()");
        assert_eq!(doc.cursors.primary().position(), Position::new(0, 7));
    }

    #[test]
    fn type_char_overtype_closing_quote() {
        let mut doc = Document::from_str("\"hello\"");
        doc.cursors = MultiCursor::new(Position::new(0, 6));
        doc.type_char('"');
        assert_eq!(doc.text(), "\"hello\"");
        assert_eq!(doc.cursors.primary().position(), Position::new(0, 7));
    }

    #[test]
    fn type_char_surround_selection_with_bracket() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 5)));
        doc.type_char('(');
        assert_eq!(doc.text(), "(hello)");
    }

    #[test]
    fn type_char_surround_selection_with_quote() {
        let mut doc = Document::from_str("hello");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 5)));
        doc.type_char('"');
        assert_eq!(doc.text(), "\"hello\"");
    }

    #[test]
    fn type_char_surround_selection_with_curly() {
        let mut doc = Document::from_str("world");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 5)));
        doc.type_char('{');
        assert_eq!(doc.text(), "{world}");
    }

    #[test]
    fn type_char_no_autoclose_after_word_char_for_quote() {
        let mut doc = Document::from_str("don");
        doc.cursors = MultiCursor::new(Position::new(0, 3));
        doc.type_char('\'');
        assert_eq!(doc.text(), "don'");
    }

    // ── Enter with auto-indent tests ──────────────────────────────

    #[test]
    fn enter_indent_outdent_braces() {
        let mut doc = Document::from_str("fn main() {}");
        doc.cursors = MultiCursor::new(Position::new(0, 12));
        doc.cursors.primary_mut().selection = Selection::caret(Position::new(0, 12));
        let mut doc2 = Document::from_str("fn main() {}");
        doc2.cursors = MultiCursor::new(Position::new(0, 12));
        // Place cursor between { and }
        doc2.cursors.cursors_mut()[0].selection = Selection::caret(Position::new(0, 12));
        // Manually set cursor between braces
        let mut doc3 = Document::from_str("{}");
        doc3.cursors = MultiCursor::new(Position::new(0, 1));
        doc3.new_line_with_indent();
        let text = doc3.text();
        assert!(text.contains("{\n"), "Should have newline after open brace");
        let lines: Vec<_> = text.lines().collect();
        assert!(
            lines.len() >= 3,
            "Should have at least 3 lines for indent-outdent"
        );
        assert_eq!(lines[2].trim(), "}", "Last line should have closing brace");
    }

    #[test]
    fn enter_preserves_indentation() {
        let mut doc = Document::from_str("    hello");
        doc.cursors = MultiCursor::new(Position::new(0, 9));
        doc.new_line_with_indent();
        let text = doc.text();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[1].starts_with("    "),
            "Second line should have same indent"
        );
    }

    #[test]
    fn enter_extra_indent_after_open_bracket() {
        let mut doc = Document::from_str("if (true) {");
        doc.cursors = MultiCursor::new(Position::new(0, 11));
        doc.new_line_with_indent();
        let text = doc.text();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[1].starts_with("    "),
            "Should have extra indent after {{"
        );
    }

    // ── Tab handling tests ────────────────────────────────────────

    #[test]
    fn tab_inserts_to_next_stop() {
        let mut doc = Document::from_str("ab");
        doc.cursors = MultiCursor::new(Position::new(0, 2));
        doc.tab();
        assert_eq!(doc.text(), "ab  ");
    }

    #[test]
    fn tab_inserts_full_tab_at_col_0() {
        let mut doc = Document::from_str("x");
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.tab();
        assert_eq!(doc.text(), "    x");
    }

    #[test]
    fn tab_indents_multiline_selection() {
        let mut doc = Document::from_str("aaa\nbbb\nccc");
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(2, 3)));
        doc.tab();
        let text = doc.text();
        let lines: Vec<_> = text.lines().collect();
        assert!(lines[0].starts_with("    "));
        assert!(lines[1].starts_with("    "));
        assert!(lines[2].starts_with("    "));
    }

    // ── Backspace special case tests ──────────────────────────────

    #[test]
    fn backspace_deletes_bracket_pair() {
        let mut doc = Document::from_str("hello()");
        doc.cursors = MultiCursor::new(Position::new(0, 6));
        doc.delete_left();
        assert_eq!(doc.text(), "hello");
    }

    #[test]
    fn backspace_deletes_quote_pair() {
        let mut doc = Document::from_str("let x = \"\"");
        doc.cursors = MultiCursor::new(Position::new(0, 9));
        doc.delete_left();
        assert_eq!(doc.text(), "let x = ");
    }

    #[test]
    fn backspace_unindent_to_tab_stop() {
        let mut doc = Document::from_str("      x");
        // cursor at col 6, within indentation (6 spaces)
        doc.cursors = MultiCursor::new(Position::new(0, 6));
        doc.delete_left();
        // Should snap to previous tab stop (col 4)
        assert_eq!(doc.text(), "    x");
    }

    #[test]
    fn backspace_unindent_from_tab_boundary() {
        let mut doc = Document::from_str("    x");
        doc.cursors = MultiCursor::new(Position::new(0, 4));
        doc.delete_left();
        assert_eq!(doc.text(), "x");
    }

    // ── Paste with auto-indent tests ──────────────────────────────

    #[test]
    fn paste_single_line() {
        let mut doc = Document::from_str("hello ");
        doc.cursors = MultiCursor::new(Position::new(0, 6));
        doc.paste_with_indent("world");
        assert_eq!(doc.text(), "hello world");
    }

    #[test]
    fn paste_multiline_adjusts_indent() {
        let mut doc = Document::from_str("    fn foo() {\n        ");
        doc.cursors = MultiCursor::new(Position::new(1, 8));
        doc.paste_with_indent("let x = 1;\nlet y = 2;");
        let text = doc.text();
        let lines: Vec<_> = text.lines().collect();
        assert!(lines.len() >= 3);
    }

    #[test]
    fn paste_distributed_to_multi_cursors() {
        let mut doc = Document::from_str("aaa\nbbb\nccc");
        doc.cursors = MultiCursor::new(Position::new(0, 3));
        doc.cursors.add_cursor(Position::new(1, 3));
        doc.cursors.add_cursor(Position::new(2, 3));
        doc.paste_with_indent("1\n2\n3");
        let text = doc.text();
        assert!(text.contains("aaa1"));
        assert!(text.contains("bbb2"));
        assert!(text.contains("ccc3"));
    }

    // ── Composition / IME tests ───────────────────────────────────

    #[test]
    fn composition_basic() {
        let mut doc = Document::from_str("hello ");
        doc.cursors = MultiCursor::new(Position::new(0, 6));
        doc.composition_start();
        assert!(doc.is_composing);
        doc.composition_type("日", 0, 0);
        doc.composition_end();
        assert!(!doc.is_composing);
        assert!(doc.text().contains('日'));
    }

    #[test]
    fn composition_replace_prev() {
        let mut doc = Document::from_str("ni");
        doc.cursors = MultiCursor::new(Position::new(0, 2));
        doc.composition_start();
        doc.composition_type("你", 2, 0);
        doc.composition_end();
        assert_eq!(doc.text(), "你");
    }

    // ── Electric character tests ──────────────────────────────────

    #[test]
    fn electric_char_outdent_closing_brace() {
        let mut doc = Document::from_str("fn main() {\n        \n}");
        // Cursor on line 1, which has 8 spaces of indent
        // Simulating typing '}' which should re-indent to match 'fn main() {'
        doc.cursors = MultiCursor::new(Position::new(1, 8));
        doc.type_char('}');
        let text = doc.text();
        let lines: Vec<_> = text.lines().collect();
        // The '}' line should be indented to match the opening line (0 indent)
        assert_eq!(lines[1].trim(), "}");
    }

    // ── Multi-cursor type_char tests ──────────────────────────────

    #[test]
    fn multi_cursor_type_char() {
        let mut doc = Document::from_str("aaa\nbbb\nccc");
        doc.cursors = MultiCursor::new(Position::new(0, 3));
        doc.cursors.add_cursor(Position::new(1, 3));
        doc.cursors.add_cursor(Position::new(2, 3));
        doc.type_char('!');
        let text = doc.text();
        assert!(text.contains("aaa!"));
        assert!(text.contains("bbb!"));
        assert!(text.contains("ccc!"));
    }

    #[test]
    fn multi_cursor_auto_close_bracket() {
        let mut doc = Document::from_str("a\nb\nc");
        doc.cursors = MultiCursor::new(Position::new(0, 1));
        doc.cursors.add_cursor(Position::new(1, 1));
        doc.cursors.add_cursor(Position::new(2, 1));
        doc.type_char('(');
        let text = doc.text();
        assert!(text.contains("a()"));
        assert!(text.contains("b()"));
        assert!(text.contains("c()"));
    }

    #[test]
    fn multi_cursor_backspace_bracket_pair() {
        let mut doc = Document::from_str("a()\nb()\nc()");
        doc.cursors = MultiCursor::new(Position::new(0, 2));
        doc.cursors.add_cursor(Position::new(1, 2));
        doc.cursors.add_cursor(Position::new(2, 2));
        doc.delete_left();
        assert_eq!(doc.text(), "a\nb\nc");
    }

    // ── EditorConfig tests ────────────────────────────────────────

    #[test]
    fn tab_with_real_tabs() {
        let mut doc = Document::from_str("x");
        doc.config.insert_spaces = false;
        doc.cursors = MultiCursor::new(Position::new(0, 0));
        doc.tab();
        assert_eq!(doc.text(), "\tx");
    }

    #[test]
    fn autoclose_disabled() {
        let mut doc = Document::from_str("hello");
        doc.config.auto_closing_brackets = AutoClosingStrategy::Never;
        doc.cursors = MultiCursor::new(Position::new(0, 5));
        doc.type_char('(');
        assert_eq!(doc.text(), "hello(");
    }

    #[test]
    fn surround_disabled() {
        let mut doc = Document::from_str("hello");
        doc.config.auto_surround = AutoSurroundStrategy::Never;
        doc.cursors
            .set_primary_selection(Selection::new(Position::new(0, 0), Position::new(0, 5)));
        doc.type_char('(');
        // With surround disabled, should just type the char
        assert!(doc.text().starts_with('('));
    }
}
