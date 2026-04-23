//! Full completion handling engine wrapping LSP `textDocument/completion`.
//!
//! [`CompletionSession`] manages an active completion session — triggering,
//! filtering, sorting, resolving, and accepting completion items. Includes
//! fuzzy matching, type-based sorting, commit characters, auto-import via
//! additional edits, and snippet expansion support.

use std::cmp::Ordering;
use std::collections::HashMap;

use anyhow::{Context, Result};
use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionResponse, CompletionTextEdit,
    CompletionTriggerKind, InsertTextFormat,
};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::{lsp_to_range, position_to_lsp};

// ── Trigger ─────────────────────────────────────────────────────────────────

/// What triggered a completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionTrigger {
    /// Manually invoked (e.g. Ctrl+Space).
    Invoked,
    /// Triggered by a character (e.g. `.`, `:`).
    Character(char),
    /// Re-triggered while a completion session is already active.
    TriggerForIncomplete,
}

impl CompletionTrigger {
    fn to_lsp(&self) -> (CompletionTriggerKind, Option<String>) {
        match self {
            Self::Invoked => (CompletionTriggerKind::INVOKED, None),
            Self::Character(c) => (
                CompletionTriggerKind::TRIGGER_CHARACTER,
                Some(c.to_string()),
            ),
            Self::TriggerForIncomplete => (
                CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS,
                None,
            ),
        }
    }
}

// ── Completion list ─────────────────────────────────────────────────────────

/// A completion list with items and metadata.
#[derive(Debug, Clone)]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

/// An edit operation produced when a completion item is accepted.
#[derive(Debug, Clone)]
pub struct EditOperation {
    pub range: sidex_text::Range,
    pub new_text: String,
}

// ── CompletionEngine ────────────────────────────────────────────────────────

/// Top-level completion configuration.
pub struct CompletionEngine {
    pub active_session: Option<CompletionSession>,
    pub word_completions: bool,
    pub snippet_completions: bool,
    pub auto_trigger: bool,
    pub trigger_characters: HashMap<String, Vec<char>>,
}

impl Default for CompletionEngine {
    fn default() -> Self {
        Self {
            active_session: None,
            word_completions: true,
            snippet_completions: true,
            auto_trigger: true,
            trigger_characters: HashMap::new(),
        }
    }
}

impl CompletionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a session is active.
    pub fn is_active(&self) -> bool {
        self.active_session.is_some()
    }

    /// Dismiss the current completion session.
    pub fn dismiss(&mut self) {
        self.active_session = None;
    }

    /// Register trigger characters for a language.
    pub fn register_triggers(&mut self, language_id: &str, chars: Vec<char>) {
        self.trigger_characters
            .insert(language_id.to_owned(), chars);
    }

    /// Returns `true` if `ch` is a trigger character for `language_id`.
    pub fn is_trigger_char(&self, language_id: &str, ch: char) -> bool {
        self.trigger_characters
            .get(language_id)
            .is_some_and(|chars| chars.contains(&ch))
    }

    /// Returns `true` if `ch` is a commit character for the currently
    /// selected item.
    pub fn is_commit_char(&self, ch: char) -> bool {
        self.active_session
            .as_ref()
            .and_then(|s| s.selected_item())
            .and_then(|item| item.commit_characters.as_ref())
            .is_some_and(|chars| chars.iter().any(|c| c.as_str().starts_with(ch)))
    }
}

// ── CompletionSession ───────────────────────────────────────────────────────

/// Manages an active completion session.
pub struct CompletionSession {
    items: Vec<CompletionItem>,
    filtered_items: Vec<(usize, f64)>,
    selected: usize,
    filter_text: String,
    is_incomplete: bool,
    #[allow(dead_code)]
    resolve_support: bool,
}

