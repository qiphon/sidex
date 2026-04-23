//! LSP semantic tokens overlay for merging semantic token data with syntax
//! highlighting events.
//!
//! Semantic tokens provide richer type information from the language server
//! that overrides the coarser tree-sitter/TextMate tokens where they exist.
//!
//! This module provides:
//! - Decoding and encoding of delta-encoded LSP semantic token data.
//! - A [`SemanticTokensManager`] for per-file token storage and legend management.
//! - Merging semantic tokens with syntax highlight tokens, with semantic priority.
//! - Delta updates via [`apply_semantic_token_delta`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::highlight::{HighlightEvent, HighlightToken, TokenModifiers, TokenScope};

/// A single semantic token as received from an LSP server (absolute coordinates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticToken {
    pub line: u32,
    pub start: u32,
    pub length: u32,
    pub token_type: u32,
    pub modifiers: u32,
}

/// Maps token type/modifier indices to their string names, as negotiated
/// during LSP initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticTokenLegend {
    pub token_types: Vec<String>,
    pub token_modifiers: Vec<String>,
}

impl SemanticTokenLegend {
    #[must_use]
    pub fn new(token_types: Vec<String>, token_modifiers: Vec<String>) -> Self {
        Self {
            token_types,
            token_modifiers,
        }
    }

    /// Resolves a token type index to its name.
    #[must_use]
    pub fn type_name(&self, idx: u32) -> Option<&str> {
        self.token_types.get(idx as usize).map(String::as_str)
    }

    /// Returns the modifier names for the given bitmask.
    #[must_use]
    pub fn modifier_names(&self, mask: u32) -> Vec<&str> {
        let mut names = Vec::new();
        for (i, name) in self.token_modifiers.iter().enumerate() {
            if mask & (1 << i) != 0 {
                names.push(name.as_str());
            }
        }
        names
    }
}

/// A styled span in the final merged output, carrying both position and
/// semantic information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    /// Start byte offset in the source.
    pub start: usize,
    /// End byte offset in the source.
    pub end: usize,
    /// The token type name (e.g. `"function"`, `"variable"`), or `None` for
    /// unstyled source text.
    pub token_type: Option<String>,
    /// Modifier names (e.g. `"declaration"`, `"readonly"`).
    pub modifiers: Vec<String>,
}

/// Merges syntax highlight events with LSP semantic tokens.
///
/// Where a semantic token overlaps a syntax span, the semantic information
/// takes priority. Gaps between semantic tokens fall back to the syntax
/// highlighting.
pub fn merge_semantic_tokens(
    syntax_tokens: &[HighlightEvent],
    semantic: &[SemanticToken],
    legend: &SemanticTokenLegend,
    source: &str,
) -> Vec<StyledSpan> {
    let mut spans = Vec::new();

    let sem_ranges: Vec<(usize, usize, Option<String>, Vec<String>)> = semantic
        .iter()
        .filter_map(|tok| {
            let byte_start = line_col_to_byte(source, tok.line, tok.start)?;
            let byte_end = byte_start + tok.length as usize;
            let type_name = legend.type_name(tok.token_type).map(String::from);
            let mods = legend
                .modifier_names(tok.modifiers)
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            Some((byte_start, byte_end, type_name, mods))
        })
        .collect();

    let mut syntax_spans: Vec<(usize, usize)> = Vec::new();
    for event in syntax_tokens {
        if let HighlightEvent::Source { start, end } = event {
            syntax_spans.push((*start, *end));
        }
    }

    if syntax_spans.is_empty() && sem_ranges.is_empty() {
        return spans;
    }

    let max_byte = syntax_spans
        .iter()
        .map(|(_, e)| *e)
        .chain(sem_ranges.iter().map(|(_, e, _, _)| *e))
        .max()
        .unwrap_or(0);

    let mut pos = 0;
    while pos < max_byte {
        if let Some((s, e, ref ty, ref mods)) = sem_ranges.iter().find(|(s, _, _, _)| *s == pos) {
            spans.push(StyledSpan {
                start: *s,
                end: *e,
                token_type: ty.clone(),
                modifiers: mods.clone(),
            });
            pos = *e;
            continue;
        }

        let next_sem_start = sem_ranges
            .iter()
            .filter(|(s, _, _, _)| *s > pos)
            .map(|(s, _, _, _)| *s)
            .min()
            .unwrap_or(max_byte);

        let gap_end = next_sem_start.min(max_byte);
        if pos < gap_end {
            spans.push(StyledSpan {
                start: pos,
                end: gap_end,
                token_type: None,
                modifiers: Vec::new(),
            });
        }
        pos = gap_end;
    }

    spans
}

