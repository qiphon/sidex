//! Peek view — inline "peek definition / peek references" widget, mirrors
//! VS Code's `PeekViewWidget` + `PeekViewZoneWidget`.
//!
//! Shows a mini-editor inline below the current line with a list of locations
//! (definitions, references, implementations) and a preview of the selected one.

use sidex_text::{Position, Range};

/// A single location (file + range) that the peek view can show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Resource identifier (file path or URI).
    pub uri: String,
    /// The range within the resource.
    pub range: Range,
}

impl Location {
    #[must_use]
    pub fn new(uri: impl Into<String>, range: Range) -> Self {
        Self {
            uri: uri.into(),
            range,
        }
    }
}

/// Full state for the inline peek view widget.
#[derive(Debug, Clone)]
pub struct PeekViewState {
    /// Whether the peek view is currently open.
    pub is_visible: bool,
    /// The URI of the resource being previewed.
    pub uri: String,
    /// The range in the active document that triggered the peek.
    pub trigger_range: Range,
    /// All locations to show in the reference list.
    pub items: Vec<Location>,
    /// Index of the currently selected location.
    pub selected: usize,
    /// The line in the host editor below which the peek zone appears.
    pub anchor_line: u32,
    /// Height of the peek zone in lines.
    pub height_in_lines: u32,
}

impl Default for PeekViewState {
    fn default() -> Self {
        Self {
            is_visible: false,
            uri: String::new(),
            trigger_range: Range::new(Position::ZERO, Position::ZERO),
            items: Vec::new(),
            selected: 0,
            anchor_line: 0,
            height_in_lines: 12,
        }
    }
}

impl PeekViewState {
    /// Opens the peek view with the given locations, anchored below `anchor_line`.
    pub fn open(&mut self, anchor_line: u32, locations: Vec<Location>) {
        if locations.is_empty() {
            return;
        }
        self.anchor_line = anchor_line;
        self.items = locations;
        self.selected = 0;
        self.uri = self.items[0].uri.clone();
        self.trigger_range = self.items[0].range;
        self.is_visible = true;
    }

    /// Closes the peek view and resets state.
    pub fn close(&mut self) {
        self.is_visible = false;
        self.items.clear();
        self.selected = 0;
        self.uri.clear();
    }

    /// Selects the next location in the list, wrapping around.
    pub fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
        self.sync_selection();
    }

    /// Selects the previous location in the list, wrapping around.
    pub fn prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.items.len() - 1
        } else {
            self.selected - 1
        };
        self.sync_selection();
    }

    /// Selects a location by index.
    pub fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected = index;
            self.sync_selection();
        }
    }

    /// Returns the currently selected location, if any.
    #[must_use]
    pub fn current_location(&self) -> Option<&Location> {
        self.items.get(self.selected)
    }

    /// Returns `(selected_1based, total)` for status display.
    #[must_use]
    pub fn count_display(&self) -> (usize, usize) {
        (
            if self.items.is_empty() {
                0
            } else {
                self.selected + 1
            },
            self.items.len(),
        )
    }

    /// Sets the height of the peek zone in lines.
    pub fn set_height(&mut self, lines: u32) {
        self.height_in_lines = lines;
    }

    fn sync_selection(&mut self) {
        if let Some(loc) = self.items.get(self.selected) {
            self.uri.clone_from(&loc.uri);
            self.trigger_range = loc.range;
        }
    }
}

// ── Peek entry (enriched location) ───────────────────────────────

/// An enriched location entry for the peek side panel, carrying display-ready
/// metadata beyond what [`Location`] provides.
#[derive(Debug, Clone)]
pub struct PeekEntry {
    /// Resource identifier (file path or URI).
    pub uri: String,
    /// Short display filename (last path component).
    pub file_name: String,
    /// Containing directory, for display below the filename.
    pub directory: String,
    /// The target range within the resource.
    pub range: Range,
    /// Preview lines around the match for rendering in the side panel.
    pub preview_lines: Vec<String>,
    /// Number of matches within this file (for the badge).
    pub match_count: u32,
}

