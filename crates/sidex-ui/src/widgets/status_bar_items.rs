//! Built-in status bar items matching VS Code's default status bar layout.
//!
//! Provides a typed registry of all standard status bar entries and a
//! controller that keeps them up-to-date from editor/workspace state.

use crate::draw::IconId;
use crate::workbench::status_bar::{ShowWhen, StatusBarItem};

use sidex_gpu::color::Color;

// ── Background semantic colors ──────────────────────────────────────────────

/// Semantic background color for a status bar item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusBarBackground {
    Warning,
    Error,
}

impl StatusBarBackground {
    pub fn to_color(self) -> Color {
        match self {
            Self::Warning => Color::from_hex("#cca700").unwrap_or(Color::BLACK),
            Self::Error => Color::from_hex("#f14c4c").unwrap_or(Color::BLACK),
        }
    }
}

// ── Item collection ─────────────────────────────────────────────────────────

/// All built-in status bar items grouped by alignment.
pub struct StatusBarItems {
    pub left: Vec<StatusBarItem>,
    pub right: Vec<StatusBarItem>,
}

impl StatusBarItems {
    /// Creates the full default set of status bar items.
    pub fn defaults() -> Self {
        Self {
            left: default_left_items(),
            right: default_right_items(),
        }
    }

    /// Return all items as a flat vec (left first, then right), suitable for
    /// passing directly to `StatusBar::new`.
    pub fn into_flat(self) -> Vec<StatusBarItem> {
        let mut all = self.left;
        all.extend(self.right);
        all
    }

    /// Lookup an item by id across both sides.
    pub fn find(&self, id: &str) -> Option<&StatusBarItem> {
        self.left
            .iter()
            .chain(self.right.iter())
            .find(|i| i.id == id)
    }

    /// Lookup a mutable item by id.
    pub fn find_mut(&mut self, id: &str) -> Option<&mut StatusBarItem> {
        self.left
            .iter_mut()
            .chain(self.right.iter_mut())
            .find(|i| i.id == id)
    }
}

impl Default for StatusBarItems {
    fn default() -> Self {
        Self::defaults()
    }
}

// ── Left-side items ─────────────────────────────────────────────────────────

fn default_left_items() -> Vec<StatusBarItem> {
    vec![
        StatusBarItem::new("remote.indicator", "")
            .with_priority(10000)
            .with_tooltip("Remote Indicator")
            .with_icon(IconId::Remote)
            .with_show_when(ShowWhen::IsRemote)
            .with_command("workbench.action.remote.showMenu"),
        StatusBarItem::new("git.branch", "main")
            .with_priority(9000)
            .with_tooltip("Git Branch (checkout)")
            .with_icon(IconId::GitBranch)
            .with_command("workbench.action.quickOpen"),
        StatusBarItem::new("git.sync", "")
            .with_priority(8900)
            .with_tooltip("Synchronize Changes")
            .with_show_when(ShowWhen::NonZero)
            .with_command("git.sync"),
        StatusBarItem::new("problems.errors", "0")
            .with_priority(8000)
            .with_tooltip("No Errors")
            .with_icon(IconId::Error)
            .with_command("workbench.actions.view.problems"),
        StatusBarItem::new("problems.warnings", "0")
            .with_priority(7900)
            .with_tooltip("No Warnings")
            .with_icon(IconId::Warning)
            .with_command("workbench.actions.view.problems"),
    ]
}

// ── Right-side items ────────────────────────────────────────────────────────

fn default_right_items() -> Vec<StatusBarItem> {
    vec![
        StatusBarItem::new("notifications.bell", "")
            .right()
            .with_priority(10000)
            .with_tooltip("No New Notifications")
            .with_icon(IconId::Bell)
            .with_command("notifications.showList"),
        StatusBarItem::new("feedback", "")
            .right()
            .with_priority(9500)
            .with_tooltip("Tweet Feedback")
            .with_icon(IconId::MoreHorizontal)
            .with_command("workbench.action.openGlobalSettings"),
        StatusBarItem::new("editor.formatting", "")
            .right()
            .with_priority(9000)
            .with_tooltip("Formatting")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("editor.action.formatDocument"),
        StatusBarItem::new("cursor.position", "Ln 1, Col 1")
            .right()
            .with_priority(8000)
            .with_tooltip("Go to Line/Column")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("workbench.action.gotoLine"),
        StatusBarItem::new("selection.info", "")
            .right()
            .with_priority(7900)
            .with_tooltip("Selection")
            .with_show_when(ShowWhen::HasSelection),
        StatusBarItem::new("editor.indent", "Spaces: 4")
            .right()
            .with_priority(7000)
            .with_tooltip("Select Indentation")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("editor.action.indentationToSpaces"),
        StatusBarItem::new("editor.encoding", "UTF-8")
            .right()
            .with_priority(6000)
            .with_tooltip("Select Encoding")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("workbench.action.editor.changeEncoding"),
        StatusBarItem::new("editor.eol", "LF")
            .right()
            .with_priority(5000)
            .with_tooltip("Select End of Line Sequence")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("workbench.action.editor.changeEOL"),
        StatusBarItem::new("editor.language", "Plain Text")
            .right()
            .with_priority(4000)
            .with_tooltip("Select Language Mode")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("workbench.action.editor.changeLanguageMode"),
        StatusBarItem::new("copilot.status", "")
            .right()
            .with_priority(3000)
            .with_tooltip("Copilot Status")
            .with_icon(IconId::CircleFilled)
            .with_show_when(ShowWhen::Never),
    ]
}

