//! Settings editor panel — searchable, categorized settings UI.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Setting types ────────────────────────────────────────────────────────────

/// The control type for a single setting entry.
#[derive(Clone, Debug)]
pub enum SettingControl {
    Boolean(bool),
    String(String),
    Number {
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    Enum {
        value: String,
        options: Vec<String>,
    },
    StringArray(Vec<String>),
    Object(String),
}

/// A single setting entry.
#[derive(Clone, Debug)]
pub struct SettingEntry {
    pub key: String,
    pub display_name: String,
    pub description: String,
    pub category: String,
    pub control: SettingControl,
    pub default_control: SettingControl,
    pub modified: bool,
    pub scope: SettingScope,
}

impl SettingEntry {
    pub fn boolean(
        key: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        value: bool,
        default: bool,
    ) -> Self {
        Self {
            key: key.into(),
            display_name: display_name.into(),
            description: description.into(),
            category: category.into(),
            control: SettingControl::Boolean(value),
            default_control: SettingControl::Boolean(default),
            modified: value != default,
            scope: SettingScope::User,
        }
    }

    pub fn string(
        key: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        value: impl Into<String>,
        default: impl Into<String>,
    ) -> Self {
        let v: String = value.into();
        let d: String = default.into();
        let modified = v != d;
        Self {
            key: key.into(),
            display_name: display_name.into(),
            description: description.into(),
            category: category.into(),
            control: SettingControl::String(v),
            default_control: SettingControl::String(d),
            modified,
            scope: SettingScope::User,
        }
    }

    pub fn number(
        key: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        value: f64,
        default: f64,
        min: Option<f64>,
        max: Option<f64>,
    ) -> Self {
        Self {
            key: key.into(),
            display_name: display_name.into(),
            description: description.into(),
            category: category.into(),
            control: SettingControl::Number { value, min, max },
            default_control: SettingControl::Number {
                value: default,
                min,
                max,
            },
            modified: (value - default).abs() > f64::EPSILON,
            scope: SettingScope::User,
        }
    }

    pub fn enumeration(
        key: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        category: impl Into<String>,
        value: impl Into<String>,
        default: impl Into<String>,
        options: Vec<String>,
    ) -> Self {
        let v: String = value.into();
        let d: String = default.into();
        let modified = v != d;
        Self {
            key: key.into(),
            display_name: display_name.into(),
            description: description.into(),
            category: category.into(),
            control: SettingControl::Enum { value: v, options },
            default_control: SettingControl::Enum {
                value: d,
                options: Vec::new(),
            },
            modified,
            scope: SettingScope::User,
        }
    }

    pub fn reset_to_default(&mut self) {
        self.control = self.default_control.clone();
        self.modified = false;
    }
}

/// Scope at which a setting is applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingScope {
    User,
    Workspace,
    Folder,
}

// ── Category ─────────────────────────────────────────────────────────────────

/// A settings category for the sidebar navigation.
#[derive(Clone, Debug)]
pub struct SettingsCategory {
    pub id: String,
    pub label: String,
    pub children: Vec<SettingsCategory>,
    pub setting_count: usize,
}

impl SettingsCategory {
    pub fn new(id: impl Into<String>, label: impl Into<String>, count: usize) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            setting_count: count,
        }
    }
}

// ── View mode ────────────────────────────────────────────────────────────────

/// Whether the settings panel shows the GUI or JSON editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsViewMode {
    #[default]
    Gui,
    Json,
}

// ── Commonly used categories ─────────────────────────────────────────────────

/// Well-known settings categories matching VS Code's sidebar.
pub fn default_setting_categories() -> Vec<SettingsCategory> {
    vec![
        SettingsCategory::new("commonly-used", "Commonly Used", 0),
        SettingsCategory::new("text-editor", "Text Editor", 0),
        SettingsCategory::new("workbench", "Workbench", 0),
        SettingsCategory::new("window", "Window", 0),
        SettingsCategory::new("features", "Features", 0),
        SettingsCategory::new("application", "Application", 0),
        SettingsCategory::new("extensions", "Extensions", 0),
    ]
}

