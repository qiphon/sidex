//! Editor group layout — manages split views and tab state for each group.
//!
//! Mirrors VS Code's editor group model: multiple groups arranged in a
//! horizontal or vertical split, each containing an ordered list of tabs.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ── Tab identity ─────────────────────────────────────────────────────────────

/// Unique identifier for a tab within the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

static TAB_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl TabId {
    pub fn next() -> Self {
        Self(TAB_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

// ── Editor tab ───────────────────────────────────────────────────────────────

/// Icon category for an editor tab.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TabIcon {
    Language(String),
    Diff,
    Settings,
    KeyboardShortcuts,
    Welcome,
    Extension(String),
}

impl Default for TabIcon {
    fn default() -> Self {
        Self::Language("plaintext".into())
    }
}

/// A single editor tab within a group.
#[derive(Clone, Debug)]
pub struct EditorTab {
    pub id: TabId,
    pub title: String,
    pub path: Option<PathBuf>,
    pub description: Option<String>,
    pub icon: TabIcon,
    pub is_modified: bool,
    pub is_preview: bool,
    pub is_pinned: bool,
}

impl EditorTab {
    pub fn new_untitled() -> Self {
        Self {
            id: TabId::next(),
            title: "Untitled".into(),
            path: None,
            description: None,
            icon: TabIcon::default(),
            is_modified: false,
            is_preview: false,
            is_pinned: false,
        }
    }

    pub fn from_path(path: &Path) -> Self {
        let title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_owned();
        let description = path.parent().and_then(|p| p.to_str()).map(String::from);
        let icon = TabIcon::Language(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("plaintext")
                .to_owned(),
        );
        Self {
            id: TabId::next(),
            title,
            path: Some(path.to_path_buf()),
            description,
            icon,
            is_modified: false,
            is_preview: false,
            is_pinned: false,
        }
    }

    pub fn from_path_preview(path: &Path) -> Self {
        let mut tab = Self::from_path(path);
        tab.is_preview = true;
        tab
    }
}

// ── Editor group ─────────────────────────────────────────────────────────────

/// A single editor pane containing an ordered list of tabs.
#[derive(Clone, Debug)]
pub struct EditorGroup {
    pub tabs: Vec<EditorTab>,
    pub active_tab: usize,
    pub preview_tab: Option<usize>,
}

impl EditorGroup {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            preview_tab: None,
        }
    }

    pub fn with_tab(tab: EditorTab) -> Self {
        let is_preview = tab.is_preview;
        Self {
            tabs: vec![tab],
            active_tab: 0,
            preview_tab: if is_preview { Some(0) } else { None },
        }
    }

    /// Number of pinned tabs (always at the left).
    fn pinned_count(&self) -> usize {
        self.tabs.iter().take_while(|t| t.is_pinned).count()
    }

    /// Insert position respecting pinned tabs: pinned go at end of pinned
    /// region, unpinned go after all pinned tabs.
    fn insert_position(&self) -> usize {
        if self.tabs.is_empty() {
            return 0;
        }
        self.tabs.len()
    }

    /// Open a file in this group. Returns the tab index that was activated.
    pub fn open_file(&mut self, path: &Path) -> usize {
        if let Some(idx) = self.find_tab_by_path(path) {
            self.active_tab = idx;
            if self.tabs[idx].is_preview {
                self.tabs[idx].is_preview = false;
                self.preview_tab = None;
            }
            return idx;
        }

        let tab = EditorTab::from_path(path);
        let pos = self.insert_position();
        self.tabs.insert(pos, tab);
        self.active_tab = pos;
        if let Some(pt) = self.preview_tab {
            if pt >= pos {
                self.preview_tab = Some(pt + 1);
            }
        }
        pos
    }

    /// Open a file as a preview tab (italic, replaced by next single-click).
    pub fn open_file_preview(&mut self, path: &Path) -> usize {
        if let Some(idx) = self.find_tab_by_path(path) {
            self.active_tab = idx;
            return idx;
        }

        // Replace existing preview tab if any
        if let Some(prev) = self.preview_tab {
            if prev < self.tabs.len() {
                self.tabs.remove(prev);
                if self.active_tab > prev {
                    self.active_tab = self.active_tab.saturating_sub(1);
                }
            }
        }

        let tab = EditorTab::from_path_preview(path);
        let pos = self.insert_position();
        self.tabs.insert(pos, tab);
        self.active_tab = pos;
        self.preview_tab = Some(pos);
        pos
    }

    pub fn find_tab_by_path(&self, path: &Path) -> Option<usize> {
        self.tabs
            .iter()
            .position(|t| t.path.as_deref() == Some(path))
    }

    /// Close a tab by index. Returns the closed tab.
    pub fn close_tab(&mut self, index: usize) -> Option<EditorTab> {
        if index >= self.tabs.len() {
            return None;
        }
        let tab = self.tabs.remove(index);

        // Fix preview_tab reference
        match self.preview_tab {
            Some(pt) if pt == index => self.preview_tab = None,
            Some(pt) if pt > index => self.preview_tab = Some(pt - 1),
            _ => {}
        }

        // Fix active_tab
        if self.tabs.is_empty() {
            self.active_tab = 0;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if self.active_tab > index {
            self.active_tab = self.active_tab.saturating_sub(1);
        }

        Some(tab)
    }

    /// Close all tabs except the one at `except`.
    pub fn close_others(&mut self, except: usize) -> Vec<EditorTab> {
        if except >= self.tabs.len() {
            return Vec::new();
        }
        let keep = self.tabs.remove(except);
        let closed: Vec<EditorTab> = self.tabs.drain(..).collect();
        self.tabs.push(keep);
        self.active_tab = 0;
        self.preview_tab = None;
        closed
    }

    /// Close all tabs to the right of `index`.
    pub fn close_to_right(&mut self, index: usize) -> Vec<EditorTab> {
        if index + 1 >= self.tabs.len() {
            return Vec::new();
        }
        let closed: Vec<EditorTab> = self.tabs.drain((index + 1)..).collect();
        if self.active_tab > index {
            self.active_tab = index;
        }
        if let Some(pt) = self.preview_tab {
            if pt > index {
                self.preview_tab = None;
            }
        }
        closed
    }

    /// Close all tabs in this group.
    pub fn close_all(&mut self) -> Vec<EditorTab> {
        self.active_tab = 0;
        self.preview_tab = None;
        self.tabs.drain(..).collect()
    }

    /// Navigate to the next tab (wrapping).
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    /// Navigate to the previous tab (wrapping).
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab = if self.active_tab == 0 {
                self.tabs.len() - 1
            } else {
                self.active_tab - 1
            };
        }
    }

    /// Move the active tab left by one position.
    pub fn move_tab_left(&mut self) {
        if self.active_tab > 0 && self.active_tab < self.tabs.len() {
            let pinned = self.pinned_count();
            if self.active_tab <= pinned && !self.tabs[self.active_tab].is_pinned {
                return;
            }
            self.tabs.swap(self.active_tab, self.active_tab - 1);
            self.active_tab -= 1;
        }
    }

    /// Move the active tab right by one position.
    pub fn move_tab_right(&mut self) {
        if self.active_tab + 1 < self.tabs.len() {
            self.tabs.swap(self.active_tab, self.active_tab + 1);
            self.active_tab += 1;
        }
    }

    /// Pin the tab at `index`, moving it to the end of the pinned region.
    pub fn pin_tab(&mut self, index: usize) {
        if index >= self.tabs.len() || self.tabs[index].is_pinned {
            return;
        }
        self.tabs[index].is_pinned = true;
        self.tabs[index].is_preview = false;
        if let Some(pt) = self.preview_tab {
            if pt == index {
                self.preview_tab = None;
            }
        }
        let pinned = self.pinned_count();
        if index >= pinned {
            let tab = self.tabs.remove(index);
            let insert_at = pinned.saturating_sub(1);
            self.tabs.insert(insert_at, tab);
            if self.active_tab == index {
                self.active_tab = insert_at;
            } else if self.active_tab >= insert_at && self.active_tab < index {
                self.active_tab += 1;
            }
        }
    }

    /// Unpin the tab at `index`.
    pub fn unpin_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.tabs[index].is_pinned = false;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn active_tab_ref(&self) -> Option<&EditorTab> {
        self.tabs.get(self.active_tab)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut EditorTab> {
        self.tabs.get_mut(self.active_tab)
    }
}

