//! Inline suggestions (ghost text) — mirrors VS Code's
//! `InlineSuggestController` + `GhostTextWidget`.
//!
//! Shows dimmed text after the cursor that can be accepted with Tab,
//! accepted word-by-word with Ctrl+Right, cycled with Alt+]/[, or
//! dismissed with Escape.

use sidex_text::Range;

/// A single inline completion suggestion (ghost text).
#[derive(Debug, Clone)]
pub struct InlineSuggestion {
    /// The text to insert when accepted.
    pub text: String,
    /// The range in the document this suggestion replaces / inserts at.
    pub range: Range,
    /// Optional command to execute after accepting.
    pub command: Option<String>,
    /// Whether the insertion text is a snippet template.
    pub is_snippet: bool,
}

/// How an inline suggestion was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineSuggestTriggerKind {
    /// Automatic (typing, cursor move).
    Automatic,
    /// Explicitly invoked (e.g. Alt+\).
    Explicit,
}

impl Default for InlineSuggestTriggerKind {
    fn default() -> Self {
        Self::Automatic
    }
}

/// Internal state for the current inline suggestion session.
#[derive(Debug, Clone, Default)]
pub struct InlineSuggestState {
    /// The suggestion currently rendered as ghost text.
    pub current: Option<InlineSuggestion>,
    /// Alternative suggestions the user can cycle through.
    pub alternatives: Vec<InlineSuggestion>,
    /// Index into `alternatives` for the currently displayed suggestion.
    pub current_index: usize,
    /// Whether ghost text is visible right now.
    pub is_visible: bool,
}

/// Full controller for the inline-suggest feature.
#[derive(Debug, Clone)]
pub struct InlineSuggestController {
    /// Session state.
    pub state: InlineSuggestState,
    /// Whether the feature is enabled globally.
    pub enabled: bool,
    /// Whether to show the accept/next/prev/dismiss toolbar.
    pub show_toolbar: bool,
    /// Whether suggestions are auto-triggered on typing.
    pub auto_trigger: bool,
    /// How many characters to type before auto-triggering.
    pub min_trigger_length: usize,
    /// How the current session was triggered.
    pub trigger_kind: InlineSuggestTriggerKind,
    /// Provider identifier (e.g. "copilot", "codeium").
    pub provider: Option<String>,
    /// Whether a request is in-flight.
    pub is_loading: bool,
}

impl Default for InlineSuggestController {
    fn default() -> Self {
        Self {
            state: InlineSuggestState::default(),
            enabled: true,
            show_toolbar: true,
            auto_trigger: true,
            min_trigger_length: 1,
            trigger_kind: InlineSuggestTriggerKind::Automatic,
            provider: None,
            is_loading: false,
        }
    }
}

impl InlineSuggestController {
    /// Shows a single inline suggestion as ghost text.
    pub fn show_inline_suggestion(&mut self, suggestion: InlineSuggestion) {
        self.state.alternatives = vec![suggestion.clone()];
        self.state.current = Some(suggestion);
        self.state.current_index = 0;
        self.state.is_visible = true;
        self.is_loading = false;
    }

    /// Provides multiple alternative suggestions. The first one is displayed.
    pub fn show_alternatives(&mut self, suggestions: Vec<InlineSuggestion>) {
        if suggestions.is_empty() {
            self.dismiss();
            return;
        }
        self.state.current = Some(suggestions[0].clone());
        self.state.alternatives = suggestions;
        self.state.current_index = 0;
        self.state.is_visible = true;
        self.is_loading = false;
    }

    /// Accepts the entire current suggestion. Returns the suggestion to insert.
    pub fn accept_suggestion(&mut self) -> Option<InlineSuggestion> {
        let accepted = self.state.current.take();
        self.clear_session();
        accepted
    }

    /// Accepts only the next word from the current ghost text.
    /// Returns `Some((word_text, remaining_text))` or `None`.
    pub fn accept_next_word(&mut self) -> Option<(String, Option<String>)> {
        let suggestion = self.state.current.as_mut()?;
        let text = &suggestion.text;
        if text.is_empty() {
            self.dismiss();
            return None;
        }

        let first_non_ws = text
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let word_end = text[first_non_ws..]
            .char_indices()
            .find(|(_, c)| c.is_whitespace() || *c == '.' || *c == '(' || *c == ',' || *c == ';')
            .map(|(i, _)| first_non_ws + i)
            .unwrap_or(text.len());

        let word_end = if word_end == first_non_ws {
            (word_end + 1).min(text.len())
        } else {
            word_end
        };

        let word = text[..word_end].to_string();
        let remaining = if word_end < text.len() {
            Some(text[word_end..].to_string())
        } else {
            None
        };

        if let Some(ref rest) = remaining {
            suggestion.text = rest.clone();
        } else {
            self.dismiss();
        }

        Some((word, remaining))
    }

    /// Cycles to the next alternative suggestion. Wraps around.
    pub fn next_alternative(&mut self) {
        if self.state.alternatives.is_empty() {
            return;
        }
        self.state.current_index =
            (self.state.current_index + 1) % self.state.alternatives.len();
        self.state.current = Some(self.state.alternatives[self.state.current_index].clone());
    }

    /// Cycles to the previous alternative suggestion. Wraps around.
    pub fn prev_alternative(&mut self) {
        if self.state.alternatives.is_empty() {
            return;
        }
        self.state.current_index = if self.state.current_index == 0 {
            self.state.alternatives.len() - 1
        } else {
            self.state.current_index - 1
        };
        self.state.current = Some(self.state.alternatives[self.state.current_index].clone());
    }

