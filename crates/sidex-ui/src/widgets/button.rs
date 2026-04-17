//! Push-button widget with hover and press states.
//!
//! Supports primary/secondary styling, icon-only variant, and disabled state.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext, IconId};
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

/// Visual style for a button.
#[derive(Clone, Debug)]
pub struct ButtonStyle {
    pub background: Color,
    pub foreground: Color,
    pub hover_background: Color,
    pub press_background: Color,
    pub disabled_background: Color,
    pub disabled_foreground: Color,
    pub border_color: Option<Color>,
    pub border_radius: f32,
    pub padding: Edges,
    pub font_size: f32,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            background: Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            foreground: Color::WHITE,
            hover_background: Color::from_hex("#1177bb").unwrap_or(Color::BLACK),
            press_background: Color::from_hex("#0d5689").unwrap_or(Color::BLACK),
            disabled_background: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            disabled_foreground: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
            border_color: None,
            border_radius: 2.0,
            padding: Edges::symmetric(14.0, 6.0),
            font_size: 13.0,
        }
    }
}

impl ButtonStyle {
    /// A secondary (ghost) button style.
    pub fn secondary() -> Self {
        Self {
            background: Color::TRANSPARENT,
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            hover_background: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            press_background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            disabled_background: Color::TRANSPARENT,
            disabled_foreground: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
            border_color: Some(Color::from_hex("#454545").unwrap_or(Color::WHITE)),
            border_radius: 2.0,
            padding: Edges::symmetric(14.0, 6.0),
            font_size: 13.0,
        }
    }
}

/// Interactive state of the button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum InteractState {
    #[default]
    Normal,
    Hovered,
    Pressed,
}

/// A clickable button with a text label and optional icon.
pub struct Button<F: FnMut()> {
    pub label: String,
    pub icon: Option<IconId>,
    pub on_click: F,
    pub style: ButtonStyle,
    pub disabled: bool,
    state: InteractState,
}

impl<F: FnMut()> Button<F> {
    pub fn new(label: impl Into<String>, on_click: F) -> Self {
        Self {
            label: label.into(),
            icon: None,
            on_click,
            style: ButtonStyle::default(),
            disabled: false,
            state: InteractState::Normal,
        }
    }

    pub fn icon_only(icon: IconId, on_click: F) -> Self {
        Self {
            label: String::new(),
            icon: Some(icon),
            on_click,
            style: ButtonStyle {
                padding: Edges::all(6.0),
                ..ButtonStyle::secondary()
            },
            disabled: false,
            state: InteractState::Normal,
        }
    }

    pub fn with_style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        let (bg, fg) = if self.disabled {
            (
                self.style.disabled_background,
                self.style.disabled_foreground,
            )
        } else {
            let bg = match self.state {
                InteractState::Normal => self.style.background,
                InteractState::Hovered => self.style.hover_background,
                InteractState::Pressed => self.style.press_background,
            };
            (bg, self.style.foreground)
        };

        // Background
        ctx.draw_rect(rect, bg, self.style.border_radius);

        // Border
        if let Some(bc) = self.style.border_color {
            ctx.draw_border(rect, bc, 1.0, self.style.border_radius);
        }

        let content = rect.inset(self.style.padding);
        let mut text_x = content.x;

        // Icon
        if let Some(icon) = self.icon {
            let icon_size = self.style.font_size;
            let icon_y = content.y + (content.height - icon_size) / 2.0;
            ctx.draw_icon(icon, (text_x, icon_y), icon_size, fg);
            if !self.label.is_empty() {
                text_x += icon_size + 4.0;
            }
        }

        // Label text
        if !self.label.is_empty() {
            let text_y = content.y + (content.height - self.style.font_size) / 2.0;
            ctx.draw_text(
                &self.label,
                (text_x, text_y),
                fg,
                self.style.font_size,
                false,
                false,
            );
        }

        // Cursor
        if !self.disabled && self.state != InteractState::Normal {
            ctx.set_cursor(CursorIcon::Pointer);
        }
    }
}

impl<F: FnMut()> Widget for Button<F> {
    #[allow(clippy::cast_precision_loss)]
    fn layout(&self) -> LayoutNode {
        let text_width = self.label.len() as f32 * self.style.font_size * 0.6;
        let icon_width = if self.icon.is_some() {
            self.style.font_size + if self.label.is_empty() { 0.0 } else { 4.0 }
        } else {
            0.0
        };
        LayoutNode {
            size: Size::Fixed(text_width + icon_width + self.style.padding.horizontal()),
            padding: self.style.padding,
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let bg = match self.state {
            InteractState::Normal => self.style.background,
            InteractState::Hovered => self.style.hover_background,
            InteractState::Pressed => self.style.press_background,
        };
        let mut rects = sidex_gpu::RectRenderer::new();
        rects.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            bg,
            self.style.border_radius,
        );
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        if self.disabled {
            return EventResult::Ignored;
        }
        match event {
            UiEvent::MouseMove { x, y } => {
                if rect.contains(*x, *y) {
                    if self.state != InteractState::Pressed {
                        self.state = InteractState::Hovered;
                    }
                } else {
                    self.state = InteractState::Normal;
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.state = InteractState::Pressed;
                EventResult::Handled
            }
            UiEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if self.state == InteractState::Pressed && rect.contains(*x, *y) {
                    (self.on_click)();
                }
                self.state = if rect.contains(*x, *y) {
                    InteractState::Hovered
                } else {
                    InteractState::Normal
                };
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