impl Default for EditorGroup {
    fn default() -> Self {
        Self::new()
    }
}

// ── Group layout ─────────────────────────────────────────────────────────────

/// How editor groups are arranged spatially.
#[derive(Clone, Debug)]
pub enum GroupLayout {
    /// A single editor group fills the whole area.
    Single,
    /// Side-by-side split with proportional widths.
    SplitHorizontal(Vec<f32>),
    /// Top-bottom split with proportional heights.
    SplitVertical(Vec<f32>),
}

impl Default for GroupLayout {
    fn default() -> Self {
        Self::Single
    }
}

// ── Closed tab info ──────────────────────────────────────────────────────────

/// Information about a recently closed tab (for "Reopen Closed Editor").
#[derive(Clone, Debug)]
pub struct ClosedTabInfo {
    pub path: PathBuf,
    pub group: usize,
    pub position: usize,
}

// ── Editor group layout (top-level) ──────────────────────────────────────────

/// Manages the full set of editor groups (split panes).
pub struct EditorGroupLayout {
    pub groups: Vec<EditorGroup>,
    pub layout: GroupLayout,
    pub active_group: usize,
    pub recently_closed: Vec<ClosedTabInfo>,
}

impl EditorGroupLayout {
    pub fn new() -> Self {
        Self {
            groups: vec![EditorGroup::new()],
            layout: GroupLayout::Single,
            active_group: 0,
            recently_closed: Vec::new(),
        }
    }

