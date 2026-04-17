//! Command Palette — Ctrl+Shift+P.
//!
//! Mirrors VS Code's Command Palette: shows all registered commands with
//! their keybindings, supports fuzzy filtering, and prioritises recently
//! used commands.

/// A single item in the command palette list.
#[derive(Debug, Clone)]
pub struct CommandPaletteItem {
    /// VS Code-style command ID (e.g. `workbench.action.files.save`).
    pub id: String,
    /// Human-readable label (e.g. "Save").
    pub label: String,
    /// Display string for the keybinding, if one is assigned.
    pub keybinding: Option<String>,
    /// Category for grouping (e.g. "File", "Edit", "View").
    pub category: Option<String>,
    /// Fuzzy match score (higher = better match).
    pub score: f64,
    /// Character positions in `label` that matched the query.
    pub match_positions: Vec<usize>,
}

/// Command categories matching VS Code's palette grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    File,
    Edit,
    Selection,
    View,
    Go,
    Run,
    Terminal,
    Debug,
    Help,
    Preferences,
    Other,
}

impl CommandCategory {
    /// Returns the display string for this category.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::Selection => "Selection",
            Self::View => "View",
            Self::Go => "Go",
            Self::Run => "Run",
            Self::Terminal => "Terminal",
            Self::Debug => "Debug",
            Self::Help => "Help",
            Self::Preferences => "Preferences",
            Self::Other => "Other",
        }
    }

    /// Infer the category from a command ID prefix.
    pub fn from_command_id(id: &str) -> Self {
        if id.starts_with("workbench.action.files.")
            || id.starts_with("workbench.action.closeA")
            || id.starts_with("workbench.action.reopen")
        {
            Self::File
        } else if id.starts_with("editor.action.clipboard")
            || id.starts_with("editor.action.undo")
            || id.starts_with("editor.action.redo")
            || id.starts_with("editor.action.comment")
            || id.starts_with("editor.action.blockComment")
            || id.starts_with("editor.action.indent")
            || id.starts_with("editor.action.outdent")
            || id.starts_with("editor.action.moveLine")
            || id.starts_with("editor.action.copyLine")
            || id.starts_with("editor.action.deleteL")
            || id.starts_with("editor.action.joinL")
            || id.starts_with("editor.action.sortLine")
            || id.starts_with("editor.action.trim")
            || id.starts_with("editor.action.transform")
            || id.starts_with("editor.action.insertLine")
            || id.starts_with("editor.action.transpose")
        {
            Self::Edit
        } else if id.starts_with("editor.action.select")
            || id.starts_with("editor.action.smartSelect")
            || id.starts_with("editor.action.wordWrap")
        {
            Self::Selection
        } else if id.starts_with("workbench.action.toggle")
            || id.starts_with("workbench.action.zoom")
            || id.starts_with("workbench.action.split")
        {
            Self::View
        } else if id.starts_with("workbench.action.quickOpen")
            || id.starts_with("workbench.action.goto")
            || id.starts_with("workbench.action.navigate")
            || id.starts_with("workbench.action.showCommands")
            || id.starts_with("editor.action.goTo")
            || id.starts_with("editor.action.reveal")
        {
            Self::Go
        } else if id.starts_with("workbench.action.terminal") {
            Self::Terminal
        } else if id.starts_with("workbench.action.debug") || id.starts_with("editor.debug") {
            Self::Debug
        } else if id.starts_with("actions.find") || id.starts_with("editor.action.find") || id.starts_with("workbench.action.findInFiles") {
            Self::Edit
        } else {
            Self::Other
        }
    }
}

/// Full state for the command palette dialog.
#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    /// Whether the palette is currently shown.
    pub is_visible: bool,
    /// Raw input text (without the `>` prefix shown in the UI).
    pub input: String,
    /// Current filtered and sorted list of items.
    pub items: Vec<CommandPaletteItem>,
    /// Index of the selected item.
    pub selected: usize,
    /// Recently used command IDs (most recent first).
    pub recently_used: Vec<String>,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            is_visible: false,
            input: String::new(),
            items: Vec::new(),
            selected: 0,
            recently_used: Vec::new(),
        }
    }
}

