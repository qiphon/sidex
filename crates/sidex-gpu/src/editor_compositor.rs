//! Full editor view compositor — the master renderer that composes all visual
//! elements into a complete VS Code-like editor view.
//!
//! Layers (bottom to top):
//!  0. Backgrounds (editor bg, gutter bg, current line highlight)
//!  1. Selections
//!  2. Find match highlights
//!  3. Indent guides
//!  4. Text (syntax highlighted)
//!  5. Bracket pair colorization
//!  6. Diagnostic squigglies
//!  7. Inlay hints
//!  8. Cursors
//!  9. Gutter (line numbers, folds, breakpoints, diff, diagnostics)
//! 10. Sticky scroll
//! 11. Minimap
//! 12. Scrollbars
//! 13. Scroll shadow
//!
//! The compositor also supports:
//! - Partial re-rendering via dirty regions
//! - Layer compositing with z-ordering, clip regions, opacity, transforms
//! - Double-buffered rendering & VSync awareness
//! - Frame timing and graceful frame dropping

use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::color::Color;
use crate::cursor_renderer::{CursorPosition, CursorRenderer, CursorStyle};
use crate::diagnostic_gutter::DiagnosticGutterRenderer;
use crate::gutter::{
    Breakpoint, FoldMarker, GutterDiagnostic, GutterDiffMark, GutterRenderer,
};
use crate::line_renderer::{
    IndentGuide, InlayHint, LineRenderConfig, LineRenderer, StyledLine, Viewport,
};
use crate::minimap::{
    DiagnosticMark, GitChange, LineRange, MinimapConfig, MinimapRenderer, MinimapViewport,
    StyledLine as MinimapStyledLine,
};
use crate::rect_renderer::RectRenderer;
use crate::scene::{Layer, Scene};
use crate::scroll::{OverviewRulerMark, ScrollbarRenderer};
use crate::selection_renderer::{
    BracketHighlight, HighlightRect, SelectionRect, SelectionRenderer,
};
use crate::squiggly::{SquigglyRenderer, SquigglySeverity};
use crate::text_atlas::TextAtlas;
use crate::text_renderer::{TextDrawContext, TextRenderer};

// ── Compositor layer system ──────────────────────────────────────────────────

/// Unique identifier for a compositor layer.
pub type LayerId = u64;

/// A single compositing layer with bounds, z-ordering, opacity, and clip.
#[derive(Debug, Clone)]
pub struct CompositorLayer {
    pub id: LayerId,
    pub bounds: Rect,
    pub z_index: i32,
    pub opacity: f32,
    pub visible: bool,
    pub clip: Option<Rect>,
    pub transform: Option<Transform2D>,
    pub content: LayerContent,
}

/// What kind of content a compositor layer holds.
#[derive(Debug, Clone)]
pub enum LayerContent {
    EditorContent { visible_lines: Range<u32>, scroll_offset: f32 },
    Gutter { width: f32, line_range: Range<u32> },
    Minimap { data: MinimapRenderData },
    Scrollbar { orientation: ScrollbarOrientation, thumb_position: f32, thumb_size: f32 },
    Overlay { widget: OverlayWidget },
    Panel { panel_type: PanelType },
    StatusBar,
    TitleBar,
    ActivityBar,
    Sidebar { content: SidebarContent },
    TabBar { tabs: Vec<TabRenderData> },
}

/// Orientation of a scrollbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarOrientation {
    Vertical,
    Horizontal,
}

/// Data needed to render the minimap within a compositor layer.
#[derive(Debug, Clone)]
pub struct MinimapRenderData {
    pub total_lines: u32,
    pub line_height: f32,
}

/// An overlay widget rendered on top of the editor.
#[derive(Debug, Clone)]
pub struct OverlayWidget {
    pub id: String,
    pub bounds: Rect,
}

/// Panel type for side/bottom panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelType {
    Terminal,
    Output,
    Problems,
    DebugConsole,
    Explorer,
    Search,
    SourceControl,
    Extensions,
    Custom,
}

/// Content type for the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarContent {
    Explorer,
    Search,
    SourceControl,
    Debug,
    Extensions,
}

/// Render data for a single editor tab.
#[derive(Debug, Clone)]
pub struct TabRenderData {
    pub label: String,
    pub is_active: bool,
    pub is_dirty: bool,
    pub icon: Option<String>,
}

/// 2D affine transform for smooth animations.
#[derive(Debug, Clone, Copy)]
pub struct Transform2D {
    pub translate: (f32, f32),
    pub scale: (f32, f32),
    pub rotation: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self { translate: (0.0, 0.0), scale: (1.0, 1.0), rotation: 0.0 }
    }
}

// ── Dirty region tracking ───────────────────────────────────────────────────

/// A rectangular region that needs repainting.
#[derive(Debug, Clone, Copy)]
pub struct DirtyRegion {
    pub bounds: Rect,
    pub reason: DirtyReason,
}

/// Why a region needs repainting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirtyReason {
    TextChanged,
    CursorMoved,
    SelectionChanged,
    ScrollChanged,
    ThemeChanged,
    Resize,
    AnimationTick,
    OverlayChanged,
}

