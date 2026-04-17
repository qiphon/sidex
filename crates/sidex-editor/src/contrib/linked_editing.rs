//! Linked editing ranges — mirrors VS Code's `LinkedEditingContribution`.
//!
//! When the cursor is inside a linked range (e.g. an HTML tag name), edits
//! to that range are mirrored in all other linked ranges simultaneously.
//! Includes a fallback HTML tag matcher for when no LSP is available.

use sidex_text::{Buffer, Position, Range};

/// Result from an LSP linked editing ranges request.
#[derive(Debug, Clone)]
pub struct LinkedEditingRangesResult {
    /// The linked ranges.
    pub ranges: Vec<Range>,
    /// A word pattern regex that constrains valid edits.
    pub word_pattern: Option<String>,
}

/// Full state for the linked-editing feature.
#[derive(Debug, Clone, Default)]
pub struct LinkedEditingState {
    /// Whether linked editing is currently active.
    pub is_active: bool,
    /// The set of ranges that are linked together.
    pub ranges: Vec<Range>,
    /// A word pattern regex that constrains valid edits within linked ranges.
    pub word_pattern: Option<String>,
    /// The position that triggered the linked editing session.
    pub trigger_position: Option<Position>,
    /// Whether an LSP request is in-flight.
    pub is_loading: bool,
    /// The original text in each range at activation time (for undo).
    pub original_texts: Vec<String>,
    /// Whether real-time sync is enabled (edit one range, all update).
    pub sync_enabled: bool,
}

impl LinkedEditingState {
    /// Requests linked editing ranges from the LSP at the given position.
    pub fn request(&mut self, pos: Position) {
        self.is_loading = true;
        self.trigger_position = Some(pos);
    }

    /// Activates linked editing with the given ranges.
    pub fn activate(&mut self, pos: Position, ranges: Vec<Range>, word_pattern: Option<String>) {
        self.is_active = !ranges.is_empty();
        self.trigger_position = Some(pos);
        self.ranges = ranges;
        self.word_pattern = word_pattern;
        self.is_loading = false;
        self.sync_enabled = true;
    }

    /// Activates from an LSP result.
    pub fn activate_from_lsp(&mut self, pos: Position, result: LinkedEditingRangesResult) {
        self.activate(pos, result.ranges, result.word_pattern);
    }

    /// Stores the original text from each linked range for undo support.
    pub fn capture_originals(&mut self, buffer: &Buffer) {
        self.original_texts = self
            .ranges
            .iter()
            .map(|r| {
                let start = buffer.position_to_offset(r.start);
                let end = buffer.position_to_offset(r.end);
                buffer.slice(start..end)
            })
            .collect();
    }

    /// Deactivates linked editing.
    pub fn deactivate(&mut self) {
        self.is_active = false;
        self.ranges.clear();
        self.word_pattern = None;
        self.trigger_position = None;
        self.is_loading = false;
        self.original_texts.clear();
    }

    /// Returns `true` if the given position falls inside one of the linked
    /// ranges.
    #[must_use]
    pub fn contains_position(&self, pos: Position) -> bool {
        self.ranges.iter().any(|r| r.contains(pos))
    }

    /// Returns the linked range that contains `pos`, if any.
    #[must_use]
    pub fn range_at(&self, pos: Position) -> Option<&Range> {
        self.ranges.iter().find(|r| r.contains(pos))
    }

    /// Returns all other ranges that should mirror an edit made at `pos`.
    #[must_use]
    pub fn mirror_ranges(&self, pos: Position) -> Vec<Range> {
        self.ranges
            .iter()
            .filter(|r| !r.contains(pos))
            .copied()
            .collect()
    }

    /// Applies a synchronized edit: replaces text in ALL linked ranges with
    /// `new_text`. Edits are applied in reverse order to preserve offsets.
    pub fn apply_sync_edit(&self, buffer: &mut Buffer, new_text: &str) {
        if !self.sync_enabled || !self.is_active {
            return;
        }
        let mut sorted_ranges = self.ranges.clone();
        sorted_ranges.sort_by(|a, b| b.start.cmp(&a.start));

        for range in &sorted_ranges {
            let start = buffer.position_to_offset(range.start);
            let end = buffer.position_to_offset(range.end);
            buffer.replace(start..end, new_text);
        }
    }

    /// Checks if the cursor has moved outside all linked ranges and auto-
    /// deactivates if so. Returns `true` if deactivated.
    pub fn check_cursor(&mut self, pos: Position) -> bool {
        if self.is_active && !self.contains_position(pos) {
            self.deactivate();
            return true;
        }
        false
    }
}

