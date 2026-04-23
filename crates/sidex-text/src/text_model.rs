//! High-level text model that wraps a [`Buffer`] with document metadata.
//!
//! `TextModel` is the primary entry point for editor features that need both
//! text content and its associated settings (encoding, line ending, language,
//! indentation preferences, etc.). It mirrors the role of VS Code's
//! `TextModel` / `ITextModel`.

use crate::buffer::{Buffer, EditResult};
use crate::edit::{ChangeEvent, EditOperation};
use crate::encoding::Encoding;
use crate::line_ending::LineEnding;

/// Configurable options that govern how the model behaves on save and edit.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct TextModelOptions {
    /// Number of spaces a tab represents.
    pub tab_size: u32,
    /// Use spaces instead of tabs for indentation.
    pub insert_spaces: bool,
    /// Strip trailing whitespace from every line on save.
    pub trim_trailing_whitespace: bool,
    /// Ensure the file ends with a newline on save.
    pub insert_final_newline: bool,
    /// Remove extra trailing newlines (keeping at most one) on save.
    pub trim_final_newlines: bool,
    /// Default line ending for new files or normalization.
    pub default_line_ending: LineEnding,
}

impl Default for TextModelOptions {
    fn default() -> Self {
        Self {
            tab_size: 4,
            insert_spaces: true,
            trim_trailing_whitespace: false,
            insert_final_newline: true,
            trim_final_newlines: false,
            default_line_ending: LineEnding::os_default(),
        }
    }
}

const LARGE_FILE_THRESHOLD: usize = 5_000_000; // 5 MB

/// A high-level text model combining a [`Buffer`] with document metadata.
#[derive(Debug, Clone)]
pub struct TextModel {
    /// The underlying text buffer.
    pub buffer: Buffer,
    /// The language identifier (e.g. `"rust"`, `"python"`).
    pub language_id: String,
    /// The document URI (e.g. `"file:///path/to/file.rs"`).
    pub uri: String,
    /// Monotonically increasing version counter for LSP sync.
    pub version: i32,
    /// The file's encoding.
    pub encoding: Encoding,
    /// The file's line ending style.
    pub line_ending: LineEnding,
    /// Whether the model has unsaved changes.
    pub is_dirty: bool,
    /// Whether the model is read-only.
    pub is_readonly: bool,
    /// Whether the file is considered "large" (above 5 MB).
    pub is_large_file: bool,
    /// Editor options for this model.
    pub options: TextModelOptions,
}

impl TextModel {
    /// Creates a new text model from raw content.
    ///
    /// Automatically detects line endings and encoding, and sets the
    /// `is_large_file` flag when content exceeds the threshold.
    pub fn new(content: &str, language_id: &str, uri: &str) -> Self {
        let buffer = Buffer::from_str(content);
        let line_ending = buffer.get_eol();
        Self {
            is_large_file: buffer.len_bytes() > LARGE_FILE_THRESHOLD,
            buffer,
            language_id: language_id.to_string(),
            uri: uri.to_string(),
            version: 1,
            encoding: Encoding::Utf8,
            line_ending,
            is_dirty: false,
            is_readonly: false,
            options: TextModelOptions::default(),
        }
    }

    /// Creates a text model from raw bytes with encoding detection.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes cannot be decoded.
    pub fn from_bytes(
        bytes: &[u8],
        language_id: &str,
        uri: &str,
    ) -> Result<Self, crate::encoding::EncodingError> {
        let encoding = crate::encoding::detect_encoding(bytes);
        let content = crate::encoding::decode(bytes, encoding)?;
        let mut model = Self::new(&content, language_id, uri);
        model.encoding = encoding;
        Ok(model)
    }

    /// Applies a single edit, marks the model dirty, and increments version.
    pub fn apply_edit(&mut self, edit: &EditOperation) -> ChangeEvent {
        let event = self.buffer.apply_edit(edit);
        self.is_dirty = true;
        self.version += 1;
        event
    }

    /// Applies multiple edits with undo information.
    pub fn apply_edits(&mut self, edits: &[EditOperation]) -> Vec<EditResult> {
        let results = self.buffer.apply_edits_with_undo(edits);
        if !edits.is_empty() {
            self.is_dirty = true;
            self.version += 1;
        }
        results
    }

    /// Returns the full content of the model.
    pub fn get_full_content(&self) -> String {
        self.buffer.text()
    }

    /// Replaces the model options.
    pub fn set_options(&mut self, options: TextModelOptions) {
        self.options = options;
    }

    /// Detects the indentation style from buffer content.
    ///
    /// Returns `(insert_spaces, tab_size)`.
    pub fn detect_indentation(&self) -> (bool, u32) {
        let info = self.buffer.detect_indentation();
        (!info.use_tabs, info.tab_size)
    }

    /// Increments the version counter and returns the new value.
    pub fn increment_version(&mut self) -> i32 {
        self.version += 1;
        self.version
    }

    /// Returns the number of lines in the model.
    pub fn line_count(&self) -> u32 {
        self.buffer.get_line_count()
    }

