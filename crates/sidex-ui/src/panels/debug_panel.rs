//! Debug panel — variables, call stack, breakpoints, watch, toolbar, console,
//! conditional breakpoints, logpoints, exception breakpoints.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use sidex_gpu::color::Color;
use sidex_gpu::GpuRenderer;

use crate::layout::{LayoutNode, Rect, Size};
use crate::widget::{EventResult, Key, MouseButton, UiEvent, Widget};

// ── Debug toolbar actions ────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugAction {
    Continue,
    Pause,
    StepOver,
    StepInto,
    StepOut,
    Restart,
    Stop,
    Disconnect,
}

// ── Debug state ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugState {
    Inactive,
    Running,
    Paused { reason: String },
    Initializing,
}

impl Default for DebugState {
    fn default() -> Self {
        Self::Inactive
    }
}

// ── Variables ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub variable_type: Option<String>,
    pub children: Option<Vec<Variable>>,
    pub named_variables: u32,
    pub indexed_variables: u32,
    pub evaluate_name: Option<String>,
    pub memory_reference: Option<String>,
    pub is_changed: bool,
    pub var_type: Option<String>,
    pub expanded: bool,
    pub changed: bool,
}

impl Variable {
    pub fn leaf(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            variable_type: None,
            children: None,
            named_variables: 0,
            indexed_variables: 0,
            evaluate_name: None,
            memory_reference: None,
            is_changed: false,
            var_type: None,
            expanded: false,
            changed: false,
        }
    }

    pub fn object(
        name: impl Into<String>,
        value: impl Into<String>,
        children: Vec<Variable>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            variable_type: None,
            children: Some(children),
            named_variables: 0,
            indexed_variables: 0,
            evaluate_name: None,
            memory_reference: None,
            is_changed: false,
            var_type: None,
            expanded: false,
            changed: false,
        }
    }

    pub fn has_children(&self) -> bool {
        match &self.children {
            Some(c) => !c.is_empty(),
            None => self.named_variables > 0 || self.indexed_variables > 0,
        }
    }

    pub fn children_slice(&self) -> &[Variable] {
        match &self.children {
            Some(c) => c,
            None => &[],
        }
    }
}

#[derive(Clone, Debug)]
pub struct VariableScope {
    pub name: String,
    pub variables: Vec<Variable>,
    pub is_expanded: bool,
}

#[derive(Clone, Debug)]
pub struct VariablesView {
    pub scopes: Vec<VariableScope>,
    pub expanded: HashSet<String>,
    lazy_load_batch: u32,
}

impl Default for VariablesView {
    fn default() -> Self {
        Self { scopes: Vec::new(), expanded: HashSet::new(), lazy_load_batch: 100 }
    }
}

impl VariablesView {
    pub fn toggle_expand(&mut self, path: &str) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_string());
        }
    }

    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded.contains(path)
    }

    pub fn lazy_load_batch(&self) -> u32 {
        self.lazy_load_batch
    }

    pub fn visible_row_count(&self) -> usize {
        let mut count = 0;
        for scope in &self.scopes {
            count += 1;
            if scope.is_expanded {
                count += Self::count_vars(&scope.variables, &self.expanded, &scope.name);
            }
        }
        count
    }

    fn count_vars(vars: &[Variable], expanded: &HashSet<String>, prefix: &str) -> usize {
        let mut c = 0;
        for (i, v) in vars.iter().enumerate() {
            c += 1;
            let path = format!("{}/{}", prefix, i);
            if expanded.contains(&path) {
                c += Self::count_vars(v.children_slice(), expanded, &path);
            }
        }
        c
    }
}

// ── Watch view ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct WatchExpression {
    pub id: u64,
    pub expression: String,
    pub value: Option<String>,
    pub variable_type: Option<String>,
    pub error: Option<String>,
    pub children: Option<Vec<Variable>>,
    pub expanded: bool,
}

