//! Welcome page — first-launch tab with start actions, recent items,
//! walkthroughs, and help links.

use std::path::PathBuf;

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Data types ───────────────────────────────────────────────────────────────

/// A recent file or workspace.
#[derive(Clone, Debug)]
pub struct RecentItem {
    pub name: String,
    pub path: PathBuf,
    pub is_folder: bool,
    pub last_opened: Option<String>,
}

impl RecentItem {
    pub fn file(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self { name: name.into(), path: path.into(), is_folder: false, last_opened: None }
    }
    pub fn folder(name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self { name: name.into(), path: path.into(), is_folder: true, last_opened: None }
    }
    pub fn with_time(mut self, time: impl Into<String>) -> Self {
        self.last_opened = Some(time.into());
        self
    }
}

/// A single item in a welcome section (Start, Help, etc.).
#[derive(Clone, Debug)]
pub struct WelcomeItem {
    pub label: String,
    pub description: String,
    pub command: String,
    pub icon: Option<String>,
    pub keybinding: Option<String>,
}

impl WelcomeItem {
    pub fn new(label: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: String::new(),
            command: command.into(),
            icon: None,
            keybinding: None,
        }
    }
    pub fn with_desc(mut self, d: impl Into<String>) -> Self { self.description = d.into(); self }
    pub fn with_icon(mut self, i: impl Into<String>) -> Self { self.icon = Some(i.into()); self }
    pub fn with_key(mut self, k: impl Into<String>) -> Self { self.keybinding = Some(k.into()); self }
}

/// A named section on the welcome page.
#[derive(Clone, Debug)]
pub struct WelcomeSection {
    pub title: String,
    pub items: Vec<WelcomeItem>,
}

impl WelcomeSection {
    pub fn new(title: impl Into<String>, items: Vec<WelcomeItem>) -> Self {
        Self { title: title.into(), items }
    }
}

/// A walkthrough step with completion tracking.
#[derive(Clone, Debug)]
pub struct WalkthroughStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub completed: bool,
    pub action_label: Option<String>,
}

impl WalkthroughStep {
    pub fn new(id: impl Into<String>, title: impl Into<String>, desc: impl Into<String>) -> Self {
        Self { id: id.into(), title: title.into(), description: desc.into(), completed: false, action_label: None }
    }
    pub fn with_action(mut self, label: impl Into<String>) -> Self { self.action_label = Some(label.into()); self }
    pub fn completed(mut self) -> Self { self.completed = true; self }
}

/// A walkthrough category with progress tracking.
#[derive(Clone, Debug)]
pub struct WalkthroughCategory {
    pub id: String,
    pub title: String,
    pub description: String,
    pub steps: Vec<WalkthroughStep>,
}

impl WalkthroughCategory {
    pub fn new(id: impl Into<String>, title: impl Into<String>, desc: impl Into<String>, steps: Vec<WalkthroughStep>) -> Self {
        Self { id: id.into(), title: title.into(), description: desc.into(), steps }
    }
    pub fn progress(&self) -> (usize, usize) {
        (self.steps.iter().filter(|s| s.completed).count(), self.steps.len())
    }
}

// ── Actions ──────────────────────────────────────────────────────────────────

/// User actions from the welcome page.
#[derive(Clone, Debug)]
pub enum WelcomePageAction {
    NewFile,
    OpenFile,
    OpenFolder,
    CloneRepository,
    OpenRecent(PathBuf),
    RunCommand(String),
    RunWalkthroughStep(String, String),
    ToggleShowOnStartup(bool),
    Dismiss,
}

// ── Welcome page ─────────────────────────────────────────────────────────────

/// The Welcome tab shown on first launch.
#[allow(dead_code)]
pub struct WelcomePage<OnAction>
where
    OnAction: FnMut(WelcomePageAction),
{
    pub show_on_startup: bool,
    pub recent_items: Vec<RecentItem>,
    pub sections: Vec<WelcomeSection>,
    pub walkthroughs: Vec<WalkthroughCategory>,
    pub on_action: OnAction,

    hovered_action: Option<usize>,
    hovered_recent: Option<usize>,
    hovered_section_item: Option<(usize, usize)>,
    scroll_offset: f32,
    focused: bool,

    max_content_width: f32,
    section_spacing: f32,
    action_button_height: f32,
    recent_row_height: f32,
    item_row_height: f32,
    walkthrough_step_height: f32,

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
    keybinding_badge_bg: Color,
    separator_color: Color,
    checkbox_bg: Color,
    checkbox_checked_bg: Color,
    title_fg: Color,
    foreground: Color,
    secondary_fg: Color,
}