impl PeekEntry {
    /// Creates a `PeekEntry` from a URI, range, and optional preview lines.
    #[must_use]
    pub fn new(uri: impl Into<String>, range: Range, preview_lines: Vec<String>) -> Self {
        let uri = uri.into();
        let (dir, file) = split_uri_parts(&uri);
        Self {
            uri,
            file_name: file,
            directory: dir,
            range,
            preview_lines,
            match_count: 1,
        }
    }

    /// Converts this entry into a plain [`Location`].
    #[must_use]
    pub fn to_location(&self) -> Location {
        Location::new(self.uri.clone(), self.range)
    }
}

fn split_uri_parts(uri: &str) -> (String, String) {
    let sep = if uri.contains('/') { '/' } else { '\\' };
    if let Some(pos) = uri.rfind(sep) {
        let dir = uri[..pos].to_string();
        let file = uri[pos + 1..].to_string();
        (dir, file)
    } else {
        (String::new(), uri.to_string())
    }
}

// ── Embedded editor state ────────────────────────────────────────

/// State for the read-only mini-editor embedded inside the peek zone.
#[derive(Debug, Clone, Default)]
pub struct PeekEmbeddedEditor {
    /// The text content loaded in the embedded editor.
    pub content: String,
    /// The language identifier for syntax highlighting.
    pub language: String,
    /// Current vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// The highlighted range (the definition/reference itself).
    pub highlight_range: Option<Range>,
    /// Whether the embedded editor is read-only (default `true`).
    pub read_only: bool,
}

impl PeekEmbeddedEditor {
    /// Loads content into the embedded editor.
    pub fn load(&mut self, content: String, language: String, highlight: Option<Range>) {
        self.content = content;
        self.language = language;
        self.highlight_range = highlight;
        self.scroll_y = 0.0;
        self.read_only = true;
    }

    /// Clears the embedded editor content.
    pub fn clear(&mut self) {
        self.content.clear();
        self.language.clear();
        self.scroll_y = 0.0;
        self.highlight_range = None;
    }
}

// ── Peek resize state ────────────────────────────────────────────

/// Tracks drag-resize state for the peek zone border.
#[derive(Debug, Clone, Copy, Default)]
pub struct PeekResizeState {
    /// Whether the user is currently dragging the resize border.
    pub is_dragging: bool,
    /// Y position where the drag started.
    pub drag_start_y: f32,
    /// Height at drag start.
    pub height_at_start: f32,
}

// ── Peek view mode ───────────────────────────────────────────────

/// Which flavour of peek is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeekMode {
    /// Peek Definition (Alt+F12) — show definition inline.
    Definition,
    /// Peek References — show all references with a side list.
    References,
    /// Peek Implementation — show implementations inline.
    Implementation,
    /// Peek Type Definition.
    TypeDefinition,
}

// ── Breadcrumb ───────────────────────────────────────────────────

/// File-path breadcrumb displayed at the top of the peek zone.
#[derive(Debug, Clone)]
pub struct PeekBreadcrumb {
    /// Full URI / path.
    pub uri: String,
    /// Short filename (last component).
    pub filename: String,
    /// Relative path for display.
    pub relative_path: String,
    /// Target line shown in the breadcrumb (1-based).
    pub line_display: u32,
}

impl PeekBreadcrumb {
    /// Build a breadcrumb from a URI and a line number (0-based).
    #[must_use]
    pub fn from_uri(uri: &str, line_0based: u32) -> Self {
        let filename = uri
            .rsplit('/')
            .next()
            .or_else(|| uri.rsplit('\\').next())
            .unwrap_or(uri)
            .to_string();
        Self {
            uri: uri.to_string(),
            filename,
            relative_path: uri.to_string(),
            line_display: line_0based + 1,
        }
    }
}

// ── Reference list (left panel in peek references) ───────────────

/// A file group in the reference list, with its reference count.
#[derive(Debug, Clone)]
pub struct ReferenceFileGroup {
    /// The file URI.
    pub uri: String,
    /// Short display name.
    pub filename: String,
    /// Individual references within this file.
    pub references: Vec<ReferenceItem>,
}

/// A single reference within a file.
#[derive(Debug, Clone)]
pub struct ReferenceItem {
    /// The range of the reference.
    pub range: Range,
    /// Preview text for this line.
    pub line_preview: String,
    /// 0-based line number.
    pub line: u32,
}

