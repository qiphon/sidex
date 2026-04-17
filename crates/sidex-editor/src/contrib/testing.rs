//! Test decorations — gutter play buttons, pass/fail icons, inline error
//! messages, and language-specific test detection for the editor layer.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use sidex_text::Range;

// ── Test state ───────────────────────────────────────────────────────────────

/// Visual state of a single test in the editor gutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TestState {
    /// Gray play icon — test has not been executed yet.
    Unrun,
    /// Spinner — test is currently executing.
    Running,
    /// Green check — test passed.
    Passed,
    /// Red X — test failed.
    Failed,
    /// Gray dash — test was skipped.
    Skipped,
    /// Red exclamation — test errored (e.g. compilation failure).
    Errored,
}

impl TestState {
    /// RGBA color for the gutter icon of this state.
    pub fn color_rgba(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Unrun => (0.6, 0.6, 0.6, 0.7),
            Self::Running => (0.3, 0.6, 1.0, 1.0),
            Self::Passed => (0.306, 0.788, 0.392, 1.0),
            Self::Failed => (0.957, 0.278, 0.278, 1.0),
            Self::Skipped => (0.5, 0.5, 0.5, 0.6),
            Self::Errored => (0.957, 0.278, 0.278, 1.0),
        }
    }

    /// Single-character icon for minimal gutter rendering.
    pub fn icon_char(self) -> char {
        match self {
            Self::Unrun => '▶',
            Self::Running => '◌',
            Self::Passed => '✓',
            Self::Failed => '✗',
            Self::Skipped => '–',
            Self::Errored => '!',
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Passed | Self::Failed | Self::Skipped | Self::Errored)
    }
}

// ── Test action ──────────────────────────────────────────────────────────────

/// An action available from the gutter icon context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TestAction {
    Run,
    Debug,
    RunWithCoverage,
}

impl TestAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Run => "Run Test",
            Self::Debug => "Debug Test",
            Self::RunWithCoverage => "Run with Coverage",
        }
    }
}

// ── Test decoration ──────────────────────────────────────────────────────────

/// Decoration data for a single test detected in the editor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDecoration {
    /// Zero-based line number where the test function begins.
    pub line: u32,
    /// Unique identifier for this test (e.g. `module::test_name`).
    pub test_id: String,
    /// Human-readable name shown in hover tooltip.
    pub test_name: String,
    /// Current visual state.
    pub state: TestState,
    /// The range of the test attribute/decorator/keyword for highlighting.
    pub range: Option<Range>,
}

impl TestDecoration {
    pub fn new(line: u32, test_id: impl Into<String>, test_name: impl Into<String>) -> Self {
        Self {
            line,
            test_id: test_id.into(),
            test_name: test_name.into(),
            state: TestState::Unrun,
            range: None,
        }
    }

    pub fn with_range(mut self, range: Range) -> Self {
        self.range = Some(range);
        self
    }

    pub fn with_state(mut self, state: TestState) -> Self {
        self.state = state;
        self
    }
}

// ── Gutter action ────────────────────────────────────────────────────────────

/// Available actions for a gutter icon at a given line.
#[derive(Debug, Clone)]
pub struct TestGutterAction {
    pub line: u32,
    pub actions: Vec<TestAction>,
}

impl TestGutterAction {
    /// Default set of actions for a test gutter icon.
    pub fn default_actions(line: u32) -> Self {
        Self {
            line,
            actions: vec![TestAction::Run, TestAction::Debug, TestAction::RunWithCoverage],
        }
    }
}

// ── Inline error decoration ──────────────────────────────────────────────────

/// Error message rendered inline after the failing test line.
#[derive(Debug, Clone)]
pub struct TestInlineError {
    pub line: u32,
    pub test_id: String,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

impl TestInlineError {
    /// RGBA for the inline error text.
    pub fn color_rgba() -> (f32, f32, f32, f32) {
        (0.957, 0.278, 0.278, 0.85)
    }
}

// ── Coverage overlay ─────────────────────────────────────────────────────────

/// Per-line coverage state for semi-transparent overlay rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineCoverageState {
    Covered,
    Uncovered,
    Partial,
}

