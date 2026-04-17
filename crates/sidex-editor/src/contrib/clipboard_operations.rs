//! Clipboard operations — mirrors VS Code's clipboard contribution.
//!
//! Enhanced cut/copy/paste: copy with syntax highlighting data, paste with
//! auto-indentation, multi-cursor copy/paste distribution.

use sidex_text::{Buffer, Position, Range};

/// Metadata attached to a clipboard entry for rich paste.
#[derive(Debug, Clone, Default)]
pub struct ClipboardMetadata {
    /// Whether the clipboard contains a full line (should paste as a new line).
    pub is_full_line: bool,
    /// Number of cursors that produced this clipboard content.
    pub cursor_count: usize,
    /// Per-cursor text segments (for distributing paste across cursors).
    pub segments: Vec<String>,
    /// Optional syntax-highlighted HTML for rich paste into other apps.
    pub html: Option<String>,
    /// The mode that produced this clipboard (e.g. "column" for box selection).
    pub mode: Option<String>,
}

impl ClipboardMetadata {
    /// Returns `true` if this metadata supports multi-cursor distribution
    /// for the given number of cursors.
    #[must_use]
    pub fn supports_distribution(&self, cursor_count: usize) -> bool {
        self.segments.len() == cursor_count && cursor_count > 1
    }
}

/// Per-cursor clipboard entry for multi-cursor copy/paste.
#[derive(Debug, Clone)]
pub struct CursorClipboard {
    /// The per-cursor clipboard ring (most recent first).
    entries: Vec<ClipboardEntry>,
    /// Maximum ring size.
    max_size: usize,
}

/// A single entry in the clipboard ring.
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub text: String,
    pub metadata: ClipboardMetadata,
}

impl Default for CursorClipboard {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            max_size: 20,
        }
    }
}

impl CursorClipboard {
    /// Pushes a new entry to the front of the ring.
    pub fn push(&mut self, entry: ClipboardEntry) {
        self.entries.insert(0, entry);
        if self.entries.len() > self.max_size {
            self.entries.truncate(self.max_size);
        }
    }

    /// Returns the most recent entry.
    #[must_use]
    pub fn latest(&self) -> Option<&ClipboardEntry> {
        self.entries.first()
    }

    /// Returns all entries.
    #[must_use]
    pub fn entries(&self) -> &[ClipboardEntry] {
        &self.entries
    }

    /// Clears the clipboard ring.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// Copies the selected text and produces clipboard metadata.
#[must_use]
pub fn copy_selections(buffer: &Buffer, selections: &[Range]) -> (String, ClipboardMetadata) {
    let mut texts = Vec::with_capacity(selections.len());
    for sel in selections {
        let start = buffer.position_to_offset(sel.start);
        let end = buffer.position_to_offset(sel.end);
        texts.push(buffer.slice(start..end));
    }

    let full_text = texts.join("\n");
    let metadata = ClipboardMetadata {
        is_full_line: false,
        cursor_count: selections.len(),
        segments: texts,
        html: None,
        mode: None,
    };
    (full_text, metadata)
}

/// Copies a full line (no selection) — the paste should insert a new line.
#[must_use]
pub fn copy_line(buffer: &Buffer, line: u32) -> (String, ClipboardMetadata) {
    let content = buffer.line_content(line as usize).clone();
    let metadata = ClipboardMetadata {
        is_full_line: true,
        cursor_count: 1,
        segments: vec![content.clone()],
        html: None,
        mode: None,
    };
    (content, metadata)
}

/// Multi-cursor copy: each cursor gets its own clipboard segment.
#[must_use]
pub fn copy_multi_cursor(buffer: &Buffer, selections: &[Range]) -> (String, ClipboardMetadata) {
    let segments: Vec<String> = selections
        .iter()
        .map(|sel| {
            let start = buffer.position_to_offset(sel.start);
            let end = buffer.position_to_offset(sel.end);
            buffer.slice(start..end)
        })
        .collect();

    let full_text = segments.join("\n");
    let metadata = ClipboardMetadata {
        is_full_line: false,
        cursor_count: selections.len(),
        segments,
        html: None,
        mode: Some("multicursor".into()),
    };
    (full_text, metadata)
}

/// Pastes text, auto-indenting each line to match the current cursor line's
/// indentation.
pub fn paste_and_auto_indent(
    buffer: &mut Buffer,
    pos: Position,
    text: &str,
    _tab_size: u32,
    _use_spaces: bool,
) {
    if text.is_empty() {
        return;
    }

    let current_line = buffer.line_content(pos.line as usize);
    let base_indent = leading_whitespace(&current_line);

    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= 1 {
        let offset = buffer.position_to_offset(pos);
        buffer.insert(offset, text);
        return;
    }

    let paste_indent = lines
        .iter()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| leading_whitespace(l))
        .min()
        .unwrap_or_default();

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
            if !line.trim().is_empty() {
                result.push_str(&base_indent);
                let stripped = strip_indent(line, &paste_indent);
                result.push_str(stripped);
            }
        } else {
            result.push_str(line);
        }
    }
    if text.ends_with('\n') {
        result.push('\n');
    }

    let offset = buffer.position_to_offset(pos);
    buffer.insert(offset, &result);
}

