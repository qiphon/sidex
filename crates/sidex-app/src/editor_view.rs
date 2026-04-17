//! Editor rendering integration — wires Buffer + Syntax + Theme + GPU
//! to render a file with syntax highlighting on screen.

use sidex_gpu::color::Color as GpuColor;
use sidex_gpu::cursor_renderer::CursorPosition;
use sidex_gpu::editor_view::{DocumentSnapshot, EditorConfig as GpuEditorConfig, FrameInput};
use sidex_gpu::line_renderer::{StyledLine, StyledSpan, TextStyle, Viewport as GpuViewport};
use sidex_gpu::selection_renderer::SelectionRect;
use sidex_syntax::highlight::HighlightEvent;
use sidex_text::Position;
use sidex_theme::token_color::TokenColorMap;
use sidex_theme::Theme;

use crate::document_state::DocumentState;
use crate::layout::Rect;

/// Configuration for the editor view.
#[derive(Debug, Clone)]
pub struct EditorViewConfig {
    pub font_size: f32,
    pub line_height: f32,
    pub char_width: f32,
    pub gutter_width: f32,
    pub minimap_enabled: bool,
}

impl Default for EditorViewConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            line_height: 20.0,
            char_width: 8.4,
            gutter_width: 64.0,
            minimap_enabled: false,
        }
    }
}

/// Convert a `sidex_theme::Color` (u8 channels) to `sidex_gpu::color::Color` (f32 channels).
fn theme_color_to_gpu(c: sidex_theme::Color) -> GpuColor {
    GpuColor {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: f32::from(c.a) / 255.0,
    }
}

/// Build styled lines from highlight events + source text + theme token colors.
///
/// Walks the `HighlightEvent` stream produced by tree-sitter and maps each
/// captured scope to its resolved color from the theme.
pub fn build_styled_lines(
    doc: &DocumentState,
    theme: &Theme,
    default_fg: GpuColor,
) -> Vec<StyledLine> {
    let text = doc.document.text();
    if text.is_empty() {
        return vec![StyledLine { spans: vec![] }];
    }

    let token_map = TokenColorMap::new(theme.token_colors.clone());

    if doc.highlight_events.is_empty() {
        return text_to_plain_styled_lines(&text, default_fg);
    }

    let mut lines: Vec<StyledLine> = Vec::new();
    let mut current_spans: Vec<StyledSpan> = Vec::new();
    let mut style_stack: Vec<TextStyle> = Vec::new();
    let bytes = text.as_bytes();

    let current_style = |stack: &[TextStyle], default_fg: GpuColor| -> TextStyle {
        stack.last().copied().unwrap_or(TextStyle {
            color: default_fg,
            ..TextStyle::default()
        })
    };

    for event in &doc.highlight_events {
        match event {
            HighlightEvent::Source { start, end } => {
                let s = std::cmp::min(*start, bytes.len());
                let e = std::cmp::min(*end, bytes.len());
                if s >= e {
                    continue;
                }
                let slice = &text[s..e];
                let style = current_style(&style_stack, default_fg);

                for (i, part) in slice.split('\n').enumerate() {
                    if i > 0 {
                        lines.push(StyledLine {
                            spans: std::mem::take(&mut current_spans),
                        });
                    }
                    if !part.is_empty() {
                        let clean = part.strip_suffix('\r').unwrap_or(part);
                        if !clean.is_empty() {
                            current_spans.push(StyledSpan {
                                text: clean.to_owned(),
                                style,
                            });
                        }
                    }
                }
            }
            HighlightEvent::HighlightStart(highlight) => {
                let highlight_config = match doc.highlight_config.as_ref() {
                    Some(cfg) => cfg,
                    None => continue,
                };
                let capture_names = highlight_config.capture_names();
                let capture_name = capture_names
                    .get(highlight.0 as usize)
                    .map(String::as_str)
                    .unwrap_or("");

                let resolved = token_map.resolve(capture_name);

                let fg = resolved
                    .foreground
                    .map(theme_color_to_gpu)
                    .unwrap_or(default_fg);

                let font_style = resolved.font_style;
                style_stack.push(TextStyle {
                    color: fg,
                    bold: font_style.contains(sidex_theme::FontStyle::BOLD),
                    italic: font_style.contains(sidex_theme::FontStyle::ITALIC),
                    underline: font_style.contains(sidex_theme::FontStyle::UNDERLINE),
                    strikethrough: font_style.contains(sidex_theme::FontStyle::STRIKETHROUGH),
                });
            }
            HighlightEvent::HighlightEnd => {
                style_stack.pop();
            }
        }
    }

    lines.push(StyledLine {
        spans: current_spans,
    });

    lines
}

