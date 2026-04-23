//! Diagnostic collection and manager for tracking LSP diagnostics per file.
//!
//! Stores [`lsp_types::Diagnostic`] instances keyed by document URI,
//! typically updated from `textDocument/publishDiagnostics` notifications.
//! The [`DiagnosticManager`] extends the basic collection with version
//! tracking, aggregate counts, status bar integration, quick-fix
//! associations, diagnostic tags, and related-information support.

use std::collections::HashMap;
use std::path::PathBuf;

use lsp_types::Diagnostic;

// ── DiagnosticCollection ────────────────────────────────────────────────────

/// Stores diagnostics grouped by document URI.
#[derive(Debug, Default, Clone)]
pub struct DiagnosticCollection {
    inner: HashMap<String, Vec<Diagnostic>>,
}

impl DiagnosticCollection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, uri: &str, diagnostics: Vec<Diagnostic>) {
        if diagnostics.is_empty() {
            self.inner.remove(uri);
        } else {
            self.inner.insert(uri.to_owned(), diagnostics);
        }
    }

    pub fn get(&self, uri: &str) -> &[Diagnostic] {
        self.inner.get(uri).map_or(&[], Vec::as_slice)
    }

    pub fn all(&self) -> impl Iterator<Item = (&str, &[Diagnostic])> {
        self.inner
            .iter()
            .map(|(uri, diags)| (uri.as_str(), diags.as_slice()))
    }

    pub fn clear(&mut self, uri: &str) {
        self.inner.remove(uri);
    }

    pub fn clear_all(&mut self) {
        self.inner.clear();
    }

    pub fn total_count(&self) -> usize {
        self.inner.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// ── DiagnosticCounts ────────────────────────────────────────────────────────

/// Aggregate severity counts across all tracked diagnostics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiagnosticCounts {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub hints: usize,
}

impl DiagnosticCounts {
    pub fn total(&self) -> usize {
        self.errors + self.warnings + self.info + self.hints
    }

    /// Status bar text like `"2 errors, 3 warnings"`.
    pub fn status_text(&self) -> String {
        format!("{} errors, {} warnings", self.errors, self.warnings)
    }

    /// Compact status bar text with icons: `"⊘ 2  ⚠ 3"`.
    pub fn icon_status_text(&self) -> String {
        let mut parts = Vec::new();
        if self.errors > 0 {
            parts.push(format!("\u{2298} {}", self.errors));
        }
        if self.warnings > 0 {
            parts.push(format!("\u{26A0} {}", self.warnings));
        }
        if self.info > 0 {
            parts.push(format!("\u{2139} {}", self.info));
        }
        if parts.is_empty() {
            "\u{2714} No problems".to_owned()
        } else {
            parts.join("  ")
        }
    }
}

// ── DiagnosticTag helpers ───────────────────────────────────────────────────

/// Checks whether a diagnostic has the `Unnecessary` tag (render faded).
pub fn is_unnecessary(diag: &Diagnostic) -> bool {
    diag.tags
        .as_ref()
        .is_some_and(|tags| tags.contains(&lsp_types::DiagnosticTag::UNNECESSARY))
}

/// Checks whether a diagnostic has the `Deprecated` tag (render strikethrough).
pub fn is_deprecated(diag: &Diagnostic) -> bool {
    diag.tags
        .as_ref()
        .is_some_and(|tags| tags.contains(&lsp_types::DiagnosticTag::DEPRECATED))
}

// ── DiagnosticKey ───────────────────────────────────────────────────────────

/// Unique key for a diagnostic, used to associate quick fixes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiagnosticKey {
    pub uri: String,
    pub line: u32,
    pub character: u32,
    pub message: String,
}

impl DiagnosticKey {
    pub fn from_diagnostic(uri: &str, diag: &Diagnostic) -> Self {
        Self {
            uri: uri.to_owned(),
            line: diag.range.start.line,
            character: diag.range.start.character,
            message: diag.message.clone(),
        }
    }
}

// ── RelatedInfo ─────────────────────────────────────────────────────────────

/// Simplified related-information entry for the UI.
#[derive(Debug, Clone)]
pub struct RelatedInfo {
    pub uri: String,
    pub range: sidex_text::Range,
    pub message: String,
}

/// Extracts related information from an LSP diagnostic.
pub fn extract_related_info(diag: &Diagnostic) -> Vec<RelatedInfo> {
    diag.related_information
        .as_ref()
        .map_or_else(Vec::new, |infos| {
            infos
                .iter()
                .map(|info| RelatedInfo {
                    uri: info.location.uri.to_string(),
                    range: crate::conversion::lsp_to_range(info.location.range),
                    message: info.message.clone(),
                })
                .collect()
        })
}

