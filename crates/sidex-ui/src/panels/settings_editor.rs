//! Full-featured settings editor — VS Code Ctrl+, style.

use std::path::PathBuf;

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Scope ────────────────────────────────────────────────────────────────────

/// The scope at which settings are read/written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsScope {
    User,
    Workspace,
    Folder(PathBuf),
}

/// Where a particular value was defined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingValueScope {
    Default,
    User,
    Workspace,
    Folder,
    LanguageOverride(String),
}

// ── Types ────────────────────────────────────────────────────────────────────

/// The JSON-schema type of a setting, determining which control to render.
#[derive(Clone, Debug)]
pub enum SettingType {
    Boolean,
    String,
    Number,
    Integer,
    Array(Box<SettingType>),
    Object,
    Enum(Vec<String>),
    Color,
    MultilineString,
}

// ── Group / Entry ────────────────────────────────────────────────────────────

/// A hierarchical group of settings (e.g. "Editor > Font").
#[derive(Clone, Debug)]
pub struct SettingGroup {
    pub id: String,
    pub title: String,
    pub order: u32,
    pub settings: Vec<SettingEntry>,
    pub subgroups: Vec<SettingGroup>,
}

impl SettingGroup {
    pub fn new(id: impl Into<String>, title: impl Into<String>, order: u32) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            order,
            settings: Vec::new(),
            subgroups: Vec::new(),
        }
    }

    pub fn total_count(&self) -> usize {
        self.settings.len() + self.subgroups.iter().map(Self::total_count).sum::<usize>()
    }

    pub fn modified_count(&self) -> usize {
        self.settings.iter().filter(|s| s.is_modified).count()
            + self.subgroups.iter().map(Self::modified_count).sum::<usize>()
    }
}

/// A single setting entry rendered as a control row.
#[derive(Clone, Debug)]
pub struct SettingEntry {
    pub key: String,
    pub title: String,
    pub description: String,
    pub description_markdown: bool,
    pub setting_type: SettingType,
    pub default_value: serde_json::Value,
    pub current_value: serde_json::Value,
    pub scope: SettingValueScope,
    pub is_modified: bool,
    pub enum_descriptions: Vec<String>,
    pub tags: Vec<String>,
    pub deprecation_message: Option<String>,
}

impl SettingEntry {
    pub fn boolean(
        key: impl Into<String>,
        title: impl Into<String>,
        desc: impl Into<String>,
        value: bool,
        default: bool,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            description: desc.into(),
            description_markdown: false,
            setting_type: SettingType::Boolean,
            default_value: serde_json::Value::Bool(default),
            current_value: serde_json::Value::Bool(value),
            scope: SettingValueScope::Default,
            is_modified: value != default,
            enum_descriptions: Vec::new(),
            tags: Vec::new(),
            deprecation_message: None,
        }
    }

    pub fn string(
        key: impl Into<String>,
        title: impl Into<String>,
        desc: impl Into<String>,
        value: impl Into<String>,
        default: impl Into<String>,
    ) -> Self {
        let v: String = value.into();
        let d: String = default.into();
        let modified = v != d;
        Self {
            key: key.into(),
            title: title.into(),
            description: desc.into(),
            description_markdown: false,
            setting_type: SettingType::String,
            default_value: serde_json::Value::String(d),
            current_value: serde_json::Value::String(v),
            scope: SettingValueScope::Default,
            is_modified: modified,
            enum_descriptions: Vec::new(),
            tags: Vec::new(),
            deprecation_message: None,
        }
    }

    pub fn number(
        key: impl Into<String>,
        title: impl Into<String>,
        desc: impl Into<String>,
        value: f64,
        default: f64,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            description: desc.into(),
            description_markdown: false,
            setting_type: SettingType::Number,
            default_value: serde_json::json!(default),
            current_value: serde_json::json!(value),
            scope: SettingValueScope::Default,
            is_modified: (value - default).abs() > f64::EPSILON,
            enum_descriptions: Vec::new(),
            tags: Vec::new(),
            deprecation_message: None,
        }
    }

    pub fn enumeration(
        key: impl Into<String>,
        title: impl Into<String>,
        desc: impl Into<String>,
        value: impl Into<String>,
        default: impl Into<String>,
        options: Vec<String>,
        descriptions: Vec<String>,
    ) -> Self {
        let v: String = value.into();
        let d: String = default.into();
        let modified = v != d;
        Self {
            key: key.into(),
            title: title.into(),
            description: desc.into(),
            description_markdown: false,
            setting_type: SettingType::Enum(options),
            default_value: serde_json::Value::String(d),
            current_value: serde_json::Value::String(v),
            scope: SettingValueScope::Default,
            is_modified: modified,
            enum_descriptions: descriptions,
            tags: Vec::new(),
            deprecation_message: None,
        }
    }

    pub fn reset_to_default(&mut self) {
        self.current_value = self.default_value.clone();
        self.is_modified = false;
    }
}

