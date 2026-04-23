//! File decoration service for the explorer — badges, colors, and visual
//! states from git status, diagnostics, and gitignore, with folder propagation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sidex_theme::color::Color;

use crate::file_tree::{FileDecoration, FileTree};

// ── Input types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    Modified,
    Added,
    Deleted,
    Untracked,
    Conflicted,
    Renamed,
    Ignored,
}

#[derive(Debug, Clone)]
pub struct DiagnosticSummary {
    pub errors: usize,
    pub warnings: usize,
}

// ── Provider ────────────────────────────────────────────────────────────────

pub trait DecorationProvider: Send + Sync {
    fn id(&self) -> &str;
    fn provide(&self) -> HashMap<PathBuf, FileDecoration>;
}

// ── Service ─────────────────────────────────────────────────────────────────

/// Central service that aggregates file decorations from multiple providers.
#[derive(Default)]
pub struct FileDecorationService {
    providers: Vec<Box<dyn DecorationProvider>>,
    cache: HashMap<PathBuf, FileDecoration>,
}

impl FileDecorationService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, provider: Box<dyn DecorationProvider>) {
        self.providers.push(provider);
    }

    pub fn refresh(&mut self) {
        self.cache.clear();
        for provider in &self.providers {
            for (path, dec) in provider.provide() {
                self.cache
                    .entry(path)
                    .and_modify(|existing| merge_decoration(existing, &dec))
                    .or_insert(dec);
            }
        }
    }

    pub fn get_decoration(&self, path: &Path) -> Option<&FileDecoration> {
        self.cache.get(path)
    }

    pub fn all(&self) -> &HashMap<PathBuf, FileDecoration> {
        &self.cache
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

const BLUE: Color = Color {
    r: 30,
    g: 136,
    b: 229,
    a: 255,
};
const GREEN: Color = Color {
    r: 76,
    g: 175,
    b: 80,
    a: 255,
};
const RED: Color = Color {
    r: 244,
    g: 67,
    b: 54,
    a: 255,
};
const YELLOW: Color = Color {
    r: 255,
    g: 193,
    b: 7,
    a: 255,
};

#[derive(Debug, Clone)]
pub struct GitStatusInput {
    pub path: PathBuf,
    pub status: GitFileStatus,
}

fn git_dec(badge: &str, color: Color, tooltip: &str, strikethrough: bool) -> FileDecoration {
    FileDecoration {
        badge: Some(badge.into()),
        badge_color: Some(color),
        tooltip: Some(tooltip.into()),
        color: Some(color),
        strikethrough,
        ..Default::default()
    }
}

pub fn compute_git_decorations(statuses: &[GitStatusInput]) -> HashMap<PathBuf, FileDecoration> {
    let mut map = HashMap::with_capacity(statuses.len());
    for entry in statuses {
        let dec = match entry.status {
            GitFileStatus::Modified => git_dec("M", BLUE, "Modified", false),
            GitFileStatus::Untracked => git_dec("U", GREEN, "Untracked", false),
            GitFileStatus::Deleted => git_dec("D", RED, "Deleted", true),
            GitFileStatus::Conflicted => git_dec("C", YELLOW, "Conflict", false),
            GitFileStatus::Added => git_dec("A", GREEN, "Added", false),
            GitFileStatus::Renamed => git_dec("R", BLUE, "Renamed", false),
            GitFileStatus::Ignored => FileDecoration {
                faded: true,
                tooltip: Some("Ignored".into()),
                ..Default::default()
            },
        };
        map.insert(entry.path.clone(), dec);
    }
    map
}

pub fn compute_diagnostic_decorations<S: ::std::hash::BuildHasher>(
    diagnostics: &HashMap<PathBuf, DiagnosticSummary, S>,
) -> HashMap<PathBuf, FileDecoration> {
    let mut map = HashMap::with_capacity(diagnostics.len());
    for (path, summary) in diagnostics {
        let (count, color, label) = if summary.errors > 0 {
            (summary.errors, RED, "error")
        } else if summary.warnings > 0 {
            (summary.warnings, YELLOW, "warning")
        } else {
            continue;
        };
        map.insert(
            path.clone(),
            FileDecoration {
                badge: Some(count.to_string()),
                badge_color: Some(color),
                tooltip: Some(format!("{count} {label}(s)")),
                ..Default::default()
            },
        );
    }
    map
}

/// Propagate child decorations up to ancestor folders. Folder badges show
/// the aggregate count of decorated children. Faded-only entries are skipped.
pub fn propagate_decorations<S: ::std::hash::BuildHasher>(
    tree: &FileTree,
    decorations: &HashMap<PathBuf, FileDecoration, S>,
) -> HashMap<PathBuf, FileDecoration> {
    let mut folder_counts: HashMap<PathBuf, usize> = HashMap::new();
    let mut worst_color: HashMap<PathBuf, Color> = HashMap::new();

    for (path, dec) in decorations {
        if dec.faded && dec.badge.is_none() {
            continue;
        }
        let mut ancestor = path.parent();
        while let Some(dir) = ancestor {
            if dir < tree.root.path.as_path() {
                break;
            }
            *folder_counts.entry(dir.to_path_buf()).or_default() += 1;
            if let Some(c) = dec.badge_color {
                worst_color
                    .entry(dir.to_path_buf())
                    .and_modify(|e| {
                        if color_priority(c) > color_priority(*e) {
                            *e = c;
                        }
                    })
                    .or_insert(c);
            }
            ancestor = dir.parent();
        }
    }

    folder_counts
        .into_iter()
        .map(|(dir, count)| {
            let dec = FileDecoration {
                badge: Some(count.to_string()),
                badge_color: worst_color.get(&dir).copied(),
                tooltip: Some(format!("{count} file(s) with issues")),
                ..Default::default()
            };
            (dir, dec)
        })
        .collect()
}

fn color_priority(c: Color) -> u8 {
    match (c.r > 200, c.g < 100, c.g > 150, c.b > 200) {
        (true, true, _, _) => 3, // red
        (true, _, true, _) => 2, // yellow
        (_, _, _, true) => 1,    // blue
        _ => 0,
    }
}

fn merge_decoration(existing: &mut FileDecoration, other: &FileDecoration) {
    if other.badge.is_some() && existing.badge.is_none() {
        existing.badge.clone_from(&other.badge);
    }
    if let Some(oc) = other.badge_color {
        if existing
            .badge_color
            .is_none_or(|c| color_priority(c) < color_priority(oc))
        {
            existing.badge_color = Some(oc);
        }
    }
    if other.tooltip.is_some() && existing.tooltip.is_none() {
        existing.tooltip.clone_from(&other.tooltip);
    }
    existing.strikethrough |= other.strikethrough;
    existing.faded |= other.faded;
    if other.color.is_some() && existing.color.is_none() {
        existing.color = other.color;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_modified_produces_blue_badge() {
        let input = vec![GitStatusInput {
            path: PathBuf::from("/repo/src/main.rs"),
            status: GitFileStatus::Modified,
        }];
        let decs = compute_git_decorations(&input);
        let dec = decs.get(Path::new("/repo/src/main.rs")).unwrap();
        assert_eq!(dec.badge.as_deref(), Some("M"));
        assert_eq!(dec.badge_color, Some(BLUE));
    }

    #[test]
    fn diagnostic_errors_produce_count_badge() {
        let mut diags = HashMap::new();
        diags.insert(
            PathBuf::from("/repo/lib.rs"),
            DiagnosticSummary {
                errors: 5,
                warnings: 2,
            },
        );
        let decs = compute_diagnostic_decorations(&diags);
        let dec = decs.get(Path::new("/repo/lib.rs")).unwrap();
        assert_eq!(dec.badge.as_deref(), Some("5"));
        assert_eq!(dec.badge_color, Some(RED));
    }
}
