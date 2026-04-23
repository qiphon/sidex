//! Type hierarchy support wrapping LSP `textDocument/prepareTypeHierarchy`,
//! `typeHierarchy/supertypes`, and `typeHierarchy/subtypes`.
//!
//! Provides both low-level async functions and a high-level
//! [`TypeHierarchyService`] for the editor's type hierarchy panel.

use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams};
use serde::{Deserialize, Serialize};

use crate::call_hierarchy::{SymbolKind, SymbolTag};
use crate::client::LspClient;
use crate::conversion::{lsp_to_range, position_to_lsp};

/// A single item in the type hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeHierarchyItemInfo {
    pub name: String,
    pub kind: u32,
    pub tags: Vec<SymbolTag>,
    pub uri: String,
    pub range: sidex_text::Range,
    pub selection_range: sidex_text::Range,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The raw LSP item, needed for follow-up requests.
    #[serde(skip)]
    pub raw: Option<lsp_types::TypeHierarchyItem>,
}

impl TypeHierarchyItemInfo {
    pub fn symbol_kind(&self) -> SymbolKind {
        SymbolKind::from_u32(self.kind)
    }
}

fn convert_item(item: lsp_types::TypeHierarchyItem) -> TypeHierarchyItemInfo {
    let tags = item.tags.as_ref().map_or_else(Vec::new, |tag| {
        if *tag == lsp_types::SymbolTag::DEPRECATED {
            vec![SymbolTag::Deprecated]
        } else {
            vec![]
        }
    });

    TypeHierarchyItemInfo {
        name: item.name.clone(),
        #[allow(clippy::cast_possible_truncation)]
        kind: serde_json::to_value(item.kind)
            .ok()
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        tags,
        uri: item.uri.to_string(),
        range: lsp_to_range(item.range),
        selection_range: lsp_to_range(item.selection_range),
        detail: item.detail.clone(),
        raw: Some(item),
    }
}

// ── TypeHierarchyService ────────────────────────────────────────────────────

/// High-level service for type hierarchy exploration.
pub struct TypeHierarchyService;

impl TypeHierarchyService {
    /// Prepare the type hierarchy at a given cursor position.
    pub async fn prepare(
        client: &LspClient,
        uri: &str,
        position: sidex_text::Position,
    ) -> Result<Vec<TypeHierarchyItemInfo>> {
        prepare_type_hierarchy(client, uri, position).await
    }

    /// Get all supertypes (base classes, implemented traits).
    pub async fn supertypes(
        client: &LspClient,
        item: &lsp_types::TypeHierarchyItem,
    ) -> Result<Vec<TypeHierarchyItemInfo>> {
        supertypes(client, item).await
    }

    /// Get all subtypes (derived classes, implementors).
    pub async fn subtypes(
        client: &LspClient,
        item: &lsp_types::TypeHierarchyItem,
    ) -> Result<Vec<TypeHierarchyItemInfo>> {
        subtypes(client, item).await
    }
}

// ── Raw LSP requests ────────────────────────────────────────────────────────

