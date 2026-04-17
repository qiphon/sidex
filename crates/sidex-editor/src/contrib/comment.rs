//! Comment toggle — mirrors VS Code's `LineCommentCommand` +
//! `BlockCommentCommand`.
//!
//! Provides line-comment and block-comment toggling that operates on a buffer
//! and selection range, plus comment continuation on Enter.

use sidex_text::{Buffer, Position, Range};

/// Toggles line comments for the given line range using `prefix` (e.g. `"//"`).
///
/// If all non-empty lines in the range already have the prefix, the prefix is
/// removed.  Otherwise, the prefix is added to every line.
pub fn toggle_line_comment(buffer: &mut Buffer, start_line: u32, end_line: u32, prefix: &str) {
    let line_count = buffer.len_lines() as u32;
    let end = end_line.min(line_count.saturating_sub(1));

    let all_commented = (start_line..=end).all(|l| {
        let content = buffer.line_content(l as usize);
        let trimmed = content.trim_start();
        trimmed.is_empty() || trimmed.starts_with(prefix)
    });

    if all_commented {
        remove_line_comments(buffer, start_line, end, prefix);
    } else {
        add_line_comments(buffer, start_line, end, prefix);
    }
}

/// Adds the comment prefix to each line, aligning to the minimum indentation.
fn add_line_comments(buffer: &mut Buffer, start: u32, end: u32, prefix: &str) {
    let min_indent = (start..=end)
        .map(|l| {
            let content = buffer.line_content(l as usize);
            let trimmed = content.trim_start();
            if trimmed.is_empty() {
                usize::MAX
            } else {
                content.len() - trimmed.len()
            }
        })
        .min()
        .unwrap_or(0);

    let prefix_with_space = format!("{prefix} ");
    for line in (start..=end).rev() {
        let content = buffer.line_content(line as usize);
        let trimmed = content.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let insert_offset = buffer.line_to_char(line as usize) + min_indent;
        buffer.insert(insert_offset, &prefix_with_space);
    }
}

fn remove_line_comments(buffer: &mut Buffer, start: u32, end: u32, prefix: &str) {
    for line in (start..=end).rev() {
        let content = buffer.line_content(line as usize);
        let trimmed = content.trim_start();
        if !trimmed.starts_with(prefix) {
            continue;
        }
        let indent_len = content.len() - trimmed.len();
        let remove_len = if trimmed.len() > prefix.len()
            && trimmed.as_bytes().get(prefix.len()) == Some(&b' ')
        {
            prefix.len() + 1
        } else {
            prefix.len()
        };
        let char_start = buffer.line_to_char(line as usize) + indent_len;
        let char_end = char_start + remove_len;
        buffer.remove(char_start..char_end);
    }
}

/// Toggles a block comment around the given range using `open`/`close`
/// delimiters (e.g. `"/*"` and `"*/"`).
///
/// Handles proper indentation for multi-line block comments.
pub fn toggle_block_comment(buffer: &mut Buffer, range: Range, open: &str, close: &str) {
    let start_offset = buffer.position_to_offset(range.start);
    let end_offset = buffer.position_to_offset(range.end);
    let text = buffer.slice(start_offset..end_offset);

    let trimmed = text.trim();
    if trimmed.starts_with(open) && trimmed.ends_with(close) {
        let inner = trimmed
            .strip_prefix(open)
            .and_then(|s| s.strip_suffix(close))
            .unwrap_or(trimmed);
        let inner = inner.strip_prefix(' ').unwrap_or(inner);
        let inner = inner.strip_suffix(' ').unwrap_or(inner);
        buffer.replace(start_offset..end_offset, inner);
    } else if range.start.line != range.end.line {
        wrap_block_comment_multiline(buffer, range, open, close);
    } else {
        let wrapped = format!("{open} {text} {close}");
        buffer.replace(start_offset..end_offset, &wrapped);
    }
}

