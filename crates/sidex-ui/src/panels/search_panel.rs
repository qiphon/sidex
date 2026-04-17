//! Workspace-wide search panel with regex, case, and word match toggles,
//! results streaming, replace preview, collapse/expand all, search history,
//! context lines, progress indicator, and individual result dismissal.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Search options ───────────────────────────────────────────────────────────

/// Toggle flags for search mode.
#[derive(Clone, Debug, Default)]
pub struct SearchOptions {
    pub regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include_ignored: bool,
}

// ── Search result model ──────────────────────────────────────────────────────

/// A single line match within a file.
#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub line_number: u32,
    pub column: u32,
    pub length: u32,
    pub line_text: String,
    pub match_start: u32,
    pub match_end: u32,
    pub preview_before: String,
    pub preview_match: String,
    pub preview_after: String,
}

/// A context line shown around a search match.
#[derive(Clone, Debug)]
pub struct SearchContextLine {
    pub line_number: u32,
    pub text: String,
}

/// All matches within a single file.
#[derive(Clone, Debug)]
pub struct FileSearchResult {
    pub path: PathBuf,
    pub matches: Vec<SearchMatch>,
    pub expanded: bool,
}

impl FileSearchResult {
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }
}

// ── Glob patterns ────────────────────────────────────────────────────────────

/// Include/exclude glob patterns for filtering search scope.
#[derive(Clone, Debug, Default)]
pub struct SearchGlobs {
    pub include: String,
    pub exclude: String,
    pub show_globs: bool,
}

impl SearchGlobs {
    /// Parse the include pattern into individual glob strings.
    pub fn include_patterns(&self) -> Vec<String> {
        parse_glob_string(&self.include)
    }

    /// Parse the exclude pattern into individual glob strings.
    pub fn exclude_patterns(&self) -> Vec<String> {
        parse_glob_string(&self.exclude)
    }
}

fn parse_glob_string(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect()
}

// ── Search progress ──────────────────────────────────────────────────────────

/// Progress information during an active search.
#[derive(Clone, Debug, Default)]
pub struct SearchProgressInfo {
    pub files_searched: u32,
    pub total_files: u32,
    pub matches_found: u32,
}

impl SearchProgressInfo {
    /// Returns a fraction 0.0..=1.0 representing search progress.
    #[must_use]
    pub fn fraction(&self) -> f32 {
        if self.total_files == 0 {
            0.0
        } else {
            (self.files_searched as f32 / self.total_files as f32).min(1.0)
        }
    }

    /// Returns a formatted progress string.
    #[must_use]
    pub fn label(&self) -> String {
        if self.total_files == 0 {
            "Searching...".to_string()
        } else {
            format!(
                "Searched {}/{} files ({} matches)",
                self.files_searched, self.total_files, self.matches_found
            )
        }
    }
}

// ── Search panel ─────────────────────────────────────────────────────────────

/// The Search sidebar panel.
///
/// Provides workspace-wide text search with regex/case/word toggles,
/// replace mode, results grouped by file, include/exclude glob patterns,
/// context lines, progress indicator, and individual result dismissal.
#[allow(dead_code)]
pub struct SearchPanel<OnSearch, OnReplace>
where
    OnSearch: FnMut(&str, &SearchOptions, &SearchGlobs),
    OnReplace: FnMut(ReplaceScope, &str, &str),
{
    pub query: String,
    pub replace_text: String,
    pub options: SearchOptions,
    pub globs: SearchGlobs,
    pub results: Vec<FileSearchResult>,
    pub replace_mode: bool,
    pub on_search: OnSearch,
    pub on_replace: OnReplace,

    // Streaming
    stream_state: SearchStreamState,

    // Progress
    progress: SearchProgressInfo,

    // History
    history: SearchHistory,

    // Replace previews
    replace_previews: Vec<ReplacePreview>,
    show_replace_preview: bool,

    // Dismissed files and individual matches
    dismissed_files: HashSet<PathBuf>,
    dismissed_matches: HashSet<(PathBuf, u32, u32)>,

    // Context lines
    context_lines: usize,

    selected_file: Option<usize>,
    selected_match: Option<(usize, usize)>,
    scroll_offset: f32,
    focused: bool,
    focused_field: SearchField,

    total_match_count: u32,
    total_file_count: u32,

    row_height: f32,
    input_height: f32,
    toggle_size: f32,

    background: Color,
    input_bg: Color,
    input_border: Color,
    input_border_focused: Color,
    toggle_active_bg: Color,
    toggle_inactive_bg: Color,
    file_row_bg: Color,
    match_highlight: Color,
    selected_bg: Color,
    badge_bg: Color,
    badge_fg: Color,
    foreground: Color,
    replace_add_bg: Color,
    replace_remove_bg: Color,
    streaming_indicator: Color,
    dismiss_fg: Color,
    progress_bar_bg: Color,
    progress_bar_fg: Color,
}

