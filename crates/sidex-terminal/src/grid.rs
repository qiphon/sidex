//! Terminal grid/screen buffer.
//!
//! Provides the character grid that represents the visible terminal screen,
//! along with a ring-buffer scrollback for historical lines. Supports wide
//! characters (CJK), configurable tab stops, alternate screen buffers,
//! and multiple selection modes.

use bitflags::bitflags;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Default scrollback capacity (number of lines).
const DEFAULT_SCROLLBACK_CAPACITY: usize = 10_000;

/// Maximum scrollback capacity.
pub const MAX_SCROLLBACK_CAPACITY: usize = 100_000;

/// Default tab stop interval.
const DEFAULT_TAB_INTERVAL: u16 = 8;

// ---------------------------------------------------------------------------
// Cell attributes (bitflags)
// ---------------------------------------------------------------------------

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CellAttributes: u16 {
        const BOLD             = 0b0000_0000_0001;
        const DIM              = 0b0000_0000_0010;
        const ITALIC           = 0b0000_0000_0100;
        const UNDERLINE        = 0b0000_0000_1000;
        const BLINK            = 0b0000_0001_0000;
        const INVERSE          = 0b0000_0010_0000;
        const HIDDEN           = 0b0000_0100_0000;
        const STRIKETHROUGH    = 0b0000_1000_0000;
        const DOUBLE_UNDERLINE = 0b0001_0000_0000;
        const CURLY_UNDERLINE  = 0b0010_0000_0000;
        const DOTTED_UNDERLINE = 0b0100_0000_0000;
        const DASHED_UNDERLINE = 0b1000_0000_0000;
        const OVERLINE         = 0b0001_0000_0000_0000;
    }
}

impl Default for CellAttributes {
    fn default() -> Self {
        Self::empty()
    }
}

impl Serialize for CellAttributes {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CellAttributes {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bits = u16::deserialize(deserializer)?;
        Ok(Self::from_bits_truncate(bits))
    }
}

// ---------------------------------------------------------------------------
// Color types
// ---------------------------------------------------------------------------

/// Named ANSI colors (the 16 standard colors).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl NamedColor {
    /// Returns the standard 256-color palette index for this named color.
    pub fn to_index(self) -> u8 {
        match self {
            Self::Black => 0,
            Self::Red => 1,
            Self::Green => 2,
            Self::Yellow => 3,
            Self::Blue => 4,
            Self::Magenta => 5,
            Self::Cyan => 6,
            Self::White => 7,
            Self::BrightBlack => 8,
            Self::BrightRed => 9,
            Self::BrightGreen => 10,
            Self::BrightYellow => 11,
            Self::BrightBlue => 12,
            Self::BrightMagenta => 13,
            Self::BrightCyan => 14,
            Self::BrightWhite => 15,
        }
    }
}

/// Terminal color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    Default,
    Named(NamedColor),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

// ---------------------------------------------------------------------------
// Cell
// ---------------------------------------------------------------------------

/// A single cell in the terminal grid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttributes,
    /// 1 for normal characters, 2 for wide (CJK). 0 marks a continuation
    /// cell that is the right half of a wide character.
    pub width: u8,
    /// Optional hyperlink URL attached to this cell (OSC 8).
    pub hyperlink: Option<String>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            fg: Color::Default,
            bg: Color::Default,
            attrs: CellAttributes::empty(),
            width: 1,
            hyperlink: None,
        }
    }
}

