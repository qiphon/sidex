//! Quick Open dialog — Ctrl+P file picker with fuzzy matching.
//!
//! Mirrors VS Code's Quick Open with support for:
//! - Empty query → recently opened files
//! - Typing → fuzzy file matching from workspace index
//! - `path:line` → open file at line (e.g. `main.rs:42`)
//! - `@symbol` → search symbols in the current file
//! - `#symbol` → search symbols across the workspace
//! - `:line` → go to line in the current file

use std::path::PathBuf;

/// The kind of query the user has typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuickOpenMode {
    /// Empty query — show recent files.
    RecentFiles,
    /// Normal text — fuzzy match workspace files.
    FileSearch(String),
    /// `path:line` — open specific file at a line.
    FileAtLine { query: String, line: u32 },
    /// `@symbol` — search symbols in the current file.
    SymbolInFile(String),
    /// `#symbol` — search symbols across the workspace.
    SymbolInWorkspace(String),
    /// `:line` — go to line in the current file.
    GoToLine(u32),
}

/// A single item shown in the quick-open list.
#[derive(Debug, Clone)]
pub struct QuickOpenItem {
    /// Display label (filename or symbol name).
    pub label: String,
    /// Description/subtitle (relative path or container).
    pub description: String,
    /// Full path to the file (if applicable).
    pub path: Option<PathBuf>,
    /// Target line number (0-based), if the query specifies one.
    pub target_line: Option<u32>,
    /// Fuzzy match score for sorting.
    pub score: f64,
    /// Character positions in `label` that matched the query.
    pub match_positions: Vec<usize>,
}

/// Full state for the Ctrl+P quick-open dialog.
#[derive(Debug, Clone)]
pub struct QuickOpenState {
    /// Whether the dialog is currently shown.
    pub is_visible: bool,
    /// Raw input text from the user.
    pub input: String,
    /// Parsed mode derived from `input`.
    pub mode: QuickOpenMode,
    /// Currently displayed items.
    pub items: Vec<QuickOpenItem>,
    /// Index of the selected item.
    pub selected: usize,
}

impl Default for QuickOpenState {
    fn default() -> Self {
        Self {
            is_visible: false,
            input: String::new(),
            mode: QuickOpenMode::RecentFiles,
            items: Vec::new(),
            selected: 0,
        }
    }
}

impl QuickOpenState {
    /// Open the quick-open dialog with an empty filter showing recent files.
    pub fn show(&mut self) {
        self.is_visible = true;
        self.input.clear();
        self.mode = QuickOpenMode::RecentFiles;
        self.selected = 0;
    }

    /// Close the dialog and reset state.
    pub fn cancel(&mut self) {
        self.is_visible = false;
        self.input.clear();
        self.mode = QuickOpenMode::RecentFiles;
        self.items.clear();
        self.selected = 0;
    }

    /// Update the filter query and reparse the mode.
    pub fn filter(&mut self, query: &str) {
        self.input = query.to_string();
        self.mode = parse_mode(query);
        self.selected = 0;
    }

    /// Set the filtered result items (called by the application after
    /// computing matches from the workspace index / LSP).
    pub fn set_items(&mut self, items: Vec<QuickOpenItem>) {
        self.items = items;
        if self.selected >= self.items.len() {
            self.selected = 0;
        }
    }

    /// Returns the currently selected item, if any.
    pub fn select(&self) -> Option<&QuickOpenItem> {
        self.items.get(self.selected)
    }

    /// Returns all current items (for rendering the list).
    pub fn items(&self) -> &[QuickOpenItem] {
        &self.items
    }

