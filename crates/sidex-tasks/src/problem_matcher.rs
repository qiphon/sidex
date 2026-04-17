//! Problem matchers — parse build output lines into structured diagnostics,
//! mirrors VS Code's `ProblemMatcher` / `ProblemPattern`.

use regex::Regex;

/// How file paths in problem output should be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileLocation {
    Relative,
    Absolute,
    AutoDetect,
}

/// Background task matcher — identifies start/end patterns for background tasks.
#[derive(Debug, Clone)]
pub struct BackgroundMatcher {
    pub active_on_start: bool,
    pub begin_pattern: String,
    pub end_pattern: String,
}

/// Severity level for a matched problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Diagnostic severity aligned with LSP conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

/// A matched diagnostic extracted from a build-output line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub file: String,
    pub line: u32,
    pub column: Option<u32>,
    pub end_line: Option<u32>,
    pub end_column: Option<u32>,
    pub severity: Severity,
    pub message: String,
    pub code: Option<String>,
}

/// A regex pattern that captures file, line, column, severity, and message
/// groups from a single line of build output.
#[derive(Debug, Clone)]
pub struct ProblemPattern {
    pub regexp: Regex,
    pub file_group: usize,
    pub line_group: usize,
    pub column_group: usize,
    pub end_line_group: usize,
    pub end_column_group: usize,
    pub severity_group: usize,
    pub message_group: usize,
    pub code_group: usize,
    pub loop_: bool,
}

/// A named problem matcher with one or more patterns.
#[derive(Debug, Clone)]
pub struct ProblemMatcher {
    pub name: String,
    pub owner: String,
    pub file_location: FileLocation,
    pub patterns: Vec<ProblemPattern>,
    pub severity: Option<DiagnosticSeverity>,
    pub background: Option<BackgroundMatcher>,
}

/// Attempts to match a single output line against a problem matcher.
///
/// Uses the first pattern in the matcher. Multi-line patterns are not yet
/// fully supported.
#[must_use]
pub fn match_line(line: &str, matcher: &ProblemMatcher) -> Option<Diagnostic> {
    let pattern = matcher.patterns.first()?;
    let caps = pattern.regexp.captures(line)?;

    let file = caps.get(pattern.file_group)?.as_str().to_string();

    let line_no: u32 = caps.get(pattern.line_group)?.as_str().parse().ok()?;

    let column = if pattern.column_group > 0 {
        caps.get(pattern.column_group)
            .and_then(|m| m.as_str().parse().ok())
    } else {
        None
    };

    let end_line = if pattern.end_line_group > 0 {
        caps.get(pattern.end_line_group)
            .and_then(|m| m.as_str().parse().ok())
    } else {
        None
    };

    let end_column = if pattern.end_column_group > 0 {
        caps.get(pattern.end_column_group)
            .and_then(|m| m.as_str().parse().ok())
    } else {
        None
    };

    let severity = if pattern.severity_group > 0 {
        caps.get(pattern.severity_group)
            .map_or(Severity::Error, |m| parse_severity(m.as_str()))
    } else {
        Severity::Error
    };

    let message = caps
        .get(pattern.message_group)
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    let code = if pattern.code_group > 0 {
        caps.get(pattern.code_group)
            .map(|m| m.as_str().to_string())
    } else {
        None
    };

    Some(Diagnostic {
        file,
        line: line_no,
        column,
        end_line,
        end_column,
        severity,
        message,
        code,
    })
}

/// Parses all output lines through a problem matcher, returning diagnostics.
#[must_use]
pub fn parse_problem_output(output: &str, matcher: &ProblemMatcher) -> Vec<Diagnostic> {
    output
        .lines()
        .filter_map(|line| match_line(line, matcher))
        .collect()
}

fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "warning" | "warn" => Severity::Warning,
        "info" | "information" | "note" | "hint" => Severity::Info,
        _ => Severity::Error,
    }
}

// ── Built-in matchers ──────────────────────────────────────────────────────

fn make_matcher(name: &str, pattern: ProblemPattern) -> ProblemMatcher {
    ProblemMatcher {
        name: name.into(),
        owner: name.into(),
        file_location: FileLocation::AutoDetect,
        patterns: vec![pattern],
        severity: None,
        background: None,
    }
}

