//! Auto-close brackets, quotes, and HTML tags — mirrors VS Code's
//! `AutoClosingPairsContribution` and `AutoClosingTags`.
//!
//! Provides configurable rules for auto-inserting matching delimiters,
//! over-typing closing characters, and auto-surrounding selections.

use std::collections::HashMap;

/// Top-level configuration for auto-closing behaviour.
#[derive(Debug, Clone)]
pub struct AutoCloseConfig {
    pub brackets: bool,
    pub quotes: bool,
    pub before_whitespace: bool,
    pub language_rules: HashMap<String, AutoCloseRules>,
}

impl Default for AutoCloseConfig {
    fn default() -> Self {
        let mut language_rules = HashMap::new();

        language_rules.insert("rust".into(), AutoCloseRules {
            brackets: vec![('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')],
            quotes: vec!['"', '\''],
            not_before: vec![],
        });
        language_rules.insert("python".into(), AutoCloseRules {
            brackets: vec![('(', ')'), ('[', ']'), ('{', '}')],
            quotes: vec!['"', '\''],
            not_before: vec![],
        });
        language_rules.insert("html".into(), AutoCloseRules {
            brackets: vec![('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')],
            quotes: vec!['"', '\'', '`'],
            not_before: vec![],
        });

        Self {
            brackets: true,
            quotes: true,
            before_whitespace: true,
            language_rules,
        }
    }
}

/// Language-specific auto-close rules.
#[derive(Debug, Clone)]
pub struct AutoCloseRules {
    pub brackets: Vec<(char, char)>,
    pub quotes: Vec<char>,
    pub not_before: Vec<char>,
}

impl Default for AutoCloseRules {
    fn default() -> Self {
        Self {
            brackets: vec![('(', ')'), ('[', ']'), ('{', '}')],
            quotes: vec!['"', '\'', '`'],
            not_before: vec![],
        }
    }
}

/// Context provided to auto-close decision functions.
#[derive(Debug, Clone)]
pub struct EditContext {
    pub language: String,
    pub line_text: String,
    pub column: u32,
    pub has_selection: bool,
    pub selected_text: String,
}

impl Default for EditContext {
    fn default() -> Self {
        Self {
            language: String::new(),
            line_text: String::new(),
            column: 0,
            has_selection: false,
            selected_text: String::new(),
        }
    }
}

const DEFAULT_BRACKETS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}')];
const DEFAULT_QUOTES: &[char] = &['"', '\'', '`'];

impl AutoCloseConfig {
    fn rules_for(&self, language: &str) -> AutoCloseRules {
        self.language_rules
            .get(language)
            .cloned()
            .unwrap_or_default()
    }
}

/// Determines whether typing `char_typed` should auto-insert a closing
/// character. Returns the closing character if so.
#[must_use]
pub fn should_auto_close(
    char_typed: char,
    context: &EditContext,
    config: &AutoCloseConfig,
) -> Option<char> {
    let rules = config.rules_for(&context.language);
    let col = context.column as usize;
    let chars: Vec<char> = context.line_text.chars().collect();

    // Check brackets
    if config.brackets {
        for &(open, close) in &rules.brackets {
            if char_typed == open {
                if !can_auto_close_at(col, &chars, &rules.not_before, config.before_whitespace) {
                    return None;
                }
                return Some(close);
            }
        }
        for &(open, close) in DEFAULT_BRACKETS {
            if char_typed == open && !rules.brackets.iter().any(|(o, _)| *o == open) {
                if !can_auto_close_at(col, &chars, &rules.not_before, config.before_whitespace) {
                    return None;
                }
                return Some(close);
            }
            let _ = close;
        }
    }

    // Check quotes
    if config.quotes {
        let quote_list: Vec<char> = if rules.quotes.is_empty() {
            DEFAULT_QUOTES.to_vec()
        } else {
            rules.quotes.clone()
        };
        if quote_list.contains(&char_typed) {
            if col > 0 && chars.get(col - 1).is_some_and(|c| c.is_alphanumeric()) {
                return None;
            }
            if !can_auto_close_at(col, &chars, &rules.not_before, config.before_whitespace) {
                return None;
            }
            return Some(char_typed);
        }
    }

    None
}