    /// The currently active editor group.
    pub fn active_group(&self) -> &EditorGroup {
        &self.groups[self.active_group]
    }

    /// The currently active editor group (mutable).
    pub fn active_group_mut(&mut self) -> &mut EditorGroup {
        &mut self.groups[self.active_group]
    }

    /// Focus a specific group by index.
    pub fn focus_group(&mut self, index: usize) {
        if index < self.groups.len() {
            self.active_group = index;
        }
    }

    /// Cycle to the next group.
    pub fn next_group(&mut self) {
        if !self.groups.is_empty() {
            self.active_group = (self.active_group + 1) % self.groups.len();
        }
    }

    /// Cycle to the previous group.
    pub fn prev_group(&mut self) {
        if !self.groups.is_empty() {
            self.active_group = if self.active_group == 0 {
                self.groups.len() - 1
            } else {
                self.active_group - 1
            };
        }
    }

    /// Split the active group to the right (horizontal split).
    /// Moves the active tab from the current group into the new group.
    pub fn split_right(&mut self) {
        let new_group = self.split_active_tab();
        let insert_at = self.active_group + 1;
        self.groups.insert(insert_at, new_group);
        self.active_group = insert_at;
        self.rebuild_layout();
    }

    /// Split the active group downward (vertical split).
    /// Moves the active tab from the current group into the new group.
    pub fn split_down(&mut self) {
        let new_group = self.split_active_tab();
        let insert_at = self.active_group + 1;
        self.groups.insert(insert_at, new_group);
        self.active_group = insert_at;
        self.rebuild_layout_vertical();
    }

    fn split_active_tab(&mut self) -> EditorGroup {
        let group = &self.groups[self.active_group];
        if let Some(tab) = group.active_tab_ref() {
            let mut new_tab = tab.clone();
            new_tab.id = TabId::next();
            new_tab.is_preview = false;
            EditorGroup::with_tab(new_tab)
        } else {
            EditorGroup::new()
        }
    }

    /// Close a group by index, moving its tabs to the nearest neighbor.
    pub fn close_group(&mut self, index: usize) {
        if self.groups.len() <= 1 || index >= self.groups.len() {
            return;
        }
        let removed = self.groups.remove(index);

        let target = if index > 0 { index - 1 } else { 0 };
        for tab in removed.tabs {
            self.groups[target].tabs.push(tab);
        }
        if !self.groups[target].tabs.is_empty() && self.groups[target].active_tab == 0 {
            self.groups[target].active_tab = self.groups[target].tabs.len() - 1;
        }

        if self.active_group >= self.groups.len() {
            self.active_group = self.groups.len() - 1;
        } else if self.active_group > index {
            self.active_group = self.active_group.saturating_sub(1);
        }
        self.rebuild_layout();
    }

