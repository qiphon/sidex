//! Test execution engine — spawns language-specific test runners, parses
//! output, streams results in real-time, supports cancel and re-run.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, SystemTime};

// ── Test state ───────────────────────────────────────────────────────────────

/// State of an individual test after execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TestState {
    Unrun,
    Running,
    Passed,
    Failed,
    Skipped,
    Errored,
}

// ── Test run profile ─────────────────────────────────────────────────────────

/// Kind of test run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestRunKind {
    Run,
    Debug,
    Coverage,
}

/// A named test execution profile.
#[derive(Debug, Clone)]
pub struct TestRunProfile {
    pub kind: TestRunKind,
    pub label: String,
    pub is_default: bool,
}

impl TestRunProfile {
    pub fn run() -> Self {
        Self {
            kind: TestRunKind::Run,
            label: "Run".into(),
            is_default: true,
        }
    }

    pub fn debug() -> Self {
        Self {
            kind: TestRunKind::Debug,
            label: "Debug".into(),
            is_default: false,
        }
    }

    pub fn coverage() -> Self {
        Self {
            kind: TestRunKind::Coverage,
            label: "Coverage".into(),
            is_default: false,
        }
    }
}

// ── Test location ────────────────────────────────────────────────────────────

/// Source location of a test for navigation on failure.
#[derive(Debug, Clone)]
pub struct TestLocation {
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
}

// ── Test error ───────────────────────────────────────────────────────────────

/// Structured error from a test failure.
#[derive(Debug, Clone)]
pub struct TestError {
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub stack_trace: Option<String>,
    pub location: Option<TestLocation>,
}

impl TestError {
    pub fn simple(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            expected: None,
            actual: None,
            stack_trace: None,
            location: None,
        }
    }

    pub fn with_diff(
        message: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            message: message.into(),
            expected: Some(expected.into()),
            actual: Some(actual.into()),
            stack_trace: None,
            location: None,
        }
    }

    pub fn has_diff(&self) -> bool {
        self.expected.is_some() && self.actual.is_some()
    }
}

// ── Test result ──────────────────────────────────────────────────────────────

/// The result of running a single test.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub test_id: String,
    pub state: TestState,
    pub duration: Duration,
    pub output: String,
    pub error: Option<TestError>,
}

impl TestResult {
    pub fn passed(test_id: impl Into<String>, duration: Duration) -> Self {
        Self {
            test_id: test_id.into(),
            state: TestState::Passed,
            duration,
            output: String::new(),
            error: None,
        }
    }

    pub fn failed(test_id: impl Into<String>, duration: Duration, error: TestError) -> Self {
        Self {
            test_id: test_id.into(),
            state: TestState::Failed,
            duration,
            output: String::new(),
            error: Some(error),
        }
    }

    pub fn skipped(test_id: impl Into<String>) -> Self {
        Self {
            test_id: test_id.into(),
            state: TestState::Skipped,
            duration: Duration::ZERO,
            output: String::new(),
            error: None,
        }
    }
}

// ── Test run state ───────────────────────────────────────────────────────────

/// State of an entire test run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestRunState {
    Queued,
    Running,
    Completed,
    Cancelled,
}

// ── Test run ─────────────────────────────────────────────────────────────────

/// A single test execution session.
pub struct TestRun {
    pub id: String,
    pub profile: TestRunProfile,
    pub state: TestRunState,
    pub results: Vec<TestResult>,
    pub started_at: SystemTime,
    pub test_ids: Vec<String>,
    pub workspace: PathBuf,
    process: Option<Child>,
    raw_output: String,
}

