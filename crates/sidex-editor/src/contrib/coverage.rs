//! Code coverage display — parsers for LCOV and Istanbul formats, per-file and
//! per-line coverage data, branch and function coverage, and a summary suitable
//! for status bar rendering.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ── Line coverage ────────────────────────────────────────────────────────────

/// Coverage state for a single source line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineCoverage {
    /// Line was executed `n` times.
    Covered(u32),
    /// Line was never executed.
    Uncovered,
    /// Line was partially covered (e.g. only one branch of an `if` taken).
    Partial,
}

impl LineCoverage {
    pub fn is_covered(self) -> bool {
        matches!(self, Self::Covered(_))
    }

    /// Gutter color RGBA.
    pub fn gutter_rgba(self) -> (f32, f32, f32, f32) {
        match self {
            Self::Covered(_) => (0.306, 0.788, 0.392, 0.6),
            Self::Uncovered => (0.957, 0.278, 0.278, 0.6),
            Self::Partial => (0.804, 0.678, 0.0, 0.6),
        }
    }
}

// ── Branch coverage ──────────────────────────────────────────────────────────

/// Coverage data for a single branch point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchCoverage {
    pub line: u32,
    pub block_number: u32,
    pub branch_number: u32,
    pub taken: bool,
    pub hit_count: u32,
}

// ── Function coverage ────────────────────────────────────────────────────────

/// Coverage data for a single function.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionCoverage {
    pub name: String,
    pub line: u32,
    pub hit_count: u32,
}

impl FunctionCoverage {
    pub fn is_covered(&self) -> bool {
        self.hit_count > 0
    }
}

// ── File coverage ────────────────────────────────────────────────────────────

/// All coverage data for a single source file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileCoverage {
    pub lines: HashMap<u32, LineCoverage>,
    pub branches: Vec<BranchCoverage>,
    pub functions: Vec<FunctionCoverage>,
}

impl FileCoverage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn line_coverage_at(&self, line: u32) -> Option<LineCoverage> {
        self.lines.get(&line).copied()
    }

    pub fn line_rate(&self) -> f64 {
        if self.lines.is_empty() {
            return 0.0;
        }
        let covered = self.lines.values().filter(|l| l.is_covered()).count();
        covered as f64 / self.lines.len() as f64
    }

    pub fn branch_rate(&self) -> f64 {
        if self.branches.is_empty() {
            return 0.0;
        }
        let taken = self.branches.iter().filter(|b| b.taken).count();
        taken as f64 / self.branches.len() as f64
    }

    pub fn function_rate(&self) -> f64 {
        if self.functions.is_empty() {
            return 0.0;
        }
        let covered = self.functions.iter().filter(|f| f.is_covered()).count();
        covered as f64 / self.functions.len() as f64
    }

    /// Marks partially-covered lines based on branch data.
    pub fn apply_branch_partial_coverage(&mut self) {
        let branch_lines: HashMap<u32, Vec<&BranchCoverage>> = {
            let mut map: HashMap<u32, Vec<&BranchCoverage>> = HashMap::new();
            for b in &self.branches {
                map.entry(b.line).or_default().push(b);
            }
            map
        };

        for (line, branches) in &branch_lines {
            let any_taken = branches.iter().any(|b| b.taken);
            let all_taken = branches.iter().all(|b| b.taken);
            if any_taken && !all_taken {
                self.lines.insert(*line, LineCoverage::Partial);
            }
        }
    }
}

// ── Coverage summary ─────────────────────────────────────────────────────────

/// Aggregate coverage statistics across all files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub total_lines: u32,
    pub covered_lines: u32,
    pub total_branches: u32,
    pub covered_branches: u32,
    pub total_functions: u32,
    pub covered_functions: u32,
}

impl CoverageSummary {
    pub fn line_percentage(&self) -> f64 {
        if self.total_lines == 0 {
            return 0.0;
        }
        self.covered_lines as f64 / self.total_lines as f64 * 100.0
    }

