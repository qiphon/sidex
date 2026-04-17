//! Go-to-line dialog — mirrors VS Code's Ctrl+G "Go to Line" quick-input.
//!
//! Parses user input like `"42"` (line 42) or `"42:10"` (line 42, column 10)
//! and provides a preview line for the viewport to scroll to while the user
//! is still typing.
//!
//! Provides both a low-level [`GoToLineState`] for internal use and a
//! higher-level [`GoToLineDialog`] view model for the UI layer.

/// Parsed target from the go-to-line input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoToTarget {
    /// 1-based line number (will be converted to 0-based when applied).
    pub line: u32,
    /// 1-based column number, if specified.
    pub column: Option<u32>,
}

/// Full state for the go-to-line dialog.
#[derive(Debug, Clone, Default)]
pub struct GoToLineState {
    /// Whether the dialog is visible.
    pub is_visible: bool,
    /// Current text in the input field.
    pub input: String,
    /// The parsed preview line (0-based) to scroll to while typing.
    pub preview_line: Option<u32>,
    /// Total lines in the current document (used for clamping).
    pub total_lines: u32,
}

impl GoToLineState {
    /// Opens the dialog, optionally pre-filling with the current line (1-based).
    pub fn open(&mut self, current_line_1based: u32, total_lines: u32) {
        self.is_visible = true;
        self.total_lines = total_lines;
        self.input = current_line_1based.to_string();
        self.preview_line = Some(current_line_1based.saturating_sub(1));
    }

    /// Closes the dialog and resets.
    pub fn close(&mut self) {
        self.is_visible = false;
        self.input.clear();
        self.preview_line = None;
    }

    /// Updates the input text and recomputes the preview line.
    pub fn set_input(&mut self, input: String) {
        self.input = input;
        self.preview_line = parse_input(&self.input, self.total_lines).map(|t| t.line - 1);
    }

    /// Confirms the dialog and returns the 0-based target position, if valid.
    #[must_use]
    pub fn confirm(&self) -> Option<(u32, u32)> {
        let target = parse_input(&self.input, self.total_lines)?;
        let line = target.line - 1;
        let col = target.column.map_or(0, |c| c.saturating_sub(1));
        Some((line, col))
    }

    /// Returns a hint string for the input placeholder, e.g.
    /// "Type a line number between 1 and 500".
    #[must_use]
    pub fn placeholder(&self) -> String {
        format!(
            "Type a line number between 1 and {} to go to",
            self.total_lines
        )
    }
}

/// Higher-level dialog view model for the Go to Line feature (Ctrl+G).
///
/// Wraps [`GoToLineState`] and adds display helpers like current line info
/// and hint text for the UI.
#[derive(Debug, Clone)]
pub struct GoToLineDialog {
    pub input: String,
    pub current_line: u32,
    pub total_lines: u32,
    state: GoToLineState,
}

impl Default for GoToLineDialog {
    fn default() -> Self {
        Self {
            input: String::new(),
            current_line: 1,
            total_lines: 1,
            state: GoToLineState::default(),
        }
    }
}

impl GoToLineDialog {
    /// Opens the dialog for the given cursor position.
    pub fn open(&mut self, current_line_1based: u32, total_lines: u32) {
        self.current_line = current_line_1based;
        self.total_lines = total_lines;
        self.input = current_line_1based.to_string();
        self.state.open(current_line_1based, total_lines);
    }

    /// Closes the dialog.
    pub fn close(&mut self) {
        self.input.clear();
        self.state.close();
    }

    /// Returns whether the dialog is visible.
    pub fn is_visible(&self) -> bool {
        self.state.is_visible
    }

    /// Updates the input and syncs with the internal state.
    pub fn set_input(&mut self, input: String) {
        self.input = input.clone();
        self.state.set_input(input);
    }

    /// Returns the preview line (0-based) if the input is valid.
    pub fn preview_line(&self) -> Option<u32> {
        self.state.preview_line
    }

    /// Confirms the dialog and returns the 0-based `(line, column)`.
    pub fn confirm(&self) -> Option<(u32, u32)> {
        self.state.confirm()
    }

    /// Returns a hint string like "Current Line: 42, Total Lines: 500".
    #[must_use]
    pub fn hint_text(&self) -> String {
        format!(
            "Current Line: {}, Total Lines: {}",
            self.current_line, self.total_lines
        )
    }

    /// Returns the placeholder text.
    #[must_use]
    pub fn placeholder(&self) -> String {
        self.state.placeholder()
    }
}

/// Parses input text into a [`GoToTarget`], clamping to `total_lines`.
///
/// Accepted formats:
/// - `"42"`      — line 42
/// - `"42:10"`   — line 42, column 10
/// - `":10"`     — column 10 on current line (returns line 1)
#[must_use]
pub fn parse_input(input: &str, total_lines: u32) -> Option<GoToTarget> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let (line_part, col_part) = if let Some((l, c)) = input.split_once(':') {
        (l.trim(), Some(c.trim()))
    } else {
        (input, None)
    };

    let line: u32 = if line_part.is_empty() {
        1
    } else {
        line_part.parse().ok()?
    };

    if line == 0 {
        return None;
    }
    let line = line.min(total_lines.max(1));

    let column = if let Some(c) = col_part {
        if c.is_empty() {
            None
        } else {
            let col: u32 = c.parse().ok()?;
            if col == 0 {
                None
            } else {
                Some(col)
            }
        }
    } else {
        None
    };

    Some(GoToTarget { line, column })
}

