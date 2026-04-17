//! Viewport management — tracks what portion of the document is visible,
//! provides smooth scrolling, scrollbar geometry, scroll shadow, and mouse
//! wheel processing.  Inspired by VS Code's `ScrollableElement` and Zed's
//! `scroll` module.

use serde::{Deserialize, Serialize};
use sidex_text::Position;

// ---------------------------------------------------------------------------
// ScrollAlign
// ---------------------------------------------------------------------------

/// How to align a target line within the viewport when scrolling to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollAlign {
    Top,
    Center,
    Bottom,
    Nearest,
}

// ---------------------------------------------------------------------------
// ScrollSettings
// ---------------------------------------------------------------------------

/// User-configurable scroll behaviour knobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrollSettings {
    pub smooth_scrolling: bool,
    pub mouse_wheel_scroll_sensitivity: f64,
    pub fast_scroll_sensitivity: f64,
}

impl Default for ScrollSettings {
    fn default() -> Self {
        Self {
            smooth_scrolling: true,
            mouse_wheel_scroll_sensitivity: 1.0,
            fast_scroll_sensitivity: 5.0,
        }
    }
}

// ---------------------------------------------------------------------------
// ScrollState
// ---------------------------------------------------------------------------

/// Full scroll model with smooth-scroll animation state.
#[derive(Debug, Clone)]
pub struct ScrollState {
    pub scroll_x: f64,
    pub scroll_y: f64,
    pub target_x: f64,
    pub target_y: f64,
    pub velocity_x: f64,
    pub velocity_y: f64,
    pub is_animating: bool,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            target_x: 0.0,
            target_y: 0.0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            is_animating: false,
        }
    }
}

const SMOOTH_SCROLL_SPEED: f64 = 8.0;
const SETTLE_THRESHOLD: f64 = 0.5;

impl ScrollState {
    /// Initiate a smooth scroll animation towards `target_y`.
    pub fn smooth_scroll_to(&mut self, target_y: f64) {
        self.target_y = target_y.max(0.0);
        self.is_animating = true;
    }

    /// Jump immediately to the given vertical offset.
    pub fn instant_scroll_to(&mut self, y: f64) {
        let y = y.max(0.0);
        self.scroll_y = y;
        self.target_y = y;
        self.velocity_y = 0.0;
        self.is_animating = false;
    }

    /// Advance the smooth-scroll animation by `dt` seconds (ease-out).
    pub fn tick(&mut self, dt: f64) {
        if !self.is_animating {
            return;
        }
        let factor = (SMOOTH_SCROLL_SPEED * dt).min(1.0);

        let diff_y = self.target_y - self.scroll_y;
        if diff_y.abs() < SETTLE_THRESHOLD {
            self.scroll_y = self.target_y;
            self.velocity_y = 0.0;
        } else {
            self.velocity_y = diff_y * factor / dt.max(1e-6);
            self.scroll_y += diff_y * factor;
        }

        let diff_x = self.target_x - self.scroll_x;
        if diff_x.abs() < SETTLE_THRESHOLD {
            self.scroll_x = self.target_x;
            self.velocity_x = 0.0;
        } else {
            self.velocity_x = diff_x * factor / dt.max(1e-6);
            self.scroll_x += diff_x * factor;
        }

        if (self.scroll_y - self.target_y).abs() < SETTLE_THRESHOLD
            && (self.scroll_x - self.target_x).abs() < SETTLE_THRESHOLD
        {
            self.scroll_y = self.target_y;
            self.scroll_x = self.target_x;
            self.velocity_x = 0.0;
            self.velocity_y = 0.0;
            self.is_animating = false;
        }
    }

    /// Scroll relative to current position (e.g. mouse wheel delta).
    pub fn scroll_by(&mut self, dx: f64, dy: f64) {
        self.target_x = (self.target_x + dx).max(0.0);
        self.target_y = (self.target_y + dy).max(0.0);
        if self.is_animating {
            return;
        }
        self.scroll_x = self.target_x;
        self.scroll_y = self.target_y;
    }

