//! Custom title bar for Windows/Linux, native title bar on macOS.
//!
//! Mirrors VS Code's title bar behaviour:
//! - Custom-drawn title bar with menu bar, title, window controls
//! - Command center (search bar in title, à la VS Code 1.64+)
//! - Compact hamburger menu option
//! - macOS: delegate to native traffic-light controls

use std::path::Path;

// ── Title bar ────────────────────────────────────────────────────────────────

/// The application title bar state.
#[derive(Debug, Clone)]
pub struct TitleBar {
    pub title: String,
    pub menu_bar: Option<MenuBarState>,
    pub window_controls: WindowControls,
    pub command_center: bool,
}

impl Default for TitleBar {
    fn default() -> Self {
        Self {
            title: "SideX".into(),
            menu_bar: Some(MenuBarState::default()),
            window_controls: WindowControls::default(),
            command_center: true,
        }
    }
}

impl TitleBar {
    /// Update the title following the VS Code pattern:
    /// `filename — workspace — SideX`.
    pub fn update_title(&mut self, filename: Option<&str>, workspace: Option<&str>) {
        self.title = match (filename, workspace) {
            (Some(f), Some(w)) => format!("{f} — {w} — SideX"),
            (Some(f), None) => format!("{f} — SideX"),
            (None, Some(w)) => format!("{w} — SideX"),
            (None, None) => "SideX".to_owned(),
        };
    }

    /// Convenience to update from a full workspace path (extracts the folder name).
    pub fn update_title_from_path(&mut self, filename: Option<&str>, workspace: Option<&Path>) {
        let ws_name = workspace
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        self.update_title(filename, ws_name);
    }

    /// Show or hide the menu bar.
    pub fn set_menu_bar_visibility(&mut self, visible: bool) {
        if visible {
            if self.menu_bar.is_none() {
                self.menu_bar = Some(MenuBarState::default());
            }
        } else {
            self.menu_bar = None;
        }
    }

    /// Toggle the command center (search bar in the title area).
    pub fn toggle_command_center(&mut self) {
        self.command_center = !self.command_center;
    }

    /// Returns true when we should use a native (platform) title bar
    /// instead of a custom-drawn one.
    pub fn use_native_title_bar() -> bool {
        cfg!(target_os = "macos")
    }

    /// Notify that the window's maximized state changed.
    pub fn set_maximized(&mut self, maximized: bool) {
        self.window_controls.is_maximized = maximized;
    }
}

// ── Window controls ──────────────────────────────────────────────────────────

/// The minimize/maximize/close button cluster on the title bar.
#[derive(Debug, Clone)]
pub struct WindowControls {
    pub minimize: bool,
    pub maximize: bool,
    pub close: bool,
    pub is_maximized: bool,
}

impl Default for WindowControls {
    fn default() -> Self {
        Self {
            minimize: true,
            maximize: true,
            close: true,
            is_maximized: false,
        }
    }
}

// ── Menu bar ─────────────────────────────────────────────────────────────────

/// State of the application menu bar embedded in the title bar
/// (Windows/Linux only; macOS uses the native menu bar).
#[derive(Debug, Clone)]
pub struct MenuBarState {
    pub menus: Vec<MenuBarItem>,
    pub active: Option<usize>,
    pub is_compact: bool,
}

impl Default for MenuBarState {
    fn default() -> Self {
        Self {
            menus: default_menus(),
            active: None,
            is_compact: false,
        }
    }
}

impl MenuBarState {
    /// Activate (highlight) a menu by index.
    pub fn activate(&mut self, index: usize) {
        if index < self.menus.len() {
            self.active = Some(index);
        }
    }

    /// Deactivate (close) the menu bar.
    pub fn deactivate(&mut self) {
        self.active = None;
    }

    /// Navigate to the next menu (wrapping).
    pub fn next(&mut self) {
        if self.menus.is_empty() {
            return;
        }
        let current = self.active.unwrap_or(0);
        self.active = Some((current + 1) % self.menus.len());
    }

    /// Navigate to the previous menu (wrapping).
    pub fn prev(&mut self) {
        if self.menus.is_empty() {
            return;
        }
        let current = self.active.unwrap_or(0);
        self.active = Some(if current == 0 {
            self.menus.len() - 1
        } else {
            current - 1
        });
    }

    /// Switch between full menu and compact (hamburger) mode.
    pub fn toggle_compact(&mut self) {
        self.is_compact = !self.is_compact;
    }
}

