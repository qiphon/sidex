//! Syntax highlighting engine powered by tree-sitter queries.
//!
//! The [`Highlighter`] runs compiled tree-sitter highlight queries against a
//! source string, producing a stream of [`HighlightEvent`]s that downstream
//! renderers consume to colorize text.
//!
//! In addition, this module provides a rich [`TokenScope`] enum and
//! [`TokenModifiers`] bitflags for fine-grained token classification, and a
//! line-oriented [`SyntaxHighlighter`] that produces [`HighlightedLine`]s for
//! efficient rendering.

use std::ops::Range as StdRange;

use serde::{Deserialize, Serialize};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor};

use crate::scope::resolve_highlight_name;

/// An opaque highlight index into the capture-name list of a [`HighlightConfig`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Highlight(pub u32);

/// Events emitted during highlighting, consumed by the renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HighlightEvent {
    /// A span of un-highlighted source text from byte `start` to byte `end`.
    Source { start: usize, end: usize },
    /// Begin a highlighted region with the given capture index.
    HighlightStart(Highlight),
    /// End the most recently started highlighted region.
    HighlightEnd,
}

// ---------------------------------------------------------------------------
// Rich token classification types
// ---------------------------------------------------------------------------

/// Semantic scope of a highlighted token, providing fine-grained classification
/// that downstream renderers use to select colors and styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenScope {
    Comment,
    CommentDoc,
    CommentBlock,
    String,
    StringEscape,
    StringRegex,
    StringTemplate,
    Number,
    NumberFloat,
    NumberHex,
    Boolean,
    Keyword,
    KeywordControl,
    KeywordOperator,
    KeywordImport,
    KeywordReturn,
    KeywordType,
    KeywordModifier,
    Operator,
    OperatorAssignment,
    OperatorComparison,
    OperatorArithmetic,
    OperatorLogical,
    Punctuation,
    PunctuationBracket,
    PunctuationDelimiter,
    PunctuationAccessor,
    Function,
    FunctionCall,
    FunctionDefinition,
    FunctionBuiltin,
    Method,
    MethodCall,
    MethodDefinition,
    Variable,
    VariableParameter,
    VariableProperty,
    VariableBuiltin,
    VariableReadonly,
    Class,
    ClassInherited,
    Interface,
    Struct,
    Enum,
    EnumMember,
    Type,
    TypePrimitive,
    TypeBuiltin,
    TypeParameter,
    Namespace,
    Module,
    Property,
    Attribute,
    Decorator,
    Label,
    Tag,
    TagName,
    TagAttribute,
    Macro,
    MacroCall,
    Lifetime,
    SelfKeyword,
    Constant,
    Invalid,
    Deprecated,
    Embedded,
}

impl TokenScope {
    /// All variants in definition order.
    pub const ALL: &[Self] = &[
        Self::Comment,
        Self::CommentDoc,
        Self::CommentBlock,
        Self::String,
        Self::StringEscape,
        Self::StringRegex,
        Self::StringTemplate,
        Self::Number,
        Self::NumberFloat,
        Self::NumberHex,
        Self::Boolean,
        Self::Keyword,
        Self::KeywordControl,
        Self::KeywordOperator,
        Self::KeywordImport,
        Self::KeywordReturn,
        Self::KeywordType,
        Self::KeywordModifier,
        Self::Operator,
        Self::OperatorAssignment,
        Self::OperatorComparison,
        Self::OperatorArithmetic,
        Self::OperatorLogical,
        Self::Punctuation,
        Self::PunctuationBracket,
        Self::PunctuationDelimiter,
        Self::PunctuationAccessor,
        Self::Function,
        Self::FunctionCall,
        Self::FunctionDefinition,
        Self::FunctionBuiltin,
        Self::Method,
        Self::MethodCall,
        Self::MethodDefinition,
        Self::Variable,
        Self::VariableParameter,
        Self::VariableProperty,
        Self::VariableBuiltin,
        Self::VariableReadonly,
        Self::Class,
        Self::ClassInherited,
        Self::Interface,
        Self::Struct,
        Self::Enum,
        Self::EnumMember,
        Self::Type,
        Self::TypePrimitive,
        Self::TypeBuiltin,
        Self::TypeParameter,
        Self::Namespace,
        Self::Module,
        Self::Property,
        Self::Attribute,
        Self::Decorator,
        Self::Label,
        Self::Tag,
        Self::TagName,
        Self::TagAttribute,
        Self::Macro,
        Self::MacroCall,
        Self::Lifetime,
        Self::SelfKeyword,
        Self::Constant,
        Self::Invalid,
        Self::Deprecated,
        Self::Embedded,
    ];

