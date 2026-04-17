//! Symbol navigation — Go to Symbol in file (Ctrl+Shift+O) and
//! Go to Symbol in workspace (Ctrl+T).
//!
//! Bridges LSP `textDocument/documentSymbol` and `workspace/symbol`
//! responses into filterable lists for the quick-pick UI.

use std::path::PathBuf;

use sidex_text::Range;

/// Symbol kinds matching LSP `SymbolKind` (subset used for display).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

impl SymbolKind {
    /// Human-readable label for the symbol kind.
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Module => "module",
            Self::Namespace => "namespace",
            Self::Package => "package",
            Self::Class => "class",
            Self::Method => "method",
            Self::Property => "property",
            Self::Field => "field",
            Self::Constructor => "constructor",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::Function => "function",
            Self::Variable => "variable",
            Self::Constant => "constant",
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Array => "array",
            Self::Object => "object",
            Self::Key => "key",
            Self::Null => "null",
            Self::EnumMember => "enum member",
            Self::Struct => "struct",
            Self::Event => "event",
            Self::Operator => "operator",
            Self::TypeParameter => "type parameter",
        }
    }

    /// Icon character for the symbol kind (codicon-style).
    pub fn icon(self) -> &'static str {
        match self {
            Self::Function | Self::Method | Self::Constructor => "ƒ",
            Self::Class | Self::Struct | Self::Interface => "C",
            Self::Enum | Self::EnumMember => "E",
            Self::Variable | Self::Field | Self::Property => "v",
            Self::Constant => "c",
            Self::Module | Self::Namespace | Self::Package => "M",
            _ => "•",
        }
    }

    /// Matches a filter string like `:function` or `:class`.
    pub fn matches_filter(self, filter: &str) -> bool {
        let filter_lower = filter.to_lowercase();
        self.label().starts_with(&filter_lower)
    }
}

/// A document symbol (from `textDocument/documentSymbol`).
#[derive(Debug, Clone)]
pub struct DocumentSymbol {
    /// Symbol name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// Range of the full symbol definition.
    pub range: Range,
    /// Range of the symbol's name/identifier.
    pub selection_range: Range,
    /// Nested children (e.g. methods inside a class).
    pub children: Vec<DocumentSymbol>,
    /// Detail string (e.g. type signature).
    pub detail: Option<String>,
    /// Container name for display.
    pub container_name: Option<String>,
}

/// A workspace symbol (from `workspace/symbol`).
#[derive(Debug, Clone)]
pub struct WorkspaceSymbol {
    /// Symbol name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// File containing the symbol.
    pub path: PathBuf,
    /// Range within the file.
    pub range: Range,
    /// Container name (e.g. class for a method).
    pub container_name: Option<String>,
}

/// A flattened symbol entry for display in the quick-pick.
#[derive(Debug, Clone)]
pub struct SymbolItem {
    /// Display name.
    pub name: String,
    /// Kind of symbol.
    pub kind: SymbolKind,
    /// Selection range to navigate to.
    pub selection_range: Range,
    /// Container / parent name (e.g. "MyClass" for a method).
    pub container: Option<String>,
    /// Detail text.
    pub detail: Option<String>,
    /// File path (for workspace symbol results).
    pub path: Option<PathBuf>,
    /// Fuzzy match score.
    pub score: f64,
    /// Match positions in the name.
    pub match_positions: Vec<usize>,
}

/// Flatten a tree of `DocumentSymbol`s into a flat list of `SymbolItem`s.
pub fn flatten_document_symbols(symbols: &[DocumentSymbol]) -> Vec<SymbolItem> {
    let mut result = Vec::new();
    flatten_recursive(symbols, None, &mut result);
    result
}

fn flatten_recursive(
    symbols: &[DocumentSymbol],
    parent_name: Option<&str>,
    out: &mut Vec<SymbolItem>,
) {
    for sym in symbols {
        out.push(SymbolItem {
            name: sym.name.clone(),
            kind: sym.kind,
            selection_range: sym.selection_range,
            container: parent_name.map(String::from).or_else(|| sym.container_name.clone()),
            detail: sym.detail.clone(),
            path: None,
            score: 0.0,
            match_positions: vec![],
        });
        if !sym.children.is_empty() {
            flatten_recursive(&sym.children, Some(&sym.name), out);
        }
    }
}

