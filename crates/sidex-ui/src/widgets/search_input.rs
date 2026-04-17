//! Search input widget with toggle buttons, history navigation, and regex
//! syntax validation.
//!
//! Used by both the editor find bar and the workspace search panel to provide
//! a consistent search input experience with inline option toggles.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::DrawContext;
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// Icon type for a search toggle button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleIcon {
    CaseSensitive,
    WholeWord,
    Regex,
    PreserveCase,
    InSelection,
}

impl ToggleIcon {
    /// Returns the display label for this toggle icon.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::CaseSensitive => "Aa",
            Self::WholeWord => "Ab|",
            Self::Regex => ".*",
            Self::PreserveCase => "AB",
            Self::InSelection => "[]",
        }
    }

    /// Returns the tooltip text for this toggle icon.
    #[must_use]
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::CaseSensitive => "Match Case",
            Self::WholeWord => "Match Whole Word",
            Self::Regex => "Use Regular Expression",
            Self::PreserveCase => "Preserve Case",
            Self::InSelection => "Find in Selection",
        }
    }
}

/// A toggle button displayed inline within the search input.
#[derive(Debug, Clone)]
pub struct SearchToggle {
    pub id: String,
    pub label: String,
    pub tooltip: String,
    pub icon: ToggleIcon,
    pub active: bool,
}

impl SearchToggle {
    pub fn new(icon: ToggleIcon, active: bool) -> Self {
        Self {
            id: format!("{icon:?}"),
            label: icon.label().to_string(),
            tooltip: icon.tooltip().to_string(),
            icon,
            active,
        }
    }
}

/// A search/replace input field with inline toggle buttons and history.
#[allow(dead_code)]
pub struct SearchInput {
    pub text: String,
    pub placeholder: String,
    pub cursor_pos: usize,
    pub selection: Option<(usize, usize)>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub toggles: Vec<SearchToggle>,
    pub is_focused: bool,
    pub is_regex_mode: bool,
    pub is_regex_valid: bool,

    font_size: f32,
    toggle_size: f32,
    background: Color,
    foreground: Color,
    placeholder_color: Color,
    border_color: Color,
    focus_border_color: Color,
    error_border_color: Color,
    selection_color: Color,
    cursor_color: Color,
    toggle_active_bg: Color,
    toggle_inactive_bg: Color,
    toggle_hover_bg: Color,
    toggle_fg: Color,
    scroll_x: f32,
    blink_timer: f32,
    hovered_toggle: Option<usize>,
}

impl Default for SearchInput {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchInput {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            placeholder: String::new(),
            cursor_pos: 0,
            selection: None,
            history: Vec::new(),
            history_index: None,
            toggles: Vec::new(),
            is_focused: false,
            is_regex_mode: false,
            is_regex_valid: true,

            font_size: 13.0,
            toggle_size: 20.0,
            background: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            placeholder_color: Color::from_hex("#cccccc80").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            focus_border_color: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            error_border_color: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            selection_color: Color::from_hex("#264f78").unwrap_or(Color::BLACK),
            cursor_color: Color::from_hex("#aeafad").unwrap_or(Color::WHITE),
            toggle_active_bg: Color::from_hex("#3a3a3a").unwrap_or(Color::BLACK),
            toggle_inactive_bg: Color::TRANSPARENT,
            toggle_hover_bg: Color::from_hex("#404040").unwrap_or(Color::BLACK),
            toggle_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            scroll_x: 0.0,
            blink_timer: 0.0,
            hovered_toggle: None,
        }
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_toggles(mut self, toggles: Vec<SearchToggle>) -> Self {
        self.toggles = toggles;
        self
    }

