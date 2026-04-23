//! JSON with Comments (JSONC) parser — strips `//`, `/* */` comments and
//! trailing commas before delegating to `serde_json`.

use anyhow::{Context, Result};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone)]
pub struct JsoncError {
    pub line: u32,
    pub column: u32,
    pub message: String,
}

impl fmt::Display for JsoncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.column, self.message)
    }
}
impl std::error::Error for JsoncError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
}

#[derive(Debug, Clone)]
pub struct JsoncComment {
    pub line: u32,
    pub text: String,
    pub kind: CommentKind,
}

/// Parse a JSONC string into a `serde_json::Value`.
pub fn parse_jsonc(input: &str) -> Result<Value> {
    let stripped = strip_comments(input);
    let cleaned = remove_trailing_commas(&stripped);
    serde_json::from_str(&cleaned).context("failed to parse JSONC")
}

/// Parse JSONC, returning the value and all extracted comments for round-tripping.
pub fn parse_jsonc_with_comments(input: &str) -> Result<(Value, Vec<JsoncComment>)> {
    Ok((parse_jsonc(input)?, extract_comments(input)))
}

/// Remove comments but preserve line numbers (replace comment chars with spaces).
pub fn strip_comments(input: &str) -> String {
    let b = input.as_bytes();
    let (len, mut out, mut i) = (b.len(), String::with_capacity(b.len()), 0);
    while i < len {
        match b[i] {
            b'"' => {
                out.push('"');
                i += 1;
                i = skip_string(b, i, &mut out);
            }
            b'/' if i + 1 < len && b[i + 1] == b'/' => {
                i += 2;
                while i < len && b[i] != b'\n' {
                    out.push(' ');
                    i += 1;
                }
            }
            b'/' if i + 1 < len && b[i + 1] == b'*' => {
                i += 2;
                let mut depth: u32 = 1;
                while i < len && depth > 0 {
                    if i + 1 < len && b[i] == b'/' && b[i + 1] == b'*' {
                        depth += 1;
                        out.push_str("  ");
                        i += 2;
                    } else if i + 1 < len && b[i] == b'*' && b[i + 1] == b'/' {
                        depth -= 1;
                        out.push_str("  ");
                        i += 2;
                    } else {
                        out.push(if b[i] == b'\n' { '\n' } else { ' ' });
                        i += 1;
                    }
                }
            }
            _ => {
                out.push(b[i] as char);
                i += 1;
            }
        }
    }
    out
}

/// Pretty-print a `serde_json::Value` as JSONC-compatible JSON.
pub fn format_jsonc(value: &Value, indent: u32) -> String {
    fmt_val(value, &" ".repeat(indent as usize), 0)
}

/// Edit a value at the given key path inside a JSONC document, preserving formatting/comments.
pub fn modify_jsonc(input: &str, path: &[&str], value: &Value) -> Result<String> {
    if path.is_empty() {
        return Ok(format_jsonc(value, 2));
    }
    let mut root = parse_jsonc(input)?;
    let mut target = &mut root;
    for (i, key) in path.iter().enumerate() {
        if i == path.len() - 1 {
            match target {
                Value::Object(m) => {
                    m.insert((*key).to_string(), value.clone());
                }
                _ => anyhow::bail!("path element '{key}' is not an object"),
            }
        } else {
            target = match target {
                Value::Object(m) => m
                    .entry((*key).to_string())
                    .or_insert_with(|| Value::Object(serde_json::Map::new())),
                _ => anyhow::bail!("path element '{key}' is not an object"),
            };
        }
    }
    let needle = format!("\"{}\"", path.last().unwrap());
    if let Some(kp) = find_key_in_src(input, &needle) {
        let colon = input[kp + needle.len()..].find(':').unwrap() + kp + needle.len();
        let (vs, ve) = value_span(input, colon + 1);
        let ser = serde_json::to_string(value).unwrap();
        let mut r = String::with_capacity(input.len() + ser.len());
        r.push_str(&input[..vs]);
        r.push_str(&ser);
        r.push_str(&input[ve..]);
        Ok(r)
    } else {
        Ok(format_jsonc(&root, 2))
    }
}

