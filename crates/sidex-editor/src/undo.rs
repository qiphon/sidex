use std::time::Instant;

use sidex_text::EditOperation;

use crate::selection::Selection;

/// How long between edits before we stop merging into the same undo group.
const AUTO_GROUP_TIMEOUT_MS: u128 = 500;

/// A group of edits that are undone/redone together as a single unit.
#[derive(Debug, Clone)]
pub struct EditGroup {
    /// Pairs of (forward edit, inverse edit) for each edit in this group.
    pub edits: Vec<(EditOperation, EditOperation)>,
    /// Cursor state before the edit was applied.
    pub cursor_before: Vec<Selection>,
    /// Cursor state after the edit was applied.
    pub cursor_after: Vec<Selection>,
    /// Timestamp of the last edit added to this group (for auto-grouping).
    pub timestamp: Instant,
}

impl EditGroup {
    /// Creates a new edit group with a single edit pair.
    #[must_use]
    pub fn new(
        forward: EditOperation,
        inverse: EditOperation,
        cursor_before: Vec<Selection>,
        cursor_after: Vec<Selection>,
    ) -> Self {
        Self {
            edits: vec![(forward, inverse)],
            cursor_before,
            cursor_after,
            timestamp: Instant::now(),
        }
    }

    /// Creates an empty edit group with the given cursor states.
    #[must_use]
    pub fn empty(cursor_before: Vec<Selection>, cursor_after: Vec<Selection>) -> Self {
        Self {
            edits: Vec::new(),
            cursor_before,
            cursor_after,
            timestamp: Instant::now(),
        }
    }

    /// Returns `true` if this group can be merged with a new edit based on
    /// timing (edits within 500ms are grouped).
    #[must_use]
    pub fn can_merge(&self, now: Instant) -> bool {
        now.duration_since(self.timestamp).as_millis() < AUTO_GROUP_TIMEOUT_MS
    }
}

/// Manages undo and redo stacks for the editor.
///
/// Edits are organized into [`EditGroup`]s. Consecutive character insertions
/// within 500ms are automatically merged into a single undo unit.
#[derive(Debug, Clone)]
pub struct UndoRedoStack {
    undo_stack: Vec<EditGroup>,
    redo_stack: Vec<EditGroup>,
}

impl UndoRedoStack {
    /// Creates a new, empty undo/redo stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Pushes an edit group onto the undo stack and clears the redo stack.
    ///
    /// If the last undo group can be merged with this edit (based on timing),
    /// the edits are combined into a single group.
    pub fn push(&mut self, group: EditGroup) {
        self.redo_stack.clear();

        if let Some(last) = self.undo_stack.last_mut() {
            if last.can_merge(group.timestamp) && group.edits.len() == 1 {
                last.edits.extend(group.edits);
                last.cursor_after = group.cursor_after;
                last.timestamp = group.timestamp;
                return;
            }
        }

        self.undo_stack.push(group);
    }

    /// Pushes an edit group that should NOT be merged with previous groups.
    pub fn push_barrier(&mut self, group: EditGroup) {
        self.redo_stack.clear();
        self.undo_stack.push(group);
    }

    /// Pops the top group from the undo stack and pushes it to redo.
    ///
    /// Returns `None` if the undo stack is empty.
    pub fn undo(&mut self) -> Option<EditGroup> {
        let group = self.undo_stack.pop()?;
        self.redo_stack.push(group.clone());
        Some(group)
    }

    /// Pops the top group from the redo stack and pushes it to undo.
    ///
    /// Returns `None` if the redo stack is empty.
    pub fn redo(&mut self) -> Option<EditGroup> {
        let group = self.redo_stack.pop()?;
        self.undo_stack.push(group.clone());
        Some(group)
    }

    /// Returns `true` if there are edits to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` if there are edits to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clears both undo and redo stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Returns the depth of the undo stack.
    #[must_use]
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Returns the depth of the redo stack.
    #[must_use]
    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }
}

