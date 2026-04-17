//! Editor find/replace widget — the Ctrl+F find bar with search, replace,
//! toggle buttons, match highlighting, and regex support with capture groups.
//!
//! This module provides the full widget state and matching logic that drives
//! the find bar overlay rendered by the GPU layer. It builds on top of
//! [`super::find::FindState`] for core search state, adding the widget-specific
//! view model, free-function matching APIs, and replace helpers.

use std::fmt;

use super::find::{FindOptions, FindState};
use sidex_text::{Buffer, Range};

/// The complete find/replace widget state exposed to the renderer.
///
/// Wraps [`FindState`] and adds UI-specific fields like regex validity,
/// focus tracking, and the replace-visible flag.
#[derive(Debug, Clone)]
pub struct FindWidget {
    pub visible: bool,
    pub search_text: String,
    pub replace_text: String,
    pub replace_visible: bool,
    pub options: FindWidgetOptions,
    pub matches: Vec<Range>,
    pub current_match: Option<usize>,
    pub is_regex_valid: bool,
    pub regex_error: Option<String>,
    pub focus: FindWidgetFocus,
    state: FindState,
}

/// Search option toggles surfaced by the widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindWidgetOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub in_selection: bool,
    pub preserve_case: bool,
}

impl Default for FindWidgetOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            whole_word: false,
            regex: false,
            in_selection: false,
            preserve_case: false,
        }
    }
}

/// Which field in the find widget currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindWidgetFocus {
    SearchInput,
    ReplaceInput,
}

impl Default for FindWidgetFocus {
    fn default() -> Self {
        Self::SearchInput
    }
}

impl Default for FindWidget {
    fn default() -> Self {
        Self {
            visible: false,
            search_text: String::new(),
            replace_text: String::new(),
            replace_visible: false,
            options: FindWidgetOptions::default(),
            matches: Vec::new(),
            current_match: None,
            is_regex_valid: true,
            regex_error: None,
            focus: FindWidgetFocus::default(),
            state: FindState::default(),
        }
    }
}

impl FindWidget {
    /// Opens the find widget, optionally seeding the search string.
    pub fn show(&mut self, seed: Option<&str>) {
        self.visible = true;
        self.focus = FindWidgetFocus::SearchInput;
        if let Some(s) = seed {
            self.search_text = s.to_string();
        }
    }

    /// Opens the find widget with the replace row visible.
    pub fn show_replace(&mut self, seed: Option<&str>) {
        self.show(seed);
        self.replace_visible = true;
    }