impl LineCoverageState {
    /// Background overlay RGBA (semi-transparent).
    pub fn overlay_rgba(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Covered => (0.0, 0.6, 0.0, 0.08),
            Self::Uncovered => (0.8, 0.0, 0.0, 0.08),
            Self::Partial => (0.8, 0.7, 0.0, 0.08),
        }
    }
}

// ── Language patterns ────────────────────────────────────────────────────────

/// Language-specific patterns for detecting test functions.
#[derive(Debug, Clone)]
pub struct TestPattern {
    pub language: String,
    pub patterns: Vec<TestMatchRule>,
}

/// A single rule for matching test functions in source code.
#[derive(Debug, Clone)]
pub struct TestMatchRule {
    /// What to match (attribute, function call, function name prefix, etc.)
    pub kind: TestMatchKind,
    /// The pattern string (e.g. `"#[test]"`, `"it("`, `"def test_"`).
    pub pattern: String,
}

/// Kind of pattern used to identify test functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestMatchKind {
    /// An attribute/decorator above the function (e.g. `#[test]`, `@pytest.mark`).
    Attribute,
    /// A function call wrapper (e.g. `it(`, `test(`, `describe(`).
    FunctionCall,
    /// A function name prefix (e.g. `def test_`, `func Test`).
    NamePrefix,
}

/// Returns the built-in test detection patterns for a language.
pub fn builtin_test_patterns(language: &str) -> Option<TestPattern> {
    let (lang, patterns) = match language {
        "rust" => (
            "rust",
            vec![
                TestMatchRule {
                    kind: TestMatchKind::Attribute,
                    pattern: "#[test]".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::Attribute,
                    pattern: "#[tokio::test]".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::Attribute,
                    pattern: "#[rstest]".into(),
                },
            ],
        ),
        "javascript" | "typescript" | "javascriptreact" | "typescriptreact" => (
            language,
            vec![
                TestMatchRule {
                    kind: TestMatchKind::FunctionCall,
                    pattern: "it(".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::FunctionCall,
                    pattern: "test(".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::FunctionCall,
                    pattern: "describe(".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::FunctionCall,
                    pattern: "it.only(".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::FunctionCall,
                    pattern: "test.only(".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::FunctionCall,
                    pattern: "it.skip(".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::FunctionCall,
                    pattern: "test.skip(".into(),
                },
            ],
        ),
        "python" => (
            "python",
            vec![
                TestMatchRule {
                    kind: TestMatchKind::NamePrefix,
                    pattern: "def test_".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::NamePrefix,
                    pattern: "class Test".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::Attribute,
                    pattern: "@pytest.mark".into(),
                },
            ],
        ),
        "go" => (
            "go",
            vec![
                TestMatchRule {
                    kind: TestMatchKind::NamePrefix,
                    pattern: "func Test".into(),
                },
                TestMatchRule {
                    kind: TestMatchKind::NamePrefix,
                    pattern: "func Benchmark".into(),
                },
            ],
        ),
        _ => return None,
    };

    Some(TestPattern {
        language: lang.into(),
        patterns,
    })
}

// ── Detection ────────────────────────────────────────────────────────────────

/// Detects test functions in source code using simple line scanning against
/// language-specific patterns. Returns decorations for each detected test.
///
/// For production use, prefer tree-sitter or LSP `textDocument/codeLens`
/// responses; this function provides a fast fallback.
#[allow(clippy::cast_possible_truncation)]
pub fn detect_test_functions(lines: &[&str], language: &str) -> Vec<TestDecoration> {
    let patterns = match builtin_test_patterns(language) {
        Some(p) => p,
        None => return Vec::new(),
    };

    let mut decorations = Vec::new();
    let mut pending_attribute_line: Option<u32> = None;

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        for rule in &patterns.patterns {
            let matched = match rule.kind {
                TestMatchKind::Attribute => {
                    if trimmed.starts_with(&rule.pattern) {
                        pending_attribute_line = Some(line_idx as u32);
                        false
                    } else {
                        false
                    }
                }
                TestMatchKind::FunctionCall => trimmed.starts_with(&rule.pattern),
                TestMatchKind::NamePrefix => trimmed.starts_with(&rule.pattern),
            };

            if matched {
                let test_line = line_idx as u32;
                let test_name = extract_test_name(trimmed, language);
                let test_id = format!("{language}::{test_name}");
                decorations.push(TestDecoration::new(test_line, &test_id, &test_name));
                break;
            }
        }

        if let Some(attr_line) = pending_attribute_line {
            if attr_line != line_idx as u32 && trimmed.starts_with("fn ") || trimmed.starts_with("pub fn ") || trimmed.starts_with("async fn ") || trimmed.starts_with("pub async fn ") {
                let test_name = extract_test_name(trimmed, language);
                let test_id = format!("{language}::{test_name}");
                decorations.push(TestDecoration::new(attr_line, &test_id, &test_name));
                pending_attribute_line = None;
            } else if attr_line != line_idx as u32 && !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("//") {
                pending_attribute_line = None;
            }
        }
    }

    decorations
}