impl Cell {
    #[inline]
    pub fn bold(&self) -> bool {
        self.attrs.contains(CellAttributes::BOLD)
    }
    #[inline]
    pub fn dim(&self) -> bool {
        self.attrs.contains(CellAttributes::DIM)
    }
    #[inline]
    pub fn italic(&self) -> bool {
        self.attrs.contains(CellAttributes::ITALIC)
    }
    #[inline]
    pub fn underline(&self) -> bool {
        self.attrs.contains(CellAttributes::UNDERLINE)
    }
    #[inline]
    pub fn blink(&self) -> bool {
        self.attrs.contains(CellAttributes::BLINK)
    }
    #[inline]
    pub fn inverse(&self) -> bool {
        self.attrs.contains(CellAttributes::INVERSE)
    }
    #[inline]
    pub fn hidden(&self) -> bool {
        self.attrs.contains(CellAttributes::HIDDEN)
    }
    #[inline]
    pub fn strikethrough(&self) -> bool {
        self.attrs.contains(CellAttributes::STRIKETHROUGH)
    }

    /// Returns the underline style: 0=none, 1=single, 2=double, 3=curly, 4=dotted, 5=dashed.
    pub fn underline_style(&self) -> u8 {
        if self.attrs.contains(CellAttributes::DASHED_UNDERLINE) {
            5
        } else if self.attrs.contains(CellAttributes::DOTTED_UNDERLINE) {
            4
        } else if self.attrs.contains(CellAttributes::CURLY_UNDERLINE) {
            3
        } else if self.attrs.contains(CellAttributes::DOUBLE_UNDERLINE) {
            2
        } else {
            u8::from(self.attrs.contains(CellAttributes::UNDERLINE))
        }
    }

    /// Returns `true` if this cell is the continuation (right half) of a wide char.
    #[inline]
    pub fn is_wide_continuation(&self) -> bool {
        self.width == 0
    }
}

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

/// Terminal cursor state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalCursor {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

impl Default for TerminalCursor {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Scrollback
// ---------------------------------------------------------------------------

/// Ring-buffer scrollback for historical terminal lines.
#[derive(Debug, Clone)]
pub struct Scrollback {
    pub lines: VecDeque<Vec<Cell>>,
    pub max_lines: usize,
}

impl Scrollback {
    pub fn new(max_lines: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            max_lines: max_lines.min(MAX_SCROLLBACK_CAPACITY),
        }
    }

    pub fn push(&mut self, line: Vec<Cell>) {
        if self.lines.len() >= self.max_lines {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&[Cell]> {
        self.lines.get(index).map(Vec::as_slice)
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    pub fn drain_back(&mut self, count: usize) -> Vec<Vec<Cell>> {
        let n = count.min(self.lines.len());
        self.lines.drain(self.lines.len() - n..).collect()
    }
}

impl Default for Scrollback {
    fn default() -> Self {
        Self::new(DEFAULT_SCROLLBACK_CAPACITY)
    }
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// A point in the terminal grid, where negative line values refer to scrollback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionPoint {
    pub line: i32,
    pub col: u16,
}

impl PartialOrd for SelectionPoint {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SelectionPoint {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.line.cmp(&other.line).then(self.col.cmp(&other.col))
    }
}

/// Selection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionMode {
    Normal,
    Word,
    Line,
    Block,
}

/// A text selection in the terminal grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSelection {
    pub start: SelectionPoint,
    pub end: SelectionPoint,
    pub mode: SelectionMode,
}

impl TerminalSelection {
    pub fn new_simple(start_row: u16, start_col: u16, end_row: u16, end_col: u16) -> Self {
        Self {
            start: SelectionPoint {
                line: i32::from(start_row),
                col: start_col,
            },
            end: SelectionPoint {
                line: i32::from(end_row),
                col: end_col,
            },
            mode: SelectionMode::Normal,
        }
    }

