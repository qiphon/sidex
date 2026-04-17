//! Keyboard shortcuts editor — Ctrl+K Ctrl+S style.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Source filter ────────────────────────────────────────────────────────────

/// Filter keybindings by their origin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeybindingSourceFilter {
    #[default]
    All,
    Default,
    User,
    Extension,
}

// ── Sort column ──────────────────────────────────────────────────────────────

/// Which column to sort the table by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortColumn {
    #[default]
    Command,
    Keybinding,
    Source,
    When,
}

/// Sort direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

// ── Display entry ────────────────────────────────────────────────────────────

/// A single row in the keyboard shortcuts table.
#[derive(Clone, Debug)]
pub struct KeybindingDisplayEntry {
    pub command: String,
    pub command_title: String,
    pub keybinding_label: Option<String>,
    pub when_clause: Option<String>,
    pub source: String,
    pub is_user_modified: bool,
}

impl KeybindingDisplayEntry {
    pub fn new(
        command: impl Into<String>,
        title: impl Into<String>,
        keybinding: Option<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            command: command.into(),
            command_title: title.into(),
            keybinding_label: keybinding,
            when_clause: None,
            source: source.into(),
            is_user_modified: false,
        }
    }

    pub fn with_when(mut self, when: impl Into<String>) -> Self {
        self.when_clause = Some(when.into());
        self
    }

    pub fn user_modified(mut self) -> Self {
        self.is_user_modified = true;
        self
    }
}

// ── Context menu action ──────────────────────────────────────────────────────

/// Right-click actions on a keybinding row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeybindingContextAction {
    ChangeKeybinding(String),
    RemoveKeybinding(String),
    ResetKeybinding(String),
    CopyCommandId(String),
    CopyCommandTitle(String),
    ShowWhenClause(String),
}

// ── Editor events ────────────────────────────────────────────────────────────

/// Events emitted by the keybindings editor.
#[derive(Clone, Debug)]
pub enum KeybindingsEvent {
    RecordKeybinding { command: String, keys: String },
    RemoveKeybinding { command: String },
    ResetKeybinding { command: String },
    DefineKeybinding,
    OpenKeybindingsJson,
}

// ── Keybindings editor ───────────────────────────────────────────────────────

/// The Keyboard Shortcuts editor panel (Ctrl+K Ctrl+S).
#[allow(dead_code)]
pub struct KeybindingsEditor<OnEvent>
where
    OnEvent: FnMut(KeybindingsEvent),
{
    pub entries: Vec<KeybindingDisplayEntry>,
    pub search_query: String,
    pub recording_keybinding: bool,
    pub recorded_keys: Option<String>,
    pub source_filter: KeybindingSourceFilter,
    pub on_event: OnEvent,

    sort_column: SortColumn,
    sort_direction: SortDirection,
    editing_index: Option<usize>,
    selected_index: Option<usize>,
    context_menu_index: Option<usize>,
    scroll_offset: f32,
    focused: bool,
    search_focused: bool,

    search_bar_height: f32,
    header_height: f32,
    row_height: f32,
    col_command_w: f32,
    col_keybinding_w: f32,
    col_when_w: f32,

    background: Color,
    search_bg: Color,
    search_border: Color,
    search_border_focused: Color,
    recording_border: Color,
    header_bg: Color,
    row_hover_bg: Color,
    row_selected_bg: Color,
    user_modified_fg: Color,
    separator_color: Color,
    edit_button_bg: Color,
    define_button_bg: Color,
    foreground: Color,
    secondary_fg: Color,
    keybinding_badge_bg: Color,
}

