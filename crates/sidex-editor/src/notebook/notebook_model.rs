//! Jupyter notebook model.
//!
//! Provides a cell-based document model that can round-trip `.ipynb` files.
//! Each cell contains a [`Document`] for its source text, plus optional
//! outputs and metadata.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::document::Document;

/// The kind of a notebook cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellKind {
    /// An executable code cell.
    Code,
    /// A rich-text markup cell (Markdown).
    Markup,
}

/// A single output of a code cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellOutput {
    /// The output type string (e.g. `"execute_result"`, `"stream"`, `"display_data"`).
    pub output_type: String,
    /// MIME-type → data mapping.
    pub data: HashMap<String, Value>,
    /// Optional output metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// A single notebook cell.
pub struct NotebookCell {
    /// Cell kind (code or markup).
    pub kind: CellKind,
    /// The cell's source text, backed by a full `Document` for editing.
    pub source: Document,
    /// Cell outputs (only meaningful for code cells).
    pub outputs: Vec<CellOutput>,
    /// Arbitrary cell-level metadata.
    pub metadata: Value,
}

impl NotebookCell {
    pub fn new(kind: CellKind) -> Self {
        Self {
            kind,
            source: Document::new(),
            outputs: Vec::new(),
            metadata: Value::Object(serde_json::Map::new()),
        }
    }

    pub fn with_source(kind: CellKind, text: &str) -> Self {
        Self {
            kind,
            source: Document::from_str(text),
            outputs: Vec::new(),
            metadata: Value::Object(serde_json::Map::new()),
        }
    }

    pub fn source_text(&self) -> String {
        self.source.text()
    }
}

/// A notebook consisting of an ordered list of cells.
pub struct Notebook {
    /// The cells of the notebook in order.
    pub cells: Vec<NotebookCell>,
    /// Index of the currently focused cell.
    pub active_cell: usize,
    /// Top-level notebook metadata.
    pub metadata: Value,
    /// nbformat major version (usually 4).
    pub nbformat: u32,
    /// nbformat minor version.
    pub nbformat_minor: u32,
}

