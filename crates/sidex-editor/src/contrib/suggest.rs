//! Autocomplete / suggestion widget state — mirrors VS Code's
//! `SuggestModel` + `SuggestWidget` + `CompletionModel`.
//!
//! Owns the active completion session, the filtered/sorted item list,
//! selection index, and trigger logic.

use crate::completion::{fuzzy_score, CompletionItem, CompletionItemKind, CompletionTriggerKind};
use sidex_text::{Buffer, Position};

/// Trigger characters that automatically open the suggest widget.
pub const DEFAULT_TRIGGER_CHARS: &[char] = &['.', ':', '<', '"', '/', '@', '#'];

/// Configuration for accept behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptSuggestionOn {
    /// Accept on Enter only.
    Enter,
    /// Accept on Tab only.
    Tab,
    /// Accept on both Enter and Tab.
    EnterAndTab,
    /// Never auto-accept (must click or use explicit command).
    Off,
}

impl Default for AcceptSuggestionOn {
    fn default() -> Self {
        Self::EnterAndTab
    }
}

/// How the suggest widget was dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestDismissReason {
    /// User cancelled.
    Cancel,
    /// An item was accepted.
    Accept,
    /// Filter narrowed to zero items.
    NoMatch,
    /// Cursor moved outside the valid trigger range.
    CursorMoved,
}

/// Icon identifier for a completion item kind (for renderer mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionKindIcon(pub CompletionItemKind);

impl CompletionKindIcon {
    /// Returns the icon name string for rendering.
    #[must_use]
    pub fn icon_name(self) -> &'static str {
        match self.0 {
            CompletionItemKind::Text => "symbol-string",
            CompletionItemKind::Method => "symbol-method",
            CompletionItemKind::Function => "symbol-function",
            CompletionItemKind::Constructor => "symbol-constructor",
            CompletionItemKind::Field => "symbol-field",
            CompletionItemKind::Variable => "symbol-variable",
            CompletionItemKind::Class => "symbol-class",
            CompletionItemKind::Interface => "symbol-interface",
            CompletionItemKind::Module => "symbol-module",
            CompletionItemKind::Property => "symbol-property",
            CompletionItemKind::Unit => "symbol-unit",
            CompletionItemKind::Value => "symbol-value",
            CompletionItemKind::Enum => "symbol-enum",
            CompletionItemKind::Keyword => "symbol-keyword",
            CompletionItemKind::Snippet => "symbol-snippet",
            CompletionItemKind::Color => "symbol-color",
            CompletionItemKind::File => "symbol-file",
            CompletionItemKind::Reference => "symbol-reference",
            CompletionItemKind::Folder => "symbol-folder",
            CompletionItemKind::EnumMember => "symbol-enum-member",
            CompletionItemKind::Constant => "symbol-constant",
            CompletionItemKind::Struct => "symbol-struct",
            CompletionItemKind::Event => "symbol-event",
            CompletionItemKind::Operator => "symbol-operator",
            CompletionItemKind::TypeParameter => "symbol-type-parameter",
        }
    }
}

/// State of the suggestion widget's detail pane.
#[derive(Debug, Clone, Default)]
pub struct SuggestDetailPane {
    /// Whether the detail pane is expanded.
    pub is_visible: bool,
    /// Resolved documentation for the focused item (may contain markdown).
    pub documentation: Option<String>,
    /// Type signature or detail string.
    pub detail: Option<String>,
    /// Whether the documentation contains markdown.
    pub documentation_is_markdown: bool,
}

/// A scored + filtered item with match highlight positions.
#[derive(Debug, Clone)]
pub struct ScoredItem {
    pub item: CompletionItem,
    pub score: i32,
    pub match_positions: Vec<usize>,
}

