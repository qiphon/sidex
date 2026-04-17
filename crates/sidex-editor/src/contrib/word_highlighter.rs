//! Word highlighter — mirrors VS Code's `WordHighlighter` contribution.
//!
//! Highlights all occurrences of the word under the cursor (debounced).  Also
//! supports LSP `documentHighlight` results with read/write distinction.

use sidex_text::{Buffer, Position, Range};

/// The kind of a document highlight (from LSP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DocumentHighlightKind {
    /// A textual occurrence.
    #[default]
    Text,
    /// A read-access to a symbol.
    Read,
    /// A write-access to a symbol.
    Write,
}

/// A single highlight range with its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightRange {
    pub range: Range,
    pub kind: DocumentHighlightKind,
}

/// Full state for the word-highlighter feature.
#[derive(Debug, Clone)]
pub struct WordHighlightState {
    /// The currently highlighted ranges.
    pub highlights: Vec<HighlightRange>,
    /// The word that is currently highlighted (for display/debugging).
    pub highlighted_word: Option<String>,
    /// Debounce delay in milliseconds (default 250ms).
    pub debounce_ms: u64,
    /// Whether a highlight request is in-flight.
    pub is_loading: bool,
    /// Timestamp (ms) of the last cursor movement (for debounce).
    pub last_cursor_move_ms: u64,
    /// Whether document highlights from LSP are being used (vs word-based).
    pub using_lsp: bool,
    /// Whether word highlighting is enabled.
    pub enabled: bool,
}

impl Default for WordHighlightState {
    fn default() -> Self {
        Self {
            highlights: Vec::new(),
            highlighted_word: None,
            debounce_ms: 250,
            is_loading: false,
            last_cursor_move_ms: 0,
            using_lsp: false,
            enabled: true,
        }
    }
}