    /// Advance the blink timer by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.blink_timer = (self.blink_timer + dt) % 1.0;
    }

    // ── Text editing ────────────────────────────────────────────────────

    fn has_selection(&self) -> bool {
        self.selection.is_some()
    }

    fn selection_range(&self) -> Option<(usize, usize)> {
        self.selection.map(|(a, b)| (a.min(b), a.max(b)))
    }

    fn delete_selection(&mut self) {
        if let Some((lo, hi)) = self.selection_range() {
            self.text.drain(lo..hi);
            self.cursor_pos = lo;
            self.selection = None;
        }
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        if self.has_selection() {
            self.delete_selection();
        }
        self.text.insert(self.cursor_pos, ch);
        self.cursor_pos += ch.len_utf8();
    }

    /// Insert a string at the cursor (paste support).
    pub fn insert_str(&mut self, s: &str) {
        if self.has_selection() {
            self.delete_selection();
        }
        self.text.insert_str(self.cursor_pos, s);
        self.cursor_pos += s.len();
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.cursor_pos > 0 {
            let prev = self.text[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            self.text.drain(prev..self.cursor_pos);
            self.cursor_pos = prev;
        }
    }

    /// Move the cursor left, optionally extending selection.
    pub fn move_left(&mut self, shift: bool) {
        if self.cursor_pos > 0 {
            let prev = self.text[..self.cursor_pos]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i);
            if shift {
                let anchor = self.selection.map_or(self.cursor_pos, |(a, _)| a);
                self.cursor_pos = prev;
                self.selection = Some((anchor, self.cursor_pos));
            } else {
                self.cursor_pos = prev;
                self.selection = None;
            }
        }
    }

    /// Move the cursor right, optionally extending selection.
    pub fn move_right(&mut self, shift: bool) {
        if self.cursor_pos < self.text.len() {
            let next = self.text[self.cursor_pos..]
                .char_indices()
                .nth(1)
                .map_or(self.text.len(), |(i, _)| self.cursor_pos + i);
            if shift {
                let anchor = self.selection.map_or(self.cursor_pos, |(a, _)| a);
                self.cursor_pos = next;
                self.selection = Some((anchor, self.cursor_pos));
            } else {
                self.cursor_pos = next;
                self.selection = None;
            }
        }
    }

    /// Select all text (Ctrl+A).
    pub fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.selection = Some((0, self.text.len()));
            self.cursor_pos = self.text.len();
        }
    }

    // ── History navigation ──────────────────────────────────────────────

    /// Push current text into history.
    pub fn push_history(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let t = self.text.clone();
        self.history.retain(|e| *e != t);
        self.history.insert(0, t);
        if self.history.len() > 50 {
            self.history.truncate(50);
        }
        self.history_index = None;
    }

    /// Navigate to the previous history entry (Up arrow).
    pub fn history_prev(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }
        let idx = match self.history_index {
            Some(i) => (i + 1).min(self.history.len() - 1),
            None => 0,
        };
        self.history_index = Some(idx);
        if let Some(entry) = self.history.get(idx) {
            self.text = entry.clone();
            self.cursor_pos = self.text.len();
            self.selection = None;
        }
        true
    }

    /// Navigate to the next history entry (Down arrow).
    pub fn history_next(&mut self) -> bool {
        let Some(idx) = self.history_index else {
            return false;
        };
        if let Some(new_idx) = idx.checked_sub(1) {
            self.history_index = Some(new_idx);
            if let Some(entry) = self.history.get(new_idx) {
                self.text = entry.clone();
                self.cursor_pos = self.text.len();
                self.selection = None;
            }
        } else {
            self.history_index = None;
            self.text.clear();
            self.cursor_pos = 0;
            self.selection = None;
        }
        true
    }

    // ── Regex validation ────────────────────────────────────────────────

    /// Validate the current text as a regex pattern.
    pub fn validate_regex(&mut self) {
        if !self.is_regex_mode || self.text.is_empty() {
            self.is_regex_valid = true;
            return;
        }
        self.is_regex_valid = regex::Regex::new(&self.text).is_ok();
    }

    // ── Toggle buttons ──────────────────────────────────────────────────

    /// Toggle a toggle button by index.
    pub fn toggle_button(&mut self, index: usize) -> bool {
        if let Some(toggle) = self.toggles.get_mut(index) {
            toggle.active = !toggle.active;
            true
        } else {
            false
        }
    }

    /// Returns the active state of a toggle by its icon type.
    pub fn is_toggle_active(&self, icon: ToggleIcon) -> bool {
        self.toggles
            .iter()
            .any(|t| t.icon == icon && t.active)
    }

    // ── Rendering helpers ───────────────────────────────────────────────

    fn char_width(&self) -> f32 {
        self.font_size * 0.6
    }

    #[allow(clippy::cast_precision_loss)]
    fn cursor_x_offset(&self) -> f32 {
        self.text[..self.cursor_pos].chars().count() as f32 * self.char_width()
    }

    fn toggles_width(&self) -> f32 {
        if self.toggles.is_empty() {
            0.0
        } else {
            self.toggles.len() as f32 * (self.toggle_size + 2.0) + 4.0
        }
    }

    fn border_color(&self) -> Color {
        if self.is_regex_mode && !self.is_regex_valid {
            self.error_border_color
        } else if self.is_focused {
            self.focus_border_color
        } else {
            self.border_color
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        ctx.draw_rect(rect, self.background, 2.0);
        ctx.draw_border(rect, self.border_color(), 1.0, 2.0);

        let inner_pad = 6.0;
        let toggle_w = self.toggles_width();
        let text_area_w = rect.width - inner_pad * 2.0 - toggle_w;
        let text_y = rect.y + (rect.height - self.font_size) / 2.0;

        // Selection highlight
        if let Some((lo, hi)) = self.selection_range() {
            let char_w = self.char_width();
            let sel_x = rect.x + inner_pad + self.text[..lo].chars().count() as f32 * char_w
                - self.scroll_x;
            let sel_w = self.text[lo..hi].chars().count() as f32 * char_w;
            let sel_rect = Rect::new(sel_x.max(rect.x + inner_pad), rect.y + 2.0, sel_w.min(text_area_w), rect.height - 4.0);
            ctx.draw_rect(sel_rect, self.selection_color, 0.0);
        }

        // Text or placeholder
        if self.text.is_empty() {
            if !self.placeholder.is_empty() {
                ctx.draw_text(
                    &self.placeholder,
                    (rect.x + inner_pad, text_y),
                    self.placeholder_color,
                    self.font_size,
                    false,
                    true,
                );
            }
        } else {
            ctx.draw_text(
                &self.text,
                (rect.x + inner_pad - self.scroll_x, text_y),
                self.foreground,
                self.font_size,
                false,
                false,
            );
        }

        // Blinking cursor
        if self.is_focused && self.blink_timer < 0.5 {
            let cx = rect.x + inner_pad + self.cursor_x_offset() - self.scroll_x;
            let cursor_rect = Rect::new(cx, rect.y + 3.0, 1.0, rect.height - 6.0);
            ctx.draw_rect(cursor_rect, self.cursor_color, 0.0);
        }

        // Toggle buttons on the right
        let mut tx = rect.x + rect.width - toggle_w;
        for (i, toggle) in self.toggles.iter().enumerate() {
            let bg = if toggle.active {
                self.toggle_active_bg
            } else if self.hovered_toggle == Some(i) {
                self.toggle_hover_bg
            } else {
                self.toggle_inactive_bg
            };

            let tr = Rect::new(tx, rect.y + (rect.height - self.toggle_size) / 2.0, self.toggle_size, self.toggle_size);
            ctx.draw_rect(tr, bg, 2.0);
            if toggle.active {
                ctx.draw_border(tr, self.focus_border_color, 1.0, 2.0);
            }

            let label = toggle.icon.label();
            let lx = tx + (self.toggle_size - label.len() as f32 * self.font_size * 0.6) / 2.0;
            let ly = tr.y + (self.toggle_size - self.font_size) / 2.0;
            ctx.draw_text(label, (lx, ly), self.toggle_fg, self.font_size - 1.0, toggle.active, false);

            tx += self.toggle_size + 2.0;
        }
    }
}