impl TestRun {
    pub fn new(
        id: impl Into<String>,
        profile: TestRunProfile,
        test_ids: Vec<String>,
        workspace: PathBuf,
    ) -> Self {
        Self {
            id: id.into(),
            profile,
            state: TestRunState::Queued,
            results: Vec::new(),
            started_at: SystemTime::now(),
            test_ids,
            workspace,
            process: None,
            raw_output: String::new(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed().unwrap_or(Duration::ZERO)
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.state, TestRunState::Completed | TestRunState::Cancelled)
    }

    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.state == TestState::Passed).count()
    }

    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| r.state == TestState::Failed).count()
    }

    pub fn total_count(&self) -> usize {
        self.test_ids.len()
    }

    pub fn summary_text(&self) -> String {
        format!(
            "{}/{} passed, {} failed ({:.1}s)",
            self.passed_count(),
            self.total_count(),
            self.failed_count(),
            self.elapsed().as_secs_f64()
        )
    }

    pub fn add_result(&mut self, result: TestResult) {
        self.results.push(result);
    }

    pub fn append_output(&mut self, text: &str) {
        self.raw_output.push_str(text);
    }

    pub fn raw_output(&self) -> &str {
        &self.raw_output
    }

    pub fn set_process(&mut self, child: Child) {
        self.process = Some(child);
    }

    /// Attempts to cancel the running process.
    pub fn cancel(&mut self) -> Result<(), std::io::Error> {
        self.state = TestRunState::Cancelled;
        if let Some(ref mut child) = self.process {
            child.kill()?;
        }
        Ok(())
    }
}

// ── Runner kind ──────────────────────────────────────────────────────────────

/// Language/framework-specific test runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerKind {
    CargoTest,
    Jest,
    Vitest,
    Mocha,
    Pytest,
    GoTest,
    Custom(String),
}

impl RunnerKind {
    /// Detects the appropriate runner for a workspace.
    pub fn detect(workspace: &Path) -> Option<Self> {
        if workspace.join("Cargo.toml").exists() {
            return Some(Self::CargoTest);
        }
        if workspace.join("vitest.config.ts").exists()
            || workspace.join("vitest.config.js").exists()
        {
            return Some(Self::Vitest);
        }
        if workspace.join("jest.config.js").exists()
            || workspace.join("jest.config.ts").exists()
        {
            return Some(Self::Jest);
        }
        if workspace.join(".mocharc.yml").exists()
            || workspace.join(".mocharc.json").exists()
        {
            return Some(Self::Mocha);
        }
        if workspace.join("pytest.ini").exists()
            || workspace.join("pyproject.toml").exists()
            || workspace.join("setup.py").exists()
        {
            return Some(Self::Pytest);
        }
        if workspace.join("go.mod").exists() {
            return Some(Self::GoTest);
        }
        None
    }

    /// Returns the base command and args for this runner.
    pub fn command_and_args(&self) -> (&str, Vec<&str>) {
        match self {
            Self::CargoTest => ("cargo", vec!["test"]),
            Self::Jest => ("npx", vec!["jest"]),
            Self::Vitest => ("npx", vec!["vitest", "run"]),
            Self::Mocha => ("npx", vec!["mocha"]),
            Self::Pytest => ("python", vec!["-m", "pytest"]),
            Self::GoTest => ("go", vec!["test"]),
            Self::Custom(cmd) => (cmd.as_str(), vec![]),
        }
    }

    /// Additional args for running specific tests.
    pub fn filter_args(&self, test_names: &[String]) -> Vec<String> {
        if test_names.is_empty() {
            return Vec::new();
        }
        match self {
            Self::CargoTest => {
                let mut args = vec!["--".to_string()];
                for name in test_names {
                    args.push(name.clone());
                }
                args
            }
            Self::Jest | Self::Vitest => {
                let pattern = test_names.join("|");
                vec!["-t".to_string(), pattern]
            }
            Self::Mocha => {
                let pattern = test_names.join("|");
                vec!["--grep".to_string(), pattern]
            }
            Self::Pytest => {
                let mut args = Vec::new();
                for name in test_names {
                    args.push("-k".to_string());
                    args.push(name.clone());
                }
                args
            }
            Self::GoTest => {
                let pattern = test_names.join("|");
                vec!["-run".to_string(), pattern]
            }
            Self::Custom(_) => test_names.to_vec(),
        }
    }