    /// Returns the canonical TextMate-style scope string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::CommentDoc => "comment.doc",
            Self::CommentBlock => "comment.block",
            Self::String => "string",
            Self::StringEscape => "string.escape",
            Self::StringRegex => "string.regex",
            Self::StringTemplate => "string.template",
            Self::Number => "number",
            Self::NumberFloat => "number.float",
            Self::NumberHex => "number.hex",
            Self::Boolean => "boolean",
            Self::Keyword => "keyword",
            Self::KeywordControl => "keyword.control",
            Self::KeywordOperator => "keyword.operator",
            Self::KeywordImport => "keyword.import",
            Self::KeywordReturn => "keyword.return",
            Self::KeywordType => "keyword.type",
            Self::KeywordModifier => "keyword.modifier",
            Self::Operator => "operator",
            Self::OperatorAssignment => "operator.assignment",
            Self::OperatorComparison => "operator.comparison",
            Self::OperatorArithmetic => "operator.arithmetic",
            Self::OperatorLogical => "operator.logical",
            Self::Punctuation => "punctuation",
            Self::PunctuationBracket => "punctuation.bracket",
            Self::PunctuationDelimiter => "punctuation.delimiter",
            Self::PunctuationAccessor => "punctuation.accessor",
            Self::Function => "function",
            Self::FunctionCall => "function.call",
            Self::FunctionDefinition => "function.definition",
            Self::FunctionBuiltin => "function.builtin",
            Self::Method => "method",
            Self::MethodCall => "method.call",
            Self::MethodDefinition => "method.definition",
            Self::Variable => "variable",
            Self::VariableParameter => "variable.parameter",
            Self::VariableProperty => "variable.property",
            Self::VariableBuiltin => "variable.builtin",
            Self::VariableReadonly => "variable.readonly",
            Self::Class => "class",
            Self::ClassInherited => "class.inherited",
            Self::Interface => "interface",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::EnumMember => "enum.member",
            Self::Type => "type",
            Self::TypePrimitive => "type.primitive",
            Self::TypeBuiltin => "type.builtin",
            Self::TypeParameter => "type.parameter",
            Self::Namespace => "namespace",
            Self::Module => "module",
            Self::Property => "property",
            Self::Attribute => "attribute",
            Self::Decorator => "decorator",
            Self::Label => "label",
            Self::Tag => "tag",
            Self::TagName => "tag.name",
            Self::TagAttribute => "tag.attribute",
            Self::Macro => "macro",
            Self::MacroCall => "macro.call",
            Self::Lifetime => "lifetime",
            Self::SelfKeyword => "self",
            Self::Constant => "constant",
            Self::Invalid => "invalid",
            Self::Deprecated => "deprecated",
            Self::Embedded => "embedded",
        }
    }

    /// Resolve a dotted scope string (e.g. `"keyword.control"`) to a `TokenScope`.
    #[must_use]
    pub fn from_scope_str(s: &str) -> Option<Self> {
        match s {
            "comment" => Some(Self::Comment),
            "comment.doc" | "comment.documentation" => Some(Self::CommentDoc),
            "comment.block" => Some(Self::CommentBlock),
            "string" => Some(Self::String),
            "string.escape" | "constant.character.escape" => Some(Self::StringEscape),
            "string.regex" | "string.regexp" => Some(Self::StringRegex),
            "string.template" | "string.interpolated" => Some(Self::StringTemplate),
            "number" | "constant.numeric" => Some(Self::Number),
            "number.float" | "constant.numeric.float" => Some(Self::NumberFloat),
            "number.hex" | "constant.numeric.hex" => Some(Self::NumberHex),
            "boolean" | "constant.language.boolean" => Some(Self::Boolean),
            "keyword" => Some(Self::Keyword),
            "keyword.control" | "keyword.control.flow" => Some(Self::KeywordControl),
            "keyword.operator" => Some(Self::KeywordOperator),
            "keyword.import" | "keyword.control.import" => Some(Self::KeywordImport),
            "keyword.return" | "keyword.control.return" => Some(Self::KeywordReturn),
            "keyword.type" => Some(Self::KeywordType),
            "keyword.modifier" | "keyword.storage.modifier" => Some(Self::KeywordModifier),
            "operator" => Some(Self::Operator),
            "operator.assignment" => Some(Self::OperatorAssignment),
            "operator.comparison" => Some(Self::OperatorComparison),
            "operator.arithmetic" => Some(Self::OperatorArithmetic),
            "operator.logical" => Some(Self::OperatorLogical),
            "punctuation" => Some(Self::Punctuation),
            "punctuation.bracket" => Some(Self::PunctuationBracket),
            "punctuation.delimiter" | "punctuation.separator" => Some(Self::PunctuationDelimiter),
            "punctuation.accessor" => Some(Self::PunctuationAccessor),
            "function" => Some(Self::Function),
            "function.call" => Some(Self::FunctionCall),
            "function.definition" => Some(Self::FunctionDefinition),
            "function.builtin" => Some(Self::FunctionBuiltin),
            "method" => Some(Self::Method),
            "method.call" => Some(Self::MethodCall),
            "method.definition" => Some(Self::MethodDefinition),
            "variable" => Some(Self::Variable),
            "variable.parameter" | "parameter" => Some(Self::VariableParameter),
            "variable.property" | "variable.member" => Some(Self::VariableProperty),
            "variable.builtin" => Some(Self::VariableBuiltin),
            "variable.readonly" => Some(Self::VariableReadonly),
            "class" => Some(Self::Class),
            "class.inherited" | "class.superclass" => Some(Self::ClassInherited),
            "interface" => Some(Self::Interface),
            "struct" => Some(Self::Struct),
            "enum" => Some(Self::Enum),
            "enum.member" | "enumMember" => Some(Self::EnumMember),
            "type" => Some(Self::Type),
            "type.primitive" | "type.builtin.primitive" => Some(Self::TypePrimitive),
            "type.builtin" => Some(Self::TypeBuiltin),
            "type.parameter" | "typeParameter" => Some(Self::TypeParameter),
            "namespace" => Some(Self::Namespace),
            "module" => Some(Self::Module),
            "property" | "field" => Some(Self::Property),
            "attribute" => Some(Self::Attribute),
            "decorator" => Some(Self::Decorator),
            "label" => Some(Self::Label),
            "tag" => Some(Self::Tag),
            "tag.name" => Some(Self::TagName),
            "tag.attribute" => Some(Self::TagAttribute),
            "macro" => Some(Self::Macro),
            "macro.call" => Some(Self::MacroCall),
            "lifetime" => Some(Self::Lifetime),
            "self" | "variable.self" => Some(Self::SelfKeyword),
            "constant" | "constant.language" => Some(Self::Constant),
            "invalid" => Some(Self::Invalid),
            "deprecated" => Some(Self::Deprecated),
            "embedded" => Some(Self::Embedded),
            other => Self::from_prefix(other),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn from_prefix(s: &str) -> Option<Self> {
        if s.starts_with("comment.doc") {
            return Some(Self::CommentDoc);
        }
        if s.starts_with("comment.block") {
            return Some(Self::CommentBlock);
        }
        if s.starts_with("comment") {
            return Some(Self::Comment);
        }
        if s.starts_with("string.escape") {
            return Some(Self::StringEscape);
        }
        if s.starts_with("string.regex") || s.starts_with("string.regexp") {
            return Some(Self::StringRegex);
        }
        if s.starts_with("string.template") || s.starts_with("string.interpolated") {
            return Some(Self::StringTemplate);
        }
        if s.starts_with("string") {
            return Some(Self::String);
        }
        if s.starts_with("number.float") || s.starts_with("constant.numeric.float") {
            return Some(Self::NumberFloat);
        }
        if s.starts_with("number.hex") || s.starts_with("constant.numeric.hex") {
            return Some(Self::NumberHex);
        }
        if s.starts_with("number") || s.starts_with("constant.numeric") {
            return Some(Self::Number);
        }
        if s.starts_with("keyword.control.import") || s.starts_with("keyword.import") {
            return Some(Self::KeywordImport);
        }
        if s.starts_with("keyword.control.return") || s.starts_with("keyword.return") {
            return Some(Self::KeywordReturn);
        }
        if s.starts_with("keyword.control") {
            return Some(Self::KeywordControl);
        }
        if s.starts_with("keyword.operator") {
            return Some(Self::KeywordOperator);
        }
        if s.starts_with("keyword.type") {
            return Some(Self::KeywordType);
        }
        if s.starts_with("keyword.modifier") || s.starts_with("storage.modifier") {
            return Some(Self::KeywordModifier);
        }
        if s.starts_with("keyword") {
            return Some(Self::Keyword);
        }
        if s.starts_with("operator") {
            return Some(Self::Operator);
        }
        if s.starts_with("punctuation.bracket") {
            return Some(Self::PunctuationBracket);
        }
        if s.starts_with("punctuation.delimiter") || s.starts_with("punctuation.separator") {
            return Some(Self::PunctuationDelimiter);
        }
        if s.starts_with("punctuation.accessor") {
            return Some(Self::PunctuationAccessor);
        }
        if s.starts_with("punctuation") {
            return Some(Self::Punctuation);
        }
        if s.starts_with("function.builtin") {
            return Some(Self::FunctionBuiltin);
        }
        if s.starts_with("function.definition") {
            return Some(Self::FunctionDefinition);
        }
        if s.starts_with("function.call") {
            return Some(Self::FunctionCall);
        }
        if s.starts_with("function") {
            return Some(Self::Function);
        }
        if s.starts_with("method.definition") {
            return Some(Self::MethodDefinition);
        }
        if s.starts_with("method.call") {
            return Some(Self::MethodCall);
        }
        if s.starts_with("method") {
            return Some(Self::Method);
        }
        if s.starts_with("variable.parameter") || s.starts_with("parameter") {
            return Some(Self::VariableParameter);
        }
        if s.starts_with("variable.property") || s.starts_with("variable.member") {
            return Some(Self::VariableProperty);
        }
        if s.starts_with("variable.builtin") {
            return Some(Self::VariableBuiltin);
        }
        if s.starts_with("variable.readonly") {
            return Some(Self::VariableReadonly);
        }
        if s.starts_with("variable") {
            return Some(Self::Variable);
        }
        if s.starts_with("type.parameter") || s.starts_with("typeParameter") {
            return Some(Self::TypeParameter);
        }
        if s.starts_with("type.primitive") {
            return Some(Self::TypePrimitive);
        }
        if s.starts_with("type.builtin") {
            return Some(Self::TypeBuiltin);
        }
        if s.starts_with("type") {
            return Some(Self::Type);
        }
        if s.starts_with("class") {
            return Some(Self::Class);
        }
        if s.starts_with("interface") {
            return Some(Self::Interface);
        }
        if s.starts_with("struct") {
            return Some(Self::Struct);
        }
        if s.starts_with("enum.member") || s.starts_with("enumMember") {
            return Some(Self::EnumMember);
        }
        if s.starts_with("enum") {
            return Some(Self::Enum);
        }
        if s.starts_with("namespace") {
            return Some(Self::Namespace);
        }
        if s.starts_with("module") {
            return Some(Self::Module);
        }
        if s.starts_with("property") || s.starts_with("field") {
            return Some(Self::Property);
        }
        if s.starts_with("attribute") {
            return Some(Self::Attribute);
        }
        if s.starts_with("decorator") {
            return Some(Self::Decorator);
        }
        if s.starts_with("tag.name") {
            return Some(Self::TagName);
        }
        if s.starts_with("tag.attribute") {
            return Some(Self::TagAttribute);
        }
        if s.starts_with("tag") {
            return Some(Self::Tag);
        }
        if s.starts_with("macro.call") {
            return Some(Self::MacroCall);
        }
        if s.starts_with("macro") {
            return Some(Self::Macro);
        }
        if s.starts_with("lifetime") {
            return Some(Self::Lifetime);
        }
        if s.starts_with("constant") || s.starts_with("boolean") {
            return Some(Self::Constant);
        }
        if s.starts_with("label") {
            return Some(Self::Label);
        }
        if s.starts_with("invalid") {
            return Some(Self::Invalid);
        }
        if s.starts_with("deprecated") {
            return Some(Self::Deprecated);
        }
        if s.starts_with("embedded") {
            return Some(Self::Embedded);
        }
        None
    }

    /// Returns the parent/base scope for hierarchical lookups.
    #[must_use]
    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::CommentDoc | Self::CommentBlock => Some(Self::Comment),
            Self::StringEscape | Self::StringRegex | Self::StringTemplate => Some(Self::String),
            Self::NumberFloat | Self::NumberHex => Some(Self::Number),
            Self::KeywordControl
            | Self::KeywordOperator
            | Self::KeywordImport
            | Self::KeywordReturn
            | Self::KeywordType
            | Self::KeywordModifier => Some(Self::Keyword),
            Self::OperatorAssignment
            | Self::OperatorComparison
            | Self::OperatorArithmetic
            | Self::OperatorLogical => Some(Self::Operator),
            Self::PunctuationBracket | Self::PunctuationDelimiter | Self::PunctuationAccessor => {
                Some(Self::Punctuation)
            }
            Self::FunctionCall | Self::FunctionDefinition | Self::FunctionBuiltin => {
                Some(Self::Function)
            }
            Self::MethodCall | Self::MethodDefinition => Some(Self::Method),
            Self::VariableParameter
            | Self::VariableProperty
            | Self::VariableBuiltin
            | Self::VariableReadonly => Some(Self::Variable),
            Self::ClassInherited => Some(Self::Class),
            Self::EnumMember => Some(Self::Enum),
            Self::TypePrimitive | Self::TypeBuiltin | Self::TypeParameter => Some(Self::Type),
            Self::TagName | Self::TagAttribute => Some(Self::Tag),
            Self::MacroCall => Some(Self::Macro),
            _ => None,
        }
    }
}

