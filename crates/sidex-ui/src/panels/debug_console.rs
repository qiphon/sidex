//! Debug console panel — REPL, output, and expression evaluation during
//! debugging sessions.

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Entry types ──────────────────────────────────────────────────────────────

/// Category of debug output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugOutputCategory {
    Console,
    Stdout,
    Stderr,
    Important,
}

impl DebugOutputCategory {
    pub fn color(self) -> Color {
        match self {
            Self::Console => Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            Self::Stdout => Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            Self::Stderr => Color::from_rgb(244, 71, 71),
            Self::Important => Color::from_rgb(75, 160, 230),
        }
    }
}

/// A single entry in the debug console.
#[derive(Clone, Debug)]
pub enum DebugConsoleEntry {
    Output { text: String, category: DebugOutputCategory },
    Expression { input: String, result: String },
    Error { message: String },
    Group { label: String, entries: Vec<DebugConsoleEntry>, collapsed: bool },
}

impl DebugConsoleEntry {
    pub fn output(text: impl Into<String>, cat: DebugOutputCategory) -> Self {
        Self::Output { text: text.into(), category: cat }
    }
    pub fn expression(input: impl Into<String>, result: impl Into<String>) -> Self {
        Self::Expression { input: input.into(), result: result.into() }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self::Error { message: msg.into() }
    }
    pub fn group(label: impl Into<String>, entries: Vec<DebugConsoleEntry>) -> Self {
        Self::Group { label: label.into(), entries, collapsed: false }
    }

    fn visible_line_count(&self) -> usize {
        match self {
            Self::Output { .. } | Self::Error { .. } => 1,
            Self::Expression { .. } => 2,
            Self::Group { entries, collapsed, .. } => {
                if *collapsed { 1 } else { 1 + entries.iter().map(Self::visible_line_count).sum::<usize>() }
            }
        }
    }
}

// ── Input history ────────────────────────────────────────────────────────────

/// REPL input history with up/down navigation.
#[derive(Clone, Debug, Default)]
pub struct DebugInputHistory {
    entries: Vec<String>,
    index: Option<usize>,
    max: usize,
}

impl DebugInputHistory {
    pub fn new() -> Self {
        Self { entries: Vec::new(), index: None, max: 100 }
    }

    pub fn push(&mut self, input: &str) {
        if input.is_empty() { return; }
        let s = input.to_string();
        self.entries.retain(|e| *e != s);
        self.entries.push(s);
        if self.entries.len() > self.max { self.entries.remove(0); }
        self.index = None;
    }

    pub fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() { return None; }
        let idx = match self.index {
            Some(i) if i > 0 => i - 1,
            None => self.entries.len() - 1,
            _ => return self.entries.first().map(String::as_str),
        };
        self.index = Some(idx);
        self.entries.get(idx).map(String::as_str)
    }

    pub fn next(&mut self) -> Option<&str> {
        let idx = self.index?;
        if idx + 1 >= self.entries.len() { self.index = None; return None; }
        self.index = Some(idx + 1);
        self.entries.get(idx + 1).map(String::as_str)
    }
}

// ── Console events ───────────────────────────────────────────────────────────

/// Events emitted by the debug console.
#[derive(Clone, Debug)]
pub enum DebugConsoleEvent {
    Evaluate(String),
    CopyEntry(usize),
    ClearAll,
    ToggleGroup(usize),
}

// ── Auto-complete suggestion ─────────────────────────────────────────────────

/// A variable-name suggestion for the REPL input.
#[derive(Clone, Debug)]
pub struct ConsoleSuggestion {
    pub label: String,
    pub detail: Option<String>,
}

// ── Debug console ────────────────────────────────────────────────────────────

/// The Debug Console panel (shown during debugging).
#[allow(dead_code)]
pub struct DebugConsole<OnEvent>
where
    OnEvent: FnMut(DebugConsoleEvent),
{
    pub entries: Vec<DebugConsoleEntry>,
    pub input_text: String,
    pub on_event: OnEvent,

    history: DebugInputHistory,
    filter: Option<String>,
    suggestions: Vec<ConsoleSuggestion>,
    suggestion_index: Option<usize>,
    show_suggestions: bool,
    scroll_offset: f32,
    auto_scroll: bool,
    focused: bool,
    input_focused: bool,

    line_height: f32,
    input_height: f32,
    suggestion_row_height: f32,
    indent_width: f32,

    background: Color,
    input_bg: Color,
    input_border: Color,
    input_border_focused: Color,
    stdout_fg: Color,
    stderr_fg: Color,
    expression_input_fg: Color,
    expression_result_fg: Color,
    error_fg: Color,
    important_fg: Color,
    group_header_bg: Color,
    suggestion_bg: Color,
    suggestion_selected_bg: Color,
    separator_color: Color,
    foreground: Color,
    secondary_fg: Color,
}

