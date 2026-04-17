//! Autocomplete / suggestion popup widget.
//!
//! Renders the completion dropdown below the cursor, with fuzzy-match
//! highlighting, icon per item kind, and a detail/documentation side pane.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId, TextStyle};
use crate::layout::{Edges, LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// The kind of a completion item — drives the icon rendered in each row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionItemKind {
    Text,
    Method,
    Function,
    Constructor,
    Field,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Unit,
    Value,
    Enum,
    Keyword,
    Snippet,
    Color,
    File,
    Reference,
    Folder,
    EnumMember,
    Constant,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

/// A single completion item displayed in the suggest widget.
#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
    pub filter_text: Option<String>,
    pub sort_text: Option<String>,
    pub preselect: bool,
    /// Extension / source that contributed this item.
    pub source: Option<String>,
}

impl CompletionItem {
    pub fn new(label: impl Into<String>, kind: CompletionItemKind) -> Self {
        Self {
            label: label.into(),
            kind,
            detail: None,
            documentation: None,
            insert_text: None,
            filter_text: None,
            sort_text: None,
            preselect: false,
            source: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_documentation(mut self, doc: impl Into<String>) -> Self {
        self.documentation = Some(doc.into());
        self
    }

    fn effective_filter_text(&self) -> &str {
        self.filter_text.as_deref().unwrap_or(&self.label)
    }
}

/// The autocomplete dropdown that appears below the cursor.
#[allow(dead_code)]
pub struct SuggestWidget {
    pub items: Vec<CompletionItem>,
    filtered_items: Vec<usize>,
    selected: usize,
    visible: bool,
    position: (f32, f32),
    detail_visible: bool,

    scroll_offset: f32,
    row_height: f32,
    max_visible_items: usize,
    font_size: f32,
    min_width: f32,
    max_width: f32,
    detail_width: f32,
    max_detail_height: f32,

    background: Color,
    border_color: Color,
    shadow_color: Color,
    foreground: Color,
    selected_bg: Color,
    selected_fg: Color,
    highlight_fg: Color,
    detail_fg: Color,
    icon_fg: Color,
    detail_pane_bg: Color,
    detail_pane_border: Color,
    loading_fg: Color,
}

impl Default for SuggestWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl SuggestWidget {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered_items: Vec::new(),
            selected: 0,
            visible: false,
            position: (0.0, 0.0),
            detail_visible: false,
            scroll_offset: 0.0,
            row_height: 24.0,
            max_visible_items: 12,
            font_size: 13.0,
            min_width: 300.0,
            max_width: 600.0,
            detail_width: 300.0,
            max_detail_height: 250.0,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#00000080").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            selected_fg: Color::from_hex("#ffffff").unwrap_or(Color::WHITE),
            highlight_fg: Color::from_hex("#18a3ff").unwrap_or(Color::WHITE),
            detail_fg: Color::from_hex("#9d9d9d").unwrap_or(Color::WHITE),
            icon_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            detail_pane_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            detail_pane_border: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            loading_fg: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
        }
    }

    /// Shows the popup at the given cursor position with the provided items.
    pub fn show(&mut self, items: Vec<CompletionItem>, position: (f32, f32)) {
        self.items = items;
        self.position = position;
        self.visible = true;
        self.selected = 0;
        self.scroll_offset = 0.0;
        self.detail_visible = false;
        self.filtered_items = (0..self.items.len()).collect();
        self.update_detail_visibility();
    }

    /// Dismisses the popup.
    pub fn hide(&mut self) {
        self.visible = false;
        self.items.clear();
        self.filtered_items.clear();
        self.selected = 0;
        self.detail_visible = false;
    }

    /// Filters items as the user types a prefix.
    pub fn filter(&mut self, prefix: &str) {
        if prefix.is_empty() {
            self.filtered_items = (0..self.items.len()).collect();
        } else {
            let query = prefix.to_lowercase();
            self.filtered_items = (0..self.items.len())
                .filter(|&i| fuzzy_match(self.items[i].effective_filter_text(), &query))
                .collect();
        }
        self.selected = 0;
        self.scroll_offset = 0.0;
        if self.filtered_items.is_empty() {
            self.hide();
        } else {
            self.update_detail_visibility();
        }
    }

    /// Selects the next item in the list.
    pub fn select_next(&mut self) {
        if !self.filtered_items.is_empty() {
            self.selected = (self.selected + 1) % self.filtered_items.len();
            self.ensure_selected_visible();
            self.update_detail_visibility();
        }
    }

