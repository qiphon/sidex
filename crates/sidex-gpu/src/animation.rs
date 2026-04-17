//! GPU animation system for smooth visual transitions.
//!
//! Provides:
//! - [`EasingFunction`] — standard easing curves including cubic bezier.
//! - [`AnimationState`] — a single animation's progress and parameters.
//! - [`ActiveAnimation`] — a named, timed animation with from/to values.
//! - [`Animator`] — manages a set of active animations, advancing them
//!   each frame and providing interpolated values.
//!
//! Used for cursor blink, smooth scrolling, cursor movement, and widget
//! fade/slide transitions.

use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Easing functions
// ---------------------------------------------------------------------------

/// Standard easing curves for animations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// Custom cubic bezier curve with two control points:
    /// `CubicBezier(x1, y1, x2, y2)`.
    CubicBezier(f32, f32, f32, f32),
}

impl EasingFunction {
    /// Evaluates the easing function at time `t` (clamped to `[0, 1]`).
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t * t,
            Self::EaseOut => {
                let inv = 1.0 - t;
                1.0 - inv * inv * inv
            }
            Self::EaseInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let p = -2.0 * t + 2.0;
                    1.0 - p * p * p / 2.0
                }
            }
            Self::CubicBezier(x1, y1, x2, y2) => {
                cubic_bezier_evaluate(t, *x1, *y1, *x2, *y2)
            }
        }
    }
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::EaseInOut
    }
}

/// Approximate evaluation of a cubic bezier timing function.
///
/// Uses Newton's method to find the `t` parameter on the bezier X curve
/// that corresponds to the input `x`, then evaluates the Y curve at that `t`.
fn cubic_bezier_evaluate(x: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let mut t = x;
    for _ in 0..8 {
        let bx = bezier_component(t, x1, x2) - x;
        let dx = bezier_derivative(t, x1, x2);
        if dx.abs() < 1e-6 {
            break;
        }
        t -= bx / dx;
        t = t.clamp(0.0, 1.0);
    }
    bezier_component(t, y1, y2)
}

fn bezier_component(t: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - t;
    3.0 * inv * inv * t * p1 + 3.0 * inv * t * t * p2 + t * t * t
}

fn bezier_derivative(t: f32, p1: f32, p2: f32) -> f32 {
    let inv = 1.0 - t;
    3.0 * inv * inv * p1 + 6.0 * inv * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
}

// ---------------------------------------------------------------------------
// AnimationState — lightweight stateless progress
// ---------------------------------------------------------------------------

/// Lightweight animation progress descriptor.
#[derive(Debug, Clone, Copy)]
pub struct AnimationState {
    /// Current normalised time (0.0 = start, 1.0 = end).
    pub t: f32,
    /// Total duration in seconds.
    pub duration: f32,
    /// Easing curve.
    pub easing: EasingFunction,
}

impl AnimationState {
    pub fn new(duration: f32, easing: EasingFunction) -> Self {
        Self {
            t: 0.0,
            duration,
            easing,
        }
    }

    /// Advances by `dt` seconds and returns the eased value.
    pub fn advance(&mut self, dt: f32) -> f32 {
        if self.duration > 0.0 {
            self.t = (self.t + dt / self.duration).min(1.0);
        } else {
            self.t = 1.0;
        }
        self.easing.evaluate(self.t)
    }

    /// Returns the current eased value without advancing.
    pub fn value(&self) -> f32 {
        self.easing.evaluate(self.t)
    }

    /// Whether the animation has reached its end.
    pub fn is_finished(&self) -> bool {
        self.t >= 1.0
    }

    /// Resets to the beginning.
    pub fn reset(&mut self) {
        self.t = 0.0;
    }
}

// ---------------------------------------------------------------------------
// ActiveAnimation — a named, timed animation with from/to
// ---------------------------------------------------------------------------

