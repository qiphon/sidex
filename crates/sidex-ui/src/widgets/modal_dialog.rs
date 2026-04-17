//! Modal dialog system: confirmation, input, error, and information dialogs
//! with keyboard navigation and button focus.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Dialog result ───────────────────────────────────────────────────────────

/// The result of a modal dialog interaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogResult {
    /// The user pressed a button (index into the button list).
    Button(usize),
    /// The dialog was cancelled (Escape or close).
    Cancelled,
}

// ── Components ──────────────────────────────────────────────────────────────

/// A button in a modal dialog.
#[derive(Clone, Debug)]
pub struct DialogButton {
    pub label: String,
    pub is_primary: bool,
    pub is_cancel: bool,
}

impl DialogButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            is_primary: false,
            is_cancel: false,
        }
    }

    pub fn primary(mut self) -> Self {
        self.is_primary = true;
        self
    }

    pub fn cancel(mut self) -> Self {
        self.is_cancel = true;
        self
    }
}

/// A checkbox in a modal dialog (e.g. "Don't show again").
#[derive(Clone, Debug)]
pub struct DialogCheckbox {
    pub label: String,
    pub checked: bool,
}

impl DialogCheckbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            checked: false,
        }
    }
}

/// A text input field in a modal dialog.
#[derive(Clone, Debug)]
pub struct DialogInput {
    pub value: String,
    pub placeholder: String,
    pub password: bool,
    pub validation_error: Option<String>,
}

impl DialogInput {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            password: false,
            validation_error: None,
        }
    }

    pub fn with_default(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }
}

// ── ModalDialog ─────────────────────────────────────────────────────────────

/// A modal dialog overlaying the workbench.
#[allow(dead_code)]
pub struct ModalDialog {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub buttons: Vec<DialogButton>,
    pub checkbox: Option<DialogCheckbox>,
    pub input: Option<DialogInput>,
    pub is_open: bool,
    pub result: Option<DialogResult>,

    focused_button: usize,
    input_cursor: usize,

    dialog_width: f32,
    font_size: f32,
    title_font_size: f32,

    hovered_button: Option<usize>,
    hovered_checkbox: bool,
    hovered_close: bool,

    overlay_color: Color,
    background: Color,
    foreground: Color,
    detail_fg: Color,
    border_color: Color,
    shadow_color: Color,
    title_bg: Color,
    button_bg: Color,
    button_fg: Color,
    button_primary_bg: Color,
    button_primary_fg: Color,
    button_hover_bg: Color,
    input_bg: Color,
    input_border: Color,
    input_fg: Color,
    input_placeholder_fg: Color,
    error_fg: Color,
    check_color: Color,
    close_hover_bg: Color,
}