impl<OnEvent> DebugConsole<OnEvent>
where
    OnEvent: FnMut(DebugConsoleEvent),
{
    pub fn new(on_event: OnEvent) -> Self {
        Self {
            entries: Vec::new(),
            input_text: String::new(),
            on_event,

            history: DebugInputHistory::new(),
            filter: None,
            suggestions: Vec::new(),
            suggestion_index: None,
            show_suggestions: false,
            scroll_offset: 0.0,
            auto_scroll: true,
            focused: false,
            input_focused: false,

            line_height: 20.0,
            input_height: 28.0,
            suggestion_row_height: 22.0,
            indent_width: 16.0,

            background: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            input_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            input_border: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
            input_border_focused: Color::from_hex("#007fd4").unwrap_or(Color::WHITE),
            stdout_fg: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            stderr_fg: Color::from_rgb(244, 71, 71),
            expression_input_fg: Color::from_hex("#dcdcaa").unwrap_or(Color::WHITE),
            expression_result_fg: Color::from_hex("#4ec9b0").unwrap_or(Color::WHITE),
            error_fg: Color::from_rgb(220, 80, 80),
            important_fg: Color::from_rgb(75, 160, 230),
            group_header_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            suggestion_bg: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            suggestion_selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
        }
    }

    pub fn add_output(&mut self, text: impl Into<String>, category: DebugOutputCategory) {
        self.entries.push(DebugConsoleEntry::output(text, category));
    }

    pub fn add_expression_result(&mut self, input: impl Into<String>, result: impl Into<String>) {
        self.entries.push(DebugConsoleEntry::expression(input, result));
    }

    pub fn add_error(&mut self, msg: impl Into<String>) {
        self.entries.push(DebugConsoleEntry::error(msg));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.scroll_offset = 0.0;
        (self.on_event)(DebugConsoleEvent::ClearAll);
    }

    pub fn set_filter(&mut self, filter: Option<String>) {
        self.filter = filter;
    }

    pub fn set_suggestions(&mut self, suggestions: Vec<ConsoleSuggestion>) {
        self.suggestions = suggestions;
        self.suggestion_index = if self.suggestions.is_empty() { None } else { Some(0) };
        self.show_suggestions = !self.suggestions.is_empty();
    }

    pub fn evaluate_input(&mut self) {
        if self.input_text.is_empty() { return; }
        let expr = self.input_text.clone();
        self.history.push(&expr);
        (self.on_event)(DebugConsoleEvent::Evaluate(expr));
        self.input_text.clear();
        self.show_suggestions = false;
    }

    pub fn toggle_auto_scroll(&mut self) { self.auto_scroll = !self.auto_scroll; }

    pub fn toggle_group(&mut self, index: usize) {
        if let Some(DebugConsoleEntry::Group { collapsed, .. }) = self.entries.get_mut(index) {
            *collapsed = !*collapsed;
            (self.on_event)(DebugConsoleEvent::ToggleGroup(index));
        }
    }

    fn total_visible_lines(&self) -> usize {
        let filtered = self.filtered_entries();
        filtered.iter().map(|e| e.visible_line_count()).sum()
    }

    fn filtered_entries(&self) -> Vec<&DebugConsoleEntry> {
        match &self.filter {
            None => self.entries.iter().collect(),
            Some(f) => {
                let fl = f.to_lowercase();
                self.entries.iter().filter(|e| match e {
                    DebugConsoleEntry::Output { text, .. } => text.to_lowercase().contains(&fl),
                    DebugConsoleEntry::Expression { input, result, .. } => {
                        input.to_lowercase().contains(&fl) || result.to_lowercase().contains(&fl)
                    }
                    DebugConsoleEntry::Error { message } => message.to_lowercase().contains(&fl),
                    DebugConsoleEntry::Group { label, .. } => label.to_lowercase().contains(&fl),
                }).collect()
            }
        }
    }

    fn scroll_to_bottom(&mut self, content_h: f32) {
        let total = self.total_visible_lines() as f32 * self.line_height;
        self.scroll_offset = (total - content_h).max(0.0);
    }
}

