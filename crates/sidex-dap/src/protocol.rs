//! DAP message types from the Debug Adapter Protocol specification.
//!
//! All types use serde for JSON serialization matching the DAP wire format.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Top-level DAP message
// ---------------------------------------------------------------------------

/// A DAP message on the wire — request, response, or event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DapMessage {
    Request(DapRequest),
    Response(DapResponse),
    Event(DapEvent),
}

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

/// All standard DAP request commands.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DapCommand {
    #[serde(rename = "initialize")]
    Initialize,
    #[serde(rename = "launch")]
    Launch,
    #[serde(rename = "attach")]
    Attach,
    #[serde(rename = "disconnect")]
    Disconnect,
    #[serde(rename = "terminate")]
    Terminate,
    #[serde(rename = "restart")]
    Restart,
    #[serde(rename = "setBreakpoints")]
    SetBreakpoints,
    #[serde(rename = "setFunctionBreakpoints")]
    SetFunctionBreakpoints,
    #[serde(rename = "setExceptionBreakpoints")]
    SetExceptionBreakpoints,
    #[serde(rename = "dataBreakpointInfo")]
    DataBreakpointInfo,
    #[serde(rename = "setDataBreakpoints")]
    SetDataBreakpoints,
    #[serde(rename = "setInstructionBreakpoints")]
    SetInstructionBreakpoints,
    #[serde(rename = "configurationDone")]
    ConfigurationDone,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "next")]
    Next,
    #[serde(rename = "stepIn")]
    StepIn,
    #[serde(rename = "stepOut")]
    StepOut,
    #[serde(rename = "stepBack")]
    StepBack,
    #[serde(rename = "reverseContinue")]
    ReverseContinue,
    #[serde(rename = "restartFrame")]
    RestartFrame,
    #[serde(rename = "goto")]
    Goto,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "stackTrace")]
    StackTrace,
    #[serde(rename = "scopes")]
    Scopes,
    #[serde(rename = "variables")]
    Variables,
    #[serde(rename = "source")]
    Source,
    #[serde(rename = "threads")]
    Threads,
    #[serde(rename = "terminateThreads")]
    TerminateThreads,
    #[serde(rename = "modules")]
    Modules,
    #[serde(rename = "loadedSources")]
    LoadedSources,
    #[serde(rename = "evaluate")]
    Evaluate,
    #[serde(rename = "setExpression")]
    SetExpression,
    #[serde(rename = "setVariable")]
    SetVariable,
    #[serde(rename = "disassemble")]
    Disassemble,
    #[serde(rename = "cancel")]
    Cancel,
    #[serde(rename = "completions")]
    Completions,
    #[serde(rename = "exceptionInfo")]
    ExceptionInfo,
    #[serde(rename = "readMemory")]
    ReadMemory,
    #[serde(rename = "writeMemory")]
    WriteMemory,
}

/// A DAP request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapRequest {
    pub seq: i64,
    pub command: DapCommand,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub arguments: Value,
}

impl DapRequest {
    pub fn new(seq: i64, command: DapCommand, arguments: Value) -> Self {
        Self {
            seq,
            command,
            arguments,
        }
    }
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// A DAP response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapResponse {
    pub seq: i64,
    pub request_seq: i64,
    pub success: bool,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub body: Value,
}

// ---------------------------------------------------------------------------
// Event
// ---------------------------------------------------------------------------

/// All standard DAP event kinds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DapEventKind {
    #[serde(rename = "initialized")]
    Initialized,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "continued")]
    Continued,
    #[serde(rename = "exited")]
    Exited,
    #[serde(rename = "terminated")]
    Terminated,
    #[serde(rename = "thread")]
    Thread,
    #[serde(rename = "output")]
    Output,
    #[serde(rename = "breakpoint")]
    Breakpoint,
    #[serde(rename = "module")]
    Module,
    #[serde(rename = "loadedSource")]
    LoadedSource,
    #[serde(rename = "process")]
    Process,
    #[serde(rename = "capabilities")]
    Capabilities,
    #[serde(rename = "progressStart")]
    ProgressStart,
    #[serde(rename = "progressUpdate")]
    ProgressUpdate,
    #[serde(rename = "progressEnd")]
    ProgressEnd,
    #[serde(rename = "invalidated")]
    Invalidated,
    #[serde(rename = "memory")]
    Memory,
}

