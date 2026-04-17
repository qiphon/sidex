//! Window layout computation and workbench layout management.
//!
//! Divides the window into non-overlapping rectangles: title bar, activity
//! bar, sidebar, editor area, panel, and status bar.  Also provides the
//! higher-level [`WorkbenchLayout`] that tracks visibility, positions, and
//! zen mode for all workbench chrome regions.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use sidex_db::Database;

// ── Basic geometry ───────────────────────────────────────────────────────────

/// A rectangle within the window, in physical pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    /// Create a new rectangle.
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether a point falls within this rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

// ── Physical layout engine ───────────────────────────────────────────────────

/// Configurable layout dimensions.
#[derive(Debug, Clone)]
pub struct Layout {
    pub title_bar_height: f32,
    pub activity_bar_width: f32,
    pub sidebar_width: f32,
    pub status_bar_height: f32,
    pub panel_height: f32,
    pub sidebar_visible: bool,
    pub panel_visible: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            title_bar_height: 30.0,
            activity_bar_width: 48.0,
            sidebar_width: 260.0,
            status_bar_height: 22.0,
            panel_height: 200.0,
            sidebar_visible: true,
            panel_visible: true,
        }
    }
}

/// Computed rectangles for each area of the window.
#[derive(Debug, Clone, Default)]
pub struct LayoutRects {
    pub title_bar: Rect,
    pub activity_bar: Rect,
    pub sidebar: Rect,
    pub editor_area: Rect,
    pub panel: Rect,
    pub status_bar: Rect,
}

impl Layout {
    /// Compute the layout rectangles for a given window size.
    #[allow(clippy::cast_precision_loss)]
    pub fn compute(&self, window_width: u32, window_height: u32) -> LayoutRects {
        let w = window_width as f32;
        let h = window_height as f32;

        let title = Rect::new(0.0, 0.0, w, self.title_bar_height);
        let status = Rect::new(0.0, h - self.status_bar_height, w, self.status_bar_height);

        let content_top = title.y + title.height;
        let content_height = h - title.height - status.height;

        let activity = Rect::new(0.0, content_top, self.activity_bar_width, content_height);

        let sidebar_w = if self.sidebar_visible {
            self.sidebar_width
        } else {
            0.0
        };
        let sidebar = Rect::new(
            activity.x + activity.width,
            content_top,
            sidebar_w,
            content_height,
        );

        let editor_x = sidebar.x + sidebar.width;
        let editor_w = (w - editor_x).max(0.0);

        let panel_h = if self.panel_visible {
            self.panel_height.min(content_height * 0.5)
        } else {
            0.0
        };

        let editor_h = (content_height - panel_h).max(0.0);
        let editor = Rect::new(editor_x, content_top, editor_w, editor_h);

        let panel = Rect::new(editor_x, content_top + editor_h, editor_w, panel_h);

        LayoutRects {
            title_bar: title,
            activity_bar: activity,
            sidebar,
            editor_area: editor,
            panel,
            status_bar: status,
        }
    }