impl<OnEvent> Widget for DebugConsole<OnEvent>
where
    OnEvent: FnMut(DebugConsoleEvent),
{
    fn layout(&self) -> LayoutNode {
        LayoutNode { size: Size::Flex(1.0), ..LayoutNode::default() }
    }

    #[allow(clippy::cast_precision_loss)]
    fn render(&self, rect: Rect, renderer: &mut GpuRenderer) {
        let mut rr = sidex_gpu::RectRenderer::new();
        rr.draw_rect(rect.x, rect.y, rect.width, rect.height, self.background, 0.0);

        let content_h = rect.height - self.input_height;

        // Entries
        let filtered = self.filtered_entries();
        let mut line_idx: usize = 0;
        for entry in &filtered {
            let lines = entry.visible_line_count();
            for l in 0..lines {
                let ly = rect.y + (line_idx + l) as f32 * self.line_height - self.scroll_offset;
                if ly + self.line_height < rect.y || ly > rect.y + content_h { continue; }

                match entry {
                    DebugConsoleEntry::Output { category, .. } => {
                        rr.draw_rect(rect.x, ly, 3.0, self.line_height, category.color(), 0.0);
                    }
                    DebugConsoleEntry::Expression { .. } => {
                        let fg = if l == 0 { self.expression_input_fg } else { self.expression_result_fg };
                        rr.draw_rect(rect.x, ly, 3.0, self.line_height, fg, 0.0);
                    }
                    DebugConsoleEntry::Error { .. } => {
                        rr.draw_rect(rect.x, ly, 3.0, self.line_height, self.error_fg, 0.0);
                    }
                    DebugConsoleEntry::Group { collapsed, .. } => {
                        if l == 0 {
                            rr.draw_rect(rect.x, ly, rect.width, self.line_height, self.group_header_bg, 0.0);
                            let arrow_s = 8.0;
                            let _ = collapsed;
                            rr.draw_rect(rect.x + 4.0, ly + (self.line_height - arrow_s) / 2.0, arrow_s, arrow_s, self.foreground, 1.0);
                        }
                    }
                }
            }
            line_idx += lines;
        }

        // Input bar
        let iy = rect.y + content_h;
        let ib = if self.input_focused { self.input_border_focused } else { self.input_border };
        rr.draw_rect(rect.x, iy, rect.width, self.input_height, self.input_bg, 0.0);
        rr.draw_border(rect.x, iy, rect.width, self.input_height, ib, 1.0);
        rr.draw_rect(rect.x, iy, 2.0, self.input_height, self.expression_input_fg, 0.0);

        // Suggestions popup
        if self.show_suggestions && !self.suggestions.is_empty() {
            let popup_h = (self.suggestions.len().min(8) as f32) * self.suggestion_row_height;
            let popup_y = iy - popup_h;
            rr.draw_rect(rect.x + 8.0, popup_y, 240.0, popup_h, self.suggestion_bg, 2.0);
            for (i, _s) in self.suggestions.iter().take(8).enumerate() {
                let sy = popup_y + i as f32 * self.suggestion_row_height;
                if self.suggestion_index == Some(i) {
                    rr.draw_rect(rect.x + 8.0, sy, 240.0, self.suggestion_row_height, self.suggestion_selected_bg, 0.0);
                }
            }
        }

        let _ = renderer;
    }

    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult {
        let content_h = rect.height - self.input_height;
        match event {
            UiEvent::Focus => { self.focused = true; EventResult::Handled }
            UiEvent::Blur => { self.focused = false; self.input_focused = false; self.show_suggestions = false; EventResult::Handled }
            UiEvent::MouseDown { x, y, button: MouseButton::Left } if rect.contains(*x, *y) => {
                self.focused = true;
                let input_top = rect.y + content_h;
                self.input_focused = *y >= input_top;
                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                let total = self.total_visible_lines() as f32 * self.line_height;
                let max = (total - content_h).max(0.0);
                self.scroll_offset = (self.scroll_offset - dy * 40.0).clamp(0.0, max);
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Enter, .. } if self.input_focused => {
                if self.show_suggestions {
                    if let Some(idx) = self.suggestion_index {
                        if let Some(s) = self.suggestions.get(idx) {
                            self.input_text = s.label.clone();
                        }
                    }
                    self.show_suggestions = false;
                } else {
                    self.evaluate_input();
                    if self.auto_scroll { self.scroll_to_bottom(content_h); }
                }
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::ArrowUp, .. } if self.input_focused => {
                if self.show_suggestions {
                    if let Some(idx) = self.suggestion_index {
                        self.suggestion_index = Some(idx.saturating_sub(1));
                    }
                } else if let Some(prev) = self.history.prev().map(str::to_string) {
                    self.input_text = prev;
                }
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::ArrowDown, .. } if self.input_focused => {
                if self.show_suggestions {
                    if let Some(idx) = self.suggestion_index {
                        let max = self.suggestions.len().saturating_sub(1);
                        self.suggestion_index = Some((idx + 1).min(max));
                    }
                } else if let Some(next) = self.history.next().map(str::to_string) {
                    self.input_text = next;
                } else {
                    self.input_text.clear();
                }
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::Escape, .. } if self.show_suggestions => {
                self.show_suggestions = false;
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::End, .. } if self.focused && !self.input_focused => {
                self.scroll_to_bottom(content_h);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
