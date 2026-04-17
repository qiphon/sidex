//! Three-way merge model.
//!
//! Implements the VS Code merge editor concept: given a **base** document and
//! two divergent inputs (**input1** / ours and **input2** / theirs), detect
//! conflicts and produce a merged **result** document. Each conflict can be
//! independently resolved.

use serde::{Deserialize, Serialize};

use super::diff_model::{compute_diff, DiffResult, LineRange};
use crate::document::Document;

/// How a single conflict was resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeResolution {
    /// Use the text from input1 (ours).
    AcceptInput1,
    /// Use the text from input2 (theirs).
    AcceptInput2,
    /// Concatenate both inputs (input1 first, then input2).
    AcceptBoth,
    /// Concatenate both inputs (input2 first, then input1).
    AcceptBothReversed,
    /// User-provided custom text.
    Custom(String),
}

/// A region where input1 and input2 both modified the base differently.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    /// Unique identifier for this conflict.
    pub id: u32,
    /// The affected range in the base document.
    pub base_range: LineRange,
    /// The affected range in input1.
    pub input1_range: LineRange,
    /// The affected range in input2.
    pub input2_range: LineRange,
    /// Whether this conflict has been resolved.
    pub is_resolved: bool,
    /// The chosen resolution (meaningful only when `is_resolved` is true).
    pub resolution: MergeResolution,
    /// Word-level change offsets within input1 text (byte ranges).
    pub input1_word_changes: Vec<(usize, usize)>,
    /// Word-level change offsets within input2 text (byte ranges).
    pub input2_word_changes: Vec<(usize, usize)>,
}

/// Three-way merge editor: base + input1 (ours) + input2 (theirs) → result.
pub struct MergeEditor {
    /// The common ancestor document.
    pub base: Document,
    /// "Ours" — typically the current branch.
    pub input1: Document,
    /// "Theirs" — typically the incoming branch.
    pub input2: Document,
    /// The merged result document.
    pub result: Document,
    /// Detected conflicts.
    pub conflicts: Vec<MergeConflict>,
    /// Diff from base → input1.
    diff1: DiffResult,
    /// Diff from base → input2.
    diff2: DiffResult,
}

impl MergeEditor {
    /// Create a new merge editor and auto-detect conflicts.
    pub fn new(base: Document, input1: Document, input2: Document) -> Self {
        let diff1 = compute_diff(&base.buffer, &input1.buffer);
        let diff2 = compute_diff(&base.buffer, &input2.buffer);
        let conflicts = detect_conflicts(&diff1, &diff2);

        let result = build_initial_result(&base, &input1, &input2, &diff1, &diff2, &conflicts);

        Self {
            base,
            input1,
            input2,
            result,
            conflicts,
            diff1,
            diff2,
        }
    }

    /// Resolve conflict at `idx` with the given resolution.
    pub fn resolve_conflict(&mut self, idx: usize, resolution: MergeResolution) {
        if idx >= self.conflicts.len() {
            return;
        }
        self.conflicts[idx].resolution = resolution;
        self.conflicts[idx].is_resolved = true;
        self.rebuild_result();
    }

    /// Access the merged result document.
    pub fn result_document(&self) -> &Document {
        &self.result
    }

    /// Returns only unresolved conflicts.
    pub fn unresolved_conflicts(&self) -> Vec<&MergeConflict> {
        self.conflicts.iter().filter(|c| !c.is_resolved).collect()
    }

    /// Whether every conflict has been resolved.
    pub fn is_fully_resolved(&self) -> bool {
        self.conflicts.iter().all(|c| c.is_resolved)
    }

    /// Number of total conflicts detected.
    pub fn conflict_count(&self) -> usize {
        self.conflicts.len()
    }

    /// Diff from base to input1.
    pub fn diff_input1(&self) -> &DiffResult {
        &self.diff1
    }

    /// Diff from base to input2.
    pub fn diff_input2(&self) -> &DiffResult {
        &self.diff2
    }

    /// Rebuild the result document from scratch based on current resolutions.
    fn rebuild_result(&mut self) {
        self.result = build_initial_result(
            &self.base,
            &self.input1,
            &self.input2,
            &self.diff1,
            &self.diff2,
            &self.conflicts,
        );
    }

    // ── Extended merge features ───────────────────────────────────

    /// Number of resolved conflicts.
    pub fn resolved_count(&self) -> usize {
        self.conflicts.iter().filter(|c| c.is_resolved).count()
    }