// ── Settings filter mode ─────────────────────────────────────────────────────

/// Special filter modes for the settings panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsFilterMode {
    #[default]
    None,
    Modified,
}

// ── Setting tag for fuzzy matching ───────────────────────────────────────────

/// Tags associated with settings for improved fuzzy search.
#[derive(Clone, Debug)]
pub struct SettingTag {
    pub id: String,
    pub label: String,
}

/// Predefined setting tags.
pub fn common_setting_tags() -> Vec<SettingTag> {
    vec![
        SettingTag {
            id: "@modified".into(),
            label: "Modified".into(),
        },
        SettingTag {
            id: "@tag:preview".into(),
            label: "Preview".into(),
        },
        SettingTag {
            id: "@tag:experimental".into(),
            label: "Experimental".into(),
        },
        SettingTag {
            id: "@tag:deprecated".into(),
            label: "Deprecated".into(),
        },
        SettingTag {
            id: "@ext:".into(),
            label: "Extension ID".into(),
        },
        SettingTag {
            id: "@feature:".into(),
            label: "Feature".into(),
        },
        SettingTag {
            id: "@id:".into(),
            label: "Setting ID".into(),
        },
        SettingTag {
            id: "@lang:".into(),
            label: "Language".into(),
        },
    ]
}

// ── Settings panel ───────────────────────────────────────────────────────────

/// The Settings editor tab.
///
/// Provides a searchable settings UI with categories in a sidebar,
/// each setting rendered as an appropriate control (checkbox, dropdown,
/// text input, number input). Supports modified indicators, reset to
/// default, and a JSON view toggle.
#[allow(dead_code)]
pub struct SettingsPanel<OnChange>
where
    OnChange: FnMut(&str, &SettingControl),
{
    pub settings: Vec<SettingEntry>,
    pub categories: Vec<SettingsCategory>,
    pub search_query: String,
    pub active_scope: SettingScope,
    pub view_mode: SettingsViewMode,
    pub on_change: OnChange,

    // Filter mode
    filter_mode: SettingsFilterMode,

    // Show default value
    show_default_values: bool,

    // JSON editor content (when in JSON mode)
    json_content: String,

    // Search history
    search_history: Vec<String>,

    selected_category: Option<String>,
    selected_setting: Option<usize>,
    scroll_offset: f32,
    category_scroll_offset: f32,
    focused: bool,
    search_focused: bool,

    search_bar_height: f32,
    scope_tab_height: f32,
    setting_row_height: f32,
    category_width: f32,
    checkbox_size: f32,

    background: Color,
    search_bg: Color,
    search_border: Color,
    search_border_focused: Color,
    category_bg: Color,
    category_selected_bg: Color,
    category_hover_bg: Color,
    setting_hover_bg: Color,
    modified_indicator: Color,
    checkbox_bg: Color,
    checkbox_checked_bg: Color,
    input_bg: Color,
    input_border: Color,
    dropdown_bg: Color,
    reset_button_fg: Color,
    scope_active_border: Color,
    separator_color: Color,
    foreground: Color,
    secondary_fg: Color,
}