impl WatchExpression {
    pub fn new(id: u64, expression: impl Into<String>) -> Self {
        Self {
            id,
            expression: expression.into(),
            value: None,
            variable_type: None,
            error: None,
            children: None,
            expanded: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct WatchView {
    pub expressions: Vec<WatchExpression>,
    editing_index: Option<usize>,
    input_text: String,
}

impl WatchView {
    pub fn add(&mut self, expr: impl Into<String>) -> u64 {
        let id = self.expressions.len() as u64;
        self.expressions.push(WatchExpression::new(id, expr));
        id
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.expressions.len() {
            self.expressions.remove(index);
        }
    }

    pub fn edit(&mut self, index: usize) {
        if let Some(w) = self.expressions.get(index) {
            self.input_text = w.expression.clone();
            self.editing_index = Some(index);
        }
    }

    pub fn commit_edit(&mut self) {
        if let Some(idx) = self.editing_index.take() {
            if let Some(w) = self.expressions.get_mut(idx) {
                w.expression = self.input_text.clone();
            }
            self.input_text.clear();
        }
    }
}

// ── Call stack ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct FrameSource {
    pub path: Option<PathBuf>,
    pub name: String,
    pub origin: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FramePresentation {
    Normal,
    Label,
    Subtle,
}

#[derive(Clone, Debug)]
pub struct StackFrame {
    pub id: u32,
    pub name: String,
    pub source: Option<FrameSource>,
    pub line: u32,
    pub column: u32,
    pub module_id: Option<String>,
    pub presentation_hint: FramePresentation,
    // Legacy compat fields
    pub source_path: Option<PathBuf>,
    pub is_subtle: bool,
}

impl StackFrame {
    pub fn new(id: u32, name: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            id,
            name: name.into(),
            source: None,
            line,
            column,
            module_id: None,
            presentation_hint: FramePresentation::Normal,
            source_path: None,
            is_subtle: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadStatus {
    Running,
    Paused,
    Stopped(()),
}

#[derive(Clone, Debug)]
pub struct ThreadState {
    pub id: u32,
    pub name: String,
    pub state: ThreadStatus,
    pub frames: Vec<StackFrame>,
    pub is_expanded: bool,
}

/// Legacy alias
pub type DebugThread = ThreadState;

impl ThreadState {
    pub fn new(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            state: ThreadStatus::Paused,
            frames: Vec::new(),
            is_expanded: true,
        }
    }

    pub fn paused(&self) -> bool {
        matches!(self.state, ThreadStatus::Paused | ThreadStatus::Stopped(_))
    }

    pub fn expanded(&self) -> bool {
        self.is_expanded
    }
}

#[derive(Clone, Debug, Default)]
pub struct CallStackView {
    pub threads: Vec<ThreadState>,
    pub selected_frame: Option<(usize, usize)>,
}

impl CallStackView {
    pub fn select_frame(&mut self, thread_idx: usize, frame_idx: usize) {
        self.selected_frame = Some((thread_idx, frame_idx));
    }

    pub fn selected_source(&self) -> Option<(&PathBuf, u32)> {
        let (ti, fi) = self.selected_frame?;
        let frame = self.threads.get(ti)?.frames.get(fi)?;
        let src = frame.source.as_ref()?;
        let path = src.path.as_ref()?;
        Some((path, frame.line))
    }

    pub fn copy_call_stack(&self) -> String {
        let mut out = String::new();
        for t in &self.threads {
            out.push_str(&format!("Thread {} ({}):\n", t.id, t.name));
            for f in &t.frames {
                let loc = f.source.as_ref().map_or(String::new(), |s| {
                    format!(" ({}:{})", s.name, f.line)
                });
                out.push_str(&format!("  {}{}\n", f.name, loc));
            }
        }
        out
    }

    pub fn visible_row_count(&self) -> usize {
        self.threads.iter().map(|t| {
            1 + if t.is_expanded { t.frames.len() } else { 0 }
        }).sum()
    }
}

// ── Breakpoints ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BreakpointKind {
    Line,
    Conditional,
    Logpoint,
    Function(String),
    Data(String),
}

#[derive(Clone, Debug)]
pub struct BreakpointEntry {
    pub id: String,
    pub kind: BreakpointKind,
    pub enabled: bool,
    pub verified: bool,
    pub file: PathBuf,
    pub line: u32,
    pub column: Option<u32>,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
    pub log_message: Option<String>,
    pub hit_count: u32,
}

impl BreakpointEntry {
    pub fn new_line(id: impl Into<String>, file: impl Into<PathBuf>, line: u32) -> Self {
        Self {
            id: id.into(),
            kind: BreakpointKind::Line,
            enabled: true,
            verified: true,
            file: file.into(),
            line,
            column: None,
            condition: None,
            hit_condition: None,
            log_message: None,
            hit_count: 0,
        }
    }

    pub fn filename(&self) -> &str {
        self.file.file_name().and_then(|n| n.to_str()).unwrap_or("")
    }

    pub fn is_logpoint(&self) -> bool {
        self.kind == BreakpointKind::Logpoint
    }

    pub fn is_conditional(&self) -> bool {
        self.kind == BreakpointKind::Conditional
    }

    pub fn convert_to_logpoint(&mut self, message: String) {
        self.kind = BreakpointKind::Logpoint;
        self.log_message = Some(message);
    }
}

/// Legacy alias
pub type Breakpoint = BreakpointEntry;

impl Breakpoint {
    pub fn new(path: impl Into<PathBuf>, line: u32) -> Self {
        Self {
            id: String::new(),
            kind: BreakpointKind::Line,
            enabled: true,
            verified: true,
            file: path.into(),
            line,
            column: None,
            condition: None,
            hit_condition: None,
            log_message: None,
            hit_count: 0,
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.file
    }
}

// ── Exception breakpoints ────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ExceptionBreakpoint {
    pub filter: String,
    pub label: String,
    pub enabled: bool,
    pub condition: Option<String>,
    pub description: Option<String>,
    pub supports_condition: bool,
    // Legacy compat
    pub id: String,
}

impl ExceptionBreakpoint {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        let id_val = id.into();
        Self {
            filter: id_val.clone(),
            label: label.into(),
            enabled: false,
            condition: None,
            description: None,
            supports_condition: true,
            id: id_val,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BreakpointsView {
    pub breakpoints: Vec<BreakpointEntry>,
    pub exception_breakpoints: Vec<ExceptionBreakpoint>,
}

impl Default for BreakpointsView {
    fn default() -> Self {
        Self { breakpoints: Vec::new(), exception_breakpoints: Vec::new() }
    }
}

impl BreakpointsView {
    pub fn toggle_enabled(&mut self, index: usize) {
        if let Some(bp) = self.breakpoints.get_mut(index) {
            bp.enabled = !bp.enabled;
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.breakpoints.len() {
            self.breakpoints.remove(index);
        }
    }

    pub fn toggle_exception(&mut self, filter: &str) {
        if let Some(eb) = self.exception_breakpoints.iter_mut().find(|b| b.filter == filter) {
            eb.enabled = !eb.enabled;
        }
    }

    pub fn edit_condition(&mut self, index: usize, condition: String) {
        if let Some(bp) = self.breakpoints.get_mut(index) {
            bp.condition = Some(condition);
            bp.kind = BreakpointKind::Conditional;
        }
    }
}

// ── Data breakpoints ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataAccessType {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Debug)]
pub struct DataBreakpoint {
    pub id: String,
    pub label: String,
    pub enabled: bool,
    pub access_type: DataAccessType,
    pub condition: Option<String>,
}

// ── Function breakpoints ─────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct FunctionBreakpoint {
    pub id: u64,
    pub name: String,
    pub enabled: bool,
    pub condition: Option<String>,
    pub hit_condition: Option<String>,
}

// ── Inline values ────────────────────────────────────────────────────────────

/// A variable value shown inline in the editor during debugging.
#[derive(Clone, Debug)]
pub struct InlineValue {
    pub name: String,
    pub value: String,
    pub line: u32,
    pub column: u32,
}

// ── Debug console ────────────────────────────────────────────────────────────

/// History of console inputs.
#[derive(Clone, Debug, Default)]
pub struct ConsoleHistory {
    entries: Vec<String>,
    current_index: Option<usize>,
    max_entries: usize,
}

impl ConsoleHistory {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            current_index: None,
            max_entries: 100,
        }
    }

    pub fn push(&mut self, input: &str) {
        if input.is_empty() {
            return;
        }
        let s = input.to_string();
        self.entries.retain(|e| *e != s);
        self.entries.push(s);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.current_index = None;
    }

    pub fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let idx = match self.current_index {
            Some(i) if i > 0 => i - 1,
            None => self.entries.len() - 1,
            _ => return self.entries.first().map(String::as_str),
        };
        self.current_index = Some(idx);
        self.entries.get(idx).map(String::as_str)
    }

    pub fn next(&mut self) -> Option<&str> {
        let idx = self.current_index?;
        if idx + 1 >= self.entries.len() {
            self.current_index = None;
            return None;
        }
        self.current_index = Some(idx + 1);
        self.entries.get(idx + 1).map(String::as_str)
    }
}

// ── Debug toolbar ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebugToolbarButton {
    Continue,
    Pause,
    StepOver,
    StepInto,
    StepOut,
    Restart,
    Stop,
    Disconnect,
}

#[derive(Clone, Debug)]
pub struct DebugToolbar {
    pub visible: bool,
    pub position: (f32, f32),
    pub buttons: Vec<DebugToolbarButton>,
    pub dragging: bool,
    drag_offset: (f32, f32),
    config_name: Option<String>,
}

impl Default for DebugToolbar {
    fn default() -> Self {
        Self {
            visible: false,
            position: (0.0, 0.0),
            buttons: vec![
                DebugToolbarButton::Continue,
                DebugToolbarButton::StepOver,
                DebugToolbarButton::StepInto,
                DebugToolbarButton::StepOut,
                DebugToolbarButton::Restart,
                DebugToolbarButton::Stop,
            ],
            dragging: false,
            drag_offset: (0.0, 0.0),
            config_name: None,
        }
    }
}

impl DebugToolbar {
    pub fn set_config_name(&mut self, name: impl Into<String>) {
        self.config_name = Some(name.into());
    }