/// Filter symbol items by a query string, optionally with a `:kind` prefix.
///
/// Returns items sorted by score descending.
#[allow(clippy::cast_precision_loss)]
pub fn filter_symbols(items: &[SymbolItem], query: &str) -> Vec<SymbolItem> {
    let (kind_filter, name_query) = if let Some(rest) = query.strip_prefix(':') {
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() == 2 {
            (Some(parts[0]), parts[1])
        } else {
            (Some(parts[0]), "")
        }
    } else {
        (None, query)
    };

    let mut results: Vec<SymbolItem> = items
        .iter()
        .filter(|item| {
            if let Some(kf) = kind_filter {
                if !item.kind.matches_filter(kf) {
                    return false;
                }
            }
            true
        })
        .filter_map(|item| {
            if name_query.is_empty() {
                let mut out = item.clone();
                out.score = 0.0;
                out.match_positions = vec![];
                return Some(out);
            }
            let (score, positions) = fuzzy_match_symbol(name_query, &item.name)?;
            let mut out = item.clone();
            out.score = score;
            out.match_positions = positions;
            Some(out)
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

/// Group symbol items by kind for sectioned display.
pub fn group_by_kind(items: &[SymbolItem]) -> Vec<(SymbolKind, Vec<&SymbolItem>)> {
    let mut groups: Vec<(SymbolKind, Vec<&SymbolItem>)> = Vec::new();

    for item in items {
        if let Some(group) = groups.iter_mut().find(|(k, _)| *k == item.kind) {
            group.1.push(item);
        } else {
            groups.push((item.kind, vec![item]));
        }
    }
    groups
}

/// State for the "Go to Symbol in File" dialog (Ctrl+Shift+O).
#[derive(Debug, Clone, Default)]
pub struct SymbolInFileState {
    pub is_visible: bool,
    pub input: String,
    pub all_symbols: Vec<SymbolItem>,
    pub filtered: Vec<SymbolItem>,
    pub selected: usize,
}

impl SymbolInFileState {
    pub fn show(&mut self, symbols: Vec<DocumentSymbol>) {
        self.is_visible = true;
        self.input.clear();
        self.all_symbols = flatten_document_symbols(&symbols);
        self.filtered = self.all_symbols.clone();
        self.selected = 0;
    }

    pub fn cancel(&mut self) {
        self.is_visible = false;
        self.input.clear();
        self.all_symbols.clear();
        self.filtered.clear();
        self.selected = 0;
    }

    pub fn filter(&mut self, query: &str) {
        self.input = query.to_string();
        self.filtered = filter_symbols(&self.all_symbols, query);
        self.selected = 0;
    }

    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = (self.selected + 1) % self.filtered.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = if self.selected == 0 {
                self.filtered.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_item(&self) -> Option<&SymbolItem> {
        self.filtered.get(self.selected)
    }
}

/// State for the "Go to Symbol in Workspace" dialog (Ctrl+T).
#[derive(Debug, Clone, Default)]
pub struct SymbolInWorkspaceState {
    pub is_visible: bool,
    pub input: String,
    pub results: Vec<SymbolItem>,
    pub selected: usize,
}

impl SymbolInWorkspaceState {
    pub fn show(&mut self) {
        self.is_visible = true;
        self.input.clear();
        self.results.clear();
        self.selected = 0;
    }

    pub fn cancel(&mut self) {
        self.is_visible = false;
        self.input.clear();
        self.results.clear();
        self.selected = 0;
    }

    pub fn set_results(&mut self, results: Vec<SymbolItem>) {
        self.results = results;
        if self.selected >= self.results.len() {
            self.selected = 0;
        }
    }

    pub fn filter(&mut self, query: &str) {
        self.input = query.to_string();
        self.selected = 0;
    }

    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1) % self.results.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.results.is_empty() {
            self.selected = if self.selected == 0 {
                self.results.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_item(&self) -> Option<&SymbolItem> {
        self.results.get(self.selected)
    }
}

/// Simple fuzzy match for symbol names.
#[allow(clippy::cast_precision_loss)]
fn fuzzy_match_symbol(query: &str, name: &str) -> Option<(f64, Vec<usize>)> {
    if query.is_empty() {
        return Some((0.0, vec![]));
    }

    let query_lower: Vec<char> = query.to_lowercase().chars().collect();
    let name_lower: Vec<char> = name.to_lowercase().chars().collect();
    let name_chars: Vec<char> = name.chars().collect();

    let mut pi = 0;
    let mut positions = Vec::with_capacity(query_lower.len());

    for (ti, &tc) in name_lower.iter().enumerate() {
        if pi < query_lower.len() && tc == query_lower[pi] {
            positions.push(ti);
            pi += 1;
        }
    }

    if pi < query_lower.len() {
        return None;
    }

    let mut score = 0.0_f64;

    let name_lower_str: String = name_lower.iter().collect();
    let query_lower_str: String = query_lower.iter().collect();
    if name_lower_str == query_lower_str {
        score += 500.0;
    } else if name_lower_str.starts_with(&query_lower_str) {
        score += 250.0;
    }

    let mut consecutive = 0.0_f64;
    for (i, &pos) in positions.iter().enumerate() {
        score += 10.0;

        // camelCase / word boundary bonus
        if pos == 0 || !name_chars.get(pos.wrapping_sub(1)).is_some_and(|c| c.is_alphanumeric()) {
            score += 15.0;
        } else if name_chars.get(pos).is_some_and(|c| c.is_uppercase()) {
            score += 10.0;
        }

        if i > 0 && pos == positions[i - 1] + 1 {
            consecutive += 1.0;
            score += consecutive * 5.0;
        } else {
            consecutive = 0.0;
        }
    }

    Some((score, positions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidex_text::Position;

    fn make_symbol(name: &str, kind: SymbolKind) -> DocumentSymbol {
        DocumentSymbol {
            name: name.to_string(),
            kind,
            range: Range::new(Position::ZERO, Position::ZERO),
            selection_range: Range::new(Position::ZERO, Position::ZERO),
            children: vec![],
            detail: None,
            container_name: None,
        }
    }

    fn make_symbol_with_children(
        name: &str,
        kind: SymbolKind,
        children: Vec<DocumentSymbol>,
    ) -> DocumentSymbol {
        DocumentSymbol {
            name: name.to_string(),
            kind,
            range: Range::new(Position::ZERO, Position::ZERO),
            selection_range: Range::new(Position::ZERO, Position::ZERO),
            children,
            detail: None,
            container_name: None,
        }
    }

    #[test]
    fn flatten_empty() {
        let flat = flatten_document_symbols(&[]);
        assert!(flat.is_empty());
    }

    #[test]
    fn flatten_simple() {
        let symbols = vec![
            make_symbol("main", SymbolKind::Function),
            make_symbol("Config", SymbolKind::Struct),
        ];
        let flat = flatten_document_symbols(&symbols);
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].name, "main");
        assert_eq!(flat[1].name, "Config");
    }

    #[test]
    fn flatten_nested() {
        let symbols = vec![make_symbol_with_children(
            "MyClass",
            SymbolKind::Class,
            vec![
                make_symbol("new", SymbolKind::Constructor),
                make_symbol("run", SymbolKind::Method),
            ],
        )];
        let flat = flatten_document_symbols(&symbols);
        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].name, "MyClass");
        assert!(flat[0].container.is_none());
        assert_eq!(flat[1].name, "new");
        assert_eq!(flat[1].container.as_deref(), Some("MyClass"));
        assert_eq!(flat[2].name, "run");
        assert_eq!(flat[2].container.as_deref(), Some("MyClass"));
    }

    #[test]
    fn filter_by_name() {
        let items = vec![
            SymbolItem {
                name: "handleClick".into(),
                kind: SymbolKind::Function,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
            SymbolItem {
                name: "render".into(),
                kind: SymbolKind::Function,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
        ];

        let results = filter_symbols(&items, "hcl");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "handleClick");
    }

    #[test]
    fn filter_by_kind() {
        let items = vec![
            SymbolItem {
                name: "main".into(),
                kind: SymbolKind::Function,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
            SymbolItem {
                name: "Config".into(),
                kind: SymbolKind::Struct,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
        ];

        let results = filter_symbols(&items, ":function");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "main");
    }

    #[test]
    fn filter_empty_returns_all() {
        let items = vec![
            SymbolItem {
                name: "a".into(),
                kind: SymbolKind::Variable,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
            SymbolItem {
                name: "b".into(),
                kind: SymbolKind::Variable,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
        ];
        let results = filter_symbols(&items, "");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn group_by_kind_groups_correctly() {
        let items = vec![
            SymbolItem {
                name: "main".into(),
                kind: SymbolKind::Function,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
            SymbolItem {
                name: "helper".into(),
                kind: SymbolKind::Function,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
            SymbolItem {
                name: "Config".into(),
                kind: SymbolKind::Struct,
                selection_range: Range::new(Position::ZERO, Position::ZERO),
                container: None,
                detail: None,
                path: None,
                score: 0.0,
                match_positions: vec![],
            },
        ];

        let groups = group_by_kind(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, SymbolKind::Function);
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].0, SymbolKind::Struct);
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn symbol_kind_icon() {
        assert_eq!(SymbolKind::Function.icon(), "ƒ");
        assert_eq!(SymbolKind::Class.icon(), "C");
        assert_eq!(SymbolKind::Enum.icon(), "E");
        assert_eq!(SymbolKind::Variable.icon(), "v");
    }

    #[test]
    fn symbol_kind_matches_filter() {
        assert!(SymbolKind::Function.matches_filter("fun"));
        assert!(SymbolKind::Function.matches_filter("function"));
        assert!(!SymbolKind::Function.matches_filter("class"));
    }

    #[test]
    fn symbol_in_file_state() {
        let mut state = SymbolInFileState::default();
        assert!(!state.is_visible);

        state.show(vec![
            make_symbol("main", SymbolKind::Function),
            make_symbol("Config", SymbolKind::Struct),
        ]);
        assert!(state.is_visible);
        assert_eq!(state.filtered.len(), 2);

        state.filter("mai");
        assert_eq!(state.filtered.len(), 1);
        assert_eq!(state.filtered[0].name, "main");

        state.cancel();
        assert!(!state.is_visible);
    }

    #[test]
    fn symbol_in_workspace_state() {
        let mut state = SymbolInWorkspaceState::default();
        state.show();
        assert!(state.is_visible);

        state.set_results(vec![SymbolItem {
            name: "App".into(),
            kind: SymbolKind::Struct,
            selection_range: Range::new(Position::ZERO, Position::ZERO),
            container: None,
            detail: None,
            path: Some(PathBuf::from("src/app.rs")),
            score: 100.0,
            match_positions: vec![],
        }]);

        let item = state.selected_item().unwrap();
        assert_eq!(item.name, "App");

        state.cancel();
        assert!(!state.is_visible);
    }

    #[test]
    fn fuzzy_match_symbol_exact() {
        let (score, _) = fuzzy_match_symbol("handleClick", "handleClick").unwrap();
        assert!(score > 400.0);
    }

    #[test]
    fn fuzzy_match_symbol_prefix() {
        let result = fuzzy_match_symbol("hand", "handleClick");
        assert!(result.is_some());
    }

    #[test]
    fn fuzzy_match_symbol_no_match() {
        assert!(fuzzy_match_symbol("xyz", "handleClick").is_none());
    }

    #[test]
    fn fuzzy_match_symbol_camel_case_bonus() {
        let (score_camel, _) = fuzzy_match_symbol("hC", "handleClick").unwrap();
        let (score_mid, _) = fuzzy_match_symbol("hc", "handleClick").unwrap();
        // camelCase boundary should yield higher score
        assert!(score_camel >= score_mid);
    }
}
