//! Code folding model — mirrors VS Code's `FoldingModel` +
//! `FoldingRanges` + indent/syntax/marker providers.
//!
//! Tracks which regions of a document are foldable and whether each region is
//! currently collapsed.  Folding ranges can originate from indentation
//! analysis, tree-sitter (language), or explicit markers (`#region`/`#endregion`).

use std::collections::HashMap;

use sidex_text::Buffer;

/// The source that produced a folding range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldSource {
    /// Computed from indentation levels.
    Indentation,
    /// Provided by a language provider (tree-sitter / LSP).
    Language,
    /// Explicit markers (#region / #endregion, // region, etc.).
    Marker,
    /// Manually toggled by the user.
    Manual,
}

/// The semantic kind of a folding range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    /// A general region (function body, object literal, etc.).
    Region,
    /// An import block.
    Imports,
    /// A comment block.
    Comment,
}

/// A single foldable region in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldingRegion {
    /// First line of the region (zero-based).
    pub start_line: u32,
    /// Last line of the region (zero-based, inclusive).
    pub end_line: u32,
    /// Whether this region is currently collapsed.
    pub is_collapsed: bool,
    /// How this region was detected.
    pub source: FoldSource,
    /// Semantic kind (if known).
    pub kind: Option<FoldKind>,
    /// Optional label to display in the collapse decoration (e.g. "#region Foo").
    pub label: Option<String>,
}

impl FoldingRegion {
    #[must_use]
    pub fn line_count(&self) -> u32 {
        self.end_line.saturating_sub(self.start_line) + 1
    }

    #[must_use]
    pub fn contains_line(&self, line: u32) -> bool {
        line >= self.start_line && line <= self.end_line
    }
}

/// Serialisable memento for persisting fold state across sessions.
#[derive(Debug, Clone)]
pub struct FoldStateMemento {
    /// (start_line, end_line, is_collapsed) tuples for every region.
    pub collapsed_regions: Vec<(u32, u32, bool)>,
    /// Line count at time of snapshot (for invalidation).
    pub line_count: usize,
    /// Which provider was active.
    pub provider: Option<String>,
}

/// The complete folding state for a document.
#[derive(Debug, Clone, Default)]
pub struct FoldingModel {
    /// All known folding regions, sorted by `start_line`.
    regions: Vec<FoldingRegion>,
}