    /// Status label like "3 of 7 conflicts remaining".
    pub fn status_label(&self) -> String {
        let total = self.conflicts.len();
        let remaining = total - self.resolved_count();
        if remaining == 0 {
            "All conflicts resolved".to_string()
        } else {
            format!("{remaining} of {total} conflicts remaining")
        }
    }

    /// Accept all conflicts using input1 (current/ours).
    pub fn accept_all_input1(&mut self) {
        for idx in 0..self.conflicts.len() {
            if !self.conflicts[idx].is_resolved {
                self.conflicts[idx].resolution = MergeResolution::AcceptInput1;
                self.conflicts[idx].is_resolved = true;
            }
        }
        self.rebuild_result();
    }

    /// Accept all conflicts using input2 (incoming/theirs).
    pub fn accept_all_input2(&mut self) {
        for idx in 0..self.conflicts.len() {
            if !self.conflicts[idx].is_resolved {
                self.conflicts[idx].resolution = MergeResolution::AcceptInput2;
                self.conflicts[idx].is_resolved = true;
            }
        }
        self.rebuild_result();
    }

    /// Navigate to the next unresolved conflict after `current_idx`.
    pub fn next_unresolved(&self, current_idx: Option<usize>) -> Option<usize> {
        let start = current_idx.map_or(0, |i| i + 1);
        for i in start..self.conflicts.len() {
            if !self.conflicts[i].is_resolved {
                return Some(i);
            }
        }
        for i in 0..start.min(self.conflicts.len()) {
            if !self.conflicts[i].is_resolved {
                return Some(i);
            }
        }
        None
    }

    /// Navigate to the previous unresolved conflict before `current_idx`.
    pub fn prev_unresolved(&self, current_idx: Option<usize>) -> Option<usize> {
        let start = current_idx.unwrap_or(self.conflicts.len());
        if start > 0 {
            for i in (0..start).rev() {
                if !self.conflicts[i].is_resolved {
                    return Some(i);
                }
            }
        }
        for i in (start..self.conflicts.len()).rev() {
            if !self.conflicts[i].is_resolved {
                return Some(i);
            }
        }
        None
    }

    /// Get the text for a conflict from input1.
    pub fn input1_text(&self, conflict_idx: usize) -> String {
        if conflict_idx >= self.conflicts.len() {
            return String::new();
        }
        let c = &self.conflicts[conflict_idx];
        let lines = buffer_lines(&self.input1.buffer);
        lines[c.input1_range.start..c.input1_range.end().min(lines.len())].join("\n")
    }

    /// Get the text for a conflict from input2.
    pub fn input2_text(&self, conflict_idx: usize) -> String {
        if conflict_idx >= self.conflicts.len() {
            return String::new();
        }
        let c = &self.conflicts[conflict_idx];
        let lines = buffer_lines(&self.input2.buffer);
        lines[c.input2_range.start..c.input2_range.end().min(lines.len())].join("\n")
    }

    /// Get the text for a conflict from base.
    pub fn base_text(&self, conflict_idx: usize) -> String {
        if conflict_idx >= self.conflicts.len() {
            return String::new();
        }
        let c = &self.conflicts[conflict_idx];
        let lines = buffer_lines(&self.base.buffer);
        lines[c.base_range.start..c.base_range.end().min(lines.len())].join("\n")
    }

    /// Apply the merged result text (for direct editing of the result pane).
    pub fn set_result_text(&mut self, text: &str) {
        self.result = Document::from_str(text);
    }

    /// Get the full result text.
    pub fn result_text(&self) -> String {
        self.result.text()
    }
}

// ── Conflict detection ───────────────────────────────────────────────

/// Detect conflicts by finding overlapping change regions between the two
/// diffs against the base. Two changes conflict when their base-side
/// ranges overlap and the modified text is different.
fn detect_conflicts(diff1: &DiffResult, diff2: &DiffResult) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();
    let mut next_id: u32 = 0;

    for c1 in &diff1.changes {
        for c2 in &diff2.changes {
            if ranges_overlap(&c1.original_range, &c2.original_range) {
                let base_range = merge_ranges(&c1.original_range, &c2.original_range);
                conflicts.push(MergeConflict {
                    id: next_id,
                    base_range,
                    input1_range: c1.modified_range,
                    input2_range: c2.modified_range,
                    is_resolved: false,
                    resolution: MergeResolution::AcceptInput1,
                    input1_word_changes: Vec::new(),
                    input2_word_changes: Vec::new(),
                });
                next_id += 1;
            }
        }
    }

    // Sort by base position and deduplicate overlapping conflicts.
    conflicts.sort_by_key(|c| c.base_range.start);
    dedup_conflicts(&mut conflicts);
    conflicts
}

