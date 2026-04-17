//! Scrollbar renderer with overview ruler decorations and smooth scrolling.
//!
//! Renders vertical and horizontal scrollbars with proportional thumbs,
//! an overview ruler showing errors/warnings/search matches/git changes,
//! smooth scrolling animation, and a scroll shadow at the top edge.

use crate::color::Color;
use crate::rect_renderer::RectRenderer;

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

/// A decoration mark to display on the overview ruler.
#[derive(Debug, Clone, Copy)]
pub struct OverviewRulerMark {
    /// Normalised position within the document (0.0 = top, 1.0 = bottom).
    pub position: f32,
    /// Color of the mark.
    pub color: Color,
}

/// Kind of overview ruler decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverviewMarkKind {
    Error,
    Warning,
    SearchMatch,
    GitChange,
    Selection,
}

impl OverviewMarkKind {
    /// Returns a sensible default color for this kind.
    pub fn default_color(self) -> Color {
        match self {
            Self::Error => Color::from_rgb(230, 60, 60),
            Self::Warning => Color::from_rgb(220, 180, 40),
            Self::SearchMatch => Color::from_rgb(200, 180, 50),
            Self::GitChange => Color::from_rgb(80, 140, 220),
            Self::Selection => Color::from_rgb(100, 150, 255),
        }
    }
}

// ---------------------------------------------------------------------------
// Scroll animation state
// ---------------------------------------------------------------------------

/// Smooth scroll state for a single axis.
#[derive(Debug, Clone, Copy)]
pub struct SmoothScrollAxis {
    /// Current visual offset (the one used for rendering).
    pub current: f32,
    /// Target offset (where we want to end up).
    pub target: f32,
    /// Animation speed factor (larger = faster snap).
    pub speed: f32,
}

impl SmoothScrollAxis {
    pub fn new(speed: f32) -> Self {
        Self {
            current: 0.0,
            target: 0.0,
            speed,
        }
    }

    /// Sets the target scroll offset.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Jumps immediately to the target (no animation).
    pub fn jump(&mut self, offset: f32) {
        self.current = offset;
        self.target = offset;
    }

    /// Advances the animation by `dt` seconds.
    pub fn update(&mut self, dt: f32) {
        let diff = self.target - self.current;
        if diff.abs() < 0.5 {
            self.current = self.target;
        } else {
            self.current += diff * (self.speed * dt).min(1.0);
        }
    }

    /// Returns `true` when the animation has settled.
    pub fn is_settled(&self) -> bool {
        (self.current - self.target).abs() < 0.5
    }
}

impl Default for SmoothScrollAxis {
    fn default() -> Self {
        Self::new(12.0)
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for scrollbar rendering.
#[derive(Debug, Clone)]
pub struct ScrollbarConfig {
    /// Width of the vertical scrollbar track.
    pub vertical_width: f32,
    /// Height of the horizontal scrollbar track.
    pub horizontal_height: f32,
    /// Minimum thumb length/height.
    pub min_thumb_size: f32,
    /// Scrollbar track background color.
    pub track_color: Color,
    /// Scrollbar thumb color.
    pub thumb_color: Color,
    /// Scrollbar thumb color when hovered.
    pub thumb_hover_color: Color,
    /// Width of overview ruler marks.
    pub overview_mark_width: f32,
    /// Height of each overview ruler mark.
    pub overview_mark_height: f32,
    /// Color and height of the scroll shadow gradient.
    pub shadow_color: Color,
    /// Height of the scroll shadow in pixels.
    pub shadow_height: f32,
}

impl Default for ScrollbarConfig {
    fn default() -> Self {
        Self {
            vertical_width: 14.0,
            horizontal_height: 14.0,
            min_thumb_size: 30.0,
            track_color: Color {
                r: 0.15,
                g: 0.15,
                b: 0.15,
                a: 0.5,
            },
            thumb_color: Color {
                r: 0.45,
                g: 0.45,
                b: 0.45,
                a: 0.5,
            },
            thumb_hover_color: Color {
                r: 0.55,
                g: 0.55,
                b: 0.55,
                a: 0.7,
            },
            overview_mark_width: 6.0,
            overview_mark_height: 3.0,
            shadow_color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.4,
            },
            shadow_height: 6.0,
        }
    }
}

// ---------------------------------------------------------------------------
// ScrollbarRenderer
// ---------------------------------------------------------------------------

/// Renders scrollbars, overview ruler, and scroll shadow.
pub struct ScrollbarRenderer {
    config: ScrollbarConfig,
    /// Smooth vertical scroll state.
    pub scroll_y: SmoothScrollAxis,
    /// Smooth horizontal scroll state.
    pub scroll_x: SmoothScrollAxis,
    /// Whether the vertical thumb is hovered.
    vertical_hovered: bool,
    /// Whether the horizontal thumb is hovered.
    horizontal_hovered: bool,
}

impl ScrollbarRenderer {
    pub fn new(config: ScrollbarConfig) -> Self {
        Self {
            config,
            scroll_y: SmoothScrollAxis::default(),
            scroll_x: SmoothScrollAxis::default(),
            vertical_hovered: false,
            horizontal_hovered: false,
        }
    }

