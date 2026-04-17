//! Inlay hints — mirrors VS Code's inlay-hint contribution.
//!
//! Tracks inlay hints (type annotations, parameter names) returned by the
//! language server for rendering between text characters.

use sidex_text::Position;

/// The kind of an inlay hint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlayHintKind {
    /// A type annotation hint (e.g. `: i32`).
    Type,
    /// A parameter name hint (e.g. `name:`).
    Parameter,
    /// An unclassified hint.
    Other,
}

/// A single inlay hint label part (hints can have clickable parts).
#[derive(Debug, Clone)]
pub struct InlayHintLabelPart {
    /// The display text.
    pub value: String,
    /// Optional tooltip (markdown).
    pub tooltip: Option<String>,
    /// Optional command to execute on click.
    pub command_id: Option<String>,
    /// Optional location to navigate to on click.
    pub location: Option<InlayHintLocation>,
}

/// A location reference for an inlay hint click target.
#[derive(Debug, Clone)]
pub struct InlayHintLocation {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

/// Styling information for an inlay hint based on its kind.
#[derive(Debug, Clone)]
pub struct InlayHintStyle {
    /// CSS-like font style: "normal", "italic".
    pub font_style: &'static str,
    /// Whether to show with reduced opacity.
    pub dimmed: bool,
    /// Background color class name.
    pub background: &'static str,
    /// Border style.
    pub border: &'static str,
}

impl InlayHintStyle {
    /// Returns the style for a given hint kind.
    #[must_use]
    pub fn for_kind(kind: InlayHintKind) -> Self {
        match kind {
            InlayHintKind::Type => Self {
                font_style: "italic",
                dimmed: true,
                background: "inlayHint.typeBackground",
                border: "none",
            },
            InlayHintKind::Parameter => Self {
                font_style: "normal",
                dimmed: true,
                background: "inlayHint.parameterBackground",
                border: "none",
            },
            InlayHintKind::Other => Self {
                font_style: "normal",
                dimmed: true,
                background: "inlayHint.otherBackground",
                border: "none",
            },
        }
    }
}

/// A single inlay hint to render in the editor.
#[derive(Debug, Clone)]
pub struct InlayHint {
    /// Position in the document where the hint should be rendered.
    pub position: Position,
    /// The label parts (concatenated for display).
    pub label: Vec<InlayHintLabelPart>,
    /// The kind of hint.
    pub kind: InlayHintKind,
    /// Whether the hint should be rendered with padding on the left.
    pub padding_left: bool,
    /// Whether the hint should be rendered with padding on the right.
    pub padding_right: bool,
    /// Opaque data for deferred resolution.
    pub data: Option<String>,
}

impl InlayHint {
    /// Returns the full display text of this hint.
    #[must_use]
    pub fn display_text(&self) -> String {
        self.label.iter().map(|p| p.value.as_str()).collect()
    }

    /// Returns the total rendered width in characters (including padding).
    #[must_use]
    pub fn rendered_width(&self) -> usize {
        let text_width: usize = self.label.iter().map(|p| p.value.len()).sum();
        let left_pad = if self.padding_left { 1 } else { 0 };
        let right_pad = if self.padding_right { 1 } else { 0 };
        text_width + left_pad + right_pad
    }

    /// Returns the style for this hint.
    #[must_use]
    pub fn style(&self) -> InlayHintStyle {
        InlayHintStyle::for_kind(self.kind)
    }

    /// Returns `true` if any label part has a click handler.
    #[must_use]
    pub fn is_clickable(&self) -> bool {
        self.label
            .iter()
            .any(|p| p.command_id.is_some() || p.location.is_some())
    }
}

/// Full state for the inlay-hints feature.
#[derive(Debug, Clone, Default)]
pub struct InlayHintState {
    /// All inlay hints for the current document / viewport.
    pub hints: Vec<InlayHint>,
    /// Whether hints are currently enabled.
    pub enabled: bool,
    /// Whether a fetch is in-flight.
    pub is_loading: bool,
    /// The viewport range that hints were fetched for.
    pub fetched_range: Option<(u32, u32)>,
}

impl InlayHintState {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    /// Sets hints received from the language server.
    pub fn set_hints(&mut self, hints: Vec<InlayHint>) {
        self.hints = hints;
        self.is_loading = false;
    }

