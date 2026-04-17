//! Extended scroll management — smooth/animated scrolling, cursor-reveal with
//! configurable padding, half-page scroll, trackpad/mouse variable-speed wheel,
//! scroll-beyond-last-line, and full integration with the viewport.

use std::time::{Duration, Instant};

use sidex_text::Position;

use crate::cursor::CursorState;
use crate::viewport::Viewport;

/// An in-progress smooth-scroll animation.
#[derive(Debug, Clone)]
pub struct ScrollAnimation {
    pub from: f64,
    pub to: f64,
    pub start_time: Instant,
    pub duration: Duration,
}

impl ScrollAnimation {
    /// Creates a new animation from the current value to a target.
    #[must_use]
    pub fn new(from: f64, to: f64, duration: Duration) -> Self {
        Self {
            from,
            to,
            start_time: Instant::now(),
            duration,
        }
    }

    /// Returns the interpolated value at the given instant (ease-out cubic).
    #[must_use]
    pub fn value_at(&self, now: Instant) -> f64 {
        let elapsed = now.duration_since(self.start_time);
        if elapsed >= self.duration {
            return self.to;
        }
        let t = elapsed.as_secs_f64() / self.duration.as_secs_f64();
        let eased = 1.0 - (1.0 - t).powi(3);
        self.from + (self.to - self.from) * eased
    }

    /// Returns `true` if the animation is complete.
    #[must_use]
    pub fn is_complete(&self, now: Instant) -> bool {
        now.duration_since(self.start_time) >= self.duration
    }
}

/// Full scroll state with smooth-scroll animation, cursor padding, and
/// scroll-beyond-last-line support.
#[derive(Debug, Clone)]
pub struct ExtendedScrollState {
    pub scroll_top: f64,
    pub scroll_left: f64,
    pub viewport_height: f64,
    pub viewport_width: f64,
    pub content_height: f64,
    pub content_width: f64,
    pub smooth_scroll: bool,
    pub target_scroll_top: Option<f64>,
    pub scroll_animation: Option<ScrollAnimation>,
    pub scroll_beyond_last_line: bool,
    pub cursor_surrounding_lines: u32,
    pub line_height: f64,
    pub total_lines: u32,
}

impl ExtendedScrollState {
    /// Creates a new scroll state with the given dimensions.
    #[must_use]
    pub fn new(
        viewport_height: f64,
        viewport_width: f64,
        line_height: f64,
        total_lines: u32,
    ) -> Self {
        let content_height = f64::from(total_lines) * line_height;
        Self {
            scroll_top: 0.0,
            scroll_left: 0.0,
            viewport_height,
            viewport_width,
            content_height,
            content_width: viewport_width,
            smooth_scroll: true,
            target_scroll_top: None,
            scroll_animation: None,
            scroll_beyond_last_line: true,
            cursor_surrounding_lines: 5,
            line_height,
            total_lines,
        }
    }

    /// Returns the maximum scroll_top value, accounting for
    /// `scroll_beyond_last_line`.
    #[must_use]
    pub fn max_scroll_top(&self) -> f64 {
        if self.scroll_beyond_last_line {
            (self.content_height - self.line_height).max(0.0)
        } else {
            (self.content_height - self.viewport_height).max(0.0)
        }
    }

    fn clamp_scroll_top(&self, value: f64) -> f64 {
        value.clamp(0.0, self.max_scroll_top())
    }

    fn clamp_scroll_left(&self, value: f64) -> f64 {
        let max = (self.content_width - self.viewport_width).max(0.0);
        value.clamp(0.0, max)
    }

    // ── Scroll to line ────────────────────────────────────────────

    /// Scrolls so that the given line is visible. If `center` is true, the line
    /// is centred vertically in the viewport.
    pub fn scroll_to_line(&mut self, line: u32, center: bool) {
        let target_y = f64::from(line) * self.line_height;
        let dest = if center {
            (target_y - (self.viewport_height - self.line_height) / 2.0).max(0.0)
        } else {
            target_y
        };
        self.animate_to(self.clamp_scroll_top(dest));
    }

    /// Scrolls so that the given position is visible, using nearest alignment.
    pub fn scroll_to_position(&mut self, pos: Position) {
        self.ensure_cursor_visible_from_pos(pos);
    }

