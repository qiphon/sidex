//! Zen mode — distraction-free editing by hiding all chrome.
//!
//! When activated, hides the activity bar, sidebar, panel, status bar, and tabs,
//! optionally centering the editor content.

use serde::{Deserialize, Serialize};

/// Zen mode configuration matching `zenMode.*` VS Code settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZenModeSettings {
    pub center_layout: bool,
    pub full_screen: bool,
    pub hide_activity_bar: bool,
    pub hide_status_bar: bool,
    pub hide_line_numbers: bool,
    pub hide_tabs: bool,
    pub restore_on_exit: bool,
    pub silent_notifications: bool,
}

impl Default for ZenModeSettings {
    fn default() -> Self {
        Self {
            center_layout: true,
            full_screen: true,
            hide_activity_bar: true,
            hide_status_bar: true,
            hide_line_numbers: false,
            hide_tabs: true,
            restore_on_exit: true,
            silent_notifications: true,
        }
    }
}

/// Snapshot of the workbench layout before entering zen mode, so we can restore it.
#[derive(Debug, Clone, Default)]
struct SavedLayout {
    sidebar_visible: bool,
    panel_visible: bool,
    activity_bar_visible: bool,
    status_bar_visible: bool,
    tabs_visible: bool,
    line_numbers_visible: bool,
    was_full_screen: bool,
}

/// Controls zen mode state and transitions.
pub struct ZenMode {
    pub settings: ZenModeSettings,
    active: bool,
    saved: Option<SavedLayout>,
    transition_progress: f32,
}

impl ZenMode {
    /// Create zen mode with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            settings: ZenModeSettings::default(),
            active: false,
            saved: None,
            transition_progress: 0.0,
        }
    }

    /// Create zen mode with custom settings.
    #[must_use]
    pub fn with_settings(settings: ZenModeSettings) -> Self {
        Self {
            settings,
            active: false,
            saved: None,
            transition_progress: 0.0,
        }
    }

    /// Whether zen mode is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Whether a transition animation is in progress.
    #[must_use]
    pub fn is_transitioning(&self) -> bool {
        self.transition_progress > 0.0 && self.transition_progress < 1.0
    }

    /// Current transition progress (0.0 = normal, 1.0 = fully zen).
    #[must_use]
    pub fn transition_progress(&self) -> f32 {
        self.transition_progress
    }

    /// Enter zen mode, saving the current layout state.
    pub fn enter(&mut self, layout: &WorkbenchLayoutState) {
        if self.active {
            return;
        }

        self.saved = Some(SavedLayout {
            sidebar_visible: layout.sidebar_visible,
            panel_visible: layout.panel_visible,
            activity_bar_visible: layout.activity_bar_visible,
            status_bar_visible: layout.status_bar_visible,
            tabs_visible: layout.tabs_visible,
            line_numbers_visible: layout.line_numbers_visible,
            was_full_screen: layout.is_full_screen,
        });

        self.active = true;
        self.transition_progress = 0.0;
    }

    /// Exit zen mode and return the saved layout state to restore.
    pub fn exit(&mut self) -> Option<ZenRestoreState> {
        if !self.active {
            return None;
        }

        self.active = false;
        self.transition_progress = 1.0;

        self.saved.take().map(|saved| ZenRestoreState {
            sidebar_visible: saved.sidebar_visible,
            panel_visible: saved.panel_visible,
            activity_bar_visible: saved.activity_bar_visible,
            status_bar_visible: saved.status_bar_visible,
            tabs_visible: saved.tabs_visible,
            line_numbers_visible: saved.line_numbers_visible,
            restore_full_screen: !saved.was_full_screen,
        })
    }

    /// Toggle zen mode on/off.
    pub fn toggle(&mut self, layout: &WorkbenchLayoutState) -> ZenToggleResult {
        if self.active {
            let restore = self.exit();
            ZenToggleResult::Exited(restore)
        } else {
            self.enter(layout);
            ZenToggleResult::Entered(self.zen_layout_overrides())
        }
    }

    /// Get the layout overrides that should be applied in zen mode.
    #[must_use]
    pub fn zen_layout_overrides(&self) -> ZenLayoutOverrides {
        ZenLayoutOverrides {
            hide_sidebar: true,
            hide_panel: true,
            hide_activity_bar: self.settings.hide_activity_bar,
            hide_status_bar: self.settings.hide_status_bar,
            hide_tabs: self.settings.hide_tabs,
            hide_line_numbers: self.settings.hide_line_numbers,
            center_layout: self.settings.center_layout,
            full_screen: self.settings.full_screen,
            silent_notifications: self.settings.silent_notifications,
        }
    }

    /// Advance the transition animation by `dt` seconds.
    /// Returns `true` while the animation is still running.
    pub fn update_transition(&mut self, dt: f32) -> bool {
        const TRANSITION_SPEED: f32 = 4.0;

        if self.active && self.transition_progress < 1.0 {
            self.transition_progress = (self.transition_progress + dt * TRANSITION_SPEED).min(1.0);
            true
        } else if !self.active && self.transition_progress > 0.0 {
            self.transition_progress = (self.transition_progress - dt * TRANSITION_SPEED).max(0.0);
            true
        } else {
            false
        }
    }
}

