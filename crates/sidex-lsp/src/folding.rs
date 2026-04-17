//! Folding range support wrapping LSP `textDocument/foldingRange`.
//!
//! Provides collapsible regions in the editor (functions, imports,
//! comments, user-defined regions).

use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{FoldingRangeParams, TextDocumentIdentifier, Uri, WorkDoneProgressParams};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;

/// The kind of folding range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FoldingRangeKind {
    Comment,
    Imports,
    Region,
}

impl FoldingRangeKind {
    fn from_lsp(kind: &lsp_types::FoldingRangeKind) -> Option<Self> {
        if *kind == lsp_types::FoldingRangeKind::Comment {
            Some(Self::Comment)
        } else if *kind == lsp_types::FoldingRangeKind::Imports {
            Some(Self::Imports)
        } else if *kind == lsp_types::FoldingRangeKind::Region {
            Some(Self::Region)
        } else {
            None
        }
    }
}

/// A single folding range in the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldingRange {
    pub start_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_character: Option<u32>,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_character: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<FoldingRangeKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapsed_text: Option<String>,
}

/// Service for providing folding ranges from the language server.
pub struct FoldingRangeService;

impl FoldingRangeService {
    pub async fn provide_folding_ranges(
        client: &LspClient,
        uri: &str,
    ) -> Result<Vec<FoldingRange>> {
        provide_folding_ranges(client, uri).await
    }
}

/// Requests folding ranges for a file.
pub async fn provide_folding_ranges(
    client: &LspClient,
    uri: &str,
) -> Result<Vec<FoldingRange>> {
    let params = FoldingRangeParams {
        text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/foldingRange", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let ranges: Vec<lsp_types::FoldingRange> =
        serde_json::from_value(result).context("failed to parse folding ranges")?;
    Ok(ranges.into_iter().map(convert_folding_range).collect())
}

fn convert_folding_range(range: lsp_types::FoldingRange) -> FoldingRange {
    FoldingRange {
        start_line: range.start_line,
        start_character: range.start_character,
        end_line: range.end_line,
        end_character: range.end_character,
        kind: range.kind.as_ref().and_then(FoldingRangeKind::from_lsp),
        collapsed_text: range.collapsed_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folding_range_serde() {
        let range = FoldingRange {
            start_line: 10,
            start_character: Some(0),
            end_line: 20,
            end_character: Some(1),
            kind: Some(FoldingRangeKind::Region),
            collapsed_text: Some("...".into()),
        };
        let json = serde_json::to_string(&range).unwrap();
        let back: FoldingRange = serde_json::from_str(&json).unwrap();
        assert_eq!(back.start_line, 10);
        assert_eq!(back.end_line, 20);
        assert_eq!(back.kind, Some(FoldingRangeKind::Region));
        assert_eq!(back.collapsed_text.as_deref(), Some("..."));
    }

    #[test]
    fn folding_range_minimal() {
        let range = FoldingRange {
            start_line: 0,
            start_character: None,
            end_line: 5,
            end_character: None,
            kind: None,
            collapsed_text: None,
        };
        let json = serde_json::to_string(&range).unwrap();
        assert!(!json.contains("start_character"));
        assert!(!json.contains("end_character"));
        assert!(!json.contains("kind"));
        assert!(!json.contains("collapsed_text"));
    }

    #[test]
    fn folding_range_kind_serde() {
        let kind = FoldingRangeKind::Comment;
        let json = serde_json::to_string(&kind).unwrap();
        let back: FoldingRangeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, FoldingRangeKind::Comment);
    }

    #[test]
    fn convert_lsp_folding_range_comment() {
        let lsp_range = lsp_types::FoldingRange {
            start_line: 1,
            start_character: Some(0),
            end_line: 5,
            end_character: None,
            kind: Some(lsp_types::FoldingRangeKind::Comment),
            collapsed_text: None,
        };
        let range = convert_folding_range(lsp_range);
        assert_eq!(range.start_line, 1);
        assert_eq!(range.end_line, 5);
        assert_eq!(range.kind, Some(FoldingRangeKind::Comment));
    }

    #[test]
    fn convert_lsp_folding_range_imports() {
        let lsp_range = lsp_types::FoldingRange {
            start_line: 0,
            start_character: None,
            end_line: 8,
            end_character: None,
            kind: Some(lsp_types::FoldingRangeKind::Imports),
            collapsed_text: Some("use ...".into()),
        };
        let range = convert_folding_range(lsp_range);
        assert_eq!(range.kind, Some(FoldingRangeKind::Imports));
        assert_eq!(range.collapsed_text.as_deref(), Some("use ..."));
    }

    #[test]
    fn convert_lsp_folding_range_no_kind() {
        let lsp_range = lsp_types::FoldingRange {
            start_line: 10,
            start_character: None,
            end_line: 20,
            end_character: None,
            kind: None,
            collapsed_text: None,
        };
        let range = convert_folding_range(lsp_range);
        assert!(range.kind.is_none());
    }
}