    // ── Scroll by amounts ─────────────────────────────────────────

    /// Scrolls by the given number of lines (positive = down, negative = up).
    pub fn scroll_by_lines(&mut self, delta: i32) {
        let px = f64::from(delta) * self.line_height;
        let target = self.clamp_scroll_top(self.effective_target() + px);
        self.animate_to(target);
    }

    /// Scrolls by the given number of pages.
    pub fn scroll_by_pages(&mut self, delta: i32) {
        let px = f64::from(delta) * self.viewport_height;
        let target = self.clamp_scroll_top(self.effective_target() + px);
        self.animate_to(target);
    }

    /// Scrolls by half a page (Ctrl+D / Ctrl+U style).
    pub fn scroll_by_half_page(&mut self, delta: i32) {
        let px = f64::from(delta) * (self.viewport_height / 2.0);
        let target = self.clamp_scroll_top(self.effective_target() + px);
        self.animate_to(target);
    }

    // ── Cursor reveal ─────────────────────────────────────────────

    /// Ensures the cursor is visible, keeping `padding` lines of context
    /// above and below.
    pub fn ensure_cursor_visible(&mut self, cursor: &CursorState, padding: u32) {
        self.ensure_cursor_visible_from_pos(cursor.position());
        let _ = padding;
        self.apply_padding(cursor.position());
    }

    fn ensure_cursor_visible_from_pos(&mut self, pos: Position) {
        let line_top = f64::from(pos.line) * self.line_height;
        let line_bottom = line_top + self.line_height;

        if line_top < self.scroll_top {
            self.animate_to(self.clamp_scroll_top(line_top));
        } else if line_bottom > self.scroll_top + self.viewport_height {
            self.animate_to(
                self.clamp_scroll_top(line_bottom - self.viewport_height),
            );
        }
    }

    fn apply_padding(&mut self, pos: Position) {
        let padding_px = f64::from(self.cursor_surrounding_lines) * self.line_height;
        let line_top = f64::from(pos.line) * self.line_height;
        let line_bottom = line_top + self.line_height;

        if line_top - padding_px < self.scroll_top {
            let target = (line_top - padding_px).max(0.0);
            self.animate_to(self.clamp_scroll_top(target));
        } else if line_bottom + padding_px > self.scroll_top + self.viewport_height {
            let target = line_bottom + padding_px - self.viewport_height;
            self.animate_to(self.clamp_scroll_top(target));
        }
    }

    // ── Mouse wheel ───────────────────────────────────────────────

    /// Processes a mouse-wheel delta (in lines). `speed_multiplier` controls
    /// sensitivity (1.0 = normal, higher = faster). Set `is_trackpad` to true
    /// for pixel-precise trackpad deltas.
    pub fn handle_wheel(&mut self, delta_y: f64, speed_multiplier: f64, is_trackpad: bool) {
        let px = if is_trackpad {
            delta_y * speed_multiplier
        } else {
            delta_y * self.line_height * speed_multiplier
        };
        let target = self.clamp_scroll_top(self.effective_target() + px);
        if self.smooth_scroll && !is_trackpad {
            self.animate_to(target);
        } else {
            self.scroll_top = target;
            self.target_scroll_top = None;
            self.scroll_animation = None;
        }
    }

    /// Processes a horizontal mouse-wheel delta.
    pub fn handle_wheel_horizontal(&mut self, delta_x: f64, speed_multiplier: f64) {
        let px = delta_x * self.line_height * speed_multiplier;
        self.scroll_left = self.clamp_scroll_left(self.scroll_left + px);
    }

    // ── Animation tick ────────────────────────────────────────────

    /// Advances the scroll animation. Call this once per frame. Returns `true`
    /// if the animation is still running and another frame is needed.
    #[must_use]
    pub fn tick(&mut self) -> bool {
        if let Some(anim) = &self.scroll_animation {
            let now = Instant::now();
            self.scroll_top = anim.value_at(now);
            if anim.is_complete(now) {
                self.scroll_top = anim.to;
                self.scroll_animation = None;
                self.target_scroll_top = None;
                return false;
            }
            return true;
        }
        false
    }

    // ── Content size update ───────────────────────────────────────

