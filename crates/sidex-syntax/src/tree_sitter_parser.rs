//! Advanced tree-sitter integration with manager, query caching, incremental
//! parsing, language injection, fold/indent queries, and local variable tracking.

use std::collections::HashMap;

use streaming_iterator::StreamingIterator;
use tree_sitter::{InputEdit, Parser, Query, QueryCursor, Tree};

/// Manages multiple tree-sitter parsers and cached queries, keyed by language.
#[derive(Default)]
pub struct TreeSitterManager {
    pub parsers: HashMap<String, TreeSitterParserState>,
    pub query_cache: HashMap<String, TreeSitterQueries>,
}

impl std::fmt::Debug for TreeSitterManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeSitterManager")
            .field("parser_count", &self.parsers.len())
            .field("cached_queries", &self.query_cache.len())
            .finish()
    }
}

/// Per-language parser state holding the tree-sitter parser, current tree,
/// and compiled queries.
pub struct TreeSitterParserState {
    pub language_name: String,
    pub parser: Parser,
    pub tree: Option<Tree>,
    pub highlight_query: Option<Query>,
    pub locals_query: Option<Query>,
    pub injections_query: Option<Query>,
}

impl std::fmt::Debug for TreeSitterParserState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeSitterParserState")
            .field("language_name", &self.language_name)
            .field("has_tree", &self.tree.is_some())
            .field("has_highlight_query", &self.highlight_query.is_some())
            .field("has_locals_query", &self.locals_query.is_some())
            .field("has_injections_query", &self.injections_query.is_some())
            .finish_non_exhaustive()
    }
}

/// Raw query source strings for a language, cached for reuse.
#[derive(Debug, Clone, Default)]
pub struct TreeSitterQueries {
    pub highlights: String,
    pub locals: Option<String>,
    pub injections: Option<String>,
    pub folds: Option<String>,
    pub indents: Option<String>,
}

/// A range within the source where a different language should be parsed
/// (language injection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectionRange {
    pub language: String,
    pub range: tree_sitter::Range,
}

/// A local definition or reference tracked by the locals query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalBinding {
    Definition {
        name: String,
        start_byte: usize,
        end_byte: usize,
    },
    Reference {
        name: String,
        start_byte: usize,
        end_byte: usize,
    },
}

/// Errors from tree-sitter operations.
#[derive(Debug, thiserror::Error)]
pub enum TreeSitterError {
    #[error("language not registered: {0}")]
    LanguageNotRegistered(String),
    #[error("invalid query: {0}")]
    InvalidQuery(#[from] tree_sitter::QueryError),
    #[error("parse failed")]
    ParseFailed,
    #[error("language version mismatch")]
    LanguageVersionMismatch,
}

impl TreeSitterManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a language with its tree-sitter grammar and optional query
    /// sources.
    pub fn register_language(
        &mut self,
        name: &str,
        ts_language: &tree_sitter::Language,
        queries: TreeSitterQueries,
    ) -> Result<(), TreeSitterError> {
        let mut parser = Parser::new();
        parser
            .set_language(ts_language)
            .map_err(|_| TreeSitterError::LanguageVersionMismatch)?;

        let highlight_query = if queries.highlights.is_empty() {
            None
        } else {
            Some(Query::new(ts_language, &queries.highlights)?)
        };
        let locals_query = match &queries.locals {
            Some(src) if !src.is_empty() => Some(Query::new(ts_language, src)?),
            _ => None,
        };
        let injections_query = match &queries.injections {
            Some(src) if !src.is_empty() => Some(Query::new(ts_language, src)?),
            _ => None,
        };

        self.parsers.insert(
            name.to_owned(),
            TreeSitterParserState {
                language_name: name.to_owned(),
                parser,
                tree: None,
                highlight_query,
                locals_query,
                injections_query,
            },
        );
        self.query_cache.insert(name.to_owned(), queries);
        Ok(())
    }

    /// Full parse of source for the given language.
    pub fn parse(&mut self, language: &str, source: &str) -> Result<&Tree, TreeSitterError> {
        let state = self
            .parsers
            .get_mut(language)
            .ok_or_else(|| TreeSitterError::LanguageNotRegistered(language.to_owned()))?;
        state.tree = state.parser.parse(source, None);
        state.tree.as_ref().ok_or(TreeSitterError::ParseFailed)
    }