impl CommandPaletteState {
    /// Open the command palette.
    pub fn show(&mut self) {
        self.is_visible = true;
        self.input.clear();
        self.selected = 0;
    }

    /// Close the command palette and reset filter state.
    pub fn cancel(&mut self) {
        self.is_visible = false;
        self.input.clear();
        self.items.clear();
        self.selected = 0;
    }

    /// Update the filter query.
    pub fn filter(&mut self, query: &str) {
        self.input = query.to_string();
        self.selected = 0;
    }

    /// Set the filtered/sorted list of items.
    pub fn set_items(&mut self, items: Vec<CommandPaletteItem>) {
        self.items = items;
        if self.selected >= self.items.len() {
            self.selected = 0;
        }
    }

    /// Returns the currently displayed items.
    pub fn items(&self) -> &[CommandPaletteItem] {
        &self.items
    }

    /// Move selection down by one, wrapping.
    pub fn select_next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Move selection up by one, wrapping.
    pub fn select_prev(&mut self) {
        if !self.items.is_empty() {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Returns the currently selected item, if any.
    pub fn selected_item(&self) -> Option<&CommandPaletteItem> {
        self.items.get(self.selected)
    }

    /// Record a command as recently used (for priority sorting).
    pub fn record_usage(&mut self, command_id: &str) {
        self.recently_used.retain(|id| id != command_id);
        self.recently_used.insert(0, command_id.to_string());
        if self.recently_used.len() > 50 {
            self.recently_used.truncate(50);
        }
    }

    /// Returns the recency boost for a command (higher = more recent).
    pub fn recency_boost(&self, command_id: &str) -> f64 {
        self.recently_used
            .iter()
            .position(|id| id == command_id)
            .map_or(0.0, |idx| 100.0 / (idx as f64 + 1.0))
    }

    /// Placeholder text for the palette input.
    pub fn placeholder(&self) -> &str {
        "Type a command name"
    }
}

/// Fuzzy-match a query against a label string.
///
/// Returns `(score, match_positions)` or `None` if no match.
#[allow(clippy::cast_precision_loss)]
pub fn fuzzy_match_command(query: &str, label: &str) -> Option<(f64, Vec<usize>)> {
    if query.is_empty() {
        return Some((0.0, vec![]));
    }

    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let label_lower: Vec<char> = label.to_lowercase().chars().collect();
    let label_chars: Vec<char> = label.chars().collect();

    let mut pi = 0;
    let mut positions = Vec::with_capacity(query_lower.len());

    for (ti, &tc) in label_lower.iter().enumerate() {
        if pi < query_lower.len() && tc == query_lower[pi] {
            positions.push(ti);
            pi += 1;
        }
    }

    if pi < query_lower.len() {
        return None;
    }

    let mut score = 0.0_f64;

    // Exact match bonus
    let label_lower_str: String = label_lower.iter().collect();
    let query_lower_str: String = query_lower.iter().collect();
    if label_lower_str == query_lower_str {
        score += 500.0;
    } else if label_lower_str.starts_with(&query_lower_str) {
        score += 250.0;
    } else if label_lower_str.contains(&query_lower_str) {
        score += 100.0;
    }

    let mut consecutive = 0.0_f64;
    for (i, &pos) in positions.iter().enumerate() {
        score += 10.0;

        if pos == 0
            || !label_chars
                .get(pos.wrapping_sub(1))
                .is_some_and(|c| c.is_alphanumeric())
        {
            score += 15.0;
        }

        if i > 0 && pos == positions[i - 1] + 1 {
            consecutive += 1.0;
            score += consecutive * 5.0;
        } else {
            consecutive = 0.0;
        }
    }

    Some((score, positions))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_from_file_command() {
        assert_eq!(
            CommandCategory::from_command_id("workbench.action.files.save"),
            CommandCategory::File
        );
    }

    #[test]
    fn category_from_edit_command() {
        assert_eq!(
            CommandCategory::from_command_id("editor.action.clipboardCopyAction"),
            CommandCategory::Edit
        );
    }

    #[test]
    fn category_from_go_command() {
        assert_eq!(
            CommandCategory::from_command_id("workbench.action.quickOpen"),
            CommandCategory::Go
        );
    }

    #[test]
    fn category_from_debug_command() {
        assert_eq!(
            CommandCategory::from_command_id("workbench.action.debug.start"),
            CommandCategory::Debug
        );
    }

    #[test]
    fn category_from_terminal_command() {
        assert_eq!(
            CommandCategory::from_command_id("workbench.action.terminal.new"),
            CommandCategory::Terminal
        );
    }

    #[test]
    fn category_from_unknown() {
        assert_eq!(
            CommandCategory::from_command_id("some.random.command"),
            CommandCategory::Other
        );
    }

    #[test]
    fn fuzzy_match_exact() {
        let (score, positions) = fuzzy_match_command("Save", "Save").unwrap();
        assert!(score > 400.0);
        assert_eq!(positions, vec![0, 1, 2, 3]);
    }

    #[test]
    fn fuzzy_match_prefix() {
        let (score, _) = fuzzy_match_command("Sav", "Save All").unwrap();
        assert!(score > 200.0);
    }

    #[test]
    fn fuzzy_match_subsequence() {
        let (_, positions) = fuzzy_match_command("tl", "Toggle Line Comment").unwrap();
        assert!(!positions.is_empty());
    }

    #[test]
    fn fuzzy_match_no_match() {
        assert!(fuzzy_match_command("xyz", "Save").is_none());
    }

    #[test]
    fn fuzzy_match_empty_query() {
        let (score, positions) = fuzzy_match_command("", "Save").unwrap();
        assert_eq!(score, 0.0);
        assert!(positions.is_empty());
    }

    #[test]
    fn fuzzy_match_case_insensitive() {
        let result = fuzzy_match_command("save", "Save All");
        assert!(result.is_some());
    }

    #[test]
    fn show_and_cancel() {
        let mut state = CommandPaletteState::default();
        state.show();
        assert!(state.is_visible);

        state.cancel();
        assert!(!state.is_visible);
        assert!(state.items.is_empty());
    }

    #[test]
    fn filter_resets_selection() {
        let mut state = CommandPaletteState::default();
        state.selected = 5;
        state.filter("save");
        assert_eq!(state.selected, 0);
        assert_eq!(state.input, "save");
    }

    #[test]
    fn select_navigation() {
        let mut state = CommandPaletteState::default();
        state.set_items(vec![
            CommandPaletteItem {
                id: "a".into(),
                label: "A".into(),
                keybinding: None,
                category: None,
                score: 100.0,
                match_positions: vec![],
            },
            CommandPaletteItem {
                id: "b".into(),
                label: "B".into(),
                keybinding: None,
                category: None,
                score: 90.0,
                match_positions: vec![],
            },
        ]);

        assert_eq!(state.selected, 0);
        state.select_next();
        assert_eq!(state.selected, 1);
        state.select_next();
        assert_eq!(state.selected, 0);
        state.select_prev();
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn record_usage() {
        let mut state = CommandPaletteState::default();
        state.record_usage("cmd_a");
        state.record_usage("cmd_b");
        state.record_usage("cmd_a");

        assert_eq!(state.recently_used[0], "cmd_a");
        assert_eq!(state.recently_used[1], "cmd_b");
        assert_eq!(state.recently_used.len(), 2);
    }

    #[test]
    fn recency_boost() {
        let mut state = CommandPaletteState::default();
        state.record_usage("cmd_a");
        state.record_usage("cmd_b");

        assert!(state.recency_boost("cmd_b") > state.recency_boost("cmd_a"));
        assert_eq!(state.recency_boost("cmd_unknown"), 0.0);
    }

    #[test]
    fn selected_item() {
        let mut state = CommandPaletteState::default();
        assert!(state.selected_item().is_none());

        state.set_items(vec![CommandPaletteItem {
            id: "test".into(),
            label: "Test".into(),
            keybinding: Some("Ctrl+T".into()),
            category: Some("Edit".into()),
            score: 1.0,
            match_positions: vec![],
        }]);

        let item = state.selected_item().unwrap();
        assert_eq!(item.id, "test");
        assert_eq!(item.keybinding.as_deref(), Some("Ctrl+T"));
    }
}
