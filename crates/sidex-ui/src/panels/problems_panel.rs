//! Problems panel — diagnostics grouped by file with severity filtering,
//! grouping modes, auto-scroll to current file, and quick fix support.

use std::path::PathBuf;

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── ProblemCount ─────────────────────────────────────────────────────────────

/// Aggregate problem counts by severity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProblemCount {
    pub errors: u32,
    pub warnings: u32,
    pub infos: u32,
    pub hints: u32,
}

impl ProblemCount {
    pub fn total(&self) -> u32 {
        self.errors + self.warnings + self.infos + self.hints
    }

    pub fn status_text(&self) -> String {
        format!(
            "\u{2298} {}  \u{26A0} {}",
            self.errors, self.warnings
        )
    }
}

// ── Severity ─────────────────────────────────────────────────────────────────

/// Diagnostic severity level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

impl DiagnosticSeverity {
    pub fn color(self) -> Color {
        match self {
            Self::Error => Color::from_rgb(244, 71, 71),
            Self::Warning => Color::from_rgb(205, 173, 0),
            Self::Info => Color::from_rgb(55, 148, 255),
            Self::Hint => Color::from_rgb(160, 160, 160),
        }
    }

    pub fn icon_letter(self) -> char {
        match self {
            Self::Error => 'E',
            Self::Warning => 'W',
            Self::Info => 'I',
            Self::Hint => 'H',
        }
    }
}

// ── Diagnostic ───────────────────────────────────────────────────────────────

/// Trait for converting external diagnostic types into the panel's format.
pub trait IntoPanelDiagnostic {
    fn into_panel_diagnostic(self) -> Diagnostic;
}

/// A single diagnostic (error, warning, etc.).
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
    pub line: u32,
    pub column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            source: None,
            code: None,
            line,
            column,
            end_line: None,
            end_column: None,
        }
    }

    pub fn warning(message: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            message: message.into(),
            source: None,
            code: None,
            line,
            column,
            end_line: None,
            end_column: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn location_label(&self) -> String {
        format!("[Ln {}, Col {}]", self.line, self.column)
    }
}

impl IntoPanelDiagnostic for Diagnostic {
    fn into_panel_diagnostic(self) -> Diagnostic {
        self
    }
}

/// Lightweight external diagnostic for bridging from `DiagnosticManager`.
#[derive(Clone, Debug)]
pub struct ExternalDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
}

impl IntoPanelDiagnostic for ExternalDiagnostic {
    fn into_panel_diagnostic(self) -> Diagnostic {
        Diagnostic {
            severity: self.severity,
            message: self.message,
            source: self.source,
            code: self.code,
            line: self.start_line,
            column: self.start_column,
            end_line: self.end_line,
            end_column: self.end_column,
        }
    }
}

// ── Quick fix ────────────────────────────────────────────────────────────────

/// A quick fix action that can be applied from the problems panel.
#[derive(Clone, Debug)]
pub struct QuickFix {
    pub title: String,
    pub kind: QuickFixKind,
    pub is_preferred: bool,
    pub diagnostic_index: usize,
    pub file_index: usize,
}

/// Kind of quick fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickFixKind {
    Fix,
    Refactor,
    SourceAction,
    Organize,
}

// ── Grouping mode ────────────────────────────────────────────────────────────

/// How diagnostics are grouped in the problems panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProblemsGrouping {
    #[default]
    ByFile,
    BySeverity,
    BySource,
}

// ── Sort order ───────────────────────────────────────────────────────────────

/// How diagnostics are sorted within groups.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProblemsSortOrder {
    #[default]
    BySeverity,
    ByPosition,
    BySource,
}

// ── File diagnostics ─────────────────────────────────────────────────────────

/// All diagnostics for a single file.
#[derive(Clone, Debug)]
pub struct FileDiagnostics {
    pub path: PathBuf,
    pub diagnostics: Vec<Diagnostic>,
    pub expanded: bool,
}

impl FileDiagnostics {
    pub fn new(path: impl Into<PathBuf>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            path: path.into(),
            diagnostics,
            expanded: true,
        }
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count()
    }

    pub fn filename(&self) -> &str {
        self.path.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }
}

// ── Filter ───────────────────────────────────────────────────────────────────