    pub fn config_name(&self) -> Option<&str> {
        self.config_name.as_deref()
    }

    pub fn update_for_state(&mut self, state: &DebugState) {
        self.visible = !matches!(state, DebugState::Inactive);
        self.buttons = match state {
            DebugState::Running => vec![
                DebugToolbarButton::Pause,
                DebugToolbarButton::StepOver,
                DebugToolbarButton::StepInto,
                DebugToolbarButton::StepOut,
                DebugToolbarButton::Restart,
                DebugToolbarButton::Stop,
            ],
            DebugState::Paused { .. } => vec![
                DebugToolbarButton::Continue,
                DebugToolbarButton::StepOver,
                DebugToolbarButton::StepInto,
                DebugToolbarButton::StepOut,
                DebugToolbarButton::Restart,
                DebugToolbarButton::Stop,
            ],
            DebugState::Initializing => vec![
                DebugToolbarButton::Pause,
                DebugToolbarButton::Stop,
            ],
            DebugState::Inactive => Vec::new(),
        };
    }

    pub fn start_drag(&mut self, x: f32, y: f32) {
        self.dragging = true;
        self.drag_offset = (x - self.position.0, y - self.position.1);
    }

    pub fn update_drag(&mut self, x: f32, y: f32) {
        if self.dragging {
            self.position = (x - self.drag_offset.0, y - self.drag_offset.1);
        }
    }

