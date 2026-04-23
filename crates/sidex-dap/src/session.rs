//! Debug session state tracking.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::protocol::{Breakpoint, SourceBreakpoint, StackFrame, Thread, Variable};

/// The lifecycle state of a debug session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Initializing,
    Running,
    Stopped,
    Terminated,
}

/// A serializable snapshot of breakpoints for save/restore across sessions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BreakpointPersistence {
    pub source_breakpoints: HashMap<String, Vec<SourceBreakpoint>>,
}

impl BreakpointPersistence {
    /// Saves the persistence data to a JSON file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Loads persistence data from a JSON file. Returns default if file missing.
    pub fn load(path: &std::path::Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    /// Records source breakpoints for a file.
    pub fn set_breakpoints(&mut self, source: String, bps: Vec<SourceBreakpoint>) {
        if bps.is_empty() {
            self.source_breakpoints.remove(&source);
        } else {
            self.source_breakpoints.insert(source, bps);
        }
    }
}

/// Tracks the full state of a debug session on the client side.
#[derive(Debug)]
pub struct DebugSession {
    pub state: SessionState,
    pub threads: Vec<Thread>,
    /// Confirmed breakpoints keyed by source file path.
    pub breakpoints: HashMap<String, Vec<Breakpoint>>,
    pub active_thread: Option<i64>,
    pub active_frame: Option<i64>,
    pub call_stack: Vec<StackFrame>,
    /// Cached variables keyed by `variablesReference`.
    pub variables: HashMap<i64, Vec<Variable>>,
}

impl DebugSession {
    /// Creates a new session in the `Initializing` state.
    pub fn new() -> Self {
        Self {
            state: SessionState::Initializing,
            threads: Vec::new(),
            breakpoints: HashMap::new(),
            active_thread: None,
            active_frame: None,
            call_stack: Vec::new(),
            variables: HashMap::new(),
        }
    }

    /// Transitions the session to `Running`.
    pub fn set_running(&mut self) {
        self.state = SessionState::Running;
    }

    /// Transitions the session to `Stopped`, setting the active thread.
    pub fn set_stopped(&mut self, thread_id: i64) {
        self.state = SessionState::Stopped;
        self.active_thread = Some(thread_id);
    }

    /// Transitions the session to `Terminated`.
    pub fn set_terminated(&mut self) {
        self.state = SessionState::Terminated;
        self.active_thread = None;
        self.active_frame = None;
    }

    /// Updates the thread list.
    pub fn update_threads(&mut self, threads: Vec<Thread>) {
        self.threads = threads;
    }

    /// Updates the call stack and sets the active frame to the top frame.
    pub fn update_call_stack(&mut self, frames: Vec<StackFrame>) {
        self.active_frame = frames.first().map(|f| f.id);
        self.call_stack = frames;
    }

    /// Stores confirmed breakpoints for a source file.
    pub fn update_breakpoints(&mut self, source: String, breakpoints: Vec<Breakpoint>) {
        self.breakpoints.insert(source, breakpoints);
    }

    /// Caches variables for a given variables reference.
    pub fn cache_variables(&mut self, reference: i64, variables: Vec<Variable>) {
        self.variables.insert(reference, variables);
    }

    /// Clears cached variables (e.g. on continue/step).
    pub fn clear_variable_cache(&mut self) {
        self.variables.clear();
    }

    /// Returns `true` if the session is in a stopped state.
    pub fn is_stopped(&self) -> bool {
        self.state == SessionState::Stopped
    }

    /// Returns `true` if the session has terminated.
    pub fn is_terminated(&self) -> bool {
        self.state == SessionState::Terminated
    }
}

impl Default for DebugSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_lifecycle() {
        let mut session = DebugSession::new();
        assert_eq!(session.state, SessionState::Initializing);
        assert!(!session.is_stopped());

        session.set_running();
        assert_eq!(session.state, SessionState::Running);

        session.set_stopped(1);
        assert!(session.is_stopped());
        assert_eq!(session.active_thread, Some(1));

        session.set_running();
        session.set_terminated();
        assert!(session.is_terminated());
        assert_eq!(session.active_thread, None);
    }

    #[test]
    fn update_call_stack_sets_active_frame() {
        let mut session = DebugSession::new();
        let frames = vec![
            StackFrame {
                id: 10,
                name: "main".to_owned(),
                source: None,
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
                module_id: None,
                presentation_hint: None,
            },
            StackFrame {
                id: 11,
                name: "foo".to_owned(),
                source: None,
                line: 5,
                column: 1,
                end_line: None,
                end_column: None,
                module_id: None,
                presentation_hint: None,
            },
        ];
        session.update_call_stack(frames);
        assert_eq!(session.active_frame, Some(10));
        assert_eq!(session.call_stack.len(), 2);
    }

    #[test]
    fn breakpoint_management() {
        let mut session = DebugSession::new();
        let bps = vec![crate::protocol::Breakpoint {
            id: Some(1),
            verified: true,
            message: None,
            source: None,
            line: Some(10),
            column: None,
            end_line: None,
            end_column: None,
        }];
        session.update_breakpoints("/src/main.rs".to_owned(), bps);
        assert_eq!(session.breakpoints.len(), 1);
        assert!(session.breakpoints.contains_key("/src/main.rs"));
    }

    #[test]
    fn breakpoint_persistence_roundtrip() {
        let mut persist = BreakpointPersistence::default();
        persist.set_breakpoints(
            "/src/main.rs".into(),
            vec![crate::protocol::SourceBreakpoint {
                line: 10,
                column: None,
                condition: Some("x > 5".into()),
                hit_condition: None,
                log_message: None,
            }],
        );

        let json = serde_json::to_string(&persist).unwrap();
        let back: BreakpointPersistence = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source_breakpoints.len(), 1);
        assert_eq!(back.source_breakpoints["/src/main.rs"][0].line, 10);
    }

    #[test]
    fn breakpoint_persistence_file_io() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "sidex_bp_test_{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let mut persist = BreakpointPersistence::default();
        persist.set_breakpoints(
            "test.rs".into(),
            vec![crate::protocol::SourceBreakpoint {
                line: 42,
                column: None,
                condition: None,
                hit_condition: None,
                log_message: None,
            }],
        );
        persist.save(&path).unwrap();

        let loaded = BreakpointPersistence::load(&path);
        assert_eq!(loaded.source_breakpoints["test.rs"][0].line, 42);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn breakpoint_persistence_missing_file_returns_default() {
        let loaded = BreakpointPersistence::load(std::path::Path::new("/nonexistent/bp.json"));
        assert!(loaded.source_breakpoints.is_empty());
    }

    #[test]
    fn set_breakpoints_removes_empty() {
        let mut persist = BreakpointPersistence::default();
        persist.set_breakpoints(
            "a.rs".into(),
            vec![crate::protocol::SourceBreakpoint {
                line: 1,
                column: None,
                condition: None,
                hit_condition: None,
                log_message: None,
            }],
        );
        assert_eq!(persist.source_breakpoints.len(), 1);
        persist.set_breakpoints("a.rs".into(), Vec::new());
        assert!(persist.source_breakpoints.is_empty());
    }
}