/// Decodes delta-encoded semantic tokens into absolute-position tokens.
///
/// The LSP protocol sends tokens as a flat `[deltaLine, deltaStartChar,
/// length, tokenType, tokenModifiers]` array. This function converts them
/// to absolute [`SemanticToken`] values.
pub fn decode_semantic_tokens(data: &[u32]) -> Vec<SemanticToken> {
    let mut tokens = Vec::with_capacity(data.len() / 5);
    let mut line = 0u32;
    let mut start = 0u32;

    for chunk in data.chunks_exact(5) {
        let delta_line = chunk[0];
        let delta_start = chunk[1];
        let length = chunk[2];
        let token_type = chunk[3];
        let modifiers = chunk[4];

        line += delta_line;
        if delta_line > 0 {
            start = delta_start;
        } else {
            start += delta_start;
        }

        tokens.push(SemanticToken {
            line,
            start,
            length,
            token_type,
            modifiers,
        });
    }

    tokens
}

/// Encodes absolute semantic tokens back into delta-encoded format.
pub fn encode_semantic_tokens(tokens: &[SemanticToken]) -> Vec<u32> {
    let mut data = Vec::with_capacity(tokens.len() * 5);
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for tok in tokens {
        let delta_line = tok.line - prev_line;
        let delta_start = if delta_line > 0 {
            tok.start
        } else {
            tok.start - prev_start
        };

        data.push(delta_line);
        data.push(delta_start);
        data.push(tok.length);
        data.push(tok.token_type);
        data.push(tok.modifiers);

        prev_line = tok.line;
        prev_start = tok.start;
    }

    data
}

/// Applies an incremental semantic token edit (delta) to existing data.
pub fn apply_semantic_token_edits(data: &mut Vec<u32>, edits: &[SemanticTokenEdit]) {
    for edit in edits.iter().rev() {
        let start = edit.start as usize;
        let delete_count = edit.delete_count as usize;
        let end = (start + delete_count).min(data.len());
        data.splice(start..end, edit.data.iter().copied());
    }
}

/// A single incremental edit to a semantic token data array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTokenEdit {
    pub start: u32,
    pub delete_count: u32,
    pub data: Vec<u32>,
}

/// Delta update message containing a result ID and a list of edits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTokensDelta {
    pub result_id: Option<String>,
    pub edits: Vec<SemanticTokenEdit>,
}

// ---------------------------------------------------------------------------
// Standard legend
// ---------------------------------------------------------------------------

/// Standard LSP semantic token type names.
pub const STANDARD_TOKEN_TYPES: &[&str] = &[
    "namespace",
    "type",
    "class",
    "enum",
    "interface",
    "struct",
    "typeParameter",
    "parameter",
    "variable",
    "property",
    "enumMember",
    "event",
    "function",
    "method",
    "macro",
    "keyword",
    "modifier",
    "comment",
    "string",
    "number",
    "regexp",
    "operator",
    "decorator",
];

/// Standard LSP semantic token modifier names.
pub const STANDARD_TOKEN_MODIFIERS: &[&str] = &[
    "declaration",
    "definition",
    "readonly",
    "static",
    "deprecated",
    "abstract",
    "async",
    "modification",
    "documentation",
    "defaultLibrary",
];