    pub fn end_drag(&mut self) {
        self.dragging = false;
    }

    pub fn button_action(btn: DebugToolbarButton) -> DebugAction {
        match btn {
            DebugToolbarButton::Continue => DebugAction::Continue,
            DebugToolbarButton::Pause => DebugAction::Pause,
            DebugToolbarButton::StepOver => DebugAction::StepOver,
            DebugToolbarButton::StepInto => DebugAction::StepInto,
            DebugToolbarButton::StepOut => DebugAction::StepOut,
            DebugToolbarButton::Restart => DebugAction::Restart,
            DebugToolbarButton::Stop => DebugAction::Stop,
            DebugToolbarButton::Disconnect => DebugAction::Disconnect,
        }
    }
}

// ── Debug console entry ──────────────────────────────────────────────────────

/// An entry in the debug console output.
#[derive(Clone, Debug)]
pub enum ConsoleEntry {
    Output {
        text: String,
        category: OutputCategory,
    },
    Evaluation {
        expression: String,
        result: String,
    },
    Error {
        text: String,
    },
}

/// Category of debug console output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputCategory {
    Stdout,
    Stderr,
    Console,
    Important,
}

// ── Loaded scripts ───────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LoadedScript {
    pub path: PathBuf,
    pub name: String,
    pub source_reference: Option<u32>,
}

// ── Debug session state (legacy compat) ──────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugSessionState {
    #[default]
    Inactive,
    Running,
    Paused,
    Initializing,
}

// ── Section visibility ───────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct DebugSections {
    pub variables: bool,
    pub watch: bool,
    pub call_stack: bool,
    pub breakpoints: bool,
}

