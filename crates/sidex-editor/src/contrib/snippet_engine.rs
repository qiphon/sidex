//! Full snippet engine with tab stops, placeholders, choices, variables,
//! transforms, and nested placeholders — mirrors VS Code's snippet engine.

use std::collections::HashMap;

use regex::Regex;
use sidex_text::Range;

/// A fully parsed snippet ready for expansion.
#[derive(Debug, Clone)]
pub struct ParsedSnippet {
    pub parts: Vec<SnippetPart>,
}

/// One element of a parsed snippet template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnippetPart {
    Text(String),
    TabStop(u32),
    Placeholder {
        index: u32,
        default: Vec<SnippetPart>,
    },
    Choice {
        index: u32,
        options: Vec<String>,
    },
    Variable {
        name: String,
        default: Option<Vec<SnippetPart>>,
        transform: Option<Transform>,
    },
    FinalTabStop,
}

/// Regex transform applied to a variable or tab stop value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transform {
    pub regex: String,
    pub replacement: String,
    pub flags: String,
}

/// A resolved tab stop with document ranges and placeholder text.
#[derive(Debug, Clone)]
pub struct TabStop {
    pub index: u32,
    pub ranges: Vec<Range>,
    pub placeholder_text: String,
}

/// Edit operation produced during snippet expansion.
#[derive(Debug, Clone)]
pub struct EditOperation {
    pub range: Range,
    pub text: String,
}

/// An active snippet session tracking state for tab-stop navigation.
#[derive(Debug, Clone)]
pub struct SnippetSession {
    pub snippet: ParsedSnippet,
    pub tab_stops: Vec<TabStop>,
    pub current_tab_stop: usize,
    pub is_active: bool,
    pub applied_edits: Vec<EditOperation>,
}

/// Context for variable resolution and snippet expansion.
#[derive(Debug, Clone, Default)]
pub struct SnippetContext {
    pub filename: String,
    pub filepath: String,
    pub directory: String,
    pub line_index: u32,
    pub line_number: u32,
    pub current_line: String,
    pub current_word: String,
    pub selected_text: String,
    pub clipboard: String,
    pub block_comment_start: String,
    pub block_comment_end: String,
    pub line_comment: String,
}

impl Transform {
    /// Applies the regex transform to the given text.
    #[must_use]
    pub fn apply(&self, text: &str) -> String {
        let Ok(re) = Regex::new(&self.regex) else {
            return text.to_string();
        };
        let global = self.flags.contains('g');
        if global {
            re.replace_all(text, self.replacement.as_str()).into_owned()
        } else {
            re.replace(text, self.replacement.as_str()).into_owned()
        }
    }
}

// ── Parser ───────────────────────────────────────────────────────

/// Parses a VS Code snippet body string into a [`ParsedSnippet`].
///
/// Supports: `$1`, `${1}`, `${1:placeholder}`, `${1|a,b,c|}`,
/// `$VAR`, `${VAR}`, `${VAR:default}`, `${VAR/regex/replace/flags}`,
/// `$0` (final tab stop), nested `${1:${2:inner}}`, and `\\` escapes.
pub fn parse_snippet(body: &str) -> Result<ParsedSnippet, String> {
    let chars: Vec<char> = body.chars().collect();
    let parts = parse_parts(&chars, 0, chars.len())?;
    Ok(ParsedSnippet { parts })
}

fn parse_parts(chars: &[char], start: usize, end: usize) -> Result<Vec<SnippetPart>, String> {
    let mut parts = Vec::new();
    let mut i = start;
    let mut text_buf = String::new();

    while i < end {
        if chars[i] == '\\' && i + 1 < end {
            text_buf.push(chars[i + 1]);
            i += 2;
            continue;
        }

        if chars[i] == '$' {
            if !text_buf.is_empty() {
                parts.push(SnippetPart::Text(text_buf.clone()));
                text_buf.clear();
            }
            i += 1;
            if i >= end {
                break;
            }

            if chars[i] == '{' {
                i += 1;
                let (part, consumed) = parse_braced(chars, i, end)?;
                parts.push(part);
                i += consumed;
            } else if chars[i].is_ascii_digit() {
                let num_start = i;
                while i < end && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let num: u32 = chars[num_start..i]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0);
                if num == 0 {
                    parts.push(SnippetPart::FinalTabStop);
                } else {
                    parts.push(SnippetPart::TabStop(num));
                }
            } else if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
                let name_start = i;
                while i < end && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let name: String = chars[name_start..i].iter().collect();
                parts.push(SnippetPart::Variable {
                    name,
                    default: None,
                    transform: None,
                });
            }
        } else {
            text_buf.push(chars[i]);
            i += 1;
        }
    }

    if !text_buf.is_empty() {
        parts.push(SnippetPart::Text(text_buf));
    }
    Ok(parts)
}