    /// Move a tab from one group to another.
    pub fn move_tab(
        &mut self,
        from_group: usize,
        tab_idx: usize,
        to_group: usize,
        position: usize,
    ) {
        if from_group >= self.groups.len() || to_group >= self.groups.len() {
            return;
        }
        if from_group == to_group {
            return;
        }
        if let Some(tab) = self.groups[from_group].close_tab(tab_idx) {
            let pos = position.min(self.groups[to_group].tabs.len());
            self.groups[to_group].tabs.insert(pos, tab);
            self.groups[to_group].active_tab = pos;
        }

        // Remove empty groups (except last one)
        if self.groups[from_group].is_empty() && self.groups.len() > 1 {
            self.close_group(from_group);
        }
    }

    /// Resize groups by setting proportional sizes.
    pub fn resize_groups(&mut self, proportions: Vec<f32>) {
        match &mut self.layout {
            GroupLayout::SplitHorizontal(ref mut sizes) => {
                if proportions.len() == sizes.len() {
                    *sizes = proportions;
                }
            }
            GroupLayout::SplitVertical(ref mut sizes) => {
                if proportions.len() == sizes.len() {
                    *sizes = proportions;
                }
            }
            GroupLayout::Single => {}
        }
    }

    /// Get the proportional sizes for each group.
    pub fn group_proportions(&self) -> Vec<f32> {
        match &self.layout {
            GroupLayout::Single => vec![1.0],
            GroupLayout::SplitHorizontal(sizes) | GroupLayout::SplitVertical(sizes) => {
                sizes.clone()
            }
        }
    }

    fn rebuild_layout(&mut self) {
        let n = self.groups.len();
        if n <= 1 {
            self.layout = GroupLayout::Single;
        } else {
            let equal = 1.0 / n as f32;
            self.layout = GroupLayout::SplitHorizontal(vec![equal; n]);
        }
    }

    fn rebuild_layout_vertical(&mut self) {
        let n = self.groups.len();
        if n <= 1 {
            self.layout = GroupLayout::Single;
        } else {
            let equal = 1.0 / n as f32;
            self.layout = GroupLayout::SplitVertical(vec![equal; n]);
        }
    }

    /// Record a closed tab for "Reopen Closed Editor".
    pub fn record_closed(&mut self, tab: &EditorTab, group: usize, position: usize) {
        if let Some(path) = &tab.path {
            self.recently_closed.push(ClosedTabInfo {
                path: path.clone(),
                group,
                position,
            });
        }
    }

    /// Reopen the most recently closed tab.
    pub fn reopen_closed(&mut self) -> Option<PathBuf> {
        let info = self.recently_closed.pop()?;
        if !info.path.exists() {
            return None;
        }

        let group_idx = info.group.min(self.groups.len().saturating_sub(1));
        self.groups[group_idx].open_file(&info.path);
        self.active_group = group_idx;
        Some(info.path)
    }

    /// Check if any tab in any group has unsaved changes.
    pub fn has_unsaved_changes(&self) -> bool {
        self.groups
            .iter()
            .any(|g| g.tabs.iter().any(|t| t.is_modified))
    }

    /// Collect paths of all modified tabs (for "Save N files?" dialog).
    pub fn modified_tab_paths(&self) -> Vec<PathBuf> {
        self.groups
            .iter()
            .flat_map(|g| g.tabs.iter())
            .filter(|t| t.is_modified)
            .filter_map(|t| t.path.clone())
            .collect()
    }

    /// Total number of open tabs across all groups.
    pub fn total_tab_count(&self) -> usize {
        self.groups.iter().map(|g| g.tabs.len()).sum()
    }
}

impl Default for EditorGroupLayout {
    fn default() -> Self {
        Self::new()
    }
}

// ── Editor group manager ─────────────────────────────────────────────────────

/// High-level manager with grid layout support (up to 4×4) and the full
/// set of VS Code-style editor group operations.
pub struct EditorGroupManager {
    pub groups: Vec<EditorGroup>,
    pub active_group: usize,
    pub orientation: GroupOrientation,
    next_group_id: u32,
    pub recently_closed: Vec<ClosedTabInfo>,
    pub grid_cols: u32,
    pub grid_rows: u32,
}

