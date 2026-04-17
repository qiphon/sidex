//! Breadcrumbs — document path + symbol breadcrumb trail, mirrors VS Code's
//! `BreadcrumbsWidget` / `BreadcrumbsModel`.
//!
//! Computes a hierarchical breadcrumb trail from the file path segments and
//! the document symbol tree, given the current cursor position.

use std::path::Path;

use sidex_text::{Position, Range};

/// The kind of a breadcrumb segment (file path part vs. symbol kind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreadcrumbKind {
    Root,
    File,
    Folder,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    EnumMember,
    Interface,
    Function,
    Variable,
    Constant,
    Struct,
    Event,
    Operator,
    TypeParameter,
    Key,
}

/// Icon type for breadcrumb rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreadcrumbIcon {
    Folder,
    File(String),
    Symbol(BreadcrumbKind),
    Root,
}

impl BreadcrumbIcon {
    #[must_use]
    pub fn for_kind(kind: BreadcrumbKind, extension: Option<&str>) -> Self {
        match kind {
            BreadcrumbKind::Root => Self::Root,
            BreadcrumbKind::Folder => Self::Folder,
            BreadcrumbKind::File => Self::File(extension.unwrap_or("").to_string()),
            _ => Self::Symbol(kind),
        }
    }
}

/// A document symbol with children, matching LSP `DocumentSymbol`.
#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    pub name: String,
    pub kind: BreadcrumbKind,
    pub range: Range,
    pub selection_range: Range,
    pub children: Vec<DocumentSymbol>,
}

/// A single segment in the breadcrumb trail.
#[derive(Debug, Clone)]
pub struct BreadcrumbSegment {
    pub label: String,
    pub kind: BreadcrumbKind,
    pub icon: BreadcrumbIcon,
    pub range: Range,
    pub children: Vec<BreadcrumbSegment>,
}

/// Full state for the breadcrumbs bar.
#[derive(Debug, Clone, Default)]
pub struct BreadcrumbsState {
    /// The computed breadcrumb segments.
    pub segments: Vec<BreadcrumbSegment>,
    /// Index of the currently focused segment (for keyboard navigation), if any.
    pub focused: Option<usize>,
    /// Whether the breadcrumbs bar is visible.
    pub is_visible: bool,
    /// Dropdown state.
    pub dropdown: BreadcrumbDropdown,
}

/// Action emitted by the breadcrumbs bar.
#[derive(Debug, Clone)]
pub enum BreadcrumbAction {
    /// Navigate to a file path segment.
    NavigateToPath(String),
    /// Navigate to a symbol at the given range.
    NavigateToSymbol(Range),
    /// Open a file from the dropdown.
    OpenFile(String),
}

/// State for the breadcrumb dropdown menu.
#[derive(Debug, Clone, Default)]
pub struct BreadcrumbDropdown {
    pub items: Vec<BreadcrumbSegment>,
    pub highlighted: Option<usize>,
    pub is_open: bool,
    pub owner_segment: Option<usize>,
    pub filter: String,
    filtered_indices: Vec<usize>,
}

impl BreadcrumbDropdown {
    pub fn open(&mut self, segment_index: usize, items: Vec<BreadcrumbSegment>) {
        self.items = items;
        self.filter.clear();
        self.filtered_indices = (0..self.items.len()).collect();
        self.highlighted = if self.filtered_indices.is_empty() {
            None
        } else {
            Some(0)
        };
        self.is_open = true;
        self.owner_segment = Some(segment_index);
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.items.clear();
        self.highlighted = None;
        self.owner_segment = None;
        self.filter.clear();
        self.filtered_indices.clear();
    }

    /// Updates the type-ahead filter and recomputes visible items.
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        let lower = filter.to_lowercase();
        self.filtered_indices = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                lower.is_empty() || item.label.to_lowercase().contains(&lower)
            })
            .map(|(i, _)| i)
            .collect();
        self.highlighted = if self.filtered_indices.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Returns the visible items after filtering.
    #[must_use]
    pub fn visible_items(&self) -> Vec<&BreadcrumbSegment> {
        self.filtered_indices
            .iter()
            .filter_map(|&i| self.items.get(i))
            .collect()
    }

    pub fn highlight_next(&mut self) {
        let count = self.filtered_indices.len();
        if count == 0 {
            return;
        }
        self.highlighted = Some(match self.highlighted {
            Some(i) if i + 1 < count => i + 1,
            _ => 0,
        });
    }

    pub fn highlight_prev(&mut self) {
        let count = self.filtered_indices.len();
        if count == 0 {
            return;
        }
        self.highlighted = Some(match self.highlighted {
            Some(0) | None => count.saturating_sub(1),
            Some(i) => i - 1,
        });
    }

    #[must_use]
    pub fn highlighted_item(&self) -> Option<&BreadcrumbSegment> {
        let vis_idx = self.highlighted?;
        let real_idx = *self.filtered_indices.get(vis_idx)?;
        self.items.get(real_idx)
    }

    #[must_use]
    pub fn accept_highlighted(&self) -> Option<BreadcrumbAction> {
        let item = self.highlighted_item()?;
        let empty_range = Range::new(Position::ZERO, Position::ZERO);
        if item.range == empty_range {
            Some(BreadcrumbAction::OpenFile(item.label.clone()))
        } else {
            Some(BreadcrumbAction::NavigateToSymbol(item.range))
        }
    }
}