/// Which input field is focused in the search panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchField {
    #[default]
    Query,
    Replace,
    IncludeGlob,
    ExcludeGlob,
}

/// Scope of a replace operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplaceScope {
    All,
    File(PathBuf),
    Single {
        file: PathBuf,
        line: u32,
        column: u32,
    },
}

// ── Search history ───────────────────────────────────────────────────────────

/// Maintains recent search and replace history.
#[derive(Clone, Debug)]
pub struct SearchHistory {
    pub search_entries: VecDeque<String>,
    pub replace_entries: VecDeque<String>,
    pub max_entries: usize,
    current_search_index: Option<usize>,
    current_replace_index: Option<usize>,
}

impl Default for SearchHistory {
    fn default() -> Self {
        Self {
            search_entries: VecDeque::new(),
            replace_entries: VecDeque::new(),
            max_entries: 50,
            current_search_index: None,
            current_replace_index: None,
        }
    }
}

impl SearchHistory {
    pub fn push_search(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        let q = query.to_string();
        self.search_entries.retain(|e| *e != q);
        self.search_entries.push_front(q);
        if self.search_entries.len() > self.max_entries {
            self.search_entries.pop_back();
        }
        self.current_search_index = None;
    }

    pub fn push_replace(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let t = text.to_string();
        self.replace_entries.retain(|e| *e != t);
        self.replace_entries.push_front(t);
        if self.replace_entries.len() > self.max_entries {
            self.replace_entries.pop_back();
        }
        self.current_replace_index = None;
    }

    pub fn prev_search(&mut self) -> Option<&str> {
        if self.search_entries.is_empty() {
            return None;
        }
        let idx = match self.current_search_index {
            Some(i) => (i + 1).min(self.search_entries.len() - 1),
            None => 0,
        };
        self.current_search_index = Some(idx);
        self.search_entries.get(idx).map(String::as_str)
    }

    pub fn next_search(&mut self) -> Option<&str> {
        let idx = self.current_search_index?.checked_sub(1)?;
        self.current_search_index = Some(idx);
        self.search_entries.get(idx).map(String::as_str)
    }

    pub fn prev_replace(&mut self) -> Option<&str> {
        if self.replace_entries.is_empty() {
            return None;
        }
        let idx = match self.current_replace_index {
            Some(i) => (i + 1).min(self.replace_entries.len() - 1),
            None => 0,
        };
        self.current_replace_index = Some(idx);
        self.replace_entries.get(idx).map(String::as_str)
    }

    pub fn next_replace(&mut self) -> Option<&str> {
        let idx = self.current_replace_index?.checked_sub(1)?;
        self.current_replace_index = Some(idx);
        self.replace_entries.get(idx).map(String::as_str)
    }
}

// ── Streaming state ──────────────────────────────────────────────────────────

/// State of the search result streaming.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchStreamState {
    #[default]
    Idle,
    Streaming,
    Completed,
    Cancelled,
}

// ── Replace preview ──────────────────────────────────────────────────────────

/// A preview of what a replace operation would change.
#[derive(Clone, Debug)]
pub struct ReplacePreview {
    pub original_line: String,
    pub replaced_line: String,
    pub match_range: (u32, u32),
}