// --- helpers ---

fn skip_string(b: &[u8], mut i: usize, out: &mut String) -> usize {
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            out.push(b[i] as char);
            out.push(b[i + 1] as char);
            i += 2;
        } else if b[i] == b'"' {
            out.push('"');
            i += 1;
            break;
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    i
}

fn fmt_val(v: &Value, ind: &str, d: usize) -> String {
    match v {
        Value::Object(m) if m.is_empty() => "{}".into(),
        Value::Array(a) if a.is_empty() => "[]".into(),
        Value::Object(m) => {
            let ip = ind.repeat(d + 1);
            let op = ind.repeat(d);
            let e: Vec<String> = m
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{ip}{}: {}",
                        serde_json::to_string(k).unwrap(),
                        fmt_val(v, ind, d + 1)
                    )
                })
                .collect();
            format!("{{\n{}\n{op}}}", e.join(",\n"))
        }
        Value::Array(a) => {
            let ip = ind.repeat(d + 1);
            let op = ind.repeat(d);
            let e: Vec<String> = a
                .iter()
                .map(|v| format!("{ip}{}", fmt_val(v, ind, d + 1)))
                .collect();
            format!("[\n{}\n{op}]", e.join(",\n"))
        }
        _ => serde_json::to_string(v).unwrap(),
    }
}