impl std::fmt::Display for TokenScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

bitflags::bitflags! {
    /// Modifier flags that can be combined with a [`TokenScope`] to convey
    /// additional semantic information about a token.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct TokenModifiers: u16 {
        const DECLARATION     = 0b0000_0000_0000_0001;
        const DEFINITION      = 0b0000_0000_0000_0010;
        const READONLY        = 0b0000_0000_0000_0100;
        const STATIC          = 0b0000_0000_0000_1000;
        const DEPRECATED      = 0b0000_0000_0001_0000;
        const ABSTRACT        = 0b0000_0000_0010_0000;
        const ASYNC           = 0b0000_0000_0100_0000;
        const MODIFICATION    = 0b0000_0000_1000_0000;
        const DOCUMENTATION   = 0b0000_0001_0000_0000;
        const DEFAULT_LIBRARY = 0b0000_0010_0000_0000;
    }
}

impl TokenModifiers {
    /// Standard modifier names in bit order, matching the LSP semantic token
    /// modifier legend.
    pub const NAMES: &[&str] = &[
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

    /// Build a `TokenModifiers` from a list of modifier name strings.
    #[must_use]
    pub fn from_names(names: &[&str]) -> Self {
        let mut flags = Self::empty();
        for name in names {
            match *name {
                "declaration" => flags |= Self::DECLARATION,
                "definition" => flags |= Self::DEFINITION,
                "readonly" => flags |= Self::READONLY,
                "static" => flags |= Self::STATIC,
                "deprecated" => flags |= Self::DEPRECATED,
                "abstract" => flags |= Self::ABSTRACT,
                "async" => flags |= Self::ASYNC,
                "modification" => flags |= Self::MODIFICATION,
                "documentation" => flags |= Self::DOCUMENTATION,
                "defaultLibrary" => flags |= Self::DEFAULT_LIBRARY,
                _ => {}
            }
        }
        flags
    }

    /// Returns the names of the set modifier bits.
    #[must_use]
    pub fn to_names(self) -> Vec<&'static str> {
        let mut names = Vec::new();
        for (i, &name) in Self::NAMES.iter().enumerate() {
            if self.bits() & (1 << i) != 0 {
                names.push(name);
            }
        }
        names
    }
}

