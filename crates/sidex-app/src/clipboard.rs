//! Clipboard service — OS clipboard integration with history, multi-cursor, and
//! whole-line semantics.

use std::time::Instant;

use anyhow::{Context, Result};

// ── Clipboard source ─────────────────────────────────────────────────────────

/// Where a clipboard entry originated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipboardSource {
    User,
    Editor,
    Terminal,
    External,
}

// ── Clipboard entry ──────────────────────────────────────────────────────────

/// A single item in the clipboard history ring.
#[derive(Clone, Debug)]
pub struct ClipboardEntry {
    pub text: String,
    pub timestamp: Instant,
    pub source: ClipboardSource,
    /// When `true`, the text was copied with no selection (entire line).
    /// Paste should insert above the current line rather than at cursor.
    pub is_whole_line: bool,
    /// When `true`, the entry was produced from a multi-cursor selection.
    /// Each cursor's text is stored in `pieces`; `text` is the joined form.
    pub is_multi_cursor: bool,
    /// Per-cursor pieces. For single-cursor copies this has one element
    /// identical to `text`.
    pub pieces: Vec<String>,
}

impl ClipboardEntry {
    fn single(text: String, source: ClipboardSource) -> Self {
        Self {
            pieces: vec![text.clone()],
            text,
            timestamp: Instant::now(),
            source,
            is_whole_line: false,
            is_multi_cursor: false,
        }
    }

    fn whole_line(text: String, source: ClipboardSource) -> Self {
        Self {
            pieces: vec![text.clone()],
            text,
            timestamp: Instant::now(),
            source,
            is_whole_line: true,
            is_multi_cursor: false,
        }
    }

    fn multi_cursor(pieces: Vec<String>, source: ClipboardSource) -> Self {
        let text = pieces.join("\n");
        Self {
            text,
            timestamp: Instant::now(),
            source,
            is_whole_line: false,
            is_multi_cursor: true,
            pieces,
        }
    }
}

// ── Clipboard service ────────────────────────────────────────────────────────

const DEFAULT_MAX_HISTORY: usize = 20;

/// Manages OS clipboard access and an in-process history ring.
#[derive(Debug)]
pub struct ClipboardService {
    history: Vec<ClipboardEntry>,
    max_history: usize,
}

impl Default for ClipboardService {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardService {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            max_history: DEFAULT_MAX_HISTORY,
        }
    }

    pub fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    // ── Copy ─────────────────────────────────────────────────────────────

    /// Copies `text` to the OS clipboard and pushes it onto the history ring.
    pub fn copy(&mut self, text: &str, source: ClipboardSource) -> Result<()> {
        write_os_clipboard(text)?;
        self.push_entry(ClipboardEntry::single(text.to_string(), source));
        Ok(())
    }

    /// Copies an entire line (no active selection). The entry is tagged so that
    /// paste can insert above the current line.
    pub fn copy_whole_line(&mut self, text: &str, source: ClipboardSource) -> Result<()> {
        write_os_clipboard(text)?;
        self.push_entry(ClipboardEntry::whole_line(text.to_string(), source));
        Ok(())
    }

    /// Copies text from multiple cursors. Each piece is stored separately so
    /// that pasting with the same number of cursors distributes one piece per
    /// cursor.
    pub fn copy_multi_cursor(
        &mut self,
        pieces: Vec<String>,
        source: ClipboardSource,
    ) -> Result<()> {
        let joined = pieces.join("\n");
        write_os_clipboard(&joined)?;
        self.push_entry(ClipboardEntry::multi_cursor(pieces, source));
        Ok(())
    }

    /// Alias for [`copy`] — the caller is responsible for deleting the source
    /// text after calling this.
    pub fn cut(&mut self, text: &str, source: ClipboardSource) -> Result<()> {
        self.copy(text, source)
    }

    // ── Paste ────────────────────────────────────────────────────────────

    /// Reads text from the OS clipboard.
    pub fn paste(&self) -> Result<String> {
        read_os_clipboard()
    }

    /// Reads text from a specific history entry (0 = most recent).
    pub fn paste_from_history(&self, index: usize) -> Result<String> {
        self.history
            .get(index)
            .map(|e| e.text.clone())
            .context("clipboard history index out of range")
    }

    /// Returns the most recent entry, if any.
    pub fn latest(&self) -> Option<&ClipboardEntry> {
        self.history.first()
    }

    /// Returns `true` if the latest entry was a multi-cursor copy whose piece
    /// count matches `cursor_count`.
    pub fn should_distribute_paste(&self, cursor_count: usize) -> bool {
        self.history
            .first()
            .is_some_and(|e| e.is_multi_cursor && e.pieces.len() == cursor_count)
    }

    /// Returns the per-cursor pieces from the latest entry. Falls back to
    /// repeating the full text for each cursor.
    pub fn pieces_for_cursors(&self, cursor_count: usize) -> Vec<String> {
        match self.history.first() {
            Some(entry) if entry.is_multi_cursor && entry.pieces.len() == cursor_count => {
                entry.pieces.clone()
            }
            Some(entry) => vec![entry.text.clone(); cursor_count],
            None => vec![String::new(); cursor_count],
        }
    }

    // ── History ──────────────────────────────────────────────────────────

    pub fn history(&self) -> &[ClipboardEntry] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn max_history(&self) -> usize {
        self.max_history
    }

    pub fn set_max_history(&mut self, max: usize) {
        self.max_history = max;
        self.trim();
    }

    fn push_entry(&mut self, entry: ClipboardEntry) {
        self.history.insert(0, entry);
        self.trim();
    }

    fn trim(&mut self) {
        if self.history.len() > self.max_history {
            self.history.truncate(self.max_history);
        }
    }
}

