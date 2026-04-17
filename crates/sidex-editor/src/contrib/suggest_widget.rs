//! Autocomplete popup widget — mirrors VS Code's `SuggestWidget` view model.
//!
//! Provides the visual state for the autocomplete popup that appears when
//! typing: item list with icons, fuzzy-filter highlighting, detail/docs
//! side panel, and positional logic for rendering above or below the cursor.

use crate::completion::{fuzzy_score, CompletionItem, CompletionItemKind};

/// Which side/direction the documentation panel is rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocsSide {
    /// Docs pane appears to the right of the item list.
    Right,
    /// Docs pane appears below the item list.
    Below,
}

impl Default for DocsSide {
    fn default() -> Self {
        Self::Right
    }
}

/// Whether the popup opens above or below the cursor line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupDirection {
    Below,
    Above,
}

impl Default for PopupDirection {
    fn default() -> Self {
        Self::Below
    }
}

/// A single item in the autocomplete popup with scoring metadata.
#[derive(Debug, Clone)]
pub struct SuggestItem {
    /// The label shown in the list.
    pub label: String,
    /// Optional label detail (grayed, appended after label).
    pub label_detail: Option<String>,
    /// Completion kind, drives the icon.
    pub kind: CompletionItemKind,
    /// Short detail string (type signature).
    pub detail: Option<String>,
    /// Longer documentation (markdown / plain text).
    pub documentation: Option<String>,
    /// String used for sorting.
    pub sort_text: Option<String>,
    /// String used for filtering.
    pub filter_text: Option<String>,
    /// The text that gets inserted on accept.
    pub insert_text: String,
    /// Whether this item is deprecated (rendered with strikethrough).
    pub is_deprecated: bool,
    /// Fuzzy match score (higher is better).
    pub score: f64,
    /// Character positions in the label that matched the filter (for highlighting).
    pub match_positions: Vec<usize>,
}

impl SuggestItem {
    /// Creates a `SuggestItem` from a [`CompletionItem`].
    #[must_use]
    pub fn from_completion(item: &CompletionItem) -> Self {
        Self {
            label: item.label.clone(),
            label_detail: None,
            kind: item.kind,
            detail: item.detail.clone(),
            documentation: item.documentation.clone(),
            sort_text: item.sort_text.clone(),
            filter_text: item.filter_text.clone(),
            insert_text: item.effective_insert_text().to_string(),
            is_deprecated: false,
            score: 0.0,
            match_positions: Vec::new(),
        }
    }

    /// The effective filter text (falls back to label).
    #[must_use]
    pub fn effective_filter_text(&self) -> &str {
        self.filter_text.as_deref().unwrap_or(&self.label)
    }

