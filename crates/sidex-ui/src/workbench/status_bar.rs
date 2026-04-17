//! Status bar at the bottom of the workbench.
//!
//! Implements a full VS Code-compatible status bar with left/right item
//! alignment, context-dependent background colors, visibility conditions,
//! click commands, and tooltips.

use std::collections::HashMap;

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, MouseButton, UiEvent, Widget};

// ── Alignment ────────────────────────────────────────────────────────────────

/// Alignment of a status bar item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusBarAlignment {
    #[default]
    Left,
    Right,
}

// ── Mode ─────────────────────────────────────────────────────────────────────

/// Context-dependent background mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusBarMode {
    #[default]
    Normal,
    Debugging,
    Remote,
    NoFolder,
}

// ── Visibility condition ─────────────────────────────────────────────────────

/// When a status bar item should be shown.
#[derive(Clone, Debug)]
pub enum ShowWhen {
    Always,
    Never,
    NonZero,
    HasEditor,
    HasSelection,
    IsRemote,
    Custom(String),
}

impl Default for ShowWhen {
    fn default() -> Self {
        Self::Always
    }
}

// ── Status bar item ──────────────────────────────────────────────────────────

/// A single status bar item.
#[derive(Clone, Debug)]
pub struct StatusBarItem {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
    pub icon: Option<IconId>,
    pub alignment: StatusBarAlignment,
    pub priority: i32,
    pub color: Option<Color>,
    pub background_color: Option<Color>,
    pub command: Option<String>,
    pub show_when: ShowWhen,
    pub visible: bool,
}