impl BreadcrumbsState {
    /// Shows the breadcrumbs bar.
    pub fn show(&mut self) {
        self.is_visible = true;
    }

    /// Hides the breadcrumbs bar.
    pub fn hide(&mut self) {
        self.is_visible = false;
        self.focused = None;
        self.dropdown.close();
    }

    /// Updates the breadcrumb trail for the given file path, symbols, and
    /// cursor position.
    pub fn update(&mut self, path: &Path, symbols: &[DocumentSymbol], cursor_pos: Position) {
        self.segments = compute_breadcrumbs(path, symbols, cursor_pos);
        if let Some(idx) = self.focused {
            if idx >= self.segments.len() {
                self.focused = None;
            }
        }
    }

    /// Focuses the next breadcrumb segment (Alt+Right).
    pub fn focus_next(&mut self) {
        if self.segments.is_empty() {
            return;
        }
        self.dropdown.close();
        self.focused = Some(match self.focused {
            Some(i) if i + 1 < self.segments.len() => i + 1,
            _ => 0,
        });
    }

    /// Focuses the previous breadcrumb segment (Alt+Left).
    pub fn focus_prev(&mut self) {
        if self.segments.is_empty() {
            return;
        }
        self.dropdown.close();
        self.focused = Some(match self.focused {
            Some(0) | None => self.segments.len() - 1,
            Some(i) => i - 1,
        });
    }

    /// Returns the currently focused segment, if any.
    #[must_use]
    pub fn focused_segment(&self) -> Option<&BreadcrumbSegment> {
        self.focused.and_then(|i| self.segments.get(i))
    }

    /// Click on a segment — opens its dropdown showing children.
    pub fn click_segment(&mut self, index: usize) {
        if index >= self.segments.len() {
            return;
        }
        self.focused = Some(index);
        let children = self.segments[index].children.clone();
        if children.is_empty() {
            self.dropdown.close();
        } else {
            self.dropdown.open(index, children);
        }
    }

    /// Toggle the dropdown for the currently focused segment.
    pub fn toggle_dropdown(&mut self) {
        if self.dropdown.is_open {
            self.dropdown.close();
            return;
        }
        if let Some(idx) = self.focused {
            self.click_segment(idx);
        }
    }

    /// Handle Enter key on the dropdown — returns an action if accepted.
    #[must_use]
    pub fn accept_dropdown(&mut self) -> Option<BreadcrumbAction> {
        if !self.dropdown.is_open {
            return None;
        }
        let action = self.dropdown.accept_highlighted();
        self.dropdown.close();
        action
    }

    /// Build a symbol outline from the current document symbols.
    #[must_use]
    pub fn symbol_outline(symbols: &[DocumentSymbol]) -> Vec<BreadcrumbSegment> {
        symbols_to_segments(symbols)
    }

    /// Get a reference to the dropdown state.
    #[must_use]
    pub fn dropdown(&self) -> &BreadcrumbDropdown {
        &self.dropdown
    }
}

/// Builds a breadcrumb trail from the file path and the document symbol tree,
/// selecting the symbol chain that contains `cursor_pos`.
///
/// If `workspace_root` is provided, the first segment will be a `Root` breadcrumb
/// with the workspace name.
#[must_use]
pub fn compute_breadcrumbs(
    path: &Path,
    symbols: &[DocumentSymbol],
    cursor_pos: Position,
) -> Vec<BreadcrumbSegment> {
    compute_breadcrumbs_with_root(path, None, symbols, cursor_pos)
}

