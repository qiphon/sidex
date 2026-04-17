//! Whitespace renderer — renders whitespace characters as visible glyphs.
//!
//! Renders spaces as dots (·), tabs as arrows (→), and supports multiple
//! rendering modes matching VS Code's `editor.renderWhitespace` setting.

/// When to render whitespace characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderWhitespace {
    /// Never render whitespace.
    None,
    /// Render only boundary whitespace (leading + trailing).
    Boundary,
    /// Render only whitespace inside selections.
    Selection,
    /// Render only trailing whitespace.
    Trailing,
    /// Always render all whitespace.
    All,
}

impl Default for RenderWhitespace {
    fn default() -> Self {
        Self::Selection
    }
}

/// A single whitespace character to render with a visible glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhitespaceGlyph {
    /// The line number (zero-based).
    pub line: u32,
    /// The column (zero-based).
    pub column: u32,
    /// The kind of whitespace.
    pub kind: WhitespaceKind,
}

/// The kind of whitespace character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceKind {
    /// A space character — rendered as `·` (middle dot).
    Space,
    /// A tab character — rendered as `→` (right arrow).
    Tab,
    /// A non-breaking space — rendered as `°`.
    NonBreakingSpace,
    /// A full-width space.
    FullWidthSpace,
}

impl WhitespaceKind {
    /// Returns the glyph character for rendering.
    #[must_use]
    pub fn glyph(self) -> char {
        match self {
            Self::Space => '·',
            Self::Tab => '→',
            Self::NonBreakingSpace => '°',
            Self::FullWidthSpace => '□',
        }
    }
}

/// Full state for whitespace rendering.
#[derive(Debug, Clone, Default)]
pub struct WhitespaceRendererState {
    /// The render mode setting.
    pub mode: RenderWhitespace,
    /// The tab size (for computing tab glyph width).
    pub tab_size: u32,
    /// Computed whitespace glyphs for the visible viewport.
    pub glyphs: Vec<WhitespaceGlyph>,
}

impl WhitespaceRendererState {
    pub fn new(mode: RenderWhitespace, tab_size: u32) -> Self {
        Self {
            mode,
            tab_size,
            glyphs: Vec::new(),
        }
    }

    /// Computes whitespace glyphs for the given lines.
    pub fn compute(
        &mut self,
        lines: &[(u32, &str)],
        selections: &[(u32, u32, u32)], // (line, start_col, end_col)
    ) {
        self.glyphs.clear();
        if self.mode == RenderWhitespace::None {
            return;
        }

        for &(line_num, content) in lines {
            let chars: Vec<char> = content.chars().collect();
            let content_start = chars
                .iter()
                .position(|c| !c.is_whitespace())
                .unwrap_or(chars.len());
            let content_end = chars
                .iter()
                .rposition(|c| !c.is_whitespace())
                .map_or(0, |i| i + 1);

            for (col, &ch) in chars.iter().enumerate() {
                let kind = match ch {
                    ' ' => WhitespaceKind::Space,
                    '\t' => WhitespaceKind::Tab,
                    '\u{00A0}' => WhitespaceKind::NonBreakingSpace,
                    '\u{3000}' => WhitespaceKind::FullWidthSpace,
                    _ => continue,
                };

                let should_render = match self.mode {
                    RenderWhitespace::None => false,
                    RenderWhitespace::All => true,
                    RenderWhitespace::Boundary => col < content_start || col >= content_end,
                    RenderWhitespace::Trailing => col >= content_end,
                    RenderWhitespace::Selection => selections.iter().any(|&(sl, sc, ec)| {
                        sl == line_num && (col as u32) >= sc && (col as u32) < ec
                    }),
                };

                if should_render {
                    self.glyphs.push(WhitespaceGlyph {
                        line: line_num,
                        column: col as u32,
                        kind,
                    });
                }
            }
        }
    }

    /// Returns the glyphs for a specific line.
    #[must_use]
    pub fn glyphs_for_line(&self, line: u32) -> Vec<&WhitespaceGlyph> {
        self.glyphs.iter().filter(|g| g.line == line).collect()
    }

    /// Clears computed glyphs.
    pub fn clear(&mut self) {
        self.glyphs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_mode_shows_everything() {
        let mut state = WhitespaceRendererState::new(RenderWhitespace::All, 4);
        state.compute(&[(0, "  hello  ")], &[]);
        assert_eq!(state.glyphs.len(), 4); // 2 leading + 2 trailing
    }

    #[test]
    fn boundary_mode() {
        let mut state = WhitespaceRendererState::new(RenderWhitespace::Boundary, 4);
        state.compute(&[(0, "  hello world  ")], &[]);
        // 2 leading + 2 trailing = 4 boundary, the space between words is not boundary
        assert_eq!(state.glyphs.len(), 4);
    }

    #[test]
    fn trailing_mode() {
        let mut state = WhitespaceRendererState::new(RenderWhitespace::Trailing, 4);
        state.compute(&[(0, "  hello  ")], &[]);
        assert_eq!(state.glyphs.len(), 2); // only trailing
    }

    #[test]
    fn selection_mode() {
        let mut state = WhitespaceRendererState::new(RenderWhitespace::Selection, 4);
        state.compute(&[(0, "  hello  ")], &[(0, 0, 3)]);
        assert_eq!(state.glyphs.len(), 2); // 2 spaces in selection range 0..3
    }

    #[test]
    fn none_mode() {
        let mut state = WhitespaceRendererState::new(RenderWhitespace::None, 4);
        state.compute(&[(0, "  hello  ")], &[]);
        assert!(state.glyphs.is_empty());
    }

    #[test]
    fn tab_glyph() {
        let mut state = WhitespaceRendererState::new(RenderWhitespace::All, 4);
        state.compute(&[(0, "\thello")], &[]);
        assert_eq!(state.glyphs.len(), 1);
        assert_eq!(state.glyphs[0].kind, WhitespaceKind::Tab);
        assert_eq!(state.glyphs[0].kind.glyph(), '→');
    }

    #[test]
    fn glyph_chars() {
        assert_eq!(WhitespaceKind::Space.glyph(), '·');
        assert_eq!(WhitespaceKind::Tab.glyph(), '→');
        assert_eq!(WhitespaceKind::NonBreakingSpace.glyph(), '°');
    }
}
