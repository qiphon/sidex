//! Code lens — mirrors VS Code's `CodeLensController` + `CodeLensWidget`.
//!
//! Code lenses are actionable text rendered above lines (e.g. "3 references",
//! "Run test"). They are fetched lazily and resolved when scrolled into view.

use std::collections::HashMap;

use sidex_text::Range;

/// A single code lens item.
#[derive(Debug, Clone)]
pub struct CodeLensItem {
    /// The range in the document this lens applies to.
    pub range: Range,
    /// The command title to display (e.g. "Run | Debug").
    pub command_title: Option<String>,
    /// An opaque command identifier to invoke on click.
    pub command_id: Option<String>,
    /// Arguments to pass to the command.
    pub command_args: Vec<String>,
    /// Whether this lens has been resolved (title populated).
    pub is_resolved: bool,
    /// Opaque provider data for deferred resolution.
    pub data: Option<String>,
}

/// Cached lens data for a document version.
#[derive(Debug, Clone)]
struct LensCache {
    /// The document version this cache was built for.
    version: u64,
    /// The cached lenses.
    lenses: Vec<CodeLensItem>,
}

/// Full state for the code-lens feature.
#[derive(Debug, Clone, Default)]
pub struct CodeLensState {
    /// All code lenses for the current document.
    pub lenses: Vec<CodeLensItem>,
    /// Whether a fetch is in-flight.
    pub is_loading: bool,
    /// Lines currently visible in the viewport (for lazy resolution).
    pub visible_range: Option<(u32, u32)>,
    /// Whether code lenses are enabled.
    pub enabled: bool,
    /// Per-document cache keyed by document URI/path.
    cache: HashMap<String, LensCache>,
    /// Indices of lenses pending resolution.
    pending_resolution: Vec<usize>,
}

impl CodeLensState {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    /// Sets new unresolved lenses (e.g. from an LSP `codeLens` request).
    pub fn set_lenses(&mut self, lenses: Vec<CodeLensItem>) {
        self.lenses = lenses;
        self.is_loading = false;
        self.pending_resolution.clear();
    }

    /// Sets lenses with caching support.
    pub fn set_lenses_cached(&mut self, doc_uri: &str, version: u64, lenses: Vec<CodeLensItem>) {
        self.cache.insert(
            doc_uri.to_string(),
            LensCache {
                version,
                lenses: lenses.clone(),
            },
        );
        self.set_lenses(lenses);
    }

    /// Tries to restore lenses from cache for the given document version.
    /// Returns `true` if cache hit.
    pub fn restore_from_cache(&mut self, doc_uri: &str, version: u64) -> bool {
        if let Some(cached) = self.cache.get(doc_uri) {
            if cached.version == version {
                self.lenses = cached.lenses.clone();
                self.is_loading = false;
                return true;
            }
        }
        false
    }

    /// Invalidates the cache for a document.
    pub fn invalidate_cache(&mut self, doc_uri: &str) {
        self.cache.remove(doc_uri);
    }

    /// Marks a lens as resolved with the given title and command.
    pub fn resolve_lens(&mut self, index: usize, title: String, command_id: String) {
        if let Some(lens) = self.lenses.get_mut(index) {
            lens.command_title = Some(title);
            lens.command_id = Some(command_id);
            lens.is_resolved = true;
        }
        self.pending_resolution.retain(|&i| i != index);
    }

