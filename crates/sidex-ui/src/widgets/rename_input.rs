//! Inline rename input widget.
//!
//! A small input box that appears at the rename location, pre-filled with
//! the current symbol name (fully selected). Shows a preview of all
//! locations that will be renamed.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::DrawContext;
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// A location that will be affected by the rename.
#[derive(Clone, Debug)]
pub struct RenameLocation {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

/// Result of the rename operation.
#[derive(Clone, Debug)]
pub enum RenameResult {
    /// User confirmed the rename with this new name.
    Confirmed(String),
    /// User cancelled the rename.
    Cancelled,
}

/// The inline rename input box.
#[allow(dead_code)]
pub struct RenameInput {
    pub value: String,
    cursor_pos: usize,
    selection: Option<(usize, usize)>,
    original_name: String,
    visible: bool,
    position: (f32, f32),

    /// Preview locations that will be renamed.
    pub preview_locations: Vec<RenameLocation>,
    /// Validation error message, if any.
    pub validation_error: Option<String>,

    font_size: f32,
    input_width: f32,
    input_height: f32,
    blink_timer: f32,

    background: Color,
    border_color: Color,
    focus_border: Color,
    shadow_color: Color,
    foreground: Color,
    selection_bg: Color,
    cursor_color: Color,
    preview_fg: Color,
    error_fg: Color,
    error_border: Color,
    preview_bg: Color,
}

impl Default for RenameInput {
    fn default() -> Self {
        Self::new()
    }
}

impl RenameInput {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            cursor_pos: 0,
            selection: None,
            original_name: String::new(),
            visible: false,
            position: (0.0, 0.0),
            preview_locations: Vec::new(),
            validation_error: None,
            font_size: 13.0,
            input_width: 200.0,
            input_height: 24.0,
            blink_timer: 0.0,
            background: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            focus_border: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            shadow_color: Color::from_hex("#00000060").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            selection_bg: Color::from_hex("#264f78").unwrap_or(Color::BLACK),
            cursor_color: Color::from_hex("#aeafad").unwrap_or(Color::WHITE),
            preview_fg: Color::from_hex("#9d9d9d80").unwrap_or(Color::WHITE),
            error_fg: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            error_border: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            preview_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
        }
    }

    /// Shows the rename input at the given position, pre-filled with the
    /// symbol name and fully selected.
    pub fn show(&mut self, name: &str, position: (f32, f32)) {
        self.value = name.to_string();
        self.original_name = name.to_string();
        self.cursor_pos = name.len();
        self.selection = Some((0, name.len()));
        self.position = position;
        self.visible = true;
        self.validation_error = None;
        self.blink_timer = 0.0;
    }

    /// Hides the rename input.
    pub fn hide(&mut self) {
        self.visible = false;
        self.preview_locations.clear();
        self.validation_error = None;
    }

    /// Confirms the rename. Returns the result.
    pub fn confirm(&mut self) -> RenameResult {
        let result = if self.value.is_empty() || self.value == self.original_name {
            RenameResult::Cancelled
        } else {
            RenameResult::Confirmed(self.value.clone())
        };
        self.hide();
        result
    }

    /// Cancels the rename.
    pub fn cancel(&mut self) -> RenameResult {
        self.hide();
        RenameResult::Cancelled
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Advance the blink timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.blink_timer = (self.blink_timer + dt) % 1.0;
    }

    /// Sets a validation error message.
    pub fn set_validation_error(&mut self, msg: Option<String>) {
        self.validation_error = msg;
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn has_selection(&self) -> bool {
        self.selection.map_or(false, |(a, b)| a != b)
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection.map(|(a, b)| (a.min(b), a.max(b)))
    }

    fn delete_selection(&mut self) {
        if let Some((lo, hi)) = self.selection_range() {
            self.value.drain(lo..hi);
            self.cursor_pos = lo;
            self.selection = None;
        }
    }

    fn insert_char(&mut self, ch: char) {
        if self.has_selection() {
            self.delete_selection();
        }
        self.value.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
        self.selection = None;
        self.blink_timer = 0.0;
    }

    fn move_cursor_left(&mut self, shift: bool) {
        if self.cursor_pos > 0 {
            let prev = self.value[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            let anchor = if shift {
                self.selection.map_or(self.cursor_pos, |(a, _)| a)
            } else {
                prev
            };
            self.cursor_pos = prev;
            self.selection = if shift {
                Some((anchor, self.cursor_pos))
            } else {
                None
            };
        } else if !shift {
            self.selection = None;
        }
        self.blink_timer = 0.0;
    }

    fn move_cursor_right(&mut self, shift: bool) {
        if self.cursor_pos < self.value.len() {
            let next = self.value[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map_or(self.value.len(), |(i, _)| self.cursor_pos + i);
            let anchor = if shift {
                self.selection.map_or(self.cursor_pos, |(a, _)| a)
            } else {
                next
            };
            self.cursor_pos = next;
            self.selection = if shift {
                Some((anchor, self.cursor_pos))
            } else {
                None
            };
        } else if !shift {
            self.selection = None;
        }
        self.blink_timer = 0.0;
    }

    fn select_all(&mut self) {
        self.selection = Some((0, self.value.len()));
        self.cursor_pos = self.value.len();
    }

    fn char_width(&self) -> f32 {
        self.font_size * 0.6
    }

    fn widget_rect(&self) -> Rect {
        let w = self.input_width;
        let h = self.input_height;
        Rect::new(self.position.0, self.position.1, w, h)
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, _editor_rect: Rect) {
        if !self.visible {
            return;
        }
        let wr = self.widget_rect();

        // Shadow
        let shadow = Rect::new(wr.x + 1.0, wr.y + 1.0, wr.width, wr.height);
        ctx.draw_rect(shadow, self.shadow_color, 2.0);

        // Background
        ctx.draw_rect(wr, self.background, 2.0);

        // Border (error or focus)
        let border_c = if self.validation_error.is_some() {
            self.error_border
        } else {
            self.focus_border
        };
        ctx.draw_border(wr, border_c, 1.0, 2.0);

        let inner_x = wr.x + 4.0;
        let text_y = wr.y + (wr.height - self.font_size) / 2.0;
        let char_w = self.char_width();

        // Selection highlight
        if let Some((lo, hi)) = self.selection_range() {
            let sel_x = inner_x + self.value[..lo].chars().count() as f32 * char_w;
            let sel_w = self.value[lo..hi].chars().count() as f32 * char_w;
            let sel_r = Rect::new(sel_x, wr.y + 2.0, sel_w, wr.height - 4.0);
            ctx.draw_rect(sel_r, self.selection_bg, 0.0);
        }

        // Text
        ctx.draw_text(
            &self.value,
            (inner_x, text_y),
            self.foreground,
            self.font_size,
            false,
            false,
        );

        // Blinking cursor
        if self.blink_timer < 0.5 {
            let cx = inner_x + self.value[..self.cursor_pos].chars().count() as f32 * char_w;
            let cur_r = Rect::new(cx, wr.y + 3.0, 1.0, wr.height - 6.0);
            ctx.draw_rect(cur_r, self.cursor_color, 0.0);
        }

        // Validation error below
        if let Some(ref err) = self.validation_error {
            let err_y = wr.y + wr.height + 2.0;
            ctx.draw_text(err, (wr.x, err_y), self.error_fg, self.font_size - 1.0, false, false);
        }

        // Preview locations (dimmed text below)
        if !self.preview_locations.is_empty() {
            let preview_y = wr.y + wr.height + if self.validation_error.is_some() { 18.0 } else { 4.0 };
            let preview_r = Rect::new(
                wr.x,
                preview_y,
                wr.width + 100.0,
                self.preview_locations.len().min(5) as f32 * (self.font_size + 2.0) + 8.0,
            );
            ctx.draw_rect(preview_r, self.preview_bg, 3.0);
            ctx.draw_border(preview_r, self.border_color, 1.0, 3.0);

            let mut py = preview_y + 4.0;
            for (i, loc) in self.preview_locations.iter().enumerate() {
                if i >= 5 {
                    let remaining = self.preview_locations.len() - 5;
                    let msg = format!("... and {remaining} more locations");
                    ctx.draw_text(
                        &msg,
                        (wr.x + 8.0, py),
                        self.preview_fg,
                        self.font_size - 2.0,
                        false,
                        true,
                    );
                    break;
                }
                let loc_text = format!("{}:{}:{}", loc.file_path, loc.line, loc.column);
                ctx.draw_text(
                    &loc_text,
                    (wr.x + 8.0, py),
                    self.preview_fg,
                    self.font_size - 2.0,
                    false,
                    false,
                );
                py += self.font_size + 2.0;
            }
        }
    }
}

impl Widget for RenameInput {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Auto,
            padding: Edges::all(0.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, _rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible {
            return;
        }
        let wr = self.widget_rect();
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(wr.x, wr.y, wr.width, wr.height, self.background, 2.0);
        rr.draw_border(wr.x, wr.y, wr.width, wr.height, self.focus_border, 1.0);
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, _rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        match event {
            UiEvent::KeyPress { key, modifiers } => {
                self.blink_timer = 0.0;
                match key {
                    Key::Escape => {
                        let _ = self.cancel();
                        EventResult::Handled
                    }
                    Key::Enter => {
                        let _ = self.confirm();
                        EventResult::Handled
                    }
                    Key::Char('a') if modifiers.command() => {
                        self.select_all();
                        EventResult::Handled
                    }
                    Key::Char(ch) if !modifiers.command() => {
                        self.insert_char(*ch);
                        EventResult::Handled
                    }
                    Key::Backspace => {
                        if self.has_selection() {
                            self.delete_selection();
                        } else if self.cursor_pos > 0 {
                            self.move_cursor_left(false);
                            let end = self.value[self.cursor_pos..]
                                .char_indices()
                                .nth(1)
                                .map_or(self.value.len(), |(i, _)| self.cursor_pos + i);
                            self.value.drain(self.cursor_pos..end);
                        }
                        EventResult::Handled
                    }
                    Key::Delete => {
                        if self.has_selection() {
                            self.delete_selection();
                        } else if self.cursor_pos < self.value.len() {
                            let end = self.value[self.cursor_pos..]
                                .char_indices()
                                .nth(1)
                                .map_or(self.value.len(), |(i, _)| self.cursor_pos + i);
                            self.value.drain(self.cursor_pos..end);
                        }
                        EventResult::Handled
                    }
                    Key::ArrowLeft => {
                        self.move_cursor_left(modifiers.shift);
                        EventResult::Handled
                    }
                    Key::ArrowRight => {
                        self.move_cursor_right(modifiers.shift);
                        EventResult::Handled
                    }
                    Key::Home => {
                        self.cursor_pos = 0;
                        if !modifiers.shift {
                            self.selection = None;
                        }
                        EventResult::Handled
                    }
                    Key::End => {
                        self.cursor_pos = self.value.len();
                        if !modifiers.shift {
                            self.selection = None;
                        }
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let wr = self.widget_rect();
                if !wr.contains(*x, *y) {
                    let _ = self.cancel();
                    return EventResult::Handled;
                }
                // Position cursor from click
                let rel_x = (x - wr.x - 4.0).max(0.0);
                let char_w = self.char_width();
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let char_idx = (rel_x / char_w).round() as usize;
                let mut byte_offset = 0;
                for (i, (offset, _)) in self.value.char_indices().enumerate() {
                    if i >= char_idx {
                        byte_offset = offset;
                        break;
                    }
                    byte_offset = self.value.len();
                }
                self.cursor_pos = byte_offset.min(self.value.len());
                self.selection = None;
                self.blink_timer = 0.0;
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_preselects_name() {
        let mut w = RenameInput::new();
        w.show("myVar", (100.0, 50.0));
        assert!(w.is_visible());
        assert_eq!(w.value, "myVar");
        assert_eq!(w.selection, Some((0, 5)));
    }

    #[test]
    fn confirm_rename() {
        let mut w = RenameInput::new();
        w.show("oldName", (0.0, 0.0));
        // Type over the selection
        w.delete_selection();
        for ch in "newName".chars() {
            w.insert_char(ch);
        }
        match w.confirm() {
            RenameResult::Confirmed(name) => assert_eq!(name, "newName"),
            RenameResult::Cancelled => panic!("expected confirmed"),
        }
        assert!(!w.is_visible());
    }

    #[test]
    fn cancel_rename() {
        let mut w = RenameInput::new();
        w.show("test", (0.0, 0.0));
        match w.cancel() {
            RenameResult::Cancelled => {}
            RenameResult::Confirmed(_) => panic!("expected cancelled"),
        }
    }

    #[test]
    fn same_name_cancels() {
        let mut w = RenameInput::new();
        w.show("same", (0.0, 0.0));
        // Don't change anything
        w.selection = None;
        match w.confirm() {
            RenameResult::Cancelled => {}
            RenameResult::Confirmed(_) => panic!("expected cancelled for same name"),
        }
    }

    #[test]
    fn select_all() {
        let mut w = RenameInput::new();
        w.show("test", (0.0, 0.0));
        w.selection = None;
        w.select_all();
        assert_eq!(w.selection, Some((0, 4)));
        assert_eq!(w.cursor_pos, 4);
    }
}