fn make_pattern(
    regexp: &str,
    file: usize,
    line: usize,
    column: usize,
    severity: usize,
    message: usize,
) -> ProblemPattern {
    ProblemPattern {
        regexp: Regex::new(regexp).expect("problem matcher regex"),
        file_group: file,
        line_group: line,
        column_group: column,
        end_line_group: 0,
        end_column_group: 0,
        severity_group: severity,
        message_group: message,
        code_group: 0,
        loop_: false,
    }
}

/// Creates the built-in `$tsc` (TypeScript compiler) problem matcher.
///
/// Matches lines like: `src/app.ts(10,5): error TS2322: ...`
#[must_use]
pub fn builtin_tsc() -> ProblemMatcher {
    make_matcher(
        "$tsc",
        make_pattern(
            r"^(.+)\((\d+),(\d+)\):\s+(error|warning)\s+TS\d+:\s+(.+)$",
            1, 2, 3, 4, 5,
        ),
    )
}

/// Creates the built-in `$gcc` problem matcher for GCC/Clang.
///
/// Matches lines like: `src/main.c:10:5: error: undeclared identifier`
#[must_use]
pub fn builtin_gcc() -> ProblemMatcher {
    make_matcher(
        "$gcc",
        make_pattern(
            r"^(.+):(\d+):(\d+):\s+(error|warning|note):\s+(.+)$",
            1, 2, 3, 4, 5,
        ),
    )
}

/// Creates the built-in `$msvc` problem matcher for Microsoft Visual C++.
///
/// Matches lines like: `main.cpp(10): error C2065: 'foo': undeclared identifier`
#[must_use]
pub fn builtin_msvc() -> ProblemMatcher {
    make_matcher(
        "$msvc",
        make_pattern(
            r"^(.+)\((\d+)\):\s+(error|warning)\s+\w+:\s+(.+)$",
            1, 2, 0, 3, 4,
        ),
    )
}

/// Creates the built-in `$rustc` problem matcher.
///
/// Matches the location line: `  --> src/main.rs:10:5`
#[must_use]
pub fn builtin_rustc() -> ProblemMatcher {
    make_matcher(
        "$rustc",
        make_pattern(r"^\s+-->\s+(.+):(\d+):(\d+)$", 1, 2, 3, 0, 0),
    )
}

/// Creates the built-in `$eslint` problem matcher (stylish reporter).
///
/// Matches lines like: `  10:5  error  Missing semicolon  semi`
#[must_use]
pub fn builtin_eslint() -> ProblemMatcher {
    make_matcher(
        "$eslint",
        make_pattern(
            r"^\s+(\d+):(\d+)\s+(error|warning)\s+(.+?)\s+\S+$",
            0, 1, 2, 3, 4,
        ),
    )
}

/// Creates the built-in `$go` problem matcher.
///
/// Matches lines like: `./main.go:10:5: undefined: foo`
#[must_use]
pub fn builtin_go() -> ProblemMatcher {
    make_matcher(
        "$go",
        make_pattern(r"^(.+):(\d+):(\d+):\s+(.+)$", 1, 2, 3, 0, 4),
    )
}

/// Creates the built-in `$python` problem matcher.
///
/// Matches lines like: `  File "main.py", line 10`
#[must_use]
pub fn builtin_python() -> ProblemMatcher {
    make_matcher(
        "$python",
        make_pattern(
            r#"^\s+File "(.+)", line (\d+)"#,
            1, 2, 0, 0, 0,
        ),
    )
}

/// Creates the built-in `$javac` problem matcher.
///
/// Matches lines like: `Main.java:10: error: ';' expected`
#[must_use]
pub fn builtin_javac() -> ProblemMatcher {
    make_matcher(
        "$javac",
        make_pattern(
            r"^(.+):(\d+):\s+(error|warning):\s+(.+)$",
            1, 2, 0, 3, 4,
        ),
    )
}

/// Returns all built-in problem matchers indexed by name.
#[must_use]
pub fn builtin_matchers() -> Vec<ProblemMatcher> {
    vec![
        builtin_tsc(),
        builtin_gcc(),
        builtin_msvc(),
        builtin_rustc(),
        builtin_eslint(),
        builtin_go(),
        builtin_python(),
        builtin_javac(),
    ]
}