impl Default for UndoRedoStack {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════
// Extended undo system — richer model with explicit open-group API,
// max-size enforcement, and per-edit inverse tracking.
// ═══════════════════════════════════════════════════════════════════

/// A single atomic text edit with its inverse, used inside [`UndoGroup`].
#[derive(Debug, Clone)]
pub struct UndoEdit {
    /// The range that was replaced (in the *before* state of the document).
    pub range: sidex_text::Range,
    /// The text that was inserted at `range.start`.
    pub text: String,
    /// The range that the inserted text occupies (in the *after* state).
    pub inverse_range: sidex_text::Range,
    /// The text that was removed (to restore the *before* state).
    pub inverse_text: String,
}

/// A group of edits that should be undone/redone as a single user action.
#[derive(Debug, Clone)]
pub struct UndoGroup {
    pub edits: Vec<UndoEdit>,
    pub cursor_state_before: Vec<crate::cursor::CursorState>,
    pub cursor_state_after: Vec<crate::cursor::CursorState>,
    pub timestamp: Instant,
}

impl UndoGroup {
    /// Creates a new empty group stamped at the current instant.
    #[must_use]
    pub fn new() -> Self {
        Self {
            edits: Vec::new(),
            cursor_state_before: Vec::new(),
            cursor_state_after: Vec::new(),
            timestamp: Instant::now(),
        }
    }
}

impl Default for UndoGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Duration of inactivity after which an open group is automatically closed.
const AUTO_CLOSE_GROUP_MS: u128 = 300;

/// Extended undo stack with explicit group open/close, auto-close after a
/// pause, maximum stack size, and cursor-state restoration.
#[derive(Debug, Clone)]
pub struct UndoStack {
    pub past: Vec<UndoGroup>,
    pub future: Vec<UndoGroup>,
    pub open_group: Option<UndoGroup>,
    pub max_size: usize,
}

impl UndoStack {
    /// Creates a new undo stack with the given maximum size.
    #[must_use]
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            open_group: None,
            max_size,
        }
    }

    /// Creates a new undo stack with a default maximum of 1024 groups.
    #[must_use]
    pub fn new() -> Self {
        Self::with_max_size(1024)
    }

    /// Begins a new undo group. All subsequent `push_edit` calls will be added
    /// to this group until `end_undo_group` is called (or the group is
    /// auto-closed on timeout).
    pub fn begin_undo_group(&mut self) {
        if let Some(open) = self.open_group.take() {
            self.commit_group(open);
        }
        self.open_group = Some(UndoGroup::new());
    }

    /// Ends the currently open undo group, committing it to the past stack.
    pub fn end_undo_group(&mut self) {
        if let Some(group) = self.open_group.take() {
            self.commit_group(group);
        }
    }

    /// Pushes a single edit into the currently open group. If no group is open,
    /// one is created implicitly (and auto-closed on the next pause).
    pub fn push_edit(&mut self, edit: UndoEdit) {
        self.future.clear();
        self.auto_close_if_stale();

        if let Some(group) = &mut self.open_group {
            group.edits.push(edit);
            group.timestamp = Instant::now();
        } else {
            let mut group = UndoGroup::new();
            group.edits.push(edit);
            self.open_group = Some(group);
        }
    }

    /// Undoes the last group. Returns `Some` with the group that was undone,
    /// or `None` if there is nothing to undo.
    pub fn undo(&mut self) -> Option<&UndoGroup> {
        self.flush_open_group();
        let group = self.past.pop()?;
        self.future.push(group);
        self.future.last()
    }

    /// Redoes the last undone group. Returns `Some` with the group that was
    /// redone, or `None` if there is nothing to redo.
    pub fn redo(&mut self) -> Option<&UndoGroup> {
        let group = self.future.pop()?;
        self.past.push(group);
        self.past.last()
    }

    /// Clears both stacks and any open group.
    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.open_group = None;
    }

    /// Returns `true` if there are groups to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty() || self.open_group.is_some()
    }

    /// Returns `true` if there are groups to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Returns the depth of the undo stack (committed groups).
    #[must_use]
    pub fn depth(&self) -> usize {
        self.past.len()
    }

    /// If an open group exists and has been idle for >= 300ms, close it.
    fn auto_close_if_stale(&mut self) {
        let should_close = self
            .open_group
            .as_ref()
            .is_some_and(|g| {
                Instant::now().duration_since(g.timestamp).as_millis() >= AUTO_CLOSE_GROUP_MS
            });
        if should_close {
            if let Some(group) = self.open_group.take() {
                self.commit_group(group);
            }
        }
    }

    /// Flushes the open group (if any) to the past stack.
    fn flush_open_group(&mut self) {
        if let Some(group) = self.open_group.take() {
            self.commit_group(group);
        }
    }

    fn commit_group(&mut self, group: UndoGroup) {
        if group.edits.is_empty() {
            return;
        }
        self.past.push(group);
        self.enforce_max_size();
    }

    fn enforce_max_size(&mut self) {
        while self.past.len() > self.max_size {
            self.past.remove(0);
        }
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use sidex_text::{Position, Range};

    use super::*;

    fn sel(line: u32, col: u32) -> Selection {
        Selection::caret(Position::new(line, col))
    }

    fn make_group() -> EditGroup {
        EditGroup::new(
            EditOperation::insert(Position::new(0, 0), "a".into()),
            EditOperation::delete(Range::new(Position::new(0, 0), Position::new(0, 1))),
            vec![sel(0, 0)],
            vec![sel(0, 1)],
        )
    }

    #[test]
    fn empty_stack() {
        let stack = UndoRedoStack::new();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn push_and_undo() {
        let mut stack = UndoRedoStack::new();
        stack.push_barrier(make_group());
        assert!(stack.can_undo());
        assert!(!stack.can_redo());

        let group = stack.undo().unwrap();
        assert_eq!(group.edits.len(), 1);
        assert!(!stack.can_undo());
        assert!(stack.can_redo());
    }

    #[test]
    fn undo_and_redo() {
        let mut stack = UndoRedoStack::new();
        stack.push_barrier(make_group());
        stack.undo();
        let group = stack.redo().unwrap();
        assert_eq!(group.edits.len(), 1);
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn push_clears_redo() {
        let mut stack = UndoRedoStack::new();
        stack.push_barrier(make_group());
        stack.undo();
        assert!(stack.can_redo());
        stack.push_barrier(make_group());
        assert!(!stack.can_redo());
    }

    #[test]
    fn auto_grouping() {
        let mut stack = UndoRedoStack::new();
        // Two pushes in rapid succession should merge
        stack.push(make_group());
        stack.push(make_group());
        assert_eq!(stack.undo_depth(), 1);

        let group = stack.undo().unwrap();
        assert_eq!(group.edits.len(), 2);
    }

    #[test]
    fn barrier_prevents_merge() {
        let mut stack = UndoRedoStack::new();
        stack.push_barrier(make_group());
        stack.push_barrier(make_group());
        assert_eq!(stack.undo_depth(), 2);
    }

    #[test]
    fn clear() {
        let mut stack = UndoRedoStack::new();
        stack.push_barrier(make_group());
        stack.push_barrier(make_group());
        stack.undo();
        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn cursor_state_preserved() {
        let mut stack = UndoRedoStack::new();
        let group = EditGroup::new(
            EditOperation::insert(Position::new(0, 0), "x".into()),
            EditOperation::delete(Range::new(Position::new(0, 0), Position::new(0, 1))),
            vec![sel(0, 0)],
            vec![sel(0, 1)],
        );
        stack.push_barrier(group);

        let undone = stack.undo().unwrap();
        assert_eq!(undone.cursor_before, vec![sel(0, 0)]);
        assert_eq!(undone.cursor_after, vec![sel(0, 1)]);
    }

    #[test]
    fn undo_empty_returns_none() {
        let mut stack = UndoRedoStack::new();
        assert!(stack.undo().is_none());
    }

    #[test]
    fn redo_empty_returns_none() {
        let mut stack = UndoRedoStack::new();
        assert!(stack.redo().is_none());
    }

    // ── UndoStack (extended) tests ────────────────────────────────

    fn make_undo_edit() -> UndoEdit {
        UndoEdit {
            range: Range::new(Position::new(0, 0), Position::new(0, 0)),
            text: "a".into(),
            inverse_range: Range::new(Position::new(0, 0), Position::new(0, 1)),
            inverse_text: String::new(),
        }
    }

    #[test]
    fn undo_stack_empty() {
        let stack = UndoStack::new();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.depth(), 0);
    }

    #[test]
    fn undo_stack_begin_end_group() {
        let mut stack = UndoStack::new();
        stack.begin_undo_group();
        stack.push_edit(make_undo_edit());
        stack.push_edit(make_undo_edit());
        stack.end_undo_group();
        assert_eq!(stack.depth(), 1);
        assert!(stack.can_undo());
    }

    #[test]
    fn undo_stack_push_edit_auto_group() {
        let mut stack = UndoStack::new();
        stack.push_edit(make_undo_edit());
        stack.push_edit(make_undo_edit());
        stack.end_undo_group();
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn undo_stack_undo_redo() {
        let mut stack = UndoStack::new();
        stack.begin_undo_group();
        stack.push_edit(make_undo_edit());
        stack.end_undo_group();

        let undone = stack.undo();
        assert!(undone.is_some());
        assert_eq!(undone.unwrap().edits.len(), 1);
        assert!(!stack.can_undo());
        assert!(stack.can_redo());

        let redone = stack.redo();
        assert!(redone.is_some());
        assert!(stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_clear() {
        let mut stack = UndoStack::new();
        stack.begin_undo_group();
        stack.push_edit(make_undo_edit());
        stack.end_undo_group();
        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_max_size() {
        let mut stack = UndoStack::with_max_size(3);
        for _ in 0..5 {
            stack.begin_undo_group();
            stack.push_edit(make_undo_edit());
            stack.end_undo_group();
        }
        assert!(stack.depth() <= 3);
    }

    #[test]
    fn undo_stack_push_edit_clears_redo() {
        let mut stack = UndoStack::new();
        stack.begin_undo_group();
        stack.push_edit(make_undo_edit());
        stack.end_undo_group();
        stack.undo();
        assert!(stack.can_redo());
        stack.push_edit(make_undo_edit());
        assert!(!stack.can_redo());
    }

    #[test]
    fn undo_stack_empty_group_not_committed() {
        let mut stack = UndoStack::new();
        stack.begin_undo_group();
        stack.end_undo_group();
        assert_eq!(stack.depth(), 0);
    }

    #[test]
    fn undo_stack_default() {
        let stack = UndoStack::default();
        assert_eq!(stack.max_size, 1024);
    }

    #[test]
    fn undo_group_default() {
        let group = UndoGroup::default();
        assert!(group.edits.is_empty());
    }
}