impl StatusBarItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            tooltip: None,
            icon: None,
            alignment: StatusBarAlignment::Left,
            priority: 0,
            color: None,
            background_color: None,
            command: None,
            show_when: ShowWhen::Always,
            visible: true,
        }
    }

    pub fn right(mut self) -> Self {
        self.alignment = StatusBarAlignment::Right;
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn with_icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    pub fn with_show_when(mut self, show_when: ShowWhen) -> Self {
        self.show_when = show_when;
        self
    }

    pub fn with_background(mut self, color: Color) -> Self {
        self.background_color = Some(color);
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

// ── Default items factory ────────────────────────────────────────────────────

/// Creates the full set of default VS Code status bar items.
pub fn default_status_bar_items() -> Vec<StatusBarItem> {
    vec![
        // ── Left side (high priority = rendered first) ──────────
        StatusBarItem::new("remote.indicator", "")
            .with_priority(10000)
            .with_tooltip("Remote Indicator")
            .with_icon(IconId::Remote)
            .with_show_when(ShowWhen::IsRemote)
            .with_command("workbench.action.remote.showMenu"),
        StatusBarItem::new("git.branch", "main")
            .with_priority(9000)
            .with_tooltip("Git Branch")
            .with_icon(IconId::GitBranch)
            .with_command("workbench.action.quickOpen"),
        StatusBarItem::new("git.sync", "")
            .with_priority(8900)
            .with_tooltip("Synchronize Changes")
            .with_show_when(ShowWhen::NonZero)
            .with_command("git.sync"),
        StatusBarItem::new("problems.errors", "0")
            .with_priority(8000)
            .with_tooltip("Errors")
            .with_icon(IconId::Error)
            .with_command("workbench.actions.view.problems"),
        StatusBarItem::new("problems.warnings", "0")
            .with_priority(7900)
            .with_tooltip("Warnings")
            .with_icon(IconId::Warning)
            .with_command("workbench.actions.view.problems"),
        // ── Right side (high priority = furthest right) ─────────
        StatusBarItem::new("notifications.bell", "")
            .right()
            .with_priority(10000)
            .with_tooltip("Notifications")
            .with_icon(IconId::Bell)
            .with_command("notifications.showList"),
        StatusBarItem::new("layout.indicator", "")
            .right()
            .with_priority(9500)
            .with_tooltip("Editor Layout")
            .with_icon(IconId::MoreHorizontal)
            .with_command("workbench.action.editorLayoutGroup"),
        StatusBarItem::new("feedback", "")
            .right()
            .with_priority(9000)
            .with_tooltip("Tweet Feedback")
            .with_icon(IconId::MoreHorizontal)
            .with_command("workbench.action.openGlobalSettings"),
        StatusBarItem::new("cursor.position", "Ln 1, Col 1")
            .right()
            .with_priority(8000)
            .with_tooltip("Go to Line/Column")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("workbench.action.gotoLine"),
        StatusBarItem::new("selection.info", "")
            .right()
            .with_priority(7900)
            .with_tooltip("Selection")
            .with_show_when(ShowWhen::HasSelection),
        StatusBarItem::new("editor.indent", "Spaces: 4")
            .right()
            .with_priority(7000)
            .with_tooltip("Select Indentation")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("editor.action.indentationToSpaces"),
        StatusBarItem::new("editor.encoding", "UTF-8")
            .right()
            .with_priority(6000)
            .with_tooltip("Select Encoding")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("workbench.action.editor.changeEncoding"),
        StatusBarItem::new("editor.eol", "LF")
            .right()
            .with_priority(5000)
            .with_tooltip("Select End of Line Sequence")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("workbench.action.editor.changeEOL"),
        StatusBarItem::new("editor.language", "Plain Text")
            .right()
            .with_priority(4000)
            .with_tooltip("Select Language Mode")
            .with_show_when(ShowWhen::HasEditor)
            .with_command("workbench.action.editor.changeLanguageMode"),
        StatusBarItem::new("copilot.status", "")
            .right()
            .with_priority(3000)
            .with_tooltip("Copilot Status")
            .with_icon(IconId::CircleFilled)
            .with_show_when(ShowWhen::Never),
    ]
}

// ── StatusBar widget ─────────────────────────────────────────────────────────

/// The status bar at the bottom of the window.
#[allow(dead_code)]
pub struct StatusBar<F: FnMut(&str)> {
    pub items: Vec<StatusBarItem>,
    pub on_click: F,
    pub mode: StatusBarMode,

    item_map: HashMap<String, usize>,

    height: f32,
    font_size: f32,
    hovered_index: Option<usize>,
    tooltip_hover_time: Option<std::time::Instant>,

    background: Color,
    foreground: Color,
    hover_bg: Color,
    border_color: Color,
    debug_bg: Color,
    debug_fg: Color,
    remote_bg: Color,
    remote_fg: Color,
    no_folder_bg: Color,
    error_fg: Color,
    warning_fg: Color,
}

impl<F: FnMut(&str)> StatusBar<F> {
    pub fn new(items: Vec<StatusBarItem>, on_click: F) -> Self {
        let item_map = items
            .iter()
            .enumerate()
            .map(|(i, item)| (item.id.clone(), i))
            .collect();
        Self {
            items,
            on_click,
            mode: StatusBarMode::Normal,
            item_map,
            height: 22.0,
            font_size: 12.0,
            hovered_index: None,
            tooltip_hover_time: None,
            background: Color::from_hex("#007acc").unwrap_or(Color::BLACK),
            foreground: Color::WHITE,
            hover_bg: Color::from_hex("#ffffff1f").unwrap_or(Color::WHITE),
            border_color: Color::from_hex("#ffffff1f").unwrap_or(Color::WHITE),
            debug_bg: Color::from_hex("#cc6633").unwrap_or(Color::BLACK),
            debug_fg: Color::WHITE,
            remote_bg: Color::from_hex("#16825d").unwrap_or(Color::BLACK),
            remote_fg: Color::WHITE,
            no_folder_bg: Color::from_hex("#68217a").unwrap_or(Color::BLACK),
            error_fg: Color::from_hex("#f14c4c").unwrap_or(Color::WHITE),
            warning_fg: Color::from_hex("#cca700").unwrap_or(Color::WHITE),
        }
    }

    /// Creates a status bar populated with all default VS Code items.
    pub fn with_defaults(on_click: F) -> Self {
        Self::new(default_status_bar_items(), on_click)
    }

    pub fn set_mode(&mut self, mode: StatusBarMode) {
        self.mode = mode;
    }

    /// Set the text of a status bar item by id.
    pub fn set_item(&mut self, id: &str, text: &str) {
        if let Some(&idx) = self.item_map.get(id) {
            self.items[idx].text = text.to_string();
        }
    }

    /// Set the visibility of a status bar item by id.
    pub fn set_item_visible(&mut self, id: &str, visible: bool) {
        if let Some(&idx) = self.item_map.get(id) {
            self.items[idx].visible = visible;
        }
    }

    /// Set the tooltip of a status bar item by id.
    pub fn set_item_tooltip(&mut self, id: &str, tooltip: &str) {
        if let Some(&idx) = self.item_map.get(id) {
            self.items[idx].tooltip = Some(tooltip.to_string());
        }
    }

    /// Set the color of a status bar item by id.
    pub fn set_item_color(&mut self, id: &str, color: Color) {
        if let Some(&idx) = self.item_map.get(id) {
            self.items[idx].color = Some(color);
        }
    }

    /// Set the background color of a status bar item by id.
    pub fn set_item_background(&mut self, id: &str, color: Color) {
        if let Some(&idx) = self.item_map.get(id) {
            self.items[idx].background_color = Some(color);
        }
    }

    /// Update the status bar background colors from theme values.
    pub fn apply_theme_colors(
        &mut self,
        bg: Color,
        fg: Color,
        debug_bg: Color,
        remote_bg: Color,
        no_folder_bg: Color,
    ) {
        self.background = bg;
        self.foreground = fg;
        self.debug_bg = debug_bg;
        self.remote_bg = remote_bg;
        self.no_folder_bg = no_folder_bg;
    }

    fn effective_bg(&self) -> Color {
        match self.mode {
            StatusBarMode::Normal => self.background,
            StatusBarMode::Debugging => self.debug_bg,
            StatusBarMode::Remote => self.remote_bg,
            StatusBarMode::NoFolder => self.no_folder_bg,
        }
    }

    fn effective_fg(&self) -> Color {
        match self.mode {
            StatusBarMode::Normal => self.foreground,
            StatusBarMode::Debugging => self.debug_fg,
            StatusBarMode::Remote => self.remote_fg,
            StatusBarMode::NoFolder => self.foreground,
        }
    }

    fn visible_items(&self) -> Vec<(usize, &StatusBarItem)> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.visible)
            .collect()
    }

    fn left_items(&self) -> Vec<(usize, &StatusBarItem)> {
        let mut items: Vec<_> = self
            .visible_items()
            .into_iter()
            .filter(|(_, i)| i.alignment == StatusBarAlignment::Left)
            .collect();
        items.sort_by_key(|(_, i)| std::cmp::Reverse(i.priority));
        items
    }

    fn right_items(&self) -> Vec<(usize, &StatusBarItem)> {
        let mut items: Vec<_> = self
            .visible_items()
            .into_iter()
            .filter(|(_, i)| i.alignment == StatusBarAlignment::Right)
            .collect();
        items.sort_by_key(|(_, i)| std::cmp::Reverse(i.priority));
        items
    }

    #[allow(clippy::cast_precision_loss)]
    fn item_width(&self, item: &StatusBarItem) -> f32 {
        let padding_h = 8.0;
        let icon_pad = 16.0;
        let icon_w = if item.icon.is_some() { icon_pad } else { 0.0 };
        let text_w = if item.text.is_empty() {
            0.0
        } else {
            item.text.len() as f32 * self.font_size * 0.6
        };
        let min_w = if item.icon.is_some() && item.text.is_empty() {
            icon_w + padding_h
        } else {
            text_w + icon_w + padding_h * 2.0
        };
        min_w.max(20.0)
    }

    #[allow(clippy::cast_precision_loss)]
    fn item_rects(&self, rect: Rect) -> Vec<(usize, Rect)> {
        let mut result = Vec::new();

        let mut x = rect.x;
        for (idx, item) in self.left_items() {
            let w = self.item_width(item);
            result.push((idx, Rect::new(x, rect.y, w, rect.height)));
            x += w;
        }

        let mut x = rect.x + rect.width;
        for (idx, item) in self.right_items() {
            let w = self.item_width(item);
            x -= w;
            result.push((idx, Rect::new(x, rect.y, w, rect.height)));
        }

        result
    }

    fn item_fg(&self, item: &StatusBarItem, fg: Color) -> Color {
        if let Some(c) = item.color {
            return c;
        }
        match item.id.as_str() {
            "problems.errors" if item.text != "0" => self.error_fg,
            "problems.warnings" if item.text != "0" => self.warning_fg,
            _ => fg,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, rect: Rect) {
        let bg = self.effective_bg();
        let fg = self.effective_fg();

        ctx.draw_rect(rect, bg, 0.0);

        let border = Rect::new(rect.x, rect.y, rect.width, 1.0);
        ctx.draw_rect(border, self.border_color, 0.0);

        for (idx, ir) in self.item_rects(rect) {
            let item = &self.items[idx];

            if let Some(item_bg) = item.background_color {
                ctx.draw_rect(ir, item_bg, 0.0);
            }

            if self.hovered_index == Some(idx) {
                ctx.draw_rect(ir, self.hover_bg, 0.0);
            }

            let item_fg = self.item_fg(item, fg);
            let text_y = ir.y + (ir.height - self.font_size) / 2.0;
            let mut text_x = ir.x + 8.0;

            if let Some(icon) = item.icon {
                let iy = ir.y + (ir.height - 12.0) / 2.0;
                ctx.draw_icon(icon, (text_x, iy), 12.0, item_fg);
                text_x += 16.0;
            }

            if !item.text.is_empty() {
                ctx.draw_text(
                    &item.text,
                    (text_x, text_y),
                    item_fg,
                    self.font_size,
                    false,
                    false,
                );
            }
        }

        // Tooltip rendering
        if let Some(hovered) = self.hovered_index {
            if let Some(tooltip) = &self.items[hovered].tooltip {
                if let Some(start) = self.tooltip_hover_time {
                    if start.elapsed().as_millis() > 400 {
                        if let Some((_, ir)) =
                            self.item_rects(rect).iter().find(|(i, _)| *i == hovered)
                        {
                            let tip_w = tooltip.len() as f32 * 7.0 + 16.0;
                            let tip_h = 22.0;
                            let tip_x = ir.x.min(rect.x + rect.width - tip_w);
                            let tip_y = rect.y - tip_h - 2.0;
                            let tip_rect = Rect::new(tip_x, tip_y, tip_w, tip_h);
                            let tip_bg =
                                Color::from_hex("#252526").unwrap_or(Color::BLACK);
                            let tip_border =
                                Color::from_hex("#454545").unwrap_or(Color::BLACK);
                            ctx.draw_rect(tip_rect, tip_bg, 4.0);
                            ctx.draw_border(tip_rect, tip_border, 1.0, 4.0);
                            ctx.draw_text(
                                tooltip,
                                (tip_x + 8.0, tip_y + 4.0),
                                Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
                                12.0,
                                false,
                                false,
                            );
                        }
                    }
                }
            }
        }

        if self.hovered_index.is_some() {
            ctx.set_cursor(CursorIcon::Pointer);
        }
    }
}

impl<F: FnMut(&str)> Widget for StatusBar<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Fixed(self.height),
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
            self.effective_bg(),
            0.0,
        );
        for (idx, ir) in self.item_rects(rect) {
            if let Some(bg) = self.items[idx].background_color {
                rr.draw_rect(ir.x, ir.y, ir.width, ir.height, bg, 0.0);
            }
            if self.hovered_index == Some(idx) {
                rr.draw_rect(ir.x, ir.y, ir.width, ir.height, self.hover_bg, 0.0);
            }
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        let item_rects = self.item_rects(rect);
        match event {
            UiEvent::MouseMove { x, y } => {
                let new_hover = item_rects
                    .iter()
                    .find(|(_, r)| r.contains(*x, *y))
                    .map(|(idx, _)| *idx);
                if new_hover != self.hovered_index {
                    self.hovered_index = new_hover;
                    self.tooltip_hover_time =
                        new_hover.map(|_| std::time::Instant::now());
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if let Some((idx, _)) = item_rects.iter().find(|(_, r)| r.contains(*x, *y)) {
                    let item = &self.items[*idx];
                    let cmd = item
                        .command
                        .as_deref()
                        .unwrap_or(&item.id);
                    let cmd_owned = cmd.to_string();
                    (self.on_click)(&cmd_owned);
                    return EventResult::Handled;
                }
                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_click(_: &str) {}

    #[test]
    fn default_items_count() {
        let items = default_status_bar_items();
        assert!(items.len() >= 14);
    }

    #[test]
    fn left_items_sorted_by_priority_desc() {
        let sb = StatusBar::new(default_status_bar_items(), noop_click);
        let left = sb.left_items();
        for w in left.windows(2) {
            assert!(w[0].1.priority >= w[1].1.priority);
        }
    }

    #[test]
    fn right_items_sorted_by_priority_desc() {
        let sb = StatusBar::new(default_status_bar_items(), noop_click);
        let right = sb.right_items();
        for w in right.windows(2) {
            assert!(w[0].1.priority >= w[1].1.priority);
        }
    }

    #[test]
    fn set_item_updates_text() {
        let mut sb = StatusBar::new(default_status_bar_items(), noop_click);
        sb.set_item("cursor.position", "Ln 42, Col 10");
        let item = sb.items.iter().find(|i| i.id == "cursor.position").unwrap();
        assert_eq!(item.text, "Ln 42, Col 10");
    }

    #[test]
    fn set_item_visible() {
        let mut sb = StatusBar::new(default_status_bar_items(), noop_click);
        sb.set_item_visible("copilot.status", true);
        let item = sb.items.iter().find(|i| i.id == "copilot.status").unwrap();
        assert!(item.visible);
    }

    #[test]
    fn effective_bg_modes() {
        let sb = StatusBar::new(vec![], noop_click);
        assert_eq!(sb.effective_bg(), sb.background);
        let mut sb2 = StatusBar::new(vec![], noop_click);
        sb2.set_mode(StatusBarMode::Debugging);
        assert_eq!(sb2.effective_bg(), sb2.debug_bg);
        sb2.set_mode(StatusBarMode::Remote);
        assert_eq!(sb2.effective_bg(), sb2.remote_bg);
        sb2.set_mode(StatusBarMode::NoFolder);
        assert_eq!(sb2.effective_bg(), sb2.no_folder_bg);
    }

    #[test]
    fn item_rects_non_overlapping() {
        let sb = StatusBar::new(default_status_bar_items(), noop_click);
        let rect = Rect::new(0.0, 0.0, 1200.0, 22.0);
        let rects = sb.item_rects(rect);
        let left_rects: Vec<_> = rects
            .iter()
            .filter(|(i, _)| sb.items[*i].alignment == StatusBarAlignment::Left)
            .collect();
        for w in left_rects.windows(2) {
            assert!(w[0].1.x + w[0].1.width <= w[1].1.x + 0.01);
        }
    }

    #[test]
    fn handle_click_fires_command() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let clicked = Rc::new(RefCell::new(String::new()));
        let clicked_clone = clicked.clone();
        let mut sb = StatusBar::new(default_status_bar_items(), move |cmd: &str| {
            *clicked_clone.borrow_mut() = cmd.to_string();
        });

        let rect = Rect::new(0.0, 0.0, 1200.0, 22.0);
        let rects = sb.item_rects(rect);

        if let Some((_, ir)) = rects.first() {
            let event = UiEvent::MouseDown {
                x: ir.x + 5.0,
                y: ir.y + 5.0,
                button: MouseButton::Left,
            };
            let result = sb.handle_event(&event, rect);
            assert_eq!(result, EventResult::Handled);
            assert!(!clicked.borrow().is_empty());
        }
    }

    #[test]
    fn apply_theme_colors() {
        let mut sb = StatusBar::new(vec![], noop_click);
        let new_bg = Color::from_rgb(10, 20, 30);
        sb.apply_theme_colors(
            new_bg,
            Color::WHITE,
            Color::BLACK,
            Color::BLACK,
            Color::BLACK,
        );
        assert_eq!(sb.background, new_bg);
    }

    #[test]
    fn with_defaults_creates_full_bar() {
        let sb = StatusBar::with_defaults(noop_click);
        assert!(sb.items.len() >= 14);
    }
}