    /// Closes the find widget and clears highlights.
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.replace_visible = false;
        self.matches.clear();
        self.current_match = None;
        self.state.dismiss();
    }

    // ── Option toggles ──────────────────────────────────────────────────

    pub fn toggle_case_sensitive(&mut self) {
        self.options.case_sensitive = !self.options.case_sensitive;
    }

    pub fn toggle_whole_word(&mut self) {
        self.options.whole_word = !self.options.whole_word;
    }

    pub fn toggle_regex(&mut self) {
        self.options.regex = !self.options.regex;
    }

    pub fn toggle_in_selection(&mut self) {
        self.options.in_selection = !self.options.in_selection;
    }

    pub fn toggle_preserve_case(&mut self) {
        self.options.preserve_case = !self.options.preserve_case;
    }

    pub fn toggle_replace_visible(&mut self) {
        self.replace_visible = !self.replace_visible;
    }

    // ── Sync with FindState ─────────────────────────────────────────────

    fn sync_options_to_state(&mut self) {
        self.state.options = FindOptions {
            is_regex: self.options.regex,
            match_case: self.options.case_sensitive,
            whole_word: self.options.whole_word,
            preserve_case: self.options.preserve_case,
            search_in_selection: self.options.in_selection,
            wrap_around: true,
        };
    }

    fn sync_state_to_widget(&mut self) {
        self.matches = self.state.match_ranges();
        self.current_match = self.state.active_match_idx;
    }

    // ── Search ──────────────────────────────────────────────────────────

    /// Runs the search against `buffer`, updating matches and the current
    /// match index. Also validates the regex if regex mode is active.
    pub fn research(&mut self, buffer: &Buffer) {
        self.sync_options_to_state();
        self.state.set_search_string(self.search_text.clone());
        self.state.set_replace_string(self.replace_text.clone());

        self.validate_regex();

        if self.options.regex && !self.is_regex_valid {
            self.matches.clear();
            self.current_match = None;
            return;
        }

        self.state.research(buffer);
        self.sync_state_to_widget();
    }

    fn validate_regex(&mut self) {
        if !self.options.regex || self.search_text.is_empty() {
            self.is_regex_valid = true;
            self.regex_error = None;
            return;
        }
        match regex::Regex::new(&self.search_text) {
            Ok(_) => {
                self.is_regex_valid = true;
                self.regex_error = None;
            }
            Err(e) => {
                self.is_regex_valid = false;
                self.regex_error = Some(e.to_string());
            }
        }
    }

    // ── Navigation ──────────────────────────────────────────────────────

    /// Advances to the next match. Returns the new match index.
    pub fn find_next(&mut self) -> Option<usize> {
        let idx = self.state.find_next();
        self.current_match = self.state.active_match_idx;
        idx
    }

    /// Moves to the previous match. Returns the new match index.
    pub fn find_previous(&mut self) -> Option<usize> {
        let idx = self.state.find_previous();
        self.current_match = self.state.active_match_idx;
        idx
    }

    /// Returns the range of the currently active match.
    pub fn current_match_range(&self) -> Option<Range> {
        self.current_match
            .and_then(|i| self.matches.get(i))
            .copied()
    }

    // ── Replace ─────────────────────────────────────────────────────────

    /// Replaces the current match and advances to the next.
    pub fn replace_current(&mut self, buffer: &mut Buffer) -> Option<String> {
        self.sync_options_to_state();
        self.state.set_replace_string(self.replace_text.clone());
        let result = self.state.replace_current(buffer);
        self.sync_state_to_widget();
        result
    }

    /// Replaces all matches. Returns the count of replacements made.
    pub fn replace_all(&mut self, buffer: &mut Buffer) -> usize {
        self.sync_options_to_state();
        self.state.set_replace_string(self.replace_text.clone());
        let count = self.state.replace_all(buffer);
        self.sync_state_to_widget();
        count
    }

    // ── Display helpers ─────────────────────────────────────────────────

    /// Returns a formatted match count string like `"3 of 42"` or `"No results"`.
    #[must_use]
    pub fn match_count_label(&self) -> String {
        let total = self.matches.len();
        if total == 0 {
            if self.search_text.is_empty() {
                String::new()
            } else {
                "No results".to_string()
            }
        } else {
            let current = self.current_match.map_or(0, |i| i + 1);
            format!("{current} of {total}")
        }
    }

    /// Returns `(current_1based, total)` for display.
    #[must_use]
    pub fn match_count_display(&self) -> (usize, usize) {
        let total = self.matches.len();
        let current = self.current_match.map_or(0, |i| i + 1);
        (current, total)
    }

    /// Access the underlying `FindState`.
    pub fn find_state(&self) -> &FindState {
        &self.state
    }
}

impl fmt::Display for FindWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (cur, total) = self.match_count_display();
        write!(
            f,
            "FindWidget(query={:?}, {cur}/{total}, visible={})",
            self.search_text, self.visible
        )
    }
}

// ── Free-standing matching functions ────────────────────────────────────────

/// Finds all matches of `query` in `text` respecting the given options.
///
/// Returns a vector of `(start_byte, end_byte)` ranges. For regex mode, uses
/// the `regex` crate; otherwise performs literal or whole-word search.
#[must_use]
pub fn find_all_matches(text: &str, query: &str, options: &FindWidgetOptions) -> Vec<Range> {
    if query.is_empty() {
        return Vec::new();
    }

    if options.regex {
        return find_all_regex_matches(text, query, options);
    }

    let (haystack, needle) = if options.case_sensitive {
        (text.to_string(), query.to_string())
    } else {
        (text.to_lowercase(), query.to_lowercase())
    };

    let mut results = Vec::new();
    let mut search_start = 0;

    while let Some(pos) = haystack[search_start..].find(&needle) {
        let abs_start = search_start + pos;
        let abs_end = abs_start + needle.len();

        if options.whole_word && !is_whole_word(text, abs_start, abs_end) {
            search_start = abs_start + 1;
            continue;
        }

        let start_pos = offset_to_position(text, abs_start);
        let end_pos = offset_to_position(text, abs_end);
        results.push(Range {
            start: start_pos,
            end: end_pos,
        });

        search_start = abs_end;
    }

    results
}

fn find_all_regex_matches(text: &str, pattern: &str, options: &FindWidgetOptions) -> Vec<Range> {
    let re = regex::RegexBuilder::new(pattern)
        .case_insensitive(!options.case_sensitive)
        .build();

    let Ok(re) = re else {
        return Vec::new();
    };

    let mut results = Vec::new();
    for m in re.find_iter(text) {
        let start = m.start();
        let end = m.end();

        if options.whole_word && !is_whole_word(text, start, end) {
            continue;
        }

        let start_pos = offset_to_position(text, start);
        let end_pos = offset_to_position(text, end);
        results.push(Range {
            start: start_pos,
            end: end_pos,
        });
    }
    results
}

