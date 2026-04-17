//! Error navigation — F8 / Shift+F8 to jump between diagnostics in a file
//! or across workspace files, with an inline peek view for the error.

use sidex_text::{Position, Range};

// ── NavigationTarget ────────────────────────────────────────────────────────

/// Where an error navigation command should jump to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationTarget {
    /// File URI (for cross-file navigation).
    pub uri: String,
    /// Position to place the cursor.
    pub position: Position,
    /// The full range of the diagnostic.
    pub range: Range,
    /// The diagnostic message to show in the peek view.
    pub message: String,
    /// Severity label.
    pub severity: ErrorSeverity,
    /// Source of the diagnostic.
    pub source: Option<String>,
}

/// Error severity for navigation display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl ErrorSeverity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Information => "Info",
            Self::Hint => "Hint",
        }
    }
}

// ── ErrorNavigationState ────────────────────────────────────────────────────

/// State for the error navigation feature (F8/Shift+F8 cycling).
#[derive(Debug, Clone)]
pub struct ErrorNavigationState {
    /// All known diagnostics in the current file, sorted by position.
    diagnostics: Vec<NavigationTarget>,
    /// Index of the currently focused diagnostic (-1 = none).
    current_index: Option<usize>,
    /// Whether the peek view is showing.
    pub peek_visible: bool,
    /// The anchor line for the inline peek zone.
    pub peek_anchor_line: u32,
    /// Height of the peek zone in lines.
    pub peek_height_lines: u32,
}

impl Default for ErrorNavigationState {
    fn default() -> Self {
        Self {
            diagnostics: Vec::new(),
            current_index: None,
            peek_visible: false,
            peek_anchor_line: 0,
            peek_height_lines: 8,
        }
    }
}

impl ErrorNavigationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the diagnostics for the current file (must be sorted by position).
    pub fn set_diagnostics(&mut self, diagnostics: Vec<NavigationTarget>) {
        self.diagnostics = diagnostics;
        self.diagnostics
            .sort_by_key(|d| (d.range.start.line, d.range.start.column));
        self.current_index = None;
        self.peek_visible = false;
    }

    /// F8: jump to next diagnostic in file.
    pub fn go_to_next_error(&mut self, current_pos: Position) -> Option<&NavigationTarget> {
        if self.diagnostics.is_empty() {
            return None;
        }

        let next_idx = match self.current_index {
            Some(idx) => (idx + 1) % self.diagnostics.len(),
            None => {
                self.diagnostics
                    .iter()
                    .position(|d| d.range.start > current_pos)
                    .unwrap_or(0)
            }
        };

        self.current_index = Some(next_idx);
        let target = &self.diagnostics[next_idx];
        self.peek_anchor_line = target.range.start.line;
        self.peek_visible = true;
        Some(target)
    }

    /// Shift+F8: jump to previous diagnostic in file.
    pub fn go_to_prev_error(&mut self, current_pos: Position) -> Option<&NavigationTarget> {
        if self.diagnostics.is_empty() {
            return None;
        }

        let prev_idx = match self.current_index {
            Some(idx) => {
                if idx == 0 {
                    self.diagnostics.len() - 1
                } else {
                    idx - 1
                }
            }
            None => {
                self.diagnostics
                    .iter()
                    .rposition(|d| d.range.start < current_pos)
                    .unwrap_or(self.diagnostics.len() - 1)
            }
        };

        self.current_index = Some(prev_idx);
        let target = &self.diagnostics[prev_idx];
        self.peek_anchor_line = target.range.start.line;
        self.peek_visible = true;
        Some(target)
    }

    /// Close the error peek view.
    pub fn close_peek(&mut self) {
        self.peek_visible = false;
        self.current_index = None;
    }

    /// Returns the currently focused diagnostic target.
    pub fn current(&self) -> Option<&NavigationTarget> {
        self.current_index
            .and_then(|idx| self.diagnostics.get(idx))
    }

    /// Returns `(current_1based, total)` for display in the peek header.
    pub fn count_display(&self) -> (usize, usize) {
        match self.current_index {
            Some(idx) => (idx + 1, self.diagnostics.len()),
            None => (0, self.diagnostics.len()),
        }
    }

    /// Returns the total number of diagnostics.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Returns `true` if there are no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

// ── Workspace-level navigation ──────────────────────────────────────────────

/// Workspace-level diagnostic navigation across files.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceErrorNavigation {
    /// All diagnostics across all files, sorted by (uri, position).
    targets: Vec<NavigationTarget>,
    current_index: Option<usize>,
}