/// A single highlighted token within a line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighlightToken {
    pub start: u32,
    pub length: u32,
    pub scope: TokenScope,
    pub modifiers: TokenModifiers,
}

impl HighlightToken {
    #[must_use]
    pub fn new(start: u32, length: u32, scope: TokenScope) -> Self {
        Self {
            start,
            length,
            scope,
            modifiers: TokenModifiers::empty(),
        }
    }

    #[must_use]
    pub fn with_modifiers(mut self, modifiers: TokenModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    #[must_use]
    pub fn end(&self) -> u32 {
        self.start + self.length
    }
}

/// Tokens for a single line of source code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighlightedLine {
    pub line: u32,
    pub tokens: Vec<HighlightToken>,
}

impl HighlightedLine {
    #[must_use]
    pub fn new(line: u32) -> Self {
        Self {
            line,
            tokens: Vec::new(),
        }
    }

    pub fn push(&mut self, token: HighlightToken) {
        self.tokens.push(token);
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }
}

/// Line-oriented syntax highlighter that produces [`HighlightedLine`]s with
/// rich [`TokenScope`] and [`TokenModifiers`].
#[derive(Debug)]
pub struct SyntaxHighlighter {
    pub language: String,
    pub tokens: Vec<HighlightedLine>,
}

