//! Diagnostic rendering data — squiggly underline decorations, severity
//! mapping, positional queries, and navigation helpers for the editor layer.

use sidex_text::{Position, Range};

// ── Severity ────────────────────────────────────────────────────────────────

/// Diagnostic severity levels with associated rendering colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    /// Returns the RGBA color tuple for this severity.
    /// Error=red, Warning=yellow, Information=blue, Hint=grey.
    pub fn color_rgba(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Error => (0.957, 0.278, 0.278, 1.0),
            Self::Warning => (0.804, 0.678, 0.0, 1.0),
            Self::Information => (0.216, 0.580, 1.0, 1.0),
            Self::Hint => (0.627, 0.627, 0.627, 0.7),
        }
    }
}

// ── DiagnosticDecoration ────────────────────────────────────────────────────

/// Data needed to render a squiggly underline for a single diagnostic.
#[derive(Debug, Clone)]
pub struct DiagnosticDecoration {
    /// The range in the document this diagnostic covers.
    pub range: Range,
    /// Severity determines the underline color.
    pub severity: DiagnosticSeverity,
    /// The human-readable error/warning message.
    pub message: String,
    /// The source of the diagnostic (e.g. `"rust-analyzer"`, `"eslint"`).
    pub source: Option<String>,
    /// An optional error code (e.g. `"E0308"`, `"no-unused-vars"`).
    pub code: Option<String>,
    /// Whether the diagnostic is stale (document edited since received).
    pub is_stale: bool,
}

impl DiagnosticDecoration {
    /// Returns `true` if the given position falls within this decoration's range.
    pub fn contains(&self, pos: Position) -> bool {
        self.range.contains(pos)
    }
}

// ── Diagnostic ──────────────────────────────────────────────────────────────

/// Simplified diagnostic input (decoupled from `lsp_types`).
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
}

// ── Computation ─────────────────────────────────────────────────────────────

/// Converts a slice of diagnostics into decoration data suitable for rendering.
pub fn compute_diagnostic_decorations(diagnostics: &[Diagnostic]) -> Vec<DiagnosticDecoration> {
    diagnostics
        .iter()
        .map(|d| DiagnosticDecoration {
            range: d.range,
            severity: d.severity,
            message: d.message.clone(),
            source: d.source.clone(),
            code: d.code.clone(),
            is_stale: false,
        })
        .collect()
}

/// Finds the diagnostic decoration at the given cursor position (for hover).
pub fn diagnostic_at_position<'a>(
    pos: Position,
    diagnostics: &'a [DiagnosticDecoration],
) -> Option<&'a DiagnosticDecoration> {
    diagnostics
        .iter()
        .filter(|d| d.contains(pos))
        .min_by_key(|d| d.severity)
}

/// Finds the next diagnostic after `pos` (for F8 navigation).
/// If no diagnostic exists after `pos`, wraps to the first diagnostic.
pub fn next_diagnostic<'a>(
    pos: Position,
    diagnostics: &'a [DiagnosticDecoration],
) -> Option<&'a DiagnosticDecoration> {
    if diagnostics.is_empty() {
        return None;
    }
    diagnostics
        .iter()
        .filter(|d| d.range.start > pos)
        .min_by_key(|d| d.range.start)
        .or_else(|| {
            diagnostics
                .iter()
                .min_by_key(|d| d.range.start)
        })
}

/// Finds the previous diagnostic before `pos` (for Shift+F8 navigation).
/// If no diagnostic exists before `pos`, wraps to the last diagnostic.
pub fn prev_diagnostic<'a>(
    pos: Position,
    diagnostics: &'a [DiagnosticDecoration],
) -> Option<&'a DiagnosticDecoration> {
    if diagnostics.is_empty() {
        return None;
    }
    diagnostics
        .iter()
        .filter(|d| d.range.start < pos)
        .max_by_key(|d| d.range.start)
        .or_else(|| {
            diagnostics
                .iter()
                .max_by_key(|d| d.range.start)
        })
}

/// Returns all diagnostics for a specific line (for gutter icon rendering).
pub fn diagnostics_on_line(
    line: u32,
    diagnostics: &[DiagnosticDecoration],
) -> Vec<&DiagnosticDecoration> {
    diagnostics
        .iter()
        .filter(|d| d.range.start.line <= line && d.range.end.line >= line)
        .collect()
}

