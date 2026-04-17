//! Per-line gutter rendering — breakpoints, git decorations, fold chevrons,
//! and line numbers, emitted as [`GutterPrimitive`] instances that map
//! directly to [`RenderCommand`]s.

use crate::color::Color;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Visual kind of a breakpoint marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointKind {
    /// Active breakpoint — solid red circle.
    Active,
    /// Disabled breakpoint — gray circle.
    Disabled,
    /// Conditional breakpoint — diamond shape.
    Conditional,
    /// Log-point — diamond with inner dot.
    Logpoint,
}

/// Whether a foldable region is expanded or collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineFoldState {
    Expanded,
    Collapsed,
    None,
}

/// Kind of source-control modification on a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitLineChange {
    Added,
    Modified,
    Deleted,
    None,
}

/// Identifies an icon shape drawn in the gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Circle,
    Diamond,
    DotInDiamond,
    ChevronDown,
    ChevronRight,
}

// ---------------------------------------------------------------------------
// Input for a single gutter line
// ---------------------------------------------------------------------------

/// Everything needed to render one line's gutter area.
#[derive(Debug, Clone)]
pub struct GutterLineInput {
    /// 1-based line number.
    pub line_number: u32,
    /// Breakpoint on this line, if any.
    pub breakpoint: Option<BreakpointKind>,
    /// Fold state for this line.
    pub fold_state: LineFoldState,
    /// Git change marker.
    pub git_change: GitLineChange,
    /// Whether the debugger is currently paused on this line.
    pub debug_line: bool,
}

// ---------------------------------------------------------------------------
// Margin widths
// ---------------------------------------------------------------------------

/// Column widths within the gutter.
#[derive(Debug, Clone, Copy)]
pub struct GutterMargins {
    pub breakpoint_width: f32,
    pub line_number_width: f32,
    pub fold_width: f32,
    pub git_bar_width: f32,
    pub padding: f32,
}

