//! Multi-cursor commands — mirrors VS Code's `multicursor` contribution.
//!
//! Higher-level multi-cursor operations: add cursors above/below, add cursors
//! at line ends, select all occurrences, column/box selection.

use sidex_text::{Buffer, Position};

use crate::cursor::CursorState;
use crate::multi_cursor::MultiCursor;
use crate::selection::Selection;

/// Adds a new cursor one line above the primary cursor.
pub fn add_cursor_above(mc: &mut MultiCursor, buffer: &Buffer) {
    mc.add_cursor_above(buffer);
}

/// Adds a new cursor one line below the primary cursor.
pub fn add_cursor_below(mc: &mut MultiCursor, buffer: &Buffer) {
    mc.add_cursor_below(buffer);
}

/// Adds cursors at the end of each line in the current selection (Shift+Alt+I).
pub fn add_cursors_to_line_ends(mc: &mut MultiCursor, buffer: &Buffer) {
    let sel = mc.primary().selection;
    if sel.is_empty() {
        return;
    }
    let start = sel.start();
    let end = sel.end();

    mc.collapse_to_primary();
    for line in start.line..=end.line {
        let line_len = buffer.line_content_len(line as usize) as u32;
        if line == start.line {
            mc.primary_mut().selection = Selection::caret(Position::new(line, line_len));
        } else {
            mc.add_cursor(Position::new(line, line_len));
        }
    }
}

/// Creates column/box selection between two positions (Alt+Shift+drag).
/// Places a cursor on every line between `anchor` and `active`, each with a
/// selection from `left_col` to `right_col`.
pub fn column_selection(mc: &mut MultiCursor, buffer: &Buffer, anchor: Position, active: Position) {
    let start_line = anchor.line.min(active.line);
    let end_line = anchor.line.max(active.line);
    let left_col = anchor.column.min(active.column);
    let right_col = anchor.column.max(active.column);

    mc.collapse_to_primary();
    let line_count = buffer.len_lines() as u32;

    let mut first = true;
    for line in start_line..=end_line.min(line_count.saturating_sub(1)) {
        let line_len = buffer.line_content_len(line as usize) as u32;
        let actual_left = left_col.min(line_len);
        let actual_right = right_col.min(line_len);

        let sel = Selection::new(
            Position::new(line, actual_left),
            Position::new(line, actual_right),
        );

        if first {
            mc.primary_mut().selection = sel;
            first = false;
        } else {
            mc.add_cursor(Position::new(line, actual_right));
            let idx = mc.len() - 1;
            mc.cursors_mut()[idx].selection = sel;
        }
    }
}

/// Adds a cursor at every occurrence of the given search term in the document.
pub fn add_cursors_to_search_matches(mc: &mut MultiCursor, buffer: &Buffer, search: &str) {
    if search.is_empty() {
        return;
    }

    let line_count = buffer.len_lines();
    let mut new_cursors = Vec::new();

    for line_idx in 0..line_count {
        let content = buffer.line_content(line_idx);
        let mut start = 0;
        while let Some(found) = content[start..].find(search) {
            let abs_start = start + found;
            let abs_end = abs_start + search.len();

            let sel = Selection::new(
                Position::new(line_idx as u32, abs_start as u32),
                Position::new(line_idx as u32, abs_end as u32),
            );
            new_cursors.push(CursorState::from_selection(sel));
            start = abs_end;
        }
    }

    if !new_cursors.is_empty() {
        mc.collapse_to_primary();
        let first = new_cursors.remove(0);
        mc.primary_mut().selection = first.selection;
        for c in new_cursors {
            mc.add_cursor(c.position());
            let idx = mc.len() - 1;
            mc.cursors_mut()[idx].selection = c.selection;
        }
    }
}

/// Selects all occurrences of the word under the primary cursor (Ctrl+Shift+L),
/// placing a cursor/selection at each one.
pub fn select_all_occurrences(mc: &mut MultiCursor, buffer: &Buffer) {
    let pos = mc.primary().position();
    let line_count = buffer.len_lines();

    if pos.line as usize >= line_count {
        return;
    }

    let line = buffer.line_content(pos.line as usize);
    let col = pos.column as usize;

    let chars: Vec<char> = line.chars().collect();
    if col >= chars.len() || (!chars[col].is_alphanumeric() && chars[col] != '_') {
        return;
    }

    let start = (0..col)
        .rev()
        .take_while(|&i| chars[i].is_alphanumeric() || chars[i] == '_')
        .last()
        .unwrap_or(col);
    let end = (col..chars.len())
        .take_while(|&i| chars[i].is_alphanumeric() || chars[i] == '_')
        .last()
        .map_or(col, |i| i + 1);

    let word: String = chars[start..end].iter().collect();
    if word.is_empty() {
        return;
    }

    let mut new_cursors = Vec::new();

    for line_idx in 0..line_count {
        let content = buffer.line_content(line_idx);
        let mut search_start = 0;
        while let Some(found) = content[search_start..].find(&word) {
            let abs_start = search_start + found;
            let abs_end = abs_start + word.len();

            let before_ok = abs_start == 0 || {
                let ch = content.as_bytes()[abs_start - 1];
                !ch.is_ascii_alphanumeric() && ch != b'_'
            };
            let after_ok = abs_end >= content.len() || {
                let ch = content.as_bytes()[abs_end];
                !ch.is_ascii_alphanumeric() && ch != b'_'
            };

            if before_ok && after_ok {
                let sel = Selection::new(
                    Position::new(line_idx as u32, abs_start as u32),
                    Position::new(line_idx as u32, abs_end as u32),
                );
                new_cursors.push(CursorState::from_selection(sel));
            }

            search_start = abs_end;
        }
    }

    if !new_cursors.is_empty() {
        mc.collapse_to_primary();
        let first = new_cursors.remove(0);
        mc.primary_mut().selection = first.selection;
        for c in new_cursors {
            mc.add_cursor(c.position());
            let idx = mc.len() - 1;
            mc.cursors_mut()[idx].selection = c.selection;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn add_cursor_above_and_below() {
        let buffer = buf("line0\nline1\nline2");
        let mut mc = MultiCursor::new(Position::new(1, 2));
        add_cursor_above(&mut mc, &buffer);
        assert!(mc.len() >= 2);
        add_cursor_below(&mut mc, &buffer);
        assert!(mc.len() >= 2);
    }

    #[test]
    fn select_all_occurrences_finds_word() {
        let buffer = buf("foo bar foo baz foo");
        let mut mc = MultiCursor::new(Position::new(0, 0));
        select_all_occurrences(&mut mc, &buffer);
        assert_eq!(mc.len(), 3);
    }

    #[test]
    fn column_selection_creates_cursors() {
        let buffer = buf("abcdef\nabcdef\nabcdef");
        let mut mc = MultiCursor::new(Position::new(0, 0));
        column_selection(&mut mc, &buffer, Position::new(0, 2), Position::new(2, 4));
        assert_eq!(mc.len(), 3);
    }

    #[test]
    fn add_cursors_to_search() {
        let buffer = buf("aa bb aa cc aa");
        let mut mc = MultiCursor::new(Position::new(0, 0));
        add_cursors_to_search_matches(&mut mc, &buffer, "aa");
        assert_eq!(mc.len(), 3);
    }
}
