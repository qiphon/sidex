//! Virtual-scrolling list widget with single and multi-select.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::DrawContext;
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// Selection mode for the list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    #[default]
    Single,
    Multi,
}

/// A virtual-scrolling list that only renders visible items.
#[allow(dead_code)]
pub struct List<T, R, S>
where
    R: Fn(&T, usize, bool) -> ListRow,
    S: FnMut(usize),
{
    pub items: Vec<T>,
    pub render_item: R,
    pub on_select: S,
    pub selected: Vec<usize>,
    pub selection_mode: SelectionMode,

    row_height: f32,
    scroll_offset: f32,
    focused: bool,
    hovered_index: Option<usize>,
    font_size: f32,

    hover_bg: Color,
    selected_bg: Color,
    selected_fg: Color,
    foreground: Color,
    description_fg: Color,
    scrollbar_thumb: Color,
    keyboard_focus_outline: Color,
}

/// Pre-rendered description of a single list row.
pub struct ListRow {
    pub text: String,
    pub icon: Option<String>,
    pub description: Option<String>,
}

impl<T, R, S> List<T, R, S>
where
    R: Fn(&T, usize, bool) -> ListRow,
    S: FnMut(usize),
{
    pub fn new(items: Vec<T>, render_item: R, on_select: S) -> Self {
        Self {
            items,
            render_item,
            on_select,
            selected: Vec::new(),
            selection_mode: SelectionMode::Single,
            row_height: 22.0,
            scroll_offset: 0.0,
            focused: false,
            hovered_index: None,
            font_size: 13.0,
            hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            selected_fg: Color::WHITE,
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            description_fg: Color::from_hex("#aaaaaa").unwrap_or(Color::WHITE),
            scrollbar_thumb: Color::from_hex("#79797966").unwrap_or(Color::WHITE),
            keyboard_focus_outline: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
        }
    }

    pub fn with_selection_mode(mut self, mode: SelectionMode) -> Self {
        self.selection_mode = mode;
        self
    }

    /// Range of item indices currently visible in the viewport.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn visible_range(&self, rect: Rect) -> (usize, usize) {
        let first = (self.scroll_offset / self.row_height).floor() as usize;
        let count = (rect.height / self.row_height).ceil() as usize + 1;
        let last = (first + count).min(self.items.len());
        (first, last)
    }

    #[allow(clippy::cast_precision_loss)]
    fn total_height(&self) -> f32 {
        self.items.len() as f32 * self.row_height
    }

    fn ensure_visible(&mut self, index: usize, rect: Rect) {
        let top = index as f32 * self.row_height;
        let bottom = top + self.row_height;
        if top < self.scroll_offset {
            self.scroll_offset = top;
        } else if bottom > self.scroll_offset + rect.height {
            self.scroll_offset = bottom - rect.height;
        }
    }

    fn primary_selected(&self) -> Option<usize> {
        self.selected.last().copied()
    }

    fn select_index(&mut self, index: usize, toggle: bool) {
        match self.selection_mode {
            SelectionMode::Single => {
                self.selected = vec![index];
            }
            SelectionMode::Multi if toggle => {
                if let Some(pos) = self.selected.iter().position(|&i| i == index) {
                    self.selected.remove(pos);
                } else {
                    self.selected.push(index);
                }
            }
            SelectionMode::Multi => {
                self.selected = vec![index];
            }
        }
        (self.on_select)(index);
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        ctx.save();
        ctx.clip(rect);

        let (first, last) = self.visible_range(rect);

        for i in first..last {
            let is_selected = self.selected.contains(&i);
            let is_hovered = self.hovered_index == Some(i);
            let y = rect.y + i as f32 * self.row_height - self.scroll_offset;
            let row_rect = Rect::new(rect.x, y, rect.width, self.row_height);

            // Selection background
            if is_selected {
                ctx.draw_rect(row_rect, self.selected_bg, 0.0);
            } else if is_hovered {
                ctx.draw_rect(row_rect, self.hover_bg, 0.0);
            }

            let row = (self.render_item)(&self.items[i], i, is_selected);
            let text_color = if is_selected {
                self.selected_fg
            } else {
                self.foreground
            };
            let text_y = y + (self.row_height - self.font_size) / 2.0;
            let text_x = rect.x + 8.0;

            // Row text
            ctx.draw_text(
                &row.text,
                (text_x, text_y),
                text_color,
                self.font_size,
                false,
                false,
            );

            // Description (right-aligned, dimmed)
            if let Some(ref desc) = row.description {
                let desc_w = desc.len() as f32 * self.font_size * 0.6;
                let desc_x = rect.x + rect.width - desc_w - 8.0;
                ctx.draw_text(
                    desc,
                    (desc_x, text_y),
                    self.description_fg,
                    self.font_size,
                    false,
                    false,
                );
            }

            // Keyboard focus indicator
            if self.focused && is_selected {
                ctx.draw_border(row_rect, self.keyboard_focus_outline, 1.0, 0.0);
            }
        }

        // Scrollbar when content overflows
        let total = self.total_height();
        if total > rect.height {
            let thumb_ratio = rect.height / total;
            let thumb_h = (rect.height * thumb_ratio).max(20.0);
            let scroll_ratio = if total - rect.height > 0.0 {
                self.scroll_offset / (total - rect.height)
            } else {
                0.0
            };
            let thumb_y = rect.y + scroll_ratio * (rect.height - thumb_h);
            let sb_width = 10.0;
            let sb_rect = Rect::new(rect.x + rect.width - sb_width, thumb_y, sb_width, thumb_h);
            ctx.draw_rect(sb_rect, self.scrollbar_thumb, 3.0);
        }

        ctx.restore();
    }
}