impl FoldingModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the regions with a new set (e.g. after a re-parse).  Preserves
    /// the collapsed state for regions whose start/end lines still match.
    pub fn update_regions(&mut self, mut new_regions: Vec<FoldingRegion>) {
        for nr in &mut new_regions {
            if let Some(existing) = self
                .regions
                .iter()
                .find(|r| r.start_line == nr.start_line && r.end_line == nr.end_line)
            {
                nr.is_collapsed = existing.is_collapsed;
            }
        }
        new_regions.sort_by_key(|r| r.start_line);
        self.regions = new_regions;
    }

    /// Returns a reference to all regions.
    #[must_use]
    pub fn regions(&self) -> &[FoldingRegion] {
        &self.regions
    }

    /// Returns a mutable reference to all regions.
    pub fn regions_mut(&mut self) -> &mut [FoldingRegion] {
        &mut self.regions
    }

    /// Returns the region that starts on the given `line`, if any.
    #[must_use]
    pub fn region_at_line(&self, line: u32) -> Option<&FoldingRegion> {
        self.regions.iter().find(|r| r.start_line == line)
    }

    /// Returns all regions that contain the given line (sorted outermost first).
    #[must_use]
    pub fn regions_containing_line(&self, line: u32) -> Vec<&FoldingRegion> {
        self.regions
            .iter()
            .filter(|r| r.contains_line(line))
            .collect()
    }

    /// Toggles the collapsed state of the region that starts on `line`.
    /// Returns `true` if a region was toggled.
    pub fn toggle_fold(&mut self, line: u32) -> bool {
        if let Some(r) = self.regions.iter_mut().find(|r| r.start_line == line) {
            r.is_collapsed = !r.is_collapsed;
            true
        } else {
            false
        }
    }

    /// Collapses all regions.
    pub fn fold_all(&mut self) {
        for r in &mut self.regions {
            r.is_collapsed = true;
        }
    }

    /// Expands all regions.
    pub fn unfold_all(&mut self) {
        for r in &mut self.regions {
            r.is_collapsed = false;
        }
    }

    /// Folds all regions of a specific kind (imports, comments, etc.).
    pub fn fold_by_kind(&mut self, kind: FoldKind) {
        for r in &mut self.regions {
            if r.kind == Some(kind) {
                r.is_collapsed = true;
            }
        }
    }

    /// Collapses all regions at the given nesting level and deeper.
    /// Level 1 = top-level regions, level 2 = nested inside level 1, etc.
    pub fn fold_level(&mut self, level: u32) {
        let levels = self.compute_nesting_levels();
        for (i, r) in self.regions.iter_mut().enumerate() {
            r.is_collapsed = levels[i] >= level;
        }
    }

    /// Collapses/expands regions recursively at the given line.
    pub fn toggle_fold_recursive(&mut self, line: u32, collapse: bool) {
        let children: Vec<usize> = self
            .regions
            .iter()
            .enumerate()
            .filter(|(_, r)| r.start_line >= line)
            .take_while(|(_, r)| {
                let parent = self.regions.iter().find(|p| p.start_line == line);
                parent.is_some_and(|p| r.end_line <= p.end_line)
            })
            .map(|(i, _)| i)
            .collect();

        for i in children {
            self.regions[i].is_collapsed = collapse;
        }
    }

    /// Returns the set of lines that are hidden (inside a collapsed region,
    /// excluding the start line of each region).
    #[must_use]
    pub fn hidden_lines(&self) -> Vec<u32> {
        let mut hidden = Vec::new();
        for r in &self.regions {
            if r.is_collapsed {
                for line in (r.start_line + 1)..=r.end_line {
                    hidden.push(line);
                }
            }
        }
        hidden.sort_unstable();
        hidden.dedup();
        hidden
    }

    /// Returns `true` if `line` should be hidden by a collapsed fold.
    #[must_use]
    pub fn is_line_hidden(&self, line: u32) -> bool {
        self.regions
            .iter()
            .any(|r| r.is_collapsed && line > r.start_line && line <= r.end_line)
    }

    /// Creates a memento for saving fold state.
    #[must_use]
    pub fn save_state(&self, line_count: usize) -> FoldStateMemento {
        FoldStateMemento {
            collapsed_regions: self
                .regions
                .iter()
                .map(|r| (r.start_line, r.end_line, r.is_collapsed))
                .collect(),
            line_count,
            provider: None,
        }
    }

    /// Restores fold state from a memento. Only applies if line count matches.
    pub fn restore_state(&mut self, memento: &FoldStateMemento, current_line_count: usize) {
        if memento.line_count != current_line_count {
            return;
        }
        let lookup: HashMap<(u32, u32), bool> = memento
            .collapsed_regions
            .iter()
            .map(|&(s, e, c)| ((s, e), c))
            .collect();
        for r in &mut self.regions {
            if let Some(&collapsed) = lookup.get(&(r.start_line, r.end_line)) {
                r.is_collapsed = collapsed;
            }
        }
    }

    /// Returns the fold-preview text for a collapsed region starting at
    /// `line` — the first hidden line's content, truncated.
    #[must_use]
    pub fn fold_preview(&self, buffer: &Buffer, line: u32, max_chars: usize) -> Option<String> {
        let region = self
            .regions
            .iter()
            .find(|r| r.start_line == line && r.is_collapsed)?;
        if region.start_line + 1 > region.end_line {
            return None;
        }
        let next_line = (region.start_line + 1) as usize;
        if next_line >= buffer.len_lines() {
            return None;
        }
        let content = buffer.line_content(next_line);
        let trimmed = content.trim();
        if trimmed.len() > max_chars {
            Some(format!("{}…", &trimmed[..max_chars]))
        } else {
            Some(trimmed.to_string())
        }
    }

    /// Computes folding regions from indentation levels.
    pub fn compute_from_indentation(buffer: &Buffer, tab_size: u32) -> Vec<FoldingRegion> {
        let line_count = buffer.len_lines();
        if line_count == 0 {
            return Vec::new();
        }

        let indents: Vec<Option<u32>> = (0..line_count)
            .map(|i| {
                let content = buffer.line_content(i);
                let trimmed = content.trim_start();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(Self::visible_indent(&content, tab_size))
                }
            })
            .collect();

        let mut regions = Vec::new();
        let mut stack: Vec<(u32, u32)> = Vec::new(); // (indent_level, start_line)

        for (i, indent_opt) in indents.iter().enumerate() {
            let line = i as u32;
            if let Some(&indent) = indent_opt.as_ref() {
                while let Some(&(top_indent, top_start)) = stack.last() {
                    if indent <= top_indent {
                        stack.pop();
                        if line.saturating_sub(1) > top_start {
                            regions.push(FoldingRegion {
                                start_line: top_start,
                                end_line: line.saturating_sub(1),
                                is_collapsed: false,
                                source: FoldSource::Indentation,
                                kind: None,
                                label: None,
                            });
                        }
                    } else {
                        break;
                    }
                }
                stack.push((indent, line));
            }
        }

        let last_line = (line_count - 1) as u32;
        while let Some((_, start)) = stack.pop() {
            if last_line > start {
                regions.push(FoldingRegion {
                    start_line: start,
                    end_line: last_line,
                    is_collapsed: false,
                    source: FoldSource::Indentation,
                    kind: None,
                    label: None,
                });
            }
        }

        regions.sort_by_key(|r| r.start_line);
        regions
    }

    /// Detects `#region` / `#endregion` style markers with multiple syntax
    /// styles: `// #region`, `/* #region */`, `// region`, `#pragma region`.
    pub fn compute_from_markers(
        buffer: &Buffer,
        start_marker: &str,
        end_marker: &str,
    ) -> Vec<FoldingRegion> {
        let mut regions = Vec::new();
        let mut stack: Vec<(u32, Option<String>)> = Vec::new();

        for i in 0..buffer.len_lines() {
            let content = buffer.line_content(i);
            let trimmed = content.trim();
            if let Some(rest) = Self::extract_marker(trimmed, start_marker) {
                let label = if rest.is_empty() {
                    None
                } else {
                    Some(rest.to_string())
                };
                stack.push((i as u32, label));
            } else if trimmed.contains(end_marker) {
                if let Some((start, label)) = stack.pop() {
                    regions.push(FoldingRegion {
                        start_line: start,
                        end_line: i as u32,
                        is_collapsed: false,
                        source: FoldSource::Marker,
                        kind: Some(FoldKind::Region),
                        label,
                    });
                }
            }
        }

        regions.sort_by_key(|r| r.start_line);
        regions
    }

    /// Detects import blocks and creates fold regions for them.
    pub fn compute_import_regions(buffer: &Buffer, import_keywords: &[&str]) -> Vec<FoldingRegion> {
        let mut regions = Vec::new();
        let mut block_start: Option<u32> = None;

        for i in 0..buffer.len_lines() {
            let content = buffer.line_content(i);
            let trimmed = content.trim();
            let is_import = import_keywords.iter().any(|kw| trimmed.starts_with(kw));

            if is_import {
                if block_start.is_none() {
                    block_start = Some(i as u32);
                }
            } else if !trimmed.is_empty() {
                if let Some(start) = block_start.take() {
                    let end = (i as u32).saturating_sub(1);
                    if end > start {
                        regions.push(FoldingRegion {
                            start_line: start,
                            end_line: end,
                            is_collapsed: false,
                            source: FoldSource::Language,
                            kind: Some(FoldKind::Imports),
                            label: None,
                        });
                    }
                }
            }
        }

        regions
    }

    /// Detects consecutive comment blocks.
    pub fn compute_comment_regions(buffer: &Buffer, line_comment: &str) -> Vec<FoldingRegion> {
        let mut regions = Vec::new();
        let mut block_start: Option<u32> = None;

        for i in 0..buffer.len_lines() {
            let content = buffer.line_content(i);
            let trimmed = content.trim();
            if trimmed.starts_with(line_comment) {
                if block_start.is_none() {
                    block_start = Some(i as u32);
                }
            } else {
                if let Some(start) = block_start.take() {
                    let end = (i as u32).saturating_sub(1);
                    if end > start {
                        regions.push(FoldingRegion {
                            start_line: start,
                            end_line: end,
                            is_collapsed: false,
                            source: FoldSource::Language,
                            kind: Some(FoldKind::Comment),
                            label: None,
                        });
                    }
                }
            }
        }
        if let Some(start) = block_start {
            let end = buffer.len_lines().saturating_sub(1) as u32;
            if end > start {
                regions.push(FoldingRegion {
                    start_line: start,
                    end_line: end,
                    is_collapsed: false,
                    source: FoldSource::Language,
                    kind: Some(FoldKind::Comment),
                    label: None,
                });
            }
        }
        regions
    }

    /// Adds a manual fold range (user-defined).
    pub fn add_manual_range(&mut self, start_line: u32, end_line: u32) {
        if end_line <= start_line {
            return;
        }
        let region = FoldingRegion {
            start_line,
            end_line,
            is_collapsed: true,
            source: FoldSource::Manual,
            kind: None,
            label: None,
        };
        self.regions.push(region);
        self.regions.sort_by_key(|r| r.start_line);
    }

    /// Removes manual fold ranges that start on the given line.
    pub fn remove_manual_range(&mut self, start_line: u32) {
        self.regions
            .retain(|r| !(r.source == FoldSource::Manual && r.start_line == start_line));
    }

    // ── Private helpers ─────────────────────────────────────────────────

    fn extract_marker<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
        let stripped = line
            .strip_prefix("//")
            .or_else(|| line.strip_prefix('#'))
            .or_else(|| line.strip_prefix("/*"))
            .map(|s| s.trim_start())
            .unwrap_or(line);
        if stripped.starts_with(marker) {
            let rest = stripped[marker.len()..].trim();
            let rest = rest.strip_suffix("*/").unwrap_or(rest).trim();
            Some(rest)
        } else {
            None
        }
    }

    fn visible_indent(line: &str, tab_size: u32) -> u32 {
        let mut indent = 0u32;
        for ch in line.chars() {
            match ch {
                ' ' => indent += 1,
                '\t' => indent += tab_size - (indent % tab_size),
                _ => break,
            }
        }
        indent
    }

    fn compute_nesting_levels(&self) -> Vec<u32> {
        let mut levels = vec![0u32; self.regions.len()];
        for (i, region) in self.regions.iter().enumerate() {
            let mut depth = 1u32;
            for parent in &self.regions[..i] {
                if parent.start_line < region.start_line && parent.end_line >= region.end_line {
                    depth += 1;
                }
            }
            levels[i] = depth;
        }
        levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(text: &str) -> Buffer {
        Buffer::from_str(text)
    }

    #[test]
    fn toggle_fold() {
        let mut model = FoldingModel::new();
        model.regions = vec![FoldingRegion {
            start_line: 0,
            end_line: 5,
            is_collapsed: false,
            source: FoldSource::Language,
            kind: None,
            label: None,
        }];
        assert!(model.toggle_fold(0));
        assert!(model.regions[0].is_collapsed);
        assert!(model.toggle_fold(0));
        assert!(!model.regions[0].is_collapsed);
    }

    #[test]
    fn hidden_lines() {
        let mut model = FoldingModel::new();
        model.regions = vec![FoldingRegion {
            start_line: 2,
            end_line: 5,
            is_collapsed: true,
            source: FoldSource::Language,
            kind: None,
            label: None,
        }];
        let hidden = model.hidden_lines();
        assert_eq!(hidden, vec![3, 4, 5]);
        assert!(!model.is_line_hidden(2));
        assert!(model.is_line_hidden(3));
    }

    #[test]
    fn indentation_folding() {
        let text = "fn main() {\n    let x = 1;\n    let y = 2;\n}\n";
        let buffer = buf(text);
        let regions = FoldingModel::compute_from_indentation(&buffer, 4);
        assert!(!regions.is_empty());
        assert_eq!(regions[0].start_line, 0);
    }

    #[test]
    fn marker_folding() {
        let text = "// #region Foo\ncode\nmore\n// #endregion\n";
        let buffer = buf(text);
        let regions = FoldingModel::compute_from_markers(&buffer, "#region", "#endregion");
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].start_line, 0);
        assert_eq!(regions[0].end_line, 3);
        assert_eq!(regions[0].label.as_deref(), Some("Foo"));
    }

    #[test]
    fn save_restore_state() {
        let mut model = FoldingModel::new();
        model.regions = vec![
            FoldingRegion {
                start_line: 0,
                end_line: 5,
                is_collapsed: true,
                source: FoldSource::Language,
                kind: None,
                label: None,
            },
            FoldingRegion {
                start_line: 10,
                end_line: 15,
                is_collapsed: false,
                source: FoldSource::Language,
                kind: None,
                label: None,
            },
        ];
        let memento = model.save_state(100);

        let mut model2 = FoldingModel::new();
        model2.regions = model.regions.clone();
        model2.regions[0].is_collapsed = false;
        model2.restore_state(&memento, 100);
        assert!(model2.regions[0].is_collapsed);
    }

    #[test]
    fn fold_preview_text() {
        let text = "fn foo() {\n    let x = 42;\n    return x;\n}";
        let buffer = buf(text);
        let mut model = FoldingModel::new();
        model.regions = vec![FoldingRegion {
            start_line: 0,
            end_line: 3,
            is_collapsed: true,
            source: FoldSource::Language,
            kind: None,
            label: None,
        }];
        let preview = model.fold_preview(&buffer, 0, 50);
        assert_eq!(preview.as_deref(), Some("let x = 42;"));
    }

    #[test]
    fn manual_fold_range() {
        let mut model = FoldingModel::new();
        model.add_manual_range(5, 10);
        assert_eq!(model.regions.len(), 1);
        assert!(model.regions[0].is_collapsed);
        assert_eq!(model.regions[0].source, FoldSource::Manual);

        model.remove_manual_range(5);
        assert!(model.regions.is_empty());
    }

    #[test]
    fn fold_by_kind() {
        let mut model = FoldingModel::new();
        model.regions = vec![
            FoldingRegion {
                start_line: 0,
                end_line: 3,
                is_collapsed: false,
                source: FoldSource::Language,
                kind: Some(FoldKind::Imports),
                label: None,
            },
            FoldingRegion {
                start_line: 5,
                end_line: 10,
                is_collapsed: false,
                source: FoldSource::Language,
                kind: Some(FoldKind::Region),
                label: None,
            },
        ];
        model.fold_by_kind(FoldKind::Imports);
        assert!(model.regions[0].is_collapsed);
        assert!(!model.regions[1].is_collapsed);
    }
}