// ── Linked editing ranges result ─────────────────────────────────

/// Linked editing ranges with an optional word pattern constraint.
#[derive(Debug, Clone)]
pub struct LinkedEditingRanges {
    pub ranges: Vec<Range>,
    pub word_pattern: Option<String>,
}

// ── HTML tag matching fallback ───────────────────────────────────

/// Gets linked editing ranges at `position` for the given language.
///
/// Uses LSP `textDocument/linkedEditingRange` when available; falls back
/// to a simple text-based HTML tag matcher for `html`/`xml`-family languages.
#[must_use]
pub fn get_linked_ranges(
    buffer: &Buffer,
    position: Position,
    language: &str,
) -> Option<LinkedEditingRanges> {
    match language {
        "html" | "xml" | "jsx" | "tsx" | "vue" | "svelte" | "php" | "erb" => {
            find_html_tag_pair(buffer, position)
        }
        _ => None,
    }
}

/// Finds the matching open/close HTML tag pair at the cursor position
/// and returns their tag-name ranges as linked editing ranges.
fn find_html_tag_pair(buffer: &Buffer, position: Position) -> Option<LinkedEditingRanges> {
    let line = buffer.line_content(position.line as usize);
    let col = position.column as usize;

    // Determine if cursor is inside an opening or closing tag.
    let before: String = line.chars().take(col).collect();
    let after: String = line.chars().skip(col).collect();

    // Find the tag surrounding the cursor.
    let tag_open_lt = before.rfind('<')?;
    let rest_from_lt = &line[tag_open_lt..];
    let tag_close_gt = rest_from_lt.find('>')? + tag_open_lt;

    if col > tag_close_gt + 1 {
        return None;
    }

    let tag_content = &line[tag_open_lt + 1..tag_close_gt];
    let is_closing = tag_content.starts_with('/');

    let name_src = if is_closing {
        &tag_content[1..]
    } else {
        tag_content
    };
    let tag_name: String = name_src
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();

    if tag_name.is_empty() {
        return None;
    }

    let name_start_in_line = if is_closing {
        tag_open_lt + 2 // skip </
    } else {
        tag_open_lt + 1 // skip <
    };

    let this_range = Range::new(
        Position::new(position.line, name_start_in_line as u32),
        Position::new(position.line, (name_start_in_line + tag_name.len()) as u32),
    );

    let text = buffer.text();
    let _ = after;

    if is_closing {
        // Find matching opening tag above
        if let Some(open_range) = find_matching_open_tag(&text, &tag_name, position.line, buffer) {
            return Some(LinkedEditingRanges {
                ranges: vec![open_range, this_range],
                word_pattern: Some(r"[a-zA-Z][\w-]*".into()),
            });
        }
    } else {
        // Find matching closing tag below
        if let Some(close_range) = find_matching_close_tag(&text, &tag_name, position.line, buffer)
        {
            return Some(LinkedEditingRanges {
                ranges: vec![this_range, close_range],
                word_pattern: Some(r"[a-zA-Z][\w-]*".into()),
            });
        }
    }

    None
}

fn find_matching_close_tag(
    text: &str,
    tag_name: &str,
    start_line: u32,
    buffer: &Buffer,
) -> Option<Range> {
    let close_pattern = format!("</{tag_name}");
    let open_pattern = format!("<{tag_name}");
    let mut depth = 1i32;
    let line_count = buffer.len_lines();

    for line_idx in (start_line as usize)..line_count {
        let content = buffer.line_content(line_idx);
        let search_start = if line_idx == start_line as usize {
            content.find('>').map_or(0, |p| p + 1)
        } else {
            0
        };
        let content_slice = &content[search_start..];

        let mut offset = 0;
        while offset < content_slice.len() {
            let remaining = &content_slice[offset..];
            if remaining.starts_with(&close_pattern) {
                depth -= 1;
                if depth == 0 {
                    let abs_pos = search_start + offset + 2; // skip </
                    return Some(Range::new(
                        Position::new(line_idx as u32, abs_pos as u32),
                        Position::new(line_idx as u32, (abs_pos + tag_name.len()) as u32),
                    ));
                }
                offset += close_pattern.len();
            } else if remaining.starts_with(&open_pattern) {
                let after_tag = &remaining[open_pattern.len()..];
                if after_tag.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
                    depth += 1;
                }
                offset += open_pattern.len();
            } else {
                offset += 1;
            }
        }
    }
    let _ = text;
    None
}

