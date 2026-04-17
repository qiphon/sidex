//! Bookmark management — toggle, navigate, and persist bookmarks.
//!
//! Each bookmark records a file path, a line number, and an optional label.
//! The gutter renders bookmarks as blue diamond icons.  Bookmarks are
//! persisted through the database layer so they survive editor restarts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── Bookmark ────────────────────────────────────────────────────────────────

/// A single bookmark in a source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    pub line: u32,
    pub label: Option<String>,
    pub file: PathBuf,
}

// ── Gutter visual ───────────────────────────────────────────────────────────

/// Gutter icon style for a bookmark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookmarkGutterIcon {
    /// Blue diamond — the default bookmark icon.
    BlueDiamond,
    /// Labeled bookmark — displayed with a small badge.
    Labeled,
}

/// RGBA color for the blue bookmark gutter icon (`#3794ff`, ~56% opacity).
pub const BOOKMARK_GUTTER_COLOR: [f32; 4] = [0.216, 0.580, 1.0, 0.56];

// ── BookmarkManager ─────────────────────────────────────────────────────────

/// Manages bookmarks across all open files.
#[derive(Clone, Debug, Default)]
pub struct BookmarkManager {
    /// Bookmarks grouped by file path.
    per_file: HashMap<PathBuf, Vec<Bookmark>>,
    /// Global cursor used by `next_bookmark` / `prev_bookmark`.
    cursor: Option<(PathBuf, usize)>,
    /// When `true`, the in-memory state has unsaved changes.
    dirty: bool,
}

impl BookmarkManager {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Toggle ──────────────────────────────────────────────────────────

    /// Toggles a bookmark at `(file, line)`.
    ///
    /// Returns `true` if a bookmark was *added*, `false` if one was removed.
    pub fn toggle_bookmark(&mut self, file: &Path, line: u32) -> bool {
        let bookmarks = self.per_file.entry(file.to_path_buf()).or_default();
        if let Some(idx) = bookmarks.iter().position(|b| b.line == line) {
            bookmarks.remove(idx);
            self.dirty = true;
            false
        } else {
            bookmarks.push(Bookmark {
                line,
                label: None,
                file: file.to_path_buf(),
            });
            bookmarks.sort_by_key(|b| b.line);
            self.dirty = true;
            true
        }
    }

    /// Toggles a labeled bookmark.
    pub fn toggle_labeled(&mut self, file: &Path, line: u32, label: String) -> bool {
        let bookmarks = self.per_file.entry(file.to_path_buf()).or_default();
        if let Some(idx) = bookmarks.iter().position(|b| b.line == line) {
            bookmarks.remove(idx);
            self.dirty = true;
            false
        } else {
            bookmarks.push(Bookmark {
                line,
                label: Some(label),
                file: file.to_path_buf(),
            });
            bookmarks.sort_by_key(|b| b.line);
            self.dirty = true;
            true
        }
    }

    // ── Navigation ──────────────────────────────────────────────────────

    /// Returns the next bookmark in the flattened, sorted list, wrapping around.
    pub fn next_bookmark(&mut self) -> Option<&Bookmark> {
        let all = self.sorted_all();
        if all.is_empty() {
            return None;
        }
        let idx = match &self.cursor {
            Some((file, line_idx)) => {
                all.iter()
                    .position(|b| &b.file == file && b.line as usize > *line_idx)
                    .unwrap_or(0)
            }
            None => 0,
        };
        let bm = &all[idx];
        self.cursor = Some((bm.file.clone(), bm.line as usize));
        // Borrow from self.per_file so the reference lives long enough.
        self.per_file
            .get(&all[idx].file)
            .and_then(|bs| bs.iter().find(|b| b.line == all[idx].line))
    }

    /// Returns the previous bookmark, wrapping around.
    pub fn prev_bookmark(&mut self) -> Option<&Bookmark> {
        let all = self.sorted_all();
        if all.is_empty() {
            return None;
        }
        let idx = match &self.cursor {
            Some((file, line_idx)) => {
                all.iter()
                    .rposition(|b| &b.file == file && (b.line as usize) < *line_idx)
                    .unwrap_or(all.len() - 1)
            }
            None => all.len() - 1,
        };
        let bm = &all[idx];
        self.cursor = Some((bm.file.clone(), bm.line as usize));
        self.per_file
            .get(&all[idx].file)
            .and_then(|bs| bs.iter().find(|b| b.line == all[idx].line))
    }

    // ── Queries ─────────────────────────────────────────────────────────

    /// All bookmarks across every file, sorted by (file, line).
    #[must_use]
    pub fn list_bookmarks(&self) -> Vec<Bookmark> {
        self.sorted_all()
    }

    /// Bookmarks in a single file, sorted by line.
    #[must_use]
    pub fn bookmarks_for_file(&self, file: &Path) -> Vec<&Bookmark> {
        self.per_file
            .get(file)
            .map(|bs| bs.iter().collect())
            .unwrap_or_default()
    }

    /// Whether the given line has a bookmark.
    #[must_use]
    pub fn has_bookmark(&self, file: &Path, line: u32) -> bool {
        self.per_file
            .get(file)
            .map_or(false, |bs| bs.iter().any(|b| b.line == line))
    }

    /// Returns the gutter icon kind for a line, if bookmarked.
    #[must_use]
    pub fn gutter_icon(&self, file: &Path, line: u32) -> Option<BookmarkGutterIcon> {
        self.per_file.get(file).and_then(|bs| {
            bs.iter().find(|b| b.line == line).map(|b| {
                if b.label.is_some() {
                    BookmarkGutterIcon::Labeled
                } else {
                    BookmarkGutterIcon::BlueDiamond
                }
            })
        })
    }

    // ── Mutation ─────────────────────────────────────────────────────────

    /// Removes all bookmarks in the given file.
    pub fn clear_bookmarks(&mut self, file: &Path) {
        if self.per_file.remove(file).is_some() {
            self.dirty = true;
        }
    }

    /// Removes every bookmark across all files.
    pub fn clear_all(&mut self) {
        self.per_file.clear();
        self.cursor = None;
        self.dirty = true;
    }

    /// Whether the manager has unsaved changes.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the state as persisted (call after writing to the database).
    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Bulk-load bookmarks (e.g. from the database on startup).
    pub fn load(&mut self, bookmarks: Vec<Bookmark>) {
        self.per_file.clear();
        for bm in bookmarks {
            self.per_file
                .entry(bm.file.clone())
                .or_default()
                .push(bm);
        }
        for bs in self.per_file.values_mut() {
            bs.sort_by_key(|b| b.line);
        }
        self.dirty = false;
    }

    // ── Internal ────────────────────────────────────────────────────────

    fn sorted_all(&self) -> Vec<Bookmark> {
        let mut all: Vec<Bookmark> = self
            .per_file
            .values()
            .flat_map(|bs| bs.iter().cloned())
            .collect();
        all.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
        all
    }
}