/// A DAP event message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DapEvent {
    pub seq: i64,
    pub event: DapEventKind,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub body: Value,
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A source location.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_reference: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

/// A single frame in a call stack.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackFrame {
    pub id: i64,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    pub line: i64,
    pub column: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_id: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
}

/// A variable scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
    pub variables_reference: i64,
    #[serde(default)]
    pub named_variables: Option<i64>,
    #[serde(default)]
    pub indexed_variables: Option<i64>,
    pub expensive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
}

/// A variable or expression result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub variable_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<VariablePresentationHint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluate_name: Option<String>,
    pub variables_reference: i64,
    #[serde(default)]
    pub named_variables: Option<i64>,
    #[serde(default)]
    pub indexed_variables: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_reference: Option<String>,
}

/// Presentation hints for a variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablePresentationHint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(default)]
    pub lazy: bool,
}

/// A thread in the debuggee.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub id: i64,
    pub name: String,
}

/// A breakpoint as confirmed by the debug adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Breakpoint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
}

/// A source breakpoint set by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakpoint {
    pub line: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
}

/// A function breakpoint set by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionBreakpoint {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
}

/// Filter for exception breakpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionBreakpointsFilter {
    pub filter: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub supports_condition: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_description: Option<String>,
}

/// Capabilities reported by a debug adapter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct Capabilities {
    #[serde(default)]
    pub supports_configuration_done_request: bool,
    #[serde(default)]
    pub supports_function_breakpoints: bool,
    #[serde(default)]
    pub supports_conditional_breakpoints: bool,
    #[serde(default)]
    pub supports_hit_conditional_breakpoints: bool,
    #[serde(default)]
    pub supports_evaluate_for_hovers: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exception_breakpoint_filters: Option<Vec<ExceptionBreakpointsFilter>>,
    #[serde(default)]
    pub supports_step_back: bool,
    #[serde(default)]
    pub supports_set_variable: bool,
    #[serde(default)]
    pub supports_restart_frame: bool,
    #[serde(default)]
    pub supports_goto_targets_request: bool,
    #[serde(default)]
    pub supports_step_in_targets_request: bool,
    #[serde(default)]
    pub supports_completions_request: bool,
    #[serde(default)]
    pub supports_modules_request: bool,
    #[serde(default)]
    pub supports_restart_request: bool,
    #[serde(default)]
    pub supports_exception_options: bool,
    #[serde(default)]
    pub supports_value_formatting_options: bool,
    #[serde(default)]
    pub supports_exception_info_request: bool,
    #[serde(default)]
    pub support_terminate_debuggee: bool,
    #[serde(default)]
    pub support_suspend_debuggee: bool,
    #[serde(default)]
    pub supports_delayed_stack_trace_loading: bool,
    #[serde(default)]
    pub supports_loaded_sources_request: bool,
    #[serde(default)]
    pub supports_log_points: bool,
    #[serde(default)]
    pub supports_terminate_threads_request: bool,
    #[serde(default)]
    pub supports_terminate_request: bool,
    #[serde(default)]
    pub supports_data_breakpoints: bool,
    #[serde(default)]
    pub supports_read_memory_request: bool,
    #[serde(default)]
    pub supports_write_memory_request: bool,
    #[serde(default)]
    pub supports_disassemble_request: bool,
    #[serde(default)]
    pub supports_cancel_request: bool,
    #[serde(default)]
    pub supports_breakpoint_locations_request: bool,
    #[serde(default)]
    pub supports_clipboard_context: bool,
    #[serde(default)]
    pub supports_stepping_granularity: bool,
    #[serde(default)]
    pub supports_instruction_breakpoints: bool,
    #[serde(default)]
    pub supports_exception_filter_options: bool,
    #[serde(default)]
    pub supports_single_thread_execution_requests: bool,
}

