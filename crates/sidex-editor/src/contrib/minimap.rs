//! Minimap — a scaled-down overview of the document shown beside the editor,
//! mirrors VS Code's `MinimapWidget` + `MinimapModel`.
//!
//! Handles the minimap's configuration, visible-range slider, click-to-line
//! mapping, character-level rendering with syntax colours, and decoration
//! overlays (errors, warnings, search hits, git changes).

use serde::{Deserialize, Serialize};

use crate::decoration::Color;

/// Which side of the editor to show the minimap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MinimapSide {
    Left,
    #[default]
    Right,
}

/// When to show the minimap slider (the highlight over the visible region).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SliderVisibility {
    #[default]
    Always,
    MouseOver,
}

/// Whether to render actual characters or approximate colour blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MinimapRenderMode {
    #[default]
    Characters,
    ColorBlocks,
}

/// Configuration for the minimap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinimapConfig {
    pub enabled: bool,
    pub side: MinimapSide,
    pub max_column: u32,
    pub show_slider: SliderVisibility,
    pub scale: f32,
    pub render_mode: MinimapRenderMode,
    pub width: f32,
}

impl Default for MinimapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            side: MinimapSide::Right,
            max_column: 120,
            show_slider: SliderVisibility::Always,
            scale: 1.0,
            render_mode: MinimapRenderMode::Characters,
            width: 60.0,
        }
    }
}

// ── Decoration kinds ────────────────────────────────────────────────────────

/// Semantic kind of a minimap decoration stripe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MinimapDecorationKind {
    Error,
    Warning,
    Find,
    Selection,
    GitAdded,
    GitModified,
    GitDeleted,
}

impl MinimapDecorationKind {
    #[must_use]
    pub fn default_color(self) -> Color {
        match self {
            Self::Error => Color::new(0.957, 0.278, 0.278, 1.0),
            Self::Warning => Color::new(0.804, 0.678, 0.0, 1.0),
            Self::Find => Color::new(0.91, 0.58, 0.14, 1.0),
            Self::Selection => Color::new(0.216, 0.580, 1.0, 0.5),
            Self::GitAdded => Color::new(0.2, 0.78, 0.35, 1.0),
            Self::GitModified => Color::new(0.216, 0.580, 1.0, 1.0),
            Self::GitDeleted => Color::new(0.957, 0.278, 0.278, 1.0),
        }
    }
}

/// A decoration drawn on the minimap (e.g. error squiggle, search match).
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapDecoration {
    pub line: u32,
    pub color: [f32; 4],
    pub kind: Option<MinimapDecorationKind>,
}

// ── Character-level render data ─────────────────────────────────────────────

/// A single coloured token within a minimap line: column + colour.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapToken {
    pub column: u32,
    pub color: Color,
}

/// Rendered data for a single minimap line.
#[derive(Debug, Clone, Default)]
pub struct MinimapLine {
    pub tokens: Vec<MinimapToken>,
}

/// Decoration stripe positioned at a vertical offset in the minimap.
#[derive(Debug, Clone, PartialEq)]
pub struct MinimapDecorationStripe {
    pub y_offset: f32,
    pub height: f32,
    pub color: Color,
    pub kind: MinimapDecorationKind,
}

/// Input decoration for `compute_minimap_data`.
#[derive(Debug, Clone)]
pub struct MinimapDecorationInput {
    pub line: u32,
    pub kind: MinimapDecorationKind,
}

/// A syntax token on a source line — (column, length, colour).
#[derive(Debug, Clone)]
pub struct SyntaxToken {
    pub column: u32,
    pub length: u32,
    pub color: Color,
}

/// Complete render data produced by `compute_minimap_data`.
#[derive(Debug, Clone, Default)]
pub struct MinimapRenderData {
    pub lines: Vec<MinimapLine>,
    pub slider_top: f32,
    pub slider_height: f32,
    pub decorations: Vec<MinimapDecorationStripe>,
}

