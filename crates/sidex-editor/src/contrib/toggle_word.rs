//! Toggle word — toggle between paired tokens at the cursor.
//!
//! Cycles through predefined word pairs like `true`/`false`, `yes`/`no`,
//! `on`/`off`, `public`/`private`, etc.

use sidex_text::{Buffer, Position, Range};

/// A pair of words that can be toggled.
type WordPair = (&'static str, &'static str);

/// Default word pairs for toggling.
const DEFAULT_PAIRS: &[WordPair] = &[
    ("true", "false"),
    ("True", "False"),
    ("TRUE", "FALSE"),
    ("yes", "no"),
    ("Yes", "No"),
    ("YES", "NO"),
    ("on", "off"),
    ("On", "Off"),
    ("ON", "OFF"),
    ("enable", "disable"),
    ("enabled", "disabled"),
    ("Enable", "Disable"),
    ("Enabled", "Disabled"),
    ("public", "private"),
    ("Public", "Private"),
    ("left", "right"),
    ("Left", "Right"),
    ("top", "bottom"),
    ("Top", "Bottom"),
    ("up", "down"),
    ("Up", "Down"),
    ("open", "close"),
    ("Open", "Close"),
    ("show", "hide"),
    ("Show", "Hide"),
    ("visible", "hidden"),
    ("before", "after"),
    ("Before", "After"),
    ("first", "last"),
    ("First", "Last"),
    ("start", "end"),
    ("Start", "End"),
    ("push", "pop"),
    ("read", "write"),
    ("Read", "Write"),
    ("get", "set"),
    ("Get", "Set"),
    ("add", "remove"),
    ("Add", "Remove"),
    ("min", "max"),
    ("Min", "Max"),
    ("width", "height"),
    ("Width", "Height"),
    ("horizontal", "vertical"),
    ("Horizontal", "Vertical"),
    ("row", "column"),
    ("Row", "Column"),
    ("and", "or"),
    ("AND", "OR"),
    ("0", "1"),
    ("let", "const"),
    ("var", "let"),
    ("&&", "||"),
    ("++", "--"),
    ("+=", "-="),
    ("==", "!="),
    ("===", "!=="),
    ("<", ">"),
    ("<=", ">="),
];

/// Configuration for toggle word.
#[derive(Debug, Clone, Default)]
pub struct ToggleWordConfig {
    /// Additional custom word pairs.
    pub custom_pairs: Vec<(String, String)>,
}

/// Finds the word at the cursor position and returns the range and text.
fn word_at_cursor(buffer: &Buffer, pos: Position) -> Option<(Range, String)> {
    if pos.line as usize >= buffer.len_lines() {
        return None;
    }
    let line = buffer.line_content(pos.line as usize);
    let chars: Vec<char> = line.chars().collect();
    let col = pos.column as usize;

    if col >= chars.len() {
        return None;
    }

    // For operator-like tokens, check 1-3 char sequences at cursor
    for len in (1..=3).rev() {
        if col + len <= chars.len() {
            let candidate: String = chars[col..col + len].iter().collect();
            if is_toggleable(&candidate, &[]) {
                return Some((
                    Range::new(
                        Position::new(pos.line, col as u32),
                        Position::new(pos.line, (col + len) as u32),
                    ),
                    candidate,
                ));
            }
        }
    }

    if !chars[col].is_alphanumeric() && chars[col] != '_' {
        return None;
    }

    let start = (0..col)
        .rev()
        .take_while(|&i| chars[i].is_alphanumeric() || chars[i] == '_')
        .last()
        .unwrap_or(col);
    let end = (col..chars.len())
        .take_while(|&i| chars[i].is_alphanumeric() || chars[i] == '_')
        .last()
        .map_or(col, |i| i + 1);

    let word: String = chars[start..end].iter().collect();
    if word.is_empty() {
        return None;
    }

    Some((
        Range::new(
            Position::new(pos.line, start as u32),
            Position::new(pos.line, end as u32),
        ),
        word,
    ))
}

fn is_toggleable(word: &str, custom_pairs: &[(String, String)]) -> bool {
    for &(a, b) in DEFAULT_PAIRS {
        if word == a || word == b {
            return true;
        }
    }
    for (a, b) in custom_pairs {
        if word == a || word == b {
            return true;
        }
    }
    false
}

/// Finds the toggle replacement for a word.
fn find_toggle(word: &str, custom_pairs: &[(String, String)]) -> Option<String> {
    for (a, b) in custom_pairs {
        if word == a {
            return Some(b.clone());
        }
        if word == b {
            return Some(a.clone());
        }
    }
    for &(a, b) in DEFAULT_PAIRS {
        if word == a {
            return Some(b.to_string());
        }
        if word == b {
            return Some(a.to_string());
        }
    }
    None
}

/// Toggles the word at the cursor position. Returns `true` if a toggle was
/// performed.
pub fn toggle_word_at_cursor(
    buffer: &mut Buffer,
    pos: Position,
    config: &ToggleWordConfig,
) -> bool {
    let Some((range, word)) = word_at_cursor(buffer, pos) else {
        return false;
    };

    let Some(replacement) = find_toggle(&word, &config.custom_pairs) else {
        return false;
    };

    let start = buffer.position_to_offset(range.start);
    let end = buffer.position_to_offset(range.end);
    buffer.replace(start..end, &replacement);
    true
}

/// Returns the toggled value for preview (without applying).
#[must_use]
pub fn preview_toggle(
    buffer: &Buffer,
    pos: Position,
    config: &ToggleWordConfig,
) -> Option<(Range, String)> {
    let (range, word) = word_at_cursor(buffer, pos)?;
    let replacement = find_toggle(&word, &config.custom_pairs)?;
    Some((range, replacement))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn toggle_true_false() {
        let mut buffer = buf("let x = true;");
        let config = ToggleWordConfig::default();
        let toggled = toggle_word_at_cursor(&mut buffer, Position::new(0, 8), &config);
        assert!(toggled);
        assert_eq!(buffer.text(), "let x = false;");
    }

    #[test]
    fn toggle_false_true() {
        let mut buffer = buf("let x = false;");
        let config = ToggleWordConfig::default();
        let toggled = toggle_word_at_cursor(&mut buffer, Position::new(0, 8), &config);
        assert!(toggled);
        assert_eq!(buffer.text(), "let x = true;");
    }

    #[test]
    fn toggle_yes_no() {
        let mut buffer = buf("enabled: yes");
        let config = ToggleWordConfig::default();
        let toggled = toggle_word_at_cursor(&mut buffer, Position::new(0, 9), &config);
        assert!(toggled);
        assert_eq!(buffer.text(), "enabled: no");
    }

    #[test]
    fn no_toggle_for_unknown() {
        let mut buffer = buf("let x = hello;");
        let config = ToggleWordConfig::default();
        let toggled = toggle_word_at_cursor(&mut buffer, Position::new(0, 8), &config);
        assert!(!toggled);
    }

    #[test]
    fn custom_pair() {
        let mut buffer = buf("mode: dark");
        let config = ToggleWordConfig {
            custom_pairs: vec![("dark".into(), "light".into())],
        };
        let toggled = toggle_word_at_cursor(&mut buffer, Position::new(0, 6), &config);
        assert!(toggled);
        assert_eq!(buffer.text(), "mode: light");
    }

    #[test]
    fn preview() {
        let buffer = buf("let x = true;");
        let config = ToggleWordConfig::default();
        let (_, replacement) = preview_toggle(&buffer, Position::new(0, 8), &config).unwrap();
        assert_eq!(replacement, "false");
    }
}
