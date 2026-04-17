//! Sticky scroll — pins parent scope headers at the top of the editor viewport
//! as the user scrolls, mirroring VS Code's sticky scroll feature.

/// Kind of scope that produced a sticky line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    Function,
    Class,
    Method,
    Module,
    Block,
    If,
    For,
    While,
    Switch,
    Try,
}

/// A single line pinned to the sticky scroll header area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickyLine {
    pub text: String,
    pub line_number: u32,
    pub indent_level: u32,
    pub scope_kind: ScopeKind,
}

/// A document symbol (outline entry) used as input for sticky line computation.
#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: ScopeKind,
    pub start_line: u32,
    pub end_line: u32,
    pub indent_level: u32,
    pub children: Vec<DocumentSymbol>,
}

/// Rendering hint for the sticky scroll widget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StickyScrollStyle {
    pub bottom_border_width: f32,
    pub shadow_opacity: f32,
}

impl Default for StickyScrollStyle {
    fn default() -> Self {
        Self {
            bottom_border_width: 1.0,
            shadow_opacity: 0.08,
        }
    }
}

/// Top-level sticky scroll state for an editor instance.
#[derive(Debug, Clone)]
pub struct StickyScroll {
    pub enabled: bool,
    pub max_lines: u32,
    pub lines: Vec<StickyLine>,
    pub style: StickyScrollStyle,
}

impl Default for StickyScroll {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lines: 5,
            lines: Vec::new(),
            style: StickyScrollStyle::default(),
        }
    }
}

impl StickyScroll {
    pub fn new(enabled: bool, max_lines: u32) -> Self {
        Self {
            enabled,
            max_lines,
            ..Default::default()
        }
    }

    /// Recompute pinned lines from the document outline and current scroll position.
    pub fn update(&mut self, visible_start: u32, outline: &[DocumentSymbol]) {
        self.lines = if self.enabled {
            compute_sticky_lines(visible_start, outline, self.max_lines)
        } else {
            Vec::new()
        };
    }

    /// Handle a click on sticky line at `index`. Returns the document line to jump to.
    #[must_use]
    pub fn click_line(&self, index: usize) -> Option<u32> {
        self.lines.get(index).map(|l| l.line_number)
    }

    /// Apply settings from `editor.stickyScroll.enabled` / `editor.stickyScroll.maxLineCount`.
    pub fn apply_settings(&mut self, enabled: bool, max_line_count: u32) {
        self.enabled = enabled;
        self.max_lines = max_line_count.max(1).min(10);
    }
}

/// Walk the outline tree and collect every scope that spans `visible_start`,
/// i.e. scopes whose header has scrolled past but whose body is still visible.
/// Results are sorted by `indent_level` (outermost first) and capped at `max_lines`.
pub fn compute_sticky_lines(
    visible_start: u32,
    outline: &[DocumentSymbol],
    max_lines: u32,
) -> Vec<StickyLine> {
    let mut buf = Vec::new();
    collect_active_scopes(visible_start, outline, &mut buf);
    buf.sort_by_key(|l| l.indent_level);
    buf.truncate(max_lines as usize);
    buf
}

fn collect_active_scopes(
    visible_start: u32,
    symbols: &[DocumentSymbol],
    out: &mut Vec<StickyLine>,
) {
    for sym in symbols {
        if sym.start_line < visible_start && sym.end_line >= visible_start {
            out.push(StickyLine {
                text: sym.name.clone(),
                line_number: sym.start_line,
                indent_level: sym.indent_level,
                scope_kind: sym.kind,
            });
            collect_active_scopes(visible_start, &sym.children, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_outline() -> Vec<DocumentSymbol> {
        vec![DocumentSymbol {
            name: "class Foo".into(),
            kind: ScopeKind::Class,
            start_line: 0,
            end_line: 60,
            indent_level: 0,
            children: vec![DocumentSymbol {
                name: "fn bar()".into(),
                kind: ScopeKind::Method,
                start_line: 5,
                end_line: 40,
                indent_level: 1,
                children: vec![DocumentSymbol {
                    name: "if condition".into(),
                    kind: ScopeKind::If,
                    start_line: 10,
                    end_line: 25,
                    indent_level: 2,
                    children: vec![],
                }],
            }],
        }]
    }

    #[test]
    fn nested_scopes_produce_stacked_headers() {
        let lines = compute_sticky_lines(15, &sample_outline(), 5);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].scope_kind, ScopeKind::Class);
        assert_eq!(lines[1].scope_kind, ScopeKind::Method);
        assert_eq!(lines[2].scope_kind, ScopeKind::If);
    }

    #[test]
    fn max_lines_caps_output() {
        let lines = compute_sticky_lines(15, &sample_outline(), 1);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn disabled_shows_nothing() {
        let mut ss = StickyScroll::new(false, 5);
        ss.update(15, &sample_outline());
        assert!(ss.lines.is_empty());
    }

    #[test]
    fn click_returns_scope_start() {
        let mut ss = StickyScroll::default();
        ss.update(15, &sample_outline());
        assert_eq!(ss.click_line(0), Some(0));
        assert_eq!(ss.click_line(1), Some(5));
    }

    #[test]
    fn apply_settings_clamps() {
        let mut ss = StickyScroll::default();
        ss.apply_settings(true, 20);
        assert_eq!(ss.max_lines, 10);
        ss.apply_settings(true, 0);
        assert_eq!(ss.max_lines, 1);
    }
}
