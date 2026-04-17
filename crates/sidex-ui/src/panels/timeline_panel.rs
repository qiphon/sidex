//! Timeline panel — shows file history from multiple sources (git, local history).
//!
//! Aggregates history entries from git log and local file history into a
//! unified, chronologically sorted timeline view.

use std::path::{Path, PathBuf};

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

/// Source of a timeline entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimelineSource {
    Git,
    LocalHistory,
    Extension(u32),
}

impl std::fmt::Display for TimelineSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Git => write!(f, "Git History"),
            Self::LocalHistory => write!(f, "Local History"),
            Self::Extension(id) => write!(f, "Extension ({id})"),
        }
    }
}

/// An icon hint for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineIcon {
    GitCommit,
    GitMerge,
    GitBranch,
    FileSave,
    FileRestore,
    Edit,
}

/// A single entry in the timeline.
#[derive(Debug, Clone)]
pub struct TimelineEntry {
    /// Unix timestamp.
    pub timestamp: u64,
    /// Primary display label (e.g. commit message, "Saved version").
    pub label: String,
    /// Source of this entry.
    pub source: TimelineSource,
    /// Detail text (e.g. commit hash, author, file size).
    pub detail: String,
    /// Icon hint.
    pub icon: TimelineIcon,
    /// The file path this entry relates to.
    pub file_path: PathBuf,
    /// Opaque identifier for retrieving content (commit SHA, snapshot timestamp, etc.).
    pub id: String,
}

/// Filters for the timeline view.
#[derive(Debug, Clone)]
pub struct TimelineFilter {
    pub show_git: bool,
    pub show_local_history: bool,
    pub show_extensions: bool,
}

impl Default for TimelineFilter {
    fn default() -> Self {
        Self {
            show_git: true,
            show_local_history: true,
            show_extensions: true,
        }
    }
}

/// The timeline panel state.
#[allow(dead_code)]
pub struct TimelinePanel<F: FnMut(TimelineAction)> {
    /// All entries, sorted newest first.
    entries: Vec<TimelineEntry>,
    /// Active file path.
    file_path: Option<PathBuf>,
    /// Which sources to show.
    filter: TimelineFilter,
    /// Selected entry index.
    selected: Option<usize>,
    /// Scroll offset (in items).
    scroll_offset: usize,
    /// Visible height in items.
    visible_items: usize,
    /// Callback for user actions.
    on_action: F,

    item_height: f32,
    font_size: f32,
    header_height: f32,

    bg_color: Color,
    fg_color: Color,
    selected_bg: Color,
    hover_bg: Color,
    detail_fg: Color,
    source_fg: Color,
    header_bg: Color,
    border_color: Color,

    hovered_index: Option<usize>,
}

/// Actions the timeline panel can emit.
#[derive(Debug, Clone)]
pub enum TimelineAction {
    /// User wants to diff current file vs this timeline entry.
    DiffWithCurrent(TimelineEntry),
    /// User wants to view the content at this entry.
    ViewEntry(TimelineEntry),
    /// User wants to restore this version.
    RestoreEntry(TimelineEntry),
    /// User wants to copy the entry id (e.g. commit hash).
    CopyId(String),
}

impl<F: FnMut(TimelineAction)> TimelinePanel<F> {
    pub fn new(on_action: F) -> Self {
        Self {
            entries: Vec::new(),
            file_path: None,
            filter: TimelineFilter::default(),
            selected: None,
            scroll_offset: 0,
            visible_items: 20,
            on_action,
            item_height: 24.0,
            font_size: 12.0,
            header_height: 28.0,
            bg_color: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            fg_color: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            selected_bg: Color::from_hex("#094771").unwrap_or(Color::BLACK),
            hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            detail_fg: Color::from_hex("#858585").unwrap_or(Color::WHITE),
            source_fg: Color::from_hex("#569cd6").unwrap_or(Color::WHITE),
            header_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            border_color: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            hovered_index: None,
        }
    }

    /// Set the file to display timeline for.
    pub fn set_file(&mut self, path: &Path) {
        self.file_path = Some(path.to_path_buf());
        self.selected = None;
        self.scroll_offset = 0;
    }

    /// Load entries into the panel (already sorted newest first).
    pub fn set_entries(&mut self, entries: Vec<TimelineEntry>) {
        self.entries = entries;
        self.sort_entries();
        self.selected = None;
        self.scroll_offset = 0;
    }

    /// Add a single entry and re-sort.
    pub fn add_entry(&mut self, entry: TimelineEntry) {
        self.entries.push(entry);
        self.sort_entries();
    }