// ── Frame timing ────────────────────────────────────────────────────────────

/// Tracks frame timing for the compositor.
#[derive(Debug, Clone)]
pub struct FrameTiming {
    pub frame_count: u64,
    pub last_frame_time: Duration,
    pub target_fps: u32,
    pub last_frame_start: Instant,
    pub frame_times: Vec<Duration>,
    pub dropped_frames: u64,
}

impl Default for FrameTiming {
    fn default() -> Self {
        Self {
            frame_count: 0,
            last_frame_time: Duration::ZERO,
            target_fps: 60,
            last_frame_start: Instant::now(),
            frame_times: Vec::with_capacity(120),
            dropped_frames: 0,
        }
    }
}

impl FrameTiming {
    pub fn begin_frame(&mut self) {
        self.last_frame_start = Instant::now();
    }

    pub fn end_frame(&mut self) {
        let elapsed = self.last_frame_start.elapsed();
        self.last_frame_time = elapsed;
        self.frame_count += 1;

        self.frame_times.push(elapsed);
        if self.frame_times.len() > 120 {
            self.frame_times.remove(0);
        }

        let budget = Duration::from_secs_f64(1.0 / f64::from(self.target_fps));
        if elapsed > budget {
            self.dropped_frames += 1;
        }
    }

    pub fn average_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.frame_times.iter().sum();
        total / self.frame_times.len() as u32
    }

    pub fn current_fps(&self) -> f64 {
        let avg = self.average_frame_time();
        if avg.is_zero() {
            return 0.0;
        }
        1.0 / avg.as_secs_f64()
    }

    pub fn frame_budget(&self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.target_fps))
    }
}

// ── Rendered frame ──────────────────────────────────────────────────────────

/// A fully composed frame ready for presentation.
pub struct ComposedFrame {
    pub layers: Vec<CompositorLayer>,
    pub viewport: Viewport,
    pub frame_number: u64,
    pub render_time: Duration,
}

// ── Enums ────────────────────────────────────────────────────────────────────

/// Cursor blinking style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorBlinking {
    #[default]
    Blink,
    Smooth,
    Phase,
    Expand,
    Solid,
}

/// Which side the minimap renders on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MinimapSide {
    #[default]
    Right,
    Left,
}

/// Line number rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineNumbers {
    #[default]
    On,
    Off,
    Relative,
    Interval,
}

/// Whitespace rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RenderWhitespace {
    #[default]
    None,
    Boundary,
    Selection,
    All,
    Trailing,
}

/// Word-wrap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WordWrap {
    #[default]
    Off,
    On,
    WordWrapColumn,
    Bounded,
}

/// Gradient direction for scroll shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientDirection {
    TopToBottom,
    LeftToRight,
}

// ── EditorConfig ─────────────────────────────────────────────────────────────

/// Complete editor configuration controlling all visual aspects.
#[derive(Debug, Clone)]
pub struct CompositorConfig {
    pub font_size: f32,
    pub font_family: String,
    pub line_height: f32,
    pub char_width: f32,
    pub cursor_style: CursorStyle,
    pub cursor_blinking: CursorBlinking,
    pub minimap_enabled: bool,
    pub minimap_side: MinimapSide,
    pub line_numbers: LineNumbers,
    pub render_whitespace: RenderWhitespace,
    pub render_indent_guides: bool,
    pub bracket_pair_colorization: bool,
    pub sticky_scroll_enabled: bool,
    pub word_wrap: WordWrap,
    pub inlay_hints_enabled: bool,
    pub folding_enabled: bool,
    pub glyph_margin: bool,
}

impl Default for CompositorConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            font_family: String::from("monospace"),
            line_height: 20.0,
            char_width: 8.4,
            cursor_style: CursorStyle::Line,
            cursor_blinking: CursorBlinking::Blink,
            minimap_enabled: true,
            minimap_side: MinimapSide::Right,
            line_numbers: LineNumbers::On,
            render_whitespace: RenderWhitespace::None,
            render_indent_guides: true,
            bracket_pair_colorization: true,
            sticky_scroll_enabled: true,
            word_wrap: WordWrap::Off,
            inlay_hints_enabled: true,
            folding_enabled: true,
            glyph_margin: true,
        }
    }
}

// ── Rect helper ──────────────────────────────────────────────────────────────

/// Axis-aligned rectangle used for sub-region layout.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = (self.x + self.width).min(other.x + other.width);
        let bottom = (self.y + self.height).min(other.y + other.height);
        if right > x && bottom > y {
            Some(Rect { x, y, width: right - x, height: bottom - y })
        } else {
            None
        }
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Rect { x, y, width: right - x, height: bottom - y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
    }
}

// ── Theme colors snapshot ────────────────────────────────────────────────────

