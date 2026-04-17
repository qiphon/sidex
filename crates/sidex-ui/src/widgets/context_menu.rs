//! Context menu with nested submenus, separators, and keyboard navigation.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// A single item in a context menu.
#[derive(Clone, Debug)]
pub enum MenuItem {
    Action {
        label: String,
        command: Option<String>,
        shortcut: Option<String>,
        icon: Option<String>,
        enabled: bool,
        checked: bool,
        group: Option<String>,
    },
    Submenu {
        label: String,
        icon: Option<String>,
        items: Vec<MenuItem>,
    },
    Separator,
}

impl MenuItem {
    pub fn action(label: impl Into<String>) -> Self {
        Self::Action {
            label: label.into(),
            command: None,
            shortcut: None,
            icon: None,
            enabled: true,
            checked: false,
            group: None,
        }
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        if let Self::Action {
            shortcut: ref mut s,
            ..
        } = self
        {
            *s = Some(shortcut.into());
        }
        self
    }

    pub fn disabled(mut self) -> Self {
        if let Self::Action {
            enabled: ref mut e, ..
        } = self
        {
            *e = false;
        }
        self
    }

    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        if let Self::Action { command: ref mut c, .. } = self {
            *c = Some(command.into());
        }
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        if let Self::Action { group: ref mut g, .. } = self {
            *g = Some(group.into());
        }
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        match self {
            Self::Action { icon: ref mut i, .. } | Self::Submenu { icon: ref mut i, .. } => {
                *i = Some(icon.into());
            }
            Self::Separator => {}
        }
        self
    }

    pub fn submenu(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self::Submenu {
            label: label.into(),
            icon: None,
            items,
        }
    }

    fn is_separator(&self) -> bool {
        matches!(self, Self::Separator)
    }

    fn is_enabled(&self) -> bool {
        match self {
            Self::Action { enabled, .. } => *enabled,
            Self::Submenu { .. } => true,
            Self::Separator => false,
        }
    }
}

/// A popup context menu displayed at a screen position.
#[allow(dead_code)]
pub struct ContextMenu<F: FnMut(usize)> {
    pub items: Vec<MenuItem>,
    pub position: (f32, f32),
    pub on_select: F,

    row_height: f32,
    menu_width: f32,
    font_size: f32,
    icon_col_width: f32,
    shortcut_right_margin: f32,
    hovered_index: Option<usize>,
    active_submenu: Option<usize>,
    keyboard_index: Option<usize>,
    visible: bool,

    background: Color,
    border_color: Color,
    shadow_color: Color,
    hover_bg: Color,
    foreground: Color,
    disabled_fg: Color,
    separator_color: Color,
    shortcut_fg: Color,
    check_color: Color,
    submenu_arrow_fg: Color,
}