/// Like `compute_breadcrumbs` but accepts an optional workspace root path.
#[must_use]
pub fn compute_breadcrumbs_with_root(
    path: &Path,
    workspace_root: Option<&Path>,
    symbols: &[DocumentSymbol],
    cursor_pos: Position,
) -> Vec<BreadcrumbSegment> {
    let empty_range = Range::new(Position::ZERO, Position::ZERO);
    let mut segments = Vec::new();

    if let Some(root) = workspace_root {
        let root_name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "workspace".to_string());
        segments.push(BreadcrumbSegment {
            label: root_name,
            kind: BreadcrumbKind::Root,
            icon: BreadcrumbIcon::Root,
            range: empty_range,
            children: Vec::new(),
        });
    }

    let relative = workspace_root
        .and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);

    for component in relative.components() {
        let label = component.as_os_str().to_string_lossy().into_owned();
        if label == "/" || label == "." {
            continue;
        }
        segments.push(BreadcrumbSegment {
            label: label.clone(),
            kind: BreadcrumbKind::Folder,
            icon: BreadcrumbIcon::Folder,
            range: empty_range,
            children: Vec::new(),
        });
    }

    if let Some(last) = segments.last_mut() {
        if last.kind == BreadcrumbKind::Folder {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().into_owned())
                .unwrap_or_default();
            last.kind = BreadcrumbKind::File;
            last.icon = BreadcrumbIcon::File(ext);
        }
    }

    let mut symbol_chain = Vec::new();
    collect_symbol_chain(symbols, cursor_pos, &mut symbol_chain);

    for sym in &symbol_chain {
        let children = symbols_to_segments(&sym.children);
        segments.push(BreadcrumbSegment {
            label: sym.name.clone(),
            kind: sym.kind,
            icon: BreadcrumbIcon::Symbol(sym.kind),
            range: sym.selection_range,
            children,
        });
    }

    segments
}

fn collect_symbol_chain<'a>(
    symbols: &'a [DocumentSymbol],
    pos: Position,
    chain: &mut Vec<&'a DocumentSymbol>,
) {
    for sym in symbols {
        if sym.range.contains(pos) || sym.range.start == pos {
            chain.push(sym);
            collect_symbol_chain(&sym.children, pos, chain);
            return;
        }
    }
}