    /// Scroll so that `line` is visible according to `align`.
    pub fn scroll_to_line(
        &mut self,
        line: u32,
        align: ScrollAlign,
        line_height: f64,
        viewport_height: f64,
    ) {
        let line_y = f64::from(line) * line_height;
        let target = match align {
            ScrollAlign::Top => line_y,
            ScrollAlign::Center => (line_y - (viewport_height - line_height) / 2.0).max(0.0),
            ScrollAlign::Bottom => (line_y - viewport_height + line_height).max(0.0),
            ScrollAlign::Nearest => {
                if line_y < self.scroll_y {
                    line_y
                } else if line_y + line_height > self.scroll_y + viewport_height {
                    (line_y + line_height - viewport_height).max(0.0)
                } else {
                    return;
                }
            }
        };
        self.smooth_scroll_to(target);
    }

    /// Auto-scroll to keep cursor visible (called after cursor movement).
    pub fn ensure_position_visible(
        &mut self,
        pos: Position,
        line_height: f64,
        viewport_height: f64,
    ) {
        let line_top = f64::from(pos.line) * line_height;
        let line_bottom = line_top + line_height;

        if line_top < self.scroll_y {
            self.instant_scroll_to(line_top);
        } else if line_bottom > self.scroll_y + viewport_height {
            self.instant_scroll_to(line_bottom - viewport_height);
        }
    }
}

// ---------------------------------------------------------------------------
// Content dimension helpers
// ---------------------------------------------------------------------------

/// Total scrollable height given the document size.
#[must_use]
pub fn content_height(total_lines: u32, line_height: f64) -> f64 {
    f64::from(total_lines) * line_height
}

/// Total scrollable width (content width with a small right margin).
#[must_use]
pub fn content_width(max_line_width: f64) -> f64 {
    max_line_width + 32.0
}

// ---------------------------------------------------------------------------
// Scrollbar geometry helpers
// ---------------------------------------------------------------------------

/// Size of the scrollbar thumb in pixels.
#[must_use]
pub fn scrollbar_thumb_size(viewport_height: f64, ch: f64) -> f64 {
    if ch <= 0.0 {
        return 0.0;
    }
    let ratio = viewport_height / ch;
    (viewport_height * ratio).max(30.0).min(viewport_height)
}

/// Position of the scrollbar thumb along the track.
#[must_use]
pub fn scrollbar_thumb_position(scroll_y: f64, viewport_height: f64, ch: f64) -> f64 {
    let thumb = scrollbar_thumb_size(viewport_height, ch);
    let scroll_range = ch - viewport_height;
    let track_range = viewport_height - thumb;
    if scroll_range <= 0.0 || track_range <= 0.0 {
        return 0.0;
    }
    (scroll_y / scroll_range) * track_range
}

/// Convert a click on the scrollbar track to a scroll-y position.
#[must_use]
pub fn scrollbar_click_to_scroll(click_y: f64, viewport_height: f64, ch: f64) -> f64 {
    let thumb = scrollbar_thumb_size(viewport_height, ch);
    let track_range = viewport_height - thumb;
    let scroll_range = ch - viewport_height;
    if track_range <= 0.0 || scroll_range <= 0.0 {
        return 0.0;
    }
    let centered = (click_y - thumb / 2.0).clamp(0.0, track_range);
    (centered / track_range) * scroll_range
}

// ---------------------------------------------------------------------------
// Scroll shadow
// ---------------------------------------------------------------------------

/// Whether to display the scroll shadow at the top of the editor.
#[must_use]
pub fn should_show_scroll_shadow(scroll_y: f64) -> bool {
    scroll_y > 1.0
}

/// Shadow opacity (0.0..=1.0) that fades in proportional to scroll distance.
#[must_use]
pub fn scroll_shadow_opacity(scroll_y: f64) -> f64 {
    if scroll_y <= 0.0 {
        0.0
    } else {
        (scroll_y / 50.0).min(1.0)
    }
}