impl<OnAction> WelcomePage<OnAction>
where
    OnAction: FnMut(WelcomePageAction),
{
    pub fn new(on_action: OnAction) -> Self {
        Self {
            show_on_startup: true,
            recent_items: Vec::new(),
            sections: Self::default_sections(),
            walkthroughs: Vec::new(),
            on_action,

            hovered_action: None,
            hovered_recent: None,
            hovered_section_item: None,
            scroll_offset: 0.0,
            focused: false,

            max_content_width: 700.0,
            section_spacing: 32.0,
            action_button_height: 32.0,
            recent_row_height: 28.0,
            item_row_height: 28.0,
            walkthrough_step_height: 48.0,

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
            keybinding_badge_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            checkbox_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            checkbox_checked_bg: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            title_fg: Color::WHITE,
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
        }
    }

    pub fn set_recent(&mut self, items: Vec<RecentItem>) { self.recent_items = items; }
    pub fn set_walkthroughs(&mut self, wts: Vec<WalkthroughCategory>) { self.walkthroughs = wts; }

    fn default_sections() -> Vec<WelcomeSection> {
        vec![
            WelcomeSection::new("Start", vec![
                WelcomeItem::new("New File...", "workbench.action.files.newUntitledFile").with_icon("new-file"),
                WelcomeItem::new("Open File...", "workbench.action.files.openFile").with_icon("folder-opened"),
                WelcomeItem::new("Open Folder...", "workbench.action.files.openFolder").with_icon("folder"),
                WelcomeItem::new("Clone Repository...", "git.clone").with_icon("source-control"),
            ]),
            WelcomeSection::new("Walkthroughs", vec![
                WelcomeItem::new("Get Started with SideX", "sidex.walkthrough.getStarted").with_desc("Learn the basics of SideX"),
                WelcomeItem::new("Learn Keyboard Shortcuts", "sidex.walkthrough.shortcuts").with_desc("Master productivity shortcuts"),
            ]),
            WelcomeSection::new("Help", vec![
                WelcomeItem::new("Documentation", "sidex.openDocs").with_icon("book"),
                WelcomeItem::new("Release Notes", "sidex.openReleaseNotes").with_icon("info"),
                WelcomeItem::new("Report Issue", "sidex.openIssueReporter").with_icon("issues"),
            ]),
        ]
    }

    fn start_actions() -> &'static [(&'static str, &'static str)] {
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

impl<OnAction> Widget for WelcomePage<OnAction>
where
    OnAction: FnMut(WelcomePageAction),
{
    fn layout(&self) -> LayoutNode {
        LayoutNode { size: Size::Flex(1.0), ..LayoutNode::default() }
    }

    #[allow(clippy::cast_precision_loss)]
    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, self.background, 0.0);

        let cr = self.content_rect(rect);
        let mut y = cr.y + 40.0 - self.scroll_offset;

        // Title placeholder
        y += 60.0;

        // Start action buttons
        let actions = Self::start_actions();
        let btn_w = (cr.width - 12.0 * (actions.len() as f32 - 1.0)) / actions.len() as f32;
        for (i, _) in actions.iter().enumerate() {
            let bx = cr.x + i as f32 * (btn_w + 12.0);
            let bg = if self.hovered_action == Some(i) { self.action_button_hover } else { self.action_button_bg };
            rr.draw_rect(bx, y, btn_w, self.action_button_height, bg, 4.0);
        }
        y += self.action_button_height + self.section_spacing;

        // Recent section
        if !self.recent_items.is_empty() {
            rr.draw_rect(cr.x, y, cr.width, 1.0, self.separator_color, 0.0);
            y += 12.0;
            y += 24.0;
            let max_recent = self.recent_items.len().min(8);
            for (i, _item) in self.recent_items.iter().take(max_recent).enumerate() {
                let ry = y + i as f32 * self.recent_row_height;
                if self.hovered_recent == Some(i) {
                    rr.draw_rect(cr.x, ry, cr.width, self.recent_row_height, self.recent_hover_bg, 2.0);
                }
                // Folder/file icon area
                rr.draw_rect(cr.x + 4.0, ry + 4.0, 20.0, 20.0, self.card_bg, 2.0);
            }
            y += max_recent as f32 * self.recent_row_height + self.section_spacing;
        }

        // Walkthroughs
        for wt in &self.walkthroughs {
            rr.draw_rect(cr.x, y, cr.width, 1.0, self.separator_color, 0.0);
            y += 12.0;
            y += 24.0;
            let (done, total) = wt.progress();
            let bar_h = 4.0;
            rr.draw_rect(cr.x, y, cr.width, bar_h, self.progress_bg, 2.0);
            if total > 0 {
                let pct = done as f32 / total as f32;
                rr.draw_rect(cr.x, y, cr.width * pct, bar_h, self.progress_fill, 2.0);
            }
            y += bar_h + 12.0;
            for step in &wt.steps {
                rr.draw_rect(cr.x, y, cr.width, self.walkthrough_step_height, self.card_bg, 0.0);
                if step.completed {
                    let cs = 16.0;
                    rr.draw_rect(cr.x + 8.0, y + (self.walkthrough_step_height - cs) / 2.0, cs, cs, self.completed_check, cs / 2.0);
                }
                if let Some(ref _label) = step.action_label {
                    rr.draw_rect(cr.x + cr.width - 88.0, y + (self.walkthrough_step_height - 22.0) / 2.0, 80.0, 22.0, self.action_button_bg, 3.0);
                }
                rr.draw_rect(cr.x + 8.0, y + self.walkthrough_step_height - 1.0, cr.width - 16.0, 1.0, self.separator_color, 0.0);
                y += self.walkthrough_step_height;
            }
            y += self.section_spacing;
        }

        // Extra sections (Help, etc.)
        for (si, section) in self.sections.iter().enumerate() {
            if si == 0 { continue; } // Start section rendered as buttons above
            rr.draw_rect(cr.x, y, cr.width, 1.0, self.separator_color, 0.0);
            y += 12.0;
            y += 24.0;
            for (ii, item) in section.items.iter().enumerate() {
                let iy = y + ii as f32 * self.item_row_height;
                if self.hovered_section_item == Some((si, ii)) {
                    rr.draw_rect(cr.x, iy, cr.width, self.item_row_height, self.recent_hover_bg, 2.0);
                }
                if let Some(ref _kb) = item.keybinding {
                    rr.draw_rect(cr.x + cr.width - 100.0, iy + 4.0, 92.0, self.item_row_height - 8.0, self.keybinding_badge_bg, 3.0);
                }
            }
            y += section.items.len() as f32 * self.item_row_height + self.section_spacing;
        }

        // "Show welcome page on startup" checkbox
        let cb_y = y + 8.0;
        let cb_bg = if self.show_on_startup { self.checkbox_checked_bg } else { self.checkbox_bg };
        rr.draw_rect(cr.x, cb_y, 16.0, 16.0, cb_bg, 2.0);

        let _ = renderer;
    }

    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        let cr = self.content_rect(rect);
        match event {
            UiEvent::Focus => { self.focused = true; EventResult::Handled }
            UiEvent::Blur => {
                self.focused = false;
                self.hovered_recent = None;
                self.hovered_action = None;
                self.hovered_section_item = None;
                EventResult::Handled
            }
            UiEvent::MouseMove { x, y } => {
                if !rect.contains(*x, *y) {
                    self.hovered_recent = None;
                    self.hovered_action = None;
                    return EventResult::Ignored;
                }
                let actions_y = cr.y + 40.0 - self.scroll_offset + 60.0;
                if *y >= actions_y && *y < actions_y + self.action_button_height {
                    let actions = Self::start_actions();
                    let btn_w = (cr.width - 12.0 * (actions.len() as f32 - 1.0)) / actions.len() as f32;
                    for (i, _) in actions.iter().enumerate() {
                        let bx = cr.x + i as f32 * (btn_w + 12.0);
                        if *x >= bx && *x < bx + btn_w {
                            self.hovered_action = Some(i);
                            return EventResult::Handled;
                        }
                    }
                }
                self.hovered_action = None;

                let recent_top = actions_y + self.action_button_height + self.section_spacing + 12.0 + 24.0;
                let max_recent = self.recent_items.len().min(8);
                if *y >= recent_top && *y < recent_top + max_recent as f32 * self.recent_row_height {
                    let idx = ((*y - recent_top) / self.recent_row_height) as usize;
                    if idx < max_recent { self.hovered_recent = Some(idx); return EventResult::Handled; }
                }
                self.hovered_recent = None;
                EventResult::Ignored
            }
            UiEvent::MouseDown { x, y, button: MouseButton::Left } if rect.contains(*x, *y) => {
                self.focused = true;
                let actions_y = cr.y + 40.0 - self.scroll_offset + 60.0;
                if *y >= actions_y && *y < actions_y + self.action_button_height {
                    if let Some(idx) = self.hovered_action {
                        let action = match idx {
                            0 => WelcomePageAction::NewFile,
                            1 => WelcomePageAction::OpenFile,
                            2 => WelcomePageAction::OpenFolder,
                            3 => WelcomePageAction::CloneRepository,
                            _ => return EventResult::Handled,
                        };
                        (self.on_action)(action);
                        return EventResult::Handled;
                    }
                }
                if let Some(idx) = self.hovered_recent {
                    if let Some(item) = self.recent_items.get(idx) {
                        let p = item.path.clone();
                        (self.on_action)(WelcomePageAction::OpenRecent(p));
                        return EventResult::Handled;
                    }
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                self.scroll_offset = (self.scroll_offset - dy * 40.0).max(0.0);
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Escape, .. } => {
                (self.on_action)(WelcomePageAction::Dismiss);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
