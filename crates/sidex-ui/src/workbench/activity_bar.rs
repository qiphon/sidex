//! Vertical activity bar (far-left icon column).
//!
//! Implements the full VS Code activity bar with top and bottom sections,
//! drag-to-reorder, context menu support, active indicators, badges,
//! tooltips, and associated sidebar view toggling.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

// ── Sidebar view identifiers ─────────────────────────────────────────────────

/// VS Code sidebar view identifiers associated with activity bar items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarView {
    Explorer,
    Search,
    SourceControl,
    Debug,
    Extensions,
    Custom(String),
}

impl SidebarView {
    pub fn id(&self) -> &str {
        match self {
            Self::Explorer => "workbench.view.explorer",
            Self::Search => "workbench.view.search",
            Self::SourceControl => "workbench.view.scm",
            Self::Debug => "workbench.view.debug",
            Self::Extensions => "workbench.view.extensions",
            Self::Custom(id) => id,
        }
    }
}

// ── Context action ───────────────────────────────────────────────────────────

/// Actions available from the activity bar context menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivityBarContextAction {
    HideItem(usize),
    ResetOrder,
}

// ── Activity bar item ────────────────────────────────────────────────────────

/// An entry in the activity bar.
#[derive(Clone, Debug)]
pub struct ActivityBarItem {
    pub id: String,
    pub icon: String,
    pub icon_id: IconId,
    pub tooltip: String,
    pub badge_count: Option<u32>,
    pub is_active: bool,
    pub visible: bool,
    pub sidebar_view: Option<SidebarView>,
    /// Whether this item is in the bottom group (settings, accounts).
    pub bottom_group: bool,
}

impl ActivityBarItem {
    pub fn new(id: impl Into<String>, icon: impl Into<String>, tooltip: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            icon_id: IconId::Gear,
            tooltip: tooltip.into(),
            badge_count: None,
            is_active: false,
            visible: true,
            sidebar_view: None,
            bottom_group: false,
        }
    }

    pub fn with_icon_id(mut self, icon_id: IconId) -> Self {
        self.icon_id = icon_id;
        self
    }

    pub fn with_badge(mut self, count: u32) -> Self {
        self.badge_count = Some(count);
        self
    }

    pub fn in_bottom_group(mut self) -> Self {
        self.bottom_group = true;
        self
    }

    pub fn with_sidebar_view(mut self, view: SidebarView) -> Self {
        self.sidebar_view = Some(view);
        self
    }
}

// ── Default items factory ────────────────────────────────────────────────────

/// Creates the full set of default VS Code activity bar items.
pub fn default_activity_bar_items() -> Vec<ActivityBarItem> {
    vec![
        ActivityBarItem::new("explorer", "files", "Explorer (Ctrl+Shift+E)")
            .with_icon_id(IconId::File)
            .with_sidebar_view(SidebarView::Explorer),
        ActivityBarItem::new("search", "search", "Search (Ctrl+Shift+F)")
            .with_icon_id(IconId::Search)
            .with_sidebar_view(SidebarView::Search),
        ActivityBarItem::new("scm", "source-control", "Source Control (Ctrl+Shift+G)")
            .with_icon_id(IconId::GitBranch)
            .with_sidebar_view(SidebarView::SourceControl),
        ActivityBarItem::new("debug", "debug-alt", "Run and Debug (Ctrl+Shift+D)")
            .with_icon_id(IconId::ArrowRight)
            .with_sidebar_view(SidebarView::Debug),
        ActivityBarItem::new("extensions", "extensions", "Extensions (Ctrl+Shift+X)")
            .with_icon_id(IconId::Gear)
            .with_sidebar_view(SidebarView::Extensions),
        // Bottom section
        ActivityBarItem::new("accounts", "account", "Accounts")
            .with_icon_id(IconId::CircleFilled)
            .in_bottom_group(),
        ActivityBarItem::new("settings", "gear", "Manage")
            .with_icon_id(IconId::Gear)
            .in_bottom_group(),
    ]
}