    /// The icon name string for this item's completion kind.
    #[must_use]
    pub fn icon_name(&self) -> &'static str {
        match self.kind {
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

/// The complete autocomplete popup widget state.
#[derive(Debug, Clone)]
pub struct SuggestWidget {
    /// Whether the popup is visible.
    pub visible: bool,
    /// All items after filtering and scoring.
    pub items: Vec<SuggestItem>,
    /// Index of the currently selected (focused) item.
    pub selected: usize,
    /// Current filter/prefix text.
    pub filter_text: String,
    /// Pixel position for the popup `(x, y)`.
    pub position: (f32, f32),
    /// Maximum height of the popup in pixels.
    pub max_height: f32,
    /// Width of the popup in pixels.
    pub width: f32,
    /// Whether the documentation side panel is visible.
    pub detail_visible: bool,
    /// Where to render the docs panel.
    pub docs_side: DocsSide,
    /// Maximum number of visible items before scrolling.
    pub max_visible_items: usize,
    /// First visible item index (for scroll offset).
    pub scroll_offset: usize,
    /// Whether the popup renders above the cursor.
    pub direction: PopupDirection,
    /// Documentation for the currently selected item (resolved).
    pub resolved_docs: Option<String>,
}

impl Default for SuggestWidget {
    fn default() -> Self {
        Self {
            visible: false,
            items: Vec::new(),
            selected: 0,
            filter_text: String::new(),
            position: (0.0, 0.0),
            max_height: 300.0,
            width: 400.0,
            detail_visible: true,
            docs_side: DocsSide::default(),
            max_visible_items: 12,
            scroll_offset: 0,
            direction: PopupDirection::default(),
            resolved_docs: None,
        }
    }
}

impl SuggestWidget {
    /// Shows the popup with the given items at the given pixel position.
    pub fn show(&mut self, items: Vec<SuggestItem>, position: (f32, f32)) {
        if items.is_empty() {
            self.dismiss();
            return;
        }
        self.items = items;
        self.position = position;
        self.selected = 0;
        self.scroll_offset = 0;
        self.filter_text.clear();
        self.visible = true;
        self.update_resolved_docs();
    }

    /// Filters the current item list using fuzzy matching and re-sorts.
    pub fn filter(&mut self, text: &str) {
        self.filter_text = text.to_string();

        if text.is_empty() {
            for item in &mut self.items {
                item.score = 0.0;
                item.match_positions.clear();
            }
            self.selected = 0;
            self.scroll_offset = 0;
            self.update_resolved_docs();
            return;
        }

        for item in &mut self.items {
            if let Some((score, positions)) = fuzzy_score(text, item.effective_filter_text()) {
                item.score = f64::from(score);
                item.match_positions = positions;
            } else {
                item.score = f64::NEG_INFINITY;
                item.match_positions.clear();
            }
        }

        self.items
            .sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        self.items.retain(|item| item.score > f64::NEG_INFINITY);

        if self.items.is_empty() {
            self.dismiss();
            return;
        }

        self.selected = 0;
        self.scroll_offset = 0;
        self.update_resolved_docs();
    }

    /// Selects the next item in the list.
    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
        self.ensure_visible();
        self.update_resolved_docs();
    }

    /// Selects the previous item in the list.
    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.items.len() - 1
        } else {
            self.selected - 1
        };
        self.ensure_visible();
        self.update_resolved_docs();
    }

    /// Moves the selection down by a page.
    pub fn select_page_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + self.max_visible_items).min(self.items.len() - 1);
        self.ensure_visible();
        self.update_resolved_docs();
    }

    /// Moves the selection up by a page.
    pub fn select_page_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = self.selected.saturating_sub(self.max_visible_items);
        self.ensure_visible();
        self.update_resolved_docs();
    }

    /// Accepts the currently selected item. Returns it if available.
    pub fn accept(&mut self) -> Option<SuggestItem> {
        let item = self.focused_item().cloned();
        self.dismiss();
        item
    }

    /// Dismisses the popup.
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.items.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.filter_text.clear();
        self.resolved_docs = None;
    }

    /// Returns the currently focused item, if any.
    #[must_use]
    pub fn focused_item(&self) -> Option<&SuggestItem> {
        self.items.get(self.selected)
    }

    /// Returns the status bar label like `"3/42 items"`.
    #[must_use]
    pub fn status_label(&self) -> String {
        let total = self.items.len();
        if total == 0 {
            return String::new();
        }
        let current = self.selected + 1;
        format!("{current}/{total} items")
    }

    /// Returns the visible slice of items based on scroll offset.
    #[must_use]
    pub fn visible_items(&self) -> &[SuggestItem] {
        let end = (self.scroll_offset + self.max_visible_items).min(self.items.len());
        &self.items[self.scroll_offset..end]
    }

    /// Returns the number of visible items (capped by `max_visible_items`).
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.items.len().min(self.max_visible_items)
    }

    /// Toggles the detail/docs side panel.
    pub fn toggle_detail(&mut self) {
        self.detail_visible = !self.detail_visible;
    }

    /// Sets the direction the popup opens.
    pub fn set_direction(&mut self, direction: PopupDirection) {
        self.direction = direction;
    }

    /// Scrolls so the selected item is in the visible window.
    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + self.max_visible_items {
            self.scroll_offset = self.selected + 1 - self.max_visible_items;
        }
    }

    fn update_resolved_docs(&mut self) {
        self.resolved_docs = self
            .focused_item()
            .and_then(|item| item.documentation.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item(label: &str, kind: CompletionItemKind) -> SuggestItem {
        SuggestItem {
            label: label.to_string(),
            label_detail: None,
            kind,
            detail: None,
            documentation: None,
            sort_text: None,
            filter_text: None,
            insert_text: label.to_string(),
            is_deprecated: false,
            score: 0.0,
            match_positions: Vec::new(),
        }
    }

    fn sample_items() -> Vec<SuggestItem> {
        vec![
            make_item("forEach", CompletionItemKind::Method),
            make_item("filter", CompletionItemKind::Method),
            make_item("find", CompletionItemKind::Method),
            make_item("map", CompletionItemKind::Method),
            make_item("reduce", CompletionItemKind::Method),
        ]
    }

    #[test]
    fn show_and_dismiss() {
        let mut w = SuggestWidget::default();
        assert!(!w.visible);

        w.show(sample_items(), (100.0, 200.0));
        assert!(w.visible);
        assert_eq!(w.items.len(), 5);
        assert_eq!(w.position, (100.0, 200.0));

        w.dismiss();
        assert!(!w.visible);
        assert!(w.items.is_empty());
    }

    #[test]
    fn show_empty_dismisses() {
        let mut w = SuggestWidget::default();
        w.show(vec![], (0.0, 0.0));
        assert!(!w.visible);
    }

    #[test]
    fn navigate_next_prev() {
        let mut w = SuggestWidget::default();
        w.show(sample_items(), (0.0, 0.0));

        assert_eq!(w.selected, 0);
        w.select_next();
        assert_eq!(w.selected, 1);
        w.select_next();
        assert_eq!(w.selected, 2);

        w.select_prev();
        assert_eq!(w.selected, 1);
        w.select_prev();
        assert_eq!(w.selected, 0);
        w.select_prev();
        assert_eq!(w.selected, 4); // wraps
    }

    #[test]
    fn accept_returns_item() {
        let mut w = SuggestWidget::default();
        w.show(sample_items(), (0.0, 0.0));
        w.select_next();

        let accepted = w.accept();
        assert!(accepted.is_some());
        assert_eq!(accepted.unwrap().label, "filter");
        assert!(!w.visible);
    }

    #[test]
    fn filter_narrows_items() {
        let mut w = SuggestWidget::default();
        w.show(sample_items(), (0.0, 0.0));

        w.filter("fi");
        assert!(w.visible);
        assert!(w.items.len() >= 2);
        assert!(w.items.iter().all(|i| i.label.contains('f') || i.label.contains('i')));
    }

    #[test]
    fn filter_to_zero_dismisses() {
        let mut w = SuggestWidget::default();
        w.show(sample_items(), (0.0, 0.0));

        w.filter("zzzzz");
        assert!(!w.visible);
    }

    #[test]
    fn page_navigation() {
        let mut w = SuggestWidget::default();
        let items: Vec<SuggestItem> = (0..20)
            .map(|i| make_item(&format!("item_{i}"), CompletionItemKind::Variable))
            .collect();
        w.show(items, (0.0, 0.0));

        w.select_page_down();
        assert_eq!(w.selected, 12);

        w.select_page_up();
        assert_eq!(w.selected, 0);
    }

    #[test]
    fn status_label() {
        let mut w = SuggestWidget::default();
        assert!(w.status_label().is_empty());

        w.show(sample_items(), (0.0, 0.0));
        assert_eq!(w.status_label(), "1/5 items");
        w.select_next();
        assert_eq!(w.status_label(), "2/5 items");
    }

    #[test]
    fn visible_items_slice() {
        let mut w = SuggestWidget::default();
        w.max_visible_items = 3;
        w.show(sample_items(), (0.0, 0.0));

        assert_eq!(w.visible_items().len(), 3);
        assert_eq!(w.visible_count(), 3);
    }

    #[test]
    fn scroll_follows_selection() {
        let mut w = SuggestWidget::default();
        w.max_visible_items = 3;
        let items: Vec<SuggestItem> = (0..10)
            .map(|i| make_item(&format!("item_{i}"), CompletionItemKind::Variable))
            .collect();
        w.show(items, (0.0, 0.0));

        assert_eq!(w.scroll_offset, 0);
        for _ in 0..4 {
            w.select_next();
        }
        assert!(w.scroll_offset > 0);
    }

    #[test]
    fn icon_names() {
        let item = make_item("test", CompletionItemKind::Function);
        assert_eq!(item.icon_name(), "symbol-function");
    }

    #[test]
    fn deprecated_items() {
        let mut item = make_item("old_fn", CompletionItemKind::Function);
        item.is_deprecated = true;
        assert!(item.is_deprecated);
    }

    #[test]
    fn from_completion_item() {
        let ci = CompletionItem {
            label: "hello".to_string(),
            kind: CompletionItemKind::Function,
            detail: Some("fn hello()".to_string()),
            documentation: Some("Says hello.".to_string()),
            insert_text: Some("hello()".to_string()),
            filter_text: None,
            sort_text: None,
            text_edit: None,
            additional_edits: Vec::new(),
            command: None,
            preselect: false,
        };
        let si = SuggestItem::from_completion(&ci);
        assert_eq!(si.label, "hello");
        assert_eq!(si.insert_text, "hello()");
        assert_eq!(si.detail, Some("fn hello()".to_string()));
    }

    #[test]
    fn toggle_detail() {
        let mut w = SuggestWidget::default();
        assert!(w.detail_visible);
        w.toggle_detail();
        assert!(!w.detail_visible);
    }
}
