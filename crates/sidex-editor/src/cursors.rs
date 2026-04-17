//! Extended multi-cursor system with full VS Code parity: column/box selection,
//! find-match cursors, cursor undo history, smart select, and unified direction
//! movement across all cursors.

use sidex_text::{Buffer, Position};

use crate::cursor::CursorState;
use crate::multi_cursor::MultiCursor;
use crate::selection::Selection;
use crate::word::word_at;

/// Direction for cursor movement operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

/// Direction of a selection (which end is the anchor vs active).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionDirection {
    LeftToRight,
    RightToLeft,
}

/// A snapshot of cursor state for the cursor-undo stack.
#[derive(Debug, Clone)]
struct CursorSnapshot {
    cursors: Vec<CursorState>,
    #[allow(dead_code)]
    primary_idx: usize,
}

/// Extended multi-cursor controller layered on top of [`MultiCursor`].
///
/// Adds: unified direction movement, column/box selection, find-match cursors,
/// cursor-level undo (Ctrl+U), expand/shrink selection, and add-cursors-to-line-ends.
#[derive(Debug, Clone)]
pub struct CursorController {
    pub inner: MultiCursor,
    cursor_history: Vec<CursorSnapshot>,
    max_history: usize,
}

impl CursorController {
    /// Creates a new controller with a single cursor at the given position.
    #[must_use]
    pub fn new(pos: Position) -> Self {
        Self {
            inner: MultiCursor::new(pos),
            cursor_history: Vec::new(),
            max_history: 64,
        }
    }

    /// Wraps an existing `MultiCursor`.
    #[must_use]
    pub fn from_multi_cursor(mc: MultiCursor) -> Self {
        Self {
            inner: mc,
            cursor_history: Vec::new(),
            max_history: 64,
        }
    }

    fn push_history(&mut self) {
        let snapshot = CursorSnapshot {
            cursors: self.inner.cursors().to_vec(),
            primary_idx: self
                .inner
                .cursors()
                .iter()
                .enumerate()
                .find(|(_, c)| std::ptr::eq(*c, self.inner.primary()))
                .map_or(0, |(i, _)| i),
        };
        if self.cursor_history.len() >= self.max_history {
            self.cursor_history.remove(0);
        }
        self.cursor_history.push(snapshot);
    }

    // ── Cursor addition ───────────────────────────────────────────

    /// Adds a cursor on the line above the primary cursor (Ctrl+Alt+Up).
    pub fn add_cursor_above(&mut self, buffer: &Buffer) {
        self.push_history();
        self.inner.add_cursor_above(buffer);
    }

    /// Adds a cursor on the line below the primary cursor (Ctrl+Alt+Down).
    pub fn add_cursor_below(&mut self, buffer: &Buffer) {
        self.push_history();
        self.inner.add_cursor_below(buffer);
    }

    /// Adds a cursor at an arbitrary position.
    pub fn add_cursor_at_position(&mut self, pos: Position) {
        self.push_history();
        self.inner.add_cursor(pos);
    }

    /// Removes the cursor at `index`, if more than one cursor exists.
    pub fn remove_cursor(&mut self, index: usize) {
        if self.inner.len() <= 1 || index >= self.inner.len() {
            return;
        }
        self.push_history();
        let mut cursors: Vec<CursorState> = self.inner.cursors().to_vec();
        cursors.remove(index);
        self.inner = MultiCursor::new(cursors[0].position());
        for c in cursors.iter().skip(1) {
            self.inner.add_cursor(c.position());
        }
    }

    /// Merges any overlapping cursors after an operation.
    pub fn merge_overlapping_cursors(&mut self) {
        self.inner.merge_overlapping();
    }

    /// Undoes the last cursor operation (Ctrl+U).
    pub fn undo_last_cursor_operation(&mut self) {
        if let Some(snapshot) = self.cursor_history.pop() {
            if snapshot.cursors.is_empty() {
                return;
            }
            self.inner = MultiCursor::new(snapshot.cursors[0].position());
            if !snapshot.cursors[0].selection.is_empty() {
                self.inner
                    .set_primary_selection(snapshot.cursors[0].selection);
            }
            for c in snapshot.cursors.iter().skip(1) {
                self.inner.add_cursor(c.position());
            }
        }
    }

    // ── Unified movement ──────────────────────────────────────────

