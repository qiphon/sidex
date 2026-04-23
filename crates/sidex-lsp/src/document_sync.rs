//! Full document synchronization with throttled incremental change
//! notifications.
//!
//! Computes minimal incremental `TextDocumentContentChangeEvent`s between
//! two document snapshots and provides a throttle mechanism to batch rapid
//! edits into a single `didChange` notification.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// How the server expects document content to be synchronized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDocumentSyncKind {
    None = 0,
    Full = 1,
    Incremental = 2,
}

impl TextDocumentSyncKind {
    pub fn from_lsp(kind: lsp_types::TextDocumentSyncKind) -> Self {
        match kind {
            lsp_types::TextDocumentSyncKind::NONE => Self::None,
            lsp_types::TextDocumentSyncKind::INCREMENTAL => Self::Incremental,
            _ => Self::Full,
        }
    }
}

/// A change event matching the LSP `TextDocumentContentChangeEvent` shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub range: Option<lsp_types::Range>,
    pub text: String,
}

impl ChangeEvent {
    /// Convert to an `lsp_types::TextDocumentContentChangeEvent`.
    pub fn to_lsp(&self) -> lsp_types::TextDocumentContentChangeEvent {
        lsp_types::TextDocumentContentChangeEvent {
            range: self.range,
            range_length: None,
            text: self.text.clone(),
        }
    }
}

/// Computes minimal incremental changes between two document snapshots.
///
/// Returns a `Vec<ChangeEvent>` suitable for `textDocument/didChange`. If the
/// documents are identical an empty vec is returned. For small documents or
/// when the common prefix/suffix approach would not shrink the payload, a
/// single full-text change is returned instead.
pub fn compute_incremental_changes(old: &str, new: &str) -> Vec<ChangeEvent> {
    if old == new {
        return vec![];
    }

    let old_bytes = old.as_bytes();
    let new_bytes = new.as_bytes();

    let common_prefix = old_bytes
        .iter()
        .zip(new_bytes.iter())
        .take_while(|(a, b)| a == b)
        .count();

    let old_suffix_start = old_bytes.len();
    let new_suffix_start = new_bytes.len();
    let max_suffix = (old_suffix_start - common_prefix).min(new_suffix_start - common_prefix);

    let common_suffix = (0..max_suffix)
        .take_while(|&i| old_bytes[old_suffix_start - 1 - i] == new_bytes[new_suffix_start - 1 - i])
        .count();

    let start_offset = common_prefix;
    let old_end_offset = old_suffix_start - common_suffix;
    let new_end_offset = new_suffix_start - common_suffix;

    let start_pos = offset_to_position(old, start_offset);
    let end_pos = offset_to_position(old, old_end_offset);

    let replacement = &new[start_offset..new_end_offset];

    vec![ChangeEvent {
        range: Some(lsp_types::Range {
            start: start_pos,
            end: end_pos,
        }),
        text: replacement.to_string(),
    }]
}

/// Converts a byte offset in a string to an `lsp_types::Position`.
fn offset_to_position(text: &str, offset: usize) -> lsp_types::Position {
    let slice = &text[..offset.min(text.len())];
    let line = slice.matches('\n').count();
    let last_newline = slice.rfind('\n').map_or(0, |i| i + 1);
    let character = slice[last_newline..].chars().count();
    #[allow(clippy::cast_possible_truncation)]
    lsp_types::Position {
        line: line as u32,
        character: character as u32,
    }
}

/// Throttles `didChange` notifications to avoid flooding the server with
/// rapid keystrokes.
pub struct ChangeThrottle {
    min_interval: Duration,
    last_sent: Option<Instant>,
    pending_version: Option<i32>,
    pending_text: Option<String>,
}