impl CompletionSession {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered_items: Vec::new(),
            selected: 0,
            filter_text: String::new(),
            is_incomplete: false,
            resolve_support: true,
        }
    }

    /// Triggers a completion request against the language server.
    pub async fn trigger(
        &mut self,
        client: &LspClient,
        uri: &str,
        position: sidex_text::Position,
        trigger: CompletionTrigger,
    ) -> Result<CompletionList> {
        let lsp_pos = position_to_lsp(position);
        let (kind, character) = trigger.to_lsp();

        let params = lsp_types::CompletionParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier::new(
                    uri.parse().context("invalid URI")?,
                ),
                position: lsp_pos,
            },
            work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
            partial_result_params: lsp_types::PartialResultParams::default(),
            context: Some(lsp_types::CompletionContext {
                trigger_kind: kind,
                trigger_character: character,
            }),
        };

        let result = serde_json::to_value(params)?;
        let response_val = client
            .raw_request("textDocument/completion", Some(result))
            .await?;

        let response: CompletionResponse =
            serde_json::from_value(response_val).context("failed to parse CompletionResponse")?;

        match response {
            CompletionResponse::Array(items) => {
                self.items = items;
                self.is_incomplete = false;
            }
            CompletionResponse::List(list) => {
                self.items = list.items;
                self.is_incomplete = list.is_incomplete;
            }
        }

        self.filter_text.clear();
        self.refilter();

        Ok(CompletionList {
            is_incomplete: self.is_incomplete,
            items: self.items.clone(),
        })
    }

    /// Updates the filter text and recomputes the filtered/sorted list.
    pub fn set_filter(&mut self, text: &str) {
        text.clone_into(&mut self.filter_text);
        self.refilter();
    }

    /// Returns the current filter text.
    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    fn refilter(&mut self) {
        self.filtered_items = filter_and_sort(&self.items, &self.filter_text);
        if self.selected >= self.filtered_items.len() {
            self.selected = 0;
        }
    }

    /// Moves selection down.
    pub fn select_next(&mut self) {
        if !self.filtered_items.is_empty() {
            self.selected = (self.selected + 1) % self.filtered_items.len();
        }
    }

    /// Moves selection up.
    pub fn select_prev(&mut self) {
        if !self.filtered_items.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.filtered_items.len() - 1);
        }
    }

    /// Sets the selected index directly.
    pub fn set_selected(&mut self, idx: usize) {
        if idx < self.filtered_items.len() {
            self.selected = idx;
        }
    }

    /// Returns the currently selected index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Returns the currently selected `CompletionItem`.
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.filtered_items
            .get(self.selected)
            .and_then(|(idx, _)| self.items.get(*idx))
    }

    /// Returns the filtered items as (`original_index`, score).
    pub fn filtered(&self) -> &[(usize, f64)] {
        &self.filtered_items
    }

    /// Returns a filtered item by display index.
    pub fn item_at(&self, display_index: usize) -> Option<&CompletionItem> {
        self.filtered_items
            .get(display_index)
            .and_then(|(idx, _)| self.items.get(*idx))
    }

    /// Resolves a completion item to get full details (documentation, etc.).
    pub async fn resolve(client: &LspClient, item: &CompletionItem) -> Result<CompletionItem> {
        let val = serde_json::to_value(item)?;
        let result = client
            .raw_request("completionItem/resolve", Some(val))
            .await?;
        serde_json::from_value(result).context("failed to parse resolved CompletionItem")
    }

    /// Converts a completion item into edit operations for the editor.
    pub fn accept(item: &CompletionItem) -> Vec<EditOperation> {
        let mut ops = Vec::new();

        if let Some(ref text_edit) = item.text_edit {
            match text_edit {
                CompletionTextEdit::Edit(edit) => {
                    ops.push(EditOperation {
                        range: lsp_to_range(edit.range),
                        new_text: edit.new_text.clone(),
                    });
                }
                CompletionTextEdit::InsertAndReplace(edit) => {
                    ops.push(EditOperation {
                        range: lsp_to_range(edit.replace),
                        new_text: edit.new_text.clone(),
                    });
                }
            }
        } else if let Some(ref insert_text) = item.insert_text {
            ops.push(EditOperation {
                range: sidex_text::Range::new(
                    sidex_text::Position::ZERO,
                    sidex_text::Position::ZERO,
                ),
                new_text: insert_text.clone(),
            });
        } else {
            ops.push(EditOperation {
                range: sidex_text::Range::new(
                    sidex_text::Position::ZERO,
                    sidex_text::Position::ZERO,
                ),
                new_text: item.label.clone(),
            });
        }

        if let Some(ref additional) = item.additional_text_edits {
            for edit in additional {
                ops.push(EditOperation {
                    range: lsp_to_range(edit.range),
                    new_text: edit.new_text.clone(),
                });
            }
        }

        ops
    }

    /// Returns the current items.
    #[must_use]
    pub fn items(&self) -> &[CompletionItem] {
        &self.items
    }

    /// Whether the completion list is incomplete.
    #[must_use]
    pub fn is_incomplete(&self) -> bool {
        self.is_incomplete
    }

    /// Returns the number of visible (filtered) items.
    pub fn visible_count(&self) -> usize {
        self.filtered_items.len()
    }
}

impl Default for CompletionSession {
    fn default() -> Self {
        Self::new()
    }
}

// ── Fuzzy matching ──────────────────────────────────────────────────────────

