//! # sidex-syntax
//!
//! Syntax highlighting and parsing for the `SideX` editor, powered by
//! [tree-sitter](https://tree-sitter.github.io/) with `TextMate` grammar
//! fallback and LSP semantic token support.
//!
//! This crate provides:
//!
//! - **Highlighting** — run tree-sitter queries to produce a stream of
//!   [`HighlightEvent`]s for rendering.
//! - **Incremental parsing** — maintain a parse tree per document and cheaply
//!   re-parse after edits.
//! - **Language registry** — map file extensions to tree-sitter grammars.
//! - **Scope mapping** — resolve tree-sitter capture names to semantic
//!   highlight categories.
//! - **Bracket matching** — AST-aware matching of bracket pairs.
//! - **Code folding** — derive foldable regions from the parse tree.
//! - **`TextMate` grammars** — regex-based fallback tokenizer for languages
//!   without tree-sitter support.
//! - **Semantic tokens** — merge LSP semantic token overlays with syntax
//!   highlighting.
//! - **Auto-indentation** — rule-based indent/outdent computation.

pub mod bracket;
pub mod folding;
pub mod highlight;
pub mod indent;
pub mod language;
pub mod parser;
pub mod scope;
pub mod scope_resolver;
pub mod semantic_tokens;
pub mod textmate;
pub mod tree_sitter_parser;

pub use bracket::find_matching_bracket;
pub use folding::{compute_folding_ranges, FoldingKind, FoldingRange};
pub use highlight::{
    Highlight, HighlightConfig, HighlightError, HighlightEvent, HighlightToken, HighlightedLine,
    Highlighter, SyntaxHighlighter, TokenModifiers, TokenScope,
};
pub use indent::{compute_indent, default_indent_rules, IndentAction, IndentRule};
pub use language::{
    builtin_language_configs, builtin_language_configurations, AutoClosingPair, CommentConfig,
    EnterAction, FoldingConfig, FoldingMarkers, IndentRules, Language, LanguageConfig,
    LanguageConfiguration, LanguageRegistry, OnEnterRuleConfig,
};
pub use parser::{to_input_edit, DocumentParser};
pub use scope::{resolve_highlight_name, HighlightName};
pub use scope_resolver::{resolve_scope, FontStyle, TextStyle, TokenColorRule};
pub use semantic_tokens::{
    apply_semantic_token_delta, decode_semantic_tokens, encode_semantic_tokens,
    merge_highlights, merge_semantic_tokens, semantic_type_to_scope,
    standard_semantic_token_legend, SemanticToken, SemanticTokenEdit, SemanticTokenLegend,
    SemanticTokensDelta, SemanticTokensManager, StyledSpan, STANDARD_TOKEN_MODIFIERS,
    STANDARD_TOKEN_TYPES,
};
pub use textmate::{
    RuleStack, TextMateGrammar, TextMateTokenizer, Token, TokenInfo, TokenizeResult,
    TokenizerState,
};
pub use tree_sitter_parser::{
    get_fold_ranges, get_indent_hints, get_injections, get_local_bindings, InjectionRange,
    LocalBinding, TreeSitterError, TreeSitterManager, TreeSitterParserState, TreeSitterQueries,
};