    /// Move selection down by one, wrapping around.
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Move selection up by one, wrapping around.
    pub fn select_prev(&mut self) {
        if !self.items.is_empty() {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Returns the placeholder text for the input field.
    pub fn placeholder(&self) -> &str {
        "Search files by name (append : to go to line, @ for symbols)"
    }
}

/// Parse the raw input into a [`QuickOpenMode`].
pub fn parse_mode(input: &str) -> QuickOpenMode {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return QuickOpenMode::RecentFiles;
    }

    if let Some(symbol) = trimmed.strip_prefix('@') {
        return QuickOpenMode::SymbolInFile(symbol.to_string());
    }

    if let Some(symbol) = trimmed.strip_prefix('#') {
        return QuickOpenMode::SymbolInWorkspace(symbol.to_string());
    }

    if let Some(line_str) = trimmed.strip_prefix(':') {
        if let Ok(line) = line_str.trim().parse::<u32>() {
            if line > 0 {
                return QuickOpenMode::GoToLine(line);
            }
        }
        return QuickOpenMode::RecentFiles;
    }

    if let Some((path, line_str)) = trimmed.rsplit_once(':') {
        if !path.is_empty() {
            if let Ok(line) = line_str.trim().parse::<u32>() {
                if line > 0 {
                    return QuickOpenMode::FileAtLine {
                        query: path.to_string(),
                        line,
                    };
                }
            }
        }
    }

    QuickOpenMode::FileSearch(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_recent_files() {
        assert_eq!(parse_mode(""), QuickOpenMode::RecentFiles);
        assert_eq!(parse_mode("   "), QuickOpenMode::RecentFiles);
    }

    #[test]
    fn plain_text_is_file_search() {
        assert_eq!(
            parse_mode("main.rs"),
            QuickOpenMode::FileSearch("main.rs".into())
        );
    }

    #[test]
    fn at_prefix_is_symbol_in_file() {
        assert_eq!(
            parse_mode("@handleClick"),
            QuickOpenMode::SymbolInFile("handleClick".into())
        );
    }

    #[test]
    fn hash_prefix_is_symbol_in_workspace() {
        assert_eq!(
            parse_mode("#MyClass"),
            QuickOpenMode::SymbolInWorkspace("MyClass".into())
        );
    }

    #[test]
    fn colon_prefix_is_goto_line() {
        assert_eq!(parse_mode(":42"), QuickOpenMode::GoToLine(42));
    }

    #[test]
    fn colon_zero_is_recent() {
        assert_eq!(parse_mode(":0"), QuickOpenMode::RecentFiles);
    }

    #[test]
    fn file_at_line() {
        assert_eq!(
            parse_mode("main.rs:42"),
            QuickOpenMode::FileAtLine {
                query: "main.rs".into(),
                line: 42,
            }
        );
    }

    #[test]
    fn file_at_line_with_path() {
        assert_eq!(
            parse_mode("src/utils.rs:10"),
            QuickOpenMode::FileAtLine {
                query: "src/utils.rs".into(),
                line: 10,
            }
        );
    }

    #[test]
    fn colon_with_non_numeric_is_file_search() {
        assert_eq!(
            parse_mode("main.rs:abc"),
            QuickOpenMode::FileSearch("main.rs:abc".into())
        );
    }

    #[test]
    fn show_and_cancel() {
        let mut state = QuickOpenState::default();
        assert!(!state.is_visible);

        state.show();
        assert!(state.is_visible);
        assert!(state.input.is_empty());
        assert_eq!(state.mode, QuickOpenMode::RecentFiles);

        state.cancel();
        assert!(!state.is_visible);
    }

    #[test]
    fn filter_updates_mode() {
        let mut state = QuickOpenState::default();
        state.show();

        state.filter("main.rs");
        assert_eq!(state.mode, QuickOpenMode::FileSearch("main.rs".into()));

        state.filter("@foo");
        assert_eq!(state.mode, QuickOpenMode::SymbolInFile("foo".into()));

        state.filter(":100");
        assert_eq!(state.mode, QuickOpenMode::GoToLine(100));
    }

    #[test]
    fn select_navigation() {
        let mut state = QuickOpenState::default();
        state.set_items(vec![
            QuickOpenItem {
                label: "a.rs".into(),
                description: "src/".into(),
                path: Some(PathBuf::from("src/a.rs")),
                target_line: None,
                score: 100.0,
                match_positions: vec![],
            },
            QuickOpenItem {
                label: "b.rs".into(),
                description: "src/".into(),
                path: Some(PathBuf::from("src/b.rs")),
                target_line: None,
                score: 90.0,
                match_positions: vec![],
            },
            QuickOpenItem {
                label: "c.rs".into(),
                description: "src/".into(),
                path: Some(PathBuf::from("src/c.rs")),
                target_line: None,
                score: 80.0,
                match_positions: vec![],
            },
        ]);

        assert_eq!(state.selected, 0);

        state.select_next();
        assert_eq!(state.selected, 1);

        state.select_next();
        assert_eq!(state.selected, 2);

        state.select_next();
        assert_eq!(state.selected, 0);

        state.select_prev();
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn select_returns_current_item() {
        let mut state = QuickOpenState::default();
        assert!(state.select().is_none());

        state.set_items(vec![QuickOpenItem {
            label: "test.rs".into(),
            description: String::new(),
            path: Some(PathBuf::from("test.rs")),
            target_line: None,
            score: 1.0,
            match_positions: vec![],
        }]);

        let item = state.select().unwrap();
        assert_eq!(item.label, "test.rs");
    }

    #[test]
    fn items_accessor() {
        let state = QuickOpenState::default();
        assert!(state.items().is_empty());
    }

    #[test]
    fn set_items_clamps_selected() {
        let mut state = QuickOpenState::default();
        state.selected = 5;
        state.set_items(vec![QuickOpenItem {
            label: "only.rs".into(),
            description: String::new(),
            path: None,
            target_line: None,
            score: 1.0,
            match_positions: vec![],
        }]);
        assert_eq!(state.selected, 0);
    }
}