/// Fuzzy match `pattern` against `text`, returning a score or `None`.
///
/// Scoring: consecutive matches get a large bonus, word-boundary matches
/// get a bonus, camelCase transitions get a bonus, exact-case matches
/// get a small bonus, and longer words are penalised slightly.
pub fn fuzzy_score(pattern: &str, text: &str) -> Option<f64> {
    let p_chars: Vec<char> = pattern.chars().collect();
    let t_chars: Vec<char> = text.chars().collect();
    let p_len = p_chars.len();
    let t_len = t_chars.len();

    if p_len == 0 {
        return Some(0.0);
    }
    if p_len > t_len {
        return None;
    }

    let mut pi = 0;
    let mut score: f64 = 0.0;
    let mut prev_match: Option<usize> = None;
    let mut gap_penalty: f64 = 0.0;

    for (ti, &tc) in t_chars.iter().enumerate() {
        if pi < p_len && tc.to_lowercase().eq(p_chars[pi].to_lowercase()) {
            score += 1.0;

            if prev_match == Some(ti.wrapping_sub(1)) {
                score += 5.0;
            } else if prev_match.is_some() {
                let gap = ti - prev_match.unwrap() - 1;
                #[allow(clippy::cast_precision_loss)]
                {
                    gap_penalty += gap as f64 * 0.5;
                }
            }

            if ti == 0 || !t_chars[ti - 1].is_alphanumeric() {
                score += 10.0;
            }

            if ti > 0 && tc.is_uppercase() && t_chars[ti - 1].is_lowercase() {
                score += 8.0;
            }

            if tc == p_chars[pi] {
                score += 1.0;
            }

            prev_match = Some(ti);
            pi += 1;
        }
    }

    if pi == p_len {
        score -= gap_penalty;
        #[allow(clippy::cast_precision_loss)]
        {
            score -= (t_len as f64 - p_len as f64) * 0.25;
        }
        Some(score)
    } else {
        None
    }
}

/// Filters and sorts completion items by fuzzy match quality.
///
/// Returns `(original_index, score)` sorted best-first.
pub fn filter_and_sort(items: &[CompletionItem], input: &str) -> Vec<(usize, f64)> {
    if input.is_empty() {
        let mut result: Vec<(usize, f64)> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let kind_bonus = kind_sort_priority(item.kind);
                let preselect_bonus = if item.preselect == Some(true) {
                    1000.0
                } else {
                    0.0
                };
                (i, preselect_bonus + kind_bonus)
            })
            .collect();
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        return result;
    }

    let mut scored: Vec<(usize, f64)> = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| {
            let filter_text = item.filter_text.as_deref().unwrap_or(&item.label);
            fuzzy_score(input, filter_text).map(|score| {
                let kind_bonus = kind_sort_priority(item.kind) * 0.01;
                let preselect_bonus = if item.preselect == Some(true) {
                    50.0
                } else {
                    0.0
                };
                (idx, score + kind_bonus + preselect_bonus)
            })
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    scored
}

/// Assigns a sort priority based on `CompletionItemKind`. Higher = better.
fn kind_sort_priority(kind: Option<CompletionItemKind>) -> f64 {
    match kind {
        Some(CompletionItemKind::METHOD) => 90.0,
        Some(CompletionItemKind::FUNCTION) => 88.0,
        Some(CompletionItemKind::CONSTRUCTOR) => 87.0,
        Some(CompletionItemKind::FIELD) => 85.0,
        Some(CompletionItemKind::PROPERTY) => 84.0,
        Some(CompletionItemKind::VARIABLE) => 83.0,
        Some(CompletionItemKind::CLASS) => 80.0,
        Some(CompletionItemKind::STRUCT) => 79.0,
        Some(CompletionItemKind::INTERFACE) => 78.0,
        Some(CompletionItemKind::ENUM) => 77.0,
        Some(CompletionItemKind::ENUM_MEMBER) => 76.0,
        Some(CompletionItemKind::MODULE) => 70.0,
        Some(CompletionItemKind::CONSTANT) => 68.0,
        Some(CompletionItemKind::KEYWORD) => 60.0,
        Some(CompletionItemKind::SNIPPET) => 55.0,
        Some(CompletionItemKind::TEXT) => 10.0,
        _ => 50.0,
    }
}

// ── Legacy API (kept for backward compat) ───────────────────────────────────

/// Sorts completion items by `sort_text` first, then by `label`.
pub fn sort_completion_items(items: &mut [CompletionItem]) {
    items.sort_by(|a, b| {
        let a_sort = a.sort_text.as_deref().unwrap_or(&a.label);
        let b_sort = b.sort_text.as_deref().unwrap_or(&b.label);
        a_sort.cmp(b_sort).then_with(|| a.label.cmp(&b.label))
    });
}

