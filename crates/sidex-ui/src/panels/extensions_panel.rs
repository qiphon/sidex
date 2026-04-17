//! Extensions panel — browse, install, manage, and update extensions.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Extension info ───────────────────────────────────────────────────────────

/// Metadata for an extension card.
#[derive(Clone, Debug)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub author: String,
    pub description: String,
    pub version: String,
    pub install_count: u64,
    pub rating: f32,
    pub rating_count: u32,
    pub icon_url: Option<String>,
    pub state: ExtensionState,
    pub categories: Vec<String>,
}

impl ExtensionInfo {
    pub fn install_count_label(&self) -> String {
        if self.install_count >= 1_000_000 {
            format!("{:.1}M", self.install_count as f64 / 1_000_000.0)
        } else if self.install_count >= 1_000 {
            format!("{:.1}K", self.install_count as f64 / 1_000.0)
        } else {
            self.install_count.to_string()
        }
    }

    pub fn rating_stars(&self) -> u8 {
        self.rating.round() as u8
    }
}

/// Current state of an extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionState {
    NotInstalled,
    Installed,
    Disabled,
    UpdateAvailable,
    Installing,
    Uninstalling,
}

// ── Extension list item (marketplace-oriented card) ──────────────────────────

/// A lightweight extension card used in search results and category listings.
/// Mirrors the marketplace result shape for direct rendering.
#[derive(Clone, Debug)]
pub struct ExtensionListItem {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub description: String,
    pub version: String,
    pub icon_url: Option<String>,
    pub install_count: u64,
    pub rating: f32,
    pub is_installed: bool,
    pub is_enabled: bool,
    pub update_available: Option<String>,
}

impl ExtensionListItem {
    pub fn install_count_label(&self) -> String {
        if self.install_count >= 1_000_000 {
            format!("{:.1}M", self.install_count as f64 / 1_000_000.0)
        } else if self.install_count >= 1_000 {
            format!("{:.1}K", self.install_count as f64 / 1_000.0)
        } else {
            self.install_count.to_string()
        }
    }

    pub fn rating_stars(&self) -> u8 {
        self.rating.round() as u8
    }

    pub fn effective_state(&self) -> ExtensionState {
        if self.update_available.is_some() {
            ExtensionState::UpdateAvailable
        } else if !self.is_installed {
            ExtensionState::NotInstalled
        } else if !self.is_enabled {
            ExtensionState::Disabled
        } else {
            ExtensionState::Installed
        }
    }
}

// ── Extension filter ─────────────────────────────────────────────────────────

/// Filters for narrowing the extension list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionFilter {
    Installed,
    Enabled,
    Disabled,
    Outdated,
    Recommended,
    Popular,
    Category(String),
}

impl ExtensionFilter {
    pub fn label(&self) -> &str {
        match self {
            Self::Installed => "Installed",
            Self::Enabled => "Enabled",
            Self::Disabled => "Disabled",
            Self::Outdated => "Outdated",
            Self::Recommended => "Recommended",
            Self::Popular => "Popular",
            Self::Category(_) => "Category",
        }
    }

    pub fn all_builtin() -> Vec<Self> {
        vec![
            Self::Installed,
            Self::Enabled,
            Self::Disabled,
            Self::Outdated,
            Self::Recommended,
            Self::Popular,
        ]
    }
}

// ── Extension actions ────────────────────────────────────────────────────────

/// Actions for extension management.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionAction {
    Install(String),
    Uninstall(String),
    Update(String),
    Enable(String),
    Disable(String),
    ShowDetails(String),
    Search(String),
}

// ── View mode ────────────────────────────────────────────────────────────────

/// Which list of extensions is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExtensionView {
    #[default]
    Installed,
    Recommended,
    Search,
    Popular,
}

// ── Extension detail ─────────────────────────────────────────────────────────

/// Detailed information for the extension detail view.
#[derive(Clone, Debug)]
pub struct ExtensionDetail {
    pub id: String,
    pub readme_html: String,
    pub changelog_html: String,
    pub features_html: String,
    pub dependencies: Vec<String>,
    pub extension_pack: Vec<String>,
    pub active_tab: ExtensionDetailTab,
    pub reviews: Vec<ExtensionReview>,
}