impl Default for DebugSections {
    fn default() -> Self {
        Self { variables: true, watch: true, call_stack: true, breakpoints: true }
    }
}

// ── Debug panel ──────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub struct DebugPanel<OnAction, OnWatch>
where
    OnAction: FnMut(DebugAction),
    OnWatch: FnMut(WatchEvent),
{
    pub debug_state: DebugState,
    pub session_state: DebugSessionState,
    pub variables_view: VariablesView,
    pub watch_view: WatchView,
    pub call_stack_view: CallStackView,
    pub breakpoints_view: BreakpointsView,
    pub toolbar: DebugToolbar,
    pub loaded_scripts: Vec<LoadedScript>,

    // Legacy compat fields
    pub variables: Vec<Variable>,
    pub threads: Vec<ThreadState>,
    pub breakpoints: Vec<BreakpointEntry>,
    pub watch_expressions: Vec<WatchExpression>,
    pub console_entries: Vec<ConsoleEntry>,
    pub console_input: String,
    pub sections: DebugSections,

    pub on_action: OnAction,
    pub on_watch: OnWatch,

    exception_breakpoints: Vec<ExceptionBreakpoint>,
    function_breakpoints: Vec<FunctionBreakpoint>,
    data_breakpoints: Vec<DataBreakpoint>,
    console_history: ConsoleHistory,
    inline_values: Vec<InlineValue>,
    expanded_variable_paths: HashMap<String, bool>,
    active_thread_id: Option<u64>,
    active_frame_id: Option<u64>,

    selected_section: Option<usize>,
    scroll_offset: f32,
    focused: bool,
    console_scroll_offset: f32,
    console_focused: bool,

    row_height: f32,
    section_header_height: f32,
    toolbar_height: f32,
    indent_width: f32,

    background: Color,
    toolbar_bg: Color,
    toolbar_button_hover: Color,
    section_header_bg: Color,
    selected_bg: Color,
    changed_value_fg: Color,
    error_fg: Color,
    separator_color: Color,
    foreground: Color,
    secondary_fg: Color,
    breakpoint_enabled: Color,
    breakpoint_disabled: Color,
    console_bg: Color,
    console_input_bg: Color,
}

/// Events from watch expression interactions.
#[derive(Clone, Debug)]
pub enum WatchEvent {
    Add(String),
    Edit(u64, String),
    Remove(u64),
    Evaluate(String),
}

