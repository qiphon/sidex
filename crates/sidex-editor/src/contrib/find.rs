//! Find/Replace widget state — mirrors VS Code's `FindReplaceState` +
//! `FindModel` + `FindDecorations`.
//!
//! This module owns the search query, match list, active match index, and
//! replacement logic. The renderer reads [`FindState`] to highlight matches
//! and position the find widget.

use sidex_text::search::{find_matches, FindMatch, FindMatchesOptions};
use sidex_text::{Buffer, Position, Range};

/// Maximum matches tracked before the engine stops counting.
pub const MATCHES_LIMIT: usize = 19_999;

/// Maximum length of a search string to prevent pathological regex.
pub const SEARCH_STRING_MAX_LENGTH: usize = 524_288;

/// Search option toggles (regex, case-sensitivity, whole word, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FindOptions {
    pub is_regex: bool,
    pub match_case: bool,
    pub whole_word: bool,
    pub preserve_case: bool,
    /// When true, searching is restricted to the current selection.
    pub search_in_selection: bool,
    /// Whether the search should wrap around the document.
    pub wrap_around: bool,
}

impl Default for FindOptions {
    fn default() -> Self {
        Self {
            is_regex: false,
            match_case: false,
            whole_word: false,
            preserve_case: false,
            search_in_selection: false,
            wrap_around: true,
        }
    }
}

/// Focus target when revealing the find widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindStartFocus {
    NoChange,
    FindInput,
    ReplaceInput,
}

/// Layout / geometry state of the find widget for rendering.
#[derive(Debug, Clone)]
pub struct FindWidgetLayout {
    /// Pixel X offset from the right edge of the editor.
    pub right_offset: f32,
    /// Pixel Y offset from the top of the editor.
    pub top_offset: f32,
    /// Widget width in pixels.
    pub width: f32,
    /// Whether the replace row is expanded (affects height).
    pub replace_expanded: bool,
    /// Whether the widget is currently animating open/close.
    pub is_animating: bool,
}

impl Default for FindWidgetLayout {
    fn default() -> Self {
        Self {
            right_offset: 14.0,
            top_offset: 0.0,
            width: 411.0,
            replace_expanded: false,
            is_animating: false,
        }
    }
}

/// Full state of the find/replace widget.
#[derive(Debug, Clone, Default)]
pub struct FindState {
    /// The current search string entered by the user.
    pub search_string: String,
    /// The current replacement string.
    pub replace_string: String,
    /// Whether the find widget is visible.
    pub is_revealed: bool,
    /// Whether the replace row is revealed.
    pub is_replace_revealed: bool,
    /// Search option toggles.
    pub options: FindOptions,
    /// All matches in the document for the current query.
    pub matches: Vec<FindMatch>,
    /// Zero-based index of the currently active match, or `None`.
    pub active_match_idx: Option<usize>,
    /// Ranges to restrict the search to (when `search_in_selection` is true).
    pub search_scope: Option<Vec<Range>>,
    /// Search history (most-recent first).
    pub search_history: Vec<String>,
    /// Replace history (most-recent first).
    pub replace_history: Vec<String>,
    /// Widget layout state for the renderer.
    pub layout: FindWidgetLayout,
    /// Count display overflow flag — true when matches exceed `MATCHES_LIMIT`.
    pub match_count_overflow: bool,
}

impl FindState {
    /// Opens the find widget, optionally seeding the search string.
    pub fn reveal(&mut self, seed: Option<&str>) {
        self.is_revealed = true;
        self.layout.is_animating = true;
        if let Some(s) = seed {
            self.set_search_string(s.to_string());
        }
    }

    /// Opens the find widget with replace row visible.
    pub fn reveal_replace(&mut self, seed: Option<&str>) {
        self.reveal(seed);
        self.is_replace_revealed = true;
        self.layout.replace_expanded = true;
    }

    /// Closes the find widget and clears match highlights.
    pub fn dismiss(&mut self) {
        self.is_revealed = false;
        self.is_replace_revealed = false;
        self.layout.is_animating = true;
        self.matches.clear();
        self.active_match_idx = None;
        self.match_count_overflow = false;
    }

    /// Updates the search string and pushes it into history.
    pub fn set_search_string(&mut self, s: String) {
        if s.len() > SEARCH_STRING_MAX_LENGTH {
            return;
        }
        if !s.is_empty() && self.search_history.first() != Some(&s) {
            self.search_history.insert(0, s.clone());
            if self.search_history.len() > 50 {
                self.search_history.truncate(50);
            }
        }
        self.search_string = s;
    }