/// Prepares the type hierarchy at a given position.
pub async fn prepare_type_hierarchy(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Vec<TypeHierarchyItemInfo>> {
    let params = lsp_types::TypeHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
            position: position_to_lsp(pos),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/prepareTypeHierarchy", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let items: Vec<lsp_types::TypeHierarchyItem> =
        serde_json::from_value(result).context("failed to parse type hierarchy items")?;
    Ok(items.into_iter().map(convert_item).collect())
}

/// Returns the supertypes of the given type hierarchy item.
pub async fn supertypes(
    client: &LspClient,
    item: &lsp_types::TypeHierarchyItem,
) -> Result<Vec<TypeHierarchyItemInfo>> {
    let params = lsp_types::TypeHierarchySupertypesParams {
        item: item.clone(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("typeHierarchy/supertypes", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let items: Vec<lsp_types::TypeHierarchyItem> =
        serde_json::from_value(result).context("failed to parse supertypes")?;
    Ok(items.into_iter().map(convert_item).collect())
}

/// Returns the subtypes of the given type hierarchy item.
pub async fn subtypes(
    client: &LspClient,
    item: &lsp_types::TypeHierarchyItem,
) -> Result<Vec<TypeHierarchyItemInfo>> {
    let params = lsp_types::TypeHierarchySubtypesParams {
        item: item.clone(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: lsp_types::PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("typeHierarchy/subtypes", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let items: Vec<lsp_types::TypeHierarchyItem> =
        serde_json::from_value(result).context("failed to parse subtypes")?;
    Ok(items.into_iter().map(convert_item).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_hierarchy_item_info_serde() {
        let info = TypeHierarchyItemInfo {
            name: "MyStruct".into(),
            kind: 23,
            tags: vec![],
            uri: "file:///test.rs".into(),
            range: sidex_text::Range::new(
                sidex_text::Position::new(0, 0),
                sidex_text::Position::new(10, 1),
            ),
            selection_range: sidex_text::Range::new(
                sidex_text::Position::new(0, 11),
                sidex_text::Position::new(0, 19),
            ),
            detail: Some("struct MyStruct".into()),
            raw: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: TypeHierarchyItemInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "MyStruct");
        assert_eq!(back.kind, 23);
    }

    #[test]
    fn type_hierarchy_item_symbol_kind() {
        let info = TypeHierarchyItemInfo {
            name: "Trait".into(),
            kind: 11,
            tags: vec![],
            uri: "file:///t.rs".into(),
            range: sidex_text::Range::new(sidex_text::Position::ZERO, sidex_text::Position::ZERO),
            selection_range: sidex_text::Range::new(
                sidex_text::Position::ZERO,
                sidex_text::Position::ZERO,
            ),
            detail: None,
            raw: None,
        };
        assert_eq!(info.symbol_kind(), SymbolKind::Interface);
    }

    #[test]
    fn convert_lsp_type_hierarchy_item() {
        let item = lsp_types::TypeHierarchyItem {
            name: "Base".into(),
            kind: lsp_types::SymbolKind::CLASS,
            tags: None,
            detail: None,
            uri: "file:///base.rs".parse().unwrap(),
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(5, 1),
            ),
            selection_range: lsp_types::Range::new(
                lsp_types::Position::new(0, 6),
                lsp_types::Position::new(0, 10),
            ),
            data: None,
        };
        let info = convert_item(item);
        assert_eq!(info.name, "Base");
        assert!(info.raw.is_some());
        assert!(info.tags.is_empty());
    }

    #[test]
    fn convert_lsp_item_with_deprecated_tag() {
        let item = lsp_types::TypeHierarchyItem {
            name: "OldType".into(),
            kind: lsp_types::SymbolKind::CLASS,
            tags: Some(lsp_types::SymbolTag::DEPRECATED),
            detail: None,
            uri: "file:///old.rs".parse().unwrap(),
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(1, 0),
            ),
            selection_range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 7),
            ),
            data: None,
        };
        let info = convert_item(item);
        assert_eq!(info.tags, vec![SymbolTag::Deprecated]);
    }

    #[test]
    fn type_hierarchy_item_without_raw_serializes() {
        let info = TypeHierarchyItemInfo {
            name: "Derived".into(),
            kind: 5,
            tags: vec![],
            uri: "file:///d.rs".into(),
            range: sidex_text::Range::new(
                sidex_text::Position::new(0, 0),
                sidex_text::Position::new(1, 0),
            ),
            selection_range: sidex_text::Range::new(
                sidex_text::Position::new(0, 0),
                sidex_text::Position::new(0, 7),
            ),
            detail: None,
            raw: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(!json.contains("detail"));
        assert!(!json.contains("raw"));
    }
}