    /// Get the currently filtered and visible entries.
    #[must_use]
    pub fn visible_entries(&self) -> Vec<&TimelineEntry> {
        self.entries
            .iter()
            .filter(|e| self.passes_filter(e))
            .collect()
    }

    /// Number of visible (filtered) entries.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.visible_entries().len()
    }

    /// Update the filter settings.
    pub fn set_filter(&mut self, filter: TimelineFilter) {
        self.filter = filter;
        self.selected = None;
        self.scroll_offset = 0;
    }

    /// Get current filter.
    #[must_use]
    pub fn filter(&self) -> &TimelineFilter {
        &self.filter
    }

    /// Select the next entry.
    pub fn select_next(&mut self) {
        let count = self.entry_count();
        if count == 0 {
            return;
        }
        self.selected = Some(match self.selected {
            Some(i) if i + 1 < count => i + 1,
            _ => 0,
        });
        self.ensure_visible();
    }

    /// Select the previous entry.
    pub fn select_prev(&mut self) {
        let count = self.entry_count();
        if count == 0 {
            return;
        }
        self.selected = Some(match self.selected {
            Some(0) | None => count.saturating_sub(1),
            Some(i) => i - 1,
        });
        self.ensure_visible();
    }

    /// Get the selected entry.
    #[must_use]
    pub fn selected_entry(&self) -> Option<&TimelineEntry> {
        let entries = self.visible_entries();
        self.selected.and_then(|i| entries.get(i).copied())
    }

    /// Trigger a diff action for the selected entry.
    pub fn diff_selected(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            (self.on_action)(TimelineAction::DiffWithCurrent(entry));
        }
    }

    fn passes_filter(&self, entry: &TimelineEntry) -> bool {
        match entry.source {
            TimelineSource::Git => self.filter.show_git,
            TimelineSource::LocalHistory => self.filter.show_local_history,
            TimelineSource::Extension(_) => self.filter.show_extensions,
        }
    }

    fn sort_entries(&mut self) {
        self.entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    fn ensure_visible(&mut self) {
        if let Some(idx) = self.selected {
            if idx < self.scroll_offset {
                self.scroll_offset = idx;
            } else if idx >= self.scroll_offset + self.visible_items {
                self.scroll_offset = idx.saturating_sub(self.visible_items - 1);
            }
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn item_rect(&self, base_rect: Rect, index: usize) -> Rect {
        let y = base_rect.y
            + self.header_height
            + (index as f32 - self.scroll_offset as f32) * self.item_height;
        Rect::new(base_rect.x, y, base_rect.width, self.item_height)
    }
}

impl<F: FnMut(TimelineAction)> Widget for TimelinePanel<F> {
    fn layout(&self) -> LayoutNode {
        LayoutNode {
            size: Size::Flex(1.0),
            ..LayoutNode::default()
        }
    }

    fn render(&self, rect: Rect, _renderer: &mut GpuRenderer) {
        let _ = rect;
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        match event {
            UiEvent::MouseMove { x, y } => {
                let count = self.entry_count();
                self.hovered_index = None;
                for i in self.scroll_offset..count.min(self.scroll_offset + self.visible_items) {
                    let ir = self.item_rect(rect, i);
                    if ir.contains(*x, *y) {
                        self.hovered_index = Some(i);
                        break;
                    }
                }
                EventResult::Ignored
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                let count = self.entry_count();
                for i in self.scroll_offset..count.min(self.scroll_offset + self.visible_items) {
                    let ir = self.item_rect(rect, i);
                    if ir.contains(*x, *y) {
                        self.selected = Some(i);
                        return EventResult::Handled;
                    }
                }
                EventResult::Ignored
            }
            UiEvent::KeyPress { key, .. } => match key {
                Key::ArrowDown => {
                    self.select_next();
                    EventResult::Handled
                }
                Key::ArrowUp => {
                    self.select_prev();
                    EventResult::Handled
                }
                Key::Enter => {
                    self.diff_selected();
                    EventResult::Handled
                }
                _ => EventResult::Ignored,
            },
            UiEvent::MouseScroll { dy, .. } => {
                let lines = (*dy / self.item_height).abs() as usize;
                let lines = lines.max(1);
                if *dy < 0.0 {
                    self.scroll_offset = self.scroll_offset.saturating_add(lines);
                } else {
                    self.scroll_offset = self.scroll_offset.saturating_sub(lines);
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}

/// Utility: format a unix timestamp as a relative time string.
#[must_use]
pub fn format_relative_time(timestamp: u64, now: u64) -> String {
    if now <= timestamp {
        return "just now".to_string();
    }
    let delta = now - timestamp;
    if delta < 60 {
        return format!("{delta}s ago");
    }
    let minutes = delta / 60;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{days}d ago");
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months}mo ago");
    }
    let years = months / 12;
    format!("{years}y ago")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(ts: u64, label: &str, source: TimelineSource) -> TimelineEntry {
        TimelineEntry {
            timestamp: ts,
            label: label.to_string(),
            source,
            detail: String::new(),
            icon: TimelineIcon::GitCommit,
            file_path: PathBuf::from("test.rs"),
            id: format!("{ts}"),
        }
    }

    #[test]
    fn entries_sorted_newest_first() {
        let mut actions = Vec::new();
        let mut panel = TimelinePanel::new(|a| actions.push(a));

        panel.set_entries(vec![
            make_entry(100, "older", TimelineSource::Git),
            make_entry(300, "newest", TimelineSource::Git),
            make_entry(200, "middle", TimelineSource::Git),
        ]);

        let visible = panel.visible_entries();
        assert_eq!(visible[0].timestamp, 300);
        assert_eq!(visible[1].timestamp, 200);
        assert_eq!(visible[2].timestamp, 100);
    }

    #[test]
    fn filter_by_source() {
        let mut actions = Vec::new();
        let mut panel = TimelinePanel::new(|a| actions.push(a));

        panel.set_entries(vec![
            make_entry(100, "git", TimelineSource::Git),
            make_entry(200, "local", TimelineSource::LocalHistory),
        ]);

        assert_eq!(panel.entry_count(), 2);

        panel.set_filter(TimelineFilter {
            show_git: false,
            show_local_history: true,
            show_extensions: true,
        });
        assert_eq!(panel.entry_count(), 1);
        assert_eq!(
            panel.visible_entries()[0].source,
            TimelineSource::LocalHistory
        );
    }

    #[test]
    fn select_next_prev() {
        let mut panel = TimelinePanel::new(|_| {});
        panel.set_entries(vec![
            make_entry(300, "a", TimelineSource::Git),
            make_entry(200, "b", TimelineSource::Git),
            make_entry(100, "c", TimelineSource::Git),
        ]);

        panel.select_next();
        assert_eq!(panel.selected, Some(0));

        panel.select_next();
        assert_eq!(panel.selected, Some(1));

        panel.select_prev();
        assert_eq!(panel.selected, Some(0));

        panel.select_prev();
        assert_eq!(panel.selected, Some(2));
    }

    #[test]
    fn selected_entry() {
        let mut panel = TimelinePanel::new(|_| {});
        panel.set_entries(vec![
            make_entry(300, "first", TimelineSource::Git),
            make_entry(200, "second", TimelineSource::LocalHistory),
        ]);

        assert!(panel.selected_entry().is_none());

        panel.select_next();
        assert_eq!(panel.selected_entry().unwrap().label, "first");
    }

    #[test]
    fn add_entry_maintains_sort() {
        let mut panel = TimelinePanel::new(|_| {});
        panel.set_entries(vec![
            make_entry(300, "a", TimelineSource::Git),
            make_entry(100, "c", TimelineSource::Git),
        ]);

        panel.add_entry(make_entry(200, "b", TimelineSource::LocalHistory));

        let visible = panel.visible_entries();
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].timestamp, 300);
        assert_eq!(visible[1].timestamp, 200);
        assert_eq!(visible[2].timestamp, 100);
    }

    #[test]
    fn format_relative_times() {
        let now = 1000;
        assert_eq!(format_relative_time(1000, now), "just now");
        assert_eq!(format_relative_time(970, now), "30s ago");
        assert_eq!(format_relative_time(700, now), "5m ago");
        assert_eq!(format_relative_time(0, now), "16m ago");
    }

    #[test]
    fn format_relative_larger() {
        let now = 100_000;
        assert_eq!(format_relative_time(96_400, now), "1h ago");
        assert_eq!(format_relative_time(13_600, now), "1d ago");
    }

    #[test]
    fn timeline_source_display() {
        assert_eq!(TimelineSource::Git.to_string(), "Git History");
        assert_eq!(TimelineSource::LocalHistory.to_string(), "Local History");
    }

    #[test]
    fn empty_panel() {
        let panel = TimelinePanel::new(|_| {});
        assert_eq!(panel.entry_count(), 0);
        assert!(panel.selected_entry().is_none());
    }
}