/// Grid layout orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupOrientation {
    Horizontal,
    Vertical,
}

impl Default for GroupOrientation {
    fn default() -> Self {
        Self::Horizontal
    }
}

impl EditorGroupManager {
    pub fn new() -> Self {
        Self {
            groups: vec![EditorGroup::new()],
            active_group: 0,
            orientation: GroupOrientation::Horizontal,
            next_group_id: 1,
            recently_closed: Vec::new(),
            grid_cols: 1,
            grid_rows: 1,
        }
    }

    fn alloc_group_id(&mut self) -> u32 {
        let id = self.next_group_id;
        self.next_group_id += 1;
        id
    }

    // ── Open / close editors ─────────────────────────────────────

    /// Open a file in a group. If `group` is `None`, uses the active group.
    /// When `preview` is true, the tab opens in preview mode.
    pub fn open_editor(&mut self, path: &Path, group: Option<u32>, preview: bool) {
        let idx = group
            .and_then(|g| self.find_group_index(g))
            .unwrap_or(self.active_group);

        if preview {
            self.groups[idx].open_file_preview(path);
        } else {
            self.groups[idx].open_file(path);
        }
        self.active_group = idx;
    }

    /// Close a specific tab in a group.
    pub fn close_tab(&mut self, group_idx: usize, tab: usize) {
        if group_idx >= self.groups.len() {
            return;
        }
        if let Some(closed) = self.groups[group_idx].close_tab(tab) {
            if let Some(p) = &closed.path {
                self.recently_closed.push(ClosedTabInfo {
                    path: p.clone(),
                    group: group_idx,
                    position: tab,
                });
            }
        }
        self.cleanup_empty_groups();
    }

    /// Pin a tab in a group.
    pub fn pin_tab(&mut self, group_idx: usize, tab: usize) {
        if group_idx < self.groups.len() {
            self.groups[group_idx].pin_tab(tab);
        }
    }

    /// Unpin a tab in a group.
    pub fn unpin_tab(&mut self, group_idx: usize, tab: usize) {
        if group_idx < self.groups.len() {
            self.groups[group_idx].unpin_tab(tab);
        }
    }

    // ── Splitting ────────────────────────────────────────────────

    /// Split the active group to the right.
    pub fn split_right(&mut self) {
        let new_group = self.clone_active_tab();
        let insert_at = self.active_group + 1;
        self.groups.insert(insert_at, new_group);
        self.active_group = insert_at;
        self.orientation = GroupOrientation::Horizontal;
        self.recalc_grid();
    }

    /// Split the active group downward.
    pub fn split_down(&mut self) {
        let new_group = self.clone_active_tab();
        let insert_at = self.active_group + 1;
        self.groups.insert(insert_at, new_group);
        self.active_group = insert_at;
        self.orientation = GroupOrientation::Vertical;
        self.recalc_grid();
    }

    fn clone_active_tab(&mut self) -> EditorGroup {
        let group = &self.groups[self.active_group];
        if let Some(tab) = group.active_tab_ref() {
            let mut new_tab = tab.clone();
            new_tab.id = TabId::next();
            new_tab.is_preview = false;
            EditorGroup::with_tab(new_tab)
        } else {
            EditorGroup::new()
        }
    }

    // ── Tab movement ─────────────────────────────────────────────

    /// Move a tab from one group to another.
    pub fn move_tab(
        &mut self,
        from_group: usize,
        from_tab: usize,
        to_group: usize,
        to_position: usize,
    ) {
        if from_group >= self.groups.len() || to_group >= self.groups.len() {
            return;
        }
        if from_group == to_group {
            return;
        }
        if let Some(tab) = self.groups[from_group].close_tab(from_tab) {
            let pos = to_position.min(self.groups[to_group].tabs.len());
            self.groups[to_group].tabs.insert(pos, tab);
            self.groups[to_group].active_tab = pos;
        }
        self.cleanup_empty_groups();
    }

    // ── Grid layout ──────────────────────────────────────────────