/// Extracts the test name from a source line based on language conventions.
fn extract_test_name(line: &str, language: &str) -> String {
    match language {
        "rust" => {
            let trimmed = line.trim();
            let after_fn = if let Some(rest) = trimmed.strip_prefix("pub async fn ") {
                rest
            } else if let Some(rest) = trimmed.strip_prefix("async fn ") {
                rest
            } else if let Some(rest) = trimmed.strip_prefix("pub fn ") {
                rest
            } else if let Some(rest) = trimmed.strip_prefix("fn ") {
                rest
            } else {
                return trimmed.to_string();
            };
            after_fn
                .split('(')
                .next()
                .unwrap_or(after_fn)
                .trim()
                .to_string()
        }
        "javascript" | "typescript" | "javascriptreact" | "typescriptreact" => {
            let trimmed = line.trim();
            if let Some(start) = trimmed.find('(') {
                if let Some(quote_start) = trimmed[start..].find(['\'', '"', '`']) {
                    let abs_start = start + quote_start + 1;
                    let quote_char = trimmed.as_bytes()[abs_start - 1] as char;
                    if let Some(end) = trimmed[abs_start..].find(quote_char) {
                        return trimmed[abs_start..abs_start + end].to_string();
                    }
                }
            }
            trimmed.to_string()
        }
        "python" => {
            let trimmed = line.trim();
            let after_def = trimmed.strip_prefix("def ").or_else(|| trimmed.strip_prefix("class "));
            match after_def {
                Some(rest) => rest
                    .split(['(', ':'])
                    .next()
                    .unwrap_or(rest)
                    .trim()
                    .to_string(),
                None => trimmed.to_string(),
            }
        }
        "go" => {
            let trimmed = line.trim();
            let after_func = trimmed.strip_prefix("func ");
            match after_func {
                Some(rest) => rest
                    .split('(')
                    .next()
                    .unwrap_or(rest)
                    .trim()
                    .to_string(),
                None => trimmed.to_string(),
            }
        }
        _ => line.trim().to_string(),
    }
}

// ── Test decoration controller ───────────────────────────────────────────────

/// Manages test decorations for a single editor document.
#[derive(Debug, Clone, Default)]
pub struct TestDecorationController {
    decorations: Vec<TestDecoration>,
    inline_errors: Vec<TestInlineError>,
    coverage_lines: HashMap<u32, LineCoverageState>,
    coverage_visible: bool,
}