/// Computes the full minimap render data for the renderer.
///
/// - `line_tokens` — per-line syntax tokens (index = line number).
/// - `viewport` — the visible line range `start..end`.
/// - `total_lines` — total lines in the document.
/// - `minimap_height` — the pixel height of the minimap area.
/// - `max_column` — columns beyond this are clipped.
/// - `decorations` — error/warning/git/search marks to overlay.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn compute_minimap_data(
    line_tokens: &[Vec<SyntaxToken>],
    viewport_start: u32,
    viewport_end: u32,
    total_lines: u32,
    minimap_height: f32,
    max_column: u32,
    decorations: &[MinimapDecorationInput],
) -> MinimapRenderData {
    if total_lines == 0 || minimap_height <= 0.0 {
        return MinimapRenderData::default();
    }

    let line_height = minimap_height / total_lines as f32;

    let lines: Vec<MinimapLine> = (0..total_lines as usize)
        .map(|i| {
            let tokens = line_tokens
                .get(i)
                .map(|toks| {
                    toks.iter()
                        .filter(|t| t.column < max_column)
                        .map(|t| MinimapToken {
                            column: t.column.min(max_column.saturating_sub(1)),
                            color: t.color,
                        })
                        .collect()
                })
                .unwrap_or_default();
            MinimapLine { tokens }
        })
        .collect();

    let slider_top = viewport_start as f32 * line_height;
    let slider_height =
        (viewport_end.saturating_sub(viewport_start) + 1) as f32 * line_height;

    let deco_stripes: Vec<MinimapDecorationStripe> = decorations
        .iter()
        .filter(|d| d.line < total_lines)
        .map(|d| MinimapDecorationStripe {
            y_offset: d.line as f32 * line_height,
            height: line_height.max(2.0),
            color: d.kind.default_color(),
            kind: d.kind,
        })
        .collect();

    MinimapRenderData {
        lines,
        slider_top,
        slider_height,
        decorations: deco_stripes,
    }
}

// ── State ───────────────────────────────────────────────────────────────────

/// Full state for the minimap feature.
#[derive(Debug, Clone, Default)]
pub struct MinimapState {
    pub config: MinimapConfig,
    pub is_hovered: bool,
    pub is_dragging: bool,
    pub decorations: Vec<MinimapDecoration>,
    pub render_data: Option<MinimapRenderData>,
}

impl MinimapState {
    pub fn set_decorations(&mut self, decorations: Vec<MinimapDecoration>) {
        self.decorations = decorations;
    }

    pub fn clear_decorations(&mut self) {
        self.decorations.clear();
    }

    pub fn begin_drag(&mut self) {
        self.is_dragging = true;
    }

    pub fn end_drag(&mut self) {
        self.is_dragging = false;
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn click_to_scroll_line(
        &self,
        click_y: f32,
        total_lines: u32,
        minimap_height: f32,
    ) -> u32 {
        click_to_line(click_y, total_lines, minimap_height)
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn drag_to_scroll_delta(
        &self,
        drag_delta_y: f32,
        total_lines: u32,
        minimap_height: f32,
    ) -> i32 {
        if minimap_height <= 0.0 || total_lines == 0 {
            return 0;
        }
        let lines_per_pixel = total_lines as f32 / minimap_height;
        (drag_delta_y * lines_per_pixel).round() as i32
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn hover_preview_line(
        &self,
        hover_y: f32,
        total_lines: u32,
        minimap_height: f32,
    ) -> Option<u32> {
        if !self.is_hovered || minimap_height <= 0.0 || total_lines == 0 {
            return None;
        }
        Some(click_to_line(hover_y, total_lines, minimap_height))
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn hover_preview_range(
        &self,
        hover_y: f32,
        total_lines: u32,
        minimap_height: f32,
        preview_line_count: u32,
    ) -> Option<(u32, u32)> {
        let center = self.hover_preview_line(hover_y, total_lines, minimap_height)?;
        let half = preview_line_count / 2;
        let start = center.saturating_sub(half);
        let end = (start + preview_line_count).min(total_lines);
        Some((start, end))
    }
}

/// Computes the rendered minimap height for a given scale.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn minimap_rendered_height(total_lines: u32, scale: f32, base_line_height: f32) -> f32 {
    total_lines as f32 * base_line_height * scale
}

/// Computes the effective scale factor (default 0.25 = 1/4 scale).
#[must_use]
pub fn effective_scale(config: &MinimapConfig) -> f32 {
    if config.scale <= 0.0 {
        0.25
    } else {
        config.scale
    }
}

/// Convert a scroll position (in document lines) to the minimap slider
/// y-offset in pixels.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn scroll_to_slider_y(
    scroll_line: u32,
    total_lines: u32,
    minimap_height: f32,
) -> f32 {
    if total_lines == 0 || minimap_height <= 0.0 {
        return 0.0;
    }
    (scroll_line as f32 / total_lines as f32) * minimap_height
}

/// Convert a minimap y-offset to a scroll position in document lines.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn slider_y_to_scroll(
    slider_y: f32,
    total_lines: u32,
    minimap_height: f32,
) -> u32 {
    click_to_line(slider_y, total_lines, minimap_height)
}

/// Computes the range of lines the minimap slider covers.
#[must_use]
pub fn visible_range(viewport_first: u32, viewport_lines: u32, total_lines: u32) -> (u32, u32) {
    let first = viewport_first.min(total_lines.saturating_sub(1));
    let last =
        (viewport_first + viewport_lines.saturating_sub(1)).min(total_lines.saturating_sub(1));
    (first, last)
}

/// Converts a vertical click position on the minimap to a 0-based document line.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn click_to_line(y: f32, total_lines: u32, minimap_height: f32) -> u32 {
    if minimap_height <= 0.0 || total_lines == 0 {
        return 0;
    }
    let y = y.clamp(0.0, minimap_height);
    let ratio = y / minimap_height;
    let line = (ratio * total_lines as f32).floor() as u32;
    line.min(total_lines.saturating_sub(1))
}

