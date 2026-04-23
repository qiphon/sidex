//! Terminal text selection.
//!
//! Provides selection logic for the terminal: normal (character), word,
//! line, and block/column modes. Integrates with [`TerminalGrid`] to
//! extract selected text and compute selection boundaries.

use crate::grid::{SelectionMode, SelectionPoint, TerminalGrid, TerminalSelection};

/// Starts a new selection at the given point with the given mode.
pub fn start_selection(point: SelectionPoint, mode: SelectionMode) -> TerminalSelection {
    TerminalSelection {
        start: point,
        end: point,
        mode,
    }
}

/// Updates the end-point of an in-progress selection (drag).
pub fn update_selection(sel: &mut TerminalSelection, end: SelectionPoint) {
    sel.end = end;
}

/// Extends an existing selection to a new point (Shift+Click).
pub fn extend_selection(sel: &mut TerminalSelection, point: SelectionPoint) {
    let (start, end) = sel.ordered();
    if point < start {
        sel.start = point;
        sel.end = end;
    } else {
        sel.start = start;
        sel.end = point;
    }
}

/// Extracts the selected text from the grid, respecting the selection mode.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn selected_text(grid: &TerminalGrid, selection: &TerminalSelection) -> String {
    let (start, end) = selection.ordered();
    let mut result = String::new();

    match selection.mode {
        SelectionMode::Normal | SelectionMode::Word => {
            for line in start.line..=end.line {
                if line < 0 {
                    continue;
                }
                let row = line as u16;
                if row >= grid.rows() {
                    break;
                }
                let col_start = if line == start.line { start.col } else { 0 };
                let col_end = if line == end.line {
                    end.col
                } else {
                    grid.cols().saturating_sub(1)
                };
                let mut line_text = String::new();
                for col in col_start..=col_end {
                    if col >= grid.cols() {
                        break;
                    }
                    let cell = grid.cell(row, col);
                    if cell.width != 0 {
                        line_text.push(cell.c);
                    }
                }
                if line == end.line {
                    result.push_str(&line_text);
                } else {
                    result.push_str(line_text.trim_end_matches(' '));
                    result.push('\n');
                }
            }
        }
        SelectionMode::Line => {
            for line in start.line..=end.line {
                if line < 0 {
                    continue;
                }
                let row = line as u16;
                if row >= grid.rows() {
                    break;
                }
                for col in 0..grid.cols() {
                    let cell = grid.cell(row, col);
                    if cell.width != 0 {
                        result.push(cell.c);
                    }
                }
                // Trim trailing spaces per line
                let trimmed = result.trim_end_matches(' ').len();
                result.truncate(trimmed);
                if line != end.line {
                    result.push('\n');
                }
            }
        }
        SelectionMode::Block => {
            let col_left = start.col.min(end.col);
            let col_right = start.col.max(end.col);
            for line in start.line..=end.line {
                if line < 0 {
                    continue;
                }
                let row = line as u16;
                if row >= grid.rows() {
                    break;
                }
                for col in col_left..=col_right {
                    if col >= grid.cols() {
                        break;
                    }
                    let cell = grid.cell(row, col);
                    if cell.width != 0 {
                        result.push(cell.c);
                    }
                }
                if line != end.line {
                    result.push('\n');
                }
            }
        }
    }

    result.trim_end().to_string()
}

/// Returns `true` if the given grid cell is within the selection.
pub fn is_selected(selection: &TerminalSelection, row: u16, col: u16) -> bool {
    let (start, end) = selection.ordered();
    let line = i32::from(row);

    if line < start.line || line > end.line {
        return false;
    }

    match selection.mode {
        SelectionMode::Normal | SelectionMode::Word => {
            if start.line == end.line {
                col >= start.col && col <= end.col
            } else if line == start.line {
                col >= start.col
            } else if line == end.line {
                col <= end.col
            } else {
                true
            }
        }
        SelectionMode::Line => true,
        SelectionMode::Block => {
            let col_left = start.col.min(end.col);
            let col_right = start.col.max(end.col);
            col >= col_left && col <= col_right
        }
    }
}

