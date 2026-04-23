use serde::Serialize;
use sidex_syntax::highlight::{HighlightConfig, Highlighter, SyntaxHighlighter, TokenScope};
use sidex_syntax::language::{builtin_language_configurations, LanguageConfiguration};
use std::path::Path;
use std::sync::OnceLock;

static LANGUAGE_CONFIGS: OnceLock<Vec<LanguageConfiguration>> = OnceLock::new();

fn configs() -> &'static [LanguageConfiguration] {
    LANGUAGE_CONFIGS.get_or_init(builtin_language_configurations)
}

#[derive(Debug, Serialize)]
pub struct LanguageInfo {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
    pub filenames: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AutoClosePair {
    pub open: String,
    pub close: String,
    pub not_in: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LanguageConfigResponse {
    pub line_comment: Option<String>,
    pub block_comment: Option<(String, String)>,
    pub brackets: Vec<(String, String)>,
    pub auto_closing_pairs: Vec<AutoClosePair>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntaxToken {
    pub line: u32,
    pub start: u32,
    pub length: u32,
    pub scope: TokenScope,
}

#[allow(clippy::unnecessary_wraps)]
#[tauri::command]
pub fn syntax_get_languages() -> Result<Vec<LanguageInfo>, String> {
    let langs = configs()
        .iter()
        .map(|cfg| LanguageInfo {
            id: cfg.id.clone(),
            name: cfg.name.clone(),
            extensions: cfg.extensions.clone(),
            filenames: cfg.filenames.clone(),
        })
        .collect();
    Ok(langs)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn syntax_detect_language(filename: String) -> Result<String, String> {
    detect_by_path(&filename).ok_or_else(|| format!("No language detected for '{filename}'"))
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
#[allow(clippy::unnecessary_wraps)]
pub fn syntax_detect_from_content(
    filename: Option<String>,
    content: Option<String>,
    supported: Option<Vec<String>>,
) -> Result<Option<String>, String> {
    let candidate = filename
        .as_deref()
        .and_then(detect_by_path)
        .or_else(|| content.as_deref().and_then(detect_by_first_line));

    Ok(candidate.filter(|lang| {
        supported
            .as_ref()
            .is_none_or(|allowed| allowed.iter().any(|l| l == lang))
    }))
}

fn detect_by_path(filename: &str) -> Option<String> {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"));
    let fname = Path::new(filename)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");

    for cfg in configs() {
        if let Some(ref ext) = ext {
            if cfg.extensions.iter().any(|e| e == ext) {
                return Some(cfg.id.clone());
            }
        }
        if cfg.filenames.iter().any(|f| f == fname) {
            return Some(cfg.id.clone());
        }
    }
    None
}

fn detect_by_first_line(content: &str) -> Option<String> {
    let first = content.lines().next()?.trim_end();
    if first.is_empty() {
        return None;
    }
    for cfg in configs() {
        if let Some(pattern) = cfg.first_line_pattern.as_deref() {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(first) {
                    return Some(cfg.id.clone());
                }
            }
        }
    }
    None
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn syntax_get_language_config(language_id: String) -> Result<LanguageConfigResponse, String> {
    let cfg = configs()
        .iter()
        .find(|c| c.id == language_id)
        .ok_or_else(|| format!("Unknown language ID '{language_id}'"))?;

    Ok(LanguageConfigResponse {
        line_comment: cfg.comments.line_comment.clone(),
        block_comment: cfg.comments.block_comment.clone(),
        brackets: cfg.brackets.clone(),
        auto_closing_pairs: cfg
            .auto_closing_pairs
            .iter()
            .map(|p| AutoClosePair {
                open: p.open.clone(),
                close: p.close.clone(),
                not_in: p.not_in.clone(),
            })
            .collect(),
    })
}

/// Minimal bundled tree-sitter highlight query for Rust.
const RUST_HIGHLIGHT_QUERY: &str = r#"
(line_comment) @comment
(block_comment) @comment
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string
(integer_literal) @number
(float_literal) @number
(boolean_literal) @constant.builtin
"self" @variable.builtin
(primitive_type) @type.builtin
(type_identifier) @type
(field_identifier) @property
(function_item name: (identifier) @function)
(call_expression function: (identifier) @function.call)
(macro_invocation macro: (identifier) @function.macro)
(attribute_item) @attribute
[
  "as" "async" "await" "break" "const" "continue" "crate" "dyn" "else" "enum"
  "extern" "fn" "for" "if" "impl" "in" "let" "loop" "match" "mod" "move" "mut"
  "pub" "ref" "return" "static" "struct" "trait" "type" "unsafe" "use" "where" "while"
] @keyword
"#;

fn rust_highlight_config() -> Option<&'static HighlightConfig> {
    static CONFIG: OnceLock<Option<HighlightConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let language: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
            HighlightConfig::new(language, RUST_HIGHLIGHT_QUERY).ok()
        })
        .as_ref()
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn syntax_tokenize(language: String, source: String) -> Result<Vec<SyntaxToken>, String> {
    let config = match language.as_str() {
        "rust" => rust_highlight_config().ok_or("rust highlight config unavailable")?,
        _ => return Ok(Vec::new()),
    };

    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(config, &source, None)
        .map_err(|e| e.to_string())?;

    let capture_names: Vec<String> = config.capture_names().to_vec();
    let rendered = SyntaxHighlighter::from_events(&language, &events, &source, &capture_names);

    let mut tokens: Vec<SyntaxToken> = Vec::new();
    for line in &rendered.tokens {
        for token in &line.tokens {
            tokens.push(SyntaxToken {
                line: line.line,
                start: token.start,
                length: token.length,
                scope: token.scope,
            });
        }
    }
    Ok(tokens)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMateLineTokens {
    pub line: u32,
    pub tokens: Vec<TextMateToken>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMateToken {
    pub start: u32,
    pub end: u32,
    pub scopes: Vec<String>,
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn textmate_tokenize_lines(
    grammar_json: String,
    source: String,
) -> Result<Vec<TextMateLineTokens>, String> {
    let grammar = sidex_syntax::textmate::TextMateGrammar::from_json(&grammar_json)
        .map_err(|e| format!("parse grammar: {e}"))?;
    let tokenizer = sidex_syntax::textmate::TextMateTokenizer::new(&grammar);
    let mut state = sidex_syntax::textmate::TokenizerState::default();

    let mut out = Vec::with_capacity(source.lines().count());
    for (idx, line) in source.lines().enumerate() {
        let tokens = tokenizer
            .tokenize_line(line, &mut state)
            .into_iter()
            .map(|t| TextMateToken {
                start: u32::try_from(t.start).unwrap_or(u32::MAX),
                end: u32::try_from(t.end).unwrap_or(u32::MAX),
                scopes: t.scopes,
            })
            .collect::<Vec<_>>();
        out.push(TextMateLineTokens {
            line: u32::try_from(idx).unwrap_or(u32::MAX),
            tokens,
        });
    }
    Ok(out)
}