/// Tabs in the extension detail view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExtensionDetailTab {
    #[default]
    Readme,
    Features,
    Changelog,
    Dependencies,
    Reviews,
}

/// A user review of an extension.
#[derive(Clone, Debug)]
pub struct ExtensionReview {
    pub author: String,
    pub rating: u8,
    pub text: String,
    pub timestamp: u64,
}

// ── Runtime status ───────────────────────────────────────────────────────────

/// Runtime status of an installed extension.
#[derive(Clone, Debug)]
pub struct ExtensionRuntimeStatus {
    pub id: String,
    pub activation_time_ms: Option<u64>,
    pub activated: bool,
    pub startup_error: Option<String>,
    pub unresponsive: bool,
    pub memory_usage: Option<u64>,
}

// ── Recommendations ──────────────────────────────────────────────────────────

/// Source of an extension recommendation.
#[derive(Clone, Debug)]
pub struct ExtensionRecommendation {
    pub id: String,
    pub reason: RecommendationReason,
}

/// Why an extension is recommended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecommendationReason {
    Workspace,
    FileType(String),
    Popular,
    Explicit,
}

// ── Extension bisect ─────────────────────────────────────────────────────────

/// State of an extension bisect session.
#[derive(Clone, Debug)]
pub struct ExtensionBisect {
    pub active: bool,
    pub all_extensions: Vec<String>,
    pub disabled_set: Vec<String>,
    pub step: u32,
    pub total_steps: u32,
    pub result: Option<String>,
}

impl ExtensionBisect {
    pub fn new(extension_ids: Vec<String>) -> Self {
        let total = (extension_ids.len() as f64).log2().ceil() as u32 + 1;
        Self {
            active: true,
            all_extensions: extension_ids,
            disabled_set: Vec::new(),
            step: 0,
            total_steps: total,
            result: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.result.is_some()
    }

    pub fn progress_label(&self) -> String {
        format!("Step {} of {}", self.step + 1, self.total_steps)
    }
}

// ── Workspace recommendations ────────────────────────────────────────────────

/// Workspace extension recommendations from `.vscode/extensions.json`.
#[derive(Clone, Debug, Default)]
pub struct WorkspaceRecommendations {
    pub recommended: Vec<String>,
    pub unwanted: Vec<String>,
}

// ── Extensions panel ─────────────────────────────────────────────────────────

/// The Extensions sidebar panel.
///
/// Displays installed, recommended, and marketplace search results.
/// Each extension is rendered as a card with icon, name, author, install count,
/// rating, and description. Provides install/uninstall/update actions and
/// extension detail view.
#[allow(dead_code)]
pub struct ExtensionsPanel<OnAction>
where
    OnAction: FnMut(ExtensionAction),
{
    pub installed: Vec<ExtensionInfo>,
    pub recommended: Vec<ExtensionInfo>,
    pub search_results: Vec<ExtensionInfo>,
    pub list_items: Vec<ExtensionListItem>,
    pub search_query: String,
    pub view: ExtensionView,
    pub active_filter: Option<ExtensionFilter>,
    pub on_action: OnAction,

    // Detail view
    detail: Option<ExtensionDetail>,
    detail_scroll_offset: f32,

    // Runtime statuses
    runtime_statuses: Vec<ExtensionRuntimeStatus>,

    // Recommendations
    recommendations: Vec<ExtensionRecommendation>,
    workspace_recommendations: WorkspaceRecommendations,

    // Bisect
    bisect: Option<ExtensionBisect>,

    selected_index: Option<usize>,
    detail_extension: Option<String>,
    scroll_offset: f32,
    focused: bool,
    search_focused: bool,

    card_height: f32,
    search_bar_height: f32,
    section_header_height: f32,

    background: Color,
    search_bg: Color,
    search_border: Color,
    search_border_focused: Color,
    card_bg: Color,
    card_hover_bg: Color,
    card_selected_bg: Color,
    install_button_bg: Color,
    uninstall_button_bg: Color,
    update_button_bg: Color,
    rating_star: Color,
    rating_star_empty: Color,
    separator_color: Color,
    foreground: Color,
    secondary_fg: Color,
    disabled_fg: Color,
}

impl<OnAction> ExtensionsPanel<OnAction>
where
    OnAction: FnMut(ExtensionAction),
{
    pub fn new(on_action: OnAction) -> Self {
        Self {
            installed: Vec::new(),
            recommended: Vec::new(),
            search_results: Vec::new(),
            list_items: Vec::new(),
            search_query: String::new(),
            view: ExtensionView::Installed,
            active_filter: None,
            on_action,

            detail: None,
            detail_scroll_offset: 0.0,
            runtime_statuses: Vec::new(),
            recommendations: Vec::new(),
            workspace_recommendations: WorkspaceRecommendations::default(),
            bisect: None,

            selected_index: None,
            detail_extension: None,
            scroll_offset: 0.0,
            focused: false,
            search_focused: false,

            card_height: 56.0,
            search_bar_height: 32.0,
            section_header_height: 26.0,

            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            search_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            search_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            search_border_focused: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            card_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            card_hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            card_selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            install_button_bg: Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            uninstall_button_bg: Color::from_hex("#6c2020").unwrap_or(Color::BLACK),
            update_button_bg: Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            rating_star: Color::from_rgb(255, 190, 0),
            rating_star_empty: Color::from_hex("#555555").unwrap_or(Color::BLACK),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
            disabled_fg: Color::from_hex("#6b6b6b").unwrap_or(Color::WHITE),
        }
    }

    pub fn search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        if self.search_query.is_empty() {
            self.view = ExtensionView::Installed;
        } else {
            self.view = ExtensionView::Search;
            let q = self.search_query.clone();
            (self.on_action)(ExtensionAction::Search(q));
        }
    }