/// Subset of theme colors needed for compositor rendering.
#[derive(Debug, Clone)]
pub struct CompositorTheme {
    pub editor_background: Color,
    pub editor_foreground: Color,
    pub editor_gutter_background: Color,
    pub editor_line_highlight_background: Color,
    pub editor_selection_background: Color,
    pub editor_find_match_highlight_background: Color,
    pub editor_find_range_highlight_background: Color,
    pub editor_error_foreground: Color,
    pub editor_warning_foreground: Color,
    pub editor_info_foreground: Color,
    pub editor_hint_foreground: Color,
    pub editor_indent_guide_color: Color,
    pub editor_indent_guide_active_color: Color,
    pub editor_bracket_match_border: Color,
    pub editor_bracket_pair_colors: [Color; 6],
    pub editor_inlay_hint_foreground: Color,
    pub editor_inlay_hint_background: Color,
    pub editor_sticky_scroll_background: Color,
    pub editor_sticky_scroll_border: Color,
    pub scrollbar_shadow: Color,
    pub line_number_foreground: Color,
    pub line_number_active_foreground: Color,
}

impl Default for CompositorTheme {
    fn default() -> Self {
        Self {
            editor_background: Color::from_hex("#1e1e1e").unwrap(),
            editor_foreground: Color::from_hex("#d4d4d4").unwrap(),
            editor_gutter_background: Color::from_hex("#1e1e1e").unwrap(),
            editor_line_highlight_background: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.04 },
            editor_selection_background: Color { r: 0.17, g: 0.34, b: 0.56, a: 0.6 },
            editor_find_match_highlight_background: Color { r: 0.9, g: 0.8, b: 0.2, a: 0.35 },
            editor_find_range_highlight_background: Color { r: 0.95, g: 0.6, b: 0.15, a: 0.55 },
            editor_error_foreground: Color::from_rgb(244, 71, 71),
            editor_warning_foreground: Color::from_rgb(205, 173, 0),
            editor_info_foreground: Color::from_rgb(55, 148, 255),
            editor_hint_foreground: Color::from_rgb(160, 160, 160),
            editor_indent_guide_color: Color { r: 0.3, g: 0.3, b: 0.3, a: 0.3 },
            editor_indent_guide_active_color: Color { r: 0.5, g: 0.5, b: 0.5, a: 0.5 },
            editor_bracket_match_border: Color::from_rgb(136, 136, 136),
            editor_bracket_pair_colors: [
                Color::from_rgb(255, 215, 0),
                Color::from_rgb(218, 112, 214),
                Color::from_rgb(23, 159, 255),
                Color::from_rgb(255, 215, 0),
                Color::from_rgb(218, 112, 214),
                Color::from_rgb(23, 159, 255),
            ],
            editor_inlay_hint_foreground: Color::from_rgb(140, 140, 140),
            editor_inlay_hint_background: Color { r: 0.2, g: 0.2, b: 0.2, a: 0.5 },
            editor_sticky_scroll_background: Color::from_hex("#1e1e1e").unwrap(),
            editor_sticky_scroll_border: Color { r: 1.0, g: 1.0, b: 1.0, a: 0.1 },
            scrollbar_shadow: Color { r: 0.0, g: 0.0, b: 0.0, a: 0.4 },
            line_number_foreground: Color::from_rgb(130, 130, 130),
            line_number_active_foreground: Color::from_rgb(220, 220, 220),
        }
    }
}

// ── Diagnostic info ──────────────────────────────────────────────────────────

/// Diagnostic severity used by the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorDiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A diagnostic range for squiggly rendering.
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// A diagnostic entry for the compositor.
#[derive(Debug, Clone)]
pub struct CompositorDiagnostic {
    pub range: DiagnosticRange,
    pub severity: CompositorDiagnosticSeverity,
}

// ── Bracket pair ─────────────────────────────────────────────────────────────

/// A matched bracket pair for colorization.
#[derive(Debug, Clone, Copy)]
pub struct BracketPair {
    pub open_line: u32,
    pub open_col: u32,
    pub close_line: u32,
    pub close_col: u32,
    pub nesting_level: u32,
}

// ── Find state ───────────────────────────────────────────────────────────────

/// A single find match range.
#[derive(Debug, Clone, Copy)]
pub struct FindMatch {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// State of the find/replace widget.
#[derive(Debug, Clone)]
pub struct FindState {
    pub matches: Vec<FindMatch>,
    pub current_match: usize,
}

// ── Scroll state ─────────────────────────────────────────────────────────────

/// Scroll position for the editor viewport.
#[derive(Debug, Clone, Copy)]
pub struct ScrollState {
    pub scroll_x: f64,
    pub scroll_y: f64,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self { scroll_x: 0.0, scroll_y: 0.0 }
    }
}

// ── Document snapshot ────────────────────────────────────────────────────────