impl WordHighlightState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a cursor move for debounce timing.
    pub fn on_cursor_move(&mut self, timestamp_ms: u64) {
        self.last_cursor_move_ms = timestamp_ms;
    }

    /// Returns `true` if enough time has passed since last cursor move.
    #[must_use]
    pub fn should_trigger(&self, now_ms: u64) -> bool {
        self.enabled && now_ms.saturating_sub(self.last_cursor_move_ms) >= self.debounce_ms
    }

    /// Computes textual word highlights by finding all occurrences of the word
    /// at the cursor position.  This is the fallback when no LSP provider is
    /// available.
    pub fn highlight_word_at_cursor(&mut self, buffer: &Buffer, pos: Position) {
        self.highlights.clear();
        self.highlighted_word = None;
        self.using_lsp = false;

        if !self.enabled {
            return;
        }

        let line_count = buffer.len_lines();
        if pos.line as usize >= line_count {
            return;
        }

        let line = buffer.line_content(pos.line as usize);
        let col = pos.column as usize;

        let chars: Vec<char> = line.chars().collect();
        if col >= chars.len() || !chars[col].is_alphanumeric() && chars[col] != '_' {
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

        self.highlighted_word = Some(word.clone());

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
                    self.highlights.push(HighlightRange {
                        range: Range::new(
                            Position::new(line_idx as u32, abs_start as u32),
                            Position::new(line_idx as u32, abs_end as u32),
                        ),
                        kind: DocumentHighlightKind::Text,
                    });
                }

                search_start = abs_end;
            }
        }
    }

    /// Receives LSP document-highlight results.
    pub fn set_lsp_highlights(&mut self, highlights: Vec<HighlightRange>) {
        self.highlights = highlights;
        self.is_loading = false;
        self.using_lsp = true;
    }

    /// Clears all highlights.
    pub fn clear(&mut self) {
        self.highlights.clear();
        self.highlighted_word = None;
        self.is_loading = false;
        self.using_lsp = false;
    }

    /// Returns just the ranges for rendering.
    #[must_use]
    pub fn ranges(&self) -> Vec<Range> {
        self.highlights.iter().map(|h| h.range).collect()
    }

    /// Returns the read-access highlight ranges.
    #[must_use]
    pub fn read_ranges(&self) -> Vec<Range> {
        self.highlights
            .iter()
            .filter(|h| h.kind == DocumentHighlightKind::Read)
            .map(|h| h.range)
            .collect()
    }

    /// Returns the write-access highlight ranges.
    #[must_use]
    pub fn write_ranges(&self) -> Vec<Range> {
        self.highlights
            .iter()
            .filter(|h| h.kind == DocumentHighlightKind::Write)
            .map(|h| h.range)
            .collect()
    }

    /// Returns the occurrence count for status bar display.
    #[must_use]
    pub fn occurrence_count(&self) -> usize {
        self.highlights.len()
    }

    /// Returns a formatted string for the status bar like "3 occurrences"
    /// or "2 reads, 1 write".
    #[must_use]
    pub fn status_bar_label(&self) -> String {
        if self.highlights.is_empty() {
            return String::new();
        }

        if !self.using_lsp {
            let count = self.highlights.len();
            return if count == 1 {
                "1 occurrence".to_string()
            } else {
                format!("{count} occurrences")
            };
        }

        let reads = self
            .highlights
            .iter()
            .filter(|h| h.kind == DocumentHighlightKind::Read)
            .count();
        let writes = self
            .highlights
            .iter()
            .filter(|h| h.kind == DocumentHighlightKind::Write)
            .count();
        let texts = self
            .highlights
            .iter()
            .filter(|h| h.kind == DocumentHighlightKind::Text)
            .count();

        let mut parts = Vec::new();
        if reads > 0 {
            parts.push(format!("{reads} read{}", if reads == 1 { "" } else { "s" }));
        }
        if writes > 0 {
            parts.push(format!(
                "{writes} write{}",
                if writes == 1 { "" } else { "s" }
            ));
        }
        if texts > 0 {
            parts.push(format!(
                "{texts} occurrence{}",
                if texts == 1 { "" } else { "s" }
            ));
        }

        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn highlights_word_occurrences() {
        let buffer = buf("let foo = foo + bar;");
        let mut state = WordHighlightState::new();
        state.highlight_word_at_cursor(&buffer, Position::new(0, 4));
        assert_eq!(state.highlighted_word.as_deref(), Some("foo"));
        assert_eq!(state.highlights.len(), 2);
    }

    #[test]
    fn no_highlight_on_whitespace() {
        let buffer = buf("hello world");
        let mut state = WordHighlightState::new();
        state.highlight_word_at_cursor(&buffer, Position::new(0, 5));
        assert!(state.highlights.is_empty());
    }

    #[test]
    fn lsp_read_write_highlights() {
        let mut state = WordHighlightState::new();
        state.set_lsp_highlights(vec![
            HighlightRange {
                range: Range::new(Position::new(0, 0), Position::new(0, 3)),
                kind: DocumentHighlightKind::Write,
            },
            HighlightRange {
                range: Range::new(Position::new(1, 5), Position::new(1, 8)),
                kind: DocumentHighlightKind::Read,
            },
            HighlightRange {
                range: Range::new(Position::new(2, 5), Position::new(2, 8)),
                kind: DocumentHighlightKind::Read,
            },
        ]);
        assert_eq!(state.write_ranges().len(), 1);
        assert_eq!(state.read_ranges().len(), 2);
        assert_eq!(state.status_bar_label(), "2 reads, 1 write");
    }

    #[test]
    fn debounce_timing() {
        let mut state = WordHighlightState::new();
        state.debounce_ms = 100;
        state.on_cursor_move(1000);
        assert!(!state.should_trigger(1050));
        assert!(state.should_trigger(1100));
    }

    #[test]
    fn occurrence_count_in_status_bar() {
        let buffer = buf("foo foo foo");
        let mut state = WordHighlightState::new();
        state.highlight_word_at_cursor(&buffer, Position::new(0, 0));
        assert_eq!(state.occurrence_count(), 3);
        assert_eq!(state.status_bar_label(), "3 occurrences");
    }
}