    /// Returns (start, end) in canonical order (start <= end).
    pub fn ordered(&self) -> (SelectionPoint, SelectionPoint) {
        if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

/// The terminal character grid and scrollback buffer.
pub struct TerminalGrid {
    pub rows: u16,
    pub cols: u16,
    cells: Vec<Vec<Cell>>,
    pub cursor: TerminalCursor,
    scroll_top: u16,
    scroll_bottom: u16,
    pub scrollback: Scrollback,
    pub selection: Option<TerminalSelection>,
    pub alternate_screen: Option<Vec<Vec<Cell>>>,
    pub tabs: Vec<bool>,
    pub saved_cursor: Option<TerminalCursor>,
    /// Viewport scroll offset (0 = bottom / live, >0 = scrolled up).
    pub scroll_offset: usize,
}

impl TerminalGrid {
    pub fn new(rows: u16, cols: u16) -> Self {
        let cells = (0..rows)
            .map(|_| vec![Cell::default(); cols as usize])
            .collect();
        let mut tabs = vec![false; cols as usize];
        for i in (0..cols as usize).step_by(DEFAULT_TAB_INTERVAL as usize) {
            tabs[i] = true;
        }
        Self {
            rows,
            cols,
            cells,
            cursor: TerminalCursor::default(),
            scroll_top: 0,
            scroll_bottom: rows.saturating_sub(1),
            scrollback: Scrollback::default(),
            selection: None,
            alternate_screen: None,
            tabs,
            saved_cursor: None,
            scroll_offset: 0,
        }
    }

    pub fn with_scrollback_capacity(rows: u16, cols: u16, capacity: usize) -> Self {
        let mut grid = Self::new(rows, cols);
        grid.scrollback = Scrollback::new(capacity);
        grid
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }
    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn cells(&self) -> &[Vec<Cell>] {
        &self.cells
    }

    pub fn cell(&self, row: u16, col: u16) -> &Cell {
        &self.cells[row as usize][col as usize]
    }

    pub fn cell_mut(&mut self, row: u16, col: u16) -> &mut Cell {
        &mut self.cells[row as usize][col as usize]
    }

    pub fn cursor_position(&self) -> (u16, u16) {
        (self.cursor.row, self.cursor.col)
    }

    pub fn set_cursor(&mut self, row: u16, col: u16) {
        self.cursor.row = row.min(self.rows.saturating_sub(1));
        self.cursor.col = col.min(self.cols.saturating_sub(1));
    }

    pub fn scroll_region(&self) -> (u16, u16) {
        (self.scroll_top, self.scroll_bottom)
    }

    pub fn set_scroll_region(&mut self, top: u16, bottom: u16) {
        if top < bottom && bottom < self.rows {
            self.scroll_top = top;
            self.scroll_bottom = bottom;
        }
    }

    pub fn save_cursor(&mut self) {
        self.saved_cursor = Some(self.cursor);
    }

    pub fn restore_cursor(&mut self) {
        if let Some(saved) = self.saved_cursor {
            self.cursor = saved;
        }
    }

    // --- Tab stops ---

    #[allow(clippy::cast_possible_truncation)]
    pub fn next_tab_stop(&self, col: u16) -> u16 {
        for i in (col as usize + 1)..self.tabs.len() {
            if self.tabs[i] {
                return i as u16;
            }
        }
        self.cols.saturating_sub(1)
    }

    pub fn set_tab_stop(&mut self, col: u16) {
        if (col as usize) < self.tabs.len() {
            self.tabs[col as usize] = true;
        }
    }

    pub fn clear_tab_stop(&mut self, col: u16) {
        if (col as usize) < self.tabs.len() {
            self.tabs[col as usize] = false;
        }
    }

    pub fn clear_all_tab_stops(&mut self) {
        self.tabs.iter_mut().for_each(|t| *t = false);
    }

    // --- Clear operations ---

    pub fn clear(&mut self) {
        for row in &mut self.cells {
            for cell in row.iter_mut() {
                *cell = Cell::default();
            }
        }
    }

    pub fn clear_line(&mut self, row: u16) {
        if (row as usize) < self.cells.len() {
            for cell in &mut self.cells[row as usize] {
                *cell = Cell::default();
            }
        }
    }

    pub fn clear_line_from_cursor(&mut self) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;
        if row < self.cells.len() {
            for cell in self.cells[row].iter_mut().skip(col) {
                *cell = Cell::default();
            }
        }
    }

    pub fn clear_line_to_cursor(&mut self) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;
        if row < self.cells.len() {
            for cell in self.cells[row].iter_mut().take(col + 1) {
                *cell = Cell::default();
            }
        }
    }