/// Filters completion items by matching the typed prefix against
/// `filter_text` (falling back to `label`).
pub fn filter_completion_items<'a>(
    items: &'a [CompletionItem],
    prefix: &str,
) -> Vec<&'a CompletionItem> {
    if prefix.is_empty() {
        return items.iter().collect();
    }
    let lower_prefix = prefix.to_lowercase();
    items
        .iter()
        .filter(|item| {
            let filter = item
                .filter_text
                .as_deref()
                .unwrap_or(&item.label)
                .to_lowercase();
            filter.starts_with(&lower_prefix)
        })
        .collect()
}

/// Returns items with `preselect` set to `true` first.
pub fn preselect_first(items: &mut [CompletionItem]) {
    items.sort_by(|a, b| {
        let a_pre = a.preselect.unwrap_or(false);
        let b_pre = b.preselect.unwrap_or(false);
        match (a_pre, b_pre) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });
}

/// Returns `true` if the item's `insert_text_format` indicates a snippet.
#[must_use]
pub fn is_snippet(item: &CompletionItem) -> bool {
    item.insert_text_format == Some(InsertTextFormat::SNIPPET)
}

#[cfg(test)]
mod tests {
    use lsp_types::TextEdit;

    use super::*;

    fn make_item(label: &str) -> CompletionItem {
        CompletionItem {
            label: label.to_owned(),
            ..CompletionItem::default()
        }
    }

    fn make_item_with_sort(label: &str, sort_text: &str) -> CompletionItem {
        CompletionItem {
            label: label.to_owned(),
            sort_text: Some(sort_text.to_owned()),
            ..CompletionItem::default()
        }
    }

    fn make_item_with_kind(label: &str, kind: CompletionItemKind) -> CompletionItem {
        CompletionItem {
            label: label.to_owned(),
            kind: Some(kind),
            ..CompletionItem::default()
        }
    }

    // ── fuzzy_score tests ─────────────────────────────────────────────

    #[test]
    fn fuzzy_exact_match() {
        let s = fuzzy_score("hello", "hello").unwrap();
        assert!(s > 0.0);
    }

    #[test]
    fn fuzzy_prefix_match() {
        assert!(fuzzy_score("hel", "hello").is_some());
    }

    #[test]
    fn fuzzy_no_match() {
        assert!(fuzzy_score("xyz", "hello").is_none());
    }

    #[test]
    fn fuzzy_empty_pattern() {
        let s = fuzzy_score("", "hello").unwrap();
        assert_eq!(s, 0.0);
    }

    #[test]
    fn fuzzy_case_insensitive() {
        assert!(fuzzy_score("HEL", "hello").is_some());
    }

    #[test]
    fn fuzzy_camel_case_bonus() {
        let camel = fuzzy_score("gN", "getName").unwrap();
        let flat = fuzzy_score("gn", "getnothing").unwrap_or(0.0);
        assert!(camel > flat);
    }

    #[test]
    fn fuzzy_pattern_longer_than_word() {
        assert!(fuzzy_score("longpattern", "short").is_none());
    }

    #[test]
    fn fuzzy_gap_penalty() {
        // "abc" in "abcdef" scores higher because all 3 chars are consecutive
        // vs "axbxcx" where there are gaps between matches (no word boundaries).
        let consecutive = fuzzy_score("abc", "abcdef").unwrap();
        let gapped = fuzzy_score("abc", "axbxcxdef").unwrap();
        assert!(
            consecutive > gapped,
            "consecutive {consecutive} should beat gapped {gapped}"
        );
    }

    // ── filter_and_sort tests ─────────────────────────────────────────

    #[test]
    fn filter_and_sort_empty_input_returns_all() {
        let items = vec![make_item("alpha"), make_item("beta")];
        let result = filter_and_sort(&items, "");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn filter_and_sort_removes_non_matching() {
        let items = vec![make_item("println"), make_item("format")];
        let result = filter_and_sort(&items, "pri");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, 0);
    }

    #[test]
    fn filter_and_sort_ranks_by_score() {
        let items = vec![
            make_item("toString"),
            make_item("toJSON"),
            make_item("total"),
        ];
        let result = filter_and_sort(&items, "to");
        assert!(!result.is_empty());
        assert!(result[0].1 >= result.last().unwrap().1);
    }

    #[test]
    fn filter_and_sort_kind_priority() {
        let items = vec![
            make_item_with_kind("getText", CompletionItemKind::TEXT),
            make_item_with_kind("getMethod", CompletionItemKind::METHOD),
        ];
        let result = filter_and_sort(&items, "get");
        assert_eq!(result.len(), 2);
        assert_eq!(items[result[0].0].label, "getMethod");
    }