/// A running animation that interpolates between two values over time.
#[derive(Debug, Clone)]
pub struct ActiveAnimation {
    /// Unique identifier for this animation (e.g. `"cursor_blink"`,
    /// `"scroll_y"`, `"cursor_move_0"`).
    pub id: String,
    /// When the animation started.
    pub start_time: Instant,
    /// How long the animation lasts.
    pub duration: Duration,
    /// Easing curve.
    pub easing: EasingFunction,
    /// Starting value.
    pub from: f32,
    /// Ending value.
    pub to: f32,
    /// Whether the animation should repeat.
    pub repeat: bool,
    /// Whether the animation should reverse on repeat (ping-pong).
    pub ping_pong: bool,
}

impl ActiveAnimation {
    /// Creates a new animation starting now.
    pub fn new(
        id: impl Into<String>,
        duration: Duration,
        easing: EasingFunction,
        from: f32,
        to: f32,
    ) -> Self {
        Self {
            id: id.into(),
            start_time: Instant::now(),
            duration,
            easing,
            from,
            to,
            repeat: false,
            ping_pong: false,
        }
    }

    /// Evaluates the animation at the current time.
    pub fn evaluate(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.start_time).as_secs_f32();
        let total = self.duration.as_secs_f32().max(0.001);
        let mut t = elapsed / total;

        if self.repeat {
            if self.ping_pong {
                let cycle = t % 2.0;
                t = if cycle > 1.0 { 2.0 - cycle } else { cycle };
            } else {
                t %= 1.0;
            }
        } else {
            t = t.min(1.0);
        }

        let eased = self.easing.evaluate(t);
        self.from + (self.to - self.from) * eased
    }

    /// Whether the animation is complete (ignoring repeat).
    pub fn is_finished(&self, now: Instant) -> bool {
        if self.repeat {
            return false;
        }
        now.duration_since(self.start_time) >= self.duration
    }

    /// Restarts the animation from the current instant.
    pub fn restart(&mut self) {
        self.start_time = Instant::now();
    }
}

// ---------------------------------------------------------------------------
// Animator — manages multiple active animations
// ---------------------------------------------------------------------------

/// Manages a set of active animations, advancing them each frame and
/// providing interpolated values by id.
pub struct Animator {
    animations: Vec<ActiveAnimation>,
}

impl Animator {
    pub fn new() -> Self {
        Self {
            animations: Vec::new(),
        }
    }

    /// Starts or replaces an animation with the given id.
    pub fn start(&mut self, animation: ActiveAnimation) {
        if let Some(existing) = self.animations.iter_mut().find(|a| a.id == animation.id) {
            *existing = animation;
        } else {
            self.animations.push(animation);
        }
    }

    /// Cancels an animation by id.
    pub fn cancel(&mut self, id: &str) {
        self.animations.retain(|a| a.id != id);
    }

    /// Returns the current interpolated value for the given animation id,
    /// or `None` if no such animation is running.
    pub fn value(&self, id: &str) -> Option<f32> {
        let now = Instant::now();
        self.animations
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.evaluate(now))
    }

    /// Returns the current value or a default if the animation doesn't exist.
    pub fn value_or(&self, id: &str, default: f32) -> f32 {
        self.value(id).unwrap_or(default)
    }

    /// Removes all finished (non-repeating) animations.
    pub fn gc(&mut self) {
        let now = Instant::now();
        self.animations.retain(|a| !a.is_finished(now));
    }

    /// Advances all animations and garbage-collects finished ones.
    /// Returns `true` if any animations are still running (the frame
    /// should be redrawn).
    pub fn tick(&mut self) -> bool {
        self.gc();
        !self.animations.is_empty()
    }

    /// Whether any animations are currently running.
    pub fn is_animating(&self) -> bool {
        !self.animations.is_empty()
    }

    /// Number of active animations.
    pub fn active_count(&self) -> usize {
        self.animations.len()
    }

    /// Returns an iterator over all active animations.
    pub fn iter(&self) -> impl Iterator<Item = &ActiveAnimation> {
        self.animations.iter()
    }

    /// Cancels all running animations.
    pub fn cancel_all(&mut self) {
        self.animations.clear();
    }
}