    pub fn clear_below(&mut self) {
        self.clear_line_from_cursor();
        for r in (self.cursor.row + 1)..self.rows {
            self.clear_line(r);
        }
    }

    pub fn clear_above(&mut self) {
        self.clear_line_to_cursor();
        for r in 0..self.cursor.row {
            self.clear_line(r);
        }
    }

    // --- Scroll operations ---

    #[allow(clippy::assigning_clones)]
    pub fn scroll_up(&mut self) {
        let top = self.scroll_top as usize;
        let bottom = self.scroll_bottom as usize;
        if top > bottom || bottom >= self.cells.len() {
            return;
        }
        let evicted = self.cells[top].clone();
        if self.scroll_top == 0 {
            self.scrollback.push(evicted);
        }
        for r in top..bottom {
            self.cells[r] = self.cells[r + 1].clone();
        }
        self.cells[bottom] = vec![Cell::default(); self.cols as usize];
    }

    #[allow(clippy::assigning_clones)]
    pub fn scroll_down(&mut self) {
        let top = self.scroll_top as usize;
        let bottom = self.scroll_bottom as usize;
        if top > bottom || bottom >= self.cells.len() {
            return;
        }
        for r in (top + 1..=bottom).rev() {
            self.cells[r] = self.cells[r - 1].clone();
        }
        self.cells[top] = vec![Cell::default(); self.cols as usize];
    }

    pub fn scroll_viewport_up(&mut self, lines: usize) {
        let max = self.scrollback.len();
        self.scroll_offset = (self.scroll_offset + lines).min(max);
    }

    pub fn scroll_viewport_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let new_cols = cols as usize;
        for row in &mut self.cells {
            row.resize(new_cols, Cell::default());
        }
        let new_rows = rows as usize;
        let old_rows = self.cells.len();
        if new_rows > old_rows {
            let extra = new_rows - old_rows;
            let from_scrollback = extra.min(self.scrollback.len());
            let mut restored = self.scrollback.drain_back(from_scrollback);
            for row in &mut restored {
                row.resize(new_cols, Cell::default());
            }
            restored.append(&mut self.cells);
            self.cells = restored;
            while self.cells.len() < new_rows {
                self.cells.push(vec![Cell::default(); new_cols]);
            }
        } else if new_rows < old_rows {
            let excess = old_rows - new_rows;
            for row in self.cells.drain(..excess) {
                self.scrollback.push(row);
            }
        }
        self.rows = rows;
        self.cols = cols;
        self.scroll_top = 0;
        self.scroll_bottom = rows.saturating_sub(1);
        self.cursor.row = self.cursor.row.min(rows.saturating_sub(1));
        self.cursor.col = self.cursor.col.min(cols.saturating_sub(1));
        self.tabs.resize(new_cols, false);
        for i in (0..new_cols).step_by(DEFAULT_TAB_INTERVAL as usize) {
            self.tabs[i] = true;
        }
    }

    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    pub fn scrollback_line(&self, index: usize) -> Option<&[Cell]> {
        self.scrollback.get(index)
    }