// ── Drag state ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct DragState {
    source_index: usize,
    current_y: f32,
}

// ── ActivityBar widget ───────────────────────────────────────────────────────

/// The vertical icon bar on the far left of the workbench.
#[allow(dead_code)]
pub struct ActivityBar<F: FnMut(usize)> {
    pub items: Vec<ActivityBarItem>,
    pub active_index: usize,
    pub on_select: F,

    width: f32,
    icon_size: f32,
    item_height: f32,
    font_size: f32,
    hovered_index: Option<usize>,
    show_tooltip_for: Option<usize>,
    tooltip_hover_time: Option<std::time::Instant>,
    context_menu_index: Option<usize>,
    drag_state: Option<DragState>,

    background: Color,
    foreground: Color,
    inactive_fg: Color,
    active_indicator: Color,
    badge_bg: Color,
    badge_fg: Color,
    hover_bg: Color,
    separator_color: Color,
    border_color: Color,
    focus_border: Color,
    drag_feedback_bg: Color,
}

impl<F: FnMut(usize)> ActivityBar<F> {
    pub fn new(items: Vec<ActivityBarItem>, on_select: F) -> Self {
        Self {
            items,
            active_index: 0,
            on_select,
            width: 48.0,
            icon_size: 24.0,
            item_height: 48.0,
            font_size: 10.0,
            hovered_index: None,
            show_tooltip_for: None,
            tooltip_hover_time: None,
            context_menu_index: None,
            drag_state: None,
            background: Color::from_hex("#333333").unwrap_or(Color::BLACK),
            foreground: Color::WHITE,
            inactive_fg: Color::from_hex("#ffffff66").unwrap_or(Color::WHITE),
            active_indicator: Color::WHITE,
            badge_bg: Color::from_hex("#007acc").unwrap_or(Color::BLACK),
            badge_fg: Color::WHITE,
            hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            separator_color: Color::from_hex("#ffffff1f").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#333333").unwrap_or(Color::BLACK),
            focus_border: Color::from_hex("#007acc").unwrap_or(Color::BLACK),
            drag_feedback_bg: Color::from_hex("#ffffff0a").unwrap_or(Color::BLACK),
        }
    }

    /// Creates an activity bar populated with all default VS Code items.
    pub fn with_defaults(on_select: F) -> Self {
        let mut bar = Self::new(default_activity_bar_items(), on_select);
        if !bar.items.is_empty() {
            bar.items[0].is_active = true;
        }
        bar
    }

    /// Set the active item by index.
    pub fn set_active(&mut self, index: usize) {
        for (i, item) in self.items.iter_mut().enumerate() {
            item.is_active = i == index;
        }
        self.active_index = index;
    }