    /// Returns the content of a specific line.
    pub fn get_line_content(&self, line: u32) -> String {
        self.buffer.get_line_content(line)
    }

    /// Marks the model as saved (not dirty) and returns the current version.
    pub fn mark_saved(&mut self) -> i32 {
        self.is_dirty = false;
        self.version
    }

    /// Prepares content for saving by applying `TextModelOptions` transformations
    /// (trim trailing whitespace, insert final newline, trim final newlines).
    pub fn get_save_content(&self) -> String {
        let mut content = self.get_full_content();

        if self.options.trim_trailing_whitespace {
            content = content
                .lines()
                .map(str::trim_end)
                .collect::<Vec<_>>()
                .join(self.line_ending.as_str());
            if content.ends_with(self.line_ending.as_str()) || self.options.insert_final_newline {
                content.push_str(self.line_ending.as_str());
            }
        }

        if self.options.trim_final_newlines {
            let eol = self.line_ending.as_str();
            while content.ends_with(&format!("{eol}{eol}")) {
                content.truncate(content.len() - eol.len());
            }
        }

        if self.options.insert_final_newline {
            let eol = self.line_ending.as_str();
            if !content.ends_with(eol) {
                content.push_str(eol);
            }
        }

        content
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Position, Range};

    fn pos(line: u32, col: u32) -> Position {
        Position::new(line, col)
    }

    #[test]
    fn new_model_basic() {
        let model = TextModel::new("hello\nworld", "plaintext", "file:///test.txt");
        assert_eq!(model.language_id, "plaintext");
        assert_eq!(model.version, 1);
        assert!(!model.is_dirty);
        assert!(!model.is_readonly);
        assert!(!model.is_large_file);
        assert_eq!(model.encoding, Encoding::Utf8);
        assert_eq!(model.line_count(), 2);
    }

    #[test]
    fn apply_edit_marks_dirty() {
        let mut model = TextModel::new("hello", "rust", "file:///test.rs");
        let edit = EditOperation::insert(pos(0, 5), " world".into());
        model.apply_edit(&edit);
        assert!(model.is_dirty);
        assert_eq!(model.version, 2);
        assert_eq!(model.get_full_content(), "hello world");
    }

    #[test]
    fn apply_edits_with_undo() {
        let mut model = TextModel::new("hello world", "rust", "file:///test.rs");
        let edits = vec![EditOperation::replace(
            Range::new(pos(0, 6), pos(0, 11)),
            "rust".into(),
        )];
        let results = model.apply_edits(&edits);
        assert_eq!(model.get_full_content(), "hello rust");
        assert_eq!(results.len(), 1);

        model.buffer.apply_edit(&results[0].inverse_edit);
        assert_eq!(model.get_full_content(), "hello world");
    }

    #[test]
    fn detect_indentation_spaces() {
        let model = TextModel::new(
            "function() {\n    a;\n    b;\n        c;\n}",
            "js",
            "file:///test.js",
        );
        let (insert_spaces, tab_size) = model.detect_indentation();
        assert!(insert_spaces);
        assert_eq!(tab_size, 4);
    }

    #[test]
    fn detect_indentation_tabs() {
        let model = TextModel::new("function() {\n\ta;\n\tb;\n}", "js", "file:///test.js");
        let (insert_spaces, _) = model.detect_indentation();
        assert!(!insert_spaces);
    }

    #[test]
    fn increment_version() {
        let mut model = TextModel::new("hello", "txt", "file:///test.txt");
        assert_eq!(model.version, 1);
        let v = model.increment_version();
        assert_eq!(v, 2);
        assert_eq!(model.version, 2);
    }

    #[test]
    fn mark_saved() {
        let mut model = TextModel::new("hello", "txt", "file:///test.txt");
        model.apply_edit(&EditOperation::insert(pos(0, 5), "!".into()));
        assert!(model.is_dirty);
        model.mark_saved();
        assert!(!model.is_dirty);
    }

    #[test]
    fn get_save_content_inserts_final_newline() {
        let mut model = TextModel::new("hello", "txt", "file:///test.txt");
        model.options.insert_final_newline = true;
        let content = model.get_save_content();
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn get_save_content_trims_trailing_whitespace() {
        let mut model = TextModel::new("hello   \nworld  \n", "txt", "file:///test.txt");
        model.options.trim_trailing_whitespace = true;
        model.options.insert_final_newline = true;
        let content = model.get_save_content();
        assert!(content.starts_with("hello\nworld\n"));
    }

    #[test]
    fn options_default() {
        let opts = TextModelOptions::default();
        assert_eq!(opts.tab_size, 4);
        assert!(opts.insert_spaces);
        assert!(!opts.trim_trailing_whitespace);
        assert!(opts.insert_final_newline);
    }

    #[test]
    fn get_line_content() {
        let model = TextModel::new("aaa\nbbb\nccc", "txt", "file:///test.txt");
        assert_eq!(model.get_line_content(0), "aaa");
        assert_eq!(model.get_line_content(1), "bbb");
        assert_eq!(model.get_line_content(2), "ccc");
    }
}
