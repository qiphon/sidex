//! Word wrap and column ruler features.
//!
//! Computes visual (wrapped) lines from logical lines using word-aware
//! breaking, and provides column ruler positioning for rendering vertical
//! guide lines at configured columns (e.g. 80, 120).

/// How the editor should wrap long lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordWrapMode {
    Off,
    On,
    WordWrapColumn(u32),
    /// Wrap at the minimum of the viewport width and the configured column.
    Bounded,
}

impl Default for WordWrapMode {
    fn default() -> Self { Self::Off }
}

/// A single visual line produced by wrapping a logical line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedLine {
    pub logical_line: u32,
    pub visual_offset: u32,
    pub start_column: u32,
    pub end_column: u32,
    pub indent: u32,
}

/// Computes visual lines by wrapping `content` at `wrap_column`.
///
/// Tabs are expanded to `tab_size` spaces for width calculation. Breaks prefer
/// word boundaries (whitespace) rather than splitting mid-word.
#[must_use]
pub fn compute_wrapped_lines(content: &str, wrap_column: u32, tab_size: u32) -> Vec<WrappedLine> {
    let wrap = wrap_column.max(1) as usize;
    let tab = tab_size.max(1) as usize;
    let mut result = Vec::new();

    for (logical_idx, line) in content.lines().enumerate() {
        let logical_line = logical_idx as u32;
        let expanded = expand_tabs(line, tab);
        let chars: Vec<char> = expanded.chars().collect();

        if chars.is_empty() {
            result.push(WrappedLine { logical_line, visual_offset: 0, start_column: 0, end_column: 0, indent: 0 });
            continue;
        }

        let leading_indent = chars.iter().take_while(|c| c.is_whitespace()).count() as u32;
        let mut offset: u32 = 0;
        let mut col_start: usize = 0;

        while col_start < chars.len() {
            let remaining = chars.len() - col_start;
            let segment_end = if remaining <= wrap {
                chars.len()
            } else {
                find_wrap_break(&chars, col_start, col_start + wrap)
            };

            result.push(WrappedLine {
                logical_line,
                visual_offset: offset,
                start_column: col_start as u32,
                end_column: segment_end as u32,
                indent: if offset == 0 { 0 } else { leading_indent },
            });

            offset += 1;
            col_start = segment_end;
        }
    }

    if content.ends_with('\n') && !content.is_empty() {
        let logical_line = content.lines().count() as u32;
        result.push(WrappedLine { logical_line, visual_offset: 0, start_column: 0, end_column: 0, indent: 0 });
    }

    result
}

/// Finds the best break point in `chars[start..limit]`, preferring the last
/// whitespace boundary. Falls back to `limit` (hard break) when none found.
fn find_wrap_break(chars: &[char], start: usize, limit: usize) -> usize {
    let limit = limit.min(chars.len());
    (start..limit).rev().find(|&i| chars[i].is_whitespace()).map_or(limit, |i| i + 1)
}