impl<OnChange> SettingsPanel<OnChange>
where
    OnChange: FnMut(&str, &SettingControl),
{
    pub fn new(on_change: OnChange) -> Self {
        Self {
            settings: Vec::new(),
            categories: Vec::new(),
            search_query: String::new(),
            active_scope: SettingScope::User,
            view_mode: SettingsViewMode::Gui,
            on_change,

            filter_mode: SettingsFilterMode::default(),
            show_default_values: true,
            json_content: String::new(),
            search_history: Vec::new(),

            selected_category: None,
            selected_setting: None,
            scroll_offset: 0.0,
            category_scroll_offset: 0.0,
            focused: false,
            search_focused: false,

            search_bar_height: 32.0,
            scope_tab_height: 28.0,
            setting_row_height: 80.0,
            category_width: 200.0,
            checkbox_size: 16.0,

            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            search_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            search_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            search_border_focused: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            category_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            category_selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            category_hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            setting_hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            modified_indicator: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            checkbox_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            checkbox_checked_bg: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            input_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            input_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            dropdown_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            reset_button_fg: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            scope_active_border: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
        }
    }

    pub fn set_settings(&mut self, settings: Vec<SettingEntry>) {
        self.settings = settings;
    }

    pub fn set_categories(&mut self, categories: Vec<SettingsCategory>) {
        self.categories = categories;
    }

    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.scroll_offset = 0.0;
    }

    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            SettingsViewMode::Gui => SettingsViewMode::Json,
            SettingsViewMode::Json => SettingsViewMode::Gui,
        };
    }

    pub fn set_scope(&mut self, scope: SettingScope) {
        self.active_scope = scope;
        self.scroll_offset = 0.0;
    }

    pub fn reset_setting(&mut self, index: usize) {
        if let Some(entry) = self.settings.get_mut(index) {
            let key = entry.key.clone();
            entry.reset_to_default();
            let control = entry.control.clone();
            (self.on_change)(&key, &control);
        }
    }

    // ── Filter mode ──────────────────────────────────────────────────────

    pub fn set_filter_mode(&mut self, mode: SettingsFilterMode) {
        self.filter_mode = mode;
        self.scroll_offset = 0.0;
    }

    pub fn filter_mode(&self) -> SettingsFilterMode {
        self.filter_mode
    }

    pub fn toggle_modified_filter(&mut self) {
        self.filter_mode = match self.filter_mode {
            SettingsFilterMode::None => SettingsFilterMode::Modified,
            SettingsFilterMode::Modified => SettingsFilterMode::None,
        };
        self.scroll_offset = 0.0;
    }

    pub fn modified_settings_count(&self) -> usize {
        self.settings.iter().filter(|s| s.modified).count()
    }

    // ── Default value display ────────────────────────────────────────────

    pub fn set_show_default_values(&mut self, show: bool) {
        self.show_default_values = show;
    }

    pub fn show_default_values(&self) -> bool {
        self.show_default_values
    }

    // ── JSON view ────────────────────────────────────────────────────────

    pub fn set_json_content(&mut self, content: impl Into<String>) {
        self.json_content = content.into();
    }

    pub fn json_content(&self) -> &str {
        &self.json_content
    }

    // ── Search history ───────────────────────────────────────────────────

    pub fn push_search_history(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let q = query.to_string();
        self.search_history.retain(|e| *e != q);
        self.search_history.insert(0, q);
        if self.search_history.len() > 20 {
            self.search_history.pop();
        }
    }

    pub fn search_history(&self) -> &[String] {
        &self.search_history
    }

    // ── Fuzzy search ─────────────────────────────────────────────────────

    pub fn fuzzy_matches_setting(query: &str, setting: &SettingEntry) -> bool {
        if query.is_empty() {
            return true;
        }
        let q = query.to_lowercase();

        if q.starts_with("@modified") {
            return setting.modified;
        }
        if let Some(id) = q.strip_prefix("@id:") {
            return setting.key.to_lowercase().contains(id.trim());
        }

        setting.key.to_lowercase().contains(&q)
            || setting.display_name.to_lowercase().contains(&q)
            || setting.description.to_lowercase().contains(&q)
    }

    fn filtered_settings(&self) -> Vec<(usize, &SettingEntry)> {
        self.settings
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                // Modified filter
                if self.filter_mode == SettingsFilterMode::Modified && !s.modified {
                    return false;
                }
                if !self.search_query.is_empty() {
                    return Self::fuzzy_matches_setting(&self.search_query, s);
                }
                if let Some(ref cat) = self.selected_category {
                    return s.category == *cat;
                }
                true
            })
            .filter(|(_, s)| s.scope == self.active_scope)
            .collect()
    }
}