/// Whether file-level results are preserved or dismissed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileResultAction {
    Replace,
    Dismiss,
    Expand,
    Collapse,
}

impl<OnSearch, OnReplace> SearchPanel<OnSearch, OnReplace>
where
    OnSearch: FnMut(&str, &SearchOptions, &SearchGlobs),
    OnReplace: FnMut(ReplaceScope, &str, &str),
{
    pub fn new(on_search: OnSearch, on_replace: OnReplace) -> Self {
        Self {
            query: String::new(),
            replace_text: String::new(),
            options: SearchOptions::default(),
            globs: SearchGlobs::default(),
            results: Vec::new(),
            replace_mode: false,
            on_search,
            on_replace,

            stream_state: SearchStreamState::Idle,
            progress: SearchProgressInfo::default(),
            history: SearchHistory::default(),
            replace_previews: Vec::new(),
            show_replace_preview: false,
            dismissed_files: HashSet::new(),
            dismissed_matches: HashSet::new(),
            context_lines: 0,

            selected_file: None,
            selected_match: None,
            scroll_offset: 0.0,
            focused: false,
            focused_field: SearchField::Query,

            total_match_count: 0,
            total_file_count: 0,

            row_height: 22.0,
            input_height: 28.0,
            toggle_size: 22.0,

            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            input_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            input_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            input_border_focused: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            toggle_active_bg: Color::from_hex("#5a5d5e80").unwrap_or(Color::BLACK),
            toggle_inactive_bg: Color::TRANSPARENT,
            file_row_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            match_highlight: Color::from_hex("#ea5c0055").unwrap_or(Color::BLACK),
            selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            badge_bg: Color::from_hex("#4d4d4d").unwrap_or(Color::BLACK),
            badge_fg: Color::WHITE,
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            replace_add_bg: Color::from_hex("#9bb95533").unwrap_or(Color::BLACK),
            replace_remove_bg: Color::from_hex("#ff000033").unwrap_or(Color::BLACK),
            streaming_indicator: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            dismiss_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
            progress_bar_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            progress_bar_fg: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
        }
    }

    pub fn set_results(&mut self, results: Vec<FileSearchResult>) {
        self.total_file_count = results.len() as u32;
        self.total_match_count = results.iter().map(|f| f.matches.len() as u32).sum();
        self.results = results;
    }

    pub fn toggle_regex(&mut self) {
        self.options.regex = !self.options.regex;
        self.trigger_search();
    }

    pub fn toggle_case(&mut self) {
        self.options.case_sensitive = !self.options.case_sensitive;
        self.trigger_search();
    }

    pub fn toggle_whole_word(&mut self) {
        self.options.whole_word = !self.options.whole_word;
        self.trigger_search();
    }

    pub fn toggle_replace_mode(&mut self) {
        self.replace_mode = !self.replace_mode;
    }

    pub fn toggle_globs(&mut self) {
        self.globs.show_globs = !self.globs.show_globs;
    }

    pub fn replace_all(&mut self) {
        if !self.query.is_empty() {
            (self.on_replace)(
                ReplaceScope::All,
                &self.query.clone(),
                &self.replace_text.clone(),
            );
        }
    }

    pub fn replace_in_file(&mut self, index: usize) {
        if let Some(file) = self.results.get(index) {
            let path = file.path.clone();
            let q = self.query.clone();
            let r = self.replace_text.clone();
            (self.on_replace)(ReplaceScope::File(path), &q, &r);
        }
    }

    // ── Streaming results ────────────────────────────────────────────────

    pub fn begin_streaming(&mut self) {
        self.stream_state = SearchStreamState::Streaming;
        self.results.clear();
        self.total_match_count = 0;
        self.total_file_count = 0;
    }

    pub fn append_streaming_result(&mut self, result: FileSearchResult) {
        self.total_match_count += result.matches.len() as u32;
        self.total_file_count += 1;
        self.results.push(result);
    }

    pub fn finish_streaming(&mut self) {
        self.stream_state = SearchStreamState::Completed;
    }

    pub fn cancel_streaming(&mut self) {
        self.stream_state = SearchStreamState::Cancelled;
    }

    pub fn stream_state(&self) -> SearchStreamState {
        self.stream_state
    }

    pub fn is_streaming(&self) -> bool {
        self.stream_state == SearchStreamState::Streaming
    }

    // ── Collapse / expand all ────────────────────────────────────────────

    pub fn collapse_all(&mut self) {
        for file in &mut self.results {
            file.expanded = false;
        }
    }

    pub fn expand_all(&mut self) {
        for file in &mut self.results {
            file.expanded = true;
        }
    }

    pub fn all_collapsed(&self) -> bool {
        self.results.iter().all(|f| !f.expanded)
    }

    pub fn all_expanded(&self) -> bool {
        self.results.iter().all(|f| f.expanded)
    }

    // ── File-level actions ───────────────────────────────────────────────

    pub fn dismiss_file(&mut self, index: usize) {
        if let Some(file) = self.results.get(index) {
            self.dismissed_files.insert(file.path.clone());
        }
        self.results
            .retain(|f| !self.dismissed_files.contains(&f.path));
        self.total_file_count = self.results.len() as u32;
        self.total_match_count = self.results.iter().map(|f| f.matches.len() as u32).sum();
    }

    pub fn undismiss_all(&mut self) {
        self.dismissed_files.clear();
        self.dismissed_matches.clear();
    }

    /// Dismiss an individual match within a file.
    pub fn dismiss_match(&mut self, file_index: usize, match_index: usize) {
        if let Some(file) = self.results.get_mut(file_index) {
            if let Some(m) = file.matches.get(match_index) {
                self.dismissed_matches
                    .insert((file.path.clone(), m.line_number, m.match_start));
            }
            file.matches.retain(|m| {
                !self.dismissed_matches
                    .contains(&(file.path.clone(), m.line_number, m.match_start))
            });
            if file.matches.is_empty() {
                self.dismissed_files.insert(file.path.clone());
                self.results
                    .retain(|f| !self.dismissed_files.contains(&f.path));
            }
        }
        self.recount_totals();
    }

    fn recount_totals(&mut self) {
        self.total_file_count = self.results.len() as u32;
        self.total_match_count = self.results.iter().map(|f| f.matches.len() as u32).sum();
    }

    // ── Context lines ─────────────────────────────────────────────────────

    /// Set the number of context lines to show before/after each match.
    pub fn set_context_lines(&mut self, n: usize) {
        self.context_lines = n;
    }

    /// Returns the current context lines setting.
    pub fn context_lines(&self) -> usize {
        self.context_lines
    }

    // ── Progress ──────────────────────────────────────────────────────────

    /// Update progress information during an active search.
    pub fn set_progress(&mut self, progress: SearchProgressInfo) {
        self.progress = progress;
    }

    /// Returns the current search progress.
    pub fn progress(&self) -> &SearchProgressInfo {
        &self.progress
    }

    /// Returns true if a search is currently in progress.
    pub fn is_searching(&self) -> bool {
        self.stream_state == SearchStreamState::Streaming
    }

    /// Returns the include/exclude glob patterns.
    pub fn include_pattern(&self) -> &str {
        &self.globs.include
    }

    /// Returns the exclude pattern.
    pub fn exclude_pattern(&self) -> &str {
        &self.globs.exclude
    }

    /// Set the include glob pattern.
    pub fn set_include_pattern(&mut self, pattern: String) {
        self.globs.include = pattern;
    }

    /// Set the exclude glob pattern.
    pub fn set_exclude_pattern(&mut self, pattern: String) {
        self.globs.exclude = pattern;
    }

    /// Replace a single match at a specific file/line/column.
    pub fn replace_single(&mut self, file_index: usize, match_index: usize) {
        if let Some(file) = self.results.get(file_index) {
            if let Some(m) = file.matches.get(match_index) {
                let path = file.path.clone();
                let q = self.query.clone();
                let r = self.replace_text.clone();
                (self.on_replace)(
                    ReplaceScope::Single {
                        file: path,
                        line: m.line_number,
                        column: m.match_start,
                    },
                    &q,
                    &r,
                );
            }
        }
    }

    // ── Replace preview ──────────────────────────────────────────────────

    pub fn toggle_replace_preview(&mut self) {
        self.show_replace_preview = !self.show_replace_preview;
    }

    pub fn set_replace_previews(&mut self, previews: Vec<ReplacePreview>) {
        self.replace_previews = previews;
    }

    pub fn showing_replace_preview(&self) -> bool {
        self.show_replace_preview
    }

    // ── Search history ───────────────────────────────────────────────────

    pub fn history(&self) -> &SearchHistory {
        &self.history
    }

    pub fn history_prev_search(&mut self) {
        if let Some(q) = self.history.prev_search().map(str::to_string) {
            self.query = q;
        }
    }

    pub fn history_next_search(&mut self) {
        if let Some(q) = self.history.next_search().map(str::to_string) {
            self.query = q;
        }
    }

    pub fn history_prev_replace(&mut self) {
        if let Some(r) = self.history.prev_replace().map(str::to_string) {
            self.replace_text = r;
        }
    }

    pub fn history_next_replace(&mut self) {
        if let Some(r) = self.history.next_replace().map(str::to_string) {
            self.replace_text = r;
        }
    }

    pub fn result_count_label(&self) -> String {
        format!(
            "{} results in {} files",
            self.total_match_count, self.total_file_count
        )
    }

    fn trigger_search(&mut self) {
        if !self.query.is_empty() {
            self.history.push_search(&self.query);
            let q = self.query.clone();
            let opts = self.options.clone();
            let globs = self.globs.clone();
            (self.on_search)(&q, &opts, &globs);
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn header_height(&self) -> f32 {
        let mut h = self.input_height + 8.0;
        if self.replace_mode {
            h += self.input_height + 4.0;
        }
        if self.globs.show_globs {
            h += (self.input_height + 4.0) * 2.0;
        }
        h += self.row_height; // result count
        h
    }

    fn toggle_file_expanded(&mut self, index: usize) {
        if let Some(file) = self.results.get_mut(index) {
            file.expanded = !file.expanded;
        }
    }
}

impl<OnSearch, OnReplace> Widget for SearchPanel<OnSearch, OnReplace>
where
    OnSearch: FnMut(&str, &SearchOptions, &SearchGlobs),
    OnReplace: FnMut(ReplaceScope, &str, &str),
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
        let input_x = rect.x + 8.0;
        let input_w = rect.width - 16.0 - (self.toggle_size + 2.0) * 3.0;

        // Search input
        let border = if self.focused_field == SearchField::Query {
            self.input_border_focused
        } else {
            self.input_border
        };
        rr.draw_rect(input_x, y, input_w, self.input_height, self.input_bg, 2.0);
        rr.draw_border(input_x, y, input_w, self.input_height, border, 1.0);

        // Toggle buttons (regex, case, word)
        let toggle_y = y + (self.input_height - self.toggle_size) / 2.0;
        let mut tx = input_x + input_w + 4.0;
        for active in [
            self.options.regex,
            self.options.case_sensitive,
            self.options.whole_word,
        ] {
            let bg = if active {
                self.toggle_active_bg
            } else {
                self.toggle_inactive_bg
            };
            rr.draw_rect(tx, toggle_y, self.toggle_size, self.toggle_size, bg, 3.0);
            tx += self.toggle_size + 2.0;
        }
        y += self.input_height + 4.0;

        // Replace input
        if self.replace_mode {
            let rborder = if self.focused_field == SearchField::Replace {
                self.input_border_focused
            } else {
                self.input_border
            };
            rr.draw_rect(
                input_x,
                y,
                input_w + (self.toggle_size + 2.0) * 3.0,
                self.input_height,
                self.input_bg,
                2.0,
            );
            rr.draw_border(
                input_x,
                y,
                input_w + (self.toggle_size + 2.0) * 3.0,
                self.input_height,
                rborder,
                1.0,
            );
            y += self.input_height + 4.0;
        }

        // Glob patterns
        if self.globs.show_globs {
            let full_w = rect.width - 16.0;
            for field in [SearchField::IncludeGlob, SearchField::ExcludeGlob] {
                let gb = if self.focused_field == field {
                    self.input_border_focused
                } else {
                    self.input_border
                };
                rr.draw_rect(input_x, y, full_w, self.input_height, self.input_bg, 2.0);
                rr.draw_border(input_x, y, full_w, self.input_height, gb, 1.0);
                y += self.input_height + 4.0;
            }
        }

        // Result count badge
        if self.total_match_count > 0 || self.is_searching() {
            let badge_w = 60.0;
            rr.draw_rect(
                rect.x + rect.width - badge_w - 8.0,
                y + 2.0,
                badge_w,
                self.row_height - 4.0,
                self.badge_bg,
                8.0,
            );
        }
        y += self.row_height;

        // Progress bar when searching
        if self.is_searching() {
            let bar_h = 2.0;
            rr.draw_rect(rect.x, y, rect.width, bar_h, self.progress_bar_bg, 0.0);
            let fill_w = rect.width * self.progress.fraction();
            rr.draw_rect(rect.x, y, fill_w, bar_h, self.progress_bar_fg, 0.0);
            y += bar_h + 2.0;
        }

        // Results tree
        for (fi, file) in self.results.iter().enumerate() {
            if y > rect.y + rect.height {
                break;
            }
            // File header
            let is_sel_file = self.selected_file == Some(fi);
            if is_sel_file {
                rr.draw_rect(
                    rect.x,
                    y,
                    rect.width,
                    self.row_height,
                    self.selected_bg,
                    0.0,
                );
            }
            rr.draw_rect(
                rect.x,
                y,
                rect.width,
                self.row_height,
                self.file_row_bg,
                0.0,
            );

            // Match count badge per file
            let mc = file.match_count();
            if mc > 0 {
                let badge_w = 24.0;
                rr.draw_rect(
                    rect.x + rect.width - badge_w - 8.0,
                    y + 3.0,
                    badge_w,
                    self.row_height - 6.0,
                    self.badge_bg,
                    7.0,
                );
            }
            y += self.row_height;

            // Individual matches
            if file.expanded {
                for (mi, _m) in file.matches.iter().enumerate() {
                    if y > rect.y + rect.height {
                        break;
                    }
                    let is_sel_match = self.selected_match == Some((fi, mi));
                    if is_sel_match {
                        rr.draw_rect(
                            rect.x,
                            y,
                            rect.width,
                            self.row_height,
                            self.selected_bg,
                            0.0,
                        );
                    }
                    y += self.row_height;
                }
            }
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
                EventResult::Handled
            }
            UiEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } if rect.contains(*x, *y) => {
                self.focused = true;
                let results_top = rect.y + self.header_height();
                if *y >= results_top {
                    let mut row_y = results_top - self.scroll_offset;
                    for (fi, file) in self.results.iter().enumerate() {
                        if *y >= row_y && *y < row_y + self.row_height {
                            self.selected_file = Some(fi);
                            self.selected_match = None;
                            self.toggle_file_expanded(fi);
                            return EventResult::Handled;
                        }
                        row_y += self.row_height;
                        if file.expanded {
                            for mi in 0..file.matches.len() {
                                if *y >= row_y && *y < row_y + self.row_height {
                                    self.selected_file = Some(fi);
                                    self.selected_match = Some((fi, mi));
                                    return EventResult::Handled;
                                }
                                row_y += self.row_height;
                            }
                        }
                    }
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let total = self.results.iter().fold(0.0_f32, |acc, f| {
                    acc + self.row_height
                        + if f.expanded {
                            f.matches.len() as f32 * self.row_height
                        } else {
                            0.0
                        }
                });
                let max = (total - rect.height + self.header_height()).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                EventResult::Handled
            }
            UiEvent::KeyPress {
                key: Key::Enter, ..
            } if self.focused => {
                self.trigger_search();
                EventResult::Handled
            }
            UiEvent::KeyPress {
                key: Key::Escape, ..
            } if self.focused => {
                self.query.clear();
                self.results.clear();
                self.total_match_count = 0;
                self.total_file_count = 0;
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