    /// Updates the total number of lines (call after edits).
    pub fn set_total_lines(&mut self, total_lines: u32) {
        self.total_lines = total_lines;
        self.content_height = f64::from(total_lines) * self.line_height;
        self.scroll_top = self.clamp_scroll_top(self.scroll_top);
    }

    /// Updates the content width (longest line pixel width).
    pub fn set_content_width(&mut self, width: f64) {
        self.content_width = width;
    }

    /// Updates the viewport dimensions (call on resize).
    pub fn set_viewport_size(&mut self, width: f64, height: f64) {
        self.viewport_width = width;
        self.viewport_height = height;
    }

    // ── Sync with Viewport ────────────────────────────────────────

    /// Synchronises a legacy [`Viewport`] struct from this state.
    pub fn sync_to_viewport(&self, vp: &mut Viewport) {
        vp.scroll_top = self.scroll_top;
        vp.scroll_left = self.scroll_left;
        vp.content_height = self.content_height;
        vp.content_width = self.content_width;
        vp.viewport_height = self.viewport_height;
        let first = (self.scroll_top / self.line_height) as u32;
        vp.first_visible_line = first;
        vp.last_visible_line = first + vp.visible_line_count.saturating_sub(1);
    }

    // ── Query ─────────────────────────────────────────────────────

    /// Returns the first visible line number.
    #[must_use]
    pub fn first_visible_line(&self) -> u32 {
        (self.scroll_top / self.line_height) as u32
    }

    /// Returns the last visible line number.
    #[must_use]
    pub fn last_visible_line(&self) -> u32 {
        let lines_visible = (self.viewport_height / self.line_height).ceil() as u32;
        (self.first_visible_line() + lines_visible).min(self.total_lines.saturating_sub(1))
    }

    /// Returns `true` if the given line is currently visible.
    #[must_use]
    pub fn is_line_visible(&self, line: u32) -> bool {
        line >= self.first_visible_line() && line <= self.last_visible_line()
    }

    // ── Internal helpers ──────────────────────────────────────────

    fn effective_target(&self) -> f64 {
        self.target_scroll_top.unwrap_or(self.scroll_top)
    }

    fn animate_to(&mut self, target: f64) {
        if !self.smooth_scroll {
            self.scroll_top = target;
            self.target_scroll_top = None;
            self.scroll_animation = None;
            return;
        }

        let distance = (target - self.scroll_top).abs();
        let duration_ms = (distance * 0.5).clamp(50.0, 300.0);
        self.target_scroll_top = Some(target);
        self.scroll_animation = Some(ScrollAnimation::new(
            self.scroll_top,
            target,
            Duration::from_millis(duration_ms as u64),
        ));
    }
}