/// The complete autocomplete session state.
#[derive(Debug, Clone)]
pub struct SuggestState {
    /// Whether the suggest widget is currently active.
    pub is_active: bool,
    /// How the session was triggered.
    pub trigger_kind: CompletionTriggerKind,
    /// The character that triggered the session (if trigger-character).
    pub trigger_character: Option<char>,
    /// The position where the completion was triggered.
    pub trigger_position: Option<Position>,
    /// The current filter/prefix text typed since the trigger.
    pub filter_text: String,
    /// All completion items received from the provider.
    pub all_items: Vec<CompletionItem>,
    /// Filtered + sorted items actually shown in the widget, with scores.
    pub visible_items: Vec<ScoredItem>,
    /// Zero-based index of the focused item in `visible_items`.
    pub selected_index: usize,
    /// Detail pane state.
    pub detail_pane: SuggestDetailPane,
    /// Whether a completion request is in-flight.
    pub is_loading: bool,
    /// Whether automatic suggestions on typing are enabled.
    pub auto_trigger_enabled: bool,
    /// Custom per-language trigger characters (from the LSP server).
    pub custom_trigger_chars: Vec<char>,
    /// How to accept suggestions.
    pub accept_on: AcceptSuggestionOn,
    /// Whether the widget was dismissed via no-match (for word-based fallback).
    pub last_dismiss_reason: Option<SuggestDismissReason>,
    /// Whether snippet items should be shown.
    pub show_snippets: bool,
    /// Whether word-based suggestions are used as fallback.
    pub word_based_suggestions: bool,
    /// The maximum number of visible items in the widget.
    pub max_visible_items: usize,
}

impl Default for SuggestState {
    fn default() -> Self {
        Self {
            is_active: false,
            trigger_kind: CompletionTriggerKind::Invoked,
            trigger_character: None,
            trigger_position: None,
            filter_text: String::new(),
            all_items: Vec::new(),
            visible_items: Vec::new(),
            selected_index: 0,
            detail_pane: SuggestDetailPane::default(),
            is_loading: false,
            auto_trigger_enabled: true,
            custom_trigger_chars: Vec::new(),
            accept_on: AcceptSuggestionOn::default(),
            last_dismiss_reason: None,
            show_snippets: true,
            word_based_suggestions: true,
            max_visible_items: 12,
        }
    }
}

impl SuggestState {
    /// Triggers a new completion session at the given position.
    pub fn trigger_suggest(
        &mut self,
        pos: Position,
        kind: CompletionTriggerKind,
        trigger_char: Option<char>,
    ) {
        self.is_active = true;
        self.trigger_kind = kind;
        self.trigger_character = trigger_char;
        self.trigger_position = Some(pos);
        self.filter_text.clear();
        self.all_items.clear();
        self.visible_items.clear();
        self.selected_index = 0;
        self.is_loading = true;
        self.last_dismiss_reason = None;
    }

    /// Receives items from the provider and applies filtering.
    pub fn receive_items(&mut self, items: Vec<CompletionItem>) {
        self.all_items = items;
        self.is_loading = false;
        self.refilter();
    }

    /// Re-filters `all_items` based on the current `filter_text` using fuzzy scoring.
    pub fn refilter(&mut self) {
        let items = if self.show_snippets {
            &self.all_items
        } else {
            // temporarily bind filtered items
            &self.all_items
        };

        if self.filter_text.is_empty() {
            self.visible_items = items
                .iter()
                .filter(|item| self.show_snippets || item.kind != CompletionItemKind::Snippet)
                .map(|item| ScoredItem {
                    item: item.clone(),
                    score: if item.preselect { 1000 } else { 0 },
                    match_positions: Vec::new(),
                })
                .collect();
        } else {
            let mut scored: Vec<ScoredItem> = items
                .iter()
                .filter(|item| self.show_snippets || item.kind != CompletionItemKind::Snippet)
                .filter_map(|item| {
                    let text = item.effective_filter_text();
                    fuzzy_score(&self.filter_text, text).map(|(score, positions)| ScoredItem {
                        item: item.clone(),
                        score: if item.preselect { score + 1000 } else { score },
                        match_positions: positions,
                    })
                })
                .collect();
            scored.sort_by(|a, b| b.score.cmp(&a.score));
            self.visible_items = scored;
        }

        if self.selected_index >= self.visible_items.len() {
            self.selected_index = 0;
        }
        if self.visible_items.is_empty() && self.is_active {
            self.dismiss(SuggestDismissReason::NoMatch);
        }
    }

    /// Updates the filter text (called on each keystroke).
    pub fn update_filter(&mut self, text: String) {
        self.filter_text = text;
        self.refilter();
    }