fn ranges_overlap(a: &LineRange, b: &LineRange) -> bool {
    // Two ranges overlap if neither ends before the other starts.
    // Empty ranges at the same position also overlap (adjacent insertions).
    a.start < b.end() && b.start < a.end()
        || (a.is_empty() && b.is_empty() && a.start == b.start)
        || (a.is_empty() && a.start >= b.start && a.start < b.end())
        || (b.is_empty() && b.start >= a.start && b.start < a.end())
}

fn merge_ranges(a: &LineRange, b: &LineRange) -> LineRange {
    let start = a.start.min(b.start);
    let end = a.end().max(b.end());
    LineRange::new(start, end - start)
}

fn dedup_conflicts(conflicts: &mut Vec<MergeConflict>) {
    if conflicts.len() <= 1 {
        return;
    }
    let mut i = 0;
    while i + 1 < conflicts.len() {
        if ranges_overlap(&conflicts[i].base_range, &conflicts[i + 1].base_range) {
            let merged_base = merge_ranges(&conflicts[i].base_range, &conflicts[i + 1].base_range);
            let merged_in1 =
                merge_ranges(&conflicts[i].input1_range, &conflicts[i + 1].input1_range);
            let merged_in2 =
                merge_ranges(&conflicts[i].input2_range, &conflicts[i + 1].input2_range);
            conflicts[i].base_range = merged_base;
            conflicts[i].input1_range = merged_in1;
            conflicts[i].input2_range = merged_in2;
            conflicts.remove(i + 1);
        } else {
            i += 1;
        }
    }
    for (idx, c) in conflicts.iter_mut().enumerate() {
        c.id = idx as u32;
    }
}

// ── Result document construction ─────────────────────────────────────

/// Build the initial result by applying non-conflicting changes and
/// using the resolution strategy for conflicts.
fn build_initial_result(
    base: &Document,
    input1: &Document,
    input2: &Document,
    diff1: &DiffResult,
    diff2: &DiffResult,
    conflicts: &[MergeConflict],
) -> Document {
    let base_lines = buffer_lines(&base.buffer);
    let input1_lines = buffer_lines(&input1.buffer);
    let input2_lines = buffer_lines(&input2.buffer);

    // Collect base-line regions that are part of a conflict.
    let conflict_base_regions: Vec<(usize, usize)> = conflicts
        .iter()
        .map(|c| (c.base_range.start, c.base_range.end()))
        .collect();

    // Collect non-conflicting changes from each side.
    let nc1 = non_conflicting_changes(diff1, &conflict_base_regions);
    let nc2 = non_conflicting_changes(diff2, &conflict_base_regions);

    let mut result_lines: Vec<String> = Vec::new();
    let mut base_idx: usize = 0;

    // Merge all events (non-conflicting changes from both sides + conflicts)
    // sorted by their base position.
    let mut events: Vec<MergeEvent> = Vec::new();

    for c in &nc1 {
        events.push(MergeEvent {
            base_start: c.original_range.start,
            base_end: c.original_range.end(),
            source: EventSource::Input1,
            modified_range: c.modified_range,
            resolution: None,
        });
    }
    for c in &nc2 {
        events.push(MergeEvent {
            base_start: c.original_range.start,
            base_end: c.original_range.end(),
            source: EventSource::Input2,
            modified_range: c.modified_range,
            resolution: None,
        });
    }
    for c in conflicts {
        events.push(MergeEvent {
            base_start: c.base_range.start,
            base_end: c.base_range.end(),
            source: EventSource::Conflict,
            modified_range: LineRange::new(0, 0), // unused
            resolution: Some(c),
        });
    }

    events.sort_by_key(|e| e.base_start);

    for event in &events {
        // Copy unchanged base lines before this event.
        while base_idx < event.base_start && base_idx < base_lines.len() {
            result_lines.push(base_lines[base_idx].clone());
            base_idx += 1;
        }

        match event.source {
            EventSource::Input1 => {
                for i in event.modified_range.start..event.modified_range.end() {
                    if i < input1_lines.len() {
                        result_lines.push(input1_lines[i].clone());
                    }
                }
            }
            EventSource::Input2 => {
                for i in event.modified_range.start..event.modified_range.end() {
                    if i < input2_lines.len() {
                        result_lines.push(input2_lines[i].clone());
                    }
                }
            }
            EventSource::Conflict => {
                if let Some(conflict) = event.resolution {
                    apply_conflict_resolution(
                        conflict,
                        &input1_lines,
                        &input2_lines,
                        &mut result_lines,
                    );
                }
            }
        }

        base_idx = event.base_end;
    }

    // Copy remaining base lines.
    while base_idx < base_lines.len() {
        result_lines.push(base_lines[base_idx].clone());
        base_idx += 1;
    }

    let text = result_lines.join("\n");
    Document::from_str(&text)
}

