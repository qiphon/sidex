//! Single-line text input widget.
//!
//! Supports cursor movement, text selection, placeholder, focus ring,
//! and horizontal scroll for long text.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext};
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// A single-line text input field.
#[allow(dead_code)]
pub struct TextInput<F: FnMut(&str)> {
    pub value: String,
    pub placeholder: String,
    pub on_change: F,
    /// Byte offset of the cursor within `value`.
    cursor: usize,
    /// Byte offset of the selection anchor (equal to `cursor` when no selection).
    selection_anchor: usize,
    focused: bool,
    font_size: f32,
    background: Color,
    foreground: Color,
    placeholder_color: Color,
    border_color: Color,
    focus_border_color: Color,
    selection_color: Color,
    cursor_color: Color,
    /// Horizontal pixel scroll offset for long text.
    scroll_x: f32,
    /// Blink timer in seconds; cursor visible when `blink_timer < 0.5`.
    blink_timer: f32,
}

impl<F: FnMut(&str)> TextInput<F> {
    pub fn new(value: impl Into<String>, on_change: F) -> Self {
        let value = value.into();
        let len = value.len();
        Self {
            value,
            placeholder: String::new(),
            on_change,
            cursor: len,
            selection_anchor: len,
            focused: false,
            font_size: 13.0,
            background: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            placeholder_color: Color::from_hex("#cccccc80").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            focus_border_color: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            selection_color: Color::from_hex("#264f78").unwrap_or(Color::BLACK),
            cursor_color: Color::from_hex("#aeafad").unwrap_or(Color::WHITE),
            scroll_x: 0.0,
            blink_timer: 0.0,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Advance the blink timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.blink_timer = (self.blink_timer + dt) % 1.0;
    }

    fn selection_range(&self) -> (usize, usize) {
        let lo = self.cursor.min(self.selection_anchor);
        let hi = self.cursor.max(self.selection_anchor);
        (lo, hi)
    }

    fn has_selection(&self) -> bool {
        self.cursor != self.selection_anchor
    }

    fn delete_selection(&mut self) {
        let (lo, hi) = self.selection_range();
        self.value.drain(lo..hi);
        self.cursor = lo;
        self.selection_anchor = lo;
    }

    fn insert_char(&mut self, ch: char) {
        if self.has_selection() {
            self.delete_selection();
        }
        self.value.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.selection_anchor = self.cursor;
    }

    fn move_cursor_left(&mut self, shift: bool) {
        if self.cursor > 0 {
            let prev = self.value[..self.cursor]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            self.cursor = prev;
        }
        if !shift {
            self.selection_anchor = self.cursor;
        }
    }

    fn move_cursor_right(&mut self, shift: bool) {
        if self.cursor < self.value.len() {
            let next = self.value[self.cursor..]
                .char_indices()
                .nth(1)
                .map_or(self.value.len(), |(i, _)| self.cursor + i);
            self.cursor = next;
        }
        if !shift {
            self.selection_anchor = self.cursor;
        }
    }

    fn char_width(&self) -> f32 {
        self.font_size * 0.6
    }

    #[allow(clippy::cast_precision_loss)]
    fn cursor_x_offset(&self) -> f32 {
        self.value[..self.cursor].chars().count() as f32 * self.char_width()
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation
    )]
    fn x_to_offset(&self, x: f32, rect: Rect) -> usize {
        let char_width = self.char_width();
        let rel = (x - rect.x - 6.0 + self.scroll_x).max(0.0);
        let idx = (rel / char_width).round() as usize;
        let mut byte_offset = 0;
        for (i, (offset, _)) in self.value.char_indices().enumerate() {
            if i >= idx {
                return offset;
            }
            byte_offset = offset;
        }
        if idx > 0 {
            self.value.len()
        } else {
            byte_offset
        }
    }

