//! Apply workspace edits from the language server.
//!
//! Handles text edits, file creation, rename, and deletion. Tracks all changes
//! for bulk undo support. Includes edit preview and atomic application.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::conversion::lsp_to_range;
use crate::rename_engine::TextEditInfo;

// ── ResourceOperation ───────────────────────────────────────────────────────

/// A resource operation — create, rename, or delete a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceOperation {
    Create { uri: String },
    Rename { old_uri: String, new_uri: String },
    Delete { uri: String },
}

// ── AppliedEdit ─────────────────────────────────────────────────────────────

/// Record of a single text edit that was applied, for undo support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedEdit {
    pub uri: String,
    pub range: sidex_text::Range,
    pub new_text: String,
    pub original_text: String,
}

// ── UndoRecord ──────────────────────────────────────────────────────────────

/// Record of all changes applied in a single workspace edit, for bulk undo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UndoRecord {
    pub text_edits: Vec<AppliedEdit>,
    pub resource_ops: Vec<ResourceOperation>,
}

impl UndoRecord {
    pub fn is_empty(&self) -> bool {
        self.text_edits.is_empty() && self.resource_ops.is_empty()
    }
}

// ── DocumentState ───────────────────────────────────────────────────────────

/// State of a single document managed by the editor.
pub struct DocumentState {
    pub uri: String,
    pub content: String,
    pub version: i32,
}

impl DocumentState {
    pub fn new(uri: String, content: String, version: i32) -> Self {
        Self {
            uri,
            content,
            version,
        }
    }
}

// ── EditPreview ─────────────────────────────────────────────────────────────

/// Preview of changes to a single file before applying.
#[derive(Debug, Clone)]
pub struct EditPreview {
    pub file: PathBuf,
    pub uri: String,
    pub original: String,
    pub modified: String,
    pub is_new: bool,
    pub is_renamed: bool,
    pub is_deleted: bool,
    pub edit_count: usize,
}

// ── WorkspaceEditSummary ────────────────────────────────────────────────────

/// Summary of a workspace edit that was applied.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceEditSummary {
    pub files_changed: usize,
    pub files_created: usize,
    pub files_renamed: usize,
    pub files_deleted: usize,
    pub total_edits: usize,
}

impl WorkspaceEditSummary {
    pub fn description(&self) -> String {
        let mut parts = Vec::new();
        if self.total_edits > 0 {
            parts.push(format!(
                "{} edit{} in {} file{}",
                self.total_edits,
                if self.total_edits == 1 { "" } else { "s" },
                self.files_changed,
                if self.files_changed == 1 { "" } else { "s" },
            ));
        }
        if self.files_created > 0 {
            parts.push(format!("{} created", self.files_created));
        }
        if self.files_renamed > 0 {
            parts.push(format!("{} renamed", self.files_renamed));
        }
        if self.files_deleted > 0 {
            parts.push(format!("{} deleted", self.files_deleted));
        }
        if parts.is_empty() {
            "No changes".to_owned()
        } else {
            parts.join(", ")
        }
    }
}

// ── Internal helpers ────────────────────────────────────────────────────────

#[allow(clippy::unnecessary_wraps)]
fn apply_text_edits(
    uri: &str,
    content: &str,
    edits: &[TextEditInfo],
) -> Result<(String, Vec<AppliedEdit>)> {
    let mut sorted: Vec<&TextEditInfo> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.range
            .start
            .cmp(&a.range.start)
            .then_with(|| b.range.end.cmp(&a.range.end))
    });

    let lines: Vec<&str> = content.lines().collect();
    let mut result = content.to_string();
    let mut applied = Vec::with_capacity(sorted.len());

    for edit in sorted {
        let start_offset = position_to_offset(&lines, edit.range.start);
        let end_offset = position_to_offset(&lines, edit.range.end);

        if start_offset > result.len() || end_offset > result.len() {
            continue;
        }

        let original = result[start_offset..end_offset].to_string();

        applied.push(AppliedEdit {
            uri: uri.to_string(),
            range: edit.range,
            new_text: edit.new_text.clone(),
            original_text: original,
        });

        result.replace_range(start_offset..end_offset, &edit.new_text);
    }

    applied.reverse();
    Ok((result, applied))
}

fn position_to_offset(lines: &[&str], pos: sidex_text::Position) -> usize {
    let line = pos.line as usize;
    let col = pos.column as usize;
    let mut offset = 0;
    for (i, l) in lines.iter().enumerate() {
        if i == line {
            return offset + col.min(l.len());
        }
        offset += l.len() + 1;
    }
    offset
}

// ── Preview ─────────────────────────────────────────────────────────────────

