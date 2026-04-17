//! Context keys and a full "when" clause expression evaluator.
//!
//! Supports: `&&`, `||`, `!`, `==`, `!=`, `=~` (regex), `in`, `<`, `>`,
//! `<=`, `>=`, parentheses, boolean/string literals, and identifier lookup.
//!
//! Provides both a direct `evaluate()` function (parses + evaluates in one step)
//! and an AST-based workflow via `parse_when_clause()` + `WhenClause::evaluate()`.

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};

// ── Context value ────────────────────────────────────────────────────────────

/// A value stored in the context key map.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContextValue {
    Bool(bool),
    String(String),
    Number(f64),
    List(Vec<String>),
}

impl ContextValue {
    /// Coerce to bool: `Bool(b)` → `b`, non-empty `String` → `true`,
    /// non-zero `Number` → `true`, non-empty `List` → `true`.
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::String(s) => !s.is_empty(),
            Self::Number(n) => *n != 0.0,
            Self::List(l) => !l.is_empty(),
        }
    }

    fn to_string_repr(&self) -> String {
        match self {
            Self::Bool(b) => b.to_string(),
            Self::String(s) => s.clone(),
            Self::Number(n) => n.to_string(),
            Self::List(l) => format!("{l:?}"),
        }
    }

    fn contains(&self, needle: &str) -> bool {
        match self {
            Self::List(l) => l.iter().any(|s| s == needle),
            Self::String(s) => s.contains(needle),
            _ => false,
        }
    }
}

// ── Context keys store ───────────────────────────────────────────────────────

/// A key-value store of contextual state used to evaluate "when" clauses
/// on keybindings (e.g. `editorTextFocus && !editorReadonly`).
#[derive(Clone, Debug, Default)]
pub struct ContextKeys {
    map: HashMap<String, ContextValue>,
}

impl ContextKeys {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<String>, value: ContextValue) {
        self.map.insert(key.into(), value);
    }

    pub fn set_bool(&mut self, key: impl Into<String>, value: bool) {
        self.set(key, ContextValue::Bool(value));
    }

    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.set(key, ContextValue::String(value.into()));
    }

    pub fn set_number(&mut self, key: impl Into<String>, value: f64) {
        self.set(key, ContextValue::Number(value));
    }

    pub fn set_list(&mut self, key: impl Into<String>, value: Vec<String>) {
        self.set(key, ContextValue::List(value));
    }

    pub fn get(&self, key: &str) -> Option<&ContextValue> {
        self.map.get(key)
    }

    /// Get a context key as a boolean, defaulting to `false` if unset.
    pub fn is_true(&self, key: &str) -> bool {
        self.get(key).is_some_and(ContextValue::as_bool)
    }

    pub fn remove(&mut self, key: &str) {
        self.map.remove(key);
    }

    /// Get a numeric context value.
    pub fn get_number(&self, key: &str) -> Option<f64> {
        match self.get(key) {
            Some(ContextValue::Number(n)) => Some(*n),
            _ => None,
        }
    }

    /// Return all context key names currently set.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }
}

// ── WhenClause AST ──────────────────────────────────────────────────────────

/// A parsed "when" clause expression tree. Can be serialized, cached, and
/// evaluated repeatedly against different context snapshots.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WhenClause {
    True,
    False,
    /// Truthy test on a single context key.
    Key(String),
    Not(Box<WhenClause>),
    And(Vec<WhenClause>),
    Or(Vec<WhenClause>),
    Equals(String, String),
    NotEquals(String, String),
    Regex(String, String),
    /// `key in containerKey`
    In(String, String),
    Greater(String, f64),
    GreaterEquals(String, f64),
    Less(String, f64),
    LessEquals(String, f64),
}

impl WhenClause {
    /// Evaluate this clause against a context key store.
    pub fn evaluate(&self, ctx: &ContextKeyService) -> bool {
        match self {
            Self::True => true,
            Self::False => false,
            Self::Key(k) => ctx.keys.is_true(k),
            Self::Not(inner) => !inner.evaluate(ctx),
            Self::And(parts) => parts.iter().all(|p| p.evaluate(ctx)),
            Self::Or(parts) => parts.iter().any(|p| p.evaluate(ctx)),
            Self::Equals(k, v) => match ctx.keys.get(k) {
                Some(ContextValue::String(s)) => s == v,
                Some(ContextValue::Bool(b)) => {
                    (v == "true" && *b) || (v == "false" && !*b)
                }
                Some(ContextValue::Number(n)) => v
                    .parse::<f64>()
                    .is_ok_and(|r| (*n - r).abs() < f64::EPSILON),
                Some(ContextValue::List(_)) => false,
                None => v == "false" || v.is_empty(),
            },
            Self::NotEquals(k, v) => !Self::Equals(k.clone(), v.clone()).evaluate(ctx),
            Self::Regex(k, pattern) => {
                let hay = match ctx.keys.get(k) {
                    Some(v) => v.to_string_repr(),
                    None => return false,
                };
                regex::Regex::new(pattern).is_ok_and(|re| re.is_match(&hay))
            }
            Self::In(k, container) => {
                let needle = match ctx.keys.get(k) {
                    Some(v) => v.to_string_repr(),
                    None => k.to_owned(),
                };
                match ctx.keys.get(container) {
                    Some(c) => c.contains(&needle),
                    None => false,
                }
            }
            Self::Greater(k, threshold) => ctx
                .keys
                .get_number(k)
                .is_some_and(|n| n > *threshold),
            Self::GreaterEquals(k, threshold) => ctx
                .keys
                .get_number(k)
                .is_some_and(|n| n >= *threshold),
            Self::Less(k, threshold) => ctx
                .keys
                .get_number(k)
                .is_some_and(|n| n < *threshold),
            Self::LessEquals(k, threshold) => ctx
                .keys
                .get_number(k)
                .is_some_and(|n| n <= *threshold),
        }
    }
}

// ── ContextKeyService ───────────────────────────────────────────────────────

/// High-level service wrapping `ContextKeys` with convenience methods for
/// managing scoped context and evaluating when-clauses.
#[derive(Clone, Debug, Default)]
pub struct ContextKeyService {
    pub keys: ContextKeys,
}

impl ContextKeyService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a service pre-populated with platform keys.
    pub fn with_platform_defaults() -> Self {
        let mut svc = Self::new();
        svc.keys.set_bool(keys::IS_MAC, cfg!(target_os = "macos"));
        svc.keys
            .set_bool(keys::IS_LINUX, cfg!(target_os = "linux"));
        svc.keys
            .set_bool(keys::IS_WINDOWS, cfg!(target_os = "windows"));
        svc.keys.set_bool(keys::IS_WEB, false);
        svc
    }

    pub fn set_bool(&mut self, key: &str, val: bool) {
        self.keys.set_bool(key, val);
    }

    pub fn set_string(&mut self, key: &str, val: impl Into<String>) {
        self.keys.set_string(key, val);
    }

    pub fn set_number(&mut self, key: &str, val: f64) {
        self.keys.set_number(key, val);
    }

    pub fn evaluate_expr(&self, expr: &str) -> bool {
        evaluate(expr, &self.keys)
    }

    pub fn evaluate_clause(&self, clause: &WhenClause) -> bool {
        clause.evaluate(self)
    }
}

