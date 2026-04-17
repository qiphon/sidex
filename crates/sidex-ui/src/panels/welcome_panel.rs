//! Welcome panel — getting started walkthrough, recent files, and shortcuts.

use std::path::PathBuf;

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Recent item ──────────────────────────────────────────────────────────────

/// A recently opened file or folder.
#[derive(Clone, Debug)]
pub struct RecentItem {
    pub name: String,
    pub path: PathBuf,
    pub is_folder: bool,
    pub last_opened: Option<String>,
}

impl RecentItem {
    pub fn file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            is_folder: false,
            last_opened: None,
        }
    }

    pub fn folder(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            is_folder: true,
            last_opened: None,
        }
    }
}

// ── Walkthrough ──────────────────────────────────────────────────────────────

/// A step in the getting-started walkthrough.
#[derive(Clone, Debug)]
pub struct WalkthroughStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub completed: bool,
    pub action_label: Option<String>,
}

impl WalkthroughStep {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            completed: false,
            action_label: None,
        }
    }

    pub fn with_action(mut self, label: impl Into<String>) -> Self {
        self.action_label = Some(label.into());
        self
    }

    pub fn completed(mut self) -> Self {
        self.completed = true;
        self
    }
}

/// A getting-started walkthrough category.
#[derive(Clone, Debug)]
pub struct Walkthrough {
    pub id: String,
    pub title: String,
    pub description: String,
    pub steps: Vec<WalkthroughStep>,
}

impl Walkthrough {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        steps: Vec<WalkthroughStep>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            steps,
        }
    }

    pub fn progress(&self) -> (usize, usize) {
        let done = self.steps.iter().filter(|s| s.completed).count();
        (done, self.steps.len())
    }
}

// ── Keyboard shortcut ────────────────────────────────────────────────────────

/// A keyboard shortcut reference entry.
#[derive(Clone, Debug)]
pub struct ShortcutEntry {
    pub label: String,
    pub keys: String,
    pub category: String,
}

impl ShortcutEntry {
    pub fn new(
        label: impl Into<String>,
        keys: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            keys: keys.into(),
            category: category.into(),
        }
    }
}

// ── Welcome actions ──────────────────────────────────────────────────────────

/// Actions from the welcome panel.
#[derive(Clone, Debug)]
pub enum WelcomeAction {
    OpenRecent(PathBuf),
    OpenFile,
    OpenFolder,
    CloneRepository,
    NewFile,
    RunWalkthroughStep(String, String),
    ShowShortcuts,
    ShowSettings,
    ShowExtensions,
    DismissWelcome,
}

// ── Welcome section ──────────────────────────────────────────────────────────

/// Which section of the welcome page is focused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
enum WelcomeSection {
    #[default]
    Start,
    Recent,
    Walkthrough,
    Shortcuts,
}

// ── Welcome panel ────────────────────────────────────────────────────────────

/// The Welcome tab / getting-started page.
///
/// Shows a getting-started walkthrough, recent files/folders,
/// quick-start actions, and a keyboard shortcuts reference.
#[allow(dead_code)]
pub struct WelcomePanel<OnAction>
where
    OnAction: FnMut(WelcomeAction),
{
    pub recent_items: Vec<RecentItem>,
    pub walkthroughs: Vec<Walkthrough>,
    pub shortcuts: Vec<ShortcutEntry>,
    pub on_action: OnAction,
    pub show_on_startup: bool,

    active_section: WelcomeSection,
    hovered_recent: Option<usize>,
    hovered_action: Option<usize>,
    scroll_offset: f32,
    focused: bool,

    max_content_width: f32,
    section_spacing: f32,
    recent_row_height: f32,
    action_button_height: f32,
    walkthrough_step_height: f32,
    shortcut_row_height: f32,

    background: Color,
    card_bg: Color,
    card_border: Color,
    action_button_bg: Color,
    action_button_hover: Color,
    recent_hover_bg: Color,
    link_color: Color,
    progress_bg: Color,
    progress_fill: Color,
    completed_check: Color,
    shortcut_key_bg: Color,
    separator_color: Color,
    title_fg: Color,
    foreground: Color,
    secondary_fg: Color,
}

