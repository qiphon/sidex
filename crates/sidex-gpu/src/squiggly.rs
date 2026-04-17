//! Squiggly underline renderer for diagnostic decorations.
//!
//! Generates wavy underline geometry with 2px amplitude, 4px wavelength, and
//! 1px thickness. Colors are driven by diagnostic severity.

use crate::color::Color;
use crate::rect_renderer::RectRenderer;

// ── Severity colors ─────────────────────────────────────────────────────────

/// Diagnostic severity level for the squiggly renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SquigglySeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl SquigglySeverity {
    /// Returns the underline color for this severity.
    pub fn color(self) -> Color {
        match self {
            Self::Error => Color::from_rgb(244, 71, 71),
            Self::Warning => Color::from_rgb(205, 173, 0),
            Self::Information => Color::from_rgb(55, 148, 255),
            Self::Hint => Color::from_rgb(160, 160, 160),
        }
    }

    /// Returns a dimmed (stale) variant of the severity color.
    pub fn stale_color(self) -> Color {
        let c = self.color();
        Color {
            r: c.r,
            g: c.g,
            b: c.b,
            a: 0.4,
        }
    }
}

// ── SquigglyDecoration ──────────────────────────────────────────────────────

/// A single squiggly underline to render.
#[derive(Debug, Clone, Copy)]
pub struct SquigglyDecoration {
    /// X pixel coordinate of the start of the underline.
    pub x: f32,
    /// Y pixel coordinate (baseline of the text line).
    pub y: f32,
    /// Width of the underline in pixels.
    pub width: f32,
    /// Diagnostic severity (determines color).
    pub severity: SquigglySeverity,
    /// Whether this diagnostic is stale (dimmed rendering).
    pub is_stale: bool,
}

// ── SquigglyRenderer ────────────────────────────────────────────────────────

/// Draws wavy underlines for diagnostics using small rect segments.
///
/// The wave is approximated by a sequence of small rectangles offset
/// vertically in a sine-like pattern: amplitude 2px, wavelength 4px,
/// stroke thickness 1px.
pub struct SquigglyRenderer {
    amplitude: f32,
    wavelength: f32,
    thickness: f32,
}

impl SquigglyRenderer {
    pub fn new() -> Self {
        Self {
            amplitude: 2.0,
            wavelength: 4.0,
            thickness: 1.0,
        }
    }

    /// Draws a single wavy underline at the specified position.
    pub fn draw_squiggly(&self, rects: &mut RectRenderer, x: f32, y: f32, width: f32, color: Color) {
        if width <= 0.0 {
            return;
        }

        let half_wave = self.wavelength / 2.0;
        let step = self.thickness.max(1.0);
        let mut cx = 0.0_f32;
        let mut segment_index = 0u32;

        while cx < width {
            let seg_width = step.min(width - cx);
            let phase = cx % self.wavelength;
            let dy = if phase < half_wave {
                let t = phase / half_wave;
                -self.amplitude * (1.0 - 2.0 * t)
            } else {
                let t = (phase - half_wave) / half_wave;
                self.amplitude * (1.0 - 2.0 * t)
            };

            rects.draw_rect(
                x + cx,
                y + dy,
                seg_width,
                self.thickness,
                color,
                0.0,
            );

            cx += step;
            segment_index += 1;
            let _ = segment_index;
        }
    }

    /// Draws a squiggly underline for a diagnostic decoration.
    pub fn draw_decoration(&self, rects: &mut RectRenderer, dec: &SquigglyDecoration) {
        let color = if dec.is_stale {
            dec.severity.stale_color()
        } else {
            dec.severity.color()
        };
        self.draw_squiggly(rects, dec.x, dec.y, dec.width, color);
    }

    /// Batch-draws all squiggly decorations.
    pub fn draw_all(&self, rects: &mut RectRenderer, decorations: &[SquigglyDecoration]) {
        for dec in decorations {
            self.draw_decoration(rects, dec);
        }
    }
}

impl Default for SquigglyRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_colors_are_opaque() {
        for severity in [
            SquigglySeverity::Error,
            SquigglySeverity::Warning,
            SquigglySeverity::Information,
            SquigglySeverity::Hint,
        ] {
            let c = severity.color();
            assert!((c.a - 1.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn stale_colors_have_reduced_alpha() {
        for severity in [
            SquigglySeverity::Error,
            SquigglySeverity::Warning,
            SquigglySeverity::Information,
            SquigglySeverity::Hint,
        ] {
            let c = severity.stale_color();
            assert!(c.a < 1.0);
        }
    }

    #[test]
    fn draw_squiggly_zero_width_is_noop() {
        let renderer = SquigglyRenderer::new();
        let mut rects = RectRenderer::new();
        renderer.draw_squiggly(&mut rects, 0.0, 0.0, 0.0, Color::WHITE);
        // Nothing panics, and the rect buffer is empty
    }

    #[test]
    fn draw_squiggly_negative_width_is_noop() {
        let renderer = SquigglyRenderer::new();
        let mut rects = RectRenderer::new();
        renderer.draw_squiggly(&mut rects, 0.0, 0.0, -10.0, Color::WHITE);
    }

    #[test]
    fn draw_all_processes_multiple_decorations() {
        let renderer = SquigglyRenderer::new();
        let mut rects = RectRenderer::new();
        let decorations = vec![
            SquigglyDecoration {
                x: 10.0,
                y: 100.0,
                width: 50.0,
                severity: SquigglySeverity::Error,
                is_stale: false,
            },
            SquigglyDecoration {
                x: 70.0,
                y: 100.0,
                width: 30.0,
                severity: SquigglySeverity::Warning,
                is_stale: true,
            },
        ];
        renderer.draw_all(&mut rects, &decorations);
    }
}