impl Notebook {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            active_cell: 0,
            metadata: Value::Object(serde_json::Map::new()),
            nbformat: 4,
            nbformat_minor: 5,
        }
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Add a new empty cell at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index > self.cells.len()`.
    pub fn add_cell(&mut self, index: usize, kind: CellKind) {
        assert!(
            index <= self.cells.len(),
            "add_cell index {index} out of range (len = {})",
            self.cells.len()
        );
        self.cells.insert(index, NotebookCell::new(kind));
    }

    /// Remove the cell at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index >= self.cells.len()`.
    pub fn remove_cell(&mut self, index: usize) {
        assert!(
            index < self.cells.len(),
            "remove_cell index {index} out of range (len = {})",
            self.cells.len()
        );
        self.cells.remove(index);
        if self.active_cell >= self.cells.len() && !self.cells.is_empty() {
            self.active_cell = self.cells.len() - 1;
        }
    }

    /// Move a cell from `from` to `to`.
    pub fn move_cell(&mut self, from: usize, to: usize) {
        if from == to || from >= self.cells.len() || to >= self.cells.len() {
            return;
        }
        let cell = self.cells.remove(from);
        self.cells.insert(to, cell);
        self.active_cell = to;
    }

    /// Placeholder for kernel execution. In a real implementation this would
    /// send the cell source to a Jupyter kernel and populate outputs.
    pub fn execute_cell(&mut self, index: usize) {
        if index >= self.cells.len() {
            return;
        }
        // Placeholder: clear old outputs and add a stub.
        self.cells[index].outputs.clear();
        self.cells[index].outputs.push(CellOutput {
            output_type: "execute_result".to_string(),
            data: {
                let mut m = HashMap::new();
                m.insert(
                    "text/plain".to_string(),
                    Value::String("[execution placeholder]".to_string()),
                );
                m
            },
            metadata: None,
        });
    }

    // ── .ipynb serialization ─────────────────────────────────────

    /// Parse a `.ipynb` JSON string into a `Notebook`.
    pub fn from_ipynb(json: &str) -> Result<Self, serde_json::Error> {
        let root: Value = serde_json::from_str(json)?;

        let nbformat = root.get("nbformat").and_then(Value::as_u64).unwrap_or(4) as u32;
        let nbformat_minor = root
            .get("nbformat_minor")
            .and_then(Value::as_u64)
            .unwrap_or(5) as u32;
        let metadata = root
            .get("metadata")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let mut cells = Vec::new();

        if let Some(raw_cells) = root.get("cells").and_then(Value::as_array) {
            for raw in raw_cells {
                let kind = match raw.get("cell_type").and_then(Value::as_str) {
                    Some("code") => CellKind::Code,
                    _ => CellKind::Markup,
                };

                let source_text = match raw.get("source") {
                    Some(Value::Array(arr)) => arr
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(""),
                    Some(Value::String(s)) => s.clone(),
                    _ => String::new(),
                };

                let outputs =
                    if let Some(raw_outputs) = raw.get("outputs").and_then(Value::as_array) {
                        raw_outputs
                            .iter()
                            .filter_map(|o| serde_json::from_value::<CellOutput>(o.clone()).ok())
                            .collect()
                    } else {
                        Vec::new()
                    };

                let cell_metadata = raw
                    .get("metadata")
                    .cloned()
                    .unwrap_or(Value::Object(serde_json::Map::new()));

                cells.push(NotebookCell {
                    kind,
                    source: Document::from_str(&source_text),
                    outputs,
                    metadata: cell_metadata,
                });
            }
        }

        Ok(Self {
            cells,
            active_cell: 0,
            metadata,
            nbformat,
            nbformat_minor,
        })
    }

    /// Serialize the notebook back to `.ipynb` JSON.
    pub fn to_ipynb(&self) -> String {
        let cells: Vec<Value> = self
            .cells
            .iter()
            .map(|cell| {
                let cell_type = match cell.kind {
                    CellKind::Code => "code",
                    CellKind::Markup => "markdown",
                };

                let source_lines = split_to_ipynb_lines(&cell.source_text());

                let outputs: Vec<Value> = cell
                    .outputs
                    .iter()
                    .filter_map(|o| serde_json::to_value(o).ok())
                    .collect();

                let mut obj = serde_json::Map::new();
                obj.insert("cell_type".into(), Value::String(cell_type.into()));
                obj.insert("source".into(), Value::Array(source_lines));
                obj.insert("metadata".into(), cell.metadata.clone());

                if cell.kind == CellKind::Code {
                    obj.insert("outputs".into(), Value::Array(outputs));
                    obj.insert("execution_count".into(), Value::Null);
                }

                Value::Object(obj)
            })
            .collect();

        let mut root = serde_json::Map::new();
        root.insert("nbformat".into(), Value::from(self.nbformat));
        root.insert("nbformat_minor".into(), Value::from(self.nbformat_minor));
        root.insert("metadata".into(), self.metadata.clone());
        root.insert("cells".into(), Value::Array(cells));

        serde_json::to_string_pretty(&Value::Object(root)).expect("notebook serialization failed")
    }
}

impl Default for Notebook {
    fn default() -> Self {
        Self::new()
    }
}

