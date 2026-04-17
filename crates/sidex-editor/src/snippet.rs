//! Snippet engine for VS Code-style snippet insertion with tabstops,
//! placeholders, choice lists, variable expansion, and linked tabstop groups.

use std::collections::HashMap;

use crate::document::Document;
use crate::selection::Selection;

/// A parsed snippet ready for insertion.
#[derive(Debug, Clone)]
pub struct Snippet {
    /// The parts that make up this snippet, in order.
    pub parts: Vec<SnippetPart>,
}

/// One fragment of a parsed snippet template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnippetPart {
    /// Literal text to insert as-is.
    Text(String),
    /// A tabstop: `$1` or `${1}`.
    Tabstop(u32),
    /// A tabstop with a default placeholder: `${1:placeholder}`.
    Placeholder(u32, String),
    /// A tabstop with a choice list: `${1|choice1,choice2|}`.
    Choice(u32, Vec<String>),
    /// A variable reference: `$TM_FILENAME` or `${TM_SELECTED_TEXT}`.
    Variable(String),
}

/// An active snippet insertion session, tracking the current tabstop index
/// and the positions of each tabstop group in the document.
#[derive(Debug, Clone)]
pub struct SnippetSession {
    /// Ordered, unique tabstop numbers found in the snippet (ascending).
    tabstop_order: Vec<u32>,
    /// Index into `tabstop_order` for the currently active tabstop.
    current_idx: usize,
    /// Map from tabstop number to all document ranges where it appears.
    tabstop_positions: HashMap<u32, Vec<Selection>>,
    /// Whether the session has finished.
    pub finished: bool,
}

impl SnippetSession {
    /// Starts a snippet session by parsing `template`, expanding it into
    /// `document` at the primary cursor, and initialising tabstop tracking.
    pub fn start(document: &mut Document, template: &str) -> Self {
        let snippet = parse_snippet(template);
        let pos = document.cursors.primary().position();
        let insert_offset = document.buffer.position_to_offset(pos);

        let mut text = String::new();
        let mut tabstop_offsets: HashMap<u32, Vec<(usize, usize)>> = HashMap::new();
        let mut current_char = 0usize;

        for part in &snippet.parts {
            match part {
                SnippetPart::Text(t) => {
                    text.push_str(t);
                    current_char += t.chars().count();
                }
                SnippetPart::Tabstop(n) => {
                    tabstop_offsets
                        .entry(*n)
                        .or_default()
                        .push((current_char, current_char));
                }
                SnippetPart::Placeholder(n, default) => {
                    let start = current_char;
                    text.push_str(default);
                    current_char += default.chars().count();
                    tabstop_offsets
                        .entry(*n)
                        .or_default()
                        .push((start, current_char));
                }
                SnippetPart::Choice(n, choices) => {
                    let first = choices.first().map_or("", |s| s.as_str());
                    let start = current_char;
                    text.push_str(first);
                    current_char += first.chars().count();
                    tabstop_offsets
                        .entry(*n)
                        .or_default()
                        .push((start, current_char));
                }
                SnippetPart::Variable(name) => {
                    let value = resolve_variable(name);
                    text.push_str(&value);
                    current_char += value.chars().count();
                }
            }
        }

        // Delete any selection then insert the expanded text.
        let sel = document.cursors.primary().selection;
        let sel_start = document.buffer.position_to_offset(sel.start());
        let sel_end = document.buffer.position_to_offset(sel.end());
        if sel_start < sel_end {
            document.buffer.remove(sel_start..sel_end);
        }
        document.buffer.insert(insert_offset, &text);

        // Convert char offsets to document positions.
        let mut tabstop_positions: HashMap<u32, Vec<Selection>> = HashMap::new();
        for (n, ranges) in &tabstop_offsets {
            let sels: Vec<Selection> = ranges
                .iter()
                .map(|&(s, e)| {
                    let start_pos = document.buffer.offset_to_position(insert_offset + s);
                    let end_pos = document.buffer.offset_to_position(insert_offset + e);
                    Selection::new(start_pos, end_pos)
                })
                .collect();
            tabstop_positions.insert(*n, sels);
        }

        let mut tabstop_order: Vec<u32> = tabstop_offsets.keys().copied().collect();
        tabstop_order.sort_unstable();
        // $0 is always last (final cursor position).
        if let Some(zero_pos) = tabstop_order.iter().position(|&n| n == 0) {
            tabstop_order.remove(zero_pos);
            tabstop_order.push(0);
        }

        let session = Self {
            tabstop_order,
            current_idx: 0,
            tabstop_positions,
            finished: false,
        };

        session.select_current(document);
        session
    }