/// Previews a workspace edit without applying it.
pub fn preview_workspace_edit(
    edit: &lsp_types::WorkspaceEdit,
    documents: &[DocumentState],
) -> Vec<EditPreview> {
    let mut previews = Vec::new();
    let doc_map: HashMap<String, usize> = documents
        .iter()
        .enumerate()
        .map(|(i, d)| (d.uri.clone(), i))
        .collect();

    if let Some(ref changes) = edit.changes {
        for (uri, text_edits) in changes {
            let uri_str = uri.to_string();
            if let Some(&idx) = doc_map.get(&uri_str) {
                let edits: Vec<TextEditInfo> = text_edits
                    .iter()
                    .map(|e| TextEditInfo {
                        range: lsp_to_range(e.range),
                        new_text: e.new_text.clone(),
                    })
                    .collect();
                let edit_count = edits.len();
                if let Ok((modified, _)) =
                    apply_text_edits(&uri_str, &documents[idx].content, &edits)
                {
                    previews.push(EditPreview {
                        file: uri_to_path(&uri_str).unwrap_or_default(),
                        uri: uri_str,
                        original: documents[idx].content.clone(),
                        modified,
                        is_new: false,
                        is_renamed: false,
                        is_deleted: false,
                        edit_count,
                    });
                }
            }
        }
    }

    if let Some(ref doc_changes) = edit.document_changes {
        let operations = match doc_changes {
            lsp_types::DocumentChanges::Edits(edits) => edits
                .iter()
                .map(|e| lsp_types::DocumentChangeOperation::Edit(e.clone()))
                .collect::<Vec<_>>(),
            lsp_types::DocumentChanges::Operations(ops) => ops.clone(),
        };

        for op in operations {
            match op {
                lsp_types::DocumentChangeOperation::Edit(text_doc_edit) => {
                    let uri_str = text_doc_edit.text_document.uri.to_string();
                    if let Some(&idx) = doc_map.get(&uri_str) {
                        let edits: Vec<TextEditInfo> = text_doc_edit
                            .edits
                            .iter()
                            .map(|e| match e {
                                lsp_types::OneOf::Left(edit) => TextEditInfo {
                                    range: lsp_to_range(edit.range),
                                    new_text: edit.new_text.clone(),
                                },
                                lsp_types::OneOf::Right(annotated) => TextEditInfo {
                                    range: lsp_to_range(annotated.text_edit.range),
                                    new_text: annotated.text_edit.new_text.clone(),
                                },
                            })
                            .collect();
                        let edit_count = edits.len();
                        if let Ok((modified, _)) =
                            apply_text_edits(&uri_str, &documents[idx].content, &edits)
                        {
                            previews.push(EditPreview {
                                file: uri_to_path(&uri_str).unwrap_or_default(),
                                uri: uri_str,
                                original: documents[idx].content.clone(),
                                modified,
                                is_new: false,
                                is_renamed: false,
                                is_deleted: false,
                                edit_count,
                            });
                        }
                    }
                }
                lsp_types::DocumentChangeOperation::Op(resource_op) => match resource_op {
                    lsp_types::ResourceOp::Create(create) => {
                        let uri_str = create.uri.to_string();
                        previews.push(EditPreview {
                            file: uri_to_path(&uri_str).unwrap_or_default(),
                            uri: uri_str,
                            original: String::new(),
                            modified: String::new(),
                            is_new: true,
                            is_renamed: false,
                            is_deleted: false,
                            edit_count: 0,
                        });
                    }
                    lsp_types::ResourceOp::Rename(rename) => {
                        let old_uri = rename.old_uri.to_string();
                        previews.push(EditPreview {
                            file: uri_to_path(&old_uri).unwrap_or_default(),
                            uri: old_uri,
                            original: String::new(),
                            modified: String::new(),
                            is_new: false,
                            is_renamed: true,
                            is_deleted: false,
                            edit_count: 0,
                        });
                    }
                    lsp_types::ResourceOp::Delete(delete) => {
                        let uri_str = delete.uri.to_string();
                        previews.push(EditPreview {
                            file: uri_to_path(&uri_str).unwrap_or_default(),
                            uri: uri_str,
                            original: String::new(),
                            modified: String::new(),
                            is_new: false,
                            is_renamed: false,
                            is_deleted: true,
                            edit_count: 0,
                        });
                    }
                },
            }
        }
    }

    previews
}

// ── Apply ───────────────────────────────────────────────────────────────────