/// Fall back to unstyled lines if no highlights exist.
fn text_to_plain_styled_lines(text: &str, default_fg: GpuColor) -> Vec<StyledLine> {
    text.lines()
        .map(|line| StyledLine {
            spans: vec![StyledSpan {
                text: line.to_owned(),
                style: TextStyle {
                    color: default_fg,
                    ..TextStyle::default()
                },
            }],
        })
        .collect()
}

/// Build a `DocumentSnapshot` suitable for the GPU `EditorView::render`.
pub fn build_document_snapshot(styled_lines: &[StyledLine], char_width: f32) -> DocumentSnapshot {
    let max_chars = styled_lines
        .iter()
        .map(StyledLine::char_count)
        .max()
        .unwrap_or(0);
    #[allow(clippy::cast_possible_truncation)]
    DocumentSnapshot {
        lines: styled_lines.to_vec(),
        total_lines: styled_lines.len() as u32,
        max_line_width: (max_chars as f32 * char_width) as u32,
    }
}

/// Build a `GpuViewport` from `DocumentState` viewport and editor rect.
pub fn build_gpu_viewport(
    doc: &DocumentState,
    editor_rect: &Rect,
    config: &EditorViewConfig,
) -> GpuViewport {
    let first_line = doc.viewport.first_visible_line;
    #[allow(clippy::cast_possible_truncation)]
    let visible_lines = (editor_rect.height / config.line_height) as u32 + 1;
    GpuViewport {
        first_line,
        visible_lines,
        scroll_x: doc.viewport.scroll_left as f32,
        scroll_y: doc.viewport.scroll_top as f32,
        width: editor_rect.width,
        height: editor_rect.height,
    }
}

/// Build cursor positions for the GPU cursor renderer.
pub fn build_cursor_positions(
    doc: &DocumentState,
    config: &EditorViewConfig,
    cursor_visible: bool,
) -> Vec<CursorPosition> {
    if !cursor_visible {
        return Vec::new();
    }
    let first_line = doc.viewport.first_visible_line;
    doc.document
        .cursors
        .cursors()
        .iter()
        .filter_map(|cursor| {
            let pos = cursor.selection.active;
            if pos.line >= first_line {
                let x = config.gutter_width + pos.column as f32 * config.char_width
                    - doc.viewport.scroll_left as f32;
                let y = (pos.line - first_line) as f32 * config.line_height;
                Some(CursorPosition {
                    x,
                    y,
                    cell_width: config.char_width,
                    cell_height: config.line_height,
                })
            } else {
                None
            }
        })
        .collect()
}

/// Build selection rectangles for the GPU selection renderer.
pub fn build_selection_rects(doc: &DocumentState, config: &EditorViewConfig) -> Vec<SelectionRect> {
    let first_line = doc.viewport.first_visible_line;
    let mut rects = Vec::new();

    for cursor in doc.document.cursors.cursors() {
        let sel = &cursor.selection;
        if sel.is_empty() {
            continue;
        }
        let start = sel.start();
        let end = sel.end();

        for line in start.line..=end.line {
            if line < first_line {
                continue;
            }
            let rel_y = (line - first_line) as f32 * config.line_height;
            let line_len = doc.document.buffer.line_content_len(line as usize) as u32;

            let col_start = if line == start.line { start.column } else { 0 };
            let col_end = if line == end.line {
                end.column
            } else {
                line_len
            };

            let x = config.gutter_width + col_start as f32 * config.char_width
                - doc.viewport.scroll_left as f32;
            let width = (col_end - col_start) as f32 * config.char_width;

            let is_first = line == start.line;
            let is_last = line == end.line;

            rects.push(SelectionRect {
                x,
                y: rel_y,
                width: width.max(config.char_width * 0.25),
                height: config.line_height,
                is_first,
                is_last,
            });
        }
    }
    rects
}

/// Build a `GpuEditorConfig` from the editor view config and theme.
pub fn build_gpu_editor_config(config: &EditorViewConfig, theme: &Theme) -> GpuEditorConfig {
    let bg = theme
        .workbench_colors
        .editor_background
        .map(theme_color_to_gpu)
        .unwrap_or(GpuColor {
            r: 0.12,
            g: 0.12,
            b: 0.12,
            a: 1.0,
        });

    GpuEditorConfig {
        font_size: config.font_size,
        font_family: String::from("monospace"),
        line_height: config.line_height,
        minimap_enabled: config.minimap_enabled,
        line_numbers: true,
        gutter_width: config.gutter_width,
        word_wrap: false,
        whitespace_rendering: sidex_gpu::line_renderer::WhitespaceRender::None,
        background_color: bg,
    }
}

