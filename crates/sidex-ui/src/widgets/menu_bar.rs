//! Application menu bar with nested submenus matching VS Code's menu structure.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::draw::{CursorIcon, DrawContext, IconId};
use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Menu item ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum MenuBarMenuItem {
    Action {
        label: String,
        command: String,
        keybinding: Option<String>,
        enabled: bool,
        checked: Option<bool>,
    },
    Submenu {
        label: String,
        items: Vec<MenuBarMenuItem>,
    },
    Separator,
}

impl MenuBarMenuItem {
    pub fn action(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self::Action {
            label: label.into(),
            command: command.into(),
            keybinding: None,
            enabled: true,
            checked: None,
        }
    }

    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        if let Self::Action { ref mut keybinding, .. } = self {
            *keybinding = Some(key.into());
        }
        self
    }

    pub fn disabled(mut self) -> Self {
        if let Self::Action { ref mut enabled, .. } = self {
            *enabled = false;
        }
        self
    }

    pub fn checked(mut self, val: bool) -> Self {
        if let Self::Action { ref mut checked, .. } = self {
            *checked = Some(val);
        }
        self
    }

    pub fn submenu(label: impl Into<String>, items: Vec<MenuBarMenuItem>) -> Self {
        Self::Submenu { label: label.into(), items }
    }

    fn is_separator(&self) -> bool { matches!(self, Self::Separator) }

    fn is_enabled(&self) -> bool {
        match self {
            Self::Action { enabled, .. } => *enabled,
            Self::Submenu { .. } => true,
            Self::Separator => false,
        }
    }
}

// ── Menu ────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct MenuBarMenu {
    pub label: String,
    pub items: Vec<MenuBarMenuItem>,
}

impl MenuBarMenu {
    pub fn new(label: impl Into<String>, items: Vec<MenuBarMenuItem>) -> Self {
        Self { label: label.into(), items }
    }
}

// ── MenuBar ─────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub struct MenuBar<F: FnMut(&str)> {
    pub menus: Vec<MenuBarMenu>,
    pub active_menu: Option<usize>,
    pub visible: bool,
    pub on_command: F,

    font_size: f32,
    row_height: f32,
    dropdown_width: f32,

    hovered_top: Option<usize>,
    hovered_item: Option<usize>,
    keyboard_item: Option<usize>,

    top_bg: Color,
    top_fg: Color,
    top_hover_bg: Color,
    top_active_bg: Color,
    dropdown_bg: Color,
    dropdown_border: Color,
    dropdown_shadow: Color,
    item_hover_bg: Color,
    item_fg: Color,
    item_disabled_fg: Color,
    separator_color: Color,
    shortcut_fg: Color,
    check_color: Color,
}

impl<F: FnMut(&str)> MenuBar<F> {
    pub fn new(on_command: F) -> Self {
        Self {
            menus: default_menus(),
            active_menu: None,
            visible: true,
            on_command,
            font_size: 12.0,
            row_height: 26.0,
            dropdown_width: 240.0,
            hovered_top: None,
            hovered_item: None,
            keyboard_item: None,
            top_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            top_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            top_hover_bg: Color::from_hex("#505050").unwrap_or(Color::BLACK),
            top_active_bg: Color::from_hex("#094771").unwrap_or(Color::BLACK),
            dropdown_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            dropdown_border: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            dropdown_shadow: Color::from_hex("#00000080").unwrap_or(Color::BLACK),
            item_hover_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            item_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            item_disabled_fg: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#454545").unwrap_or(Color::BLACK),
            shortcut_fg: Color::from_hex("#aaaaaa").unwrap_or(Color::WHITE),
            check_color: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
        }
    }

    pub fn close_menu(&mut self) {
        self.active_menu = None;
        self.hovered_item = None;
        self.keyboard_item = None;
    }