/// Applies a workspace edit across multiple document states.
///
/// Returns an `UndoRecord` that can be used to reverse all changes.
pub fn apply_workspace_edit(
    edit: &lsp_types::WorkspaceEdit,
    documents: &mut [DocumentState],
) -> Result<UndoRecord> {
    let mut undo = UndoRecord::default();
    let mut doc_map: HashMap<String, usize> = documents
        .iter()
        .enumerate()
        .map(|(i, d)| (d.uri.clone(), i))
        .collect();

    if let Some(ref changes) = edit.changes {
        for (uri, text_edits) in changes {
            let uri_str = uri.to_string();
            if let Some(&idx) = doc_map.get(&uri_str) {
                let edits: Vec<TextEditInfo> = text_edits
                    .iter()
                    .map(|e| TextEditInfo {
                        range: lsp_to_range(e.range),
                        new_text: e.new_text.clone(),
                    })
                    .collect();
                let (new_content, applied) =
                    apply_text_edits(&uri_str, &documents[idx].content, &edits)?;
                documents[idx].content = new_content;
                documents[idx].version += 1;
                undo.text_edits.extend(applied);
            }
        }
    }

    if let Some(ref doc_changes) = edit.document_changes {
        let operations = match doc_changes {
            lsp_types::DocumentChanges::Edits(edits) => edits
                .iter()
                .map(|e| lsp_types::DocumentChangeOperation::Edit(e.clone()))
                .collect::<Vec<_>>(),
            lsp_types::DocumentChanges::Operations(ops) => ops.clone(),
        };

        for op in operations {
            match op {
                lsp_types::DocumentChangeOperation::Edit(text_doc_edit) => {
                    let uri_str = text_doc_edit.text_document.uri.to_string();
                    if let Some(&idx) = doc_map.get(&uri_str) {
                        let edits: Vec<TextEditInfo> = text_doc_edit
                            .edits
                            .iter()
                            .map(|e| match e {
                                lsp_types::OneOf::Left(edit) => TextEditInfo {
                                    range: lsp_to_range(edit.range),
                                    new_text: edit.new_text.clone(),
                                },
                                lsp_types::OneOf::Right(annotated) => TextEditInfo {
                                    range: lsp_to_range(annotated.text_edit.range),
                                    new_text: annotated.text_edit.new_text.clone(),
                                },
                            })
                            .collect();
                        let (new_content, applied) =
                            apply_text_edits(&uri_str, &documents[idx].content, &edits)?;
                        documents[idx].content = new_content;
                        documents[idx].version += 1;
                        undo.text_edits.extend(applied);
                    }
                }
                lsp_types::DocumentChangeOperation::Op(resource_op) => match resource_op {
                    lsp_types::ResourceOp::Create(create) => {
                        let uri_str = create.uri.to_string();
                        undo.resource_ops
                            .push(ResourceOperation::Create { uri: uri_str });
                    }
                    lsp_types::ResourceOp::Rename(rename) => {
                        let old_uri = rename.old_uri.to_string();
                        let new_uri = rename.new_uri.to_string();
                        if let Some(idx) = doc_map.remove(&old_uri) {
                            documents[idx].uri.clone_from(&new_uri);
                            doc_map.insert(new_uri.clone(), idx);
                        }
                        undo.resource_ops
                            .push(ResourceOperation::Rename { old_uri, new_uri });
                    }
                    lsp_types::ResourceOp::Delete(delete) => {
                        let uri_str = delete.uri.to_string();
                        undo.resource_ops
                            .push(ResourceOperation::Delete { uri: uri_str });
                    }
                },
            }
        }
    }

    Ok(undo)
}

/// Strips the `file://` prefix from a URI to get a filesystem path.
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    uri.strip_prefix("file://").map(PathBuf::from)
}