// ── TOC item ─────────────────────────────────────────────────────────────────

/// An entry in the table-of-contents sidebar.
#[derive(Clone, Debug)]
pub struct TocEntry {
    pub group_id: String,
    pub label: String,
    pub depth: u32,
    pub count: usize,
}

// ── Settings editor ──────────────────────────────────────────────────────────

/// The full settings editor (Ctrl+,).
#[allow(dead_code)]
pub struct SettingsEditor<OnChange>
where
    OnChange: FnMut(&str, &serde_json::Value),
{
    pub search_query: String,
    pub filtered_settings: Vec<SettingEntry>,
    pub groups: Vec<SettingGroup>,
    pub active_scope: SettingsScope,
    pub modified_only: bool,
    pub language_overrides: Vec<String>,
    pub active_language_override: Option<String>,
    pub on_change: OnChange,

    toc_entries: Vec<TocEntry>,
    recently_modified: Vec<String>,
    selected_toc: Option<String>,
    selected_setting: Option<usize>,
    scroll_offset: f32,
    toc_scroll_offset: f32,
    focused: bool,
    search_focused: bool,

    search_bar_height: f32,
    scope_tab_height: f32,
    setting_row_height: f32,
    toc_width: f32,
    checkbox_size: f32,

    background: Color,
    search_bg: Color,
    search_border: Color,
    search_border_focused: Color,
    toc_bg: Color,
    toc_selected_bg: Color,
    toc_hover_bg: Color,
    setting_hover_bg: Color,
    modified_indicator: Color,
    checkbox_bg: Color,
    checkbox_checked_bg: Color,
    input_bg: Color,
    input_border: Color,
    dropdown_bg: Color,
    color_swatch_border: Color,
    reset_button_fg: Color,
    scope_active_border: Color,
    separator_color: Color,
    deprecated_bg: Color,
    foreground: Color,
    secondary_fg: Color,
    link_fg: Color,
}