impl Widget for SearchInput {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Flex(1.0),
            padding: Edges::symmetric(6.0, 4.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, self.background, 2.0);
        let border = self.border_color();
        rr.draw_border(rect.x, rect.y, rect.width, rect.height, border, 1.0);
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => {
                self.is_focused = true;
                self.blink_timer = 0.0;
                EventResult::Handled
            }
            UiEvent::Blur => {
                self.is_focused = false;
                EventResult::Handled
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.is_focused = true;
                self.blink_timer = 0.0;

                // Check if click is on a toggle button
                let toggle_w = self.toggles_width();
                let toggle_start_x = rect.x + rect.width - toggle_w;
                if *x >= toggle_start_x {
                    let rel = *x - toggle_start_x;
                    let idx = (rel / (self.toggle_size + 2.0)) as usize;
                    if idx < self.toggles.len() {
                        self.toggle_button(idx);
                        return EventResult::Handled;
                    }
                }

                EventResult::Handled
            }
            UiEvent::MouseMove { x, y } if rect.contains(*x, *y) => {
                let toggle_w = self.toggles_width();
                let toggle_start_x = rect.x + rect.width - toggle_w;
                if *x >= toggle_start_x {
                    let rel = *x - toggle_start_x;
                    let idx = (rel / (self.toggle_size + 2.0)) as usize;
                    self.hovered_toggle = if idx < self.toggles.len() {
                        Some(idx)
                    } else {
                        None
                    };
                } else {
                    self.hovered_toggle = None;
                }
                EventResult::Ignored
            }
            UiEvent::KeyPress { key, modifiers } if self.is_focused => {
                self.blink_timer = 0.0;
                match key {
                    Key::Char('a') if modifiers.command() => {
                        self.select_all();
                        EventResult::Handled
                    }
                    Key::Char(ch) if !modifiers.command() => {
                        self.insert_char(*ch);
                        if self.is_regex_mode {
                            self.validate_regex();
                        }
                        EventResult::Handled
                    }
                    Key::Backspace => {
                        self.backspace();
                        if self.is_regex_mode {
                            self.validate_regex();
                        }
                        EventResult::Handled
                    }
                    Key::ArrowLeft => {
                        self.move_left(modifiers.shift);
                        EventResult::Handled
                    }
                    Key::ArrowRight => {
                        self.move_right(modifiers.shift);
                        EventResult::Handled
                    }
                    Key::ArrowUp => {
                        self.history_prev();
                        EventResult::Handled
                    }
                    Key::ArrowDown => {
                        self.history_next();
                        EventResult::Handled
                    }
                    Key::Home => {
                        self.cursor_pos = 0;
                        if !modifiers.shift {
                            self.selection = None;
                        }
                        self.scroll_x = 0.0;
                        EventResult::Handled
                    }
                    Key::End => {
                        self.cursor_pos = self.text.len();
                        if !modifiers.shift {
                            self.selection = None;
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_backspace() {
        let mut input = SearchInput::new();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.text, "hi");
        assert_eq!(input.cursor_pos, 2);

        input.backspace();
        assert_eq!(input.text, "h");
        assert_eq!(input.cursor_pos, 1);
    }

    #[test]
    fn select_all_and_delete() {
        let mut input = SearchInput::new();
        input.text = "hello".to_string();
        input.cursor_pos = 5;
        input.select_all();
        assert_eq!(input.selection, Some((0, 5)));

        input.insert_char('x');
        assert_eq!(input.text, "x");
        assert!(input.selection.is_none());
    }

    #[test]
    fn history_navigation() {
        let mut input = SearchInput::new();
        input.text = "first".to_string();
        input.push_history();
        input.text = "second".to_string();
        input.push_history();

        input.text.clear();
        input.history_prev();
        assert_eq!(input.text, "second");

        input.history_prev();
        assert_eq!(input.text, "first");

        input.history_next();
        assert_eq!(input.text, "second");

        input.history_next();
        assert!(input.text.is_empty());
    }

    #[test]
    fn toggle_buttons() {
        let mut input = SearchInput::new().with_toggles(vec![
            SearchToggle::new(ToggleIcon::CaseSensitive, false),
            SearchToggle::new(ToggleIcon::WholeWord, false),
            SearchToggle::new(ToggleIcon::Regex, false),
        ]);

        assert!(!input.is_toggle_active(ToggleIcon::CaseSensitive));
        input.toggle_button(0);
        assert!(input.is_toggle_active(ToggleIcon::CaseSensitive));
    }

    #[test]
    fn regex_validation() {
        let mut input = SearchInput::new();
        input.is_regex_mode = true;
        input.text = "[invalid".to_string();
        input.validate_regex();
        assert!(!input.is_regex_valid);

        input.text = r"\w+".to_string();
        input.validate_regex();
        assert!(input.is_regex_valid);
    }

    #[test]
    fn regex_valid_when_disabled() {
        let mut input = SearchInput::new();
        input.is_regex_mode = false;
        input.text = "[invalid".to_string();
        input.validate_regex();
        assert!(input.is_regex_valid);
    }

    #[test]
    fn toggle_icon_labels() {
        assert_eq!(ToggleIcon::CaseSensitive.label(), "Aa");
        assert_eq!(ToggleIcon::WholeWord.label(), "Ab|");
        assert_eq!(ToggleIcon::Regex.label(), ".*");
        assert_eq!(ToggleIcon::PreserveCase.label(), "AB");
        assert_eq!(ToggleIcon::InSelection.label(), "[]");
    }

    #[test]
    fn paste_support() {
        let mut input = SearchInput::new();
        input.insert_str("hello world");
        assert_eq!(input.text, "hello world");
        assert_eq!(input.cursor_pos, 11);
    }

    #[test]
    fn move_left_right() {
        let mut input = SearchInput::new();
        input.text = "abc".to_string();
        input.cursor_pos = 3;

        input.move_left(false);
        assert_eq!(input.cursor_pos, 2);

        input.move_right(false);
        assert_eq!(input.cursor_pos, 3);
    }

    #[test]
    fn move_with_shift_creates_selection() {
        let mut input = SearchInput::new();
        input.text = "abc".to_string();
        input.cursor_pos = 1;

        input.move_right(true);
        assert_eq!(input.selection, Some((1, 2)));

        input.move_right(true);
        assert_eq!(input.selection, Some((1, 3)));
    }

    #[test]
    fn history_dedup() {
        let mut input = SearchInput::new();
        input.text = "foo".to_string();
        input.push_history();
        input.text = "bar".to_string();
        input.push_history();
        input.text = "foo".to_string();
        input.push_history();

        assert_eq!(input.history.len(), 2);
        assert_eq!(input.history[0], "foo");
        assert_eq!(input.history[1], "bar");
    }
}
