//! Full line renderer for the editor.
//!
//! Provides [`StyledSpan`], [`TextStyle`], and [`StyledLine`] for representing
//! syntax-highlighted source lines, along with rendering of whitespace
//! indicators, indent guides, sticky scroll headers, word-wrap arrows,
//! code lens text, and inlay hints.

use crate::color::Color;
use crate::rect_renderer::RectRenderer;
use crate::text_renderer::{TextDrawContext, TextRenderer};

// ---------------------------------------------------------------------------
// Core styled-text types
// ---------------------------------------------------------------------------

/// Visual style for a span of text.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct TextStyle {
    pub color: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
        }
    }
}

/// A span of text with a visual style.
#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub style: TextStyle,
}

/// A complete editor line composed of styled spans.
#[derive(Debug, Clone)]
pub struct StyledLine {
    pub spans: Vec<StyledSpan>,
}

impl StyledLine {
    /// Returns the total character count of the line.
    pub fn char_count(&self) -> usize {
        self.spans.iter().map(|s| s.text.chars().count()).sum()
    }

    /// Returns the concatenated plain text.
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for span in &self.spans {
            out.push_str(&span.text);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Viewport
// ---------------------------------------------------------------------------

/// Describes the visible viewport for line rendering.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    /// First visible line (0-based).
    pub first_line: u32,
    /// Number of visible lines.
    pub visible_lines: u32,
    /// Horizontal scroll offset in pixels.
    pub scroll_x: f32,
    /// Vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// Width of the viewport in pixels.
    pub width: f32,
    /// Height of the viewport in pixels.
    pub height: f32,
}

// ---------------------------------------------------------------------------
// Whitespace rendering mode
// ---------------------------------------------------------------------------

/// Controls when whitespace characters are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhitespaceRender {
    #[default]
    None,
    Boundary,
    Selection,
    All,
}

// ---------------------------------------------------------------------------
// Auxiliary decorations
// ---------------------------------------------------------------------------

/// An indent guide (vertical line at a tab stop).
#[derive(Debug, Clone, Copy)]
pub struct IndentGuide {
    pub x: f32,
    pub y_start: f32,
    pub y_end: f32,
    pub active: bool,
}

/// A sticky scroll header pinned at the top of the editor.
#[derive(Debug, Clone)]
pub struct StickyHeader {
    pub line: StyledLine,
    pub indent_level: u32,
}

/// A code lens annotation displayed above a line.
#[derive(Debug, Clone)]
pub struct CodeLens {
    pub text: String,
    pub line: u32,
}

/// An inlay hint displayed inline within a line.
#[derive(Debug, Clone)]
pub struct InlayHint {
    /// Column position (character offset) where the hint appears.
    pub column: u32,
    /// The hint text to display.
    pub text: String,
}

/// Word-wrap continuation indicator for a wrapped line.
#[derive(Debug, Clone, Copy)]
pub struct WrapIndicator {
    pub y: f32,
    pub x: f32,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the line renderer.
#[derive(Debug, Clone)]
pub struct LineRenderConfig {
    pub font_size: f32,
    pub line_height: f32,
    pub tab_size: u32,
    pub whitespace_render: WhitespaceRender,
    pub indent_guide_color: Color,
    pub indent_guide_active_color: Color,
    pub indent_guide_width: f32,
    pub whitespace_color: Color,
    pub code_lens_color: Color,
    pub code_lens_font_size: f32,
    pub inlay_hint_color: Color,
    pub inlay_hint_bg_color: Color,
    pub inlay_hint_font_size: f32,
    pub wrap_indicator_color: Color,
    pub sticky_header_bg: Color,
    pub underline_thickness: f32,
    pub strikethrough_thickness: f32,
}

impl Default for LineRenderConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            line_height: 20.0,
            tab_size: 4,
            whitespace_render: WhitespaceRender::None,
            indent_guide_color: Color {
                r: 0.3,
                g: 0.3,
                b: 0.3,
                a: 0.3,
            },
            indent_guide_active_color: Color {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 0.5,
            },
            indent_guide_width: 1.0,
            whitespace_color: Color {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 0.4,
            },
            code_lens_color: Color::from_rgb(160, 160, 160),
            code_lens_font_size: 11.0,
            inlay_hint_color: Color::from_rgb(140, 140, 140),
            inlay_hint_bg_color: Color {
                r: 0.2,
                g: 0.2,
                b: 0.2,
                a: 0.5,
            },
            inlay_hint_font_size: 12.0,
            wrap_indicator_color: Color::from_rgb(100, 100, 100),
            sticky_header_bg: Color {
                r: 0.14,
                g: 0.14,
                b: 0.14,
                a: 1.0,
            },
            underline_thickness: 1.0,
            strikethrough_thickness: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// LineRenderer
// ---------------------------------------------------------------------------

/// Renders fully styled editor lines, including decorations.
pub struct LineRenderer {
    config: LineRenderConfig,
}

impl LineRenderer {
    pub fn new(config: LineRenderConfig) -> Self {
        Self { config }
    }

