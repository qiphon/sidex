//! Animation system — interpolation, easing, and timed transitions.

use std::time::Duration;

// ── Easing functions ─────────────────────────────────────────────────────────

/// Easing curve for animation timing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    /// Evaluates the easing function at time `t` (0.0 to 1.0).
    pub fn apply(self, t: f32) -> f32 {
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
            Self::CubicBezier(x1, y1, x2, y2) => cubic_bezier(x1, y1, x2, y2, t),
        }
    }
}

impl Default for Easing {
    fn default() -> Self {
        Self::EaseInOut
    }
}

fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    // Newton-Raphson method to find the t parameter for x, then evaluate y.
    let mut guess = t;
    for _ in 0..8 {
        let x = sample_bezier(x1, x2, guess) - t;
        let dx = sample_bezier_derivative(x1, x2, guess);
        if dx.abs() < 1e-6 {
            break;
        }
        guess -= x / dx;
        guess = guess.clamp(0.0, 1.0);
    }
    sample_bezier(y1, y2, guess)
}

fn sample_bezier(a: f32, b: f32, t: f32) -> f32 {
    let a_coeff = 3.0 * a;
    let b_coeff = 3.0 * (b - a) - a_coeff;
    let c_coeff = 1.0 - a_coeff - b_coeff;
    ((c_coeff * t + b_coeff) * t + a_coeff) * t
}

fn sample_bezier_derivative(a: f32, b: f32, t: f32) -> f32 {
    let a_coeff = 3.0 * a;
    let b_coeff = 3.0 * (b - a) - a_coeff;
    let c_coeff = 1.0 - a_coeff - b_coeff;
    (3.0 * c_coeff * t + 2.0 * b_coeff) * t + a_coeff
}

// ── Interpolation trait ──────────────────────────────────────────────────────

/// A value that can be linearly interpolated.
pub trait Lerp: Clone {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

impl Lerp for f64 {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        a + (b - a) * f64::from(t)
    }
}

impl Lerp for (f32, f32) {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        (f32::lerp(&a.0, &b.0, t), f32::lerp(&a.1, &b.1, t))
    }
}

impl Lerp for (f32, f32, f32, f32) {
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        (
            f32::lerp(&a.0, &b.0, t),
            f32::lerp(&a.1, &b.1, t),
            f32::lerp(&a.2, &b.2, t),
            f32::lerp(&a.3, &b.3, t),
        )
    }
}

// ── Animation ────────────────────────────────────────────────────────────────

/// Animates a value from `from` to `to` over a given duration using an easing curve.
///
/// Used for cursor blink, smooth scroll, panel resize, notification slide-in,
/// hover fade, etc.
#[derive(Clone, Debug)]
pub struct Animation<T: Lerp> {
    from: T,
    to: T,
    duration: Duration,
    easing: Easing,
    elapsed: Duration,
    current: T,
    finished: bool,
    repeat: AnimationRepeat,
    delay: Duration,
    on_complete: AnimationComplete,
}

/// How an animation repeats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationRepeat {
    Once,
    Loop,
    PingPong,
}

/// What happens when animation completes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationComplete {
    Hold,
    Reset,
}

impl<T: Lerp> Animation<T> {
    pub fn new(from: T, to: T, duration: Duration, easing: Easing) -> Self {
        let current = from.clone();
        Self {
            from,
            to,
            duration,
            easing,
            elapsed: Duration::ZERO,
            current,
            finished: false,
            repeat: AnimationRepeat::Once,
            delay: Duration::ZERO,
            on_complete: AnimationComplete::Hold,
        }
    }

    pub fn with_repeat(mut self, repeat: AnimationRepeat) -> Self {
        self.repeat = repeat;
        self
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn with_on_complete(mut self, on_complete: AnimationComplete) -> Self {
        self.on_complete = on_complete;
        self
    }

    /// Advances the animation by `dt` and returns the current interpolated value.
    pub fn tick(&mut self, dt: Duration) -> &T {
        if self.finished {
            return &self.current;
        }

        self.elapsed += dt;

        if self.elapsed < self.delay {
            self.current = self.from.clone();
            return &self.current;
        }

        let active_elapsed = self.elapsed - self.delay;
        let dur_secs = self.duration.as_secs_f32();
        if dur_secs <= 0.0 {
            self.current = self.to.clone();
            self.finished = true;
            return &self.current;
        }

        let raw_t = active_elapsed.as_secs_f32() / dur_secs;

        match self.repeat {
            AnimationRepeat::Once => {
                if raw_t >= 1.0 {
                    self.finished = true;
                    match self.on_complete {
                        AnimationComplete::Hold => {
                            self.current = self.to.clone();
                        }
                        AnimationComplete::Reset => {
                            self.current = self.from.clone();
                        }
                    }
                } else {
                    let eased = self.easing.apply(raw_t);
                    self.current = T::lerp(&self.from, &self.to, eased);
                }
            }
            AnimationRepeat::Loop => {
                let t = raw_t % 1.0;
                let eased = self.easing.apply(t);
                self.current = T::lerp(&self.from, &self.to, eased);
            }
            AnimationRepeat::PingPong => {
                let cycle = raw_t % 2.0;
                let t = if cycle <= 1.0 { cycle } else { 2.0 - cycle };
                let eased = self.easing.apply(t);
                self.current = T::lerp(&self.from, &self.to, eased);
            }
        }

        &self.current
    }

    /// Returns the current value without advancing.
    pub fn value(&self) -> &T {
        &self.current
    }

    /// Returns `true` if the animation has finished.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Resets the animation to the beginning.
    pub fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
        self.finished = false;
        self.current = self.from.clone();
    }