fn expand_tabs(line: &str, tab_size: usize) -> String {
    let mut out = String::with_capacity(line.len());
    let mut col = 0usize;
    for ch in line.chars() {
        if ch == '\t' {
            let spaces = tab_size - (col % tab_size);
            (0..spaces).for_each(|_| out.push(' '));
            col += spaces;
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
}

/// Wraps logical lines according to the configured mode.
#[derive(Debug, Clone)]
pub struct WordWrapper {
    pub mode: WordWrapMode,
    pub wrap_column: u32,
    pub tab_size: u32,
    pub viewport_columns: u32,
}

impl WordWrapper {
    #[must_use]
    pub fn new(mode: WordWrapMode, wrap_column: u32, tab_size: u32, vp_cols: u32) -> Self {
        Self { mode, wrap_column, tab_size, viewport_columns: vp_cols }
    }

    #[must_use]
    pub fn effective_wrap_column(&self) -> Option<u32> {
        match self.mode {
            WordWrapMode::Off => None,
            WordWrapMode::On => Some(self.viewport_columns),
            WordWrapMode::WordWrapColumn(col) => Some(col),
            WordWrapMode::Bounded => Some(self.wrap_column.min(self.viewport_columns)),
        }
    }

    #[must_use]
    pub fn wrap(&self, content: &str) -> Vec<WrappedLine> {
        match self.effective_wrap_column() {
            Some(col) => compute_wrapped_lines(content, col, self.tab_size),
            None => one_visual_per_logical(content),
        }
    }
}

/// When wrapping is off, each logical line maps 1:1 to a visual line.
fn one_visual_per_logical(content: &str) -> Vec<WrappedLine> {
    content
        .lines()
        .enumerate()
        .map(|(i, line)| WrappedLine {
            logical_line: i as u32,
            visual_offset: 0,
            start_column: 0,
            end_column: line.len() as u32,
            indent: 0,
        })
        .collect()
}

/// Configuration for column rulers rendered as vertical lines.
#[derive(Debug, Clone)]
pub struct ColumnRulerConfig {
    pub columns: Vec<u32>,
    pub color: [f32; 4],
}

impl Default for ColumnRulerConfig {
    fn default() -> Self {
        Self { columns: vec![80], color: [0.5, 0.5, 0.5, 0.25] }
    }
}

/// Computes pixel x-positions for each ruler.
#[must_use]
pub fn compute_ruler_positions(rulers: &[u32], char_width: f32, gutter_width: f32) -> Vec<f32> {
    rulers.iter().map(|&col| gutter_width + col as f32 * char_width).collect()
}

/// Manages column ruler state and rendering data.
#[derive(Debug, Clone)]
pub struct ColumnRuler {
    pub config: ColumnRulerConfig,
}

impl ColumnRuler {
    #[must_use]
    pub fn new(config: ColumnRulerConfig) -> Self { Self { config } }

    #[must_use]
    pub fn positions(&self, char_width: f32, gutter_width: f32) -> Vec<f32> {
        compute_ruler_positions(&self.config.columns, char_width, gutter_width)
    }
}

/// Toggles word wrap between `Off` and `On`.
pub fn toggle_word_wrap(mode: &mut WordWrapMode) {
    *mode = match *mode {
        WordWrapMode::Off => WordWrapMode::On,
        _ => WordWrapMode::Off,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_wrap_short_line() {
        let lines = compute_wrapped_lines("hello world", 80, 4);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].end_column, 11);
    }
    #[test]
    fn wraps_at_word_boundary() {
        let lines = compute_wrapped_lines("hello world foo", 12, 4);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].end_column, 12);
    }
    #[test]
    fn hard_break_no_spaces() {
        let lines = compute_wrapped_lines("abcdefghij", 5, 4);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].end_column, 5);
    }
    #[test]
    fn tab_expansion() {
        let lines = compute_wrapped_lines("\thello", 80, 4);
        assert_eq!(lines[0].end_column, 9);
    }
    #[test]
    fn ruler_positions() {
        let pos = compute_ruler_positions(&[80, 120], 8.0, 50.0);
        assert_eq!(pos, vec![690.0, 1010.0]);
    }
    #[test]
    fn toggle() {
        let mut mode = WordWrapMode::Off;
        toggle_word_wrap(&mut mode);
        assert_eq!(mode, WordWrapMode::On);
        toggle_word_wrap(&mut mode);
        assert_eq!(mode, WordWrapMode::Off);
    }
    #[test]
    fn wrapper_bounded_picks_smaller() {
        let w = WordWrapper::new(WordWrapMode::Bounded, 100, 4, 80);
        assert_eq!(w.effective_wrap_column(), Some(80));
        let w2 = WordWrapper::new(WordWrapMode::Off, 80, 4, 120);
        assert!(w2.effective_wrap_column().is_none());
    }
}