// ── OS clipboard helpers ─────────────────────────────────────────────────────

fn write_os_clipboard(text: &str) -> Result<()> {
    let mut cb = arboard::Clipboard::new().context("failed to init clipboard")?;
    cb.set_text(text).context("failed to set clipboard text")?;
    Ok(())
}

fn read_os_clipboard() -> Result<String> {
    let mut cb = arboard::Clipboard::new().context("failed to init clipboard")?;
    cb.get_text().context("clipboard does not contain text")
}

// ── Legacy free-function API (kept for backwards compat) ─────────────────────

/// Copies text to the system clipboard.
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    write_os_clipboard(text)
}

/// Reads text from the system clipboard. Returns `None` if empty.
pub fn paste_from_clipboard() -> Option<String> {
    read_os_clipboard().ok()
}

/// Cuts text: copies to clipboard (caller deletes from document).
pub fn cut_to_clipboard(text: &str) -> Result<()> {
    write_os_clipboard(text)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_ordering_and_cap() {
        let mut svc = ClipboardService::new().with_max_history(3);

        // Push entries directly (avoids needing real clipboard in CI)
        svc.push_entry(ClipboardEntry::single("a".into(), ClipboardSource::User));
        svc.push_entry(ClipboardEntry::single("b".into(), ClipboardSource::User));
        svc.push_entry(ClipboardEntry::single("c".into(), ClipboardSource::User));
        svc.push_entry(ClipboardEntry::single("d".into(), ClipboardSource::User));

        assert_eq!(svc.history().len(), 3);
        assert_eq!(svc.history()[0].text, "d");
        assert_eq!(svc.history()[2].text, "b");
    }

    #[test]
    fn paste_from_history_bounds() {
        let svc = ClipboardService::new();
        assert!(svc.paste_from_history(0).is_err());
    }

    #[test]
    fn multi_cursor_distribute() {
        let mut svc = ClipboardService::new();
        let pieces = vec!["alpha".into(), "beta".into(), "gamma".into()];
        svc.push_entry(ClipboardEntry::multi_cursor(pieces.clone(), ClipboardSource::Editor));

        assert!(svc.should_distribute_paste(3));
        assert!(!svc.should_distribute_paste(2));

        let dist = svc.pieces_for_cursors(3);
        assert_eq!(dist, pieces);

        let fallback = svc.pieces_for_cursors(5);
        assert_eq!(fallback.len(), 5);
        assert!(fallback.iter().all(|p| p == "alpha\nbeta\ngamma"));
    }

    #[test]
    fn whole_line_flag() {
        let entry = ClipboardEntry::whole_line("fn main() {}".into(), ClipboardSource::Editor);
        assert!(entry.is_whole_line);
        assert!(!entry.is_multi_cursor);
    }

    #[test]
    fn clear_history() {
        let mut svc = ClipboardService::new();
        svc.push_entry(ClipboardEntry::single("x".into(), ClipboardSource::User));
        assert_eq!(svc.history().len(), 1);
        svc.clear_history();
        assert!(svc.history().is_empty());
    }
}