impl WorkspaceErrorNavigation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_targets(&mut self, mut targets: Vec<NavigationTarget>) {
        targets.sort_by(|a, b| {
            a.uri
                .cmp(&b.uri)
                .then(a.range.start.line.cmp(&b.range.start.line))
                .then(a.range.start.column.cmp(&b.range.start.column))
        });
        self.targets = targets;
        self.current_index = None;
    }

    /// Jump to the next diagnostic across the entire workspace.
    pub fn go_to_next_error_in_workspace(
        &mut self,
        current_uri: &str,
        current_pos: Position,
    ) -> Option<&NavigationTarget> {
        if self.targets.is_empty() {
            return None;
        }

        let next_idx = match self.current_index {
            Some(idx) => (idx + 1) % self.targets.len(),
            None => {
                self.targets
                    .iter()
                    .position(|t| {
                        t.uri.as_str() > current_uri
                            || (t.uri == current_uri && t.range.start > current_pos)
                    })
                    .unwrap_or(0)
            }
        };

        self.current_index = Some(next_idx);
        Some(&self.targets[next_idx])
    }

    pub fn current(&self) -> Option<&NavigationTarget> {
        self.current_index
            .and_then(|idx| self.targets.get(idx))
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(uri: &str, line: u32, col: u32, msg: &str) -> NavigationTarget {
        NavigationTarget {
            uri: uri.to_string(),
            position: Position::new(line, col),
            range: Range::new(Position::new(line, col), Position::new(line, col + 5)),
            message: msg.to_string(),
            severity: ErrorSeverity::Error,
            source: Some("test".to_string()),
        }
    }

    #[test]
    fn next_error_cycles_through_all() {
        let mut state = ErrorNavigationState::new();
        state.set_diagnostics(vec![
            target("a.rs", 2, 0, "err1"),
            target("a.rs", 5, 0, "err2"),
            target("a.rs", 10, 0, "err3"),
        ]);

        let t1 = state.go_to_next_error(Position::new(0, 0)).unwrap().clone();
        assert_eq!(t1.range.start.line, 2);

        let t2 = state.go_to_next_error(Position::new(2, 0)).unwrap().clone();
        assert_eq!(t2.range.start.line, 5);

        let t3 = state.go_to_next_error(Position::new(5, 0)).unwrap().clone();
        assert_eq!(t3.range.start.line, 10);

        // Wraps around
        let t4 = state.go_to_next_error(Position::new(10, 0)).unwrap().clone();
        assert_eq!(t4.range.start.line, 2);
    }

    #[test]
    fn prev_error_cycles_backwards() {
        let mut state = ErrorNavigationState::new();
        state.set_diagnostics(vec![
            target("a.rs", 2, 0, "err1"),
            target("a.rs", 5, 0, "err2"),
            target("a.rs", 10, 0, "err3"),
        ]);

        let t1 = state.go_to_prev_error(Position::new(20, 0)).unwrap().clone();
        assert_eq!(t1.range.start.line, 10);

        let t2 = state.go_to_prev_error(Position::new(10, 0)).unwrap().clone();
        assert_eq!(t2.range.start.line, 5);
    }

    #[test]
    fn next_on_empty_returns_none() {
        let mut state = ErrorNavigationState::new();
        assert!(state.go_to_next_error(Position::new(0, 0)).is_none());
    }

    #[test]
    fn prev_on_empty_returns_none() {
        let mut state = ErrorNavigationState::new();
        assert!(state.go_to_prev_error(Position::new(0, 0)).is_none());
    }

    #[test]
    fn close_peek_resets_state() {
        let mut state = ErrorNavigationState::new();
        state.set_diagnostics(vec![target("a.rs", 1, 0, "err")]);
        state.go_to_next_error(Position::new(0, 0));
        assert!(state.peek_visible);

        state.close_peek();
        assert!(!state.peek_visible);
        assert!(state.current_index.is_none());
    }

    #[test]
    fn count_display_shows_correct_values() {
        let mut state = ErrorNavigationState::new();
        state.set_diagnostics(vec![
            target("a.rs", 1, 0, "err1"),
            target("a.rs", 5, 0, "err2"),
        ]);
        assert_eq!(state.count_display(), (0, 2));

        state.go_to_next_error(Position::new(0, 0));
        assert_eq!(state.count_display(), (1, 2));

        state.go_to_next_error(Position::new(1, 0));
        assert_eq!(state.count_display(), (2, 2));
    }

    #[test]
    fn workspace_navigation_crosses_files() {
        let mut nav = WorkspaceErrorNavigation::new();
        nav.set_targets(vec![
            target("file:///a.rs", 5, 0, "err in a"),
            target("file:///b.rs", 10, 0, "err in b"),
            target("file:///c.rs", 1, 0, "err in c"),
        ]);

        let t1 = nav
            .go_to_next_error_in_workspace("file:///a.rs", Position::new(6, 0))
            .unwrap()
            .clone();
        assert_eq!(t1.uri, "file:///b.rs");

        let t2 = nav
            .go_to_next_error_in_workspace("file:///b.rs", Position::new(10, 0))
            .unwrap()
            .clone();
        assert_eq!(t2.uri, "file:///c.rs");
    }

    #[test]
    fn workspace_navigation_wraps() {
        let mut nav = WorkspaceErrorNavigation::new();
        nav.set_targets(vec![
            target("file:///a.rs", 1, 0, "err"),
        ]);

        let t1 = nav
            .go_to_next_error_in_workspace("file:///z.rs", Position::new(0, 0))
            .unwrap()
            .clone();
        assert_eq!(t1.uri, "file:///a.rs");
    }
}