impl<OnEvent> KeybindingsEditor<OnEvent>
where
    OnEvent: FnMut(KeybindingsEvent),
{
    pub fn new(on_event: OnEvent) -> Self {
        Self {
            entries: Vec::new(),
            search_query: String::new(),
            recording_keybinding: false,
            recorded_keys: None,
            source_filter: KeybindingSourceFilter::All,
            on_event,

            sort_column: SortColumn::Command,
            sort_direction: SortDirection::Ascending,
            editing_index: None,
            selected_index: None,
            context_menu_index: None,
            scroll_offset: 0.0,
            focused: false,
            search_focused: false,

            search_bar_height: 32.0,
            header_height: 28.0,
            row_height: 24.0,
            col_command_w: 0.35,
            col_keybinding_w: 0.25,
            col_when_w: 0.25,

            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            search_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            search_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            search_border_focused: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            recording_border: Color::from_hex("#b5200d").unwrap_or(Color::WHITE),
            header_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            row_hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            row_selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            user_modified_fg: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            edit_button_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            define_button_bg: Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
            keybinding_badge_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
        }
    }

    pub fn set_entries(&mut self, entries: Vec<KeybindingDisplayEntry>) {
        self.entries = entries;
        self.apply_sort();
    }

    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.scroll_offset = 0.0;
    }

    pub fn set_source_filter(&mut self, filter: KeybindingSourceFilter) {
        self.source_filter = filter;
        self.scroll_offset = 0.0;
    }

    pub fn sort_by(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.sort_direction = match self.sort_direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        } else {
            self.sort_column = column;
            self.sort_direction = SortDirection::Ascending;
        }
        self.apply_sort();
    }

    pub fn start_recording(&mut self, index: usize) {
        self.recording_keybinding = true;
        self.recorded_keys = None;
        self.editing_index = Some(index);
    }

    pub fn finish_recording(&mut self) {
        if let (Some(idx), Some(ref keys)) = (self.editing_index, &self.recorded_keys) {
            if let Some(entry) = self.entries.get(idx) {
                let cmd = entry.command.clone();
                let k = keys.clone();
                (self.on_event)(KeybindingsEvent::RecordKeybinding { command: cmd, keys: k });
            }
        }
        self.recording_keybinding = false;
        self.recorded_keys = None;
        self.editing_index = None;
    }

    pub fn cancel_recording(&mut self) {
        self.recording_keybinding = false;
        self.recorded_keys = None;
        self.editing_index = None;
    }

    pub fn remove_keybinding(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            let cmd = entry.command.clone();
            (self.on_event)(KeybindingsEvent::RemoveKeybinding { command: cmd });
        }
    }

    pub fn reset_keybinding(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            let cmd = entry.command.clone();
            (self.on_event)(KeybindingsEvent::ResetKeybinding { command: cmd });
        }
    }

    pub fn define_keybinding(&mut self) {
        (self.on_event)(KeybindingsEvent::DefineKeybinding);
    }

    fn apply_sort(&mut self) {
        let dir = self.sort_direction;
        self.entries.sort_by(|a, b| {
            let ord = match self.sort_column {
                SortColumn::Command => a.command_title.to_lowercase().cmp(&b.command_title.to_lowercase()),
                SortColumn::Keybinding => a.keybinding_label.cmp(&b.keybinding_label),
                SortColumn::Source => a.source.cmp(&b.source),
                SortColumn::When => a.when_clause.cmp(&b.when_clause),
            };
            if dir == SortDirection::Descending { ord.reverse() } else { ord }
        });
    }

    fn filtered_entries(&self) -> Vec<(usize, &KeybindingDisplayEntry)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                match self.source_filter {
                    KeybindingSourceFilter::All => {}
                    KeybindingSourceFilter::Default => if e.is_user_modified { return false; }
                    KeybindingSourceFilter::User => if !e.is_user_modified { return false; }
                    KeybindingSourceFilter::Extension => if e.source != "extension" { return false; }
                }
                if self.search_query.is_empty() {
                    return true;
                }
                let q = self.search_query.to_lowercase();
                e.command_title.to_lowercase().contains(&q)
                    || e.command.to_lowercase().contains(&q)
                    || e.keybinding_label.as_deref().unwrap_or("").to_lowercase().contains(&q)
                    || e.when_clause.as_deref().unwrap_or("").to_lowercase().contains(&q)
            })
            .collect()
    }
}