/// Build the `FrameInput` for a frame with the active document.
pub fn build_frame_input<'a>(
    viewport: GpuViewport,
    gpu_config: &'a GpuEditorConfig,
    cursor_positions: &'a [CursorPosition],
    selections: &'a [SelectionRect],
    active_line: u32,
    dt: f32,
) -> FrameInput<'a> {
    FrameInput {
        viewport,
        config: gpu_config,
        cursor_positions,
        active_line,
        selections,
        word_highlights: &[],
        find_matches: &[],
        find_current_index: None,
        bracket_highlights: &[],
        indent_guides: &[],
        sticky_headers: &[],
        code_lenses: &[],
        inlay_hints: &[],
        wrap_indicators: &[],
        folds: &[],
        breakpoints: &[],
        gutter_diff_marks: &[],
        gutter_diagnostics: &[],
        minimap_lines: &[],
        minimap_selections: &[],
        minimap_search_matches: &[],
        minimap_diagnostics: &[],
        minimap_git_changes: &[],
        overview_marks: &[],
        dt,
    }
}

/// Convert a pixel coordinate relative to the editor rect into a document position.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn pixel_to_position(
    px: f32,
    py: f32,
    doc: &DocumentState,
    config: &EditorViewConfig,
) -> Position {
    let rel_y = py as f64 + doc.viewport.scroll_top;
    let line = (rel_y / doc.viewport.line_height).floor().max(0.0) as u32;

    let rel_x = (px - config.gutter_width).max(0.0) as f64 + doc.viewport.scroll_left;
    let col = (rel_x / config.char_width as f64).round().max(0.0) as u32;

    let max_line = doc.document.buffer.len_lines().saturating_sub(1) as u32;
    let clamped_line = line.min(max_line);
    let max_col = doc.document.buffer.line_content_len(clamped_line as usize) as u32;
    let clamped_col = col.min(max_col);

    Position::new(clamped_line, clamped_col)
}

/// Convert a document position to a pixel coordinate relative to the editor rect.
#[allow(clippy::cast_precision_loss)]
pub fn position_to_pixel(
    pos: Position,
    doc: &DocumentState,
    config: &EditorViewConfig,
) -> (f32, f32) {
    let x = config.gutter_width + pos.column as f32 * config.char_width
        - doc.viewport.scroll_left as f32;
    let y = pos.line as f32 * config.line_height as f32 - doc.viewport.scroll_top as f32;
    (x, y)
}

/// Ensure the viewport scrolls to keep the primary cursor visible.
pub fn ensure_cursor_visible(doc: &mut DocumentState) {
    let pos = doc.document.cursors.primary().position();
    doc.viewport.ensure_visible(pos);
}

// ── Mouse-event helpers for the event loop ──────────────────────

/// Determine whether a pixel position is inside the editor gutter
/// (line-number area to the left of the code).
pub fn is_in_gutter(px: f32, doc: &DocumentState, config: &EditorViewConfig) -> bool {
    let rel_x = px as f64 + doc.viewport.scroll_left;
    rel_x < config.gutter_width as f64
}

/// Given a pixel Y position in the editor area, compute the document
/// line number (for gutter clicks like setting breakpoints).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn gutter_line_at(py: f32, doc: &DocumentState) -> u32 {
    let rel_y = py as f64 + doc.viewport.scroll_top;
    let line = (rel_y / doc.viewport.line_height).floor().max(0.0) as u32;
    let max_line = doc.document.buffer.len_lines().saturating_sub(1) as u32;
    line.min(max_line)
}

/// Compute the word range at a given document position.
pub fn word_range_at(doc: &DocumentState, pos: Position) -> sidex_editor::Selection {
    let range = sidex_editor::word_at(&doc.document.buffer, pos);
    sidex_editor::Selection::new(range.start, range.end)
}

/// Compute the full-line selection for a given line number.
pub fn line_selection(doc: &DocumentState, line: u32) -> sidex_editor::Selection {
    let max_line = doc.document.buffer.len_lines().saturating_sub(1) as u32;
    let clamped = line.min(max_line);
    let start = Position::new(clamped, 0);
    let end_col = doc.document.buffer.line_content_len(clamped as usize) as u32;
    let end = Position::new(clamped, end_col);
    sidex_editor::Selection::new(start, end)
}

/// Compute the selection for extending from an anchor to a target
/// position (used for Shift+Click).
pub fn extend_selection(
    anchor: Position,
    target: Position,
) -> sidex_editor::Selection {
    sidex_editor::Selection::new(anchor, target)
}

/// Determine the visible line range for the current viewport.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn visible_line_range(doc: &DocumentState, config: &EditorViewConfig, editor_height: f32) -> (u32, u32) {
    let first = doc.viewport.first_visible_line;
    let count = (editor_height / config.line_height) as u32 + 1;
    let total = doc.document.buffer.len_lines() as u32;
    (first, (first + count).min(total))
}