impl Default for GutterMargins {
    fn default() -> Self {
        Self {
            breakpoint_width: 16.0,
            line_number_width: 40.0,
            fold_width: 14.0,
            git_bar_width: 3.0,
            padding: 4.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Output primitives
// ---------------------------------------------------------------------------

/// A primitive produced by [`render_gutter_line`].
#[derive(Debug, Clone)]
pub enum GutterPrimitive {
    Rect { x: f32, y: f32, w: f32, h: f32, color: Color, corner_radius: f32 },
    Text { x: f32, y: f32, content: String, color: Color, font_size: f32, bold: bool },
    Icon { x: f32, y: f32, size: f32, kind: IconKind, color: Color },
}

// ---------------------------------------------------------------------------
// Theme colors
// ---------------------------------------------------------------------------

/// Colors used when rendering the gutter.
#[derive(Debug, Clone)]
pub struct GutterTheme {
    pub background: Color,
    pub line_number: Color,
    pub line_number_active: Color,
    pub breakpoint_active: Color,
    pub breakpoint_disabled: Color,
    pub breakpoint_conditional: Color,
    pub breakpoint_logpoint: Color,
    pub git_added: Color,
    pub git_modified: Color,
    pub git_deleted: Color,
    pub fold_icon: Color,
    pub debug_line_bg: Color,
}

impl Default for GutterTheme {
    fn default() -> Self {
        Self {
            background: Color { r: 0.12, g: 0.12, b: 0.12, a: 1.0 },
            line_number: Color::from_rgb(130, 130, 130),
            line_number_active: Color::from_rgb(220, 220, 220),
            breakpoint_active: Color::from_rgb(220, 50, 50),
            breakpoint_disabled: Color::from_rgb(140, 140, 140),
            breakpoint_conditional: Color::from_rgb(220, 160, 40),
            breakpoint_logpoint: Color::from_rgb(220, 120, 40),
            git_added: Color::from_rgb(80, 200, 80),
            git_modified: Color::from_rgb(80, 140, 220),
            git_deleted: Color::from_rgb(220, 80, 80),
            fold_icon: Color::from_rgb(160, 160, 160),
            debug_line_bg: Color { r: 0.30, g: 0.25, b: 0.10, a: 1.0 },
        }
    }
}

// ---------------------------------------------------------------------------
// GutterRenderer
// ---------------------------------------------------------------------------

/// Stateful renderer that converts [`GutterLineInput`]s into draw primitives.
pub struct GutterRenderer {
    pub margins: GutterMargins,
    pub theme: GutterTheme,
    pub font_size: f32,
}

impl GutterRenderer {
    pub fn new(margins: GutterMargins, theme: GutterTheme, font_size: f32) -> Self {
        Self { margins, theme, font_size }
    }

    /// Total pixel width of the gutter.
    pub fn total_width(&self) -> f32 {
        let m = &self.margins;
        m.padding + m.breakpoint_width + m.line_number_width + m.fold_width + m.git_bar_width
    }
}

impl Default for GutterRenderer {
    fn default() -> Self {
        Self::new(GutterMargins::default(), GutterTheme::default(), 13.0)
    }
}

// ---------------------------------------------------------------------------
// Core render function
// ---------------------------------------------------------------------------

/// Renders a single gutter line, returning draw primitives.
///
/// * `renderer`    — gutter configuration / theme
/// * `input`       — per-line data (line number, breakpoint, etc.)
/// * `y`           — vertical pixel offset for this line
/// * `line_height` — pixel height of one editor line
/// * `is_current`  — whether this is the line with the primary cursor
#[allow(clippy::cast_precision_loss)]
pub fn render_gutter_line(
    renderer: &GutterRenderer,
    input: &GutterLineInput,
    y: f32,
    line_height: f32,
    is_current: bool,
) -> Vec<GutterPrimitive> {
    let mut out = Vec::with_capacity(6);
    let m = &renderer.margins;
    let t = &renderer.theme;
    let mut x_cursor = m.padding;

    // Debug line highlight
    if input.debug_line {
        out.push(GutterPrimitive::Rect {
            x: 0.0, y, w: renderer.total_width(), h: line_height,
            color: t.debug_line_bg, corner_radius: 0.0,
        });
    }

    // Breakpoint column
    if let Some(kind) = &input.breakpoint {
        let bp_size = (line_height * 0.55).min(12.0);
        let cx = x_cursor + m.breakpoint_width * 0.5 - bp_size * 0.5;
        let cy = y + line_height * 0.5 - bp_size * 0.5;
        let (icon, color) = match kind {
            BreakpointKind::Active => (IconKind::Circle, t.breakpoint_active),
            BreakpointKind::Disabled => (IconKind::Circle, t.breakpoint_disabled),
            BreakpointKind::Conditional => (IconKind::Diamond, t.breakpoint_conditional),
            BreakpointKind::Logpoint => (IconKind::DotInDiamond, t.breakpoint_logpoint),
        };
        out.push(GutterPrimitive::Icon { x: cx, y: cy, size: bp_size, kind: icon, color });
    }
    x_cursor += m.breakpoint_width;

    // Line number — bold for current line
    let (color, bold) = if is_current {
        (t.line_number_active, true)
    } else {
        (t.line_number, false)
    };
    out.push(GutterPrimitive::Text {
        x: x_cursor,
        y: y + (line_height - renderer.font_size) * 0.5,
        content: input.line_number.to_string(),
        color, font_size: renderer.font_size, bold,
    });
    x_cursor += m.line_number_width;

    // Fold indicator — chevron icons
    let fold_icon = match input.fold_state {
        LineFoldState::Expanded => Some(IconKind::ChevronDown),
        LineFoldState::Collapsed => Some(IconKind::ChevronRight),
        LineFoldState::None => None,
    };
    if let Some(kind) = fold_icon {
        let sz = line_height.min(14.0) * 0.6;
        out.push(GutterPrimitive::Icon {
            x: x_cursor + (m.fold_width - sz) * 0.5,
            y: y + (line_height - sz) * 0.5,
            size: sz, kind, color: t.fold_icon,
        });
    }
    x_cursor += m.fold_width;

    // Git change bar
    match input.git_change {
        GitLineChange::Added | GitLineChange::Modified => {
            let color = if input.git_change == GitLineChange::Added {
                t.git_added
            } else {
                t.git_modified
            };
            out.push(GutterPrimitive::Rect {
                x: x_cursor, y, w: m.git_bar_width, h: line_height,
                color, corner_radius: 0.0,
            });
        }
        GitLineChange::Deleted => {
            let tri_h = 6.0_f32.min(line_height * 0.4);
            out.push(GutterPrimitive::Rect {
                x: x_cursor, y: y + line_height - tri_h,
                w: m.git_bar_width + 2.0, h: tri_h,
                color: t.git_deleted, corner_radius: 0.0,
            });
        }
        GitLineChange::None => {}
    }

    out
}
