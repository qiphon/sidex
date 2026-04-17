//! Rename support wrapping LSP `textDocument/prepareRename` and
//! `textDocument/rename`.
//!
//! Provides prepare-rename validation, rename execution with preview,
//! and workspace edit application as a single undo group.

use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Context, Result};
use lsp_types::{PrepareRenameResponse, TextDocumentIdentifier, TextDocumentPositionParams, Uri};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::{lsp_to_range, position_to_lsp};

/// Information returned by `prepareRename` — the valid range and a suggested
/// placeholder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameInfo {
    /// Range of the symbol to be renamed.
    pub range: sidex_text::Range,
    /// Suggested placeholder text (usually the current symbol name).
    pub placeholder: String,
}

/// Result of `prepareRename` — validated range and placeholder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareRenameResult {
    pub range: sidex_text::Range,
    pub placeholder: String,
}

/// A set of text edits grouped by file URI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceEdit {
    /// Map from file URI to the list of text edits for that file.
    pub changes: HashMap<String, Vec<TextEditInfo>>,
}

/// Result of a rename operation, including both simple changes and
/// document-level operations (create/rename/delete).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenameResult {
    pub changes: HashMap<String, Vec<TextEditInfo>>,
    pub document_changes: Vec<DocumentChange>,
}

impl RenameResult {
    /// Total number of edits across all files.
    pub fn total_edits(&self) -> usize {
        self.changes.values().map(Vec::len).sum::<usize>()
            + self
                .document_changes
                .iter()
                .filter_map(|dc| {
                    if let DocumentChange::Edit { edits, .. } = dc {
                        Some(edits.len())
                    } else {
                        None
                    }
                })
                .sum::<usize>()
    }

    /// Number of files affected.
    pub fn file_count(&self) -> usize {
        self.changes.len() + self.document_changes.len()
    }
}

/// A document-level change (text edits, create, rename, or delete).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentChange {
    Edit {
        uri: String,
        version: Option<i32>,
        edits: Vec<TextEditInfo>,
    },
    Create {
        uri: String,
    },
    Rename {
        old_uri: String,
        new_uri: String,
    },
    Delete {
        uri: String,
    },
}

/// A single text edit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEditInfo {
    pub range: sidex_text::Range,
    pub new_text: String,
}

// ── RenameService ───────────────────────────────────────────────────────────

/// High-level rename service with prepare + execute + preview workflow.
pub struct RenameService;

impl RenameService {
    /// Validate that rename is possible at the given position. Returns the
    /// symbol range and a placeholder name.
    pub async fn prepare_rename(
        client: &LspClient,
        uri: &str,
        position: sidex_text::Position,
    ) -> Result<Option<PrepareRenameResult>> {
        let info = prepare_rename(client, uri, position).await?;
        Ok(info.map(|i| PrepareRenameResult {
            range: i.range,
            placeholder: i.placeholder,
        }))
    }

    /// Execute the rename and return a full `RenameResult` with all changes
    /// across files, suitable for preview before applying.
    pub async fn rename(
        client: &LspClient,
        uri: &str,
        position: sidex_text::Position,
        new_name: &str,
    ) -> Result<RenameResult> {
        let lsp_pos = position_to_lsp(position);
        let response = client.rename(uri, lsp_pos, new_name).await?;

        match response {
            Some(edit) => Ok(convert_full_workspace_edit(edit)),
            None => Ok(RenameResult::default()),
        }
    }
}

// ── Raw LSP requests ────────────────────────────────────────────────────────

/// Checks whether a rename is valid at the given position and returns the
/// rename range and placeholder text.
pub async fn prepare_rename(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Option<RenameInfo>> {
    let lsp_pos = position_to_lsp(pos);
    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
        position: lsp_pos,
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/prepareRename", Some(val))
        .await?;

    if result.is_null() {
        return Ok(None);
    }

    let response: PrepareRenameResponse =
        serde_json::from_value(result).context("failed to parse prepareRename response")?;

    let info = match response {
        PrepareRenameResponse::Range(range) => RenameInfo {
            range: lsp_to_range(range),
            placeholder: String::new(),
        },
        PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } => RenameInfo {
            range: lsp_to_range(range),
            placeholder,
        },
        PrepareRenameResponse::DefaultBehavior {
            default_behavior: _,
        } => {
            return Ok(None);
        }
    };

    Ok(Some(info))
}

