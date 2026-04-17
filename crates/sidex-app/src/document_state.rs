//! Per-document state — ties together buffer, parser, highlighting,
//! diagnostics, viewport, and file metadata for a single open file.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use sidex_editor::decoration::DecorationCollection;
use sidex_editor::{Document, Viewport};
use sidex_syntax::highlight::{HighlightConfig, HighlightEvent, Highlighter};
use sidex_syntax::parser::DocumentParser;
use sidex_syntax::LanguageRegistry;
use sidex_text::encoding::Encoding;

/// Diagnostic sourced from LSP or linters, stored per-document.
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub line: u32,
    pub col_start: u32,
    pub col_end: u32,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub source: Option<String>,
}

/// Severity levels for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// All state associated with a single open document / editor tab.
pub struct DocumentState {
    pub document: Document,
    pub file_path: Option<PathBuf>,
    pub language_id: String,
    pub parser: Option<DocumentParser>,
    pub highlight_events: Vec<HighlightEvent>,
    pub highlighter: Highlighter,
    pub highlight_config: Option<HighlightConfig>,
    pub diagnostics: Vec<DiagnosticEntry>,
    pub viewport: Viewport,
    pub decorations: DecorationCollection,
    pub encoding: Encoding,
    /// Version counter synced with the editor `Document::version` at last save.
    saved_version: u64,
}

impl DocumentState {
    /// Create a new empty (untitled) document state.
    pub fn new_untitled() -> Self {
        Self {
            document: Document::new(),
            file_path: None,
            language_id: "plaintext".to_owned(),
            parser: None,
            highlight_events: Vec::new(),
            highlighter: Highlighter::new(),
            highlight_config: None,
            diagnostics: Vec::new(),
            viewport: Viewport::new(20.0, 600.0, 800.0),
            decorations: DecorationCollection::new(),
            encoding: Encoding::Utf8,
            saved_version: 0,
        }
    }

    /// Open a file from disk, detecting encoding and language.
    pub fn open_file(path: &Path, lang_registry: &LanguageRegistry) -> Result<Self> {
        let bytes =
            std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let encoding = sidex_text::encoding::detect_encoding(&bytes);
        let text = sidex_text::encoding::decode(&bytes, encoding)
            .with_context(|| format!("failed to decode {} as {encoding}", path.display()))?;

        let document = Document::from_str(&text);

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();

        let language = lang_registry.language_for_extension(&ext);
        let language_id = language.map_or_else(|| "plaintext".to_owned(), |l| l.name.clone());

        let parser = language.map(DocumentParser::new);

        let highlight_config = language
            .and_then(|lang| {
                lang.highlight_query
                    .as_ref()
                    .map(|q| HighlightConfig::new(lang.ts_language.clone(), q).ok())
            })
            .flatten();

        let mut state = Self {
            document,
            file_path: Some(path.to_path_buf()),
            language_id,
            parser,
            highlight_events: Vec::new(),
            highlighter: Highlighter::new(),
            highlight_config,
            diagnostics: Vec::new(),
            viewport: Viewport::new(20.0, 600.0, 800.0),
            decorations: DecorationCollection::new(),
            encoding,
            saved_version: 0,
        };

        state.initial_parse();
        state.update_highlights();

        Ok(state)
    }

    /// Perform the initial full parse of the document.
    fn initial_parse(&mut self) {
        if let Some(parser) = self.parser.as_mut() {
            let text = self.document.text();
            parser.parse(&text);
        }
    }

    /// Save the document to its current file path.
    pub fn save(&mut self) -> Result<()> {
        let path = self.file_path.as_ref().context("no file path set")?;
        self.write_to(path)?;
        self.saved_version = self.document.version;
        self.document.is_modified = false;
        Ok(())
    }

    /// Save the document to a new path.
    pub fn save_as(&mut self, path: &Path) -> Result<()> {
        self.write_to(path)?;
        self.file_path = Some(path.to_path_buf());
        self.saved_version = self.document.version;
        self.document.is_modified = false;
        Ok(())
    }

    fn write_to(&self, path: &Path) -> Result<()> {
        let text = self.document.text();
        let output = sidex_text::normalize_line_endings(&text, self.document.line_ending);
        let bytes = sidex_text::encoding::encode(&output, self.encoding)
            .with_context(|| format!("failed to encode for {}", path.display()))?;
        std::fs::write(path, bytes)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    /// Re-highlight visible lines after an edit.
    pub fn update_highlights(&mut self) {
        let config = match self.highlight_config.as_ref() {
            Some(c) => c,
            None => return,
        };

        let text = self.document.text();
        if text.is_empty() {
            self.highlight_events.clear();
            return;
        }

        match self.highlighter.highlight(config, &text, None) {
            Ok(events) => self.highlight_events = events,
            Err(e) => log::warn!("highlight failed: {e}"),
        }
    }

    /// Replace stored diagnostics (called when LSP publishes new diagnostics).
    pub fn update_diagnostics(&mut self, diagnostics: Vec<DiagnosticEntry>) {
        self.diagnostics = diagnostics;
    }

    /// Called after every edit: bump version, re-parse incrementally, re-highlight.
    pub fn on_edit(&mut self) {
        if let Some(parser) = self.parser.as_mut() {
            let text = self.document.text();
            if let Some(old_tree) = parser.tree().cloned() {
                parser.parse_incremental(&text, &old_tree, &[]);
            } else {
                parser.parse(&text);
            }
        }

        self.update_highlights();
    }

    /// Whether the document has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.document.version != self.saved_version
    }

    /// Short display name for the tab (filename or "Untitled").
    pub fn display_name(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(String::from)
            .unwrap_or_else(|| "Untitled".to_owned())
    }

    /// File URI for LSP communication.
    pub fn uri(&self) -> Option<String> {
        self.file_path
            .as_ref()
            .map(|p| format!("file://{}", p.display()))
    }
}