    /// Incremental re-parse after edits.
    pub fn parse_incremental(
        &mut self,
        language: &str,
        source: &str,
        edits: &[InputEdit],
    ) -> Result<&Tree, TreeSitterError> {
        let state = self
            .parsers
            .get_mut(language)
            .ok_or_else(|| TreeSitterError::LanguageNotRegistered(language.to_owned()))?;

        if let Some(ref old_tree) = state.tree {
            let mut tree = old_tree.clone();
            for edit in edits {
                tree.edit(edit);
            }
            state.tree = state.parser.parse(source, Some(&tree));
        } else {
            state.tree = state.parser.parse(source, None);
        }
        state.tree.as_ref().ok_or(TreeSitterError::ParseFailed)
    }

    /// Returns a reference to the current tree for a language.
    #[must_use]
    pub fn tree(&self, language: &str) -> Option<&Tree> {
        self.parsers.get(language).and_then(|s| s.tree.as_ref())
    }

    /// Returns true if the language has been registered.
    #[must_use]
    pub fn has_language(&self, language: &str) -> bool {
        self.parsers.contains_key(language)
    }

    /// Number of registered languages.
    #[must_use]
    pub fn language_count(&self) -> usize {
        self.parsers.len()
    }
}

/// Performs an incremental parse using an existing parser state and old tree.
pub fn parse_incremental(
    state: &mut TreeSitterParserState,
    old_tree: &Tree,
    source: &str,
    edits: &[InputEdit],
) -> Result<Tree, TreeSitterError> {
    let mut tree = old_tree.clone();
    for edit in edits {
        tree.edit(edit);
    }
    state
        .parser
        .parse(source, Some(&tree))
        .ok_or(TreeSitterError::ParseFailed)
}

/// Extracts language injection ranges from a tree using an injections query.
///
/// Each injection range identifies a sub-region of the source that should be
/// parsed with a different grammar (e.g. JavaScript inside HTML `<script>` tags).
pub fn get_injections(tree: &Tree, source: &str, injections_query: &Query) -> Vec<InjectionRange> {
    let mut cursor = QueryCursor::new();
    let root = tree.root_node();
    let mut results = Vec::new();

    let language_idx = injections_query
        .capture_names()
        .iter()
        .position(|n| *n == "injection.content");
    let lang_name_idx = injections_query
        .capture_names()
        .iter()
        .position(|n| *n == "injection.language");

    let mut matches = cursor.matches(injections_query, root, source.as_bytes());
    while let Some(m) = matches.next() {
        let mut language_name: Option<String> = None;
        let mut content_range: Option<tree_sitter::Range> = None;

        for capture in m.captures {
            if Some(capture.index as usize) == lang_name_idx {
                if let Ok(text) = capture.node.utf8_text(source.as_bytes()) {
                    language_name = Some(String::from(text).to_lowercase());
                }
            }
            if Some(capture.index as usize) == language_idx {
                content_range = Some(capture.node.range());
            }
        }

        if let (Some(lang), Some(range)) = (language_name, content_range) {
            results.push(InjectionRange {
                language: lang,
                range,
            });
        }
    }
    results
}

/// Extracts foldable regions from a tree using a folds query.
///
/// Returns `(start_line, end_line)` pairs for each foldable region.
pub fn get_fold_ranges(tree: &Tree, source: &str, folds_query: &Query) -> Vec<(usize, usize)> {
    let mut cursor = QueryCursor::new();
    let root = tree.root_node();
    let mut ranges = Vec::new();

    let mut matches = cursor.matches(folds_query, root, source.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let start = capture.node.start_position().row;
            let end = capture.node.end_position().row;
            if end > start {
                ranges.push((start, end));
            }
        }
    }
    ranges.sort_unstable();
    ranges.dedup();
    ranges
}

/// Extracts indentation hints from a tree using an indents query.
///
/// Returns a map from line number to signed indent delta (+1 for indent, -1 for outdent).
pub fn get_indent_hints(tree: &Tree, source: &str, indents_query: &Query) -> HashMap<usize, i32> {
    let mut cursor = QueryCursor::new();
    let root = tree.root_node();
    let mut hints: HashMap<usize, i32> = HashMap::new();

    let indent_idx = indents_query
        .capture_names()
        .iter()
        .position(|n| *n == "indent");
    let outdent_idx = indents_query
        .capture_names()
        .iter()
        .position(|n| *n == "outdent");

    let mut matches = cursor.matches(indents_query, root, source.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let line = capture.node.start_position().row;
            if Some(capture.index as usize) == indent_idx {
                *hints.entry(line).or_insert(0) += 1;
            } else if Some(capture.index as usize) == outdent_idx {
                *hints.entry(line).or_insert(0) -= 1;
            }
        }
    }
    hints
}