    /// Advances to the next tabstop (Tab key).
    pub fn next_tabstop(&mut self, document: &mut Document) {
        if self.finished {
            return;
        }
        if self.current_idx + 1 < self.tabstop_order.len() {
            self.current_idx += 1;
            self.select_current(document);
        } else {
            self.finish(document);
        }
    }

    /// Moves back to the previous tabstop (Shift+Tab).
    pub fn prev_tabstop(&mut self, document: &mut Document) {
        if self.finished || self.current_idx == 0 {
            return;
        }
        self.current_idx -= 1;
        self.select_current(document);
    }

    /// Returns the tabstop number that is currently active.
    #[must_use]
    pub fn current_tabstop_number(&self) -> u32 {
        self.tabstop_order
            .get(self.current_idx)
            .copied()
            .unwrap_or(0)
    }

    /// Exits snippet mode, placing the cursor at tabstop `$0` if defined,
    /// otherwise at the end of the inserted text.
    pub fn finish(&mut self, document: &mut Document) {
        self.finished = true;
        if let Some(sels) = self.tabstop_positions.get(&0) {
            if let Some(sel) = sels.first() {
                document
                    .cursors
                    .set_primary_selection(Selection::caret(sel.start()));
            }
        }
    }

    fn select_current(&self, document: &mut Document) {
        if let Some(&n) = self.tabstop_order.get(self.current_idx) {
            if let Some(sels) = self.tabstop_positions.get(&n) {
                if let Some(sel) = sels.first() {
                    document.cursors.set_primary_selection(*sel);
                }
            }
        }
    }
}

fn resolve_variable(name: &str) -> String {
    match name {
        "TM_FILENAME" => "filename".into(),
        "TM_LINE_INDEX" => "0".into(),
        "TM_LINE_NUMBER" => "1".into(),
        _ => String::new(),
    }
}

// ── Snippet parser ────────────────────────────────────────────────

/// Parses a VS Code snippet template string into a [`Snippet`].
///
/// Supports: `$1`, `${1:placeholder}`, `${1|choice1,choice2|}`,
/// `$VARIABLE`, `${VARIABLE}`.
pub fn parse_snippet(template: &str) -> Snippet {
    let chars: Vec<char> = template.chars().collect();
    let len = chars.len();
    let mut parts = Vec::new();
    let mut i = 0;
    let mut text_buf = String::new();

    while i < len {
        if chars[i] == '\\' && i + 1 < len {
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
            if i >= len {
                break;
            }

            if chars[i] == '{' {
                i += 1;
                if let Some((part, consumed)) = parse_braced(&chars, i) {
                    parts.push(part);
                    i += consumed;
                }
            } else if chars[i].is_ascii_digit() {
                let start = i;
                while i < len && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let num: u32 = chars[start..i]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0);
                parts.push(SnippetPart::Tabstop(num));
            } else if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
                let start = i;
                while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                parts.push(SnippetPart::Variable(name));
            }
        } else {
            text_buf.push(chars[i]);
            i += 1;
        }
    }

    if !text_buf.is_empty() {
        parts.push(SnippetPart::Text(text_buf));
    }

    Snippet { parts }
}

