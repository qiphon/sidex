//! Breakpoint decorations in the editor gutter.
//!
//! Renders breakpoint indicators (circles, diamonds, arrows) in the gutter
//! and handles gutter click interactions for toggling breakpoints.

use std::collections::HashMap;
use std::path::PathBuf;

use sidex_text::Range;

// ── Breakpoint kind ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BreakpointKind {
    Line,
    Conditional,
    Logpoint,
    Function(String),
    Data(String),
}

// ── Breakpoint decoration ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct BreakpointDecoration {
    pub line: u32,
    pub kind: BreakpointKind,
    pub enabled: bool,
    pub verified: bool,
    pub condition: Option<String>,
}

impl BreakpointDecoration {
    pub fn new_line(line: u32) -> Self {
        Self {
            line,
            kind: BreakpointKind::Line,
            enabled: true,
            verified: true,
            condition: None,
        }
    }

    pub fn new_conditional(line: u32, condition: String) -> Self {
        Self {
            line,
            kind: BreakpointKind::Conditional,
            enabled: true,
            verified: true,
            condition: Some(condition),
        }
    }

    pub fn new_logpoint(line: u32, message: String) -> Self {
        Self {
            line,
            kind: BreakpointKind::Logpoint,
            enabled: true,
            verified: true,
            condition: Some(message),
        }
    }
}

// ── Inline breakpoint ────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct InlineBreakpoint {
    pub line: u32,
    pub column: u32,
    pub enabled: bool,
}

// ── Execution point ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ExecutionPoint {
    pub line: u32,
    pub column: Option<u32>,
    pub is_top_frame: bool,
}

// ── Gutter context action ────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GutterContextAction {
    ToggleBreakpoint,
    AddConditionalBreakpoint,
    AddLogpoint,
    EditCondition,
    DisableBreakpoint,
    EnableBreakpoint,
    RemoveBreakpoint,
    RunToCursor,
}

// ── Gutter click result ──────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum GutterClickResult {
    BreakpointToggled { line: u32, now_set: bool },
    ContextMenuRequested { line: u32, existing: bool },
    None,
}

// ── Breakpoint gutter controller ─────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct BreakpointGutterController {
    decorations: Vec<BreakpointDecoration>,
    inline_breakpoints: Vec<InlineBreakpoint>,
    execution_point: Option<ExecutionPoint>,
    file_breakpoints: HashMap<PathBuf, Vec<BreakpointDecoration>>,
    hover_line: Option<u32>,
}

