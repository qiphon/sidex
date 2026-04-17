//! Bracket matching — mirrors VS Code's bracket-matching contribution.
//!
//! Tracks the matching bracket pair at the current cursor position,
//! bracket pair colorization (rainbow brackets), and bracket pair guides.

use sidex_text::{Buffer, Position, Range};

/// A matched pair of brackets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketPair {
    pub open: Range,
    pub close: Range,
}

/// A color assignment for a bracket pair at a given nesting depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BracketColor {
    /// Nesting depth (0-based).
    pub depth: u32,
    /// Color index into the bracket colorization palette (cycles).
    pub color_index: u32,
}

/// A vertical guide line connecting a bracket pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketGuide {
    /// The column where the guide is drawn.
    pub column: u32,
    /// Start line of the guide (exclusive of the bracket line).
    pub start_line: u32,
    /// End line of the guide (exclusive of the closing bracket line).
    pub end_line: u32,
    /// Nesting depth for color assignment.
    pub depth: u32,
    /// Whether this guide is currently active (cursor inside).
    pub is_active: bool,
}

/// Configuration for bracket features.
#[derive(Debug, Clone)]
pub struct BracketConfig {
    /// Whether bracket pair colorization is enabled.
    pub colorization_enabled: bool,
    /// Number of colors in the colorization palette (cycles after this).
    pub colorization_palette_size: u32,
    /// Whether bracket pair guides are enabled.
    pub guides_enabled: bool,
    /// Whether to show the active guide with a different style.
    pub active_guide_enabled: bool,
}

impl Default for BracketConfig {
    fn default() -> Self {
        Self {
            colorization_enabled: true,
            colorization_palette_size: 6,
            guides_enabled: true,
            active_guide_enabled: true,
        }
    }
}

/// A single colorized bracket in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorizedBracket {
    pub position: Position,
    pub bracket_char: char,
    pub color: BracketColor,
    pub is_open: bool,
}

/// Full state for the bracket-matching feature.
#[derive(Debug, Clone, Default)]
pub struct BracketMatchState {
    /// The current matching bracket pair (if cursor is adjacent to a bracket).
    pub current_pair: Option<BracketPair>,
    /// All colorized brackets in the visible range.
    pub colorized_brackets: Vec<ColorizedBracket>,
    /// All bracket pair guides in the visible range.
    pub guides: Vec<BracketGuide>,
    /// Configuration.
    pub config: BracketConfig,
}

const OPEN_BRACKETS: &[char] = &['(', '[', '{'];
const CLOSE_BRACKETS: &[char] = &[')', ']', '}'];

fn matching_close(ch: char) -> Option<char> {
    OPEN_BRACKETS
        .iter()
        .zip(CLOSE_BRACKETS.iter())
        .find(|(&o, _)| o == ch)
        .map(|(_, &c)| c)
}

fn matching_open(ch: char) -> Option<char> {
    CLOSE_BRACKETS
        .iter()
        .zip(OPEN_BRACKETS.iter())
        .find(|(&c, _)| c == ch)
        .map(|(_, &o)| o)
}

fn is_bracket(ch: char) -> bool {
    OPEN_BRACKETS.contains(&ch) || CLOSE_BRACKETS.contains(&ch)
}

fn is_open_bracket(ch: char) -> bool {
    OPEN_BRACKETS.contains(&ch)
}

impl BracketMatchState {
    /// Updates the bracket match for the given cursor position.
    pub fn update(&mut self, buffer: &Buffer, pos: Position) {
        self.current_pair = Self::find_match(buffer, pos);
    }

    /// Computes bracket pair colorization for the given line range.
    pub fn compute_colorization(&mut self, buffer: &Buffer, start_line: u32, end_line: u32) {
        self.colorized_brackets.clear();
        if !self.config.colorization_enabled {
            return;
        }

        let palette_size = self.config.colorization_palette_size.max(1);
        let mut depth_stack: Vec<(char, Position)> = Vec::new();

        let line_count = buffer.len_lines() as u32;
        let end = end_line.min(line_count.saturating_sub(1));

        for line_idx in 0..=end.min(line_count.saturating_sub(1)) {
            let content = buffer.line_content(line_idx as usize);
            for (col, ch) in content.chars().enumerate() {
                if !is_bracket(ch) {
                    continue;
                }
                let pos = Position::new(line_idx, col as u32);

                if is_open_bracket(ch) {
                    let depth = depth_stack.len() as u32;
                    let color = BracketColor {
                        depth,
                        color_index: depth % palette_size,
                    };
                    depth_stack.push((ch, pos));
                    if line_idx >= start_line {
                        self.colorized_brackets.push(ColorizedBracket {
                            position: pos,
                            bracket_char: ch,
                            color,
                            is_open: true,
                        });
                    }
                } else if let Some(expected_open) = matching_open(ch) {
                    if let Some(stack_pos) =
                        depth_stack.iter().rposition(|&(c, _)| c == expected_open)
                    {
                        let depth = stack_pos as u32;
                        let color = BracketColor {
                            depth,
                            color_index: depth % palette_size,
                        };
                        depth_stack.truncate(stack_pos);
                        if line_idx >= start_line {
                            self.colorized_brackets.push(ColorizedBracket {
                                position: pos,
                                bracket_char: ch,
                                color,
                                is_open: false,
                            });
                        }
                    }
                }
            }
        }
    }

