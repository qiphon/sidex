//! Gutter / margin renderer.
//!
//! Renders line numbers, fold icons, breakpoint dots, git diff indicators,
//! and diagnostic markers in the gutter area to the left of the editor.

use crate::color::Color;
use crate::rect_renderer::RectRenderer;
use crate::text_renderer::{TextDrawContext, TextRenderer};

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// State of a foldable region on a given line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldState {
    /// Region is expanded (chevron points down).
    Expanded,
    /// Region is collapsed (chevron points right).
    Collapsed,
}

/// A fold marker for a specific line.
#[derive(Debug, Clone, Copy)]
pub struct FoldMarker {
    pub line: u32,
    pub state: FoldState,
}

/// A breakpoint on a specific line.
#[derive(Debug, Clone, Copy)]
pub struct Breakpoint {
    pub line: u32,
    /// Whether the breakpoint is verified / active.
    pub verified: bool,
}

/// Git diff indicator for a line or range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterDiffKind {
    Added,
    Modified,
    Deleted,
}

/// A git diff indicator on a specific line.
#[derive(Debug, Clone, Copy)]
pub struct GutterDiffMark {
    pub line: u32,
    pub kind: GutterDiffKind,
}

/// Diagnostic severity for gutter dots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterDiagnosticSeverity {
    Error,
    Warning,
}

/// A diagnostic indicator on a specific line.
#[derive(Debug, Clone, Copy)]
pub struct GutterDiagnostic {
    pub line: u32,
    pub severity: GutterDiagnosticSeverity,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for gutter rendering.
#[derive(Debug, Clone)]
pub struct GutterConfig {
    /// Total gutter width in pixels.
    pub width: f32,
    /// Left padding inside the gutter.
    pub padding_left: f32,
    /// Width reserved for fold icons.
    pub fold_column_width: f32,
    /// Width reserved for breakpoint dots.
    pub breakpoint_column_width: f32,
    /// Width reserved for git diff bars.
    pub diff_bar_width: f32,
    /// Font size for line numbers.
    pub font_size: f32,
    /// Line number color for normal lines.
    pub line_number_color: Color,
    /// Line number color for the active (cursor) line.
    pub active_line_number_color: Color,
    /// Background color for the gutter area.
    pub background_color: Color,
    /// Breakpoint dot color.
    pub breakpoint_color: Color,
    /// Breakpoint dot color for unverified breakpoints.
    pub breakpoint_unverified_color: Color,
    /// Color for fold chevrons.
    pub fold_icon_color: Color,
    /// Git added bar color.
    pub diff_added_color: Color,
    /// Git modified bar color.
    pub diff_modified_color: Color,
    /// Git deleted triangle color.
    pub diff_deleted_color: Color,
    /// Error diagnostic dot color.
    pub diagnostic_error_color: Color,
    /// Warning diagnostic dot color.
    pub diagnostic_warning_color: Color,
}

impl Default for GutterConfig {
    fn default() -> Self {
        Self {
            width: 64.0,
            padding_left: 4.0,
            fold_column_width: 14.0,
            breakpoint_column_width: 14.0,
            diff_bar_width: 3.0,
            font_size: 13.0,
            line_number_color: Color::from_rgb(130, 130, 130),
            active_line_number_color: Color::from_rgb(220, 220, 220),
            background_color: Color {
                r: 0.12,
                g: 0.12,
                b: 0.12,
                a: 1.0,
            },
            breakpoint_color: Color::from_rgb(220, 50, 50),
            breakpoint_unverified_color: Color::from_rgb(140, 140, 140),
            fold_icon_color: Color::from_rgb(160, 160, 160),
            diff_added_color: Color::from_rgb(80, 200, 80),
            diff_modified_color: Color::from_rgb(80, 140, 220),
            diff_deleted_color: Color::from_rgb(220, 80, 80),
            diagnostic_error_color: Color::from_rgb(230, 60, 60),
            diagnostic_warning_color: Color::from_rgb(220, 180, 40),
        }
    }
}

// ---------------------------------------------------------------------------
// GutterRenderer
// ---------------------------------------------------------------------------

/// Renders the editor gutter: line numbers, fold icons, breakpoints, git
/// diff bars, and diagnostic indicators.
pub struct GutterRenderer {
    config: GutterConfig,
    /// Scratch buffer for formatting line numbers to avoid allocation.
    line_num_buf: String,
}

impl GutterRenderer {
    pub fn new(config: GutterConfig) -> Self {
        Self {
            config,
            line_num_buf: String::with_capacity(8),
        }
    }

    pub fn config_mut(&mut self) -> &mut GutterConfig {
        &mut self.config
    }

    /// Returns the configured gutter width.
    pub fn width(&self) -> f32 {
        self.config.width
    }

