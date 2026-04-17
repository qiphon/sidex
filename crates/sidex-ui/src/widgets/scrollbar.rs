//! Scrollbar widget with thumb, track, overview ruler annotations, and hover expansion.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

/// Scrollbar orientation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Vertical,
    Horizontal,
}

/// A colored annotation mark on the scrollbar (overview ruler).
#[derive(Clone, Copy, Debug)]
pub struct OverviewMark {
    /// Fractional position in [0, 1] along the scrollbar.
    pub position: f32,
    pub color: Color,
}

/// A scrollbar with proportional thumb sizing, drag support, and overview ruler.
pub struct Scrollbar<F: FnMut(f32)> {
    pub orientation: Orientation,
    pub total: f32,
    pub visible: f32,
    pub position: f32,
    pub on_scroll: F,
    pub marks: Vec<OverviewMark>,

    base_width: f32,
    hover_width: f32,
    thumb_color: Color,
    thumb_hover_color: Color,
    thumb_active_color: Color,
    track_color: Color,

    dragging: bool,
    hovered: bool,
    track_hovered: bool,
    drag_start_offset: f32,
}

impl<F: FnMut(f32)> Scrollbar<F> {
    pub fn new(total: f32, visible: f32, position: f32, on_scroll: F) -> Self {
        Self {
            orientation: Orientation::Vertical,
            total,
            visible,
            position,
            on_scroll,
            marks: Vec::new(),
            base_width: 10.0,
            hover_width: 14.0,
            thumb_color: Color::from_hex("#79797966").unwrap_or(Color::WHITE),
            thumb_hover_color: Color::from_hex("#646464b3").unwrap_or(Color::WHITE),
            thumb_active_color: Color::from_hex("#bfbfbf66").unwrap_or(Color::WHITE),
            track_color: Color::TRANSPARENT,
            dragging: false,
            hovered: false,
            track_hovered: false,
            drag_start_offset: 0.0,
        }
    }

    pub fn horizontal(mut self) -> Self {
        self.orientation = Orientation::Horizontal;
        self
    }

    pub fn with_marks(mut self, marks: Vec<OverviewMark>) -> Self {
        self.marks = marks;
        self
    }

    fn current_width(&self) -> f32 {
        if self.track_hovered || self.dragging {
            self.hover_width
        } else {
            self.base_width
        }
    }

    fn track_length(&self, rect: Rect) -> f32 {
        match self.orientation {
            Orientation::Vertical => rect.height,
            Orientation::Horizontal => rect.width,
        }
    }

    fn thumb_rect(&self, rect: Rect) -> Rect {
        if self.total <= 0.0 || self.visible >= self.total {
            return rect;
        }
        let track = self.track_length(rect);
        let thumb_size = (self.visible / self.total * track).max(20.0).min(track);
        let max_offset = self.total - self.visible;
        let ratio = if max_offset > 0.0 {
            self.position / max_offset
        } else {
            0.0
        };
        let thumb_pos = ratio * (track - thumb_size);
        let w = self.current_width();
        match self.orientation {
            Orientation::Vertical => {
                let x = rect.x + rect.width - w;
                Rect::new(x, rect.y + thumb_pos, w, thumb_size)
            }
            Orientation::Horizontal => {
                let y = rect.y + rect.height - w;
                Rect::new(rect.x + thumb_pos, y, thumb_size, w)
            }
        }
    }

    fn position_from_track(&self, track_pos: f32, rect: Rect) -> f32 {
        let track = self.track_length(rect);
        let thumb_size = (self.visible / self.total * track).max(20.0).min(track);
        let usable = track - thumb_size;
        if usable <= 0.0 {
            return 0.0;
        }
        let ratio = (track_pos / usable).clamp(0.0, 1.0);
        ratio * (self.total - self.visible)
    }

    fn event_pos(&self, x: f32, y: f32, rect: Rect) -> f32 {
        match self.orientation {
            Orientation::Vertical => y - rect.y,
            Orientation::Horizontal => x - rect.x,
        }
    }

    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        // Track background
        let track_rect = match self.orientation {
            Orientation::Vertical => {
                let w = self.current_width();
                Rect::new(rect.x + rect.width - w, rect.y, w, rect.height)
            }
            Orientation::Horizontal => {
                let w = self.current_width();
                Rect::new(rect.x, rect.y + rect.height - w, rect.width, w)
            }
        };
        ctx.draw_rect(track_rect, self.track_color, 0.0);

        // Overview ruler marks
        for mark in &self.marks {
            let pos = mark.position.clamp(0.0, 1.0);
            let mark_rect = match self.orientation {
                Orientation::Vertical => {
                    let my = track_rect.y + pos * track_rect.height;
                    Rect::new(track_rect.x, my, track_rect.width, 2.0)
                }
                Orientation::Horizontal => {
                    let mx = track_rect.x + pos * track_rect.width;
                    Rect::new(mx, track_rect.y, 2.0, track_rect.height)
                }
            };
            ctx.draw_rect(mark_rect, mark.color, 0.0);
        }

        // Thumb
        let thumb = self.thumb_rect(rect);
        let thumb_color = if self.dragging {
            self.thumb_active_color
        } else if self.hovered {
            self.thumb_hover_color
        } else {
            self.thumb_color
        };
        ctx.draw_rect(thumb, thumb_color, 3.0);

        if self.track_hovered || self.dragging {
            ctx.set_cursor(CursorIcon::Default);
        }
    }
}

impl<F: FnMut(f32)> Widget for Scrollbar<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Fixed(self.base_width),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.track_color,
            0.0,
        );
        let thumb = self.thumb_rect(rect);
        let thumb_color = if self.dragging {
            self.thumb_active_color
        } else if self.hovered {
            self.thumb_hover_color
        } else {
            self.thumb_color
        };
        rr.draw_rect(
            thumb.x,
            thumb.y,
            thumb.width,
            thumb.height,
            thumb_color,
            3.0,
        );
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                let thumb = self.thumb_rect(rect);
                let pos = self.event_pos(*x, *y, rect);
                if thumb.contains(*x, *y) {
                    self.dragging = true;
                    let thumb_start = match self.orientation {
                        Orientation::Vertical => thumb.y - rect.y,
                        Orientation::Horizontal => thumb.x - rect.x,
                    };
                    self.drag_start_offset = pos - thumb_start;
                } else {
                    let new_pos = self.position_from_track(
                        pos - self.track_length(rect) * self.visible / self.total / 2.0,
                        rect,
                    );
                    self.position = new_pos;
                    (self.on_scroll)(self.position);
                }
                EventResult::Handled
            }
            UiEvent::MouseMove { x, y } => {
                self.track_hovered = rect.contains(*x, *y);
                if self.dragging {
                    let pos = self.event_pos(*x, *y, rect);
                    let new_pos = self.position_from_track(pos - self.drag_start_offset, rect);
                    self.position = new_pos;
                    (self.on_scroll)(self.position);
                    EventResult::Handled
                } else {
                    let thumb = self.thumb_rect(rect);
                    self.hovered = thumb.contains(*x, *y);
                    EventResult::Ignored
                }
            }
            UiEvent::MouseUp { .. } if self.dragging => {
                self.dragging = false;
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } if rect.contains(0.0, 0.0) => {
                let max = (self.total - self.visible).max(0.0);
                self.position = (self.position - dy * 40.0).clamp(0.0, max);
                (self.on_scroll)(self.position);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