/// Snapshot of document state fed to the compositor each frame.
pub struct CompositorDocumentSnapshot {
    pub total_lines: u32,
    pub max_line_width: u32,
    pub cursor_line: u32,
    pub cursor_col: u32,
    pub highlight_lines: Vec<StyledLine>,
    pub selections: Vec<SelectionRect>,
    pub cursor_positions: Vec<CursorPosition>,
    pub find_state: Option<FindState>,
    pub diagnostics: Vec<CompositorDiagnostic>,
    pub bracket_pairs: Vec<BracketPair>,
    pub inlay_hints: Vec<Vec<InlayHint>>,
    pub indent_guides: Vec<IndentGuide>,
    pub sticky_lines: Vec<StyledLine>,
    pub folds: Vec<FoldMarker>,
    pub breakpoints: Vec<Breakpoint>,
    pub gutter_diff_marks: Vec<GutterDiffMark>,
    pub gutter_diagnostics: Vec<GutterDiagnostic>,
    pub minimap_lines: Vec<MinimapStyledLine>,
    pub minimap_selections: Vec<LineRange>,
    pub minimap_search_matches: Vec<LineRange>,
    pub minimap_diagnostics: Vec<DiagnosticMark>,
    pub minimap_git_changes: Vec<GitChange>,
    pub overview_marks: Vec<OverviewRulerMark>,
    pub word_highlights: Vec<HighlightRect>,
    pub bracket_highlights: Vec<BracketHighlight>,
}

// ── EditorCompositor ─────────────────────────────────────────────────────────

/// The master compositor that composes every visual element into a complete
/// editor view, matching VS Code's layered rendering approach.
///
/// Supports partial re-rendering via dirty regions, layer compositing with
/// z-ordering, clip regions, opacity, transforms, double-buffered rendering,
/// and frame timing with graceful frame dropping.
pub struct EditorCompositor {
    pub text_renderer: TextRenderer,
    pub rect_renderer: RectRenderer,
    pub line_renderer: LineRenderer,
    pub cursor_renderer: CursorRenderer,
    pub selection_renderer: SelectionRenderer,
    pub gutter_renderer: GutterRenderer,
    pub minimap_renderer: MinimapRenderer,
    pub scrollbar_renderer: ScrollbarRenderer,
    pub squiggly_renderer: SquigglyRenderer,
    pub diagnostic_gutter: DiagnosticGutterRenderer,
    pub text_atlas: TextAtlas,
    pub scene: Scene,
    pub layers: Vec<CompositorLayer>,
    pub dirty_regions: Vec<DirtyRegion>,
    pub frame_timing: FrameTiming,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    next_layer_id: LayerId,
    front_buffer_dirty: bool,
}

impl EditorCompositor {
    /// Creates a new compositor with all sub-renderers initialized.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let text_atlas = TextAtlas::new(&device, &queue);
        Self {
            text_renderer: TextRenderer::new(),
            rect_renderer: RectRenderer::new(),
            line_renderer: LineRenderer::new(LineRenderConfig::default()),
            cursor_renderer: CursorRenderer::new(CursorStyle::Line, Color::WHITE),
            selection_renderer: SelectionRenderer::default(),
            gutter_renderer: GutterRenderer::default(),
            minimap_renderer: MinimapRenderer::new(MinimapConfig::default()),
            scrollbar_renderer: ScrollbarRenderer::default(),
            squiggly_renderer: SquigglyRenderer::new(),
            diagnostic_gutter: DiagnosticGutterRenderer::new(),
            text_atlas,
            scene: Scene::new(),
            layers: Vec::new(),
            dirty_regions: Vec::new(),
            frame_timing: FrameTiming::default(),
            device,
            queue,
            next_layer_id: 0,
            front_buffer_dirty: true,
        }
    }

    // ── Layer management ─────────────────────────────────────────────────

    /// Adds a compositor layer and returns its id.
    pub fn add_layer(&mut self, content: LayerContent, bounds: Rect, z_index: i32) -> LayerId {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        self.layers.push(CompositorLayer {
            id,
            bounds,
            z_index,
            opacity: 1.0,
            visible: true,
            clip: None,
            transform: None,
            content,
        });
        self.layers.sort_by_key(|l| l.z_index);
        id
    }

    /// Removes a layer by id.
    pub fn remove_layer(&mut self, id: LayerId) {
        self.layers.retain(|l| l.id != id);
    }

    /// Returns a mutable reference to a layer by id.
    pub fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut CompositorLayer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    // ── Dirty region management ──────────────────────────────────────────

    /// Marks a rectangular region as needing repaint.
    pub fn mark_dirty(&mut self, bounds: Rect, reason: DirtyReason) {
        self.dirty_regions.push(DirtyRegion { bounds, reason });
        self.front_buffer_dirty = true;
    }

    /// Returns `true` if any region needs repaint.
    pub fn needs_repaint(&self) -> bool {
        self.front_buffer_dirty || !self.dirty_regions.is_empty()
    }

    /// Clears all dirty regions (call after a successful repaint).
    pub fn clear_dirty(&mut self) {
        self.dirty_regions.clear();
        self.front_buffer_dirty = false;
    }

    // ── Frame composition ────────────────────────────────────────────────

    /// Composes a frame from the current layer stack and viewport.
    /// Only repaints dirty regions when possible.
    #[allow(clippy::cast_precision_loss)]
    pub fn compose_frame(
        &mut self,
        viewport: &Viewport,
    ) -> ComposedFrame {
        self.frame_timing.begin_frame();

        let sorted_layers = self.layers.clone();

        self.frame_timing.end_frame();

        ComposedFrame {
            layers: sorted_layers,
            viewport: *viewport,
            frame_number: self.frame_timing.frame_count,
            render_time: self.frame_timing.last_frame_time,
        }
    }

    /// Computes gutter width based on the number of lines and config.
    #[allow(clippy::cast_precision_loss)]
    fn compute_gutter_width(&self, total_lines: u32, config: &CompositorConfig) -> f32 {
        let digit_count = if total_lines == 0 {
            1
        } else {
            ((total_lines as f64).log10().floor() as u32) + 1
        };
        let number_width = digit_count.max(2) as f32 * config.char_width;
        let glyph_margin = if config.glyph_margin { 18.0 } else { 0.0 };
        let fold_width = if config.folding_enabled { 14.0 } else { 0.0 };
        let diff_bar = 3.0;
        let padding = 12.0;
        glyph_margin + number_width + fold_width + diff_bar + padding
    }

    /// Computes the sub-region layout for all editor areas.
    fn compute_layout(
        &self,
        editor_rect: Rect,
        total_lines: u32,
        config: &CompositorConfig,
    ) -> EditorLayout {
        let gutter_width = self.compute_gutter_width(total_lines, config);
        let minimap_width = if config.minimap_enabled { 60.0 } else { 0.0 };
        let scrollbar_width = 14.0;
        let content_width =
            editor_rect.width - gutter_width - minimap_width - scrollbar_width;

        EditorLayout {
            gutter: Rect::new(editor_rect.x, editor_rect.y, gutter_width, editor_rect.height),
            content: Rect::new(
                editor_rect.x + gutter_width,
                editor_rect.y,
                content_width.max(0.0),
                editor_rect.height,
            ),
            minimap: Rect::new(
                editor_rect.x + editor_rect.width - minimap_width - scrollbar_width,
                editor_rect.y,
                minimap_width,
                editor_rect.height,
            ),
            scrollbar: Rect::new(
                editor_rect.x + editor_rect.width - scrollbar_width,
                editor_rect.y,
                scrollbar_width,
                editor_rect.height,
            ),
            gutter_width,
            minimap_width,
            scrollbar_width,
        }
    }
}