impl ModalDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            detail: None,
            buttons: Vec::new(),
            checkbox: None,
            input: None,
            is_open: false,
            result: None,
            focused_button: 0,
            input_cursor: 0,
            dialog_width: 460.0,
            font_size: 13.0,
            title_font_size: 14.0,
            hovered_button: None,
            hovered_checkbox: false,
            hovered_close: false,
            overlay_color: Color::from_hex("#00000088").unwrap_or(Color::BLACK),
            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            detail_fg: Color::from_hex("#999999").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#000000a0").unwrap_or(Color::BLACK),
            title_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            button_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            button_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            button_primary_bg: Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            button_primary_fg: Color::WHITE,
            button_hover_bg: Color::from_hex("#505050").unwrap_or(Color::BLACK),
            input_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            input_border: Color::from_hex("#007acc").unwrap_or(Color::BLACK),
            input_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            input_placeholder_fg: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
            error_fg: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            check_color: Color::from_hex("#007acc").unwrap_or(Color::WHITE),
            close_hover_bg: Color::from_hex("#404040").unwrap_or(Color::BLACK),
        }
    }

    // ── Builder API ─────────────────────────────────────────────────────

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_buttons(mut self, buttons: Vec<DialogButton>) -> Self {
        self.buttons = buttons;
        self.focused_button = self
            .buttons
            .iter()
            .position(|b| b.is_primary)
            .unwrap_or(0);
        self
    }

    pub fn with_checkbox(mut self, checkbox: DialogCheckbox) -> Self {
        self.checkbox = Some(checkbox);
        self
    }

    pub fn with_input(mut self, input: DialogInput) -> Self {
        self.input_cursor = input.value.len();
        self.input = Some(input);
        self
    }

    // ── Convenience constructors ────────────────────────────────────────

    /// "Do you want to save?" style confirmation.
    pub fn confirm(title: &str, message: &str, buttons: &[&str]) -> Self {
        let mut dialog_buttons: Vec<DialogButton> = buttons
            .iter()
            .map(|&l| DialogButton::new(l))
            .collect();
        if let Some(first) = dialog_buttons.first_mut() {
            first.is_primary = true;
        }
        if let Some(last) = dialog_buttons.last_mut() {
            last.is_cancel = true;
        }
        Self::new(title, message).with_buttons(dialog_buttons)
    }

    /// Simple message dialog.
    pub fn show_message(message: &str, buttons: &[&str]) -> Self {
        Self::confirm("", message, buttons)
    }

    /// Input prompt dialog.
    pub fn show_input(prompt: &str, default: &str) -> Self {
        Self::new("", prompt)
            .with_input(DialogInput::new("").with_default(default))
            .with_buttons(vec![
                DialogButton::new("OK").primary(),
                DialogButton::new("Cancel").cancel(),
            ])
    }

    /// Error dialog.
    pub fn show_error(message: &str, detail: &str) -> Self {
        Self::new("Error", message)
            .with_detail(detail)
            .with_buttons(vec![DialogButton::new("OK").primary()])
    }

    // ── Lifecycle ───────────────────────────────────────────────────────

    pub fn open(&mut self) {
        self.is_open = true;
        self.result = None;
        self.focused_button = self
            .buttons
            .iter()
            .position(|b| b.is_primary)
            .unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn close_with(&mut self, result: DialogResult) {
        self.result = Some(result);
        self.is_open = false;
    }

    /// Returns the input value if the dialog had an input field and wasn't cancelled.
    pub fn input_value(&self) -> Option<&str> {
        if matches!(self.result, Some(DialogResult::Button(0))) {
            self.input.as_ref().map(|i| i.value.as_str())
        } else {
            None
        }
    }

    /// Returns the checkbox state.
    pub fn checkbox_checked(&self) -> bool {
        self.checkbox.as_ref().map_or(false, |c| c.checked)
    }

    // ── Layout helpers ──────────────────────────────────────────────────

    #[allow(clippy::cast_precision_loss)]
    fn dialog_height(&self) -> f32 {
        let mut h = 16.0; // top padding
        if !self.title.is_empty() {
            h += 32.0; // title bar
        }
        h += 20.0; // message
        if self.detail.is_some() {
            h += 20.0;
        }
        if self.input.is_some() {
            h += 40.0;
        }
        if self.checkbox.is_some() {
            h += 28.0;
        }
        h += 48.0; // button row + padding
        h
    }

    fn dialog_rect(&self, viewport: Rect) -> Rect {
        let w = self.dialog_width;
        let h = self.dialog_height();
        Rect::new(
            viewport.x + (viewport.width - w) / 2.0,
            viewport.y + (viewport.height - h) / 2.0,
            w,
            h,
        )
    }

    fn close_button_rect(&self, dr: Rect) -> Rect {
        Rect::new(dr.x + dr.width - 28.0, dr.y + 4.0, 24.0, 24.0)
    }

    #[allow(clippy::cast_precision_loss)]
    fn button_rects(&self, dr: Rect) -> Vec<Rect> {
        let button_h = 28.0;
        let button_gap = 8.0;
        let total_w: f32 = self
            .buttons
            .iter()
            .map(|b| b.label.len() as f32 * self.font_size * 0.6 + 24.0)
            .sum::<f32>()
            + (self.buttons.len().saturating_sub(1) as f32 * button_gap);

        let start_x = dr.x + dr.width - 16.0 - total_w;
        let y = dr.y + dr.height - button_h - 12.0;
        let mut x = start_x;

        self.buttons
            .iter()
            .map(|b| {
                let w = b.label.len() as f32 * self.font_size * 0.6 + 24.0;
                let r = Rect::new(x, y, w, button_h);
                x += w + button_gap;
                r
            })
            .collect()
    }

    fn input_rect(&self, dr: Rect) -> Rect {
        let y_base = dr.y + if self.title.is_empty() { 16.0 } else { 48.0 } + 24.0;
        let extra = if self.detail.is_some() { 20.0 } else { 0.0 };
        Rect::new(dr.x + 16.0, y_base + extra, dr.width - 32.0, 28.0)
    }

    fn checkbox_rect(&self, dr: Rect) -> Rect {
        let y_base = dr.y + self.dialog_height() - 48.0 - 28.0;
        Rect::new(dr.x + 16.0, y_base, dr.width - 32.0, 20.0)
    }

    // ── Rendering ───────────────────────────────────────────────────────

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, viewport: Rect) {
        if !self.is_open {
            return;
        }

        // Overlay
        ctx.draw_rect(viewport, self.overlay_color, 0.0);

        let dr = self.dialog_rect(viewport);

        // Shadow
        let shadow = Rect::new(dr.x + 4.0, dr.y + 4.0, dr.width, dr.height);
        ctx.draw_rect(shadow, self.shadow_color, 8.0);

        // Background
        ctx.draw_rect(dr, self.background, 6.0);
        ctx.draw_border(dr, self.border_color, 1.0, 6.0);

        let mut y = dr.y;

        // Title bar
        if !self.title.is_empty() {
            let title_rect = Rect::new(dr.x, dr.y, dr.width, 32.0);
            ctx.draw_rect(title_rect, self.title_bg, 0.0);
            ctx.draw_text(
                &self.title,
                (dr.x + 16.0, dr.y + 8.0),
                self.foreground,
                self.title_font_size,
                true,
                false,
            );
            y += 32.0;

            // Close button
            let cr = self.close_button_rect(dr);
            if self.hovered_close {
                ctx.draw_rect(cr, self.close_hover_bg, 2.0);
            }
            ctx.draw_icon(IconId::Close, (cr.x + 6.0, cr.y + 6.0), 12.0, self.foreground);
        }

        y += 16.0;

        // Message
        ctx.draw_text(
            &self.message,
            (dr.x + 16.0, y),
            self.foreground,
            self.font_size,
            false,
            false,
        );
        y += 20.0;

        // Detail
        if let Some(ref detail) = self.detail {
            ctx.draw_text(
                detail,
                (dr.x + 16.0, y),
                self.detail_fg,
                12.0,
                false,
                false,
            );
            y += 20.0;
        }

        // Input field
        if let Some(ref input) = self.input {
            let ir = self.input_rect(dr);
            ctx.draw_rect(ir, self.input_bg, 2.0);
            ctx.draw_border(ir, self.input_border, 1.0, 2.0);

            let display = if input.password {
                "•".repeat(input.value.len())
            } else if input.value.is_empty() {
                input.placeholder.clone()
            } else {
                input.value.clone()
            };

            let fg = if input.value.is_empty() && !input.password {
                self.input_placeholder_fg
            } else {
                self.input_fg
            };

            ctx.draw_text(
                &display,
                (ir.x + 8.0, ir.y + 6.0),
                fg,
                self.font_size,
                false,
                false,
            );

            // Cursor
            if !input.value.is_empty() || input.password {
                let cursor_x =
                    ir.x + 8.0 + self.input_cursor as f32 * self.font_size * 0.6;
                let cursor_rect = Rect::new(cursor_x, ir.y + 4.0, 1.0, ir.height - 8.0);
                ctx.draw_rect(cursor_rect, self.input_fg, 0.0);
            }

            if let Some(ref err) = input.validation_error {
                ctx.draw_text(
                    err,
                    (ir.x, ir.y + ir.height + 4.0),
                    self.error_fg,
                    11.0,
                    false,
                    false,
                );
            }
            let _ = y;
        }

        // Checkbox
        if let Some(ref cb) = self.checkbox {
            let cbr = self.checkbox_rect(dr);
            let box_rect = Rect::new(cbr.x, cbr.y + 2.0, 16.0, 16.0);
            ctx.draw_border(box_rect, self.border_color, 1.0, 2.0);
            if cb.checked {
                ctx.draw_icon(IconId::Check, (cbr.x + 2.0, cbr.y + 4.0), 12.0, self.check_color);
            }
            ctx.draw_text(
                &cb.label,
                (cbr.x + 22.0, cbr.y + 2.0),
                self.foreground,
                12.0,
                false,
                false,
            );
        }

        // Buttons
        let button_rects = self.button_rects(dr);
        for (i, br) in button_rects.iter().enumerate() {
            let btn = &self.buttons[i];
            let (bg, fg) = if btn.is_primary {
                (self.button_primary_bg, self.button_primary_fg)
            } else {
                (self.button_bg, self.button_fg)
            };

            let effective_bg = if self.hovered_button == Some(i) || self.focused_button == i {
                self.button_hover_bg
            } else {
                bg
            };

            ctx.draw_rect(*br, effective_bg, 2.0);
            if self.focused_button == i {
                ctx.draw_border(*br, self.input_border, 1.0, 2.0);
            }

            let text_x = br.x + (br.width - btn.label.len() as f32 * self.font_size * 0.6) / 2.0;
            let text_y = br.y + (br.height - self.font_size) / 2.0;
            ctx.draw_text(&btn.label, (text_x, text_y), fg, self.font_size, false, false);
        }

        ctx.set_cursor(CursorIcon::Default);
    }
}