    /// Updates the replace string and pushes it into history.
    pub fn set_replace_string(&mut self, s: String) {
        if !s.is_empty() && self.replace_history.first() != Some(&s) {
            self.replace_history.insert(0, s.clone());
            if self.replace_history.len() > 50 {
                self.replace_history.truncate(50);
            }
        }
        self.replace_string = s;
    }

    /// Seeds the search from the current selection, mirroring VS Code's
    /// `getSelectionSearchString`.
    pub fn seed_from_selection(&mut self, buffer: &Buffer, sel: Range) {
        if sel.start.line != sel.end.line {
            self.toggle_search_in_selection_with_scope(vec![sel]);
            return;
        }
        let start = buffer.position_to_offset(sel.start);
        let end = buffer.position_to_offset(sel.end);
        if start == end {
            return;
        }
        let text = buffer.slice(start..end);
        if text.len() <= SEARCH_STRING_MAX_LENGTH {
            self.set_search_string(text);
        }
    }

    /// Enables search-in-selection with explicit scope ranges (e.g. from
    /// a multi-line selection).
    pub fn toggle_search_in_selection_with_scope(&mut self, scopes: Vec<Range>) {
        self.options.search_in_selection = true;
        self.search_scope = Some(scopes);
    }

    // ── Toggle helpers ──────────────────────────────────────────────────

    pub fn toggle_regex(&mut self) {
        self.options.is_regex = !self.options.is_regex;
    }

    pub fn toggle_case_sensitive(&mut self) {
        self.options.match_case = !self.options.match_case;
    }

    pub fn toggle_whole_word(&mut self) {
        self.options.whole_word = !self.options.whole_word;
    }

    pub fn toggle_preserve_case(&mut self) {
        self.options.preserve_case = !self.options.preserve_case;
    }

    pub fn toggle_search_in_selection(&mut self) {
        self.options.search_in_selection = !self.options.search_in_selection;
        if !self.options.search_in_selection {
            self.search_scope = None;
        }
    }

    // ── Core search operations ──────────────────────────────────────────

    /// Re-runs the search against `buffer`, populating `self.matches`.
    pub fn research(&mut self, buffer: &Buffer) {
        if self.search_string.is_empty() {
            self.matches.clear();
            self.active_match_idx = None;
            self.match_count_overflow = false;
            return;
        }

        let scope = if self.options.search_in_selection {
            self.search_scope.clone()
        } else {
            None
        };

        let opts = FindMatchesOptions {
            search_string: self.search_string.clone(),
            search_scope: scope,
            is_regex: self.options.is_regex,
            match_case: self.options.match_case,
            word_separators: if self.options.whole_word {
                Some(String::new())
            } else {
                None
            },
            capture_matches: self.options.is_regex,
            limit_result_count: MATCHES_LIMIT,
        };

        self.matches = find_matches(buffer, &opts);
        self.match_count_overflow = self.matches.len() >= MATCHES_LIMIT;

        if self.matches.is_empty() {
            self.active_match_idx = None;
        } else if let Some(idx) = self.active_match_idx {
            if idx >= self.matches.len() {
                self.active_match_idx = Some(0);
            }
        } else {
            self.active_match_idx = Some(0);
        }
    }

    /// Returns the currently active match range, if any.
    #[must_use]
    pub fn current_match(&self) -> Option<&FindMatch> {
        self.active_match_idx.and_then(|i| self.matches.get(i))
    }

    /// Advances to the next match, wrapping if enabled.  Returns the new
    /// active match index.
    pub fn find_next(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        let next = match self.active_match_idx {
            Some(i) => {
                if i + 1 < self.matches.len() {
                    i + 1
                } else if self.options.wrap_around {
                    0
                } else {
                    return self.active_match_idx;
                }
            }
            None => 0,
        };
        self.active_match_idx = Some(next);
        self.active_match_idx
    }

    /// Moves to the previous match, wrapping if enabled.
    pub fn find_previous(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        let prev = match self.active_match_idx {
            Some(0) => {
                if self.options.wrap_around {
                    self.matches.len() - 1
                } else {
                    return self.active_match_idx;
                }
            }
            Some(i) => i - 1,
            None => self.matches.len() - 1,
        };
        self.active_match_idx = Some(prev);
        self.active_match_idx
    }

    /// Moves the active match to the one closest to `pos` (at or after).
    pub fn find_nearest(&mut self, pos: Position) {
        if self.matches.is_empty() {
            self.active_match_idx = None;
            return;
        }
        let idx = self
            .matches
            .iter()
            .position(|m| m.range.start >= pos)
            .unwrap_or(0);
        self.active_match_idx = Some(idx);
    }

