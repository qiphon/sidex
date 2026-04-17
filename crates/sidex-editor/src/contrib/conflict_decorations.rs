//! Conflict marker decorations for the editor.
//!
//! When a file contains standard git conflict markers (`<<<<<<<`, `=======`,
//! `>>>>>>>`), this module detects them and provides decoration data for
//! rendering colored backgrounds, codelens actions, and navigation.

use serde::{Deserialize, Serialize};

use crate::diff::merge_model::{parse_conflict_markers, ConflictMarkerRegion, MergeResolution};

/// Background color for a conflict region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictRegionKind {
    /// "Current Change" — green background.
    Current,
    /// "Incoming Change" — blue background.
    Incoming,
    /// Separator line.
    Separator,
    /// Marker lines themselves.
    Marker,
}

/// A decoration to render in the editor for a conflict region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictDecoration {
    /// 0-based start line.
    pub start_line: u32,
    /// 0-based end line (inclusive).
    pub end_line: u32,
    /// What kind of region this is.
    pub kind: ConflictRegionKind,
}

/// An action available via codelens above a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictAction {
    AcceptCurrent,
    AcceptIncoming,
    AcceptBoth,
    CompareChanges,
}

/// A codelens entry positioned above a conflict marker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictCodeLens {
    /// 0-based line where the codelens appears.
    pub line: u32,
    /// Available actions for this conflict.
    pub actions: Vec<ConflictCodeLensAction>,
}

/// A single codelens action label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictCodeLensAction {
    pub label: String,
    pub action: ConflictAction,
    /// Index of the conflict region this action belongs to.
    pub region_index: usize,
}

/// Full decoration state for conflict markers in a file.
#[derive(Debug, Clone, Default)]
pub struct ConflictMarkerDecoration {
    pub regions: Vec<ConflictMarkerRegion>,
    pub decorations: Vec<ConflictDecoration>,
    pub codelens: Vec<ConflictCodeLens>,
}

impl ConflictMarkerDecoration {
    /// Detect conflict markers in content and produce decorations.
    pub fn detect(content: &str) -> Self {
        let regions = detect_conflict_markers(content);
        let decorations = build_decorations(&regions);
        let codelens = build_codelens(&regions);
        Self {
            regions,
            decorations,
            codelens,
        }
    }

    /// Whether the file has any conflict markers.
    pub fn has_conflicts(&self) -> bool {
        !self.regions.is_empty()
    }

    /// Number of detected conflict regions.
    pub fn conflict_count(&self) -> usize {
        self.regions.len()
    }

    /// Navigate to the next conflict after the given line.
    pub fn next_conflict_after(&self, line: u32) -> Option<&ConflictMarkerRegion> {
        self.regions
            .iter()
            .find(|r| r.current_start > line)
            .or_else(|| self.regions.first())
    }

    /// Navigate to the previous conflict before the given line.
    pub fn prev_conflict_before(&self, line: u32) -> Option<&ConflictMarkerRegion> {
        self.regions
            .iter()
            .rev()
            .find(|r| r.current_start < line)
            .or_else(|| self.regions.last())
    }

    /// Find the conflict region containing the given line.
    pub fn conflict_at_line(&self, line: u32) -> Option<(usize, &ConflictMarkerRegion)> {
        self.regions
            .iter()
            .enumerate()
            .find(|(_, r)| line >= r.current_start && line <= r.incoming_end)
    }

    /// Apply a resolution to one conflict and return the updated file content.
    pub fn apply_single_resolution(
        &self,
        content: &str,
        region_index: usize,
        resolution: &MergeResolution,
    ) -> String {
        if region_index >= self.regions.len() {
            return content.to_string();
        }
        let mut resolutions: Vec<MergeResolution> = self
            .regions
            .iter()
            .map(|_| MergeResolution::Custom(String::new()))
            .collect();

        let lines: Vec<&str> = content.lines().collect();
        for (idx, region) in self.regions.iter().enumerate() {
            if idx == region_index {
                resolutions[idx] = resolution.clone();
            } else {
                let start = region.current_start as usize;
                let end = region.incoming_end as usize;
                let region_text: Vec<&str> = lines[start..=end].to_vec();
                resolutions[idx] = MergeResolution::Custom(region_text.join("\n"));
            }
        }

        crate::diff::merge_model::apply_resolution(content, &self.regions, &resolutions)
    }
}

/// Detect conflict markers in file content.
pub fn detect_conflict_markers(content: &str) -> Vec<ConflictMarkerRegion> {
    parse_conflict_markers(content)
}