    pub fn show_installed(&mut self) {
        self.view = ExtensionView::Installed;
        self.scroll_offset = 0.0;
    }

    pub fn show_recommended(&mut self) {
        self.view = ExtensionView::Recommended;
        self.scroll_offset = 0.0;
    }

    pub fn install(&mut self, id: &str) {
        (self.on_action)(ExtensionAction::Install(id.to_string()));
    }

    pub fn uninstall(&mut self, id: &str) {
        (self.on_action)(ExtensionAction::Uninstall(id.to_string()));
    }

    pub fn show_detail(&mut self, id: &str) {
        self.detail_extension = Some(id.to_string());
        (self.on_action)(ExtensionAction::ShowDetails(id.to_string()));
    }

    // ── Detail view ──────────────────────────────────────────────────────

    pub fn set_detail(&mut self, detail: ExtensionDetail) {
        self.detail = Some(detail);
        self.detail_scroll_offset = 0.0;
    }

    pub fn close_detail(&mut self) {
        self.detail = None;
        self.detail_extension = None;
    }

    pub fn detail(&self) -> Option<&ExtensionDetail> {
        self.detail.as_ref()
    }

    pub fn is_showing_detail(&self) -> bool {
        self.detail.is_some()
    }

    pub fn set_detail_tab(&mut self, tab: ExtensionDetailTab) {
        if let Some(ref mut detail) = self.detail {
            detail.active_tab = tab;
            self.detail_scroll_offset = 0.0;
        }
    }

    // ── Runtime status ───────────────────────────────────────────────────

    pub fn set_runtime_statuses(&mut self, statuses: Vec<ExtensionRuntimeStatus>) {
        self.runtime_statuses = statuses;
    }

    pub fn runtime_status_for(&self, id: &str) -> Option<&ExtensionRuntimeStatus> {
        self.runtime_statuses.iter().find(|s| s.id == id)
    }

    // ── Recommendations ──────────────────────────────────────────────────

    pub fn set_recommendations(&mut self, recs: Vec<ExtensionRecommendation>) {
        self.recommendations = recs;
    }

    pub fn set_workspace_recommendations(&mut self, recs: WorkspaceRecommendations) {
        self.workspace_recommendations = recs;
    }

    pub fn recommendations(&self) -> &[ExtensionRecommendation] {
        &self.recommendations
    }