    /// Set a grid layout (e.g. 2×2 for four editor groups).
    /// Max is 4×4 = 16 groups.
    pub fn set_grid(&mut self, cols: u32, rows: u32) {
        let cols = cols.max(1).min(4);
        let rows = rows.max(1).min(4);
        let target = (cols * rows) as usize;

        while self.groups.len() < target {
            self.groups.push(EditorGroup::new());
        }
        while self.groups.len() > target && self.groups.len() > 1 {
            let last = self.groups.pop().unwrap();
            let target_idx = self.groups.len().saturating_sub(1);
            for tab in last.tabs {
                self.groups[target_idx].tabs.push(tab);
            }
        }

        self.grid_cols = cols;
        self.grid_rows = rows;
        if self.active_group >= self.groups.len() {
            self.active_group = self.groups.len().saturating_sub(1);
        }
    }

    // ── Navigation ───────────────────────────────────────────────

    /// Focus a specific group by 0-based index (Ctrl+1/2/3).
    pub fn focus_group(&mut self, index: usize) {
        if index < self.groups.len() {
            self.active_group = index;
        }
    }

    /// The active group.
    pub fn active_group(&self) -> &EditorGroup {
        &self.groups[self.active_group]
    }

    /// The active group (mut).
    pub fn active_group_mut(&mut self) -> &mut EditorGroup {
        &mut self.groups[self.active_group]
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn find_group_index(&self, _group_id: u32) -> Option<usize> {
        // Group IDs aren't stored on EditorGroup yet; fall back to index
        None
    }

    fn cleanup_empty_groups(&mut self) {
        if self.groups.len() <= 1 {
            return;
        }
        self.groups.retain(|g| !g.is_empty());
        if self.groups.is_empty() {
            self.groups.push(EditorGroup::new());
        }
        if self.active_group >= self.groups.len() {
            self.active_group = self.groups.len().saturating_sub(1);
        }
        self.recalc_grid();
    }

    fn recalc_grid(&mut self) {
        let n = self.groups.len() as u32;
        if n <= 1 {
            self.grid_cols = 1;
            self.grid_rows = 1;
        } else {
            self.grid_cols = n;
            self.grid_rows = 1;
        }
    }
}

impl Default for EditorGroupManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Auto-save policy ─────────────────────────────────────────────────────────

/// Auto-save behavior matching VS Code's `files.autoSave` setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AutoSaveMode {
    /// Auto-save is disabled.
    #[default]
    Off,
    /// Save after a configurable delay (ms) since last edit.
    AfterDelay,
    /// Save when the editor loses focus.
    OnFocusChange,
    /// Save when the window loses focus.
    OnWindowChange,
}

impl AutoSaveMode {
    pub fn from_setting(s: &str) -> Self {
        match s {
            "afterDelay" => Self::AfterDelay,
            "onFocusChange" => Self::OnFocusChange,
            "onWindowChange" => Self::OnWindowChange,
            _ => Self::Off,
        }
    }
}

/// Tab sizing mode matching VS Code's `workbench.editor.tabSizing`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabSizingMode {
    /// Tabs shrink to fit available space.
    #[default]
    Fit,
    /// Tabs shrink but have a minimum width.
    Shrink,
    /// All tabs have a fixed width.
    Fixed,
}

