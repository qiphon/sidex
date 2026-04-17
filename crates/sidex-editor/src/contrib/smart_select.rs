//! Smart selection — mirrors VS Code's `smartSelect` contribution.
//!
//! Expands/shrinks selection based on semantic structure (AST or heuristics):
//! word → expression → statement → block → function → file.

use sidex_text::{Buffer, Position, Range};

/// A selection range with an optional parent (for hierarchical expansion).
#[derive(Debug, Clone)]
pub struct SelectionRange {
    /// The range for this level.
    pub range: Range,
    /// The parent (broader) selection range.
    pub parent: Option<Box<SelectionRange>>,
}

impl SelectionRange {
    /// Creates a leaf selection range (no parent).
    pub fn leaf(range: Range) -> Self {
        Self {
            range,
            parent: None,
        }
    }

    /// Creates a selection range with a parent.
    pub fn with_parent(range: Range, parent: SelectionRange) -> Self {
        Self {
            range,
            parent: Some(Box::new(parent)),
        }
    }

    /// Returns the chain of ranges from narrowest to broadest.
    #[must_use]
    pub fn chain(&self) -> Vec<Range> {
        let mut result = vec![self.range];
        let mut current = self.parent.as_ref();
        while let Some(p) = current {
            result.push(p.range);
            current = p.parent.as_ref();
        }
        result
    }
}

/// Full state for the smart-selection feature.
#[derive(Debug, Clone, Default)]
pub struct SmartSelectState {
    /// The current selection range chain (from narrow to broad).
    pub ranges: Vec<Range>,
    /// Current index in the chain (0 = narrowest).
    pub current_index: usize,
    /// Whether a smart selection is active.
    pub is_active: bool,
}

impl SmartSelectState {
    /// Initialises smart selection from a `SelectionRange` hierarchy (LSP).
    pub fn set_from_lsp(&mut self, selection_range: SelectionRange) {
        self.ranges = selection_range.chain();
        self.current_index = 0;
        self.is_active = !self.ranges.is_empty();
    }

    /// Computes smart selection ranges using heuristics (no AST required).
    /// The order is: word → quoted string → brackets → line → block → full doc.
    pub fn compute_from_heuristics(&mut self, buffer: &Buffer, pos: Position) {
        let mut ranges = Vec::new();

        if let Some(word_range) = word_range_at(buffer, pos) {
            ranges.push(word_range);
        }

        if let Some(quoted) = quoted_string_range(buffer, pos) {
            if ranges.last() != Some(&quoted) {
                ranges.push(quoted);
            }
        }

        if let Some(bracket) = bracket_content_range(buffer, pos) {
            if ranges.last() != Some(&bracket) {
                ranges.push(bracket);
            }
        }

        let line_range = line_content_range(buffer, pos.line);
        if ranges.last() != Some(&line_range) {
            ranges.push(line_range);
        }

        if let Some(block) = indentation_block_range(buffer, pos.line) {
            if ranges.last() != Some(&block) {
                ranges.push(block);
            }
        }

        let full_range = full_document_range(buffer);
        if ranges.last() != Some(&full_range) {
            ranges.push(full_range);
        }

        self.ranges = ranges;
        self.current_index = 0;
        self.is_active = !self.ranges.is_empty();
    }

    /// Expands the selection to the next broader range.
    /// Returns the new range, if available.
    pub fn expand(&mut self) -> Option<Range> {
        if !self.is_active || self.ranges.is_empty() {
            return None;
        }
        if self.current_index + 1 < self.ranges.len() {
            self.current_index += 1;
        }
        self.ranges.get(self.current_index).copied()
    }

    /// Shrinks the selection to the next narrower range.
    /// Returns the new range, if available.
    pub fn shrink(&mut self) -> Option<Range> {
        if !self.is_active || self.ranges.is_empty() {
            return None;
        }
        if self.current_index > 0 {
            self.current_index -= 1;
        }
        self.ranges.get(self.current_index).copied()
    }

    /// Returns the current selection range.
    #[must_use]
    pub fn current_range(&self) -> Option<Range> {
        self.ranges.get(self.current_index).copied()
    }

    /// Resets the smart selection state.
    pub fn reset(&mut self) {
        self.ranges.clear();
        self.current_index = 0;
        self.is_active = false;
    }
}