/// Filter options for the problems panel.
#[derive(Clone, Debug)]
pub struct ProblemsFilter {
    pub show_errors: bool,
    pub show_warnings: bool,
    pub show_info: bool,
    pub show_hints: bool,
    pub source_filter: Option<String>,
    pub text_filter: String,
}

impl Default for ProblemsFilter {
    fn default() -> Self {
        Self {
            show_errors: true,
            show_warnings: true,
            show_info: true,
            show_hints: true,
            source_filter: None,
            text_filter: String::new(),
        }
    }
}

impl ProblemsFilter {
    pub fn accepts(&self, diag: &Diagnostic) -> bool {
        let severity_ok = match diag.severity {
            DiagnosticSeverity::Error => self.show_errors,
            DiagnosticSeverity::Warning => self.show_warnings,
            DiagnosticSeverity::Info => self.show_info,
            DiagnosticSeverity::Hint => self.show_hints,
        };
        if !severity_ok {
            return false;
        }
        if let Some(ref src) = self.source_filter {
            if diag.source.as_deref() != Some(src.as_str()) {
                return false;
            }
        }
        if !self.text_filter.is_empty() {
            let lower = self.text_filter.to_lowercase();
            if !diag.message.to_lowercase().contains(&lower) {
                return false;
            }
        }
        true
    }
}

// ── Problems panel ───────────────────────────────────────────────────────────

/// The Problems bottom panel.
///
/// Displays errors, warnings, and info diagnostics grouped by file.
/// Supports filtering by severity, source, and text. Clicking a diagnostic
/// navigates to the problem location.
#[allow(dead_code)]
pub struct ProblemsPanel<OnNavigate>
where
    OnNavigate: FnMut(&PathBuf, u32, u32),
{
    pub files: Vec<FileDiagnostics>,
    pub filter: ProblemsFilter,
    pub on_navigate: OnNavigate,

    // Grouping
    grouping: ProblemsGrouping,
    sort_order: ProblemsSortOrder,

    // Auto-scroll
    active_file: Option<PathBuf>,
    auto_scroll_enabled: bool,

    // Quick fixes
    quick_fixes: Vec<QuickFix>,
    selected_quick_fix: Option<usize>,
    quick_fix_menu_visible: bool,

    // File filter (show only specific file's problems)
    file_filter: Option<PathBuf>,

    selected_file: Option<usize>,
    selected_diagnostic: Option<(usize, usize)>,
    scroll_offset: f32,
    focused: bool,

    total_errors: u32,
    total_warnings: u32,
    total_info: u32,

    row_height: f32,
    filter_bar_height: f32,

    background: Color,
    file_row_bg: Color,
    selected_bg: Color,
    hover_bg: Color,
    foreground: Color,
    secondary_fg: Color,
    separator_color: Color,
    filter_bg: Color,
    filter_border: Color,
    quick_fix_bg: Color,
    quick_fix_hover_bg: Color,
    quick_fix_icon: Color,
}

