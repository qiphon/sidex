//! Smart selection support wrapping LSP `textDocument/selectionRange`.
//!
//! Allows expanding/shrinking the editor selection based on semantic
//! structure (e.g. Shift+Alt+Right to expand, Shift+Alt+Left to shrink).

use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{
    SelectionRangeParams, TextDocumentIdentifier, Uri, WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::{lsp_to_range, position_to_lsp};

/// A selection range with optional parent for hierarchical selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionRange {
    pub range: sidex_text::Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Box<SelectionRange>>,
}

impl SelectionRange {
    /// Returns the depth of the selection range chain (number of parent levels).
    pub fn depth(&self) -> usize {
        match &self.parent {
            Some(parent) => 1 + parent.depth(),
            None => 0,
        }
    }

    /// Collects all ranges in the chain from innermost to outermost.
    pub fn all_ranges(&self) -> Vec<sidex_text::Range> {
        let mut ranges = vec![self.range];
        let mut current = &self.parent;
        while let Some(parent) = current {
            ranges.push(parent.range);
            current = &parent.parent;
        }
        ranges
    }
}

/// Service for providing semantic selection ranges.
pub struct SelectionRangeService;

impl SelectionRangeService {
    pub async fn provide_selection_ranges(
        client: &LspClient,
        uri: &str,
        positions: &[sidex_text::Position],
    ) -> Result<Vec<SelectionRange>> {
        provide_selection_ranges(client, uri, positions).await
    }
}

/// Requests selection ranges from the language server.
pub async fn provide_selection_ranges(
    client: &LspClient,
    uri: &str,
    positions: &[sidex_text::Position],
) -> Result<Vec<SelectionRange>> {
    let lsp_positions: Vec<lsp_types::Position> =
        positions.iter().map(|p| position_to_lsp(*p)).collect();

    let params = SelectionRangeParams {
        text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
        positions: lsp_positions,
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/selectionRange", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let ranges: Vec<lsp_types::SelectionRange> =
        serde_json::from_value(result).context("failed to parse selection ranges")?;
    Ok(ranges.into_iter().map(|r| convert_selection_range(&r)).collect())
}

fn convert_selection_range(range: &lsp_types::SelectionRange) -> SelectionRange {
    SelectionRange {
        range: lsp_to_range(range.range),
        parent: range
            .parent
            .as_ref()
            .map(|p| Box::new(convert_selection_range(p))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_range_serde() {
        let range = SelectionRange {
            range: sidex_text::Range::new(
                sidex_text::Position::new(5, 4),
                sidex_text::Position::new(5, 10),
            ),
            parent: Some(Box::new(SelectionRange {
                range: sidex_text::Range::new(
                    sidex_text::Position::new(5, 0),
                    sidex_text::Position::new(5, 30),
                ),
                parent: None,
            })),
        };
        let json = serde_json::to_string(&range).unwrap();
        let back: SelectionRange = serde_json::from_str(&json).unwrap();
        assert_eq!(back.range.start.column, 4);
        assert!(back.parent.is_some());
    }

    #[test]
    fn selection_range_depth() {
        let range = SelectionRange {
            range: sidex_text::Range::new(
                sidex_text::Position::ZERO,
                sidex_text::Position::ZERO,
            ),
            parent: Some(Box::new(SelectionRange {
                range: sidex_text::Range::new(
                    sidex_text::Position::ZERO,
                    sidex_text::Position::new(10, 0),
                ),
                parent: Some(Box::new(SelectionRange {
                    range: sidex_text::Range::new(
                        sidex_text::Position::ZERO,
                        sidex_text::Position::new(100, 0),
                    ),
                    parent: None,
                })),
            })),
        };
        assert_eq!(range.depth(), 2);
    }

    #[test]
    fn selection_range_all_ranges() {
        let range = SelectionRange {
            range: sidex_text::Range::new(
                sidex_text::Position::new(5, 4),
                sidex_text::Position::new(5, 10),
            ),
            parent: Some(Box::new(SelectionRange {
                range: sidex_text::Range::new(
                    sidex_text::Position::new(5, 0),
                    sidex_text::Position::new(5, 30),
                ),
                parent: None,
            })),
        };
        let ranges = range.all_ranges();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].start.column, 4);
        assert_eq!(ranges[1].start.column, 0);
    }

    #[test]
    fn selection_range_no_parent() {
        let range = SelectionRange {
            range: sidex_text::Range::new(
                sidex_text::Position::ZERO,
                sidex_text::Position::new(0, 10),
            ),
            parent: None,
        };
        assert_eq!(range.depth(), 0);
        assert_eq!(range.all_ranges().len(), 1);
        let json = serde_json::to_string(&range).unwrap();
        assert!(!json.contains("parent"));
    }

    #[test]
    fn convert_lsp_selection_range() {
        let lsp_range = lsp_types::SelectionRange {
            range: lsp_types::Range::new(
                lsp_types::Position::new(1, 2),
                lsp_types::Position::new(1, 8),
            ),
            parent: Some(Box::new(lsp_types::SelectionRange {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(10, 0),
                ),
                parent: None,
            })),
        };
        let range = convert_selection_range(&lsp_range);
        assert_eq!(range.range.start.line, 1);
        assert_eq!(range.range.start.column, 2);
        assert!(range.parent.is_some());
        let parent = range.parent.unwrap();
        assert_eq!(parent.range.start.line, 0);
        assert!(parent.parent.is_none());
    }
}
