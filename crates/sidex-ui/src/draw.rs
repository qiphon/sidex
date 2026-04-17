//! Drawing context that bridges widgets to `sidex-gpu` rendering primitives.
//!
//! [`DrawContext`] collects high-level draw commands (rects, borders, text,
//! icons, images) and provides a coordinate-space stack so widgets can render
//! without caring about their absolute screen position.

use sidex_gpu::color::Color;
use sidex_gpu::rect_renderer::RectRenderer;
use sidex_gpu::text_renderer::{TextDrawContext, TextRenderer};

use crate::layout::Rect;

// ── Cursor icon ─────────────────────────────────────────────────────────────

/// Mouse cursor icon that widgets can request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorIcon {
    #[default]
    Default,
    Pointer,
    Text,
    Move,
    ResizeEW,
    ResizeNS,
    ResizeNESW,
    ResizeNWSE,
    NotAllowed,
    Grab,
    Grabbing,
}

// ── Icon identifiers ────────────────────────────────────────────────────────

/// Symbolic icon identifier used by widgets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IconId {
    ChevronRight,
    ChevronDown,
    Close,
    Search,
    File,
    Folder,
    FolderOpen,
    Warning,
    Error,
    Info,
    Check,
    CircleFilled,
    ArrowRight,
    Gear,
    Bell,
    GitBranch,
    Remote,
    SymbolMethod,
    SymbolField,
    Pin,
    MoreHorizontal,
}

// ── Text style (for styled text spans) ──────────────────────────────────────

/// Style applied to a span of text in [`DrawContext::draw_styled_text`].
#[derive(Clone, Debug)]
pub struct TextStyle {
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            bold: false,
            italic: false,
        }
    }
}

// ── Saved graphics state ────────────────────────────────────────────────────

struct SavedState {
    offset_x: f32,
    offset_y: f32,
    clip: Option<Rect>,
}

// ── DrawContext ──────────────────────────────────────────────────────────────

/// High-level drawing context that widgets use to render themselves.
///
/// Internally this collects all draw commands into the underlying
/// `RectRenderer` and `TextRenderer` from `sidex-gpu`. A frame coordinator
/// flushes both renderers during the render pass.
pub struct DrawContext<'a> {
    pub rects: &'a mut RectRenderer,
    pub text: &'a mut TextRenderer,
    pub text_ctx: Option<&'a mut TextDrawContext<'a>>,

    offset_x: f32,
    offset_y: f32,
    clip: Option<Rect>,
    state_stack: Vec<SavedState>,

    /// The cursor icon that the most recent widget requested. The frame
    /// coordinator reads this after the render pass to update the window
    /// cursor.
    pub cursor_icon: CursorIcon,
}

impl<'a> DrawContext<'a> {
    /// Creates a new draw context wrapping the given renderers.
    pub fn new(rects: &'a mut RectRenderer, text: &'a mut TextRenderer) -> Self {
        Self {
            rects,
            text,
            text_ctx: None,
            offset_x: 0.0,
            offset_y: 0.0,
            clip: None,
            state_stack: Vec::new(),
            cursor_icon: CursorIcon::Default,
        }
    }

    // ── Coordinate helpers ──────────────────────────────────────────────

    fn abs_x(&self, x: f32) -> f32 {
        x + self.offset_x
    }

    fn abs_y(&self, y: f32) -> f32 {
        y + self.offset_y
    }

    fn abs_rect(&self, r: Rect) -> Rect {
        Rect::new(self.abs_x(r.x), self.abs_y(r.y), r.width, r.height)
    }

    fn is_visible(&self, r: Rect) -> bool {
        if let Some(clip) = self.clip {
            let ar = self.abs_rect(r);
            ar.x < clip.x + clip.width
                && ar.x + ar.width > clip.x
                && ar.y < clip.y + clip.height
                && ar.y + ar.height > clip.y
        } else {
            true
        }
    }

    // ── Primitives ──────────────────────────────────────────────────────

    /// Draws a filled rectangle with optional corner radius.
    pub fn draw_rect(&mut self, rect: Rect, color: Color, corner_radius: f32) {
        if !self.is_visible(rect) {
            return;
        }
        let r = self.abs_rect(rect);
        self.rects
            .draw_rect(r.x, r.y, r.width, r.height, color, corner_radius);
    }

    /// Draws an outlined rectangle border.
    pub fn draw_border(&mut self, rect: Rect, color: Color, thickness: f32, corner_radius: f32) {
        if !self.is_visible(rect) {
            return;
        }
        let r = self.abs_rect(rect);
        if corner_radius > 0.0 {
            // Top
            self.rects
                .draw_rect(r.x, r.y, r.width, thickness, color, 0.0);
            // Bottom
            self.rects.draw_rect(
                r.x,
                r.y + r.height - thickness,
                r.width,
                thickness,
                color,
                0.0,
            );
            // Left
            self.rects.draw_rect(
                r.x,
                r.y + thickness,
                thickness,
                r.height - 2.0 * thickness,
                color,
                0.0,
            );
            // Right
            self.rects.draw_rect(
                r.x + r.width - thickness,
                r.y + thickness,
                thickness,
                r.height - 2.0 * thickness,
                color,
                0.0,
            );
        } else {
            self.rects
                .draw_border(r.x, r.y, r.width, r.height, color, thickness);
        }
    }