/// A single top-level menu bar entry.
#[derive(Debug, Clone)]
pub struct MenuBarItem {
    pub label: String,
    pub menu_id: String,
}

/// The default set of menus matching VS Code's standard menu bar.
fn default_menus() -> Vec<MenuBarItem> {
    vec![
        MenuBarItem {
            label: "File".into(),
            menu_id: "file".into(),
        },
        MenuBarItem {
            label: "Edit".into(),
            menu_id: "edit".into(),
        },
        MenuBarItem {
            label: "Selection".into(),
            menu_id: "selection".into(),
        },
        MenuBarItem {
            label: "View".into(),
            menu_id: "view".into(),
        },
        MenuBarItem {
            label: "Go".into(),
            menu_id: "go".into(),
        },
        MenuBarItem {
            label: "Run".into(),
            menu_id: "run".into(),
        },
        MenuBarItem {
            label: "Terminal".into(),
            menu_id: "terminal".into(),
        },
        MenuBarItem {
            label: "Help".into(),
            menu_id: "help".into(),
        },
    ]
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_title() {
        let tb = TitleBar::default();
        assert_eq!(tb.title, "SideX");
        assert!(tb.command_center);
        assert!(tb.menu_bar.is_some());
    }

    #[test]
    fn update_title_full() {
        let mut tb = TitleBar::default();
        tb.update_title(Some("main.rs"), Some("sidex"));
        assert_eq!(tb.title, "main.rs — sidex — SideX");
    }

    #[test]
    fn update_title_no_file() {
        let mut tb = TitleBar::default();
        tb.update_title(None, Some("sidex"));
        assert_eq!(tb.title, "sidex — SideX");
    }

    #[test]
    fn update_title_no_workspace() {
        let mut tb = TitleBar::default();
        tb.update_title(Some("readme.md"), None);
        assert_eq!(tb.title, "readme.md — SideX");
    }

    #[test]
    fn update_title_bare() {
        let mut tb = TitleBar::default();
        tb.update_title(None, None);
        assert_eq!(tb.title, "SideX");
    }

    #[test]
    fn update_title_from_path() {
        let mut tb = TitleBar::default();
        tb.update_title_from_path(
            Some("main.rs"),
            Some(Path::new("/projects/my-project")),
        );
        assert_eq!(tb.title, "main.rs — my-project — SideX");
    }

    #[test]
    fn menu_bar_visibility() {
        let mut tb = TitleBar::default();
        assert!(tb.menu_bar.is_some());
        tb.set_menu_bar_visibility(false);
        assert!(tb.menu_bar.is_none());
        tb.set_menu_bar_visibility(true);
        assert!(tb.menu_bar.is_some());
    }

    #[test]
    fn menu_bar_navigation() {
        let mut state = MenuBarState::default();
        assert!(state.active.is_none());

        state.activate(0);
        assert_eq!(state.active, Some(0));

        state.next();
        assert_eq!(state.active, Some(1));

        state.prev();
        assert_eq!(state.active, Some(0));

        state.prev();
        assert_eq!(state.active, Some(state.menus.len() - 1));
    }

    #[test]
    fn menu_bar_compact_toggle() {
        let mut state = MenuBarState::default();
        assert!(!state.is_compact);
        state.toggle_compact();
        assert!(state.is_compact);
    }

    #[test]
    fn window_controls_default() {
        let wc = WindowControls::default();
        assert!(wc.minimize);
        assert!(wc.maximize);
        assert!(wc.close);
        assert!(!wc.is_maximized);
    }

    #[test]
    fn set_maximized() {
        let mut tb = TitleBar::default();
        tb.set_maximized(true);
        assert!(tb.window_controls.is_maximized);
        tb.set_maximized(false);
        assert!(!tb.window_controls.is_maximized);
    }

    #[test]
    fn command_center_toggle() {
        let mut tb = TitleBar::default();
        assert!(tb.command_center);
        tb.toggle_command_center();
        assert!(!tb.command_center);
    }

    #[test]
    fn default_menus_populated() {
        let menus = default_menus();
        assert!(menus.len() >= 7);
        assert_eq!(menus[0].label, "File");
        assert_eq!(menus[0].menu_id, "file");
    }

    #[test]
    fn menu_deactivate() {
        let mut state = MenuBarState::default();
        state.activate(2);
        assert_eq!(state.active, Some(2));
        state.deactivate();
        assert!(state.active.is_none());
    }
}