    pub fn branch_percentage(&self) -> f64 {
        if self.total_branches == 0 {
            return 0.0;
        }
        self.covered_branches as f64 / self.total_branches as f64 * 100.0
    }

    pub fn function_percentage(&self) -> f64 {
        if self.total_functions == 0 {
            return 0.0;
        }
        self.covered_functions as f64 / self.total_functions as f64 * 100.0
    }

    /// Status bar text like "Coverage: 82% lines, 74% branches".
    pub fn status_bar_text(&self) -> String {
        format!(
            "Coverage: {:.0}% lines, {:.0}% branches",
            self.line_percentage(),
            self.branch_percentage()
        )
    }
}

// ── Coverage data (top-level) ────────────────────────────────────────────────

/// Complete coverage data for a workspace.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CoverageData {
    pub files: HashMap<PathBuf, FileCoverage>,
    pub summary: CoverageSummary,
}

impl CoverageData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file_coverage(&self, path: &Path) -> Option<&FileCoverage> {
        self.files.get(path)
    }

    /// Recomputes the summary from individual file data.
    pub fn recompute_summary(&mut self) {
        let mut summary = CoverageSummary::default();
        for file_cov in self.files.values() {
            summary.total_lines += file_cov.lines.len() as u32;
            summary.covered_lines += file_cov
                .lines
                .values()
                .filter(|l| l.is_covered())
                .count() as u32;
            summary.total_branches += file_cov.branches.len() as u32;
            summary.covered_branches += file_cov
                .branches
                .iter()
                .filter(|b| b.taken)
                .count() as u32;
            summary.total_functions += file_cov.functions.len() as u32;
            summary.covered_functions += file_cov
                .functions
                .iter()
                .filter(|f| f.is_covered())
                .count() as u32;
        }
        self.summary = summary;
    }

    /// Merges another `CoverageData` into this one (useful for incremental updates).
    pub fn merge(&mut self, other: CoverageData) {
        for (path, file_cov) in other.files {
            self.files.insert(path, file_cov);
        }
        self.recompute_summary();
    }
}

// ── LCOV parser ──────────────────────────────────────────────────────────────

/// Parses LCOV-format coverage data.
///
/// LCOV is the standard format produced by `geninfo`/`lcov`, `cargo-tarpaulin`,
/// `istanbul report --reporter=lcovonly`, and many other tools.
pub fn parse_lcov(content: &str) -> Result<CoverageData, LcovParseError> {
    let mut data = CoverageData::new();
    let mut current_file: Option<PathBuf> = None;
    let mut current_cov = FileCoverage::new();

    for (line_no, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(path) = line.strip_prefix("SF:") {
            current_file = Some(PathBuf::from(path));
            current_cov = FileCoverage::new();
        } else if let Some(rest) = line.strip_prefix("DA:") {
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() >= 2 {
                let line_num: u32 = parts[0].parse().map_err(|_| LcovParseError {
                    line: line_no + 1,
                    message: format!("invalid line number: {}", parts[0]),
                })?;
                let hit_count: u32 = parts[1].parse().map_err(|_| LcovParseError {
                    line: line_no + 1,
                    message: format!("invalid hit count: {}", parts[1]),
                })?;
                let cov = if hit_count > 0 {
                    LineCoverage::Covered(hit_count)
                } else {
                    LineCoverage::Uncovered
                };
                current_cov.lines.insert(line_num, cov);
            }
        } else if let Some(rest) = line.strip_prefix("BRDA:") {
            let parts: Vec<&str> = rest.split(',').collect();
            if parts.len() >= 4 {
                let branch_line: u32 = parts[0].parse().unwrap_or(0);
                let block: u32 = parts[1].parse().unwrap_or(0);
                let branch: u32 = parts[2].parse().unwrap_or(0);
                let hits: u32 = if parts[3] == "-" {
                    0
                } else {
                    parts[3].parse().unwrap_or(0)
                };
                current_cov.branches.push(BranchCoverage {
                    line: branch_line,
                    block_number: block,
                    branch_number: branch,
                    taken: hits > 0,
                    hit_count: hits,
                });
            }
        } else if let Some(rest) = line.strip_prefix("FN:") {
            let parts: Vec<&str> = rest.splitn(2, ',').collect();
            if parts.len() == 2 {
                let fn_line: u32 = parts[0].parse().unwrap_or(0);
                current_cov.functions.push(FunctionCoverage {
                    name: parts[1].to_string(),
                    line: fn_line,
                    hit_count: 0,
                });
            }
        } else if let Some(rest) = line.strip_prefix("FNDA:") {
            let parts: Vec<&str> = rest.splitn(2, ',').collect();
            if parts.len() == 2 {
                let hits: u32 = parts[0].parse().unwrap_or(0);
                let name = parts[1];
                if let Some(func) = current_cov.functions.iter_mut().find(|f| f.name == name) {
                    func.hit_count = hits;
                }
            }
        } else if line == "end_of_record" {
            if let Some(ref path) = current_file {
                current_cov.apply_branch_partial_coverage();
                data.files.insert(path.clone(), current_cov.clone());
            }
            current_file = None;
            current_cov = FileCoverage::new();
        }
    }

    data.recompute_summary();
    Ok(data)
}