    /// Computes bracket pair guides for visible bracket pairs.
    pub fn compute_guides(
        &mut self,
        buffer: &Buffer,
        start_line: u32,
        end_line: u32,
        cursor_line: u32,
    ) {
        self.guides.clear();
        if !self.config.guides_enabled {
            return;
        }

        let pairs = Self::find_all_pairs(buffer, start_line, end_line);
        let _palette_size = self.config.colorization_palette_size.max(1);

        for (depth, pair) in pairs {
            if pair.open.start.line == pair.close.start.line {
                continue; // same-line pairs don't need guides
            }

            let guide_start = pair.open.start.line + 1;
            let guide_end = pair.close.start.line;

            if guide_start >= guide_end {
                continue;
            }

            let column = pair.open.start.column;
            let is_active = self.config.active_guide_enabled
                && cursor_line >= guide_start
                && cursor_line < guide_end;

            self.guides.push(BracketGuide {
                column,
                start_line: guide_start,
                end_line: guide_end,
                depth,
                is_active,
            });
        }

        self.guides.sort_by_key(|g| (g.start_line, g.column));
    }

    /// Clears the current bracket match.
    pub fn clear(&mut self) {
        self.current_pair = None;
    }

    /// Returns the ranges to highlight (the two bracket characters).
    #[must_use]
    pub fn highlight_ranges(&self) -> Option<(Range, Range)> {
        self.current_pair.as_ref().map(|p| (p.open, p.close))
    }

    /// Returns colorized brackets for a specific line.
    #[must_use]
    pub fn brackets_on_line(&self, line: u32) -> Vec<&ColorizedBracket> {
        self.colorized_brackets
            .iter()
            .filter(|b| b.position.line == line)
            .collect()
    }

    /// Returns guides that overlap a specific line.
    #[must_use]
    pub fn guides_on_line(&self, line: u32) -> Vec<&BracketGuide> {
        self.guides
            .iter()
            .filter(|g| line >= g.start_line && line < g.end_line)
            .collect()
    }

    fn find_all_pairs(buffer: &Buffer, _start_line: u32, end_line: u32) -> Vec<(u32, BracketPair)> {
        let mut pairs = Vec::new();
        let mut stack: Vec<(char, Position, u32)> = Vec::new();
        let line_count = buffer.len_lines() as u32;
        let end = end_line.min(line_count.saturating_sub(1));

        for line_idx in 0..=end {
            let content = buffer.line_content(line_idx as usize);
            for (col, ch) in content.chars().enumerate() {
                let pos = Position::new(line_idx, col as u32);
                if is_open_bracket(ch) {
                    let depth = stack.len() as u32;
                    stack.push((ch, pos, depth));
                } else if let Some(expected) = matching_open(ch) {
                    if let Some(idx) = stack.iter().rposition(|&(c, _, _)| c == expected) {
                        let (_, open_pos, depth) = stack.remove(idx);
                        pairs.push((
                            depth,
                            BracketPair {
                                open: Range::new(
                                    open_pos,
                                    Position::new(open_pos.line, open_pos.column + 1),
                                ),
                                close: Range::new(pos, Position::new(pos.line, col as u32 + 1)),
                            },
                        ));
                    }
                }
            }
        }
        pairs
    }

    fn find_match(buffer: &Buffer, pos: Position) -> Option<BracketPair> {
        let line_count = buffer.len_lines();
        if pos.line as usize >= line_count {
            return None;
        }
        let line = buffer.line_content(pos.line as usize);
        let col = pos.column as usize;

        if let Some(ch) = line.chars().nth(col) {
            if let Some(result) = Self::try_match_at(buffer, pos, ch) {
                return Some(result);
            }
        }

        if col > 0 {
            if let Some(ch) = line.chars().nth(col - 1) {
                let before_pos = Position::new(pos.line, pos.column - 1);
                if let Some(result) = Self::try_match_at(buffer, before_pos, ch) {
                    return Some(result);
                }
            }
        }

        None
    }

