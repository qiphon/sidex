//! Diagnostic gutter icons — colored shapes in the gutter margin for lines
//! with diagnostics: red circle (error), yellow triangle (warning), blue
//! circle (info), grey dot (hint).

use crate::color::Color;
use crate::rect_renderer::RectRenderer;

// ── Severity ────────────────────────────────────────────────────────────────

/// Severity level for gutter diagnostic icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GutterIconSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl GutterIconSeverity {
    pub fn color(self) -> Color {
        match self {
            Self::Error => Color::from_rgb(230, 60, 60),
            Self::Warning => Color::from_rgb(220, 180, 40),
            Self::Info => Color::from_rgb(55, 148, 255),
            Self::Hint => Color::from_rgb(140, 140, 140),
        }
    }
}

// ── GutterDiagnosticIcon ────────────────────────────────────────────────────

/// A single diagnostic icon to draw in the gutter.
#[derive(Debug, Clone, Copy)]
pub struct GutterDiagnosticIcon {
    pub line: u32,
    pub severity: GutterIconSeverity,
}

// ── DiagnosticGutterRenderer ────────────────────────────────────────────────

/// Renders diagnostic severity icons in the editor gutter.
pub struct DiagnosticGutterRenderer {
    icon_size: f32,
    margin_left: f32,
}

impl DiagnosticGutterRenderer {
    pub fn new() -> Self {
        Self {
            icon_size: 8.0,
            margin_left: 4.0,
        }
    }

    /// Draws a single diagnostic icon at the specified gutter position.
    ///
    /// - Error: filled circle (high corner_radius)
    /// - Warning: diamond shape approximated by a rotated square
    /// - Info: filled circle, slightly smaller
    /// - Hint: small dot
    #[allow(clippy::cast_precision_loss)]
    pub fn draw_icon(
        &self,
        rects: &mut RectRenderer,
        severity: GutterIconSeverity,
        gutter_x: f32,
        line_y: f32,
        line_height: f32,
    ) {
        let color = severity.color();
        let cx = gutter_x + self.margin_left;
        let cy = line_y + line_height / 2.0;

        match severity {
            GutterIconSeverity::Error => {
                let r = self.icon_size / 2.0;
                rects.draw_rect(cx - r, cy - r, r * 2.0, r * 2.0, color, r);
            }
            GutterIconSeverity::Warning => {
                // Approximate a triangle with a diamond (rotated square)
                let s = self.icon_size * 0.8;
                let half = s / 2.0;
                rects.draw_rect(cx - half, cy - half, s, s, color, 1.0);
            }
            GutterIconSeverity::Info => {
                let r = self.icon_size / 2.0 - 0.5;
                rects.draw_rect(cx - r, cy - r, r * 2.0, r * 2.0, color, r);
            }
            GutterIconSeverity::Hint => {
                let r = 2.5;
                rects.draw_rect(cx - r, cy - r, r * 2.0, r * 2.0, color, r);
            }
        }
    }

    /// Renders all diagnostic gutter icons for visible lines.
    ///
    /// `icons` should already be deduplicated per line (highest severity wins).
    #[allow(clippy::cast_precision_loss)]
    pub fn draw_all(
        &self,
        rects: &mut RectRenderer,
        icons: &[GutterDiagnosticIcon],
        gutter_x: f32,
        first_line: u32,
        line_height: f32,
        scroll_y: f32,
    ) {
        for icon in icons {
            if icon.line < first_line {
                continue;
            }
            let line_offset = (icon.line - first_line) as f32;
            let y = line_offset * line_height - (scroll_y % line_height);
            self.draw_icon(rects, icon.severity, gutter_x, y, line_height);
        }
    }
}

impl Default for DiagnosticGutterRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Coalesces diagnostic icons per line, keeping only the highest severity.
pub fn coalesce_gutter_icons(icons: &mut Vec<GutterDiagnosticIcon>) {
    if icons.len() <= 1 {
        return;
    }
    icons.sort_by_key(|i| (i.line, i.severity));
    icons.dedup_by(|a, b| {
        if a.line == b.line {
            // Keep b (which is earlier = higher severity thanks to Ord)
            true
        } else {
            false
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ordering() {
        assert!(GutterIconSeverity::Error < GutterIconSeverity::Warning);
        assert!(GutterIconSeverity::Warning < GutterIconSeverity::Info);
        assert!(GutterIconSeverity::Info < GutterIconSeverity::Hint);
    }

    #[test]
    fn coalesce_keeps_highest_severity() {
        let mut icons = vec![
            GutterDiagnosticIcon {
                line: 5,
                severity: GutterIconSeverity::Warning,
            },
            GutterDiagnosticIcon {
                line: 5,
                severity: GutterIconSeverity::Error,
            },
            GutterDiagnosticIcon {
                line: 10,
                severity: GutterIconSeverity::Info,
            },
        ];
        coalesce_gutter_icons(&mut icons);
        assert_eq!(icons.len(), 2);
        assert_eq!(icons[0].line, 5);
        assert_eq!(icons[0].severity, GutterIconSeverity::Error);
        assert_eq!(icons[1].line, 10);
    }

    #[test]
    fn coalesce_empty_and_single() {
        let mut empty: Vec<GutterDiagnosticIcon> = vec![];
        coalesce_gutter_icons(&mut empty);
        assert!(empty.is_empty());

        let mut single = vec![GutterDiagnosticIcon {
            line: 1,
            severity: GutterIconSeverity::Error,
        }];
        coalesce_gutter_icons(&mut single);
        assert_eq!(single.len(), 1);
    }

    #[test]
    fn draw_all_does_not_panic() {
        let renderer = DiagnosticGutterRenderer::new();
        let mut rects = RectRenderer::new();
        let icons = vec![
            GutterDiagnosticIcon {
                line: 1,
                severity: GutterIconSeverity::Error,
            },
            GutterDiagnosticIcon {
                line: 3,
                severity: GutterIconSeverity::Warning,
            },
            GutterDiagnosticIcon {
                line: 5,
                severity: GutterIconSeverity::Info,
            },
            GutterDiagnosticIcon {
                line: 7,
                severity: GutterIconSeverity::Hint,
            },
        ];
        renderer.draw_all(&mut rects, &icons, 0.0, 1, 20.0, 0.0);
    }
}