    /// Returns lenses that are within the visible range and still unresolved.
    /// These should be sent for lazy resolution.
    #[must_use]
    pub fn unresolved_in_viewport(&self) -> Vec<usize> {
        let Some((start, end)) = self.visible_range else {
            return Vec::new();
        };
        self.lenses
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                !l.is_resolved && l.range.start.line >= start && l.range.start.line <= end
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// Schedules resolution for lenses in the current viewport.
    /// Returns the indices that need resolution (newly added only).
    pub fn schedule_viewport_resolution(&mut self) -> Vec<usize> {
        let unresolved = self.unresolved_in_viewport();
        let mut new_pending = Vec::new();
        for idx in unresolved {
            if !self.pending_resolution.contains(&idx) {
                self.pending_resolution.push(idx);
                new_pending.push(idx);
            }
        }
        new_pending
    }

    /// Handles a click on a resolved code lens at the given line.
    /// Returns the `(command_id, command_args)` to execute.
    #[must_use]
    pub fn click_lens(&self, line: u32) -> Option<(&str, &[String])> {
        self.lenses
            .iter()
            .find(|l| l.is_resolved && l.range.start.line == line)
            .and_then(|l| {
                l.command_id
                    .as_deref()
                    .map(|id| (id, l.command_args.as_slice()))
            })
    }

    /// Handles a click on a specific lens by index.
    #[must_use]
    pub fn click_lens_at(&self, index: usize) -> Option<(&str, &[String])> {
        self.lenses.get(index).and_then(|l| {
            if l.is_resolved {
                l.command_id
                    .as_deref()
                    .map(|id| (id, l.command_args.as_slice()))
            } else {
                None
            }
        })
    }

    /// Returns all resolved lenses sorted by line.
    #[must_use]
    pub fn resolved_lenses(&self) -> Vec<&CodeLensItem> {
        let mut lenses: Vec<_> = self.lenses.iter().filter(|l| l.is_resolved).collect();
        lenses.sort_by_key(|l| l.range.start.line);
        lenses
    }

    /// Returns lenses grouped by line number.
    #[must_use]
    pub fn lenses_by_line(&self) -> HashMap<u32, Vec<&CodeLensItem>> {
        let mut map: HashMap<u32, Vec<&CodeLensItem>> = HashMap::new();
        for lens in &self.lenses {
            if lens.is_resolved {
                map.entry(lens.range.start.line).or_default().push(lens);
            }
        }
        map
    }

    /// Clears all lenses.
    pub fn clear(&mut self) {
        self.lenses.clear();
        self.is_loading = false;
        self.pending_resolution.clear();
    }

    /// Updates the visible range for lazy resolution.
    pub fn set_visible_range(&mut self, start: u32, end: u32) {
        self.visible_range = Some((start, end));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidex_text::Position;

    fn make_lens(line: u32) -> CodeLensItem {
        CodeLensItem {
            range: Range::new(Position::new(line, 0), Position::new(line, 0)),
            command_title: None,
            command_id: None,
            command_args: Vec::new(),
            is_resolved: false,
            data: None,
        }
    }

    #[test]
    fn resolve_lens() {
        let mut state = CodeLensState::new();
        state.set_lenses(vec![make_lens(5)]);
        state.set_visible_range(0, 10);

        let unresolved = state.unresolved_in_viewport();
        assert_eq!(unresolved, vec![0]);

        state.resolve_lens(0, "2 references".into(), "showRefs".into());
        assert!(state.lenses[0].is_resolved);
        assert!(state.unresolved_in_viewport().is_empty());
    }

    #[test]
    fn click_lens_returns_command() {
        let mut state = CodeLensState::new();
        state.set_lenses(vec![make_lens(5)]);
        state.resolve_lens(0, "Run".into(), "test.run".into());
        let result = state.click_lens(5);
        assert_eq!(result.map(|(id, _)| id), Some("test.run"));
    }

    #[test]
    fn cache_hit_and_miss() {
        let mut state = CodeLensState::new();
        let lenses = vec![make_lens(0), make_lens(10)];
        state.set_lenses_cached("file.rs", 1, lenses);

        assert!(state.restore_from_cache("file.rs", 1));
        assert_eq!(state.lenses.len(), 2);

        assert!(!state.restore_from_cache("file.rs", 2));
    }

    #[test]
    fn schedule_viewport_resolution() {
        let mut state = CodeLensState::new();
        state.set_lenses(vec![make_lens(3), make_lens(7), make_lens(15)]);
        state.set_visible_range(0, 10);

        let pending = state.schedule_viewport_resolution();
        assert_eq!(pending.len(), 2); // lines 3 and 7
        assert_eq!(state.pending_resolution.len(), 2);

        let pending2 = state.schedule_viewport_resolution();
        assert!(pending2.is_empty()); // already scheduled
    }
}