impl<F: FnMut(usize)> ContextMenu<F> {
    pub fn new(items: Vec<MenuItem>, position: (f32, f32), on_select: F) -> Self {
        Self {
            items,
            position,
            on_select,
            row_height: 26.0,
            menu_width: 220.0,
            font_size: 12.0,
            icon_col_width: 28.0,
            shortcut_right_margin: 12.0,
            hovered_index: None,
            active_submenu: None,
            keyboard_index: None,
            visible: true,
            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shadow_color: Color::from_hex("#00000080").unwrap_or(Color::BLACK),
            hover_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            disabled_fg: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shortcut_fg: Color::from_hex("#aaaaaa").unwrap_or(Color::WHITE),
            check_color: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            submenu_arrow_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.keyboard_index = None;
        self.hovered_index = None;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    fn menu_rect(&self) -> Rect {
        let h = self.items.iter().fold(0.0_f32, |acc, item| {
            acc + if item.is_separator() {
                9.0
            } else {
                self.row_height
            }
        });
        Rect::new(self.position.0, self.position.1, self.menu_width, h + 4.0)
    }

    fn item_rect_at(&self, index: usize) -> Rect {
        let base = self.menu_rect();
        let mut y = base.y + 2.0;
        for (i, item) in self.items.iter().enumerate() {
            let h = if item.is_separator() {
                9.0
            } else {
                self.row_height
            };
            if i == index {
                return Rect::new(base.x, y, base.width, h);
            }
            y += h;
        }
        Rect::ZERO
    }

    fn next_enabled(&self, from: usize, forward: bool) -> Option<usize> {
        let len = self.items.len();
        for offset in 1..=len {
            let idx = if forward {
                (from + offset) % len
            } else {
                (from + len - offset) % len
            };
            if self.items[idx].is_enabled() {
                return Some(idx);
            }
        }
        None
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, _viewport: Rect) {
        if !self.visible {
            return;
        }
        let mr = self.menu_rect();

        // Shadow
        let shadow = Rect::new(mr.x + 2.0, mr.y + 2.0, mr.width, mr.height);
        ctx.draw_rect(shadow, self.shadow_color, 4.0);

        // Background
        ctx.draw_rect(mr, self.background, 4.0);
        ctx.draw_border(mr, self.border_color, 1.0, 4.0);

        for (i, item) in self.items.iter().enumerate() {
            let ir = self.item_rect_at(i);

            if item.is_separator() {
                let cy = ir.y + ir.height / 2.0;
                let sep = Rect::new(ir.x + 8.0, cy, ir.width - 16.0, 1.0);
                ctx.draw_rect(sep, self.separator_color, 0.0);
                continue;
            }

            let is_hover = self.hovered_index == Some(i) || self.keyboard_index == Some(i);
            let is_enabled = item.is_enabled();

            // Hover highlight
            if is_hover && is_enabled {
                let hr = Rect::new(ir.x + 2.0, ir.y, ir.width - 4.0, ir.height);
                ctx.draw_rect(hr, self.hover_bg, 2.0);
            }

            let fg = if is_enabled {
                self.foreground
            } else {
                self.disabled_fg
            };
            let text_y = ir.y + (ir.height - self.font_size) / 2.0;

            match item {
                MenuItem::Action {
                    label,
                    shortcut,
                    icon: _,
                    enabled: _,
                    checked,
                    command: _,
                    group: _,
                } => {
                    // Check mark
                    if *checked {
                        ctx.draw_icon(
                            IconId::Check,
                            (ir.x + 6.0, text_y),
                            self.font_size,
                            self.check_color,
                        );
                    }

                    // Label
                    ctx.draw_text(
                        label,
                        (ir.x + self.icon_col_width, text_y),
                        fg,
                        self.font_size,
                        false,
                        false,
                    );

                    // Shortcut key text (right-aligned)
                    if let Some(sc) = shortcut {
                        let sc_w = sc.len() as f32 * self.font_size * 0.6;
                        let sc_x = ir.x + ir.width - sc_w - self.shortcut_right_margin;
                        ctx.draw_text(
                            sc,
                            (sc_x, text_y),
                            self.shortcut_fg,
                            self.font_size,
                            false,
                            false,
                        );
                    }
                }
                MenuItem::Submenu { label, .. } => {
                    ctx.draw_text(
                        label,
                        (ir.x + self.icon_col_width, text_y),
                        fg,
                        self.font_size,
                        false,
                        false,
                    );
                    // Submenu arrow indicator (>)
                    ctx.draw_icon(
                        IconId::ChevronRight,
                        (ir.x + ir.width - 16.0, text_y),
                        self.font_size,
                        self.submenu_arrow_fg,
                    );
                }
                MenuItem::Separator => {}
            }
        }
    }
}

impl<F: FnMut(usize)> Widget for ContextMenu<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Auto,
            ..LayoutNode::default()
        }
    }

    fn render(&self, _rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible {
            return;
        }
        let mr = self.menu_rect();
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(mr.x, mr.y, mr.width, mr.height, self.background, 4.0);
        rr.draw_border(mr.x, mr.y, mr.width, mr.height, self.border_color, 1.0);
        for (i, item) in self.items.iter().enumerate() {
            let ir = self.item_rect_at(i);
            if item.is_separator() {
                let cy = ir.y + ir.height / 2.0;
                rr.draw_rect(
                    ir.x + 8.0,
                    cy,
                    ir.width - 16.0,
                    1.0,
                    self.separator_color,
                    0.0,
                );
                continue;
            }
            let is_hover = self.hovered_index == Some(i) || self.keyboard_index == Some(i);
            if is_hover && item.is_enabled() {
                rr.draw_rect(
                    ir.x + 2.0,
                    ir.y,
                    ir.width - 4.0,
                    ir.height,
                    self.hover_bg,
                    2.0,
                );
            }
        }
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, _rect: Rect) -> EventResult {
        if !self.visible {
            return EventResult::Ignored;
        }
        let mr = self.menu_rect();
        match event {
            UiEvent::MouseMove { x, y } => {
                if mr.contains(*x, *y) {
                    self.hovered_index = None;
                    for (i, _) in self.items.iter().enumerate() {
                        let ir = self.item_rect_at(i);
                        if ir.contains(*x, *y) && !self.items[i].is_separator() {
                            self.hovered_index = Some(i);
                            if matches!(self.items[i], MenuItem::Submenu { .. }) {
                                self.active_submenu = Some(i);
                            }
                            break;
                        }
                    }
                    EventResult::Handled
                } else {
                    self.hovered_index = None;
                    EventResult::Ignored
                }
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                if mr.contains(*x, *y) {
                    if let Some(idx) = self.hovered_index {
                        if self.items[idx].is_enabled()
                            && !matches!(self.items[idx], MenuItem::Submenu { .. })
                        {
                            (self.on_select)(idx);
                            self.hide();
                        }
                    }
                    EventResult::Handled
                } else {
                    self.hide();
                    EventResult::Handled
                }
            }
            UiEvent::KeyPress { key, .. } => match key {
                Key::ArrowDown => {
                    let from = self.keyboard_index.unwrap_or(self.items.len() - 1);
                    self.keyboard_index = self.next_enabled(from, true);
                    EventResult::Handled
                }
                Key::ArrowUp => {
                    let from = self.keyboard_index.unwrap_or(0);
                    self.keyboard_index = self.next_enabled(from, false);
                    EventResult::Handled
                }
                Key::Enter | Key::Space => {
                    if let Some(idx) = self.keyboard_index {
                        if self.items[idx].is_enabled()
                            && !matches!(self.items[idx], MenuItem::Submenu { .. })
                        {
                            (self.on_select)(idx);
                            self.hide();
                        }
                    }
                    EventResult::Handled
                }
                Key::Escape => {
                    self.hide();
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            },
            _ => EventResult::Ignored,
        }
    }
}

