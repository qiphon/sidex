//! Minimap renderer — a scaled-down overview of the entire document.
//!
//! Each source line is rendered as a 1–2 pixel tall strip, with characters
//! mapped to 1–2 pixel wide blocks. Decorations such as the viewport slider,
//! selection ranges, search matches, git changes, and diagnostics are overlaid.

use crate::color::Color;
use crate::rect_renderer::RectRenderer;

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// A single styled span within a line, used as input to the minimap.
#[derive(Debug, Clone)]
pub struct StyledLine {
    /// Spans that make up this line. Each span has a byte length and a color.
    pub spans: Vec<MinimapSpan>,
}

/// A span within a [`StyledLine`].
#[derive(Debug, Clone, Copy)]
pub struct MinimapSpan {
    /// Number of characters this span covers.
    pub char_count: u32,
    /// Display color for this span.
    pub color: Color,
}

/// A range expressed as line indices (start inclusive, end exclusive).
#[derive(Debug, Clone, Copy)]
pub struct LineRange {
    pub start_line: u32,
    pub end_line: u32,
}

/// Viewport region the minimap reflects.
#[derive(Debug, Clone, Copy)]
pub struct MinimapViewport {
    /// First visible line in the editor.
    pub first_visible_line: u32,
    /// Number of visible lines in the editor.
    pub visible_line_count: u32,
    /// Total line count in the document.
    pub total_lines: u32,
}

/// The kind of diagnostic mark rendered in the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A diagnostic mark that should appear in the minimap.
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticMark {
    pub line: u32,
    pub severity: DiagnosticSeverity,
}

/// Describes a git change decoration on a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitChangeKind {
    Added,
    Modified,
    Deleted,
}

/// A git change indicator for the minimap.
#[derive(Debug, Clone, Copy)]
pub struct GitChange {
    pub line: u32,
    pub kind: GitChangeKind,
}

/// Result of a minimap click hit-test.
#[derive(Debug, Clone, Copy)]
pub struct MinimapClickResult {
    /// The document line that was clicked on.
    pub target_line: u32,
}

// ---------------------------------------------------------------------------
// Minimap configuration
// ---------------------------------------------------------------------------

/// Configuration for the minimap renderer.
#[derive(Debug, Clone)]
pub struct MinimapConfig {
    /// Width of the minimap in pixels.
    pub width: f32,
    /// Height of each minimap line in pixels.
    pub line_height: f32,
    /// Width of each character block in pixels.
    pub char_width: f32,
    /// Maximum number of character columns to render.
    pub max_columns: u32,
    /// Color used for the viewport slider overlay.
    pub slider_color: Color,
    /// Color for selection range highlights.
    pub selection_color: Color,
    /// Color for search match highlights.
    pub search_match_color: Color,
}