fn word_range_at(buffer: &Buffer, pos: Position) -> Option<Range> {
    if pos.line as usize >= buffer.len_lines() {
        return None;
    }
    let line = buffer.line_content(pos.line as usize);
    let chars: Vec<char> = line.chars().collect();
    let col = pos.column as usize;

    if col >= chars.len() || (!chars[col].is_alphanumeric() && chars[col] != '_') {
        return None;
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

    Some(Range::new(
        Position::new(pos.line, start as u32),
        Position::new(pos.line, end as u32),
    ))
}

fn quoted_string_range(buffer: &Buffer, pos: Position) -> Option<Range> {
    if pos.line as usize >= buffer.len_lines() {
        return None;
    }
    let line = buffer.line_content(pos.line as usize);
    let col = pos.column as usize;

    for quote in &['"', '\'', '`'] {
        let before = line[..col].rfind(*quote);
        let after = line[col..].find(*quote).map(|i| col + i);

        if let (Some(s), Some(e)) = (before, after) {
            if e > s {
                return Some(Range::new(
                    Position::new(pos.line, s as u32),
                    Position::new(pos.line, (e + 1) as u32),
                ));
            }
        }
    }
    None
}

fn bracket_content_range(buffer: &Buffer, pos: Position) -> Option<Range> {
    let pairs = [('(', ')'), ('[', ']'), ('{', '}')];

    for (open, close) in &pairs {
        let mut depth = 0i32;
        let mut open_pos = None;

        // Scan backward to find opening bracket
        for line_idx in (0..=pos.line as usize).rev() {
            let content = buffer.line_content(line_idx);
            let chars: Vec<char> = content.chars().collect();
            let end_col = if line_idx == pos.line as usize {
                pos.column as usize
            } else {
                chars.len().saturating_sub(1)
            };
            for ci in (0..=end_col.min(chars.len().saturating_sub(1))).rev() {
                if chars[ci] == *close {
                    depth += 1;
                } else if chars[ci] == *open {
                    if depth == 0 {
                        open_pos = Some(Position::new(line_idx as u32, ci as u32));
                        break;
                    }
                    depth -= 1;
                }
            }
            if open_pos.is_some() {
                break;
            }
        }

        let Some(op) = open_pos else {
            continue;
        };

        // Scan forward to find closing bracket
        depth = 0;
        let line_count = buffer.len_lines();
        for line_idx in (op.line as usize)..line_count {
            let content = buffer.line_content(line_idx);
            let start_col = if line_idx == op.line as usize {
                op.column as usize
            } else {
                0
            };
            for (ci, ch) in content.chars().enumerate().skip(start_col) {
                if ch == *open {
                    depth += 1;
                } else if ch == *close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Range::new(
                            Position::new(op.line, op.column),
                            Position::new(line_idx as u32, ci as u32 + 1),
                        ));
                    }
                }
            }
        }
    }
    None
}

fn line_content_range(buffer: &Buffer, line: u32) -> Range {
    let content = buffer.line_content(line as usize);
    let trimmed = content.trim_start();
    let start_col = (content.len() - trimmed.len()) as u32;
    let end_col = content.trim_end().len() as u32;
    Range::new(
        Position::new(line, start_col),
        Position::new(line, end_col.max(start_col)),
    )
}

fn indentation_block_range(buffer: &Buffer, line: u32) -> Option<Range> {
    let line_count = buffer.len_lines();
    if line as usize >= line_count {
        return None;
    }

    let content = buffer.line_content(line as usize);
    let indent = content.len() - content.trim_start().len();

    let mut start = line;
    while start > 0 {
        let prev = buffer.line_content((start - 1) as usize);
        let prev_indent = prev.len() - prev.trim_start().len();
        if prev.trim().is_empty() || prev_indent < indent {
            break;
        }
        start -= 1;
    }

    let mut end = line;
    while (end + 1) < line_count as u32 {
        let next = buffer.line_content((end + 1) as usize);
        let next_indent = next.len() - next.trim_start().len();
        if next.trim().is_empty() || next_indent < indent {
            break;
        }
        end += 1;
    }

    if start == end {
        return None;
    }

    let end_col = buffer.line_content(end as usize).len() as u32;
    Some(Range::new(
        Position::new(start, 0),
        Position::new(end, end_col),
    ))
}

fn full_document_range(buffer: &Buffer) -> Range {
    let last_line = buffer.len_lines().saturating_sub(1);
    let last_col = buffer.line_content(last_line).len() as u32;
    Range::new(
        Position::new(0, 0),
        Position::new(last_line as u32, last_col),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn expand_and_shrink() {
        let buffer = buf("fn foo() {\n    let x = 1;\n}");
        let mut state = SmartSelectState::default();
        state.compute_from_heuristics(&buffer, Position::new(1, 8));

        assert!(state.is_active);
        let first = state.current_range().unwrap();

        let expanded = state.expand().unwrap();
        assert!(
            expanded.start.line <= first.start.line
                || expanded.start.column <= first.start.column
                || expanded.end.line >= first.end.line
                || expanded.end.column >= first.end.column
        );

        let shrunk = state.shrink().unwrap();
        assert_eq!(shrunk, first);
    }

    #[test]
    fn word_selection() {
        let buffer = buf("let hello = world;");
        let range = word_range_at(&buffer, Position::new(0, 5)).unwrap();
        assert_eq!(range.start.column, 4);
        assert_eq!(range.end.column, 9);
    }

    #[test]
    fn selection_range_chain() {
        let inner = SelectionRange::leaf(Range::new(Position::new(0, 0), Position::new(0, 5)));
        let outer = SelectionRange::with_parent(
            Range::new(Position::new(0, 0), Position::new(0, 10)),
            inner,
        );
        let chain = outer.chain();
        assert_eq!(chain.len(), 2);
    }
}