impl<T, R, S> Widget for List<T, R, S>
where
    R: Fn(&T, usize, bool) -> ListRow,
    S: FnMut(usize),
{
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Flex(1.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        let (first, last) = self.visible_range(rect);
        for i in first..last {
            let is_selected = self.selected.contains(&i);
            #[allow(clippy::cast_precision_loss)]
            let y = rect.y + i as f32 * self.row_height - self.scroll_offset;
            if is_selected {
                rr.draw_rect(
                    rect.x,
                    y,
                    rect.width,
                    self.row_height,
                    self.selected_bg,
                    0.0,
                );
            }
            let _row = (self.render_item)(&self.items[i], i, is_selected);
        }
        let _ = renderer;
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => {
                self.focused = true;
                EventResult::Handled
            }
            UiEvent::Blur => {
                self.focused = false;
                EventResult::Handled
            }
            UiEvent::MouseMove { x, y } => {
                if rect.contains(*x, *y) {
                    let idx =
                        ((y - rect.y + self.scroll_offset) / self.row_height).floor() as usize;
                    self.hovered_index = if idx < self.items.len() {
                        Some(idx)
                    } else {
                        None
                    };
                } else {
                    self.hovered_index = None;
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.focused = true;
                let index = ((y - rect.y + self.scroll_offset) / self.row_height).floor() as usize;
                if index < self.items.len() {
                    self.select_index(index, false);
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let max = (self.total_height() - rect.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                EventResult::Handled
            }
            UiEvent::KeyPress { key, modifiers } if self.focused => {
                let current = self.primary_selected().unwrap_or(0);
                match key {
                    Key::ArrowDown => {
                        let next = (current + 1).min(self.items.len().saturating_sub(1));
                        self.select_index(next, modifiers.shift);
                        self.ensure_visible(next, rect);
                        EventResult::Handled
                    }
                    Key::ArrowUp => {
                        let next = current.saturating_sub(1);
                        self.select_index(next, modifiers.shift);
                        self.ensure_visible(next, rect);
                        EventResult::Handled
                    }
                    Key::Home => {
                        self.select_index(0, false);
                        self.ensure_visible(0, rect);
                        EventResult::Handled
                    }
                    Key::End => {
                        let last = self.items.len().saturating_sub(1);
                        self.select_index(last, false);
                        self.ensure_visible(last, rect);
                        EventResult::Handled
                    }
                    Key::PageDown => {
                        let page = (rect.height / self.row_height) as usize;
                        let next = (current + page).min(self.items.len().saturating_sub(1));
                        self.select_index(next, false);
                        self.ensure_visible(next, rect);
                        EventResult::Handled
                    }
                    Key::PageUp => {
                        let page = (rect.height / self.row_height) as usize;
                        let next = current.saturating_sub(page);
                        self.select_index(next, false);
                        self.ensure_visible(next, rect);
                        EventResult::Handled
                    }
                    _ => EventResult::Ignored,
                }
            }
            _ => EventResult::Ignored,
        }
    }
}