// ── parse_when_clause ───────────────────────────────────────────────────────

/// Parse a "when" clause string into a [`WhenClause`] AST that can be
/// evaluated repeatedly without re-parsing.
pub fn parse_when_clause(expr: &str) -> Result<WhenClause, WhenClauseError> {
    let expr = expr.trim();
    if expr.is_empty() {
        return Ok(WhenClause::True);
    }
    let tokens = tokenize(expr);
    let mut parser = AstParser::new(&tokens);
    let ast = parser.parse_or()?;
    Ok(ast)
}

/// Error from parsing a when-clause expression.
#[derive(Debug, thiserror::Error)]
pub enum WhenClauseError {
    #[error("unexpected token at position {0}")]
    UnexpectedToken(usize),
    #[error("unexpected end of expression")]
    UnexpectedEnd,
    #[error("invalid number: {0}")]
    InvalidNumber(String),
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Evaluate a "when" clause expression against a set of context keys.
///
/// Supports: `&&`, `||`, `!`, `==`, `!=`, `=~` (regex match), `in`,
/// parentheses for grouping, string/boolean literals, and dotted identifiers.
///
/// Examples:
/// - `"editorTextFocus"`
/// - `"editorTextFocus && !editorReadonly"`
/// - `"resourceScheme == 'file'"`
/// - `"resourceFilename =~ /\\.test\\.ts$/"`
/// - `"editorLangId in ['javascript', 'typescript']"`
pub fn evaluate(expression: &str, context: &ContextKeys) -> bool {
    let expr = expression.trim();
    if expr.is_empty() {
        return true;
    }
    let tokens = tokenize(expr);
    let mut parser = Parser::new(&tokens, context);
    parser.parse_or()
}

// ── Tokens ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    StringLit(String),
    NumberLit(f64),
    RegexLit(String),
    And,        // &&
    Or,         // ||
    Not,        // !
    Eq,         // ==
    Neq,        // !=
    RegexMatch, // =~
    In,         // in
    Lt,         // <
    Gt,         // >
    LtEq,      // <=
    GtEq,      // >=
    LParen,
    RParen,
    True,
    False,
}

#[allow(clippy::too_many_lines)]
fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '!' if i + 1 < len && chars[i + 1] == '=' => {
                tokens.push(Token::Neq);
                i += 2;
            }
            '!' => {
                tokens.push(Token::Not);
                i += 1;
            }
            '=' if i + 1 < len && chars[i + 1] == '~' => {
                tokens.push(Token::RegexMatch);
                i += 2;
            }
            '=' if i + 1 < len && chars[i + 1] == '=' => {
                tokens.push(Token::Eq);
                i += 2;
            }
            '<' if i + 1 < len && chars[i + 1] == '=' => {
                tokens.push(Token::LtEq);
                i += 2;
            }
            '<' => {
                tokens.push(Token::Lt);
                i += 1;
            }
            '>' if i + 1 < len && chars[i + 1] == '=' => {
                tokens.push(Token::GtEq);
                i += 2;
            }
            '>' => {
                tokens.push(Token::Gt);
                i += 1;
            }
            '&' if i + 1 < len && chars[i + 1] == '&' => {
                tokens.push(Token::And);
                i += 2;
            }
            '|' if i + 1 < len && chars[i + 1] == '|' => {
                tokens.push(Token::Or);
                i += 2;
            }
            '/' => {
                // Regex literal: /pattern/flags
                i += 1;
                let start = i;
                while i < len && chars[i] != '/' {
                    if chars[i] == '\\' && i + 1 < len {
                        i += 1; // skip escaped char
                    }
                    i += 1;
                }
                let pattern: String = chars[start..i].iter().collect();
                if i < len {
                    i += 1; // skip closing /
                }
                // Consume optional flags (i, g, m, etc.)
                let flag_start = i;
                while i < len && chars[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let flags: String = chars[flag_start..i].iter().collect();
                let full = if flags.is_empty() {
                    pattern
                } else {
                    format!("(?{flags}){pattern}")
                };
                tokens.push(Token::RegexLit(full));
            }
            '\'' | '"' => {
                let quote = chars[i];
                i += 1;
                let start = i;
                while i < len && chars[i] != quote {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token::StringLit(s));
                if i < len {
                    i += 1;
                }
            }
            _ => {
                let start = i;
                while i < len
                    && !matches!(
                        chars[i],
                        ' ' | '\t' | '(' | ')' | '!' | '=' | '&' | '|' | '\'' | '"' | '/'
                            | '<' | '>'
                    )
                {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                if !word.is_empty() {
                    match word.as_str() {
                        "true" => tokens.push(Token::True),
                        "false" => tokens.push(Token::False),
                        "in" => tokens.push(Token::In),
                        "not" => tokens.push(Token::Not),
                        _ => {
                            if let Ok(n) = word.parse::<f64>() {
                                tokens.push(Token::NumberLit(n));
                            } else {
                                tokens.push(Token::Ident(word));
                            }
                        }
                    }
                }
            }
        }
    }
    tokens
}