/// Expands a selection point to word boundaries. Returns (start, end) of the word.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn expand_selection_word(
    grid: &TerminalGrid,
    point: SelectionPoint,
) -> (SelectionPoint, SelectionPoint) {
    if point.line < 0 || point.line as u16 >= grid.rows() {
        return (point, point);
    }
    let row = point.line as u16;
    let col = point.col.min(grid.cols().saturating_sub(1));

    let is_word_char =
        |c: char| -> bool { c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '/' };

    let cell_char = grid.cell(row, col).c;
    if !is_word_char(cell_char) && cell_char != ' ' {
        // Single punctuation character — select just that
        return (point, point);
    }

    // Expand left
    let mut start_col = col;
    while start_col > 0 {
        let prev = grid.cell(row, start_col - 1).c;
        if !is_word_char(prev) {
            break;
        }
        start_col -= 1;
    }

    // Expand right
    let mut end_col = col;
    while end_col + 1 < grid.cols() {
        let next = grid.cell(row, end_col + 1).c;
        if !is_word_char(next) {
            break;
        }
        end_col += 1;
    }

    (
        SelectionPoint {
            line: point.line,
            col: start_col,
        },
        SelectionPoint {
            line: point.line,
            col: end_col,
        },
    )
}

/// Expands a selection point to full line boundaries.
pub fn expand_selection_line(
    grid: &TerminalGrid,
    point: SelectionPoint,
) -> (SelectionPoint, SelectionPoint) {
    let line = point.line;
    let last_col = grid.cols().saturating_sub(1);
    (
        SelectionPoint { line, col: 0 },
        SelectionPoint {
            line,
            col: last_col,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::grid::Cell;

    fn make_grid_with_text(text: &str) -> TerminalGrid {
        let mut grid = TerminalGrid::new(4, 20);
        let template = Cell::default();
        for ch in text.chars() {
            if ch == '\n' {
                let (row, _) = grid.cursor_position();
                grid.set_cursor(row + 1, 0);
            } else {
                grid.write_char(ch, &template);
            }
        }
        grid
    }

    #[test]
    fn normal_selection_single_line() {
        let grid = make_grid_with_text("Hello World");
        let sel = TerminalSelection {
            start: SelectionPoint { line: 0, col: 0 },
            end: SelectionPoint { line: 0, col: 4 },
            mode: SelectionMode::Normal,
        };
        assert_eq!(selected_text(&grid, &sel), "Hello");
    }

    #[test]
    fn normal_selection_multi_line() {
        let grid = make_grid_with_text("Hello\nWorld");
        let sel = TerminalSelection {
            start: SelectionPoint { line: 0, col: 3 },
            end: SelectionPoint { line: 1, col: 4 },
            mode: SelectionMode::Normal,
        };
        assert_eq!(selected_text(&grid, &sel), "lo\nWorld");
    }

    #[test]
    fn line_selection() {
        let grid = make_grid_with_text("Hello World");
        let sel = TerminalSelection {
            start: SelectionPoint { line: 0, col: 3 },
            end: SelectionPoint { line: 0, col: 3 },
            mode: SelectionMode::Line,
        };
        assert_eq!(selected_text(&grid, &sel), "Hello World");
    }

    #[test]
    fn block_selection() {
        let grid = make_grid_with_text("ABCD\nEFGH\nIJKL");
        let sel = TerminalSelection {
            start: SelectionPoint { line: 0, col: 1 },
            end: SelectionPoint { line: 2, col: 2 },
            mode: SelectionMode::Block,
        };
        let text = selected_text(&grid, &sel);
        assert_eq!(text, "BC\nFG\nJK");
    }

    #[test]
    fn is_selected_normal() {
        let sel = TerminalSelection {
            start: SelectionPoint { line: 0, col: 2 },
            end: SelectionPoint { line: 0, col: 5 },
            mode: SelectionMode::Normal,
        };
        assert!(!is_selected(&sel, 0, 1));
        assert!(is_selected(&sel, 0, 2));
        assert!(is_selected(&sel, 0, 5));
        assert!(!is_selected(&sel, 0, 6));
    }

    #[test]
    fn word_expansion() {
        let grid = make_grid_with_text("hello world");
        let point = SelectionPoint { line: 0, col: 2 };
        let (start, end) = expand_selection_word(&grid, point);
        assert_eq!(start.col, 0);
        assert_eq!(end.col, 4);
    }

    #[test]
    fn line_expansion() {
        let grid = TerminalGrid::new(4, 20);
        let point = SelectionPoint { line: 1, col: 5 };
        let (start, end) = expand_selection_line(&grid, point);
        assert_eq!(start.col, 0);
        assert_eq!(end.col, 19);
    }

    #[test]
    fn extend_selection_before_start() {
        let mut sel = TerminalSelection {
            start: SelectionPoint { line: 1, col: 5 },
            end: SelectionPoint { line: 1, col: 10 },
            mode: SelectionMode::Normal,
        };
        extend_selection(&mut sel, SelectionPoint { line: 0, col: 3 });
        let (s, e) = sel.ordered();
        assert_eq!(s, SelectionPoint { line: 0, col: 3 });
        assert_eq!(e, SelectionPoint { line: 1, col: 10 });
    }
}