    /// Sets a new target value, keeping the current value as the start.
    pub fn retarget(&mut self, new_to: T) {
        self.from = self.current.clone();
        self.to = new_to;
        self.elapsed = self.delay;
        self.finished = false;
    }

    /// Returns the progress as a value between 0.0 and 1.0.
    pub fn progress(&self) -> f32 {
        if self.finished {
            return 1.0;
        }
        let dur = self.duration.as_secs_f32();
        if dur <= 0.0 {
            return 1.0;
        }
        let active = self.elapsed.saturating_sub(self.delay).as_secs_f32();
        (active / dur).clamp(0.0, 1.0)
    }
}

// ── Animation group ──────────────────────────────────────────────────────────

/// Manages multiple named animations.
#[derive(Debug, Default)]
pub struct AnimationGroup {
    opacity: Option<Animation<f32>>,
    offset_x: Option<Animation<f32>>,
    offset_y: Option<Animation<f32>>,
    scale: Option<Animation<f32>>,
    width: Option<Animation<f32>>,
    height: Option<Animation<f32>>,
}

impl AnimationGroup {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_opacity(&mut self, anim: Animation<f32>) {
        self.opacity = Some(anim);
    }
    pub fn set_offset_x(&mut self, anim: Animation<f32>) {
        self.offset_x = Some(anim);
    }
    pub fn set_offset_y(&mut self, anim: Animation<f32>) {
        self.offset_y = Some(anim);
    }
    pub fn set_scale(&mut self, anim: Animation<f32>) {
        self.scale = Some(anim);
    }
    pub fn set_width(&mut self, anim: Animation<f32>) {
        self.width = Some(anim);
    }
    pub fn set_height(&mut self, anim: Animation<f32>) {
        self.height = Some(anim);
    }

    /// Ticks all animations forward.
    pub fn tick(&mut self, dt: Duration) {
        if let Some(ref mut a) = self.opacity {
            a.tick(dt);
        }
        if let Some(ref mut a) = self.offset_x {
            a.tick(dt);
        }
        if let Some(ref mut a) = self.offset_y {
            a.tick(dt);
        }
        if let Some(ref mut a) = self.scale {
            a.tick(dt);
        }
        if let Some(ref mut a) = self.width {
            a.tick(dt);
        }
        if let Some(ref mut a) = self.height {
            a.tick(dt);
        }
    }

    /// Returns `true` if all animations have finished (or are absent).
    pub fn is_finished(&self) -> bool {
        self.opacity.as_ref().is_none_or(Animation::is_finished)
            && self.offset_x.as_ref().is_none_or(Animation::is_finished)
            && self.offset_y.as_ref().is_none_or(Animation::is_finished)
            && self.scale.as_ref().is_none_or(Animation::is_finished)
            && self.width.as_ref().is_none_or(Animation::is_finished)
            && self.height.as_ref().is_none_or(Animation::is_finished)
    }

    pub fn opacity_value(&self) -> f32 {
        self.opacity.as_ref().map_or(1.0, |a| *a.value())
    }
    pub fn offset_x_value(&self) -> f32 {
        self.offset_x.as_ref().map_or(0.0, |a| *a.value())
    }
    pub fn offset_y_value(&self) -> f32 {
        self.offset_y.as_ref().map_or(0.0, |a| *a.value())
    }
    pub fn scale_value(&self) -> f32 {
        self.scale.as_ref().map_or(1.0, |a| *a.value())
    }
    pub fn width_value(&self) -> Option<f32> {
        self.width.as_ref().map(|a| *a.value())
    }
    pub fn height_value(&self) -> Option<f32> {
        self.height.as_ref().map(|a| *a.value())
    }
}

// ── Preset animations ────────────────────────────────────────────────────────

/// Preset animation builders for common UI transitions.
pub struct Presets;

impl Presets {
    /// Cursor blink: opacity 1.0 → 0.0, 530ms, ping-pong loop.
    pub fn cursor_blink() -> Animation<f32> {
        Animation::new(1.0, 0.0, Duration::from_millis(530), Easing::EaseInOut)
            .with_repeat(AnimationRepeat::PingPong)
    }

    /// Smooth scroll: from `start` to `end` over 120ms.
    pub fn smooth_scroll(start: f32, end: f32) -> Animation<f32> {
        Animation::new(start, end, Duration::from_millis(120), Easing::EaseOut)
    }

    /// Panel resize: from `start` to `end` over 200ms.
    pub fn panel_resize(start: f32, end: f32) -> Animation<f32> {
        Animation::new(start, end, Duration::from_millis(200), Easing::EaseInOut)
    }

    /// Notification slide-in: from off-screen to final position, 300ms.
    pub fn notification_slide_in(start_y: f32, end_y: f32) -> Animation<f32> {
        Animation::new(
            start_y,
            end_y,
            Duration::from_millis(300),
            Easing::CubicBezier(0.0, 0.0, 0.2, 1.0),
        )
    }

    /// Hover fade in: opacity 0.0 → 1.0, 150ms.
    pub fn hover_fade_in() -> Animation<f32> {
        Animation::new(0.0, 1.0, Duration::from_millis(150), Easing::EaseOut)
    }

    /// Hover fade out: opacity 1.0 → 0.0, 200ms.
    pub fn hover_fade_out() -> Animation<f32> {
        Animation::new(1.0, 0.0, Duration::from_millis(200), Easing::EaseIn)
    }

    /// Focus ring pulse: scale 0.95 → 1.0, 100ms.
    pub fn focus_ring_pulse() -> Animation<f32> {
        Animation::new(0.95, 1.0, Duration::from_millis(100), Easing::EaseOut)
    }
}