/// Looks up a built-in matcher by name (e.g. "$tsc").
#[must_use]
pub fn find_builtin(name: &str) -> Option<ProblemMatcher> {
    builtin_matchers().into_iter().find(|m| m.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_tsc_error() {
        let matcher = builtin_tsc();
        let line =
            "src/app.ts(10,5): error TS2322: Type 'string' is not assignable to type 'number'.";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.file, "src/app.ts");
        assert_eq!(diag.line, 10);
        assert_eq!(diag.column, Some(5));
        assert_eq!(diag.severity, Severity::Error);
        assert!(diag.message.contains("not assignable"));
    }

    #[test]
    fn match_tsc_warning() {
        let matcher = builtin_tsc();
        let line =
            "src/utils.ts(3,1): warning TS6133: 'x' is declared but its value is never read.";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn match_gcc_error() {
        let matcher = builtin_gcc();
        let line = "src/main.c:42:10: error: undeclared identifier 'foo'";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.file, "src/main.c");
        assert_eq!(diag.line, 42);
        assert_eq!(diag.column, Some(10));
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(diag.message, "undeclared identifier 'foo'");
    }

    #[test]
    fn match_gcc_warning() {
        let matcher = builtin_gcc();
        let line = "lib.c:5:3: warning: unused variable 'x'";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.severity, Severity::Warning);
    }

    #[test]
    fn match_gcc_note() {
        let matcher = builtin_gcc();
        let line = "lib.c:5:3: note: declared here";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.severity, Severity::Info);
    }

    #[test]
    fn match_msvc_error() {
        let matcher = builtin_msvc();
        let line = "main.cpp(10): error C2065: 'foo': undeclared identifier";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.file, "main.cpp");
        assert_eq!(diag.line, 10);
        assert_eq!(diag.severity, Severity::Error);
    }

    #[test]
    fn match_rustc_location() {
        let matcher = builtin_rustc();
        let line = "  --> src/main.rs:10:5";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.file, "src/main.rs");
        assert_eq!(diag.line, 10);
        assert_eq!(diag.column, Some(5));
    }

    #[test]
    fn match_go_error() {
        let matcher = builtin_go();
        let line = "./main.go:42:5: undefined: myFunc";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.file, "./main.go");
        assert_eq!(diag.line, 42);
        assert_eq!(diag.column, Some(5));
        assert_eq!(diag.message, "undefined: myFunc");
    }

    #[test]
    fn match_javac_error() {
        let matcher = builtin_javac();
        let line = "Main.java:10: error: ';' expected";
        let diag = match_line(line, &matcher).unwrap();
        assert_eq!(diag.file, "Main.java");
        assert_eq!(diag.line, 10);
        assert_eq!(diag.severity, Severity::Error);
    }

    #[test]
    fn no_match_returns_none() {
        let matcher = builtin_tsc();
        assert!(match_line("hello world", &matcher).is_none());
    }

    #[test]
    fn find_builtin_by_name() {
        assert!(find_builtin("$tsc").is_some());
        assert!(find_builtin("$gcc").is_some());
        assert!(find_builtin("$msvc").is_some());
        assert!(find_builtin("$rustc").is_some());
        assert!(find_builtin("$eslint").is_some());
        assert!(find_builtin("$go").is_some());
        assert!(find_builtin("$python").is_some());
        assert!(find_builtin("$javac").is_some());
        assert!(find_builtin("$nonexistent").is_none());
    }

    #[test]
    fn builtin_matchers_count() {
        assert_eq!(builtin_matchers().len(), 8);
    }

    #[test]
    fn parse_problem_output_multiple() {
        let matcher = builtin_gcc();
        let output = "\
src/a.c:1:1: error: foo\n\
src/b.c:2:2: warning: bar\n\
some random line\n\
src/c.c:3:3: note: baz\n";
        let diags = parse_problem_output(output, &matcher);
        assert_eq!(diags.len(), 3);
        assert_eq!(diags[0].file, "src/a.c");
        assert_eq!(diags[1].severity, Severity::Warning);
        assert_eq!(diags[2].severity, Severity::Info);
    }

    #[test]
    fn severity_parsing() {
        assert_eq!(parse_severity("error"), Severity::Error);
        assert_eq!(parse_severity("Error"), Severity::Error);
        assert_eq!(parse_severity("warning"), Severity::Warning);
        assert_eq!(parse_severity("warn"), Severity::Warning);
        assert_eq!(parse_severity("info"), Severity::Info);
        assert_eq!(parse_severity("note"), Severity::Info);
        assert_eq!(parse_severity("hint"), Severity::Info);
        assert_eq!(parse_severity("garbage"), Severity::Error);
    }
}