    /// Writes a character at the current cursor position with the given
    /// template cell attributes, then advances the cursor. Handles wide
    /// (CJK) characters that occupy two cells.
    pub fn write_char(&mut self, ch: char, template: &Cell) {
        let char_width = unicode_width(ch);

        if self.cursor.col >= self.cols {
            self.cursor.col = 0;
            self.cursor.row += 1;
            if self.cursor.row > self.scroll_bottom {
                self.cursor.row = self.scroll_bottom;
                self.scroll_up();
            }
        }

        // For wide chars, ensure there's room for both cells.
        if char_width == 2 && self.cursor.col + 1 >= self.cols {
            let row = self.cursor.row as usize;
            let col = self.cursor.col as usize;
            if row < self.cells.len() && col < self.cells[row].len() {
                self.cells[row][col] = Cell::default();
            }
            self.cursor.col = 0;
            self.cursor.row += 1;
            if self.cursor.row > self.scroll_bottom {
                self.cursor.row = self.scroll_bottom;
                self.scroll_up();
            }
        }

        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;
        if row < self.cells.len() && col < self.cells[row].len() {
            self.cells[row][col] = Cell {
                c: ch,
                fg: template.fg,
                bg: template.bg,
                attrs: template.attrs,
                width: char_width,
                hyperlink: template.hyperlink.clone(),
            };
        }
        self.cursor.col += 1;

        if char_width == 2 {
            let col2 = self.cursor.col as usize;
            if row < self.cells.len() && col2 < self.cells[row].len() {
                self.cells[row][col2] = Cell {
                    c: ' ',
                    fg: template.fg,
                    bg: template.bg,
                    attrs: template.attrs,
                    width: 0, // continuation cell
                    hyperlink: None,
                };
            }
            self.cursor.col += 1;
        }
    }

    pub fn row_text(&self, row: u16) -> String {
        if row as usize >= self.cells.len() {
            return String::new();
        }
        let text: String = self.cells[row as usize]
            .iter()
            .filter(|c| c.width != 0)
            .map(|c| c.c)
            .collect();
        text.trim_end().to_string()
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn get_selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        let (start, end) = sel.ordered();
        let mut result = String::new();
        for line in start.line..=end.line {
            if line < 0 {
                continue;
            }
            let row = line as u16;
            let col_start = if line == start.line { start.col } else { 0 };
            let col_end = if line == end.line {
                end.col
            } else {
                self.cols.saturating_sub(1)
            };
            if (row as usize) < self.cells.len() {
                let cells = &self.cells[row as usize];
                for col in col_start..=col_end {
                    if (col as usize) < cells.len() && cells[col as usize].width != 0 {
                        result.push(cells[col as usize].c);
                    }
                }
            }
            if line != end.line {
                result.push('\n');
            }
        }
        let trimmed = result.trim_end().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }

    pub fn find_in_terminal(&self, query: &str) -> Vec<(u16, u16)> {
        let mut results = Vec::new();
        if query.is_empty() {
            return results;
        }
        let query_chars: Vec<char> = query.chars().collect();
        let qlen = query_chars.len();
        for (row_idx, row) in self.cells.iter().enumerate() {
            let row_chars: Vec<char> = row.iter().filter(|c| c.width != 0).map(|c| c.c).collect();
            if row_chars.len() < qlen {
                continue;
            }
            for col in 0..=(row_chars.len() - qlen) {
                if row_chars[col..col + qlen] == query_chars[..] {
                    #[allow(clippy::cast_possible_truncation)]
                    results.push((row_idx as u16, col as u16));
                }
            }
        }
        results
    }

    pub fn insert_lines(&mut self, count: u16) {
        let top = self.cursor.row as usize;
        let bottom = self.scroll_bottom as usize;
        if top > bottom || bottom >= self.cells.len() {
            return;
        }
        for _ in 0..count {
            if bottom < self.cells.len() {
                self.cells.remove(bottom);
            }
            self.cells
                .insert(top, vec![Cell::default(); self.cols as usize]);
        }
    }

    pub fn delete_lines(&mut self, count: u16) {
        let top = self.cursor.row as usize;
        let bottom = self.scroll_bottom as usize;
        if top > bottom || bottom >= self.cells.len() {
            return;
        }
        for _ in 0..count {
            if top < self.cells.len() {
                self.cells.remove(top);
            }
            if self.cells.len() <= bottom {
                self.cells.push(vec![Cell::default(); self.cols as usize]);
            } else {
                self.cells
                    .insert(bottom, vec![Cell::default(); self.cols as usize]);
            }
        }
    }