impl<OnNavigate> ProblemsPanel<OnNavigate>
where
    OnNavigate: FnMut(&PathBuf, u32, u32),
{
    pub fn new(on_navigate: OnNavigate) -> Self {
        Self {
            files: Vec::new(),
            filter: ProblemsFilter::default(),
            on_navigate,

            grouping: ProblemsGrouping::default(),
            sort_order: ProblemsSortOrder::default(),
            active_file: None,
            auto_scroll_enabled: true,
            quick_fixes: Vec::new(),
            selected_quick_fix: None,
            quick_fix_menu_visible: false,
            file_filter: None,

            selected_file: None,
            selected_diagnostic: None,
            scroll_offset: 0.0,
            focused: false,

            total_errors: 0,
            total_warnings: 0,
            total_info: 0,

            row_height: 22.0,
            filter_bar_height: 28.0,

            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            file_row_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            hover_bg: Color::from_hex("#2a2d2e").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            filter_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            filter_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            quick_fix_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            quick_fix_hover_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            quick_fix_icon: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
        }
    }

    pub fn set_diagnostics(&mut self, files: Vec<FileDiagnostics>) {
        self.total_errors = files.iter().map(|f| f.error_count() as u32).sum();
        self.total_warnings = files.iter().map(|f| f.warning_count() as u32).sum();
        self.total_info = files
            .iter()
            .flat_map(|f| &f.diagnostics)
            .filter(|d| d.severity == DiagnosticSeverity::Info)
            .count() as u32;
        self.files = files;
    }

    pub fn badge_counts(&self) -> (u32, u32, u32) {
        (self.total_errors, self.total_warnings, self.total_info)
    }

    // ── Grouping ─────────────────────────────────────────────────────────

    pub fn set_grouping(&mut self, grouping: ProblemsGrouping) {
        self.grouping = grouping;
    }

    pub fn grouping(&self) -> ProblemsGrouping {
        self.grouping
    }

    pub fn set_sort_order(&mut self, order: ProblemsSortOrder) {
        self.sort_order = order;
    }

    pub fn sort_order(&self) -> ProblemsSortOrder {
        self.sort_order
    }

    // ── File filter ──────────────────────────────────────────────────────

    pub fn set_file_filter(&mut self, path: Option<PathBuf>) {
        self.file_filter = path;
    }

    pub fn file_filter(&self) -> Option<&PathBuf> {
        self.file_filter.as_ref()
    }

    pub fn clear_file_filter(&mut self) {
        self.file_filter = None;
    }

    // ── Auto-scroll ──────────────────────────────────────────────────────

    pub fn set_active_file(&mut self, path: &PathBuf) {
        self.active_file = Some(path.clone());
        if self.auto_scroll_enabled {
            self.scroll_to_file(path);
        }
    }

    pub fn set_auto_scroll(&mut self, enabled: bool) {
        self.auto_scroll_enabled = enabled;
    }

    pub fn is_auto_scroll_enabled(&self) -> bool {
        self.auto_scroll_enabled
    }

    #[allow(clippy::cast_precision_loss)]
    fn scroll_to_file(&mut self, path: &PathBuf) {
        for (i, file) in self.files.iter().enumerate() {
            if file.path == *path {
                self.scroll_offset = i as f32 * self.row_height;
                self.selected_file = Some(i);
                return;
            }
        }
    }

    // ── Quick fixes ──────────────────────────────────────────────────────

    pub fn set_quick_fixes(&mut self, fixes: Vec<QuickFix>) {
        self.quick_fixes = fixes;
    }

    pub fn show_quick_fix_menu(&mut self, diagnostic_index: usize, file_index: usize) {
        self.selected_quick_fix = None;
        self.quick_fix_menu_visible = true;
        let _ = (diagnostic_index, file_index);
    }

    pub fn hide_quick_fix_menu(&mut self) {
        self.quick_fix_menu_visible = false;
    }

    pub fn available_quick_fixes(&self, file_idx: usize, diag_idx: usize) -> Vec<&QuickFix> {
        self.quick_fixes
            .iter()
            .filter(|f| f.file_index == file_idx && f.diagnostic_index == diag_idx)
            .collect()
    }

    // ── Collapse / expand all ────────────────────────────────────────────

    pub fn collapse_all(&mut self) {
        for file in &mut self.files {
            file.expanded = false;
        }
    }

    pub fn expand_all(&mut self) {
        for file in &mut self.files {
            file.expanded = true;
        }
    }

    pub fn status_text(&self) -> String {
        format!(
            "{} errors, {} warnings",
            self.total_errors, self.total_warnings
        )
    }

    /// Syncs the panel from external diagnostic data. This is the bridge
    /// between `DiagnosticManager` (in sidex-lsp) and the UI panel.
    ///
    /// `file_diagnostics` should be an iterator of `(file_path, diagnostics)`
    /// pairs, sorted with errors first within each file.
    pub fn sync_from_diagnostic_data<I, D>(&mut self, file_diagnostics: I)
    where
        I: IntoIterator<Item = (PathBuf, Vec<D>)>,
        D: IntoPanelDiagnostic,
    {
        let files: Vec<FileDiagnostics> = file_diagnostics
            .into_iter()
            .map(|(path, diags)| {
                let mut panel_diags: Vec<Diagnostic> =
                    diags.into_iter().map(|d| d.into_panel_diagnostic()).collect();
                panel_diags.sort_by_key(|d| (d.severity, d.line, d.column));
                FileDiagnostics::new(path, panel_diags)
            })
            .collect();
        self.set_diagnostics(files);
    }

    /// Double-click handler: open file and navigate to the diagnostic.
    pub fn handle_double_click(&mut self, file_index: usize, diag_index: usize) {
        if let Some(file) = self.files.get(file_index) {
            if let Some(diag) = file.diagnostics.get(diag_index) {
                let path = file.path.clone();
                (self.on_navigate)(&path, diag.line, diag.column);
            }
        }
    }

    /// Returns aggregate problem counts.
    pub fn problem_count(&self) -> ProblemCount {
        let mut count = ProblemCount::default();
        for file in &self.files {
            for diag in &file.diagnostics {
                match diag.severity {
                    DiagnosticSeverity::Error => count.errors += 1,
                    DiagnosticSeverity::Warning => count.warnings += 1,
                    DiagnosticSeverity::Info => count.infos += 1,
                    DiagnosticSeverity::Hint => count.hints += 1,
                }
            }
        }
        count
    }

    /// Copies the diagnostic message to clipboard (returns the text).
    pub fn copy_diagnostic_message(
        &self,
        file_index: usize,
        diag_index: usize,
    ) -> Option<String> {
        self.files
            .get(file_index)
            .and_then(|f| f.diagnostics.get(diag_index))
            .map(|d| {
                let src = d.source.as_deref().unwrap_or("unknown");
                let code = d
                    .code
                    .as_ref()
                    .map(|c| format!(" [{c}]"))
                    .unwrap_or_default();
                format!(
                    "{}{}: {} ({}:{})",
                    src,
                    code,
                    d.message,
                    d.line,
                    d.column
                )
            })
    }

    /// Returns all unique diagnostic sources across all files.
    pub fn all_sources(&self) -> Vec<String> {
        let mut sources: Vec<String> = self
            .files
            .iter()
            .flat_map(|f| &f.diagnostics)
            .filter_map(|d| d.source.clone())
            .collect();
        sources.sort();
        sources.dedup();
        sources
    }

    fn toggle_file_expanded(&mut self, index: usize) {
        if let Some(file) = self.files.get_mut(index) {
            file.expanded = !file.expanded;
        }
    }

    fn filtered_diagnostics<'a>(
        &'a self,
        file: &'a FileDiagnostics,
    ) -> Vec<(usize, &'a Diagnostic)> {
        file.diagnostics
            .iter()
            .enumerate()
            .filter(|(_, d)| self.filter.accepts(d))
            .collect()
    }
}

