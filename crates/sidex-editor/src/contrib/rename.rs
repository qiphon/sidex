//! Inline rename — mirrors VS Code's `RenameController` + `RenameWidget`.

use std::collections::HashMap;

use sidex_text::{Position, Range};

/// The phase of a rename operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenamePhase {
    /// No rename in progress.
    Idle,
    /// Resolving the rename range from the language server.
    Resolving,
    /// The rename input box is visible and the user is typing.
    Editing,
    /// The rename preview is being shown.
    Previewing,
    /// The rename edits are being computed / applied.
    Applying,
}

/// A single file edit produced by a rename operation.
#[derive(Debug, Clone)]
pub struct RenameFileEdit {
    /// URI or path of the file to edit.
    pub file_path: String,
    /// The text edits within this file.
    pub edits: Vec<RenameTextEdit>,
}

/// A single text edit within a rename operation.
#[derive(Debug, Clone)]
pub struct RenameTextEdit {
    /// Range to replace.
    pub range: Range,
    /// New text.
    pub new_text: String,
}

/// Validation result for a proposed rename.
#[derive(Debug, Clone)]
pub enum RenameValidation {
    /// The new name is valid.
    Valid,
    /// The new name is invalid with a reason.
    Invalid(String),
    /// Validation is still pending.
    Pending,
}

/// Preview of all changes a rename would make.
#[derive(Debug, Clone, Default)]
pub struct RenamePreview {
    /// Per-file edits grouped by file path.
    pub file_edits: Vec<RenameFileEdit>,
    /// Total number of occurrences across all files.
    pub total_occurrences: usize,
    /// Number of files affected.
    pub file_count: usize,
    /// Whether the user has confirmed the preview.
    pub confirmed: bool,
    /// Per-file checkboxes (file path → included).
    pub file_included: HashMap<String, bool>,
}

impl RenamePreview {
    /// Creates a preview from file edits.
    pub fn from_edits(edits: Vec<RenameFileEdit>) -> Self {
        let total_occurrences: usize = edits.iter().map(|f| f.edits.len()).sum();
        let file_count = edits.len();
        let file_included: HashMap<String, bool> =
            edits.iter().map(|f| (f.file_path.clone(), true)).collect();
        Self {
            file_edits: edits,
            total_occurrences,
            file_count,
            confirmed: false,
            file_included,
        }
    }

    /// Toggles whether a file is included in the rename.
    pub fn toggle_file(&mut self, path: &str) {
        if let Some(included) = self.file_included.get_mut(path) {
            *included = !*included;
        }
    }

    /// Returns the file edits that are included.
    #[must_use]
    pub fn included_edits(&self) -> Vec<&RenameFileEdit> {
        self.file_edits
            .iter()
            .filter(|f| {
                self.file_included
                    .get(&f.file_path)
                    .copied()
                    .unwrap_or(true)
            })
            .collect()
    }

    /// Returns a summary string like "Renaming 12 occurrences across 4 files".
    #[must_use]
    pub fn summary(&self) -> String {
        let files = self.included_edits().len();
        let occurrences: usize = self.included_edits().iter().map(|f| f.edits.len()).sum();
        format!("Renaming {occurrences} occurrences across {files} files")
    }
}

/// Full state for the inline-rename feature.
#[derive(Debug, Clone)]
pub struct RenameState {
    /// Current phase.
    pub phase: RenamePhase,
    /// The position that triggered the rename.
    pub trigger_position: Option<Position>,
    /// The range of the symbol being renamed.
    pub rename_range: Option<Range>,
    /// The original symbol text before rename.
    pub original_text: String,
    /// The current text in the rename input box.
    pub new_name: String,
    /// Whether the provider supports rename at the trigger position.
    pub is_valid: bool,
    /// A placeholder hint from the provider (pre-fills the input).
    pub placeholder: Option<String>,
    /// Current validation state of the new name.
    pub validation: RenameValidation,
    /// Preview of all changes (populated when user requests preview).
    pub preview: Option<RenamePreview>,
}

impl Default for RenameState {
    fn default() -> Self {
        Self {
            phase: RenamePhase::Idle,
            trigger_position: None,
            rename_range: None,
            original_text: String::new(),
            new_name: String::new(),
            is_valid: false,
            placeholder: None,
            validation: RenameValidation::Valid,
            preview: None,
        }
    }
}

impl RenameState {
    /// Initiates a rename at the given position.
    pub fn start_rename(&mut self, pos: Position) {
        self.phase = RenamePhase::Resolving;
        self.trigger_position = Some(pos);
        self.original_text.clear();
        self.new_name.clear();
        self.rename_range = None;
        self.is_valid = false;
        self.preview = None;
        self.validation = RenameValidation::Pending;
    }

    /// Called when the provider resolves the rename range and placeholder.
    pub fn resolve(&mut self, range: Range, text: String, placeholder: Option<String>) {
        self.rename_range = Some(range);
        self.original_text.clone_from(&text);
        self.new_name = placeholder.clone().unwrap_or(text);
        self.placeholder = placeholder;
        self.is_valid = true;
        self.phase = RenamePhase::Editing;
        self.validation = RenameValidation::Valid;
    }

