//! Navigation features wrapping LSP go-to and find-references requests.
//!
//! Provides a unified API for definition, declaration, implementation,
//! type definition, and reference lookup. Includes a navigation history
//! stack for back/forward navigation and peek-view support.

use std::str::FromStr;
use std::time::Instant;

use anyhow::{Context, Result};
use lsp_types::{
    GotoDefinitionResponse, PartialResultParams, ReferenceContext, ReferenceParams,
    TextDocumentIdentifier, TextDocumentPositionParams, Uri, WorkDoneProgressParams,
};
use serde::{Deserialize, Serialize};

use crate::client::LspClient;
use crate::conversion::{lsp_to_range, position_to_lsp};

/// A source location (file URI + range).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// File URI (e.g. `"file:///home/user/project/src/main.rs"`).
    pub uri: String,
    /// Range within the file.
    pub range: sidex_text::Range,
}

// ── Navigation history ──────────────────────────────────────────────────────

/// A single entry in the navigation history.
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    pub uri: String,
    pub position: sidex_text::Position,
    pub timestamp: Instant,
}

/// Back/forward navigation history (Alt+Left / Alt+Right).
#[derive(Debug, Clone)]
pub struct NavigationHistory {
    pub back_stack: Vec<NavigationEntry>,
    pub forward_stack: Vec<NavigationEntry>,
    max_size: usize,
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new(100)
    }
}

impl NavigationHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            back_stack: Vec::new(),
            forward_stack: Vec::new(),
            max_size,
        }
    }

    /// Push a navigation point. Clears the forward stack since we've branched.
    pub fn push(&mut self, uri: String, position: sidex_text::Position) {
        self.forward_stack.clear();
        self.back_stack.push(NavigationEntry {
            uri,
            position,
            timestamp: Instant::now(),
        });
        if self.back_stack.len() > self.max_size {
            self.back_stack.remove(0);
        }
    }

    /// Navigate back — returns the previous entry, moving the current
    /// location to the forward stack.
    pub fn go_back(
        &mut self,
        current_uri: &str,
        current_pos: sidex_text::Position,
    ) -> Option<&NavigationEntry> {
        let entry = self.back_stack.pop()?;
        self.forward_stack.push(NavigationEntry {
            uri: current_uri.to_owned(),
            position: current_pos,
            timestamp: Instant::now(),
        });
        self.back_stack.push(entry);
        self.back_stack.last()
    }

    /// Navigate forward — returns the next entry, moving the current
    /// location to the back stack.
    pub fn go_forward(
        &mut self,
        current_uri: &str,
        current_pos: sidex_text::Position,
    ) -> Option<&NavigationEntry> {
        let entry = self.forward_stack.pop()?;
        self.back_stack.push(NavigationEntry {
            uri: current_uri.to_owned(),
            position: current_pos,
            timestamp: Instant::now(),
        });
        self.back_stack.push(entry);
        self.back_stack.last()
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub fn clear(&mut self) {
        self.back_stack.clear();
        self.forward_stack.clear();
    }
}

// ── Peek view ───────────────────────────────────────────────────────────────

/// Result of a peek request — shown as an inline embedded editor.
#[derive(Debug, Clone)]
pub struct PeekResult {
    /// The locations to show in the peek view.
    pub locations: Vec<Location>,
    /// The kind of peek (definition, references, etc.).
    pub kind: PeekKind,
}

/// What kind of peek view to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeekKind {
    Definition,
    Declaration,
    TypeDefinition,
    Implementation,
    References,
}

// ── GoToService ─────────────────────────────────────────────────────────────

/// Stateful service wrapping all go-to features with navigation history.
pub struct GoToService {
    pub history: NavigationHistory,
}

impl Default for GoToService {
    fn default() -> Self {
        Self::new()
    }
}

impl GoToService {
    pub fn new() -> Self {
        Self {
            history: NavigationHistory::default(),
        }
    }

    /// Go to Definition (F12).
    pub async fn go_to_definition(
        &mut self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
    ) -> Result<Vec<Location>> {
        self.history.push(uri.to_owned(), pos);
        goto_definition(client, uri, pos).await
    }