    /// Moves all cursors in the given direction. When `select` is true, the
    /// selection is extended; when `word` is true, movement jumps by word
    /// boundaries.
    pub fn move_cursors(
        &mut self,
        direction: Direction,
        select: bool,
        word: bool,
        buffer: &Buffer,
        viewport_lines: u32,
    ) {
        match direction {
            Direction::Left => {
                if word {
                    self.inner.move_all_word_left(buffer, select);
                } else {
                    self.inner.move_all_left(buffer, select);
                }
            }
            Direction::Right => {
                if word {
                    self.inner.move_all_word_right(buffer, select);
                } else {
                    self.inner.move_all_right(buffer, select);
                }
            }
            Direction::Up => self.inner.move_all_up(buffer, select),
            Direction::Down => self.inner.move_all_down(buffer, select),
            Direction::Home => self.inner.move_all_to_line_start(buffer, select),
            Direction::End => self.inner.move_all_to_line_end(buffer, select),
            Direction::PageUp => self.inner.move_all_page_up(buffer, viewport_lines, select),
            Direction::PageDown => {
                self.inner
                    .move_all_page_down(buffer, viewport_lines, select);
            }
            Direction::Top => self.inner.move_all_to_buffer_start(buffer, select),
            Direction::Bottom => self.inner.move_all_to_buffer_end(buffer, select),
        }
    }

    // ── Find-match cursors ────────────────────────────────────────

    /// Adds a selection to the next occurrence of the current primary selection
    /// text (Ctrl+D). If the primary cursor has no selection, selects the word
    /// at cursor first.
    pub fn add_selection_to_next_find_match(&mut self, buffer: &Buffer) {
        self.push_history();

        let primary_sel = self.inner.primary().selection;
        let search = if primary_sel.is_empty() {
            let range = word_at(buffer, primary_sel.active);
            let sel = Selection::new(range.start, range.end);
            self.inner.set_primary_selection(sel);
            buffer.slice(
                buffer.position_to_offset(range.start)..buffer.position_to_offset(range.end),
            )
        } else {
            let s = buffer.position_to_offset(primary_sel.start());
            let e = buffer.position_to_offset(primary_sel.end());
            buffer.slice(s..e)
        };

        if search.is_empty() {
            return;
        }

        let last_cursor_end = self
            .inner
            .cursors()
            .iter()
            .map(|c| c.selection.end())
            .max()
            .unwrap_or(Position::ZERO);

        let text = buffer.text();
        let search_after_offset = buffer.position_to_offset(last_cursor_end);
        let search_len = search.chars().count() as u32;

        let found = text[search_after_offset..]
            .find(&search)
            .map(|rel| search_after_offset + text[search_after_offset..][..rel].chars().count())
            .or_else(|| {
                text[..search_after_offset]
                    .find(&search)
                    .map(|rel| text[..rel].chars().count())
            });

        if let Some(char_offset) = found {
            let start_pos = buffer.offset_to_position(char_offset);
            let end_pos = Position::new(start_pos.line, start_pos.column + search_len);
            let already_has = self
                .inner
                .cursors()
                .iter()
                .any(|c| c.selection.start() == start_pos && c.selection.end() == end_pos);
            if !already_has {
                self.inner.add_cursor(start_pos);
                let idx = self
                    .inner
                    .cursors()
                    .iter()
                    .position(|c| c.position() == start_pos)
                    .unwrap_or(0);
                self.inner.cursors_mut()[idx].selection = Selection::new(start_pos, end_pos);
                self.inner.merge_overlapping();
            }
        }
    }

    /// Creates cursors at all occurrences of the given text (Ctrl+Shift+L).
    pub fn select_all_occurrences(&mut self, buffer: &Buffer, search: &str) {
        self.push_history();
        self.inner.select_all_occurrences(buffer, search);
    }

    // ── Word/line selection ───────────────────────────────────────

    /// Selects the word at every cursor position (double-click or Ctrl+D with
    /// no selection).
    pub fn select_word_at_cursor(&mut self, buffer: &Buffer) {
        for cursor in self.inner.cursors_mut() {
            if cursor.selection.is_empty() {
                let range = word_at(buffer, cursor.position());
                cursor.selection = Selection::new(range.start, range.end);
                cursor.preferred_column = None;
            }
        }
        self.inner.merge_overlapping();
    }

