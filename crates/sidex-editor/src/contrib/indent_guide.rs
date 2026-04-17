//! Indentation guide lines — mirrors VS Code's indent-guides view part.
//!
//! Computes the vertical indentation guide lines to render in the editor
//! gutter/text area, with support for active-level highlighting, rainbow
//! colour mode, and bracket-pair guide lines.

use crate::decoration::Color;
use sidex_text::Buffer;

// ── Configuration ───────────────────────────────────────────────────────────

/// Configuration for indent guides.
#[derive(Debug, Clone)]
pub struct IndentGuidesConfig {
    pub enabled: bool,
    pub highlight_active: bool,
    pub rainbow: bool,
    pub bracket_pair_guides: bool,
}

impl Default for IndentGuidesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            highlight_active: true,
            rainbow: false,
            bracket_pair_guides: false,
        }
    }
}

/// Palette for rainbow indent guides.
const RAINBOW_PALETTE: &[Color] = &[
    Color::new(1.0, 0.84, 0.0, 0.4),   // gold
    Color::new(0.55, 0.27, 0.94, 0.4),  // violet
    Color::new(0.0, 0.8, 0.6, 0.4),     // teal
    Color::new(0.93, 0.35, 0.35, 0.4),  // red
    Color::new(0.22, 0.6, 0.95, 0.4),   // blue
    Color::new(0.95, 0.6, 0.07, 0.4),   // orange
];

// ── Indent guide ────────────────────────────────────────────────────────────

/// A single indentation guide line.
#[derive(Debug, Clone, PartialEq)]
pub struct IndentGuide {
    pub line: u32,
    pub column: u32,
    pub is_active: bool,
    pub level: u32,
    pub color: Option<Color>,
}

// Eq is valid: Color fields are finite f32s produced by constants.
impl Eq for IndentGuide {}

/// Computes indent guides for the visible viewport.
///
/// `active_line` is the cursor line; guides at the cursor's indent scope are
/// marked as active.
#[must_use]
pub fn compute_indent_guides(
    buffer: &Buffer,
    first_line: u32,
    last_line: u32,
    tab_size: u32,
    active_line: Option<u32>,
) -> Vec<IndentGuide> {
    compute_indent_guides_with_config(
        buffer,
        first_line,
        last_line,
        tab_size,
        active_line,
        &IndentGuidesConfig::default(),
    )
}

/// Computes indent guides with full configuration support.
#[must_use]
pub fn compute_indent_guides_with_config(
    buffer: &Buffer,
    first_line: u32,
    last_line: u32,
    tab_size: u32,
    active_line: Option<u32>,
    config: &IndentGuidesConfig,
) -> Vec<IndentGuide> {
    if !config.enabled {
        return Vec::new();
    }

    let mut guides = Vec::new();
    let line_count = buffer.len_lines() as u32;

    let active_indent_col = active_line.map(|l| {
        if (l as usize) < buffer.len_lines() {
            let content = buffer.line_content(l as usize);
            visible_indent(&content, tab_size)
        } else {
            0
        }
    });

    for line_idx in first_line..=last_line.min(line_count.saturating_sub(1)) {
        let content = buffer.line_content(line_idx as usize);
        let indent = visible_indent(&content, tab_size);

        let num_guides = indent / tab_size;
        for g in 0..num_guides {
            let col = g * tab_size;
            let level = g + 1;
            let is_active = config.highlight_active
                && active_indent_col.is_some_and(|ac| col < ac && active_line.is_some());

            let color = if config.rainbow {
                let idx = (level as usize).saturating_sub(1) % RAINBOW_PALETTE.len();
                Some(RAINBOW_PALETTE[idx])
            } else {
                None
            };

            guides.push(IndentGuide {
                line: line_idx,
                column: col,
                is_active,
                level,
                color,
            });
        }
    }

    guides
}

// ── Bracket pair guides ─────────────────────────────────────────────────────

/// A guide line connecting a bracket pair.
#[derive(Debug, Clone, PartialEq)]
pub struct BracketPairGuide {
    pub start_line: u32,
    pub end_line: u32,
    pub column: u32,
    pub color: Color,
    pub is_active: bool,
}

impl Eq for BracketPairGuide {}

/// A bracket pair for guide computation.
#[derive(Debug, Clone)]
pub struct BracketPair {
    pub open_line: u32,
    pub open_column: u32,
    pub close_line: u32,
    pub close_column: u32,
}