impl<OnChange> SettingsEditor<OnChange>
where
    OnChange: FnMut(&str, &serde_json::Value),
{
    pub fn new(on_change: OnChange) -> Self {
        Self {
            search_query: String::new(),
            filtered_settings: Vec::new(),
            groups: Vec::new(),
            active_scope: SettingsScope::User,
            modified_only: false,
            language_overrides: Vec::new(),
            active_language_override: None,
            on_change,

            toc_entries: Vec::new(),
            recently_modified: Vec::new(),
            selected_toc: None,
            selected_setting: None,
            scroll_offset: 0.0,
            toc_scroll_offset: 0.0,
            focused: false,
            search_focused: false,

            search_bar_height: 32.0,
            scope_tab_height: 28.0,
            setting_row_height: 88.0,
            toc_width: 220.0,
            checkbox_size: 16.0,

            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            search_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            search_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            search_border_focused: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            toc_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            toc_selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            toc_hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            setting_hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            modified_indicator: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            checkbox_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            checkbox_checked_bg: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            input_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            input_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            dropdown_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            color_swatch_border: Color::from_hex("#6c6c6c").unwrap_or(Color::WHITE),
            reset_button_fg: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            scope_active_border: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            deprecated_bg: Color::from_hex("#3c3c1e").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
            link_fg: Color::from_hex("#3794ff").unwrap_or(Color::WHITE),
        }
    }

    pub fn set_groups(&mut self, groups: Vec<SettingGroup>) {
        self.rebuild_toc(&groups);
        self.groups = groups;
        self.refilter();
    }

    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.scroll_offset = 0.0;
        self.refilter();
    }

    pub fn set_scope(&mut self, scope: SettingsScope) {
        self.active_scope = scope;
        self.scroll_offset = 0.0;
    }

    pub fn toggle_modified_only(&mut self) {
        self.modified_only = !self.modified_only;
        self.scroll_offset = 0.0;
        self.refilter();
    }

    pub fn set_language_override(&mut self, lang: Option<String>) {
        self.active_language_override = lang;
        self.scroll_offset = 0.0;
    }

    pub fn reset_setting(&mut self, index: usize) {
        if let Some(entry) = self.filtered_settings.get_mut(index) {
            let key = entry.key.clone();
            entry.reset_to_default();
            let val = entry.current_value.clone();
            (self.on_change)(&key, &val);
        }
    }

    pub fn update_setting(&mut self, index: usize, value: serde_json::Value) {
        if let Some(entry) = self.filtered_settings.get_mut(index) {
            let key = entry.key.clone();
            entry.current_value = value.clone();
            entry.is_modified = entry.current_value != entry.default_value;
            if entry.is_modified && !self.recently_modified.contains(&key) {
                self.recently_modified.insert(0, key.clone());
                if self.recently_modified.len() > 20 {
                    self.recently_modified.pop();
                }
            }
            (self.on_change)(&key, &value);
        }
    }

    pub fn recently_modified(&self) -> &[String] {
        &self.recently_modified
    }

    fn rebuild_toc(&mut self, groups: &[SettingGroup]) {
        self.toc_entries.clear();
        for g in groups {
            self.collect_toc(g, 0);
        }
    }

    fn collect_toc(&mut self, group: &SettingGroup, depth: u32) {
        self.toc_entries.push(TocEntry {
            group_id: group.id.clone(),
            label: group.title.clone(),
            depth,
            count: group.total_count(),
        });
        for sub in &group.subgroups {
            self.collect_toc(sub, depth + 1);
        }
    }

    fn refilter(&mut self) {
        self.filtered_settings.clear();
        let query = self.search_query.clone();
        let modified_only = self.modified_only;
        let selected_toc = self.selected_toc.clone();

        for g in &self.groups {
            Self::collect_filtered_into(
                &mut self.filtered_settings,
                g,
                &query,
                modified_only,
                selected_toc.as_deref(),
            );
        }
    }

    fn collect_filtered_into(
        out: &mut Vec<SettingEntry>,
        group: &SettingGroup,
        query: &str,
        modified_only: bool,
        selected_toc: Option<&str>,
    ) {
        for s in &group.settings {
            if modified_only && !s.is_modified {
                continue;
            }
            if !query.is_empty() && !Self::matches_search(query, s) {
                continue;
            }
            if let Some(toc) = selected_toc {
                if group.id != toc {
                    continue;
                }
            }
            out.push(s.clone());
        }
        for sub in &group.subgroups {
            Self::collect_filtered_into(out, sub, query, modified_only, selected_toc);
        }
    }

    fn matches_search(query: &str, entry: &SettingEntry) -> bool {
        let q = query.to_lowercase();
        if q.starts_with("@modified") {
            return entry.is_modified;
        }
        if let Some(id) = q.strip_prefix("@id:") {
            return entry.key.to_lowercase().contains(id.trim());
        }
        if let Some(tag) = q.strip_prefix("@tag:") {
            return entry.tags.iter().any(|t| t.to_lowercase().contains(tag.trim()));
        }
        entry.key.to_lowercase().contains(&q)
            || entry.title.to_lowercase().contains(&q)
            || entry.description.to_lowercase().contains(&q)
    }
}

