//! Document link support wrapping LSP `textDocument/documentLink` and
//! `documentLink/resolve`.
//!
//! Provides clickable links in code for file paths, URLs, and other
//! navigable references.

use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{
    DocumentLinkParams, PartialResultParams, TextDocumentIdentifier, Uri, WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::lsp_to_range;

/// A clickable link in a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentLink {
    /// The range this link covers in the document.
    pub range: sidex_text::Range,
    /// The URI this link points to. May be `None` until resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// An optional tooltip shown on hover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// Opaque data preserved for resolve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Service for providing and resolving document links.
pub struct DocumentLinkService;

impl DocumentLinkService {
    /// Request document links for the given file.
    pub async fn provide_links(
        client: &LspClient,
        uri: &str,
    ) -> Result<Vec<DocumentLink>> {
        provide_document_links(client, uri).await
    }

    /// Resolve a partially-resolved link to fill in the target URI.
    pub async fn resolve_link(
        client: &LspClient,
        link: &lsp_types::DocumentLink,
    ) -> Result<DocumentLink> {
        resolve_document_link(client, link).await
    }
}

/// Requests all document links for a file.
pub async fn provide_document_links(
    client: &LspClient,
    uri: &str,
) -> Result<Vec<DocumentLink>> {
    let params = DocumentLinkParams {
        text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/documentLink", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let links: Vec<lsp_types::DocumentLink> =
        serde_json::from_value(result).context("failed to parse document links")?;
    Ok(links.into_iter().map(convert_link).collect())
}

/// Resolves a document link to fill in the target URI.
pub async fn resolve_document_link(
    client: &LspClient,
    link: &lsp_types::DocumentLink,
) -> Result<DocumentLink> {
    let val = serde_json::to_value(link)?;
    let result = client
        .raw_request("documentLink/resolve", Some(val))
        .await?;
    let resolved: lsp_types::DocumentLink =
        serde_json::from_value(result).context("failed to parse resolved document link")?;
    Ok(convert_link(resolved))
}

fn convert_link(link: lsp_types::DocumentLink) -> DocumentLink {
    DocumentLink {
        range: lsp_to_range(link.range),
        target: link.target.map(|u| u.to_string()),
        tooltip: link.tooltip,
        data: link.data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_link_serde() {
        let link = DocumentLink {
            range: sidex_text::Range::new(
                sidex_text::Position::new(5, 10),
                sidex_text::Position::new(5, 30),
            ),
            target: Some("https://example.com".into()),
            tooltip: Some("Click to open".into()),
            data: None,
        };
        let json = serde_json::to_string(&link).unwrap();
        let back: DocumentLink = serde_json::from_str(&json).unwrap();
        assert_eq!(back.target.as_deref(), Some("https://example.com"));
        assert_eq!(back.tooltip.as_deref(), Some("Click to open"));
    }

    #[test]
    fn document_link_no_target() {
        let link = DocumentLink {
            range: sidex_text::Range::new(
                sidex_text::Position::ZERO,
                sidex_text::Position::new(0, 10),
            ),
            target: None,
            tooltip: None,
            data: Some(serde_json::json!({"id": 42})),
        };
        let json = serde_json::to_string(&link).unwrap();
        assert!(!json.contains("target"));
        assert!(!json.contains("tooltip"));
        assert!(json.contains("data"));
    }

    #[test]
    fn convert_lsp_link() {
        let lsp_link = lsp_types::DocumentLink {
            range: lsp_types::Range::new(
                lsp_types::Position::new(1, 0),
                lsp_types::Position::new(1, 20),
            ),
            target: Some("file:///readme.md".parse().unwrap()),
            tooltip: Some("Open file".into()),
            data: None,
        };
        let link = convert_link(lsp_link);
        assert_eq!(link.range.start.line, 1);
        assert!(link.target.unwrap().contains("readme.md"));
        assert_eq!(link.tooltip.as_deref(), Some("Open file"));
    }

    #[test]
    fn convert_lsp_link_no_target() {
        let lsp_link = lsp_types::DocumentLink {
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 5),
            ),
            target: None,
            tooltip: None,
            data: None,
        };
        let link = convert_link(lsp_link);
        assert!(link.target.is_none());
    }
}
