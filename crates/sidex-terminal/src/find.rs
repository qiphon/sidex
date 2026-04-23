//! Terminal find/search functionality (Ctrl+F in terminal).
//!
//! Searches the visible grid and scrollback buffer with support for
//! case-insensitive, regex, and whole-word matching.

use crate::grid::{Cell, TerminalGrid};
use regex::Regex;

/// A match location in the terminal grid.
/// Negative row values refer to scrollback lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalMatch {
    pub start_row: i32,
    pub start_col: u16,
    pub end_row: i32,
    pub end_col: u16,
}

#[derive(Debug, Clone, Default)]
pub struct FindOptions {
    pub case_sensitive: bool,
    pub regex: bool,
    pub whole_word: bool,
}

/// Persistent state for the terminal find widget.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct TerminalFind {
    pub visible: bool,
    pub query: String,
    pub matches: Vec<TerminalMatch>,
    pub current_match: usize,
    pub case_sensitive: bool,
    pub regex: bool,
    pub whole_word: bool,
}

impl TerminalFind {
    pub fn open(&mut self) {
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.matches.clear();
        self.current_match = 0;
    }

    pub fn options(&self) -> FindOptions {
        FindOptions {
            case_sensitive: self.case_sensitive,
            regex: self.regex,
            whole_word: self.whole_word,
        }
    }

    /// Run the search against `grid`, replacing previous results.
    pub fn search(&mut self, grid: &TerminalGrid) {
        self.matches = find_in_terminal(grid, &self.query, &self.options());
        if self.matches.is_empty() {
            self.current_match = 0;
        } else {
            self.current_match = self.current_match.min(self.matches.len() - 1);
        }
    }

    /// Navigate to the next match (Enter).
    pub fn next_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = (self.current_match + 1) % self.matches.len();
        }
    }

    /// Navigate to the previous match (Shift+Enter).
    pub fn previous_match(&mut self) {
        if !self.matches.is_empty() {
            self.current_match = self
                .current_match
                .checked_sub(1)
                .unwrap_or(self.matches.len() - 1);
        }
    }

    /// Format the match counter, e.g. "3 of 12".
    pub fn match_status(&self) -> String {
        if self.matches.is_empty() {
            "No results".into()
        } else {
            format!("{} of {}", self.current_match + 1, self.matches.len())
        }
    }

    /// Returns `true` if the given cell falls inside any match.
    pub fn is_highlighted(&self, row: i32, col: u16) -> bool {
        self.matches.iter().any(|m| {
            if m.start_row == m.end_row {
                row == m.start_row && col >= m.start_col && col <= m.end_col
            } else if row == m.start_row {
                col >= m.start_col
            } else if row == m.end_row {
                col <= m.end_col
            } else {
                row > m.start_row && row < m.end_row
            }
        })
    }

    pub fn current(&self) -> Option<&TerminalMatch> {
        self.matches.get(self.current_match)
    }
}

// ---------------------------------------------------------------------------
// Core search
// ---------------------------------------------------------------------------

fn line_text(cells: &[Cell]) -> String {
    cells.iter().filter(|c| c.width != 0).map(|c| c.c).collect()
}

/// Search the terminal grid (scrollback + visible) for `query`.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
pub fn find_in_terminal(
    grid: &TerminalGrid,
    query: &str,
    options: &FindOptions,
) -> Vec<TerminalMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let Some(re) = build_pattern(query, options) else {
        return Vec::new();
    };
    let sb_len = grid.scrollback.len();
    let mut results = Vec::new();
    for i in 0..sb_len {
        if let Some(cells) = grid.scrollback.get(i) {
            collect(
                &re,
                &line_text(cells),
                -(sb_len as i32) + i as i32,
                &mut results,
            );
        }
    }
    for row in 0..grid.rows() {
        collect(
            &re,
            &line_text(&grid.cells()[row as usize]),
            i32::from(row),
            &mut results,
        );
    }
    results
}

fn build_pattern(query: &str, opts: &FindOptions) -> Option<Regex> {
    let pat = if opts.regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    let pat = if opts.whole_word {
        format!(r"\b{pat}\b")
    } else {
        pat
    };
    let pat = if opts.case_sensitive {
        pat
    } else {
        format!("(?i){pat}")
    };
    Regex::new(&pat).ok()
}

fn collect(re: &Regex, text: &str, row: i32, out: &mut Vec<TerminalMatch>) {
    for m in re.find_iter(text) {
        #[allow(clippy::cast_possible_truncation)]
        out.push(TerminalMatch {
            start_row: row,
            start_col: m.start() as u16,
            end_row: row,
            end_col: m.end().saturating_sub(1) as u16,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Cell, TerminalGrid};

    fn grid_with(lines: &[&str]) -> TerminalGrid {
        let cols = lines.iter().map(|l| l.len()).max().unwrap_or(20).max(20);
        let mut g = TerminalGrid::new(lines.len().max(4) as u16, cols as u16);
        let t = Cell::default();
        for (i, l) in lines.iter().enumerate() {
            g.set_cursor(i as u16, 0);
            l.chars().for_each(|ch| g.write_char(ch, &t));
        }
        g
    }

    #[test]
    fn basic_and_case() {
        let g = grid_with(&["hello world", "hello rust"]);
        let m = find_in_terminal(&g, "hello", &FindOptions::default());
        assert_eq!(m.len(), 2);
        assert_eq!((m[0].start_col, m[0].end_col), (0, 4));
        let g2 = grid_with(&["Hello HELLO hello"]);
        assert_eq!(
            find_in_terminal(&g2, "hello", &FindOptions::default()).len(),
            3
        );
        let cs = FindOptions {
            case_sensitive: true,
            ..Default::default()
        };
        assert_eq!(find_in_terminal(&g2, "hello", &cs).len(), 1);
    }

    #[test]
    fn whole_word_and_counter() {
        let g = grid_with(&["cat catfish concatenate"]);
        let ww = FindOptions {
            whole_word: true,
            ..Default::default()
        };
        assert_eq!(find_in_terminal(&g, "cat", &ww).len(), 1);
        let mut f = TerminalFind::default();
        assert_eq!(f.match_status(), "No results");
        f.matches.push(TerminalMatch {
            start_row: 0,
            start_col: 0,
            end_row: 0,
            end_col: 4,
        });
        f.matches.push(TerminalMatch {
            start_row: 1,
            start_col: 0,
            end_row: 1,
            end_col: 4,
        });
        assert_eq!(f.match_status(), "1 of 2");
        f.next_match();
        assert_eq!(f.match_status(), "2 of 2");
        f.next_match();
        assert_eq!(f.match_status(), "1 of 2");
    }
}