impl<OnChange> Widget for SettingsEditor<OnChange>
where
    OnChange: FnMut(&str, &serde_json::Value),
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

        let mut y = rect.y;
        let pad = 12.0;

        // Search bar
        let sb = if self.search_focused { self.search_border_focused } else { self.search_border };
        rr.draw_rect(rect.x + pad, y + 8.0, rect.width - pad * 2.0, self.search_bar_height, self.search_bg, 2.0);
        rr.draw_border(rect.x + pad, y + 8.0, rect.width - pad * 2.0, self.search_bar_height, sb, 1.0);
        y += self.search_bar_height + 16.0;

        // Scope tabs
        let scopes = ["User", "Workspace"];
        let tab_w = 80.0;
        for (i, _label) in scopes.iter().enumerate() {
            let tx = rect.x + pad + i as f32 * tab_w;
            let is_active = match (&self.active_scope, i) {
                (SettingsScope::User, 0) | (SettingsScope::Workspace, 1) => true,
                _ => false,
            };
            if is_active {
                rr.draw_rect(tx, y + self.scope_tab_height - 2.0, tab_w, 2.0, self.scope_active_border, 0.0);
            }
        }
        y += self.scope_tab_height;
        rr.draw_rect(rect.x, y, rect.width, 1.0, self.separator_color, 0.0);
        y += 1.0;

        // TOC sidebar
        let content_x = rect.x + self.toc_width;
        let content_w = rect.width - self.toc_width;
        rr.draw_rect(rect.x, y, self.toc_width, rect.height - (y - rect.y), self.toc_bg, 0.0);
        rr.draw_rect(content_x - 1.0, y, 1.0, rect.height - (y - rect.y), self.separator_color, 0.0);

        let mut toc_y = y + 4.0;
        for entry in &self.toc_entries {
            let is_sel = self.selected_toc.as_deref() == Some(&entry.group_id);
            if is_sel {
                rr.draw_rect(rect.x, toc_y, self.toc_width, 22.0, self.toc_selected_bg, 0.0);
                rr.draw_rect(rect.x, toc_y, 3.0, 22.0, self.modified_indicator, 0.0);
            }
            toc_y += 22.0;
        }

        // Recently modified section header
        if !self.recently_modified.is_empty() && self.search_query.is_empty() {
            rr.draw_rect(content_x + 8.0, y + 4.0, content_w - 16.0, 24.0, self.setting_hover_bg, 2.0);
            y += 32.0;
        }

        // Settings rows
        for (i, setting) in self.filtered_settings.iter().enumerate() {
            let sy = y + i as f32 * self.setting_row_height - self.scroll_offset;
            if sy + self.setting_row_height < y || sy > rect.y + rect.height {
                continue;
            }

            let is_sel = self.selected_setting == Some(i);
            if is_sel {
                rr.draw_rect(content_x, sy, content_w, self.setting_row_height, self.setting_hover_bg, 0.0);
            }

            // Deprecation background
            if setting.deprecation_message.is_some() {
                rr.draw_rect(content_x, sy, content_w, self.setting_row_height, self.deprecated_bg, 0.0);
            }

            // Modified indicator (blue bar)
            if setting.is_modified {
                rr.draw_rect(content_x + 4.0, sy + 4.0, 4.0, self.setting_row_height - 8.0, self.modified_indicator, 2.0);
            }

            // Control
            let cx = content_x + 16.0;
            let cy = sy + 48.0;
            match &setting.setting_type {
                SettingType::Boolean => {
                    let checked = setting.current_value.as_bool().unwrap_or(false);
                    let bg = if checked { self.checkbox_checked_bg } else { self.checkbox_bg };
                    rr.draw_rect(cx, cy, self.checkbox_size, self.checkbox_size, bg, 2.0);
                }
                SettingType::String | SettingType::MultilineString | SettingType::Number | SettingType::Integer => {
                    let h = if matches!(setting.setting_type, SettingType::MultilineString) { 60.0 } else { 24.0 };
                    rr.draw_rect(cx, cy, content_w - 40.0, h, self.input_bg, 2.0);
                    rr.draw_border(cx, cy, content_w - 40.0, h, self.input_border, 1.0);
                }
                SettingType::Enum(_) => {
                    rr.draw_rect(cx, cy, 200.0, 24.0, self.dropdown_bg, 2.0);
                }
                SettingType::Array(_) => {
                    rr.draw_rect(cx, cy, content_w - 40.0, 24.0, self.input_bg, 2.0);
                    // Add item button
                    rr.draw_rect(cx, cy + 28.0, 80.0, 22.0, self.modified_indicator, 3.0);
                }
                SettingType::Object => {
                    rr.draw_rect(cx, cy, content_w - 40.0, 24.0, self.input_bg, 2.0);
                }
                SettingType::Color => {
                    rr.draw_rect(cx, cy, 24.0, 24.0, self.input_bg, 2.0);
                    rr.draw_border(cx, cy, 24.0, 24.0, self.color_swatch_border, 1.0);
                    rr.draw_rect(cx + 32.0, cy, 140.0, 24.0, self.input_bg, 2.0);
                }
            }

            // Reset button
            if setting.is_modified {
                let rs = 16.0;
                rr.draw_rect(content_x + content_w - rs - 12.0, sy + 8.0, rs, rs, self.reset_button_fg, 8.0);
            }

            // Separator
            rr.draw_rect(content_x + 8.0, sy + self.setting_row_height - 1.0, content_w - 16.0, 1.0, self.separator_color, 0.0);
        }

        let _ = renderer;
    }

    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => { self.focused = true; EventResult::Handled }
            UiEvent::Blur => { self.focused = false; self.search_focused = false; EventResult::Handled }
            UiEvent::MouseDown { x, y, button: MouseButton::Left } if rect.contains(*x, *y) => {
                self.focused = true;
                let search_bottom = rect.y + 8.0 + self.search_bar_height;
                if *y < search_bottom {
                    self.search_focused = true;
                    return EventResult::Handled;
                }
                self.search_focused = false;

                // Scope tabs
                let scope_bottom = search_bottom + 16.0 + self.scope_tab_height + 1.0;
                if *y < scope_bottom && *y >= search_bottom + 16.0 {
                    let pad = 12.0;
                    let tab_w = 80.0;
                    let rel_x = *x - rect.x - pad;
                    if rel_x >= 0.0 && rel_x < tab_w {
                        self.set_scope(SettingsScope::User);
                    } else if rel_x >= tab_w && rel_x < tab_w * 2.0 {
                        self.set_scope(SettingsScope::Workspace);
                    }
                    return EventResult::Handled;
                }

                // TOC sidebar
                if *x < rect.x + self.toc_width {
                    let toc_top = scope_bottom + 4.0;
                    let idx = ((*y - toc_top) / 22.0) as usize;
                    if let Some(entry) = self.toc_entries.get(idx) {
                        self.selected_toc = Some(entry.group_id.clone());
                        self.scroll_offset = 0.0;
                        self.refilter();
                    }
                    return EventResult::Handled;
                }

                // Setting rows
                let content_top = scope_bottom;
                let idx = ((*y - content_top + self.scroll_offset) / self.setting_row_height) as usize;
                if idx < self.filtered_settings.len() {
                    self.selected_setting = Some(idx);
                    if matches!(self.filtered_settings[idx].setting_type, SettingType::Boolean) {
                        let cur = self.filtered_settings[idx].current_value.as_bool().unwrap_or(false);
                        let new_val = serde_json::Value::Bool(!cur);
                        self.update_setting(idx, new_val);
                    }
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let total = self.filtered_settings.len() as f32 * self.setting_row_height;
                let max = (total - rect.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Enter, .. } if self.search_focused => EventResult::Handled,
            _ => EventResult::Ignored,
        }
    }
}