// ---------------------------------------------------------------------------
// Mouse wheel handling
// ---------------------------------------------------------------------------

/// Process a raw wheel event and return the `(dx, dy)` pixel deltas to apply.
///
/// * Pixel-based deltas (trackpad) are used as-is after sensitivity scaling.
/// * Line-based deltas (mouse wheel) are multiplied by `line_height`.
/// * Alt+scroll applies the fast-scroll multiplier.
#[must_use]
pub fn process_wheel_event(
    delta_x: f64,
    delta_y: f64,
    line_height: f64,
    settings: &ScrollSettings,
) -> (f64, f64) {
    let sens = settings.mouse_wheel_scroll_sensitivity;
    let dx = delta_x * line_height * sens;
    let dy = delta_y * line_height * sens;
    (dx, dy)
}

/// Same as [`process_wheel_event`] but with fast-scroll (Alt held).
#[must_use]
pub fn process_wheel_event_fast(
    delta_x: f64,
    delta_y: f64,
    line_height: f64,
    settings: &ScrollSettings,
) -> (f64, f64) {
    let (dx, dy) = process_wheel_event(delta_x, delta_y, line_height, settings);
    (dx * settings.fast_scroll_sensitivity, dy * settings.fast_scroll_sensitivity)
}

// ---------------------------------------------------------------------------
// Viewport (retained from original API for backwards compat)
// ---------------------------------------------------------------------------

/// Represents the visible area of the document on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewport {
    pub first_visible_line: u32,
    pub last_visible_line: u32,
    pub scroll_top: f64,
    pub scroll_left: f64,
    pub visible_line_count: u32,
    pub content_width: f64,
    pub content_height: f64,
    pub line_height: f64,
    pub viewport_height: f64,
}

impl Viewport {
    pub fn new(line_height: f64, viewport_height: f64, viewport_width: f64) -> Self {
        let visible = lines_per_page(line_height, viewport_height);
        Self {
            first_visible_line: 0,
            last_visible_line: visible.saturating_sub(1),
            scroll_top: 0.0,
            scroll_left: 0.0,
            visible_line_count: visible,
            content_width: viewport_width,
            content_height: 0.0,
            line_height,
            viewport_height,
        }
    }

    pub fn scroll_to_line(&mut self, line: u32) {
        self.first_visible_line = line;
        self.last_visible_line = line + self.visible_line_count.saturating_sub(1);
        self.scroll_top = f64::from(line) * self.line_height;
    }

    pub fn scroll_to_position(&mut self, pos: Position) {
        let center_offset = self.visible_line_count / 2;
        let target = pos.line.saturating_sub(center_offset);
        self.scroll_to_line(target);
    }

    pub fn ensure_visible(&mut self, pos: Position) {
        if pos.line < self.first_visible_line {
            self.scroll_to_line(pos.line);
        } else if pos.line > self.last_visible_line {
            let new_first = pos
                .line
                .saturating_sub(self.visible_line_count.saturating_sub(1));
            self.scroll_to_line(new_first);
        }
    }

    pub fn is_line_visible(&self, line: u32) -> bool {
        line >= self.first_visible_line && line <= self.last_visible_line
    }

    pub fn is_position_visible(&self, pos: Position) -> bool {
        self.is_line_visible(pos.line)
    }

    pub fn set_content_size(&mut self, width: f64, height: f64) {
        self.content_width = width;
        self.content_height = height;
    }

    pub fn scroll_by(&mut self, delta_y: f64, delta_x: f64) {
        self.scroll_top = (self.scroll_top + delta_y).max(0.0);
        self.scroll_left = (self.scroll_left + delta_x).max(0.0);
        self.first_visible_line = (self.scroll_top / self.line_height) as u32;
        self.last_visible_line =
            self.first_visible_line + self.visible_line_count.saturating_sub(1);
    }

    pub fn page_up(&mut self) {
        let delta = -(self.viewport_height);
        self.scroll_by(delta, 0.0);
    }