    /// Returns hints for a specific line.
    #[must_use]
    pub fn hints_for_line(&self, line: u32) -> Vec<&InlayHint> {
        self.hints
            .iter()
            .filter(|h| h.position.line == line)
            .collect()
    }

    /// Returns the total extra width (in characters) that inlay hints add to
    /// a specific line (for layout computation).
    #[must_use]
    pub fn extra_width_for_line(&self, line: u32) -> usize {
        self.hints_for_line(line)
            .iter()
            .map(|h| h.rendered_width())
            .sum()
    }

    /// Handles a click at the given position, returning the command or location
    /// to execute if a clickable hint part was hit.
    #[must_use]
    pub fn click_at(&self, line: u32, hint_index: usize, part_index: usize) -> Option<ClickResult> {
        let hints = self.hints_for_line(line);
        let hint = hints.get(hint_index)?;
        let part = hint.label.get(part_index)?;

        if let Some(cmd) = &part.command_id {
            return Some(ClickResult::Command(cmd.clone()));
        }
        if let Some(loc) = &part.location {
            return Some(ClickResult::Navigate(loc.clone()));
        }
        None
    }

    /// Clears all hints.
    pub fn clear(&mut self) {
        self.hints.clear();
        self.is_loading = false;
        self.fetched_range = None;
    }

    /// Requests a refresh for the given viewport range.
    pub fn request_refresh(&mut self, start_line: u32, end_line: u32) {
        self.fetched_range = Some((start_line, end_line));
        self.is_loading = true;
    }
}

/// Result of clicking on an inlay hint part.
#[derive(Debug, Clone)]
pub enum ClickResult {
    /// Execute a command.
    Command(String),
    /// Navigate to a location.
    Navigate(InlayHintLocation),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hint(line: u32, col: u32, text: &str, kind: InlayHintKind) -> InlayHint {
        InlayHint {
            position: Position::new(line, col),
            label: vec![InlayHintLabelPart {
                value: text.into(),
                tooltip: None,
                command_id: None,
                location: None,
            }],
            kind,
            padding_left: true,
            padding_right: false,
            data: None,
        }
    }

    #[test]
    fn hints_for_line() {
        let mut state = InlayHintState::new();
        state.set_hints(vec![
            make_hint(5, 10, ": i32", InlayHintKind::Type),
            make_hint(7, 3, "name:", InlayHintKind::Parameter),
        ]);
        assert_eq!(state.hints_for_line(5).len(), 1);
        assert_eq!(state.hints_for_line(7).len(), 1);
        assert!(state.hints_for_line(0).is_empty());
    }

    #[test]
    fn rendered_width() {
        let hint = make_hint(0, 0, ": i32", InlayHintKind::Type);
        assert_eq!(hint.rendered_width(), 6); // 5 chars + 1 left pad
    }

    #[test]
    fn extra_width() {
        let mut state = InlayHintState::new();
        state.set_hints(vec![
            make_hint(5, 10, ": i32", InlayHintKind::Type),
            make_hint(5, 20, ": bool", InlayHintKind::Type),
        ]);
        assert_eq!(state.extra_width_for_line(5), 13); // 6 + 7
    }

    #[test]
    fn clickable_hint() {
        let hint = InlayHint {
            position: Position::new(0, 0),
            label: vec![InlayHintLabelPart {
                value: "Go".into(),
                tooltip: None,
                command_id: Some("goToDefinition".into()),
                location: None,
            }],
            kind: InlayHintKind::Other,
            padding_left: false,
            padding_right: false,
            data: None,
        };
        assert!(hint.is_clickable());
    }

    #[test]
    fn style_for_kind() {
        let style = InlayHintStyle::for_kind(InlayHintKind::Type);
        assert_eq!(style.font_style, "italic");
    }
}
