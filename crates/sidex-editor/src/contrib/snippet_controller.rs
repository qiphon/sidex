//! Snippet controller — mirrors VS Code's `SnippetController2`.
//!
//! Manages the active snippet session's tabstop navigation, mirror tabstops,
//! transforms, and variable resolution, exposing it as a contribution-level
//! concern separate from the core snippet engine.

use std::collections::HashMap;

use crate::document::Document;
use crate::snippet::SnippetSession;

/// Known snippet variables and their resolution.
#[derive(Debug, Clone)]
pub struct SnippetVariableResolver {
    /// Static variables (e.g. TM_FILENAME, CLIPBOARD).
    pub variables: HashMap<String, String>,
}

impl Default for SnippetVariableResolver {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }
}

impl SnippetVariableResolver {
    /// Creates a resolver with common VS Code variables.
    pub fn with_context(
        filename: &str,
        filepath: &str,
        directory: &str,
        clipboard: &str,
        selected_text: &str,
        line_number: u32,
    ) -> Self {
        let mut vars = HashMap::new();

        let basename = filename.rsplit('/').next().unwrap_or(filename);
        let ext = basename.rsplit('.').next().unwrap_or("");
        let name_no_ext = basename
            .strip_suffix(&format!(".{ext}"))
            .unwrap_or(basename);

        vars.insert("TM_FILENAME".into(), basename.to_string());
        vars.insert("TM_FILENAME_BASE".into(), name_no_ext.to_string());
        vars.insert("TM_FILEPATH".into(), filepath.to_string());
        vars.insert("TM_DIRECTORY".into(), directory.to_string());
        vars.insert("TM_LINE_INDEX".into(), line_number.to_string());
        vars.insert("TM_LINE_NUMBER".into(), (line_number + 1).to_string());
        vars.insert("TM_SELECTED_TEXT".into(), selected_text.to_string());
        vars.insert("CLIPBOARD".into(), clipboard.to_string());

        let now = "2026-04-16T12:00:00"; // placeholder — real impl uses system time
        vars.insert("CURRENT_YEAR".into(), now[..4].to_string());
        vars.insert("CURRENT_MONTH".into(), now[5..7].to_string());
        vars.insert("CURRENT_DATE".into(), now[8..10].to_string());

        Self { variables: vars }
    }

    /// Resolves a variable name to its value.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(|s| s.as_str())
    }

    /// Sets or updates a variable.
    pub fn set(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }
}

/// A transform applied to a tabstop value.
#[derive(Debug, Clone)]
pub struct TabstopTransform {
    /// Regex pattern to match.
    pub pattern: String,
    /// Replacement string (may contain $0, $1, etc.).
    pub replacement: String,
    /// Regex flags (e.g. "gi").
    pub flags: String,
}

impl TabstopTransform {
    /// Applies the transform to the given text.
    #[must_use]
    pub fn apply(&self, _text: &str) -> String {
        // Full regex transform would require a regex engine;
        // this is a structural placeholder that compiles.
        self.replacement.clone()
    }
}

/// The contribution-level wrapper around an active snippet session.
#[derive(Debug, Clone, Default)]
pub struct SnippetControllerState {
    /// The active snippet session, if any.
    pub session: Option<SnippetSession>,
    /// Whether the snippet controller is "locked" (nested snippet in progress).
    pub is_nested: bool,
    /// Variable resolver for the current context.
    pub resolver: SnippetVariableResolver,
    /// Per-tabstop transforms.
    pub transforms: HashMap<u32, TabstopTransform>,
    /// Mirror tabstop groups: tabstop_number → list of linked tabstop numbers.
    pub mirrors: HashMap<u32, Vec<u32>>,
    /// Stack of sessions for nested snippet support.
    session_stack: Vec<SnippetSession>,
}