impl BreakpointGutterController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_breakpoints(&mut self, breakpoints: Vec<BreakpointDecoration>) {
        self.decorations = breakpoints;
    }

    pub fn set_file_breakpoints(&mut self, path: PathBuf, breakpoints: Vec<BreakpointDecoration>) {
        self.file_breakpoints.insert(path, breakpoints);
    }

    pub fn breakpoints_for_file(&self, path: &PathBuf) -> &[BreakpointDecoration] {
        self.file_breakpoints.get(path).map_or(&[], Vec::as_slice)
    }

    pub fn set_inline_breakpoints(&mut self, bps: Vec<InlineBreakpoint>) {
        self.inline_breakpoints = bps;
    }

    pub fn inline_breakpoints(&self) -> &[InlineBreakpoint] {
        &self.inline_breakpoints
    }

    pub fn set_execution_point(&mut self, point: Option<ExecutionPoint>) {
        self.execution_point = point;
    }

    pub fn execution_point(&self) -> Option<&ExecutionPoint> {
        self.execution_point.as_ref()
    }

    pub fn set_hover_line(&mut self, line: Option<u32>) {
        self.hover_line = line;
    }

    pub fn decorations(&self) -> &[BreakpointDecoration] {
        &self.decorations
    }

    // ── Toggle logic ─────────────────────────────────────────────────────

    pub fn toggle_breakpoint(&mut self, line: u32) -> GutterClickResult {
        if let Some(idx) = self.decorations.iter().position(|d| d.line == line) {
            self.decorations.remove(idx);
            GutterClickResult::BreakpointToggled { line, now_set: false }
        } else {
            self.decorations.push(BreakpointDecoration::new_line(line));
            GutterClickResult::BreakpointToggled { line, now_set: true }
        }
    }

    pub fn disable_breakpoint(&mut self, line: u32) {
        if let Some(bp) = self.decorations.iter_mut().find(|d| d.line == line) {
            bp.enabled = false;
        }
    }

    pub fn enable_breakpoint(&mut self, line: u32) {
        if let Some(bp) = self.decorations.iter_mut().find(|d| d.line == line) {
            bp.enabled = true;
        }
    }

    pub fn set_condition(&mut self, line: u32, condition: String) {
        if let Some(bp) = self.decorations.iter_mut().find(|d| d.line == line) {
            bp.kind = BreakpointKind::Conditional;
            bp.condition = Some(condition);
        }
    }

    pub fn convert_to_logpoint(&mut self, line: u32, message: String) {
        if let Some(bp) = self.decorations.iter_mut().find(|d| d.line == line) {
            bp.kind = BreakpointKind::Logpoint;
            bp.condition = Some(message);
        }
    }

    pub fn remove_breakpoint(&mut self, line: u32) {
        self.decorations.retain(|d| d.line != line);
    }

    pub fn has_breakpoint_on_line(&self, line: u32) -> bool {
        self.decorations.iter().any(|d| d.line == line)
    }

    pub fn breakpoint_on_line(&self, line: u32) -> Option<&BreakpointDecoration> {
        self.decorations.iter().find(|d| d.line == line)
    }

    // ── Gutter click handler ─────────────────────────────────────────────

    pub fn handle_gutter_click(&mut self, line: u32, is_right_click: bool) -> GutterClickResult {
        if is_right_click {
            let existing = self.has_breakpoint_on_line(line);
            GutterClickResult::ContextMenuRequested { line, existing }
        } else {
            self.toggle_breakpoint(line)
        }
    }

    // ── Decoration queries for rendering ─────────────────────────────────

    pub fn decorations_in_range(&self, start_line: u32, end_line: u32) -> Vec<&BreakpointDecoration> {
        self.decorations
            .iter()
            .filter(|d| d.line >= start_line && d.line <= end_line)
            .collect()
    }

    pub fn should_show_hover_dot(&self, line: u32) -> bool {
        self.hover_line == Some(line) && !self.has_breakpoint_on_line(line)
    }
}

// ── Compute decoration ranges for sidex-editor integration ───────────────────

pub fn compute_breakpoint_ranges(
    decorations: &[BreakpointDecoration],
) -> Vec<(Range, BreakpointVisual)> {
    decorations
        .iter()
        .map(|d| {
            let range = Range {
                start: sidex_text::Position { line: d.line, column: 0 },
                end: sidex_text::Position { line: d.line, column: 0 },
            };
            let visual = match (&d.kind, d.enabled, d.verified) {
                (_, _, false) => BreakpointVisual::Unverified,
                (_, false, _) => BreakpointVisual::Disabled,
                (BreakpointKind::Conditional, true, true) => BreakpointVisual::Conditional,
                (BreakpointKind::Logpoint, true, true) => BreakpointVisual::Logpoint,
                _ => BreakpointVisual::Active,
            };
            (range, visual)
        })
        .collect()
}

// ── Visual style enum for rendering ──────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakpointVisual {
    /// Red filled circle
    Active,
    /// Gray filled circle
    Disabled,
    /// Red circle with strikethrough
    Unverified,
    /// Red diamond
    Conditional,
    /// Diamond with message icon
    Logpoint,
    /// Yellow arrow for current execution line
    ExecutionPoint,
    /// Small red dot for column-level breakpoints
    InlineDot,
}