    pub fn config_mut(&mut self) -> &mut ScrollbarConfig {
        &mut self.config
    }

    /// Sets hover state for scrollbar thumbs.
    pub fn set_hover(&mut self, vertical: bool, horizontal: bool) {
        self.vertical_hovered = vertical;
        self.horizontal_hovered = horizontal;
    }

    /// Advance smooth scroll animation. Call once per frame.
    pub fn update(&mut self, dt: f32) {
        self.scroll_y.update(dt);
        self.scroll_x.update(dt);
    }

    /// Returns `true` if either scroll axis is still animating.
    pub fn is_animating(&self) -> bool {
        !self.scroll_y.is_settled() || !self.scroll_x.is_settled()
    }

    /// Renders the vertical scrollbar.
    ///
    /// * `viewport_height` — visible area height in pixels.
    /// * `content_height` — total document height in pixels.
    /// * `area_x` — x position of the scrollbar track.
    #[allow(clippy::cast_precision_loss)]
    pub fn render_vertical(
        &self,
        rects: &mut RectRenderer,
        viewport_height: f32,
        content_height: f32,
        area_x: f32,
    ) {
        let cfg = &self.config;
        if content_height <= viewport_height {
            return;
        }

        // Track
        rects.draw_rect(
            area_x,
            0.0,
            cfg.vertical_width,
            viewport_height,
            cfg.track_color,
            0.0,
        );

        // Thumb
        let ratio = viewport_height / content_height;
        let thumb_h = (viewport_height * ratio).max(cfg.min_thumb_size);
        let scroll_range = content_height - viewport_height;
        let thumb_range = viewport_height - thumb_h;
        let thumb_y = if scroll_range > 0.0 {
            (self.scroll_y.current / scroll_range) * thumb_range
        } else {
            0.0
        };
        let color = if self.vertical_hovered {
            cfg.thumb_hover_color
        } else {
            cfg.thumb_color
        };
        rects.draw_rect(
            area_x + 2.0,
            thumb_y,
            cfg.vertical_width - 4.0,
            thumb_h,
            color,
            4.0,
        );
    }

    /// Renders the horizontal scrollbar.
    ///
    /// * `viewport_width` — visible area width in pixels.
    /// * `content_width` — total horizontal content width in pixels.
    /// * `area_y` — y position of the scrollbar track.
    pub fn render_horizontal(
        &self,
        rects: &mut RectRenderer,
        viewport_width: f32,
        content_width: f32,
        area_y: f32,
    ) {
        let cfg = &self.config;
        if content_width <= viewport_width {
            return;
        }

        rects.draw_rect(
            0.0,
            area_y,
            viewport_width,
            cfg.horizontal_height,
            cfg.track_color,
            0.0,
        );

        let ratio = viewport_width / content_width;
        let thumb_w = (viewport_width * ratio).max(cfg.min_thumb_size);
        let scroll_range = content_width - viewport_width;
        let thumb_range = viewport_width - thumb_w;
        let thumb_x = if scroll_range > 0.0 {
            (self.scroll_x.current / scroll_range) * thumb_range
        } else {
            0.0
        };
        let color = if self.horizontal_hovered {
            cfg.thumb_hover_color
        } else {
            cfg.thumb_color
        };
        rects.draw_rect(
            thumb_x,
            area_y + 2.0,
            thumb_w,
            cfg.horizontal_height - 4.0,
            color,
            4.0,
        );
    }

    /// Renders overview ruler marks on the vertical scrollbar track.
    pub fn render_overview_ruler(
        &self,
        rects: &mut RectRenderer,
        marks: &[OverviewRulerMark],
        viewport_height: f32,
        area_x: f32,
    ) {
        let cfg = &self.config;
        let mark_x = area_x + (cfg.vertical_width - cfg.overview_mark_width) * 0.5;

        for mark in marks {
            let mark_y = mark.position.clamp(0.0, 1.0) * viewport_height;
            rects.draw_rect(
                mark_x,
                mark_y,
                cfg.overview_mark_width,
                cfg.overview_mark_height,
                mark.color,
                1.0,
            );
        }
    }

    /// Renders the scroll shadow at the top when scrolled down.
    pub fn render_scroll_shadow(&self, rects: &mut RectRenderer, editor_width: f32) {
        if self.scroll_y.current <= 1.0 {
            return;
        }
        let cfg = &self.config;
        let alpha_scale = (self.scroll_y.current / 50.0).min(1.0);
        let color = Color {
            a: cfg.shadow_color.a * alpha_scale,
            ..cfg.shadow_color
        };
        // Draw multiple thin rects to simulate a gradient
        let steps = 4u32;
        #[allow(clippy::cast_precision_loss)]
        let steps_f = steps as f32;
        for i in 0..steps {
            #[allow(clippy::cast_precision_loss)]
            let t = i as f32 / steps_f;
            let step_h = cfg.shadow_height / steps_f;
            let step_color = Color {
                a: color.a * (1.0 - t),
                ..color
            };
            rects.draw_rect(
                0.0,
                t * cfg.shadow_height,
                editor_width,
                step_h,
                step_color,
                0.0,
            );
        }
    }
}

impl Default for ScrollbarRenderer {
    fn default() -> Self {
        Self::new(ScrollbarConfig::default())
    }
}