impl SyntaxHighlighter {
    #[must_use]
    pub fn new(language: &str) -> Self {
        Self {
            language: language.to_owned(),
            tokens: Vec::new(),
        }
    }

    /// Converts a flat [`HighlightEvent`] stream into line-oriented
    /// [`HighlightedLine`]s using the given source text and capture-name
    /// list from a [`HighlightConfig`].
    pub fn from_events(
        language: &str,
        events: &[HighlightEvent],
        source: &str,
        capture_names: &[String],
    ) -> Self {
        let mut highlighter = Self::new(language);
        let mut current_scope: Option<TokenScope> = None;

        for event in events {
            match event {
                HighlightEvent::HighlightStart(Highlight(idx)) => {
                    if let Some(name) = capture_names.get(*idx as usize) {
                        current_scope = TokenScope::from_scope_str(name);
                    }
                }
                HighlightEvent::HighlightEnd => {
                    current_scope = None;
                }
                HighlightEvent::Source { start, end } => {
                    if let Some(scope) = current_scope {
                        let text = &source[*start..*end];
                        let start_pos = byte_offset_to_point(source, *start);
                        #[allow(clippy::cast_possible_truncation)]
                        let mut line = start_pos.row as u32;
                        #[allow(clippy::cast_possible_truncation)]
                        let mut col = start_pos.column as u32;

                        #[allow(clippy::explicit_counter_loop)]
                        for segment in text.split('\n') {
                            if !segment.is_empty() {
                                #[allow(clippy::cast_possible_truncation)]
                                let len = segment.len() as u32;
                                while highlighter.tokens.len() <= line as usize {
                                    #[allow(clippy::cast_possible_truncation)]
                                    let line_num = highlighter.tokens.len() as u32;
                                    highlighter.tokens.push(HighlightedLine::new(line_num));
                                }
                                highlighter.tokens[line as usize]
                                    .push(HighlightToken::new(col, len, scope));
                            }
                            line += 1;
                            col = 0;
                        }
                    }
                }
            }
        }
        highlighter
    }