/// Build reference file groups from a flat list of locations.
///
/// Groups locations by URI for the left-panel display in peek references.
pub fn group_references(locations: &[Location]) -> Vec<ReferenceFileGroup> {
    let mut groups: Vec<ReferenceFileGroup> = Vec::new();

    for loc in locations {
        let filename = loc
            .uri
            .rsplit('/')
            .next()
            .or_else(|| loc.uri.rsplit('\\').next())
            .unwrap_or(&loc.uri)
            .to_string();

        let item = ReferenceItem {
            range: loc.range,
            line_preview: String::new(),
            line: loc.range.start.line,
        };

        if let Some(group) = groups.iter_mut().find(|g| g.uri == loc.uri) {
            group.references.push(item);
        } else {
            groups.push(ReferenceFileGroup {
                uri: loc.uri.clone(),
                filename,
                references: vec![item],
            });
        }
    }
    groups
}

// ── Mini-editor state for the peek preview ───────────────────────

/// Scroll state for the mini-editor preview inside the peek zone.
#[derive(Debug, Clone, Copy)]
pub struct PeekPreviewScroll {
    /// First visible line in the preview (0-based).
    pub first_line: u32,
    /// Number of visible lines.
    pub visible_lines: u32,
}

impl Default for PeekPreviewScroll {
    fn default() -> Self {
        Self {
            first_line: 0,
            visible_lines: 10,
        }
    }
}

impl PeekPreviewScroll {
    /// Center the preview around a target line.
    pub fn center_on(&mut self, target_line: u32, total_lines: u32) {
        let half = self.visible_lines / 2;
        let start = target_line.saturating_sub(half);
        let max_start = total_lines.saturating_sub(self.visible_lines);
        self.first_line = start.min(max_start);
    }

    /// Scroll the preview up by one line.
    pub fn scroll_up(&mut self) {
        self.first_line = self.first_line.saturating_sub(1);
    }

    /// Scroll the preview down by one line.
    pub fn scroll_down(&mut self, total_lines: u32) {
        let max = total_lines.saturating_sub(self.visible_lines);
        if self.first_line < max {
            self.first_line += 1;
        }
    }
}

// ── Extended peek controller ─────────────────────────────────────

/// Full peek controller that wraps `PeekViewState` with mode tracking,
/// breadcrumb, reference groups, and preview scroll.
#[derive(Debug, Clone)]
pub struct PeekController {
    /// Core peek state.
    pub state: PeekViewState,
    /// Which peek mode is active.
    pub mode: PeekMode,
    /// Breadcrumb for the currently previewed file.
    pub breadcrumb: Option<PeekBreadcrumb>,
    /// Grouped references (populated in References mode).
    pub reference_groups: Vec<ReferenceFileGroup>,
    /// Selected file group index (for references left panel).
    pub selected_group: usize,
    /// Selected reference within the active group.
    pub selected_ref_in_group: usize,
    /// Preview scroll state.
    pub preview_scroll: PeekPreviewScroll,
    /// Enriched entries (when provided via `show_peek`).
    pub entries: Vec<PeekEntry>,
    /// Embedded editor state for the preview pane.
    pub embedded_editor: PeekEmbeddedEditor,
    /// Width of the side panel in pixels (resizable).
    pub side_panel_width: f32,
    /// Drag-resize state for the peek zone height.
    pub resize: PeekResizeState,
    /// Title displayed in the blue header bar.
    pub title: String,
}

impl Default for PeekController {
    fn default() -> Self {
        Self {
            state: PeekViewState::default(),
            mode: PeekMode::Definition,
            breadcrumb: None,
            reference_groups: Vec::new(),
            selected_group: 0,
            selected_ref_in_group: 0,
            preview_scroll: PeekPreviewScroll::default(),
            entries: Vec::new(),
            embedded_editor: PeekEmbeddedEditor::default(),
            side_panel_width: 240.0,
            resize: PeekResizeState::default(),
            title: String::new(),
        }
    }
}

impl PeekController {
    /// Open a peek-definition view.
    pub fn open_definition(&mut self, anchor_line: u32, locations: Vec<Location>) {
        self.mode = PeekMode::Definition;
        self.state.open(anchor_line, locations);
        self.update_breadcrumb();
        self.reference_groups.clear();
    }