/// Returns the pixel y-range `(top, bottom)` of the slider on the minimap.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn slider_pixel_range(
    viewport_first: u32,
    viewport_lines: u32,
    total_lines: u32,
    minimap_height: f32,
) -> (f32, f32) {
    if total_lines == 0 || minimap_height <= 0.0 {
        return (0.0, 0.0);
    }
    let line_height = minimap_height / total_lines as f32;
    let top = viewport_first as f32 * line_height;
    let bottom = (viewport_first + viewport_lines).min(total_lines) as f32 * line_height;
    (top, bottom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_range_basic() {
        assert_eq!(visible_range(10, 20, 100), (10, 29));
    }

    #[test]
    fn visible_range_clamps_at_end() {
        assert_eq!(visible_range(90, 20, 100), (90, 99));
    }

    #[test]
    fn visible_range_zero_total() {
        assert_eq!(visible_range(0, 20, 0), (0, 0));
    }

    #[test]
    fn click_to_line_middle() {
        let line = click_to_line(250.0, 1000, 500.0);
        assert_eq!(line, 500);
    }

    #[test]
    fn click_to_line_top() {
        let line = click_to_line(0.0, 1000, 500.0);
        assert_eq!(line, 0);
    }

    #[test]
    fn click_to_line_bottom() {
        let line = click_to_line(500.0, 1000, 500.0);
        assert_eq!(line, 999);
    }

    #[test]
    fn click_to_line_clamps_negative() {
        let line = click_to_line(-10.0, 100, 200.0);
        assert_eq!(line, 0);
    }

    #[test]
    fn click_to_line_clamps_overflow() {
        let line = click_to_line(999.0, 100, 200.0);
        assert_eq!(line, 99);
    }

    #[test]
    fn click_to_line_zero_height() {
        assert_eq!(click_to_line(10.0, 100, 0.0), 0);
    }

    #[test]
    fn click_to_line_zero_lines() {
        assert_eq!(click_to_line(10.0, 0, 200.0), 0);
    }

    #[test]
    fn slider_pixel_range_basic() {
        let (top, bottom) = slider_pixel_range(0, 20, 100, 200.0);
        assert!((top - 0.0).abs() < f32::EPSILON);
        assert!((bottom - 40.0).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_pixel_range_middle() {
        let (top, bottom) = slider_pixel_range(50, 20, 100, 200.0);
        assert!((top - 100.0).abs() < f32::EPSILON);
        assert!((bottom - 140.0).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_pixel_range_clamps_at_bottom() {
        let (top, bottom) = slider_pixel_range(90, 20, 100, 200.0);
        assert!((top - 180.0).abs() < f32::EPSILON);
        assert!((bottom - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn config_defaults() {
        let config = MinimapConfig::default();
        assert!(config.enabled);
        assert_eq!(config.side, MinimapSide::Right);
        assert_eq!(config.max_column, 120);
        assert_eq!(config.show_slider, SliderVisibility::Always);
        assert!((config.width - 60.0).abs() < f32::EPSILON);
    }

    #[test]
    fn state_decorations() {
        let mut state = MinimapState::default();
        state.set_decorations(vec![
            MinimapDecoration {
                line: 10,
                color: [1.0, 0.0, 0.0, 1.0],
                kind: Some(MinimapDecorationKind::Error),
            },
            MinimapDecoration {
                line: 20,
                color: [1.0, 1.0, 0.0, 1.0],
                kind: Some(MinimapDecorationKind::Warning),
            },
        ]);
        assert_eq!(state.decorations.len(), 2);

        state.clear_decorations();
        assert!(state.decorations.is_empty());
    }

    #[test]
    fn state_drag() {
        let mut state = MinimapState::default();
        assert!(!state.is_dragging);
        state.begin_drag();
        assert!(state.is_dragging);
        state.end_drag();
        assert!(!state.is_dragging);
    }

    #[test]
    fn click_to_scroll_line_center() {
        let state = MinimapState::default();
        let line = state.click_to_scroll_line(100.0, 1000, 500.0);
        assert_eq!(line, 200);
    }

    #[test]
    fn drag_to_scroll_delta_basic() {
        let state = MinimapState::default();
        let delta = state.drag_to_scroll_delta(50.0, 1000, 500.0);
        assert_eq!(delta, 100);
    }

    #[test]
    fn drag_to_scroll_delta_zero_height() {
        let state = MinimapState::default();
        assert_eq!(state.drag_to_scroll_delta(50.0, 1000, 0.0), 0);
    }

    #[test]
    fn hover_preview_line_not_hovered() {
        let state = MinimapState::default();
        assert!(state.hover_preview_line(100.0, 1000, 500.0).is_none());
    }

    #[test]
    fn hover_preview_line_hovered() {
        let mut state = MinimapState::default();
        state.is_hovered = true;
        let line = state.hover_preview_line(250.0, 1000, 500.0);
        assert_eq!(line, Some(500));
    }

    #[test]
    fn hover_preview_range_basic() {
        let mut state = MinimapState::default();
        state.is_hovered = true;
        let range = state.hover_preview_range(250.0, 1000, 500.0, 10);
        assert_eq!(range, Some((495, 505)));
    }

    #[test]
    fn minimap_rendered_height_basic() {
        let h = minimap_rendered_height(1000, 0.25, 2.0);
        assert!((h - 500.0).abs() < f32::EPSILON);
    }

    #[test]
    fn effective_scale_default() {
        let config = MinimapConfig::default();
        assert!((effective_scale(&config) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn effective_scale_zero() {
        let mut config = MinimapConfig::default();
        config.scale = 0.0;
        assert!((effective_scale(&config) - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_to_slider_y_basic() {
        let y = scroll_to_slider_y(500, 1000, 200.0);
        assert!((y - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn slider_y_to_scroll_basic() {
        let line = slider_y_to_scroll(100.0, 1000, 200.0);
        assert_eq!(line, 500);
    }

    #[test]
    fn decoration_kind_colors() {
        let c = MinimapDecorationKind::Error.default_color();
        assert!(c.r > 0.9);
        let c = MinimapDecorationKind::GitAdded.default_color();
        assert!(c.g > 0.7);
    }

    #[test]
    fn compute_minimap_data_empty() {
        let data = compute_minimap_data(&[], 0, 0, 0, 500.0, 120, &[]);
        assert!(data.lines.is_empty());
    }

    #[test]
    fn compute_minimap_data_basic() {
        let tokens = vec![
            vec![SyntaxToken {
                column: 0,
                length: 3,
                color: Color::RED,
            }],
            vec![SyntaxToken {
                column: 4,
                length: 5,
                color: Color::BLUE,
            }],
        ];
        let data = compute_minimap_data(&tokens, 0, 1, 2, 100.0, 120, &[]);
        assert_eq!(data.lines.len(), 2);
        assert_eq!(data.lines[0].tokens.len(), 1);
        assert!((data.slider_height - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn compute_minimap_data_with_decorations() {
        let tokens = vec![vec![]];
        let decos = vec![MinimapDecorationInput {
            line: 0,
            kind: MinimapDecorationKind::Error,
        }];
        let data = compute_minimap_data(&tokens, 0, 0, 1, 100.0, 120, &decos);
        assert_eq!(data.decorations.len(), 1);
        assert_eq!(data.decorations[0].kind, MinimapDecorationKind::Error);
    }

    #[test]
    fn compute_minimap_data_clamps_column() {
        let tokens = vec![vec![SyntaxToken {
            column: 200,
            length: 1,
            color: Color::GREEN,
        }]];
        let data = compute_minimap_data(&tokens, 0, 0, 1, 100.0, 120, &[]);
        assert!(data.lines[0].tokens.is_empty());
    }
}