impl<OnAction> WelcomePanel<OnAction>
where
    OnAction: FnMut(WelcomeAction),
{
    pub fn new(on_action: OnAction) -> Self {
        Self {
            recent_items: Vec::new(),
            walkthroughs: Vec::new(),
            shortcuts: Self::default_shortcuts(),
            on_action,
            show_on_startup: true,

            active_section: WelcomeSection::Start,
            hovered_recent: None,
            hovered_action: None,
            scroll_offset: 0.0,
            focused: false,

            max_content_width: 700.0,
            section_spacing: 32.0,
            recent_row_height: 28.0,
            action_button_height: 32.0,
            walkthrough_step_height: 48.0,
            shortcut_row_height: 28.0,

            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            card_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            card_border: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            action_button_bg: Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            action_button_hover: Color::from_hex("#1177bb").unwrap_or(Color::BLACK),
            recent_hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            link_color: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
            progress_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            progress_fill: Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            completed_check: Color::from_rgb(81, 154, 81),
            shortcut_key_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            title_fg: Color::WHITE,
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
        }
    }

    pub fn set_recent(&mut self, items: Vec<RecentItem>) {
        self.recent_items = items;
    }

    pub fn set_walkthroughs(&mut self, walkthroughs: Vec<Walkthrough>) {
        self.walkthroughs = walkthroughs;
    }

    fn default_shortcuts() -> Vec<ShortcutEntry> {
        vec![
            ShortcutEntry::new("Show Command Palette", "Ctrl+Shift+P", "General"),
            ShortcutEntry::new("Quick Open File", "Ctrl+P", "General"),
            ShortcutEntry::new("Toggle Terminal", "Ctrl+`", "General"),
            ShortcutEntry::new("Toggle Sidebar", "Ctrl+B", "General"),
            ShortcutEntry::new("Find in Files", "Ctrl+Shift+F", "Search"),
            ShortcutEntry::new("Go to Definition", "F12", "Editor"),
            ShortcutEntry::new("Peek Definition", "Alt+F12", "Editor"),
            ShortcutEntry::new("Find References", "Shift+F12", "Editor"),
            ShortcutEntry::new("Rename Symbol", "F2", "Editor"),
            ShortcutEntry::new("Format Document", "Shift+Alt+F", "Editor"),
        ]
    }

    fn quick_actions() -> &'static [(&'static str, &'static str)] {
        &[
            ("New File...", "new_file"),
            ("Open File...", "open_file"),
            ("Open Folder...", "open_folder"),
            ("Clone Repository...", "clone_repo"),
        ]
    }

    fn content_rect(&self, rect: Rect) -> Rect {
        let w = rect.width.min(self.max_content_width);
        let x = rect.x + (rect.width - w) / 2.0;
        Rect::new(x, rect.y, w, rect.height)
    }
}