    /// Open a peek-references view.
    pub fn open_references(&mut self, anchor_line: u32, locations: Vec<Location>) {
        self.mode = PeekMode::References;
        self.reference_groups = group_references(&locations);
        self.selected_group = 0;
        self.selected_ref_in_group = 0;
        self.state.open(anchor_line, locations);
        self.update_breadcrumb();
    }

    /// Open a peek-implementation view.
    pub fn open_implementation(&mut self, anchor_line: u32, locations: Vec<Location>) {
        self.mode = PeekMode::Implementation;
        self.state.open(anchor_line, locations);
        self.update_breadcrumb();
        self.reference_groups.clear();
    }

    /// Close the peek view (Escape).
    pub fn close(&mut self) {
        self.state.close();
        self.breadcrumb = None;
        self.reference_groups.clear();
        self.entries.clear();
        self.embedded_editor.clear();
        self.title.clear();
    }

    /// High-level show: opens the peek view with enriched entries.
    pub fn show_peek(
        &mut self,
        anchor_line: u32,
        title: &str,
        mode: PeekMode,
        entries: Vec<PeekEntry>,
    ) {
        if entries.is_empty() {
            return;
        }
        self.title = title.to_string();
        self.mode = mode;
        let locations: Vec<Location> = entries.iter().map(PeekEntry::to_location).collect();
        if mode == PeekMode::References {
            self.reference_groups = group_references(&locations);
            self.selected_group = 0;
            self.selected_ref_in_group = 0;
        } else {
            self.reference_groups.clear();
        }
        self.entries = entries;
        self.state.open(anchor_line, locations);
        self.update_breadcrumb();
        self.load_entry_content(0);
    }

    /// Closes the peek view (alias for `close`).
    pub fn close_peek(&mut self) {
        self.close();
    }

    /// Navigate to the next or previous result by direction.
    pub fn navigate_peek(&mut self, forward: bool) {
        if forward {
            self.next_result();
        } else {
            self.prev_result();
        }
    }

    /// Navigate to the next result.
    pub fn next_result(&mut self) {
        self.state.next();
        self.update_breadcrumb();
        self.load_entry_content(self.state.selected);
    }

    /// Navigate to the previous result.
    pub fn prev_result(&mut self) {
        self.state.prev();
        self.update_breadcrumb();
        self.load_entry_content(self.state.selected);
    }

    /// Select a result by index.
    pub fn select_result(&mut self, index: usize) {
        self.state.select(index);
        self.update_breadcrumb();
        self.load_entry_content(self.state.selected);
    }

    /// Navigate to the next file group (references mode, arrows).
    pub fn next_group(&mut self) {
        if self.reference_groups.is_empty() {
            return;
        }
        self.selected_group = (self.selected_group + 1) % self.reference_groups.len();
        self.selected_ref_in_group = 0;
        self.sync_group_selection();
    }

    /// Navigate to the previous file group.
    pub fn prev_group(&mut self) {
        if self.reference_groups.is_empty() {
            return;
        }
        self.selected_group = if self.selected_group == 0 {
            self.reference_groups.len() - 1
        } else {
            self.selected_group - 1
        };
        self.selected_ref_in_group = 0;
        self.sync_group_selection();
    }

    /// Navigate to the next reference within the current group.
    pub fn next_ref_in_group(&mut self) {
        if let Some(group) = self.reference_groups.get(self.selected_group) {
            if group.references.is_empty() {
                return;
            }
            self.selected_ref_in_group =
                (self.selected_ref_in_group + 1) % group.references.len();
            self.sync_group_selection();
        }
    }

    /// Navigate to the previous reference within the current group.
    pub fn prev_ref_in_group(&mut self) {
        if let Some(group) = self.reference_groups.get(self.selected_group) {
            if group.references.is_empty() {
                return;
            }
            self.selected_ref_in_group = if self.selected_ref_in_group == 0 {
                group.references.len() - 1
            } else {
                self.selected_ref_in_group - 1
            };
            self.sync_group_selection();
        }
    }

    /// Returns the location to open in the full editor (Enter).
    #[must_use]
    pub fn confirm(&self) -> Option<&Location> {
        self.state.current_location()
    }

    /// Returns `true` if the peek view is open.
    pub fn is_visible(&self) -> bool {
        self.state.is_visible
    }