/// Executes a rename at the given position with the new name, returning a
/// workspace edit.
pub async fn execute_rename(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
    new_name: &str,
) -> Result<WorkspaceEdit> {
    let lsp_pos = position_to_lsp(pos);
    let response = client.rename(uri, lsp_pos, new_name).await?;

    match response {
        Some(edit) => Ok(convert_workspace_edit(edit)),
        None => Ok(WorkspaceEdit::default()),
    }
}

fn convert_workspace_edit(edit: lsp_types::WorkspaceEdit) -> WorkspaceEdit {
    let mut changes = HashMap::new();

    if let Some(raw_changes) = edit.changes {
        for (uri, edits) in raw_changes {
            let converted: Vec<TextEditInfo> = edits
                .into_iter()
                .map(|e| TextEditInfo {
                    range: lsp_to_range(e.range),
                    new_text: e.new_text,
                })
                .collect();
            changes.insert(uri.to_string(), converted);
        }
    }

    if let Some(document_changes) = edit.document_changes {
        use lsp_types::DocumentChanges;
        let operations = match document_changes {
            DocumentChanges::Edits(edits) => edits
                .into_iter()
                .map(lsp_types::DocumentChangeOperation::Edit)
                .collect::<Vec<_>>(),
            DocumentChanges::Operations(ops) => ops,
        };
        for change in operations {
            if let lsp_types::DocumentChangeOperation::Edit(text_doc_edit) = change {
                let uri_str = text_doc_edit.text_document.uri.to_string();
                let edits: Vec<TextEditInfo> = text_doc_edit
                    .edits
                    .into_iter()
                    .map(|e| match e {
                        lsp_types::OneOf::Left(edit) => TextEditInfo {
                            range: lsp_to_range(edit.range),
                            new_text: edit.new_text,
                        },
                        lsp_types::OneOf::Right(annotated) => TextEditInfo {
                            range: lsp_to_range(annotated.text_edit.range),
                            new_text: annotated.text_edit.new_text,
                        },
                    })
                    .collect();
                changes
                    .entry(uri_str)
                    .or_insert_with(Vec::new)
                    .extend(edits);
            }
        }
    }

    WorkspaceEdit { changes }
}