    /// Replaces the current match with `self.replace_string` and advances.
    /// Returns the replacement text that was applied, if any.
    pub fn replace_current(&mut self, buffer: &mut Buffer) -> Option<String> {
        let idx = self.active_match_idx?;
        let m = self.matches.get(idx)?;
        let range = m.range;
        let captures = m.matches.clone();
        let matched_text = buffer
            .slice(buffer.position_to_offset(range.start)..buffer.position_to_offset(range.end));
        let replacement = self.replacement_text(&matched_text, &captures);
        let start = buffer.position_to_offset(range.start);
        let end = buffer.position_to_offset(range.end);
        buffer.replace(start..end, &replacement);
        self.research(buffer);
        Some(replacement)
    }

    /// Replaces all matches. Returns the number of replacements made.
    pub fn replace_all(&mut self, buffer: &mut Buffer) -> usize {
        if self.matches.is_empty() {
            return 0;
        }
        let mut count = 0;
        let matches: Vec<_> = self.matches.iter().rev().cloned().collect();
        for m in &matches {
            let matched_text = buffer.slice(
                buffer.position_to_offset(m.range.start)..buffer.position_to_offset(m.range.end),
            );
            let replacement = self.replacement_text(&matched_text, &m.matches);
            let start = buffer.position_to_offset(m.range.start);
            let end = buffer.position_to_offset(m.range.end);
            buffer.replace(start..end, &replacement);
            count += 1;
        }
        self.research(buffer);
        count
    }

    /// Returns all match ranges for decoration/highlighting.
    #[must_use]
    pub fn match_ranges(&self) -> Vec<Range> {
        self.matches.iter().map(|m| m.range).collect()
    }

    /// Returns `(current_1based, total)` for status display, e.g. "3 of 42".
    #[must_use]
    pub fn match_count_display(&self) -> (usize, usize) {
        let total = self.matches.len();
        let current = self.active_match_idx.map_or(0, |i| i + 1);
        (current, total)
    }

