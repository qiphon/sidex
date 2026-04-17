//! Bracket pair colorization and matching — cycling rainbow colors for nested
//! brackets and vertical guides between matched pairs, mirroring VS Code's
//! `editor.bracketPairColorization` feature.

/// Default cycling palette (VS Code defaults).
pub const BRACKET_COLORS: [&str; 6] =
    ["gold", "orchid", "cornflowerblue", "green", "red", "cyan"];

const DEFAULT_BRACKET_TYPES: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketPair {
    pub open: char,
    pub close: char,
    pub nesting_level: u32,
    pub color_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BracketGuide {
    pub start_line: u32,
    pub end_line: u32,
    pub column: u32,
    pub color_index: usize,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct BracketPairColorizer {
    pub enabled: bool,
    pub colors: Vec<String>,
    pub bracket_pairs: Vec<(char, char)>,
    pub independent_color_pool_per_bracket_type: bool,
}

impl Default for BracketPairColorizer {
    fn default() -> Self {
        Self {
            enabled: true,
            colors: BRACKET_COLORS.iter().map(|s| (*s).to_owned()).collect(),
            bracket_pairs: DEFAULT_BRACKET_TYPES.to_vec(),
            independent_color_pool_per_bracket_type: false,
        }
    }
}

impl BracketPairColorizer {
    pub fn from_settings(enabled: bool, _guides_enabled: bool) -> Self {
        Self { enabled, ..Default::default() }
    }

    pub fn colorize(&self, content: &str) -> Vec<BracketPair> {
        if !self.enabled {
            return Vec::new();
        }
        if self.independent_color_pool_per_bracket_type {
            self.colorize_independent(content)
        } else {
            compute_bracket_pairs(content, &self.bracket_pairs, self.colors.len())
        }
    }

    fn colorize_independent(&self, content: &str) -> Vec<BracketPair> {
        let palette = self.colors.len().max(1);
        let mut out = Vec::new();
        for &(open, close) in &self.bracket_pairs {
            let mut depth: u32 = 0;
            for ch in content.chars() {
                if ch == open {
                    out.push(BracketPair { open, close, nesting_level: depth, color_index: depth as usize % palette });
                    depth += 1;
                } else if ch == close && depth > 0 {
                    depth -= 1;
                    out.push(BracketPair { open, close, nesting_level: depth, color_index: depth as usize % palette });
                }
            }
        }
        out
    }

    pub fn guides(&self, pairs: &[BracketPair], cursor_line: u32) -> Vec<BracketGuide> {
        compute_bracket_guides(pairs, cursor_line)
    }
}

/// Scan `content` and return a [`BracketPair`] for every bracket character,
/// with nesting level and cycling color index.
pub fn compute_bracket_pairs(content: &str, bracket_types: &[(char, char)], palette_size: usize) -> Vec<BracketPair> {
    let palette = palette_size.max(1);
    let mut stack: Vec<(char, char)> = Vec::new();
    let mut result = Vec::new();
    for ch in content.chars() {
        if let Some(&(open, close)) = bracket_types.iter().find(|(o, _)| *o == ch) {
            let depth = stack.len() as u32;
            result.push(BracketPair { open, close, nesting_level: depth, color_index: depth as usize % palette });
            stack.push((open, close));
        } else if let Some(&(open, close)) = bracket_types.iter().find(|(_, c)| *c == ch) {
            if let Some(pos) = stack.iter().rposition(|&(o, _)| o == open) {
                let depth = pos as u32;
                stack.truncate(pos);
                result.push(BracketPair { open, close, nesting_level: depth, color_index: depth as usize % palette });
            }
        }
    }
    result
}

/// Produce vertical bracket guides; `is_active` is set when `cursor_line`
/// falls within the guide's vertical span.
pub fn compute_bracket_guides(pairs: &[BracketPair], cursor_line: u32) -> Vec<BracketGuide> {
    pairs.iter().enumerate().map(|(i, pair)| {
        let start = i as u32;
        let end = start + pair.nesting_level + 1;
        BracketGuide {
            start_line: start,
            end_line: end,
            column: pair.nesting_level,
            color_index: pair.color_index,
            is_active: cursor_line >= start && cursor_line <= end,
        }
    }).collect()
}

/// Find the position of the bracket matching the one at `position`.
/// Returns `Some((line, col))` or `None`.
pub fn find_matching_bracket(content: &str, position: (u32, u32)) -> Option<(u32, u32)> {
    let lines: Vec<&str> = content.lines().collect();
    let (tl, tc) = (position.0 as usize, position.1 as usize);
    let ch = lines.get(tl)?.chars().nth(tc)?;
    let types = DEFAULT_BRACKET_TYPES;
    if let Some(&(open, close)) = types.iter().find(|(o, _)| *o == ch) {
        scan(content, &lines, tl, tc, open, close, true)
    } else if let Some(&(open, close)) = types.iter().find(|(_, c)| *c == ch) {
        scan(content, &lines, tl, tc, open, close, false)
    } else {
        None
    }
}

fn scan(
    _content: &str, lines: &[&str], start_line: usize, start_col: usize,
    open: char, close: char, forward: bool,
) -> Option<(u32, u32)> {
    let mut depth: i32 = 0;
    if forward {
        for (li, &line) in lines.iter().enumerate().skip(start_line) {
            let cs = if li == start_line { start_col } else { 0 };
            for (ci, ch) in line.chars().enumerate().skip(cs) {
                if ch == open { depth += 1; }
                if ch == close { depth -= 1; if depth == 0 { return Some((li as u32, ci as u32)); } }
            }
        }
    } else {
        for li in (0..=start_line).rev() {
            let chars: Vec<char> = lines[li].chars().collect();
            let end = if li == start_line { start_col } else { chars.len().saturating_sub(1) };
            for ci in (0..=end).rev() {
                if ci >= chars.len() { continue; }
                if chars[ci] == close { depth += 1; }
                if chars[ci] == open { depth -= 1; if depth == 0 { return Some((li as u32, ci as u32)); } }
            }
        }
    }
    None
}