/// Error encountered while parsing LCOV data.
#[derive(Debug, Clone)]
pub struct LcovParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for LcovParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LCOV parse error at line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for LcovParseError {}

// ── Istanbul JSON parser ─────────────────────────────────────────────────────

/// Parses Istanbul/NYC JSON coverage output.
///
/// The format is a JSON object mapping file paths to coverage objects with
/// `statementMap`, `s`, `branchMap`, `b`, `fnMap`, `f` fields.
pub fn parse_istanbul(content: &str) -> Result<CoverageData, IstanbulParseError> {
    let root: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| IstanbulParseError(format!("invalid JSON: {e}")))?;

    let obj = root
        .as_object()
        .ok_or_else(|| IstanbulParseError("expected top-level object".into()))?;

    let mut data = CoverageData::new();

    for (file_path, file_val) in obj {
        let mut file_cov = FileCoverage::new();

        // Statement coverage → line coverage
        if let (Some(stmt_map), Some(s)) = (file_val.get("statementMap"), file_val.get("s")) {
            if let (Some(stmt_obj), Some(s_obj)) = (stmt_map.as_object(), s.as_object()) {
                for (key, range_val) in stmt_obj {
                    if let Some(start) = range_val.get("start").and_then(|s| s.get("line")).and_then(|l| l.as_u64()) {
                        let hits = s_obj
                            .get(key)
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u32;
                        let line = start as u32;
                        let cov = if hits > 0 {
                            LineCoverage::Covered(hits)
                        } else {
                            LineCoverage::Uncovered
                        };
                        file_cov.lines.entry(line).or_insert(cov);
                    }
                }
            }
        }

        // Branch coverage
        if let (Some(branch_map), Some(b)) = (file_val.get("branchMap"), file_val.get("b")) {
            if let (Some(bm_obj), Some(b_obj)) = (branch_map.as_object(), b.as_object()) {
                for (key, branch_val) in bm_obj {
                    let branch_line = branch_val
                        .get("loc")
                        .and_then(|l| l.get("start"))
                        .and_then(|s| s.get("line"))
                        .and_then(|l| l.as_u64())
                        .unwrap_or(0) as u32;

                    if let Some(counts) = b_obj.get(key).and_then(|v| v.as_array()) {
                        for (i, count_val) in counts.iter().enumerate() {
                            let hits = count_val.as_u64().unwrap_or(0) as u32;
                            file_cov.branches.push(BranchCoverage {
                                line: branch_line,
                                block_number: 0,
                                branch_number: i as u32,
                                taken: hits > 0,
                                hit_count: hits,
                            });
                        }
                    }
                }
            }
        }

        // Function coverage
        if let (Some(fn_map), Some(f)) = (file_val.get("fnMap"), file_val.get("f")) {
            if let (Some(fm_obj), Some(f_obj)) = (fn_map.as_object(), f.as_object()) {
                for (key, fn_val) in fm_obj {
                    let name = fn_val
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("(anonymous)")
                        .to_string();
                    let fn_line = fn_val
                        .get("loc")
                        .and_then(|l| l.get("start"))
                        .and_then(|s| s.get("line"))
                        .and_then(|l| l.as_u64())
                        .unwrap_or(0) as u32;
                    let hits = f_obj
                        .get(key)
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;

                    file_cov.functions.push(FunctionCoverage {
                        name,
                        line: fn_line,
                        hit_count: hits,
                    });
                }
            }
        }

        file_cov.apply_branch_partial_coverage();
        data.files.insert(PathBuf::from(file_path), file_cov);
    }

    data.recompute_summary();
    Ok(data)
}

