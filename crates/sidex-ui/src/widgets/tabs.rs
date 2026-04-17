//! Tab bar widget with close buttons, dirty indicators, drag, overflow,
//! pinned tabs, preview (italic) tabs, context menu, and sizing modes.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

// ── Tab descriptor ───────────────────────────────────────────────────────────

/// A single tab descriptor passed to the tab bar for rendering.
#[derive(Clone, Debug)]
pub struct Tab {
    pub id: String,
    pub label: String,
    pub is_dirty: bool,
    pub is_preview: bool,
    pub is_pinned: bool,
}

/// Tab sizing mode matching VS Code's `workbench.editor.tabSizing`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabSizingMode {
    /// Tabs shrink to fit available space.
    #[default]
    Fit,
    /// Tabs shrink but have a minimum width.
    Shrink,
    /// All tabs have a fixed width.
    Fixed,
}

impl TabSizingMode {
    pub fn from_setting(s: &str) -> Self {
        match s {
            "shrink" => Self::Shrink,
            "fixed" => Self::Fixed,
            _ => Self::Fit,
        }
    }
}

// ── Context menu ─────────────────────────────────────────────────────────────

/// Actions that can be triggered from a tab's right-click context menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabContextAction {
    Close,
    CloseOthers,
    CloseAll,
    CloseToRight,
    CopyPath,
    RevealInExplorer,
    Pin,
    Unpin,
}

/// Pending context menu state.
#[derive(Clone, Debug)]
pub struct TabContextMenu {
    pub tab_index: usize,
    pub x: f32,
    pub y: f32,
    pub is_pinned: bool,
}

// ── Drag state ───────────────────────────────────────────────────────────────

/// Drag state for tab reordering.
#[derive(Clone, Debug, Default)]
struct DragState {
    active: bool,
    source_index: usize,
    current_x: f32,
    target_group: Option<usize>,
}

// ── Tab bar ──────────────────────────────────────────────────────────────────

/// A tab bar with selection, close, overflow scrolling, drag reordering,
/// pinned tabs, preview (italic) tabs, context menu, and configurable sizing.
#[allow(dead_code)]
pub struct TabBar<S, C>
where
    S: FnMut(usize),
    C: FnMut(usize),
{
    pub tabs: Vec<Tab>,
    pub active: usize,
    pub on_select: S,
    pub on_close: C,

    // ── Sizing configuration ─────────────────────────────────────
    pub sizing_mode: TabSizingMode,
    tab_height: f32,
    tab_min_width: f32,
    tab_max_width: f32,
    pinned_tab_width: f32,
    fixed_tab_width: f32,
    scroll_offset: f32,
    font_size: f32,

    // ── Colors ───────────────────────────────────────────────────
    active_bg: Color,
    active_fg: Color,
    inactive_bg: Color,
    inactive_fg: Color,
    hover_bg: Color,
    border_color: Color,
    active_border_bottom: Color,
    dirty_dot_color: Color,
    close_hover_bg: Color,
    close_fg: Color,
    drop_indicator: Color,
    pinned_border: Color,

    // ── Interaction state ────────────────────────────────────────
    hovered_tab: Option<usize>,
    hovered_close: Option<usize>,
    drag: DragState,
    pub context_menu: Option<TabContextMenu>,

    // ── Overflow ─────────────────────────────────────────────────
    overflow_visible: bool,
}