    /// Additional args for coverage mode.
    pub fn coverage_args(&self) -> Vec<String> {
        match self {
            Self::CargoTest => vec![],
            Self::Jest => vec!["--coverage".to_string()],
            Self::Vitest => vec!["--coverage".to_string()],
            Self::Mocha => vec![],
            Self::Pytest => vec!["--cov".to_string()],
            Self::GoTest => vec!["-coverprofile=coverage.out".to_string()],
            Self::Custom(_) => vec![],
        }
    }
}

// ── Output parsing ───────────────────────────────────────────────────────────

/// Parses a line of cargo test output into a `TestResult` if it matches.
pub fn parse_cargo_test_line(line: &str) -> Option<TestResult> {
    let trimmed = line.trim();

    if let Some(rest) = trimmed.strip_prefix("test ") {
        if let Some(name_end) = rest.find(" ... ") {
            let name = &rest[..name_end];
            let status = rest[name_end + 5..].trim();
            return match status {
                "ok" => Some(TestResult::passed(name, Duration::ZERO)),
                "FAILED" => Some(TestResult::failed(
                    name,
                    Duration::ZERO,
                    TestError::simple("test failed"),
                )),
                "ignored" => Some(TestResult::skipped(name)),
                _ => None,
            };
        }
    }
    None
}

/// Parses a line of pytest output into a `TestResult` if it matches.
pub fn parse_pytest_line(line: &str) -> Option<TestResult> {
    let trimmed = line.trim();

    if trimmed.contains("PASSED") {
        let name = trimmed.split("PASSED").next()?.trim().trim_end_matches(' ');
        return Some(TestResult::passed(name, Duration::ZERO));
    }
    if trimmed.contains("FAILED") {
        let name = trimmed.split("FAILED").next()?.trim().trim_end_matches(' ');
        return Some(TestResult::failed(
            name,
            Duration::ZERO,
            TestError::simple("test failed"),
        ));
    }
    if trimmed.contains("SKIPPED") {
        let name = trimmed.split("SKIPPED").next()?.trim().trim_end_matches(' ');
        return Some(TestResult::skipped(name));
    }
    None
}

/// Parses a line of go test output into a `TestResult` if it matches.
pub fn parse_go_test_line(line: &str) -> Option<TestResult> {
    let trimmed = line.trim();

    if let Some(rest) = trimmed.strip_prefix("--- PASS: ") {
        let name = rest.split_whitespace().next()?;
        return Some(TestResult::passed(name, Duration::ZERO));
    }
    if let Some(rest) = trimmed.strip_prefix("--- FAIL: ") {
        let name = rest.split_whitespace().next()?;
        return Some(TestResult::failed(
            name,
            Duration::ZERO,
            TestError::simple("test failed"),
        ));
    }
    if let Some(rest) = trimmed.strip_prefix("--- SKIP: ") {
        let name = rest.split_whitespace().next()?;
        return Some(TestResult::skipped(name));
    }
    None
}

// ── Test runner ──────────────────────────────────────────────────────────────