    fn try_match_at(buffer: &Buffer, pos: Position, ch: char) -> Option<BracketPair> {
        if let Some(close_ch) = matching_close(ch) {
            let open_range = Range::new(pos, Position::new(pos.line, pos.column + 1));
            if let Some(close_pos) = Self::scan_forward(buffer, pos, ch, close_ch) {
                let close_range = Range::new(
                    close_pos,
                    Position::new(close_pos.line, close_pos.column + 1),
                );
                return Some(BracketPair {
                    open: open_range,
                    close: close_range,
                });
            }
        } else if let Some(open_ch) = matching_open(ch) {
            let close_range = Range::new(pos, Position::new(pos.line, pos.column + 1));
            if let Some(open_pos) = Self::scan_backward(buffer, pos, open_ch, ch) {
                let open_range =
                    Range::new(open_pos, Position::new(open_pos.line, open_pos.column + 1));
                return Some(BracketPair {
                    open: open_range,
                    close: close_range,
                });
            }
        }
        None
    }

    fn scan_forward(buffer: &Buffer, start: Position, open: char, close: char) -> Option<Position> {
        let mut depth = 0i32;
        let line_count = buffer.len_lines();

        for line_idx in (start.line as usize)..line_count {
            let content = buffer.line_content(line_idx);
            let start_col = if line_idx == start.line as usize {
                start.column as usize
            } else {
                0
            };

            for (ci, ch) in content.chars().enumerate().skip(start_col) {
                if ch == open {
                    depth += 1;
                } else if ch == close {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Position::new(line_idx as u32, ci as u32));
                    }
                }
            }
        }
        None
    }

    fn scan_backward(
        buffer: &Buffer,
        start: Position,
        open: char,
        close: char,
    ) -> Option<Position> {
        let mut depth = 0i32;

        for line_idx in (0..=start.line as usize).rev() {
            let content = buffer.line_content(line_idx);
            let chars: Vec<char> = content.chars().collect();
            let end_col = if line_idx == start.line as usize {
                start.column as usize
            } else {
                chars.len().saturating_sub(1)
            };

            for ci in (0..=end_col).rev() {
                if ci >= chars.len() {
                    continue;
                }
                let ch = chars[ci];
                if ch == close {
                    depth += 1;
                } else if ch == open {
                    depth -= 1;
                    if depth == 0 {
                        return Some(Position::new(line_idx as u32, ci as u32));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn matches_parens() {
        let buffer = buf("(hello)");
        let mut state = BracketMatchState::default();
        state.update(&buffer, Position::new(0, 0));
        let pair = state.current_pair.as_ref().unwrap();
        assert_eq!(pair.open.start.column, 0);
        assert_eq!(pair.close.start.column, 6);
    }

    #[test]
    fn no_match_without_bracket() {
        let buffer = buf("hello");
        let mut state = BracketMatchState::default();
        state.update(&buffer, Position::new(0, 2));
        assert!(state.current_pair.is_none());
    }

    #[test]
    fn colorization_assigns_depths() {
        let buffer = buf("((()))");
        let mut state = BracketMatchState::default();
        state.config.colorization_enabled = true;
        state.config.colorization_palette_size = 3;
        state.compute_colorization(&buffer, 0, 0);
        assert_eq!(state.colorized_brackets.len(), 6);
        assert_eq!(state.colorized_brackets[0].color.depth, 0);
        assert_eq!(state.colorized_brackets[1].color.depth, 1);
        assert_eq!(state.colorized_brackets[2].color.depth, 2);
    }

    #[test]
    fn guides_span_multiline() {
        let buffer = buf("{\n  a\n  b\n}");
        let mut state = BracketMatchState::default();
        state.config.guides_enabled = true;
        state.compute_guides(&buffer, 0, 3, 2);
        assert!(!state.guides.is_empty());
        let g = &state.guides[0];
        assert_eq!(g.start_line, 1);
        assert_eq!(g.end_line, 3);
        assert!(g.is_active); // cursor on line 2 is inside
    }

    #[test]
    fn disabled_colorization() {
        let buffer = buf("()");
        let mut state = BracketMatchState::default();
        state.config.colorization_enabled = false;
        state.compute_colorization(&buffer, 0, 0);
        assert!(state.colorized_brackets.is_empty());
    }
}
