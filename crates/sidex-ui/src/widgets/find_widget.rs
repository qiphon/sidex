//! Find/Replace bar widget.
//!
//! The search bar that appears at the top-right of the editor with toggle
//! buttons for case sensitivity, whole word, and regex, plus match
//! navigation and an expandable replace row.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId};
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// The find/replace bar that appears at the top-right of the editor.
#[allow(dead_code)]
pub struct FindWidget {
    pub search_text: String,
    pub replace_text: String,
    pub show_replace: bool,
    pub match_case: bool,
    pub whole_word: bool,
    pub use_regex: bool,
    pub current_match: usize,
    pub total_matches: usize,
    pub preserve_case: bool,

    visible: bool,
    focused_field: FocusedField,
    search_cursor: usize,
    replace_cursor: usize,

    widget_width: f32,
    row_height: f32,
    input_height: f32,
    button_size: f32,
    font_size: f32,
    right_margin: f32,
    top_margin: f32,

    background: Color,
    border_color: Color,
    shadow_color: Color,
    input_bg: Color,
    input_focus_border: Color,
    foreground: Color,
    match_count_fg: Color,
    button_fg: Color,
    button_active_bg: Color,
    button_hover_bg: Color,
    close_hover_bg: Color,
    no_match_border: Color,

    hovered_button: Option<FindButton>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusedField {
    Search,
    Replace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum FindButton {
    MatchCase,
    WholeWord,
    UseRegex,
    PrevMatch,
    NextMatch,
    Close,
    ReplaceToggle,
    Replace,
    ReplaceAll,
    PreserveCase,
}

impl Default for FindWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl FindWidget {
    pub fn new() -> Self {
        Self {
            search_text: String::new(),
            replace_text: String::new(),
            show_replace: false,
            match_case: false,
            whole_word: false,
            use_regex: false,
            current_match: 0,
            total_matches: 0,
            preserve_case: false,
            visible: false,
            focused_field: FocusedField::Search,
            search_cursor: 0,
            replace_cursor: 0,
            widget_width: 411.0,
            row_height: 33.0,
            input_height: 24.0,
            button_size: 22.0,
            font_size: 13.0,
            right_margin: 14.0,
            top_margin: 0.0,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#00000040").unwrap_or(Color::BLACK),
            input_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            input_focus_border: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            match_count_fg: Color::from_hex("#9d9d9d").unwrap_or(Color::WHITE),
            button_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            button_active_bg: Color::from_hex("#3a3a3a").unwrap_or(Color::BLACK),
            button_hover_bg: Color::from_hex("#404040").unwrap_or(Color::BLACK),
            close_hover_bg: Color::from_hex("#404040").unwrap_or(Color::BLACK),
            no_match_border: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            hovered_button: None,
        }
    }

    /// Opens the find bar, optionally with a pre-filled search term.
    pub fn show(&mut self, initial_text: Option<&str>) {
        self.visible = true;
        self.focused_field = FocusedField::Search;
        if let Some(text) = initial_text {
            self.search_text = text.to_string();
            self.search_cursor = self.search_text.len();
        }
    }

    /// Closes the find bar.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Toggles the replace row visibility.
    pub fn toggle_replace(&mut self) {
        self.show_replace = !self.show_replace;
    }

    /// Toggles case-sensitive matching.
    pub fn toggle_match_case(&mut self) {
        self.match_case = !self.match_case;
    }

    /// Toggles whole-word matching.
    pub fn toggle_whole_word(&mut self) {
        self.whole_word = !self.whole_word;
    }

    /// Toggles regex mode.
    pub fn toggle_regex(&mut self) {
        self.use_regex = !self.use_regex;
    }

    /// Navigates to the next match.
    pub fn next_match(&mut self) {
        if self.total_matches > 0 {
            self.current_match = (self.current_match % self.total_matches) + 1;
        }
    }

    /// Navigates to the previous match.
    pub fn prev_match(&mut self) {
        if self.total_matches > 0 {
            self.current_match = if self.current_match <= 1 {
                self.total_matches
            } else {
                self.current_match - 1
            };
        }
    }

    /// Updates match count information from the search engine.
    pub fn set_matches(&mut self, current: usize, total: usize) {
        self.current_match = current;
        self.total_matches = total;
    }

    /// Returns the match count display string (e.g. "3 of 12" or "No results").
    pub fn match_count_text(&self) -> String {
        if self.search_text.is_empty() {
            String::new()
        } else if self.total_matches == 0 {
            "No results".to_string()
        } else {
            format!("{} of {}", self.current_match, self.total_matches)
        }
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn widget_height(&self) -> f32 {
        if self.show_replace {
            self.row_height * 2.0 + 4.0
        } else {
            self.row_height + 2.0
        }
    }

    fn widget_rect(&self, editor_width: f32) -> Rect {
        let x = editor_width - self.widget_width - self.right_margin;
        Rect::new(
            x.max(0.0),
            self.top_margin,
            self.widget_width,
            self.widget_height(),
        )
    }

    fn active_search_text(&self) -> &str {
        &self.search_text
    }

    fn insert_char_in_field(&mut self, ch: char) {
        match self.focused_field {
            FocusedField::Search => {
                self.search_text.insert(self.search_cursor, ch);
                self.search_cursor += ch.len_utf8();
            }
            FocusedField::Replace => {
                self.replace_text.insert(self.replace_cursor, ch);
                self.replace_cursor += ch.len_utf8();
            }
        }
    }

    fn backspace_field(&mut self) {
        match self.focused_field {
            FocusedField::Search => {
                if self.search_cursor > 0 {
                    let prev = self.search_text[..self.search_cursor]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(i, _)| i);
                    self.search_text.drain(prev..self.search_cursor);
                    self.search_cursor = prev;
                }
            }
            FocusedField::Replace => {
                if self.replace_cursor > 0 {
                    let prev = self.replace_text[..self.replace_cursor]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(i, _)| i);
                    self.replace_text.drain(prev..self.replace_cursor);
                    self.replace_cursor = prev;
                }
            }
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, editor_rect: Rect) {
        if !self.visible {
            return;
        }
        let wr = self.widget_rect(editor_rect.width);

        // Shadow
        let shadow = Rect::new(wr.x + 2.0, wr.y + 2.0, wr.width, wr.height);
        ctx.draw_rect(shadow, self.shadow_color, 4.0);

        // Background
        ctx.draw_rect(wr, self.background, 4.0);
        ctx.draw_border(wr, self.border_color, 1.0, 4.0);

        let pad = 6.0;
        let toggle_area_start = wr.x + pad;
        let row_y = wr.y + 4.0;

        // Replace toggle chevron
        let chevron_icon = if self.show_replace {
            IconId::ChevronDown
        } else {
            IconId::ChevronRight
        };
        ctx.draw_icon(
            chevron_icon,
            (toggle_area_start, row_y + 5.0),
            14.0,
            self.button_fg,
        );

        // Search input
        let input_x = toggle_area_start + 20.0;
        let input_w = 200.0;
        let input_r = Rect::new(input_x, row_y, input_w, self.input_height);
        ctx.draw_rect(input_r, self.input_bg, 2.0);

        let input_border = if self.focused_field == FocusedField::Search {
            if self.total_matches == 0 && !self.search_text.is_empty() {
                self.no_match_border
            } else {
                self.input_focus_border
            }
        } else {
            self.input_bg
        };
        ctx.draw_border(input_r, input_border, 1.0, 2.0);

        let text_y = row_y + (self.input_height - self.font_size) / 2.0;
        if self.search_text.is_empty() {
            ctx.draw_text(
                "Find",
                (input_x + 4.0, text_y),
                self.match_count_fg,
                self.font_size,
                false,
                true,
            );
        } else {
            ctx.draw_text(
                self.active_search_text(),
                (input_x + 4.0, text_y),
                self.foreground,
                self.font_size,
                false,
                false,
            );
        }

        // Toggle buttons: Aa (case), |ab| (word), .* (regex)
        let mut bx = input_x + input_w + 4.0;
        let btn_y = row_y + 1.0;

        // Match Case button
        self.render_toggle_button(ctx, "Aa", bx, btn_y, self.match_case, FindButton::MatchCase);
        bx += self.button_size + 2.0;

        // Whole Word button
        self.render_toggle_button(ctx, "ab", bx, btn_y, self.whole_word, FindButton::WholeWord);
        bx += self.button_size + 2.0;

        // Regex button
        self.render_toggle_button(ctx, ".*", bx, btn_y, self.use_regex, FindButton::UseRegex);
        bx += self.button_size + 6.0;

        // Match count
        let count_text = self.match_count_text();
        if !count_text.is_empty() {
            ctx.draw_text(
                &count_text,
                (bx, text_y),
                self.match_count_fg,
                self.font_size - 1.0,
                false,
                false,
            );
            bx += count_text.len() as f32 * (self.font_size - 1.0) * 0.6 + 8.0;
        }

        // Prev/Next match arrows
        let arrow_y = row_y + 2.0;
        self.render_icon_button(ctx, IconId::ChevronRight, bx, arrow_y, FindButton::PrevMatch);
        bx += self.button_size + 2.0;
        self.render_icon_button(ctx, IconId::ChevronDown, bx, arrow_y, FindButton::NextMatch);
        bx += self.button_size + 2.0;

        // Close button
        self.render_icon_button(ctx, IconId::Close, bx, arrow_y, FindButton::Close);

        // Replace row
        if self.show_replace {
            let replace_y = row_y + self.row_height + 2.0;

            // Replace input
            let rep_r = Rect::new(input_x, replace_y, input_w, self.input_height);
            ctx.draw_rect(rep_r, self.input_bg, 2.0);
            let rep_border = if self.focused_field == FocusedField::Replace {
                self.input_focus_border
            } else {
                self.input_bg
            };
            ctx.draw_border(rep_r, rep_border, 1.0, 2.0);

            let rep_text_y = replace_y + (self.input_height - self.font_size) / 2.0;
            if self.replace_text.is_empty() {
                ctx.draw_text(
                    "Replace",
                    (input_x + 4.0, rep_text_y),
                    self.match_count_fg,
                    self.font_size,
                    false,
                    true,
                );
            } else {
                ctx.draw_text(
                    &self.replace_text,
                    (input_x + 4.0, rep_text_y),
                    self.foreground,
                    self.font_size,
                    false,
                    false,
                );
            }

            // Replace + Replace All buttons
            let mut rbx = input_x + input_w + 4.0;

            // Preserve case toggle
            self.render_toggle_button(
                ctx,
                "AB",
                rbx,
                replace_y + 1.0,
                self.preserve_case,
                FindButton::PreserveCase,
            );
            rbx += self.button_size + 4.0;

            // Replace button
            self.render_text_button(ctx, "Replace", rbx, replace_y + 1.0, FindButton::Replace);
            rbx += 60.0;

            // Replace All button
            self.render_text_button(ctx, "All", rbx, replace_y + 1.0, FindButton::ReplaceAll);
        }
    }

    fn render_toggle_button(
        &self,
        ctx: &mut DrawContext,
        label: &str,
        x: f32,
        y: f32,
        active: bool,
        id: FindButton,
    ) {
        let r = Rect::new(x, y, self.button_size, self.button_size);
        if active {
            ctx.draw_rect(r, self.button_active_bg, 2.0);
            ctx.draw_border(r, self.input_focus_border, 1.0, 2.0);
        } else if self.hovered_button == Some(id) {
            ctx.draw_rect(r, self.button_hover_bg, 2.0);
        }
        let ty = y + (self.button_size - self.font_size) / 2.0;
        let tx = x + (self.button_size - label.len() as f32 * self.font_size * 0.6) / 2.0;
        ctx.draw_text(label, (tx, ty), self.button_fg, self.font_size, active, false);
    }

    fn render_icon_button(
        &self,
        ctx: &mut DrawContext,
        icon: IconId,
        x: f32,
        y: f32,
        id: FindButton,
    ) {
        let r = Rect::new(x, y, self.button_size, self.button_size);
        if self.hovered_button == Some(id) {
            ctx.draw_rect(r, self.button_hover_bg, 2.0);
        }
        let ix = x + (self.button_size - 12.0) / 2.0;
        let iy = y + (self.button_size - 12.0) / 2.0;
        ctx.draw_icon(icon, (ix, iy), 12.0, self.button_fg);
    }

    #[allow(clippy::cast_precision_loss)]
    fn render_text_button(
        &self,
        ctx: &mut DrawContext,
        label: &str,
        x: f32,
        y: f32,
        id: FindButton,
    ) {
        let w = label.len() as f32 * self.font_size * 0.6 + 12.0;
        let r = Rect::new(x, y, w, self.button_size);
        if self.hovered_button == Some(id) {
            ctx.draw_rect(r, self.button_hover_bg, 2.0);
        }
        let ty = y + (self.button_size - self.font_size) / 2.0;
        ctx.draw_text(label, (x + 6.0, ty), self.button_fg, self.font_size, false, false);
    }
}

impl Widget for FindWidget {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Auto,
            padding: Edges::all(0.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible {
            return;
        }
        let wr = self.widget_rect(rect.width);
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(wr.x, wr.y, wr.width, wr.height, self.background, 4.0);
        rr.draw_border(wr.x, wr.y, wr.width, wr.height, self.border_color, 1.0);
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        match event {
            UiEvent::KeyPress { key, modifiers } => match key {
                Key::Escape => {
                    self.hide();
                    EventResult::Handled
                }
                Key::Enter => {
                    if modifiers.shift {
                        self.prev_match();
                    } else {
                        self.next_match();
                    }
                    EventResult::Handled
                }
                Key::Tab => {
                    if self.show_replace {
                        self.focused_field = match self.focused_field {
                            FocusedField::Search => FocusedField::Replace,
                            FocusedField::Replace => FocusedField::Search,
                        };
                    }
                    EventResult::Handled
                }
                Key::Char(ch) if !modifiers.command() => {
                    self.insert_char_in_field(*ch);
                    EventResult::Handled
                }
                Key::Backspace => {
                    self.backspace_field();
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            },
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let wr = self.widget_rect(rect.width);
                if !wr.contains(*x, *y) {
                    return EventResult::Ignored;
                }

                // Check chevron toggle
                let toggle_x = wr.x + 6.0;
                let toggle_r = Rect::new(toggle_x, wr.y + 4.0, 16.0, 16.0);
                if toggle_r.contains(*x, *y) {
                    self.toggle_replace();
                    return EventResult::Handled;
                }

                // Check close button area (approximate)
                let close_x = wr.x + wr.width - self.button_size - 4.0;
                let close_r = Rect::new(close_x, wr.y + 4.0, self.button_size, self.button_size);
                if close_r.contains(*x, *y) {
                    self.hide();
                    return EventResult::Handled;
                }

                // Focus search vs replace field
                let row_y = wr.y + 4.0;
                if *y < row_y + self.row_height {
                    self.focused_field = FocusedField::Search;
                } else if self.show_replace {
                    self.focused_field = FocusedField::Replace;
                }

                EventResult::Handled
            }
            UiEvent::MouseMove { x, y } => {
                let wr = self.widget_rect(rect.width);
                if !wr.contains(*x, *y) {
                    self.hovered_button = None;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_type() {
        let mut w = FindWidget::new();
        w.show(Some("hello"));
        assert!(w.is_visible());
        assert_eq!(w.search_text, "hello");
    }

    #[test]
    fn match_navigation() {
        let mut w = FindWidget::new();
        w.show(Some("test"));
        w.set_matches(1, 5);
        assert_eq!(w.match_count_text(), "1 of 5");

        w.next_match();
        assert_eq!(w.current_match, 2);

        w.prev_match();
        assert_eq!(w.current_match, 1);

        w.prev_match();
        assert_eq!(w.current_match, 5); // wraps
    }

    #[test]
    fn toggle_options() {
        let mut w = FindWidget::new();
        assert!(!w.match_case);
        w.toggle_match_case();
        assert!(w.match_case);
        w.toggle_whole_word();
        assert!(w.whole_word);
        w.toggle_regex();
        assert!(w.use_regex);
    }

    #[test]
    fn replace_toggle() {
        let mut w = FindWidget::new();
        w.show(None);
        assert!(!w.show_replace);
        w.toggle_replace();
        assert!(w.show_replace);
    }

    #[test]
    fn no_results_text() {
        let mut w = FindWidget::new();
        w.show(None);
        w.search_text = "xyz".into();
        w.set_matches(0, 0);
        assert_eq!(w.match_count_text(), "No results");
    }
}
