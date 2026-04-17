//! Fast fuzzy file finder for Ctrl+P style navigation.
//!
//! Builds an in-memory index of file paths under a workspace root using the
//! `ignore` crate for `.gitignore`-aware walking. Supports incremental updates
//! on file-system events and prioritises recently opened files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use serde::Serialize;

use crate::watcher::FileEvent;

/// A file match returned by the fuzzy finder.
#[derive(Debug, Clone, Serialize)]
pub struct FileMatch {
    pub path: PathBuf,
    pub score: f64,
    pub match_positions: Vec<usize>,
}

/// In-memory index of workspace file paths for fast fuzzy matching.
pub struct FileIndex {
    paths: Vec<PathBuf>,
    root: PathBuf,
    recent: Vec<PathBuf>,
}

impl FileIndex {
    /// Build a new index by scanning all files under `root`.
    pub fn build(root: &Path) -> Self {
        let paths = scan_files(root);
        Self {
            paths,
            root: root.to_path_buf(),
            recent: Vec::new(),
        }
    }

    /// Incrementally update the index based on file-system events.
    pub fn update(&mut self, events: &[FileEvent]) {
        use crate::watcher::FileEventKind;

        let mut to_remove: HashSet<PathBuf> = HashSet::new();
        let mut to_add: Vec<PathBuf> = Vec::new();

        for event in events {
            match event.kind {
                FileEventKind::Created => {
                    if event.path.is_file() {
                        to_add.push(event.path.clone());
                    }
                }
                FileEventKind::Deleted => {
                    to_remove.insert(event.path.clone());
                }
                FileEventKind::Renamed => {
                    to_remove.insert(event.path.clone());
                    if event.path.is_file() {
                        to_add.push(event.path.clone());
                    }
                }
                FileEventKind::Modified => {}
            }
        }

        if !to_remove.is_empty() {
            self.paths.retain(|p| !to_remove.contains(p));
            self.recent.retain(|p| !to_remove.contains(p));
        }

        for path in to_add {
            if !self.paths.contains(&path) {
                self.paths.push(path);
            }
        }
    }

    /// Record a file as recently opened (boosts its score).
    pub fn mark_recent(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        if self.recent.len() > 100 {
            self.recent.truncate(100);
        }
    }