fn apply_conflict_resolution(
    conflict: &MergeConflict,
    input1_lines: &[String],
    input2_lines: &[String],
    result: &mut Vec<String>,
) {
    if !conflict.is_resolved {
        // Unresolved — insert conflict markers.
        result.push("<<<<<<< input1".to_string());
        for i in conflict.input1_range.start..conflict.input1_range.end() {
            if i < input1_lines.len() {
                result.push(input1_lines[i].clone());
            }
        }
        result.push("=======".to_string());
        for i in conflict.input2_range.start..conflict.input2_range.end() {
            if i < input2_lines.len() {
                result.push(input2_lines[i].clone());
            }
        }
        result.push(">>>>>>> input2".to_string());
        return;
    }

    match &conflict.resolution {
        MergeResolution::AcceptInput1 => {
            for i in conflict.input1_range.start..conflict.input1_range.end() {
                if i < input1_lines.len() {
                    result.push(input1_lines[i].clone());
                }
            }
        }
        MergeResolution::AcceptInput2 => {
            for i in conflict.input2_range.start..conflict.input2_range.end() {
                if i < input2_lines.len() {
                    result.push(input2_lines[i].clone());
                }
            }
        }
        MergeResolution::AcceptBoth => {
            for i in conflict.input1_range.start..conflict.input1_range.end() {
                if i < input1_lines.len() {
                    result.push(input1_lines[i].clone());
                }
            }
            for i in conflict.input2_range.start..conflict.input2_range.end() {
                if i < input2_lines.len() {
                    result.push(input2_lines[i].clone());
                }
            }
        }
        MergeResolution::AcceptBothReversed => {
            for i in conflict.input2_range.start..conflict.input2_range.end() {
                if i < input2_lines.len() {
                    result.push(input2_lines[i].clone());
                }
            }
            for i in conflict.input1_range.start..conflict.input1_range.end() {
                if i < input1_lines.len() {
                    result.push(input1_lines[i].clone());
                }
            }
        }
        MergeResolution::Custom(text) => {
            for line in text.lines() {
                result.push(line.to_string());
            }
        }
    }
}

struct MergeEvent<'a> {
    base_start: usize,
    base_end: usize,
    source: EventSource,
    modified_range: LineRange,
    resolution: Option<&'a MergeConflict>,
}

#[derive(Debug, Clone, Copy)]
enum EventSource {
    Input1,
    Input2,
    Conflict,
}

/// Filter changes that do NOT overlap any conflict region.
fn non_conflicting_changes(
    diff: &DiffResult,
    conflict_regions: &[(usize, usize)],
) -> Vec<super::diff_model::DiffChange> {
    diff.changes
        .iter()
        .filter(|c| {
            let cs = c.original_range.start;
            let ce = c.original_range.end();
            !conflict_regions.iter().any(|&(rs, re)| cs < re && rs < ce)
        })
        .cloned()
        .collect()
}

/// Extract all lines from a buffer as owned strings (without trailing newlines).
fn buffer_lines(buf: &sidex_text::Buffer) -> Vec<String> {
    (0..buf.len_lines())
        .map(|i| {
            buf.line_content(i)
                .trim_end_matches(&['\n', '\r'][..])
                .to_string()
        })
        .collect()
}

// ── Conflict marker parsing ──────────────────────────────────────

/// A region detected from standard git conflict markers in file content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictMarkerRegion {
    /// Line number of the `<<<<<<<` marker (0-based).
    pub current_start: u32,
    /// Line number of the `=======` separator (0-based).
    pub separator: u32,
    /// Line number of the `>>>>>>>` marker (0-based).
    pub incoming_end: u32,
    /// Label from the `<<<<<<<` marker (e.g., branch name).
    pub current_label: String,
    /// Label from the `>>>>>>>` marker.
    pub incoming_label: String,
}