/// Extracts local variable definitions and references using a locals query.
pub fn get_local_bindings(tree: &Tree, source: &str, locals_query: &Query) -> Vec<LocalBinding> {
    let mut cursor = QueryCursor::new();
    let root = tree.root_node();
    let mut bindings = Vec::new();

    let def_idx = locals_query
        .capture_names()
        .iter()
        .position(|n| *n == "local.definition");
    let ref_idx = locals_query
        .capture_names()
        .iter()
        .position(|n| *n == "local.reference");

    let mut matches = cursor.matches(locals_query, root, source.as_bytes());
    while let Some(m) = matches.next() {
        for capture in m.captures {
            let node = capture.node;
            let name = node.utf8_text(source.as_bytes()).unwrap_or("").to_owned();
            let start_byte = node.start_byte();
            let end_byte = node.end_byte();

            if Some(capture.index as usize) == def_idx {
                bindings.push(LocalBinding::Definition {
                    name,
                    start_byte,
                    end_byte,
                });
            } else if Some(capture.index as usize) == ref_idx {
                bindings.push(LocalBinding::Reference {
                    name,
                    start_byte,
                    end_byte,
                });
            }
        }
    }
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_language() -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    #[test]
    fn manager_register_and_parse() {
        let mut mgr = TreeSitterManager::new();
        let queries = TreeSitterQueries {
            highlights: "(line_comment) @comment".into(),
            ..Default::default()
        };
        mgr.register_language("rust", rust_language(), queries)
            .unwrap();
        assert!(mgr.has_language("rust"));
        assert_eq!(mgr.language_count(), 1);

        let tree = mgr.parse("rust", "fn main() {}").unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn manager_incremental_parse() {
        let mut mgr = TreeSitterManager::new();
        mgr.register_language("rust", rust_language(), TreeSitterQueries::default())
            .unwrap();
        mgr.parse("rust", "fn main() { let x = 1; }").unwrap();

        let edit = InputEdit {
            start_byte: 20,
            old_end_byte: 21,
            new_end_byte: 22,
            start_position: tree_sitter::Point { row: 0, column: 20 },
            old_end_position: tree_sitter::Point { row: 0, column: 21 },
            new_end_position: tree_sitter::Point { row: 0, column: 22 },
        };
        let tree = mgr
            .parse_incremental("rust", "fn main() { let x = 42; }", &[edit])
            .unwrap();
        assert!(!tree.root_node().has_error());
    }

    #[test]
    fn manager_unknown_language_error() {
        let mut mgr = TreeSitterManager::new();
        let result = mgr.parse("nonexistent", "hello");
        assert!(result.is_err());
    }

    #[test]
    fn tree_sitter_queries_default() {
        let q = TreeSitterQueries::default();
        assert!(q.highlights.is_empty());
        assert!(q.locals.is_none());
        assert!(q.injections.is_none());
        assert!(q.folds.is_none());
        assert!(q.indents.is_none());
    }

    #[test]
    fn parse_incremental_fn() {
        let lang = rust_language();
        let mut parser = Parser::new();
        parser.set_language(&lang).unwrap();
        let mut state = TreeSitterParserState {
            language_name: "rust".into(),
            parser,
            tree: None,
            highlight_query: None,
            locals_query: None,
            injections_query: None,
        };

        let source = "fn main() { let x = 1; }";
        state.tree = state.parser.parse(source, None);
        let old_tree = state.tree.as_ref().unwrap().clone();

        let edit = InputEdit {
            start_byte: 20,
            old_end_byte: 21,
            new_end_byte: 22,
            start_position: tree_sitter::Point { row: 0, column: 20 },
            old_end_position: tree_sitter::Point { row: 0, column: 21 },
            new_end_position: tree_sitter::Point { row: 0, column: 22 },
        };
        let new_tree =
            parse_incremental(&mut state, &old_tree, "fn main() { let x = 42; }", &[edit]).unwrap();
        assert!(!new_tree.root_node().has_error());
    }

    #[test]
    fn debug_impls() {
        let mgr = TreeSitterManager::new();
        let dbg = format!("{mgr:?}");
        assert!(dbg.contains("TreeSitterManager"));
    }
}