    // ── Legacy API tests ──────────────────────────────────────────────

    #[test]
    fn sort_by_sort_text() {
        let mut items = vec![
            make_item_with_sort("beta", "2"),
            make_item_with_sort("alpha", "1"),
            make_item_with_sort("gamma", "3"),
        ];
        sort_completion_items(&mut items);
        assert_eq!(items[0].label, "alpha");
        assert_eq!(items[1].label, "beta");
        assert_eq!(items[2].label, "gamma");
    }

    #[test]
    fn sort_by_label_fallback() {
        let mut items = vec![make_item("zebra"), make_item("apple"), make_item("mango")];
        sort_completion_items(&mut items);
        assert_eq!(items[0].label, "apple");
        assert_eq!(items[1].label, "mango");
        assert_eq!(items[2].label, "zebra");
    }

    #[test]
    fn filter_by_prefix() {
        let items = vec![
            make_item("println"),
            make_item("print"),
            make_item("format"),
        ];
        let filtered = filter_completion_items(&items, "pri");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_case_insensitive() {
        let items = vec![make_item("HashMap"), make_item("hashCode")];
        let filtered = filter_completion_items(&items, "hash");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_empty_prefix_returns_all() {
        let items = vec![make_item("a"), make_item("b")];
        let filtered = filter_completion_items(&items, "");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn preselect_items_first() {
        let mut items = vec![
            make_item("normal"),
            CompletionItem {
                label: "preselected".to_owned(),
                preselect: Some(true),
                ..CompletionItem::default()
            },
        ];
        preselect_first(&mut items);
        assert_eq!(items[0].label, "preselected");
    }

    #[test]
    fn is_snippet_check() {
        let mut item = make_item("test");
        assert!(!is_snippet(&item));
        item.insert_text_format = Some(InsertTextFormat::SNIPPET);
        assert!(is_snippet(&item));
    }

    #[test]
    fn accept_with_text_edit() {
        let item = CompletionItem {
            label: "println!".to_owned(),
            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 5),
                ),
                new_text: "println!".to_owned(),
            })),
            ..CompletionItem::default()
        };
        let ops = CompletionSession::accept(&item);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].new_text, "println!");
    }

    #[test]
    fn accept_with_additional_edits() {
        let item = CompletionItem {
            label: "HashMap".to_owned(),
            additional_text_edits: Some(vec![TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 0),
                ),
                new_text: "use std::collections::HashMap;\n".to_owned(),
            }]),
            ..CompletionItem::default()
        };
        let ops = CompletionSession::accept(&item);
        assert_eq!(ops.len(), 2);
    }

    #[test]
    fn session_default() {
        let session = CompletionSession::default();
        assert!(session.items().is_empty());
        assert!(!session.is_incomplete());
    }

    #[test]
    fn completion_trigger_variants() {
        let (kind, ch) = CompletionTrigger::Invoked.to_lsp();
        assert_eq!(kind, CompletionTriggerKind::INVOKED);
        assert!(ch.is_none());

        let (kind, ch) = CompletionTrigger::Character('.').to_lsp();
        assert_eq!(kind, CompletionTriggerKind::TRIGGER_CHARACTER);
        assert_eq!(ch, Some(".".to_owned()));
    }

    #[test]
    fn engine_trigger_chars() {
        let mut engine = CompletionEngine::new();
        engine.register_triggers("rust", vec!['.', ':']);
        assert!(engine.is_trigger_char("rust", '.'));
        assert!(!engine.is_trigger_char("rust", ','));
        assert!(!engine.is_trigger_char("python", '.'));
    }

    #[test]
    fn session_select_next_prev() {
        let mut session = CompletionSession::new();
        session.items = vec![make_item("a"), make_item("b"), make_item("c")];
        session.refilter();
        assert_eq!(session.selected, 0);
        session.select_next();
        assert_eq!(session.selected, 1);
        session.select_next();
        assert_eq!(session.selected, 2);
        session.select_next();
        assert_eq!(session.selected, 0);
        session.select_prev();
        assert_eq!(session.selected, 2);
    }

    #[test]
    fn session_filter_narrows() {
        let mut session = CompletionSession::new();
        session.items = vec![
            make_item("println"),
            make_item("print"),
            make_item("format"),
        ];
        session.set_filter("pri");
        assert_eq!(session.visible_count(), 2);
        session.set_filter("printf");
        assert_eq!(session.visible_count(), 0);
    }
}