// ── Built-in context menus ──────────────────────────────────────────────────

/// Editor right-click context menu.
pub fn editor_context_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action("Cut").with_shortcut("Ctrl+X").with_command("editor.action.clipboardCutAction"),
        MenuItem::action("Copy").with_shortcut("Ctrl+C").with_command("editor.action.clipboardCopyAction"),
        MenuItem::action("Paste").with_shortcut("Ctrl+V").with_command("editor.action.clipboardPasteAction"),
        MenuItem::action("Select All").with_shortcut("Ctrl+A").with_command("editor.action.selectAll"),
        MenuItem::Separator,
        MenuItem::action("Change All Occurrences").with_shortcut("Ctrl+F2").with_command("editor.action.changeAll"),
        MenuItem::action("Format Document").with_shortcut("Shift+Alt+F").with_command("editor.action.formatDocument"),
        MenuItem::Separator,
        MenuItem::action("Go to Definition").with_shortcut("F12").with_command("editor.action.revealDefinition"),
        MenuItem::action("Peek Definition").with_shortcut("Alt+F12").with_command("editor.action.peekDefinition"),
        MenuItem::action("Go to References").with_shortcut("Shift+F12").with_command("editor.action.goToReferences"),
        MenuItem::Separator,
        MenuItem::action("Rename Symbol").with_shortcut("F2").with_command("editor.action.rename"),
        MenuItem::submenu("Refactor...", vec![
            MenuItem::action("Extract Method").with_command("editor.action.extractMethod"),
            MenuItem::action("Extract Variable").with_command("editor.action.extractVariable"),
            MenuItem::action("Inline Variable").with_command("editor.action.inlineVariable"),
        ]),
        MenuItem::submenu("Source Action...", vec![
            MenuItem::action("Organize Imports").with_command("editor.action.organizeImports"),
            MenuItem::action("Sort Imports").with_command("editor.action.sortImports"),
        ]),
        MenuItem::Separator,
        MenuItem::action("Command Palette...").with_shortcut("Ctrl+Shift+P").with_command("workbench.action.showCommands"),
    ]
}