    /// Dismisses the ghost text and clears the session.
    pub fn dismiss(&mut self) {
        self.clear_session();
    }

    /// Initiates a request for inline suggestions (marks loading).
    pub fn request(&mut self, kind: InlineSuggestTriggerKind) {
        if !self.enabled {
            return;
        }
        self.trigger_kind = kind;
        self.is_loading = true;
    }

    /// Returns `true` if ghost text is currently rendered.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.state.is_visible && self.state.current.is_some()
    }

    /// Returns the text currently shown as ghost text.
    #[must_use]
    pub fn ghost_text(&self) -> Option<&str> {
        self.state.current.as_ref().map(|s| s.text.as_str())
    }

    /// Returns `(current_1based, total)` for toolbar display.
    #[must_use]
    pub fn alternatives_display(&self) -> (usize, usize) {
        let total = self.state.alternatives.len();
        if total == 0 {
            (0, 0)
        } else {
            (self.state.current_index + 1, total)
        }
    }

    /// Returns `true` when the toolbar should be rendered.
    #[must_use]
    pub fn should_show_toolbar(&self) -> bool {
        self.show_toolbar && self.is_visible()
    }

    fn clear_session(&mut self) {
        self.state.current = None;
        self.state.alternatives.clear();
        self.state.current_index = 0;
        self.state.is_visible = false;
        self.is_loading = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidex_text::Position;

    fn make_suggestion(text: &str) -> InlineSuggestion {
        InlineSuggestion {
            text: text.to_string(),
            range: Range::new(Position::new(0, 5), Position::new(0, 5)),
            command: None,
            is_snippet: false,
        }
    }

    #[test]
    fn show_and_accept() {
        let mut ctrl = InlineSuggestController::default();
        assert!(!ctrl.is_visible());

        ctrl.show_inline_suggestion(make_suggestion("hello()"));
        assert!(ctrl.is_visible());
        assert_eq!(ctrl.ghost_text(), Some("hello()"));

        let accepted = ctrl.accept_suggestion();
        assert!(accepted.is_some());
        assert_eq!(accepted.unwrap().text, "hello()");
        assert!(!ctrl.is_visible());
    }

    #[test]
    fn cycle_alternatives() {
        let mut ctrl = InlineSuggestController::default();
        ctrl.show_alternatives(vec![
            make_suggestion("alpha"),
            make_suggestion("beta"),
            make_suggestion("gamma"),
        ]);

        assert_eq!(ctrl.ghost_text(), Some("alpha"));
        assert_eq!(ctrl.alternatives_display(), (1, 3));

        ctrl.next_alternative();
        assert_eq!(ctrl.ghost_text(), Some("beta"));
        assert_eq!(ctrl.alternatives_display(), (2, 3));

        ctrl.next_alternative();
        assert_eq!(ctrl.ghost_text(), Some("gamma"));

        ctrl.next_alternative();
        assert_eq!(ctrl.ghost_text(), Some("alpha")); // wraps

        ctrl.prev_alternative();
        assert_eq!(ctrl.ghost_text(), Some("gamma")); // wraps back
    }

    #[test]
    fn dismiss_clears_state() {
        let mut ctrl = InlineSuggestController::default();
        ctrl.show_inline_suggestion(make_suggestion("test"));
        ctrl.dismiss();
        assert!(!ctrl.is_visible());
        assert!(ctrl.state.alternatives.is_empty());
        assert!(ctrl.ghost_text().is_none());
    }

    #[test]
    fn accept_next_word() {
        let mut ctrl = InlineSuggestController::default();
        ctrl.show_inline_suggestion(make_suggestion("hello world done"));

        let (word, remaining) = ctrl.accept_next_word().unwrap();
        assert_eq!(word, "hello");
        assert_eq!(remaining, Some(" world done".to_string()));
        assert!(ctrl.is_visible());

        let (word, remaining) = ctrl.accept_next_word().unwrap();
        assert_eq!(word, " world");
        assert!(remaining.is_some());
    }

    #[test]
    fn accept_next_word_exhausts() {
        let mut ctrl = InlineSuggestController::default();
        ctrl.show_inline_suggestion(make_suggestion("x"));

        let (word, remaining) = ctrl.accept_next_word().unwrap();
        assert_eq!(word, "x");
        assert!(remaining.is_none());
        assert!(!ctrl.is_visible());
    }

    #[test]
    fn show_empty_alternatives_dismisses() {
        let mut ctrl = InlineSuggestController::default();
        ctrl.show_alternatives(vec![]);
        assert!(!ctrl.is_visible());
    }

    #[test]
    fn disabled_request_is_noop() {
        let mut ctrl = InlineSuggestController::default();
        ctrl.enabled = false;
        ctrl.request(InlineSuggestTriggerKind::Automatic);
        assert!(!ctrl.is_loading);
    }

    #[test]
    fn toolbar_visibility() {
        let mut ctrl = InlineSuggestController::default();
        assert!(!ctrl.should_show_toolbar());

        ctrl.show_inline_suggestion(make_suggestion("foo"));
        assert!(ctrl.should_show_toolbar());

        ctrl.show_toolbar = false;
        assert!(!ctrl.should_show_toolbar());
    }

    #[test]
    fn accept_next_word_at_punctuation() {
        let mut ctrl = InlineSuggestController::default();
        ctrl.show_inline_suggestion(make_suggestion("foo(bar, baz)"));

        let (word, _) = ctrl.accept_next_word().unwrap();
        assert_eq!(word, "foo");
    }
}