/// Creates a [`SemanticTokenLegend`] with the standard LSP types and modifiers.
#[must_use]
pub fn standard_semantic_token_legend() -> SemanticTokenLegend {
    SemanticTokenLegend::new(
        STANDARD_TOKEN_TYPES.iter().map(|s| (*s).into()).collect(),
        STANDARD_TOKEN_MODIFIERS
            .iter()
            .map(|s| (*s).into())
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// SemanticTokensManager
// ---------------------------------------------------------------------------

/// Manages per-file semantic tokens and the shared legend.
#[derive(Debug, Clone, Default)]
pub struct SemanticTokensManager {
    pub tokens: HashMap<PathBuf, Vec<SemanticToken>>,
    pub legend: SemanticTokenLegend,
}

impl SemanticTokensManager {
    /// Creates a new manager with the standard LSP legend.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
            legend: standard_semantic_token_legend(),
        }
    }

    /// Creates a manager with a custom legend.
    #[must_use]
    pub fn with_legend(legend: SemanticTokenLegend) -> Self {
        Self {
            tokens: HashMap::new(),
            legend,
        }
    }

    /// Stores decoded tokens for a file (replaces any existing tokens).
    pub fn set_tokens(&mut self, path: &Path, tokens: Vec<SemanticToken>) {
        self.tokens.insert(path.to_path_buf(), tokens);
    }

    /// Stores tokens from delta-encoded LSP data for a file.
    pub fn set_tokens_from_data(&mut self, path: &Path, data: &[u32]) {
        let tokens = decode_semantic_tokens(data);
        self.set_tokens(path, tokens);
    }

    /// Returns the tokens for a file, if any.
    #[must_use]
    pub fn get_tokens(&self, path: &Path) -> Option<&[SemanticToken]> {
        self.tokens.get(path).map(Vec::as_slice)
    }

    /// Applies a delta update to existing tokens for a file.
    pub fn apply_delta(&mut self, path: &Path, delta: &SemanticTokensDelta) {
        let entry = self.tokens.entry(path.to_path_buf()).or_default();
        let mut data = encode_semantic_tokens(entry);
        apply_semantic_token_edits(&mut data, &delta.edits);
        *entry = decode_semantic_tokens(&data);
    }

    /// Removes tokens for a file.
    pub fn remove(&mut self, path: &Path) {
        self.tokens.remove(path);
    }

    /// Clears all stored tokens.
    pub fn clear(&mut self) {
        self.tokens.clear();
    }

    /// Number of files with stored tokens.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.tokens.len()
    }
}

// ---------------------------------------------------------------------------
// Merging semantic tokens with syntax highlight tokens
// ---------------------------------------------------------------------------

/// Maps an LSP semantic token type name to a [`TokenScope`].
#[must_use]
pub fn semantic_type_to_scope(type_name: &str) -> Option<TokenScope> {
    match type_name {
        "namespace" => Some(TokenScope::Namespace),
        "type" => Some(TokenScope::Type),
        "class" => Some(TokenScope::Class),
        "enum" => Some(TokenScope::Enum),
        "interface" => Some(TokenScope::Interface),
        "struct" => Some(TokenScope::Struct),
        "typeParameter" => Some(TokenScope::TypeParameter),
        "parameter" => Some(TokenScope::VariableParameter),
        "variable" => Some(TokenScope::Variable),
        "property" | "event" => Some(TokenScope::Property),
        "enumMember" => Some(TokenScope::EnumMember),
        "function" => Some(TokenScope::Function),
        "method" => Some(TokenScope::Method),
        "macro" => Some(TokenScope::Macro),
        "keyword" => Some(TokenScope::Keyword),
        "modifier" => Some(TokenScope::KeywordModifier),
        "comment" => Some(TokenScope::Comment),
        "string" => Some(TokenScope::String),
        "number" => Some(TokenScope::Number),
        "regexp" => Some(TokenScope::StringRegex),
        "operator" => Some(TokenScope::Operator),
        "decorator" => Some(TokenScope::Decorator),
        _ => None,
    }
}