/// Computes vertical bracket-pair guide lines from a set of matched bracket pairs.
#[must_use]
pub fn compute_bracket_pair_guides(
    pairs: &[BracketPair],
    active_line: Option<u32>,
) -> Vec<BracketPairGuide> {
    let mut guides = Vec::new();

    for (i, pair) in pairs.iter().enumerate() {
        if pair.close_line <= pair.open_line {
            continue;
        }

        let column = pair.open_column;
        let palette_idx = i % RAINBOW_PALETTE.len();
        let color = RAINBOW_PALETTE[palette_idx];

        let is_active = active_line
            .is_some_and(|l| l >= pair.open_line && l <= pair.close_line);

        guides.push(BracketPairGuide {
            start_line: pair.open_line,
            end_line: pair.close_line,
            column,
            color,
            is_active,
        });
    }

    guides
}

/// Returns the visible indentation width of a line, expanding tabs.
fn visible_indent(line: &str, tab_size: u32) -> u32 {
    let mut indent = 0u32;
    for ch in line.chars() {
        match ch {
            ' ' => indent += 1,
            '\t' => indent += tab_size - (indent % tab_size),
            _ => break,
        }
    }
    indent
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn basic_indent_guides() {
        let buffer = buf("fn main() {\n    let x = 1;\n}");
        let guides = compute_indent_guides(&buffer, 0, 2, 4, None);
        let line1_guides: Vec<_> = guides.iter().filter(|g| g.line == 1).collect();
        assert_eq!(line1_guides.len(), 1);
        assert_eq!(line1_guides[0].column, 0);
    }

    #[test]
    fn nested_indent() {
        let buffer = buf("a\n    b\n        c\n    d\ne");
        let guides = compute_indent_guides(&buffer, 0, 4, 4, None);
        let line2_guides: Vec<_> = guides.iter().filter(|g| g.line == 2).collect();
        assert_eq!(line2_guides.len(), 2);
    }

    #[test]
    fn active_guide_highlighting() {
        let buffer = buf("a\n    b\n        c\n    d\ne");
        let config = IndentGuidesConfig {
            highlight_active: true,
            ..Default::default()
        };
        let guides =
            compute_indent_guides_with_config(&buffer, 0, 4, 4, Some(2), &config);
        let active: Vec<_> = guides.iter().filter(|g| g.is_active).collect();
        assert!(!active.is_empty());
    }

    #[test]
    fn rainbow_guides() {
        let buffer = buf("a\n    b\n        c");
        let config = IndentGuidesConfig {
            rainbow: true,
            ..Default::default()
        };
        let guides =
            compute_indent_guides_with_config(&buffer, 0, 2, 4, None, &config);
        let line2_guides: Vec<_> = guides.iter().filter(|g| g.line == 2).collect();
        assert!(line2_guides[0].color.is_some());
        assert!(line2_guides[1].color.is_some());
        assert_ne!(line2_guides[0].color, line2_guides[1].color);
    }

    #[test]
    fn disabled_returns_empty() {
        let buffer = buf("    a\n        b");
        let config = IndentGuidesConfig {
            enabled: false,
            ..Default::default()
        };
        let guides =
            compute_indent_guides_with_config(&buffer, 0, 1, 4, None, &config);
        assert!(guides.is_empty());
    }

    #[test]
    fn bracket_pair_guides_basic() {
        let pairs = vec![BracketPair {
            open_line: 0,
            open_column: 4,
            close_line: 5,
            close_column: 4,
        }];
        let guides = compute_bracket_pair_guides(&pairs, Some(3));
        assert_eq!(guides.len(), 1);
        assert!(guides[0].is_active);
        assert_eq!(guides[0].column, 4);
    }

    #[test]
    fn bracket_pair_guides_inactive() {
        let pairs = vec![BracketPair {
            open_line: 10,
            open_column: 0,
            close_line: 20,
            close_column: 0,
        }];
        let guides = compute_bracket_pair_guides(&pairs, Some(5));
        assert_eq!(guides.len(), 1);
        assert!(!guides[0].is_active);
    }

    #[test]
    fn bracket_pair_single_line_skipped() {
        let pairs = vec![BracketPair {
            open_line: 5,
            open_column: 0,
            close_line: 5,
            close_column: 10,
        }];
        let guides = compute_bracket_pair_guides(&pairs, None);
        assert!(guides.is_empty());
    }
}