// ── StatusBarController ─────────────────────────────────────────────────────

/// Convenience wrapper that holds a `StatusBarItems` set and provides methods
/// to update items from editor / workspace state changes.
pub struct StatusBarController {
    pub items: StatusBarItems,
}

impl StatusBarController {
    pub fn new() -> Self {
        Self {
            items: StatusBarItems::defaults(),
        }
    }

    pub fn set_cursor_position(&mut self, line: u32, col: u32) {
        if let Some(item) = self.items.find_mut("cursor.position") {
            item.text = format!("Ln {line}, Col {col}");
        }
    }

    pub fn set_selection_info(&mut self, lines: u32, chars: u32) {
        if let Some(item) = self.items.find_mut("selection.info") {
            if lines == 0 && chars == 0 {
                item.text.clear();
                item.visible = false;
            } else {
                item.text = format!("{lines} lines, {chars} chars selected");
                item.visible = true;
            }
        }
    }

    pub fn set_language_mode(&mut self, mode: &str) {
        if let Some(item) = self.items.find_mut("editor.language") {
            item.text = mode.to_string();
        }
    }

    pub fn set_encoding(&mut self, encoding: &str) {
        if let Some(item) = self.items.find_mut("editor.encoding") {
            item.text = encoding.to_string();
        }
    }

    pub fn set_eol(&mut self, eol: &str) {
        if let Some(item) = self.items.find_mut("editor.eol") {
            item.text = eol.to_string();
        }
    }

    pub fn set_indentation(&mut self, spaces: bool, size: u32) {
        if let Some(item) = self.items.find_mut("editor.indent") {
            item.text = if spaces {
                format!("Spaces: {size}")
            } else {
                format!("Tab Size: {size}")
            };
        }
    }

    pub fn set_git_branch(&mut self, branch: &str) {
        if let Some(item) = self.items.find_mut("git.branch") {
            item.text = branch.to_string();
        }
    }

    pub fn set_problems(&mut self, errors: u32, warnings: u32) {
        if let Some(item) = self.items.find_mut("problems.errors") {
            item.text = errors.to_string();
            item.tooltip = Some(format!(
                "{} Error{}",
                errors,
                if errors == 1 { "" } else { "s" }
            ));
        }
        if let Some(item) = self.items.find_mut("problems.warnings") {
            item.text = warnings.to_string();
            item.tooltip = Some(format!(
                "{} Warning{}",
                warnings,
                if warnings == 1 { "" } else { "s" }
            ));
        }
    }

    pub fn set_notification_count(&mut self, count: usize) {
        if let Some(item) = self.items.find_mut("notifications.bell") {
            item.tooltip = Some(if count == 0 {
                "No New Notifications".to_string()
            } else {
                format!("{count} New Notification{}", if count == 1 { "" } else { "s" })
            });
        }
    }

    /// Return all items as a flat vector for `StatusBar::new`.
    pub fn into_items(self) -> Vec<StatusBarItem> {
        self.items.into_flat()
    }
}

impl Default for StatusBarController {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_items() {
        let items = StatusBarItems::defaults();
        assert!(items.left.len() >= 4);
        assert!(items.right.len() >= 8);
    }

    #[test]
    fn find_by_id() {
        let items = StatusBarItems::defaults();
        assert!(items.find("cursor.position").is_some());
        assert!(items.find("nonexistent").is_none());
    }

    #[test]
    fn into_flat_combines() {
        let items = StatusBarItems::defaults();
        let left_count = items.left.len();
        let right_count = items.right.len();
        let flat = items.into_flat();
        assert_eq!(flat.len(), left_count + right_count);
    }

    #[test]
    fn controller_updates() {
        let mut ctrl = StatusBarController::new();
        ctrl.set_cursor_position(42, 10);
        assert_eq!(ctrl.items.find("cursor.position").unwrap().text, "Ln 42, Col 10");

        ctrl.set_language_mode("Rust");
        assert_eq!(ctrl.items.find("editor.language").unwrap().text, "Rust");

        ctrl.set_git_branch("feature/ui");
        assert_eq!(ctrl.items.find("git.branch").unwrap().text, "feature/ui");

        ctrl.set_problems(3, 7);
        assert_eq!(ctrl.items.find("problems.errors").unwrap().text, "3");
        assert_eq!(ctrl.items.find("problems.warnings").unwrap().text, "7");
    }

    #[test]
    fn background_semantic_colors() {
        let warn = StatusBarBackground::Warning.to_color();
        let err = StatusBarBackground::Error.to_color();
        assert_ne!(warn, err);
    }
}