    /// Returns a formatted status string like "3 of 42" or "No results" or
    /// "99999+ of 99999+".
    #[must_use]
    pub fn match_count_label(&self) -> String {
        let (current, total) = self.match_count_display();
        if total == 0 {
            if self.search_string.is_empty() {
                String::new()
            } else {
                "No results".to_string()
            }
        } else if self.match_count_overflow {
            format!("{current} of {total}+")
        } else {
            format!("{current} of {total}")
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────

    /// Builds the replacement text handling:
    /// - Regex capture group references ($0, $1, ..., $n)
    /// - Preserve-case transforms
    fn replacement_text(&self, matched_text: &str, captures: &[String]) -> String {
        let mut result = self.replace_string.clone();

        if self.options.is_regex && !captures.is_empty() {
            result = Self::expand_capture_groups(&result, captures);
        }

        if self.options.preserve_case {
            result = Self::apply_preserve_case(matched_text, &result);
        }

        result
    }

    /// Expands `$0`, `$1` ... `$9` and `${nn}` references in the replacement
    /// string with captured groups.
    fn expand_capture_groups(pattern: &str, captures: &[String]) -> String {
        let mut result = String::with_capacity(pattern.len());
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '$' && i + 1 < chars.len() {
                if chars[i + 1] == '{' {
                    if let Some(close) = chars[i + 2..].iter().position(|&c| c == '}') {
                        let num_str: String = chars[i + 2..i + 2 + close].iter().collect();
                        if let Ok(n) = num_str.parse::<usize>() {
                            if let Some(cap) = captures.get(n) {
                                result.push_str(cap);
                            }
                            i += 3 + close;
                            continue;
                        }
                    }
                } else if chars[i + 1].is_ascii_digit() {
                    let n = (chars[i + 1] as u32 - '0' as u32) as usize;
                    if let Some(cap) = captures.get(n) {
                        result.push_str(cap);
                    }
                    i += 2;
                    continue;
                }
            }
            if chars[i] == '\\' && i + 1 < chars.len() {
                match chars[i + 1] {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    '\\' => result.push('\\'),
                    other => {
                        result.push('\\');
                        result.push(other);
                    }
                }
                i += 2;
                continue;
            }
            result.push(chars[i]);
            i += 1;
        }
        result
    }

    /// Applies preserve-case heuristics from VS Code:
    /// - If match is all upper, replacement is uppercased
    /// - If match is all lower, replacement is lowercased
    /// - If match starts with upper, replacement's first char is uppercased
    fn apply_preserve_case(matched: &str, replacement: &str) -> String {
        if matched.is_empty() || replacement.is_empty() {
            return replacement.to_string();
        }
        let matched_chars: Vec<char> = matched.chars().collect();
        let all_upper = matched_chars
            .iter()
            .all(|c| !c.is_alphabetic() || c.is_uppercase());
        let all_lower = matched_chars
            .iter()
            .all(|c| !c.is_alphabetic() || c.is_lowercase());

        if all_upper && matched_chars.iter().any(|c| c.is_alphabetic()) {
            return replacement.to_uppercase();
        }
        if all_lower && matched_chars.iter().any(|c| c.is_alphabetic()) {
            return replacement.to_lowercase();
        }
        if matched_chars[0].is_uppercase() {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_buffer(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn basic_find_and_navigate() {
        let buf = make_buffer("foo bar foo baz foo");
        let mut state = FindState::default();
        state.set_search_string("foo".into());
        state.research(&buf);

        assert_eq!(state.matches.len(), 3);
        assert_eq!(state.active_match_idx, Some(0));

        state.find_next();
        assert_eq!(state.active_match_idx, Some(1));

        state.find_next();
        assert_eq!(state.active_match_idx, Some(2));

        // wrap around
        state.find_next();
        assert_eq!(state.active_match_idx, Some(0));
    }

    #[test]
    fn find_previous_wraps() {
        let buf = make_buffer("a a a");
        let mut state = FindState::default();
        state.set_search_string("a".into());
        state.research(&buf);

        assert_eq!(state.active_match_idx, Some(0));
        state.find_previous();
        assert_eq!(state.active_match_idx, Some(2));
    }

    #[test]
    fn replace_all_returns_count() {
        let mut buf = make_buffer("aaa");
        let mut state = FindState::default();
        state.set_search_string("a".into());
        state.set_replace_string("bb".into());
        state.research(&buf);

        let count = state.replace_all(&mut buf);
        assert_eq!(count, 3);
        assert_eq!(buf.text(), "bbbbbb");
    }

    #[test]
    fn dismiss_clears() {
        let buf = make_buffer("hello");
        let mut state = FindState::default();
        state.set_search_string("hello".into());
        state.research(&buf);
        assert!(!state.matches.is_empty());

        state.dismiss();
        assert!(state.matches.is_empty());
        assert!(!state.is_revealed);
    }

    #[test]
    fn match_count_label_formatting() {
        let buf = make_buffer("ab ab ab");
        let mut state = FindState::default();
        state.set_search_string("ab".into());
        state.research(&buf);
        assert_eq!(state.match_count_label(), "1 of 3");

        state.set_search_string("zzz".into());
        state.research(&buf);
        assert_eq!(state.match_count_label(), "No results");

        state.set_search_string(String::new());
        state.research(&buf);
        assert!(state.match_count_label().is_empty());
    }

    #[test]
    fn preserve_case_all_upper() {
        let result = FindState::apply_preserve_case("FOO", "bar");
        assert_eq!(result, "BAR");
    }

    #[test]
    fn preserve_case_first_upper() {
        let result = FindState::apply_preserve_case("Foo", "bar");
        assert_eq!(result, "Bar");
    }

    #[test]
    fn preserve_case_all_lower() {
        let result = FindState::apply_preserve_case("foo", "BAR");
        assert_eq!(result, "bar");
    }

    #[test]
    fn capture_group_expansion() {
        let result =
            FindState::expand_capture_groups("$1-$2", &["full".into(), "a".into(), "b".into()]);
        assert_eq!(result, "a-b");
    }

    #[test]
    fn capture_group_braces() {
        let result = FindState::expand_capture_groups("${0}!", &["hello".into()]);
        assert_eq!(result, "hello!");
    }

    #[test]
    fn escape_sequences_in_replace() {
        let result = FindState::expand_capture_groups("a\\nb", &[]);
        assert_eq!(result, "a\nb");
    }

    #[test]
    fn reveal_replace_sets_flags() {
        let mut state = FindState::default();
        state.reveal_replace(Some("test"));
        assert!(state.is_revealed);
        assert!(state.is_replace_revealed);
        assert!(state.layout.replace_expanded);
        assert_eq!(state.search_string, "test");
    }

    #[test]
    fn rejects_oversized_search_string() {
        let mut state = FindState::default();
        let huge = "x".repeat(SEARCH_STRING_MAX_LENGTH + 1);
        state.set_search_string(huge);
        assert!(state.search_string.is_empty());
    }
}