/// File explorer right-click context menu.
pub fn explorer_context_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action("New File...").with_command("explorer.newFile"),
        MenuItem::action("New Folder...").with_command("explorer.newFolder"),
        MenuItem::Separator,
        MenuItem::action("Reveal in Finder").with_command("revealFileInOS"),
        MenuItem::action("Open in Integrated Terminal").with_command("openInIntegratedTerminal"),
        MenuItem::Separator,
        MenuItem::action("Cut").with_shortcut("Ctrl+X").with_command("filesExplorer.cut"),
        MenuItem::action("Copy").with_shortcut("Ctrl+C").with_command("filesExplorer.copy"),
        MenuItem::action("Paste").with_shortcut("Ctrl+V").with_command("filesExplorer.paste"),
        MenuItem::Separator,
        MenuItem::action("Copy Path").with_shortcut("Ctrl+Shift+C").with_command("copyFilePath"),
        MenuItem::action("Copy Relative Path").with_shortcut("Ctrl+Shift+Alt+C").with_command("copyRelativeFilePath"),
        MenuItem::Separator,
        MenuItem::action("Rename").with_shortcut("F2").with_command("renameFile"),
        MenuItem::action("Delete").with_shortcut("Delete").with_command("moveFileToTrash"),
    ]
}

/// Tab bar right-click context menu.
pub fn tab_context_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action("Close").with_shortcut("Ctrl+W").with_command("workbench.action.closeActiveEditor"),
        MenuItem::action("Close Others").with_command("workbench.action.closeOtherEditors"),
        MenuItem::action("Close to the Right").with_command("workbench.action.closeEditorsToTheRight"),
        MenuItem::action("Close All").with_command("workbench.action.closeAllEditors"),
        MenuItem::Separator,
        MenuItem::action("Copy Path").with_command("copyFilePath"),
        MenuItem::action("Reveal in Explorer").with_command("workbench.action.files.revealActiveFileInExplorer"),
        MenuItem::Separator,
        MenuItem::action("Split Right").with_command("workbench.action.splitEditorRight"),
        MenuItem::action("Split Down").with_command("workbench.action.splitEditorDown"),
    ]
}

/// Editor gutter right-click context menu.
pub fn gutter_context_menu() -> Vec<MenuItem> {
    vec![
        MenuItem::action("Toggle Breakpoint").with_shortcut("F9").with_command("editor.debug.action.toggleBreakpoint"),
        MenuItem::action("Add Conditional Breakpoint...").with_command("editor.debug.action.conditionalBreakpoint"),
        MenuItem::action("Add Logpoint...").with_command("editor.debug.action.addLogPoint"),
        MenuItem::Separator,
        MenuItem::action("Run to Cursor").with_command("editor.debug.action.runToCursor"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_context_menu_has_items() {
        let items = editor_context_menu();
        assert!(items.len() > 10);
    }

    #[test]
    fn explorer_context_menu_has_items() {
        let items = explorer_context_menu();
        assert!(items.len() > 8);
    }

    #[test]
    fn tab_context_menu_has_items() {
        let items = tab_context_menu();
        assert!(items.len() > 5);
    }

    #[test]
    fn gutter_context_menu_has_items() {
        let items = gutter_context_menu();
        assert!(items.len() >= 4);
    }

    #[test]
    fn action_with_command() {
        let item = MenuItem::action("Test").with_command("test.command");
        if let MenuItem::Action { command, .. } = item {
            assert_eq!(command, Some("test.command".to_string()));
        } else {
            panic!("Expected Action");
        }
    }
}

