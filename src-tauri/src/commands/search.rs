use globset::{Glob, GlobSet, GlobSetBuilder};
use memchr::memmem;
use rayon::prelude::*;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct FileMatch {
    pub path: String,
    pub name: String,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextMatch {
    pub path: String,
    pub line_number: usize,
    pub line_content: String,
    pub column: usize,
    pub match_length: usize,
}

#[derive(Debug, Deserialize)]
pub struct SearchFileOptions {
    pub max_results: Option<usize>,
    pub include_hidden: Option<bool>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct SearchTextOptions {
    pub max_results: Option<usize>,
    pub case_sensitive: Option<bool>,
    pub is_regex: Option<bool>,
    pub include_hidden: Option<bool>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub max_file_size: Option<u64>,
}

const DEFAULT_MAX_RESULTS: usize = 500;
const DEFAULT_MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

static ALWAYS_SKIP: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "dist",
    "build",
    "out",
    "__pycache__",
    ".next",
    ".cache",
];

fn build_globset(patterns: &[String]) -> Option<GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        if let Ok(g) = Glob::new(p) {
            builder.add(g);
        }
    }
    builder.build().ok()
}

fn should_skip(entry: &walkdir::DirEntry, include_hidden: bool) -> bool {
    let name = entry.file_name().to_string_lossy();
    if !include_hidden && name.starts_with('.') {
        return true;
    }
    if entry.file_type().is_dir() && ALWAYS_SKIP.contains(&name.as_ref()) {
        return true;
    }
    false
}

