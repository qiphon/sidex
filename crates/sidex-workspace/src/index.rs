//! High-performance in-memory text indexing for code search.
//!
//! Provides fast text search capabilities using:
//! - Inverted index (word -> locations)
//! - Trigram indexing for fuzzy/substring search
//! - Incremental updates for file changes
//! - Multi-threaded indexing with Rayon
//!
//! Ported from `src-tauri/src/commands/index.rs`, stripped of Tauri state wrappers.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use dashmap::DashMap;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};

const DEFAULT_MAX_FILE_SIZE: u64 = 1024 * 1024; // 1 MiB
const DEFAULT_MAX_RESULTS: usize = 1000;

/// Options for building the index.
#[derive(Debug, Clone, Deserialize)]
pub struct IndexOptions {
    pub file_extensions: Vec<String>,
    pub max_file_size: Option<u64>,
    pub exclude_dirs: Option<Vec<String>>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            file_extensions: vec![],
            max_file_size: Some(DEFAULT_MAX_FILE_SIZE),
            exclude_dirs: Some(vec![
                "node_modules".to_string(),
                ".git".to_string(),
                "target".to_string(),
                "dist".to_string(),
                "build".to_string(),
                "__pycache__".to_string(),
                ".next".to_string(),
            ]),
        }
    }
}

/// Options for searching the index.
#[derive(Debug, Clone, Deserialize)]
pub struct IndexSearchOptions {
    pub case_sensitive: bool,
    pub max_results: Option<usize>,
    pub whole_word: bool,
    pub regex: bool,
    pub file_pattern: Option<String>,
}

impl Default for IndexSearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            max_results: Some(DEFAULT_MAX_RESULTS),
            whole_word: false,
            regex: false,
            file_pattern: None,
        }
    }
}

/// A single search result from the index.
#[derive(Debug, Clone, Serialize)]
pub struct IndexSearchResult {
    pub path: String,
    pub line_number: usize,
    pub column: usize,
    pub line_content: String,
    pub score: f32,
}

/// File change event for incremental updates.
#[derive(Debug, Clone, Deserialize)]
pub struct FileChange {
    pub path: String,
    /// One of `"created"`, `"modified"`, `"deleted"`.
    pub change_type: String,
}

/// Statistics about the index.
#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
    pub total_files: usize,
    pub total_words: usize,
    pub memory_bytes: usize,
    pub root_path: String,
}

#[derive(Debug, Clone)]
struct WordLocation {
    file_id: u32,
    line: usize,
    column: usize,
}

#[derive(Debug, Clone)]
struct FileInfo {
    path: String,
    words: HashSet<String>,
}

/// Inverted index for fast text search.
pub struct InvertedIndex {
    word_index: DashMap<String, Vec<WordLocation>>,
    trigram_index: Option<DashMap<String, Vec<String>>>,
    files: DashMap<u32, FileInfo>,
    path_to_id: DashMap<String, u32>,
    next_file_id: AtomicU32,
    root_path: std::sync::RwLock<String>,
    word_regex: Regex,
    memory_estimate: AtomicUsize,
}