impl<OnAction> Widget for WelcomePanel<OnAction>
where
    OnAction: FnMut(WelcomeAction),
{
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Flex(1.0),
            ..LayoutNode::default()
        }
    }

    #[allow(clippy::cast_precision_loss)]
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

        let cr = self.content_rect(rect);
        let mut y = cr.y + 40.0 - self.scroll_offset;

        // Title area placeholder
        y += 60.0;

        // Quick action buttons
        let actions = Self::quick_actions();
        let btn_w = (cr.width - 12.0 * (actions.len() as f32 - 1.0)) / actions.len() as f32;
        for (i, _action) in actions.iter().enumerate() {
            let bx = cr.x + i as f32 * (btn_w + 12.0);
            let is_hover = self.hovered_action == Some(i);
            let bg = if is_hover {
                self.action_button_hover
            } else {
                self.action_button_bg
            };
            rr.draw_rect(bx, y, btn_w, self.action_button_height, bg, 4.0);
        }
        y += self.action_button_height + self.section_spacing;

        // Recent files section
        rr.draw_rect(cr.x, y, cr.width, 1.0, self.separator_color, 0.0);
        y += 12.0;
        y += 24.0; // Section title

        let max_recent = self.recent_items.len().min(8);
        for (i, _item) in self.recent_items.iter().take(max_recent).enumerate() {
            let ry = y + i as f32 * self.recent_row_height;
            let is_hover = self.hovered_recent == Some(i);
            if is_hover {
                rr.draw_rect(
                    cr.x,
                    ry,
                    cr.width,
                    self.recent_row_height,
                    self.recent_hover_bg,
                    2.0,
                );
            }
        }
        y += max_recent as f32 * self.recent_row_height + self.section_spacing;

        // Walkthrough section
        for wt in &self.walkthroughs {
            rr.draw_rect(cr.x, y, cr.width, 1.0, self.separator_color, 0.0);
            y += 12.0;
            y += 24.0; // Walkthrough title

            // Progress bar
            let (done, total) = wt.progress();
            let bar_h = 4.0;
            rr.draw_rect(cr.x, y, cr.width, bar_h, self.progress_bg, 2.0);
            if total > 0 {
                let pct = done as f32 / total as f32;
                rr.draw_rect(cr.x, y, cr.width * pct, bar_h, self.progress_fill, 2.0);
            }
            y += bar_h + 12.0;

            // Steps
            for step in &wt.steps {
                rr.draw_rect(
                    cr.x,
                    y,
                    cr.width,
                    self.walkthrough_step_height,
                    self.card_bg,
                    0.0,
                );

                // Completed checkmark
                if step.completed {
                    let check_s = 16.0;
                    rr.draw_rect(
                        cr.x + 8.0,
                        y + (self.walkthrough_step_height - check_s) / 2.0,
                        check_s,
                        check_s,
                        self.completed_check,
                        check_s / 2.0,
                    );
                }

                // Action button
                if let Some(ref _label) = step.action_label {
                    let btn_w2 = 80.0;
                    let btn_h = 22.0;
                    rr.draw_rect(
                        cr.x + cr.width - btn_w2 - 8.0,
                        y + (self.walkthrough_step_height - btn_h) / 2.0,
                        btn_w2,
                        btn_h,
                        self.action_button_bg,
                        3.0,
                    );
                }

                rr.draw_rect(
                    cr.x + 8.0,
                    y + self.walkthrough_step_height - 1.0,
                    cr.width - 16.0,
                    1.0,
                    self.separator_color,
                    0.0,
                );
                y += self.walkthrough_step_height;
            }
            y += self.section_spacing;
        }

        // Keyboard shortcuts section
        rr.draw_rect(cr.x, y, cr.width, 1.0, self.separator_color, 0.0);
        y += 12.0;
        y += 24.0; // Section title

        for shortcut in &self.shortcuts {
            // Key badge
            let key_w = 120.0;
            rr.draw_rect(
                cr.x + cr.width - key_w - 8.0,
                y + 4.0,
                key_w,
                self.shortcut_row_height - 8.0,
                self.shortcut_key_bg,
                3.0,
            );
            let _ = shortcut;
            y += self.shortcut_row_height;
        }

        let _ = renderer;
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        let cr = self.content_rect(rect);

        match event {
            UiEvent::Focus => {
                self.focused = true;
                EventResult::Handled
            }
            UiEvent::Blur => {
                self.focused = false;
                self.hovered_recent = None;
                self.hovered_action = None;
                EventResult::Handled
            }
            UiEvent::MouseMove { x, y } => {
                if !rect.contains(*x, *y) {
                    self.hovered_recent = None;
                    self.hovered_action = None;
                    return EventResult::Ignored;
                }

                // Action buttons hover
                let actions = Self::quick_actions();
                let actions_y = cr.y + 40.0 - self.scroll_offset + 60.0;
                if *y >= actions_y && *y < actions_y + self.action_button_height {
                    let btn_w =
                        (cr.width - 12.0 * (actions.len() as f32 - 1.0)) / actions.len() as f32;
                    for (i, _) in actions.iter().enumerate() {
                        let bx = cr.x + i as f32 * (btn_w + 12.0);
                        if *x >= bx && *x < bx + btn_w {
                            self.hovered_action = Some(i);
                            return EventResult::Handled;
                        }
                    }
                }
                self.hovered_action = None;

                // Recent items hover
                let recent_top =
                    actions_y + self.action_button_height + self.section_spacing + 12.0 + 24.0;
                let max_recent = self.recent_items.len().min(8);
                if *y >= recent_top && *y < recent_top + max_recent as f32 * self.recent_row_height
                {
                    let idx = ((*y - recent_top) / self.recent_row_height) as usize;
                    if idx < max_recent {
                        self.hovered_recent = Some(idx);
                        return EventResult::Handled;
                    }
                }
                self.hovered_recent = None;
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.focused = true;

                // Action buttons
                let actions_y = cr.y + 40.0 - self.scroll_offset + 60.0;
                if *y >= actions_y && *y < actions_y + self.action_button_height {
                    if let Some(idx) = self.hovered_action {
                        let action = match idx {
                            0 => WelcomeAction::NewFile,
                            1 => WelcomeAction::OpenFile,
                            2 => WelcomeAction::OpenFolder,
                            3 => WelcomeAction::CloneRepository,
                            _ => return EventResult::Handled,
                        };
                        (self.on_action)(action);
                        return EventResult::Handled;
                    }
                }

                // Recent items
                if let Some(idx) = self.hovered_recent {
                    if let Some(item) = self.recent_items.get(idx) {
                        let path = item.path.clone();
                        (self.on_action)(WelcomeAction::OpenRecent(path));
                        return EventResult::Handled;
                    }
                }

                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                self.scroll_offset = (self.scroll_offset - dy * 40.0).max(0.0);
                EventResult::Handled
            }
            UiEvent::KeyPress {
                key: Key::Escape, ..
            } => {
                (self.on_action)(WelcomeAction::DismissWelcome);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