#[allow(clippy::cast_possible_wrap)]
fn fuzzy_score(pattern: &[u8], target: &str) -> Option<i64> {
    if pattern.is_empty() {
        return Some(0);
    }
    let target_bytes = target.as_bytes();
    let mut pi = 0;
    let mut score: i64 = 0;
    let mut consecutive = 0i64;
    let mut prev_match = false;

    for (ti, &tc) in target_bytes.iter().enumerate() {
        if pi < pattern.len() && tc.eq_ignore_ascii_case(&pattern[pi]) {
            score += 1;
            if ti == 0 || !target_bytes[ti - 1].is_ascii_alphanumeric() {
                score += 5;
            }
            if tc == pattern[pi] {
                score += 1;
            }
            if prev_match {
                consecutive += 1;
                score += consecutive * 2;
            } else {
                consecutive = 0;
            }
            prev_match = true;
            pi += 1;
        } else {
            prev_match = false;
            consecutive = 0;
        }
    }

    if pi == pattern.len() {
        let len_penalty = (target_bytes.len() as i64 - pattern.len() as i64).min(20);
        Some(score * 100 - len_penalty)
    } else {
        None
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn search_files(
    root: String,
    pattern: String,
    options: Option<SearchFileOptions>,
) -> Result<Vec<FileMatch>, String> {
    let max_results = options
        .as_ref()
        .and_then(|o| o.max_results)
        .unwrap_or(DEFAULT_MAX_RESULTS);
    let include_hidden = options
        .as_ref()
        .and_then(|o| o.include_hidden)
        .unwrap_or(false);
    let include_set = options
        .as_ref()
        .and_then(|o| o.include.as_deref())
        .and_then(|v| if v.is_empty() { None } else { build_globset(v) });
    let exclude_set = options
        .as_ref()
        .and_then(|o| o.exclude.as_deref())
        .and_then(|v| if v.is_empty() { None } else { build_globset(v) });

    let pattern_bytes = pattern.as_bytes().to_vec();
    let mut scored: Vec<FileMatch> = Vec::with_capacity(max_results * 2);

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .max_depth(20)
        .into_iter()
        .filter_entry(|e| !should_skip(e, include_hidden))
        .filter_map(std::result::Result::ok)
    {
        if entry.file_type().is_dir() {
            continue;
        }

        let path = entry.path();

        if let Some(ref inc) = include_set {
            if !inc.is_match(path) {
                continue;
            }
        }
        if let Some(ref exc) = exclude_set {
            if exc.is_match(path) {
                continue;
            }
        }

        let name = entry.file_name().to_string_lossy();
        let Some(score) = fuzzy_score(&pattern_bytes, &name) else {
            continue;
        };

        scored.push(FileMatch {
            path: path.to_string_lossy().into_owned(),
            name: name.into_owned(),
            score,
        });
    }

    scored.sort_unstable_by(|a, b| b.score.cmp(&a.score));
    scored.truncate(max_results);
    Ok(scored)
}

#[tauri::command]
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
pub fn search_text(
    root: String,
    query: String,
    options: Option<SearchTextOptions>,
) -> Result<Vec<TextMatch>, String> {
    let max_results = options
        .as_ref()
        .and_then(|o| o.max_results)
        .unwrap_or(DEFAULT_MAX_RESULTS);
    let case_sensitive = options
        .as_ref()
        .and_then(|o| o.case_sensitive)
        .unwrap_or(false);
    let is_regex = options.as_ref().and_then(|o| o.is_regex).unwrap_or(false);
    let include_hidden = options
        .as_ref()
        .and_then(|o| o.include_hidden)
        .unwrap_or(false);
    let max_file_size = options
        .as_ref()
        .and_then(|o| o.max_file_size)
        .unwrap_or(DEFAULT_MAX_FILE_SIZE);
    let include_set = options
        .as_ref()
        .and_then(|o| o.include.as_deref())
        .and_then(|v| if v.is_empty() { None } else { build_globset(v) });
    let exclude_set = options
        .as_ref()
        .and_then(|o| o.exclude.as_deref())
        .and_then(|v| if v.is_empty() { None } else { build_globset(v) });

    let use_literal = !is_regex && case_sensitive;
    let literal_finder = if use_literal {
        Some(Arc::new(memmem::Finder::new(query.as_bytes())))
    } else {
        None
    };

    let pattern = if is_regex {
        query.clone()
    } else {
        regex::escape(&query)
    };

    let re = if use_literal {
        None
    } else {
        Some(
            RegexBuilder::new(&pattern)
                .case_insensitive(!case_sensitive)
                .build()
                .map_err(|e| format!("Invalid search pattern: {e}"))?,
        )
    };

    let files: Vec<_> = WalkDir::new(&root)
        .follow_links(false)
        .max_depth(20)
        .into_iter()
        .filter_entry(|e| !should_skip(e, include_hidden))
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let path = e.path();
            if let Some(ref inc) = include_set {
                if !inc.is_match(path) {
                    return false;
                }
            }
            if let Some(ref exc) = exclude_set {
                if exc.is_match(path) {
                    return false;
                }
            }
            if let Ok(meta) = e.metadata() {
                if meta.len() > max_file_size {
                    return false;
                }
            }
            true
        })
        .collect();

    let hit_count = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));

    let all_matches: Vec<Vec<TextMatch>> = files
        .par_iter()
        .filter_map(|entry| {
            if done.load(Ordering::Relaxed) {
                return None;
            }

            let content = fs::read_to_string(entry.path()).ok()?;
            let path_str = entry.path().to_string_lossy().into_owned();
            let mut local = Vec::new();

            if let Some(ref finder) = literal_finder {
                for (line_idx, line) in content.lines().enumerate() {
                    let line_bytes = line.as_bytes();
                    let mut start = 0;
                    while let Some(pos) = finder.find(&line_bytes[start..]) {
                        local.push(TextMatch {
                            path: path_str.clone(),
                            line_number: line_idx + 1,
                            line_content: line.to_string(),
                            column: start + pos,
                            match_length: query.len(),
                        });
                        start += pos + 1;
                        if hit_count.load(Ordering::Relaxed) + local.len() >= max_results {
                            break;
                        }
                    }
                    if hit_count.load(Ordering::Relaxed) + local.len() >= max_results {
                        break;
                    }
                }
            } else if let Some(ref re) = re {
                for (line_idx, line) in content.lines().enumerate() {
                    for m in re.find_iter(line) {
                        local.push(TextMatch {
                            path: path_str.clone(),
                            line_number: line_idx + 1,
                            line_content: line.to_string(),
                            column: m.start(),
                            match_length: m.end() - m.start(),
                        });
                        if hit_count.load(Ordering::Relaxed) + local.len() >= max_results {
                            break;
                        }
                    }
                    if hit_count.load(Ordering::Relaxed) + local.len() >= max_results {
                        break;
                    }
                }
            }

            if local.is_empty() {
                None
            } else {
                let prev = hit_count.fetch_add(local.len(), Ordering::Relaxed);
                if prev + local.len() >= max_results {
                    done.store(true, Ordering::Relaxed);
                    local.truncate(max_results.saturating_sub(prev));
                }
                Some(local)
            }
        })
        .collect();

    let total: usize = all_matches.iter().map(std::vec::Vec::len).sum();
    let mut results = Vec::with_capacity(total.min(max_results));
    for batch in all_matches {
        let remaining = max_results.saturating_sub(results.len());
        if remaining == 0 {
            break;
        }
        results.extend(batch.into_iter().take(remaining));
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// New commands backed by sidex-workspace::SearchEngine
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct WorkspaceSearchOptions {
    pub case_sensitive: Option<bool>,
    pub is_regex: Option<bool>,
    pub whole_word: Option<bool>,
    pub max_results: Option<usize>,
    pub max_file_size: Option<u64>,
    pub include_patterns: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    pub context_lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsSearchMatch {
    pub path: String,
    pub line_number: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsContextLine {
    pub line_number: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsMatchWithContext {
    pub line_number: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
    pub before_context: Vec<WsContextLine>,
    pub after_context: Vec<WsContextLine>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsSearchGroup {
    pub file_path: String,
    pub matches: Vec<WsMatchWithContext>,
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsReplacementEdit {
    pub line_number: usize,
    pub match_start: usize,
    pub match_end: usize,
    pub original: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsFileReplacement {
    pub path: String,
    pub edits: Vec<WsReplacementEdit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WsReplaceReport {
    pub files_modified: u32,
    pub replacements_made: u32,
    pub errors: Vec<(String, String)>,
}

fn build_ws_query(query: &str, options: &Option<WorkspaceSearchOptions>) -> sidex_workspace::SearchQuery {
    sidex_workspace::SearchQuery {
        pattern: query.to_string(),
        is_regex: options.as_ref().and_then(|o| o.is_regex).unwrap_or(false),
        case_sensitive: options.as_ref().and_then(|o| o.case_sensitive).unwrap_or(false),
        whole_word: options.as_ref().and_then(|o| o.whole_word).unwrap_or(false),
        max_results: options.as_ref().and_then(|o| o.max_results),
    }
}

fn build_ws_options(options: &Option<WorkspaceSearchOptions>) -> sidex_workspace::SearchOptions {
    sidex_workspace::SearchOptions {
        include_patterns: options
            .as_ref()
            .and_then(|o| o.include_patterns.clone())
            .unwrap_or_default(),
        exclude_patterns: options
            .as_ref()
            .and_then(|o| o.exclude_patterns.clone())
            .unwrap_or_default(),
        max_results: options.as_ref().and_then(|o| o.max_results),
        max_file_size: options.as_ref().and_then(|o| o.max_file_size),
        context_lines: options.as_ref().and_then(|o| o.context_lines),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn search_workspace(
    root: String,
    query: String,
    options: Option<WorkspaceSearchOptions>,
) -> Result<Vec<WsSearchMatch>, String> {
    let ws_query = build_ws_query(&query, &options);
    let ws_opts = build_ws_options(&options);
    let root_path = std::path::Path::new(&root);

    let results = sidex_workspace::SearchEngine::search_with_options(root_path, &ws_query, &ws_opts)
        .map_err(|e| e.to_string())?;

    Ok(results
        .into_iter()
        .map(|r| WsSearchMatch {
            path: r.path.to_string_lossy().into_owned(),
            line_number: r.line_number,
            line_text: r.line_text,
            match_start: r.match_start,
            match_end: r.match_end,
        })
        .collect())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn search_workspace_grouped(
    root: String,
    query: String,
    options: Option<WorkspaceSearchOptions>,
) -> Result<Vec<WsSearchGroup>, String> {
    let ws_query = build_ws_query(&query, &options);
    let ws_opts = build_ws_options(&options);
    let root_path = std::path::Path::new(&root);

    let groups = sidex_workspace::SearchEngine::search_grouped(root_path, &ws_query, &ws_opts)
        .map_err(|e| e.to_string())?;

    Ok(groups
        .into_iter()
        .map(|g| WsSearchGroup {
            file_path: g.file_path.to_string_lossy().into_owned(),
            line_count: g.line_count,
            matches: g
                .matches
                .into_iter()
                .map(|m| WsMatchWithContext {
                    line_number: m.line_number,
                    line_text: m.line_text,
                    match_start: m.match_start,
                    match_end: m.match_end,
                    before_context: m
                        .before_context
                        .into_iter()
                        .map(|c| WsContextLine {
                            line_number: c.line_number,
                            text: c.text,
                        })
                        .collect(),
                    after_context: m
                        .after_context
                        .into_iter()
                        .map(|c| WsContextLine {
                            line_number: c.line_number,
                            text: c.text,
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn search_workspace_replace_preview(
    root: String,
    query: String,
    replacement: String,
    options: Option<WorkspaceSearchOptions>,
) -> Result<Vec<WsFileReplacement>, String> {
    let ws_query = build_ws_query(&query, &options);
    let root_path = std::path::Path::new(&root);

    let replacements =
        sidex_workspace::SearchEngine::replace_in_files(root_path, &ws_query, &replacement)
            .map_err(|e| e.to_string())?;

    Ok(replacements
        .into_iter()
        .map(|fr| WsFileReplacement {
            path: fr.path.to_string_lossy().into_owned(),
            edits: fr
                .edits
                .into_iter()
                .map(|e| WsReplacementEdit {
                    line_number: e.line_number,
                    match_start: e.match_start,
                    match_end: e.match_end,
                    original: e.original,
                    replacement: e.replacement,
                })
                .collect(),
        })
        .collect())
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub fn search_workspace_replace_apply(
    root: String,
    query: String,
    replacement: String,
) -> Result<WsReplaceReport, String> {
    let ws_query = sidex_workspace::SearchQuery {
        pattern: query,
        is_regex: false,
        case_sensitive: true,
        whole_word: false,
        max_results: None,
    };
    let root_path = std::path::Path::new(&root);

    let report =
        sidex_workspace::SearchEngine::replace_in_files_with_report(root_path, &ws_query, &replacement)
            .map_err(|e| e.to_string())?;

    Ok(WsReplaceReport {
        files_modified: report.files_modified,
        replacements_made: report.replacements_made,
        errors: report
            .errors
            .into_iter()
            .map(|(p, e)| (p.to_string_lossy().into_owned(), e))
            .collect(),
    })
}