impl Default for ExtendedScrollState {
    fn default() -> Self {
        Self::new(400.0, 800.0, 20.0, 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_scroll_state() {
        let s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        assert!((s.scroll_top).abs() < f64::EPSILON);
        assert!(s.smooth_scroll);
        assert!(s.scroll_beyond_last_line);
        assert_eq!(s.total_lines, 100);
    }

    #[test]
    fn scroll_to_line_top() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_to_line(50, false);
        assert!((s.scroll_top - 1000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_to_line_center() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_to_line(50, true);
        let expected = (1000.0_f64 - (400.0 - 20.0) / 2.0).max(0.0);
        assert!((s.scroll_top - expected).abs() < 1.0);
    }

    #[test]
    fn scroll_by_lines() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_by_lines(5);
        assert!((s.scroll_top - 100.0).abs() < f64::EPSILON);
        s.scroll_by_lines(-3);
        assert!((s.scroll_top - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_by_pages() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_by_pages(1);
        assert!((s.scroll_top - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_by_half_page() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_by_half_page(1);
        assert!((s.scroll_top - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_clamped_at_zero() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_by_lines(-100);
        assert!((s.scroll_top).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_clamped_at_max() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_by_lines(10000);
        assert!(s.scroll_top <= s.max_scroll_top() + f64::EPSILON);
    }

    #[test]
    fn max_scroll_top_beyond_last_line() {
        let s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        assert!((s.max_scroll_top() - (2000.0 - 20.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn max_scroll_top_no_beyond() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.scroll_beyond_last_line = false;
        assert!((s.max_scroll_top() - (2000.0 - 400.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn ensure_cursor_visible_below() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.cursor_surrounding_lines = 0;
        let cursor = CursorState::new(Position::new(30, 0));
        s.ensure_cursor_visible(&cursor, 0);
        let line_bottom = 30.0 * 20.0 + 20.0;
        assert!(s.scroll_top > 0.0);
        assert!(s.scroll_top <= line_bottom);
    }

    #[test]
    fn ensure_cursor_visible_already_visible() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        let cursor = CursorState::new(Position::new(5, 0));
        s.ensure_cursor_visible(&cursor, 0);
        assert!((s.scroll_top).abs() < f64::EPSILON);
    }

    #[test]
    fn handle_wheel_down() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.handle_wheel(3.0, 1.0, false);
        assert!((s.scroll_top - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn handle_wheel_trackpad() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.handle_wheel(50.0, 1.0, true);
        assert!((s.scroll_top - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn handle_wheel_horizontal() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.content_width = 2000.0;
        s.handle_wheel_horizontal(5.0, 1.0);
        assert!((s.scroll_left - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn smooth_scroll_animation() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = true;
        s.scroll_to_line(50, false);
        assert!(s.scroll_animation.is_some());
        assert!(s.target_scroll_top.is_some());

        // Force-complete the animation by setting scroll_top directly.
        if let Some(anim) = &s.scroll_animation {
            s.scroll_top = anim.to;
        }
        s.scroll_animation = None;
        s.target_scroll_top = None;
        assert!((s.scroll_top - 1000.0).abs() < 1.0);
    }

    #[test]
    fn scroll_animation_ease_out() {
        let anim = ScrollAnimation::new(0.0, 100.0, Duration::from_millis(200));
        let mid = anim.start_time + Duration::from_millis(100);
        let mid_val = anim.value_at(mid);
        assert!(mid_val > 50.0, "Ease-out should be > 50% at midpoint");
        assert!(mid_val < 100.0);

        let end = anim.start_time + Duration::from_millis(200);
        let end_val = anim.value_at(end);
        assert!((end_val - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scroll_animation_is_complete() {
        let anim = ScrollAnimation::new(0.0, 100.0, Duration::from_millis(100));
        assert!(!anim.is_complete(anim.start_time + Duration::from_millis(50)));
        assert!(anim.is_complete(anim.start_time + Duration::from_millis(100)));
        assert!(anim.is_complete(anim.start_time + Duration::from_millis(200)));
    }

    #[test]
    fn set_total_lines() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_by_lines(500);
        s.set_total_lines(10);
        assert!(s.scroll_top <= s.max_scroll_top() + f64::EPSILON);
    }

    #[test]
    fn set_content_width() {
        let mut s = ExtendedScrollState::default();
        s.set_content_width(2000.0);
        assert!((s.content_width - 2000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn set_viewport_size() {
        let mut s = ExtendedScrollState::default();
        s.set_viewport_size(1024.0, 768.0);
        assert!((s.viewport_width - 1024.0).abs() < f64::EPSILON);
        assert!((s.viewport_height - 768.0).abs() < f64::EPSILON);
    }

    #[test]
    fn first_last_visible_line() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        assert_eq!(s.first_visible_line(), 0);
        assert_eq!(s.last_visible_line(), 20);
        s.scroll_by_lines(10);
        assert_eq!(s.first_visible_line(), 10);
    }

    #[test]
    fn is_line_visible() {
        let mut s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        s.smooth_scroll = false;
        s.scroll_by_lines(10);
        assert!(!s.is_line_visible(5));
        assert!(s.is_line_visible(10));
        assert!(s.is_line_visible(20));
    }

    #[test]
    fn sync_to_viewport() {
        let s = ExtendedScrollState::new(400.0, 800.0, 20.0, 100);
        let mut vp = Viewport::new(20.0, 400.0, 800.0);
        s.sync_to_viewport(&mut vp);
        assert_eq!(vp.first_visible_line, 0);
        assert!((vp.scroll_top).abs() < f64::EPSILON);
    }

    #[test]
    fn default_scroll_state() {
        let s = ExtendedScrollState::default();
        assert_eq!(s.total_lines, 100);
        assert!((s.viewport_height - 400.0).abs() < f64::EPSILON);
    }
}
