//! Test explorer panel — tree view of workspace tests with run/debug controls,
//! filtering, sorting, continuous run mode, and coverage summary.

use std::path::PathBuf;
use std::time::Duration;

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Test state ───────────────────────────────────────────────────────────────

/// Visual state of a test node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TestState {
    #[default]
    Unrun,
    Running,
    Passed,
    Failed,
    Skipped,
    Errored,
}

impl TestState {
    pub fn icon_char(self) -> char {
        match self {
            Self::Unrun => '○',
            Self::Running => '◌',
            Self::Passed => '✓',
            Self::Failed => '✗',
            Self::Skipped => '–',
            Self::Errored => '!',
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Unrun => Color::from_rgb(150, 150, 150),
            Self::Running => Color::from_rgb(77, 153, 255),
            Self::Passed => Color::from_rgb(78, 201, 100),
            Self::Failed => Color::from_rgb(244, 71, 71),
            Self::Skipped => Color::from_rgb(150, 150, 150),
            Self::Errored => Color::from_rgb(244, 71, 71),
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Passed | Self::Failed | Self::Skipped | Self::Errored)
    }
}

// ── Test node kind ───────────────────────────────────────────────────────────

/// The kind of node in the test tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestNodeKind {
    #[default]
    Root,
    File,
    Suite,
    Test,
    Parameterized,
}

impl TestNodeKind {
    pub fn icon_char(self) -> char {
        match self {
            Self::Root => '⊞',
            Self::File => '📄',
            Self::Suite => '◈',
            Self::Test => '▷',
            Self::Parameterized => '⟐',
        }
    }
}

// ── Test location ────────────────────────────────────────────────────────────

/// Source location of a test for click-to-navigate.
#[derive(Clone, Debug)]
pub struct TestLocation {
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
}

// ── Test tree node ───────────────────────────────────────────────────────────

/// A node in the test explorer tree.
#[derive(Clone, Debug)]
pub struct TestTreeNode {
    pub id: String,
    pub label: String,
    pub kind: TestNodeKind,
    pub state: TestState,
    pub children: Vec<TestTreeNode>,
    pub duration: Option<Duration>,
    pub error_message: Option<String>,
    pub location: Option<TestLocation>,
    pub expanded: bool,
}

impl TestTreeNode {
    pub fn new(id: impl Into<String>, label: impl Into<String>, kind: TestNodeKind) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            state: TestState::Unrun,
            children: Vec::new(),
            duration: None,
            error_message: None,
            location: None,
            expanded: true,
        }
    }

    pub fn with_children(mut self, children: Vec<TestTreeNode>) -> Self {
        self.children = children;
        self
    }

    pub fn with_location(mut self, loc: TestLocation) -> Self {
        self.location = Some(loc);
        self
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn duration_text(&self) -> Option<String> {
        self.duration.map(|d| {
            if d.as_secs() > 0 {
                format!("{:.1}s", d.as_secs_f64())
            } else {
                format!("{}ms", d.as_millis())
            }
        })
    }

    /// Total test count (leaf nodes only).
    pub fn test_count(&self) -> usize {
        if self.is_leaf() && self.kind == TestNodeKind::Test {
            1
        } else {
            self.children.iter().map(|c| c.test_count()).sum()
        }
    }

    /// Count of tests in a given state.
    pub fn count_by_state(&self, state: TestState) -> usize {
        let self_count = if self.is_leaf() && self.state == state { 1 } else { 0 };
        self_count + self.children.iter().map(|c| c.count_by_state(state)).sum::<usize>()
    }

    /// Find a node by id (depth-first).
    pub fn find_by_id(&self, id: &str) -> Option<&TestTreeNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_id(id) {
                return Some(found);
            }
        }
        None
    }

    /// Find a mutable node by id (depth-first).
    pub fn find_by_id_mut(&mut self, id: &str) -> Option<&mut TestTreeNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_by_id_mut(id) {
                return Some(found);
            }
        }
        None
    }

    /// Collect all test ids (leaf nodes).
    pub fn collect_test_ids(&self) -> Vec<String> {
        if self.is_leaf() && self.kind == TestNodeKind::Test {
            return vec![self.id.clone()];
        }
        self.children.iter().flat_map(|c| c.collect_test_ids()).collect()
    }

    /// Collect ids of failed tests.
    pub fn collect_failed_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        if self.is_leaf() && (self.state == TestState::Failed || self.state == TestState::Errored) {
            ids.push(self.id.clone());
        }
        for child in &self.children {
            ids.extend(child.collect_failed_ids());
        }
        ids
    }
}

// ── Sort order ───────────────────────────────────────────────────────────────

/// How tests are sorted in the tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestSortOrder {
    #[default]
    Alphabetical,
    Duration,
    Status,
    Location,
}

