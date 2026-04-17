//! `TextMate` grammar support for languages without tree-sitter grammars.
//!
//! Provides a regex-based tokenizer that loads `.tmLanguage.json` or `.plist`
//! grammar definitions and tokenizes source lines using `TextMate` scope rules.

use std::collections::HashMap;
use std::path::Path;

use fancy_regex::Regex as FancyRegex;
use serde::Deserialize;

/// Helper to call `fancy_regex::Regex::find` with explicit error handling.
fn fancy_find<'t>(re: &FancyRegex, text: &'t str) -> Option<fancy_regex::Match<'t>> {
    re.find(text).ok().flatten()
}

/// Interned scope identifier.
pub type ScopeId = u32;

/// Token with string scopes (legacy API).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenInfo {
    pub start: usize,
    pub end: usize,
    pub scopes: Vec<String>,
}

/// Token with interned scope ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub scopes: Vec<ScopeId>,
}

/// Legacy state carried between lines.
#[derive(Debug, Clone)]
pub struct TokenizerState {
    pub rule_stack: Vec<(String, usize)>,
}

impl Default for TokenizerState {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenizerState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rule_stack: Vec::new(),
        }
    }
}

/// Stack of active rules tracking nested grammar contexts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleStack {
    frames: Vec<RuleFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuleFrame {
    scope_name: String,
    rule_index: usize,
    end_pattern: Option<String>,
}

impl Default for RuleStack {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleStack {
    #[must_use]
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    pub fn push(&mut self, scope: String, rule_index: usize, end_pattern: Option<String>) {
        self.frames.push(RuleFrame {
            scope_name: scope,
            rule_index,
            end_pattern,
        });
    }

    pub fn pop(&mut self) -> Option<(String, usize, Option<String>)> {
        self.frames
            .pop()
            .map(|f| (f.scope_name, f.rule_index, f.end_pattern))
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    #[must_use]
    pub fn current_scopes(&self) -> Vec<&str> {
        self.frames.iter().map(|f| f.scope_name.as_str()).collect()
    }
}

/// Result of tokenizing a single line.
#[derive(Debug, Clone)]
pub struct TokenizeResult {
    pub tokens: Vec<Token>,
    pub end_state: RuleStack,
}

// ---------------------------------------------------------------------------
// Grammar types
// ---------------------------------------------------------------------------

/// A compiled `TextMate` grammar.
#[derive(Debug, Clone)]
pub struct TextMateGrammar {
    pub scope_name: String,
    pub file_types: Vec<String>,
    pub patterns: Vec<Pattern>,
    pub repository: HashMap<String, RepositoryRule>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Match(MatchRule),
    BeginEnd(BeginEndRule),
    BeginWhile(BeginWhileRule),
    Include(IncludeRef),
}

#[derive(Debug, Clone)]
pub struct MatchRule {
    pub regex: String,
    pub scope: Option<String>,
    pub captures: HashMap<usize, String>,
}

#[derive(Debug, Clone)]
pub struct BeginEndRule {
    pub begin: String,
    pub end: String,
    pub scope: Option<String>,
    pub begin_captures: HashMap<usize, String>,
    pub end_captures: HashMap<usize, String>,
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone)]
pub struct BeginWhileRule {
    pub begin: String,
    pub while_pattern: String,
    pub scope: Option<String>,
    pub begin_captures: HashMap<usize, String>,
    pub while_captures: HashMap<usize, String>,
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, Clone)]
pub enum IncludeRef {
    SelfRef,
    BaseRef,
    Repository(String),
    External(String),
}

#[derive(Debug, Clone)]
pub struct RepositoryRule {
    pub patterns: Vec<Pattern>,
}