    /// Selects the entire line at every cursor position (triple-click or
    /// Ctrl+L).
    pub fn select_line(&mut self, buffer: &Buffer) {
        for cursor in self.inner.cursors_mut() {
            let line = cursor.position().line;
            let start = Position::new(line, 0);
            let end_col = buffer.line_content_len(line as usize) as u32;
            cursor.selection = Selection::new(start, Position::new(line, end_col));
            cursor.preferred_column = None;
        }
        self.inner.merge_overlapping();
    }

    // ── Smart select (expand/shrink) ──────────────────────────────

    /// Expands the primary cursor's selection outward to the next enclosing
    /// scope (Shift+Alt+Right).
    pub fn expand_selection(&mut self, buffer: &Buffer) {
        self.push_history();
        let sel = self.inner.primary().selection;
        let start_off = buffer.position_to_offset(sel.start());
        let end_off = buffer.position_to_offset(sel.end());

        if sel.is_empty() {
            let range = word_at(buffer, sel.active);
            if range.start != range.end {
                self.inner
                    .set_primary_selection(Selection::new(range.start, range.end));
                return;
            }
        }

        let line = sel.start().line;
        let line_start = Position::new(line, 0);
        let line_end_col = buffer.line_content_len(line as usize) as u32;
        let line_end = Position::new(line, line_end_col);

        if sel.start() != line_start || sel.end() != line_end {
            self.inner
                .set_primary_selection(Selection::new(line_start, line_end));
            return;
        }

        let text = buffer.text();
        let chars: Vec<char> = text.chars().collect();
        let total = chars.len();
        if let Some((ns, ne)) = find_enclosing_brackets(&chars, start_off, end_off, total) {
            let ns_pos = buffer.offset_to_position(ns);
            let ne_pos = buffer.offset_to_position(ne);
            self.inner
                .set_primary_selection(Selection::new(ns_pos, ne_pos));
            return;
        }

        let last_line = (buffer.len_lines() - 1) as u32;
        let last_col = buffer.line_content_len(last_line as usize) as u32;
        self.inner.set_primary_selection(Selection::new(
            Position::ZERO,
            Position::new(last_line, last_col),
        ));
    }

    /// Shrinks the primary cursor's selection inward (Shift+Alt+Left).
    pub fn shrink_selection(&mut self) {
        if let Some(snapshot) = self.cursor_history.pop() {
            if snapshot.cursors.is_empty() {
                return;
            }
            self.inner = MultiCursor::new(snapshot.cursors[0].position());
            if !snapshot.cursors[0].selection.is_empty() {
                self.inner
                    .set_primary_selection(snapshot.cursors[0].selection);
            }
            for c in snapshot.cursors.iter().skip(1) {
                self.inner.add_cursor(c.position());
            }
        }
    }

    // ── Column / box selection ────────────────────────────────────

    /// Creates a cursor on each line within the rectangle defined by `start`
    /// and `end`, placing each cursor at the column of `end`
    /// (Shift+Alt+drag or Ctrl+Shift+Alt+Arrow).
    pub fn column_select(&mut self, start: Position, end: Position, buffer: &Buffer) {
        self.push_history();
        let min_line = start.line.min(end.line);
        let max_line = start.line.max(end.line);
        let min_col = start.column.min(end.column);
        let max_col = start.column.max(end.column);

        let first_line = min_line;
        let first_line_len = buffer.line_content_len(first_line as usize) as u32;
        let first_col = min_col.min(first_line_len);

        self.inner = MultiCursor::new(Position::new(first_line, first_col));
        if min_col != max_col {
            let end_col = max_col.min(first_line_len);
            self.inner.set_primary_selection(Selection::new(
                Position::new(first_line, first_col),
                Position::new(first_line, end_col),
            ));
        }

        for line in (min_line + 1)..=max_line {
            let line_len = buffer.line_content_len(line as usize) as u32;
            let col = min_col.min(line_len);
            self.inner.add_cursor(Position::new(line, col));
            if min_col != max_col {
                let end_col = max_col.min(line_len);
                let idx = self.inner.len() - 1;
                self.inner.cursors_mut()[idx].selection = Selection::new(
                    Position::new(line, col),
                    Position::new(line, end_col),
                );
            }
        }
    }