impl<S, C> TabBar<S, C>
where
    S: FnMut(usize),
    C: FnMut(usize),
{
    pub fn new(tabs: Vec<Tab>, active: usize, on_select: S, on_close: C) -> Self {
        Self {
            tabs,
            active,
            on_select,
            on_close,
            sizing_mode: TabSizingMode::Fit,
            tab_height: 35.0,
            tab_min_width: 80.0,
            tab_max_width: 200.0,
            pinned_tab_width: 42.0,
            fixed_tab_width: 120.0,
            scroll_offset: 0.0,
            font_size: 12.0,
            active_bg: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            active_fg: Color::WHITE,
            inactive_bg: Color::from_hex("#2d2d2d").unwrap_or(Color::BLACK),
            inactive_fg: Color::from_hex("#ffffff80").unwrap_or(Color::WHITE),
            hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            active_border_bottom: Color::from_hex("#007acc").unwrap_or(Color::WHITE),
            dirty_dot_color: Color::from_hex("#e8e8e8").unwrap_or(Color::WHITE),
            close_hover_bg: Color::from_hex("#404040").unwrap_or(Color::BLACK),
            close_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            drop_indicator: Color::from_hex("#007acc").unwrap_or(Color::WHITE),
            pinned_border: Color::from_hex("#585858").unwrap_or(Color::BLACK),
            hovered_tab: None,
            hovered_close: None,
            drag: DragState::default(),
            context_menu: None,
            overflow_visible: false,
        }
    }

    /// Set the sizing mode (from settings).
    pub fn set_sizing_mode(&mut self, mode: TabSizingMode) {
        self.sizing_mode = mode;
    }

    /// Width of a single tab, respecting sizing mode and pinned status.
    fn tab_width_for(&self, tab: &Tab, container_width: f32) -> f32 {
        if tab.is_pinned {
            return self.pinned_tab_width;
        }
        match self.sizing_mode {
            TabSizingMode::Fixed => self.fixed_tab_width,
            TabSizingMode::Shrink => {
                let non_pinned = self.tabs.iter().filter(|t| !t.is_pinned).count();
                let pinned_total: f32 = self
                    .tabs
                    .iter()
                    .filter(|t| t.is_pinned)
                    .map(|_| self.pinned_tab_width)
                    .sum();
                let avail = container_width - pinned_total;
                if non_pinned > 0 {
                    (avail / non_pinned as f32)
                        .clamp(self.tab_min_width, self.tab_max_width)
                } else {
                    self.tab_min_width
                }
            }
            TabSizingMode::Fit => {
                let non_pinned = self.tabs.iter().filter(|t| !t.is_pinned).count();
                let pinned_total: f32 = self
                    .tabs
                    .iter()
                    .filter(|t| t.is_pinned)
                    .map(|_| self.pinned_tab_width)
                    .sum();
                let avail = container_width - pinned_total;
                if non_pinned > 0 {
                    (avail / non_pinned as f32).min(self.tab_max_width)
                } else {
                    self.tab_max_width
                }
            }
        }
    }

    fn tab_rect_at_with_width(&self, index: usize, container: Rect) -> Rect {
        let mut x = container.x - self.scroll_offset;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = self.tab_width_for(tab, container.width);
            if i == index {
                return Rect::new(x, container.y, w, self.tab_height);
            }
            x += w;
        }
        Rect::new(x, container.y, 0.0, self.tab_height)
    }

    fn close_button_rect(&self, tab_rect: Rect) -> Rect {
        let size = 16.0;
        Rect::new(
            tab_rect.x + tab_rect.width - size - 8.0,
            tab_rect.y + (tab_rect.height - size) / 2.0,
            size,
            size,
        )
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn tab_index_at(&self, x: f32, container: Rect) -> Option<usize> {
        let mut cursor = container.x - self.scroll_offset;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = self.tab_width_for(tab, container.width);
            if x >= cursor && x < cursor + w {
                return Some(i);
            }
            cursor += w;
        }
        None
    }

    fn total_width(&self, container_width: f32) -> f32 {
        self.tabs
            .iter()
            .map(|t| self.tab_width_for(t, container_width))
            .sum()
    }

    /// Check whether tabs overflow the container.
    pub fn has_overflow(&self, container_width: f32) -> bool {
        self.total_width(container_width) > container_width
    }

    /// Ensure the active tab is scrolled into view.
    pub fn ensure_active_visible(&mut self, container_width: f32) {
        let rect = self.tab_rect_at_with_width(
            self.active,
            Rect::new(0.0, 0.0, container_width, self.tab_height),
        );
        let tab_start = rect.x + self.scroll_offset;
        let tab_end = tab_start + rect.width;

        if tab_start < self.scroll_offset {
            self.scroll_offset = tab_start;
        } else if tab_end > self.scroll_offset + container_width {
            self.scroll_offset = tab_end - container_width;
        }
    }

    /// Render the tab bar using the draw context API (themed, full-featured).
    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        ctx.save();
        ctx.clip(rect);

        // Background
        let bar_bg = Rect::new(rect.x, rect.y, rect.width, self.tab_height);
        ctx.draw_rect(bar_bg, self.inactive_bg, 0.0);

        for (i, tab) in self.tabs.iter().enumerate() {
            let tr = self.tab_rect_at_with_width(i, rect);
            if tr.right() < rect.x || tr.x > rect.right() {
                continue;
            }

            let is_active = i == self.active;
            let is_hovered = self.hovered_tab == Some(i) && !is_active;

            // Tab background
            let bg = if is_active {
                self.active_bg
            } else if is_hovered {
                self.hover_bg
            } else {
                self.inactive_bg
            };
            ctx.draw_rect(tr, bg, 0.0);

            // Right separator
            let sep = Rect::new(tr.right() - 1.0, tr.y + 4.0, 1.0, tr.height - 8.0);
            ctx.draw_rect(sep, self.border_color, 0.0);

            // Pinned tab right border
            if tab.is_pinned {
                if let Some(next) = self.tabs.get(i + 1) {
                    if !next.is_pinned {
                        let pinned_sep = Rect::new(tr.right() - 1.0, tr.y, 1.0, tr.height);
                        ctx.draw_rect(pinned_sep, self.pinned_border, 0.0);
                    }
                }
            }

            // Active bottom border
            if is_active {
                let bot = Rect::new(tr.x, tr.y + tr.height - 2.0, tr.width, 2.0);
                ctx.draw_rect(bot, self.active_border_bottom, 0.0);
            }

            // Tab content
            let fg = if is_active {
                self.active_fg
            } else {
                self.inactive_fg
            };

            if tab.is_pinned {
                // Pinned tab: just show pin icon centered
                let icon_size = 14.0;
                let ix = tr.x + (tr.width - icon_size) / 2.0;
                let iy = tr.y + (tr.height - icon_size) / 2.0;
                ctx.draw_icon(IconId::Pin, (ix, iy), icon_size, fg);

                // Dirty indicator on pinned tab
                if tab.is_dirty {
                    let dot_r = 3.0;
                    let dot_x = tr.x + tr.width - dot_r * 2.0 - 4.0;
                    let dot_y = tr.y + 4.0;
                    let dot_rect = Rect::new(dot_x, dot_y, dot_r * 2.0, dot_r * 2.0);
                    ctx.draw_rect(dot_rect, self.dirty_dot_color, dot_r);
                }
            } else {
                let text_x = tr.x + 12.0;
                let text_y = tr.y + (tr.height - self.font_size) / 2.0;

                // Dirty dot (before filename)
                if tab.is_dirty {
                    let dot_r = 4.0;
                    let dot_y = tr.y + (tr.height - dot_r * 2.0) / 2.0;
                    let dot_rect =
                        Rect::new(text_x - 10.0, dot_y, dot_r * 2.0, dot_r * 2.0);
                    ctx.draw_rect(dot_rect, self.dirty_dot_color, dot_r);
                }

                // Label (italic for preview tabs)
                ctx.draw_text(
                    &tab.label,
                    (text_x, text_y),
                    fg,
                    self.font_size,
                    false,
                    tab.is_preview,
                );

                // Close button — hidden for inactive tabs until hover
                let cr = self.close_button_rect(tr);
                let show_close = is_active || self.hovered_tab == Some(i);
                if show_close {
                    if self.hovered_close == Some(i) {
                        ctx.draw_rect(cr, self.close_hover_bg, 2.0);
                    }
                    if !tab.is_dirty || self.hovered_close == Some(i) {
                        ctx.draw_icon(
                            IconId::Close,
                            (cr.x + 2.0, cr.y + 2.0),
                            12.0,
                            self.close_fg,
                        );
                    }
                }
            }
        }

        // Drag drop indicator
        if self.drag.active {
            let rel_x = self.drag.current_x;
            let mut best_idx = 0;
            let mut cursor = rect.x - self.scroll_offset;
            for (i, tab) in self.tabs.iter().enumerate() {
                let w = self.tab_width_for(tab, rect.width);
                if rel_x > cursor + w / 2.0 {
                    best_idx = i + 1;
                }
                cursor += w;
            }
            let mut drop_x = rect.x - self.scroll_offset;
            for (i, tab) in self.tabs.iter().enumerate() {
                if i == best_idx {
                    break;
                }
                drop_x += self.tab_width_for(tab, rect.width);
            }
            let indicator = Rect::new(drop_x - 1.0, rect.y, 2.0, self.tab_height);
            ctx.draw_rect(indicator, self.drop_indicator, 0.0);
        }

        // Overflow indicator (>> dropdown)
        let total = self.total_width(rect.width);
        if total > rect.width {
            let btn_w = 28.0;
            let arrow_rect = Rect::new(rect.right() - btn_w, rect.y, btn_w, self.tab_height);
            ctx.draw_rect(arrow_rect, self.active_bg, 0.0);
            ctx.draw_icon(
                IconId::MoreHorizontal,
                (arrow_rect.x + 8.0, rect.y + 10.0),
                14.0,
                self.active_fg,
            );
        }

        ctx.restore();
    }
}

