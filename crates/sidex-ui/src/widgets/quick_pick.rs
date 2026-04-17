//! Command palette / quick-pick widget with fuzzy filtering.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId};
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// A single item in the quick-pick list.
#[derive(Clone, Debug)]
pub struct QuickPickItem {
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    pub group: Option<String>,
    pub picked: bool,
}

impl QuickPickItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
            detail: None,
            group: None,
            picked: false,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }
}

/// A command-palette style picker with fuzzy filter and optional multi-select.
#[allow(dead_code)]
pub struct QuickPick<F: FnMut(usize)> {
    pub items: Vec<QuickPickItem>,
    pub placeholder: String,
    pub on_select: F,
    pub multi_select: bool,

    filter_text: String,
    filtered_indices: Vec<usize>,
    selected_index: usize,
    scroll_offset: f32,
    visible: bool,

    row_height: f32,
    max_visible_items: usize,
    width: f32,
    font_size: f32,

    background: Color,
    border_color: Color,
    shadow_color: Color,
    input_bg: Color,
    foreground: Color,
    highlight_fg: Color,
    selected_bg: Color,
    description_fg: Color,
    group_header_fg: Color,
    no_results_fg: Color,
    placeholder_fg: Color,
}

impl<F: FnMut(usize)> QuickPick<F> {
    pub fn new(items: Vec<QuickPickItem>, on_select: F) -> Self {
        let count = items.len();
        let indices: Vec<usize> = (0..count).collect();
        Self {
            items,
            placeholder: "Type to search...".into(),
            on_select,
            multi_select: false,
            filter_text: String::new(),
            filtered_indices: indices,
            selected_index: 0,
            scroll_offset: 0.0,
            visible: true,
            row_height: 26.0,
            max_visible_items: 12,
            width: 600.0,
            font_size: 13.0,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#00000080").unwrap_or(Color::BLACK),
            input_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            highlight_fg: Color::from_hex("#18a3ff").unwrap_or(Color::WHITE),
            selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            description_fg: Color::from_hex("#aaaaaa").unwrap_or(Color::WHITE),
            group_header_fg: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
            no_results_fg: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
            placeholder_fg: Color::from_hex("#aaaaaa").unwrap_or(Color::WHITE),
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.filter_text.clear();
        self.selected_index = 0;
        self.scroll_offset = 0.0;
        self.refilter();
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    fn refilter(&mut self) {
        if self.filter_text.is_empty() {
            self.filtered_indices = (0..self.items.len()).collect();
        } else {
            let query = self.filter_text.to_lowercase();
            self.filtered_indices = (0..self.items.len())
                .filter(|&i| fuzzy_match(&self.items[i].label, &query))
                .collect();
        }
        self.selected_index = 0;
        self.scroll_offset = 0.0;
    }

    fn visible_count(&self) -> usize {
        self.filtered_indices.len().min(self.max_visible_items)
    }

    fn list_height(&self) -> f32 {
        self.visible_count() as f32 * self.row_height
    }

    fn input_height(&self) -> f32 {
        32.0
    }

    fn panel_rect(&self, viewport_width: f32) -> Rect {
        let x = (viewport_width - self.width) / 2.0;
        let total_h = self.input_height() + self.list_height() + 8.0;
        Rect::new(x.max(0.0), 80.0, self.width, total_h)
    }

    fn ensure_selected_visible(&mut self) {
        let top = self.selected_index as f32 * self.row_height;
        let bottom = top + self.row_height;
        let vis = self.visible_count() as f32 * self.row_height;
        if top < self.scroll_offset {
            self.scroll_offset = top;
        } else if bottom > self.scroll_offset + vis {
            self.scroll_offset = bottom - vis;
        }
    }

    /// Computes fuzzy match positions for highlighting matched characters.
    fn match_positions(label: &str, query: &str) -> Vec<usize> {
        let label_lower = label.to_lowercase();
        let mut positions = Vec::new();
        let mut label_iter = label_lower.char_indices();
        for qc in query.chars() {
            for (idx, lc) in label_iter.by_ref() {
                if lc == qc {
                    positions.push(idx);
                    break;
                }
            }
        }
        positions
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        if !self.visible {
            return;
        }
        let pr = self.panel_rect(rect.width);

        // Shadow
        let shadow = Rect::new(pr.x + 3.0, pr.y + 3.0, pr.width, pr.height);
        ctx.draw_rect(shadow, self.shadow_color, 6.0);

        // Background
        ctx.draw_rect(pr, self.background, 6.0);
        ctx.draw_border(pr, self.border_color, 1.0, 6.0);

        // Input field
        let input_r = Rect::new(pr.x + 8.0, pr.y + 4.0, pr.width - 16.0, self.input_height());
        ctx.draw_rect(input_r, self.input_bg, 2.0);

        // Search icon
        ctx.draw_icon(
            IconId::Search,
            (input_r.x + 6.0, input_r.y + 8.0),
            14.0,
            self.description_fg,
        );

        // Input text or placeholder
        let text_x = input_r.x + 24.0;
        let text_y = input_r.y + (input_r.height - self.font_size) / 2.0;
        if self.filter_text.is_empty() {
            ctx.draw_text(
                &self.placeholder,
                (text_x, text_y),
                self.placeholder_fg,
                self.font_size,
                false,
                true,
            );
        } else {
            ctx.draw_text(
                &self.filter_text,
                (text_x, text_y),
                self.foreground,
                self.font_size,
                false,
                false,
            );
        }

        // Input cursor
        let cursor_x = text_x + self.filter_text.len() as f32 * self.font_size * 0.6;
        let cursor_rect = Rect::new(cursor_x, text_y, 1.0, self.font_size);
        ctx.draw_rect(cursor_rect, self.foreground, 0.0);

        // List
        let list_y = pr.y + self.input_height() + 4.0;
        ctx.save();
        let list_clip = Rect::new(pr.x, list_y, pr.width, self.list_height());
        ctx.clip(list_clip);

        if self.filtered_indices.is_empty() {
            // "No results" message
            let no_y = list_y + 4.0;
            ctx.draw_text(
                "No results",
                (pr.x + 16.0, no_y),
                self.no_results_fg,
                self.font_size,
                false,
                true,
            );
        } else {
            let query_lower = self.filter_text.to_lowercase();
            let mut last_group: Option<&str> = None;

            for (vi, &item_idx) in self.filtered_indices.iter().enumerate() {
                let y = list_y + vi as f32 * self.row_height - self.scroll_offset;
                if y + self.row_height < list_y || y > list_y + self.list_height() {
                    continue;
                }

                let item = &self.items[item_idx];

                // Group header separator
                if let Some(ref group) = item.group {
                    if last_group != Some(group.as_str()) {
                        if last_group.is_some() {
                            let sep = Rect::new(pr.x + 8.0, y - 1.0, pr.width - 16.0, 1.0);
                            ctx.draw_rect(sep, self.border_color, 0.0);
                        }
                        last_group = Some(group.as_str());
                    }
                }

                // Selected item background
                if vi == self.selected_index {
                    let sel_rect = Rect::new(pr.x + 4.0, y, pr.width - 8.0, self.row_height);
                    ctx.draw_rect(sel_rect, self.selected_bg, 2.0);
                }

                // Fuzzy match highlighting: bold matched characters
                let text_y_item = y + (self.row_height - self.font_size) / 2.0;
                if !query_lower.is_empty() {
                    let positions = Self::match_positions(&item.label, &query_lower);
                    let mut spans: Vec<(String, crate::draw::TextStyle)> = Vec::new();
                    let mut last_end = 0;
                    for &pos in &positions {
                        if pos > last_end {
                            spans.push((
                                item.label[last_end..pos].to_string(),
                                crate::draw::TextStyle {
                                    color: self.foreground,
                                    bold: false,
                                    italic: false,
                                },
                            ));
                        }
                        let ch_end =
                            pos + item.label[pos..].chars().next().map_or(1, |c| c.len_utf8());
                        spans.push((
                            item.label[pos..ch_end].to_string(),
                            crate::draw::TextStyle {
                                color: self.highlight_fg,
                                bold: true,
                                italic: false,
                            },
                        ));
                        last_end = ch_end;
                    }
                    if last_end < item.label.len() {
                        spans.push((
                            item.label[last_end..].to_string(),
                            crate::draw::TextStyle {
                                color: self.foreground,
                                bold: false,
                                italic: false,
                            },
                        ));
                    }
                    ctx.draw_styled_text(&spans, (pr.x + 16.0, text_y_item), self.font_size);
                } else {
                    ctx.draw_text(
                        &item.label,
                        (pr.x + 16.0, text_y_item),
                        self.foreground,
                        self.font_size,
                        false,
                        false,
                    );
                }

                // Description (right-aligned, dimmed)
                if let Some(ref desc) = item.description {
                    let desc_w = desc.len() as f32 * self.font_size * 0.6;
                    let desc_x = pr.x + pr.width - desc_w - 12.0;
                    ctx.draw_text(
                        desc,
                        (desc_x, text_y_item),
                        self.description_fg,
                        self.font_size,
                        false,
                        false,
                    );
                }
            }
        }

        ctx.restore();
    }
}

impl<F: FnMut(usize)> Widget for QuickPick<F> {
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
        let pr = self.panel_rect(rect.width);
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(pr.x, pr.y, pr.width, pr.height, self.background, 6.0);
        rr.draw_border(pr.x, pr.y, pr.width, pr.height, self.border_color, 1.0);
        let input_r = Rect::new(pr.x + 8.0, pr.y + 4.0, pr.width - 16.0, self.input_height());
        rr.draw_rect(
            input_r.x,
            input_r.y,
            input_r.width,
            input_r.height,
            self.input_bg,
            2.0,
        );
        let list_y = pr.y + self.input_height() + 4.0;
        for (vi, &item_idx) in self.filtered_indices.iter().enumerate() {
            let y = list_y + vi as f32 * self.row_height - self.scroll_offset;
            if y + self.row_height < list_y || y > list_y + self.list_height() {
                continue;
            }
            if vi == self.selected_index {
                rr.draw_rect(
                    pr.x + 4.0,
                    y,
                    pr.width - 8.0,
                    self.row_height,
                    self.selected_bg,
                    2.0,
                );
            }
            let _ = &self.items[item_idx];
        }
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
                Key::ArrowDown => {
                    if !self.filtered_indices.is_empty() {
                        self.selected_index =
                            (self.selected_index + 1).min(self.filtered_indices.len() - 1);
                        self.ensure_selected_visible();
                    }
                    EventResult::Handled
                }
                Key::ArrowUp => {
                    self.selected_index = self.selected_index.saturating_sub(1);
                    self.ensure_selected_visible();
                    EventResult::Handled
                }
                Key::Enter => {
                    if let Some(&item_idx) = self.filtered_indices.get(self.selected_index) {
                        if self.multi_select {
                            self.items[item_idx].picked = !self.items[item_idx].picked;
                        }
                        (self.on_select)(item_idx);
                        if !self.multi_select {
                            self.hide();
                        }
                    }
                    EventResult::Handled
                }
                Key::Backspace => {
                    self.filter_text.pop();
                    self.refilter();
                    EventResult::Handled
                }
                Key::Char(ch) if !modifiers.command() => {
                    self.filter_text.push(*ch);
                    self.refilter();
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            },
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let pr = self.panel_rect(rect.width);
                if !pr.contains(*x, *y) {
                    self.hide();
                    return EventResult::Handled;
                }
                let list_y = pr.y + self.input_height() + 4.0;
                if *y >= list_y {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let vi = ((y - list_y + self.scroll_offset) / self.row_height) as usize;
                    if let Some(&item_idx) = self.filtered_indices.get(vi) {
                        self.selected_index = vi;
                        (self.on_select)(item_idx);
                        if !self.multi_select {
                            self.hide();
                        }
                    }
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

fn fuzzy_match(haystack: &str, query: &str) -> bool {
    let hay = haystack.to_lowercase();
    let mut hay_chars = hay.chars();
    for qc in query.chars() {
        loop {
            match hay_chars.next() {
                Some(hc) if hc == qc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_match_basic() {
        assert!(fuzzy_match("Open File", "opfi"));
        assert!(fuzzy_match("Toggle Sidebar", "tgsb"));
        assert!(!fuzzy_match("abc", "abdc"));
    }

    #[test]
    fn fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("FooBar", "foob"));
        assert!(fuzzy_match("FooBar", "fb"));
    }
}
