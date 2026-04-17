//! Document and range formatting via LSP.
//!
//! Wraps `textDocument/formatting`, `textDocument/rangeFormatting`, and
//! `textDocument/onTypeFormatting` requests.

use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{
    DocumentFormattingParams, DocumentOnTypeFormattingParams, DocumentRangeFormattingParams,
    FormattingOptions, TextDocumentIdentifier, TextEdit, Uri, WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::{lsp_to_range, range_to_lsp};

/// A formatting edit translated to `sidex_text` types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatEdit {
    pub range: sidex_text::Range,
    pub new_text: String,
}

fn convert_edits(edits: Vec<TextEdit>) -> Vec<FormatEdit> {
    edits
        .into_iter()
        .map(|e| FormatEdit {
            range: lsp_to_range(e.range),
            new_text: e.new_text,
        })
        .collect()
}

/// Requests formatting for the entire document.
pub async fn format_document(
    client: &LspClient,
    uri: &str,
    options: FormattingOptions,
) -> Result<Vec<FormatEdit>> {
    let params = DocumentFormattingParams {
        text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
        options,
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/formatting", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let edits: Vec<TextEdit> =
        serde_json::from_value(result).context("failed to parse formatting edits")?;
    Ok(convert_edits(edits))
}

/// Requests formatting for a specific range within a document.
pub async fn format_range(
    client: &LspClient,
    uri: &str,
    range: sidex_text::Range,
    options: FormattingOptions,
) -> Result<Vec<FormatEdit>> {
    let params = DocumentRangeFormattingParams {
        text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
        range: range_to_lsp(range),
        options,
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/rangeFormatting", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let edits: Vec<TextEdit> =
        serde_json::from_value(result).context("failed to parse range formatting edits")?;
    Ok(convert_edits(edits))
}

/// Requests on-type formatting triggered by a character.
pub async fn format_on_type(
    client: &LspClient,
    uri: &str,
    position: sidex_text::Position,
    ch: char,
    options: FormattingOptions,
) -> Result<Vec<FormatEdit>> {
    let params = DocumentOnTypeFormattingParams {
        text_document_position: lsp_types::TextDocumentPositionParams {
            text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
            position: crate::conversion::position_to_lsp(position),
        },
        ch: ch.to_string(),
        options,
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/onTypeFormatting", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let edits: Vec<TextEdit> =
        serde_json::from_value(result).context("failed to parse on-type formatting edits")?;
    Ok(convert_edits(edits))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_edit_serde() {
        let edit = FormatEdit {
            range: sidex_text::Range::new(
                sidex_text::Position::new(0, 0),
                sidex_text::Position::new(0, 4),
            ),
            new_text: "    ".into(),
        };
        let json = serde_json::to_string(&edit).unwrap();
        let back: FormatEdit = serde_json::from_str(&json).unwrap();
        assert_eq!(back.new_text, "    ");
    }

    #[test]
    fn convert_empty_edits() {
        let result = convert_edits(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn convert_single_edit() {
        let edits = vec![TextEdit {
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 2),
            ),
            new_text: "  ".into(),
        }];
        let result = convert_edits(edits);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].new_text, "  ");
        assert_eq!(result[0].range.start.line, 0);
    }

    #[test]
    fn convert_multiple_edits() {
        let edits = vec![
            TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 1),
                ),
                new_text: "a".into(),
            },
            TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(1, 0),
                    lsp_types::Position::new(1, 1),
                ),
                new_text: "b".into(),
            },
        ];
        let result = convert_edits(edits);
        assert_eq!(result.len(), 2);
    }
}