impl<OnEvent> Widget for KeybindingsEditor<OnEvent>
where
    OnEvent: FnMut(KeybindingsEvent),
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
        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, self.background, 0.0);

        let pad = 12.0;
        let mut y = rect.y;

        // Search bar
        let border = if self.recording_keybinding {
            self.recording_border
        } else if self.search_focused {
            self.search_border_focused
        } else {
            self.search_border
        };
        rr.draw_rect(rect.x + pad, y + 8.0, rect.width - pad * 2.0 - 120.0, self.search_bar_height, self.search_bg, 2.0);
        rr.draw_border(rect.x + pad, y + 8.0, rect.width - pad * 2.0 - 120.0, self.search_bar_height, border, 1.0);

        // Define Keybinding button
        let btn_w = 108.0;
        rr.draw_rect(rect.x + rect.width - pad - btn_w, y + 8.0, btn_w, self.search_bar_height, self.define_button_bg, 3.0);
        y += self.search_bar_height + 16.0;

        // Column headers
        rr.draw_rect(rect.x, y, rect.width, self.header_height, self.header_bg, 0.0);
        rr.draw_rect(rect.x, y + self.header_height - 1.0, rect.width, 1.0, self.separator_color, 0.0);
        y += self.header_height;

        // Rows
        let filtered = self.filtered_entries();
        for (i, (_idx, entry)) in filtered.iter().enumerate() {
            let ry = y + i as f32 * self.row_height - self.scroll_offset;
            if ry + self.row_height < y || ry > rect.y + rect.height {
                continue;
            }

            let is_sel = self.selected_index == Some(*_idx);
            if is_sel {
                rr.draw_rect(rect.x, ry, rect.width, self.row_height, self.row_selected_bg, 0.0);
            }

            // User-modified indicator
            if entry.is_user_modified {
                rr.draw_rect(rect.x, ry, 3.0, self.row_height, self.user_modified_fg, 0.0);
            }

            // Keybinding badge
            if entry.keybinding_label.is_some() {
                let badge_x = rect.x + rect.width * self.col_command_w + 4.0;
                rr.draw_rect(badge_x, ry + 3.0, 80.0, self.row_height - 6.0, self.keybinding_badge_bg, 3.0);
            }

            // Edit pencil button
            let edit_x = rect.x + rect.width - 28.0;
            rr.draw_rect(edit_x, ry + 2.0, 20.0, self.row_height - 4.0, self.edit_button_bg, 2.0);

            // Row separator
            rr.draw_rect(rect.x, ry + self.row_height - 1.0, rect.width, 1.0, self.separator_color, 0.0);
        }

        let _ = renderer;
    }

    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => { self.focused = true; EventResult::Handled }
            UiEvent::Blur => { self.focused = false; self.search_focused = false; EventResult::Handled }
            UiEvent::KeyPress { key: Key::Escape, .. } if self.recording_keybinding => {
                self.cancel_recording();
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Enter, .. } if self.recording_keybinding => {
                self.finish_recording();
                EventResult::Handled
            }
            UiEvent::MouseDown { x, y, button: MouseButton::Left } if rect.contains(*x, *y) => {
                self.focused = true;
                let search_bottom = rect.y + 8.0 + self.search_bar_height;
                if *y < search_bottom {
                    self.search_focused = true;
                    return EventResult::Handled;
                }
                self.search_focused = false;

                let header_bottom = search_bottom + 16.0 + self.header_height;
                if *y < header_bottom && *y >= search_bottom + 16.0 {
                    // Column header click → sort
                    let rel = (*x - rect.x) / rect.width;
                    if rel < self.col_command_w {
                        self.sort_by(SortColumn::Command);
                    } else if rel < self.col_command_w + self.col_keybinding_w {
                        self.sort_by(SortColumn::Keybinding);
                    } else if rel < self.col_command_w + self.col_keybinding_w + self.col_when_w {
                        self.sort_by(SortColumn::When);
                    } else {
                        self.sort_by(SortColumn::Source);
                    }
                    return EventResult::Handled;
                }

                // Row click
                let row_top = header_bottom;
                let filtered = self.filtered_entries();
                let idx = ((*y - row_top + self.scroll_offset) / self.row_height) as usize;
                if idx < filtered.len() {
                    let real_idx = filtered[idx].0;
                    self.selected_index = Some(real_idx);
                    // Edit pencil area
                    if *x >= rect.x + rect.width - 28.0 {
                        self.start_recording(real_idx);
                    }
                }
                EventResult::Handled
            }
            UiEvent::MouseDown { x, y, button: MouseButton::Right } if rect.contains(*x, *y) => {
                let header_bottom = rect.y + 8.0 + self.search_bar_height + 16.0 + self.header_height;
                let filtered = self.filtered_entries();
                let idx = ((*y - header_bottom + self.scroll_offset) / self.row_height) as usize;
                if idx < filtered.len() {
                    self.context_menu_index = Some(filtered[idx].0);
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let filtered = self.filtered_entries();
                let total = filtered.len() as f32 * self.row_height;
                let max = (total - rect.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