// ── Test run profile ─────────────────────────────────────────────────────────

/// Kind of test run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestRunKind {
    #[default]
    Run,
    Debug,
    Coverage,
}

impl TestRunKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Run => "Run Tests",
            Self::Debug => "Debug Tests",
            Self::Coverage => "Run with Coverage",
        }
    }
}

/// A named test execution profile.
#[derive(Clone, Debug)]
pub struct TestRunProfile {
    pub kind: TestRunKind,
    pub label: String,
    pub is_default: bool,
}

impl TestRunProfile {
    pub fn run() -> Self {
        Self {
            kind: TestRunKind::Run,
            label: "Run".into(),
            is_default: true,
        }
    }

    pub fn debug() -> Self {
        Self {
            kind: TestRunKind::Debug,
            label: "Debug".into(),
            is_default: false,
        }
    }

    pub fn coverage() -> Self {
        Self {
            kind: TestRunKind::Coverage,
            label: "Coverage".into(),
            is_default: false,
        }
    }
}

// ── Test explorer events ─────────────────────────────────────────────────────

/// Events emitted by the test explorer panel.
#[derive(Clone, Debug)]
pub enum TestExplorerEvent {
    RunAll,
    RunFailed,
    RunTest(String),
    DebugTest(String),
    CoverageTest(String),
    CancelRun,
    NavigateToTest(PathBuf, u32, u32),
    ToggleContinuousRun,
    ToggleAutoRun,
}

// ── Coverage summary display ─────────────────────────────────────────────────

/// Coverage statistics for display at top of test explorer.
#[derive(Clone, Debug, Default)]
pub struct CoverageSummaryDisplay {
    pub line_percentage: f64,
    pub branch_percentage: f64,
    pub function_percentage: f64,
    pub visible: bool,
}

impl CoverageSummaryDisplay {
    pub fn text(&self) -> String {
        format!(
            "Lines: {:.0}%  Branches: {:.0}%  Functions: {:.0}%",
            self.line_percentage, self.branch_percentage, self.function_percentage
        )
    }
}

// ── Test explorer ────────────────────────────────────────────────────────────

/// The Testing sidebar panel.
#[allow(dead_code)]
pub struct TestExplorer<OnEvent>
where
    OnEvent: FnMut(TestExplorerEvent),
{
    pub root: TestTreeNode,
    pub filter: String,
    pub sort_order: TestSortOrder,
    pub show_only_failed: bool,
    pub auto_run: bool,
    pub continuous_run: bool,
    pub profiles: Vec<TestRunProfile>,
    pub coverage_summary: CoverageSummaryDisplay,

    pub on_event: OnEvent,

    selected_id: Option<String>,
    scroll_offset: f32,
    focused: bool,
    is_running: bool,

    row_height: f32,
    toolbar_height: f32,
    indent_width: f32,
    filter_bar_height: f32,

    background: Color,
    toolbar_bg: Color,
    toolbar_button_hover: Color,
    selected_bg: Color,
    hover_bg: Color,
    foreground: Color,
    secondary_fg: Color,
    separator_color: Color,
    filter_bg: Color,
    coverage_bar_bg: Color,
    coverage_bar_fill: Color,
}