    #[allow(clippy::cast_precision_loss)]
    fn top_rects(&self, bar_rect: Rect) -> Vec<Rect> {
        let mut x = bar_rect.x;
        self.menus.iter().map(|m| {
            let w = m.label.len() as f32 * self.font_size * 0.6 + 16.0;
            let r = Rect::new(x, bar_rect.y, w, bar_rect.height);
            x += w;
            r
        }).collect()
    }

    fn dropdown_rect(&self, menu_idx: usize, bar_rect: Rect) -> Rect {
        let tops = self.top_rects(bar_rect);
        let anchor = &tops[menu_idx];
        let items = &self.menus[menu_idx].items;
        let h: f32 = items.iter().map(|i| if i.is_separator() { 9.0 } else { self.row_height }).sum::<f32>() + 4.0;
        Rect::new(anchor.x, anchor.y + anchor.height, self.dropdown_width, h)
    }

    fn item_rect_at(&self, menu_idx: usize, item_idx: usize, bar_rect: Rect) -> Rect {
        let dr = self.dropdown_rect(menu_idx, bar_rect);
        let items = &self.menus[menu_idx].items;
        let mut y = dr.y + 2.0;
        for (i, item) in items.iter().enumerate() {
            let h = if item.is_separator() { 9.0 } else { self.row_height };
            if i == item_idx { return Rect::new(dr.x, y, dr.width, h); }
            y += h;
        }
        Rect::ZERO
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn render_draw(&self, ctx: &mut DrawContext, bar_rect: Rect) {
        if !self.visible { return; }

        let tops = self.top_rects(bar_rect);
        for (i, tr) in tops.iter().enumerate() {
            let bg = if self.active_menu == Some(i) {
                self.top_active_bg
            } else if self.hovered_top == Some(i) {
                self.top_hover_bg
            } else {
                self.top_bg
            };
            ctx.draw_rect(*tr, bg, 0.0);
            let ty = tr.y + (tr.height - self.font_size) / 2.0;
            ctx.draw_text(&self.menus[i].label, (tr.x + 8.0, ty), self.top_fg, self.font_size, false, false);
        }

        if let Some(mi) = self.active_menu {
            let dr = self.dropdown_rect(mi, bar_rect);
            let shadow = Rect::new(dr.x + 2.0, dr.y + 2.0, dr.width, dr.height);
            ctx.draw_rect(shadow, self.dropdown_shadow, 4.0);
            ctx.draw_rect(dr, self.dropdown_bg, 4.0);
            ctx.draw_border(dr, self.dropdown_border, 1.0, 4.0);

            for (ii, item) in self.menus[mi].items.iter().enumerate() {
                let ir = self.item_rect_at(mi, ii, bar_rect);
                if item.is_separator() {
                    let cy = ir.y + ir.height / 2.0;
                    ctx.draw_rect(Rect::new(ir.x + 8.0, cy, ir.width - 16.0, 1.0), self.separator_color, 0.0);
                    continue;
                }
                let is_hover = self.hovered_item == Some(ii) || self.keyboard_item == Some(ii);
                if is_hover && item.is_enabled() {
                    ctx.draw_rect(Rect::new(ir.x + 2.0, ir.y, ir.width - 4.0, ir.height), self.item_hover_bg, 2.0);
                }
                let fg = if item.is_enabled() { self.item_fg } else { self.item_disabled_fg };
                let ty = ir.y + (ir.height - self.font_size) / 2.0;
                match item {
                    MenuBarMenuItem::Action { label, keybinding, checked, .. } => {
                        if matches!(checked, Some(true)) {
                            ctx.draw_icon(IconId::Check, (ir.x + 6.0, ty), self.font_size, self.check_color);
                        }
                        ctx.draw_text(label, (ir.x + 28.0, ty), fg, self.font_size, false, false);
                        if let Some(kb) = keybinding {
                            let kw = kb.len() as f32 * self.font_size * 0.6;
                            ctx.draw_text(kb, (ir.x + ir.width - kw - 12.0, ty), self.shortcut_fg, self.font_size, false, false);
                        }
                    }
                    MenuBarMenuItem::Submenu { label, .. } => {
                        ctx.draw_text(label, (ir.x + 28.0, ty), fg, self.font_size, false, false);
                        ctx.draw_icon(IconId::ChevronRight, (ir.x + ir.width - 16.0, ty), self.font_size, self.item_fg);
                    }
                    MenuBarMenuItem::Separator => {}
                }
            }
        }

        if self.hovered_top.is_some() || self.active_menu.is_some() {
            ctx.set_cursor(CursorIcon::Pointer);
        }
    }
}

impl<F: FnMut(&str)> Widget for MenuBar<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode { size: Size::Fixed(self.row_height), ..LayoutNode::default() }
    }

    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        if !self.visible { return; }
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, self.top_bg, 0.0);
        let _ = renderer;
    }

    fn handle_event(&mut self, event: &UiEvent, bar_rect: Rect) -> EventResult {
        if !self.visible { return EventResult::Ignored; }
        let tops = self.top_rects(bar_rect);
        match event {
            UiEvent::MouseMove { x, y } => {
                self.hovered_top = tops.iter().position(|r| r.contains(*x, *y));
                if self.active_menu.is_some() {
                    if let Some(ht) = self.hovered_top {
                        self.active_menu = Some(ht);
                        self.hovered_item = None;
                        self.keyboard_item = None;
                    }
                }
                if let Some(mi) = self.active_menu {
                    let dr = self.dropdown_rect(mi, bar_rect);
                    if dr.contains(*x, *y) {
                        self.hovered_item = None;
                        for (ii, item) in self.menus[mi].items.iter().enumerate() {
                            let ir = self.item_rect_at(mi, ii, bar_rect);
                            if ir.contains(*x, *y) && !item.is_separator() {
                                self.hovered_item = Some(ii);
                                break;
                            }
                        }
                    }
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown { x, y, button: MouseButton::Left } => {
                if let Some(idx) = tops.iter().position(|r| r.contains(*x, *y)) {
                    if self.active_menu == Some(idx) { self.close_menu(); }
                    else {
                        self.active_menu = Some(idx);
                        self.hovered_item = None;
                        self.keyboard_item = None;
                    }
                    return EventResult::Handled;
                }
                if let Some(mi) = self.active_menu {
                    let dr = self.dropdown_rect(mi, bar_rect);
                    if dr.contains(*x, *y) {
                        if let Some(ii) = self.hovered_item {
                            let item = &self.menus[mi].items[ii];
                            if let MenuBarMenuItem::Action { command, enabled: true, .. } = item {
                                let cmd = command.clone();
                                self.close_menu();
                                (self.on_command)(&cmd);
                            }
                        }
                        return EventResult::Handled;
                    }
                    self.close_menu();
                }
                EventResult::Ignored
            }
            UiEvent::KeyPress { key, .. } if self.active_menu.is_some() => {
                let mi = self.active_menu.unwrap();
                let items = &self.menus[mi].items;
                match key {
                    Key::ArrowDown => {
                        let from = self.keyboard_item.unwrap_or(items.len() - 1);
                        self.keyboard_item = next_enabled(items, from, true);
                        EventResult::Handled
                    }
                    Key::ArrowUp => {
                        let from = self.keyboard_item.unwrap_or(0);
                        self.keyboard_item = next_enabled(items, from, false);
                        EventResult::Handled
                    }
                    Key::ArrowRight => {
                        let next = (mi + 1) % self.menus.len();
                        self.active_menu = Some(next);
                        self.keyboard_item = None;
                        EventResult::Handled
                    }
                    Key::ArrowLeft => {
                        let prev = if mi == 0 { self.menus.len() - 1 } else { mi - 1 };
                        self.active_menu = Some(prev);
                        self.keyboard_item = None;
                        EventResult::Handled
                    }
                    Key::Enter | Key::Space => {
                        if let Some(ii) = self.keyboard_item {
                            if let MenuBarMenuItem::Action { command, enabled: true, .. } = &items[ii] {
                                let cmd = command.clone();
                                self.close_menu();
                                (self.on_command)(&cmd);
                            }
                        }
                        EventResult::Handled
                    }
                    Key::Escape => { self.close_menu(); EventResult::Handled }
                    _ => EventResult::Ignored,
                }
            }
            _ => EventResult::Ignored,
        }
    }
}

fn next_enabled(items: &[MenuBarMenuItem], from: usize, forward: bool) -> Option<usize> {
    let len = items.len();
    for offset in 1..=len {
        let idx = if forward { (from + offset) % len } else { (from + len - offset) % len };
        if items[idx].is_enabled() { return Some(idx); }
    }
    None
}

// ── Default VS Code menus ───────────────────────────────────────────────────

pub fn default_menus() -> Vec<MenuBarMenu> {
    vec![
        file_menu(), edit_menu(), selection_menu(), view_menu(),
        go_menu(), run_menu(), terminal_menu(), help_menu(),
    ]
}

fn cmd(s: &str) -> String { format!("Ctrl+{s}") }
fn cmd_shift(s: &str) -> String { format!("Ctrl+Shift+{s}") }

fn file_menu() -> MenuBarMenu {
    MenuBarMenu::new("File", vec![
        MenuBarMenuItem::action("New File", "workbench.action.files.newUntitledFile").with_key(cmd("N")),
        MenuBarMenuItem::action("New Window", "workbench.action.newWindow").with_key(cmd_shift("N")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Open File...", "workbench.action.files.openFile").with_key(cmd("O")),
        MenuBarMenuItem::action("Open Folder...", "workbench.action.files.openFolder").with_key(cmd("K")),
        MenuBarMenuItem::submenu("Open Recent", vec![
            MenuBarMenuItem::action("Reopen Closed Editor", "workbench.action.reopenClosedEditor").with_key(cmd_shift("T")),
            MenuBarMenuItem::Separator,
            MenuBarMenuItem::action("More...", "workbench.action.openRecent"),
        ]),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Save", "workbench.action.files.save").with_key(cmd("S")),
        MenuBarMenuItem::action("Save As...", "workbench.action.files.saveAs").with_key(cmd_shift("S")),
        MenuBarMenuItem::action("Save All", "workbench.action.files.saveAll"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Auto Save", "workbench.action.toggleAutoSave").checked(false),
        MenuBarMenuItem::submenu("Preferences", vec![
            MenuBarMenuItem::action("Settings", "workbench.action.openSettings").with_key(cmd(",")),
            MenuBarMenuItem::action("Keyboard Shortcuts", "workbench.action.openGlobalKeybindings").with_key(cmd("K")),
            MenuBarMenuItem::action("Extensions", "workbench.view.extensions").with_key(cmd_shift("X")),
        ]),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Close Editor", "workbench.action.closeActiveEditor").with_key(cmd("W")),
        MenuBarMenuItem::action("Close Window", "workbench.action.closeWindow").with_key(cmd_shift("W")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Exit", "workbench.action.quit"),
    ])
}

fn edit_menu() -> MenuBarMenu {
    MenuBarMenu::new("Edit", vec![
        MenuBarMenuItem::action("Undo", "editor.action.undo").with_key(cmd("Z")),
        MenuBarMenuItem::action("Redo", "editor.action.redo").with_key(cmd_shift("Z")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Cut", "editor.action.clipboardCutAction").with_key(cmd("X")),
        MenuBarMenuItem::action("Copy", "editor.action.clipboardCopyAction").with_key(cmd("C")),
        MenuBarMenuItem::action("Paste", "editor.action.clipboardPasteAction").with_key(cmd("V")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Find", "actions.find").with_key(cmd("F")),
        MenuBarMenuItem::action("Replace", "editor.action.startFindReplaceAction").with_key(cmd("H")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Find in Files", "workbench.action.findInFiles").with_key(cmd_shift("F")),
        MenuBarMenuItem::action("Replace in Files", "workbench.action.replaceInFiles").with_key(cmd_shift("H")),
    ])
}

fn selection_menu() -> MenuBarMenu {
    MenuBarMenu::new("Selection", vec![
        MenuBarMenuItem::action("Select All", "editor.action.selectAll").with_key(cmd("A")),
        MenuBarMenuItem::action("Expand Selection", "editor.action.smartSelect.expand").with_key("Shift+Alt+Right"),
        MenuBarMenuItem::action("Shrink Selection", "editor.action.smartSelect.shrink").with_key("Shift+Alt+Left"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Copy Line Up", "editor.action.copyLinesUpAction").with_key("Shift+Alt+Up"),
        MenuBarMenuItem::action("Copy Line Down", "editor.action.copyLinesDownAction").with_key("Shift+Alt+Down"),
        MenuBarMenuItem::action("Move Line Up", "editor.action.moveLinesUpAction").with_key("Alt+Up"),
        MenuBarMenuItem::action("Move Line Down", "editor.action.moveLinesDownAction").with_key("Alt+Down"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Add Cursor Above", "editor.action.insertCursorAbove").with_key(cmd("Alt+Up")),
        MenuBarMenuItem::action("Add Cursor Below", "editor.action.insertCursorBelow").with_key(cmd("Alt+Down")),
    ])
}

fn view_menu() -> MenuBarMenu {
    MenuBarMenu::new("View", vec![
        MenuBarMenuItem::action("Command Palette...", "workbench.action.showCommands").with_key(cmd_shift("P")),
        MenuBarMenuItem::action("Open View...", "workbench.action.openView"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Explorer", "workbench.view.explorer").with_key(cmd_shift("E")),
        MenuBarMenuItem::action("Search", "workbench.view.search").with_key(cmd_shift("F")),
        MenuBarMenuItem::action("Source Control", "workbench.view.scm").with_key(cmd_shift("G")),
        MenuBarMenuItem::action("Run and Debug", "workbench.view.debug").with_key(cmd_shift("D")),
        MenuBarMenuItem::action("Extensions", "workbench.view.extensions").with_key(cmd_shift("X")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Terminal", "workbench.action.terminal.toggleTerminal").with_key(cmd("`")),
        MenuBarMenuItem::action("Output", "workbench.action.output.toggleOutput"),
        MenuBarMenuItem::action("Problems", "workbench.actions.view.problems").with_key(cmd_shift("M")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Toggle Word Wrap", "editor.action.toggleWordWrap").with_key("Alt+Z"),
        MenuBarMenuItem::action("Zoom In", "workbench.action.zoomIn").with_key(cmd("=")),
        MenuBarMenuItem::action("Zoom Out", "workbench.action.zoomOut").with_key(cmd("-")),
        MenuBarMenuItem::action("Toggle Full Screen", "workbench.action.toggleFullScreen").with_key("F11"),
    ])
}

fn go_menu() -> MenuBarMenu {
    MenuBarMenu::new("Go", vec![
        MenuBarMenuItem::action("Go to File...", "workbench.action.quickOpen").with_key(cmd("P")),
        MenuBarMenuItem::action("Go to Symbol in Workspace...", "workbench.action.showAllSymbols").with_key(cmd("T")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Go to Symbol in Editor...", "workbench.action.gotoSymbol").with_key(cmd_shift("O")),
        MenuBarMenuItem::action("Go to Line/Column...", "workbench.action.gotoLine").with_key(cmd("G")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Go to Definition", "editor.action.revealDefinition").with_key("F12"),
        MenuBarMenuItem::action("Go to Type Definition", "editor.action.goToTypeDefinition"),
        MenuBarMenuItem::action("Go to References", "editor.action.goToReferences").with_key("Shift+F12"),
        MenuBarMenuItem::action("Go to Implementation", "editor.action.goToImplementation").with_key(cmd("F12")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Go Back", "workbench.action.navigateBack").with_key("Alt+Left"),
        MenuBarMenuItem::action("Go Forward", "workbench.action.navigateForward").with_key("Alt+Right"),
    ])
}

fn run_menu() -> MenuBarMenu {
    MenuBarMenu::new("Run", vec![
        MenuBarMenuItem::action("Start Debugging", "workbench.action.debug.start").with_key("F5"),
        MenuBarMenuItem::action("Start Without Debugging", "workbench.action.debug.run").with_key(cmd("F5")),
        MenuBarMenuItem::action("Stop Debugging", "workbench.action.debug.stop").with_key("Shift+F5"),
        MenuBarMenuItem::action("Restart Debugging", "workbench.action.debug.restart").with_key(cmd_shift("F5")),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Step Over", "workbench.action.debug.stepOver").with_key("F10"),
        MenuBarMenuItem::action("Step Into", "workbench.action.debug.stepInto").with_key("F11"),
        MenuBarMenuItem::action("Step Out", "workbench.action.debug.stepOut").with_key("Shift+F11"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Toggle Breakpoint", "editor.debug.action.toggleBreakpoint").with_key("F9"),
        MenuBarMenuItem::action("Add Configuration...", "debug.addConfiguration"),
    ])
}

fn terminal_menu() -> MenuBarMenu {
    MenuBarMenu::new("Terminal", vec![
        MenuBarMenuItem::action("New Terminal", "workbench.action.terminal.new").with_key(cmd_shift("`")),
        MenuBarMenuItem::action("Split Terminal", "workbench.action.terminal.split"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Run Task...", "workbench.action.tasks.runTask"),
        MenuBarMenuItem::action("Run Build Task", "workbench.action.tasks.build").with_key(cmd_shift("B")),
        MenuBarMenuItem::action("Run Active File", "workbench.action.tasks.runActiveFile"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Configure Tasks...", "workbench.action.tasks.configureTaskRunner"),
        MenuBarMenuItem::action("Configure Default Build Task...", "workbench.action.tasks.configureDefaultBuildTask"),
    ])
}

fn help_menu() -> MenuBarMenu {
    MenuBarMenu::new("Help", vec![
        MenuBarMenuItem::action("Welcome", "workbench.action.showWelcomePage"),
        MenuBarMenuItem::action("Documentation", "workbench.action.openDocumentationUrl"),
        MenuBarMenuItem::action("Release Notes", "update.showCurrentReleaseNotes"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("Report Issue", "workbench.action.openIssueReporter"),
        MenuBarMenuItem::Separator,
        MenuBarMenuItem::action("About", "workbench.action.showAboutDialog"),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop(_: &str) {}

    #[test]
    fn default_menus_count() {
        let menus = default_menus();
        assert_eq!(menus.len(), 8);
        assert_eq!(menus[0].label, "File");
        assert_eq!(menus[7].label, "Help");
    }

    #[test]
    fn file_menu_has_items() {
        let fm = file_menu();
        assert!(fm.items.len() > 10);
    }

    #[test]
    fn toggle_active_menu() {
        let mut mb = MenuBar::new(noop);
        assert!(mb.active_menu.is_none());
        mb.active_menu = Some(0);
        mb.close_menu();
        assert!(mb.active_menu.is_none());
    }

    #[test]
    fn keyboard_navigation() {
        let items = vec![
            MenuBarMenuItem::Separator,
            MenuBarMenuItem::action("A", "a"),
            MenuBarMenuItem::action("B", "b"),
        ];
        assert_eq!(next_enabled(&items, 0, true), Some(1));
        assert_eq!(next_enabled(&items, 1, true), Some(2));
        assert_eq!(next_enabled(&items, 2, true), Some(1));
    }
}