impl InvertedIndex {
    /// Create a new index. Set `enable_trigram` for fuzzy substring matching.
    pub fn new(enable_trigram: bool) -> Self {
        Self {
            word_index: DashMap::new(),
            trigram_index: if enable_trigram {
                Some(DashMap::new())
            } else {
                None
            },
            files: DashMap::new(),
            path_to_id: DashMap::new(),
            next_file_id: AtomicU32::new(1),
            root_path: std::sync::RwLock::new(String::new()),
            word_regex: Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]{2,}").unwrap(),
            memory_estimate: AtomicUsize::new(0),
        }
    }

    /// Set the root path for this index.
    pub fn set_root(&self, root: String) {
        let mut rp = self.root_path.write().unwrap();
        *rp = root;
    }

    fn get_root(&self) -> String {
        self.root_path.read().unwrap().clone()
    }

    fn get_or_create_file_id(&self, path: &str) -> u32 {
        *self
            .path_to_id
            .entry(path.to_string())
            .or_insert_with(|| self.next_file_id.fetch_add(1, Ordering::SeqCst))
    }

    fn add_trigrams(&self, word: &str) {
        if let Some(ref trigram_index) = self.trigram_index {
            if word.len() >= 3 {
                let word_lower = word.to_lowercase();
                for i in 0..=word_lower.len().saturating_sub(3) {
                    let trigram = &word_lower[i..i + 3];
                    trigram_index
                        .entry(trigram.to_string())
                        .and_modify(|v| {
                            if !v.contains(&word_lower) {
                                v.push(word_lower.clone());
                            }
                        })
                        .or_insert_with(|| vec![word_lower.clone()]);
                }
            }
        }
    }

    /// Index a single file.
    pub fn index_file(&self, file_path: &Path, max_file_size: u64) -> Result<(), String> {
        let path_str = file_path.to_string_lossy().to_string();

        let metadata = fs::metadata(file_path).map_err(|e| e.to_string())?;
        if metadata.len() > max_file_size {
            return Ok(());
        }

        let content = fs::read(file_path).map_err(|e| e.to_string())?;
        if content[..content.len().min(8192)].contains(&0) {
            return Ok(());
        }

        let Ok(text) = String::from_utf8(content) else {
            return Ok(());
        };

        self.remove_file(&path_str);

        let file_id = self.get_or_create_file_id(&path_str);
        let mut file_words: HashSet<String> = HashSet::new();

        for (line_idx, line) in text.lines().enumerate() {
            for mat in self.word_regex.find_iter(line) {
                let word = mat.as_str();
                let column = mat.start();
                let normalized_word = word.to_lowercase();

                self.word_index
                    .entry(normalized_word.clone())
                    .or_default()
                    .push(WordLocation {
                        file_id,
                        line: line_idx + 1,
                        column: column + 1,
                    });

                self.add_trigrams(&normalized_word);
                file_words.insert(normalized_word);

                self.memory_estimate.fetch_add(
                    std::mem::size_of::<WordLocation>() + word.len(),
                    Ordering::Relaxed,
                );
            }
        }

        self.files.insert(
            file_id,
            FileInfo {
                path: path_str,
                words: file_words,
            },
        );

        Ok(())
    }

    /// Remove a file from the index.
    pub fn remove_file(&self, path: &str) {
        if let Some((_, file_id)) = self.path_to_id.remove(path) {
            if let Some((_, file_info)) = self.files.remove(&file_id) {
                for word in file_info.words {
                    if let Some(mut locations) = self.word_index.get_mut(&word) {
                        locations.retain(|loc| loc.file_id != file_id);
                        if locations.is_empty() {
                            drop(locations);
                            self.word_index.remove(&word);
                        }
                    }
                }
            }
        }
    }

    /// Search for exact word matches.
    pub fn search_exact(
        &self,
        query: &str,
        options: &IndexSearchOptions,
    ) -> Vec<IndexSearchResult> {
        let max_results = options.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let mut results = Vec::new();

        let search_word = if options.case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        let file_pattern = options
            .file_pattern
            .as_ref()
            .and_then(|p| globset::Glob::new(p).ok().map(|g| g.compile_matcher()));

        if let Some(locations) = self.word_index.get(&search_word) {
            for loc in locations.iter() {
                if results.len() >= max_results {
                    break;
                }
                if let Some(file_info) = self.files.get(&loc.file_id) {
                    if let Some(ref pattern) = file_pattern {
                        if !pattern.is_match(&file_info.path) {
                            continue;
                        }
                    }
                    if let Ok(line_content) = get_line_at(&file_info.path, loc.line) {
                        results.push(IndexSearchResult {
                            path: file_info.path.clone(),
                            line_number: loc.line,
                            column: loc.column,
                            line_content,
                            score: 1.0,
                        });
                    }
                }
            }
        }

        results
    }

    /// Fuzzy search using trigram index.
    pub fn search_fuzzy(
        &self,
        query: &str,
        options: &IndexSearchOptions,
    ) -> Vec<IndexSearchResult> {
        let max_results = options.max_results.unwrap_or(DEFAULT_MAX_RESULTS);

        if query.len() < 3 {
            return self.search_exact(query, options);
        }

        let query_lower = query.to_lowercase();
        let mut candidate_words: Option<HashSet<String>> = None;

        if let Some(ref trigram_index) = self.trigram_index {
            for i in 0..=query_lower.len().saturating_sub(3) {
                let trigram = &query_lower[i..i + 3];
                if let Some(words) = trigram_index.get(trigram) {
                    let word_set: HashSet<String> = words.iter().cloned().collect();
                    candidate_words = match candidate_words {
                        Some(existing) => {
                            let intersection: HashSet<String> =
                                existing.intersection(&word_set).cloned().collect();
                            if intersection.is_empty() {
                                Some(existing.union(&word_set).cloned().collect())
                            } else {
                                Some(intersection)
                            }
                        }
                        None => Some(word_set),
                    };
                }
            }
        }

        let Some(candidates) = candidate_words else {
            return self.search_exact(query, options);
        };

        let mut scored_results: Vec<(IndexSearchResult, f32)> = Vec::new();

        let file_pattern = options
            .file_pattern
            .as_ref()
            .and_then(|p| globset::Glob::new(p).ok().map(|g| g.compile_matcher()));

        for word in candidates {
            if let Some(locations) = self.word_index.get(&word) {
                for loc in locations.iter() {
                    if scored_results.len() >= max_results * 2 {
                        break;
                    }
                    if let Some(file_info) = self.files.get(&loc.file_id) {
                        if let Some(ref pattern) = file_pattern {
                            if !pattern.is_match(&file_info.path) {
                                continue;
                            }
                        }
                        let score = calculate_score(query, &word);
                        if score > 0.3 {
                            if let Ok(line_content) = get_line_at(&file_info.path, loc.line) {
                                let line_check = if options.case_sensitive {
                                    line_content.clone()
                                } else {
                                    line_content.to_lowercase()
                                };
                                if line_check.contains(&query_lower) {
                                    scored_results.push((
                                        IndexSearchResult {
                                            path: file_info.path.clone(),
                                            line_number: loc.line,
                                            column: loc.column,
                                            line_content,
                                            score,
                                        },
                                        score,
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored_results.truncate(max_results);
        scored_results.into_iter().map(|(r, _)| r).collect()
    }

    /// Search with a regex pattern.
    pub fn search_regex(
        &self,
        pattern: &str,
        options: &IndexSearchOptions,
    ) -> Result<Vec<IndexSearchResult>, String> {
        let regex = if options.case_sensitive {
            Regex::new(pattern).map_err(|e| e.to_string())?
        } else {
            Regex::new(&format!("(?i){pattern}")).map_err(|e| e.to_string())?
        };

        let max_results = options.max_results.unwrap_or(DEFAULT_MAX_RESULTS);
        let mut results = Vec::new();

        let file_pattern = options
            .file_pattern
            .as_ref()
            .and_then(|p| globset::Glob::new(p).ok().map(|g| g.compile_matcher()));

        for entry in &self.files {
            if results.len() >= max_results {
                break;
            }
            let file_info = entry.value();
            if let Some(ref pat) = file_pattern {
                if !pat.is_match(&file_info.path) {
                    continue;
                }
            }
            if let Ok(content) = fs::read_to_string(&file_info.path) {
                for (line_idx, line) in content.lines().enumerate() {
                    if results.len() >= max_results {
                        break;
                    }
                    if let Some(mat) = regex.find(line) {
                        results.push(IndexSearchResult {
                            path: file_info.path.clone(),
                            line_number: line_idx + 1,
                            column: mat.start() + 1,
                            line_content: line.to_string(),
                            score: 1.0,
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Return statistics about the index.
    pub fn stats(&self) -> IndexStats {
        IndexStats {
            total_files: self.files.len(),
            total_words: self.word_index.len(),
            memory_bytes: self.memory_estimate.load(Ordering::Relaxed),
            root_path: self.get_root(),
        }
    }

    /// Clear the entire index.
    pub fn clear(&self) {
        self.word_index.clear();
        if let Some(ref trigram_index) = self.trigram_index {
            trigram_index.clear();
        }
        self.files.clear();
        self.path_to_id.clear();
        self.next_file_id.store(1, Ordering::SeqCst);
        self.memory_estimate.store(0, Ordering::Relaxed);
    }

    /// Search the index, choosing strategy based on options.
    pub fn search(
        &self,
        query: &str,
        options: &IndexSearchOptions,
    ) -> Result<Vec<IndexSearchResult>, String> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        if options.regex {
            self.search_regex(query, options)
        } else if options.whole_word {
            Ok(self.search_exact(query, options))
        } else {
            Ok(self.search_fuzzy(query, options))
        }
    }

    /// Apply incremental updates for file changes.
    pub fn update(&self, changes: &[FileChange]) -> Result<(), String> {
        let max_file_size = DEFAULT_MAX_FILE_SIZE;
        for change in changes {
            match change.change_type.as_str() {
                "created" | "modified" => {
                    let path = Path::new(&change.path);
                    if path.exists() {
                        self.index_file(path, max_file_size)?;
                    }
                }
                "deleted" => {
                    self.remove_file(&change.path);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Build the index for all files under `root`.
pub fn build_index(
    index: &InvertedIndex,
    root: &Path,
    options: &IndexOptions,
) -> Result<IndexStats, String> {
    if !root.exists() {
        return Err(format!("Root path does not exist: {}", root.display()));
    }

    index.clear();
    index.set_root(root.to_string_lossy().to_string());

    let files = collect_files(root, options)?;
    let max_file_size = options.max_file_size.unwrap_or(DEFAULT_MAX_FILE_SIZE);

    files.par_iter().for_each(|file_path| {
        let _ = index.index_file(file_path, max_file_size);
    });

    Ok(index.stats())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn calculate_score(query: &str, word: &str) -> f32 {
    let query_lower = query.to_lowercase();
    let word_lower = word.to_lowercase();

    if query_lower == word_lower {
        return 1.0;
    }
    if word_lower.starts_with(&query_lower) {
        return 0.9;
    }
    if word_lower.contains(&query_lower) {
        return 0.8;
    }
    if is_subsequence(&query_lower, &word_lower) {
        return 0.6;
    }

    let distance = levenshtein_distance(&query_lower, &word_lower);
    let max_len = query_lower.len().max(word_lower.len());
    if max_len == 0 {
        return 0.0;
    }

    #[allow(clippy::cast_precision_loss)]
    let similarity = 1.0 - (distance as f32 / max_len as f32);
    similarity * 0.5
}

fn is_subsequence(query: &str, word: &str) -> bool {
    let mut query_chars = query.chars();
    let mut current = query_chars.next();
    for c in word.chars() {
        if let Some(qc) = current {
            if qc == c {
                current = query_chars.next();
            }
        } else {
            break;
        }
    }
    current.is_none()
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row = vec![0; b_len + 1];

    for i in 1..=a_len {
        curr_row[0] = i;
        for j in 1..=b_len {
            let cost = usize::from(a_chars[i - 1] != b_chars[j - 1]);
            curr_row[j] = (curr_row[j - 1] + 1)
                .min(prev_row[j] + 1)
                .min(prev_row[j - 1] + cost);
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[b_len]
}

fn get_line_at(path: &str, line_number: usize) -> Result<String, String> {
    use std::io::BufRead;
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = std::io::BufReader::new(file);
    let target = line_number.saturating_sub(1);
    reader
        .lines()
        .nth(target)
        .ok_or_else(|| "Line not found".to_string())?
        .map_err(|e| e.to_string())
}

#[allow(clippy::unnecessary_wraps)]
fn collect_files(root: &Path, options: &IndexOptions) -> Result<Vec<std::path::PathBuf>, String> {
    let max_file_size = options.max_file_size.unwrap_or(DEFAULT_MAX_FILE_SIZE);
    let exclude_dirs: HashSet<String> = options
        .exclude_dirs
        .as_ref()
        .map(|v| v.iter().cloned().collect())
        .unwrap_or_default();

    let extensions: HashSet<String> = options
        .file_extensions
        .iter()
        .map(|e| e.to_lowercase())
        .collect();

    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .max_depth(50)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !exclude_dirs.contains(name.as_ref())
        })
    {
        let Ok(entry) = entry else {
            continue;
        };

        if !entry.file_type().is_file() {
            continue;
        }

        if !extensions.is_empty() {
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase);

            if ext.is_none_or(|e| !extensions.contains(&e)) {
                continue;
            }
        }

        if let Ok(metadata) = entry.metadata() {
            if metadata.len() > max_file_size {
                continue;
            }
        }

        files.push(entry.path().to_path_buf());
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_score() {
        assert!((calculate_score("hello", "hello") - 1.0).abs() < f32::EPSILON);
        assert!((calculate_score("hel", "hello") - 0.9).abs() < f32::EPSILON);
        assert!(calculate_score("ell", "hello") > 0.5);
    }

    #[test]
    fn test_is_subsequence() {
        assert!(is_subsequence("abc", "aabbcc"));
        assert!(!is_subsequence("abc", "acb"));
        assert!(is_subsequence("", "anything"));
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("a", ""), 1);
        assert_eq!(levenshtein_distance("", "a"), 1);
    }

    #[test]
    fn build_and_search_index() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("hello.rs"),
            "fn main() {\n    println!(\"world\");\n}\n",
        )
        .unwrap();

        let index = InvertedIndex::new(true);
        let stats = build_index(&index, tmp.path(), &IndexOptions::default()).unwrap();
        assert_eq!(stats.total_files, 1);
        assert!(stats.total_words > 0);

        let opts = IndexSearchOptions::default();
        let results = index.search("main", &opts).unwrap();
        assert!(!results.is_empty());
    }
}