/// Split a source string into ipynb-style line arrays where each element
/// retains its trailing newline except possibly the last line.
fn split_to_ipynb_lines(s: &str) -> Vec<Value> {
    if s.is_empty() {
        return vec![Value::String(String::new())];
    }

    let mut lines: Vec<Value> = Vec::new();
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        if ch == '\n' {
            lines.push(Value::String(s[start..=i].to_string()));
            start = i + 1;
        }
    }
    if start < s.len() {
        lines.push(Value::String(s[start..].to_string()));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_notebook_is_empty() {
        let nb = Notebook::new();
        assert_eq!(nb.cell_count(), 0);
        assert_eq!(nb.active_cell, 0);
        assert_eq!(nb.nbformat, 4);
    }

    #[test]
    fn add_and_remove_cells() {
        let mut nb = Notebook::new();
        nb.add_cell(0, CellKind::Code);
        nb.add_cell(1, CellKind::Markup);
        assert_eq!(nb.cell_count(), 2);
        assert_eq!(nb.cells[0].kind, CellKind::Code);
        assert_eq!(nb.cells[1].kind, CellKind::Markup);

        nb.remove_cell(0);
        assert_eq!(nb.cell_count(), 1);
        assert_eq!(nb.cells[0].kind, CellKind::Markup);
    }

    #[test]
    fn move_cell() {
        let mut nb = Notebook::new();
        nb.add_cell(0, CellKind::Code);
        nb.add_cell(1, CellKind::Markup);
        nb.add_cell(2, CellKind::Code);
        nb.move_cell(0, 2);
        assert_eq!(nb.cells[0].kind, CellKind::Markup);
        assert_eq!(nb.cells[2].kind, CellKind::Code);
    }

    #[test]
    fn execute_cell_placeholder() {
        let mut nb = Notebook::new();
        nb.add_cell(0, CellKind::Code);
        nb.execute_cell(0);
        assert_eq!(nb.cells[0].outputs.len(), 1);
        assert_eq!(nb.cells[0].outputs[0].output_type, "execute_result");
    }

    #[test]
    fn ipynb_roundtrip() {
        let ipynb = r##"{
            "nbformat": 4,
            "nbformat_minor": 5,
            "metadata": {},
            "cells": [
                {
                    "cell_type": "code",
                    "source": ["print('hello')\n", "print('world')"],
                    "metadata": {},
                    "outputs": [],
                    "execution_count": null
                },
                {
                    "cell_type": "markdown",
                    "source": "# Title",
                    "metadata": {}
                }
            ]
        }"##;

        let nb = Notebook::from_ipynb(ipynb).expect("parse failed");
        assert_eq!(nb.cell_count(), 2);
        assert_eq!(nb.cells[0].kind, CellKind::Code);
        assert!(nb.cells[0].source_text().contains("hello"));
        assert_eq!(nb.cells[1].kind, CellKind::Markup);

        let serialized = nb.to_ipynb();
        let nb2 = Notebook::from_ipynb(&serialized).expect("re-parse failed");
        assert_eq!(nb2.cell_count(), 2);
        assert_eq!(nb2.cells[0].kind, CellKind::Code);
    }

    #[test]
    fn from_ipynb_empty() {
        let ipynb = r#"{"nbformat":4,"nbformat_minor":5,"metadata":{},"cells":[]}"#;
        let nb = Notebook::from_ipynb(ipynb).unwrap();
        assert_eq!(nb.cell_count(), 0);
    }

    #[test]
    fn cell_with_source() {
        let cell = NotebookCell::with_source(CellKind::Code, "x = 1");
        assert_eq!(cell.source_text(), "x = 1");
        assert_eq!(cell.kind, CellKind::Code);
    }

    #[test]
    fn split_lines_preserves_newlines() {
        let lines = split_to_ipynb_lines("a\nb\nc");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], Value::String("a\n".into()));
        assert_eq!(lines[1], Value::String("b\n".into()));
        assert_eq!(lines[2], Value::String("c".into()));
    }

    #[test]
    fn split_lines_empty() {
        let lines = split_to_ipynb_lines("");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    #[should_panic(expected = "add_cell index")]
    fn add_cell_out_of_bounds_panics() {
        let mut nb = Notebook::new();
        nb.add_cell(5, CellKind::Code);
    }

    #[test]
    #[should_panic(expected = "remove_cell index")]
    fn remove_cell_out_of_bounds_panics() {
        let mut nb = Notebook::new();
        nb.remove_cell(0);
    }
}