fn find_matching_open_tag(
    text: &str,
    tag_name: &str,
    start_line: u32,
    buffer: &Buffer,
) -> Option<Range> {
    let close_pattern = format!("</{tag_name}");
    let open_pattern = format!("<{tag_name}");
    let mut depth = 1i32;

    for line_idx in (0..=start_line as usize).rev() {
        let content = buffer.line_content(line_idx);
        let search_end = if line_idx == start_line as usize {
            content.rfind('<').unwrap_or(content.len())
        } else {
            content.len()
        };
        let content_slice = &content[..search_end];

        // Search backwards through the line
        let mut offsets: Vec<(usize, bool)> = Vec::new();
        let mut pos = 0;
        while pos < content_slice.len() {
            let remaining = &content_slice[pos..];
            if remaining.starts_with(&close_pattern) {
                offsets.push((pos, false));
                pos += close_pattern.len();
            } else if remaining.starts_with(&open_pattern) {
                let after = &remaining[open_pattern.len()..];
                if after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
                    offsets.push((pos, true));
                }
                pos += open_pattern.len();
            } else {
                pos += 1;
            }
        }

        for &(off, is_open) in offsets.iter().rev() {
            if !is_open {
                depth += 1;
            } else {
                depth -= 1;
                if depth == 0 {
                    let name_start = off + 1; // skip <
                    return Some(Range::new(
                        Position::new(line_idx as u32, name_start as u32),
                        Position::new(line_idx as u32, (name_start + tag_name.len()) as u32),
                    ));
                }
            }
        }
    }
    let _ = text;
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_editing_lifecycle() {
        let mut state = LinkedEditingState::default();
        let ranges = vec![
            Range::new(Position::new(0, 1), Position::new(0, 4)),
            Range::new(Position::new(0, 10), Position::new(0, 13)),
        ];
        state.activate(Position::new(0, 2), ranges, None);
        assert!(state.is_active);

        let mirrors = state.mirror_ranges(Position::new(0, 2));
        assert_eq!(mirrors.len(), 1);
        assert_eq!(mirrors[0].start.column, 10);

        state.deactivate();
        assert!(!state.is_active);
    }

    #[test]
    fn auto_deactivate_on_cursor_leave() {
        let mut state = LinkedEditingState::default();
        state.activate(
            Position::new(0, 2),
            vec![Range::new(Position::new(0, 0), Position::new(0, 5))],
            None,
        );
        assert!(!state.check_cursor(Position::new(0, 3)));
        assert!(state.check_cursor(Position::new(1, 0)));
        assert!(!state.is_active);
    }

    #[test]
    fn sync_edit() {
        let mut buffer = Buffer::from_str("<div></div>");
        let mut state = LinkedEditingState::default();
        state.activate(
            Position::new(0, 1),
            vec![
                Range::new(Position::new(0, 1), Position::new(0, 4)), // "div"
                Range::new(Position::new(0, 7), Position::new(0, 10)), // "div"
            ],
            None,
        );
        state.apply_sync_edit(&mut buffer, "span");
        assert_eq!(buffer.text(), "<span></span>");
    }

    #[test]
    fn get_linked_ranges_html_open_tag() {
        let buffer = Buffer::from_str("<div>hello</div>");
        let result = get_linked_ranges(&buffer, Position::new(0, 2), "html");
        assert!(result.is_some());
        let lr = result.unwrap();
        assert_eq!(lr.ranges.len(), 2);
        assert_eq!(lr.ranges[0].start.column, 1);
        assert_eq!(lr.ranges[0].end.column, 4);
        assert_eq!(lr.ranges[1].start.column, 12);
        assert_eq!(lr.ranges[1].end.column, 15);
    }

    #[test]
    fn get_linked_ranges_html_close_tag() {
        let buffer = Buffer::from_str("<span>text</span>");
        let result = get_linked_ranges(&buffer, Position::new(0, 13), "html");
        assert!(result.is_some());
        let lr = result.unwrap();
        assert_eq!(lr.ranges.len(), 2);
    }

    #[test]
    fn get_linked_ranges_unsupported_language() {
        let buffer = Buffer::from_str("<div></div>");
        assert!(get_linked_ranges(&buffer, Position::new(0, 2), "rust").is_none());
    }

    #[test]
    fn linked_editing_ranges_struct() {
        let lr = LinkedEditingRanges {
            ranges: vec![Range::new(Position::new(0, 0), Position::new(0, 3))],
            word_pattern: Some(r"\w+".into()),
        };
        assert_eq!(lr.ranges.len(), 1);
        assert!(lr.word_pattern.is_some());
    }
}