impl Widget for ModalDialog {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Auto,
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        if !self.is_open {
            return;
        }
        let dr = self.dialog_rect(rect);
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, self.overlay_color, 0.0);
        rr.draw_rect(dr.x, dr.y, dr.width, dr.height, self.background, 6.0);
        rr.draw_border(dr.x, dr.y, dr.width, dr.height, self.border_color, 1.0);
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, viewport: Rect) -> EventResult {
        if !self.is_open {
            return EventResult::Ignored;
        }
        let dr = self.dialog_rect(viewport);

        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered_button = None;
                self.hovered_checkbox = false;
                self.hovered_close = false;

                if !self.title.is_empty()
                    && self.close_button_rect(dr).contains(*x, *y)
                {
                    self.hovered_close = true;
                }

                for (i, br) in self.button_rects(dr).iter().enumerate() {
                    if br.contains(*x, *y) {
                        self.hovered_button = Some(i);
                        break;
                    }
                }

                if self.checkbox.is_some() {
                    let cbr = self.checkbox_rect(dr);
                    self.hovered_checkbox = cbr.contains(*x, *y);
                }

                EventResult::Handled
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                // Close button
                if self.hovered_close {
                    self.close_with(DialogResult::Cancelled);
                    return EventResult::Handled;
                }

                // Button click
                if let Some(idx) = self.hovered_button {
                    if self.buttons[idx].is_cancel {
                        self.close_with(DialogResult::Cancelled);
                    } else {
                        self.close_with(DialogResult::Button(idx));
                    }
                    return EventResult::Handled;
                }

                // Checkbox toggle
                if self.hovered_checkbox {
                    if let Some(ref mut cb) = self.checkbox {
                        cb.checked = !cb.checked;
                    }
                    return EventResult::Handled;
                }

                // Input focus
                if self.input.is_some() {
                    let ir = self.input_rect(dr);
                    if ir.contains(*x, *y) {
                        return EventResult::Handled;
                    }
                }

                // Click outside dialog = cancel
                if !dr.contains(*x, *y) {
                    self.close_with(DialogResult::Cancelled);
                }
                EventResult::Handled
            }
            UiEvent::KeyPress { key, .. } => match key {
                Key::Escape => {
                    self.close_with(DialogResult::Cancelled);
                    EventResult::Handled
                }
                Key::Enter => {
                    self.close_with(DialogResult::Button(self.focused_button));
                    EventResult::Handled
                }
                Key::Tab => {
                    if !self.buttons.is_empty() {
                        self.focused_button =
                            (self.focused_button + 1) % self.buttons.len();
                    }
                    EventResult::Handled
                }
                Key::Backspace => {
                    if let Some(ref mut input) = self.input {
                        if self.input_cursor > 0 {
                            self.input_cursor -= 1;
                            input.value.remove(self.input_cursor);
                        }
                    }
                    EventResult::Handled
                }
                Key::Delete => {
                    if let Some(ref mut input) = self.input {
                        if self.input_cursor < input.value.len() {
                            input.value.remove(self.input_cursor);
                        }
                    }
                    EventResult::Handled
                }
                Key::ArrowLeft => {
                    if self.input.is_some() && self.input_cursor > 0 {
                        self.input_cursor -= 1;
                    }
                    EventResult::Handled
                }
                Key::ArrowRight => {
                    if let Some(ref input) = self.input {
                        if self.input_cursor < input.value.len() {
                            self.input_cursor += 1;
                        }
                    }
                    EventResult::Handled
                }
                Key::Home => {
                    if self.input.is_some() {
                        self.input_cursor = 0;
                    }
                    EventResult::Handled
                }
                Key::End => {
                    if let Some(ref input) = self.input {
                        self.input_cursor = input.value.len();
                    }
                    EventResult::Handled
                }
                Key::Char(ch) => {
                    if let Some(ref mut input) = self.input {
                        input.value.insert(self.input_cursor, *ch);
                        self.input_cursor += 1;
                    }
                    EventResult::Handled
                }
                _ => EventResult::Handled,
            },
            _ => EventResult::Handled,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_dialog_creates_buttons() {
        let d = ModalDialog::confirm("Save", "Save changes?", &["Save", "Don't Save", "Cancel"]);
        assert_eq!(d.buttons.len(), 3);
        assert!(d.buttons[0].is_primary);
        assert!(d.buttons[2].is_cancel);
    }

    #[test]
    fn input_dialog_captures_value() {
        let mut d = ModalDialog::show_input("File name:", "untitled.rs");
        d.open();
        assert_eq!(d.input.as_ref().unwrap().value, "untitled.rs");
        d.close_with(DialogResult::Button(0));
        assert_eq!(d.input_value(), Some("untitled.rs"));
    }

    #[test]
    fn error_dialog_has_single_ok() {
        let d = ModalDialog::show_error("File not found", "/path/to/file");
        assert_eq!(d.buttons.len(), 1);
        assert!(d.buttons[0].is_primary);
    }

    #[test]
    fn checkbox_toggles() {
        let mut d = ModalDialog::new("Confirm", "Do it?")
            .with_checkbox(DialogCheckbox::new("Don't show again"));
        d.open();
        assert!(!d.checkbox_checked());
        d.checkbox.as_mut().unwrap().checked = true;
        assert!(d.checkbox_checked());
    }

    #[test]
    fn escape_cancels() {
        let mut d = ModalDialog::confirm("Test", "msg", &["OK"]);
        d.open();
        let viewport = Rect::new(0.0, 0.0, 800.0, 600.0);
        let event = UiEvent::KeyPress {
            key: Key::Escape,
            modifiers: crate::widget::Modifiers::NONE,
        };
        d.handle_event(&event, viewport);
        assert!(!d.is_open);
        assert_eq!(d.result, Some(DialogResult::Cancelled));
    }
}