/// Top-level test execution manager.
pub struct TestRunner {
    pub active_runs: HashMap<String, TestRun>,
    pub profiles: Vec<TestRunProfile>,
    next_run_id: u64,
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            active_runs: HashMap::new(),
            profiles: vec![
                TestRunProfile::run(),
                TestRunProfile::debug(),
                TestRunProfile::coverage(),
            ],
            next_run_id: 0,
        }
    }

    /// Creates a new test run (but does not start the process).
    pub fn create_run(
        &mut self,
        tests: Vec<String>,
        profile: TestRunProfile,
        workspace: PathBuf,
    ) -> String {
        let id = format!("run-{}", self.next_run_id);
        self.next_run_id += 1;
        let run = TestRun::new(&id, profile, tests, workspace);
        self.active_runs.insert(id.clone(), run);
        id
    }

    /// Builds the `Command` for a test run, suitable for spawning.
    pub fn build_command(
        &self,
        run_id: &str,
    ) -> Option<std::process::Command> {
        let run = self.active_runs.get(run_id)?;
        let runner = RunnerKind::detect(&run.workspace)?;
        let (cmd, base_args) = runner.command_and_args();
        let mut command = std::process::Command::new(cmd);
        command.current_dir(&run.workspace);
        for arg in &base_args {
            command.arg(arg);
        }
        let filter = runner.filter_args(&run.test_ids);
        for arg in &filter {
            command.arg(arg);
        }
        if run.profile.kind == TestRunKind::Coverage {
            for arg in &runner.coverage_args() {
                command.arg(arg);
            }
        }
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        Some(command)
    }

    /// Marks a run as started with the given child process.
    pub fn start_run(&mut self, run_id: &str, child: Child) {
        if let Some(run) = self.active_runs.get_mut(run_id) {
            run.state = TestRunState::Running;
            run.set_process(child);
        }
    }

    /// Cancels a running test.
    pub fn cancel_run(&mut self, run_id: &str) -> Result<(), std::io::Error> {
        if let Some(run) = self.active_runs.get_mut(run_id) {
            run.cancel()?;
        }
        Ok(())
    }

    /// Marks a run as completed.
    pub fn complete_run(&mut self, run_id: &str) {
        if let Some(run) = self.active_runs.get_mut(run_id) {
            run.state = TestRunState::Completed;
        }
    }

    /// Adds a test result to a run.
    pub fn add_result(&mut self, run_id: &str, result: TestResult) {
        if let Some(run) = self.active_runs.get_mut(run_id) {
            run.add_result(result);
        }
    }

    /// Gets a reference to a run.
    pub fn get_run(&self, run_id: &str) -> Option<&TestRun> {
        self.active_runs.get(run_id)
    }

    /// Removes completed runs older than the given duration.
    pub fn prune_old_runs(&mut self, max_age: Duration) {
        self.active_runs.retain(|_, run| {
            if run.is_finished() {
                run.elapsed() < max_age
            } else {
                true
            }
        });
    }

    /// Collects failed test IDs from the most recent completed run.
    pub fn last_failed_ids(&self) -> Vec<String> {
        self.active_runs
            .values()
            .filter(|r| r.state == TestRunState::Completed)
            .max_by_key(|r| r.started_at)
            .map(|r| {
                r.results
                    .iter()
                    .filter(|t| t.state == TestState::Failed || t.state == TestState::Errored)
                    .map(|t| t.test_id.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_profile_defaults() {
        let p = TestRunProfile::run();
        assert_eq!(p.kind, TestRunKind::Run);
        assert!(p.is_default);
    }

    #[test]
    fn test_error_simple() {
        let err = TestError::simple("oops");
        assert_eq!(err.message, "oops");
        assert!(!err.has_diff());
    }

    #[test]
    fn test_error_with_diff() {
        let err = TestError::with_diff("mismatch", "4", "5");
        assert!(err.has_diff());
        assert_eq!(err.expected.as_deref(), Some("4"));
        assert_eq!(err.actual.as_deref(), Some("5"));
    }

    #[test]
    fn test_result_constructors() {
        let p = TestResult::passed("test_a", Duration::from_millis(10));
        assert_eq!(p.state, TestState::Passed);
        assert!(p.error.is_none());

        let f = TestResult::failed("test_b", Duration::from_millis(5), TestError::simple("fail"));
        assert_eq!(f.state, TestState::Failed);
        assert!(f.error.is_some());

        let s = TestResult::skipped("test_c");
        assert_eq!(s.state, TestState::Skipped);
    }

    #[test]
    fn parse_cargo_test_lines() {
        assert!(
            matches!(
                parse_cargo_test_line("test my_module::tests::test_add ... ok"),
                Some(TestResult { state: TestState::Passed, .. })
            )
        );
        assert!(
            matches!(
                parse_cargo_test_line("test my_module::tests::test_fail ... FAILED"),
                Some(TestResult { state: TestState::Failed, .. })
            )
        );
        assert!(
            matches!(
                parse_cargo_test_line("test my_module::tests::test_skip ... ignored"),
                Some(TestResult { state: TestState::Skipped, .. })
            )
        );
        assert!(parse_cargo_test_line("running 3 tests").is_none());
    }

    #[test]
    fn parse_go_test_lines() {
        assert!(
            matches!(
                parse_go_test_line("--- PASS: TestAdd (0.00s)"),
                Some(TestResult { state: TestState::Passed, .. })
            )
        );
        assert!(
            matches!(
                parse_go_test_line("--- FAIL: TestBad (0.01s)"),
                Some(TestResult { state: TestState::Failed, .. })
            )
        );
        assert!(parse_go_test_line("ok  \tpkg\t0.123s").is_none());
    }

    #[test]
    fn runner_kind_command() {
        let (cmd, args) = RunnerKind::CargoTest.command_and_args();
        assert_eq!(cmd, "cargo");
        assert_eq!(args, vec!["test"]);

        let (cmd, args) = RunnerKind::Pytest.command_and_args();
        assert_eq!(cmd, "python");
        assert_eq!(args, vec!["-m", "pytest"]);
    }

    #[test]
    fn runner_kind_filter_args() {
        let filter = RunnerKind::CargoTest.filter_args(&["test_add".to_string()]);
        assert_eq!(filter, vec!["--", "test_add"]);

        let filter = RunnerKind::Jest.filter_args(&["add".to_string(), "sub".to_string()]);
        assert_eq!(filter, vec!["-t", "add|sub"]);

        let empty = RunnerKind::CargoTest.filter_args(&[]);
        assert!(empty.is_empty());
    }

    #[test]
    fn test_runner_create_and_manage() {
        let mut runner = TestRunner::new();
        let id = runner.create_run(
            vec!["test_a".into(), "test_b".into()],
            TestRunProfile::run(),
            PathBuf::from("/workspace"),
        );
        assert!(runner.get_run(&id).is_some());

        let run = runner.get_run(&id).unwrap();
        assert_eq!(run.state, TestRunState::Queued);
        assert_eq!(run.total_count(), 2);

        runner.add_result(&id, TestResult::passed("test_a", Duration::from_millis(5)));
        runner.add_result(
            &id,
            TestResult::failed("test_b", Duration::from_millis(10), TestError::simple("fail")),
        );
        runner.complete_run(&id);

        let run = runner.get_run(&id).unwrap();
        assert_eq!(run.state, TestRunState::Completed);
        assert_eq!(run.passed_count(), 1);
        assert_eq!(run.failed_count(), 1);
        assert!(run.is_finished());
    }

    #[test]
    fn test_runner_last_failed() {
        let mut runner = TestRunner::new();
        let id = runner.create_run(
            vec!["a".into(), "b".into()],
            TestRunProfile::run(),
            PathBuf::from("/ws"),
        );
        runner.add_result(&id, TestResult::passed("a", Duration::ZERO));
        runner.add_result(
            &id,
            TestResult::failed("b", Duration::ZERO, TestError::simple("x")),
        );
        runner.complete_run(&id);

        let failed = runner.last_failed_ids();
        assert_eq!(failed, vec!["b"]);
    }

    #[test]
    fn test_run_summary_text() {
        let mut run = TestRun::new("r1", TestRunProfile::run(), vec!["a".into(), "b".into()], PathBuf::from("/ws"));
        run.state = TestRunState::Completed;
        run.add_result(TestResult::passed("a", Duration::ZERO));
        run.add_result(TestResult::failed("b", Duration::ZERO, TestError::simple("x")));
        let text = run.summary_text();
        assert!(text.contains("1/2 passed"));
        assert!(text.contains("1 failed"));
    }

    #[test]
    fn coverage_args_populated() {
        let args = RunnerKind::Jest.coverage_args();
        assert_eq!(args, vec!["--coverage"]);

        let args = RunnerKind::GoTest.coverage_args();
        assert_eq!(args, vec!["-coverprofile=coverage.out"]);
    }
}
