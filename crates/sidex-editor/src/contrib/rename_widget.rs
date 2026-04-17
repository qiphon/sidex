//! Inline rename input widget (F2) — renders a text input at the symbol
//! location, validates the new name, and shows a preview of all affected
//! locations before committing the rename.

use std::fmt;
use std::ops::Range as StdRange;
use std::path::PathBuf;

use sidex_text::Position;

use super::rename::RenameState;

// ── Preview ─────────────────────────────────────────────────────────────────

/// Summary of all workspace edits a rename would produce.
#[derive(Debug, Clone, Default)]
pub struct RenamePreview {
    pub file_count: u32,
    pub edit_count: u32,
    /// `(file_path, occurrences_in_file)`.
    pub files: Vec<(PathBuf, u32)>,
}

impl RenamePreview {
    #[must_use]
    pub fn new(files: Vec<(PathBuf, u32)>) -> Self {
        let file_count = files.len() as u32;
        let edit_count = files.iter().map(|(_, n)| n).sum();
        Self {
            file_count,
            edit_count,
            files,
        }
    }

    #[must_use]
    pub fn status_bar_text(&self) -> String {
        if self.file_count == 0 {
            return String::new();
        }
        format!(
            "{} edit{} in {} file{}",
            self.edit_count,
            if self.edit_count == 1 { "" } else { "s" },
            self.file_count,
            if self.file_count == 1 { "" } else { "s" },
        )
    }
}

// ── Validation ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NameValidation {
    Valid,
    Empty,
    Unchanged,
    InvalidChars,
}

impl NameValidation {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    #[must_use]
    pub fn message(&self) -> Option<&'static str> {
        match self {
            Self::Valid => None,
            Self::Empty => Some("Name cannot be empty"),
            Self::Unchanged => Some("Name is unchanged"),
            Self::InvalidChars => Some("Name contains invalid characters"),
        }
    }
}

fn validate_name(new_name: &str, original: &str) -> NameValidation {
    if new_name.is_empty() {
        return NameValidation::Empty;
    }
    if new_name == original {
        return NameValidation::Unchanged;
    }
    if new_name.contains('\n') || new_name.contains('\r') || new_name.contains('\0') {
        return NameValidation::InvalidChars;
    }
    NameValidation::Valid
}

// ── Widget ──────────────────────────────────────────────────────────────────

/// The inline rename input widget shown at a symbol location.
#[derive(Debug, Clone)]
pub struct RenameWidget {
    pub visible: bool,
    pub position: Position,
    pub text: String,
    pub selection: StdRange<usize>,
    pub original_name: String,
    pub preview_edits: Option<RenamePreview>,
    pub validation: NameValidation,
}

impl Default for RenameWidget {
    fn default() -> Self {
        Self {
            visible: false,
            position: Position::new(0, 0),
            text: String::new(),
            selection: 0..0,
            original_name: String::new(),
            preview_edits: None,
            validation: NameValidation::Valid,
        }
    }
}

impl RenameWidget {
    /// Shows the rename widget at `position`, pre-filled with `current_name`.
    pub fn show(&mut self, position: Position, current_name: &str) {
        self.visible = true;
        self.position = position;
        self.text = current_name.to_string();
        self.original_name = current_name.to_string();
        self.selection = 0..current_name.len();
        self.preview_edits = None;
        self.validation = NameValidation::Valid;
    }

    /// Accepts the rename and returns the new name if valid.
    #[must_use]
    pub fn accept(&mut self) -> Option<String> {
        let v = validate_name(&self.text, &self.original_name);
        self.validation = v.clone();
        if !v.is_valid() {
            return None;
        }
        let name = self.text.clone();
        self.dismiss();
        Some(name)
    }

    /// Dismisses the widget without applying.
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.text.clear();
        self.original_name.clear();
        self.selection = 0..0;
        self.preview_edits = None;
        self.validation = NameValidation::Valid;
    }

    /// Sets the input text and revalidates.
    pub fn set_text(&mut self, text: String) {
        self.text = text;
        self.validation = validate_name(&self.text, &self.original_name);
    }

    /// Attaches a rename preview (all locations that will change).
    pub fn set_preview(&mut self, preview: RenamePreview) {
        self.preview_edits = Some(preview);
    }

    /// Synchronises the widget state from a `RenameState` when the
    /// controller resolves a rename range.
    pub fn sync_from_state(&mut self, state: &RenameState) {
        if let Some(pos) = state.trigger_position {
            self.position = pos;
        }
        self.text.clone_from(&state.new_name);
        self.original_name.clone_from(&state.original_text);
        self.selection = 0..self.text.len();
    }

    /// Returns the validation error message, if any.
    #[must_use]
    pub fn error_message(&self) -> Option<&'static str> {
        self.validation.message()
    }

    /// Width hint (in characters) for laying out the input box.
    #[must_use]
    pub fn input_width_chars(&self) -> usize {
        self.text.len().max(self.original_name.len()).max(12) + 4
    }
}

impl fmt::Display for RenameWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RenameWidget({:?} → {:?}, visible={})",
            self.original_name, self.text, self.visible,
        )
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_accept() {
        let mut w = RenameWidget::default();
        w.show(Position::new(5, 10), "foo");
        assert!(w.visible);
        assert_eq!(w.selection, 0..3);

        w.set_text("bar".into());
        let result = w.accept();
        assert_eq!(result, Some("bar".into()));
        assert!(!w.visible);
    }

    #[test]
    fn reject_empty_name() {
        let mut w = RenameWidget::default();
        w.show(Position::new(0, 0), "hello");
        w.set_text(String::new());
        assert_eq!(w.accept(), None);
        assert_eq!(w.validation, NameValidation::Empty);
    }

    #[test]
    fn reject_unchanged_name() {
        let mut w = RenameWidget::default();
        w.show(Position::new(0, 0), "hello");
        assert_eq!(w.accept(), None);
        assert_eq!(w.validation, NameValidation::Unchanged);
    }

    #[test]
    fn reject_newline_in_name() {
        let mut w = RenameWidget::default();
        w.show(Position::new(0, 0), "hello");
        w.set_text("he\nllo".into());
        assert_eq!(w.accept(), None);
        assert_eq!(w.validation, NameValidation::InvalidChars);
    }

    #[test]
    fn dismiss_clears_state() {
        let mut w = RenameWidget::default();
        w.show(Position::new(1, 2), "abc");
        w.dismiss();
        assert!(!w.visible);
        assert!(w.text.is_empty());
        assert!(w.preview_edits.is_none());
    }

    #[test]
    fn preview_status_bar_text() {
        let preview = RenamePreview::new(vec![
            (PathBuf::from("src/main.rs"), 3),
            (PathBuf::from("src/lib.rs"), 1),
        ]);
        assert_eq!(preview.file_count, 2);
        assert_eq!(preview.edit_count, 4);
        assert_eq!(preview.status_bar_text(), "4 edits in 2 files");
    }

    #[test]
    fn preview_single() {
        let preview = RenamePreview::new(vec![(PathBuf::from("a.rs"), 1)]);
        assert_eq!(preview.status_bar_text(), "1 edit in 1 file");
    }

    #[test]
    fn input_width_minimum() {
        let w = RenameWidget::default();
        assert!(w.input_width_chars() >= 16);
    }
}