/// Returns the highest severity among diagnostics on a given line.
pub fn highest_severity_on_line(
    line: u32,
    diagnostics: &[DiagnosticDecoration],
) -> Option<DiagnosticSeverity> {
    diagnostics_on_line(line, diagnostics)
        .into_iter()
        .map(|d| d.severity)
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_diag(
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
        severity: DiagnosticSeverity,
        message: &str,
    ) -> Diagnostic {
        Diagnostic {
            range: Range::new(
                Position::new(start_line, start_col),
                Position::new(end_line, end_col),
            ),
            severity,
            message: message.to_string(),
            source: Some("test".to_string()),
            code: None,
        }
    }

    fn make_decorations() -> Vec<DiagnosticDecoration> {
        let diags = vec![
            make_diag(2, 5, 2, 15, DiagnosticSeverity::Error, "error on line 2"),
            make_diag(5, 0, 5, 10, DiagnosticSeverity::Warning, "warning on line 5"),
            make_diag(10, 3, 10, 20, DiagnosticSeverity::Information, "info on line 10"),
            make_diag(2, 20, 2, 30, DiagnosticSeverity::Hint, "hint on line 2"),
        ];
        compute_diagnostic_decorations(&diags)
    }

    #[test]
    fn compute_decorations_preserves_all() {
        let decorations = make_decorations();
        assert_eq!(decorations.len(), 4);
        assert_eq!(decorations[0].severity, DiagnosticSeverity::Error);
        assert_eq!(decorations[1].severity, DiagnosticSeverity::Warning);
    }

    #[test]
    fn diagnostic_at_position_finds_matching() {
        let decorations = make_decorations();
        let pos = Position::new(2, 10);
        let found = diagnostic_at_position(pos, &decorations);
        assert!(found.is_some());
        assert_eq!(found.unwrap().severity, DiagnosticSeverity::Error);
    }

    #[test]
    fn diagnostic_at_position_returns_none_when_no_match() {
        let decorations = make_decorations();
        let pos = Position::new(7, 0);
        assert!(diagnostic_at_position(pos, &decorations).is_none());
    }

    #[test]
    fn diagnostic_at_position_prefers_higher_severity() {
        let diags = vec![
            make_diag(1, 0, 1, 20, DiagnosticSeverity::Warning, "warn"),
            make_diag(1, 5, 1, 15, DiagnosticSeverity::Error, "err"),
        ];
        let decorations = compute_diagnostic_decorations(&diags);
        let found = diagnostic_at_position(Position::new(1, 10), &decorations);
        assert_eq!(found.unwrap().severity, DiagnosticSeverity::Error);
    }

    #[test]
    fn next_diagnostic_finds_after_pos() {
        let decorations = make_decorations();
        let next = next_diagnostic(Position::new(3, 0), &decorations);
        assert!(next.is_some());
        assert_eq!(next.unwrap().range.start.line, 5);
    }

    #[test]
    fn next_diagnostic_wraps_around() {
        let decorations = make_decorations();
        let next = next_diagnostic(Position::new(20, 0), &decorations);
        assert!(next.is_some());
        assert_eq!(next.unwrap().range.start.line, 2);
    }

    #[test]
    fn prev_diagnostic_finds_before_pos() {
        let decorations = make_decorations();
        let prev = prev_diagnostic(Position::new(6, 0), &decorations);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().range.start.line, 5);
    }

    #[test]
    fn prev_diagnostic_wraps_around() {
        let decorations = make_decorations();
        let prev = prev_diagnostic(Position::new(0, 0), &decorations);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().range.start.line, 10);
    }

    #[test]
    fn next_diagnostic_on_empty_returns_none() {
        assert!(next_diagnostic(Position::new(0, 0), &[]).is_none());
    }

    #[test]
    fn prev_diagnostic_on_empty_returns_none() {
        assert!(prev_diagnostic(Position::new(0, 0), &[]).is_none());
    }

    #[test]
    fn highest_severity_on_line_picks_error_over_hint() {
        let decorations = make_decorations();
        let sev = highest_severity_on_line(2, &decorations);
        assert_eq!(sev, Some(DiagnosticSeverity::Error));
    }

    #[test]
    fn highest_severity_on_line_none_for_empty_line() {
        let decorations = make_decorations();
        assert!(highest_severity_on_line(99, &decorations).is_none());
    }

    #[test]
    fn diagnostics_on_line_finds_all() {
        let decorations = make_decorations();
        let on_line_2 = diagnostics_on_line(2, &decorations);
        assert_eq!(on_line_2.len(), 2);
    }

    #[test]
    fn severity_ordering() {
        assert!(DiagnosticSeverity::Error < DiagnosticSeverity::Warning);
        assert!(DiagnosticSeverity::Warning < DiagnosticSeverity::Information);
        assert!(DiagnosticSeverity::Information < DiagnosticSeverity::Hint);
    }
}