fn find_key_in_src(input: &str, needle: &str) -> Option<usize> {
    let (b, nb) = (input.as_bytes(), needle.as_bytes());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let s = i;
                i += 1;
                while i < b.len() {
                    if b[i] == b'\\' && i + 1 < b.len() {
                        i += 2;
                    } else if b[i] == b'"' {
                        i += 1;
                        if &b[s..i] == nb {
                            return Some(s);
                        }
                        break;
                    } else {
                        i += 1;
                    }
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'/' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < b.len() && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < b.len() {
                    i += 2;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    None
}

fn value_span(input: &str, after: usize) -> (usize, usize) {
    let bytes = input.as_bytes();
    let mut pos = after;
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    let start = pos;
    match bytes.get(pos) {
        Some(b'"') => {
            pos += 1;
            while pos < bytes.len() {
                if bytes[pos] == b'\\' && pos + 1 < bytes.len() {
                    pos += 2;
                } else if bytes[pos] == b'"' {
                    return (start, pos + 1);
                } else {
                    pos += 1;
                }
            }
        }
        Some(&ch @ (b'{' | b'[')) => {
            let close = if ch == b'{' { b'}' } else { b']' };
            let mut depth = 1;
            pos += 1;
            while pos < bytes.len() && depth > 0 {
                if bytes[pos] == ch {
                    depth += 1;
                } else if bytes[pos] == close {
                    depth -= 1;
                } else if bytes[pos] == b'"' {
                    pos += 1;
                    while pos < bytes.len() {
                        if bytes[pos] == b'\\' && pos + 1 < bytes.len() {
                            pos += 2;
                        } else if bytes[pos] == b'"' {
                            break;
                        } else {
                            pos += 1;
                        }
                    }
                }
                pos += 1;
            }
            return (start, pos);
        }
        _ => {
            while pos < bytes.len() && !matches!(bytes[pos], b',' | b'}' | b']' | b'\n') {
                pos += 1;
            }
            return (start, input[start..pos].trim_end().len() + start);
        }
    }
    (start, input.len())
}

fn extract_comments(input: &str) -> Vec<JsoncComment> {
    let b = input.as_bytes();
    let (len, mut comments, mut i, mut line) = (b.len(), Vec::new(), 0, 1u32);
    while i < len {
        match b[i] {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b'"' => {
                i += 1;
                while i < len {
                    if b[i] == b'\\' && i + 1 < len {
                        i += 2;
                    } else if b[i] == b'"' {
                        i += 1;
                        break;
                    } else {
                        if b[i] == b'\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                }
            }
            b'/' if i + 1 < len && b[i + 1] == b'/' => {
                let sl = line;
                i += 2;
                let ts = i;
                while i < len && b[i] != b'\n' {
                    i += 1;
                }
                comments.push(JsoncComment {
                    line: sl,
                    text: input[ts..i].trim().into(),
                    kind: CommentKind::Line,
                });
            }
            b'/' if i + 1 < len && b[i + 1] == b'*' => {
                let sl = line;
                i += 2;
                let ts = i;
                let mut depth: u32 = 1;
                while i < len && depth > 0 {
                    if i + 1 < len && b[i] == b'/' && b[i + 1] == b'*' {
                        depth += 1;
                        i += 2;
                    } else if i + 1 < len && b[i] == b'*' && b[i + 1] == b'/' {
                        depth -= 1;
                        if depth == 0 {
                            comments.push(JsoncComment {
                                line: sl,
                                text: input[ts..i].trim().into(),
                                kind: CommentKind::Block,
                            });
                        }
                        i += 2;
                    } else {
                        if b[i] == b'\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    comments
}

fn remove_trailing_commas(input: &str) -> String {
    let b = input.as_bytes();
    let (len, mut out, mut i) = (b.len(), String::with_capacity(b.len()), 0);
    while i < len {
        if b[i] == b'"' {
            out.push('"');
            i += 1;
            i = skip_string(b, i, &mut out);
        } else if b[i] == b',' {
            let mut j = i + 1;
            while j < len && matches!(b[j], b' ' | b'\t' | b'\n' | b'\r') {
                j += 1;
            }
            if j < len && matches!(b[j], b']' | b'}') {
                i += 1;
            } else {
                out.push(',');
                i += 1;
            }
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_strips_comments_and_trailing_commas() {
        let v = parse_jsonc("{\n // c\n \"a\": 1,\n /* b */ \"b\": 2,\n}").unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"], 2);
    }
    #[test]
    fn comments_inside_strings_preserved() {
        let v = parse_jsonc(r#"{"u":"https://x.com","n":"/* not */"}"#).unwrap();
        assert_eq!(v["u"], "https://x.com");
        assert_eq!(v["n"], "/* not */");
    }
    #[test]
    fn escaped_quotes() {
        assert_eq!(parse_jsonc(r#"{"k":"v\"al"}"#).unwrap()["k"], "v\"al");
    }
    #[test]
    fn strip_preserves_line_count() {
        let s = "{\n  // comment\n  \"a\": 1\n}";
        assert_eq!(strip_comments(s).lines().count(), s.lines().count());
    }
    #[test]
    fn format_round_trip() {
        let v = parse_jsonc(r#"{"a":1,"b":[2,3]}"#).unwrap();
        assert_eq!(v, parse_jsonc(&format_jsonc(&v, 2)).unwrap());
    }
    #[test]
    fn with_comments_extracts() {
        let (v, c) = parse_jsonc_with_comments("{\n // l\n /* b */\n \"a\":1\n}").unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].kind, CommentKind::Line);
        assert_eq!(c[1].kind, CommentKind::Block);
    }
    #[test]
    fn modify_preserves_comments() {
        let r = modify_jsonc("{\n // keep\n \"f\": 14\n}", &["f"], &Value::from(16)).unwrap();
        assert!(r.contains("// keep"));
        assert_eq!(parse_jsonc(&r).unwrap()["f"], 16);
    }
    #[test]
    fn nested_block_comments() {
        assert_eq!(
            parse_jsonc("{\n /* o /* i */ s */\n \"a\":1\n}").unwrap()["a"],
            1
        );
    }
}