    /// Returns the highlighted tokens for a specific line.
    #[must_use]
    pub fn line(&self, line: u32) -> Option<&HighlightedLine> {
        self.tokens.get(line as usize)
    }

    /// Total number of lines with highlight data.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.tokens.len()
    }

    /// Clears all stored highlight data.
    pub fn clear(&mut self) {
        self.tokens.clear();
    }
}

/// Compiled configuration for highlighting a single language.
///
/// Wraps a tree-sitter [`Query`] together with the list of capture names
/// so that highlight indices can be resolved to semantic categories.
pub struct HighlightConfig {
    pub(crate) query: Query,
    pub(crate) capture_names: Vec<String>,
    pub(crate) language: Language,
}

impl std::fmt::Debug for HighlightConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HighlightConfig")
            .field("capture_names", &self.capture_names)
            .finish_non_exhaustive()
    }
}

/// Errors that can occur when constructing a [`HighlightConfig`].
#[derive(Debug, thiserror::Error)]
pub enum HighlightError {
    /// The highlight query could not be compiled.
    #[error("invalid highlight query: {0}")]
    InvalidQuery(#[from] tree_sitter::QueryError),
    /// The parser failed to parse the source.
    #[error("tree-sitter parse failed")]
    ParseFailed,
}

impl HighlightConfig {
    /// Creates a new highlight configuration from a tree-sitter language and
    /// a `highlights.scm` query source string.
    pub fn new(language: Language, query_source: &str) -> Result<Self, HighlightError> {
        let query = Query::new(&language, query_source)?;
        let capture_names = query
            .capture_names()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        Ok(Self {
            query,
            capture_names,
            language,
        })
    }

    /// Returns the list of capture names defined in the query.
    #[must_use]
    pub fn capture_names(&self) -> &[String] {
        &self.capture_names
    }

    /// Resolves a [`Highlight`] index to its capture name string.
    #[must_use]
    pub fn capture_name(&self, highlight: Highlight) -> Option<&str> {
        self.capture_names
            .get(highlight.0 as usize)
            .map(String::as_str)
    }
}

/// Reusable syntax highlighter.
///
/// Holds a tree-sitter [`Parser`] and [`QueryCursor`] to avoid repeated
/// allocation across highlight calls.
pub struct Highlighter {
    parser: Parser,
    cursor: QueryCursor,
}

impl std::fmt::Debug for Highlighter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Highlighter").finish_non_exhaustive()
    }
}