impl<OnAction, OnWatch> DebugPanel<OnAction, OnWatch>
where
    OnAction: FnMut(DebugAction),
    OnWatch: FnMut(WatchEvent),
{
    pub fn new(on_action: OnAction, on_watch: OnWatch) -> Self {
        Self {
            debug_state: DebugState::Inactive,
            session_state: DebugSessionState::Inactive,
            variables_view: VariablesView::default(),
            watch_view: WatchView::default(),
            call_stack_view: CallStackView::default(),
            breakpoints_view: BreakpointsView::default(),
            toolbar: DebugToolbar::default(),
            loaded_scripts: Vec::new(),

            variables: Vec::new(),
            threads: Vec::new(),
            breakpoints: Vec::new(),
            watch_expressions: Vec::new(),
            console_entries: Vec::new(),
            console_input: String::new(),
            sections: DebugSections::default(),

            on_action,
            on_watch,

            exception_breakpoints: Vec::new(),
            function_breakpoints: Vec::new(),
            data_breakpoints: Vec::new(),
            console_history: ConsoleHistory::new(),
            inline_values: Vec::new(),
            expanded_variable_paths: HashMap::new(),
            active_thread_id: None,
            active_frame_id: None,

            selected_section: None,
            scroll_offset: 0.0,
            focused: false,
            console_scroll_offset: 0.0,
            console_focused: false,

            row_height: 22.0,
            section_header_height: 22.0,
            toolbar_height: 28.0,
            indent_width: 16.0,

            background: Color::from_hex("#252526").unwrap_or(Color::BLACK),
            toolbar_bg: Color::from_hex("#383838").unwrap_or(Color::BLACK),
            toolbar_button_hover: Color::from_hex("#505050").unwrap_or(Color::BLACK),
            section_header_bg: Color::from_hex("#383838").unwrap_or(Color::BLACK),
            selected_bg: Color::from_hex("#04395e").unwrap_or(Color::BLACK),
            changed_value_fg: Color::from_rgb(220, 220, 100),
            error_fg: Color::from_rgb(220, 80, 80),
            separator_color: Color::from_hex("#2b2b2b").unwrap_or(Color::BLACK),
            foreground: Color::from_hex("#cccccc").unwrap_or(Color::WHITE),
            secondary_fg: Color::from_hex("#969696").unwrap_or(Color::WHITE),
            breakpoint_enabled: Color::from_rgb(220, 60, 60),
            breakpoint_disabled: Color::from_rgb(120, 120, 120),
            console_bg: Color::from_hex("#1e1e1e").unwrap_or(Color::BLACK),
            console_input_bg: Color::from_hex("#3c3c3c").unwrap_or(Color::BLACK),
        }
    }

    pub fn set_debug_state(&mut self, state: DebugState) {
        self.debug_state = state.clone();
        self.toolbar.update_for_state(&state);
        self.session_state = match &state {
            DebugState::Inactive => DebugSessionState::Inactive,
            DebugState::Running => DebugSessionState::Running,
            DebugState::Paused { .. } => DebugSessionState::Paused,
            DebugState::Initializing => DebugSessionState::Initializing,
        };
    }

    pub fn set_paused(&mut self, variables: Vec<Variable>, threads: Vec<ThreadState>) {
        self.session_state = DebugSessionState::Paused;
        self.debug_state = DebugState::Paused { reason: String::new() };
        self.variables = variables;
        self.threads = threads.clone();
        self.call_stack_view.threads = threads;
        self.toolbar.update_for_state(&self.debug_state);
    }

    pub fn set_running(&mut self) {
        self.session_state = DebugSessionState::Running;
        self.debug_state = DebugState::Running;
        self.variables.clear();
        self.toolbar.update_for_state(&self.debug_state);
    }

    pub fn stop_session(&mut self) {
        self.session_state = DebugSessionState::Inactive;
        self.debug_state = DebugState::Inactive;
        self.variables.clear();
        self.threads.clear();
        self.call_stack_view.threads.clear();
        self.loaded_scripts.clear();
        self.toolbar.update_for_state(&self.debug_state);
    }

    pub fn add_console_output(&mut self, text: impl Into<String>, category: OutputCategory) {
        self.console_entries.push(ConsoleEntry::Output {
            text: text.into(),
            category,
        });
    }

    pub fn toggle_breakpoint_enabled(&mut self, index: usize) {
        if let Some(bp) = self.breakpoints.get_mut(index) {
            bp.enabled = !bp.enabled;
        }
    }

    pub fn add_watch(&mut self, expression: impl Into<String>) {
        let expr = expression.into();
        let id = self.watch_expressions.len() as u64;
        (self.on_watch)(WatchEvent::Add(expr.clone()));
        self.watch_expressions.push(WatchExpression::new(id, expr.clone()));
        self.watch_view.add(expr);
    }

    pub fn remove_watch(&mut self, index: usize) {
        if let Some(w) = self.watch_expressions.get(index) {
            let id = w.id;
            (self.on_watch)(WatchEvent::Remove(id));
            self.watch_expressions.remove(index);
            self.watch_view.remove(index);
        }
    }

    // ── Variable expansion ───────────────────────────────────────────────

    pub fn toggle_variable_expanded(&mut self, path: &str) {
        let entry = self
            .expanded_variable_paths
            .entry(path.to_string())
            .or_insert(false);
        *entry = !*entry;
    }

    pub fn is_variable_expanded(&self, path: &str) -> bool {
        self.expanded_variable_paths
            .get(path)
            .copied()
            .unwrap_or(false)
    }

    pub fn expand_variable_at(&mut self, vars: &mut [Variable], indices: &[usize]) {
        if let Some((&first, rest)) = indices.split_first() {
            if let Some(var) = vars.get_mut(first) {
                if rest.is_empty() {
                    var.expanded = !var.expanded;
                } else if let Some(children) = var.children.as_mut() {
                    Self::expand_variable_at_inner(children, rest);
                }
            }
        }
    }

    fn expand_variable_at_inner(vars: &mut [Variable], indices: &[usize]) {
        if let Some((&first, rest)) = indices.split_first() {
            if let Some(var) = vars.get_mut(first) {
                if rest.is_empty() {
                    var.expanded = !var.expanded;
                } else if let Some(children) = var.children.as_mut() {
                    Self::expand_variable_at_inner(children, rest);
                }
            }
        }
    }

    // ── Watch expression evaluation ──────────────────────────────────────

    pub fn evaluate_console_input(&mut self) {
        if !self.console_input.is_empty() {
            let expr = self.console_input.clone();
            self.console_history.push(&expr);
            (self.on_watch)(WatchEvent::Evaluate(expr));
            self.console_input.clear();
        }
    }

    pub fn console_history_prev(&mut self) {
        if let Some(prev) = self.console_history.prev().map(str::to_string) {
            self.console_input = prev;
        }
    }

    pub fn console_history_next(&mut self) {
        if let Some(next) = self.console_history.next().map(str::to_string) {
            self.console_input = next;
        } else {
            self.console_input.clear();
        }
    }

    // ── Exception breakpoints ────────────────────────────────────────────

    pub fn set_exception_breakpoints(&mut self, bps: Vec<ExceptionBreakpoint>) {
        self.exception_breakpoints = bps.clone();
        self.breakpoints_view.exception_breakpoints = bps;
    }

    pub fn toggle_exception_breakpoint(&mut self, id: &str) {
        if let Some(bp) = self.exception_breakpoints.iter_mut().find(|b| b.id == id) {
            bp.enabled = !bp.enabled;
        }
        self.breakpoints_view.toggle_exception(id);
    }

    pub fn exception_breakpoints(&self) -> &[ExceptionBreakpoint] {
        &self.exception_breakpoints
    }

    // ── Function breakpoints ─────────────────────────────────────────────

    pub fn add_function_breakpoint(&mut self, name: impl Into<String>) {
        let id = self.function_breakpoints.len() as u64;
        self.function_breakpoints.push(FunctionBreakpoint {
            id,
            name: name.into(),
            enabled: true,
            condition: None,
            hit_condition: None,
        });
    }

    pub fn remove_function_breakpoint(&mut self, id: u64) {
        self.function_breakpoints.retain(|b| b.id != id);
    }

    pub fn function_breakpoints(&self) -> &[FunctionBreakpoint] {
        &self.function_breakpoints
    }

    // ── Data breakpoints ─────────────────────────────────────────────────

    pub fn set_data_breakpoints(&mut self, bps: Vec<DataBreakpoint>) {
        self.data_breakpoints = bps;
    }

    pub fn data_breakpoints(&self) -> &[DataBreakpoint] {
        &self.data_breakpoints
    }

    // ── Inline values ────────────────────────────────────────────────────

    pub fn set_inline_values(&mut self, values: Vec<InlineValue>) {
        self.inline_values = values;
    }

    pub fn inline_values(&self) -> &[InlineValue] {
        &self.inline_values
    }

    // ── Active frame tracking ────────────────────────────────────────────

    pub fn set_active_frame(&mut self, thread_id: u64, frame_id: u64) {
        self.active_thread_id = Some(thread_id);
        self.active_frame_id = Some(frame_id);
    }

    fn count_variable_rows(vars: &[Variable], depth: usize) -> usize {
        let mut count = 0;
        for var in vars {
            count += 1;
            if var.expanded && var.has_children() {
                count += Self::count_variable_rows(var.children_slice(), depth + 1);
            }
        }
        count
    }

    fn toolbar_buttons() -> &'static [(&'static str, DebugAction)] {
        &[
            ("Continue", DebugAction::Continue),
            ("Step Over", DebugAction::StepOver),
            ("Step Into", DebugAction::StepInto),
            ("Step Out", DebugAction::StepOut),
            ("Restart", DebugAction::Restart),
            ("Stop", DebugAction::Stop),
        ]
    }
}