#[derive(Debug, thiserror::Error)]
pub enum TextMateError {
    #[error("failed to read grammar file: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid JSON grammar: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid plist grammar: {0}")]
    Plist(#[from] plist::Error),
    #[error("invalid regex in grammar: {pattern}")]
    InvalidRegex {
        pattern: String,
        #[source]
        source: Box<fancy_regex::Error>,
    },
    #[error("unsupported grammar format")]
    UnsupportedFormat,
}

// ---------------------------------------------------------------------------
// Raw deserialization types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGrammar {
    scope_name: Option<String>,
    #[serde(default)]
    file_types: Vec<String>,
    #[serde(default)]
    patterns: Vec<RawPattern>,
    #[serde(default)]
    repository: HashMap<String, RawRepo>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawPattern {
    #[serde(rename = "match")]
    match_regex: Option<String>,
    begin: Option<String>,
    end: Option<String>,
    #[serde(rename = "while")]
    while_regex: Option<String>,
    name: Option<String>,
    #[serde(default, rename = "contentName")]
    _content_name: Option<String>,
    #[serde(default)]
    captures: HashMap<String, RawCaptureName>,
    #[serde(default, rename = "beginCaptures")]
    begin_captures: HashMap<String, RawCaptureName>,
    #[serde(default, rename = "endCaptures")]
    end_captures: HashMap<String, RawCaptureName>,
    #[serde(default, rename = "whileCaptures")]
    while_captures: HashMap<String, RawCaptureName>,
    #[serde(default)]
    patterns: Vec<RawPattern>,
    include: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawCaptureName {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRepo {
    #[serde(default)]
    patterns: Vec<RawPattern>,
    #[serde(rename = "match")]
    match_regex: Option<String>,
    begin: Option<String>,
    end: Option<String>,
    name: Option<String>,
    #[serde(default)]
    captures: HashMap<String, RawCaptureName>,
    #[serde(default, rename = "beginCaptures")]
    begin_captures: HashMap<String, RawCaptureName>,
    #[serde(default, rename = "endCaptures")]
    end_captures: HashMap<String, RawCaptureName>,
    include: Option<String>,
}

// ---------------------------------------------------------------------------
// Grammar loading
// ---------------------------------------------------------------------------

impl TextMateGrammar {
    pub fn from_json(json: &str) -> Result<Self, TextMateError> {
        let raw: RawGrammar = serde_json::from_str(json)?;
        Ok(Self::from_raw(raw))
    }

    pub fn from_plist(data: &[u8]) -> Result<Self, TextMateError> {
        let raw: RawGrammar = plist::from_bytes(data)?;
        Ok(Self::from_raw(raw))
    }

    pub fn from_file(path: &Path) -> Result<Self, TextMateError> {
        let data = std::fs::read(path)?;
        match path.extension().and_then(|e| e.to_str()) {
            Some("json") => Self::from_json(&String::from_utf8_lossy(&data)),
            Some("plist" | "tmLanguage") => Self::from_plist(&data),
            _ => Err(TextMateError::UnsupportedFormat),
        }
    }

    fn from_raw(raw: RawGrammar) -> Self {
        let patterns = raw.patterns.into_iter().map(convert_pattern).collect();
        let repository = raw
            .repository
            .into_iter()
            .map(|(name, repo)| (name, convert_repo(repo)))
            .collect();
        Self {
            scope_name: raw.scope_name.unwrap_or_default(),
            file_types: raw.file_types,
            patterns,
            repository,
        }
    }
}

fn convert_captures(raw: &HashMap<String, RawCaptureName>) -> HashMap<usize, String> {
    raw.iter()
        .filter_map(|(k, v)| {
            let idx = k.parse::<usize>().ok()?;
            let name = v.name.clone()?;
            Some((idx, name))
        })
        .collect()
}

fn convert_pattern(raw: RawPattern) -> Pattern {
    if let Some(include) = raw.include {
        return Pattern::Include(parse_include(&include));
    }
    if let Some(regex) = raw.match_regex {
        return Pattern::Match(MatchRule {
            regex,
            scope: raw.name,
            captures: convert_captures(&raw.captures),
        });
    }
    if let Some(begin) = raw.begin {
        if let Some(while_pat) = raw.while_regex {
            return Pattern::BeginWhile(BeginWhileRule {
                begin,
                while_pattern: while_pat,
                scope: raw.name,
                begin_captures: convert_captures(&raw.begin_captures),
                while_captures: convert_captures(&raw.while_captures),
                patterns: raw.patterns.into_iter().map(convert_pattern).collect(),
            });
        }
        if let Some(end) = raw.end {
            return Pattern::BeginEnd(BeginEndRule {
                begin,
                end,
                scope: raw.name,
                begin_captures: convert_captures(&raw.begin_captures),
                end_captures: convert_captures(&raw.end_captures),
                patterns: raw.patterns.into_iter().map(convert_pattern).collect(),
            });
        }
    }
    Pattern::Match(MatchRule {
        regex: String::new(),
        scope: raw.name,
        captures: HashMap::new(),
    })
}

fn convert_repo(raw: RawRepo) -> RepositoryRule {
    let mut patterns = Vec::new();
    if let Some(include) = raw.include {
        patterns.push(Pattern::Include(parse_include(&include)));
    } else if let Some(regex) = raw.match_regex {
        patterns.push(Pattern::Match(MatchRule {
            regex,
            scope: raw.name.clone(),
            captures: convert_captures(&raw.captures),
        }));
    } else if let Some(begin) = raw.begin {
        if let Some(end) = raw.end {
            patterns.push(Pattern::BeginEnd(BeginEndRule {
                begin,
                end,
                scope: raw.name.clone(),
                begin_captures: convert_captures(&raw.begin_captures),
                end_captures: convert_captures(&raw.end_captures),
                patterns: raw.patterns.iter().cloned().map(convert_pattern).collect(),
            }));
        }
    }
    for p in raw.patterns {
        patterns.push(convert_pattern(p));
    }
    RepositoryRule { patterns }
}

fn parse_include(s: &str) -> IncludeRef {
    match s {
        "$self" => IncludeRef::SelfRef,
        "$base" => IncludeRef::BaseRef,
        s if s.starts_with('#') => IncludeRef::Repository(s[1..].to_owned()),
        other => IncludeRef::External(other.to_owned()),
    }
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/// Tokenizer that processes source lines using a [`TextMateGrammar`].
pub struct TextMateTokenizer<'g> {
    grammar: &'g TextMateGrammar,
    scope_interner: HashMap<String, ScopeId>,
    next_scope_id: ScopeId,
}

impl<'g> TextMateTokenizer<'g> {
    #[must_use]
    pub fn new(grammar: &'g TextMateGrammar) -> Self {
        Self {
            grammar,
            scope_interner: HashMap::new(),
            next_scope_id: 0,
        }
    }

    /// Interns a scope name, returning a stable [`ScopeId`].
    pub fn intern_scope(&mut self, name: &str) -> ScopeId {
        if let Some(&id) = self.scope_interner.get(name) {
            return id;
        }
        let id = self.next_scope_id;
        self.next_scope_id += 1;
        self.scope_interner.insert(name.to_owned(), id);
        id
    }

    #[must_use]
    pub fn scope_name(&self, id: ScopeId) -> Option<&str> {
        self.scope_interner
            .iter()
            .find(|(_, &v)| v == id)
            .map(|(k, _)| k.as_str())
    }

    /// Tokenizes a line using the [`RuleStack`]-based state.
    pub fn tokenize_line_with_stack(&mut self, line: &str, state: &RuleStack) -> TokenizeResult {
        let mut new_state = state.clone();
        let mut tokens: Vec<Token> = Vec::new();
        let mut pos = 0;

        let root_id = self.intern_scope(&self.grammar.scope_name);
        let mut base_ids: Vec<ScopeId> = vec![root_id];
        for scope in state.current_scopes() {
            let id = self.intern_scope(scope);
            base_ids.push(id);
        }

        // Handle continuation of a begin/end rule from previous line
        if !new_state.is_empty() {
            if let Some(end_pat) = new_state.frames.last().and_then(|f| f.end_pattern.clone()) {
                if let Ok(re) = FancyRegex::new(&end_pat) {
                    if let Some(m) = fancy_find(&re, line) {
                        if pos < m.start() {
                            tokens.push(Token {
                                start: pos,
                                end: m.start(),
                                scopes: base_ids.clone(),
                            });
                        }
                        tokens.push(Token {
                            start: m.start(),
                            end: m.end(),
                            scopes: base_ids.clone(),
                        });
                        pos = m.end();
                        new_state.pop();
                        base_ids.pop();
                    } else {
                        tokens.push(Token {
                            start: 0,
                            end: line.len(),
                            scopes: base_ids.clone(),
                        });
                        return TokenizeResult {
                            tokens,
                            end_state: new_state,
                        };
                    }
                }
            }
        }

        while pos < line.len() {
            let remaining = &line[pos..];
            let pats = self.grammar.patterns.clone();
            if let Some((t, adv)) =
                self.try_match_compiled(&pats, remaining, pos, &base_ids, &mut new_state, 0)
            {
                tokens.extend(t);
                pos += adv;
            } else {
                let ne = (pos + 1).min(line.len());
                tokens.push(Token {
                    start: pos,
                    end: ne,
                    scopes: base_ids.clone(),
                });
                pos = ne;
            }
        }

        merge_compiled(&mut tokens);
        TokenizeResult {
            tokens,
            end_state: new_state,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn try_match_compiled(
        &mut self,
        patterns: &[Pattern],
        text: &str,
        offset: usize,
        base: &[ScopeId],
        state: &mut RuleStack,
        depth: usize,
    ) -> Option<(Vec<Token>, usize)> {
        if depth > 8 {
            return None;
        }
        let mut best: Option<(usize, Vec<Token>, usize)> = None;

        for pattern in patterns {
            match pattern {
                Pattern::Match(rule) => {
                    if rule.regex.is_empty() {
                        continue;
                    }
                    let Ok(re) = FancyRegex::new(&rule.regex) else {
                        continue;
                    };
                    let Some(m) = fancy_find(&re, text) else {
                        continue;
                    };
                    if m.start() == 0 && m.end() > 0 {
                        let bp = best.as_ref().map_or(usize::MAX, |b| b.0);
                        if m.start() < bp {
                            let mut sc = base.to_vec();
                            if let Some(ref s) = rule.scope {
                                sc.push(self.intern_scope(s));
                            }
                            best = Some((
                                m.start(),
                                vec![Token {
                                    start: offset,
                                    end: offset + m.end(),
                                    scopes: sc,
                                }],
                                m.end(),
                            ));
                        }
                    }
                }
                Pattern::BeginEnd(rule) => {
                    let Ok(re) = FancyRegex::new(&rule.begin) else {
                        continue;
                    };
                    let Some(m) = fancy_find(&re, text) else {
                        continue;
                    };
                    if m.start() == 0 && m.end() > 0 {
                        let bp = best.as_ref().map_or(usize::MAX, |b| b.0);
                        if m.start() < bp {
                            let mut sc = base.to_vec();
                            let sn = rule.scope.clone().unwrap_or_default();
                            if !sn.is_empty() {
                                sc.push(self.intern_scope(&sn));
                            }
                            state.push(sn, 0, Some(rule.end.clone()));
                            best = Some((
                                m.start(),
                                vec![Token {
                                    start: offset,
                                    end: offset + m.end(),
                                    scopes: sc,
                                }],
                                m.end(),
                            ));
                        }
                    }
                }
                Pattern::BeginWhile(rule) => {
                    let Ok(re) = FancyRegex::new(&rule.begin) else {
                        continue;
                    };
                    let Some(m) = fancy_find(&re, text) else {
                        continue;
                    };
                    if m.start() == 0 && m.end() > 0 {
                        let bp = best.as_ref().map_or(usize::MAX, |b| b.0);
                        if m.start() < bp {
                            let mut sc = base.to_vec();
                            if let Some(ref s) = rule.scope {
                                sc.push(self.intern_scope(s));
                            }
                            best = Some((
                                m.start(),
                                vec![Token {
                                    start: offset,
                                    end: offset + m.end(),
                                    scopes: sc,
                                }],
                                m.end(),
                            ));
                        }
                    }
                }
                Pattern::Include(inc) => {
                    let ps = resolve_inc(self.grammar, inc);
                    if let Some(r) =
                        self.try_match_compiled(&ps, text, offset, base, state, depth + 1)
                    {
                        let bp = best.as_ref().map_or(usize::MAX, |b| b.0);
                        if 0 < bp {
                            best = Some((0, r.0, r.1));
                        }
                    }
                }
            }
        }
        best.map(|(_, t, a)| (t, a))
    }

    /// Tokenizes a line using the legacy [`TokenizerState`] API.
    pub fn tokenize_line(&self, line: &str, state: &mut TokenizerState) -> Vec<TokenInfo> {
        let mut tokens = Vec::new();
        let mut pos = 0;
        let base: Vec<String> = std::iter::once(self.grammar.scope_name.clone())
            .chain(state.rule_stack.iter().map(|(s, _)| s.clone()))
            .collect();

        if !state.rule_stack.is_empty() {
            let (scope, rule_idx) = state.rule_stack.last().unwrap().clone();
            if let Some(rule) = find_first_begin_end(&self.grammar.patterns, rule_idx) {
                if let Ok(re) = FancyRegex::new(&rule.end) {
                    if let Some(m) = fancy_find(&re, line) {
                        if pos < m.start() {
                            let mut sc = base.clone();
                            sc.push(scope.clone());
                            tokens.push(TokenInfo {
                                start: pos,
                                end: m.start(),
                                scopes: sc,
                            });
                        }
                        let mut sc = base.clone();
                        sc.push(scope);
                        tokens.push(TokenInfo {
                            start: m.start(),
                            end: m.end(),
                            scopes: sc,
                        });
                        pos = m.end();
                        state.rule_stack.pop();
                    } else {
                        let mut sc = base.clone();
                        sc.push(scope);
                        tokens.push(TokenInfo {
                            start: 0,
                            end: line.len(),
                            scopes: sc,
                        });
                        return tokens;
                    }
                }
            }
        }

        while pos < line.len() {
            let remaining = &line[pos..];
            if let Some((info, adv)) = try_match_legacy(
                &self.grammar.patterns,
                remaining,
                pos,
                &base,
                self.grammar,
                0,
            ) {
                tokens.extend(info);
                pos += adv;
            } else {
                let ne = (pos + 1).min(line.len());
                tokens.push(TokenInfo {
                    start: pos,
                    end: ne,
                    scopes: base.clone(),
                });
                pos = ne;
            }
        }
        merge_legacy(&mut tokens);
        tokens
    }
}

fn resolve_inc(grammar: &TextMateGrammar, inc: &IncludeRef) -> Vec<Pattern> {
    match inc {
        IncludeRef::SelfRef | IncludeRef::BaseRef => grammar.patterns.clone(),
        IncludeRef::Repository(name) => grammar
            .repository
            .get(name)
            .map_or_else(Vec::new, |r| r.patterns.clone()),
        IncludeRef::External(_) => Vec::new(),
    }
}

fn find_first_begin_end(patterns: &[Pattern], _idx: usize) -> Option<&BeginEndRule> {
    patterns.iter().find_map(|p| {
        if let Pattern::BeginEnd(r) = p {
            Some(r)
        } else {
            None
        }
    })
}

fn try_match_legacy(
    patterns: &[Pattern],
    text: &str,
    offset: usize,
    base: &[String],
    grammar: &TextMateGrammar,
    depth: usize,
) -> Option<(Vec<TokenInfo>, usize)> {
    if depth > 8 {
        return None;
    }
    let mut best: Option<(usize, Vec<TokenInfo>, usize)> = None;

    for pattern in patterns {
        match pattern {
            Pattern::Match(rule) => {
                if rule.regex.is_empty() {
                    continue;
                }
                let Ok(re) = FancyRegex::new(&rule.regex) else {
                    continue;
                };
                let Some(m) = fancy_find(&re, text) else {
                    continue;
                };
                if m.start() == 0 && m.end() > 0 {
                    let bp = best.as_ref().map_or(usize::MAX, |b| b.0);
                    if m.start() < bp {
                        let mut sc = base.to_vec();
                        if let Some(ref s) = rule.scope {
                            sc.push(s.clone());
                        }
                        best = Some((
                            m.start(),
                            vec![TokenInfo {
                                start: offset,
                                end: offset + m.end(),
                                scopes: sc,
                            }],
                            m.end(),
                        ));
                    }
                }
            }
            Pattern::BeginEnd(rule) => {
                let Ok(re) = FancyRegex::new(&rule.begin) else {
                    continue;
                };
                let Some(m) = fancy_find(&re, text) else {
                    continue;
                };
                if m.start() == 0 && m.end() > 0 {
                    let bp = best.as_ref().map_or(usize::MAX, |b| b.0);
                    if m.start() < bp {
                        let mut sc = base.to_vec();
                        if let Some(ref s) = rule.scope {
                            sc.push(s.clone());
                        }
                        best = Some((
                            m.start(),
                            vec![TokenInfo {
                                start: offset,
                                end: offset + m.end(),
                                scopes: sc,
                            }],
                            m.end(),
                        ));
                    }
                }
            }
            Pattern::BeginWhile(rule) => {
                let Ok(re) = FancyRegex::new(&rule.begin) else {
                    continue;
                };
                let Some(m) = fancy_find(&re, text) else {
                    continue;
                };
                if m.start() == 0 && m.end() > 0 {
                    let bp = best.as_ref().map_or(usize::MAX, |b| b.0);
                    if m.start() < bp {
                        let mut sc = base.to_vec();
                        if let Some(ref s) = rule.scope {
                            sc.push(s.clone());
                        }
                        best = Some((
                            m.start(),
                            vec![TokenInfo {
                                start: offset,
                                end: offset + m.end(),
                                scopes: sc,
                            }],
                            m.end(),
                        ));
                    }
                }
            }
            Pattern::Include(inc) => {
                let ps = resolve_inc(grammar, inc);
                if let Some(r) = try_match_legacy(&ps, text, offset, base, grammar, depth + 1) {
                    let bp = best.as_ref().map_or(usize::MAX, |b| b.0);
                    if 0 < bp {
                        best = Some((0, r.0, r.1));
                    }
                }
            }
        }
    }
    best.map(|(_, i, a)| (i, a))
}

fn merge_legacy(tokens: &mut Vec<TokenInfo>) {
    if tokens.len() < 2 {
        return;
    }
    let mut i = 0;
    while i + 1 < tokens.len() {
        if tokens[i].end == tokens[i + 1].start && tokens[i].scopes == tokens[i + 1].scopes {
            tokens[i].end = tokens[i + 1].end;
            tokens.remove(i + 1);
        } else {
            i += 1;
        }
    }
}

fn merge_compiled(tokens: &mut Vec<Token>) {
    if tokens.len() < 2 {
        return;
    }
    let mut i = 0;
    while i + 1 < tokens.len() {
        if tokens[i].end == tokens[i + 1].start && tokens[i].scopes == tokens[i + 1].scopes {
            tokens[i].end = tokens[i + 1].end;
            tokens.remove(i + 1);
        } else {
            i += 1;
        }
    }
}

/// Convenience wrapper around [`TextMateTokenizer::tokenize_line`].
pub fn tokenize_line(
    grammar: &TextMateGrammar,
    line: &str,
    state: &mut TokenizerState,
) -> Vec<TokenInfo> {
    let tokenizer = TextMateTokenizer::new(grammar);
    tokenizer.tokenize_line(line, state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_grammar() -> TextMateGrammar {
        TextMateGrammar {
            scope_name: "source.test".into(),
            file_types: vec!["test".into()],
            patterns: vec![
                Pattern::Match(MatchRule {
                    regex: r"//.*".into(),
                    scope: Some("comment.line".into()),
                    captures: HashMap::new(),
                }),
                Pattern::Match(MatchRule {
                    regex: r#""[^"]*""#.into(),
                    scope: Some("string.quoted.double".into()),
                    captures: HashMap::new(),
                }),
                Pattern::Match(MatchRule {
                    regex: r"\b(fn|let|if|else|return)\b".into(),
                    scope: Some("keyword.control".into()),
                    captures: HashMap::new(),
                }),
                Pattern::Match(MatchRule {
                    regex: r"\b\d+\b".into(),
                    scope: Some("constant.numeric".into()),
                    captures: HashMap::new(),
                }),
            ],
            repository: HashMap::new(),
        }
    }

    #[test]
    fn tokenize_keywords() {
        let grammar = make_simple_grammar();
        let mut state = TokenizerState::new();
        let tokens = tokenize_line(&grammar, "fn main", &mut state);
        let kw = tokens
            .iter()
            .find(|t| t.scopes.iter().any(|s| s.contains("keyword")));
        assert!(kw.is_some(), "should find a keyword token");
    }

    #[test]
    fn tokenize_comment() {
        let grammar = make_simple_grammar();
        let mut state = TokenizerState::new();
        let tokens = tokenize_line(&grammar, "// hello world", &mut state);
        assert!(!tokens.is_empty());
        let comment = tokens
            .iter()
            .find(|t| t.scopes.iter().any(|s| s.contains("comment")));
        assert!(comment.is_some(), "should find a comment token");
    }

    #[test]
    fn tokenize_string() {
        let grammar = make_simple_grammar();
        let mut state = TokenizerState::new();
        let tokens = tokenize_line(&grammar, r#"let x = "hello""#, &mut state);
        let string_tok = tokens
            .iter()
            .find(|t| t.scopes.iter().any(|s| s.contains("string")));
        assert!(string_tok.is_some(), "should find a string token");
    }

    #[test]
    fn tokenize_number() {
        let grammar = make_simple_grammar();
        let mut state = TokenizerState::new();
        let tokens = tokenize_line(&grammar, "let x = 42", &mut state);
        let num = tokens
            .iter()
            .find(|t| t.scopes.iter().any(|s| s.contains("numeric")));
        assert!(num.is_some(), "should find a numeric token");
    }

    #[test]
    fn empty_line_produces_no_tokens() {
        let grammar = make_simple_grammar();
        let mut state = TokenizerState::new();
        let tokens = tokenize_line(&grammar, "", &mut state);
        assert!(tokens.is_empty());
    }

    #[test]
    fn state_default() {
        let state = TokenizerState::default();
        assert!(state.rule_stack.is_empty());
    }

    #[test]
    fn token_info_fields() {
        let tok = TokenInfo {
            start: 0,
            end: 5,
            scopes: vec!["source.test".into(), "keyword.control".into()],
        };
        assert_eq!(tok.start, 0);
        assert_eq!(tok.end, 5);
        assert_eq!(tok.scopes.len(), 2);
    }

    #[test]
    fn from_json_basic() {
        let json = r#"{ "scopeName": "source.example", "fileTypes": ["ex"],
            "patterns": [{ "match": "\\bif\\b", "name": "keyword.control" }],
            "repository": {} }"#;
        let grammar = TextMateGrammar::from_json(json).unwrap();
        assert_eq!(grammar.scope_name, "source.example");
        assert_eq!(grammar.file_types, vec!["ex"]);
        assert_eq!(grammar.patterns.len(), 1);
    }

    #[test]
    fn include_self_ref() {
        let grammar = TextMateGrammar {
            scope_name: "source.test".into(),
            file_types: vec![],
            patterns: vec![
                Pattern::Match(MatchRule {
                    regex: r"\bfn\b".into(),
                    scope: Some("keyword".into()),
                    captures: HashMap::new(),
                }),
                Pattern::Include(IncludeRef::SelfRef),
            ],
            repository: HashMap::new(),
        };
        let mut state = TokenizerState::new();
        let tokens = tokenize_line(&grammar, "fn", &mut state);
        assert!(!tokens.is_empty());
    }

    #[test]
    fn include_repository() {
        let mut repo = HashMap::new();
        repo.insert(
            "keywords".into(),
            RepositoryRule {
                patterns: vec![Pattern::Match(MatchRule {
                    regex: r"\blet\b".into(),
                    scope: Some("keyword".into()),
                    captures: HashMap::new(),
                })],
            },
        );
        let grammar = TextMateGrammar {
            scope_name: "source.test".into(),
            file_types: vec![],
            patterns: vec![Pattern::Include(IncludeRef::Repository("keywords".into()))],
            repository: repo,
        };
        let mut state = TokenizerState::new();
        let tokens = tokenize_line(&grammar, "let x = 1", &mut state);
        let kw = tokens
            .iter()
            .find(|t| t.scopes.iter().any(|s| s == "keyword"));
        assert!(kw.is_some());
    }

    #[test]
    fn merge_adjacent() {
        let mut tokens = vec![
            TokenInfo {
                start: 0,
                end: 3,
                scopes: vec!["a".into()],
            },
            TokenInfo {
                start: 3,
                end: 6,
                scopes: vec!["a".into()],
            },
            TokenInfo {
                start: 6,
                end: 9,
                scopes: vec!["b".into()],
            },
        ];
        merge_legacy(&mut tokens);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].end, 6);
    }

    #[test]
    fn begin_end_rule_in_grammar() {
        let grammar = TextMateGrammar {
            scope_name: "source.test".into(),
            file_types: vec![],
            patterns: vec![Pattern::BeginEnd(BeginEndRule {
                begin: r#"""#.into(),
                end: r#"""#.into(),
                scope: Some("string.quoted.double".into()),
                begin_captures: HashMap::new(),
                end_captures: HashMap::new(),
                patterns: vec![],
            })],
            repository: HashMap::new(),
        };
        let mut state = TokenizerState::new();
        let tokens = tokenize_line(&grammar, r#""hello""#, &mut state);
        assert!(!tokens.is_empty());
    }

    #[test]
    fn parse_include_variants() {
        assert!(matches!(parse_include("$self"), IncludeRef::SelfRef));
        assert!(matches!(parse_include("$base"), IncludeRef::BaseRef));
        assert!(
            matches!(parse_include("#keywords"), IncludeRef::Repository(ref s) if s == "keywords")
        );
        assert!(
            matches!(parse_include("source.other"), IncludeRef::External(ref s) if s == "source.other")
        );
    }

    #[test]
    fn unsupported_format_error() {
        let result = TextMateGrammar::from_file(Path::new("test.xyz"));
        assert!(result.is_err());
    }

    // -- New tests for RuleStack, Token, TokenizeResult --

    #[test]
    fn rule_stack_push_pop() {
        let mut stack = RuleStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.depth(), 0);

        stack.push("source.rust".into(), 0, None);
        assert_eq!(stack.depth(), 1);

        stack.push("string.quoted".into(), 1, Some(r#"""#.into()));
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.current_scopes(), vec!["source.rust", "string.quoted"]);

        let (scope, idx, end) = stack.pop().unwrap();
        assert_eq!(scope, "string.quoted");
        assert_eq!(idx, 1);
        assert!(end.is_some());
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn rule_stack_default() {
        let stack = RuleStack::default();
        assert!(stack.is_empty());
    }

    #[test]
    fn tokenize_with_rule_stack() {
        let grammar = make_simple_grammar();
        let mut tokenizer = TextMateTokenizer::new(&grammar);
        let state = RuleStack::new();
        let result = tokenizer.tokenize_line_with_stack("fn main", &state);
        assert!(!result.tokens.is_empty());
    }

    #[test]
    fn scope_interning() {
        let grammar = make_simple_grammar();
        let mut tokenizer = TextMateTokenizer::new(&grammar);
        let id1 = tokenizer.intern_scope("source.test");
        let id2 = tokenizer.intern_scope("source.test");
        let id3 = tokenizer.intern_scope("keyword.control");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert_eq!(tokenizer.scope_name(id1), Some("source.test"));
        assert_eq!(tokenizer.scope_name(id3), Some("keyword.control"));
    }

    #[test]
    fn compiled_token_fields() {
        let tok = Token {
            start: 0,
            end: 5,
            scopes: vec![0, 1],
        };
        assert_eq!(tok.start, 0);
        assert_eq!(tok.end, 5);
        assert_eq!(tok.scopes.len(), 2);
    }

    #[test]
    fn merge_adjacent_compiled() {
        let mut tokens = vec![
            Token {
                start: 0,
                end: 3,
                scopes: vec![0],
            },
            Token {
                start: 3,
                end: 6,
                scopes: vec![0],
            },
            Token {
                start: 6,
                end: 9,
                scopes: vec![1],
            },
        ];
        merge_compiled(&mut tokens);
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].end, 6);
    }
}