/// Maps an LSP modifier bitmask to [`TokenModifiers`] using the given legend.
#[must_use]
pub fn semantic_modifiers_to_flags(mask: u32, legend: &SemanticTokenLegend) -> TokenModifiers {
    let names: Vec<&str> = legend.modifier_names(mask);
    TokenModifiers::from_names(&names)
}

/// Merges syntax [`HighlightToken`]s with LSP [`SemanticToken`]s. Where a
/// semantic token overlaps a syntax token, the semantic information wins.
///
/// Both inputs should cover the same line. `source` is the full document text
/// (needed for line/column to byte conversion).
pub fn merge_highlights(
    syntax: &[HighlightToken],
    semantic: &[SemanticToken],
    legend: &SemanticTokenLegend,
    line: u32,
) -> Vec<HighlightToken> {
    let sem_on_line: Vec<&SemanticToken> = semantic.iter().filter(|t| t.line == line).collect();

    if sem_on_line.is_empty() {
        return syntax.to_vec();
    }

    let mut sem_ranges: Vec<(u32, u32, TokenScope, TokenModifiers)> = sem_on_line
        .iter()
        .filter_map(|tok| {
            let scope = legend
                .type_name(tok.token_type)
                .and_then(semantic_type_to_scope)?;
            let mods = semantic_modifiers_to_flags(tok.modifiers, legend);
            Some((tok.start, tok.start + tok.length, scope, mods))
        })
        .collect();
    sem_ranges.sort_by_key(|r| r.0);

    let mut result = Vec::new();

    for syn_tok in syntax {
        let syn_start = syn_tok.start;
        let syn_end = syn_tok.end();
        let mut pos = syn_start;

        for &(sem_start, sem_end, scope, mods) in &sem_ranges {
            if sem_start >= syn_end || sem_end <= syn_start {
                continue;
            }
            let overlap_start = sem_start.max(syn_start);
            let overlap_end = sem_end.min(syn_end);

            if pos < overlap_start {
                result.push(HighlightToken::new(pos, overlap_start - pos, syn_tok.scope));
            }
            result.push(
                HighlightToken::new(overlap_start, overlap_end - overlap_start, scope)
                    .with_modifiers(mods),
            );
            pos = overlap_end;
        }
        if pos < syn_end {
            result.push(HighlightToken::new(pos, syn_end - pos, syn_tok.scope));
        }
    }

    result
}

/// Applies a [`SemanticTokensDelta`] to an existing token list, returning
/// the updated list.
pub fn apply_semantic_token_delta(tokens: &mut Vec<SemanticToken>, delta: &SemanticTokensDelta) {
    let mut data = encode_semantic_tokens(tokens);
    apply_semantic_token_edits(&mut data, &delta.edits);
    *tokens = decode_semantic_tokens(&data);
}