    pub fn config_mut(&mut self) -> &mut LineRenderConfig {
        &mut self.config
    }

    /// Renders a single styled line at the given Y coordinate.
    ///
    /// The line is clipped to the viewport horizontally.
    #[allow(clippy::cast_precision_loss)]
    pub fn render_line(
        &self,
        text_renderer: &mut TextRenderer,
        rect_renderer: &mut RectRenderer,
        ctx: &mut TextDrawContext<'_>,
        line: &StyledLine,
        y: f32,
        viewport: &Viewport,
    ) {
        let cfg = &self.config;
        let x_offset = -viewport.scroll_x;

        // Build the (text, Color) spans for the text renderer.
        let spans: Vec<(&str, Color)> = line
            .spans
            .iter()
            .map(|s| (s.text.as_str(), s.style.color))
            .collect();

        text_renderer.draw_styled_line(&spans, x_offset, y, cfg.font_size, ctx);

        // Draw underline / strikethrough decorations.
        let mut cursor_x = x_offset;
        for span in &line.spans {
            let span_w = span.text.chars().count() as f32 * cfg.font_size * 0.6;
            if span.style.underline {
                let uy = y + cfg.line_height - 2.0;
                rect_renderer.draw_rect(
                    cursor_x,
                    uy,
                    span_w,
                    cfg.underline_thickness,
                    span.style.color,
                    0.0,
                );
            }
            if span.style.strikethrough {
                let sy = y + cfg.line_height * 0.5;
                rect_renderer.draw_rect(
                    cursor_x,
                    sy,
                    span_w,
                    cfg.strikethrough_thickness,
                    span.style.color,
                    0.0,
                );
            }
            cursor_x += span_w;
        }
    }

    /// Renders whitespace indicators (dots for spaces, arrows for tabs).
    #[allow(clippy::cast_precision_loss)]
    pub fn render_whitespace(
        &self,
        rect_renderer: &mut RectRenderer,
        line_text: &str,
        x_offset: f32,
        y: f32,
        char_width: f32,
    ) {
        if self.config.whitespace_render == WhitespaceRender::None {
            return;
        }
        let cfg = &self.config;
        let dot_size = 2.0_f32;
        let mid_y = y + cfg.line_height * 0.5;

        let mut cx = x_offset;
        for ch in line_text.chars() {
            match ch {
                ' ' => {
                    rect_renderer.draw_rect(
                        cx + char_width * 0.5 - dot_size * 0.5,
                        mid_y - dot_size * 0.5,
                        dot_size,
                        dot_size,
                        cfg.whitespace_color,
                        dot_size * 0.5,
                    );
                }
                '\t' => {
                    let tab_w = char_width * cfg.tab_size as f32;
                    let arrow_h = 1.5;
                    rect_renderer.draw_rect(
                        cx + 2.0,
                        mid_y - arrow_h * 0.5,
                        tab_w - 4.0,
                        arrow_h,
                        cfg.whitespace_color,
                        0.0,
                    );
                    // Arrow head
                    let head_size = 3.0;
                    rect_renderer.draw_rect(
                        cx + tab_w - 4.0 - head_size,
                        mid_y - head_size,
                        head_size,
                        head_size * 2.0,
                        cfg.whitespace_color,
                        0.0,
                    );
                }
                _ => {}
            }
            cx += if ch == '\t' {
                char_width * cfg.tab_size as f32
            } else {
                char_width
            };
        }
    }