/// Error encountered while parsing Istanbul JSON data.
#[derive(Debug, Clone)]
pub struct IstanbulParseError(pub String);

impl std::fmt::Display for IstanbulParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Istanbul parse error: {}", self.0)
    }
}

impl std::error::Error for IstanbulParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_coverage_queries() {
        assert!(LineCoverage::Covered(5).is_covered());
        assert!(!LineCoverage::Uncovered.is_covered());
        assert!(!LineCoverage::Partial.is_covered());
    }

    #[test]
    fn file_coverage_rates() {
        let mut fc = FileCoverage::new();
        fc.lines.insert(1, LineCoverage::Covered(1));
        fc.lines.insert(2, LineCoverage::Covered(3));
        fc.lines.insert(3, LineCoverage::Uncovered);
        fc.lines.insert(4, LineCoverage::Uncovered);

        assert!((fc.line_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn file_coverage_branch_rate() {
        let mut fc = FileCoverage::new();
        fc.branches.push(BranchCoverage {
            line: 5,
            block_number: 0,
            branch_number: 0,
            taken: true,
            hit_count: 1,
        });
        fc.branches.push(BranchCoverage {
            line: 5,
            block_number: 0,
            branch_number: 1,
            taken: false,
            hit_count: 0,
        });
        assert!((fc.branch_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn summary_percentages() {
        let summary = CoverageSummary {
            total_lines: 100,
            covered_lines: 82,
            total_branches: 50,
            covered_branches: 37,
            total_functions: 20,
            covered_functions: 18,
        };
        assert!((summary.line_percentage() - 82.0).abs() < f64::EPSILON);
        assert!((summary.branch_percentage() - 74.0).abs() < f64::EPSILON);
        assert!((summary.function_percentage() - 90.0).abs() < f64::EPSILON);
        assert_eq!(summary.status_bar_text(), "Coverage: 82% lines, 74% branches");
    }

    #[test]
    fn summary_zero_totals() {
        let summary = CoverageSummary::default();
        assert!((summary.line_percentage() - 0.0).abs() < f64::EPSILON);
        assert!((summary.branch_percentage() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_lcov_basic() {
        let lcov = "\
SF:/src/main.rs
FN:1,main
FNDA:5,main
DA:1,5
DA:2,5
DA:3,0
BRDA:2,0,0,5
BRDA:2,0,1,0
end_of_record
";
        let data = parse_lcov(lcov).unwrap();
        assert_eq!(data.files.len(), 1);

        let fc = data.file_coverage(Path::new("/src/main.rs")).unwrap();
        assert_eq!(fc.lines.len(), 3);
        assert_eq!(fc.lines[&1], LineCoverage::Covered(5));
        assert_eq!(fc.lines[&3], LineCoverage::Uncovered);
        assert_eq!(fc.lines[&2], LineCoverage::Partial);

        assert_eq!(fc.functions.len(), 1);
        assert_eq!(fc.functions[0].hit_count, 5);

        assert_eq!(data.summary.total_lines, 3);
        assert_eq!(data.summary.covered_lines, 1);
        assert_eq!(data.summary.total_branches, 2);
        assert_eq!(data.summary.covered_branches, 1);
    }

    #[test]
    fn parse_lcov_multiple_files() {
        let lcov = "\
SF:/src/a.rs
DA:1,1
end_of_record
SF:/src/b.rs
DA:1,0
DA:2,0
end_of_record
";
        let data = parse_lcov(lcov).unwrap();
        assert_eq!(data.files.len(), 2);
        assert_eq!(data.summary.total_lines, 3);
        assert_eq!(data.summary.covered_lines, 1);
    }

    #[test]
    fn parse_lcov_invalid_line_number() {
        let lcov = "SF:/src/a.rs\nDA:abc,1\nend_of_record\n";
        assert!(parse_lcov(lcov).is_err());
    }

    #[test]
    fn parse_istanbul_basic() {
        let istanbul = r#"{
            "/src/app.js": {
                "statementMap": {
                    "0": { "start": { "line": 1, "column": 0 }, "end": { "line": 1, "column": 20 } },
                    "1": { "start": { "line": 2, "column": 0 }, "end": { "line": 2, "column": 20 } }
                },
                "s": { "0": 5, "1": 0 },
                "branchMap": {},
                "b": {},
                "fnMap": {
                    "0": { "name": "render", "loc": { "start": { "line": 1, "column": 0 }, "end": { "line": 3, "column": 1 } } }
                },
                "f": { "0": 5 }
            }
        }"#;

        let data = parse_istanbul(istanbul).unwrap();
        assert_eq!(data.files.len(), 1);
        let fc = data.file_coverage(Path::new("/src/app.js")).unwrap();
        assert_eq!(fc.lines.len(), 2);
        assert_eq!(fc.lines[&1], LineCoverage::Covered(5));
        assert_eq!(fc.lines[&2], LineCoverage::Uncovered);
        assert_eq!(fc.functions.len(), 1);
        assert_eq!(fc.functions[0].name, "render");
        assert_eq!(fc.functions[0].hit_count, 5);
    }

    #[test]
    fn parse_istanbul_invalid_json() {
        assert!(parse_istanbul("not json").is_err());
    }

    #[test]
    fn coverage_data_merge() {
        let mut a = CoverageData::new();
        let mut fa = FileCoverage::new();
        fa.lines.insert(1, LineCoverage::Covered(1));
        a.files.insert(PathBuf::from("/a.rs"), fa);
        a.recompute_summary();

        let mut b = CoverageData::new();
        let mut fb = FileCoverage::new();
        fb.lines.insert(1, LineCoverage::Uncovered);
        fb.lines.insert(2, LineCoverage::Covered(1));
        b.files.insert(PathBuf::from("/b.rs"), fb);
        b.recompute_summary();

        a.merge(b);
        assert_eq!(a.files.len(), 2);
        assert_eq!(a.summary.total_lines, 3);
        assert_eq!(a.summary.covered_lines, 2);
    }

    #[test]
    fn partial_coverage_from_branches() {
        let mut fc = FileCoverage::new();
        fc.lines.insert(5, LineCoverage::Covered(1));
        fc.branches.push(BranchCoverage {
            line: 5,
            block_number: 0,
            branch_number: 0,
            taken: true,
            hit_count: 1,
        });
        fc.branches.push(BranchCoverage {
            line: 5,
            block_number: 0,
            branch_number: 1,
            taken: false,
            hit_count: 0,
        });
        fc.apply_branch_partial_coverage();
        assert_eq!(fc.lines[&5], LineCoverage::Partial);
    }
}