    // ── Bisect ───────────────────────────────────────────────────────────

    pub fn start_bisect(&mut self) {
        let ids: Vec<String> = self.installed.iter().map(|e| e.id.clone()).collect();
        self.bisect = Some(ExtensionBisect::new(ids));
    }

    pub fn bisect_good(&mut self) {
        if let Some(ref mut bisect) = self.bisect {
            bisect.step += 1;
            if bisect.step >= bisect.total_steps {
                bisect.result = bisect.disabled_set.last().cloned();
            }
        }
    }

    pub fn bisect_bad(&mut self) {
        if let Some(ref mut bisect) = self.bisect {
            bisect.step += 1;
            if bisect.step >= bisect.total_steps {
                bisect.result = bisect.disabled_set.last().cloned();
            }
        }
    }

    pub fn end_bisect(&mut self) {
        self.bisect = None;
    }

    pub fn bisect(&self) -> Option<&ExtensionBisect> {
        self.bisect.as_ref()
    }

    // ── Filtering ─────────────────────────────────────────────────────────

    pub fn set_filter(&mut self, filter: ExtensionFilter) {
        self.active_filter = Some(filter);
        self.scroll_offset = 0.0;
    }

    pub fn clear_filter(&mut self) {
        self.active_filter = None;
    }

    /// Returns the list items matching the active filter.
    pub fn filtered_list_items(&self) -> Vec<&ExtensionListItem> {
        let Some(ref filter) = self.active_filter else {
            return self.list_items.iter().collect();
        };
        self.list_items
            .iter()
            .filter(|item| match filter {
                ExtensionFilter::Installed => item.is_installed,
                ExtensionFilter::Enabled => item.is_installed && item.is_enabled,
                ExtensionFilter::Disabled => item.is_installed && !item.is_enabled,
                ExtensionFilter::Outdated => item.update_available.is_some(),
                ExtensionFilter::Recommended | ExtensionFilter::Popular => true,
                ExtensionFilter::Category(_) => true,
            })
            .collect()
    }

    pub fn set_list_items(&mut self, items: Vec<ExtensionListItem>) {
        self.list_items = items;
    }

    fn active_list(&self) -> &[ExtensionInfo] {
        match self.view {
            ExtensionView::Installed => &self.installed,
            ExtensionView::Recommended | ExtensionView::Popular => &self.recommended,
            ExtensionView::Search => &self.search_results,
        }
    }

    fn action_button_for(state: ExtensionState) -> Option<(&'static str, Color)> {
        match state {
            ExtensionState::NotInstalled => Some((
                "Install",
                Color::from_hex("#0e639c").unwrap_or(Color::BLACK),
            )),
            ExtensionState::UpdateAvailable => {
                Some(("Update", Color::from_hex("#0e639c").unwrap_or(Color::BLACK)))
            }
            ExtensionState::Installed => Some((
                "Uninstall",
                Color::from_hex("#6c2020").unwrap_or(Color::BLACK),
            )),
            _ => None,
        }
    }
}