    fn update_breadcrumb(&mut self) {
        if let Some(loc) = self.state.current_location() {
            self.breadcrumb = Some(PeekBreadcrumb::from_uri(&loc.uri, loc.range.start.line));
        } else {
            self.breadcrumb = None;
        }
    }

    /// Begins a height-resize drag.
    pub fn start_resize(&mut self, y: f32) {
        self.resize.is_dragging = true;
        self.resize.drag_start_y = y;
        self.resize.height_at_start = self.state.height_in_lines as f32;
    }

    /// Updates the height during a resize drag. `line_height` converts pixels
    /// to line units.
    pub fn update_resize(&mut self, current_y: f32, line_height: f32) {
        if !self.resize.is_dragging || line_height <= 0.0 {
            return;
        }
        let delta_lines = (current_y - self.resize.drag_start_y) / line_height;
        let new_lines = (self.resize.height_at_start + delta_lines).round();
        let clamped = new_lines.clamp(4.0, 40.0) as u32;
        self.state.set_height(clamped);
    }

    /// Ends the resize drag.
    pub fn end_resize(&mut self) {
        self.resize.is_dragging = false;
    }

    /// Sets the side panel width (for resizing the reference list).
    pub fn set_side_panel_width(&mut self, width: f32) {
        self.side_panel_width = width.clamp(120.0, 600.0);
    }

    /// Loads preview content from the entry at the given index into the
    /// embedded editor.
    fn load_entry_content(&mut self, index: usize) {
        if let Some(entry) = self.entries.get(index) {
            let content = entry.preview_lines.join("\n");
            let language = guess_language_from_uri(&entry.uri);
            self.embedded_editor
                .load(content, language, Some(entry.range));
        }
    }

    fn sync_group_selection(&mut self) {
        if let Some(group) = self.reference_groups.get(self.selected_group) {
            if let Some(ref_item) = group.references.get(self.selected_ref_in_group) {
                let flat_idx = self
                    .state
                    .items
                    .iter()
                    .position(|loc| {
                        loc.uri == group.uri && loc.range.start.line == ref_item.range.start.line
                    })
                    .unwrap_or(0);
                self.state.select(flat_idx);
                self.update_breadcrumb();
            }
        }
    }
}