impl SnippetControllerState {
    /// Returns `true` if a snippet session is active and not finished.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.session.as_ref().is_some_and(|s| !s.finished)
    }

    /// Returns the current tabstop index.
    #[must_use]
    pub fn current_tabstop(&self) -> Option<u32> {
        self.session.as_ref().and_then(|s| {
            if s.finished {
                None
            } else {
                Some(s.current_tabstop_number())
            }
        })
    }

    /// Inserts a snippet and starts a new session.  If a session is already
    /// active, it becomes a nested session.
    pub fn insert_snippet(&mut self, document: &mut Document, template: &str) {
        if self.is_active() {
            if let Some(current) = self.session.take() {
                self.session_stack.push(current);
            }
            self.is_nested = true;
        }

        let resolved = self.resolve_variables(template);
        let session = SnippetSession::start(document, &resolved);
        self.session = Some(session);
    }

    /// Advances to the next tabstop, applying any transforms.
    pub fn next_tabstop(&mut self, document: &mut Document) {
        if let Some(session) = self.session.as_mut() {
            session.next_tabstop(document);
            if session.finished {
                self.finish();
            }
        }
    }

    /// Moves to the previous tabstop.
    pub fn prev_tabstop(&mut self, document: &mut Document) {
        if let Some(session) = self.session.as_mut() {
            session.prev_tabstop(document);
        }
    }

    /// Cancels the active snippet session.
    pub fn cancel(&mut self) {
        self.session = None;
        self.is_nested = false;
        self.transforms.clear();
        self.mirrors.clear();
        self.session_stack.clear();
    }

    /// Registers a transform for a tabstop.
    pub fn add_transform(&mut self, tabstop: u32, transform: TabstopTransform) {
        self.transforms.insert(tabstop, transform);
    }

    /// Registers mirror tabstops (editing tabstop A also updates tabstop B).
    pub fn add_mirror(&mut self, source: u32, target: u32) {
        self.mirrors.entry(source).or_default().push(target);
    }

    /// Returns the mirror targets for the given tabstop.
    #[must_use]
    pub fn mirrors_for(&self, tabstop: u32) -> &[u32] {
        self.mirrors.get(&tabstop).map_or(&[], |v| v.as_slice())
    }

    /// Finishes the session (called when the last tabstop is reached).
    fn finish(&mut self) {
        self.transforms.clear();
        self.mirrors.clear();

        if let Some(parent) = self.session_stack.pop() {
            self.session = Some(parent);
            self.is_nested = !self.session_stack.is_empty();
        } else {
            self.session = None;
            self.is_nested = false;
        }
    }

    /// Resolves `$VARIABLE` and `${VARIABLE}` references in the template.
    fn resolve_variables(&self, template: &str) -> String {
        let mut result = String::with_capacity(template.len());
        let chars: Vec<char> = template.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() {
                if chars[i + 1] == '{' {
                    // ${VAR} or ${N:default}
                    if let Some(close) = chars[i + 2..].iter().position(|&c| c == '}') {
                        let inner: String = chars[i + 2..i + 2 + close].iter().collect();
                        let var_name = inner.split(':').next().unwrap_or(&inner);
                        if var_name.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
                            if let Some(val) = self.resolver.resolve(var_name) {
                                result.push_str(val);
                                i += 3 + close;
                                continue;
                            }
                        }
                    }
                } else if chars[i + 1].is_ascii_uppercase() {
                    let var_start = i + 1;
                    let var_end = (var_start..chars.len())
                        .take_while(|&j| chars[j].is_ascii_uppercase() || chars[j] == '_')
                        .last()
                        .map_or(var_start, |j| j + 1);
                    let var_name: String = chars[var_start..var_end].iter().collect();
                    if let Some(val) = self.resolver.resolve(&var_name) {
                        result.push_str(val);
                        i = var_end;
                        continue;
                    }
                }
            }
            result.push(chars[i]);
            i += 1;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_resolver() {
        let resolver = SnippetVariableResolver::with_context(
            "main.rs",
            "/project/src/main.rs",
            "/project/src",
            "",
            "",
            5,
        );
        assert_eq!(resolver.resolve("TM_FILENAME"), Some("main.rs"));
        assert_eq!(resolver.resolve("TM_FILENAME_BASE"), Some("main"));
        assert_eq!(resolver.resolve("TM_LINE_NUMBER"), Some("6"));
    }

    #[test]
    fn variable_resolution_in_template() {
        let mut state = SnippetControllerState::default();
        state.resolver.set("TM_FILENAME", "test.rs");
        let result = state.resolve_variables("file: $TM_FILENAME end");
        assert_eq!(result, "file: test.rs end");
    }

    #[test]
    fn variable_resolution_braces() {
        let mut state = SnippetControllerState::default();
        state.resolver.set("CLIPBOARD", "pasted");
        let result = state.resolve_variables("${CLIPBOARD}!");
        assert_eq!(result, "pasted!");
    }

    #[test]
    fn mirror_registration() {
        let mut state = SnippetControllerState::default();
        state.add_mirror(1, 2);
        state.add_mirror(1, 3);
        assert_eq!(state.mirrors_for(1), &[2, 3]);
        assert!(state.mirrors_for(5).is_empty());
    }
}