    /// Selects the previous item in the list.
    pub fn select_prev(&mut self) {
        if !self.filtered_items.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered_items.len() - 1
            } else {
                self.selected - 1
            };
            self.ensure_selected_visible();
            self.update_detail_visibility();
        }
    }

    /// Scrolls one page down.
    pub fn select_page_down(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }
        self.selected =
            (self.selected + self.max_visible_items).min(self.filtered_items.len() - 1);
        self.ensure_selected_visible();
        self.update_detail_visibility();
    }

    /// Scrolls one page up.
    pub fn select_page_up(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(self.max_visible_items);
        self.ensure_selected_visible();
        self.update_detail_visibility();
    }

    /// Accepts the currently selected item. Returns it for insertion.
    pub fn accept(&mut self) -> Option<CompletionItem> {
        let item = self
            .filtered_items
            .get(self.selected)
            .map(|&idx| self.items[idx].clone());
        self.hide();
        item
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.filtered_items
            .get(self.selected)
            .map(|&idx| &self.items[idx])
    }

    // ── Private helpers ──────────────────────────────────────────────────

    fn visible_count(&self) -> usize {
        self.filtered_items.len().min(self.max_visible_items)
    }

    fn list_height(&self) -> f32 {
        self.visible_count() as f32 * self.row_height
    }

    #[allow(clippy::cast_precision_loss)]
    fn computed_width(&self) -> f32 {
        let longest = self
            .filtered_items
            .iter()
            .map(|&i| {
                let label_w = self.items[i].label.len() as f32;
                let detail_w = self.items[i]
                    .detail
                    .as_ref()
                    .map_or(0.0, |d| d.len() as f32 + 2.0);
                label_w + detail_w
            })
            .fold(0.0_f32, f32::max);
        let icon_pad = 28.0;
        let char_w = self.font_size * 0.6;
        (longest * char_w + icon_pad + 24.0)
            .max(self.min_width)
            .min(self.max_width)
    }

    fn ensure_selected_visible(&mut self) {
        let top = self.selected as f32 * self.row_height;
        let bottom = top + self.row_height;
        let vis = self.visible_count() as f32 * self.row_height;
        if top < self.scroll_offset {
            self.scroll_offset = top;
        } else if bottom > self.scroll_offset + vis {
            self.scroll_offset = bottom - vis;
        }
    }

    fn update_detail_visibility(&mut self) {
        self.detail_visible = self
            .selected_item()
            .map_or(false, |item| item.documentation.is_some() || item.detail.is_some());
    }

    fn widget_rect(&self, editor_height: f32) -> Rect {
        let w = self.computed_width();
        let h = self.list_height();
        let (mut x, mut y) = self.position;
        // Shift up if near bottom of editor
        if y + h > editor_height {
            y = (self.position.1 - h - self.row_height).max(0.0);
        }
        if x + w > 2000.0 {
            x = (2000.0 - w).max(0.0);
        }
        Rect::new(x, y, w, h)
    }

    fn icon_for_kind(kind: CompletionItemKind) -> IconId {
        match kind {
            CompletionItemKind::Method => IconId::SymbolMethod,
            CompletionItemKind::Field | CompletionItemKind::Property => IconId::SymbolField,
            CompletionItemKind::File => IconId::File,
            CompletionItemKind::Folder => IconId::Folder,
            _ => IconId::CircleFilled,
        }
    }

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
    pub fn render_draw(&self, ctx: &mut DrawContext, editor_rect: Rect) {
        if !self.visible || self.filtered_items.is_empty() {
            return;
        }
        let wr = self.widget_rect(editor_rect.height);

        // Shadow
        let shadow = Rect::new(wr.x + 2.0, wr.y + 2.0, wr.width, wr.height);
        ctx.draw_rect(shadow, self.shadow_color, 4.0);

        // Background
        ctx.draw_rect(wr, self.background, 4.0);
        ctx.draw_border(wr, self.border_color, 1.0, 4.0);

        // Clipped list area
        ctx.save();
        ctx.clip(wr);

        let query_lower: String;
        let has_query = !self.filtered_items.is_empty();
        query_lower = String::new();
        let _ = has_query;

        for (vi, &item_idx) in self.filtered_items.iter().enumerate() {
            let y = wr.y + vi as f32 * self.row_height - self.scroll_offset;
            if y + self.row_height < wr.y || y > wr.y + wr.height {
                continue;
            }

            let item = &self.items[item_idx];
            let is_selected = vi == self.selected;
            let fg = if is_selected {
                self.selected_fg
            } else {
                self.foreground
            };

            // Selected background
            if is_selected {
                let sel = Rect::new(wr.x + 2.0, y, wr.width - 4.0, self.row_height);
                ctx.draw_rect(sel, self.selected_bg, 2.0);
            }

            // Icon
            let icon = Self::icon_for_kind(item.kind);
            let icon_y = y + (self.row_height - 14.0) / 2.0;
            ctx.draw_icon(icon, (wr.x + 6.0, icon_y), 14.0, self.icon_fg);

            // Label with fuzzy highlighting
            let text_x = wr.x + 28.0;
            let text_y = y + (self.row_height - self.font_size) / 2.0;

            if query_lower.is_empty() {
                ctx.draw_text(&item.label, (text_x, text_y), fg, self.font_size, false, false);
            } else {
                let positions = Self::match_positions(&item.label, &query_lower);
                let spans = build_highlight_spans(&item.label, &positions, fg, self.highlight_fg);
                ctx.draw_styled_text(&spans, (text_x, text_y), self.font_size);
            }

            // Detail (right-aligned, dimmed)
            if let Some(ref detail) = item.detail {
                let detail_w = detail.len() as f32 * self.font_size * 0.6;
                let detail_x = wr.x + wr.width - detail_w - 8.0;
                ctx.draw_text(
                    detail,
                    (detail_x, text_y),
                    self.detail_fg,
                    self.font_size,
                    false,
                    false,
                );
            }
        }

        ctx.restore();

        // Detail pane (side panel for documentation)
        if self.detail_visible {
            if let Some(item) = self.selected_item() {
                self.render_detail_pane(ctx, wr, item);
            }
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn render_detail_pane(&self, ctx: &mut DrawContext, list_rect: Rect, item: &CompletionItem) {
        let dx = list_rect.x + list_rect.width + 2.0;
        let dy = list_rect.y;
        let dh = self.max_detail_height.min(list_rect.height);
        let dr = Rect::new(dx, dy, self.detail_width, dh);

        // Shadow + background
        let shadow = Rect::new(dr.x + 2.0, dr.y + 2.0, dr.width, dr.height);
        ctx.draw_rect(shadow, self.shadow_color, 4.0);
        ctx.draw_rect(dr, self.detail_pane_bg, 4.0);
        ctx.draw_border(dr, self.detail_pane_border, 1.0, 4.0);

        ctx.save();
        ctx.clip(dr);

        let mut cy = dr.y + 8.0;
        let pad_x = dr.x + 10.0;

        // Type signature at top
        if let Some(ref detail) = item.detail {
            ctx.draw_text(
                detail,
                (pad_x, cy),
                self.foreground,
                self.font_size,
                true,
                false,
            );
            cy += self.font_size + 8.0;
            // separator
            let sep = Rect::new(dr.x + 4.0, cy, dr.width - 8.0, 1.0);
            ctx.draw_rect(sep, self.border_color, 0.0);
            cy += 6.0;
        }

        // Documentation (markdown rendered as plain text)
        if let Some(ref doc) = item.documentation {
            for line in doc.lines() {
                if cy > dr.y + dh - self.font_size {
                    break;
                }
                ctx.draw_text(
                    line,
                    (pad_x, cy),
                    self.detail_fg,
                    self.font_size - 1.0,
                    false,
                    false,
                );
                cy += self.font_size + 2.0;
            }
        }

        // Source at bottom
        if let Some(ref source) = item.source {
            let src_y = dr.y + dh - self.font_size - 6.0;
            ctx.draw_text(
                source,
                (pad_x, src_y),
                self.loading_fg,
                self.font_size - 2.0,
                false,
                true,
            );
        }

        ctx.restore();
    }
}

impl Widget for SuggestWidget {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Auto,
            padding: Edges::all(0.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible || self.filtered_items.is_empty() {
            return;
        }
        let wr = self.widget_rect(rect.height);
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(wr.x, wr.y, wr.width, wr.height, self.background, 4.0);
        rr.draw_border(wr.x, wr.y, wr.width, wr.height, self.border_color, 1.0);
        for (vi, &_item_idx) in self.filtered_items.iter().enumerate() {
            let y = wr.y + vi as f32 * self.row_height - self.scroll_offset;
            if y + self.row_height < wr.y || y > wr.y + wr.height {
                continue;
            }
            if vi == self.selected {
                rr.draw_rect(wr.x + 2.0, y, wr.width - 4.0, self.row_height, self.selected_bg, 2.0);
            }
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        match event {
            UiEvent::KeyPress { key, .. } => match key {
                Key::Escape => {
                    self.hide();
                    EventResult::Handled
                }
                Key::ArrowDown => {
                    self.select_next();
                    EventResult::Handled
                }
                Key::ArrowUp => {
                    self.select_prev();
                    EventResult::Handled
                }
                Key::PageDown => {
                    self.select_page_down();
                    EventResult::Handled
                }
                Key::PageUp => {
                    self.select_page_up();
                    EventResult::Handled
                }
                Key::Enter | Key::Tab => {
                    let _ = self.accept();
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            },
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let wr = self.widget_rect(rect.height);
                if !wr.contains(*x, *y) {
                    self.hide();
                    return EventResult::Handled;
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let vi = ((y - wr.y + self.scroll_offset) / self.row_height) as usize;
                if vi < self.filtered_items.len() {
                    self.selected = vi;
                    let _ = self.accept();
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let max_scroll =
                    (self.filtered_items.len().saturating_sub(self.max_visible_items)) as f32
                        * self.row_height;
                self.scroll_offset = (self.scroll_offset - dy * self.row_height)
                    .max(0.0)
                    .min(max_scroll);
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

fn build_highlight_spans(
    label: &str,
    positions: &[usize],
    normal_color: Color,
    highlight_color: Color,
) -> Vec<(String, TextStyle)> {
    let mut spans = Vec::new();
    let mut last_end = 0;
    for &pos in positions {
        if pos > last_end {
            spans.push((
                label[last_end..pos].to_string(),
                TextStyle {
                    color: normal_color,
                    bold: false,
                    italic: false,
                },
            ));
        }
        let ch_end = pos + label[pos..].chars().next().map_or(1, |c| c.len_utf8());
        spans.push((
            label[pos..ch_end].to_string(),
            TextStyle {
                color: highlight_color,
                bold: true,
                italic: false,
            },
        ));
        last_end = ch_end;
    }
    if last_end < label.len() {
        spans.push((
            label[last_end..].to_string(),
            TextStyle {
                color: normal_color,
                bold: false,
                italic: false,
            },
        ));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_filter() {
        let mut w = SuggestWidget::new();
        w.show(
            vec![
                CompletionItem::new("forEach", CompletionItemKind::Method),
                CompletionItem::new("filter", CompletionItemKind::Method),
                CompletionItem::new("map", CompletionItemKind::Method),
            ],
            (100.0, 200.0),
        );
        assert!(w.is_visible());
        assert_eq!(w.filtered_items.len(), 3);

        w.filter("fi");
        assert_eq!(w.filtered_items.len(), 1);
    }

    #[test]
    fn navigate_and_accept() {
        let mut w = SuggestWidget::new();
        w.show(
            vec![
                CompletionItem::new("a", CompletionItemKind::Text),
                CompletionItem::new("b", CompletionItemKind::Text),
                CompletionItem::new("c", CompletionItemKind::Text),
            ],
            (0.0, 0.0),
        );
        assert_eq!(w.selected, 0);
        w.select_next();
        assert_eq!(w.selected, 1);
        w.select_prev();
        assert_eq!(w.selected, 0);

        let accepted = w.accept();
        assert_eq!(accepted.unwrap().label, "a");
        assert!(!w.is_visible());
    }

    #[test]
    fn page_navigation() {
        let mut w = SuggestWidget::new();
        let items: Vec<_> = (0..20)
            .map(|i| CompletionItem::new(format!("item{i}"), CompletionItemKind::Text))
            .collect();
        w.show(items, (0.0, 0.0));
        w.select_page_down();
        assert_eq!(w.selected, 12);
        w.select_page_up();
        assert_eq!(w.selected, 0);
    }

    #[test]
    fn hide_on_empty_filter() {
        let mut w = SuggestWidget::new();
        w.show(
            vec![CompletionItem::new("foo", CompletionItemKind::Text)],
            (0.0, 0.0),
        );
        w.filter("zzz");
        assert!(!w.is_visible());
    }

    #[test]
    fn fuzzy_match_basic() {
        assert!(fuzzy_match("forEach", "fe"));
        assert!(fuzzy_match("Filter", "fi"));
        assert!(!fuzzy_match("map", "mx"));
    }
}