impl<OnChange> Widget for SettingsPanel<OnChange>
where
    OnChange: FnMut(&str, &SettingControl),
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

        let mut y = rect.y;
        let pad = 12.0;

        // Search bar
        let sb = if self.search_focused {
            self.search_border_focused
        } else {
            self.search_border
        };
        rr.draw_rect(
            rect.x + pad,
            y + 8.0,
            rect.width - pad * 2.0,
            self.search_bar_height,
            self.search_bg,
            2.0,
        );
        rr.draw_border(
            rect.x + pad,
            y + 8.0,
            rect.width - pad * 2.0,
            self.search_bar_height,
            sb,
            1.0,
        );

        // JSON toggle button
        let toggle_size = 24.0;
        let toggle_x = rect.x + rect.width - pad - toggle_size;
        rr.draw_rect(
            toggle_x,
            y + 8.0 + (self.search_bar_height - toggle_size) / 2.0,
            toggle_size,
            toggle_size,
            self.input_bg,
            3.0,
        );
        y += self.search_bar_height + 16.0;

        // Scope tabs (User / Workspace)
        let scopes = [
            ("User", SettingScope::User),
            ("Workspace", SettingScope::Workspace),
        ];
        let tab_w = 80.0;
        for (i, (_, scope)) in scopes.iter().enumerate() {
            let tx = rect.x + pad + i as f32 * tab_w;
            if *scope == self.active_scope {
                rr.draw_rect(
                    tx,
                    y + self.scope_tab_height - 2.0,
                    tab_w,
                    2.0,
                    self.scope_active_border,
                    0.0,
                );
            }
        }
        y += self.scope_tab_height;
        rr.draw_rect(rect.x, y, rect.width, 1.0, self.separator_color, 0.0);
        y += 1.0;

        // Category sidebar
        let cat_x = rect.x;
        let content_x = rect.x + self.category_width;
        let content_w = rect.width - self.category_width;

        rr.draw_rect(
            cat_x,
            y,
            self.category_width,
            rect.height - (y - rect.y),
            self.category_bg,
            0.0,
        );
        rr.draw_rect(
            content_x - 1.0,
            y,
            1.0,
            rect.height - (y - rect.y),
            self.separator_color,
            0.0,
        );

        let mut cat_y = y + 4.0;
        for cat in &self.categories {
            let is_sel = self.selected_category.as_deref() == Some(&cat.id);
            if is_sel {
                rr.draw_rect(
                    cat_x,
                    cat_y,
                    self.category_width,
                    22.0,
                    self.category_selected_bg,
                    0.0,
                );
                rr.draw_rect(cat_x, cat_y, 3.0, 22.0, self.modified_indicator, 0.0);
            }
            cat_y += 22.0;
        }

        // Settings list
        let filtered = self.filtered_settings();
        for (i, (_idx, setting)) in filtered.iter().enumerate() {
            let sy = y + i as f32 * self.setting_row_height - self.scroll_offset;
            if sy + self.setting_row_height < y || sy > rect.y + rect.height {
                continue;
            }

            let is_sel = self.selected_setting == Some(i);
            if is_sel {
                rr.draw_rect(
                    content_x,
                    sy,
                    content_w,
                    self.setting_row_height,
                    self.setting_hover_bg,
                    0.0,
                );
            }

            // Modified indicator
            if setting.modified {
                rr.draw_rect(
                    content_x + 4.0,
                    sy + 4.0,
                    4.0,
                    self.setting_row_height - 8.0,
                    self.modified_indicator,
                    2.0,
                );
            }

            // Control rendering
            let control_x = content_x + 16.0;
            let control_y = sy + 42.0;
            match &setting.control {
                SettingControl::Boolean(val) => {
                    let bg = if *val {
                        self.checkbox_checked_bg
                    } else {
                        self.checkbox_bg
                    };
                    rr.draw_rect(
                        control_x,
                        control_y,
                        self.checkbox_size,
                        self.checkbox_size,
                        bg,
                        2.0,
                    );
                }
                SettingControl::String(_) | SettingControl::Number { .. } => {
                    rr.draw_rect(
                        control_x,
                        control_y,
                        content_w - 40.0,
                        24.0,
                        self.input_bg,
                        2.0,
                    );
                    rr.draw_border(
                        control_x,
                        control_y,
                        content_w - 40.0,
                        24.0,
                        self.input_border,
                        1.0,
                    );
                }
                SettingControl::Enum { .. } => {
                    rr.draw_rect(control_x, control_y, 200.0, 24.0, self.dropdown_bg, 2.0);
                }
                SettingControl::StringArray(_) | SettingControl::Object(_) => {
                    rr.draw_rect(
                        control_x,
                        control_y,
                        content_w - 40.0,
                        24.0,
                        self.input_bg,
                        2.0,
                    );
                }
            }

            // Reset button for modified settings
            if setting.modified {
                let reset_s = 16.0;
                rr.draw_rect(
                    content_x + content_w - reset_s - 12.0,
                    sy + 8.0,
                    reset_s,
                    reset_s,
                    self.reset_button_fg,
                    8.0,
                );
            }

            // Row separator
            rr.draw_rect(
                content_x + 8.0,
                sy + self.setting_row_height - 1.0,
                content_w - 16.0,
                1.0,
                self.separator_color,
                0.0,
            );
        }

        let _ = renderer;
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::Focus => {
                self.focused = true;
                EventResult::Handled
            }
            UiEvent::Blur => {
                self.focused = false;
                self.search_focused = false;
                EventResult::Handled
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.focused = true;

                // Search bar
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
                        self.set_scope(SettingScope::User);
                    } else if rel_x >= tab_w && rel_x < tab_w * 2.0 {
                        self.set_scope(SettingScope::Workspace);
                    }
                    return EventResult::Handled;
                }

                // Category sidebar
                if *x < rect.x + self.category_width {
                    let cat_top = scope_bottom + 4.0;
                    let idx = ((*y - cat_top) / 22.0) as usize;
                    if let Some(cat) = self.categories.get(idx) {
                        self.selected_category = Some(cat.id.clone());
                        self.scroll_offset = 0.0;
                    }
                    return EventResult::Handled;
                }

                // Setting rows
                let content_top = scope_bottom;
                let filtered: Vec<(usize, String, SettingControl)> = self
                    .filtered_settings()
                    .iter()
                    .map(|(idx, s)| (*idx, s.key.clone(), s.control.clone()))
                    .collect();
                let idx =
                    ((*y - content_top + self.scroll_offset) / self.setting_row_height) as usize;
                if idx < filtered.len() {
                    let (real_idx, ref key, ref control) = filtered[idx];
                    self.selected_setting = Some(idx);

                    // Toggle boolean on click
                    if let SettingControl::Boolean(val) = control {
                        let new_val = !val;
                        let key = key.clone();
                        self.settings[real_idx].control = SettingControl::Boolean(new_val);
                        self.settings[real_idx].modified = true;
                        let ctrl = self.settings[real_idx].control.clone();
                        (self.on_change)(&key, &ctrl);
                    }
                }

                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let filtered = self.filtered_settings();
                let total = filtered.len() as f32 * self.setting_row_height;
                let max = (total - rect.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                EventResult::Handled
            }
            UiEvent::KeyPress {
                key: Key::Enter, ..
            } if self.search_focused => EventResult::Handled,
            _ => EventResult::Ignored,
        }
    }
}