impl Default for MinimapConfig {
    fn default() -> Self {
        Self {
            width: 60.0,
            line_height: 2.0,
            char_width: 1.4,
            max_columns: 120,
            slider_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.15,
            },
            selection_color: Color {
                r: 0.2,
                g: 0.5,
                b: 1.0,
                a: 0.5,
            },
            search_match_color: Color {
                r: 0.9,
                g: 0.8,
                b: 0.2,
                a: 0.7,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// MinimapRenderer
// ---------------------------------------------------------------------------

/// Renders a scaled-down overview of the document.
///
/// The minimap is drawn entirely with the [`RectRenderer`]; each character
/// block is a tiny filled rectangle. Overlays (viewport slider, selections,
/// diagnostics, git changes, search matches) are drawn on top.
pub struct MinimapRenderer {
    config: MinimapConfig,
    /// X origin of the minimap within the editor surface.
    origin_x: f32,
    /// Y origin of the minimap.
    origin_y: f32,
    /// Total rendered height (used for click hit-testing).
    rendered_height: f32,
    /// Total lines last rendered (for hit-testing).
    rendered_total_lines: u32,
}

impl MinimapRenderer {
    /// Creates a new minimap renderer.
    pub fn new(config: MinimapConfig) -> Self {
        Self {
            config,
            origin_x: 0.0,
            origin_y: 0.0,
            rendered_height: 0.0,
            rendered_total_lines: 0,
        }
    }

    /// Sets the screen-space origin where the minimap will be drawn.
    pub fn set_origin(&mut self, x: f32, y: f32) {
        self.origin_x = x;
        self.origin_y = y;
    }

    /// Returns a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut MinimapConfig {
        &mut self.config
    }

    /// Renders the complete minimap into the given [`RectRenderer`].
    ///
    /// The minimap is positioned at the origin set by [`set_origin`](Self::set_origin).
    #[allow(
        clippy::cast_precision_loss,
        clippy::too_many_arguments,
        clippy::cast_possible_truncation
    )]
    pub fn render(
        &mut self,
        rects: &mut RectRenderer,
        buffer_lines: &[StyledLine],
        viewport: &MinimapViewport,
        selections: &[LineRange],
        search_matches: &[LineRange],
        diagnostics: &[DiagnosticMark],
        git_changes: &[GitChange],
    ) {
        let cfg = &self.config;
        let total_lines = buffer_lines.len() as u32;
        self.rendered_total_lines = total_lines;
        self.rendered_height = total_lines as f32 * cfg.line_height;

        let x0 = self.origin_x;
        let y0 = self.origin_y;

        // -- Character blocks for each line ----------------------------------
        for (line_idx, styled_line) in buffer_lines.iter().enumerate() {
            let ly = y0 + line_idx as f32 * cfg.line_height;
            let mut cx = x0;
            for span in &styled_line.spans {
                let span_w = span.char_count.min(cfg.max_columns) as f32 * cfg.char_width;
                rects.draw_rect(cx, ly, span_w, cfg.line_height, span.color, 0.0);
                cx += span_w;
            }
        }

        // -- Selection highlights --------------------------------------------
        for sel in selections {
            let sy = y0 + sel.start_line as f32 * cfg.line_height;
            let sh = (sel.end_line - sel.start_line).max(1) as f32 * cfg.line_height;
            rects.draw_rect(x0, sy, cfg.width, sh, cfg.selection_color, 0.0);
        }

        // -- Search match highlights -----------------------------------------
        for m in search_matches {
            let sy = y0 + m.start_line as f32 * cfg.line_height;
            let sh = (m.end_line - m.start_line).max(1) as f32 * cfg.line_height;
            rects.draw_rect(x0, sy, cfg.width, sh, cfg.search_match_color, 0.0);
        }

        // -- Git change indicators (narrow bar on the left) ------------------
        for gc in git_changes {
            let gy = y0 + gc.line as f32 * cfg.line_height;
            let color = match gc.kind {
                GitChangeKind::Added => Color::from_rgb(80, 200, 80),
                GitChangeKind::Modified => Color::from_rgb(80, 140, 220),
                GitChangeKind::Deleted => Color::from_rgb(220, 80, 80),
            };
            rects.draw_rect(x0, gy, 3.0, cfg.line_height, color, 0.0);
        }

        // -- Diagnostic marks (narrow bar on the right) ----------------------
        for diag in diagnostics {
            let dy = y0 + diag.line as f32 * cfg.line_height;
            let color = match diag.severity {
                DiagnosticSeverity::Error => Color::from_rgb(230, 60, 60),
                DiagnosticSeverity::Warning => Color::from_rgb(220, 180, 40),
                DiagnosticSeverity::Info => Color::from_rgb(80, 160, 230),
                DiagnosticSeverity::Hint => Color::from_rgb(150, 150, 150),
            };
            rects.draw_rect(x0 + cfg.width - 3.0, dy, 3.0, cfg.line_height, color, 0.0);
        }

        // -- Viewport slider -------------------------------------------------
        let slider_y = y0 + viewport.first_visible_line as f32 * cfg.line_height;
        let slider_h = viewport.visible_line_count as f32 * cfg.line_height;
        rects.draw_rect(x0, slider_y, cfg.width, slider_h, cfg.slider_color, 0.0);
    }

    /// Hit-tests a click at screen coordinates `(mx, my)` and returns the
    /// target document line, if the click falls within the minimap bounds.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn hit_test(&self, mx: f32, my: f32) -> Option<MinimapClickResult> {
        let cfg = &self.config;
        if mx < self.origin_x || mx > self.origin_x + cfg.width {
            return None;
        }
        if my < self.origin_y || my > self.origin_y + self.rendered_height {
            return None;
        }
        let relative_y = my - self.origin_y;
        let line = (relative_y / cfg.line_height) as u32;
        let target_line = line.min(self.rendered_total_lines.saturating_sub(1));
        Some(MinimapClickResult { target_line })
    }
}