    pub fn insert_chars(&mut self, count: u16) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;
        let cols = self.cols as usize;
        if row >= self.cells.len() {
            return;
        }
        for _ in 0..count {
            if col < cols {
                self.cells[row].insert(col, Cell::default());
                self.cells[row].truncate(cols);
            }
        }
    }

    pub fn delete_chars(&mut self, count: u16) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;
        let cols = self.cols as usize;
        if row >= self.cells.len() {
            return;
        }
        for _ in 0..count {
            if col < self.cells[row].len() {
                self.cells[row].remove(col);
                self.cells[row].push(Cell::default());
            }
        }
        self.cells[row].truncate(cols);
    }

    pub fn erase_chars(&mut self, count: u16) {
        let row = self.cursor.row as usize;
        let col = self.cursor.col as usize;
        if row >= self.cells.len() {
            return;
        }
        for i in 0..count as usize {
            if col + i < self.cells[row].len() {
                self.cells[row][col + i] = Cell::default();
            }
        }
    }

    pub fn cursor_visible(&self) -> bool {
        (self.cursor.row as usize) < self.cells.len()
            && (self.cursor.col as usize) < self.cols as usize
    }

    // --- Alternate screen ---

    pub fn enter_alternate_screen(&mut self) {
        if self.alternate_screen.is_some() {
            return;
        }
        let saved = std::mem::replace(
            &mut self.cells,
            (0..self.rows)
                .map(|_| vec![Cell::default(); self.cols as usize])
                .collect(),
        );
        self.alternate_screen = Some(saved);
    }

    pub fn exit_alternate_screen(&mut self) {
        if let Some(main) = self.alternate_screen.take() {
            self.cells = main;
        }
    }
}