    /// Called when resolution fails — cancels the rename.
    pub fn resolve_failed(&mut self) {
        self.cancel_rename();
    }

    /// Updates the new name as the user types.
    pub fn set_new_name(&mut self, name: String) {
        self.new_name = name;
        self.validation = RenameValidation::Pending;
    }

    /// Receives a validation result from the language server.
    pub fn set_validation(&mut self, result: RenameValidation) {
        self.validation = result;
    }

    /// Validates the new name locally (basic checks).
    pub fn validate_local(&mut self) {
        if self.new_name.is_empty() {
            self.validation = RenameValidation::Invalid("Name cannot be empty".into());
        } else if self.new_name == self.original_text {
            self.validation = RenameValidation::Invalid("Name is unchanged".into());
        } else if self.new_name.contains(char::is_whitespace)
            && !self.original_text.contains(char::is_whitespace)
        {
            self.validation = RenameValidation::Invalid("Name contains whitespace".into());
        } else {
            self.validation = RenameValidation::Valid;
        }
    }

    /// Returns `true` if the rename can be applied.
    #[must_use]
    pub fn can_apply(&self) -> bool {
        self.phase == RenamePhase::Editing
            && matches!(self.validation, RenameValidation::Valid)
            && !self.new_name.is_empty()
            && self.new_name != self.original_text
    }

    /// Enters preview mode with the given edits.
    pub fn show_preview(&mut self, edits: Vec<RenameFileEdit>) {
        self.preview = Some(RenamePreview::from_edits(edits));
        self.phase = RenamePhase::Previewing;
    }

    /// Confirms the rename (transitions to Applying phase).
    /// Returns the new name if valid.
    pub fn apply_rename(&mut self) -> Option<String> {
        if !self.can_apply() && self.phase != RenamePhase::Previewing {
            return None;
        }
        self.phase = RenamePhase::Applying;
        Some(self.new_name.clone())
    }

    /// Cancels the rename and resets to idle.
    pub fn cancel_rename(&mut self) {
        self.phase = RenamePhase::Idle;
        self.trigger_position = None;
        self.rename_range = None;
        self.original_text.clear();
        self.new_name.clear();
        self.is_valid = false;
        self.placeholder = None;
        self.preview = None;
        self.validation = RenameValidation::Valid;
    }

    /// Finalises after the rename edits have been applied.
    pub fn finish(&mut self) {
        self.cancel_rename();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_lifecycle() {
        let mut state = RenameState::default();
        state.start_rename(Position::new(3, 10));
        assert_eq!(state.phase, RenamePhase::Resolving);

        let range = Range::new(Position::new(3, 8), Position::new(3, 13));
        state.resolve(range, "hello".into(), None);
        assert_eq!(state.phase, RenamePhase::Editing);
        assert_eq!(state.new_name, "hello");

        state.set_new_name("world".into());
        state.validate_local();
        assert!(state.can_apply());
        let result = state.apply_rename();
        assert_eq!(result, Some("world".into()));
        assert_eq!(state.phase, RenamePhase::Applying);
    }

    #[test]
    fn rename_same_name_cancels() {
        let mut state = RenameState::default();
        state.start_rename(Position::new(0, 0));
        let range = Range::new(Position::new(0, 0), Position::new(0, 3));
        state.resolve(range, "foo".into(), None);
        state.validate_local();
        assert!(!state.can_apply());
    }

    #[test]
    fn rename_preview() {
        let mut state = RenameState::default();
        state.start_rename(Position::new(0, 0));
        let range = Range::new(Position::new(0, 0), Position::new(0, 3));
        state.resolve(range, "foo".into(), None);
        state.set_new_name("bar".into());

        state.show_preview(vec![
            RenameFileEdit {
                file_path: "main.rs".into(),
                edits: vec![RenameTextEdit {
                    range,
                    new_text: "bar".into(),
                }],
            },
            RenameFileEdit {
                file_path: "lib.rs".into(),
                edits: vec![
                    RenameTextEdit {
                        range,
                        new_text: "bar".into(),
                    },
                    RenameTextEdit {
                        range,
                        new_text: "bar".into(),
                    },
                ],
            },
        ]);

        assert_eq!(state.phase, RenamePhase::Previewing);
        let preview = state.preview.as_ref().unwrap();
        assert_eq!(preview.file_count, 2);
        assert_eq!(preview.total_occurrences, 3);
    }

    #[test]
    fn empty_name_invalid() {
        let mut state = RenameState::default();
        state.start_rename(Position::new(0, 0));
        let range = Range::new(Position::new(0, 0), Position::new(0, 3));
        state.resolve(range, "foo".into(), None);
        state.set_new_name(String::new());
        state.validate_local();
        assert!(matches!(state.validation, RenameValidation::Invalid(_)));
    }
}