    pub fn page_down(&mut self) {
        self.scroll_by(self.viewport_height, 0.0);
    }

    /// Synchronise this viewport from a [`ScrollState`].
    pub fn sync_from_scroll_state(&mut self, state: &ScrollState) {
        self.scroll_top = state.scroll_y;
        self.scroll_left = state.scroll_x;
        self.first_visible_line = (self.scroll_top / self.line_height) as u32;
        self.last_visible_line =
            self.first_visible_line + self.visible_line_count.saturating_sub(1);
    }
}

/// Calculates how many full lines fit in the viewport.
pub fn lines_per_page(line_height: f64, viewport_height: f64) -> u32 {
    if line_height <= 0.0 {
        return 0;
    }
    (viewport_height / line_height).floor() as u32
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_per_page_basic() {
        assert_eq!(lines_per_page(20.0, 400.0), 20);
        assert_eq!(lines_per_page(18.0, 400.0), 22);
    }

    #[test]
    fn lines_per_page_zero_height() {
        assert_eq!(lines_per_page(0.0, 400.0), 0);
        assert_eq!(lines_per_page(-1.0, 400.0), 0);
    }

    #[test]
    fn new_viewport() {
        let vp = Viewport::new(20.0, 400.0, 800.0);
        assert_eq!(vp.first_visible_line, 0);
        assert_eq!(vp.visible_line_count, 20);
        assert_eq!(vp.last_visible_line, 19);
        assert!((vp.scroll_top).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_to_line() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.scroll_to_line(10);
        assert_eq!(vp.first_visible_line, 10);
        assert_eq!(vp.last_visible_line, 29);
        assert!((vp.scroll_top - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_to_position() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.scroll_to_position(Position::new(50, 0));
        assert!(vp.first_visible_line <= 50);
        assert!(vp.last_visible_line >= 50);
    }

    #[test]
    fn ensure_visible_already_visible() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.scroll_to_line(0);
        let old_first = vp.first_visible_line;
        vp.ensure_visible(Position::new(5, 0));
        assert_eq!(vp.first_visible_line, old_first);
    }

    #[test]
    fn ensure_visible_below() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.scroll_to_line(0);
        vp.ensure_visible(Position::new(50, 0));
        assert!(vp.first_visible_line > 0);
        assert!(vp.last_visible_line >= 50);
    }

    #[test]
    fn ensure_visible_above() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.scroll_to_line(100);
        vp.ensure_visible(Position::new(50, 0));
        assert_eq!(vp.first_visible_line, 50);
    }

    #[test]
    fn is_line_visible() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.scroll_to_line(10);
        assert!(vp.is_line_visible(10));
        assert!(vp.is_line_visible(29));
        assert!(!vp.is_line_visible(9));
        assert!(!vp.is_line_visible(30));
    }

    #[test]
    fn scroll_by() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.scroll_by(100.0, 50.0);
        assert_eq!(vp.first_visible_line, 5);
        assert!((vp.scroll_left - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_by_negative_clamped() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.scroll_by(-999.0, -999.0);
        assert!((vp.scroll_top).abs() < f64::EPSILON);
        assert!((vp.scroll_left).abs() < f64::EPSILON);
    }

    #[test]
    fn page_up_down() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.page_down();
        assert_eq!(vp.first_visible_line, 20);
        vp.page_up();
        assert_eq!(vp.first_visible_line, 0);
    }

    #[test]
    fn set_content_size() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        vp.set_content_size(1200.0, 10000.0);
        assert!((vp.content_width - 1200.0).abs() < f64::EPSILON);
        assert!((vp.content_height - 10000.0).abs() < f64::EPSILON);
    }

    // -- ScrollState tests --

    #[test]
    fn scroll_state_instant() {
        let mut s = ScrollState::default();
        s.instant_scroll_to(100.0);
        assert!((s.scroll_y - 100.0).abs() < f64::EPSILON);
        assert!(!s.is_animating);
    }

    #[test]
    fn scroll_state_smooth_converges() {
        let mut s = ScrollState::default();
        s.smooth_scroll_to(500.0);
        assert!(s.is_animating);
        for _ in 0..200 {
            s.tick(1.0 / 60.0);
        }
        assert!(!s.is_animating);
        assert!((s.scroll_y - 500.0).abs() < 1.0);
    }

    #[test]
    fn scroll_state_scroll_by() {
        let mut s = ScrollState::default();
        s.scroll_by(10.0, 50.0);
        assert!((s.scroll_x - 10.0).abs() < f64::EPSILON);
        assert!((s.scroll_y - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_state_scroll_to_line_center() {
        let mut s = ScrollState::default();
        s.scroll_to_line(50, ScrollAlign::Center, 20.0, 400.0);
        assert!(s.is_animating);
        assert!(s.target_y > 0.0);
    }

    #[test]
    fn scroll_state_ensure_position_visible() {
        let mut s = ScrollState::default();
        // line 30 at line_height=20: line_bottom = 30*20+20 = 620
        // viewport=400 so target = 620-400 = 220
        s.ensure_position_visible(Position::new(30, 0), 20.0, 400.0);
        assert!((s.scroll_y - 220.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_state_ensure_position_already_visible() {
        let mut s = ScrollState::default();
        s.ensure_position_visible(Position::new(5, 0), 20.0, 400.0);
        assert!((s.scroll_y).abs() < f64::EPSILON);
    }

    // -- helper function tests --

    #[test]
    fn content_height_basic() {
        assert!((content_height(100, 20.0) - 2000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn content_width_basic() {
        assert!((content_width(800.0) - 832.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_size_basic() {
        let sz = scrollbar_thumb_size(400.0, 2000.0);
        assert!(sz >= 30.0);
        assert!(sz <= 400.0);
    }

    #[test]
    fn thumb_position_at_zero() {
        let pos = scrollbar_thumb_position(0.0, 400.0, 2000.0);
        assert!(pos.abs() < f64::EPSILON);
    }

    #[test]
    fn scrollbar_click_roundtrip() {
        let ch = 2000.0;
        let vh = 400.0;
        let scroll = 800.0;
        let thumb_pos = scrollbar_thumb_position(scroll, vh, ch);
        let thumb_center = thumb_pos + scrollbar_thumb_size(vh, ch) / 2.0;
        let recovered = scrollbar_click_to_scroll(thumb_center, vh, ch);
        assert!((recovered - scroll).abs() < 2.0);
    }

    #[test]
    fn scroll_shadow_off_at_top() {
        assert!(!should_show_scroll_shadow(0.0));
        assert!(!should_show_scroll_shadow(0.5));
    }

    #[test]
    fn scroll_shadow_on_when_scrolled() {
        assert!(should_show_scroll_shadow(10.0));
    }

    #[test]
    fn scroll_shadow_opacity_range() {
        assert!((scroll_shadow_opacity(0.0)).abs() < f64::EPSILON);
        assert!((scroll_shadow_opacity(50.0) - 1.0).abs() < f64::EPSILON);
        assert!(scroll_shadow_opacity(25.0) > 0.0);
        assert!(scroll_shadow_opacity(25.0) < 1.0);
    }

    #[test]
    fn process_wheel_basic() {
        let settings = ScrollSettings::default();
        let (dx, dy) = process_wheel_event(0.0, 3.0, 20.0, &settings);
        assert!((dy - 60.0).abs() < f64::EPSILON);
        assert!(dx.abs() < f64::EPSILON);
    }

    #[test]
    fn process_wheel_fast() {
        let settings = ScrollSettings::default();
        let (_, dy) = process_wheel_event_fast(0.0, 3.0, 20.0, &settings);
        assert!((dy - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn viewport_sync_from_scroll_state() {
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        let mut s = ScrollState::default();
        s.instant_scroll_to(200.0);
        vp.sync_from_scroll_state(&s);
        assert_eq!(vp.first_visible_line, 10);
        assert!((vp.scroll_top - 200.0).abs() < f64::EPSILON);
    }
}
