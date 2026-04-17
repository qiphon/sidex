//! Scroll bar decorations / overview ruler — coloured marks in the scrollbar
//! track showing locations of errors, warnings, search results, selections,
//! git changes, bookmarks, and folded regions.

use crate::decoration::Color;

/// The semantic kind of a scroll mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScrollMarkKind {
    Error,
    Warning,
    Info,
    SearchResult,
    Selection,
    GitAdded,
    GitModified,
    GitDeleted,
    Cursor,
    Bookmark,
    FoldedRegion,
}

impl ScrollMarkKind {
    #[must_use]
    pub fn default_color(self) -> Color {
        match self {
            Self::Error => Color::new(0.957, 0.278, 0.278, 1.0),
            Self::Warning => Color::new(0.804, 0.678, 0.0, 1.0),
            Self::Info => Color::new(0.216, 0.580, 1.0, 1.0),
            Self::SearchResult => Color::new(0.91, 0.58, 0.14, 1.0),
            Self::Selection => Color::new(0.216, 0.580, 1.0, 0.5),
            Self::GitAdded => Color::new(0.2, 0.78, 0.35, 1.0),
            Self::GitModified => Color::new(0.216, 0.580, 1.0, 1.0),
            Self::GitDeleted => Color::new(0.957, 0.278, 0.278, 0.8),
            Self::Cursor => Color::new(0.8, 0.8, 0.8, 1.0),
            Self::Bookmark => Color::new(0.35, 0.55, 0.95, 1.0),
            Self::FoldedRegion => Color::new(0.5, 0.5, 0.5, 0.4),
        }
    }

    /// Priority for rendering order — higher priority marks render on top.
    #[must_use]
    pub fn priority(self) -> u8 {
        match self {
            Self::FoldedRegion => 0,
            Self::GitAdded | Self::GitModified | Self::GitDeleted => 1,
            Self::Info => 2,
            Self::Selection => 3,
            Self::Bookmark => 4,
            Self::SearchResult => 5,
            Self::Warning => 6,
            Self::Error => 7,
            Self::Cursor => 8,
        }
    }
}

/// A single coloured mark in the scrollbar track.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollMark {
    pub line: u32,
    pub kind: ScrollMarkKind,
    pub color: Color,
}

/// Rendered rectangle for a mark in the overview ruler.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollMarkRect {
    pub y_offset: f32,
    pub height: f32,
    pub color: Color,
    pub kind: ScrollMarkKind,
}

/// Full state for the scroll-bar decoration / overview ruler feature.
#[derive(Debug, Clone, Default)]
pub struct ScrollDecorations {
    pub marks: Vec<ScrollMark>,
    pub total_lines: u32,
}

impl ScrollDecorations {
    pub fn new(total_lines: u32) -> Self {
        Self {
            marks: Vec::new(),
            total_lines,
        }
    }

    pub fn set_marks(&mut self, marks: Vec<ScrollMark>) {
        self.marks = marks;
    }

    pub fn clear(&mut self) {
        self.marks.clear();
    }

    /// Appends marks to the existing set without clearing.
    pub fn add_marks(&mut self, marks: impl IntoIterator<Item = ScrollMark>) {
        self.marks.extend(marks);
    }

    /// Projects marks onto a scrollbar track of the given pixel height.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn compute_rects(&self, track_height: f32, min_mark_height: f32) -> Vec<ScrollMarkRect> {
        if self.total_lines == 0 || track_height <= 0.0 {
            return Vec::new();
        }
        let line_height = track_height / self.total_lines as f32;
        let mark_h = line_height.max(min_mark_height);

        let mut rects: Vec<ScrollMarkRect> = self
            .marks
            .iter()
            .filter(|m| m.line < self.total_lines)
            .map(|m| ScrollMarkRect {
                y_offset: m.line as f32 * line_height,
                height: mark_h,
                color: m.color,
                kind: m.kind,
            })
            .collect();

        rects.sort_by(|a, b| {
            a.kind
                .priority()
                .cmp(&b.kind.priority())
                .then_with(|| a.y_offset.partial_cmp(&b.y_offset).unwrap_or(std::cmp::Ordering::Equal))
        });

        rects
    }

    /// Returns the document line corresponding to a click at `y` on a
    /// scrollbar track of `track_height` pixels.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn click_to_line(&self, y: f32, track_height: f32) -> u32 {
        if track_height <= 0.0 || self.total_lines == 0 {
            return 0;
        }
        let y = y.clamp(0.0, track_height);
        let ratio = y / track_height;
        let line = (ratio * self.total_lines as f32).floor() as u32;
        line.min(self.total_lines.saturating_sub(1))
    }
}

// ── Builder helpers ─────────────────────────────────────────────────────────

/// Diagnostic input for `compute_scroll_marks`.
#[derive(Debug, Clone)]
pub struct DiagnosticInput {
    pub line: u32,
    pub is_error: bool,
    pub is_warning: bool,
}