fn convert_full_workspace_edit(edit: lsp_types::WorkspaceEdit) -> RenameResult {
    let mut changes = HashMap::new();
    let mut document_changes = Vec::new();

    if let Some(raw_changes) = edit.changes {
        for (uri, edits) in raw_changes {
            let converted: Vec<TextEditInfo> = edits
                .into_iter()
                .map(|e| TextEditInfo {
                    range: lsp_to_range(e.range),
                    new_text: e.new_text,
                })
                .collect();
            changes.insert(uri.to_string(), converted);
        }
    }

    if let Some(doc_changes) = edit.document_changes {
        use lsp_types::DocumentChanges;
        let operations = match doc_changes {
            DocumentChanges::Edits(edits) => edits
                .into_iter()
                .map(lsp_types::DocumentChangeOperation::Edit)
                .collect::<Vec<_>>(),
            DocumentChanges::Operations(ops) => ops,
        };
        for op in operations {
            match op {
                lsp_types::DocumentChangeOperation::Edit(text_doc_edit) => {
                    let uri_str = text_doc_edit.text_document.uri.to_string();
                    let version = text_doc_edit.text_document.version;
                    let edits: Vec<TextEditInfo> = text_doc_edit
                        .edits
                        .into_iter()
                        .map(|e| match e {
                            lsp_types::OneOf::Left(edit) => TextEditInfo {
                                range: lsp_to_range(edit.range),
                                new_text: edit.new_text,
                            },
                            lsp_types::OneOf::Right(annotated) => TextEditInfo {
                                range: lsp_to_range(annotated.text_edit.range),
                                new_text: annotated.text_edit.new_text,
                            },
                        })
                        .collect();
                    document_changes.push(DocumentChange::Edit {
                        uri: uri_str,
                        version,
                        edits,
                    });
                }
                lsp_types::DocumentChangeOperation::Op(resource_op) => match resource_op {
                    lsp_types::ResourceOp::Create(create) => {
                        document_changes.push(DocumentChange::Create {
                            uri: create.uri.to_string(),
                        });
                    }
                    lsp_types::ResourceOp::Rename(rename) => {
                        document_changes.push(DocumentChange::Rename {
                            old_uri: rename.old_uri.to_string(),
                            new_uri: rename.new_uri.to_string(),
                        });
                    }
                    lsp_types::ResourceOp::Delete(delete) => {
                        document_changes.push(DocumentChange::Delete {
                            uri: delete.uri.to_string(),
                        });
                    }
                },
            }
        }
    }

    RenameResult {
        changes,
        document_changes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_info_serialize() {
        let info = RenameInfo {
            range: sidex_text::Range::new(
                sidex_text::Position::new(5, 10),
                sidex_text::Position::new(5, 15),
            ),
            placeholder: "old_name".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: RenameInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.placeholder, "old_name");
    }

    #[test]
    fn prepare_rename_result_serialize() {
        let result = PrepareRenameResult {
            range: sidex_text::Range::new(
                sidex_text::Position::new(1, 0),
                sidex_text::Position::new(1, 5),
            ),
            placeholder: "foo".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: PrepareRenameResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.placeholder, "foo");
    }

    #[test]
    fn workspace_edit_default_empty() {
        let edit = WorkspaceEdit::default();
        assert!(edit.changes.is_empty());
    }

    #[test]
    fn rename_result_default_empty() {
        let result = RenameResult::default();
        assert_eq!(result.total_edits(), 0);
        assert_eq!(result.file_count(), 0);
    }

    #[test]
    fn rename_result_counts() {
        let mut changes = HashMap::new();
        changes.insert(
            "file:///a.rs".into(),
            vec![
                TextEditInfo {
                    range: sidex_text::Range::new(
                        sidex_text::Position::ZERO,
                        sidex_text::Position::new(0, 3),
                    ),
                    new_text: "bar".into(),
                },
                TextEditInfo {
                    range: sidex_text::Range::new(
                        sidex_text::Position::new(5, 0),
                        sidex_text::Position::new(5, 3),
                    ),
                    new_text: "bar".into(),
                },
            ],
        );
        let result = RenameResult {
            changes,
            document_changes: vec![DocumentChange::Edit {
                uri: "file:///b.rs".into(),
                version: Some(1),
                edits: vec![TextEditInfo {
                    range: sidex_text::Range::new(
                        sidex_text::Position::ZERO,
                        sidex_text::Position::new(0, 3),
                    ),
                    new_text: "bar".into(),
                }],
            }],
        };
        assert_eq!(result.total_edits(), 3);
        assert_eq!(result.file_count(), 2);
    }

    #[test]
    fn document_change_serde() {
        let dc = DocumentChange::Rename {
            old_uri: "file:///old.rs".into(),
            new_uri: "file:///new.rs".into(),
        };
        let json = serde_json::to_string(&dc).unwrap();
        let back: DocumentChange = serde_json::from_str(&json).unwrap();
        match back {
            DocumentChange::Rename { old_uri, new_uri } => {
                assert_eq!(old_uri, "file:///old.rs");
                assert_eq!(new_uri, "file:///new.rs");
            }
            _ => panic!("expected Rename"),
        }
    }

    #[test]
    fn convert_workspace_edit_from_changes() {
        let mut raw_changes = HashMap::new();
        raw_changes.insert(
            "file:///test.rs".parse::<Uri>().unwrap(),
            vec![lsp_types::TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(1, 0),
                    lsp_types::Position::new(1, 5),
                ),
                new_text: "new_name".into(),
            }],
        );
        let lsp_edit = lsp_types::WorkspaceEdit {
            changes: Some(raw_changes),
            document_changes: None,
            change_annotations: None,
        };
        let result = convert_workspace_edit(lsp_edit);
        assert_eq!(result.changes.len(), 1);
        let edits = result.changes.values().next().unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "new_name");
    }

    #[test]
    fn text_edit_info_fields() {
        let edit = TextEditInfo {
            range: sidex_text::Range::new(
                sidex_text::Position::new(0, 0),
                sidex_text::Position::new(0, 5),
            ),
            new_text: "replacement".into(),
        };
        assert_eq!(edit.new_text, "replacement");
        assert_eq!(edit.range.start.column, 0);
        assert_eq!(edit.range.end.column, 5);
    }

    #[test]
    fn workspace_edit_serialize() {
        let mut changes = HashMap::new();
        changes.insert(
            "file:///a.rs".into(),
            vec![TextEditInfo {
                range: sidex_text::Range::new(
                    sidex_text::Position::new(0, 0),
                    sidex_text::Position::new(0, 3),
                ),
                new_text: "bar".into(),
            }],
        );
        let edit = WorkspaceEdit { changes };
        let json = serde_json::to_string(&edit).unwrap();
        let back: WorkspaceEdit = serde_json::from_str(&json).unwrap();
        assert_eq!(back.changes.len(), 1);
    }
}
