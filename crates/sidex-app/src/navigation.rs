//! Navigation history — tracks editor positions for Back/Forward navigation.
//!
//! Mirrors VS Code's `NavigationHistory` with Alt+Left (back) and
//! Alt+Right (forward) support. Debounces entries on the same line and
//! caps history at 50 entries.

use std::time::Instant;

use sidex_text::Position;

/// Optional selection stored alongside a history entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistorySelection {
    pub anchor: Position,
    pub active: Position,
}

/// A single entry in the navigation history.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    /// File URI or path.
    pub uri: String,
    /// Cursor position at the time of the snapshot.
    pub position: Position,
    /// Optional selection range.
    pub selection: Option<HistorySelection>,
    /// When this entry was recorded.
    pub timestamp: Instant,
}

const MAX_HISTORY: usize = 50;

/// Navigation history stack supporting back/forward traversal.
///
/// When pushing a new entry after going back, the forward stack is
/// cleared (matching browser-style history semantics).
#[derive(Debug)]
pub struct NavigationHistory {
    back_stack: Vec<HistoryEntry>,
    forward_stack: Vec<HistoryEntry>,
}

impl NavigationHistory {
    /// Creates an empty navigation history.
    pub fn new() -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
        }
    }

    /// Push a new location into the history.
    ///
    /// Debounce: if the most recent back-stack entry has the same URI and
    /// line number, the entry is updated in place rather than duplicated.
    /// Pushing clears the forward stack.
    pub fn push(&mut self, entry: HistoryEntry) {
        if let Some(top) = self.back_stack.last() {
            if top.uri == entry.uri && top.position.line == entry.position.line {
                let last = self.back_stack.last_mut().unwrap();
                last.position = entry.position;
                last.selection = entry.selection;
                last.timestamp = entry.timestamp;
                return;
            }
        }

        self.forward_stack.clear();
        self.back_stack.push(entry);

        if self.back_stack.len() > MAX_HISTORY {
            self.back_stack.remove(0);
        }
    }

    /// Navigate backward (Alt+Left). Returns the entry to restore, if any.
    ///
    /// The caller should push the *current* location onto the forward
    /// stack before applying the returned entry. This method handles
    /// moving the popped entry to the forward stack internally.
    pub fn back(&mut self) -> Option<HistoryEntry> {
        let entry = self.back_stack.pop()?;
        Some(entry)
    }

    /// Navigate forward (Alt+Right). Returns the entry to restore, if any.
    pub fn forward(&mut self) -> Option<HistoryEntry> {
        let entry = self.forward_stack.pop()?;
        Some(entry)
    }

    /// Push an entry onto the forward stack (used when going back).
    pub fn push_forward(&mut self, entry: HistoryEntry) {
        self.forward_stack.push(entry);
        if self.forward_stack.len() > MAX_HISTORY {
            self.forward_stack.remove(0);
        }
    }

    /// Push an entry onto the back stack (used when going forward).
    pub fn push_back(&mut self, entry: HistoryEntry) {
        self.back_stack.push(entry);
        if self.back_stack.len() > MAX_HISTORY {
            self.back_stack.remove(0);
        }
    }

    /// Whether there are entries to go back to.
    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    /// Whether there are entries to go forward to.
    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    /// Number of entries in the back stack.
    pub fn back_len(&self) -> usize {
        self.back_stack.len()
    }

    /// Number of entries in the forward stack.
    pub fn forward_len(&self) -> usize {
        self.forward_stack.len()
    }

    /// Clears all history.
    pub fn clear(&mut self) {
        self.back_stack.clear();
        self.forward_stack.clear();
    }
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidex_text::Position;

    fn entry(uri: &str, line: u32, col: u32) -> HistoryEntry {
        HistoryEntry {
            uri: uri.to_string(),
            position: Position::new(line, col),
            selection: None,
            timestamp: Instant::now(),
        }
    }

    #[test]
    fn push_and_back() {
        let mut history = NavigationHistory::new();
        history.push(entry("a.rs", 0, 0));
        history.push(entry("a.rs", 10, 0));
        history.push(entry("b.rs", 5, 0));

        assert!(history.can_go_back());
        let e = history.back().unwrap();
        assert_eq!(e.uri, "b.rs");
        assert_eq!(e.position.line, 5);
    }

    #[test]
    fn back_and_forward() {
        let mut history = NavigationHistory::new();
        history.push(entry("a.rs", 0, 0));
        history.push(entry("b.rs", 10, 0));

        let went_back = history.back().unwrap();
        history.push_forward(went_back);

        assert!(history.can_go_forward());
        let went_fwd = history.forward().unwrap();
        assert_eq!(went_fwd.uri, "b.rs");
    }

    #[test]
    fn push_clears_forward() {
        let mut history = NavigationHistory::new();
        history.push(entry("a.rs", 0, 0));
        history.push(entry("b.rs", 10, 0));

        let went_back = history.back().unwrap();
        history.push_forward(went_back);
        assert!(history.can_go_forward());

        history.push(entry("c.rs", 20, 0));
        assert!(!history.can_go_forward());
    }

    #[test]
    fn debounce_same_line() {
        let mut history = NavigationHistory::new();
        history.push(entry("a.rs", 5, 0));
        history.push(entry("a.rs", 5, 10));
        assert_eq!(history.back_len(), 1);

        let e = history.back().unwrap();
        assert_eq!(e.position.column, 10);
    }

    #[test]
    fn different_line_not_debounced() {
        let mut history = NavigationHistory::new();
        history.push(entry("a.rs", 5, 0));
        history.push(entry("a.rs", 6, 0));
        assert_eq!(history.back_len(), 2);
    }

    #[test]
    fn different_uri_not_debounced() {
        let mut history = NavigationHistory::new();
        history.push(entry("a.rs", 5, 0));
        history.push(entry("b.rs", 5, 0));
        assert_eq!(history.back_len(), 2);
    }

    #[test]
    fn max_history_enforced() {
        let mut history = NavigationHistory::new();
        for i in 0..60 {
            history.push(entry("a.rs", i * 10, 0));
        }
        assert_eq!(history.back_len(), MAX_HISTORY);
    }

    #[test]
    fn empty_history() {
        let history = NavigationHistory::new();
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn back_on_empty_returns_none() {
        let mut history = NavigationHistory::new();
        assert!(history.back().is_none());
    }

    #[test]
    fn forward_on_empty_returns_none() {
        let mut history = NavigationHistory::new();
        assert!(history.forward().is_none());
    }

    #[test]
    fn clear() {
        let mut history = NavigationHistory::new();
        history.push(entry("a.rs", 1, 0));
        history.push(entry("b.rs", 2, 0));
        let went_back = history.back().unwrap();
        history.push_forward(went_back);

        history.clear();
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn selection_preserved() {
        let mut history = NavigationHistory::new();
        let mut e = entry("a.rs", 5, 0);
        e.selection = Some(HistorySelection {
            anchor: Position::new(5, 0),
            active: Position::new(5, 10),
        });
        history.push(e);

        let restored = history.back().unwrap();
        assert!(restored.selection.is_some());
        let sel = restored.selection.unwrap();
        assert_eq!(sel.active, Position::new(5, 10));
    }
}