/// Guesses the language identifier from a file URI/path extension.
fn guess_language_from_uri(uri: &str) -> String {
    let ext = uri
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "sql" => "sql",
        "sh" | "bash" | "zsh" => "shellscript",
        _ => "plaintext",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(uri: &str, line: u32) -> Location {
        Location::new(
            uri,
            Range::new(Position::new(line, 0), Position::new(line, 10)),
        )
    }

    #[test]
    fn open_and_close() {
        let mut state = PeekViewState::default();
        assert!(!state.is_visible);

        state.open(5, vec![loc("a.rs", 10), loc("b.rs", 20)]);
        assert!(state.is_visible);
        assert_eq!(state.items.len(), 2);
        assert_eq!(state.selected, 0);
        assert_eq!(state.uri, "a.rs");
        assert_eq!(state.anchor_line, 5);

        state.close();
        assert!(!state.is_visible);
        assert!(state.items.is_empty());
    }

    #[test]
    fn open_empty_locations_does_nothing() {
        let mut state = PeekViewState::default();
        state.open(5, vec![]);
        assert!(!state.is_visible);
    }

    #[test]
    fn next_wraps_around() {
        let mut state = PeekViewState::default();
        state.open(0, vec![loc("a.rs", 1), loc("b.rs", 2), loc("c.rs", 3)]);

        assert_eq!(state.selected, 0);
        state.next();
        assert_eq!(state.selected, 1);
        state.next();
        assert_eq!(state.selected, 2);
        state.next();
        assert_eq!(state.selected, 0);
        assert_eq!(state.uri, "a.rs");
    }

    #[test]
    fn prev_wraps_around() {
        let mut state = PeekViewState::default();
        state.open(0, vec![loc("a.rs", 1), loc("b.rs", 2)]);

        assert_eq!(state.selected, 0);
        state.prev();
        assert_eq!(state.selected, 1);
        assert_eq!(state.uri, "b.rs");
        state.prev();
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn select_by_index() {
        let mut state = PeekViewState::default();
        state.open(0, vec![loc("a.rs", 1), loc("b.rs", 2), loc("c.rs", 3)]);
        state.select(2);
        assert_eq!(state.selected, 2);
        assert_eq!(state.uri, "c.rs");

        // out of bounds is a no-op
        state.select(99);
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn count_display() {
        let mut state = PeekViewState::default();
        assert_eq!(state.count_display(), (0, 0));

        state.open(0, vec![loc("a.rs", 1), loc("b.rs", 2)]);
        assert_eq!(state.count_display(), (1, 2));
        state.next();
        assert_eq!(state.count_display(), (2, 2));
    }

    #[test]
    fn current_location() {
        let mut state = PeekViewState::default();
        assert!(state.current_location().is_none());

        state.open(0, vec![loc("a.rs", 1)]);
        let current = state.current_location().unwrap();
        assert_eq!(current.uri, "a.rs");
    }

    // ── Breadcrumb tests ─────────────────────────────────────────

    #[test]
    fn breadcrumb_from_uri() {
        let bc = PeekBreadcrumb::from_uri("src/main.rs", 41);
        assert_eq!(bc.filename, "main.rs");
        assert_eq!(bc.line_display, 42);
    }

    #[test]
    fn breadcrumb_from_uri_no_slash() {
        let bc = PeekBreadcrumb::from_uri("main.rs", 0);
        assert_eq!(bc.filename, "main.rs");
        assert_eq!(bc.line_display, 1);
    }

    // ── Group references tests ───────────────────────────────────

    #[test]
    fn group_references_basic() {
        let locations = vec![
            loc("a.rs", 5),
            loc("a.rs", 10),
            loc("b.rs", 3),
        ];
        let groups = group_references(&locations);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].uri, "a.rs");
        assert_eq!(groups[0].references.len(), 2);
        assert_eq!(groups[1].uri, "b.rs");
        assert_eq!(groups[1].references.len(), 1);
    }

    #[test]
    fn group_references_empty() {
        let groups = group_references(&[]);
        assert!(groups.is_empty());
    }

    // ── Preview scroll tests ─────────────────────────────────────

    #[test]
    fn preview_scroll_center() {
        let mut scroll = PeekPreviewScroll::default();
        scroll.visible_lines = 10;
        scroll.center_on(50, 100);
        assert_eq!(scroll.first_line, 45);
    }

    #[test]
    fn preview_scroll_center_near_start() {
        let mut scroll = PeekPreviewScroll::default();
        scroll.visible_lines = 10;
        scroll.center_on(2, 100);
        assert_eq!(scroll.first_line, 0);
    }

    #[test]
    fn preview_scroll_center_near_end() {
        let mut scroll = PeekPreviewScroll::default();
        scroll.visible_lines = 10;
        scroll.center_on(98, 100);
        assert_eq!(scroll.first_line, 90);
    }

    #[test]
    fn preview_scroll_up_down() {
        let mut scroll = PeekPreviewScroll::default();
        scroll.visible_lines = 10;
        scroll.first_line = 5;
        scroll.scroll_up();
        assert_eq!(scroll.first_line, 4);
        scroll.scroll_down(100);
        assert_eq!(scroll.first_line, 5);
    }

    #[test]
    fn preview_scroll_up_clamped() {
        let mut scroll = PeekPreviewScroll::default();
        scroll.first_line = 0;
        scroll.scroll_up();
        assert_eq!(scroll.first_line, 0);
    }

    // ── Peek controller tests ────────────────────────────────────

    #[test]
    fn controller_open_definition() {
        let mut ctrl = PeekController::default();
        ctrl.open_definition(5, vec![loc("a.rs", 10)]);
        assert!(ctrl.is_visible());
        assert_eq!(ctrl.mode, PeekMode::Definition);
        assert!(ctrl.breadcrumb.is_some());
        assert_eq!(ctrl.breadcrumb.as_ref().unwrap().filename, "a.rs");
    }

    #[test]
    fn controller_open_references() {
        let mut ctrl = PeekController::default();
        ctrl.open_references(5, vec![loc("a.rs", 10), loc("a.rs", 20), loc("b.rs", 3)]);
        assert!(ctrl.is_visible());
        assert_eq!(ctrl.mode, PeekMode::References);
        assert_eq!(ctrl.reference_groups.len(), 2);
    }

    #[test]
    fn controller_close() {
        let mut ctrl = PeekController::default();
        ctrl.open_definition(5, vec![loc("a.rs", 10)]);
        ctrl.close();
        assert!(!ctrl.is_visible());
        assert!(ctrl.breadcrumb.is_none());
    }

    #[test]
    fn controller_next_prev_result() {
        let mut ctrl = PeekController::default();
        ctrl.open_definition(5, vec![loc("a.rs", 10), loc("b.rs", 20)]);

        ctrl.next_result();
        assert_eq!(ctrl.state.selected, 1);
        assert_eq!(ctrl.breadcrumb.as_ref().unwrap().filename, "b.rs");

        ctrl.prev_result();
        assert_eq!(ctrl.state.selected, 0);
        assert_eq!(ctrl.breadcrumb.as_ref().unwrap().filename, "a.rs");
    }

    #[test]
    fn controller_confirm() {
        let mut ctrl = PeekController::default();
        ctrl.open_definition(5, vec![loc("a.rs", 10)]);
        let confirmed = ctrl.confirm().unwrap();
        assert_eq!(confirmed.uri, "a.rs");
    }

    #[test]
    fn controller_group_navigation() {
        let mut ctrl = PeekController::default();
        ctrl.open_references(5, vec![
            loc("a.rs", 1),
            loc("a.rs", 5),
            loc("b.rs", 10),
        ]);

        assert_eq!(ctrl.selected_group, 0);
        ctrl.next_group();
        assert_eq!(ctrl.selected_group, 1);
        ctrl.next_group();
        assert_eq!(ctrl.selected_group, 0);
        ctrl.prev_group();
        assert_eq!(ctrl.selected_group, 1);
    }

    #[test]
    fn controller_ref_in_group_navigation() {
        let mut ctrl = PeekController::default();
        ctrl.open_references(0, vec![
            loc("a.rs", 1),
            loc("a.rs", 5),
            loc("a.rs", 10),
        ]);

        assert_eq!(ctrl.selected_ref_in_group, 0);
        ctrl.next_ref_in_group();
        assert_eq!(ctrl.selected_ref_in_group, 1);
        ctrl.next_ref_in_group();
        assert_eq!(ctrl.selected_ref_in_group, 2);
        ctrl.next_ref_in_group();
        assert_eq!(ctrl.selected_ref_in_group, 0);
    }

    // ── PeekEntry tests ──────────────────────────────────────────

    #[test]
    fn peek_entry_creation() {
        let entry = PeekEntry::new(
            "src/lib.rs",
            Range::new(Position::new(10, 0), Position::new(10, 20)),
            vec!["fn main() {".to_string()],
        );
        assert_eq!(entry.file_name, "lib.rs");
        assert_eq!(entry.directory, "src");
        assert_eq!(entry.match_count, 1);
    }

    #[test]
    fn peek_entry_to_location() {
        let entry = PeekEntry::new(
            "a.rs",
            Range::new(Position::new(5, 0), Position::new(5, 10)),
            vec![],
        );
        let loc = entry.to_location();
        assert_eq!(loc.uri, "a.rs");
        assert_eq!(loc.range.start.line, 5);
    }

    // ── Embedded editor tests ────────────────────────────────────

    #[test]
    fn embedded_editor_load_and_clear() {
        let mut editor = PeekEmbeddedEditor::default();
        let range = Range::new(Position::new(0, 0), Position::new(0, 5));
        editor.load("fn foo() {}".into(), "rust".into(), Some(range));
        assert_eq!(editor.content, "fn foo() {}");
        assert_eq!(editor.language, "rust");
        assert!(editor.read_only);

        editor.clear();
        assert!(editor.content.is_empty());
    }

    // ── show_peek tests ──────────────────────────────────────────

    #[test]
    fn show_peek_definition() {
        let mut ctrl = PeekController::default();
        let entries = vec![PeekEntry::new(
            "src/main.rs",
            Range::new(Position::new(10, 0), Position::new(10, 10)),
            vec!["fn main() {".to_string()],
        )];
        ctrl.show_peek(5, "Definition", PeekMode::Definition, entries);
        assert!(ctrl.is_visible());
        assert_eq!(ctrl.title, "Definition");
        assert_eq!(ctrl.entries.len(), 1);
        assert!(!ctrl.embedded_editor.content.is_empty());
    }

    #[test]
    fn show_peek_references() {
        let mut ctrl = PeekController::default();
        let entries = vec![
            PeekEntry::new(
                "a.rs",
                Range::new(Position::new(1, 0), Position::new(1, 5)),
                vec!["line 1".to_string()],
            ),
            PeekEntry::new(
                "b.rs",
                Range::new(Position::new(3, 0), Position::new(3, 5)),
                vec!["line 3".to_string()],
            ),
        ];
        ctrl.show_peek(0, "2 references", PeekMode::References, entries);
        assert!(ctrl.is_visible());
        assert_eq!(ctrl.reference_groups.len(), 2);
    }

    #[test]
    fn show_peek_empty_entries() {
        let mut ctrl = PeekController::default();
        ctrl.show_peek(0, "Nothing", PeekMode::Definition, vec![]);
        assert!(!ctrl.is_visible());
    }

    #[test]
    fn close_peek_clears_all() {
        let mut ctrl = PeekController::default();
        let entries = vec![PeekEntry::new(
            "a.rs",
            Range::new(Position::new(0, 0), Position::new(0, 5)),
            vec!["text".to_string()],
        )];
        ctrl.show_peek(0, "Test", PeekMode::Definition, entries);
        ctrl.close_peek();
        assert!(!ctrl.is_visible());
        assert!(ctrl.entries.is_empty());
        assert!(ctrl.embedded_editor.content.is_empty());
        assert!(ctrl.title.is_empty());
    }

    // ── Resize tests ─────────────────────────────────────────────

    #[test]
    fn resize_peek_height() {
        let mut ctrl = PeekController::default();
        ctrl.show_peek(
            0,
            "Def",
            PeekMode::Definition,
            vec![PeekEntry::new(
                "a.rs",
                Range::new(Position::new(0, 0), Position::new(0, 5)),
                vec![],
            )],
        );
        assert_eq!(ctrl.state.height_in_lines, 12);

        ctrl.start_resize(100.0);
        assert!(ctrl.resize.is_dragging);

        ctrl.update_resize(160.0, 20.0);
        assert!(ctrl.state.height_in_lines > 12);

        ctrl.end_resize();
        assert!(!ctrl.resize.is_dragging);
    }

    #[test]
    fn resize_clamped() {
        let mut ctrl = PeekController::default();
        ctrl.state.set_height(12);
        ctrl.start_resize(0.0);
        ctrl.update_resize(-1000.0, 20.0);
        assert!(ctrl.state.height_in_lines >= 4);
    }

    #[test]
    fn side_panel_width_clamped() {
        let mut ctrl = PeekController::default();
        ctrl.set_side_panel_width(50.0);
        assert_eq!(ctrl.side_panel_width, 120.0);
        ctrl.set_side_panel_width(1000.0);
        assert_eq!(ctrl.side_panel_width, 600.0);
    }

    // ── navigate_peek tests ──────────────────────────────────────

    #[test]
    fn navigate_peek_forward_backward() {
        let mut ctrl = PeekController::default();
        let entries = vec![
            PeekEntry::new(
                "a.rs",
                Range::new(Position::new(1, 0), Position::new(1, 5)),
                vec!["a".to_string()],
            ),
            PeekEntry::new(
                "b.rs",
                Range::new(Position::new(2, 0), Position::new(2, 5)),
                vec!["b".to_string()],
            ),
        ];
        ctrl.show_peek(0, "test", PeekMode::Definition, entries);

        ctrl.navigate_peek(true);
        assert_eq!(ctrl.state.selected, 1);

        ctrl.navigate_peek(false);
        assert_eq!(ctrl.state.selected, 0);
    }

    // ── Language guessing ────────────────────────────────────────

    #[test]
    fn guess_language() {
        assert_eq!(guess_language_from_uri("src/main.rs"), "rust");
        assert_eq!(guess_language_from_uri("index.ts"), "typescript");
        assert_eq!(guess_language_from_uri("app.py"), "python");
        assert_eq!(guess_language_from_uri("unknown"), "plaintext");
    }
}
