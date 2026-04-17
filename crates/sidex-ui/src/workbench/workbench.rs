//! Top-level workbench layout composing all VS Code chrome components.
//!
//! The [`WorkbenchCompositor`] is the master compositor that computes the full
//! VS Code window layout and renders the entire application window in the
//! correct back-to-front order.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;
use sidex_theme::Theme;

use crate::draw::{CursorIcon, DrawContext};
use crate::layout::Rect;
use crate::widget::{EventResult, MouseButton, UiEvent};
use crate::workbench::zen_mode::{WorkbenchLayoutState, ZenLayoutOverrides, ZenMode};

/// Position of the sidebar relative to the editor area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarPosition {
    #[default]
    Left,
    Right,
}

/// Position of the bottom panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanelPosition {
    #[default]
    Bottom,
    Right,
}

/// Which sash (resize divider) is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SashType {
    SidebarEdge,
    PanelEdge,
}

/// Pre-computed rectangles for each region of the workbench.
#[derive(Clone, Debug, Default)]
pub struct WorkbenchLayout {
    pub title_bar: Rect,
    pub activity_bar: Rect,
    pub sidebar: Rect,
    pub editor_area: Rect,
    pub panel: Rect,
    pub status_bar: Rect,
}

/// Sash hit-test zones computed alongside the layout.
#[derive(Clone, Debug, Default)]
struct SashZones {
    sidebar_sash: Option<Rect>,
    panel_sash: Option<Rect>,
}

const SASH_SIZE: f32 = 4.0;
const SIDEBAR_MIN_WIDTH: f32 = 170.0;
const PANEL_MIN_HEIGHT: f32 = 100.0;
const SIDEBAR_MAX_FRACTION: f32 = 0.8;
const PANEL_MAX_FRACTION: f32 = 0.8;

/// The master workbench compositor that composes the full VS Code window layout.
pub struct WorkbenchCompositor {
    pub sidebar_visible: bool,
    pub sidebar_position: SidebarPosition,
    pub sidebar_width: f32,

    pub panel_visible: bool,
    pub panel_position: PanelPosition,
    pub panel_height: f32,

    pub activity_bar_visible: bool,
    pub status_bar_visible: bool,
    pub is_zen_mode: bool,
    pub is_fullscreen: bool,

    pub title_bar_height: f32,
    pub activity_bar_width: f32,
    pub status_bar_height: f32,

    zen_mode: ZenMode,
    cached_layout: Option<WorkbenchLayout>,
    sash_zones: SashZones,
    active_sash: Option<SashType>,
    sash_drag_start: f32,
    window_width: f32,
    window_height: f32,
}

impl WorkbenchCompositor {
    /// Creates a compositor with default dimensions derived from the theme.
    pub fn new(_theme: &Theme) -> Self {
        Self {
            sidebar_visible: true,
            sidebar_position: SidebarPosition::Left,
            sidebar_width: 250.0,

            panel_visible: true,
            panel_position: PanelPosition::Bottom,
            panel_height: 250.0,

            activity_bar_visible: true,
            status_bar_visible: true,
            is_zen_mode: false,
            is_fullscreen: false,

            title_bar_height: if cfg!(target_os = "macos") { 28.0 } else { 30.0 },
            activity_bar_width: 48.0,
            status_bar_height: 22.0,

            zen_mode: ZenMode::new(),
            cached_layout: None,
            sash_zones: SashZones::default(),
            active_sash: None,
            sash_drag_start: 0.0,
            window_width: 0.0,
            window_height: 0.0,
        }
    }

    // ── Visibility toggles ───────────────────────────────────────────

    pub fn toggle_sidebar(&mut self) {
        self.sidebar_visible = !self.sidebar_visible;
    }

    pub fn toggle_panel(&mut self) {
        self.panel_visible = !self.panel_visible;
    }

