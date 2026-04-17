//! Terminal GPU rendering primitives.
//!
//! Converts a [`TerminalGrid`] into render primitives (glyphs, rectangles,
//! lines) that can be consumed by a GPU renderer such as `sidex-gpu`.

use crate::grid::{CellAttributes, Color, TerminalGrid};
use crate::selection;
use std::ops::Range;

/// Terminal cursor shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Underline,
    Bar,
}

impl Default for CursorShape {
    fn default() -> Self {
        Self::Block
    }
}

/// Underline style for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderlineStyle {
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// Font metrics needed for layout calculations.
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    pub cell_width: f32,
    pub cell_height: f32,
    pub baseline: f32,
    pub underline_offset: f32,
    pub underline_thickness: f32,
}

/// Configuration for terminal rendering.
pub struct TerminalRenderer {
    pub font_size: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub padding: f32,
    pub cursor_style: CursorShape,
    pub cursor_blink: bool,
    pub cursor_blink_visible: bool,
}

impl Default for TerminalRenderer {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            cell_width: 8.4,
            cell_height: 18.0,
            padding: 4.0,
            cursor_style: CursorShape::Block,
            cursor_blink: true,
            cursor_blink_visible: true,
        }
    }
}

/// A single glyph to render.
#[derive(Debug, Clone)]
pub struct GlyphInstance {
    pub x: f32,
    pub y: f32,
    pub glyph: char,
    pub color: [f32; 4],
    pub bold: bool,
    pub italic: bool,
}

/// A filled rectangle to render (backgrounds, cursor, selection).
#[derive(Debug, Clone)]
pub struct RectInstance {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
}

/// A line to render (underlines, link underlines).
#[derive(Debug, Clone)]
pub struct LineInstance {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub color: [f32; 4],
    pub style: UnderlineStyle,
}

/// The complete render output for a terminal frame.
#[derive(Debug, Clone, Default)]
pub struct TerminalRenderOutput {
    pub glyphs: Vec<GlyphInstance>,
    pub backgrounds: Vec<RectInstance>,
    pub cursor: Option<RectInstance>,
    pub selection_rects: Vec<RectInstance>,
    pub underlines: Vec<LineInstance>,
    pub link_underlines: Vec<LineInstance>,
}

/// Converts a terminal grid into render primitives for the given viewport row range.
pub fn render_terminal(
    grid: &TerminalGrid,
    viewport: Range<u32>,
    renderer: &TerminalRenderer,
) -> TerminalRenderOutput {
    let mut output = TerminalRenderOutput::default();
    let metrics = FontMetrics {
        cell_width: renderer.cell_width,
        cell_height: renderer.cell_height,
        baseline: renderer.cell_height * 0.8,
        underline_offset: renderer.cell_height * 0.9,
        underline_thickness: 1.0,
    };

    let selection_color: [f32; 4] = [0.25, 0.45, 0.75, 0.4];

    for view_row in viewport.clone() {
        let grid_row = view_row as u16;
        if grid_row >= grid.rows() {
            break;
        }

        let y_offset = (view_row - viewport.start) as f32 * metrics.cell_height + renderer.padding;

        for col in 0..grid.cols() {
            let cell = grid.cell(grid_row, col);

            if cell.is_wide_continuation() {
                continue;
            }

            let x_offset = col as f32 * metrics.cell_width + renderer.padding;
            let cell_w = if cell.width == 2 {
                metrics.cell_width * 2.0
            } else {
                metrics.cell_width
            };

            // Background
            if cell.bg != Color::Default {
                output.backgrounds.push(RectInstance {
                    x: x_offset,
                    y: y_offset,
                    w: cell_w,
                    h: metrics.cell_height,
                    color: color_to_rgba(cell.bg, false),
                });
            }

            // Selection highlight
            if let Some(ref sel) = grid.selection {
                if selection::is_selected(sel, grid_row, col) {
                    output.selection_rects.push(RectInstance {
                        x: x_offset,
                        y: y_offset,
                        w: cell_w,
                        h: metrics.cell_height,
                        color: selection_color,
                    });
                }
            }

            // Glyph
            if cell.c != ' ' && !cell.attrs.contains(CellAttributes::HIDDEN) {
                let fg_color = if cell.attrs.contains(CellAttributes::INVERSE) {
                    color_to_rgba(cell.bg, true)
                } else {
                    color_to_rgba(cell.fg, true)
                };

                output.glyphs.push(GlyphInstance {
                    x: x_offset,
                    y: y_offset + metrics.baseline,
                    glyph: cell.c,
                    color: fg_color,
                    bold: cell.bold(),
                    italic: cell.italic(),
                });
            }

            // Underlines
            let ul_style = cell.underline_style();
            if ul_style > 0 {
                let style = match ul_style {
                    2 => UnderlineStyle::Double,
                    3 => UnderlineStyle::Curly,
                    4 => UnderlineStyle::Dotted,
                    5 => UnderlineStyle::Dashed,
                    _ => UnderlineStyle::Single,
                };
                output.underlines.push(LineInstance {
                    x: x_offset,
                    y: y_offset + metrics.underline_offset,
                    width: cell_w,
                    color: color_to_rgba(cell.fg, true),
                    style,
                });
            }

            // Strikethrough
            if cell.strikethrough() {
                output.underlines.push(LineInstance {
                    x: x_offset,
                    y: y_offset + metrics.cell_height * 0.5,
                    width: cell_w,
                    color: color_to_rgba(cell.fg, true),
                    style: UnderlineStyle::Single,
                });
            }

            // Hyperlink underline
            if cell.hyperlink.is_some() {
                output.link_underlines.push(LineInstance {
                    x: x_offset,
                    y: y_offset + metrics.underline_offset,
                    width: cell_w,
                    color: [0.4, 0.6, 1.0, 0.8],
                    style: UnderlineStyle::Single,
                });
            }
        }
    }

    // Cursor
    if grid.cursor.visible && renderer.cursor_blink_visible {
        let (crow, ccol) = grid.cursor_position();
        let crow32 = crow as u32;
        if viewport.contains(&crow32) {
            let cy = (crow32 - viewport.start) as f32 * metrics.cell_height + renderer.padding;
            let cx = ccol as f32 * metrics.cell_width + renderer.padding;

            let cursor_rect = match renderer.cursor_style {
                CursorShape::Block => RectInstance {
                    x: cx,
                    y: cy,
                    w: metrics.cell_width,
                    h: metrics.cell_height,
                    color: [1.0, 1.0, 1.0, 0.6],
                },
                CursorShape::Underline => RectInstance {
                    x: cx,
                    y: cy + metrics.cell_height - 2.0,
                    w: metrics.cell_width,
                    h: 2.0,
                    color: [1.0, 1.0, 1.0, 0.9],
                },
                CursorShape::Bar => RectInstance {
                    x: cx,
                    y: cy,
                    w: 2.0,
                    h: metrics.cell_height,
                    color: [1.0, 1.0, 1.0, 0.9],
                },
            };
            output.cursor = Some(cursor_rect);
        }
    }

    output
}