impl<OnNavigate> Widget for ProblemsPanel<OnNavigate>
where
    OnNavigate: FnMut(&PathBuf, u32, u32),
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

        // Filter bar
        rr.draw_rect(
            rect.x,
            y,
            rect.width,
            self.filter_bar_height,
            self.filter_bg,
            0.0,
        );

        // Severity toggle indicators
        let toggle_size = 18.0;
        let mut tx = rect.x + rect.width - 8.0;
        for (active, severity) in [
            (self.filter.show_hints, DiagnosticSeverity::Hint),
            (self.filter.show_info, DiagnosticSeverity::Info),
            (self.filter.show_warnings, DiagnosticSeverity::Warning),
            (self.filter.show_errors, DiagnosticSeverity::Error),
        ] {
            tx -= toggle_size + 4.0;
            if active {
                rr.draw_rect(
                    tx,
                    y + (self.filter_bar_height - toggle_size) / 2.0,
                    toggle_size,
                    toggle_size,
                    severity.color(),
                    3.0,
                );
            }
        }

        rr.draw_rect(
            rect.x,
            y + self.filter_bar_height - 1.0,
            rect.width,
            1.0,
            self.separator_color,
            0.0,
        );
        y += self.filter_bar_height;

        // File groups
        for (fi, file) in self.files.iter().enumerate() {
            let filtered = self.filtered_diagnostics(file);
            if filtered.is_empty() {
                continue;
            }
            if y - self.scroll_offset > rect.y + rect.height {
                break;
            }

            let ry = y - self.scroll_offset;

            // File header
            let is_sel_file = self.selected_file == Some(fi);
            if is_sel_file {
                rr.draw_rect(
                    rect.x,
                    ry,
                    rect.width,
                    self.row_height,
                    self.selected_bg,
                    0.0,
                );
            } else {
                rr.draw_rect(
                    rect.x,
                    ry,
                    rect.width,
                    self.row_height,
                    self.file_row_bg,
                    0.0,
                );
            }

            // Error/warning count badges
            let ec = file.error_count();
            let wc = file.warning_count();
            let mut badge_x = rect.x + rect.width - 8.0;
            if wc > 0 {
                badge_x -= 26.0;
                rr.draw_rect(
                    badge_x,
                    ry + 3.0,
                    22.0,
                    self.row_height - 6.0,
                    DiagnosticSeverity::Warning.color(),
                    7.0,
                );
            }
            if ec > 0 {
                badge_x -= 26.0;
                rr.draw_rect(
                    badge_x,
                    ry + 3.0,
                    22.0,
                    self.row_height - 6.0,
                    DiagnosticSeverity::Error.color(),
                    7.0,
                );
            }

            y += self.row_height;

            // Individual diagnostics
            if file.expanded {
                for (di, diag) in &filtered {
                    let dry = y - self.scroll_offset;
                    if dry > rect.y + rect.height {
                        break;
                    }
                    let is_sel = self.selected_diagnostic == Some((fi, *di));
                    if is_sel {
                        rr.draw_rect(
                            rect.x,
                            dry,
                            rect.width,
                            self.row_height,
                            self.selected_bg,
                            0.0,
                        );
                    }

                    // Severity icon dot
                    let dot_r = 4.0;
                    rr.draw_rect(
                        rect.x + 12.0,
                        dry + self.row_height / 2.0 - dot_r,
                        dot_r * 2.0,
                        dot_r * 2.0,
                        diag.severity.color(),
                        dot_r,
                    );

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

                // Filter bar severity toggles
                if *y < rect.y + self.filter_bar_height {
                    let toggle_size = 18.0;
                    let mut tx = rect.x + rect.width - 8.0;
                    let mut toggles = [
                        self.filter.show_hints,
                        self.filter.show_info,
                        self.filter.show_warnings,
                        self.filter.show_errors,
                    ];
                    for (i, toggle) in toggles.iter_mut().enumerate() {
                        tx -= toggle_size + 4.0;
                        if *x >= tx && *x < tx + toggle_size {
                            *toggle = !*toggle;
                            match i {
                                0 => self.filter.show_hints = *toggle,
                                1 => self.filter.show_info = *toggle,
                                2 => self.filter.show_warnings = *toggle,
                                3 => self.filter.show_errors = *toggle,
                                _ => {}
                            }
                            return EventResult::Handled;
                        }
                    }
                    return EventResult::Handled;
                }

                // Click on file/diagnostic rows
                let mut row_y = rect.y + self.filter_bar_height - self.scroll_offset;

                // Pre-collect the hit-test data to avoid borrow conflicts.
                let file_info: Vec<(usize, bool, Vec<(usize, u32, u32)>, std::path::PathBuf)> =
                    self.files
                        .iter()
                        .enumerate()
                        .map(|(fi, file)| {
                            let diags: Vec<(usize, u32, u32)> = file
                                .diagnostics
                                .iter()
                                .enumerate()
                                .filter(|(_, d)| self.filter.accepts(d))
                                .map(|(di, d)| (di, d.line, d.column))
                                .collect();
                            (fi, file.expanded, diags, file.path.clone())
                        })
                        .collect();

                for (fi, expanded, filtered, path) in &file_info {
                    if filtered.is_empty() {
                        continue;
                    }
                    if *y >= row_y && *y < row_y + self.row_height {
                        self.selected_file = Some(*fi);
                        self.selected_diagnostic = None;
                        self.toggle_file_expanded(*fi);
                        return EventResult::Handled;
                    }
                    row_y += self.row_height;
                    if *expanded {
                        for &(di, line, column) in filtered {
                            if *y >= row_y && *y < row_y + self.row_height {
                                self.selected_file = Some(*fi);
                                self.selected_diagnostic = Some((*fi, di));
                                (self.on_navigate)(path, line, column);
                                return EventResult::Handled;
                            }
                            row_y += self.row_height;
                        }
                    }
                }
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                self.scroll_offset = (self.scroll_offset - dy * 40.0).max(0.0);
                EventResult::Handled
            }
            UiEvent::KeyPress {
                key: Key::Enter, ..
            } if self.focused => {
                if let Some((fi, di)) = self.selected_diagnostic {
                    if let Some(file) = self.files.get(fi) {
                        if let Some(diag) = file.diagnostics.get(di) {
                            let path = file.path.clone();
                            (self.on_navigate)(&path, diag.line, diag.column);
                        }
                    }
                }
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