/// Pre-computed layout rectangles for the editor regions.
#[allow(dead_code)]
struct EditorLayout {
    gutter: Rect,
    content: Rect,
    minimap: Rect,
    scrollbar: Rect,
    gutter_width: f32,
    minimap_width: f32,
    scrollbar_width: f32,
}

// ── Main render method ───────────────────────────────────────────────────────

impl EditorCompositor {
    /// Renders a complete editor frame into the rect and text batches.
    ///
    /// This is the primary entry point. After calling this, flush the
    /// `rect_renderer` and `text_renderer` into a render pass.
    #[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
    pub fn render_editor(
        &mut self,
        font_system: &mut cosmic_text::FontSystem,
        editor_rect: Rect,
        doc: &CompositorDocumentSnapshot,
        scroll: &ScrollState,
        theme: &CompositorTheme,
        config: &CompositorConfig,
        dt: f32,
    ) {
        self.cursor_renderer.update(dt);
        self.scrollbar_renderer.update(dt);

        let layout = self.compute_layout(editor_rect, doc.total_lines, config);
        let line_height = config.line_height;
        let char_width = config.char_width;

        let first_visible_line = (scroll.scroll_y / line_height as f64) as u32;
        let visible_line_count = (editor_rect.height / line_height) as u32 + 2;
        let last_visible_line =
            (first_visible_line + visible_line_count).min(doc.total_lines);

        let rects = &mut self.rect_renderer;

        // === LAYER 0: Backgrounds ===
        rects.draw_rect(
            editor_rect.x, editor_rect.y,
            editor_rect.width, editor_rect.height,
            theme.editor_background, 0.0,
        );
        rects.draw_rect(
            layout.gutter.x, layout.gutter.y,
            layout.gutter.width, layout.gutter.height,
            theme.editor_gutter_background, 0.0,
        );

        // Current line highlight
        if doc.cursor_line >= first_visible_line && doc.cursor_line < last_visible_line {
            let y = layout.content.y
                + (doc.cursor_line - first_visible_line) as f32 * line_height;
            rects.draw_rect(
                layout.content.x, y,
                layout.content.width, line_height,
                theme.editor_line_highlight_background, 0.0,
            );
        }

        // === LAYER 1: Selections ===
        self.selection_renderer.draw_selections(rects, &doc.selections);

        // === LAYER 2: Find match highlights ===
        if let Some(find_state) = &doc.find_state {
            let find_rects: Vec<HighlightRect> = find_state
                .matches
                .iter()
                .filter(|m| m.start_line >= first_visible_line && m.start_line < last_visible_line)
                .map(|m| {
                    let y = layout.content.y
                        + (m.start_line - first_visible_line) as f32 * line_height;
                    let x = layout.content.x + m.start_col as f32 * char_width
                        - scroll.scroll_x as f32;
                    let w = (m.end_col - m.start_col).max(1) as f32 * char_width;
                    HighlightRect { x, y, width: w, height: line_height }
                })
                .collect();
            self.selection_renderer.draw_find_matches(
                rects,
                &find_rects,
                Some(find_state.current_match),
            );
        }

        // === LAYER 3: Word highlights ===
        self.selection_renderer.draw_word_highlights(rects, &doc.word_highlights);

        // === LAYER 4: Indent guides ===
        if config.render_indent_guides {
            Self::render_indent_guides_static(
                rects, doc, first_visible_line, last_visible_line,
                &layout.content, line_height, char_width,
                scroll.scroll_x as f32, theme,
            );
        }

        // === LAYER 5: Text (syntax highlighted) ===
        {
            let mut ctx = TextDrawContext {
                font_system,
                atlas: &mut self.text_atlas,
                device: &self.device,
                queue: &self.queue,
            };
            let text = &mut self.text_renderer;
            let viewport = Viewport {
                first_line: first_visible_line,
                visible_lines: visible_line_count,
                scroll_x: scroll.scroll_x as f32,
                scroll_y: scroll.scroll_y as f32,
                width: layout.content.width,
                height: layout.content.height,
            };

            for line_idx in first_visible_line..last_visible_line {
                let y = layout.content.y
                    + (line_idx - first_visible_line) as f32 * line_height;
                if let Some(styled_line) = doc.highlight_lines.get(line_idx as usize) {
                    self.line_renderer.render_line(
                        text, rects, &mut ctx, styled_line, y, &viewport,
                    );
                }
            }

            // === LAYER 6: Bracket pair colorization ===
            if config.bracket_pair_colorization {
                Self::render_bracket_pairs_static(
                    rects, doc, first_visible_line, last_visible_line,
                    &layout.content, line_height, char_width,
                    scroll.scroll_x as f32, theme,
                );
            }

            // === LAYER 7: Diagnostic squigglies ===
            {
                let squiggly = &self.squiggly_renderer;
                for diag in &doc.diagnostics {
                    if diag.range.start_line >= first_visible_line
                        && diag.range.start_line < last_visible_line
                    {
                        let severity = match diag.severity {
                            CompositorDiagnosticSeverity::Error => SquigglySeverity::Error,
                            CompositorDiagnosticSeverity::Warning => SquigglySeverity::Warning,
                            CompositorDiagnosticSeverity::Information => {
                                SquigglySeverity::Information
                            }
                            CompositorDiagnosticSeverity::Hint => SquigglySeverity::Hint,
                        };
                        let y = layout.content.y
                            + (diag.range.start_line - first_visible_line) as f32 * line_height
                            + line_height
                            - 3.0;
                        let x = layout.content.x
                            + diag.range.start_col as f32 * char_width
                            - scroll.scroll_x as f32;
                        let w = (diag.range.end_col.saturating_sub(diag.range.start_col))
                            .max(1) as f32
                            * char_width;
                        squiggly.draw_squiggly(rects, x, y, w, severity.color());
                    }
                }
            }

            // === LAYER 8: Inlay hints ===
            if config.inlay_hints_enabled {
                Self::render_inlay_hints_static(
                    &self.line_renderer, text, rects, &mut ctx, doc,
                    first_visible_line, last_visible_line,
                    line_height, char_width,
                );
            }

            // === LAYER 9: Cursors ===
            self.cursor_renderer.render(rects, &doc.cursor_positions);

            // === LAYER 10: Gutter ===
            if config.line_numbers != LineNumbers::Off {
                self.gutter_renderer.render(
                    rects, text, &mut ctx,
                    first_visible_line + 1,
                    visible_line_count,
                    line_height,
                    doc.cursor_line + 1,
                    scroll.scroll_y as f32,
                    &doc.folds,
                    &doc.breakpoints,
                    &doc.gutter_diff_marks,
                    &doc.gutter_diagnostics,
                );
            }

            // === LAYER 11: Sticky scroll ===
            if config.sticky_scroll_enabled && !doc.sticky_lines.is_empty() {
                Self::render_sticky_scroll_static(
                    &mut self.line_renderer, text, rects, &mut ctx,
                    &doc.sticky_lines, layout.content.width, line_height, theme,
                );
            }
        }

        // === LAYER 12: Minimap ===
        if config.minimap_enabled {
            self.minimap_renderer
                .set_origin(layout.minimap.x, layout.minimap.y);
            let mvp = MinimapViewport {
                first_visible_line,
                visible_line_count,
                total_lines: doc.total_lines,
            };
            self.minimap_renderer.render(
                rects,
                &doc.minimap_lines,
                &mvp,
                &doc.minimap_selections,
                &doc.minimap_search_matches,
                &doc.minimap_diagnostics,
                &doc.minimap_git_changes,
            );
        }

        // === LAYER 13: Scrollbars ===
        let content_height = doc.total_lines as f32 * line_height;
        let content_width = doc.max_line_width as f32 * char_width;
        let h_scrollbar_height = self.scrollbar_renderer.config_mut().horizontal_height;
        self.scrollbar_renderer.render_vertical(
            rects, layout.content.height, content_height, layout.scrollbar.x,
        );
        self.scrollbar_renderer.render_horizontal(
            rects,
            layout.content.width,
            content_width,
            editor_rect.y + editor_rect.height - h_scrollbar_height,
        );
        self.scrollbar_renderer.render_overview_ruler(
            rects, &doc.overview_marks, layout.content.height, layout.scrollbar.x,
        );

        // === LAYER 14: Scroll shadow ===
        if scroll.scroll_y > 0.0 {
            self.scrollbar_renderer.render_scroll_shadow(rects, editor_rect.width);
        }

        // === LAYER 15: Bracket highlights (border boxes) ===
        self.selection_renderer.draw_bracket_highlights(rects, &doc.bracket_highlights);
    }
}