    /// Renders the gutter for visible lines.
    ///
    /// * `first_line` — 1-based line number of the first visible line.
    /// * `visible_count` — number of visible lines.
    /// * `line_height` — pixel height of each line.
    /// * `active_line` — 1-based line number where the primary cursor is.
    /// * `scroll_y` — current vertical scroll offset in pixels.
    #[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
    pub fn render(
        &mut self,
        rects: &mut RectRenderer,
        text: &mut TextRenderer,
        ctx: &mut TextDrawContext<'_>,
        first_line: u32,
        visible_count: u32,
        line_height: f32,
        active_line: u32,
        scroll_y: f32,
        folds: &[FoldMarker],
        breakpoints: &[Breakpoint],
        diff_marks: &[GutterDiffMark],
        diagnostics: &[GutterDiagnostic],
    ) {
        let cfg = &self.config;

        // -- Gutter background -----------------------------------------------
        let gutter_height = visible_count as f32 * line_height;
        rects.draw_rect(
            0.0,
            0.0,
            cfg.width,
            gutter_height,
            cfg.background_color,
            0.0,
        );

        let bp_x = cfg.padding_left;
        let fold_x = bp_x + cfg.breakpoint_column_width;
        let number_x = fold_x + cfg.fold_column_width;
        let diff_x = cfg.width - cfg.diff_bar_width;

        for i in 0..visible_count {
            let line = first_line + i;
            let y = i as f32 * line_height - (scroll_y % line_height);

            // -- Line number -------------------------------------------------
            let color = if line == active_line {
                cfg.active_line_number_color
            } else {
                cfg.line_number_color
            };

            self.line_num_buf.clear();
            let _ = std::fmt::Write::write_fmt(&mut self.line_num_buf, format_args!("{line}"));
            text.draw_line(&self.line_num_buf, number_x, y, color, cfg.font_size, ctx);

            // -- Breakpoints ------------------------------------------------
            if let Some(bp) = breakpoints.iter().find(|b| b.line == line) {
                let bp_radius = (line_height * 0.32).min(6.0);
                let bp_center_x = bp_x + cfg.breakpoint_column_width * 0.5;
                let bp_center_y = y + line_height * 0.5;
                let color = if bp.verified {
                    cfg.breakpoint_color
                } else {
                    cfg.breakpoint_unverified_color
                };
                rects.draw_rect(
                    bp_center_x - bp_radius,
                    bp_center_y - bp_radius,
                    bp_radius * 2.0,
                    bp_radius * 2.0,
                    color,
                    bp_radius,
                );
            }

            // -- Fold icons --------------------------------------------------
            if let Some(fm) = folds.iter().find(|f| f.line == line) {
                let icon_size = line_height.min(14.0) * 0.5;
                let ix = fold_x + cfg.fold_column_width * 0.5 - icon_size * 0.5;
                let iy = y + line_height * 0.5 - icon_size * 0.5;
                match fm.state {
                    FoldState::Expanded => {
                        // Draw downward chevron as a small triangle approximation
                        rects.draw_rect(
                            ix,
                            iy,
                            icon_size,
                            icon_size * 0.5,
                            cfg.fold_icon_color,
                            1.0,
                        );
                    }
                    FoldState::Collapsed => {
                        // Draw rightward chevron as a small rect approximation
                        rects.draw_rect(
                            ix,
                            iy,
                            icon_size * 0.5,
                            icon_size,
                            cfg.fold_icon_color,
                            1.0,
                        );
                    }
                }
            }

            // -- Git diff bars -----------------------------------------------
            if let Some(dm) = diff_marks.iter().find(|d| d.line == line) {
                let color = match dm.kind {
                    GutterDiffKind::Added => cfg.diff_added_color,
                    GutterDiffKind::Modified => cfg.diff_modified_color,
                    GutterDiffKind::Deleted => cfg.diff_deleted_color,
                };
                match dm.kind {
                    GutterDiffKind::Added | GutterDiffKind::Modified => {
                        rects.draw_rect(diff_x, y, cfg.diff_bar_width, line_height, color, 0.0);
                    }
                    GutterDiffKind::Deleted => {
                        // Small triangle at line boundary
                        let tri_h = 6.0_f32.min(line_height * 0.5);
                        rects.draw_rect(
                            diff_x,
                            y + line_height - tri_h,
                            cfg.diff_bar_width + 2.0,
                            tri_h,
                            color,
                            0.0,
                        );
                    }
                }
            }

            // -- Diagnostic dots ---------------------------------------------
            if let Some(diag) = diagnostics.iter().find(|d| d.line == line) {
                let diag_radius = 3.0_f32;
                let diag_center_x = cfg.padding_left + 2.0;
                let diag_center_y = y + line_height - diag_radius - 1.0;
                let color = match diag.severity {
                    GutterDiagnosticSeverity::Error => cfg.diagnostic_error_color,
                    GutterDiagnosticSeverity::Warning => cfg.diagnostic_warning_color,
                };
                rects.draw_rect(
                    diag_center_x - diag_radius,
                    diag_center_y - diag_radius,
                    diag_radius * 2.0,
                    diag_radius * 2.0,
                    color,
                    diag_radius,
                );
            }
        }
    }
}

impl Default for GutterRenderer {
    fn default() -> Self {
        Self::new(GutterConfig::default())
    }
}