impl<S, C> Widget for TabBar<S, C>
where
    S: FnMut(usize),
    C: FnMut(usize),
{
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Fixed(self.tab_height),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        for (i, tab) in self.tabs.iter().enumerate() {
            let tr = self.tab_rect_at_with_width(i, rect);
            if tr.right() < rect.x || tr.x > rect.right() {
                continue;
            }
            let is_active = i == self.active;
            let bg = if is_active {
                self.active_bg
            } else {
                self.inactive_bg
            };
            rr.draw_rect(tr.x, tr.y, tr.width, tr.height, bg, 0.0);
            rr.draw_rect(
                tr.right() - 1.0,
                tr.y,
                1.0,
                tr.height,
                self.border_color,
                0.0,
            );

            // Pinned border
            if tab.is_pinned {
                if let Some(next) = self.tabs.get(i + 1) {
                    if !next.is_pinned {
                        rr.draw_rect(
                            tr.right() - 1.0,
                            tr.y,
                            1.0,
                            tr.height,
                            self.pinned_border,
                            0.0,
                        );
                    }
                }
            }

            if tab.is_dirty && !tab.is_pinned {
                let dot_r = 4.0;
                let close_r = self.close_button_rect(tr);
                rr.draw_rect(
                    close_r.x + close_r.width / 2.0 - dot_r,
                    close_r.y + close_r.height / 2.0 - dot_r,
                    dot_r * 2.0,
                    dot_r * 2.0,
                    self.dirty_dot_color,
                    dot_r,
                );
            }
            if self.hovered_close == Some(i) && !tab.is_pinned {
                let cr = self.close_button_rect(tr);
                rr.draw_rect(cr.x, cr.y, cr.width, cr.height, self.close_hover_bg, 2.0);
            }
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::MouseMove { x, y } => {
                if self.drag.active {
                    self.drag.current_x = *x;
                    return EventResult::Handled;
                }
                if rect.contains(*x, *y) {
                    self.hovered_tab = self.tab_index_at(*x, rect);
                    self.hovered_close = self.hovered_tab.filter(|&i| {
                        if self.tabs.get(i).map_or(false, |t| t.is_pinned) {
                            return false;
                        }
                        let tr = self.tab_rect_at_with_width(i, rect);
                        let cr = self.close_button_rect(tr);
                        cr.contains(*x, *y)
                    });
                } else {
                    self.hovered_tab = None;
                    self.hovered_close = None;
                }
                EventResult::Ignored
            }

            // Left click — select or close
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.context_menu = None;
                if let Some(idx) = self.tab_index_at(*x, rect) {
                    let tr = self.tab_rect_at_with_width(idx, rect);
                    let cr = self.close_button_rect(tr);
                    let is_pinned = self.tabs.get(idx).map_or(false, |t| t.is_pinned);

                    if cr.contains(*x, *y) && !is_pinned {
                        (self.on_close)(idx);
                    } else {
                        self.active = idx;
                        (self.on_select)(idx);
                        self.drag = DragState {
                            active: true,
                            source_index: idx,
                            current_x: *x,
                            target_group: None,
                        };
                    }
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }

            // Middle click — close tab
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Middle,
            } if rect.contains(*x, *y) => {
                if let Some(idx) = self.tab_index_at(*x, rect) {
                    (self.on_close)(idx);
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }

            // Right click — context menu
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Right,
            } if rect.contains(*x, *y) => {
                if let Some(idx) = self.tab_index_at(*x, rect) {
                    let is_pinned = self.tabs.get(idx).map_or(false, |t| t.is_pinned);
                    self.context_menu = Some(TabContextMenu {
                        tab_index: idx,
                        x: *x,
                        y: *y,
                        is_pinned,
                    });
                    EventResult::Handled
                } else {
                    EventResult::Ignored
                }
            }

            // Double-click empty space — new file
            UiEvent::DoubleClick { x, y } if rect.contains(*x, *y) => {
                if self.tab_index_at(*x, rect).is_none() {
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }

            // Mouse up — end drag, maybe reorder
            UiEvent::MouseUp { .. } if self.drag.active => {
                let source = self.drag.source_index;
                if let Some(target) = self.drop_target_index(rect) {
                    if target != source && target != source + 1 {
                        let adjusted = if target > source {
                            target - 1
                        } else {
                            target
                        };
                        if source < self.tabs.len() {
                            let tab = self.tabs.remove(source);
                            let insert_at = adjusted.min(self.tabs.len());
                            self.tabs.insert(insert_at, tab);
                            self.active = insert_at;
                        }
                    }
                }
                self.drag = DragState::default();
                EventResult::Handled
            }

            // Scroll — overflow navigation
            UiEvent::MouseScroll { dx, .. } => {
                let total_w = self.total_width(rect.width);
                let max = (total_w - rect.width).max(0.0);
                self.scroll_offset = (self.scroll_offset - dx * 30.0).clamp(0.0, max);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

impl<S, C> TabBar<S, C>
where
    S: FnMut(usize),
    C: FnMut(usize),
{
    /// Compute the drop-target index from the drag state.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn drop_target_index(&self, rect: Rect) -> Option<usize> {
        if !self.drag.active {
            return None;
        }
        let rel_x = self.drag.current_x;
        let mut cursor = rect.x - self.scroll_offset;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = self.tab_width_for(tab, rect.width);
            if rel_x < cursor + w / 2.0 {
                return Some(i);
            }
            cursor += w;
        }
        Some(self.tabs.len())
    }
}