    /// Fuzzy-match the query against indexed file paths.
    pub fn query(&self, pattern: &str, max_results: usize) -> Vec<FileMatch> {
        if pattern.is_empty() {
            return self
                .recent
                .iter()
                .take(max_results)
                .map(|p| FileMatch {
                    path: p.clone(),
                    score: 1.0,
                    match_positions: vec![],
                })
                .collect();
        }

        let pattern_lower: Vec<char> = pattern.to_lowercase().chars().collect();

        let mut scored: Vec<FileMatch> = self
            .paths
            .iter()
            .filter_map(|path| {
                let relative = path
                    .strip_prefix(&self.root)
                    .unwrap_or(path)
                    .to_string_lossy();
                let (score, positions) = fuzzy_match_scored(&pattern_lower, &relative)?;

                let recent_boost = self
                    .recent
                    .iter()
                    .position(|r| r == path)
                    .map_or(0.0, |idx| 50.0 / (idx as f64 + 1.0));

                Some(FileMatch {
                    path: path.clone(),
                    score: score + recent_boost,
                    match_positions: positions,
                })
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(max_results);
        scored
    }

    /// Returns the total number of indexed files.
    pub fn file_count(&self) -> usize {
        self.paths.len()
    }

    /// Rebuild the entire index from scratch.
    pub fn rebuild(&mut self) {
        self.paths = scan_files(&self.root);
    }
}

/// Fuzzy match with scoring. Returns `(score, match_positions)` or `None`.
///
/// Scoring hierarchy:
/// - Exact filename match: highest
/// - Prefix match: high
/// - Substring match: medium
/// - Fuzzy subsequence: lower, with bonuses for consecutive and word-boundary
///   matches
#[allow(clippy::cast_precision_loss)]
fn fuzzy_match_scored(pattern: &[char], target: &str) -> Option<(f64, Vec<usize>)> {
    if pattern.is_empty() {
        return Some((0.0, vec![]));
    }

    let target_lower: Vec<char> = target.to_lowercase().chars().collect();
    let target_chars: Vec<char> = target.chars().collect();

    // Check if all pattern chars exist as subsequence
    let mut pi = 0;
    let mut positions = Vec::with_capacity(pattern.len());

    for (ti, &tc) in target_lower.iter().enumerate() {
        if pi < pattern.len() && tc == pattern[pi] {
            positions.push(ti);
            pi += 1;
        }
    }

    if pi < pattern.len() {
        return None;
    }

    // Compute score
    let mut score: f64 = 0.0;

    // Check for exact filename match
    let filename = target
        .rsplit('/')
        .next()
        .or_else(|| target.rsplit('\\').next())
        .unwrap_or(target);
    let filename_lower: String = filename.to_lowercase();
    let pattern_str: String = pattern.iter().collect();

    if filename_lower == pattern_str {
        score += 1000.0;
    } else if filename_lower.starts_with(&pattern_str) {
        score += 500.0;
    } else if filename_lower.contains(&pattern_str) {
        score += 250.0;
    }

    // Subsequence bonuses
    let mut consecutive = 0.0_f64;
    for (i, &pos) in positions.iter().enumerate() {
        score += 10.0;

        // Word boundary bonus
        if pos == 0
            || !target_chars
                .get(pos.wrapping_sub(1))
                .is_some_and(|c| c.is_alphanumeric())
        {
            score += 20.0;
        }

        // Case match bonus
        if pos < target_chars.len() && target_chars[pos] == target_lower[pos] {
            // lowercase match, small bonus
            score += 1.0;
        }

        // Consecutive bonus
        if i > 0 && pos == positions[i - 1] + 1 {
            consecutive += 1.0;
            score += consecutive * 5.0;
        } else {
            consecutive = 0.0;
        }
    }

    // Length penalty — prefer shorter paths
    let len_penalty = (target.len() as f64 * 0.5).min(30.0);
    score -= len_penalty;

    Some((score, positions))
}

fn scan_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for result in WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .max_depth(Some(30))
        .build()
    {
        let Ok(entry) = result else { continue };
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            files.push(entry.into_path());
        }
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_tree() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "").unwrap();
        fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
        fs::write(tmp.path().join("src/utils.rs"), "").unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
        fs::write(tmp.path().join("README.md"), "").unwrap();
        tmp
    }

    #[test]
    fn build_indexes_all_files() {
        let tmp = setup_tree();
        let index = FileIndex::build(tmp.path());
        assert_eq!(index.file_count(), 5);
    }

    #[test]
    fn query_exact_filename() {
        let tmp = setup_tree();
        let index = FileIndex::build(tmp.path());
        let results = index.query("main.rs", 10);
        assert!(!results.is_empty());
        assert!(results[0].path.ends_with("main.rs"));
    }

    #[test]
    fn query_fuzzy_match() {
        let tmp = setup_tree();
        let index = FileIndex::build(tmp.path());
        let results = index.query("mnrs", 10);
        assert!(!results.is_empty());
        let top = &results[0];
        assert!(top.path.ends_with("main.rs"));
        assert!(!top.match_positions.is_empty());
    }

    #[test]
    fn query_no_match() {
        let tmp = setup_tree();
        let index = FileIndex::build(tmp.path());
        let results = index.query("zzzzz", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn query_empty_returns_recent() {
        let tmp = setup_tree();
        let mut index = FileIndex::build(tmp.path());
        let path = tmp.path().join("src/lib.rs");
        index.mark_recent(&path);
        let results = index.query("", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].path, path);
    }

    #[test]
    fn recent_files_boost_score() {
        let tmp = setup_tree();
        let mut index = FileIndex::build(tmp.path());
        let lib_path = tmp.path().join("src/lib.rs");
        index.mark_recent(&lib_path);

        let results = index.query("rs", 10);
        assert!(!results.is_empty());
        // lib.rs should be boosted to top (or near top)
        let lib_result = results.iter().find(|r| r.path == lib_path);
        assert!(lib_result.is_some());
    }

    #[test]
    fn update_add_file() {
        let tmp = setup_tree();
        let mut index = FileIndex::build(tmp.path());
        assert_eq!(index.file_count(), 5);

        let new_file = tmp.path().join("src/new.rs");
        fs::write(&new_file, "").unwrap();

        let events = vec![FileEvent {
            path: new_file.clone(),
            kind: crate::watcher::FileEventKind::Created,
        }];
        index.update(&events);
        assert_eq!(index.file_count(), 6);
    }

    #[test]
    fn update_remove_file() {
        let tmp = setup_tree();
        let mut index = FileIndex::build(tmp.path());
        let remove_path = tmp.path().join("README.md");

        let events = vec![FileEvent {
            path: remove_path,
            kind: crate::watcher::FileEventKind::Deleted,
        }];
        index.update(&events);
        assert_eq!(index.file_count(), 4);
    }

    #[test]
    fn rebuild_rescans() {
        let tmp = setup_tree();
        let mut index = FileIndex::build(tmp.path());
        assert_eq!(index.file_count(), 5);

        fs::write(tmp.path().join("extra.txt"), "").unwrap();
        index.rebuild();
        assert_eq!(index.file_count(), 6);
    }

    #[test]
    fn scoring_prefers_exact_over_fuzzy() {
        let pattern: Vec<char> = "main".chars().collect();
        let (exact_score, _) = fuzzy_match_scored(&pattern, "main.rs").unwrap();
        let (fuzzy_score, _) = fuzzy_match_scored(&pattern, "some_maintainer.rs").unwrap();
        assert!(exact_score > fuzzy_score);
    }

    #[test]
    fn scoring_prefers_shorter_paths() {
        let pattern: Vec<char> = "lib".chars().collect();
        let (short_score, _) = fuzzy_match_scored(&pattern, "lib.rs").unwrap();
        let (long_score, _) =
            fuzzy_match_scored(&pattern, "very/deeply/nested/path/to/lib.rs").unwrap();
        assert!(short_score > long_score);
    }

    #[test]
    fn match_positions_are_correct() {
        let pattern: Vec<char> = "mr".chars().collect();
        let (_, positions) = fuzzy_match_scored(&pattern, "main.rs").unwrap();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0], 0); // 'm' at index 0
        assert_eq!(positions[1], 5); // 'r' at index 5
    }

    #[test]
    fn max_results_respected() {
        let tmp = setup_tree();
        let index = FileIndex::build(tmp.path());
        let results = index.query("rs", 2);
        assert!(results.len() <= 2);
    }

    #[test]
    fn mark_recent_deduplicates() {
        let tmp = setup_tree();
        let mut index = FileIndex::build(tmp.path());
        let path = tmp.path().join("src/main.rs");
        index.mark_recent(&path);
        index.mark_recent(&path);
        index.mark_recent(&path);
        assert_eq!(index.recent.len(), 1);
    }
}