    /// Draws a single line of plain text.
    #[allow(clippy::cast_precision_loss)]
    pub fn draw_text(
        &mut self,
        text: &str,
        pos: (f32, f32),
        color: Color,
        font_size: f32,
        _bold: bool,
        _italic: bool,
    ) {
        if text.is_empty() {
            return;
        }
        let ax = self.abs_x(pos.0);
        let ay = self.abs_y(pos.1);

        if let Some(ref mut ctx) = self.text_ctx {
            self.text.draw_line(text, ax, ay, color, font_size, ctx);
        } else {
            // Fallback: approximate text as colored rectangles for each character.
            let char_w = font_size * 0.6;
            let char_h = font_size;
            for (i, _ch) in text.chars().enumerate() {
                let cx = ax + i as f32 * char_w;
                self.rects
                    .draw_rect(cx, ay, char_w - 1.0, char_h, color, 0.0);
            }
        }
    }

    /// Draws styled (syntax-highlighted) text composed of multiple spans.
    #[allow(clippy::cast_precision_loss)]
    pub fn draw_styled_text(
        &mut self,
        spans: &[(String, TextStyle)],
        pos: (f32, f32),
        font_size: f32,
    ) {
        if spans.is_empty() {
            return;
        }
        let ax = self.abs_x(pos.0);
        let ay = self.abs_y(pos.1);

        if let Some(ref mut ctx) = self.text_ctx {
            let color_spans: Vec<(&str, Color)> =
                spans.iter().map(|(t, s)| (t.as_str(), s.color)).collect();
            self.text
                .draw_styled_line(&color_spans, ax, ay, font_size, ctx);
        } else {
            let char_w = font_size * 0.6;
            let mut cursor_x = ax;
            for (text, style) in spans {
                for _ch in text.chars() {
                    self.rects
                        .draw_rect(cursor_x, ay, char_w - 1.0, font_size, style.color, 0.0);
                    cursor_x += char_w;
                }
            }
        }
    }

    /// Draws an icon at the given position. Icons are rendered as simple
    /// geometric shapes when no icon font is available.
    #[allow(clippy::cast_precision_loss)]
    pub fn draw_icon(&mut self, icon: IconId, pos: (f32, f32), size: f32, color: Color) {
        let ax = self.abs_x(pos.0);
        let ay = self.abs_y(pos.1);
        let half = size / 2.0;

        match icon {
            IconId::ChevronRight => {
                // Right-pointing triangle
                let third = size / 3.0;
                self.rects
                    .draw_rect(ax + third, ay + third, third, third, color, 0.0);
            }
            IconId::ChevronDown => {
                let third = size / 3.0;
                self.rects
                    .draw_rect(ax + third, ay + third, third, third, color, 0.0);
            }
            IconId::Close => {
                // X shape approximated as two thin crossed rects
                self.rects
                    .draw_rect(ax + 2.0, ay + half - 0.5, size - 4.0, 1.0, color, 0.0);
                self.rects
                    .draw_rect(ax + half - 0.5, ay + 2.0, 1.0, size - 4.0, color, 0.0);
            }
            IconId::Search => {
                // Circle + handle
                self.rects
                    .draw_rect(ax + 2.0, ay + 2.0, half, half, color, half / 2.0);
                self.rects
                    .draw_rect(ax + half, ay + half, size * 0.3, 2.0, color, 0.0);
            }
            IconId::CircleFilled => {
                self.rects.draw_rect(ax, ay, size, size, color, half);
            }
            IconId::ArrowRight => {
                self.rects
                    .draw_rect(ax, ay + half - 1.0, size, 2.0, color, 0.0);
            }
            _ => {
                // Generic square icon placeholder
                self.rects
                    .draw_rect(ax + 1.0, ay + 1.0, size - 2.0, size - 2.0, color, 2.0);
            }
        }
    }

    /// Draws raw image data into the given rectangle. Currently a no-op
    /// placeholder — image upload to the atlas is not yet wired.
    pub fn draw_image(&mut self, _data: &[u8], rect: Rect) {
        let r = self.abs_rect(rect);
        self.rects.draw_rect(
            r.x,
            r.y,
            r.width,
            r.height,
            Color::from_rgb(80, 80, 80),
            0.0,
        );
    }

    // ── State management ────────────────────────────────────────────────

    /// Sets the clipping rectangle. Subsequent draws outside this rect are
    /// skipped.
    pub fn clip(&mut self, rect: Rect) {
        self.clip = Some(self.abs_rect(rect));
    }

    /// Pushes the current graphics state (offset, clip) onto the stack.
    pub fn save(&mut self) {
        self.state_stack.push(SavedState {
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            clip: self.clip,
        });
    }

    /// Pops the most recently saved graphics state.
    pub fn restore(&mut self) {
        if let Some(s) = self.state_stack.pop() {
            self.offset_x = s.offset_x;
            self.offset_y = s.offset_y;
            self.clip = s.clip;
        }
    }

    /// Translates the coordinate origin by `(dx, dy)`.
    pub fn offset(&mut self, dx: f32, dy: f32) {
        self.offset_x += dx;
        self.offset_y += dy;
    }

    /// Sets the mouse cursor icon.
    pub fn set_cursor(&mut self, cursor: CursorIcon) {
        self.cursor_icon = cursor;
    }
}