/// Free-standing parser matching the specification:
/// returns `Some((line_1based, optional_column_1based))`.
///
/// Delegates to [`parse_input`] with a high total-lines value so the caller
/// doesn't need to know the document size for simple parsing.
#[must_use]
pub fn parse_go_to_input(input: &str) -> Option<(u32, Option<u32>)> {
    let target = parse_input(input, u32::MAX)?;
    Some((target.line, target.column))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_only() {
        let t = parse_input("42", 500).unwrap();
        assert_eq!(t.line, 42);
        assert_eq!(t.column, None);
    }

    #[test]
    fn parse_line_and_column() {
        let t = parse_input("42:10", 500).unwrap();
        assert_eq!(t.line, 42);
        assert_eq!(t.column, Some(10));
    }

    #[test]
    fn parse_column_only() {
        let t = parse_input(":15", 100).unwrap();
        assert_eq!(t.line, 1);
        assert_eq!(t.column, Some(15));
    }

    #[test]
    fn parse_clamps_to_total_lines() {
        let t = parse_input("9999", 100).unwrap();
        assert_eq!(t.line, 100);
    }

    #[test]
    fn parse_zero_line_is_none() {
        assert!(parse_input("0", 100).is_none());
    }

    #[test]
    fn parse_empty_is_none() {
        assert!(parse_input("", 100).is_none());
    }

    #[test]
    fn parse_garbage_is_none() {
        assert!(parse_input("abc", 100).is_none());
    }

    #[test]
    fn parse_whitespace_trimmed() {
        let t = parse_input("  42 : 10  ", 500).unwrap();
        assert_eq!(t.line, 42);
        assert_eq!(t.column, Some(10));
    }

    #[test]
    fn state_open_and_close() {
        let mut state = GoToLineState::default();
        state.open(10, 500);
        assert!(state.is_visible);
        assert_eq!(state.input, "10");
        assert_eq!(state.preview_line, Some(9)); // 0-based

        state.close();
        assert!(!state.is_visible);
        assert!(state.input.is_empty());
    }

    #[test]
    fn state_set_input_updates_preview() {
        let mut state = GoToLineState::default();
        state.open(1, 100);

        state.set_input("50".into());
        assert_eq!(state.preview_line, Some(49));

        state.set_input("abc".into());
        assert_eq!(state.preview_line, None);
    }

    #[test]
    fn state_confirm() {
        let mut state = GoToLineState::default();
        state.open(1, 100);
        state.set_input("42:10".into());
        let (line, col) = state.confirm().unwrap();
        assert_eq!(line, 41); // 0-based
        assert_eq!(col, 9); // 0-based
    }

    #[test]
    fn state_confirm_line_only() {
        let mut state = GoToLineState::default();
        state.open(1, 100);
        state.set_input("7".into());
        let (line, col) = state.confirm().unwrap();
        assert_eq!(line, 6);
        assert_eq!(col, 0);
    }

    #[test]
    fn placeholder_text() {
        let mut state = GoToLineState::default();
        state.total_lines = 250;
        assert_eq!(
            state.placeholder(),
            "Type a line number between 1 and 250 to go to"
        );
    }

    #[test]
    fn parse_go_to_input_line_only() {
        let (line, col) = parse_go_to_input("42").unwrap();
        assert_eq!(line, 42);
        assert_eq!(col, None);
    }

    #[test]
    fn parse_go_to_input_line_and_col() {
        let (line, col) = parse_go_to_input("42:10").unwrap();
        assert_eq!(line, 42);
        assert_eq!(col, Some(10));
    }

    #[test]
    fn parse_go_to_input_empty() {
        assert!(parse_go_to_input("").is_none());
    }

    #[test]
    fn parse_go_to_input_garbage() {
        assert!(parse_go_to_input("abc").is_none());
    }

    #[test]
    fn dialog_open_and_close() {
        let mut dialog = GoToLineDialog::default();
        dialog.open(10, 500);
        assert!(dialog.is_visible());
        assert_eq!(dialog.input, "10");
        assert_eq!(dialog.current_line, 10);
        assert_eq!(dialog.total_lines, 500);
        assert_eq!(dialog.hint_text(), "Current Line: 10, Total Lines: 500");

        dialog.close();
        assert!(!dialog.is_visible());
        assert!(dialog.input.is_empty());
    }

    #[test]
    fn dialog_set_input_and_confirm() {
        let mut dialog = GoToLineDialog::default();
        dialog.open(1, 100);
        dialog.set_input("42:10".into());
        assert_eq!(dialog.preview_line(), Some(41));
        let (line, col) = dialog.confirm().unwrap();
        assert_eq!(line, 41);
        assert_eq!(col, 9);
    }
}