fn can_auto_close_at(col: usize, chars: &[char], not_before: &[char], ws_only: bool) -> bool {
    if col >= chars.len() {
        return true;
    }
    let next = chars[col];
    if not_before.contains(&next) {
        return false;
    }
    if next.is_alphanumeric() {
        return false;
    }
    if ws_only && !next.is_whitespace() && !is_closing_char(next) {
        return false;
    }
    true
}

fn is_closing_char(c: char) -> bool {
    matches!(c, ')' | ']' | '}' | '>' | ';' | ',' | '"' | '\'' | '`')
}

/// Determines whether typing `char_typed` should simply move the cursor
/// past the existing `next_char` (over-type) rather than inserting.
#[must_use]
pub fn should_over_type(char_typed: char, next_char: char) -> bool {
    if char_typed != next_char {
        return false;
    }
    matches!(
        char_typed,
        ')' | ']' | '}' | '>' | '"' | '\'' | '`'
    )
}

/// Determines whether typing `char_typed` with an active selection should
/// surround the selection with a pair. Returns `(open, close)` strings.
#[must_use]
pub fn should_auto_surround(
    char_typed: char,
    has_selection: bool,
) -> Option<(String, String)> {
    if !has_selection {
        return None;
    }
    let pair = match char_typed {
        '(' => ("(", ")"),
        '[' => ("[", "]"),
        '{' => ("{", "}"),
        '<' => ("<", ">"),
        '"' => ("\"", "\""),
        '\'' => ("'", "'"),
        '`' => ("`", "`"),
        _ => return None,
    };
    Some((pair.0.to_string(), pair.1.to_string()))
}

/// Determines if an HTML closing tag should be auto-inserted after typing `>`.
/// Returns the closing tag string (e.g. `</div>`) if applicable.
/// `column` is the cursor position *before* the `>` is inserted.
#[must_use]
pub fn should_auto_close_html_tag(line_text: &str, column: u32) -> Option<String> {
    let col = column as usize;
    // We're about to type `>`, so look at text up to cursor position.
    let before: String = line_text.chars().take(col).collect();

    let tag_start = before.rfind('<')?;
    let tag_region = &before[tag_start + 1..];

    if tag_region.starts_with('/') || tag_region.starts_with('!') {
        return None;
    }

    let tag_name: String = tag_region
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();

    if tag_name.is_empty() {
        return None;
    }

    const VOID_ELEMENTS: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input",
        "link", "meta", "param", "source", "track", "wbr",
    ];
    let lower = tag_name.to_ascii_lowercase();
    if VOID_ELEMENTS.contains(&lower.as_str()) {
        return None;
    }

    if tag_region.contains('/') {
        return None;
    }

    Some(format!("</{tag_name}>"))
}

/// Full state for the auto-close feature, tracking configuration
/// and per-language overrides.
#[derive(Debug, Clone, Default)]
pub struct AutoCloseState {
    pub config: AutoCloseConfig,
    pub enabled: bool,
}

impl AutoCloseState {
    /// Creates a new enabled state with default config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: AutoCloseConfig::default(),
            enabled: true,
        }
    }

    /// Processes a typed character and returns the action to perform.
    #[must_use]
    pub fn on_type(
        &self,
        char_typed: char,
        context: &EditContext,
    ) -> AutoCloseAction {
        if !self.enabled {
            return AutoCloseAction::None;
        }

        if context.has_selection {
            if let Some((open, close)) = should_auto_surround(char_typed, true) {
                return AutoCloseAction::Surround { open, close };
            }
        }

        let chars: Vec<char> = context.line_text.chars().collect();
        let col = context.column as usize;
        if col < chars.len() && should_over_type(char_typed, chars[col]) {
            return AutoCloseAction::OverType;
        }

        if let Some(close) = should_auto_close(char_typed, context, &self.config) {
            return AutoCloseAction::Close(close);
        }

        if char_typed == '>'
            && (context.language == "html"
                || context.language == "xml"
                || context.language == "jsx"
                || context.language == "tsx"
                || context.language == "vue"
                || context.language == "svelte")
        {
            if let Some(close_tag) = should_auto_close_html_tag(&context.line_text, context.column)
            {
                return AutoCloseAction::CloseHtmlTag(close_tag);
            }
        }

        AutoCloseAction::None
    }
}