/// Converts a `Color` to an RGBA `[f32; 4]` array.
fn color_to_rgba(color: Color, is_fg: bool) -> [f32; 4] {
    match color {
        Color::Default => {
            if is_fg {
                [0.85, 0.85, 0.85, 1.0]
            } else {
                [0.0, 0.0, 0.0, 0.0]
            }
        }
        Color::Named(named) => {
            let idx = named.to_index();
            indexed_color_to_rgba(idx)
        }
        Color::Indexed(idx) => indexed_color_to_rgba(idx),
        Color::Rgb(r, g, b) => [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0],
    }
}

fn indexed_color_to_rgba(idx: u8) -> [f32; 4] {
    #[allow(clippy::match_same_arms)]
    let (r, g, b) = match idx {
        0 => (0, 0, 0),
        1 => (205, 49, 49),
        2 => (13, 188, 121),
        3 => (229, 229, 16),
        4 => (36, 114, 200),
        5 => (188, 63, 188),
        6 => (17, 168, 205),
        7 => (204, 204, 204),
        8 => (118, 118, 118),
        9 => (241, 76, 76),
        10 => (35, 209, 139),
        11 => (245, 245, 67),
        12 => (59, 142, 234),
        13 => (214, 112, 214),
        14 => (41, 184, 219),
        15 => (229, 229, 229),
        16..=231 => {
            let c = idx - 16;
            let ri = c / 36;
            let gi = (c % 36) / 6;
            let bi = c % 6;
            let to_val = |v: u8| if v == 0 { 0u8 } else { 55 + 40 * v };
            (to_val(ri), to_val(gi), to_val(bi))
        }
        232..=255 => {
            let v = 8 + 10 * (idx - 232);
            (v, v, v)
        }
    };
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::Cell;

    #[test]
    fn render_empty_grid() {
        let grid = TerminalGrid::new(24, 80);
        let renderer = TerminalRenderer::default();
        let output = render_terminal(&grid, 0..24, &renderer);
        assert!(output.glyphs.is_empty());
        assert!(output.backgrounds.is_empty());
        assert!(output.cursor.is_some());
    }

    #[test]
    fn render_text() {
        let mut grid = TerminalGrid::new(4, 10);
        let template = Cell::default();
        for ch in "Hi".chars() {
            grid.write_char(ch, &template);
        }
        let renderer = TerminalRenderer::default();
        let output = render_terminal(&grid, 0..4, &renderer);
        assert_eq!(output.glyphs.len(), 2);
        assert_eq!(output.glyphs[0].glyph, 'H');
        assert_eq!(output.glyphs[1].glyph, 'i');
    }

    #[test]
    fn cursor_shapes() {
        let grid = TerminalGrid::new(4, 10);
        for shape in [CursorShape::Block, CursorShape::Underline, CursorShape::Bar] {
            let renderer = TerminalRenderer {
                cursor_style: shape,
                ..TerminalRenderer::default()
            };
            let output = render_terminal(&grid, 0..4, &renderer);
            assert!(output.cursor.is_some());
        }
    }

    #[test]
    fn color_conversion() {
        let rgba = color_to_rgba(Color::Rgb(255, 0, 128), true);
        assert!((rgba[0] - 1.0).abs() < f32::EPSILON);
        assert!((rgba[1]).abs() < f32::EPSILON);
        assert!((rgba[2] - 128.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn indexed_color_palette() {
        let black = indexed_color_to_rgba(0);
        assert!((black[0]).abs() < f32::EPSILON);

        let white = indexed_color_to_rgba(15);
        assert!(white[0] > 0.8);
    }
}