/// Wraps a multi-line selection in block comments with proper indentation.
fn wrap_block_comment_multiline(buffer: &mut Buffer, range: Range, open: &str, close: &str) {
    let start_line = range.start.line as usize;
    let end_line = range.end.line as usize;

    let indent = {
        let content = buffer.line_content(start_line);
        let trimmed = content.trim_start();
        content[..content.len() - trimmed.len()].to_string()
    };

    // Insert close on a new line after the selection
    let end_of_last_line = buffer.line_to_char(end_line) + buffer.line_content_len(end_line);
    buffer.insert(end_of_last_line, &format!("\n{indent}{close}"));

    // Insert open before the first line
    let start_of_first_line = buffer.line_to_char(start_line);
    buffer.insert(start_of_first_line, &format!("{open}\n"));

    // Add " * " prefix to each inner line
    for line_idx in ((start_line + 1)..=(end_line + 1)).rev() {
        let content = buffer.line_content(line_idx);
        let trimmed = content.trim_start();
        if trimmed.is_empty() {
            let offset = buffer.line_to_char(line_idx);
            buffer.insert(offset, &format!("{indent} *"));
        } else {
            let line_indent = content.len() - trimmed.len();
            let offset = buffer.line_to_char(line_idx) + line_indent;
            buffer.insert(offset, " * ");
        }
    }
}

/// Determines if pressing Enter should continue a comment and returns the
/// prefix to insert, or `None` if no continuation is needed.
///
/// Handles:
/// - `// ` → `// ` (line comment continuation)
/// - ` * ` → ` * ` (block comment continuation)
/// - `/// ` → `/// ` (doc comment continuation)
#[must_use]
pub fn comment_continuation_prefix(line_text: &str) -> Option<String> {
    let trimmed = line_text.trim_start();
    let indent: String = line_text
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();

    if trimmed.starts_with("/// ") || trimmed.starts_with("///\n") || trimmed == "///" {
        return Some(format!("{indent}/// "));
    }

    if trimmed.starts_with("// ") || trimmed.starts_with("//\n") || trimmed == "//" {
        return Some(format!("{indent}// "));
    }

    if trimmed.starts_with("* ") || trimmed.starts_with("*\n") || trimmed == "*" {
        return Some(format!("{indent} * "));
    }

    if trimmed.starts_with("/**") {
        return Some(format!("{indent} * "));
    }

    None
}

/// Checks if the cursor is at the end of a line and returns a new line with
/// comment continuation. Returns `None` if no continuation applies.
#[must_use]
pub fn on_enter_in_comment(buffer: &Buffer, pos: Position) -> Option<String> {
    if pos.line as usize >= buffer.len_lines() {
        return None;
    }
    let content = buffer.line_content(pos.line as usize);
    comment_continuation_prefix(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn add_line_comments() {
        let mut buffer = buf("foo\nbar\nbaz");
        toggle_line_comment(&mut buffer, 0, 2, "//");
        let text = buffer.text();
        assert!(text.contains("// foo"));
        assert!(text.contains("// bar"));
        assert!(text.contains("// baz"));
    }

    #[test]
    fn remove_line_comments() {
        let mut buffer = buf("// foo\n// bar");
        toggle_line_comment(&mut buffer, 0, 1, "//");
        let text = buffer.text();
        assert_eq!(text, "foo\nbar");
    }

    #[test]
    fn block_comment_toggle() {
        let mut buffer = buf("hello world");
        let range = Range::new(Position::new(0, 0), Position::new(0, 11));
        toggle_block_comment(&mut buffer, range, "/*", "*/");
        assert_eq!(buffer.text(), "/* hello world */");
    }

    #[test]
    fn block_comment_remove() {
        let mut buffer = buf("/* hello */");
        let range = Range::new(Position::new(0, 0), Position::new(0, 11));
        toggle_block_comment(&mut buffer, range, "/*", "*/");
        assert_eq!(buffer.text(), "hello");
    }

    #[test]
    fn comment_continuation_line() {
        assert_eq!(
            comment_continuation_prefix("    // hello"),
            Some("    // ".to_string()),
        );
    }

    #[test]
    fn comment_continuation_doc() {
        assert_eq!(
            comment_continuation_prefix("    /// hello"),
            Some("    /// ".to_string()),
        );
    }

    #[test]
    fn comment_continuation_block() {
        assert_eq!(
            comment_continuation_prefix("     * hello"),
            Some("      * ".to_string()),
        );
    }

    #[test]
    fn no_continuation() {
        assert_eq!(comment_continuation_prefix("    let x = 5;"), None);
    }
}