/// A completion item from the adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub completion_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_start: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_length: Option<i64>,
}

/// A data breakpoint set by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataBreakpoint {
    pub data_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_type: Option<DataBreakpointAccessType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
}

/// Access type for data breakpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataBreakpointAccessType {
    Read,
    Write,
    ReadWrite,
}

/// An instruction breakpoint set by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionBreakpoint {
    pub instruction_reference: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
}

/// A module descriptor from the debug adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub id: Value,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default)]
    pub is_optimized: bool,
    #[serde(default)]
    pub is_user_code: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_file_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_time_stamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address_range: Option<String>,
}

/// A goto target for the `goto` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GotoTarget {
    pub id: i64,
    pub label: String,
    pub line: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction_pointer_reference: Option<String>,
}

/// A single disassembled instruction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisassembledInstruction {
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction_bytes: Option<String>,
    pub instruction: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
}

/// Value format options for variable display.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValueFormat {
    #[serde(default)]
    pub hex: bool,
}

/// Exception info details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionDetails {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_type_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluate_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inner_exception: Option<Vec<ExceptionDetails>>,
}

/// Stepping granularity for next/stepIn/stepOut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SteppingGranularity {
    Statement,
    Line,
    Instruction,
}

/// Body for the `stopped` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEventBody {
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<i64>,
    #[serde(default)]
    pub preserve_focus_hint: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default)]
    pub all_threads_stopped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hit_breakpoint_ids: Option<Vec<i64>>,
}

/// Body for the `output` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputEventBody {
    pub category: Option<String>,
    pub output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables_reference: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Body for the `thread` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEventBody {
    pub reason: String,
    pub thread_id: i64,
}

/// Body for the `process` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEventBody {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_process_id: Option<i64>,
    #[serde(default)]
    pub is_local_process: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pointer_size: Option<i64>,
}

/// Body for the `progressStart` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressStartBody {
    pub progress_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<i64>,
    #[serde(default)]
    pub cancellable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
}

/// Body for the `progressUpdate` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressUpdateBody {
    pub progress_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percentage: Option<f64>,
}

/// Body for the `progressEnd` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEndBody {
    pub progress_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Body for the `invalidated` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidatedEventBody {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub areas: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_frame_id: Option<i64>,
}

/// Body for the `memory` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEventBody {
    pub memory_reference: String,
    pub offset: i64,
    pub count: i64,
}