    pub fn toggle_activity_bar(&mut self) {
        self.activity_bar_visible = !self.activity_bar_visible;
    }

    pub fn toggle_status_bar(&mut self) {
        self.status_bar_visible = !self.status_bar_visible;
    }

    pub fn set_sidebar_position(&mut self, pos: SidebarPosition) {
        self.sidebar_position = pos;
    }

    pub fn set_panel_position(&mut self, pos: PanelPosition) {
        self.panel_position = pos;
    }

    // ── Zen mode ─────────────────────────────────────────────────────

    pub fn toggle_zen_mode(&mut self) {
        let state = self.layout_state_snapshot();
        let result = self.zen_mode.toggle(&state);
        match result {
            crate::workbench::zen_mode::ZenToggleResult::Entered(overrides) => {
                self.apply_zen_overrides(&overrides);
                self.is_zen_mode = true;
            }
            crate::workbench::zen_mode::ZenToggleResult::Exited(Some(restore)) => {
                self.sidebar_visible = restore.sidebar_visible;
                self.panel_visible = restore.panel_visible;
                self.activity_bar_visible = restore.activity_bar_visible;
                self.status_bar_visible = restore.status_bar_visible;
                self.is_zen_mode = false;
            }
            crate::workbench::zen_mode::ZenToggleResult::Exited(None) => {
                self.is_zen_mode = false;
            }
        }
    }

    fn layout_state_snapshot(&self) -> WorkbenchLayoutState {
        WorkbenchLayoutState {
            sidebar_visible: self.sidebar_visible,
            panel_visible: self.panel_visible,
            activity_bar_visible: self.activity_bar_visible,
            status_bar_visible: self.status_bar_visible,
            tabs_visible: true,
            line_numbers_visible: true,
            is_full_screen: self.is_fullscreen,
        }
    }

    fn apply_zen_overrides(&mut self, overrides: &ZenLayoutOverrides) {
        if overrides.hide_sidebar {
            self.sidebar_visible = false;
        }
        if overrides.hide_panel {
            self.panel_visible = false;
        }
        if overrides.hide_activity_bar {
            self.activity_bar_visible = false;
        }
        if overrides.hide_status_bar {
            self.status_bar_visible = false;
        }
    }

    // ── Sash dragging (resize) ───────────────────────────────────────

    /// Resize sidebar or panel by dragging the sash divider.
    pub fn handle_sash_drag(&mut self, sash: SashType, delta: f32) {
        match sash {
            SashType::SidebarEdge => {
                let dir = match self.sidebar_position {
                    SidebarPosition::Left => 1.0,
                    SidebarPosition::Right => -1.0,
                };
                let new_w = self.sidebar_width + delta * dir;
                let max_w = self.window_width * SIDEBAR_MAX_FRACTION;
                self.sidebar_width = new_w.clamp(SIDEBAR_MIN_WIDTH, max_w);
            }
            SashType::PanelEdge => {
                let new_h = self.panel_height - delta;
                let editor_h = self.window_height
                    - self.title_bar_height
                    - if self.status_bar_visible { self.status_bar_height } else { 0.0 };
                let max_h = editor_h * PANEL_MAX_FRACTION;
                self.panel_height = new_h.clamp(PANEL_MIN_HEIGHT, max_h);
            }
        }
    }

    // ── Layout computation ───────────────────────────────────────────