fn parse_braced(chars: &[char], start: usize) -> Option<(SnippetPart, usize)> {
    let len = chars.len();
    let mut i = start;

    // Check if it starts with digits (tabstop).
    if i < len && chars[i].is_ascii_digit() {
        let num_start = i;
        while i < len && chars[i].is_ascii_digit() {
            i += 1;
        }
        let num: u32 = chars[num_start..i]
            .iter()
            .collect::<String>()
            .parse()
            .unwrap_or(0);

        if i < len && chars[i] == '}' {
            return Some((SnippetPart::Tabstop(num), i - start + 1));
        }

        if i < len && chars[i] == ':' {
            i += 1;
            let placeholder_start = i;
            let mut depth = 1;
            while i < len && depth > 0 {
                if chars[i] == '{' {
                    depth += 1;
                } else if chars[i] == '}' {
                    depth -= 1;
                }
                if depth > 0 {
                    i += 1;
                }
            }
            let placeholder: String = chars[placeholder_start..i].iter().collect();
            if i < len && chars[i] == '}' {
                return Some((SnippetPart::Placeholder(num, placeholder), i - start + 1));
            }
        }

        if i < len && chars[i] == '|' {
            i += 1;
            let choices_start = i;
            while i < len && !(chars[i] == '|' && i + 1 < len && chars[i + 1] == '}') {
                i += 1;
            }
            let choices_str: String = chars[choices_start..i].iter().collect();
            let choices: Vec<String> = choices_str.split(',').map(ToString::to_string).collect();
            if i + 1 < len {
                return Some((SnippetPart::Choice(num, choices), i - start + 2));
            }
        }
    }

    // Variable: ${NAME}
    if i < len && (chars[i].is_ascii_alphabetic() || chars[i] == '_') {
        let name_start = i;
        while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
            i += 1;
        }
        if i < len && chars[i] == '}' {
            let name: String = chars[name_start..i].iter().collect();
            return Some((SnippetPart::Variable(name), i - start + 1));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use sidex_text::Position;

    use super::*;

    #[test]
    fn parse_simple_tabstop() {
        let s = parse_snippet("hello $1 world $0");
        assert_eq!(s.parts.len(), 4);
        assert_eq!(s.parts[0], SnippetPart::Text("hello ".into()));
        assert_eq!(s.parts[1], SnippetPart::Tabstop(1));
        assert_eq!(s.parts[2], SnippetPart::Text(" world ".into()));
        assert_eq!(s.parts[3], SnippetPart::Tabstop(0));
    }

    #[test]
    fn parse_placeholder() {
        let s = parse_snippet("${1:name}");
        assert_eq!(s.parts.len(), 1);
        assert_eq!(s.parts[0], SnippetPart::Placeholder(1, "name".into()));
    }

    #[test]
    fn parse_choice() {
        let s = parse_snippet("${1|yes,no|}");
        assert_eq!(s.parts.len(), 1);
        assert_eq!(
            s.parts[0],
            SnippetPart::Choice(1, vec!["yes".into(), "no".into()])
        );
    }

    #[test]
    fn parse_variable() {
        let s = parse_snippet("$TM_FILENAME");
        assert_eq!(s.parts.len(), 1);
        assert_eq!(s.parts[0], SnippetPart::Variable("TM_FILENAME".into()));
    }

    #[test]
    fn parse_braced_variable() {
        let s = parse_snippet("${TM_SELECTED_TEXT}");
        assert_eq!(s.parts.len(), 1);
        assert_eq!(s.parts[0], SnippetPart::Variable("TM_SELECTED_TEXT".into()));
    }

    #[test]
    fn parse_mixed() {
        let s = parse_snippet("for (${1:i} = 0; $1 < ${2:n}; $1++) {\n\t$0\n}");
        assert!(s.parts.len() >= 5);
    }

    #[test]
    fn parse_escaped_dollar() {
        let s = parse_snippet("\\$1 literal");
        assert_eq!(s.parts.len(), 1);
        assert_eq!(s.parts[0], SnippetPart::Text("$1 literal".into()));
    }

    #[test]
    fn session_start_simple() {
        let mut doc = Document::from_str("hello ");
        doc.cursors = crate::multi_cursor::MultiCursor::new(Position::new(0, 6));
        let session = SnippetSession::start(&mut doc, "world$0");
        assert!(doc.text().contains("world"));
        assert!(!session.finished);
    }

    #[test]
    fn session_tabstop_navigation() {
        let mut doc = Document::from_str("");
        doc.cursors = crate::multi_cursor::MultiCursor::new(Position::new(0, 0));
        let mut session = SnippetSession::start(&mut doc, "${1:first} ${2:second}$0");
        assert!(!session.finished);

        session.next_tabstop(&mut doc); // $1 -> $2
        assert!(!session.finished);

        session.next_tabstop(&mut doc); // $2 -> $0
        assert!(!session.finished);

        session.next_tabstop(&mut doc); // past $0 -> finish
        assert!(session.finished);
    }

    #[test]
    fn session_prev_tabstop() {
        let mut doc = Document::from_str("");
        doc.cursors = crate::multi_cursor::MultiCursor::new(Position::new(0, 0));
        let mut session = SnippetSession::start(&mut doc, "${1:a} ${2:b}$0");

        session.next_tabstop(&mut doc);
        session.prev_tabstop(&mut doc);
        assert_eq!(session.current_idx, 0);
    }

    #[test]
    fn session_finish() {
        let mut doc = Document::from_str("");
        doc.cursors = crate::multi_cursor::MultiCursor::new(Position::new(0, 0));
        let mut session = SnippetSession::start(&mut doc, "test$0");
        session.finish(&mut doc);
        assert!(session.finished);
    }
}
