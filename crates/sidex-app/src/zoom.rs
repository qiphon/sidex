//! Zoom management — UI-wide scale factor with persistence.

use serde::{Deserialize, Serialize};

/// Minimum zoom level (50 %).
const MIN_LEVEL: f32 = -5.0;
/// Maximum zoom level (500 %).
const MAX_LEVEL: f32 = 10.0;
/// Default step per zoom-in / zoom-out action.
const DEFAULT_STEP: f32 = 1.0;
/// Scale multiplier per zoom unit (10 % per step).
const SCALE_PER_UNIT: f32 = 0.1;

/// Manages the global zoom level for the entire UI. The level is an integer
/// that maps to a scale factor via `1.0 + level * 0.1`. Persisted in the
/// `window.zoomLevel` setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoomService {
    pub level: f32,
    pub min_level: f32,
    pub max_level: f32,
    pub step: f32,
}

impl Default for ZoomService {
    fn default() -> Self {
        Self::new()
    }
}

impl ZoomService {
    pub fn new() -> Self {
        Self {
            level: 0.0,
            min_level: MIN_LEVEL,
            max_level: MAX_LEVEL,
            step: DEFAULT_STEP,
        }
    }

    /// Restore a previously persisted zoom level (clamped to bounds).
    pub fn with_level(mut self, level: f32) -> Self {
        self.level = level.clamp(self.min_level, self.max_level);
        self
    }

    /// Increase the zoom level by one step.
    pub fn zoom_in(&mut self) {
        self.level = (self.level + self.step).min(self.max_level);
    }

    /// Decrease the zoom level by one step.
    pub fn zoom_out(&mut self) {
        self.level = (self.level - self.step).max(self.min_level);
    }

    /// Reset to the default zoom (level 0 → scale 1.0).
    pub fn reset(&mut self) {
        self.level = 0.0;
    }

    /// Set an exact zoom level, clamped to the configured bounds.
    pub fn set_level(&mut self, level: f32) {
        self.level = level.clamp(self.min_level, self.max_level);
    }

    /// The multiplicative scale factor corresponding to the current level.
    ///
    /// Level  0 → 1.0 (100 %)
    /// Level  1 → 1.1 (110 %)
    /// Level −2 → 0.8 ( 80 %)
    pub fn scale_factor(&self) -> f32 {
        (1.0 + self.level * SCALE_PER_UNIT).max(0.1)
    }

    /// Convenience: scale a base font size by the zoom factor.
    pub fn scaled_font_size(&self, base: f32) -> f32 {
        base * self.scale_factor()
    }

    /// Convenience: scale a line height by the zoom factor.
    pub fn scaled_line_height(&self, base: f32) -> f32 {
        (base * self.scale_factor()).round()
    }

    /// Human-readable percentage string (e.g. "110 %").
    pub fn display_percentage(&self) -> String {
        format!("{:.0} %", self.scale_factor() * 100.0)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_level_zero() {
        let z = ZoomService::new();
        assert!((z.level - 0.0).abs() < f32::EPSILON);
        assert!((z.scale_factor() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn zoom_in_increases() {
        let mut z = ZoomService::new();
        z.zoom_in();
        assert!((z.level - 1.0).abs() < f32::EPSILON);
        assert!((z.scale_factor() - 1.1).abs() < 0.001);
    }

    #[test]
    fn zoom_out_decreases() {
        let mut z = ZoomService::new();
        z.zoom_out();
        assert!((z.level - (-1.0)).abs() < f32::EPSILON);
        assert!((z.scale_factor() - 0.9).abs() < 0.001);
    }

    #[test]
    fn clamp_at_max() {
        let mut z = ZoomService::new();
        for _ in 0..20 {
            z.zoom_in();
        }
        assert!((z.level - z.max_level).abs() < f32::EPSILON);
    }

    #[test]
    fn clamp_at_min() {
        let mut z = ZoomService::new();
        for _ in 0..20 {
            z.zoom_out();
        }
        assert!((z.level - z.min_level).abs() < f32::EPSILON);
    }

    #[test]
    fn reset_to_zero() {
        let mut z = ZoomService::new();
        z.zoom_in();
        z.zoom_in();
        z.reset();
        assert!((z.level - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn set_level_clamps() {
        let mut z = ZoomService::new();
        z.set_level(100.0);
        assert!((z.level - z.max_level).abs() < f32::EPSILON);
        z.set_level(-100.0);
        assert!((z.level - z.min_level).abs() < f32::EPSILON);
    }

    #[test]
    fn scaled_font_size() {
        let mut z = ZoomService::new();
        z.set_level(2.0);
        let scaled = z.scaled_font_size(14.0);
        assert!((scaled - 16.8).abs() < 0.1);
    }

    #[test]
    fn display_percentage_format() {
        let z = ZoomService::new();
        assert_eq!(z.display_percentage(), "100 %");
    }

    #[test]
    fn with_level_clamps() {
        let z = ZoomService::new().with_level(999.0);
        assert!((z.level - z.max_level).abs() < f32::EPSILON);
    }

    #[test]
    fn negative_level_floor() {
        let z = ZoomService::new().with_level(-5.0);
        assert!(z.scale_factor() > 0.0);
    }
}