// ── Recursive-descent parser / evaluator ─────────────────────────────────────

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    ctx: &'a ContextKeys,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], ctx: &'a ContextKeys) -> Self {
        Self {
            tokens,
            pos: 0,
            ctx,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    // or_expr = and_expr ( '||' and_expr )*
    fn parse_or(&mut self) -> bool {
        let mut result = self.parse_and();
        while self.peek() == Some(&Token::Or) {
            self.advance();
            let rhs = self.parse_and();
            result = result || rhs;
        }
        result
    }

    // and_expr = unary_expr ( '&&' unary_expr )*
    fn parse_and(&mut self) -> bool {
        let mut result = self.parse_unary();
        while self.peek() == Some(&Token::And) {
            self.advance();
            let rhs = self.parse_unary();
            result = result && rhs;
        }
        result
    }

    // unary_expr = '!' unary_expr | primary_expr
    fn parse_unary(&mut self) -> bool {
        if self.peek() == Some(&Token::Not) {
            self.advance();
            return !self.parse_unary();
        }
        self.parse_primary()
    }

    // primary_expr = '(' or_expr ')'
    //              | 'true' | 'false'
    //              | string_lit
    //              | ident ( '==' value | '!=' value | '=~' regex | 'in' ident )?
    fn parse_primary(&mut self) -> bool {
        match self.peek().cloned() {
            Some(Token::LParen) => {
                self.advance();
                let result = self.parse_or();
                if self.peek() == Some(&Token::RParen) {
                    self.advance();
                }
                result
            }
            Some(Token::True) => {
                self.advance();
                true
            }
            Some(Token::StringLit(s)) => {
                self.advance();
                !s.is_empty()
            }
            Some(Token::Ident(ident)) => {
                self.advance();
                self.parse_comparison(&ident)
            }
            _ => {
                // Covers Token::False and any unexpected token
                self.advance();
                false
            }
        }
    }

    fn parse_comparison(&mut self, ident: &str) -> bool {
        match self.peek().cloned() {
            Some(Token::Eq) => {
                self.advance();
                let rhs = self.consume_value();
                self.eval_eq(ident, &rhs)
            }
            Some(Token::Neq) => {
                self.advance();
                let rhs = self.consume_value();
                !self.eval_eq(ident, &rhs)
            }
            Some(Token::RegexMatch) => {
                self.advance();
                let pattern = self.consume_regex();
                self.eval_regex(ident, &pattern)
            }
            Some(Token::In) => {
                self.advance();
                let container_key = self.consume_ident();
                self.eval_in(ident, &container_key)
            }
            Some(Token::Lt) => {
                self.advance();
                let rhs = self.consume_number();
                self.ctx.get_number(ident).is_some_and(|n| n < rhs)
            }
            Some(Token::LtEq) => {
                self.advance();
                let rhs = self.consume_number();
                self.ctx.get_number(ident).is_some_and(|n| n <= rhs)
            }
            Some(Token::Gt) => {
                self.advance();
                let rhs = self.consume_number();
                self.ctx.get_number(ident).is_some_and(|n| n > rhs)
            }
            Some(Token::GtEq) => {
                self.advance();
                let rhs = self.consume_number();
                self.ctx.get_number(ident).is_some_and(|n| n >= rhs)
            }
            _ => self.ctx.is_true(ident),
        }
    }

    fn consume_value(&mut self) -> String {
        match self.advance().cloned() {
            Some(Token::Ident(s) | Token::StringLit(s)) => s,
            Some(Token::NumberLit(n)) => n.to_string(),
            Some(Token::True) => "true".to_owned(),
            Some(Token::False) => "false".to_owned(),
            _ => String::new(),
        }
    }

    fn consume_regex(&mut self) -> String {
        match self.advance().cloned() {
            Some(Token::RegexLit(s) | Token::StringLit(s)) => s,
            _ => String::new(),
        }
    }

    fn consume_ident(&mut self) -> String {
        match self.advance().cloned() {
            Some(Token::Ident(s) | Token::StringLit(s)) => s,
            _ => String::new(),
        }
    }

    fn consume_number(&mut self) -> f64 {
        match self.advance().cloned() {
            Some(Token::NumberLit(n)) => n,
            Some(Token::Ident(s) | Token::StringLit(s)) => s.parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    fn eval_eq(&self, ident: &str, rhs: &str) -> bool {
        match self.ctx.get(ident) {
            Some(ContextValue::String(s)) => *s == rhs,
            Some(ContextValue::Bool(b)) => (rhs == "true" && *b) || (rhs == "false" && !*b),
            Some(ContextValue::Number(n)) => rhs
                .parse::<f64>()
                .is_ok_and(|r| (*n - r).abs() < f64::EPSILON),
            Some(ContextValue::List(_)) => false,
            None => rhs == "false" || rhs.is_empty(),
        }
    }

    fn eval_regex(&self, ident: &str, pattern: &str) -> bool {
        let hay = match self.ctx.get(ident) {
            Some(v) => v.to_string_repr(),
            None => return false,
        };
        Regex::new(pattern).is_ok_and(|re| re.is_match(&hay))
    }

    /// `ident in containerKey` — checks if the value of `ident` is contained
    /// in the value of `containerKey` (list or string).
    fn eval_in(&self, ident: &str, container_key: &str) -> bool {
        let needle = match self.ctx.get(ident) {
            Some(v) => v.to_string_repr(),
            None => ident.to_owned(),
        };
        match self.ctx.get(container_key) {
            Some(container) => container.contains(&needle),
            None => false,
        }
    }
}

// ── AST-building parser (for parse_when_clause) ─────────────────────────────

struct AstParser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> AstParser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn parse_or(&mut self) -> Result<WhenClause, WhenClauseError> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = match left {
                WhenClause::Or(mut parts) => {
                    parts.push(right);
                    WhenClause::Or(parts)
                }
                _ => WhenClause::Or(vec![left, right]),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<WhenClause, WhenClauseError> {
        let mut left = self.parse_unary()?;
        while self.peek() == Some(&Token::And) {
            self.advance();
            let right = self.parse_unary()?;
            left = match left {
                WhenClause::And(mut parts) => {
                    parts.push(right);
                    WhenClause::And(parts)
                }
                _ => WhenClause::And(vec![left, right]),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<WhenClause, WhenClauseError> {
        if self.peek() == Some(&Token::Not) {
            self.advance();
            let inner = self.parse_unary()?;
            return Ok(WhenClause::Not(Box::new(inner)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<WhenClause, WhenClauseError> {
        match self.peek().cloned() {
            Some(Token::LParen) => {
                self.advance();
                let inner = self.parse_or()?;
                if self.peek() == Some(&Token::RParen) {
                    self.advance();
                }
                Ok(inner)
            }
            Some(Token::True) => {
                self.advance();
                Ok(WhenClause::True)
            }
            Some(Token::False) => {
                self.advance();
                Ok(WhenClause::False)
            }
            Some(Token::Ident(ident)) => {
                self.advance();
                self.parse_ast_comparison(&ident)
            }
            Some(_) => {
                let pos = self.pos;
                self.advance();
                Err(WhenClauseError::UnexpectedToken(pos))
            }
            None => Err(WhenClauseError::UnexpectedEnd),
        }
    }

    fn parse_ast_comparison(&mut self, ident: &str) -> Result<WhenClause, WhenClauseError> {
        match self.peek().cloned() {
            Some(Token::Eq) => {
                self.advance();
                let rhs = self.consume_ast_value();
                Ok(WhenClause::Equals(ident.to_owned(), rhs))
            }
            Some(Token::Neq) => {
                self.advance();
                let rhs = self.consume_ast_value();
                Ok(WhenClause::NotEquals(ident.to_owned(), rhs))
            }
            Some(Token::RegexMatch) => {
                self.advance();
                let pattern = self.consume_ast_regex();
                Ok(WhenClause::Regex(ident.to_owned(), pattern))
            }
            Some(Token::In) => {
                self.advance();
                let container = self.consume_ast_value();
                Ok(WhenClause::In(ident.to_owned(), container))
            }
            Some(Token::Lt) => {
                self.advance();
                let n = self.consume_ast_number()?;
                Ok(WhenClause::Less(ident.to_owned(), n))
            }
            Some(Token::LtEq) => {
                self.advance();
                let n = self.consume_ast_number()?;
                Ok(WhenClause::LessEquals(ident.to_owned(), n))
            }
            Some(Token::Gt) => {
                self.advance();
                let n = self.consume_ast_number()?;
                Ok(WhenClause::Greater(ident.to_owned(), n))
            }
            Some(Token::GtEq) => {
                self.advance();
                let n = self.consume_ast_number()?;
                Ok(WhenClause::GreaterEquals(ident.to_owned(), n))
            }
            _ => Ok(WhenClause::Key(ident.to_owned())),
        }
    }

    fn consume_ast_value(&mut self) -> String {
        match self.advance().cloned() {
            Some(Token::Ident(s) | Token::StringLit(s)) => s,
            Some(Token::NumberLit(n)) => n.to_string(),
            Some(Token::True) => "true".to_owned(),
            Some(Token::False) => "false".to_owned(),
            _ => String::new(),
        }
    }

    fn consume_ast_regex(&mut self) -> String {
        match self.advance().cloned() {
            Some(Token::RegexLit(s) | Token::StringLit(s)) => s,
            _ => String::new(),
        }
    }

    fn consume_ast_number(&mut self) -> Result<f64, WhenClauseError> {
        match self.advance().cloned() {
            Some(Token::NumberLit(n)) => Ok(n),
            Some(Token::Ident(s) | Token::StringLit(s)) => s
                .parse()
                .map_err(|_| WhenClauseError::InvalidNumber(s)),
            _ => Ok(0.0),
        }
    }
}

// ── Well-known context key constants ─────────────────────────────────────────

/// Well-known context key names used by VS Code and `SideX` keybindings.
pub mod keys {
    // ── Editor state ────────────────────────────────────────────────────
    pub const EDITOR_TEXT_FOCUS: &str = "editorTextFocus";
    pub const EDITOR_HAS_SELECTION: &str = "editorHasSelection";
    pub const EDITOR_HAS_MULTIPLE_SELECTIONS: &str = "editorHasMultipleSelections";
    pub const EDITOR_READONLY: &str = "editorReadonly";
    pub const EDITOR_LANG_ID: &str = "editorLangId";
    pub const EDITOR_HAS_COMPLETION_ITEM_PROVIDER: &str = "editorHasCompletionItemProvider";
    pub const EDITOR_HAS_CODE_ACTIONS_PROVIDER: &str = "editorHasCodeActionsProvider";
    pub const EDITOR_HAS_DEFINITION_PROVIDER: &str = "editorHasDefinitionProvider";
    pub const EDITOR_HAS_DECLARATION_PROVIDER: &str = "editorHasDeclarationProvider";
    pub const EDITOR_HAS_IMPLEMENTATION_PROVIDER: &str = "editorHasImplementationProvider";
    pub const EDITOR_HAS_TYPE_DEFINITION_PROVIDER: &str = "editorHasTypeDefinitionProvider";
    pub const EDITOR_HAS_REFERENCE_PROVIDER: &str = "editorHasReferenceProvider";
    pub const EDITOR_HAS_RENAME_PROVIDER: &str = "editorHasRenameProvider";
    pub const EDITOR_HAS_DOCUMENT_FORMATTING_PROVIDER: &str =
        "editorHasDocumentFormattingProvider";
    pub const EDITOR_HAS_DOCUMENT_SELECTION_FORMATTING_PROVIDER: &str =
        "editorHasDocumentSelectionFormattingProvider";
    pub const EDITOR_HAS_SIGNATURE_HELP_PROVIDER: &str = "editorHasSignatureHelpProvider";
    pub const EDITOR_HAS_HOVER_PROVIDER: &str = "editorHasHoverProvider";
    pub const EDITOR_HAS_DOCUMENT_SYMBOL_PROVIDER: &str = "editorHasDocumentSymbolProvider";
    pub const EDITOR_HAS_FOLDING_RANGE_PROVIDER: &str = "editorHasFoldingRangeProvider";
    pub const EDITOR_HAS_CALL_HIERARCHY_PROVIDER: &str = "editorHasCallHierarchyProvider";
    pub const EDITOR_HAS_INLAY_HINTS_PROVIDER: &str = "editorHasInlayHintsProvider";
    pub const EDITOR_PINNED: &str = "editorPinned";

    // ── Input / focus state ─────────────────────────────────────────────
    pub const INPUT_FOCUS: &str = "inputFocus";
    pub const TEXT_INPUT_FOCUS: &str = "textInputFocus";
    pub const TERMINAL_FOCUS: &str = "terminalFocus";
    pub const TERMINAL_IS_OPEN: &str = "terminalIsOpen";
    pub const TERMINAL_PROCESS_SUPPORTED: &str = "terminalProcessSupported";
    pub const TERMINAL_SHELL_TYPE: &str = "terminalShellType";
    pub const TERMINAL_TABS_FOCUS: &str = "terminalTabsFocus";

    // ── Suggest / parameter hints ───────────────────────────────────────
    pub const SUGGEST_WIDGET_VISIBLE: &str = "suggestWidgetVisible";
    pub const SUGGEST_WIDGET_MULTIPLE_SUGGESTIONS: &str = "suggestWidgetMultipleSuggestions";
    pub const SUGGEST_WIDGET_HAS_FOCUS_SUGGESTION: &str = "suggestWidgetHasFocusSuggestion";
    pub const PARAMETER_HINTS_VISIBLE: &str = "parameterHintsVisible";
    pub const PARAMETER_HINTS_MULTIPLE_SIGNATURES: &str = "parameterHintsMultipleSignatures";

    // ── Find widget ─────────────────────────────────────────────────────
    pub const FIND_WIDGET_VISIBLE: &str = "findWidgetVisible";
    pub const FIND_INPUT_FOCUSED: &str = "findInputFocussed";
    pub const REPLACE_INPUT_FOCUSED: &str = "replaceInputFocussed";
    pub const REPLACE_ACTIVE: &str = "replaceActive";
    pub const CAN_REPLACE_IN_FIND: &str = "canReplaceInFind";

    // ── Rename widget ───────────────────────────────────────────────────
    pub const RENAME_INPUT_VISIBLE: &str = "renameInputVisible";

    // ── References ──────────────────────────────────────────────────────
    pub const REFERENCE_SEARCH_VISIBLE: &str = "referenceSearchVisible";
    pub const REFERENCE_SEARCH_TREE_FOCUSED: &str = "referenceSearchTreeFocused";

    // ── Snippets ────────────────────────────────────────────────────────
    pub const IN_SNIPPET_MODE: &str = "inSnippetMode";
    pub const HAS_NEXT_TABSTOP: &str = "hasNextTabstop";
    pub const HAS_PREV_TABSTOP: &str = "hasPrevTabstop";
    pub const TAB_COMPLETION_ENABLED: &str = "tabCompletionEnabled";

    // ── Quick open / command palette ────────────────────────────────────
    pub const IN_QUICK_OPEN: &str = "inQuickOpen";
    pub const QUICK_OPEN_VISIBLE: &str = "quickOpenVisible";
    pub const QUICK_INPUT_TYPE: &str = "quickInputType";

    // ── Sidebar / panels ────────────────────────────────────────────────
    pub const SIDEBAR_VISIBLE: &str = "sideBarVisible";
    pub const SIDEBAR_FOCUS: &str = "sideBarFocus";
    pub const PANEL_VISIBLE: &str = "panelVisible";
    pub const PANEL_FOCUS: &str = "panelFocus";
    pub const PANEL_POSITION: &str = "panelPosition";
    pub const AUXILIARY_BAR_VISIBLE: &str = "auxiliaryBarVisible";

    // ── Viewlets ────────────────────────────────────────────────────────
    pub const EXPLORER_VIEWLET_VISIBLE: &str = "explorerViewletVisible";
    pub const EXPLORER_FOCUS: &str = "explorerViewletFocus";
    pub const EXPLORER_RESOURCE_IS_FOLDER: &str = "explorerResourceIsFolder";
    pub const FILES_EXPLORER_FOCUS: &str = "filesExplorerFocus";
    pub const SEARCH_VIEWLET_VISIBLE: &str = "searchViewletVisible";
    pub const SEARCH_VIEW_FOCUS: &str = "searchViewletFocus";
    pub const SCM_VIEWLET_VISIBLE: &str = "scmViewletVisible";
    pub const SCM_VIEW_FOCUS: &str = "view.scm.visible";
    pub const DEBUG_VIEWLET_VISIBLE: &str = "debugViewletVisible";
    pub const EXTENSIONS_VIEWLET_VISIBLE: &str = "extensionsViewletVisible";

    // ── Debug ───────────────────────────────────────────────────────────
    pub const IN_DEBUG_MODE: &str = "inDebugMode";
    pub const DEBUGGING_STOPPED: &str = "debuggingStopped";
    pub const DEBUG_STATE: &str = "debugState";
    pub const DEBUG_TYPE: &str = "debugType";
    pub const CALL_STACK_ITEM_TYPE: &str = "callStackItemType";
    pub const DEBUG_CONSOLE_FOCUS: &str = "inDebugRepl";
    pub const EXCEPTION_WIDGET_VISIBLE: &str = "exceptionWidgetVisible";
    pub const BREAKPOINT_WIDGET_VISIBLE: &str = "breakpointWidgetVisible";

    // ── Resource / file info ────────────────────────────────────────────
    pub const RESOURCE_SCHEME: &str = "resourceScheme";
    pub const RESOURCE_FILENAME: &str = "resourceFilename";
    pub const RESOURCE_EXTENSION: &str = "resourceExtname";
    pub const RESOURCE_LANG_ID: &str = "resourceLangId";
    pub const RESOURCE_DIRNAME: &str = "resourceDirname";
    pub const RESOURCE_PATH: &str = "resourcePath";
    pub const RESOURCE_SET: &str = "resourceSet";
    pub const IS_FILE_SYSTEM_RESOURCE: &str = "isFileSystemResource";

    // ── Diff editor ─────────────────────────────────────────────────────
    pub const IN_DIFF_EDITOR: &str = "isInDiffEditor";
    pub const DIFF_EDITOR_READONLY: &str = "diffEditorReadonly";

    // ── Embedded / walkthrough ──────────────────────────────────────────
    pub const IN_EMBEDDED_EDITOR: &str = "isInEmbeddedEditor";
    pub const IN_WALK_THROUGH: &str = "isInWalkThrough";

    // ── Symbols ─────────────────────────────────────────────────────────
    pub const HAS_SYMBOLS: &str = "hasSymbols";

    // ── List / tree ─────────────────────────────────────────────────────
    pub const LIST_FOCUS: &str = "listFocus";
    pub const LIST_HAS_SELECTION: &str = "listHasSelectionOrFocus";
    pub const LIST_SUPPORTS_MULTI_SELECT: &str = "listSupportsMultiselect";
    pub const TREE_ELEMENT_CAN_COLLAPSE: &str = "treeElementCanCollapse";
    pub const TREE_ELEMENT_CAN_EXPAND: &str = "treeElementCanExpand";
    pub const TREE_ELEMENT_HAS_PARENT: &str = "treeElementHasParent";
    pub const TREE_ELEMENT_HAS_CHILD: &str = "treeElementHasChild";

    // ── Breadcrumbs ─────────────────────────────────────────────────────
    pub const BREADCRUMB_FOCUSED: &str = "breadcrumbsFocused";
    pub const BREADCRUMB_VISIBLE: &str = "breadcrumbsVisible";

    // ── Zen mode / fullscreen ───────────────────────────────────────────
    pub const IN_ZEN_MODE: &str = "inZenMode";
    pub const IS_FULLSCREEN: &str = "isFullscreen";
    pub const IS_CENTERED_LAYOUT: &str = "isCenteredLayout";

    // ── Notifications ───────────────────────────────────────────────────
    pub const NOTIFICATION_FOCUS: &str = "notificationFocus";
    pub const NOTIFICATION_CENTER_VISIBLE: &str = "notificationCenterVisible";
    pub const NOTIFICATION_TOAST_VISIBLE: &str = "notificationToastVisible";

    // ── Editor groups / tabs ────────────────────────────────────────────
    pub const ACTIVE_EDITOR_GROUP_EMPTY: &str = "activeEditorGroupEmpty";
    pub const MULTI_EDITOR_GROUPS: &str = "multipleEditorGroups";
    pub const ACTIVE_EDITOR_IS_DIRTY: &str = "activeEditorIsDirty";
    pub const ACTIVE_EDITOR_IS_NOT_PREVIEW: &str = "activeEditorIsNotPreview";
    pub const EDITOR_GROUP_COUNT: &str = "editorGroupCount";

    // ── Platform ────────────────────────────────────────────────────────
    pub const IS_LINUX: &str = "isLinux";
    pub const IS_MAC: &str = "isMac";
    pub const IS_WINDOWS: &str = "isWindows";
    pub const IS_WEB: &str = "isWeb";

    // ── SCM ─────────────────────────────────────────────────────────────
    pub const SCM_PROVIDER_COUNT: &str = "scmProviderCount";
    pub const SCM_RESOURCE_GROUP_COUNT: &str = "scmResourceGroupCount";

    // ── Testing ─────────────────────────────────────────────────────────
    pub const TESTING_IS_RUNNING: &str = "testing.isRunning";
    pub const TESTING_CAN_RUN: &str = "testing.canRun";
    pub const TESTING_CAN_DEBUG: &str = "testing.canDebug";

    // ── Notebook ────────────────────────────────────────────────────────
    pub const NOTEBOOK_CELL_FOCUS: &str = "notebookCellFocused";
    pub const NOTEBOOK_EDITOR_FOCUSED: &str = "notebookEditorFocused";
    pub const NOTEBOOK_KERNEL_COUNT: &str = "notebookKernelCount";

    // ── Markdown ────────────────────────────────────────────────────────
    pub const MARKDOWN_PREVIEW_FOCUS: &str = "markdownPreviewFocus";

    // ── Git ─────────────────────────────────────────────────────────────
    pub const GIT_OPEN_REPOSITORY_COUNT: &str = "gitOpenRepositoryCount";
    pub const GIT_HAS_REMOTES: &str = "gitHasRemotes";
    pub const GIT_STATE: &str = "gitState";

    // ── Settings ────────────────────────────────────────────────────────
    pub const IN_SETTINGS_EDITOR: &str = "inSettingsEditor";
    pub const IN_SETTINGS_JSON_EDITOR: &str = "inSettingsJSONEditor";
    pub const IN_KEYBINDINGS_EDITOR: &str = "inKeybindingsEditor";

    // ── Accessibility ───────────────────────────────────────────────────
    pub const ACCESSIBILITY_MODE_ENABLED: &str = "accessibilityModeEnabled";
    pub const SCREEN_READER_OPTIMIZED: &str = "screenReaderOptimized";
    pub const HIGH_CONTRAST: &str = "highContrast";

    // ── Comments ────────────────────────────────────────────────────────
    pub const COMMENT_THREAD_IS_EMPTY: &str = "commentThreadIsEmpty";
    pub const ACTIVE_COMMENT_CONTROLLER: &str = "activeCommentController";

    // ── Tasks ───────────────────────────────────────────────────────────
    pub const TASK_RUNNING: &str = "taskRunning";
    pub const TASK_COUNT: &str = "taskCount";
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with(pairs: &[(&str, bool)]) -> ContextKeys {
        let mut ctx = ContextKeys::new();
        for &(k, v) in pairs {
            ctx.set_bool(k, v);
        }
        ctx
    }

    #[test]
    fn empty_expression_is_true() {
        assert!(evaluate("", &ContextKeys::new()));
        assert!(evaluate("  ", &ContextKeys::new()));
    }

    #[test]
    fn simple_true() {
        let ctx = ctx_with(&[("editorTextFocus", true)]);
        assert!(evaluate("editorTextFocus", &ctx));
    }

    #[test]
    fn simple_false() {
        let ctx = ctx_with(&[("editorTextFocus", false)]);
        assert!(!evaluate("editorTextFocus", &ctx));
    }

    #[test]
    fn missing_key_is_false() {
        assert!(!evaluate("editorTextFocus", &ContextKeys::new()));
    }

    #[test]
    fn negation() {
        let ctx = ctx_with(&[("editorReadonly", false)]);
        assert!(evaluate("!editorReadonly", &ctx));
    }

    #[test]
    fn double_negation() {
        let ctx = ctx_with(&[("a", true)]);
        assert!(evaluate("!!a", &ctx));
    }

    #[test]
    fn and_expression() {
        let ctx = ctx_with(&[("editorTextFocus", true), ("editorReadonly", false)]);
        assert!(evaluate("editorTextFocus && !editorReadonly", &ctx));
    }

    #[test]
    fn or_expression() {
        let ctx = ctx_with(&[("a", false), ("b", true)]);
        assert!(evaluate("a || b", &ctx));
    }

    #[test]
    fn equality_string() {
        let mut ctx = ContextKeys::new();
        ctx.set_string("resourceScheme", "file");
        assert!(evaluate("resourceScheme == 'file'", &ctx));
        assert!(!evaluate("resourceScheme == 'untitled'", &ctx));
    }

    #[test]
    fn equality_bool() {
        let mut ctx = ContextKeys::new();
        ctx.set_bool("isActive", true);
        assert!(evaluate("isActive == true", &ctx));
        assert!(!evaluate("isActive == false", &ctx));
    }

    #[test]
    fn inequality() {
        let mut ctx = ContextKeys::new();
        ctx.set_string("resourceScheme", "file");
        assert!(evaluate("resourceScheme != 'untitled'", &ctx));
        assert!(!evaluate("resourceScheme != 'file'", &ctx));
    }

    #[test]
    fn regex_match() {
        let mut ctx = ContextKeys::new();
        ctx.set_string("resourceFilename", "test_utils.rs");
        assert!(evaluate("resourceFilename =~ /\\.rs$/", &ctx));
        assert!(!evaluate("resourceFilename =~ /\\.ts$/", &ctx));
    }

    #[test]
    fn regex_match_case_insensitive() {
        let mut ctx = ContextKeys::new();
        ctx.set_string("resourceFilename", "README.md");
        assert!(evaluate("resourceFilename =~ /(?i)readme/", &ctx));
    }

    #[test]
    fn in_operator_list() {
        let mut ctx = ContextKeys::new();
        ctx.set_string("editorLangId", "rust");
        ctx.set_list(
            "supportedLanguages",
            vec!["rust".into(), "python".into(), "typescript".into()],
        );
        assert!(evaluate("editorLangId in supportedLanguages", &ctx));
    }

    #[test]
    fn in_operator_missing() {
        let mut ctx = ContextKeys::new();
        ctx.set_string("editorLangId", "haskell");
        ctx.set_list("supportedLanguages", vec!["rust".into(), "python".into()]);
        assert!(!evaluate("editorLangId in supportedLanguages", &ctx));
    }

    #[test]
    fn parentheses() {
        let ctx = ctx_with(&[("a", true), ("b", false), ("c", true)]);
        assert!(evaluate("a && (b || c)", &ctx));
        assert!(!evaluate("(a && b) || !c", &ctx));
    }

    #[test]
    fn complex_expression() {
        let mut ctx = ContextKeys::new();
        ctx.set_bool("editorTextFocus", true);
        ctx.set_bool("editorReadonly", false);
        ctx.set_string("resourceScheme", "file");
        assert!(evaluate(
            "editorTextFocus && !editorReadonly && resourceScheme == 'file'",
            &ctx
        ));
    }

    #[test]
    fn mixed_and_or_precedence() {
        let ctx = ctx_with(&[("a", true), ("b", false), ("c", true)]);
        // && binds tighter: a || (b && c) → true || false → true
        assert!(evaluate("a || b && c", &ctx));
        // (false && true) || true → false || true → true
        assert!(evaluate("b && a || c", &ctx));
    }

    #[test]
    fn true_false_literals() {
        let ctx = ContextKeys::new();
        assert!(evaluate("true", &ctx));
        assert!(!evaluate("false", &ctx));
        assert!(evaluate("true || false", &ctx));
        assert!(!evaluate("true && false", &ctx));
    }

    #[test]
    fn context_value_as_bool() {
        assert!(ContextValue::Bool(true).as_bool());
        assert!(!ContextValue::Bool(false).as_bool());
        assert!(ContextValue::String("hello".to_owned()).as_bool());
        assert!(!ContextValue::String(String::new()).as_bool());
        assert!(ContextValue::Number(1.0).as_bool());
        assert!(!ContextValue::Number(0.0).as_bool());
        assert!(ContextValue::List(vec!["a".into()]).as_bool());
        assert!(!ContextValue::List(vec![]).as_bool());
    }

    #[test]
    fn number_equality() {
        let mut ctx = ContextKeys::new();
        ctx.set_number("tabSize", 4.0);
        assert!(evaluate("tabSize == 4", &ctx));
        assert!(!evaluate("tabSize == 2", &ctx));
    }

    #[test]
    fn nested_parentheses() {
        let ctx = ctx_with(&[("a", true), ("b", true), ("c", false)]);
        assert!(evaluate("((a && b) || c)", &ctx));
        assert!(evaluate("(a && (b || c))", &ctx));
    }

    #[test]
    fn real_world_when_clauses() {
        let mut ctx = ContextKeys::new();
        ctx.set_bool("editorTextFocus", true);
        ctx.set_bool("editorHasSelection", false);
        ctx.set_bool("suggestWidgetVisible", false);
        ctx.set_bool("findWidgetVisible", false);

        assert!(evaluate("editorTextFocus && !suggestWidgetVisible", &ctx));
        assert!(evaluate("editorTextFocus && !editorHasSelection", &ctx));
        assert!(!evaluate("editorTextFocus && findWidgetVisible", &ctx));
    }

    #[test]
    fn unknown_key_in_comparison_defaults_false() {
        let ctx = ContextKeys::new();
        assert!(evaluate("unknownKey == false", &ctx));
        assert!(!evaluate("unknownKey == true", &ctx));
    }

    #[test]
    fn not_keyword_alias() {
        let ctx = ctx_with(&[("a", true)]);
        assert!(!evaluate("not a", &ctx));
    }

    #[test]
    fn dotted_identifiers() {
        let mut ctx = ContextKeys::new();
        ctx.set_bool("view.scm.visible", true);
        assert!(evaluate("view.scm.visible", &ctx));
    }

    #[test]
    fn keys_module_constants() {
        assert_eq!(keys::EDITOR_TEXT_FOCUS, "editorTextFocus");
        assert_eq!(keys::TERMINAL_FOCUS, "terminalFocus");
        assert_eq!(keys::IN_DEBUG_MODE, "inDebugMode");
    }

    #[test]
    fn tokenizer_handles_no_spaces() {
        let ctx = ctx_with(&[("a", true), ("b", true)]);
        assert!(evaluate("a&&b", &ctx));
    }

    #[test]
    fn tokenizer_handles_extra_spaces() {
        let ctx = ctx_with(&[("a", true), ("b", true)]);
        assert!(evaluate("  a   &&   b  ", &ctx));
    }

    #[test]
    fn regex_missing_key() {
        let ctx = ContextKeys::new();
        assert!(!evaluate("missing =~ /pattern/", &ctx));
    }

    #[test]
    fn less_than_comparison() {
        let mut ctx = ContextKeys::new();
        ctx.set_number("tabSize", 2.0);
        assert!(evaluate("tabSize < 4", &ctx));
        assert!(!evaluate("tabSize < 1", &ctx));
    }

    #[test]
    fn greater_than_comparison() {
        let mut ctx = ContextKeys::new();
        ctx.set_number("editorGroupCount", 3.0);
        assert!(evaluate("editorGroupCount > 1", &ctx));
        assert!(!evaluate("editorGroupCount > 5", &ctx));
    }

    #[test]
    fn less_equals_comparison() {
        let mut ctx = ContextKeys::new();
        ctx.set_number("tabSize", 4.0);
        assert!(evaluate("tabSize <= 4", &ctx));
        assert!(evaluate("tabSize <= 5", &ctx));
        assert!(!evaluate("tabSize <= 3", &ctx));
    }

    #[test]
    fn greater_equals_comparison() {
        let mut ctx = ContextKeys::new();
        ctx.set_number("tabSize", 4.0);
        assert!(evaluate("tabSize >= 4", &ctx));
        assert!(evaluate("tabSize >= 3", &ctx));
        assert!(!evaluate("tabSize >= 5", &ctx));
    }

    #[test]
    fn parse_when_clause_roundtrip() {
        let clause = super::parse_when_clause("editorTextFocus && !editorReadonly").unwrap();
        let mut keys = ContextKeys::new();
        keys.set_bool("editorTextFocus", true);
        keys.set_bool("editorReadonly", false);
        let svc = ContextKeyService { keys };
        assert!(clause.evaluate(&svc));
    }

    #[test]
    fn parse_when_clause_comparison_ops() {
        let clause = super::parse_when_clause("tabSize >= 4 && tabSize < 8").unwrap();
        let mut keys = ContextKeys::new();
        keys.set_number("tabSize", 4.0);
        let svc = ContextKeyService { keys };
        assert!(clause.evaluate(&svc));
    }

    #[test]
    fn parse_when_clause_empty() {
        let clause = super::parse_when_clause("").unwrap();
        let svc = ContextKeyService::new();
        assert!(svc.evaluate_clause(&clause));
    }

    #[test]
    fn context_key_service_platform_defaults() {
        let svc = ContextKeyService::with_platform_defaults();
        if cfg!(target_os = "macos") {
            assert!(svc.keys.is_true(keys::IS_MAC));
            assert!(!svc.keys.is_true(keys::IS_LINUX));
        }
    }

    #[test]
    fn keys_module_has_80_plus_constants() {
        let count = [
            keys::EDITOR_TEXT_FOCUS,
            keys::EDITOR_HAS_SELECTION,
            keys::EDITOR_HAS_MULTIPLE_SELECTIONS,
            keys::EDITOR_READONLY,
            keys::EDITOR_LANG_ID,
            keys::EDITOR_HAS_COMPLETION_ITEM_PROVIDER,
            keys::EDITOR_HAS_CODE_ACTIONS_PROVIDER,
            keys::EDITOR_HAS_DEFINITION_PROVIDER,
            keys::EDITOR_HAS_DECLARATION_PROVIDER,
            keys::EDITOR_HAS_IMPLEMENTATION_PROVIDER,
            keys::EDITOR_HAS_TYPE_DEFINITION_PROVIDER,
            keys::EDITOR_HAS_REFERENCE_PROVIDER,
            keys::EDITOR_HAS_RENAME_PROVIDER,
            keys::EDITOR_HAS_DOCUMENT_FORMATTING_PROVIDER,
            keys::EDITOR_HAS_DOCUMENT_SELECTION_FORMATTING_PROVIDER,
            keys::EDITOR_HAS_SIGNATURE_HELP_PROVIDER,
            keys::EDITOR_HAS_HOVER_PROVIDER,
            keys::EDITOR_HAS_DOCUMENT_SYMBOL_PROVIDER,
            keys::EDITOR_HAS_FOLDING_RANGE_PROVIDER,
            keys::EDITOR_HAS_CALL_HIERARCHY_PROVIDER,
            keys::EDITOR_HAS_INLAY_HINTS_PROVIDER,
            keys::EDITOR_PINNED,
            keys::INPUT_FOCUS,
            keys::TEXT_INPUT_FOCUS,
            keys::TERMINAL_FOCUS,
            keys::TERMINAL_IS_OPEN,
            keys::TERMINAL_PROCESS_SUPPORTED,
            keys::TERMINAL_SHELL_TYPE,
            keys::TERMINAL_TABS_FOCUS,
            keys::SUGGEST_WIDGET_VISIBLE,
            keys::SUGGEST_WIDGET_MULTIPLE_SUGGESTIONS,
            keys::SUGGEST_WIDGET_HAS_FOCUS_SUGGESTION,
            keys::PARAMETER_HINTS_VISIBLE,
            keys::PARAMETER_HINTS_MULTIPLE_SIGNATURES,
            keys::FIND_WIDGET_VISIBLE,
            keys::FIND_INPUT_FOCUSED,
            keys::REPLACE_INPUT_FOCUSED,
            keys::REPLACE_ACTIVE,
            keys::CAN_REPLACE_IN_FIND,
            keys::RENAME_INPUT_VISIBLE,
            keys::REFERENCE_SEARCH_VISIBLE,
            keys::REFERENCE_SEARCH_TREE_FOCUSED,
            keys::IN_SNIPPET_MODE,
            keys::HAS_NEXT_TABSTOP,
            keys::HAS_PREV_TABSTOP,
            keys::TAB_COMPLETION_ENABLED,
            keys::IN_QUICK_OPEN,
            keys::QUICK_OPEN_VISIBLE,
            keys::QUICK_INPUT_TYPE,
            keys::SIDEBAR_VISIBLE,
            keys::SIDEBAR_FOCUS,
            keys::PANEL_VISIBLE,
            keys::PANEL_FOCUS,
            keys::PANEL_POSITION,
            keys::AUXILIARY_BAR_VISIBLE,
            keys::EXPLORER_VIEWLET_VISIBLE,
            keys::EXPLORER_FOCUS,
            keys::EXPLORER_RESOURCE_IS_FOLDER,
            keys::FILES_EXPLORER_FOCUS,
            keys::SEARCH_VIEWLET_VISIBLE,
            keys::SEARCH_VIEW_FOCUS,
            keys::SCM_VIEWLET_VISIBLE,
            keys::SCM_VIEW_FOCUS,
            keys::DEBUG_VIEWLET_VISIBLE,
            keys::EXTENSIONS_VIEWLET_VISIBLE,
            keys::IN_DEBUG_MODE,
            keys::DEBUGGING_STOPPED,
            keys::DEBUG_STATE,
            keys::DEBUG_TYPE,
            keys::CALL_STACK_ITEM_TYPE,
            keys::DEBUG_CONSOLE_FOCUS,
            keys::EXCEPTION_WIDGET_VISIBLE,
            keys::BREAKPOINT_WIDGET_VISIBLE,
            keys::RESOURCE_SCHEME,
            keys::RESOURCE_FILENAME,
            keys::RESOURCE_EXTENSION,
            keys::RESOURCE_LANG_ID,
            keys::RESOURCE_DIRNAME,
            keys::RESOURCE_PATH,
            keys::RESOURCE_SET,
            keys::IS_FILE_SYSTEM_RESOURCE,
            keys::IN_DIFF_EDITOR,
            keys::DIFF_EDITOR_READONLY,
            keys::IN_EMBEDDED_EDITOR,
            keys::IN_WALK_THROUGH,
            keys::HAS_SYMBOLS,
            keys::LIST_FOCUS,
            keys::LIST_HAS_SELECTION,
            keys::LIST_SUPPORTS_MULTI_SELECT,
            keys::TREE_ELEMENT_CAN_COLLAPSE,
            keys::TREE_ELEMENT_CAN_EXPAND,
            keys::TREE_ELEMENT_HAS_PARENT,
            keys::TREE_ELEMENT_HAS_CHILD,
            keys::BREADCRUMB_FOCUSED,
            keys::BREADCRUMB_VISIBLE,
            keys::IN_ZEN_MODE,
            keys::IS_FULLSCREEN,
            keys::IS_CENTERED_LAYOUT,
            keys::NOTIFICATION_FOCUS,
            keys::NOTIFICATION_CENTER_VISIBLE,
            keys::NOTIFICATION_TOAST_VISIBLE,
            keys::ACTIVE_EDITOR_GROUP_EMPTY,
            keys::MULTI_EDITOR_GROUPS,
            keys::ACTIVE_EDITOR_IS_DIRTY,
            keys::ACTIVE_EDITOR_IS_NOT_PREVIEW,
            keys::EDITOR_GROUP_COUNT,
            keys::IS_LINUX,
            keys::IS_MAC,
            keys::IS_WINDOWS,
            keys::IS_WEB,
            keys::SCM_PROVIDER_COUNT,
            keys::SCM_RESOURCE_GROUP_COUNT,
            keys::TESTING_IS_RUNNING,
            keys::TESTING_CAN_RUN,
            keys::TESTING_CAN_DEBUG,
            keys::NOTEBOOK_CELL_FOCUS,
            keys::NOTEBOOK_EDITOR_FOCUSED,
            keys::NOTEBOOK_KERNEL_COUNT,
            keys::MARKDOWN_PREVIEW_FOCUS,
            keys::GIT_OPEN_REPOSITORY_COUNT,
            keys::GIT_HAS_REMOTES,
            keys::GIT_STATE,
            keys::IN_SETTINGS_EDITOR,
            keys::IN_SETTINGS_JSON_EDITOR,
            keys::IN_KEYBINDINGS_EDITOR,
            keys::ACCESSIBILITY_MODE_ENABLED,
            keys::SCREEN_READER_OPTIMIZED,
            keys::HIGH_CONTRAST,
            keys::COMMENT_THREAD_IS_EMPTY,
            keys::ACTIVE_COMMENT_CONTROLLER,
            keys::TASK_RUNNING,
            keys::TASK_COUNT,
        ]
        .len();
        assert!(count >= 80, "expected 80+ context keys, got {count}");
    }
}