/// Applies resource operations (create/rename/delete) to the filesystem.
pub fn apply_resource_ops(ops: &[ResourceOperation]) -> Result<()> {
    for op in ops {
        match op {
            ResourceOperation::Create { uri } => {
                if let Some(path) = uri_to_path(uri) {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).with_context(|| {
                            format!("creating parent dirs for {}", path.display())
                        })?;
                    }
                    if !path.exists() {
                        std::fs::write(&path, "")
                            .with_context(|| format!("creating file {}", path.display()))?;
                    }
                }
            }
            ResourceOperation::Rename { old_uri, new_uri } => {
                if let (Some(old_path), Some(new_path)) =
                    (uri_to_path(old_uri), uri_to_path(new_uri))
                {
                    std::fs::rename(&old_path, &new_path).with_context(|| {
                        format!("renaming {} to {}", old_path.display(), new_path.display())
                    })?;
                }
            }
            ResourceOperation::Delete { uri } => {
                if let Some(path) = uri_to_path(uri) {
                    if path.is_file() {
                        std::fs::remove_file(&path)
                            .with_context(|| format!("deleting {}", path.display()))?;
                    } else if path.is_dir() {
                        std::fs::remove_dir_all(&path)
                            .with_context(|| format!("deleting dir {}", path.display()))?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Builds the inverse `UndoRecord` from an existing one.
#[allow(clippy::cast_possible_truncation)]
pub fn build_undo_edits(record: &UndoRecord) -> Vec<TextEditInfo> {
    record
        .text_edits
        .iter()
        .map(|applied| {
            let replacement_lines: Vec<&str> = applied.new_text.lines().collect();
            let end_line =
                applied.range.start.line + replacement_lines.len().saturating_sub(1) as u32;
            let end_col = if replacement_lines.len() <= 1 {
                applied.range.start.column + applied.new_text.len() as u32
            } else {
                replacement_lines.last().map_or(0, |l| l.len() as u32)
            };

            TextEditInfo {
                range: sidex_text::Range::new(
                    applied.range.start,
                    sidex_text::Position::new(end_line, end_col),
                ),
                new_text: applied.original_text.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_single_edit() {
        let content = "hello world";
        let edits = vec![TextEditInfo {
            range: sidex_text::Range::new(
                sidex_text::Position::new(0, 0),
                sidex_text::Position::new(0, 5),
            ),
            new_text: "goodbye".into(),
        }];
        let (result, applied) = apply_text_edits("file:///t.rs", content, &edits).unwrap();
        assert_eq!(result, "goodbye world");
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].original_text, "hello");
    }

    #[test]
    fn apply_multiple_edits() {
        let content = "aaa bbb ccc";
        let edits = vec![
            TextEditInfo {
                range: sidex_text::Range::new(
                    sidex_text::Position::new(0, 0),
                    sidex_text::Position::new(0, 3),
                ),
                new_text: "111".into(),
            },
            TextEditInfo {
                range: sidex_text::Range::new(
                    sidex_text::Position::new(0, 8),
                    sidex_text::Position::new(0, 11),
                ),
                new_text: "333".into(),
            },
        ];
        let (result, _) = apply_text_edits("file:///t.rs", content, &edits).unwrap();
        assert_eq!(result, "111 bbb 333");
    }

    #[test]
    fn undo_record_empty() {
        let record = UndoRecord::default();
        assert!(record.is_empty());
    }

    #[test]
    fn uri_to_path_strips_prefix() {
        let path = uri_to_path("file:///home/user/test.rs").unwrap();
        assert_eq!(path, PathBuf::from("/home/user/test.rs"));
    }

    #[test]
    fn uri_to_path_no_prefix() {
        assert!(uri_to_path("http://example.com").is_none());
    }

    #[test]
    fn resource_operation_serde() {
        let op = ResourceOperation::Rename {
            old_uri: "file:///a.rs".into(),
            new_uri: "file:///b.rs".into(),
        };
        let json = serde_json::to_string(&op).unwrap();
        let back: ResourceOperation = serde_json::from_str(&json).unwrap();
        match back {
            ResourceOperation::Rename { old_uri, new_uri } => {
                assert_eq!(old_uri, "file:///a.rs");
                assert_eq!(new_uri, "file:///b.rs");
            }
            _ => panic!("expected Rename"),
        }
    }

    #[test]
    fn build_undo_edits_reverses() {
        let record = UndoRecord {
            text_edits: vec![AppliedEdit {
                uri: "file:///t.rs".into(),
                range: sidex_text::Range::new(
                    sidex_text::Position::new(0, 0),
                    sidex_text::Position::new(0, 5),
                ),
                new_text: "goodbye".into(),
                original_text: "hello".into(),
            }],
            resource_ops: vec![],
        };
        let undos = build_undo_edits(&record);
        assert_eq!(undos.len(), 1);
        assert_eq!(undos[0].new_text, "hello");
    }

    #[test]
    fn edit_summary_description() {
        let summary = WorkspaceEditSummary {
            files_changed: 3,
            files_created: 1,
            files_renamed: 0,
            files_deleted: 0,
            total_edits: 7,
        };
        let desc = summary.description();
        assert!(desc.contains("7 edits"));
        assert!(desc.contains("3 files"));
        assert!(desc.contains("1 created"));
    }

    #[test]
    fn edit_summary_no_changes() {
        let summary = WorkspaceEditSummary::default();
        assert_eq!(summary.description(), "No changes");
    }

    #[test]
    fn preview_produces_previews() {
        let docs = vec![DocumentState::new(
            "file:///a.rs".into(),
            "hello world".into(),
            1,
        )];
        let mut changes = HashMap::new();
        changes.insert(
            "file:///a.rs".parse::<lsp_types::Uri>().unwrap(),
            vec![lsp_types::TextEdit {
                range: lsp_types::Range::new(
                    lsp_types::Position::new(0, 0),
                    lsp_types::Position::new(0, 5),
                ),
                new_text: "goodbye".into(),
            }],
        );
        let edit = lsp_types::WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        };
        let previews = preview_workspace_edit(&edit, &docs);
        assert_eq!(previews.len(), 1);
        assert_eq!(previews[0].original, "hello world");
        assert_eq!(previews[0].modified, "goodbye world");
        assert!(!previews[0].is_new);
    }
}
