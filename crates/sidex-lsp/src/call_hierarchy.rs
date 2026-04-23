//! Call hierarchy support wrapping LSP `textDocument/prepareCallHierarchy`,
//! `callHierarchy/incomingCalls`, and `callHierarchy/outgoingCalls`.
//!
//! Provides both low-level async functions and a high-level
//! [`CallHierarchyService`] that manages lazy-loaded tree expansion for the
//! editor's call hierarchy panel.

use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    PartialResultParams, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
    WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::{lsp_to_range, position_to_lsp};

/// LSP `SymbolKind` values we map from raw u32.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

impl SymbolKind {
    pub fn from_u32(value: u32) -> Self {
        match value {
            1 => Self::File,
            2 => Self::Module,
            3 => Self::Namespace,
            4 => Self::Package,
            5 => Self::Class,
            6 => Self::Method,
            7 => Self::Property,
            8 => Self::Field,
            9 => Self::Constructor,
            10 => Self::Enum,
            11 => Self::Interface,
            13 => Self::Variable,
            14 => Self::Constant,
            15 => Self::String,
            16 => Self::Number,
            17 => Self::Boolean,
            18 => Self::Array,
            19 => Self::Object,
            20 => Self::Key,
            21 => Self::Null,
            22 => Self::EnumMember,
            23 => Self::Struct,
            24 => Self::Event,
            25 => Self::Operator,
            26 => Self::TypeParameter,
            _ => Self::Function,
        }
    }
}

/// LSP `SymbolTag` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolTag {
    Deprecated = 1,
}

/// A single item in the call hierarchy (a function/method/constructor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyItemInfo {
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
    pub raw: Option<CallHierarchyItem>,
}

impl CallHierarchyItemInfo {
    pub fn symbol_kind(&self) -> SymbolKind {
        SymbolKind::from_u32(self.kind)
    }
}

/// An incoming call — who calls a given function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingCall {
    pub from: CallHierarchyItemInfo,
    pub from_ranges: Vec<sidex_text::Range>,
}

/// An outgoing call — what a given function calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingCall {
    pub to: CallHierarchyItemInfo,
    pub from_ranges: Vec<sidex_text::Range>,
}