    // ── Convenience accessors ─────────────────────────────────────

    /// Returns a reference to the underlying `MultiCursor`.
    #[must_use]
    pub fn multi_cursor(&self) -> &MultiCursor {
        &self.inner
    }

    /// Returns the number of cursors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if there are no cursors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the primary cursor state.
    #[must_use]
    pub fn primary(&self) -> &CursorState {
        self.inner.primary()
    }

    /// Collapses to only the primary cursor (Escape).
    pub fn collapse_to_primary(&mut self) {
        self.push_history();
        self.inner.collapse_to_primary();
    }

    /// Returns a slice of all cursor states.
    #[must_use]
    pub fn cursors(&self) -> &[CursorState] {
        self.inner.cursors()
    }

    /// Determines the direction of a selection.
    #[must_use]
    pub fn selection_direction(sel: &Selection) -> SelectionDirection {
        if sel.anchor <= sel.active {
            SelectionDirection::LeftToRight
        } else {
            SelectionDirection::RightToLeft
        }
    }
}

// ── Bracket helpers (shared with document.rs smart-select) ────────

const OPEN_BRACKETS: &[char] = &['(', '[', '{'];
const CLOSE_BRACKETS: &[char] = &[')', ']', '}'];

fn matching_close_bracket(open: char) -> Option<char> {
    OPEN_BRACKETS
        .iter()
        .zip(CLOSE_BRACKETS.iter())
        .find(|(&o, _)| o == open)
        .map(|(_, &c)| c)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(s: &str) -> Buffer {
        Buffer::from_str(s)
    }

    #[test]
    fn new_controller() {
        let ctrl = CursorController::new(Position::new(0, 0));
        assert_eq!(ctrl.len(), 1);
        assert_eq!(ctrl.primary().position(), Position::new(0, 0));
    }

    #[test]
    fn add_cursor_above_below() {
        let b = buf("aaa\nbbb\nccc");
        let mut ctrl = CursorController::new(Position::new(1, 1));
        ctrl.add_cursor_above(&b);
        assert_eq!(ctrl.len(), 2);
        ctrl.add_cursor_below(&b);
        assert!(ctrl.len() >= 2);
    }

    #[test]
    fn add_cursor_at_position() {
        let mut ctrl = CursorController::new(Position::new(0, 0));
        ctrl.add_cursor_at_position(Position::new(5, 3));
        assert_eq!(ctrl.len(), 2);
    }

    #[test]
    fn remove_cursor() {
        let mut ctrl = CursorController::new(Position::new(0, 0));
        ctrl.add_cursor_at_position(Position::new(1, 0));
        ctrl.add_cursor_at_position(Position::new(2, 0));
        assert_eq!(ctrl.len(), 3);
        ctrl.remove_cursor(1);
        assert_eq!(ctrl.len(), 2);
    }

    #[test]
    fn remove_cursor_single_is_noop() {
        let mut ctrl = CursorController::new(Position::new(0, 0));
        ctrl.remove_cursor(0);
        assert_eq!(ctrl.len(), 1);
    }

    #[test]
    fn undo_last_cursor_operation() {
        let b = buf("aaa\nbbb\nccc");
        let mut ctrl = CursorController::new(Position::new(1, 1));
        assert_eq!(ctrl.len(), 1);
        ctrl.add_cursor_above(&b);
        assert_eq!(ctrl.len(), 2);
        ctrl.undo_last_cursor_operation();
        assert_eq!(ctrl.len(), 1);
    }

    #[test]
    fn move_cursors_left_right() {
        let b = buf("hello world");
        let mut ctrl = CursorController::new(Position::new(0, 5));
        ctrl.move_cursors(Direction::Left, false, false, &b, 20);
        assert_eq!(ctrl.primary().position(), Position::new(0, 4));
        ctrl.move_cursors(Direction::Right, false, false, &b, 20);
        assert_eq!(ctrl.primary().position(), Position::new(0, 5));
    }

    #[test]
    fn move_cursors_word() {
        let b = buf("hello world");
        let mut ctrl = CursorController::new(Position::new(0, 0));
        ctrl.move_cursors(Direction::Right, false, true, &b, 20);
        assert_eq!(ctrl.primary().position(), Position::new(0, 5));
    }

    #[test]
    fn move_cursors_home_end() {
        let b = buf("hello world");
        let mut ctrl = CursorController::new(Position::new(0, 5));
        ctrl.move_cursors(Direction::Home, false, false, &b, 20);
        assert_eq!(ctrl.primary().position(), Position::new(0, 0));
        ctrl.move_cursors(Direction::End, false, false, &b, 20);
        assert_eq!(ctrl.primary().position(), Position::new(0, 11));
    }

    #[test]
    fn move_cursors_top_bottom() {
        let b = buf("aaa\nbbb\nccc");
        let mut ctrl = CursorController::new(Position::new(1, 1));
        ctrl.move_cursors(Direction::Top, false, false, &b, 20);
        assert_eq!(ctrl.primary().position(), Position::new(0, 0));
        ctrl.move_cursors(Direction::Bottom, false, false, &b, 20);
        assert_eq!(ctrl.primary().position().line, 2);
    }

    #[test]
    fn select_word_at_cursor() {
        let b = buf("hello world");
        let mut ctrl = CursorController::new(Position::new(0, 7));
        ctrl.select_word_at_cursor(&b);
        let sel = ctrl.primary().selection;
        assert!(!sel.is_empty());
        assert_eq!(sel.start(), Position::new(0, 6));
        assert_eq!(sel.end(), Position::new(0, 11));
    }

    #[test]
    fn select_line() {
        let b = buf("hello\nworld");
        let mut ctrl = CursorController::new(Position::new(0, 2));
        ctrl.select_line(&b);
        let sel = ctrl.primary().selection;
        assert_eq!(sel.start(), Position::new(0, 0));
        assert_eq!(sel.end(), Position::new(0, 5));
    }

    #[test]
    fn column_select() {
        let b = buf("aaaa\nbbbb\ncccc\ndddd");
        let mut ctrl = CursorController::new(Position::new(0, 0));
        ctrl.column_select(Position::new(0, 1), Position::new(2, 3), &b);
        assert_eq!(ctrl.len(), 3);
    }

    #[test]
    fn collapse_to_primary() {
        let mut ctrl = CursorController::new(Position::new(0, 0));
        ctrl.add_cursor_at_position(Position::new(1, 0));
        ctrl.add_cursor_at_position(Position::new(2, 0));
        ctrl.collapse_to_primary();
        assert_eq!(ctrl.len(), 1);
    }

    #[test]
    fn selection_direction() {
        let ltr = Selection::new(Position::new(0, 0), Position::new(0, 5));
        assert_eq!(
            CursorController::selection_direction(&ltr),
            SelectionDirection::LeftToRight
        );
        let rtl = Selection::new(Position::new(0, 5), Position::new(0, 0));
        assert_eq!(
            CursorController::selection_direction(&rtl),
            SelectionDirection::RightToLeft
        );
    }

    #[test]
    fn expand_selection_word_then_line() {
        let b = buf("hello world");
        let mut ctrl = CursorController::new(Position::new(0, 7));
        ctrl.expand_selection(&b);
        let sel = ctrl.primary().selection;
        assert!(!sel.is_empty());
        ctrl.expand_selection(&b);
        let sel2 = ctrl.primary().selection;
        assert_eq!(sel2.start(), Position::new(0, 0));
    }

    #[test]
    fn shrink_selection_reverts() {
        let b = buf("hello world");
        let mut ctrl = CursorController::new(Position::new(0, 7));
        ctrl.expand_selection(&b);
        let after_expand = ctrl.primary().selection;
        assert!(!after_expand.is_empty());
        ctrl.shrink_selection();
        let after_shrink = ctrl.primary().selection;
        assert!(after_shrink.is_empty());
    }

    #[test]
    fn add_selection_to_next_find_match() {
        let b = buf("foo bar foo baz foo");
        let mut ctrl = CursorController::new(Position::new(0, 0));
        ctrl.inner.set_primary_selection(Selection::new(
            Position::new(0, 0),
            Position::new(0, 3),
        ));
        ctrl.add_selection_to_next_find_match(&b);
        assert!(ctrl.len() >= 2);
    }

    #[test]
    fn select_all_occurrences() {
        let b = buf("foo bar foo baz foo");
        let mut ctrl = CursorController::new(Position::new(0, 0));
        ctrl.select_all_occurrences(&b, "foo");
        assert_eq!(ctrl.len(), 3);
    }
}