// ── Helper renderers ─────────────────────────────────────────────────────────

impl EditorCompositor {
    /// Renders indent guide lines for visible lines.
    #[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
    fn render_indent_guides_static(
        rects: &mut RectRenderer,
        doc: &CompositorDocumentSnapshot,
        first_visible_line: u32,
        last_visible_line: u32,
        content_rect: &Rect,
        line_height: f32,
        _char_width: f32,
        scroll_x: f32,
        theme: &CompositorTheme,
    ) {
        for guide in &doc.indent_guides {
            let y_start_line =
                ((guide.y_start / line_height) as u32).max(first_visible_line);
            let y_end_line =
                ((guide.y_end / line_height) as u32).min(last_visible_line);
            if y_start_line >= y_end_line {
                continue;
            }
            let x = content_rect.x + guide.x - scroll_x;
            let y0 = content_rect.y
                + (y_start_line - first_visible_line) as f32 * line_height;
            let y1 = content_rect.y
                + (y_end_line - first_visible_line) as f32 * line_height;
            let color = if guide.active {
                theme.editor_indent_guide_active_color
            } else {
                theme.editor_indent_guide_color
            };
            rects.draw_rect(x, y0, 1.0, y1 - y0, color, 0.0);
        }
    }

    /// Renders bracket pair colorization highlights.
    #[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
    fn render_bracket_pairs_static(
        rects: &mut RectRenderer,
        doc: &CompositorDocumentSnapshot,
        first_visible_line: u32,
        last_visible_line: u32,
        content_rect: &Rect,
        line_height: f32,
        char_width: f32,
        scroll_x: f32,
        theme: &CompositorTheme,
    ) {
        let colors = &theme.editor_bracket_pair_colors;
        for pair in &doc.bracket_pairs {
            let color = colors[(pair.nesting_level as usize) % colors.len()];

            // Open bracket
            if pair.open_line >= first_visible_line && pair.open_line < last_visible_line {
                let x = content_rect.x + pair.open_col as f32 * char_width - scroll_x;
                let y = content_rect.y
                    + (pair.open_line - first_visible_line) as f32 * line_height;
                rects.draw_border(x, y, char_width, line_height, color, 1.0);
            }
            // Close bracket
            if pair.close_line >= first_visible_line && pair.close_line < last_visible_line {
                let x = content_rect.x + pair.close_col as f32 * char_width - scroll_x;
                let y = content_rect.y
                    + (pair.close_line - first_visible_line) as f32 * line_height;
                rects.draw_border(x, y, char_width, line_height, color, 1.0);
            }

            // Vertical guide between brackets if they span multiple lines
            if pair.close_line > pair.open_line + 1 {
                let guide_start =
                    (pair.open_line + 1).max(first_visible_line);
                let guide_end = pair.close_line.min(last_visible_line);
                if guide_start < guide_end {
                    let x = content_rect.x
                        + pair.open_col as f32 * char_width
                        + char_width * 0.5
                        - scroll_x;
                    let y0 = content_rect.y
                        + (guide_start - first_visible_line) as f32 * line_height;
                    let y1 = content_rect.y
                        + (guide_end - first_visible_line) as f32 * line_height;
                    let guide_color = Color { a: color.a * 0.3, ..color };
                    rects.draw_rect(x, y0, 1.0, y1 - y0, guide_color, 0.0);
                }
            }
        }
    }