/// Action to perform after typing a character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoCloseAction {
    None,
    Close(char),
    OverType,
    Surround { open: String, close: String },
    CloseHtmlTag(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(lang: &str, line: &str, col: u32) -> EditContext {
        EditContext {
            language: lang.into(),
            line_text: line.into(),
            column: col,
            has_selection: false,
            selected_text: String::new(),
        }
    }

    #[test]
    fn auto_close_open_bracket() {
        let config = AutoCloseConfig::default();
        let context = ctx("rust", "hello", 5);
        assert_eq!(should_auto_close('(', &context, &config), Some(')'));
        assert_eq!(should_auto_close('[', &context, &config), Some(']'));
        assert_eq!(should_auto_close('{', &context, &config), Some('}'));
    }

    #[test]
    fn auto_close_quote() {
        let config = AutoCloseConfig::default();
        let context = ctx("rust", "let x = ", 8);
        assert_eq!(should_auto_close('"', &context, &config), Some('"'));
    }

    #[test]
    fn no_auto_close_quote_after_alphanum() {
        let config = AutoCloseConfig::default();
        let context = ctx("rust", "don", 3);
        assert_eq!(should_auto_close('\'', &context, &config), None);
    }

    #[test]
    fn no_auto_close_before_alphanum() {
        let config = AutoCloseConfig::default();
        let context = ctx("rust", "hello", 0);
        assert_eq!(should_auto_close('(', &context, &config), None);
    }

    #[test]
    fn over_type_closing() {
        assert!(should_over_type(')', ')'));
        assert!(should_over_type(']', ']'));
        assert!(should_over_type('"', '"'));
        assert!(!should_over_type('(', ')'));
        assert!(!should_over_type('a', 'a'));
    }

    #[test]
    fn auto_surround() {
        let result = should_auto_surround('(', true);
        assert_eq!(result, Some(("(".into(), ")".into())));
        assert_eq!(should_auto_surround('"', true), Some(("\"".into(), "\"".into())));
        assert!(should_auto_surround('a', true).is_none());
        assert!(should_auto_surround('(', false).is_none());
    }

    #[test]
    fn html_tag_close() {
        assert_eq!(
            should_auto_close_html_tag("<div", 4),
            Some("</div>".into())
        );
        assert_eq!(
            should_auto_close_html_tag("<span class=\"foo\"", 17),
            Some("</span>".into())
        );
    }

    #[test]
    fn html_void_no_close() {
        assert!(should_auto_close_html_tag("<br", 3).is_none());
        assert!(should_auto_close_html_tag("<img src=\"\"", 11).is_none());
        assert!(should_auto_close_html_tag("<input", 6).is_none());
    }

    #[test]
    fn html_closing_tag_no_double() {
        assert!(should_auto_close_html_tag("</div", 5).is_none());
    }

    #[test]
    fn html_self_closing_no_close() {
        assert!(should_auto_close_html_tag("<br/", 4).is_none());
    }

    #[test]
    fn state_on_type_close() {
        let state = AutoCloseState::new();
        let context = ctx("html", "hello", 5);
        match state.on_type('(', &context) {
            AutoCloseAction::Close(c) => assert_eq!(c, ')'),
            other => panic!("expected Close, got {other:?}"),
        }
    }

    #[test]
    fn state_on_type_overtype() {
        let state = AutoCloseState::new();
        let context = ctx("html", "hello)", 5);
        assert_eq!(state.on_type(')', &context), AutoCloseAction::OverType);
    }

    #[test]
    fn state_on_type_surround() {
        let state = AutoCloseState::new();
        let context = EditContext {
            language: "html".into(),
            line_text: "hello".into(),
            column: 0,
            has_selection: true,
            selected_text: "hello".into(),
        };
        match state.on_type('(', &context) {
            AutoCloseAction::Surround { open, close } => {
                assert_eq!(open, "(");
                assert_eq!(close, ")");
            }
            other => panic!("expected Surround, got {other:?}"),
        }
    }

    #[test]
    fn state_on_type_html_tag() {
        let state = AutoCloseState::new();
        // Cursor at col 4, about to type '>' to complete `<div>`
        let context = ctx("html", "<div", 4);
        match state.on_type('>', &context) {
            AutoCloseAction::CloseHtmlTag(tag) => assert_eq!(tag, "</div>"),
            other => panic!("expected CloseHtmlTag, got {other:?}"),
        }
    }

    #[test]
    fn disabled_state() {
        let mut state = AutoCloseState::new();
        state.enabled = false;
        let context = ctx("html", "hello", 5);
        assert_eq!(state.on_type('(', &context), AutoCloseAction::None);
    }
}