impl<OnAction> Widget for ExtensionsPanel<OnAction>
where
    OnAction: FnMut(ExtensionAction),
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

        let mut y = rect.y + 8.0;
        let pad = 8.0;

        // Search bar
        let sb = if self.search_focused {
            self.search_border_focused
        } else {
            self.search_border
        };
        rr.draw_rect(
            rect.x + pad,
            y,
            rect.width - pad * 2.0,
            self.search_bar_height,
            self.search_bg,
            2.0,
        );
        rr.draw_border(
            rect.x + pad,
            y,
            rect.width - pad * 2.0,
            self.search_bar_height,
            sb,
            1.0,
        );
        y += self.search_bar_height + 4.0;

        // View tabs (Installed / Recommended)
        let tab_w = (rect.width - pad * 2.0) / 2.0;
        for (i, _label) in ["INSTALLED", "RECOMMENDED"].iter().enumerate() {
            let tab_x = rect.x + pad + i as f32 * tab_w;
            let is_active = (i == 0 && self.view == ExtensionView::Installed)
                || (i == 1 && self.view == ExtensionView::Recommended);
            if is_active {
                rr.draw_rect(
                    tab_x,
                    y + self.section_header_height - 2.0,
                    tab_w,
                    2.0,
                    Color::WHITE,
                    0.0,
                );
            }
        }
        y += self.section_header_height;
        rr.draw_rect(rect.x, y, rect.width, 1.0, self.separator_color, 0.0);
        y += 1.0;

        // Extension cards
        let list = self.active_list();
        for (i, ext) in list.iter().enumerate() {
            let cy = y + i as f32 * self.card_height - self.scroll_offset;
            if cy + self.card_height < rect.y || cy > rect.y + rect.height {
                continue;
            }

            let is_sel = self.selected_index == Some(i);
            let bg = if is_sel {
                self.card_selected_bg
            } else {
                self.card_bg
            };
            rr.draw_rect(rect.x, cy, rect.width, self.card_height, bg, 0.0);

            // Icon placeholder
            let icon_s = 36.0;
            rr.draw_rect(
                rect.x + pad,
                cy + (self.card_height - icon_s) / 2.0,
                icon_s,
                icon_s,
                Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
                4.0,
            );

            // Rating stars
            let stars_y = cy + self.card_height - 14.0;
            for s in 0..5u8 {
                let star_color = if s < ext.rating_stars() {
                    self.rating_star
                } else {
                    self.rating_star_empty
                };
                rr.draw_rect(
                    rect.x + pad + icon_s + 8.0 + f32::from(s) * 12.0,
                    stars_y,
                    10.0,
                    10.0,
                    star_color,
                    5.0,
                );
            }

            // Action button
            if let Some((_label, btn_color)) = Self::action_button_for(ext.state) {
                let btn_w = 60.0;
                let btn_h = 22.0;
                rr.draw_rect(
                    rect.x + rect.width - btn_w - pad,
                    cy + (self.card_height - btn_h) / 2.0,
                    btn_w,
                    btn_h,
                    btn_color,
                    3.0,
                );
            }

            // Card separator
            rr.draw_rect(
                rect.x + pad,
                cy + self.card_height - 1.0,
                rect.width - pad * 2.0,
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
                let pad = 8.0;
                let search_bottom = rect.y + 8.0 + self.search_bar_height;

                if *y < search_bottom {
                    self.search_focused = true;
                    return EventResult::Handled;
                }
                self.search_focused = false;

                let tabs_bottom = search_bottom + 4.0 + self.section_header_height + 1.0;
                if *y < tabs_bottom && *y >= search_bottom + 4.0 {
                    let tab_w = (rect.width - pad * 2.0) / 2.0;
                    if *x < rect.x + pad + tab_w {
                        self.show_installed();
                    } else {
                        self.show_recommended();
                    }
                    return EventResult::Handled;
                }

                // Card clicks
                let list_top = tabs_bottom;
                if *y >= list_top {
                    let idx = ((*y - list_top + self.scroll_offset) / self.card_height) as usize;
                    let list_len = self.active_list().len();
                    if idx < list_len {
                        self.selected_index = Some(idx);
                        let btn_w = 60.0;
                        let btn_x = rect.x + rect.width - btn_w - pad;
                        if *x >= btn_x {
                            let ext_id = self.active_list()[idx].id.clone();
                            let ext_state = self.active_list()[idx].state;
                            match ext_state {
                                ExtensionState::NotInstalled => {
                                    (self.on_action)(ExtensionAction::Install(ext_id))
                                }
                                ExtensionState::Installed => {
                                    (self.on_action)(ExtensionAction::Uninstall(ext_id))
                                }
                                ExtensionState::UpdateAvailable => {
                                    (self.on_action)(ExtensionAction::Update(ext_id))
                                }
                                _ => {}
                            }
                        } else {
                            let id = self.active_list()[idx].id.clone();
                            self.show_detail(&id);
                        }
                    }
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let list = self.active_list();
                let total = list.len() as f32 * self.card_height;
                let max = (total - rect.height).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                EventResult::Handled
            }
            UiEvent::KeyPress {
                key: Key::Enter, ..
            } if self.search_focused => {
                let q = self.search_query.clone();
                self.search(q);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
