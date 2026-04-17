//! # sidex-lsp
//!
//! Language Server Protocol 3.17 client for the `SideX` editor.
//!
//! This crate provides a full-featured LSP client that communicates with
//! language servers over stdio using JSON-RPC 2.0. It includes:
//!
//! - **Transport** — `Content-Length`-framed JSON-RPC over async stdio.
//! - **Client** — high-level async API for all common LSP requests and
//!   notifications.
//! - **Registry** — maps language IDs to server configurations with
//!   sensible built-in defaults.
//! - **Diagnostics** — stores and queries diagnostics per file URI.
//! - **Capabilities** — ergonomic wrappers for server capability
//!   negotiation.
//! - **Conversion** — lossless type mapping between `sidex_text` and
//!   `lsp_types`.
//! - **Completion** — full completion session management with sorting,
//!   filtering, and snippet support.
//! - **Hover** — hover information with plaintext and markdown content.
//! - **Signature help** — function signature / parameter hints.
//! - **Go-to** — definition, declaration, implementation, type definition,
//!   references navigation, peek views, and back/forward history.
//! - **Rename** — prepare-rename, execute-rename, and preview support.
//! - **Code actions** — quick fixes, refactorings, and source organizers.
//! - **Inlay hints** — inline type and parameter annotations.
//! - **Progress** — `$/progress` notification tracking for status bar.
//! - **Document sync** — incremental document synchronization with
//!   throttled change notifications.
//! - **Workspace edit** — apply workspace edits with undo support.
//! - **Format** — document, range, and on-type formatting.
//! - **Call hierarchy** — incoming and outgoing call navigation.
//! - **Type hierarchy** — supertype and subtype navigation.
//! - **Document links** — clickable links in code (file paths, URLs).
//! - **Folding ranges** — collapsible regions from the language server.
//! - **Selection ranges** — smart expand/shrink selection.
//! - **Document colors** — inline color swatches and color picker.

pub mod call_hierarchy;
pub mod capabilities;
pub mod client;
pub mod code_action_engine;
pub mod completion_engine;
pub mod conversion;
pub mod diagnostics;
pub mod document_color;
pub mod document_link;
pub mod document_sync;
pub mod folding;
pub mod format;
pub mod go_to;
pub mod hover_engine;
pub mod inlay_hints;
pub mod progress;
pub mod registry;
pub mod rename_engine;
pub mod selection_range;
pub mod signature_help;
pub mod transport;
pub mod type_hierarchy;
pub mod workspace_edit;

pub use call_hierarchy::{
    incoming_calls, outgoing_calls, prepare_call_hierarchy, CallHierarchyItemInfo,
    CallHierarchyService, IncomingCall, OutgoingCall, SymbolKind, SymbolTag,
};
pub use capabilities::ServerCaps;
pub use client::LspClient;
pub use code_action_engine::{request_code_actions, CodeActionInfo, CodeActionKind};
pub use completion_engine::{
    filter_and_sort, filter_completion_items, fuzzy_score, sort_completion_items,
    CompletionEngine, CompletionList, CompletionSession, CompletionTrigger,
};
pub use conversion::{lsp_to_position, lsp_to_range, position_to_lsp, range_to_lsp};
pub use diagnostics::{
    is_deprecated, is_unnecessary, DiagnosticCollection, DiagnosticCounts, DiagnosticKey,
    DiagnosticManager, QuickFixCache, RelatedInfo,
};
pub use document_color::{
    provide_color_presentations, provide_document_colors, ColorInformation, ColorPresentation,
    DocumentColorService, LspColor,
};
pub use document_link::{provide_document_links, resolve_document_link, DocumentLink, DocumentLinkService};
pub use document_sync::{
    compute_incremental_changes, ChangeEvent, ChangeThrottle, TextDocumentSyncKind,
};
pub use folding::{provide_folding_ranges, FoldingRange, FoldingRangeKind, FoldingRangeService};
pub use format::{format_document, format_on_type, format_range, FormatEdit};
pub use go_to::{
    find_references, goto_declaration, goto_definition, goto_implementation, goto_type_definition,
    resolve_single, GoToService, Location, NavigationEntry, NavigationHistory, PeekKind,
    PeekResult,
};
pub use hover_engine::{
    render_hover_markdown, request_hover, HoverContent, HoverInfo, MarkupContent,
    RenderedHoverBlock,
};
pub use inlay_hints::{request_inlay_hints, InlayHintInfo, InlayHintKind};
pub use progress::{ProgressState, ProgressToken, ProgressTracker, WorkDoneProgress};
pub use registry::{ServerConfig, ServerRegistry};
pub use rename_engine::{
    execute_rename, prepare_rename, DocumentChange, PrepareRenameResult, RenameInfo, RenameResult,
    RenameService, WorkspaceEdit,
};
pub use selection_range::{
    provide_selection_ranges, SelectionRange, SelectionRangeService,
};
pub use signature_help::{
    request_signature, request_signature_state, ParameterInfo, ParameterLabel, SignatureHelpState,
    SignatureInfo,
};
pub use transport::{JsonRpcError, JsonRpcMessage, LspTransport, RequestId};
pub use type_hierarchy::{
    prepare_type_hierarchy, subtypes, supertypes, TypeHierarchyItemInfo, TypeHierarchyService,
};
pub use workspace_edit::{
    apply_resource_ops, apply_workspace_edit, build_undo_edits, preview_workspace_edit,
    DocumentState, EditPreview, ResourceOperation, UndoRecord, WorkspaceEditSummary,
};