impl<OnAction, OnWatch> Widget for DebugPanel<OnAction, OnWatch>
where
    OnAction: FnMut(DebugAction),
    OnWatch: FnMut(WatchEvent),
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

        // Debug toolbar
        if self.session_state != DebugSessionState::Inactive {
            rr.draw_rect(
                rect.x,
                y,
                rect.width,
                self.toolbar_height,
                self.toolbar_bg,
                0.0,
            );
            let buttons = Self::toolbar_buttons();
            let btn_size = 24.0;
            let btn_pad = 4.0;
            let total_w = buttons.len() as f32 * (btn_size + btn_pad);
            let mut bx = rect.x + (rect.width - total_w) / 2.0;
            for _btn in buttons {
                rr.draw_rect(
                    bx,
                    y + 2.0,
                    btn_size,
                    btn_size,
                    self.toolbar_button_hover,
                    3.0,
                );
                bx += btn_size + btn_pad;
            }
            y += self.toolbar_height;
            rr.draw_rect(rect.x, y, rect.width, 1.0, self.separator_color, 0.0);
            y += 1.0;
        }

        let sections: &[(&str, bool, usize)] = &[
            (
                "VARIABLES",
                self.sections.variables,
                Self::count_variable_rows(&self.variables, 0),
            ),
            ("WATCH", self.sections.watch, self.watch_expressions.len()),
            (
                "CALL STACK",
                self.sections.call_stack,
                self.threads
                    .iter()
                    .map(|t| 1 + if t.is_expanded { t.frames.len() } else { 0 })
                    .sum(),
            ),
            (
                "BREAKPOINTS",
                self.sections.breakpoints,
                self.breakpoints.len(),
            ),
        ];

        for (label, expanded, item_count) in sections {
            if y > rect.y + rect.height {
                break;
            }
            // Section header
            rr.draw_rect(
                rect.x,
                y,
                rect.width,
                self.section_header_height,
                self.section_header_bg,
                0.0,
            );
            y += self.section_header_height;

            if *expanded {
                let rows = *item_count;
                let section_h = rows as f32 * self.row_height;

                // Placeholder rows
                for r in 0..rows {
                    let ry = y + r as f32 * self.row_height;
                    if ry > rect.y + rect.height {
                        break;
                    }
                    // Just draw alternating for readability
                    if r % 2 == 1 {
                        rr.draw_rect(
                            rect.x,
                            ry,
                            rect.width,
                            self.row_height,
                            Color::from_hex("#ffffff06").unwrap_or(Color::TRANSPARENT),
                            0.0,
                        );
                    }
                }
                y += section_h;
            }

            let _ = label;
        }

        // Breakpoint indicators
        if self.sections.breakpoints {
            for bp in &self.breakpoints {
                let dot_color = if bp.enabled {
                    self.breakpoint_enabled
                } else {
                    self.breakpoint_disabled
                };
                let _ = dot_color;
            }
        }

        let _ = renderer;
    }

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

                // Toolbar click
                if self.session_state != DebugSessionState::Inactive {
                    let toolbar_bottom = rect.y + self.toolbar_height;
                    if *y < toolbar_bottom {
                        let buttons = Self::toolbar_buttons();
                        let btn_size = 24.0;
                        let btn_pad = 4.0;
                        let total_w = buttons.len() as f32 * (btn_size + btn_pad);
                        let start_x = rect.x + (rect.width - total_w) / 2.0;
                        for (i, (_label, action)) in buttons.iter().enumerate() {
                            let bx = start_x + i as f32 * (btn_size + btn_pad);
                            if *x >= bx && *x < bx + btn_size {
                                (self.on_action)(*action);
                                return EventResult::Handled;
                            }
                        }
                        return EventResult::Handled;
                    }
                }

                // Section header toggles
                let toolbar_h = if self.session_state != DebugSessionState::Inactive {
                    self.toolbar_height + 1.0
                } else {
                    0.0
                };
                let mut section_y = rect.y + toolbar_h;
                let section_expanded = [
                    &mut self.sections.variables,
                    &mut self.sections.watch,
                    &mut self.sections.call_stack,
                    &mut self.sections.breakpoints,
                ];
                for expanded in section_expanded {
                    if *y >= section_y && *y < section_y + self.section_header_height {
                        *expanded = !*expanded;
                        return EventResult::Handled;
                    }
                    section_y += self.section_header_height;
                    if *expanded {
                        section_y += self.row_height * 5.0;
                    }
                }

                EventResult::Handled
            }
            UiEvent::MouseScroll { dy, .. } => {
                self.scroll_offset = (self.scroll_offset - dy * 40.0).max(0.0);
                EventResult::Handled
            }
            UiEvent::KeyPress { key: Key::F(5), .. } => {
                (self.on_action)(DebugAction::Continue);
                EventResult::Handled
            }
            UiEvent::KeyPress {
                key: Key::F(10), ..
            } => {
                (self.on_action)(DebugAction::StepOver);
                EventResult::Handled
            }
            UiEvent::KeyPress {
                key: Key::F(11), ..
            } => {
                (self.on_action)(DebugAction::StepInto);
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }
}
