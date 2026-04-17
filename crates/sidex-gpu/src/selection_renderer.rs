//! Selection and highlight rendering.
//!
//! Draws selection backgrounds (with rounded corners), current-line highlights,
//! word occurrence highlights, find-match highlights, and bracket pair boxes.

use crate::color::Color;
use crate::rect_renderer::RectRenderer;

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// A visual selection range expressed in screen coordinates.
#[derive(Debug, Clone, Copy)]
pub struct SelectionRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Whether this rect is the first line of the selection (round top corners).
    pub is_first: bool,
    /// Whether this rect is the last line of the selection (round bottom corners).
    pub is_last: bool,
}

/// A position marking a bracket to highlight.
#[derive(Debug, Clone, Copy)]
pub struct BracketHighlight {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Color of the bracket highlight box.
    pub color: Color,
}

/// A highlight occurrence (e.g. word-under-cursor, find match).
#[derive(Debug, Clone, Copy)]
pub struct HighlightRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Colors and sizes for the selection renderer.
#[derive(Debug, Clone)]
pub struct SelectionRenderConfig {
    /// Background color for selected text.
    pub selection_color: Color,
    /// Corner radius for selection background rects.
    pub selection_corner_radius: f32,
    /// Subtle background color on the active (cursor) line.
    pub current_line_color: Color,
    /// Highlight color for all occurrences of the word under cursor.
    pub word_highlight_color: Color,
    /// Highlight color for find matches (non-current).
    pub find_match_color: Color,
    /// Highlight color for the currently focused find match.
    pub find_current_match_color: Color,
    /// Border thickness for bracket pair highlights.
    pub bracket_border_thickness: f32,
}

impl Default for SelectionRenderConfig {
    fn default() -> Self {
        Self {
            selection_color: Color {
                r: 0.17,
                g: 0.34,
                b: 0.56,
                a: 0.6,
            },
            selection_corner_radius: 3.0,
            current_line_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.04,
            },
            word_highlight_color: Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 0.1,
            },
            find_match_color: Color {
                r: 0.9,
                g: 0.8,
                b: 0.2,
                a: 0.35,
            },
            find_current_match_color: Color {
                r: 0.95,
                g: 0.6,
                b: 0.15,
                a: 0.55,
            },
            bracket_border_thickness: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// SelectionRenderer
// ---------------------------------------------------------------------------

/// Draws selection backgrounds, line highlights, word highlights, find
/// matches, and bracket pair indicators.
pub struct SelectionRenderer {
    config: SelectionRenderConfig,
}

impl SelectionRenderer {
    pub fn new(config: SelectionRenderConfig) -> Self {
        Self { config }
    }

    pub fn config_mut(&mut self) -> &mut SelectionRenderConfig {
        &mut self.config
    }

    /// Draws the subtle current-line highlight behind the cursor line.
    pub fn draw_current_line_highlight(
        &self,
        rects: &mut RectRenderer,
        y: f32,
        line_height: f32,
        editor_width: f32,
    ) {
        rects.draw_rect(
            0.0,
            y,
            editor_width,
            line_height,
            self.config.current_line_color,
            0.0,
        );
    }

    /// Draws selection background rectangles with rounded top/bottom edges.
    pub fn draw_selections(&self, rects: &mut RectRenderer, selections: &[SelectionRect]) {
        let cfg = &self.config;
        for sel in selections {
            let radius = if sel.is_first && sel.is_last {
                cfg.selection_corner_radius
            } else if sel.is_first || sel.is_last {
                cfg.selection_corner_radius * 0.5
            } else {
                0.0
            };
            rects.draw_rect(
                sel.x,
                sel.y,
                sel.width,
                sel.height,
                cfg.selection_color,
                radius,
            );
        }
    }

    /// Draws word-under-cursor highlight occurrences.
    pub fn draw_word_highlights(&self, rects: &mut RectRenderer, highlights: &[HighlightRect]) {
        for h in highlights {
            rects.draw_rect(
                h.x,
                h.y,
                h.width,
                h.height,
                self.config.word_highlight_color,
                2.0,
            );
        }
    }

    /// Draws find-match highlights. `current_index` (if `Some`) indicates
    /// which match should be drawn with the "current match" color.
    pub fn draw_find_matches(
        &self,
        rects: &mut RectRenderer,
        matches: &[HighlightRect],
        current_index: Option<usize>,
    ) {
        for (i, m) in matches.iter().enumerate() {
            let color = if current_index == Some(i) {
                self.config.find_current_match_color
            } else {
                self.config.find_match_color
            };
            rects.draw_rect(m.x, m.y, m.width, m.height, color, 2.0);
        }
    }

    /// Draws bracket pair highlight boxes.
    pub fn draw_bracket_highlights(&self, rects: &mut RectRenderer, brackets: &[BracketHighlight]) {
        for b in brackets {
            rects.draw_border(
                b.x,
                b.y,
                b.width,
                b.height,
                b.color,
                self.config.bracket_border_thickness,
            );
        }
    }
}

impl Default for SelectionRenderer {
    fn default() -> Self {
        Self::new(SelectionRenderConfig::default())
    }
}