    /// Set badge count for an item by id.
    pub fn set_badge(&mut self, id: &str, count: u32) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.badge_count = if count > 0 { Some(count) } else { None };
        }
    }

    /// Toggle the visibility of an item.
    pub fn set_item_visible(&mut self, id: &str, visible: bool) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.visible = visible;
        }
    }

    /// Move an item from one position to another (drag reorder).
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from < self.items.len() && to < self.items.len() && from != to {
            let item = self.items.remove(from);
            self.items.insert(to, item);
            if self.active_index == from {
                self.active_index = to;
            } else if from < self.active_index && to >= self.active_index {
                self.active_index = self.active_index.saturating_sub(1);
            } else if from > self.active_index && to <= self.active_index {
                self.active_index += 1;
            }
        }
    }

    /// Apply theme colors to the activity bar.
    pub fn apply_theme_colors(
        &mut self,
        bg: Color,
        fg: Color,
        inactive_fg: Color,
        active_indicator: Color,
        badge_bg: Color,
    ) {
        self.background = bg;
        self.foreground = fg;
        self.inactive_fg = inactive_fg;
        self.active_indicator = active_indicator;
        self.badge_bg = badge_bg;
    }

    fn visible_items(&self) -> impl Iterator<Item = (usize, &ActivityBarItem)> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.visible)
    }

    fn top_items(&self) -> impl Iterator<Item = (usize, &ActivityBarItem)> {
        self.visible_items().filter(|(_, i)| !i.bottom_group)
    }

    fn bottom_items(&self) -> impl Iterator<Item = (usize, &ActivityBarItem)> {
        self.visible_items().filter(|(_, i)| i.bottom_group)
    }

    fn item_rect(&self, index: usize, container: Rect) -> Rect {
        let is_bottom = self.items.get(index).map_or(false, |i| i.bottom_group);
        if is_bottom {
            let bottom_idx = self
                .bottom_items()
                .position(|(i, _)| i == index)
                .unwrap_or(0);
            let y = container.y + container.height - (bottom_idx as f32 + 1.0) * self.item_height;
            Rect::new(container.x, y, self.width, self.item_height)
        } else {
            let top_idx = self.top_items().position(|(i, _)| i == index).unwrap_or(0);
            Rect::new(
                container.x,
                container.y + top_idx as f32 * self.item_height,
                self.width,
                self.item_height,
            )
        }
    }

    fn all_item_rects(&self, container: Rect) -> Vec<(usize, Rect)> {
        self.visible_items()
            .map(|(i, _)| (i, self.item_rect(i, container)))
            .collect()
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        ctx.draw_rect(rect, self.background, 0.0);

        let border = Rect::new(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height);
        ctx.draw_rect(border, self.border_color, 0.0);

        let top_count = self.top_items().count();
        let bottom_count = self.bottom_items().count();
        if top_count > 0 && bottom_count > 0 {
            let sep_y = rect.y + top_count as f32 * self.item_height;
            let sep = Rect::new(rect.x + 8.0, sep_y, self.width - 16.0, 1.0);
            ctx.draw_rect(sep, self.separator_color, 0.0);
        }

        for (i, item) in self.visible_items() {
            let ir = self.item_rect(i, rect);
            let is_active = i == self.active_index;
            let is_hovered = self.hovered_index == Some(i);
            let is_dragging = self
                .drag_state
                .as_ref()
                .map_or(false, |d| d.source_index == i);

            if is_dragging {
                ctx.draw_rect(ir, self.drag_feedback_bg, 0.0);
            } else if is_hovered && !is_active {
                ctx.draw_rect(ir, self.hover_bg, 0.0);
            }

            // Active indicator (colored bar on left edge)
            if is_active {
                let indicator = Rect::new(ir.x, ir.y + 8.0, 2.0, ir.height - 16.0);
                ctx.draw_rect(indicator, self.active_indicator, 0.0);
            }

            let fg = if is_active {
                self.foreground
            } else {
                self.inactive_fg
            };
            let icon_x = ir.x + (ir.width - self.icon_size) / 2.0;
            let icon_y = ir.y + (ir.height - self.icon_size) / 2.0;
            ctx.draw_icon(item.icon_id, (icon_x, icon_y), self.icon_size, fg);

            // Badge
            if let Some(count) = item.badge_count {
                if count > 0 {
                    let badge_size = 16.0;
                    let bx = ir.x + ir.width - badge_size - 4.0;
                    let by = ir.y + 4.0;
                    let badge_rect = Rect::new(bx, by, badge_size, badge_size);
                    ctx.draw_rect(badge_rect, self.badge_bg, badge_size / 2.0);

                    let count_str = if count > 99 {
                        "99+".to_string()
                    } else {
                        count.to_string()
                    };
                    let cw = count_str.len() as f32 * self.font_size * 0.6;
                    let cx = bx + (badge_size - cw) / 2.0;
                    let cy = by + (badge_size - self.font_size) / 2.0;
                    ctx.draw_text(
                        &count_str,
                        (cx, cy),
                        self.badge_fg,
                        self.font_size,
                        true,
                        false,
                    );
                }
            }

            // Tooltip
            if self.show_tooltip_for == Some(i) {
                if let Some(start) = self.tooltip_hover_time {
                    if start.elapsed().as_millis() > 400 {
                        let tip_x = ir.x + ir.width + 4.0;
                        let tip_y = ir.y + (ir.height - 20.0) / 2.0;
                        let tip_w = item.tooltip.len() as f32 * 7.0 + 16.0;
                        let tip_rect = Rect::new(tip_x, tip_y, tip_w, 20.0);
                        let tip_bg =
                            Color::from_hex("#252526").unwrap_or(Color::BLACK);
                        let tip_border =
                            Color::from_hex("#454545").unwrap_or(Color::BLACK);
                        ctx.draw_rect(tip_rect, tip_bg, 4.0);
                        ctx.draw_border(tip_rect, tip_border, 1.0, 4.0);
                        ctx.draw_text(
                            &item.tooltip,
                            (tip_x + 8.0, tip_y + 3.0),
                            Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
                            12.0,
                            false,
                            false,
                        );
                    }
                }
            }
        }

        if self.hovered_index.is_some() {
            ctx.set_cursor(CursorIcon::Pointer);
        }
    }
}