    /// Computes the workbench layout for the given window dimensions.
    pub fn compute_layout(&mut self, window_width: f32, window_height: f32) -> WorkbenchLayout {
        self.window_width = window_width;
        self.window_height = window_height;

        let title_bar_h = self.title_bar_height;
        let status_bar_h = if self.status_bar_visible { self.status_bar_height } else { 0.0 };
        let activity_bar_w = if self.activity_bar_visible { self.activity_bar_width } else { 0.0 };
        let sidebar_w = if self.sidebar_visible { self.sidebar_width } else { 0.0 };
        let panel_h = if self.panel_visible { self.panel_height } else { 0.0 };

        let content_top = title_bar_h;
        let content_bottom = window_height - status_bar_h;
        let content_height = (content_bottom - content_top).max(0.0);

        let mut layout = WorkbenchLayout::default();

        layout.title_bar = Rect::new(0.0, 0.0, window_width, title_bar_h);
        layout.status_bar = Rect::new(0.0, content_bottom, window_width, status_bar_h);

        match self.sidebar_position {
            SidebarPosition::Left => {
                layout.activity_bar = Rect::new(
                    0.0, content_top, activity_bar_w, content_height,
                );
                layout.sidebar = Rect::new(
                    activity_bar_w, content_top, sidebar_w, content_height - panel_h,
                );
                let editor_left = activity_bar_w + sidebar_w;
                let editor_width = (window_width - editor_left).max(0.0);
                layout.editor_area = Rect::new(
                    editor_left, content_top, editor_width, content_height - panel_h,
                );
                layout.panel = Rect::new(
                    activity_bar_w + sidebar_w,
                    content_bottom - panel_h,
                    editor_width,
                    panel_h,
                );
            }
            SidebarPosition::Right => {
                layout.activity_bar = Rect::new(
                    window_width - activity_bar_w, content_top, activity_bar_w, content_height,
                );
                layout.sidebar = Rect::new(
                    window_width - activity_bar_w - sidebar_w,
                    content_top,
                    sidebar_w,
                    content_height - panel_h,
                );
                let editor_width = (window_width - activity_bar_w - sidebar_w).max(0.0);
                layout.editor_area = Rect::new(
                    0.0, content_top, editor_width, content_height - panel_h,
                );
                layout.panel = Rect::new(
                    0.0, content_bottom - panel_h, editor_width, panel_h,
                );
            }
        }

        if self.is_zen_mode {
            layout.activity_bar = Rect::ZERO;
            layout.sidebar = Rect::ZERO;
            layout.panel = Rect::ZERO;
            layout.status_bar = Rect::ZERO;
            layout.editor_area = Rect::new(0.0, content_top, window_width, content_height);
        }

        self.sash_zones = self.compute_sash_zones(&layout);
        self.cached_layout = Some(layout.clone());
        layout
    }

    fn compute_sash_zones(&self, layout: &WorkbenchLayout) -> SashZones {
        let sidebar_sash = if self.sidebar_visible && !self.is_zen_mode {
            let sash_x = match self.sidebar_position {
                SidebarPosition::Left => layout.sidebar.right() - SASH_SIZE / 2.0,
                SidebarPosition::Right => layout.sidebar.x - SASH_SIZE / 2.0,
            };
            Some(Rect::new(sash_x, layout.sidebar.y, SASH_SIZE, layout.sidebar.height))
        } else {
            None
        };

        let panel_sash = if self.panel_visible && !self.is_zen_mode {
            Some(Rect::new(
                layout.panel.x,
                layout.panel.y - SASH_SIZE / 2.0,
                layout.panel.width,
                SASH_SIZE,
            ))
        } else {
            None
        };

        SashZones { sidebar_sash, panel_sash }
    }

    // ── Rendering ────────────────────────────────────────────────────

    /// Renders the workbench chrome (sash handles) into the draw context.
    /// Individual components are rendered by their owners via the layout rects.
    pub fn render_sash_handles(&self, ctx: &mut DrawContext, _layout: &WorkbenchLayout, theme: &Theme) {
        let sash_color = match theme.workbench_colors.sash_hover_border {
            Some(c) => Color {
                r: f32::from(c.r) / 255.0,
                g: f32::from(c.g) / 255.0,
                b: f32::from(c.b) / 255.0,
                a: f32::from(c.a) / 255.0,
            },
            None => Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
        };
        let sash_idle = Color::TRANSPARENT;

        if let Some(sash_rect) = &self.sash_zones.sidebar_sash {
            let color = if self.active_sash == Some(SashType::SidebarEdge) {
                sash_color
            } else {
                sash_idle
            };
            ctx.draw_rect(*sash_rect, color, 0.0);
        }

        if let Some(sash_rect) = &self.sash_zones.panel_sash {
            let color = if self.active_sash == Some(SashType::PanelEdge) {
                sash_color
            } else {
                sash_idle
            };
            ctx.draw_rect(*sash_rect, color, 0.0);
        }
    }