    /// Go to Declaration.
    pub async fn go_to_declaration(
        &mut self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
    ) -> Result<Vec<Location>> {
        self.history.push(uri.to_owned(), pos);
        goto_declaration(client, uri, pos).await
    }

    /// Go to Type Definition.
    pub async fn go_to_type_definition(
        &mut self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
    ) -> Result<Vec<Location>> {
        self.history.push(uri.to_owned(), pos);
        goto_type_definition(client, uri, pos).await
    }

    /// Go to Implementation (Ctrl+F12).
    pub async fn go_to_implementation(
        &mut self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
    ) -> Result<Vec<Location>> {
        self.history.push(uri.to_owned(), pos);
        goto_implementation(client, uri, pos).await
    }

    /// Find References (Shift+F12).
    pub async fn find_references(
        &mut self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>> {
        self.history.push(uri.to_owned(), pos);
        find_references(client, uri, pos, include_declaration).await
    }

    /// Peek Definition (Alt+F12) — returns a `PeekResult` for inline display.
    pub async fn peek_definition(
        &self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
    ) -> Result<PeekResult> {
        let locations = goto_definition(client, uri, pos).await?;
        Ok(PeekResult {
            locations,
            kind: PeekKind::Definition,
        })
    }

    /// Peek References — inline peek view of all references.
    pub async fn peek_references(
        &self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
        include_declaration: bool,
    ) -> Result<PeekResult> {
        let locations = find_references(client, uri, pos, include_declaration).await?;
        Ok(PeekResult {
            locations,
            kind: PeekKind::References,
        })
    }

    /// Peek Implementation.
    pub async fn peek_implementation(
        &self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
    ) -> Result<PeekResult> {
        let locations = goto_implementation(client, uri, pos).await?;
        Ok(PeekResult {
            locations,
            kind: PeekKind::Implementation,
        })
    }

    /// Peek Type Definition.
    pub async fn peek_type_definition(
        &self,
        client: &LspClient,
        uri: &str,
        pos: sidex_text::Position,
    ) -> Result<PeekResult> {
        let locations = goto_type_definition(client, uri, pos).await?;
        Ok(PeekResult {
            locations,
            kind: PeekKind::TypeDefinition,
        })
    }

    /// Go Back (Alt+Left).
    pub fn go_back(
        &mut self,
        current_uri: &str,
        current_pos: sidex_text::Position,
    ) -> Option<(String, sidex_text::Position)> {
        self.history
            .go_back(current_uri, current_pos)
            .map(|e| (e.uri.clone(), e.position))
    }

    /// Go Forward (Alt+Right).
    pub fn go_forward(
        &mut self,
        current_uri: &str,
        current_pos: sidex_text::Position,
    ) -> Option<(String, sidex_text::Position)> {
        self.history
            .go_forward(current_uri, current_pos)
            .map(|e| (e.uri.clone(), e.position))
    }
}

/// Returns a single location if there's exactly one result, or `None` if the
/// caller should show a quick pick / peek view.
pub fn resolve_single(locations: &[Location]) -> Option<&Location> {
    if locations.len() == 1 {
        Some(&locations[0])
    } else {
        None
    }
}

// ── Raw LSP requests ────────────────────────────────────────────────────────

/// `textDocument/definition`
pub async fn goto_definition(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Vec<Location>> {
    let lsp_pos = position_to_lsp(pos);
    let response = client.goto_definition(uri, lsp_pos).await?;
    Ok(convert_goto_response(response))
}

/// `textDocument/declaration`
pub async fn goto_declaration(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Vec<Location>> {
    let lsp_pos = position_to_lsp(pos);
    let params = lsp_types::GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
            position: lsp_pos,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/declaration", Some(val))
        .await?;
    if result.is_null() {
        return Ok(vec![]);
    }
    let response: GotoDefinitionResponse =
        serde_json::from_value(result).context("failed to parse declaration response")?;
    Ok(convert_goto_response(response))
}

/// `textDocument/implementation`
pub async fn goto_implementation(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Vec<Location>> {
    let lsp_pos = position_to_lsp(pos);
    let params = lsp_types::GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
            position: lsp_pos,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/implementation", Some(val))
        .await?;
    if result.is_null() {
        return Ok(vec![]);
    }
    let response: GotoDefinitionResponse =
        serde_json::from_value(result).context("failed to parse implementation response")?;
    Ok(convert_goto_response(response))
}

/// `textDocument/typeDefinition`
pub async fn goto_type_definition(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
) -> Result<Vec<Location>> {
    let lsp_pos = position_to_lsp(pos);
    let params = lsp_types::GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
            position: lsp_pos,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/typeDefinition", Some(val))
        .await?;
    if result.is_null() {
        return Ok(vec![]);
    }
    let response: GotoDefinitionResponse =
        serde_json::from_value(result).context("failed to parse typeDefinition response")?;
    Ok(convert_goto_response(response))
}

/// `textDocument/references`
pub async fn find_references(
    client: &LspClient,
    uri: &str,
    pos: sidex_text::Position,
    include_declaration: bool,
) -> Result<Vec<Location>> {
    let lsp_pos = position_to_lsp(pos);
    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier::new(Uri::from_str(uri).context("invalid URI")?),
            position: lsp_pos,
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: ReferenceContext {
            include_declaration,
        },
    };
    let val = serde_json::to_value(params)?;
    let result = client
        .raw_request("textDocument/references", Some(val))
        .await?;
    if result.is_null() {
        return Ok(vec![]);
    }
    let locations: Vec<lsp_types::Location> =
        serde_json::from_value(result).context("failed to parse references response")?;
    Ok(locations.iter().map(convert_location).collect())
}

fn convert_goto_response(response: GotoDefinitionResponse) -> Vec<Location> {
    match response {
        GotoDefinitionResponse::Scalar(loc) => vec![convert_location(&loc)],
        GotoDefinitionResponse::Array(locs) => locs.iter().map(convert_location).collect(),
        GotoDefinitionResponse::Link(links) => links
            .into_iter()
            .map(|link| Location {
                uri: link.target_uri.to_string(),
                range: lsp_to_range(link.target_selection_range),
            })
            .collect(),
    }
}

fn convert_location(loc: &lsp_types::Location) -> Location {
    Location {
        uri: loc.uri.to_string(),
        range: lsp_to_range(loc.range),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_scalar_response() {
        let response = GotoDefinitionResponse::Scalar(lsp_types::Location {
            uri: "file:///test.rs".parse().unwrap(),
            range: lsp_types::Range::new(
                lsp_types::Position::new(5, 0),
                lsp_types::Position::new(5, 10),
            ),
        });
        let locations = convert_goto_response(response);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].uri.contains("test.rs"));
        assert_eq!(locations[0].range.start.line, 5);
    }

    #[test]
    fn convert_array_response() {
        let response = GotoDefinitionResponse::Array(vec![
            lsp_types::Location {
                uri: "file:///a.rs".parse().unwrap(),
                range: lsp_types::Range::new(
                    lsp_types::Position::new(1, 0),
                    lsp_types::Position::new(1, 5),
                ),
            },
            lsp_types::Location {
                uri: "file:///b.rs".parse().unwrap(),
                range: lsp_types::Range::new(
                    lsp_types::Position::new(10, 0),
                    lsp_types::Position::new(10, 5),
                ),
            },
        ]);
        let locations = convert_goto_response(response);
        assert_eq!(locations.len(), 2);
    }

    #[test]
    fn convert_link_response() {
        let response = GotoDefinitionResponse::Link(vec![lsp_types::LocationLink {
            origin_selection_range: None,
            target_uri: "file:///target.rs".parse().unwrap(),
            target_range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(10, 0),
            ),
            target_selection_range: lsp_types::Range::new(
                lsp_types::Position::new(3, 4),
                lsp_types::Position::new(3, 15),
            ),
        }]);
        let locations = convert_goto_response(response);
        assert_eq!(locations.len(), 1);
        assert!(locations[0].uri.contains("target.rs"));
        assert_eq!(locations[0].range.start.line, 3);
        assert_eq!(locations[0].range.start.column, 4);
    }

    #[test]
    fn location_serialize() {
        let loc = Location {
            uri: "file:///test.rs".into(),
            range: sidex_text::Range::new(
                sidex_text::Position::new(1, 2),
                sidex_text::Position::new(3, 4),
            ),
        };
        let json = serde_json::to_string(&loc).unwrap();
        let back: Location = serde_json::from_str(&json).unwrap();
        assert_eq!(loc, back);
    }

    #[test]
    fn empty_array_response() {
        let response = GotoDefinitionResponse::Array(vec![]);
        let locations = convert_goto_response(response);
        assert!(locations.is_empty());
    }

    #[test]
    fn navigation_history_push_and_back() {
        let mut history = NavigationHistory::new(10);
        history.push("file:///a.rs".into(), sidex_text::Position::new(1, 0));
        history.push("file:///b.rs".into(), sidex_text::Position::new(5, 0));

        assert!(history.can_go_back());
        assert!(!history.can_go_forward());

        let entry = history
            .go_back("file:///c.rs", sidex_text::Position::new(10, 0))
            .unwrap();
        assert_eq!(entry.uri, "file:///b.rs");
        assert!(history.can_go_forward());
    }

    #[test]
    fn navigation_history_forward() {
        let mut history = NavigationHistory::new(10);
        history.push("file:///a.rs".into(), sidex_text::Position::new(1, 0));
        history.go_back("file:///b.rs", sidex_text::Position::new(2, 0));

        let entry = history
            .go_forward("file:///b.rs", sidex_text::Position::new(2, 0))
            .unwrap();
        assert!(entry.uri.contains("b.rs") || entry.uri.contains("a.rs"));
    }

    #[test]
    fn navigation_history_push_clears_forward() {
        let mut history = NavigationHistory::new(10);
        history.push("file:///a.rs".into(), sidex_text::Position::new(1, 0));
        history.go_back("file:///b.rs", sidex_text::Position::new(2, 0));
        assert!(history.can_go_forward());
        history.push("file:///c.rs".into(), sidex_text::Position::new(3, 0));
        assert!(!history.can_go_forward());
    }

    #[test]
    fn navigation_history_max_size() {
        let mut history = NavigationHistory::new(3);
        for i in 0..5 {
            history.push(format!("file:///{i}.rs"), sidex_text::Position::new(i, 0));
        }
        assert!(history.back_stack.len() <= 3);
    }

    #[test]
    fn navigation_history_clear() {
        let mut history = NavigationHistory::new(10);
        history.push("file:///a.rs".into(), sidex_text::Position::ZERO);
        history.clear();
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn resolve_single_one_location() {
        let locs = vec![Location {
            uri: "file:///a.rs".into(),
            range: sidex_text::Range::new(
                sidex_text::Position::new(0, 0),
                sidex_text::Position::new(0, 5),
            ),
        }];
        assert!(resolve_single(&locs).is_some());
    }

    #[test]
    fn resolve_single_multiple_locations() {
        let locs = vec![
            Location {
                uri: "file:///a.rs".into(),
                range: sidex_text::Range::new(
                    sidex_text::Position::ZERO,
                    sidex_text::Position::ZERO,
                ),
            },
            Location {
                uri: "file:///b.rs".into(),
                range: sidex_text::Range::new(
                    sidex_text::Position::ZERO,
                    sidex_text::Position::ZERO,
                ),
            },
        ];
        assert!(resolve_single(&locs).is_none());
    }

    #[test]
    fn resolve_single_empty() {
        let locs: Vec<Location> = vec![];
        assert!(resolve_single(&locs).is_none());
    }

    #[test]
    fn goto_service_default() {
        let svc = GoToService::default();
        assert!(!svc.history.can_go_back());
        assert!(!svc.history.can_go_forward());
    }

    #[test]
    fn peek_kind_equality() {
        assert_eq!(PeekKind::Definition, PeekKind::Definition);
        assert_ne!(PeekKind::Definition, PeekKind::References);
    }
}