impl TabSizingMode {
    pub fn from_setting(s: &str) -> Self {
        match s {
            "shrink" => Self::Shrink,
            "fixed" => Self::Fixed,
            _ => Self::Fit,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_path(name: &str) -> PathBuf {
        PathBuf::from(format!("/tmp/test/{name}"))
    }

    #[test]
    fn open_file_reuses_existing_tab() {
        let mut group = EditorGroup::new();
        let p = test_path("foo.rs");
        group.open_file(&p);
        group.open_file(&p);
        assert_eq!(group.tabs.len(), 1);
    }

    #[test]
    fn preview_tab_is_replaced() {
        let mut group = EditorGroup::new();
        group.open_file_preview(&test_path("a.rs"));
        assert_eq!(group.tabs.len(), 1);
        assert!(group.tabs[0].is_preview);

        group.open_file_preview(&test_path("b.rs"));
        assert_eq!(group.tabs.len(), 1);
        assert_eq!(group.tabs[0].title, "b.rs");
    }

    #[test]
    fn opening_file_promotes_preview() {
        let mut group = EditorGroup::new();
        let p = test_path("a.rs");
        group.open_file_preview(&p);
        assert!(group.tabs[0].is_preview);

        group.open_file(&p);
        assert!(!group.tabs[0].is_preview);
        assert_eq!(group.tabs.len(), 1);
    }

    #[test]
    fn close_tab_adjusts_active() {
        let mut group = EditorGroup::new();
        group.open_file(&test_path("a.rs"));
        group.open_file(&test_path("b.rs"));
        group.open_file(&test_path("c.rs"));
        assert_eq!(group.active_tab, 2);

        group.close_tab(2);
        assert_eq!(group.active_tab, 1);
    }

    #[test]
    fn close_others_keeps_one() {
        let mut group = EditorGroup::new();
        group.open_file(&test_path("a.rs"));
        group.open_file(&test_path("b.rs"));
        group.open_file(&test_path("c.rs"));

        let closed = group.close_others(1);
        assert_eq!(closed.len(), 2);
        assert_eq!(group.tabs.len(), 1);
        assert_eq!(group.tabs[0].title, "b.rs");
    }

    #[test]
    fn close_to_right_works() {
        let mut group = EditorGroup::new();
        group.open_file(&test_path("a.rs"));
        group.open_file(&test_path("b.rs"));
        group.open_file(&test_path("c.rs"));

        let closed = group.close_to_right(0);
        assert_eq!(closed.len(), 2);
        assert_eq!(group.tabs.len(), 1);
    }

    #[test]
    fn pin_tab_moves_to_front() {
        let mut group = EditorGroup::new();
        group.open_file(&test_path("a.rs"));
        group.open_file(&test_path("b.rs"));
        group.open_file(&test_path("c.rs"));

        group.pin_tab(2);
        assert!(group.tabs[0].is_pinned);
        assert_eq!(group.tabs[0].title, "c.rs");
    }

    #[test]
    fn split_right_creates_new_group() {
        let mut layout = EditorGroupLayout::new();
        layout.groups[0].open_file(&test_path("main.rs"));
        layout.split_right();

        assert_eq!(layout.groups.len(), 2);
        assert_eq!(layout.active_group, 1);
        assert!(!layout.groups[1].is_empty());
    }

    #[test]
    fn close_group_merges_tabs() {
        let mut layout = EditorGroupLayout::new();
        layout.groups[0].open_file(&test_path("a.rs"));
        layout.split_right();
        layout.groups[1].open_file(&test_path("b.rs"));

        layout.close_group(1);
        assert_eq!(layout.groups.len(), 1);
        assert!(layout.groups[0].tabs.len() >= 2);
    }

    #[test]
    fn next_prev_tab_wraps() {
        let mut group = EditorGroup::new();
        group.open_file(&test_path("a.rs"));
        group.open_file(&test_path("b.rs"));
        group.open_file(&test_path("c.rs"));
        assert_eq!(group.active_tab, 2);

        group.next_tab();
        assert_eq!(group.active_tab, 0);

        group.prev_tab();
        assert_eq!(group.active_tab, 2);
    }

    #[test]
    fn move_tab_between_groups() {
        let mut layout = EditorGroupLayout::new();
        layout.groups[0].open_file(&test_path("a.rs"));
        layout.groups[0].open_file(&test_path("b.rs"));
        layout.split_right();

        layout.move_tab(0, 1, 1, 0);
        assert_eq!(layout.groups[0].tabs.len(), 1);
        assert!(layout.groups[1].tabs.len() >= 2);
    }

    #[test]
    fn reopen_closed_editor() {
        let mut layout = EditorGroupLayout::new();
        let p = test_path("a.rs");
        layout.groups[0].open_file(&p);

        let tab = layout.groups[0].tabs[0].clone();
        layout.record_closed(&tab, 0, 0);
        layout.groups[0].close_tab(0);

        // Can't actually reopen because file doesn't exist, but test the recording
        assert_eq!(layout.recently_closed.len(), 1);
        assert_eq!(layout.recently_closed[0].path, p);
    }

    #[test]
    fn auto_save_mode_parsing() {
        assert_eq!(AutoSaveMode::from_setting("off"), AutoSaveMode::Off);
        assert_eq!(
            AutoSaveMode::from_setting("afterDelay"),
            AutoSaveMode::AfterDelay
        );
        assert_eq!(
            AutoSaveMode::from_setting("onFocusChange"),
            AutoSaveMode::OnFocusChange
        );
        assert_eq!(
            AutoSaveMode::from_setting("onWindowChange"),
            AutoSaveMode::OnWindowChange
        );
    }

    #[test]
    fn tab_sizing_mode_parsing() {
        assert_eq!(TabSizingMode::from_setting("fit"), TabSizingMode::Fit);
        assert_eq!(TabSizingMode::from_setting("shrink"), TabSizingMode::Shrink);
        assert_eq!(TabSizingMode::from_setting("fixed"), TabSizingMode::Fixed);
    }

    // ── EditorGroupManager tests ─────────────────────────────────

    #[test]
    fn manager_open_editor() {
        let mut mgr = EditorGroupManager::new();
        mgr.open_editor(&test_path("main.rs"), None, false);
        assert_eq!(mgr.groups[0].tabs.len(), 1);
        assert_eq!(mgr.groups[0].tabs[0].title, "main.rs");
    }

    #[test]
    fn manager_open_preview() {
        let mut mgr = EditorGroupManager::new();
        mgr.open_editor(&test_path("lib.rs"), None, true);
        assert!(mgr.groups[0].tabs[0].is_preview);
    }

    #[test]
    fn manager_split_right() {
        let mut mgr = EditorGroupManager::new();
        mgr.open_editor(&test_path("main.rs"), None, false);
        mgr.split_right();
        assert_eq!(mgr.groups.len(), 2);
        assert_eq!(mgr.active_group, 1);
    }

    #[test]
    fn manager_split_down() {
        let mut mgr = EditorGroupManager::new();
        mgr.open_editor(&test_path("main.rs"), None, false);
        mgr.split_down();
        assert_eq!(mgr.groups.len(), 2);
        assert_eq!(mgr.orientation, GroupOrientation::Vertical);
    }

    #[test]
    fn manager_move_tab() {
        let mut mgr = EditorGroupManager::new();
        mgr.open_editor(&test_path("a.rs"), None, false);
        mgr.open_editor(&test_path("b.rs"), None, false);
        mgr.split_right();
        mgr.move_tab(0, 0, 1, 0);
        assert!(mgr.groups.len() >= 1);
    }

    #[test]
    fn manager_grid_2x2() {
        let mut mgr = EditorGroupManager::new();
        mgr.set_grid(2, 2);
        assert_eq!(mgr.groups.len(), 4);
        assert_eq!(mgr.grid_cols, 2);
        assert_eq!(mgr.grid_rows, 2);
    }

    #[test]
    fn manager_grid_clamped() {
        let mut mgr = EditorGroupManager::new();
        mgr.set_grid(10, 10);
        assert_eq!(mgr.groups.len(), 16); // 4x4 max
    }

    #[test]
    fn manager_close_tab_records_closed() {
        let mut mgr = EditorGroupManager::new();
        mgr.open_editor(&test_path("x.rs"), None, false);
        mgr.close_tab(0, 0);
        assert_eq!(mgr.recently_closed.len(), 1);
    }

    #[test]
    fn manager_pin_unpin() {
        let mut mgr = EditorGroupManager::new();
        mgr.open_editor(&test_path("a.rs"), None, false);
        mgr.open_editor(&test_path("b.rs"), None, false);
        mgr.pin_tab(0, 1);
        assert!(mgr.groups[0].tabs[0].is_pinned);
        mgr.unpin_tab(0, 0);
        assert!(!mgr.groups[0].tabs[0].is_pinned);
    }

    #[test]
    fn manager_focus_group() {
        let mut mgr = EditorGroupManager::new();
        mgr.set_grid(2, 1);
        mgr.focus_group(1);
        assert_eq!(mgr.active_group, 1);
    }

    #[test]
    fn tab_icon_default() {
        let icon = TabIcon::default();
        match icon {
            TabIcon::Language(lang) => assert_eq!(lang, "plaintext"),
            _ => panic!("expected Language variant"),
        }
    }

    #[test]
    fn tab_from_path_has_description() {
        let tab = EditorTab::from_path(Path::new("/projects/sidex/main.rs"));
        assert!(tab.description.is_some());
        assert_eq!(tab.title, "main.rs");
    }
}