    /// Renders the workbench chrome into the GPU renderer (minimal).
    pub fn render(&self, _renderer: &mut GpuRenderer) {
        // Layout computation only — actual rendering is delegated to each
        // sub-component by the application event loop.
    }

    // ── Event handling ───────────────────────────────────────────────

    /// Routes an event to sash drag handling. Returns `Handled` if a sash
    /// interaction was consumed, otherwise `Ignored` so the caller can
    /// forward to sub-components.
    pub fn handle_event(&mut self, event: &UiEvent) -> EventResult {
        match event {
            UiEvent::MouseDown { x, y, button: MouseButton::Left } => {
                if let Some(sash) = self.hit_test_sash(*x, *y) {
                    self.active_sash = Some(sash);
                    self.sash_drag_start = match sash {
                        SashType::SidebarEdge => *x,
                        SashType::PanelEdge => *y,
                    };
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            UiEvent::MouseMove { x, y } => {
                if let Some(sash) = self.active_sash {
                    let delta = match sash {
                        SashType::SidebarEdge => *x - self.sash_drag_start,
                        SashType::PanelEdge => *y - self.sash_drag_start,
                    };
                    self.handle_sash_drag(sash, delta);
                    self.sash_drag_start = match sash {
                        SashType::SidebarEdge => *x,
                        SashType::PanelEdge => *y,
                    };
                    return EventResult::Handled;
                }

                // Update cursor for sash hover
                if self.hit_test_sash(*x, *y).is_some() {
                    return EventResult::Ignored;
                }
                EventResult::Ignored
            }
            UiEvent::MouseUp { button: MouseButton::Left, .. } => {
                if self.active_sash.take().is_some() {
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }

    /// Returns the cursor icon that should be shown, if a sash is hovered.
    pub fn cursor_for_position(&self, x: f32, y: f32) -> Option<CursorIcon> {
        if self.active_sash.is_some() {
            return match self.active_sash.unwrap() {
                SashType::SidebarEdge => Some(CursorIcon::ResizeEW),
                SashType::PanelEdge => Some(CursorIcon::ResizeNS),
            };
        }
        self.hit_test_sash(x, y).map(|sash| match sash {
            SashType::SidebarEdge => CursorIcon::ResizeEW,
            SashType::PanelEdge => CursorIcon::ResizeNS,
        })
    }

    fn hit_test_sash(&self, x: f32, y: f32) -> Option<SashType> {
        if let Some(ref rect) = self.sash_zones.sidebar_sash {
            if rect.contains(x, y) {
                return Some(SashType::SidebarEdge);
            }
        }
        if let Some(ref rect) = self.sash_zones.panel_sash {
            if rect.contains(x, y) {
                return Some(SashType::PanelEdge);
            }
        }
        None
    }

    /// Returns the most recently computed layout, if any.
    pub fn cached_layout(&self) -> Option<&WorkbenchLayout> {
        self.cached_layout.as_ref()
    }

    /// Determines which workbench region a point falls in.
    pub fn hit_test_region(&self, x: f32, y: f32) -> Option<WorkbenchRegion> {
        let layout = self.cached_layout.as_ref()?;
        if layout.title_bar.contains(x, y) {
            Some(WorkbenchRegion::TitleBar)
        } else if layout.activity_bar.contains(x, y) {
            Some(WorkbenchRegion::ActivityBar)
        } else if layout.sidebar.contains(x, y) {
            Some(WorkbenchRegion::Sidebar)
        } else if layout.panel.contains(x, y) {
            Some(WorkbenchRegion::Panel)
        } else if layout.status_bar.contains(x, y) {
            Some(WorkbenchRegion::StatusBar)
        } else if layout.editor_area.contains(x, y) {
            Some(WorkbenchRegion::EditorArea)
        } else {
            None
        }
    }
}

/// Named regions of the workbench for hit-testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkbenchRegion {
    TitleBar,
    ActivityBar,
    Sidebar,
    EditorArea,
    Panel,
    StatusBar,
}

// ── Backward-compatible aliases ──────────────────────────────────────────────

/// Alias for backward compatibility with code using the old `Workbench` name.
pub type Workbench = WorkbenchCompositor;

#[cfg(test)]
mod tests {
    use super::*;
    use sidex_theme::Theme;

    fn make() -> WorkbenchCompositor {
        WorkbenchCompositor::new(&Theme::default_dark())
    }

    #[test]
    fn default_layout_regions_are_non_zero() {
        let mut wb = make();
        let wl = wb.compute_layout(1280.0, 720.0);
        assert!(wl.title_bar.height > 0.0);
        assert!(wl.status_bar.height > 0.0);
        assert!(wl.activity_bar.width > 0.0);
        assert!(wl.sidebar.width > 0.0);
        assert!(wl.editor_area.width > 0.0);
    }

    #[test]
    fn sidebar_toggle_removes_sidebar() {
        let mut wb = make();
        wb.toggle_sidebar();
        let wl = wb.compute_layout(1280.0, 720.0);
        assert!((wl.sidebar.width - 0.0).abs() < 0.01);
    }

    #[test]
    fn panel_toggle_removes_panel() {
        let mut wb = make();
        wb.toggle_panel();
        let wl = wb.compute_layout(1280.0, 720.0);
        assert!((wl.panel.height - 0.0).abs() < 0.01);
    }

    #[test]
    fn title_bar_spans_full_width() {
        let mut wb = make();
        let wl = wb.compute_layout(1920.0, 1080.0);
        assert!((wl.title_bar.width - 1920.0).abs() < 0.01);
        assert!((wl.title_bar.x - 0.0).abs() < 0.01);
    }

    #[test]
    fn status_bar_at_bottom() {
        let mut wb = make();
        let wl = wb.compute_layout(1280.0, 720.0);
        let bottom = wl.status_bar.y + wl.status_bar.height;
        assert!((bottom - 720.0).abs() < 0.01);
    }

    #[test]
    fn sidebar_right_layout() {
        let mut wb = make();
        wb.set_sidebar_position(SidebarPosition::Right);
        let wl = wb.compute_layout(1280.0, 720.0);
        assert!(wl.sidebar.x > wl.editor_area.x);
        assert!(wl.activity_bar.x > wl.sidebar.x);
    }

    #[test]
    fn sash_drag_resizes_sidebar() {
        let mut wb = make();
        let original = wb.sidebar_width;
        wb.window_width = 1280.0;
        wb.window_height = 720.0;
        wb.handle_sash_drag(SashType::SidebarEdge, 50.0);
        assert!((wb.sidebar_width - (original + 50.0)).abs() < 0.01);
    }

    #[test]
    fn sash_drag_clamps_sidebar_min() {
        let mut wb = make();
        wb.window_width = 1280.0;
        wb.window_height = 720.0;
        wb.handle_sash_drag(SashType::SidebarEdge, -500.0);
        assert!(wb.sidebar_width >= SIDEBAR_MIN_WIDTH);
    }

    #[test]
    fn sash_drag_clamps_sidebar_max() {
        let mut wb = make();
        wb.window_width = 1280.0;
        wb.window_height = 720.0;
        wb.handle_sash_drag(SashType::SidebarEdge, 5000.0);
        assert!(wb.sidebar_width <= 1280.0 * SIDEBAR_MAX_FRACTION);
    }

    #[test]
    fn sash_drag_resizes_panel() {
        let mut wb = make();
        wb.window_width = 1280.0;
        wb.window_height = 720.0;
        let original = wb.panel_height;
        wb.handle_sash_drag(SashType::PanelEdge, -30.0);
        assert!((wb.panel_height - (original + 30.0)).abs() < 0.01);
    }

    #[test]
    fn sash_drag_clamps_panel_min() {
        let mut wb = make();
        wb.window_width = 1280.0;
        wb.window_height = 720.0;
        wb.handle_sash_drag(SashType::PanelEdge, 5000.0);
        assert!(wb.panel_height >= PANEL_MIN_HEIGHT);
    }

    #[test]
    fn zen_mode_hides_chrome() {
        let mut wb = make();
        wb.toggle_zen_mode();
        let wl = wb.compute_layout(1280.0, 720.0);
        assert_eq!(wl.activity_bar, Rect::ZERO);
        assert_eq!(wl.sidebar, Rect::ZERO);
        assert_eq!(wl.panel, Rect::ZERO);
        assert_eq!(wl.status_bar, Rect::ZERO);
        assert!(wl.editor_area.width > 0.0);
    }

    #[test]
    fn zen_mode_exit_restores() {
        let mut wb = make();
        assert!(wb.sidebar_visible);
        wb.toggle_zen_mode();
        assert!(wb.is_zen_mode);
        assert!(!wb.sidebar_visible);
        wb.toggle_zen_mode();
        assert!(!wb.is_zen_mode);
        assert!(wb.sidebar_visible);
    }

    #[test]
    fn hit_test_region() {
        let mut wb = make();
        wb.compute_layout(1280.0, 720.0);
        assert_eq!(wb.hit_test_region(640.0, 5.0), Some(WorkbenchRegion::TitleBar));
        assert_eq!(wb.hit_test_region(24.0, 400.0), Some(WorkbenchRegion::ActivityBar));
        assert_eq!(wb.hit_test_region(640.0, 715.0), Some(WorkbenchRegion::StatusBar));
    }

    #[test]
    fn sash_event_handling() {
        let mut wb = make();
        wb.compute_layout(1280.0, 720.0);

        let sidebar_right = wb.cached_layout().unwrap().sidebar.right();
        let sidebar_mid_y = wb.cached_layout().unwrap().sidebar.y
            + wb.cached_layout().unwrap().sidebar.height / 2.0;

        let down = UiEvent::MouseDown {
            x: sidebar_right,
            y: sidebar_mid_y,
            button: MouseButton::Left,
        };
        let result = wb.handle_event(&down);
        assert_eq!(result, EventResult::Handled);
        assert_eq!(wb.active_sash, Some(SashType::SidebarEdge));

        let up = UiEvent::MouseUp {
            x: sidebar_right + 10.0,
            y: sidebar_mid_y,
            button: MouseButton::Left,
        };
        let result = wb.handle_event(&up);
        assert_eq!(result, EventResult::Handled);
        assert_eq!(wb.active_sash, None);
    }

    #[test]
    fn activity_bar_toggle() {
        let mut wb = make();
        wb.toggle_activity_bar();
        let wl = wb.compute_layout(1280.0, 720.0);
        assert!((wl.activity_bar.width - 0.0).abs() < 0.01);
    }

    #[test]
    fn cursor_for_sash_hover() {
        let mut wb = make();
        wb.compute_layout(1280.0, 720.0);
        let sr = wb.cached_layout().unwrap().sidebar.right();
        let sy = wb.cached_layout().unwrap().sidebar.y + 100.0;
        assert_eq!(wb.cursor_for_position(sr, sy), Some(CursorIcon::ResizeEW));
        assert_eq!(wb.cursor_for_position(640.0, 400.0), None);
    }
}