// ── VersionedDiagnostics ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct VersionedDiagnostics {
    diagnostics: Vec<Diagnostic>,
    version: Option<i32>,
}

// ── QuickFixCache ───────────────────────────────────────────────────────────

/// Cache of quick-fix code actions associated with diagnostics.
#[derive(Debug, Default, Clone)]
pub struct QuickFixCache {
    fixes: HashMap<DiagnosticKey, Vec<crate::code_action_engine::CodeActionInfo>>,
}

impl QuickFixCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(
        &mut self,
        key: DiagnosticKey,
        actions: Vec<crate::code_action_engine::CodeActionInfo>,
    ) {
        self.fixes.insert(key, actions);
    }

    pub fn get(&self, key: &DiagnosticKey) -> &[crate::code_action_engine::CodeActionInfo] {
        self.fixes.get(key).map_or(&[], Vec::as_slice)
    }

    pub fn clear_for_uri(&mut self, uri: &str) {
        self.fixes.retain(|k, _| k.uri != uri);
    }

    pub fn clear(&mut self) {
        self.fixes.clear();
    }
}

// ── DiagnosticManager ───────────────────────────────────────────────────────

/// Manages diagnostics across all open files with version tracking,
/// severity counting, staleness detection, quick-fix caching, and
/// navigation helpers.
#[derive(Debug, Default, Clone)]
pub struct DiagnosticManager {
    files: HashMap<String, VersionedDiagnostics>,
    document_versions: HashMap<String, i32>,
    quick_fixes: QuickFixCache,
}