impl TestDecorationController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_decorations(&mut self, decorations: Vec<TestDecoration>) {
        self.decorations = decorations;
    }

    pub fn decorations(&self) -> &[TestDecoration] {
        &self.decorations
    }

    pub fn decoration_at_line(&self, line: u32) -> Option<&TestDecoration> {
        self.decorations.iter().find(|d| d.line == line)
    }

    pub fn update_test_state(&mut self, test_id: &str, state: TestState) {
        for dec in &mut self.decorations {
            if dec.test_id == test_id {
                dec.state = state;
            }
        }
    }

    pub fn reset_all_states(&mut self) {
        for dec in &mut self.decorations {
            dec.state = TestState::Unrun;
        }
    }

    pub fn gutter_action_at(&self, line: u32) -> Option<TestGutterAction> {
        self.decoration_at_line(line)
            .map(|_| TestGutterAction::default_actions(line))
    }

    // ── Inline errors ────────────────────────────────────────────────────

    pub fn set_inline_errors(&mut self, errors: Vec<TestInlineError>) {
        self.inline_errors = errors;
    }

    pub fn inline_errors(&self) -> &[TestInlineError] {
        &self.inline_errors
    }

    pub fn inline_error_at_line(&self, line: u32) -> Option<&TestInlineError> {
        self.inline_errors.iter().find(|e| e.line == line)
    }

    pub fn clear_inline_errors(&mut self) {
        self.inline_errors.clear();
    }

    // ── Coverage overlay ─────────────────────────────────────────────────

    pub fn set_coverage(&mut self, lines: HashMap<u32, LineCoverageState>) {
        self.coverage_lines = lines;
        self.coverage_visible = true;
    }

    pub fn clear_coverage(&mut self) {
        self.coverage_lines.clear();
        self.coverage_visible = false;
    }

    pub fn coverage_at_line(&self, line: u32) -> Option<LineCoverageState> {
        if self.coverage_visible {
            self.coverage_lines.get(&line).copied()
        } else {
            None
        }
    }

    pub fn toggle_coverage_visibility(&mut self) {
        self.coverage_visible = !self.coverage_visible;
    }

    pub fn is_coverage_visible(&self) -> bool {
        self.coverage_visible
    }

    /// Visible decorations within a line range (for rendering).
    pub fn visible_decorations(&self, first_line: u32, count: u32) -> Vec<&TestDecoration> {
        let end = first_line + count;
        self.decorations
            .iter()
            .filter(|d| d.line >= first_line && d.line < end)
            .collect()
    }

    /// Counts of each state for status bar display.
    pub fn state_counts(&self) -> HashMap<TestState, usize> {
        let mut counts = HashMap::new();
        for dec in &self.decorations {
            *counts.entry(dec.state).or_insert(0) += 1;
        }
        counts
    }

    pub fn summary_text(&self) -> String {
        let counts = self.state_counts();
        let passed = counts.get(&TestState::Passed).copied().unwrap_or(0);
        let failed = counts.get(&TestState::Failed).copied().unwrap_or(0);
        let total = self.decorations.len();
        format!("{passed}/{total} passed, {failed} failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_color_and_icon() {
        let (_, _, _, a) = TestState::Passed.color_rgba();
        assert!((a - 1.0).abs() < f32::EPSILON);
        assert_eq!(TestState::Failed.icon_char(), '✗');
        assert!(TestState::Passed.is_terminal());
        assert!(!TestState::Running.is_terminal());
    }

    #[test]
    fn detect_rust_tests() {
        let source = &[
            "#[test]",
            "fn test_addition() {",
            "    assert_eq!(2 + 2, 4);",
            "}",
            "",
            "#[tokio::test]",
            "async fn test_async() {",
            "    todo!();",
            "}",
        ];
        let decs = detect_test_functions(source, "rust");
        assert_eq!(decs.len(), 2);
        assert_eq!(decs[0].line, 0);
        assert_eq!(decs[0].test_name, "test_addition");
        assert_eq!(decs[1].line, 5);
        assert_eq!(decs[1].test_name, "test_async");
    }

    #[test]
    fn detect_js_tests() {
        let source = &[
            "describe('math', () => {",
            "  it('should add', () => {",
            "    expect(1 + 1).toBe(2);",
            "  });",
            "  test('subtract', () => {",
            "    expect(2 - 1).toBe(1);",
            "  });",
            "});",
        ];
        let decs = detect_test_functions(source, "javascript");
        assert_eq!(decs.len(), 3);
        assert_eq!(decs[0].test_name, "math");
        assert_eq!(decs[1].test_name, "should add");
        assert_eq!(decs[2].test_name, "subtract");
    }

    #[test]
    fn detect_python_tests() {
        let source = &[
            "def test_hello():",
            "    assert True",
            "",
            "class TestSuite:",
            "    def test_method(self):",
            "        pass",
        ];
        let decs = detect_test_functions(source, "python");
        assert_eq!(decs.len(), 3);
        assert_eq!(decs[0].test_name, "test_hello");
        assert_eq!(decs[1].test_name, "TestSuite");
        assert_eq!(decs[2].test_name, "test_method");
    }

    #[test]
    fn detect_go_tests() {
        let source = &[
            "func TestAdd(t *testing.T) {",
            "    if 1+1 != 2 { t.Fatal() }",
            "}",
            "",
            "func BenchmarkAdd(b *testing.B) {",
            "    for i := 0; i < b.N; i++ {}",
            "}",
        ];
        let decs = detect_test_functions(source, "go");
        assert_eq!(decs.len(), 2);
        assert_eq!(decs[0].test_name, "TestAdd");
        assert_eq!(decs[1].test_name, "BenchmarkAdd");
    }

    #[test]
    fn detect_unknown_language_returns_empty() {
        let source = &["something"];
        let decs = detect_test_functions(source, "brainfuck");
        assert!(decs.is_empty());
    }

    #[test]
    fn controller_update_state() {
        let mut ctrl = TestDecorationController::new();
        ctrl.set_decorations(vec![
            TestDecoration::new(0, "rust::test_a", "test_a"),
            TestDecoration::new(5, "rust::test_b", "test_b"),
        ]);
        ctrl.update_test_state("rust::test_a", TestState::Passed);
        ctrl.update_test_state("rust::test_b", TestState::Failed);
        assert_eq!(ctrl.decoration_at_line(0).unwrap().state, TestState::Passed);
        assert_eq!(ctrl.decoration_at_line(5).unwrap().state, TestState::Failed);
    }

    #[test]
    fn controller_summary() {
        let mut ctrl = TestDecorationController::new();
        ctrl.set_decorations(vec![
            TestDecoration::new(0, "a", "a").with_state(TestState::Passed),
            TestDecoration::new(5, "b", "b").with_state(TestState::Failed),
            TestDecoration::new(10, "c", "c").with_state(TestState::Passed),
        ]);
        assert_eq!(ctrl.summary_text(), "2/3 passed, 1 failed");
    }

    #[test]
    fn controller_visible_decorations() {
        let mut ctrl = TestDecorationController::new();
        ctrl.set_decorations(vec![
            TestDecoration::new(2, "a", "a"),
            TestDecoration::new(10, "b", "b"),
            TestDecoration::new(20, "c", "c"),
        ]);
        let visible = ctrl.visible_decorations(5, 15);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].test_name, "b");
    }

    #[test]
    fn controller_coverage() {
        let mut ctrl = TestDecorationController::new();
        let mut cov = HashMap::new();
        cov.insert(1, LineCoverageState::Covered);
        cov.insert(5, LineCoverageState::Uncovered);
        ctrl.set_coverage(cov);
        assert_eq!(ctrl.coverage_at_line(1), Some(LineCoverageState::Covered));
        assert_eq!(ctrl.coverage_at_line(5), Some(LineCoverageState::Uncovered));
        assert_eq!(ctrl.coverage_at_line(10), None);
        ctrl.toggle_coverage_visibility();
        assert_eq!(ctrl.coverage_at_line(1), None);
    }

    #[test]
    fn controller_inline_errors() {
        let mut ctrl = TestDecorationController::new();
        ctrl.set_inline_errors(vec![TestInlineError {
            line: 5,
            test_id: "rust::test_fail".into(),
            message: "assertion failed".into(),
            expected: Some("4".into()),
            actual: Some("5".into()),
        }]);
        let err = ctrl.inline_error_at_line(5).unwrap();
        assert_eq!(err.message, "assertion failed");
        assert!(ctrl.inline_error_at_line(6).is_none());
    }

    #[test]
    fn gutter_actions() {
        let mut ctrl = TestDecorationController::new();
        ctrl.set_decorations(vec![TestDecoration::new(3, "a", "a")]);
        let actions = ctrl.gutter_action_at(3).unwrap();
        assert_eq!(actions.actions.len(), 3);
        assert!(ctrl.gutter_action_at(99).is_none());
    }

    #[test]
    fn builtin_patterns_all_languages() {
        assert!(builtin_test_patterns("rust").is_some());
        assert!(builtin_test_patterns("javascript").is_some());
        assert!(builtin_test_patterns("typescript").is_some());
        assert!(builtin_test_patterns("python").is_some());
        assert!(builtin_test_patterns("go").is_some());
        assert!(builtin_test_patterns("cobol").is_none());
    }
}