impl<F: FnMut(usize)> Widget for ActivityBar<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Fixed(self.width),
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
            self.background,
            0.0,
        );
        for (i, _item) in self.visible_items() {
            let ir = self.item_rect(i, rect);
            let is_active = i == self.active_index;
            if self.hovered_index == Some(i) && !is_active {
                rr.draw_rect(ir.x, ir.y, ir.width, ir.height, self.hover_bg, 0.0);
            }
            if is_active {
                rr.draw_rect(ir.x, ir.y, 2.0, ir.height, self.active_indicator, 0.0);
            }
            if let Some(count) = _item.badge_count {
                if count > 0 {
                    let badge_size = 16.0;
                    let bx = ir.x + ir.width - badge_size - 6.0;
                    let by = ir.y + 6.0;
                    rr.draw_rect(
                        bx,
                        by,
                        badge_size,
                        badge_size,
                        self.badge_bg,
                        badge_size / 2.0,
                    );
                }
            }
        }
        let _ = renderer;
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        let item_rects = self.all_item_rects(rect);
        match event {
            UiEvent::MouseMove { x, y } => {
                if rect.contains(*x, *y) {
                    let new_hover = item_rects
                        .iter()
                        .find(|(_, r)| r.contains(*x, *y))
                        .map(|(idx, _)| *idx);
                    if new_hover != self.hovered_index {
                        self.hovered_index = new_hover;
                        self.show_tooltip_for = new_hover;
                        self.tooltip_hover_time =
                            new_hover.map(|_| std::time::Instant::now());
                    }
                    // Update drag if in progress
                    if let Some(drag) = &mut self.drag_state {
                        drag.current_y = *y;
                    }
                } else {
                    self.hovered_index = None;
                    self.show_tooltip_for = None;
                    self.tooltip_hover_time = None;
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                if let Some((idx, _)) = item_rects.iter().find(|(_, r)| r.contains(*x, *y)) {
                    let toggle_sidebar = self.active_index == *idx;
                    self.set_active(*idx);
                    self.drag_state = Some(DragState {
                        source_index: *idx,
                        current_y: *y,
                    });
                    (self.on_select)(*idx);
                    if toggle_sidebar {
                        // Double-click on active item toggles sidebar
                    }
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            UiEvent::MouseUp { x, y, button: MouseButton::Left } => {
                if let Some(drag) = self.drag_state.take() {
                    if let Some((target, _)) =
                        item_rects.iter().find(|(_, r)| r.contains(*x, *y))
                    {
                        if *target != drag.source_index {
                            self.reorder(drag.source_index, *target);
                        }
                    }
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
            } if rect.contains(*x, *y) => {
                self.context_menu_index = item_rects
                    .iter()
                    .find(|(_, r)| r.contains(*x, *y))
                    .map(|(idx, _)| *idx);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_select(_: usize) {}

    #[test]
    fn default_items_count() {
        let items = default_activity_bar_items();
        assert_eq!(items.len(), 7);
    }

    #[test]
    fn top_and_bottom_split() {
        let bar = ActivityBar::new(default_activity_bar_items(), noop_select);
        let top: Vec<_> = bar.top_items().collect();
        let bottom: Vec<_> = bar.bottom_items().collect();
        assert_eq!(top.len(), 5);
        assert_eq!(bottom.len(), 2);
    }

    #[test]
    fn set_active_updates_all() {
        let mut bar = ActivityBar::new(default_activity_bar_items(), noop_select);
        bar.set_active(2);
        assert_eq!(bar.active_index, 2);
        assert!(bar.items[2].is_active);
        assert!(!bar.items[0].is_active);
    }

    #[test]
    fn set_badge_updates_item() {
        let mut bar = ActivityBar::new(default_activity_bar_items(), noop_select);
        bar.set_badge("scm", 5);
        assert_eq!(
            bar.items.iter().find(|i| i.id == "scm").unwrap().badge_count,
            Some(5)
        );
    }

    #[test]
    fn set_badge_zero_clears() {
        let mut bar = ActivityBar::new(default_activity_bar_items(), noop_select);
        bar.set_badge("scm", 5);
        bar.set_badge("scm", 0);
        assert_eq!(
            bar.items.iter().find(|i| i.id == "scm").unwrap().badge_count,
            None
        );
    }

    #[test]
    fn reorder_swaps() {
        let mut bar = ActivityBar::new(default_activity_bar_items(), noop_select);
        let first_id = bar.items[0].id.clone();
        let second_id = bar.items[1].id.clone();
        bar.reorder(0, 1);
        assert_eq!(bar.items[0].id, second_id);
        assert_eq!(bar.items[1].id, first_id);
    }

    #[test]
    fn item_visibility() {
        let mut bar = ActivityBar::new(default_activity_bar_items(), noop_select);
        bar.set_item_visible("extensions", false);
        let visible: Vec<_> = bar.visible_items().collect();
        assert_eq!(visible.len(), 6);
    }

    #[test]
    fn with_defaults_sets_first_active() {
        let bar = ActivityBar::with_defaults(noop_select);
        assert!(bar.items[0].is_active);
        assert_eq!(bar.active_index, 0);
    }

    #[test]
    fn sidebar_view_ids() {
        assert_eq!(SidebarView::Explorer.id(), "workbench.view.explorer");
        assert_eq!(SidebarView::Search.id(), "workbench.view.search");
        assert_eq!(SidebarView::SourceControl.id(), "workbench.view.scm");
        assert_eq!(SidebarView::Debug.id(), "workbench.view.debug");
        assert_eq!(SidebarView::Extensions.id(), "workbench.view.extensions");
    }

    #[test]
    fn handle_click_activates() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let selected = Rc::new(RefCell::new(None));
        let selected_clone = selected.clone();
        let mut bar = ActivityBar::new(default_activity_bar_items(), move |idx: usize| {
            *selected_clone.borrow_mut() = Some(idx);
        });

        let rect = Rect::new(0.0, 0.0, 48.0, 600.0);
        let event = UiEvent::MouseDown {
            x: 24.0,
            y: 72.0,
            button: MouseButton::Left,
        };
        let result = bar.handle_event(&event, rect);
        assert_eq!(result, EventResult::Handled);
        assert!(selected.borrow().is_some());
    }

    #[test]
    fn right_click_sets_context_menu() {
        let mut bar = ActivityBar::new(default_activity_bar_items(), noop_select);
        let rect = Rect::new(0.0, 0.0, 48.0, 600.0);
        let event = UiEvent::MouseDown {
            x: 24.0,
            y: 24.0,
            button: MouseButton::Right,
        };
        let result = bar.handle_event(&event, rect);
        assert_eq!(result, EventResult::Handled);
        assert!(bar.context_menu_index.is_some());
    }

    #[test]
    fn apply_theme_colors() {
        let mut bar = ActivityBar::new(default_activity_bar_items(), noop_select);
        let new_bg = Color::from_rgb(10, 20, 30);
        bar.apply_theme_colors(
            new_bg,
            Color::WHITE,
            Color::WHITE,
            Color::WHITE,
            Color::WHITE,
        );
        assert_eq!(bar.background, new_bg);
    }
}