impl<OnEvent> TestExplorer<OnEvent>
where
    OnEvent: FnMut(TestExplorerEvent),
{
    pub fn new(on_event: OnEvent) -> Self {
        Self {
            root: TestTreeNode::new("root", "Tests", TestNodeKind::Root),
            filter: String::new(),
            sort_order: TestSortOrder::Alphabetical,
            show_only_failed: false,
            auto_run: false,
            continuous_run: false,
            profiles: vec![
                TestRunProfile::run(),
                TestRunProfile::debug(),
                TestRunProfile::coverage(),
            ],
            coverage_summary: CoverageSummaryDisplay::default(),

            on_event,

            selected_id: None,
            scroll_offset: 0.0,
            focused: false,
            is_running: false,

            row_height: 22.0,
            toolbar_height: 28.0,
            indent_width: 16.0,
            filter_bar_height: 28.0,

            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            toolbar_bg: Color::from_hex("#383838").unwrap_or(Color::BLACK),
            toolbar_button_hover: Color::from_hex("#505050").unwrap_or(Color::BLACK),
            selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            filter_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            coverage_bar_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            coverage_bar_fill: Color::from_rgb(78, 201, 100),
        }
    }

    pub fn set_test_tree(&mut self, root: TestTreeNode) {
        self.root = root;
    }

    pub fn set_filter(&mut self, filter: impl Into<String>) {
        self.filter = filter.into();
    }

    pub fn set_sort_order(&mut self, order: TestSortOrder) {
        self.sort_order = order;
    }

    pub fn toggle_show_only_failed(&mut self) {
        self.show_only_failed = !self.show_only_failed;
    }

    pub fn set_running(&mut self, running: bool) {
        self.is_running = running;
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    pub fn update_test_state(&mut self, test_id: &str, state: TestState) {
        if let Some(node) = self.root.find_by_id_mut(test_id) {
            node.state = state;
        }
    }

    pub fn update_test_duration(&mut self, test_id: &str, duration: Duration) {
        if let Some(node) = self.root.find_by_id_mut(test_id) {
            node.duration = Some(duration);
        }
    }

    pub fn update_test_error(&mut self, test_id: &str, message: impl Into<String>) {
        if let Some(node) = self.root.find_by_id_mut(test_id) {
            node.error_message = Some(message.into());
        }
    }

    pub fn reset_all_states(&mut self) {
        reset_states_recursive(&mut self.root);
    }

    pub fn set_coverage_summary(&mut self, summary: CoverageSummaryDisplay) {
        self.coverage_summary = summary;
    }

    // ── Statistics ────────────────────────────────────────────────────────

    pub fn total_tests(&self) -> usize {
        self.root.test_count()
    }

    pub fn passed_count(&self) -> usize {
        self.root.count_by_state(TestState::Passed)
    }

    pub fn failed_count(&self) -> usize {
        self.root.count_by_state(TestState::Failed)
    }

    pub fn status_text(&self) -> String {
        let total = self.total_tests();
        let passed = self.passed_count();
        let failed = self.failed_count();
        if self.is_running {
            format!("Running... {passed}/{total}")
        } else {
            format!("{passed}/{total} passed, {failed} failed")
        }
    }

    // ── Navigation ───────────────────────────────────────────────────────

    fn navigate_to_selected(&mut self) {
        if let Some(ref id) = self.selected_id {
            if let Some(node) = self.root.find_by_id(id) {
                if let Some(ref loc) = node.location {
                    let path = loc.file.clone();
                    (self.on_event)(TestExplorerEvent::NavigateToTest(path, loc.line, loc.column));
                }
            }
        }
    }

    fn run_selected(&mut self) {
        if let Some(ref id) = self.selected_id {
            (self.on_event)(TestExplorerEvent::RunTest(id.clone()));
        }
    }

    // ── Toolbar buttons ──────────────────────────────────────────────────

    fn toolbar_buttons() -> &'static [&'static str] {
        &["Run All", "Run Failed", "Cancel", "⟳", "☰"]
    }

    // ── Tree flattening for rendering ────────────────────────────────────

    #[allow(clippy::cast_precision_loss)]
    fn flatten_visible_rows(&self) -> Vec<FlatRow<'_>> {
        let mut rows = Vec::new();
        flatten_node(&self.root, 0, &self.filter, self.show_only_failed, &mut rows);
        rows
    }
}

/// A flattened row for rendering.
struct FlatRow<'a> {
    node: &'a TestTreeNode,
    depth: usize,
}

fn flatten_node<'a>(
    node: &'a TestTreeNode,
    depth: usize,
    filter: &str,
    failed_only: bool,
    out: &mut Vec<FlatRow<'a>>,
) {
    if node.kind != TestNodeKind::Root {
        if failed_only && node.is_leaf() && node.state != TestState::Failed && node.state != TestState::Errored {
            return;
        }
        if !filter.is_empty() {
            let lower = filter.to_lowercase();
            let name_match = node.label.to_lowercase().contains(&lower);
            let child_match = node.children.iter().any(|c| {
                c.label.to_lowercase().contains(&lower)
                    || c.children.iter().any(|gc| gc.label.to_lowercase().contains(&lower))
            });
            if !name_match && !child_match && node.is_leaf() {
                return;
            }
        }
        out.push(FlatRow { node, depth });
    }

    if node.expanded || node.kind == TestNodeKind::Root {
        for child in &node.children {
            flatten_node(child, depth + 1, filter, failed_only, out);
        }
    }
}

fn reset_states_recursive(node: &mut TestTreeNode) {
    node.state = TestState::Unrun;
    node.duration = None;
    node.error_message = None;
    for child in &mut node.children {
        reset_states_recursive(child);
    }
}