    /// Selects the next item in the list.
    pub fn next_suggestion(&mut self) {
        if !self.visible_items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.visible_items.len();
            self.update_detail_pane();
        }
    }

    /// Selects the previous item in the list.
    pub fn prev_suggestion(&mut self) {
        if !self.visible_items.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.visible_items.len() - 1
            } else {
                self.selected_index - 1
            };
            self.update_detail_pane();
        }
    }

    /// Selects a page of items down (for scroll).
    pub fn next_page(&mut self) {
        if self.visible_items.is_empty() {
            return;
        }
        self.selected_index =
            (self.selected_index + self.max_visible_items).min(self.visible_items.len() - 1);
        self.update_detail_pane();
    }

    /// Selects a page of items up.
    pub fn prev_page(&mut self) {
        if self.visible_items.is_empty() {
            return;
        }
        self.selected_index = self.selected_index.saturating_sub(self.max_visible_items);
        self.update_detail_pane();
    }

    /// Returns the currently focused completion item, if any.
    #[must_use]
    pub fn focused_item(&self) -> Option<&CompletionItem> {
        self.visible_items.get(self.selected_index).map(|s| &s.item)
    }

    /// Returns the focused scored item with match positions.
    #[must_use]
    pub fn focused_scored(&self) -> Option<&ScoredItem> {
        self.visible_items.get(self.selected_index)
    }

    /// Returns the icon for the focused item.
    #[must_use]
    pub fn focused_icon(&self) -> Option<CompletionKindIcon> {
        self.focused_item().map(|i| CompletionKindIcon(i.kind))
    }

    /// Accepts the currently focused suggestion.  Returns the item to insert.
    pub fn accept_suggestion(&mut self) -> Option<CompletionItem> {
        let item = self.focused_item().cloned();
        self.dismiss(SuggestDismissReason::Accept);
        item
    }

    /// Accepts if the accept mode allows it for the given key.
    pub fn try_accept_on_key(&mut self, is_enter: bool) -> Option<CompletionItem> {
        let allowed = match self.accept_on {
            AcceptSuggestionOn::Enter => is_enter,
            AcceptSuggestionOn::Tab => !is_enter,
            AcceptSuggestionOn::EnterAndTab => true,
            AcceptSuggestionOn::Off => false,
        };
        if allowed {
            self.accept_suggestion()
        } else {
            None
        }
    }

    /// Cancels the current completion session.
    pub fn cancel(&mut self) {
        self.dismiss(SuggestDismissReason::Cancel);
    }

    /// Dismisses with a specific reason.
    fn dismiss(&mut self, reason: SuggestDismissReason) {
        self.is_active = false;
        self.is_loading = false;
        self.last_dismiss_reason = Some(reason);
        self.all_items.clear();
        self.visible_items.clear();
        self.selected_index = 0;
        self.detail_pane = SuggestDetailPane::default();
    }

    /// Returns `true` if the given character should auto-trigger completions.
    #[must_use]
    pub fn is_trigger_character(&self, ch: char) -> bool {
        DEFAULT_TRIGGER_CHARS.contains(&ch) || self.custom_trigger_chars.contains(&ch)
    }

    /// Returns `true` if the given character should auto-trigger based on a
    /// custom set of trigger characters (from a language server).
    #[must_use]
    pub fn is_custom_trigger(ch: char, triggers: &[char]) -> bool {
        triggers.contains(&ch)
    }

    /// Generates word-based suggestions from the buffer as a fallback
    /// when no LSP items are available.
    #[must_use]
    pub fn word_based_completions(
        buffer: &Buffer,
        pos: Position,
        max_items: usize,
    ) -> Vec<CompletionItem> {
        let line_count = buffer.len_lines();
        if pos.line as usize >= line_count {
            return Vec::new();
        }

        let current_line = buffer.line_content(pos.line as usize);
        let col = pos.column as usize;
        let chars: Vec<char> = current_line.chars().collect();

        let word_start = (0..col)
            .rev()
            .take_while(|&i| i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_'))
            .last()
            .unwrap_or(col);

        let prefix: String = if word_start < col && word_start < chars.len() {
            chars[word_start..col.min(chars.len())].iter().collect()
        } else {
            return Vec::new();
        };

        if prefix.is_empty() {
            return Vec::new();
        }

        let mut seen = std::collections::HashSet::new();
        let mut items = Vec::new();

        for line_idx in 0..line_count {
            let content = buffer.line_content(line_idx);
            for word in content.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if word.len() >= 2
                    && word != prefix
                    && word.to_lowercase().starts_with(&prefix.to_lowercase())
                    && seen.insert(word.to_string())
                {
                    items.push(CompletionItem {
                        label: word.to_string(),
                        kind: CompletionItemKind::Text,
                        detail: Some("word".to_string()),
                        documentation: None,
                        insert_text: None,
                        filter_text: None,
                        sort_text: None,
                        text_edit: None,
                        additional_edits: Vec::new(),
                        command: None,
                        preselect: false,
                    });
                    if items.len() >= max_items {
                        return items;
                    }
                }
            }
        }
        items
    }

    /// Updates the detail pane from the focused item.
    fn update_detail_pane(&mut self) {
        let info = self.focused_item().map(|item| {
            (
                item.detail.clone(),
                item.documentation.clone(),
                item.detail.is_some() || item.documentation.is_some(),
            )
        });
        if let Some((detail, documentation, visible)) = info {
            self.detail_pane.detail = detail;
            self.detail_pane.documentation = documentation;
            self.detail_pane.is_visible = visible;
        } else {
            self.detail_pane = SuggestDetailPane::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(label: &str) -> CompletionItem {
        CompletionItem {
            label: label.to_string(),
            kind: CompletionItemKind::Function,
            detail: None,
            documentation: None,
            insert_text: None,
            filter_text: None,
            sort_text: None,
            text_edit: None,
            additional_edits: Vec::new(),
            command: None,
            preselect: false,
        }
    }

    #[test]
    fn trigger_and_receive() {
        let mut state = SuggestState::default();
        state.trigger_suggest(Position::new(0, 5), CompletionTriggerKind::Invoked, None);
        assert!(state.is_active);
        assert!(state.is_loading);

        state.receive_items(vec![make_item("foo"), make_item("bar")]);
        assert!(!state.is_loading);
        assert_eq!(state.visible_items.len(), 2);
    }

    #[test]
    fn filter_narrows_items() {
        let mut state = SuggestState::default();
        state.trigger_suggest(Position::new(0, 0), CompletionTriggerKind::Invoked, None);
        state.receive_items(vec![
            make_item("forEach"),
            make_item("filter"),
            make_item("map"),
        ]);

        state.update_filter("f".into());
        assert_eq!(state.visible_items.len(), 2);

        state.update_filter("fil".into());
        assert_eq!(state.visible_items.len(), 1);
        assert_eq!(state.visible_items[0].item.label, "filter");
    }

    #[test]
    fn navigate_suggestions() {
        let mut state = SuggestState::default();
        state.trigger_suggest(Position::new(0, 0), CompletionTriggerKind::Invoked, None);
        state.receive_items(vec![make_item("a"), make_item("b"), make_item("c")]);

        assert_eq!(state.selected_index, 0);
        state.next_suggestion();
        assert_eq!(state.selected_index, 1);
        state.next_suggestion();
        assert_eq!(state.selected_index, 2);
        state.next_suggestion();
        assert_eq!(state.selected_index, 0); // wraps
    }

    #[test]
    fn accept_clears_session() {
        let mut state = SuggestState::default();
        state.trigger_suggest(Position::new(0, 0), CompletionTriggerKind::Invoked, None);
        state.receive_items(vec![make_item("hello")]);

        let accepted = state.accept_suggestion();
        assert!(accepted.is_some());
        assert!(!state.is_active);
        assert_eq!(
            state.last_dismiss_reason,
            Some(SuggestDismissReason::Accept)
        );
    }

    #[test]
    fn word_based_completions() {
        let buf = Buffer::from_str("function hello() {}\nfunction world() {}\nhel");
        let items = SuggestState::word_based_completions(&buf, Position::new(2, 3), 10);
        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label == "hello"));
    }

    #[test]
    fn accept_on_key_respects_mode() {
        let mut state = SuggestState::default();
        state.trigger_suggest(Position::new(0, 0), CompletionTriggerKind::Invoked, None);
        state.receive_items(vec![make_item("x")]);

        state.accept_on = AcceptSuggestionOn::Tab;
        assert!(state.try_accept_on_key(true).is_none()); // enter rejected

        state.trigger_suggest(Position::new(0, 0), CompletionTriggerKind::Invoked, None);
        state.receive_items(vec![make_item("x")]);
        assert!(state.try_accept_on_key(false).is_some()); // tab accepted
    }

    #[test]
    fn icon_names() {
        assert_eq!(
            CompletionKindIcon(CompletionItemKind::Function).icon_name(),
            "symbol-function"
        );
        assert_eq!(
            CompletionKindIcon(CompletionItemKind::Snippet).icon_name(),
            "symbol-snippet"
        );
    }
}