/// Format a [`Variable`] according to a [`ValueFormat`].
///
/// Supports hex (0x…) display. When `format.hex` is false the raw value is
/// returned unchanged.
pub fn format_variable(var: &Variable, format: &ValueFormat) -> String {
    if format.hex {
        if let Ok(n) = var.value.parse::<i64>() {
            return format!("0x{n:X}");
        }
        if let Ok(n) = var.value.parse::<u64>() {
            return format!("0x{n:X}");
        }
    }
    var.value.clone()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serialize_request() {
        let req = DapRequest::new(
            1,
            DapCommand::Initialize,
            json!({"clientID": "sidex", "adapterID": "test"}),
        );
        let msg = DapMessage::Request(req);
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["seq"], 1);
        assert_eq!(json["type"], "request");
        assert_eq!(json["command"], "initialize");
        assert_eq!(json["arguments"]["clientID"], "sidex");
    }

    #[test]
    fn deserialize_response() {
        let raw = json!({
            "seq": 1,
            "type": "response",
            "request_seq": 1,
            "success": true,
            "command": "initialize",
            "body": {
                "supportsConfigurationDoneRequest": true,
                "supportsFunctionBreakpoints": true
            }
        });
        let resp: DapResponse = serde_json::from_value(raw).unwrap();
        assert!(resp.success);
        assert_eq!(resp.command, "initialize");
        assert_eq!(resp.request_seq, 1);
    }

    #[test]
    fn deserialize_event() {
        let raw = json!({
            "seq": 5,
            "type": "event",
            "event": "stopped",
            "body": {
                "reason": "breakpoint",
                "threadId": 1,
                "allThreadsStopped": true
            }
        });
        let event: DapEvent = serde_json::from_value(raw).unwrap();
        assert_eq!(event.event, DapEventKind::Stopped);
        assert_eq!(event.body["reason"], "breakpoint");
    }

    #[test]
    fn serialize_dap_message_request() {
        let msg = DapMessage::Request(DapRequest::new(
            1,
            DapCommand::Launch,
            json!({"program": "/bin/test"}),
        ));
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], "request");
        assert_eq!(json["command"], "launch");
    }

    #[test]
    fn roundtrip_stack_frame() {
        let frame = StackFrame {
            id: 1,
            name: "main".to_owned(),
            source: Some(Source {
                name: Some("main.rs".to_owned()),
                path: Some("/src/main.rs".to_owned()),
                ..Source::default()
            }),
            line: 42,
            column: 1,
            end_line: None,
            end_column: None,
            module_id: None,
            presentation_hint: None,
        };
        let json = serde_json::to_string(&frame).unwrap();
        let back: StackFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 1);
        assert_eq!(back.name, "main");
        assert_eq!(back.line, 42);
    }

    #[test]
    fn roundtrip_capabilities() {
        let caps = Capabilities {
            supports_configuration_done_request: true,
            supports_function_breakpoints: true,
            supports_conditional_breakpoints: true,
            ..Capabilities::default()
        };
        let json = serde_json::to_string(&caps).unwrap();
        let back: Capabilities = serde_json::from_str(&json).unwrap();
        assert!(back.supports_configuration_done_request);
        assert!(back.supports_function_breakpoints);
        assert!(!back.supports_step_back);
    }

    #[test]
    fn roundtrip_breakpoint() {
        let bp = Breakpoint {
            id: Some(1),
            verified: true,
            message: None,
            source: Some(Source {
                path: Some("/src/main.rs".to_owned()),
                ..Source::default()
            }),
            line: Some(10),
            column: None,
            end_line: None,
            end_column: None,
        };
        let json = serde_json::to_string(&bp).unwrap();
        let back: Breakpoint = serde_json::from_str(&json).unwrap();
        assert!(back.verified);
        assert_eq!(back.line, Some(10));
    }

    #[test]
    fn roundtrip_source_breakpoint() {
        let sbp = SourceBreakpoint {
            line: 15,
            column: None,
            condition: Some("x > 5".to_owned()),
            hit_condition: None,
            log_message: None,
        };
        let json = serde_json::to_string(&sbp).unwrap();
        let back: SourceBreakpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.line, 15);
        assert_eq!(back.condition.as_deref(), Some("x > 5"));
    }

    #[test]
    fn all_commands_serialize() {
        let commands = [
            DapCommand::Initialize,
            DapCommand::Launch,
            DapCommand::Attach,
            DapCommand::Disconnect,
            DapCommand::Terminate,
            DapCommand::Restart,
            DapCommand::SetBreakpoints,
            DapCommand::SetFunctionBreakpoints,
            DapCommand::SetExceptionBreakpoints,
            DapCommand::DataBreakpointInfo,
            DapCommand::SetDataBreakpoints,
            DapCommand::SetInstructionBreakpoints,
            DapCommand::ConfigurationDone,
            DapCommand::Continue,
            DapCommand::Next,
            DapCommand::StepIn,
            DapCommand::StepOut,
            DapCommand::StepBack,
            DapCommand::ReverseContinue,
            DapCommand::RestartFrame,
            DapCommand::Goto,
            DapCommand::Pause,
            DapCommand::StackTrace,
            DapCommand::Scopes,
            DapCommand::Variables,
            DapCommand::Source,
            DapCommand::Threads,
            DapCommand::TerminateThreads,
            DapCommand::Modules,
            DapCommand::LoadedSources,
            DapCommand::Evaluate,
            DapCommand::SetExpression,
            DapCommand::SetVariable,
            DapCommand::Disassemble,
            DapCommand::Cancel,
            DapCommand::Completions,
            DapCommand::ExceptionInfo,
            DapCommand::ReadMemory,
            DapCommand::WriteMemory,
        ];
        for cmd in &commands {
            let json = serde_json::to_value(cmd).unwrap();
            assert!(json.is_string(), "command should serialize to string");
        }
    }

    #[test]
    fn all_events_serialize() {
        let events = [
            DapEventKind::Initialized,
            DapEventKind::Stopped,
            DapEventKind::Continued,
            DapEventKind::Exited,
            DapEventKind::Terminated,
            DapEventKind::Thread,
            DapEventKind::Output,
            DapEventKind::Breakpoint,
            DapEventKind::Module,
            DapEventKind::LoadedSource,
            DapEventKind::Process,
            DapEventKind::Capabilities,
            DapEventKind::ProgressStart,
            DapEventKind::ProgressUpdate,
            DapEventKind::ProgressEnd,
            DapEventKind::Invalidated,
            DapEventKind::Memory,
        ];
        for evt in &events {
            let json = serde_json::to_value(evt).unwrap();
            assert!(json.is_string(), "event should serialize to string");
        }
    }

    #[test]
    fn format_variable_hex() {
        let var = Variable {
            name: "x".to_owned(),
            value: "255".to_owned(),
            variable_type: None,
            presentation_hint: None,
            evaluate_name: None,
            variables_reference: 0,
            named_variables: None,
            indexed_variables: None,
            memory_reference: None,
        };
        let fmt = ValueFormat { hex: true };
        assert_eq!(format_variable(&var, &fmt), "0xFF");

        let fmt_off = ValueFormat { hex: false };
        assert_eq!(format_variable(&var, &fmt_off), "255");
    }

    #[test]
    fn data_breakpoint_roundtrip() {
        let bp = DataBreakpoint {
            data_id: "0x1234".to_owned(),
            access_type: Some(DataBreakpointAccessType::Write),
            condition: Some("val != 0".to_owned()),
            hit_condition: None,
        };
        let json = serde_json::to_string(&bp).unwrap();
        let back: DataBreakpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.data_id, "0x1234");
        assert_eq!(back.access_type, Some(DataBreakpointAccessType::Write));
    }

    #[test]
    fn instruction_breakpoint_roundtrip() {
        let bp = InstructionBreakpoint {
            instruction_reference: "0x400100".to_owned(),
            offset: Some(4),
            condition: None,
            hit_condition: None,
        };
        let json = serde_json::to_string(&bp).unwrap();
        let back: InstructionBreakpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(back.instruction_reference, "0x400100");
        assert_eq!(back.offset, Some(4));
    }

    #[test]
    fn module_roundtrip() {
        let m = Module {
            id: json!(42),
            name: "libfoo.so".to_owned(),
            path: Some("/usr/lib/libfoo.so".to_owned()),
            is_optimized: true,
            is_user_code: false,
            version: Some("1.0".to_owned()),
            symbol_status: None,
            symbol_file_path: None,
            date_time_stamp: None,
            address_range: None,
        };
        let json_str = serde_json::to_string(&m).unwrap();
        let back: Module = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.name, "libfoo.so");
        assert!(back.is_optimized);
    }

    #[test]
    fn stopped_event_body_deserialize() {
        let body = json!({
            "reason": "breakpoint",
            "threadId": 1,
            "allThreadsStopped": true,
            "hitBreakpointIds": [1, 2]
        });
        let parsed: StoppedEventBody = serde_json::from_value(body).unwrap();
        assert_eq!(parsed.reason, "breakpoint");
        assert!(parsed.all_threads_stopped);
        assert_eq!(parsed.hit_breakpoint_ids, Some(vec![1, 2]));
    }
}