    fn ensure_cursor_visible(&mut self, inner_width: f32) {
        let cx = self.cursor_x_offset();
        let padding = 6.0;
        if cx - self.scroll_x < 0.0 {
            self.scroll_x = cx;
        } else if cx - self.scroll_x > inner_width - padding * 2.0 {
            self.scroll_x = cx - (inner_width - padding * 2.0);
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        let border_c = if self.focused {
            self.focus_border_color
        } else {
            self.border_color
        };

        // Background
        ctx.draw_rect(rect, self.background, 2.0);
        // Border (focus ring when focused)
        ctx.draw_border(rect, border_c, 1.0, 2.0);

        let inner = rect.inset(Edges::symmetric(6.0, 4.0));
        ctx.save();
        ctx.clip(inner);
        ctx.offset(-self.scroll_x, 0.0);

        // Selection highlight
        if self.has_selection() {
            let (lo, hi) = self.selection_range();
            let char_w = self.char_width();
            let sel_x = inner.x + self.value[..lo].chars().count() as f32 * char_w;
            let sel_w = self.value[lo..hi].chars().count() as f32 * char_w;
            let sel_rect = Rect::new(sel_x, inner.y, sel_w, inner.height);
            ctx.draw_rect(sel_rect, self.selection_color, 0.0);
        }

        if self.value.is_empty() {
            // Placeholder
            if !self.placeholder.is_empty() {
                let ty = inner.y + (inner.height - self.font_size) / 2.0;
                ctx.draw_text(
                    &self.placeholder,
                    (inner.x, ty),
                    self.placeholder_color,
                    self.font_size,
                    false,
                    true,
                );
            }
        } else {
            // Text
            let ty = inner.y + (inner.height - self.font_size) / 2.0;
            ctx.draw_text(
                &self.value,
                (inner.x, ty),
                self.foreground,
                self.font_size,
                false,
                false,
            );
        }

        // Blinking cursor
        if self.focused && self.blink_timer < 0.5 {
            let cx = inner.x + self.cursor_x_offset();
            let cursor_rect = Rect::new(cx, inner.y, 1.0, inner.height);
            ctx.draw_rect(cursor_rect, self.cursor_color, 0.0);
        }

        ctx.restore();

        // Set text cursor on hover
        if self.focused {
            ctx.set_cursor(CursorIcon::Text);
        }
    }
}

impl<F: FnMut(&str)> Widget for TextInput<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Flex(1.0),
            padding: Edges::symmetric(6.0, 4.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rects = sidex_gpu::RectRenderer::new();
        rects.draw_rect(
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            self.background,
            2.0,
        );
        if self.focused {
            rects.draw_border(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                self.focus_border_color,
                1.0,
            );
        }
        if self.has_selection() {
            let (lo, hi) = self.selection_range();
            let char_w = self.font_size * 0.6;
            #[allow(clippy::cast_precision_loss)]
            let sel_x = rect.x + 6.0 + self.value[..lo].chars().count() as f32 * char_w;
            #[allow(clippy::cast_precision_loss)]
            let sel_w = self.value[lo..hi].chars().count() as f32 * char_w;
            rects.draw_rect(
                sel_x,
                rect.y + 2.0,
                sel_w,
                rect.height - 4.0,
                self.selection_color,
                0.0,
            );
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => {
                self.focused = true;
                self.blink_timer = 0.0;
                EventResult::Handled
            }
            UiEvent::Blur => {
                self.focused = false;
                EventResult::Handled
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.focused = true;
                self.blink_timer = 0.0;
                let offset = self.x_to_offset(*x, rect);
                self.cursor = offset;
                self.selection_anchor = offset;
                EventResult::Handled
            }
            UiEvent::KeyPress { key, modifiers } if self.focused => {
                self.blink_timer = 0.0;
                match key {
                    Key::Char(ch) => {
                        self.insert_char(*ch);
                        self.ensure_cursor_visible(rect.width);
                        (self.on_change)(&self.value);
                        EventResult::Handled
                    }
                    Key::Backspace => {
                        if self.has_selection() {
                            self.delete_selection();
                        } else if self.cursor > 0 {
                            self.move_cursor_left(false);
                            let end = self.value[self.cursor..]
                                .char_indices()
                                .nth(1)
                                .map_or(self.value.len(), |(i, _)| self.cursor + i);
                            self.value.drain(self.cursor..end);
                        }
                        self.ensure_cursor_visible(rect.width);
                        (self.on_change)(&self.value);
                        EventResult::Handled
                    }
                    Key::Delete => {
                        if self.has_selection() {
                            self.delete_selection();
                        } else if self.cursor < self.value.len() {
                            let end = self.value[self.cursor..]
                                .char_indices()
                                .nth(1)
                                .map_or(self.value.len(), |(i, _)| self.cursor + i);
                            self.value.drain(self.cursor..end);
                        }
                        (self.on_change)(&self.value);
                        EventResult::Handled
                    }
                    Key::ArrowLeft => {
                        self.move_cursor_left(modifiers.shift);
                        self.ensure_cursor_visible(rect.width);
                        EventResult::Handled
                    }
                    Key::ArrowRight => {
                        self.move_cursor_right(modifiers.shift);
                        self.ensure_cursor_visible(rect.width);
                        EventResult::Handled
                    }
                    Key::Home => {
                        self.cursor = 0;
                        if !modifiers.shift {
                            self.selection_anchor = 0;
                        }
                        self.scroll_x = 0.0;
                        EventResult::Handled
                    }
                    Key::End => {
                        self.cursor = self.value.len();
                        if !modifiers.shift {
                            self.selection_anchor = self.value.len();
                        }
                        self.ensure_cursor_visible(rect.width);
                        EventResult::Handled
                    }
                    Key::Tab => EventResult::FocusNext,
                    _ => EventResult::Ignored,
                }
            }
            _ => EventResult::Ignored,
        }
    }
}