impl Default for Animator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

/// Creates a cursor blink animation (repeating ping-pong fade).
pub fn cursor_blink_animation(period_secs: f32) -> ActiveAnimation {
    let mut anim = ActiveAnimation::new(
        "cursor_blink",
        Duration::from_secs_f32(period_secs / 2.0),
        EasingFunction::EaseInOut,
        1.0,
        0.0,
    );
    anim.repeat = true;
    anim.ping_pong = true;
    anim
}

/// Creates a smooth scroll animation.
pub fn smooth_scroll_animation(id: &str, from: f32, to: f32, duration_ms: u64) -> ActiveAnimation {
    ActiveAnimation::new(
        id,
        Duration::from_millis(duration_ms),
        EasingFunction::EaseOut,
        from,
        to,
    )
}

/// Creates a cursor movement animation.
pub fn cursor_move_animation(
    cursor_index: usize,
    axis: &str,
    from: f32,
    to: f32,
) -> ActiveAnimation {
    ActiveAnimation::new(
        format!("cursor_move_{cursor_index}_{axis}"),
        Duration::from_millis(120),
        EasingFunction::EaseOut,
        from,
        to,
    )
}

/// Creates a widget fade-in animation.
pub fn fade_in_animation(id: &str, duration_ms: u64) -> ActiveAnimation {
    ActiveAnimation::new(
        id,
        Duration::from_millis(duration_ms),
        EasingFunction::EaseOut,
        0.0,
        1.0,
    )
}

/// Creates a widget fade-out animation.
pub fn fade_out_animation(id: &str, duration_ms: u64) -> ActiveAnimation {
    ActiveAnimation::new(
        id,
        Duration::from_millis(duration_ms),
        EasingFunction::EaseIn,
        1.0,
        0.0,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_easing() {
        let e = EasingFunction::Linear;
        assert!((e.evaluate(0.0)).abs() < f32::EPSILON);
        assert!((e.evaluate(0.5) - 0.5).abs() < f32::EPSILON);
        assert!((e.evaluate(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn ease_in_starts_slow() {
        let e = EasingFunction::EaseIn;
        assert!(e.evaluate(0.5) < 0.5);
    }

    #[test]
    fn ease_out_starts_fast() {
        let e = EasingFunction::EaseOut;
        assert!(e.evaluate(0.5) > 0.5);
    }

    #[test]
    fn ease_in_out_symmetric() {
        let e = EasingFunction::EaseInOut;
        assert!((e.evaluate(0.5) - 0.5).abs() < 0.01);
    }

    #[test]
    fn animation_state_advances() {
        let mut state = AnimationState::new(1.0, EasingFunction::Linear);
        assert!(!state.is_finished());
        let _ = state.advance(0.5);
        assert!(!state.is_finished());
        let v = state.advance(0.6);
        assert!(state.is_finished());
        assert!((v - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn animator_start_cancel() {
        let mut animator = Animator::new();
        assert!(!animator.is_animating());
        animator.start(ActiveAnimation::new(
            "test",
            Duration::from_secs(1),
            EasingFunction::Linear,
            0.0,
            100.0,
        ));
        assert!(animator.is_animating());
        assert_eq!(animator.active_count(), 1);
        animator.cancel("test");
        assert!(!animator.is_animating());
    }

    #[test]
    fn cubic_bezier_endpoints() {
        let e = EasingFunction::CubicBezier(0.25, 0.1, 0.25, 1.0);
        assert!(e.evaluate(0.0).abs() < 0.01);
        assert!((e.evaluate(1.0) - 1.0).abs() < 0.01);
    }
}