impl Default for Highlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl Highlighter {
    /// Creates a new reusable highlighter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
            cursor: QueryCursor::new(),
        }
    }

    /// Highlights `source` using the given [`HighlightConfig`].
    ///
    /// If `byte_ranges` is provided, only matches overlapping those byte ranges
    /// are emitted (useful for highlighting only the visible viewport).
    pub fn highlight(
        &mut self,
        config: &HighlightConfig,
        source: &str,
        byte_ranges: Option<&[StdRange<usize>]>,
    ) -> Result<Vec<HighlightEvent>, HighlightError> {
        self.parser
            .set_language(&config.language)
            .expect("language version mismatch");

        let tree = self
            .parser
            .parse(source, None)
            .ok_or(HighlightError::ParseFailed)?;

        if let Some(ranges) = byte_ranges {
            let ts_ranges: Vec<tree_sitter::Range> = ranges
                .iter()
                .map(|r| tree_sitter::Range {
                    start_byte: r.start,
                    end_byte: r.end,
                    start_point: byte_offset_to_point(source, r.start),
                    end_point: byte_offset_to_point(source, r.end),
                })
                .collect();
            self.cursor.set_byte_range(0..source.len());
            // Pre-filter: only iterate matches in the given ranges.
            if let Some(first) = ts_ranges.first() {
                let last = ts_ranges.last().unwrap_or(first);
                self.cursor.set_byte_range(first.start_byte..last.end_byte);
            }
        } else {
            self.cursor.set_byte_range(0..source.len());
        }

        let root = tree.root_node();
        let events = Self::collect_events(&mut self.cursor, config, root, source, byte_ranges);
        Ok(events)
    }

    /// Walk query matches and convert them into a flat event stream.
    fn collect_events(
        cursor: &mut QueryCursor,
        config: &HighlightConfig,
        root: Node<'_>,
        source: &str,
        byte_ranges: Option<&[StdRange<usize>]>,
    ) -> Vec<HighlightEvent> {
        let mut events: Vec<HighlightEvent> = Vec::new();

        // Collect all captured spans sorted by start byte, breaking ties by
        // longer spans first (so nesting works correctly).
        let mut spans: Vec<(usize, usize, u32)> = Vec::new();
        let mut matches = cursor.matches(&config.query, root, source.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let node = capture.node;
                let start = node.start_byte();
                let end = node.end_byte();

                if let Some(ranges) = byte_ranges {
                    let overlaps = ranges.iter().any(|r| start < r.end && end > r.start);
                    if !overlaps {
                        continue;
                    }
                }

                #[allow(clippy::cast_possible_truncation)]
                spans.push((start, end, capture.index));
            }
        }

        spans.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        spans.dedup();

        let source_len = source.len();
        let mut pos = 0;

        for (start, end, capture_idx) in &spans {
            let start = *start;
            let end = *end;
            let capture_idx = *capture_idx;

            // Only emit captures that map to a known highlight name.
            let capture_name = &config.capture_names[capture_idx as usize];
            if resolve_highlight_name(capture_name).is_none() {
                continue;
            }

            if start > pos {
                events.push(HighlightEvent::Source {
                    start: pos,
                    end: start,
                });
            }

            events.push(HighlightEvent::HighlightStart(Highlight(capture_idx)));
            events.push(HighlightEvent::Source {
                start,
                end: end.min(source_len),
            });
            events.push(HighlightEvent::HighlightEnd);

            pos = end;
        }

        if pos < source_len {
            events.push(HighlightEvent::Source {
                start: pos,
                end: source_len,
            });
        }

        events
    }
}