    /// Renders vertical indent guides.
    pub fn render_indent_guides(&self, rect_renderer: &mut RectRenderer, guides: &[IndentGuide]) {
        let cfg = &self.config;
        for guide in guides {
            let color = if guide.active {
                cfg.indent_guide_active_color
            } else {
                cfg.indent_guide_color
            };
            let h = guide.y_end - guide.y_start;
            rect_renderer.draw_rect(
                guide.x,
                guide.y_start,
                cfg.indent_guide_width,
                h,
                color,
                0.0,
            );
        }
    }

    /// Renders sticky scroll headers pinned at the top.
    pub fn render_sticky_headers(
        &self,
        text_renderer: &mut TextRenderer,
        rect_renderer: &mut RectRenderer,
        ctx: &mut TextDrawContext<'_>,
        headers: &[StickyHeader],
        editor_width: f32,
    ) {
        let cfg = &self.config;
        let mut y = 0.0_f32;
        for header in headers {
            rect_renderer.draw_rect(
                0.0,
                y,
                editor_width,
                cfg.line_height,
                cfg.sticky_header_bg,
                0.0,
            );
            let spans: Vec<(&str, Color)> = header
                .line
                .spans
                .iter()
                .map(|s| (s.text.as_str(), s.style.color))
                .collect();
            text_renderer.draw_styled_line(&spans, 0.0, y, cfg.font_size, ctx);
            y += cfg.line_height;
        }
    }

    /// Renders code lens annotations above a line.
    #[allow(clippy::cast_precision_loss)]
    pub fn render_code_lens(
        &self,
        text_renderer: &mut TextRenderer,
        ctx: &mut TextDrawContext<'_>,
        lenses: &[CodeLens],
        line_y_offset: impl Fn(u32) -> f32,
    ) {
        let cfg = &self.config;
        for lens in lenses {
            let y = line_y_offset(lens.line) - cfg.line_height;
            text_renderer.draw_line(
                &lens.text,
                0.0,
                y,
                cfg.code_lens_color,
                cfg.code_lens_font_size,
                ctx,
            );
        }
    }

    /// Renders inlay hints inline within a line.
    #[allow(clippy::cast_precision_loss)]
    pub fn render_inlay_hints(
        &self,
        text_renderer: &mut TextRenderer,
        rect_renderer: &mut RectRenderer,
        ctx: &mut TextDrawContext<'_>,
        hints: &[InlayHint],
        y: f32,
        char_width: f32,
    ) {
        let cfg = &self.config;
        for hint in hints {
            let hx = hint.column as f32 * char_width;
            let hw = hint.text.len() as f32 * cfg.inlay_hint_font_size * 0.55 + 6.0;
            rect_renderer.draw_rect(
                hx,
                y + 1.0,
                hw,
                cfg.line_height - 2.0,
                cfg.inlay_hint_bg_color,
                3.0,
            );
            text_renderer.draw_line(
                &hint.text,
                hx + 3.0,
                y,
                cfg.inlay_hint_color,
                cfg.inlay_hint_font_size,
                ctx,
            );
        }
    }

    /// Renders word-wrap continuation arrows.
    pub fn render_wrap_indicators(
        &self,
        rect_renderer: &mut RectRenderer,
        indicators: &[WrapIndicator],
    ) {
        let cfg = &self.config;
        let arrow_w = 8.0;
        let arrow_h = 2.0;
        let mid_h = cfg.line_height * 0.5;
        for ind in indicators {
            rect_renderer.draw_rect(
                ind.x,
                ind.y + mid_h - arrow_h * 0.5,
                arrow_w,
                arrow_h,
                cfg.wrap_indicator_color,
                0.0,
            );
        }
    }
}

impl Default for LineRenderer {
    fn default() -> Self {
        Self::new(LineRenderConfig::default())
    }
}