impl Default for ZenMode {
    fn default() -> Self {
        Self::new()
    }
}

/// Current workbench layout state (input to zen mode).
#[derive(Debug, Clone, Default)]
pub struct WorkbenchLayoutState {
    pub sidebar_visible: bool,
    pub panel_visible: bool,
    pub activity_bar_visible: bool,
    pub status_bar_visible: bool,
    pub tabs_visible: bool,
    pub line_numbers_visible: bool,
    pub is_full_screen: bool,
}

/// Overrides to apply when zen mode is active.
#[derive(Debug, Clone)]
pub struct ZenLayoutOverrides {
    pub hide_sidebar: bool,
    pub hide_panel: bool,
    pub hide_activity_bar: bool,
    pub hide_status_bar: bool,
    pub hide_tabs: bool,
    pub hide_line_numbers: bool,
    pub center_layout: bool,
    pub full_screen: bool,
    pub silent_notifications: bool,
}

/// Layout state to restore when exiting zen mode.
#[derive(Debug, Clone)]
pub struct ZenRestoreState {
    pub sidebar_visible: bool,
    pub panel_visible: bool,
    pub activity_bar_visible: bool,
    pub status_bar_visible: bool,
    pub tabs_visible: bool,
    pub line_numbers_visible: bool,
    pub restore_full_screen: bool,
}

/// Result of toggling zen mode.
#[derive(Debug)]
pub enum ZenToggleResult {
    Entered(ZenLayoutOverrides),
    Exited(Option<ZenRestoreState>),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_layout() -> WorkbenchLayoutState {
        WorkbenchLayoutState {
            sidebar_visible: true,
            panel_visible: true,
            activity_bar_visible: true,
            status_bar_visible: true,
            tabs_visible: true,
            line_numbers_visible: true,
            is_full_screen: false,
        }
    }

    #[test]
    fn default_not_active() {
        let zen = ZenMode::new();
        assert!(!zen.is_active());
    }

    #[test]
    fn enter_and_exit() {
        let mut zen = ZenMode::new();
        let layout = sample_layout();

        zen.enter(&layout);
        assert!(zen.is_active());

        let restore = zen.exit().unwrap();
        assert!(!zen.is_active());
        assert!(restore.sidebar_visible);
        assert!(restore.panel_visible);
    }

    #[test]
    fn double_enter_is_noop() {
        let mut zen = ZenMode::new();
        let layout = sample_layout();

        zen.enter(&layout);
        zen.enter(&layout);
        assert!(zen.is_active());
    }

    #[test]
    fn exit_without_enter_returns_none() {
        let mut zen = ZenMode::new();
        assert!(zen.exit().is_none());
    }

    #[test]
    fn toggle() {
        let mut zen = ZenMode::new();
        let layout = sample_layout();

        match zen.toggle(&layout) {
            ZenToggleResult::Entered(overrides) => {
                assert!(overrides.hide_sidebar);
                assert!(overrides.center_layout);
            }
            ZenToggleResult::Exited(_) => panic!("expected Entered"),
        }

        assert!(zen.is_active());

        match zen.toggle(&layout) {
            ZenToggleResult::Exited(Some(restore)) => {
                assert!(restore.sidebar_visible);
            }
            _ => panic!("expected Exited with restore"),
        }

        assert!(!zen.is_active());
    }

    #[test]
    fn transition_animation() {
        let mut zen = ZenMode::new();
        let layout = sample_layout();

        zen.enter(&layout);
        assert!(zen.is_transitioning() || zen.transition_progress() == 0.0);

        let animating = zen.update_transition(0.1);
        assert!(animating);
        assert!(zen.transition_progress() > 0.0);

        // Run until complete
        for _ in 0..100 {
            zen.update_transition(0.1);
        }
        assert!((zen.transition_progress() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn custom_settings() {
        let settings = ZenModeSettings {
            hide_activity_bar: false,
            hide_status_bar: false,
            hide_line_numbers: true,
            ..ZenModeSettings::default()
        };
        let zen = ZenMode::with_settings(settings);
        let overrides = zen.zen_layout_overrides();
        assert!(!overrides.hide_activity_bar);
        assert!(!overrides.hide_status_bar);
        assert!(overrides.hide_line_numbers);
    }

    #[test]
    fn default_settings() {
        let s = ZenModeSettings::default();
        assert!(s.center_layout);
        assert!(s.full_screen);
        assert!(s.hide_activity_bar);
        assert!(s.hide_status_bar);
        assert!(!s.hide_line_numbers);
        assert!(s.hide_tabs);
    }

    #[test]
    fn restore_full_screen_only_if_wasnt_already() {
        let mut zen = ZenMode::new();
        let mut layout = sample_layout();
        layout.is_full_screen = true;

        zen.enter(&layout);
        let restore = zen.exit().unwrap();
        assert!(!restore.restore_full_screen);
    }
}