/// Pastes with multi-cursor distribution: each cursor gets its own segment.
pub fn paste_distributed(
    buffer: &mut Buffer,
    positions: &[Position],
    metadata: &ClipboardMetadata,
) -> bool {
    if metadata.segments.len() != positions.len() || positions.is_empty() {
        return false;
    }

    let mut pairs: Vec<_> = positions.iter().zip(metadata.segments.iter()).collect();
    pairs.sort_by(|a, b| b.0.cmp(a.0));

    for (pos, text) in pairs {
        let offset = buffer.position_to_offset(*pos);
        buffer.insert(offset, text);
    }
    true
}

/// Cut operation for multi-cursor: removes text at each selection and returns
/// clipboard metadata.
pub fn cut_multi_cursor(buffer: &mut Buffer, selections: &[Range]) -> (String, ClipboardMetadata) {
    let (text, metadata) = copy_multi_cursor(buffer, selections);

    let mut sorted: Vec<Range> = selections.to_vec();
    sorted.sort_by(|a, b| b.start.cmp(&a.start));

    for sel in &sorted {
        let start = buffer.position_to_offset(sel.start);
        let end = buffer.position_to_offset(sel.end);
        buffer.remove(start..end);
    }

    (text, metadata)
}

fn leading_whitespace(line: &str) -> String {
    line.chars().take_while(|c| c.is_whitespace()).collect()
}

fn strip_indent<'a>(line: &'a str, indent: &str) -> &'a str {
    line.strip_prefix(indent).unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn copy_multiple_selections() {
        let buffer = buf("foo bar baz");
        let sels = vec![
            Range::new(Position::new(0, 0), Position::new(0, 3)),
            Range::new(Position::new(0, 8), Position::new(0, 11)),
        ];
        let (text, meta) = copy_selections(&buffer, &sels);
        assert_eq!(text, "foo\nbaz");
        assert_eq!(meta.segments.len(), 2);
    }

    #[test]
    fn copy_full_line() {
        let buffer = buf("hello\nworld");
        let (text, meta) = copy_line(&buffer, 0);
        assert_eq!(text, "hello");
        assert!(meta.is_full_line);
    }

    #[test]
    fn paste_distributed_works() {
        let mut buffer = buf("aa bb");
        let positions = vec![Position::new(0, 2), Position::new(0, 5)];
        let meta = ClipboardMetadata {
            is_full_line: false,
            cursor_count: 2,
            segments: vec!["X".into(), "Y".into()],
            html: None,
            mode: None,
        };
        let ok = paste_distributed(&mut buffer, &positions, &meta);
        assert!(ok);
        let text = buffer.text();
        assert!(text.contains("aaX"));
        assert!(text.contains("bbY"));
    }

    #[test]
    fn multi_cursor_copy() {
        let buffer = buf("foo bar baz");
        let sels = vec![
            Range::new(Position::new(0, 0), Position::new(0, 3)),
            Range::new(Position::new(0, 4), Position::new(0, 7)),
        ];
        let (_, meta) = copy_multi_cursor(&buffer, &sels);
        assert_eq!(meta.segments, vec!["foo", "bar"]);
        assert_eq!(meta.mode.as_deref(), Some("multicursor"));
        assert!(meta.supports_distribution(2));
    }

    #[test]
    fn clipboard_ring() {
        let mut ring = CursorClipboard::default();
        ring.push(ClipboardEntry {
            text: "first".into(),
            metadata: ClipboardMetadata::default(),
        });
        ring.push(ClipboardEntry {
            text: "second".into(),
            metadata: ClipboardMetadata::default(),
        });
        assert_eq!(ring.latest().unwrap().text, "second");
        assert_eq!(ring.entries().len(), 2);
    }
}