fn parse_braced(
    chars: &[char],
    start: usize,
    end: usize,
) -> Result<(SnippetPart, usize), String> {
    let mut i = start;

    if i < end && chars[i].is_ascii_digit() {
        let num_start = i;
        while i < end && chars[i].is_ascii_digit() {
            i += 1;
        }
        let num: u32 = chars[num_start..i]
            .iter()
            .collect::<String>()
            .parse()
            .unwrap_or(0);

        if i < end && chars[i] == '}' {
            let part = if num == 0 {
                SnippetPart::FinalTabStop
            } else {
                SnippetPart::TabStop(num)
            };
            return Ok((part, i - start + 1));
        }

        if i < end && chars[i] == ':' {
            i += 1;
            let content_start = i;
            let mut depth = 1u32;
            while i < end && depth > 0 {
                if chars[i] == '\\' && i + 1 < end {
                    i += 2;
                    continue;
                }
                if chars[i] == '$' && i + 1 < end && chars[i + 1] == '{' {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if chars[i] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                i += 1;
            }
            let inner = parse_parts(chars, content_start, i)?;
            if i < end && chars[i] == '}' {
                return Ok((
                    SnippetPart::Placeholder {
                        index: num,
                        default: inner,
                    },
                    i - start + 1,
                ));
            }
        }

        if i < end && chars[i] == '|' {
            i += 1;
            let choices_start = i;
            while i < end && !(chars[i] == '|' && i + 1 < end && chars[i + 1] == '}') {
                i += 1;
            }
            let choices_str: String = chars[choices_start..i].iter().collect();
            let options: Vec<String> = choices_str.split(',').map(|s| s.trim().to_string()).collect();
            if i + 1 < end {
                return Ok((
                    SnippetPart::Choice {
                        index: num,
                        options,
                    },
                    i - start + 2,
                ));
            }
        }
    }

    // Variable: ${NAME}, ${NAME:default}, ${NAME/regex/replace/flags}
    if i < end && (chars[i].is_ascii_alphabetic() || chars[i] == '_') {
        let name_start = i;
        while i < end && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
            i += 1;
        }
        let name: String = chars[name_start..i].iter().collect();

        if i < end && chars[i] == '}' {
            return Ok((
                SnippetPart::Variable {
                    name,
                    default: None,
                    transform: None,
                },
                i - start + 1,
            ));
        }

        if i < end && chars[i] == ':' {
            i += 1;
            let content_start = i;
            let mut depth = 1u32;
            while i < end && depth > 0 {
                if chars[i] == '\\' && i + 1 < end {
                    i += 2;
                    continue;
                }
                if chars[i] == '$' && i + 1 < end && chars[i + 1] == '{' {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if chars[i] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                i += 1;
            }
            let default_parts = parse_parts(chars, content_start, i)?;
            if i < end && chars[i] == '}' {
                return Ok((
                    SnippetPart::Variable {
                        name,
                        default: Some(default_parts),
                        transform: None,
                    },
                    i - start + 1,
                ));
            }
        }

        if i < end && chars[i] == '/' {
            if let Some((transform, consumed)) = parse_transform(chars, i, end) {
                return Ok((
                    SnippetPart::Variable {
                        name,
                        default: None,
                        transform: Some(transform),
                    },
                    (i + consumed) - start,
                ));
            }
        }
    }

    Err(format!("invalid snippet syntax near position {start}"))
}

fn parse_transform(chars: &[char], start: usize, end: usize) -> Option<(Transform, usize)> {
    if start >= end || chars[start] != '/' {
        return None;
    }
    let mut i = start + 1;

    let regex_start = i;
    while i < end && chars[i] != '/' {
        if chars[i] == '\\' && i + 1 < end {
            i += 2;
            continue;
        }
        i += 1;
    }
    if i >= end {
        return None;
    }
    let regex_str: String = chars[regex_start..i].iter().collect();
    i += 1; // skip /

    let replace_start = i;
    while i < end && chars[i] != '/' {
        if chars[i] == '\\' && i + 1 < end {
            i += 2;
            continue;
        }
        i += 1;
    }
    if i >= end {
        return None;
    }
    let replacement: String = chars[replace_start..i].iter().collect();
    i += 1; // skip /

    let flags_start = i;
    while i < end && chars[i] != '}' {
        i += 1;
    }
    let flags: String = chars[flags_start..i].iter().collect();
    if i < end && chars[i] == '}' {
        i += 1; // skip }
    }

    Some((
        Transform {
            regex: regex_str,
            replacement,
            flags,
        },
        i - start,
    ))
}

// ── Variable resolution ──────────────────────────────────────────

/// Resolves a snippet variable to its value given the current context.
#[must_use]
pub fn resolve_variable(name: &str, context: &SnippetContext) -> Option<String> {
    match name {
        "TM_FILENAME" => Some(
            context
                .filepath
                .rsplit('/')
                .next()
                .unwrap_or(&context.filename)
                .to_string(),
        ),
        "TM_FILENAME_BASE" => {
            let fname = context.filepath.rsplit('/').next().unwrap_or(&context.filename);
            Some(
                fname
                    .rsplit_once('.')
                    .map_or(fname, |(base, _)| base)
                    .to_string(),
            )
        }
        "TM_FILEPATH" => Some(context.filepath.clone()),
        "TM_DIRECTORY" => Some(context.directory.clone()),
        "TM_LINE_INDEX" => Some(context.line_index.to_string()),
        "TM_LINE_NUMBER" => Some(context.line_number.to_string()),
        "TM_CURRENT_LINE" => Some(context.current_line.clone()),
        "TM_CURRENT_WORD" => Some(context.current_word.clone()),
        "TM_SELECTED_TEXT" => Some(context.selected_text.clone()),
        "CLIPBOARD" => Some(context.clipboard.clone()),
        "CURRENT_YEAR" => Some("2026".into()),
        "CURRENT_YEAR_SHORT" => Some("26".into()),
        "CURRENT_MONTH" => Some("04".into()),
        "CURRENT_MONTH_NAME" => Some("April".into()),
        "CURRENT_MONTH_NAME_SHORT" => Some("Apr".into()),
        "CURRENT_DATE" => Some("16".into()),
        "CURRENT_DAY_NAME" => Some("Thursday".into()),
        "CURRENT_DAY_NAME_SHORT" => Some("Thu".into()),
        "CURRENT_HOUR" => Some("12".into()),
        "CURRENT_MINUTE" => Some("00".into()),
        "CURRENT_SECOND" => Some("00".into()),
        "CURRENT_SECONDS_UNIX" => Some("0".into()),
        "RANDOM" => Some(format!("{:06}", 123_456u32)),
        "RANDOM_HEX" => Some(format!("{:06x}", 0xab_cdefu32)),
        "UUID" => Some("00000000-0000-0000-0000-000000000000".into()),
        "BLOCK_COMMENT_START" => Some(context.block_comment_start.clone()),
        "BLOCK_COMMENT_END" => Some(context.block_comment_end.clone()),
        "LINE_COMMENT" => Some(context.line_comment.clone()),
        _ => None,
    }
}

// ── Expansion ────────────────────────────────────────────────────

/// Expands a parsed snippet using the given context, producing a
/// `SnippetSession` with resolved text and tab stop positions.
#[must_use]
pub fn expand_snippet(snippet: &ParsedSnippet, context: &SnippetContext) -> SnippetSession {
    let mut text = String::new();
    let mut tab_stop_offsets: HashMap<u32, Vec<(usize, usize, String)>> = HashMap::new();
    let mut has_final = false;

    expand_parts(
        &snippet.parts,
        context,
        &mut text,
        &mut tab_stop_offsets,
        &mut has_final,
    );

    let mut tab_stops: Vec<TabStop> = tab_stop_offsets
        .into_iter()
        .map(|(idx, ranges)| {
            let placeholder = ranges.first().map_or(String::new(), |(_, _, t)| t.clone());
            let rs = ranges
                .into_iter()
                .map(|(s, e, _)| {
                    Range::new(
                        sidex_text::Position::new(0, s as u32),
                        sidex_text::Position::new(0, e as u32),
                    )
                })
                .collect();
            TabStop {
                index: idx,
                ranges: rs,
                placeholder_text: placeholder,
            }
        })
        .collect();
    tab_stops.sort_by_key(|ts| if ts.index == 0 { u32::MAX } else { ts.index });

    let edit = EditOperation {
        range: Range::new(
            sidex_text::Position::new(0, 0),
            sidex_text::Position::new(0, 0),
        ),
        text: text.clone(),
    };

    SnippetSession {
        snippet: snippet.clone(),
        tab_stops,
        current_tab_stop: 0,
        is_active: true,
        applied_edits: vec![edit],
    }
}

fn expand_parts(
    parts: &[SnippetPart],
    context: &SnippetContext,
    text: &mut String,
    offsets: &mut HashMap<u32, Vec<(usize, usize, String)>>,
    has_final: &mut bool,
) {
    for part in parts {
        match part {
            SnippetPart::Text(t) => text.push_str(t),
            SnippetPart::TabStop(n) => {
                let pos = text.chars().count();
                offsets.entry(*n).or_default().push((pos, pos, String::new()));
            }
            SnippetPart::FinalTabStop => {
                let pos = text.chars().count();
                *has_final = true;
                offsets.entry(0).or_default().push((pos, pos, String::new()));
            }
            SnippetPart::Placeholder { index, default } => {
                let start = text.chars().count();
                let mut placeholder_text = String::new();
                expand_parts(default, context, &mut placeholder_text, offsets, has_final);
                text.push_str(&placeholder_text);
                let end = text.chars().count();
                offsets
                    .entry(*index)
                    .or_default()
                    .push((start, end, placeholder_text));
            }
            SnippetPart::Choice { index, options } => {
                let first = options.first().map_or("", |s| s.as_str());
                let start = text.chars().count();
                text.push_str(first);
                let end = text.chars().count();
                offsets
                    .entry(*index)
                    .or_default()
                    .push((start, end, first.to_string()));
            }
            SnippetPart::Variable {
                name,
                default,
                transform,
            } => {
                let value = resolve_variable(name, context);
                match (value, default, transform) {
                    (Some(val), _, Some(tf)) => text.push_str(&tf.apply(&val)),
                    (Some(val), _, None) => text.push_str(&val),
                    (None, Some(def), _) => {
                        expand_parts(def, context, text, offsets, has_final);
                    }
                    (None, None, _) => {}
                }
            }
        }
    }
}

impl SnippetSession {
    /// Advances to the next tab stop. Returns `false` if the session is done.
    pub fn next_tab_stop(&mut self) -> bool {
        if !self.is_active || self.tab_stops.is_empty() {
            self.is_active = false;
            return false;
        }
        if self.current_tab_stop + 1 < self.tab_stops.len() {
            self.current_tab_stop += 1;
            true
        } else {
            self.is_active = false;
            false
        }
    }

    /// Goes back to the previous tab stop.
    pub fn prev_tab_stop(&mut self) -> bool {
        if !self.is_active || self.current_tab_stop == 0 {
            return false;
        }
        self.current_tab_stop -= 1;
        true
    }

    /// Returns the current tab stop, if any.
    #[must_use]
    pub fn current(&self) -> Option<&TabStop> {
        if self.is_active {
            self.tab_stops.get(self.current_tab_stop)
        } else {
            None
        }
    }

    /// Finishes the session, jumping to the final tab stop.
    pub fn finish(&mut self) {
        self.is_active = false;
    }

    /// Returns linked ranges for the current tab stop (same-numbered).
    #[must_use]
    pub fn linked_ranges(&self) -> Vec<Range> {
        self.current()
            .map(|ts| ts.ranges.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_tabstops() {
        let s = parse_snippet("hello $1 world $0").unwrap();
        assert_eq!(s.parts.len(), 4);
        assert_eq!(s.parts[0], SnippetPart::Text("hello ".into()));
        assert_eq!(s.parts[1], SnippetPart::TabStop(1));
        assert_eq!(s.parts[2], SnippetPart::Text(" world ".into()));
        assert_eq!(s.parts[3], SnippetPart::FinalTabStop);
    }

    #[test]
    fn parse_placeholder() {
        let s = parse_snippet("${1:name}").unwrap();
        assert_eq!(s.parts.len(), 1);
        match &s.parts[0] {
            SnippetPart::Placeholder { index, default } => {
                assert_eq!(*index, 1);
                assert_eq!(default.len(), 1);
                assert_eq!(default[0], SnippetPart::Text("name".into()));
            }
            _ => panic!("expected placeholder"),
        }
    }

    #[test]
    fn parse_choice() {
        let s = parse_snippet("${1|yes,no|}").unwrap();
        match &s.parts[0] {
            SnippetPart::Choice { index, options } => {
                assert_eq!(*index, 1);
                assert_eq!(options, &["yes", "no"]);
            }
            _ => panic!("expected choice"),
        }
    }

    #[test]
    fn parse_nested_placeholder() {
        let s = parse_snippet("${1:${2:inner}}").unwrap();
        match &s.parts[0] {
            SnippetPart::Placeholder { index, default } => {
                assert_eq!(*index, 1);
                match &default[0] {
                    SnippetPart::Placeholder {
                        index: inner_idx,
                        default: inner_def,
                    } => {
                        assert_eq!(*inner_idx, 2);
                        assert_eq!(inner_def[0], SnippetPart::Text("inner".into()));
                    }
                    _ => panic!("expected nested placeholder"),
                }
            }
            _ => panic!("expected placeholder"),
        }
    }

    #[test]
    fn parse_variable() {
        let s = parse_snippet("$TM_FILENAME").unwrap();
        match &s.parts[0] {
            SnippetPart::Variable { name, .. } => assert_eq!(name, "TM_FILENAME"),
            _ => panic!("expected variable"),
        }
    }

    #[test]
    fn parse_variable_with_default() {
        let s = parse_snippet("${TM_SELECTED_TEXT:nothing}").unwrap();
        match &s.parts[0] {
            SnippetPart::Variable {
                name, default: Some(d), ..
            } => {
                assert_eq!(name, "TM_SELECTED_TEXT");
                assert_eq!(d[0], SnippetPart::Text("nothing".into()));
            }
            _ => panic!("expected variable with default"),
        }
    }

    #[test]
    fn parse_variable_transform() {
        let s = parse_snippet("${TM_FILENAME/(.*)/${1}/g}").unwrap();
        match &s.parts[0] {
            SnippetPart::Variable {
                name,
                transform: Some(t),
                ..
            } => {
                assert_eq!(name, "TM_FILENAME");
                assert_eq!(t.regex, "(.*)");
                assert_eq!(t.replacement, "${1}");
                assert_eq!(t.flags, "g");
            }
            _ => panic!("expected variable with transform"),
        }
    }

    #[test]
    fn parse_escaped_dollar() {
        let s = parse_snippet("\\$1 literal").unwrap();
        assert_eq!(s.parts.len(), 1);
        assert_eq!(s.parts[0], SnippetPart::Text("$1 literal".into()));
    }

    #[test]
    fn resolve_variables_basic() {
        let ctx = SnippetContext {
            filepath: "/src/main.rs".into(),
            line_number: 42,
            ..Default::default()
        };
        assert_eq!(resolve_variable("TM_FILENAME", &ctx), Some("main.rs".into()));
        assert_eq!(resolve_variable("TM_FILENAME_BASE", &ctx), Some("main".into()));
        assert_eq!(resolve_variable("TM_LINE_NUMBER", &ctx), Some("42".into()));
    }

    #[test]
    fn expand_basic_snippet() {
        let s = parse_snippet("for ${1:i} in ${2:iter} {\n\t$0\n}").unwrap();
        let ctx = SnippetContext::default();
        let session = expand_snippet(&s, &ctx);
        assert!(session.is_active);
        assert!(!session.tab_stops.is_empty());
        assert!(session.applied_edits[0].text.contains("for "));
    }

    #[test]
    fn session_navigation() {
        let s = parse_snippet("${1:a} ${2:b} $0").unwrap();
        let ctx = SnippetContext::default();
        let mut session = expand_snippet(&s, &ctx);
        assert!(session.is_active);
        assert!(session.next_tab_stop());
        assert!(session.next_tab_stop());
        assert!(!session.next_tab_stop());
        assert!(!session.is_active);
    }

    #[test]
    fn session_prev_navigation() {
        let s = parse_snippet("${1:a} ${2:b} $0").unwrap();
        let ctx = SnippetContext::default();
        let mut session = expand_snippet(&s, &ctx);
        session.next_tab_stop();
        assert!(session.prev_tab_stop());
        assert_eq!(session.current_tab_stop, 0);
    }

    #[test]
    fn transform_apply() {
        let t = Transform {
            regex: r"(\w+)".into(),
            replacement: "[$0]".into(),
            flags: String::new(),
        };
        let result = t.apply("hello");
        assert!(result.contains('['));
    }
}