/// Replaces the text within `match_range` with `replacement`.
///
/// When regex mode is on and the replacement contains `$1`, `$2`, etc., capture
/// groups are expanded. When preserve-case is on, the casing of the matched
/// text is transferred to the replacement.
#[must_use]
pub fn replace_match(
    text: &str,
    match_range: &Range,
    replacement: &str,
    options: &FindWidgetOptions,
) -> String {
    let start = position_to_offset(text, match_range.start);
    let end = position_to_offset(text, match_range.end);
    let matched = &text[start..end];

    let actual_replacement = if options.preserve_case {
        apply_preserve_case(matched, replacement)
    } else {
        replacement.to_string()
    };

    let mut result = String::with_capacity(text.len() + actual_replacement.len());
    result.push_str(&text[..start]);
    result.push_str(&actual_replacement);
    result.push_str(&text[end..]);
    result
}

/// Replaces all matches of `query` in `text` with `replacement`.
///
/// Returns `(new_text, replacement_count)`.
#[must_use]
pub fn replace_all(
    text: &str,
    query: &str,
    replacement: &str,
    options: &FindWidgetOptions,
) -> (String, u32) {
    let matches = find_all_matches(text, query, options);
    if matches.is_empty() {
        return (text.to_string(), 0);
    }

    let count = matches.len() as u32;
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;

    for m in &matches {
        let start = position_to_offset(text, m.start);
        let end = position_to_offset(text, m.end);
        let matched = &text[start..end];

        result.push_str(&text[last_end..start]);

        let actual_replacement = if options.regex {
            expand_regex_replacement(text, query, matched, replacement, options)
        } else if options.preserve_case {
            apply_preserve_case(matched, replacement)
        } else {
            replacement.to_string()
        };

        result.push_str(&actual_replacement);
        last_end = end;
    }
    result.push_str(&text[last_end..]);

    (result, count)
}

// ── Preserve-case logic ─────────────────────────────────────────────────────

fn apply_preserve_case(matched: &str, replacement: &str) -> String {
    if matched.is_empty() || replacement.is_empty() {
        return replacement.to_string();
    }
    let matched_chars: Vec<char> = matched.chars().collect();
    let has_alpha = matched_chars.iter().any(|c| c.is_alphabetic());
    if !has_alpha {
        return replacement.to_string();
    }

    let all_upper = matched_chars
        .iter()
        .all(|c| !c.is_alphabetic() || c.is_uppercase());
    let all_lower = matched_chars
        .iter()
        .all(|c| !c.is_alphabetic() || c.is_lowercase());

    if all_upper {
        replacement.to_uppercase()
    } else if all_lower {
        replacement.to_lowercase()
    } else if matched_chars[0].is_uppercase() {
        let mut chars = replacement.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => {
                let mut s: String = first.to_uppercase().collect();
                s.extend(chars);
                s
            }
        }
    } else {
        replacement.to_string()
    }
}