/// Detect conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) in content.
pub fn parse_conflict_markers(content: &str) -> Vec<ConflictMarkerRegion> {
    let mut regions = Vec::new();
    let mut current_start: Option<(u32, String)> = None;
    let mut separator: Option<u32> = None;

    for (idx, line) in content.lines().enumerate() {
        let line_num = idx as u32;
        let trimmed = line.trim();

        if trimmed.starts_with("<<<<<<<") {
            let label = trimmed
                .strip_prefix("<<<<<<<")
                .unwrap_or("")
                .trim()
                .to_string();
            current_start = Some((line_num, label));
            separator = None;
        } else if trimmed.starts_with("=======") && current_start.is_some() {
            separator = Some(line_num);
        } else if trimmed.starts_with(">>>>>>>") {
            if let (Some((start, current_label)), Some(sep)) = (current_start.take(), separator.take()) {
                let incoming_label = trimmed
                    .strip_prefix(">>>>>>>")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                regions.push(ConflictMarkerRegion {
                    current_start: start,
                    separator: sep,
                    incoming_end: line_num,
                    current_label,
                    incoming_label,
                });
            }
        }
    }
    regions
}

/// Build a resolved string from conflict regions by applying per-region resolution choices.
pub fn apply_resolution(
    content: &str,
    regions: &[ConflictMarkerRegion],
    resolutions: &[MergeResolution],
) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut skip_until: Option<u32> = None;

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx as u32;

        if let Some(skip) = skip_until {
            if line_num <= skip {
                continue;
            }
            skip_until = None;
        }

        if let Some(region_idx) = regions
            .iter()
            .position(|r| r.current_start == line_num)
        {
            let region = &regions[region_idx];
            let resolution = resolutions
                .get(region_idx)
                .unwrap_or(&MergeResolution::AcceptInput1);

            let current_lines: Vec<&str> = lines
                [(region.current_start as usize + 1)..region.separator as usize]
                .to_vec();
            let incoming_lines: Vec<&str> = lines
                [(region.separator as usize + 1)..region.incoming_end as usize]
                .to_vec();

            match resolution {
                MergeResolution::AcceptInput1 => {
                    for l in &current_lines {
                        result.push(l.to_string());
                    }
                }
                MergeResolution::AcceptInput2 => {
                    for l in &incoming_lines {
                        result.push(l.to_string());
                    }
                }
                MergeResolution::AcceptBoth => {
                    for l in &current_lines {
                        result.push(l.to_string());
                    }
                    for l in &incoming_lines {
                        result.push(l.to_string());
                    }
                }
                MergeResolution::AcceptBothReversed => {
                    for l in &incoming_lines {
                        result.push(l.to_string());
                    }
                    for l in &current_lines {
                        result.push(l.to_string());
                    }
                }
                MergeResolution::Custom(text) => {
                    for l in text.lines() {
                        result.push(l.to_string());
                    }
                }
            }
            skip_until = Some(region.incoming_end);
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> Document {
        Document::from_str(s)
    }

    #[test]
    fn no_conflicts_when_disjoint_changes() {
        let base = doc("aaa\nbbb\nccc\nddd\n");
        let input1 = doc("AAA\nbbb\nccc\nddd\n"); // changed line 0
        let input2 = doc("aaa\nbbb\nccc\nDDD\n"); // changed line 3
        let me = MergeEditor::new(base, input1, input2);
        assert_eq!(me.conflict_count(), 0);
        assert!(me.is_fully_resolved());
    }

    #[test]
    fn conflict_when_same_line_changed() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let me = MergeEditor::new(base, input1, input2);
        assert_eq!(me.conflict_count(), 1);
        assert!(!me.is_fully_resolved());
    }

    #[test]
    fn resolve_accept_input1() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let mut me = MergeEditor::new(base, input1, input2);
        me.resolve_conflict(0, MergeResolution::AcceptInput1);
        assert!(me.is_fully_resolved());
        let result_text = me.result_document().text();
        assert!(result_text.contains("XXX"));
        assert!(!result_text.contains("YYY"));
    }

    #[test]
    fn resolve_accept_input2() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let mut me = MergeEditor::new(base, input1, input2);
        me.resolve_conflict(0, MergeResolution::AcceptInput2);
        assert!(me.is_fully_resolved());
        let result_text = me.result_document().text();
        assert!(result_text.contains("YYY"));
        assert!(!result_text.contains("XXX"));
    }

    #[test]
    fn resolve_accept_both() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let mut me = MergeEditor::new(base, input1, input2);
        me.resolve_conflict(0, MergeResolution::AcceptBoth);
        let result_text = me.result_document().text();
        assert!(result_text.contains("XXX"));
        assert!(result_text.contains("YYY"));
    }

    #[test]
    fn resolve_custom() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let mut me = MergeEditor::new(base, input1, input2);
        me.resolve_conflict(0, MergeResolution::Custom("CUSTOM".to_string()));
        let result_text = me.result_document().text();
        assert!(result_text.contains("CUSTOM"));
    }

    #[test]
    fn unresolved_contains_conflict_markers() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let me = MergeEditor::new(base, input1, input2);
        let result_text = me.result_document().text();
        assert!(result_text.contains("<<<<<<<"));
        assert!(result_text.contains(">>>>>>>"));
    }

    #[test]
    fn identical_inputs_no_conflicts() {
        let base = doc("aaa\nbbb\n");
        let input1 = doc("aaa\nbbb\n");
        let input2 = doc("aaa\nbbb\n");
        let me = MergeEditor::new(base, input1, input2);
        assert_eq!(me.conflict_count(), 0);
        assert!(me.is_fully_resolved());
    }

    #[test]
    fn ranges_overlap_correctness() {
        assert!(ranges_overlap(&LineRange::new(1, 2), &LineRange::new(2, 2)));
        assert!(!ranges_overlap(
            &LineRange::new(0, 1),
            &LineRange::new(2, 1)
        ));
        assert!(ranges_overlap(&LineRange::new(1, 3), &LineRange::new(2, 1)));
    }

    #[test]
    fn status_label_with_unresolved() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let me = MergeEditor::new(base, input1, input2);
        assert_eq!(me.status_label(), "1 of 1 conflicts remaining");
    }

    #[test]
    fn accept_all_input1() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let mut me = MergeEditor::new(base, input1, input2);
        me.accept_all_input1();
        assert!(me.is_fully_resolved());
        assert!(me.result_document().text().contains("XXX"));
    }

    #[test]
    fn accept_all_input2() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let mut me = MergeEditor::new(base, input1, input2);
        me.accept_all_input2();
        assert!(me.is_fully_resolved());
        assert!(me.result_document().text().contains("YYY"));
    }

    #[test]
    fn resolve_accept_both_reversed() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let mut me = MergeEditor::new(base, input1, input2);
        me.resolve_conflict(0, MergeResolution::AcceptBothReversed);
        let result_text = me.result_document().text();
        let yyy_pos = result_text.find("YYY").unwrap();
        let xxx_pos = result_text.find("XXX").unwrap();
        assert!(yyy_pos < xxx_pos);
    }

    #[test]
    fn next_unresolved_navigation() {
        let base = doc("aaa\nbbb\nccc\n");
        let input1 = doc("aaa\nXXX\nccc\n");
        let input2 = doc("aaa\nYYY\nccc\n");
        let me = MergeEditor::new(base, input1, input2);
        assert_eq!(me.next_unresolved(None), Some(0));
    }

    #[test]
    fn parse_conflict_markers_basic() {
        let content = "before\n<<<<<<< HEAD\ncurrent\n=======\nincoming\n>>>>>>> branch\nafter";
        let regions = parse_conflict_markers(content);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].current_start, 1);
        assert_eq!(regions[0].separator, 3);
        assert_eq!(regions[0].incoming_end, 5);
        assert_eq!(regions[0].current_label, "HEAD");
        assert_eq!(regions[0].incoming_label, "branch");
    }

    #[test]
    fn apply_resolution_accept_current() {
        let content = "before\n<<<<<<< HEAD\ncurrent\n=======\nincoming\n>>>>>>> branch\nafter";
        let regions = parse_conflict_markers(content);
        let result = apply_resolution(content, &regions, &[MergeResolution::AcceptInput1]);
        assert!(result.contains("current"));
        assert!(!result.contains("incoming"));
        assert!(result.contains("before"));
        assert!(result.contains("after"));
    }

    #[test]
    fn apply_resolution_accept_incoming() {
        let content = "before\n<<<<<<< HEAD\ncurrent\n=======\nincoming\n>>>>>>> branch\nafter";
        let regions = parse_conflict_markers(content);
        let result = apply_resolution(content, &regions, &[MergeResolution::AcceptInput2]);
        assert!(!result.contains("current"));
        assert!(result.contains("incoming"));
    }
}
