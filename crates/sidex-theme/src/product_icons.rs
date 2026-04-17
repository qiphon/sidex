//! Product icon theme — codicon-like icons for UI elements.

use std::collections::HashMap;

/// A reference to a product icon (codicon).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductIcon {
    /// The codicon identifier, e.g. `"chevron-right"`.
    pub id: String,
}

impl ProductIcon {
    pub const fn new_static(id: &'static str) -> StaticProductIcon {
        StaticProductIcon { id }
    }
}

/// A `'static` variant used for built-in constants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticProductIcon {
    pub id: &'static str,
}

impl StaticProductIcon {
    pub fn to_owned_icon(&self) -> ProductIcon {
        ProductIcon {
            id: self.id.to_owned(),
        }
    }
}

// ── Well-known product icons ─────────────────────────────────────────────

pub mod icons {
    use super::StaticProductIcon;

    macro_rules! icon {
        ($name:ident, $id:literal) => {
            pub const $name: StaticProductIcon = StaticProductIcon { id: $id };
        };
    }

    // Activity bar
    icon!(EXPLORER, "files");
    icon!(SEARCH, "search");
    icon!(SOURCE_CONTROL, "source-control");
    icon!(DEBUG, "debug-alt");
    icon!(EXTENSIONS, "extensions");
    icon!(ACCOUNTS, "account");
    icon!(SETTINGS_GEAR, "settings-gear");
    icon!(REMOTE_EXPLORER, "remote-explorer");

    // Status bar
    icon!(ERROR, "error");
    icon!(WARNING, "warning");
    icon!(INFO, "info");
    icon!(GIT_BRANCH, "git-branch");
    icon!(GIT_COMMIT, "git-commit");
    icon!(FEEDBACK, "feedback");
    icon!(BELL, "bell");
    icon!(BELL_DOT, "bell-dot");
    icon!(SYNC, "sync");
    icon!(SYNC_SPIN, "sync~spin");

    // Tree view / explorer
    icon!(CHEVRON_RIGHT, "chevron-right");
    icon!(CHEVRON_DOWN, "chevron-down");
    icon!(FILE, "file");
    icon!(FOLDER, "folder");
    icon!(FOLDER_OPENED, "folder-opened");
    icon!(SYMBOL_FILE, "symbol-file");
    icon!(SYMBOL_FOLDER, "symbol-folder");
    icon!(CLOSE, "close");
    icon!(ADD, "add");
    icon!(TRASH, "trash");
    icon!(EDIT, "edit");
    icon!(REFRESH, "refresh");
    icon!(COLLAPSE_ALL, "collapse-all");
    icon!(EXPAND_ALL, "expand-all");
    icon!(NEW_FILE, "new-file");
    icon!(NEW_FOLDER, "new-folder");

    // Editor
    icon!(SPLIT_HORIZONTAL, "split-horizontal");
    icon!(SPLIT_VERTICAL, "split-vertical");
    icon!(ELLIPSIS, "ellipsis");
    icon!(GO_TO_FILE, "go-to-file");
    icon!(LIGHTBULB, "lightbulb");
    icon!(LIGHTBULB_AUTOFIX, "lightbulb-autofix");
    icon!(SAVE, "save");
    icon!(SAVE_ALL, "save-all");

    // Debug
    icon!(DEBUG_START, "debug-start");
    icon!(DEBUG_STOP, "debug-stop");
    icon!(DEBUG_PAUSE, "debug-pause");
    icon!(DEBUG_CONTINUE, "debug-continue");
    icon!(DEBUG_STEP_OVER, "debug-step-over");
    icon!(DEBUG_STEP_INTO, "debug-step-into");
    icon!(DEBUG_STEP_OUT, "debug-step-out");
    icon!(DEBUG_RESTART, "debug-restart");
    icon!(DEBUG_BREAKPOINT, "debug-breakpoint");

    // Terminal
    icon!(TERMINAL, "terminal");
    icon!(TERMINAL_KILL, "terminal-kill");
    icon!(TERMINAL_NEW, "terminal-new");

    // General
    icon!(CHECK, "check");
    icon!(CIRCLE_FILLED, "circle-filled");
    icon!(CIRCLE_OUTLINE, "circle-outline");
    icon!(ARROW_UP, "arrow-up");
    icon!(ARROW_DOWN, "arrow-down");
    icon!(ARROW_LEFT, "arrow-left");
    icon!(ARROW_RIGHT, "arrow-right");
    icon!(PIN, "pin");
    icon!(PINNED, "pinned");
    icon!(COPY, "copy");
    icon!(LINK, "link");
    icon!(LINK_EXTERNAL, "link-external");
    icon!(HISTORY, "history");
    icon!(FILTER, "filter");
    icon!(CLEAR_ALL, "clear-all");
}

/// A product icon theme that can override the default codicon mappings.
#[derive(Clone, Debug)]
pub struct ProductIconTheme {
    pub name: String,
    overrides: HashMap<String, ProductIcon>,
}

impl Default for ProductIconTheme {
    fn default() -> Self {
        Self {
            name: "Default".to_owned(),
            overrides: HashMap::new(),
        }
    }
}

impl ProductIconTheme {
    /// Resolve an icon by its identifier. Falls back to the default (the id
    /// itself) if no override is present.
    pub fn resolve(&self, id: &str) -> ProductIcon {
        self.overrides
            .get(id)
            .cloned()
            .unwrap_or_else(|| ProductIcon { id: id.to_owned() })
    }

    /// Set an override for an icon id.
    pub fn set_override(&mut self, id: impl Into<String>, icon: ProductIcon) {
        self.overrides.insert(id.into(), icon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_resolve_returns_id() {
        let theme = ProductIconTheme::default();
        let icon = theme.resolve("files");
        assert_eq!(icon.id, "files");
    }

    #[test]
    fn override_takes_precedence() {
        let mut theme = ProductIconTheme::default();
        theme.set_override(
            "files",
            ProductIcon {
                id: "custom-files".to_owned(),
            },
        );
        let icon = theme.resolve("files");
        assert_eq!(icon.id, "custom-files");
    }

    #[test]
    fn static_icon_constants() {
        assert_eq!(icons::EXPLORER.id, "files");
        assert_eq!(icons::SEARCH.id, "search");
        assert_eq!(icons::TERMINAL.id, "terminal");
    }
}