fn build_decorations(regions: &[ConflictMarkerRegion]) -> Vec<ConflictDecoration> {
    let mut decorations = Vec::new();

    for region in regions {
        decorations.push(ConflictDecoration {
            start_line: region.current_start,
            end_line: region.current_start,
            kind: ConflictRegionKind::Marker,
        });

        if region.current_start + 1 < region.separator {
            decorations.push(ConflictDecoration {
                start_line: region.current_start + 1,
                end_line: region.separator - 1,
                kind: ConflictRegionKind::Current,
            });
        }

        decorations.push(ConflictDecoration {
            start_line: region.separator,
            end_line: region.separator,
            kind: ConflictRegionKind::Separator,
        });

        if region.separator + 1 < region.incoming_end {
            decorations.push(ConflictDecoration {
                start_line: region.separator + 1,
                end_line: region.incoming_end - 1,
                kind: ConflictRegionKind::Incoming,
            });
        }

        decorations.push(ConflictDecoration {
            start_line: region.incoming_end,
            end_line: region.incoming_end,
            kind: ConflictRegionKind::Marker,
        });
    }

    decorations
}

fn build_codelens(regions: &[ConflictMarkerRegion]) -> Vec<ConflictCodeLens> {
    regions
        .iter()
        .enumerate()
        .map(|(idx, region)| ConflictCodeLens {
            line: region.current_start,
            actions: vec![
                ConflictCodeLensAction {
                    label: "Accept Current Change".to_string(),
                    action: ConflictAction::AcceptCurrent,
                    region_index: idx,
                },
                ConflictCodeLensAction {
                    label: "Accept Incoming Change".to_string(),
                    action: ConflictAction::AcceptIncoming,
                    region_index: idx,
                },
                ConflictCodeLensAction {
                    label: "Accept Both Changes".to_string(),
                    action: ConflictAction::AcceptBoth,
                    region_index: idx,
                },
                ConflictCodeLensAction {
                    label: "Compare Changes".to_string(),
                    action: ConflictAction::CompareChanges,
                    region_index: idx,
                },
            ],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFLICTED: &str = "\
before
<<<<<<< HEAD
current line 1
current line 2
=======
incoming line 1
>>>>>>> feature-branch
after";

    #[test]
    fn detect_single_conflict() {
        let dec = ConflictMarkerDecoration::detect(CONFLICTED);
        assert!(dec.has_conflicts());
        assert_eq!(dec.conflict_count(), 1);
    }

    #[test]
    fn decorations_include_all_regions() {
        let dec = ConflictMarkerDecoration::detect(CONFLICTED);
        assert!(dec
            .decorations
            .iter()
            .any(|d| d.kind == ConflictRegionKind::Current));
        assert!(dec
            .decorations
            .iter()
            .any(|d| d.kind == ConflictRegionKind::Incoming));
        assert!(dec
            .decorations
            .iter()
            .any(|d| d.kind == ConflictRegionKind::Separator));
        assert!(dec
            .decorations
            .iter()
            .any(|d| d.kind == ConflictRegionKind::Marker));
    }

    #[test]
    fn codelens_has_four_actions() {
        let dec = ConflictMarkerDecoration::detect(CONFLICTED);
        assert_eq!(dec.codelens.len(), 1);
        assert_eq!(dec.codelens[0].actions.len(), 4);
    }

    #[test]
    fn conflict_at_line() {
        let dec = ConflictMarkerDecoration::detect(CONFLICTED);
        assert!(dec.conflict_at_line(3).is_some());
        assert!(dec.conflict_at_line(0).is_none());
    }

    #[test]
    fn no_conflicts_in_clean_file() {
        let dec = ConflictMarkerDecoration::detect("just normal\ncontent\nhere");
        assert!(!dec.has_conflicts());
        assert_eq!(dec.conflict_count(), 0);
    }

    #[test]
    fn multiple_conflicts() {
        let content = "\
<<<<<<< HEAD
a
=======
b
>>>>>>> branch
middle
<<<<<<< HEAD
c
=======
d
>>>>>>> branch";
        let dec = ConflictMarkerDecoration::detect(content);
        assert_eq!(dec.conflict_count(), 2);
        assert_eq!(dec.codelens.len(), 2);
    }

    #[test]
    fn navigation() {
        let content = "\
<<<<<<< HEAD
a
=======
b
>>>>>>> branch
middle
<<<<<<< HEAD
c
=======
d
>>>>>>> branch";
        let dec = ConflictMarkerDecoration::detect(content);
        let next = dec.next_conflict_after(0);
        assert!(next.is_some());
        assert_eq!(next.unwrap().current_start, 6);
    }

    #[test]
    fn apply_accept_current() {
        let dec = ConflictMarkerDecoration::detect(CONFLICTED);
        let result = dec.apply_single_resolution(CONFLICTED, 0, &MergeResolution::AcceptInput1);
        assert!(result.contains("current line 1"));
        assert!(!result.contains("incoming line 1"));
        assert!(result.contains("before"));
        assert!(result.contains("after"));
        assert!(!result.contains("<<<<<<<"));
    }

    #[test]
    fn apply_accept_incoming() {
        let dec = ConflictMarkerDecoration::detect(CONFLICTED);
        let result = dec.apply_single_resolution(CONFLICTED, 0, &MergeResolution::AcceptInput2);
        assert!(!result.contains("current line 1"));
        assert!(result.contains("incoming line 1"));
    }
}