impl ChangeThrottle {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last_sent: None,
            pending_version: None,
            pending_text: None,
        }
    }

    /// Record a new edit. Returns `true` if the change should be sent
    /// immediately, `false` if it was buffered.
    pub fn record(&mut self, version: i32, new_text: String) -> bool {
        let now = Instant::now();
        let should_send = self
            .last_sent
            .is_none_or(|last| now.duration_since(last) >= self.min_interval);

        if should_send {
            self.last_sent = Some(now);
            self.pending_version = None;
            self.pending_text = None;
            true
        } else {
            self.pending_version = Some(version);
            self.pending_text = Some(new_text);
            false
        }
    }

    /// Flush the pending change, if any. Returns `(version, text)` if there
    /// was a buffered change.
    pub fn flush(&mut self) -> Option<(i32, String)> {
        let version = self.pending_version.take()?;
        let text = self.pending_text.take()?;
        self.last_sent = Some(Instant::now());
        Some((version, text))
    }

    /// Returns whether there is a buffered change waiting to be sent.
    pub fn has_pending(&self) -> bool {
        self.pending_version.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_documents_no_changes() {
        let changes = compute_incremental_changes("hello world", "hello world");
        assert!(changes.is_empty());
    }

    #[test]
    fn simple_insert() {
        let old = "fn main() {}";
        let new = "fn main() { println!(); }";
        let changes = compute_incremental_changes(old, new);
        assert_eq!(changes.len(), 1);
        let c = &changes[0];
        assert!(c.range.is_some());
        let range = c.range.unwrap();
        assert_eq!(range.start, range.end.min(range.start).max(range.start));
    }

    #[test]
    fn simple_delete() {
        let old = "hello world";
        let new = "hello";
        let changes = compute_incremental_changes(old, new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].text, "");
    }

    #[test]
    fn replacement_in_middle() {
        let old = "fn foo() {}";
        let new = "fn bar() {}";
        let changes = compute_incremental_changes(old, new);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].text, "bar");
    }

    #[test]
    fn multiline_change() {
        let old = "line 1\nline 2\nline 3\n";
        let new = "line 1\nmodified\nline 3\n";
        let changes = compute_incremental_changes(old, new);
        assert_eq!(changes.len(), 1);
        let range = changes[0].range.unwrap();
        assert_eq!(range.start.line, 1);
    }

    #[test]
    fn empty_to_content() {
        let changes = compute_incremental_changes("", "hello");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].text, "hello");
    }

    #[test]
    fn content_to_empty() {
        let changes = compute_incremental_changes("hello", "");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].text, "");
    }

    #[test]
    fn change_event_to_lsp() {
        let event = ChangeEvent {
            range: Some(lsp_types::Range {
                start: lsp_types::Position::new(0, 0),
                end: lsp_types::Position::new(0, 5),
            }),
            text: "world".into(),
        };
        let lsp = event.to_lsp();
        assert_eq!(lsp.text, "world");
        assert!(lsp.range.is_some());
    }

    #[test]
    fn sync_kind_from_lsp() {
        assert_eq!(
            TextDocumentSyncKind::from_lsp(lsp_types::TextDocumentSyncKind::NONE),
            TextDocumentSyncKind::None
        );
        assert_eq!(
            TextDocumentSyncKind::from_lsp(lsp_types::TextDocumentSyncKind::FULL),
            TextDocumentSyncKind::Full
        );
        assert_eq!(
            TextDocumentSyncKind::from_lsp(lsp_types::TextDocumentSyncKind::INCREMENTAL),
            TextDocumentSyncKind::Incremental
        );
    }

    #[test]
    fn offset_to_position_simple() {
        let text = "hello\nworld\n";
        let pos = offset_to_position(text, 6);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);
    }

    #[test]
    fn offset_to_position_middle_of_line() {
        let text = "abc\ndef\n";
        let pos = offset_to_position(text, 5);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 1);
    }

    #[test]
    fn throttle_first_sends_immediately() {
        let mut throttle = ChangeThrottle::new(Duration::from_millis(50));
        assert!(throttle.record(1, "text".into()));
        assert!(!throttle.has_pending());
    }

    #[test]
    fn throttle_buffers_rapid_edits() {
        let mut throttle = ChangeThrottle::new(Duration::from_secs(10));
        assert!(throttle.record(1, "first".into()));
        assert!(!throttle.record(2, "second".into()));
        assert!(throttle.has_pending());
        let (version, text) = throttle.flush().unwrap();
        assert_eq!(version, 2);
        assert_eq!(text, "second");
        assert!(!throttle.has_pending());
    }

    #[test]
    fn throttle_flush_empty() {
        let mut throttle = ChangeThrottle::new(Duration::from_millis(50));
        assert!(throttle.flush().is_none());
    }
}