/// Simple heuristic for character display width.
/// Returns 2 for CJK Unified Ideographs and fullwidth forms, 1 otherwise.
fn unicode_width(c: char) -> u8 {
    let cp = c as u32;
    if (0x1100..=0x115F).contains(&cp)
        || (0x2E80..=0x9FFF).contains(&cp)
        || (0xAC00..=0xD7AF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE10..=0xFE6F).contains(&cp)
        || (0xFF01..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x20000..=0x2FA1F).contains(&cp)
        || (0x30000..=0x3134F).contains(&cp)
    {
        2
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_grid_has_correct_dimensions() {
        let grid = TerminalGrid::new(24, 80);
        assert_eq!(grid.rows(), 24);
        assert_eq!(grid.cols(), 80);
        assert_eq!(grid.cursor_position(), (0, 0));
        assert_eq!(grid.scroll_region(), (0, 23));
    }

    #[test]
    fn cell_access_returns_default() {
        let grid = TerminalGrid::new(10, 10);
        let cell = grid.cell(0, 0);
        assert_eq!(cell.c, ' ');
        assert_eq!(cell.fg, Color::Default);
    }

    #[test]
    fn cell_mutation() {
        let mut grid = TerminalGrid::new(10, 10);
        grid.cell_mut(5, 5).c = 'X';
        grid.cell_mut(5, 5).attrs |= CellAttributes::BOLD;
        assert_eq!(grid.cell(5, 5).c, 'X');
        assert!(grid.cell(5, 5).bold());
    }

    #[test]
    fn clear_resets_all_cells() {
        let mut grid = TerminalGrid::new(4, 4);
        grid.cell_mut(0, 0).c = 'A';
        grid.cell_mut(3, 3).c = 'Z';
        grid.clear();
        assert_eq!(grid.cell(0, 0).c, ' ');
        assert_eq!(grid.cell(3, 3).c, ' ');
    }

    #[test]
    fn clear_line_resets_single_row() {
        let mut grid = TerminalGrid::new(4, 4);
        grid.cell_mut(1, 0).c = 'A';
        grid.cell_mut(1, 1).c = 'B';
        grid.clear_line(1);
        assert_eq!(grid.cell(1, 0).c, ' ');
        assert_eq!(grid.cell(1, 1).c, ' ');
    }

    #[test]
    fn resize_grow_preserves_content() {
        let mut grid = TerminalGrid::new(4, 4);
        grid.cell_mut(0, 0).c = 'A';
        grid.resize(6, 6);
        assert_eq!(grid.rows(), 6);
        assert_eq!(grid.cols(), 6);
        assert_eq!(grid.cell(0, 0).c, 'A');
    }

    #[test]
    fn resize_shrink_pushes_to_scrollback() {
        let mut grid = TerminalGrid::new(4, 4);
        grid.cell_mut(0, 0).c = 'T';
        grid.resize(2, 4);
        assert_eq!(grid.rows(), 2);
        assert_eq!(grid.scrollback_len(), 2);
        let line = grid.scrollback_line(0).unwrap();
        assert_eq!(line[0].c, 'T');
    }

    #[test]
    fn scroll_up_moves_lines_and_adds_to_scrollback() {
        let mut grid = TerminalGrid::new(3, 4);
        grid.cell_mut(0, 0).c = '0';
        grid.cell_mut(1, 0).c = '1';
        grid.cell_mut(2, 0).c = '2';
        grid.scroll_up();
        assert_eq!(grid.scrollback_len(), 1);
        assert_eq!(grid.scrollback_line(0).unwrap()[0].c, '0');
        assert_eq!(grid.cell(0, 0).c, '1');
        assert_eq!(grid.cell(1, 0).c, '2');
        assert_eq!(grid.cell(2, 0).c, ' ');
    }

    #[test]
    fn scrollback_capacity_enforced() {
        let mut grid = TerminalGrid::with_scrollback_capacity(2, 4, 3);
        for i in 0..5 {
            grid.cell_mut(0, 0).c = char::from(b'A' + i);
            grid.scroll_up();
        }
        assert_eq!(grid.scrollback_len(), 3);
    }

    #[test]
    fn write_char_and_row_text() {
        let mut grid = TerminalGrid::new(4, 10);
        let template = Cell::default();
        for c in "Hello".chars() {
            grid.write_char(c, &template);
        }
        assert_eq!(grid.row_text(0), "Hello");
        assert_eq!(grid.cursor_position(), (0, 5));
    }

    #[test]
    fn write_char_wraps_at_end_of_line() {
        let mut grid = TerminalGrid::new(4, 3);
        let template = Cell::default();
        for c in "ABCDE".chars() {
            grid.write_char(c, &template);
        }
        assert_eq!(grid.row_text(0), "ABC");
        assert_eq!(grid.row_text(1), "DE");
    }

    #[test]
    fn tab_stops_default() {
        let grid = TerminalGrid::new(24, 80);
        assert_eq!(grid.next_tab_stop(0), 8);
        assert_eq!(grid.next_tab_stop(7), 8);
        assert_eq!(grid.next_tab_stop(8), 16);
    }

    #[test]
    fn alternate_screen_roundtrip() {
        let mut grid = TerminalGrid::new(4, 10);
        grid.cell_mut(0, 0).c = 'M';
        grid.enter_alternate_screen();
        assert_eq!(grid.cell(0, 0).c, ' ');
        grid.cell_mut(0, 0).c = 'A';
        grid.exit_alternate_screen();
        assert_eq!(grid.cell(0, 0).c, 'M');
    }

    #[test]
    fn cell_attributes_bitflags() {
        let mut cell = Cell::default();
        cell.attrs = CellAttributes::BOLD | CellAttributes::ITALIC;
        assert!(cell.bold());
        assert!(cell.italic());
        assert!(!cell.underline());
    }

    #[test]
    fn named_color_index() {
        assert_eq!(NamedColor::Red.to_index(), 1);
        assert_eq!(NamedColor::BrightCyan.to_index(), 14);
    }
}