impl<OnEvent> Widget for TestExplorer<OnEvent>
where
    OnEvent: FnMut(TestExplorerEvent),
{
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Flex(1.0),
            ..LayoutNode::default()
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, self.background, 0.0);

        let mut y = rect.y;

        // Toolbar
        rr.draw_rect(rect.x, y, rect.width, self.toolbar_height, self.toolbar_bg, 0.0);
        let buttons = Self::toolbar_buttons();
        let btn_size = 24.0;
        let btn_pad = 4.0;
        let mut bx = rect.x + 4.0;
        for _btn in buttons {
            rr.draw_rect(bx, y + 2.0, btn_size, btn_size, self.toolbar_button_hover, 3.0);
            bx += btn_size + btn_pad;
        }
        y += self.toolbar_height;
        rr.draw_rect(rect.x, y, rect.width, 1.0, self.separator_color, 0.0);
        y += 1.0;

        // Filter bar
        rr.draw_rect(rect.x, y, rect.width, self.filter_bar_height, self.filter_bg, 0.0);
        y += self.filter_bar_height;
        rr.draw_rect(rect.x, y, rect.width, 1.0, self.separator_color, 0.0);
        y += 1.0;

        // Coverage summary bar
        if self.coverage_summary.visible {
            let bar_h = 20.0;
            rr.draw_rect(rect.x, y, rect.width, bar_h, self.coverage_bar_bg, 0.0);
            let fill_w = rect.width * (self.coverage_summary.line_percentage / 100.0) as f32;
            rr.draw_rect(rect.x, y, fill_w, bar_h, self.coverage_bar_fill, 0.0);
            y += bar_h;
            rr.draw_rect(rect.x, y, rect.width, 1.0, self.separator_color, 0.0);
            y += 1.0;
        }

        // Test tree rows
        let rows = self.flatten_visible_rows();
        for row in &rows {
            let ry = y - self.scroll_offset;
            if ry > rect.y + rect.height {
                break;
            }
            if ry + self.row_height < rect.y {
                y += self.row_height;
                continue;
            }

            let is_selected = self.selected_id.as_deref() == Some(&row.node.id);
            if is_selected {
                rr.draw_rect(rect.x, ry, rect.width, self.row_height, self.selected_bg, 0.0);
            }

            let indent = rect.x + 4.0 + row.depth as f32 * self.indent_width;

            // State icon dot
            let dot_r = 4.0;
            rr.draw_rect(
                indent,
                ry + self.row_height / 2.0 - dot_r,
                dot_r * 2.0,
                dot_r * 2.0,
                row.node.state.color(),
                dot_r,
            );

            // Error indicator
            if row.node.error_message.is_some() {
                let err_x = rect.x + rect.width - 14.0;
                rr.draw_rect(
                    err_x,
                    ry + 3.0,
                    10.0,
                    self.row_height - 6.0,
                    TestState::Failed.color(),
                    2.0,
                );
            }

            y += self.row_height;
        }

        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => {
                self.focused = true;
                EventResult::Handled
            }
            UiEvent::Blur => {
                self.focused = false;
                EventResult::Handled
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.focused = true;

                // Toolbar click
                if *y < rect.y + self.toolbar_height {
                    let btn_size = 24.0;
                    let btn_pad = 4.0;
                    let buttons = Self::toolbar_buttons();
                    let mut bx = rect.x + 4.0;
                    for (i, _) in buttons.iter().enumerate() {
                        if *x >= bx && *x < bx + btn_size {
                            match i {
                                0 => (self.on_event)(TestExplorerEvent::RunAll),
                                1 => (self.on_event)(TestExplorerEvent::RunFailed),
                                2 => (self.on_event)(TestExplorerEvent::CancelRun),
                                3 => (self.on_event)(TestExplorerEvent::ToggleContinuousRun),
                                _ => {}
                            }
                            return EventResult::Handled;
                        }
                        bx += btn_size + btn_pad;
                    }
                    return EventResult::Handled;
                }

                // Row click
                let content_y = rect.y + self.toolbar_height + 1.0 + self.filter_bar_height + 1.0
                    + if self.coverage_summary.visible { 22.0 } else { 0.0 };
                if *y >= content_y {
                    let rows = self.flatten_visible_rows();
                    let row_idx = ((*y - content_y + self.scroll_offset) / self.row_height) as usize;
                    let hit = rows.get(row_idx).map(|row| {
                        (row.node.id.clone(), row.node.is_leaf(), row.node.location.as_ref().map(|loc| (loc.file.clone(), loc.line, loc.column)))
                    });
                    if let Some((id, is_leaf, loc)) = hit {
                        self.selected_id = Some(id.clone());
                        if !is_leaf {
                            if let Some(node) = self.root.find_by_id_mut(&id) {
                                node.expanded = !node.expanded;
                            }
                        } else if let Some((path, line, col)) = loc {
                            (self.on_event)(TestExplorerEvent::NavigateToTest(path, line, col));
                        }
                    }
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                self.scroll_offset = (self.scroll_offset - dy * 40.0).max(0.0);
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Enter, .. } if self.focused => {
                self.navigate_to_selected();
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Char('r'), .. } if self.focused => {
                self.run_selected();
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