/// Convert a byte offset into a tree-sitter `Point` (row/column).
fn byte_offset_to_point(source: &str, byte_offset: usize) -> tree_sitter::Point {
    let slice = &source[..byte_offset.min(source.len())];
    let row = slice.bytes().filter(|&b| b == b'\n').count();
    let last_newline = slice.rfind('\n').map_or(0, |i| i + 1);
    let column = byte_offset - last_newline;
    tree_sitter::Point { row, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rust_config_with_query(query_src: &str) -> HighlightConfig {
        let lang: Language = tree_sitter_rust::LANGUAGE.into();
        HighlightConfig::new(lang, query_src).expect("valid query")
    }

    #[test]
    fn highlight_config_capture_names() {
        let config = rust_config_with_query(
            r#"(line_comment) @comment
(string_literal) @string"#,
        );
        assert!(config.capture_names().contains(&"comment".to_string()));
        assert!(config.capture_names().contains(&"string".to_string()));
    }

    #[test]
    fn highlight_config_resolve_capture() {
        let config = rust_config_with_query("(line_comment) @comment");
        let name = config.capture_name(Highlight(0));
        assert_eq!(name, Some("comment"));
    }

    #[test]
    fn highlight_empty_source() {
        let config = rust_config_with_query("(line_comment) @comment");
        let mut hl = Highlighter::new();
        let events = hl.highlight(&config, "", None).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn highlight_produces_comment_events() {
        let config = rust_config_with_query("(line_comment) @comment");
        let source = "// hello\nlet x = 1;\n";
        let mut hl = Highlighter::new();
        let events = hl.highlight(&config, source, None).unwrap();

        let has_comment_start = events
            .iter()
            .any(|e| matches!(e, HighlightEvent::HighlightStart(Highlight(0))));
        assert!(has_comment_start, "expected a HighlightStart for comment");

        let has_end = events
            .iter()
            .any(|e| matches!(e, HighlightEvent::HighlightEnd));
        assert!(has_end, "expected a HighlightEnd");
    }

    #[test]
    fn highlight_with_byte_range() {
        let config = rust_config_with_query("(line_comment) @comment");
        let source = "let x = 1;\n// second\nlet y = 2;\n";
        let mut hl = Highlighter::new();
        let events = hl.highlight(&config, source, Some(&[11..21])).unwrap();

        let has_comment = events
            .iter()
            .any(|e| matches!(e, HighlightEvent::HighlightStart(Highlight(0))));
        assert!(
            has_comment,
            "comment in the visible range should be highlighted"
        );
    }

    #[test]
    fn byte_offset_to_point_basic() {
        let source = "abc\ndef\nghi";
        let p = byte_offset_to_point(source, 5);
        assert_eq!(p.row, 1);
        assert_eq!(p.column, 1);
    }

    #[test]
    fn token_scope_roundtrip() {
        for scope in TokenScope::ALL {
            let s = scope.as_str();
            let resolved = TokenScope::from_scope_str(s);
            assert_eq!(resolved, Some(*scope), "roundtrip failed for {s}");
        }
    }

    #[test]
    fn token_scope_prefix_resolution() {
        assert_eq!(
            TokenScope::from_scope_str("comment.line.double-slash"),
            Some(TokenScope::Comment)
        );
        assert_eq!(
            TokenScope::from_scope_str("keyword.control.flow"),
            Some(TokenScope::KeywordControl)
        );
    }

    #[test]
    fn token_scope_parent() {
        assert_eq!(TokenScope::CommentDoc.parent(), Some(TokenScope::Comment));
        assert_eq!(
            TokenScope::FunctionCall.parent(),
            Some(TokenScope::Function)
        );
        assert_eq!(TokenScope::Comment.parent(), None);
    }

    #[test]
    fn token_modifiers_from_names() {
        let mods = TokenModifiers::from_names(&["declaration", "readonly", "async"]);
        assert!(mods.contains(TokenModifiers::DECLARATION));
        assert!(mods.contains(TokenModifiers::READONLY));
        assert!(mods.contains(TokenModifiers::ASYNC));
        assert!(!mods.contains(TokenModifiers::STATIC));
    }

    #[test]
    fn token_modifiers_to_names() {
        let mods = TokenModifiers::DECLARATION | TokenModifiers::STATIC;
        let names = mods.to_names();
        assert!(names.contains(&"declaration"));
        assert!(names.contains(&"static"));
        assert!(!names.contains(&"readonly"));
    }

    #[test]
    fn highlight_token_end() {
        let tok = HighlightToken::new(5, 10, TokenScope::Keyword);
        assert_eq!(tok.end(), 15);
    }

    #[test]
    fn highlighted_line_push() {
        let mut line = HighlightedLine::new(0);
        assert!(line.is_empty());
        line.push(HighlightToken::new(0, 3, TokenScope::Keyword));
        assert!(!line.is_empty());
        assert_eq!(line.tokens.len(), 1);
    }

    #[test]
    fn syntax_highlighter_new() {
        let hl = SyntaxHighlighter::new("rust");
        assert_eq!(hl.language, "rust");
        assert_eq!(hl.line_count(), 0);
    }
}