    /// Renders inlay hints for visible lines.
    #[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
    fn render_inlay_hints_static(
        line_renderer: &LineRenderer,
        text: &mut TextRenderer,
        rects: &mut RectRenderer,
        ctx: &mut TextDrawContext<'_>,
        doc: &CompositorDocumentSnapshot,
        first_visible_line: u32,
        last_visible_line: u32,
        line_height: f32,
        char_width: f32,
    ) {
        for line_idx in first_visible_line..last_visible_line {
            if let Some(hints) = doc.inlay_hints.get(line_idx as usize) {
                if !hints.is_empty() {
                    let y = (line_idx - first_visible_line) as f32 * line_height;
                    line_renderer.render_inlay_hints(text, rects, ctx, hints, y, char_width);
                }
            }
        }
    }

    /// Renders sticky scroll headers pinned at the top of the editor.
    #[allow(clippy::too_many_arguments)]
    fn render_sticky_scroll_static(
        line_renderer: &mut LineRenderer,
        text: &mut TextRenderer,
        rects: &mut RectRenderer,
        ctx: &mut TextDrawContext<'_>,
        sticky_lines: &[StyledLine],
        content_width: f32,
        line_height: f32,
        theme: &CompositorTheme,
    ) {
        let total_height = sticky_lines.len() as f32 * line_height;

        // Background
        rects.draw_rect(0.0, 0.0, content_width, total_height, theme.editor_sticky_scroll_background, 0.0);

        // Render each sticky line
        let mut y = 0.0_f32;
        for line in sticky_lines {
            let spans: Vec<(&str, Color)> = line
                .spans
                .iter()
                .map(|s| (s.text.as_str(), s.style.color))
                .collect();
            text.draw_styled_line(&spans, 0.0, y, line_renderer.config_mut().font_size, ctx);
            y += line_height;
        }

        // Bottom border
        rects.draw_rect(
            0.0, total_height - 1.0,
            content_width, 1.0,
            theme.editor_sticky_scroll_border, 0.0,
        );
    }