fn symbols_to_segments(symbols: &[DocumentSymbol]) -> Vec<BreadcrumbSegment> {
    symbols
        .iter()
        .map(|s| BreadcrumbSegment {
            label: s.name.clone(),
            kind: s.kind,
            icon: BreadcrumbIcon::Symbol(s.kind),
            range: s.selection_range,
            children: symbols_to_segments(&s.children),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_symbol(
        name: &str,
        kind: BreadcrumbKind,
        line: u32,
        children: Vec<DocumentSymbol>,
    ) -> DocumentSymbol {
        DocumentSymbol {
            name: name.to_string(),
            kind,
            range: Range::new(Position::new(line, 0), Position::new(line + 10, 0)),
            selection_range: Range::new(
                Position::new(line, 0),
                Position::new(line, name.len() as u32),
            ),
            children,
        }
    }

    #[test]
    fn path_only_breadcrumbs() {
        let path = Path::new("src/utils/helpers.rs");
        let segments = compute_breadcrumbs(path, &[], Position::ZERO);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].label, "src");
        assert_eq!(segments[0].kind, BreadcrumbKind::Folder);
        assert_eq!(segments[1].label, "utils");
        assert_eq!(segments[2].label, "helpers.rs");
        assert_eq!(segments[2].kind, BreadcrumbKind::File);
    }

    #[test]
    fn path_plus_symbols() {
        let symbols = vec![make_symbol(
            "MyClass",
            BreadcrumbKind::Class,
            0,
            vec![make_symbol("my_method", BreadcrumbKind::Method, 2, vec![])],
        )];
        let path = Path::new("src/main.rs");
        let cursor = Position::new(3, 5);
        let segments = compute_breadcrumbs(path, &symbols, cursor);

        // 2 path segments + 2 symbol segments
        assert_eq!(segments.len(), 4);
        assert_eq!(segments[2].label, "MyClass");
        assert_eq!(segments[2].kind, BreadcrumbKind::Class);
        assert_eq!(segments[3].label, "my_method");
        assert_eq!(segments[3].kind, BreadcrumbKind::Method);
    }

    #[test]
    fn cursor_outside_symbols() {
        let symbols = vec![make_symbol("Foo", BreadcrumbKind::Function, 10, vec![])];
        let path = Path::new("lib.rs");
        let cursor = Position::new(50, 0);
        let segments = compute_breadcrumbs(path, &symbols, cursor);
        assert_eq!(segments.len(), 1); // just "lib.rs"
    }

    #[test]
    fn breadcrumbs_state_navigation() {
        let mut state = BreadcrumbsState::default();
        state.update(Path::new("src/main.rs"), &[], Position::ZERO);
        state.show();
        assert!(state.is_visible);
        assert_eq!(state.segments.len(), 2);

        state.focus_next();
        assert_eq!(state.focused, Some(0));
        state.focus_next();
        assert_eq!(state.focused, Some(1));
        state.focus_next();
        assert_eq!(state.focused, Some(0)); // wraps

        state.focus_prev();
        assert_eq!(state.focused, Some(1)); // wraps back to end
    }

    #[test]
    fn focused_segment_returns_correct() {
        let mut state = BreadcrumbsState::default();
        state.update(Path::new("a/b.rs"), &[], Position::ZERO);
        assert!(state.focused_segment().is_none());

        state.focus_next();
        let seg = state.focused_segment().unwrap();
        assert_eq!(seg.label, "a");
    }

    #[test]
    fn children_are_populated() {
        let child = make_symbol("bar", BreadcrumbKind::Function, 2, vec![]);
        let parent = make_symbol("Foo", BreadcrumbKind::Class, 0, vec![child]);
        let path = Path::new("x.rs");
        let segments = compute_breadcrumbs(path, &[parent], Position::new(3, 0));

        // path(x.rs) + Foo + bar
        assert_eq!(segments.len(), 3);
        // Foo segment should have "bar" as a child
        assert_eq!(segments[1].children.len(), 1);
        assert_eq!(segments[1].children[0].label, "bar");
    }

    // ── Dropdown tests ──────────────────────────────────────────

    #[test]
    fn click_segment_opens_dropdown() {
        let child = make_symbol("bar", BreadcrumbKind::Function, 2, vec![]);
        let parent = make_symbol("Foo", BreadcrumbKind::Class, 0, vec![child]);

        let mut state = BreadcrumbsState::default();
        state.update(Path::new("x.rs"), &[parent], Position::new(3, 0));

        // Click on "Foo" segment (index 1)
        state.click_segment(1);
        assert!(state.dropdown.is_open);
        assert_eq!(state.dropdown.items.len(), 1);
        assert_eq!(state.dropdown.items[0].label, "bar");
        assert_eq!(state.dropdown.owner_segment, Some(1));
    }

    #[test]
    fn click_segment_without_children_closes_dropdown() {
        let mut state = BreadcrumbsState::default();
        state.update(Path::new("a/b.rs"), &[], Position::ZERO);

        state.click_segment(0); // "a" has no children
        assert!(!state.dropdown.is_open);
    }

    #[test]
    fn dropdown_navigation() {
        let mut dd = BreadcrumbDropdown::default();
        let items = vec![
            BreadcrumbSegment {
                label: "a".into(),
                kind: BreadcrumbKind::Function,
                icon: BreadcrumbIcon::Symbol(BreadcrumbKind::Function),
                range: Range::new(Position::ZERO, Position::ZERO),
                children: Vec::new(),
            },
            BreadcrumbSegment {
                label: "b".into(),
                kind: BreadcrumbKind::Function,
                icon: BreadcrumbIcon::Symbol(BreadcrumbKind::Function),
                range: Range::new(Position::ZERO, Position::ZERO),
                children: Vec::new(),
            },
        ];
        dd.open(0, items);
        assert!(dd.is_open);
        assert_eq!(dd.highlighted, Some(0));

        dd.highlight_next();
        assert_eq!(dd.highlighted, Some(1));

        dd.highlight_next();
        assert_eq!(dd.highlighted, Some(0)); // wraps

        dd.highlight_prev();
        assert_eq!(dd.highlighted, Some(1)); // wraps back
    }

    #[test]
    fn dropdown_filter() {
        let mut dd = BreadcrumbDropdown::default();
        let items = vec![
            BreadcrumbSegment {
                label: "alpha".into(),
                kind: BreadcrumbKind::Function,
                icon: BreadcrumbIcon::Symbol(BreadcrumbKind::Function),
                range: Range::new(Position::ZERO, Position::ZERO),
                children: Vec::new(),
            },
            BreadcrumbSegment {
                label: "beta".into(),
                kind: BreadcrumbKind::Function,
                icon: BreadcrumbIcon::Symbol(BreadcrumbKind::Function),
                range: Range::new(Position::ZERO, Position::ZERO),
                children: Vec::new(),
            },
            BreadcrumbSegment {
                label: "gamma".into(),
                kind: BreadcrumbKind::Function,
                icon: BreadcrumbIcon::Symbol(BreadcrumbKind::Function),
                range: Range::new(Position::ZERO, Position::ZERO),
                children: Vec::new(),
            },
        ];
        dd.open(0, items);
        assert_eq!(dd.visible_items().len(), 3);

        dd.set_filter("a");
        let vis = dd.visible_items();
        assert_eq!(vis.len(), 3); // "alpha", "beta" (contains 'a'), "gamma"

        dd.set_filter("bet");
        let vis = dd.visible_items();
        assert_eq!(vis.len(), 1);
        assert_eq!(vis[0].label, "beta");
    }

    #[test]
    fn toggle_dropdown() {
        let child = make_symbol("bar", BreadcrumbKind::Function, 2, vec![]);
        let parent = make_symbol("Foo", BreadcrumbKind::Class, 0, vec![child]);

        let mut state = BreadcrumbsState::default();
        state.update(Path::new("x.rs"), &[parent], Position::new(3, 0));

        state.focused = Some(1);
        state.toggle_dropdown();
        assert!(state.dropdown.is_open);

        state.toggle_dropdown();
        assert!(!state.dropdown.is_open);
    }

    #[test]
    fn accept_dropdown_symbol() {
        let child = make_symbol("bar", BreadcrumbKind::Function, 2, vec![]);
        let parent = make_symbol("Foo", BreadcrumbKind::Class, 0, vec![child]);

        let mut state = BreadcrumbsState::default();
        state.update(Path::new("x.rs"), &[parent], Position::new(3, 0));
        state.click_segment(1);

        let action = state.accept_dropdown();
        assert!(action.is_some());
        match action.unwrap() {
            BreadcrumbAction::NavigateToSymbol(range) => {
                assert_eq!(range.start.line, 2);
            }
            other => panic!("expected NavigateToSymbol, got {other:?}"),
        }
        assert!(!state.dropdown.is_open);
    }

    #[test]
    fn symbol_outline() {
        let symbols = vec![
            make_symbol(
                "Foo",
                BreadcrumbKind::Class,
                0,
                vec![make_symbol("bar", BreadcrumbKind::Method, 2, vec![])],
            ),
            make_symbol("baz", BreadcrumbKind::Function, 20, vec![]),
        ];
        let outline = BreadcrumbsState::symbol_outline(&symbols);
        assert_eq!(outline.len(), 2);
        assert_eq!(outline[0].label, "Foo");
        assert_eq!(outline[0].children.len(), 1);
        assert_eq!(outline[1].label, "baz");
    }

    #[test]
    fn focus_next_closes_dropdown() {
        let child = make_symbol("bar", BreadcrumbKind::Function, 2, vec![]);
        let parent = make_symbol("Foo", BreadcrumbKind::Class, 0, vec![child]);

        let mut state = BreadcrumbsState::default();
        state.update(Path::new("x.rs"), &[parent], Position::new(3, 0));
        state.click_segment(1);
        assert!(state.dropdown.is_open);

        state.focus_next();
        assert!(!state.dropdown.is_open);
    }

    #[test]
    fn hide_closes_dropdown() {
        let mut state = BreadcrumbsState::default();
        state.update(Path::new("a.rs"), &[], Position::ZERO);
        state.show();
        state.dropdown.open(0, vec![]);
        state.hide();
        assert!(!state.dropdown.is_open);
    }

    #[test]
    fn breadcrumbs_with_workspace_root() {
        let root = Path::new("/workspace/my-project");
        let path = Path::new("/workspace/my-project/src/main.rs");
        let segments =
            compute_breadcrumbs_with_root(path, Some(root), &[], Position::ZERO);
        assert_eq!(segments[0].kind, BreadcrumbKind::Root);
        assert_eq!(segments[0].label, "my-project");
        assert_eq!(segments[1].label, "src");
        assert_eq!(segments[2].label, "main.rs");
        assert_eq!(segments[2].kind, BreadcrumbKind::File);
    }

    #[test]
    fn breadcrumb_icon_for_kind() {
        assert_eq!(
            BreadcrumbIcon::for_kind(BreadcrumbKind::Root, None),
            BreadcrumbIcon::Root
        );
        assert_eq!(
            BreadcrumbIcon::for_kind(BreadcrumbKind::Folder, None),
            BreadcrumbIcon::Folder
        );
        assert_eq!(
            BreadcrumbIcon::for_kind(BreadcrumbKind::File, Some("rs")),
            BreadcrumbIcon::File("rs".to_string())
        );
        assert_eq!(
            BreadcrumbIcon::for_kind(BreadcrumbKind::Function, None),
            BreadcrumbIcon::Symbol(BreadcrumbKind::Function)
        );
    }
}