impl DiagnosticManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Called when LSP sends `textDocument/publishDiagnostics`.
    pub fn on_diagnostics(&mut self, uri: &str, diagnostics: Vec<Diagnostic>) {
        let version = self.document_versions.get(uri).copied();
        self.quick_fixes.clear_for_uri(uri);
        if diagnostics.is_empty() {
            self.files.remove(uri);
        } else {
            self.files.insert(
                uri.to_owned(),
                VersionedDiagnostics {
                    diagnostics,
                    version,
                },
            );
        }
    }

    /// Called when a document version changes (e.g. on edit).
    pub fn set_document_version(&mut self, uri: &str, version: i32) {
        self.document_versions.insert(uri.to_owned(), version);
    }

    /// Returns diagnostics for a specific file.
    pub fn get_diagnostics(&self, uri: &str) -> &[Diagnostic] {
        self.files
            .get(uri)
            .map_or(&[], |v| v.diagnostics.as_slice())
    }

    /// Iterates over all `(uri, diagnostics)` pairs.
    pub fn all_diagnostics(&self) -> impl Iterator<Item = (&str, &[Diagnostic])> {
        self.files
            .iter()
            .map(|(uri, v)| (uri.as_str(), v.diagnostics.as_slice()))
    }

    /// Returns diagnostics as `(PathBuf, Vec<&Diagnostic>)` for the problems panel.
    pub fn grouped_by_file(&self) -> Vec<(PathBuf, Vec<&Diagnostic>)> {
        self.files
            .iter()
            .map(|(uri, v)| {
                let path =
                    crate::workspace_edit::uri_to_path(uri).unwrap_or_else(|| PathBuf::from(uri));
                (path, v.diagnostics.iter().collect())
            })
            .collect()
    }

    /// Returns `true` if the diagnostics for `uri` are stale.
    pub fn is_stale(&self, uri: &str) -> bool {
        let Some(entry) = self.files.get(uri) else {
            return false;
        };
        match (entry.version, self.document_versions.get(uri)) {
            (Some(diag_ver), Some(&doc_ver)) => diag_ver < doc_ver,
            _ => false,
        }
    }

    /// Returns aggregate severity counts across all files.
    pub fn diagnostic_counts(&self) -> DiagnosticCounts {
        let mut counts = DiagnosticCounts::default();
        for entry in self.files.values() {
            for diag in &entry.diagnostics {
                match diag.severity {
                    Some(lsp_types::DiagnosticSeverity::ERROR) => counts.errors += 1,
                    Some(lsp_types::DiagnosticSeverity::WARNING) => counts.warnings += 1,
                    Some(lsp_types::DiagnosticSeverity::HINT) => counts.hints += 1,
                    _ => counts.info += 1,
                }
            }
        }
        counts
    }

    /// Returns the diagnostic at a position within a file (for hover).
    pub fn diagnostic_at(&self, uri: &str, line: u32, character: u32) -> Option<&Diagnostic> {
        self.get_diagnostics(uri)
            .iter()
            .filter(|d| {
                let s = d.range.start;
                let e = d.range.end;
                (s.line < line || (s.line == line && s.character <= character))
                    && (e.line > line || (e.line == line && e.character >= character))
            })
            .min_by_key(|d| {
                d.severity.map_or(3, |s| {
                    if s == lsp_types::DiagnosticSeverity::ERROR {
                        0
                    } else if s == lsp_types::DiagnosticSeverity::WARNING {
                        1
                    } else {
                        2
                    }
                })
            })
    }

    /// Finds the next diagnostic after `(line, col)` for F8 navigation.
    pub fn next_diagnostic(&self, uri: &str, line: u32, col: u32) -> Option<&Diagnostic> {
        let diags = self.get_diagnostics(uri);
        if diags.is_empty() {
            return None;
        }
        diags
            .iter()
            .filter(|d| {
                d.range.start.line > line
                    || (d.range.start.line == line && d.range.start.character > col)
            })
            .min_by_key(|d| (d.range.start.line, d.range.start.character))
            .or_else(|| {
                diags
                    .iter()
                    .min_by_key(|d| (d.range.start.line, d.range.start.character))
            })
    }

    /// Finds the previous diagnostic before `(line, col)` for Shift+F8.
    pub fn prev_diagnostic(&self, uri: &str, line: u32, col: u32) -> Option<&Diagnostic> {
        let diags = self.get_diagnostics(uri);
        if diags.is_empty() {
            return None;
        }
        diags
            .iter()
            .filter(|d| {
                d.range.start.line < line
                    || (d.range.start.line == line && d.range.start.character < col)
            })
            .max_by_key(|d| (d.range.start.line, d.range.start.character))
            .or_else(|| {
                diags
                    .iter()
                    .max_by_key(|d| (d.range.start.line, d.range.start.character))
            })
    }

    /// Mutable access to the quick-fix cache.
    pub fn quick_fixes_mut(&mut self) -> &mut QuickFixCache {
        &mut self.quick_fixes
    }

    /// Read access to the quick-fix cache.
    pub fn quick_fixes(&self) -> &QuickFixCache {
        &self.quick_fixes
    }

    pub fn on_file_closed(&mut self, uri: &str) {
        self.files.remove(uri);
        self.document_versions.remove(uri);
        self.quick_fixes.clear_for_uri(uri);
    }

    pub fn clear(&mut self, uri: &str) {
        self.files.remove(uri);
    }

    pub fn clear_all(&mut self) {
        self.files.clear();
        self.quick_fixes.clear();
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use lsp_types::{DiagnosticSeverity, Position, Range};

    use super::*;

    fn make_diagnostic(message: &str, line: u32) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(line, 0), Position::new(line, 10)),
            severity: Some(DiagnosticSeverity::ERROR),
            message: message.to_owned(),
            ..Diagnostic::default()
        }
    }

    fn make_warning(message: &str, line: u32) -> Diagnostic {
        Diagnostic {
            range: Range::new(Position::new(line, 0), Position::new(line, 10)),
            severity: Some(DiagnosticSeverity::WARNING),
            message: message.to_owned(),
            ..Diagnostic::default()
        }
    }

    #[test]
    fn set_and_get() {
        let mut coll = DiagnosticCollection::new();
        let diags = vec![make_diagnostic("unused variable", 5)];
        coll.set("file:///main.rs", diags);
        assert_eq!(coll.get("file:///main.rs").len(), 1);
    }

    #[test]
    fn get_missing_uri_returns_empty() {
        let coll = DiagnosticCollection::new();
        assert!(coll.get("file:///nonexistent.rs").is_empty());
    }

    #[test]
    fn set_empty_removes_entry() {
        let mut coll = DiagnosticCollection::new();
        coll.set("file:///a.rs", vec![make_diagnostic("err", 0)]);
        coll.set("file:///a.rs", vec![]);
        assert!(coll.is_empty());
    }

    #[test]
    fn manager_on_diagnostics_stores_and_retrieves() {
        let mut mgr = DiagnosticManager::new();
        mgr.on_diagnostics("file:///a.rs", vec![make_diagnostic("err", 5)]);
        assert_eq!(mgr.get_diagnostics("file:///a.rs").len(), 1);
    }

    #[test]
    fn manager_diagnostic_counts() {
        let mut mgr = DiagnosticManager::new();
        mgr.on_diagnostics(
            "file:///a.rs",
            vec![make_diagnostic("e1", 0), make_warning("w1", 1)],
        );
        let counts = mgr.diagnostic_counts();
        assert_eq!(counts.errors, 1);
        assert_eq!(counts.warnings, 1);
        assert_eq!(counts.total(), 2);
    }

    #[test]
    fn manager_staleness_detection() {
        let mut mgr = DiagnosticManager::new();
        mgr.set_document_version("file:///a.rs", 1);
        mgr.on_diagnostics("file:///a.rs", vec![make_diagnostic("err", 0)]);
        assert!(!mgr.is_stale("file:///a.rs"));
        mgr.set_document_version("file:///a.rs", 2);
        assert!(mgr.is_stale("file:///a.rs"));
    }

    #[test]
    fn icon_status_text_no_problems() {
        let counts = DiagnosticCounts::default();
        assert!(counts.icon_status_text().contains("No problems"));
    }

    #[test]
    fn icon_status_text_with_errors() {
        let counts = DiagnosticCounts {
            errors: 3,
            warnings: 12,
            info: 0,
            hints: 0,
        };
        let text = counts.icon_status_text();
        assert!(text.contains("3"));
        assert!(text.contains("12"));
    }

    #[test]
    fn diagnostic_key_from_diagnostic() {
        let diag = make_diagnostic("test", 5);
        let key = DiagnosticKey::from_diagnostic("file:///a.rs", &diag);
        assert_eq!(key.uri, "file:///a.rs");
        assert_eq!(key.line, 5);
        assert_eq!(key.message, "test");
    }

    #[test]
    fn next_diagnostic_finds_after() {
        let mut mgr = DiagnosticManager::new();
        mgr.on_diagnostics(
            "file:///a.rs",
            vec![make_diagnostic("e1", 2), make_diagnostic("e2", 8)],
        );
        let next = mgr.next_diagnostic("file:///a.rs", 3, 0).unwrap();
        assert_eq!(next.range.start.line, 8);
    }

    #[test]
    fn next_diagnostic_wraps() {
        let mut mgr = DiagnosticManager::new();
        mgr.on_diagnostics("file:///a.rs", vec![make_diagnostic("e1", 2)]);
        let next = mgr.next_diagnostic("file:///a.rs", 10, 0).unwrap();
        assert_eq!(next.range.start.line, 2);
    }

    #[test]
    fn prev_diagnostic_finds_before() {
        let mut mgr = DiagnosticManager::new();
        mgr.on_diagnostics(
            "file:///a.rs",
            vec![make_diagnostic("e1", 2), make_diagnostic("e2", 8)],
        );
        let prev = mgr.prev_diagnostic("file:///a.rs", 5, 0).unwrap();
        assert_eq!(prev.range.start.line, 2);
    }

    #[test]
    fn diagnostic_at_position() {
        let mut mgr = DiagnosticManager::new();
        mgr.on_diagnostics("file:///a.rs", vec![make_diagnostic("e1", 5)]);
        assert!(mgr.diagnostic_at("file:///a.rs", 5, 3).is_some());
        assert!(mgr.diagnostic_at("file:///a.rs", 7, 0).is_none());
    }

    #[test]
    fn is_unnecessary_tag() {
        let diag = Diagnostic {
            tags: Some(vec![lsp_types::DiagnosticTag::UNNECESSARY]),
            ..Diagnostic::default()
        };
        assert!(is_unnecessary(&diag));
        assert!(!is_deprecated(&diag));
    }

    #[test]
    fn is_deprecated_tag() {
        let diag = Diagnostic {
            tags: Some(vec![lsp_types::DiagnosticTag::DEPRECATED]),
            ..Diagnostic::default()
        };
        assert!(!is_unnecessary(&diag));
        assert!(is_deprecated(&diag));
    }

    #[test]
    fn quick_fix_cache() {
        let mut cache = QuickFixCache::new();
        let key = DiagnosticKey {
            uri: "file:///a.rs".into(),
            line: 5,
            character: 0,
            message: "err".into(),
        };
        assert!(cache.get(&key).is_empty());
        cache.set(
            key.clone(),
            vec![crate::code_action_engine::CodeActionInfo {
                title: "Fix it".into(),
                kind: crate::code_action_engine::CodeActionKind::QuickFix,
                edit: None,
                command: None,
                is_preferred: true,
            }],
        );
        assert_eq!(cache.get(&key).len(), 1);
        cache.clear_for_uri("file:///a.rs");
        assert!(cache.get(&key).is_empty());
    }
}
