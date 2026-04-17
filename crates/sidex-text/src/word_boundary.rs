//! Regex-based word boundary detection.
//!
//! Provides word-at-position and word-until-position queries using
//! configurable word definitions (regular expressions). This mirrors
//! VS Code's `WordOperations` module, where each language can supply
//! its own word definition regex.

use regex::Regex;
use std::sync::LazyLock;

use crate::buffer::Buffer;
use crate::Position;

/// A word range found by regex-based word boundary detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordRange {
    /// The matched word text.
    pub word: String,
    /// Start column of the word on its line (0-based char offset).
    pub start_column: u32,
    /// End column of the word on its line (exclusive, 0-based char offset).
    pub end_column: u32,
}

/// Default word definition: letters, digits, underscores, and hyphens.
static DEFAULT_WORD_DEFINITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[-\w]+").expect("default word regex"));

/// Returns the default word definition regex (`[-\w]+`).
pub fn default_word_definition() -> &'static Regex {
    &DEFAULT_WORD_DEFINITION
}

/// Language-specific word definitions.
pub mod language_definitions {
    use regex::Regex;
    use std::sync::LazyLock;

    /// CSS word definition: includes hyphens, at-signs, percent.
    pub static CSS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[-\w@%]+").expect("css word regex"));

    /// HTML word definition: includes hyphens.
    pub static HTML: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[-\w]+").expect("html word regex"));

    /// Shell word definition: includes hyphens, dots, slashes.
    pub static SHELL: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[-.\w/]+").expect("shell word regex"));

    /// Markdown word definition: letters, digits, underscores.
    pub static MARKDOWN: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"[\w]+").expect("markdown word regex"));
}

/// Finds the word at a position using the given word definition regex.
///
/// Returns `None` if the character at `pos` is not part of a word
/// according to `word_definition`.
pub fn get_word_at_position(
    buffer: &Buffer,
    pos: Position,
    word_definition: &Regex,
) -> Option<WordRange> {
    let pos = buffer.validate_position(pos);
    let line_idx = pos.line as usize;
    if line_idx >= buffer.len_lines() {
        return None;
    }
    let content = buffer.line_content(line_idx);
    let col = pos.column as usize;

    find_word_at_column(&content, col, word_definition)
}

/// Finds the word fragment *before* the cursor for autocomplete prefix detection.
///
/// If the cursor is inside or at the end of a word, returns the portion
/// from the word start up to (but not beyond) the cursor column.
/// If not on a word, returns an empty `WordRange` at the cursor position.
pub fn get_word_until_position(
    buffer: &Buffer,
    pos: Position,
    word_definition: &Regex,
) -> WordRange {
    let pos = buffer.validate_position(pos);
    let line_idx = pos.line as usize;
    if line_idx >= buffer.len_lines() {
        return WordRange {
            word: String::new(),
            start_column: pos.column,
            end_column: pos.column,
        };
    }
    let content = buffer.line_content(line_idx);
    let col = pos.column as usize;

    let word = find_word_at_column(&content, col, word_definition).or_else(|| {
        if col > 0 {
            find_word_at_column(&content, col - 1, word_definition)
                .filter(|w| w.end_column as usize == col)
        } else {
            None
        }
    });

    match word {
        Some(w) if w.start_column <= pos.column && w.end_column >= pos.column => {
            let prefix_end = pos.column.min(w.end_column);
            let start = w.start_column as usize;
            let end = prefix_end as usize;
            let word_text: String = content.chars().skip(start).take(end - start).collect();
            WordRange {
                word: word_text,
                start_column: w.start_column,
                end_column: prefix_end,
            }
        }
        _ => WordRange {
            word: String::new(),
            start_column: pos.column,
            end_column: pos.column,
        },
    }
}

fn find_word_at_column(line: &str, col: usize, word_def: &Regex) -> Option<WordRange> {
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() || col > chars.len() {
        return None;
    }

    for m in word_def.find_iter(line) {
        let start_char = line[..m.start()].chars().count();
        let end_char = start_char + m.as_str().chars().count();

        #[allow(clippy::cast_possible_truncation)]
        if col >= start_char && col < end_char {
            return Some(WordRange {
                word: m.as_str().to_string(),
                start_column: start_char as u32,
                end_column: end_char as u32,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(line: u32, col: u32) -> Position {
        Position::new(line, col)
    }

    #[test]
    fn word_at_position_default_regex() {
        let buf = Buffer::from_str("hello world");
        let w = get_word_at_position(&buf, pos(0, 0), default_word_definition());
        assert!(w.is_some());
        let w = w.unwrap();
        assert_eq!(w.word, "hello");
        assert_eq!(w.start_column, 0);
        assert_eq!(w.end_column, 5);
    }

    #[test]
    fn word_at_position_with_hyphen() {
        let buf = Buffer::from_str("font-size: 12px");
        let w = get_word_at_position(&buf, pos(0, 3), default_word_definition());
        assert!(w.is_some());
        assert_eq!(w.unwrap().word, "font-size");
    }

    #[test]
    fn word_at_position_on_space_returns_none() {
        let buf = Buffer::from_str("hello world");
        let w = get_word_at_position(&buf, pos(0, 5), default_word_definition());
        assert!(w.is_none());
    }

    #[test]
    fn word_until_position_prefix() {
        let buf = Buffer::from_str("hello world");
        let w = get_word_until_position(&buf, pos(0, 3), default_word_definition());
        assert_eq!(w.word, "hel");
        assert_eq!(w.start_column, 0);
        assert_eq!(w.end_column, 3);
    }

    #[test]
    fn word_until_position_at_end() {
        let buf = Buffer::from_str("hello world");
        let w = get_word_until_position(&buf, pos(0, 5), default_word_definition());
        assert_eq!(w.word, "hello");
    }

    #[test]
    fn word_until_position_not_on_word() {
        let buf = Buffer::from_str("  hello");
        let w = get_word_until_position(&buf, pos(0, 1), default_word_definition());
        assert_eq!(w.word, "");
    }

    #[test]
    fn css_word_definition_matches_at_sign() {
        let buf = Buffer::from_str("@media screen");
        let w = get_word_at_position(&buf, pos(0, 1), &language_definitions::CSS);
        assert!(w.is_some());
        assert_eq!(w.unwrap().word, "@media");
    }

    #[test]
    fn shell_word_definition_matches_path() {
        let buf = Buffer::from_str("ls /usr/local/bin");
        let w = get_word_at_position(&buf, pos(0, 5), &language_definitions::SHELL);
        assert!(w.is_some());
        assert_eq!(w.unwrap().word, "/usr/local/bin");
    }
}