fn line_col_to_byte(source: &str, line: u32, col: u32) -> Option<usize> {
    let mut byte_offset = 0usize;

    for (current_line, src_line) in source.split('\n').enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        if current_line == line as usize {
            let col_byte = src_line
                .char_indices()
                .nth(col as usize)
                .map_or(src_line.len(), |(i, _)| i);
            return Some(byte_offset + col_byte);
        }
        byte_offset += src_line.len() + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_empty() {
        let tokens = decode_semantic_tokens(&[]);
        assert!(tokens.is_empty());
    }

    #[test]
    fn decode_single_token() {
        let data = vec![0, 5, 3, 1, 0];
        let tokens = decode_semantic_tokens(&data);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].line, 0);
        assert_eq!(tokens[0].start, 5);
        assert_eq!(tokens[0].length, 3);
        assert_eq!(tokens[0].token_type, 1);
        assert_eq!(tokens[0].modifiers, 0);
    }

    #[test]
    fn decode_multiple_same_line() {
        let data = vec![0, 5, 3, 0, 0, 0, 10, 4, 1, 0];
        let tokens = decode_semantic_tokens(&data);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].line, 0);
        assert_eq!(tokens[0].start, 5);
        assert_eq!(tokens[1].line, 0);
        assert_eq!(tokens[1].start, 15);
    }

    #[test]
    fn decode_different_lines() {
        let data = vec![0, 5, 3, 0, 0, 2, 3, 4, 1, 0];
        let tokens = decode_semantic_tokens(&data);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].line, 0);
        assert_eq!(tokens[1].line, 2);
        assert_eq!(tokens[1].start, 3);
    }

    #[test]
    fn encode_roundtrip() {
        let original = vec![0, 5, 3, 1, 0, 2, 3, 4, 2, 1, 0, 10, 2, 0, 3];
        let tokens = decode_semantic_tokens(&original);
        let re_encoded = encode_semantic_tokens(&tokens);
        assert_eq!(original, re_encoded);
    }

    #[test]
    fn legend_type_name() {
        let legend = SemanticTokenLegend::new(
            vec!["namespace".into(), "type".into(), "function".into()],
            vec!["declaration".into(), "readonly".into()],
        );
        assert_eq!(legend.type_name(0), Some("namespace"));
        assert_eq!(legend.type_name(2), Some("function"));
        assert_eq!(legend.type_name(99), None);
    }

    #[test]
    fn legend_modifier_names() {
        let legend = SemanticTokenLegend::new(
            vec![],
            vec!["declaration".into(), "readonly".into(), "static".into()],
        );
        let names = legend.modifier_names(0b101);
        assert_eq!(names, vec!["declaration", "static"]);
    }

    #[test]
    fn merge_with_no_semantic() {
        let syntax = vec![HighlightEvent::Source { start: 0, end: 10 }];
        let legend = SemanticTokenLegend::default();
        let spans = merge_semantic_tokens(&syntax, &[], &legend, "0123456789");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[0].end, 10);
        assert!(spans[0].token_type.is_none());
    }

    #[test]
    fn merge_with_semantic_override() {
        let syntax = vec![HighlightEvent::Source { start: 0, end: 10 }];
        let legend = SemanticTokenLegend::new(vec!["variable".into()], vec![]);
        let semantic = vec![SemanticToken {
            line: 0,
            start: 0,
            length: 5,
            token_type: 0,
            modifiers: 0,
        }];
        let spans = merge_semantic_tokens(&syntax, &semantic, &legend, "hello world");
        let typed: Vec<_> = spans.iter().filter(|s| s.token_type.is_some()).collect();
        assert!(!typed.is_empty());
        assert_eq!(typed[0].token_type.as_deref(), Some("variable"));
    }

    #[test]
    fn apply_edit() {
        let mut data = vec![0, 5, 3, 1, 0, 0, 10, 4, 2, 0];
        let edit = SemanticTokenEdit {
            start: 5,
            delete_count: 5,
            data: vec![1, 3, 2, 3, 0],
        };
        apply_semantic_token_edits(&mut data, &[edit]);
        assert_eq!(data.len(), 10);
        assert_eq!(data[5..10], [1, 3, 2, 3, 0]);
    }

    #[test]
    fn styled_span_fields() {
        let span = StyledSpan {
            start: 0,
            end: 5,
            token_type: Some("function".into()),
            modifiers: vec!["declaration".into()],
        };
        assert_eq!(span.token_type.as_deref(), Some("function"));
        assert_eq!(span.modifiers, vec!["declaration"]);
    }

    #[test]
    fn line_col_to_byte_basic() {
        let source = "hello\nworld\nfoo";
        assert_eq!(line_col_to_byte(source, 0, 0), Some(0));
        assert_eq!(line_col_to_byte(source, 0, 3), Some(3));
        assert_eq!(line_col_to_byte(source, 1, 0), Some(6));
        assert_eq!(line_col_to_byte(source, 2, 0), Some(12));
    }

    #[test]
    fn standard_legend_has_all_types() {
        let legend = standard_semantic_token_legend();
        assert_eq!(legend.token_types.len(), STANDARD_TOKEN_TYPES.len());
        assert_eq!(legend.token_modifiers.len(), STANDARD_TOKEN_MODIFIERS.len());
    }

    #[test]
    fn manager_set_and_get() {
        let mut mgr = SemanticTokensManager::new();
        let path = std::path::PathBuf::from("test.rs");
        let tok = SemanticToken {
            line: 0,
            start: 0,
            length: 5,
            token_type: 0,
            modifiers: 0,
        };
        mgr.set_tokens(&path, vec![tok]);
        assert_eq!(mgr.file_count(), 1);
        assert!(mgr.get_tokens(&path).is_some());
        assert_eq!(mgr.get_tokens(&path).unwrap().len(), 1);
    }

    #[test]
    fn manager_apply_delta() {
        let mut mgr = SemanticTokensManager::new();
        let path = std::path::PathBuf::from("test.rs");
        mgr.set_tokens_from_data(&path, &[0, 5, 3, 1, 0, 0, 10, 4, 2, 0]);
        assert_eq!(mgr.get_tokens(&path).unwrap().len(), 2);

        let delta = SemanticTokensDelta {
            result_id: None,
            edits: vec![SemanticTokenEdit {
                start: 5,
                delete_count: 5,
                data: vec![1, 3, 2, 3, 0],
            }],
        };
        mgr.apply_delta(&path, &delta);
        assert_eq!(mgr.get_tokens(&path).unwrap().len(), 2);
    }

    #[test]
    fn manager_remove_and_clear() {
        let mut mgr = SemanticTokensManager::new();
        let p1 = std::path::PathBuf::from("a.rs");
        let p2 = std::path::PathBuf::from("b.rs");
        mgr.set_tokens(&p1, vec![]);
        mgr.set_tokens(&p2, vec![]);
        assert_eq!(mgr.file_count(), 2);
        mgr.remove(&p1);
        assert_eq!(mgr.file_count(), 1);
        mgr.clear();
        assert_eq!(mgr.file_count(), 0);
    }

    #[test]
    fn semantic_type_mapping() {
        assert_eq!(
            semantic_type_to_scope("function"),
            Some(TokenScope::Function)
        );
        assert_eq!(
            semantic_type_to_scope("variable"),
            Some(TokenScope::Variable)
        );
        assert_eq!(semantic_type_to_scope("unknown_type"), None);
    }

    #[test]
    fn merge_highlights_no_semantic() {
        let syntax = vec![
            HighlightToken::new(0, 5, TokenScope::Keyword),
            HighlightToken::new(6, 4, TokenScope::Variable),
        ];
        let result = merge_highlights(&syntax, &[], &standard_semantic_token_legend(), 0);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].scope, TokenScope::Keyword);
    }

    #[test]
    fn merge_highlights_semantic_override() {
        let syntax = vec![HighlightToken::new(0, 10, TokenScope::Variable)];
        let legend = standard_semantic_token_legend();
        let func_idx = legend
            .token_types
            .iter()
            .position(|t| t == "function")
            .unwrap() as u32;
        let semantic = vec![SemanticToken {
            line: 0,
            start: 0,
            length: 5,
            token_type: func_idx,
            modifiers: 0,
        }];
        let result = merge_highlights(&syntax, &semantic, &legend, 0);
        assert!(result.len() >= 2);
        assert_eq!(result[0].scope, TokenScope::Function);
        assert_eq!(result[0].length, 5);
        assert_eq!(result[1].scope, TokenScope::Variable);
    }

    #[test]
    fn apply_semantic_token_delta_test() {
        let mut tokens = decode_semantic_tokens(&[0, 5, 3, 1, 0, 0, 10, 4, 2, 0]);
        assert_eq!(tokens.len(), 2);
        let delta = SemanticTokensDelta {
            result_id: Some("v2".into()),
            edits: vec![SemanticTokenEdit {
                start: 5,
                delete_count: 5,
                data: vec![1, 3, 2, 3, 0],
            }],
        };
        apply_semantic_token_delta(&mut tokens, &delta);
        assert_eq!(tokens.len(), 2);
    }
}