fn convert_item(item: CallHierarchyItem) -> CallHierarchyItemInfo {
    let tags = item.tags.as_ref().map_or_else(Vec::new, |t| {
        t.iter()
            .filter_map(|tag| {
                if *tag == lsp_types::SymbolTag::DEPRECATED {
                    Some(SymbolTag::Deprecated)
                } else {
                    None
                }
            })
            .collect()
    });

    CallHierarchyItemInfo {
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

// ── CallHierarchyService ────────────────────────────────────────────────────

/// High-level service for call hierarchy tree with lazy loading.
pub struct CallHierarchyService;

impl CallHierarchyService {
    /// Prepare the call hierarchy at a given cursor position.
    pub async fn prepare(
        client: &LspClient,
        uri: &str,
        position: sidex_text::Position,
    ) -> Result<Vec<CallHierarchyItemInfo>> {
        prepare_call_hierarchy(client, uri, position).await
    }

    /// Get all callers of the given item (incoming calls).
    pub async fn incoming_calls(
        client: &LspClient,
        item: &CallHierarchyItem,
    ) -> Result<Vec<IncomingCall>> {
        incoming_calls(client, item).await
    }

    /// Get all callees of the given item (outgoing calls).
    pub async fn outgoing_calls(
        client: &LspClient,
        item: &CallHierarchyItem,
    ) -> Result<Vec<OutgoingCall>> {
        outgoing_calls(client, item).await
    }
}

// ── Raw LSP requests ────────────────────────────────────────────────────────

/// Prepares the call hierarchy at a given position.
pub async fn prepare_call_hierarchy(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Vec<CallHierarchyItemInfo>> {
    let params = CallHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
            position: position_to_lsp(pos),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/prepareCallHierarchy", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let items: Vec<CallHierarchyItem> =
        serde_json::from_value(result).context("failed to parse call hierarchy items")?;
    Ok(items.into_iter().map(convert_item).collect())
}

/// Returns all callers of the given call hierarchy item.
pub async fn incoming_calls(
    client: &LspClient,
    item: &CallHierarchyItem,
) -> Result<Vec<IncomingCall>> {
    let params = CallHierarchyIncomingCallsParams {
        item: item.clone(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("callHierarchy/incomingCalls", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let calls: Vec<CallHierarchyIncomingCall> =
        serde_json::from_value(result).context("failed to parse incoming calls")?;

    Ok(calls
        .into_iter()
        .map(|c| IncomingCall {
            from: convert_item(c.from),
            from_ranges: c.from_ranges.into_iter().map(lsp_to_range).collect(),
        })
        .collect())
}

/// Returns all callees of the given call hierarchy item.
pub async fn outgoing_calls(
    client: &LspClient,
    item: &CallHierarchyItem,
) -> Result<Vec<OutgoingCall>> {
    let params = CallHierarchyOutgoingCallsParams {
        item: item.clone(),
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("callHierarchy/outgoingCalls", Some(val))
        .await?;

    if result.is_null() {
        return Ok(vec![]);
    }
    let calls: Vec<CallHierarchyOutgoingCall> =
        serde_json::from_value(result).context("failed to parse outgoing calls")?;

    Ok(calls
        .into_iter()
        .map(|c| OutgoingCall {
            to: convert_item(c.to),
            from_ranges: c.from_ranges.into_iter().map(lsp_to_range).collect(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_hierarchy_item_info_serde() {
        let info = CallHierarchyItemInfo {
            name: "main".into(),
            kind: 12,
            tags: vec![],
            uri: "file:///test.rs".into(),
            range: sidex_text::Range::new(
                sidex_text::Position::new(0, 0),
                sidex_text::Position::new(5, 1),
            ),
            selection_range: sidex_text::Range::new(
                sidex_text::Position::new(0, 3),
                sidex_text::Position::new(0, 7),
            ),
            detail: Some("fn main()".into()),
            raw: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: CallHierarchyItemInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "main");
        assert_eq!(back.kind, 12);
        assert!(back.raw.is_none());
    }

    #[test]
    fn symbol_kind_from_u32() {
        assert_eq!(SymbolKind::from_u32(12), SymbolKind::Function);
        assert_eq!(SymbolKind::from_u32(6), SymbolKind::Method);
        assert_eq!(SymbolKind::from_u32(5), SymbolKind::Class);
        assert_eq!(SymbolKind::from_u32(999), SymbolKind::Function);
    }

    #[test]
    fn call_hierarchy_item_symbol_kind() {
        let info = CallHierarchyItemInfo {
            name: "test".into(),
            kind: 6,
            tags: vec![SymbolTag::Deprecated],
            uri: "file:///t.rs".into(),
            range: sidex_text::Range::new(sidex_text::Position::ZERO, sidex_text::Position::ZERO),
            selection_range: sidex_text::Range::new(
                sidex_text::Position::ZERO,
                sidex_text::Position::ZERO,
            ),
            detail: None,
            raw: None,
        };
        assert_eq!(info.symbol_kind(), SymbolKind::Method);
        assert_eq!(info.tags.len(), 1);
    }

    #[test]
    fn incoming_call_serde() {
        let call = IncomingCall {
            from: CallHierarchyItemInfo {
                name: "caller".into(),
                kind: 12,
                tags: vec![],
                uri: "file:///a.rs".into(),
                range: sidex_text::Range::new(
                    sidex_text::Position::new(10, 0),
                    sidex_text::Position::new(20, 1),
                ),
                selection_range: sidex_text::Range::new(
                    sidex_text::Position::new(10, 3),
                    sidex_text::Position::new(10, 9),
                ),
                detail: None,
                raw: None,
            },
            from_ranges: vec![sidex_text::Range::new(
                sidex_text::Position::new(15, 4),
                sidex_text::Position::new(15, 10),
            )],
        };
        let json = serde_json::to_string(&call).unwrap();
        let back: IncomingCall = serde_json::from_str(&json).unwrap();
        assert_eq!(back.from.name, "caller");
        assert_eq!(back.from_ranges.len(), 1);
    }

    #[test]
    fn outgoing_call_serde() {
        let call = OutgoingCall {
            to: CallHierarchyItemInfo {
                name: "callee".into(),
                kind: 12,
                tags: vec![],
                uri: "file:///b.rs".into(),
                range: sidex_text::Range::new(
                    sidex_text::Position::new(0, 0),
                    sidex_text::Position::new(3, 1),
                ),
                selection_range: sidex_text::Range::new(
                    sidex_text::Position::new(0, 3),
                    sidex_text::Position::new(0, 9),
                ),
                detail: None,
                raw: None,
            },
            from_ranges: vec![],
        };
        let json = serde_json::to_string(&call).unwrap();
        let back: OutgoingCall = serde_json::from_str(&json).unwrap();
        assert_eq!(back.to.name, "callee");
    }

    #[test]
    fn convert_lsp_item() {
        let item = CallHierarchyItem {
            name: "test_fn".into(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: Some("pub fn test_fn()".into()),
            uri: "file:///t.rs".parse().unwrap(),
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(5, 1),
            ),
            selection_range: lsp_types::Range::new(
                lsp_types::Position::new(0, 7),
                lsp_types::Position::new(0, 14),
            ),
            data: None,
        };
        let info = convert_item(item);
        assert_eq!(info.name, "test_fn");
        assert!(info.raw.is_some());
        assert_eq!(info.detail.as_deref(), Some("pub fn test_fn()"));
        assert!(info.tags.is_empty());
    }

    #[test]
    fn convert_lsp_item_with_deprecated_tag() {
        let item = CallHierarchyItem {
            name: "old_fn".into(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: Some(vec![lsp_types::SymbolTag::DEPRECATED]),
            detail: None,
            uri: "file:///t.rs".parse().unwrap(),
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(1, 0),
            ),
            selection_range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 6),
            ),
            data: None,
        };
        let info = convert_item(item);
        assert_eq!(info.tags, vec![SymbolTag::Deprecated]);
    }

    #[test]
    fn symbol_tag_serde() {
        let tag = SymbolTag::Deprecated;
        let json = serde_json::to_string(&tag).unwrap();
        let back: SymbolTag = serde_json::from_str(&json).unwrap();
        assert_eq!(back, SymbolTag::Deprecated);
    }
}