    /// Builds a z-ordered [`Scene`] for the current frame.
    ///
    /// This is an alternative rendering path that uses the scene graph
    /// for proper z-ordered, batched dispatch via [`GpuRenderer::render_scene`].
    #[allow(clippy::cast_precision_loss)]
    pub fn build_scene(
        &mut self,
        editor_rect: Rect,
        doc: &CompositorDocumentSnapshot,
        scroll: &ScrollState,
        theme: &CompositorTheme,
        config: &CompositorConfig,
    ) -> &Scene {
        self.scene.clear();

        let layout = self.compute_layout(editor_rect, doc.total_lines, config);
        let line_height = config.line_height;
        let first_visible_line = (scroll.scroll_y / line_height as f64) as u32;
        let visible_line_count = (editor_rect.height / line_height) as u32 + 2;
        let last_visible_line =
            (first_visible_line + visible_line_count).min(doc.total_lines);

        // Layer 0: Background
        self.scene.push_layer(Layer::Background);
        self.scene.insert_rect(
            editor_rect.x, editor_rect.y,
            editor_rect.width, editor_rect.height,
            theme.editor_background, 0.0,
        );
        self.scene.insert_rect(
            layout.gutter.x, layout.gutter.y,
            layout.gutter.width, layout.gutter.height,
            theme.editor_gutter_background, 0.0,
        );
        self.scene.pop_layer();

        // Layer 1: Current line highlight
        self.scene.push_layer(Layer::LineHighlights);
        if doc.cursor_line >= first_visible_line && doc.cursor_line < last_visible_line {
            let y = layout.content.y
                + (doc.cursor_line - first_visible_line) as f32 * line_height;
            self.scene.insert_rect(
                layout.content.x, y,
                layout.content.width, line_height,
                theme.editor_line_highlight_background, 0.0,
            );
        }
        self.scene.pop_layer();

        // Layer 2: Selections
        self.scene.push_layer(Layer::Selections);
        {
            let sel_cfg = self.selection_renderer.config_mut();
            for sel in &doc.selections {
                let radius = if sel.is_first && sel.is_last {
                    sel_cfg.selection_corner_radius
                } else if sel.is_first || sel.is_last {
                    sel_cfg.selection_corner_radius * 0.5
                } else {
                    0.0
                };
                self.scene.insert_rect(
                    sel.x, sel.y, sel.width, sel.height,
                    sel_cfg.selection_color, radius,
                );
            }
        }
        self.scene.pop_layer();

        // Layer 7: Cursors
        self.scene.push_layer(Layer::Cursors);
        self.scene.pop_layer();

        // Layer 9: Gutter
        self.scene.push_layer(Layer::Gutter);
        self.scene.pop_layer();

        // Layer 12: Scroll shadow
        self.scene.push_layer(Layer::ScrollShadow);
        if scroll.scroll_y > 0.0 {
            let alpha = (scroll.scroll_y / 100.0).min(1.0) as f32 * 0.3;
            let shadow_color = Color { r: 0.0, g: 0.0, b: 0.0, a: alpha };
            self.scene.insert_rect(
                layout.content.x, layout.content.y,
                layout.content.width, 6.0,
                shadow_color, 0.0,
            );
        }
        self.scene.pop_layer();

        self.scene.finish();
        &self.scene
    }

    /// Flushes all batched geometry into the given render pass.
    pub fn flush(
        &mut self,
        frame: &mut crate::renderer::FrameContext,
        clear_color: Color,
    ) {
        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("editor_compositor_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(clear_color.r),
                            g: f64::from(clear_color.g),
                            b: f64::from(clear_color.b),
                            a: f64::from(clear_color.a),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

        self.rect_renderer.flush(&self.device, &self.queue, &mut pass);
        self.text_renderer.flush(&self.device, &self.queue, &mut pass);
    }
}