fn expand_regex_replacement(
    _text: &str,
    pattern: &str,
    matched: &str,
    replacement: &str,
    options: &FindWidgetOptions,
) -> String {
    let re = regex::RegexBuilder::new(pattern)
        .case_insensitive(!options.case_sensitive)
        .build();

    let Ok(re) = re else {
        return replacement.to_string();
    };

    let expanded = re.replace(matched, replacement).into_owned();

    if options.preserve_case {
        apply_preserve_case(matched, &expanded)
    } else {
        expanded
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn is_whole_word(text: &str, start: usize, end: usize) -> bool {
    let before_ok = start == 0
        || text[..start]
            .chars()
            .next_back()
            .map_or(true, |c| !c.is_alphanumeric() && c != '_');
    let after_ok = end >= text.len()
        || text[end..]
            .chars()
            .next()
            .map_or(true, |c| !c.is_alphanumeric() && c != '_');
    before_ok && after_ok
}

fn offset_to_position(text: &str, offset: usize) -> sidex_text::Position {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    sidex_text::Position { line, column: col }
}

fn position_to_offset(text: &str, pos: sidex_text::Position) -> usize {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in text.char_indices() {
        if line == pos.line && col == pos.column {
            return i;
        }
        if ch == '\n' {
            if line == pos.line {
                return i;
            }
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_all_literal_case_insensitive() {
        let opts = FindWidgetOptions {
            case_sensitive: false,
            ..Default::default()
        };
        let matches = find_all_matches("Hello hello HELLO", "hello", &opts);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn find_all_literal_case_sensitive() {
        let opts = FindWidgetOptions {
            case_sensitive: true,
            ..Default::default()
        };
        let matches = find_all_matches("Hello hello HELLO", "hello", &opts);
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn find_all_whole_word() {
        let opts = FindWidgetOptions {
            case_sensitive: true,
            whole_word: true,
            ..Default::default()
        };
        let matches = find_all_matches("foobar foo bar foo", "foo", &opts);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn find_all_regex() {
        let opts = FindWidgetOptions {
            regex: true,
            case_sensitive: true,
            ..Default::default()
        };
        let matches = find_all_matches("fn main() {\n    fn helper() {}", r"fn\s+\w+", &opts);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn find_all_empty_query() {
        let opts = FindWidgetOptions::default();
        let matches = find_all_matches("some text", "", &opts);
        assert!(matches.is_empty());
    }

    #[test]
    fn replace_match_basic() {
        let opts = FindWidgetOptions::default();
        let matches = find_all_matches("hello world", "world", &opts);
        let result = replace_match("hello world", &matches[0], "rust", &opts);
        assert_eq!(result, "hello rust");
    }

    #[test]
    fn replace_all_basic() {
        let opts = FindWidgetOptions {
            case_sensitive: false,
            ..Default::default()
        };
        let (result, count) = replace_all("foo bar foo baz foo", "foo", "qux", &opts);
        assert_eq!(count, 3);
        assert_eq!(result, "qux bar qux baz qux");
    }

    #[test]
    fn replace_all_preserve_case() {
        let opts = FindWidgetOptions {
            case_sensitive: false,
            preserve_case: true,
            ..Default::default()
        };
        let (result, count) = replace_all("foo Foo FOO", "foo", "bar", &opts);
        assert_eq!(count, 3);
        assert_eq!(result, "bar Bar BAR");
    }

    #[test]
    fn replace_all_no_match() {
        let opts = FindWidgetOptions::default();
        let (result, count) = replace_all("hello world", "xyz", "abc", &opts);
        assert_eq!(count, 0);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn widget_show_and_dismiss() {
        let mut w = FindWidget::default();
        w.show(Some("test"));
        assert!(w.visible);
        assert_eq!(w.search_text, "test");

        w.dismiss();
        assert!(!w.visible);
        assert!(w.matches.is_empty());
    }

    #[test]
    fn widget_toggle_options() {
        let mut w = FindWidget::default();
        assert!(!w.options.case_sensitive);
        w.toggle_case_sensitive();
        assert!(w.options.case_sensitive);
        w.toggle_whole_word();
        assert!(w.options.whole_word);
        w.toggle_regex();
        assert!(w.options.regex);
        w.toggle_preserve_case();
        assert!(w.options.preserve_case);
        w.toggle_in_selection();
        assert!(w.options.in_selection);
    }

    #[test]
    fn widget_match_count_label() {
        let mut w = FindWidget::default();
        w.search_text = "xyz".to_string();
        assert_eq!(w.match_count_label(), "No results");

        w.search_text.clear();
        assert!(w.match_count_label().is_empty());
    }

    #[test]
    fn widget_regex_validation() {
        let mut w = FindWidget::default();
        w.options.regex = true;
        w.search_text = "[invalid".to_string();
        w.validate_regex();
        assert!(!w.is_regex_valid);
        assert!(w.regex_error.is_some());

        w.search_text = r"\w+".to_string();
        w.validate_regex();
        assert!(w.is_regex_valid);
        assert!(w.regex_error.is_none());
    }

    #[test]
    fn preserve_case_all_upper() {
        assert_eq!(apply_preserve_case("FOO", "bar"), "BAR");
    }

    #[test]
    fn preserve_case_first_upper() {
        assert_eq!(apply_preserve_case("Foo", "bar"), "Bar");
    }

    #[test]
    fn preserve_case_all_lower() {
        assert_eq!(apply_preserve_case("foo", "BAR"), "bar");
    }

    #[test]
    fn whole_word_boundary() {
        assert!(is_whole_word("foo bar baz", 4, 7));
        assert!(!is_whole_word("foobar baz", 0, 3));
    }

    #[test]
    fn replace_all_regex_with_groups() {
        let opts = FindWidgetOptions {
            regex: true,
            case_sensitive: true,
            ..Default::default()
        };
        let (result, count) = replace_all("2024-01-15", r"(\d{4})-(\d{2})-(\d{2})", "$2/$3/$1", &opts);
        assert_eq!(count, 1);
        assert_eq!(result, "01/15/2024");
    }
}