/// A line-range change from git diff.
#[derive(Debug, Clone)]
pub struct LineChangeInput {
    pub line: u32,
    pub kind: GitChangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitChangeKind {
    Added,
    Modified,
    Deleted,
}

/// A fold range input.
#[derive(Debug, Clone)]
pub struct FoldRangeInput {
    pub start_line: u32,
    pub end_line: u32,
}

/// Aggregates marks from multiple data sources into a single `Vec<ScrollMark>`.
#[must_use]
pub fn compute_scroll_marks(
    diagnostics: &[DiagnosticInput],
    search_result_lines: &[u32],
    selection_lines: &[u32],
    git_changes: &[LineChangeInput],
    bookmarks: &[u32],
    folds: &[FoldRangeInput],
    cursor_lines: &[u32],
    total_lines: u32,
) -> Vec<ScrollMark> {
    let mut marks = Vec::new();

    for d in diagnostics {
        if d.line >= total_lines {
            continue;
        }
        let kind = if d.is_error {
            ScrollMarkKind::Error
        } else if d.is_warning {
            ScrollMarkKind::Warning
        } else {
            ScrollMarkKind::Info
        };
        marks.push(ScrollMark {
            line: d.line,
            kind,
            color: kind.default_color(),
        });
    }

    for &line in search_result_lines {
        if line < total_lines {
            marks.push(ScrollMark {
                line,
                kind: ScrollMarkKind::SearchResult,
                color: ScrollMarkKind::SearchResult.default_color(),
            });
        }
    }

    for &line in selection_lines {
        if line < total_lines {
            marks.push(ScrollMark {
                line,
                kind: ScrollMarkKind::Selection,
                color: ScrollMarkKind::Selection.default_color(),
            });
        }
    }

    for gc in git_changes {
        if gc.line >= total_lines {
            continue;
        }
        let kind = match gc.kind {
            GitChangeKind::Added => ScrollMarkKind::GitAdded,
            GitChangeKind::Modified => ScrollMarkKind::GitModified,
            GitChangeKind::Deleted => ScrollMarkKind::GitDeleted,
        };
        marks.push(ScrollMark {
            line: gc.line,
            kind,
            color: kind.default_color(),
        });
    }

    for &line in bookmarks {
        if line < total_lines {
            marks.push(ScrollMark {
                line,
                kind: ScrollMarkKind::Bookmark,
                color: ScrollMarkKind::Bookmark.default_color(),
            });
        }
    }

    for fold in folds {
        if fold.start_line < total_lines {
            marks.push(ScrollMark {
                line: fold.start_line,
                kind: ScrollMarkKind::FoldedRegion,
                color: ScrollMarkKind::FoldedRegion.default_color(),
            });
        }
    }

    for &line in cursor_lines {
        if line < total_lines {
            marks.push(ScrollMark {
                line,
                kind: ScrollMarkKind::Cursor,
                color: ScrollMarkKind::Cursor.default_color(),
            });
        }
    }

    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_marks() {
        let sd = ScrollDecorations::new(100);
        let rects = sd.compute_rects(200.0, 2.0);
        assert!(rects.is_empty());
    }

    #[test]
    fn basic_rects() {
        let mut sd = ScrollDecorations::new(100);
        sd.set_marks(vec![
            ScrollMark {
                line: 0,
                kind: ScrollMarkKind::Error,
                color: ScrollMarkKind::Error.default_color(),
            },
            ScrollMark {
                line: 50,
                kind: ScrollMarkKind::Warning,
                color: ScrollMarkKind::Warning.default_color(),
            },
        ]);
        let rects = sd.compute_rects(200.0, 2.0);
        assert_eq!(rects.len(), 2);
    }

    #[test]
    fn click_to_line_basic() {
        let sd = ScrollDecorations::new(100);
        assert_eq!(sd.click_to_line(100.0, 200.0), 50);
        assert_eq!(sd.click_to_line(0.0, 200.0), 0);
        assert_eq!(sd.click_to_line(200.0, 200.0), 99);
    }

    #[test]
    fn click_to_line_edge_cases() {
        let sd = ScrollDecorations::new(0);
        assert_eq!(sd.click_to_line(50.0, 200.0), 0);

        let sd2 = ScrollDecorations::new(100);
        assert_eq!(sd2.click_to_line(50.0, 0.0), 0);
    }

    #[test]
    fn compute_scroll_marks_aggregation() {
        let marks = compute_scroll_marks(
            &[DiagnosticInput {
                line: 5,
                is_error: true,
                is_warning: false,
            }],
            &[10, 20],
            &[15],
            &[LineChangeInput {
                line: 30,
                kind: GitChangeKind::Added,
            }],
            &[40],
            &[FoldRangeInput {
                start_line: 50,
                end_line: 60,
            }],
            &[7],
            100,
        );
        assert_eq!(marks.len(), 8);
    }

    #[test]
    fn marks_skip_out_of_range() {
        let marks = compute_scroll_marks(
            &[DiagnosticInput {
                line: 200,
                is_error: true,
                is_warning: false,
            }],
            &[300],
            &[],
            &[],
            &[],
            &[],
            &[],
            100,
        );
        assert!(marks.is_empty());
    }

    #[test]
    fn priority_ordering() {
        assert!(ScrollMarkKind::Error.priority() > ScrollMarkKind::Warning.priority());
        assert!(ScrollMarkKind::Cursor.priority() > ScrollMarkKind::Error.priority());
        assert!(ScrollMarkKind::FoldedRegion.priority() < ScrollMarkKind::GitAdded.priority());
    }

    #[test]
    fn add_marks_appends() {
        let mut sd = ScrollDecorations::new(100);
        sd.set_marks(vec![ScrollMark {
            line: 0,
            kind: ScrollMarkKind::Error,
            color: ScrollMarkKind::Error.default_color(),
        }]);
        sd.add_marks(vec![ScrollMark {
            line: 50,
            kind: ScrollMarkKind::Bookmark,
            color: ScrollMarkKind::Bookmark.default_color(),
        }]);
        assert_eq!(sd.marks.len(), 2);
    }
}