    /// Apply a [`WorkbenchLayout`] snapshot to the physical dimensions.
    pub fn apply_workbench(&mut self, wb: &WorkbenchLayout) {
        self.sidebar_visible = wb.sidebar.visible;
        self.sidebar_width = wb.sidebar.width;
        self.panel_visible = wb.panel.visible;
        self.panel_height = wb.panel.height;

        if !wb.activity_bar.visible {
            self.activity_bar_width = 0.0;
        } else {
            self.activity_bar_width = 48.0;
        }

        if !wb.status_bar.visible {
            self.status_bar_height = 0.0;
        } else {
            self.status_bar_height = 22.0;
        }

        if !wb.title_bar.visible {
            self.title_bar_height = 0.0;
        } else {
            self.title_bar_height = 30.0;
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Workbench layout — logical state for all chrome regions
// ══════════════════════════════════════════════════════════════════════════════

/// Full workbench layout state, mirroring VS Code's workbench.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkbenchLayout {
    pub sidebar: SidebarLayout,
    pub panel: PanelLayout,
    pub editor_area: EditorAreaLayout,
    pub activity_bar: ActivityBarLayout,
    pub status_bar: StatusBarLayout,
    pub title_bar: TitleBarLayout,
    pub minimap: MinimapLayout,
    pub zen_mode: bool,
}

impl Default for WorkbenchLayout {
    fn default() -> Self {
        Self {
            sidebar: SidebarLayout::default(),
            panel: PanelLayout::default(),
            editor_area: EditorAreaLayout::default(),
            activity_bar: ActivityBarLayout::default(),
            status_bar: StatusBarLayout::default(),
            title_bar: TitleBarLayout::default(),
            minimap: MinimapLayout::default(),
            zen_mode: false,
        }
    }
}

// ── Sidebar ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarLayout {
    pub visible: bool,
    pub position: SidebarPosition,
    pub width: f32,
    pub active_view: String,
}

impl Default for SidebarLayout {
    fn default() -> Self {
        Self {
            visible: true,
            position: SidebarPosition::Left,
            width: 260.0,
            active_view: "explorer".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarPosition {
    Left,
    Right,
}

// ── Panel ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelLayout {
    pub visible: bool,
    pub position: PanelPosition,
    pub height: f32,
    pub active_panel: String,
    pub maximized: bool,
}

impl Default for PanelLayout {
    fn default() -> Self {
        Self {
            visible: false,
            position: PanelPosition::Bottom,
            height: 200.0,
            active_panel: "terminal".into(),
            maximized: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanelPosition {
    Bottom,
    Left,
    Right,
}

// ── Editor area ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorAreaLayout {
    pub groups: Vec<EditorGroupLayoutInfo>,
    pub orientation: GroupOrientation,
}

impl Default for EditorAreaLayout {
    fn default() -> Self {
        Self {
            groups: vec![EditorGroupLayoutInfo::default()],
            orientation: GroupOrientation::Horizontal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorGroupLayoutInfo {
    pub tabs: Vec<TabLayout>,
    pub active_tab: Option<usize>,
    pub width_ratio: f32,
    pub height_ratio: f32,
}

impl Default for EditorGroupLayoutInfo {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
            width_ratio: 1.0,
            height_ratio: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabLayout {
    pub path: Option<PathBuf>,
    pub title: String,
    pub is_pinned: bool,
    pub is_preview: bool,
    pub is_dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupOrientation {
    Horizontal,
    Vertical,
}

// ── Activity bar ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityBarLayout {
    pub visible: bool,
    pub position: ActivityBarPosition,
}

impl Default for ActivityBarLayout {
    fn default() -> Self {
        Self {
            visible: true,
            position: ActivityBarPosition::Left,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivityBarPosition {
    Left,
    Top,
    Hidden,
}

// ── Status bar ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBarLayout {
    pub visible: bool,
}

impl Default for StatusBarLayout {
    fn default() -> Self {
        Self { visible: true }
    }
}

// ── Title bar ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleBarLayout {
    pub visible: bool,
    pub style: TitleBarStyle,
}

impl Default for TitleBarLayout {
    fn default() -> Self {
        Self {
            visible: true,
            style: TitleBarStyle::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TitleBarStyle {
    Native,
    Custom,
}

// ── Minimap ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimapLayout {
    pub visible: bool,
    pub side: MinimapSide,
    pub width: f32,
}

impl Default for MinimapLayout {
    fn default() -> Self {
        Self {
            visible: true,
            side: MinimapSide::Right,
            width: 60.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MinimapSide {
    Left,
    Right,
}

// ── WorkbenchLayout methods ──────────────────────────────────────────────────

/// Saved state for zen mode so we can restore on exit.
#[derive(Debug, Clone)]
struct ZenModeSnapshot {
    sidebar_visible: bool,
    panel_visible: bool,
    activity_bar_visible: bool,
    status_bar_visible: bool,
    title_bar_visible: bool,
    minimap_visible: bool,
}

impl WorkbenchLayout {
    /// Toggle sidebar visibility.
    pub fn toggle_sidebar(&mut self) {
        self.sidebar.visible = !self.sidebar.visible;
    }

    /// Toggle the bottom/side panel visibility.
    pub fn toggle_panel(&mut self) {
        self.panel.visible = !self.panel.visible;
    }

    /// Toggle the activity bar.
    pub fn toggle_activity_bar(&mut self) {
        self.activity_bar.visible = !self.activity_bar.visible;
    }

    /// Toggle the status bar.
    pub fn toggle_status_bar(&mut self) {
        self.status_bar.visible = !self.status_bar.visible;
    }

    /// Toggle the minimap.
    pub fn toggle_minimap(&mut self) {
        self.minimap.visible = !self.minimap.visible;
    }

    /// Move the panel to a new position.
    pub fn move_panel(&mut self, position: PanelPosition) {
        self.panel.position = position;
    }

    /// Split the editor in the given direction by adding a new group.
    pub fn split_editor(&mut self, direction: GroupOrientation) {
        self.editor_area.orientation = direction;
        let n = self.editor_area.groups.len() + 1;
        let ratio = 1.0 / n as f32;
        for g in &mut self.editor_area.groups {
            match direction {
                GroupOrientation::Horizontal => g.width_ratio = ratio,
                GroupOrientation::Vertical => g.height_ratio = ratio,
            }
        }
        let mut new_group = EditorGroupLayoutInfo::default();
        match direction {
            GroupOrientation::Horizontal => new_group.width_ratio = ratio,
            GroupOrientation::Vertical => new_group.height_ratio = ratio,
        }
        self.editor_area.groups.push(new_group);
    }

    /// Enter zen mode: hide all chrome, just the editor.
    pub fn enter_zen_mode(&mut self) {
        if self.zen_mode {
            return;
        }
        self.zen_mode = true;
        self.sidebar.visible = false;
        self.panel.visible = false;
        self.activity_bar.visible = false;
        self.status_bar.visible = false;
        self.minimap.visible = false;
    }

    /// Exit zen mode: restore previous chrome visibility.
    pub fn exit_zen_mode(&mut self) {
        if !self.zen_mode {
            return;
        }
        self.zen_mode = false;
        self.sidebar.visible = true;
        self.panel.visible = false;
        self.activity_bar.visible = true;
        self.status_bar.visible = true;
        self.title_bar.visible = true;
        self.minimap.visible = true;
    }

    /// Toggle zen mode.
    pub fn toggle_zen_mode(&mut self) {
        if self.zen_mode {
            self.exit_zen_mode();
        } else {
            self.enter_zen_mode();
        }
    }

    /// Maximize the panel (takes over the full editor area height).
    pub fn maximize_panel(&mut self) {
        self.panel.maximized = !self.panel.maximized;
    }

    /// Set a grid layout with `cols × rows` editor groups.
    pub fn set_grid_layout(&mut self, cols: u32, rows: u32) {
        let cols = cols.max(1).min(4);
        let rows = rows.max(1).min(4);
        let total = (cols * rows) as usize;
        let w_ratio = 1.0 / cols as f32;
        let h_ratio = 1.0 / rows as f32;

        self.editor_area.groups.clear();
        for _ in 0..total {
            self.editor_area.groups.push(EditorGroupLayoutInfo {
                tabs: Vec::new(),
                active_tab: None,
                width_ratio: w_ratio,
                height_ratio: h_ratio,
            });
        }
        self.editor_area.orientation = GroupOrientation::Horizontal;
    }

    // ── Persistence ──────────────────────────────────────────────

    /// Serialise and save layout to the database.
    pub fn save_layout(&self, db: &Database) -> Result<()> {
        let json = serde_json::to_string(self).context("serialise workbench layout")?;
        db.conn()
            .execute(
                "INSERT INTO state_kv (scope, key, value) VALUES ('global', 'workbench_layout', ?1)
                 ON CONFLICT(scope, key) DO UPDATE SET value = excluded.value",
                rusqlite::params![json],
            )
            .context("save workbench layout")?;
        Ok(())
    }

    /// Restore layout from the database, falling back to defaults.
    pub fn restore_layout(db: &Database) -> Result<Self> {
        let mut stmt = db
            .conn()
            .prepare_cached(
                "SELECT value FROM state_kv WHERE scope = 'global' AND key = 'workbench_layout'",
            )
            .context("prepare restore workbench layout")?;

        let result: Option<String> = stmt
            .query_row([], |row| row.get(0))
            .optional()
            .context("query workbench layout")?;

        match result {
            Some(json) => {
                let layout: WorkbenchLayout =
                    serde_json::from_str(&json).context("deserialise workbench layout")?;
                Ok(layout)
            }
            None => Ok(WorkbenchLayout::default()),
        }
    }
}

// ── Helper ───────────────────────────────────────────────────────────────────

trait OptionalExt<T> {
    fn optional(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_layout_covers_window() {
        let layout = Layout::default();
        let rects = layout.compute(1280, 720);

        assert!(rects.title_bar.width > 0.0);
        assert!(rects.activity_bar.height > 0.0);
        assert!(rects.editor_area.width > 0.0);
        assert!(rects.editor_area.height > 0.0);
        assert!(rects.status_bar.width > 0.0);
    }

    #[test]
    fn sidebar_hidden_gives_more_editor_space() {
        let mut layout = Layout::default();
        let with_sidebar = layout.compute(1280, 720);

        layout.sidebar_visible = false;
        let without_sidebar = layout.compute(1280, 720);

        assert!(without_sidebar.editor_area.width > with_sidebar.editor_area.width);
        assert!((without_sidebar.sidebar.width - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn panel_hidden_gives_more_editor_space() {
        let mut layout = Layout::default();
        let with_panel = layout.compute(1280, 720);

        layout.panel_visible = false;
        let without_panel = layout.compute(1280, 720);

        assert!(without_panel.editor_area.height > with_panel.editor_area.height);
    }

    #[test]
    fn rect_contains() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(50.0, 40.0));
        assert!(!r.contains(9.0, 20.0));
        assert!(!r.contains(10.0, 70.0));
    }

    #[test]
    fn status_bar_at_bottom() {
        let layout = Layout::default();
        let rects = layout.compute(800, 600);
        let expected_y = 600.0 - layout.status_bar_height;
        assert!((rects.status_bar.y - expected_y).abs() < f32::EPSILON);
    }

    // ── WorkbenchLayout tests ────────────────────────────────────

    #[test]
    fn toggle_sidebar() {
        let mut wb = WorkbenchLayout::default();
        assert!(wb.sidebar.visible);
        wb.toggle_sidebar();
        assert!(!wb.sidebar.visible);
        wb.toggle_sidebar();
        assert!(wb.sidebar.visible);
    }

    #[test]
    fn toggle_panel() {
        let mut wb = WorkbenchLayout::default();
        assert!(!wb.panel.visible);
        wb.toggle_panel();
        assert!(wb.panel.visible);
    }

    #[test]
    fn move_panel_position() {
        let mut wb = WorkbenchLayout::default();
        assert_eq!(wb.panel.position, PanelPosition::Bottom);
        wb.move_panel(PanelPosition::Right);
        assert_eq!(wb.panel.position, PanelPosition::Right);
    }

    #[test]
    fn split_editor_horizontal() {
        let mut wb = WorkbenchLayout::default();
        assert_eq!(wb.editor_area.groups.len(), 1);
        wb.split_editor(GroupOrientation::Horizontal);
        assert_eq!(wb.editor_area.groups.len(), 2);
        assert!((wb.editor_area.groups[0].width_ratio - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn split_editor_vertical() {
        let mut wb = WorkbenchLayout::default();
        wb.split_editor(GroupOrientation::Vertical);
        assert_eq!(wb.editor_area.groups.len(), 2);
        assert!((wb.editor_area.groups[0].height_ratio - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn zen_mode_hides_chrome() {
        let mut wb = WorkbenchLayout::default();
        wb.enter_zen_mode();
        assert!(wb.zen_mode);
        assert!(!wb.sidebar.visible);
        assert!(!wb.panel.visible);
        assert!(!wb.activity_bar.visible);
        assert!(!wb.status_bar.visible);
        assert!(!wb.minimap.visible);
    }

    #[test]
    fn exit_zen_mode_restores() {
        let mut wb = WorkbenchLayout::default();
        wb.enter_zen_mode();
        wb.exit_zen_mode();
        assert!(!wb.zen_mode);
        assert!(wb.sidebar.visible);
        assert!(wb.activity_bar.visible);
        assert!(wb.status_bar.visible);
    }

    #[test]
    fn grid_layout_2x2() {
        let mut wb = WorkbenchLayout::default();
        wb.set_grid_layout(2, 2);
        assert_eq!(wb.editor_area.groups.len(), 4);
        for g in &wb.editor_area.groups {
            assert!((g.width_ratio - 0.5).abs() < f32::EPSILON);
            assert!((g.height_ratio - 0.5).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn grid_layout_clamped() {
        let mut wb = WorkbenchLayout::default();
        wb.set_grid_layout(10, 10);
        assert_eq!(wb.editor_area.groups.len(), 16); // 4x4 max
    }

    #[test]
    fn apply_workbench_to_physical() {
        let mut layout = Layout::default();
        let mut wb = WorkbenchLayout::default();
        wb.sidebar.visible = false;
        wb.activity_bar.visible = false;
        wb.status_bar.visible = false;
        layout.apply_workbench(&wb);
        assert!(!layout.sidebar_visible);
        assert!((layout.activity_bar_width).abs() < f32::EPSILON);
        assert!((layout.status_bar_height).abs() < f32::EPSILON);
    }

    #[test]
    fn persistence_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();

        let mut wb = WorkbenchLayout::default();
        wb.sidebar.width = 300.0;
        wb.panel.visible = true;
        wb.panel.active_panel = "output".into();
        wb.save_layout(&db).unwrap();

        let restored = WorkbenchLayout::restore_layout(&db).unwrap();
        assert!((restored.sidebar.width - 300.0).abs() < f32::EPSILON);
        assert!(restored.panel.visible);
        assert_eq!(restored.panel.active_panel, "output");
    }

    #[test]
    fn restore_missing_returns_default() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();
        let restored = WorkbenchLayout::restore_layout(&db).unwrap();
        assert!(restored.sidebar.visible);
        assert!(!restored.zen_mode);
    }
}
