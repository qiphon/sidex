//! Server capability negotiation helpers.
//!
//! Wraps [`lsp_types::ServerCapabilities`] with convenience methods for
//! checking which features a language server supports.

use lsp_types::ServerCapabilities;

/// Wrapper around [`ServerCapabilities`] providing ergonomic feature checks.
#[derive(Debug, Clone)]
pub struct ServerCaps {
    inner: ServerCapabilities,
}

impl ServerCaps {
    /// Creates a new wrapper from raw server capabilities.
    pub fn new(caps: ServerCapabilities) -> Self {
        Self { inner: caps }
    }

    /// Returns a reference to the underlying [`ServerCapabilities`].
    pub fn raw(&self) -> &ServerCapabilities {
        &self.inner
    }

    /// Whether the server supports `textDocument/completion`.
    pub fn supports_completion(&self) -> bool {
        self.inner.completion_provider.is_some()
    }

    /// Whether the server supports `textDocument/hover`.
    pub fn supports_hover(&self) -> bool {
        self.inner.hover_provider.is_some()
    }

    /// Whether the server supports `textDocument/definition`.
    pub fn supports_goto_definition(&self) -> bool {
        self.inner.definition_provider.is_some()
    }

    /// Whether the server supports `textDocument/references`.
    pub fn supports_references(&self) -> bool {
        self.inner.references_provider.is_some()
    }

    /// Whether the server supports `textDocument/rename`.
    pub fn supports_rename(&self) -> bool {
        self.inner.rename_provider.is_some()
    }

    /// Whether the server supports `textDocument/formatting`.
    pub fn supports_formatting(&self) -> bool {
        self.inner.document_formatting_provider.is_some()
    }

    /// Whether the server supports `textDocument/codeAction`.
    pub fn supports_code_action(&self) -> bool {
        self.inner.code_action_provider.is_some()
    }

    /// Whether the server supports `textDocument/signatureHelp`.
    pub fn supports_signature_help(&self) -> bool {
        self.inner.signature_help_provider.is_some()
    }

    /// Whether the server supports `textDocument/documentSymbol`.
    pub fn supports_document_symbols(&self) -> bool {
        self.inner.document_symbol_provider.is_some()
    }

    /// Whether the server supports `workspace/symbol`.
    pub fn supports_workspace_symbols(&self) -> bool {
        self.inner.workspace_symbol_provider.is_some()
    }

    /// Whether the server supports `textDocument/inlayHint`.
    pub fn supports_inlay_hints(&self) -> bool {
        self.inner.inlay_hint_provider.is_some()
    }

    /// Whether the server supports `textDocument/declaration`.
    pub fn supports_declaration(&self) -> bool {
        self.inner.declaration_provider.is_some()
    }

    /// Whether the server supports `textDocument/typeDefinition`.
    pub fn supports_type_definition(&self) -> bool {
        self.inner.type_definition_provider.is_some()
    }

    /// Whether the server supports `textDocument/implementation`.
    pub fn supports_implementation(&self) -> bool {
        self.inner.implementation_provider.is_some()
    }

    /// Whether the server supports `textDocument/prepareCallHierarchy`.
    pub fn supports_call_hierarchy(&self) -> bool {
        self.inner.call_hierarchy_provider.is_some()
    }

    /// Whether the server supports type hierarchy (via experimental or
    /// call-hierarchy-style registration).
    pub fn supports_type_hierarchy(&self) -> bool {
        self.inner
            .experimental
            .as_ref()
            .and_then(|v| v.get("typeHierarchyProvider"))
            .is_some()
    }

    /// Whether the server supports `textDocument/documentLink`.
    pub fn supports_document_link(&self) -> bool {
        self.inner.document_link_provider.is_some()
    }

    /// Whether the server supports `textDocument/foldingRange`.
    pub fn supports_folding_range(&self) -> bool {
        self.inner.folding_range_provider.is_some()
    }

    /// Whether the server supports `textDocument/selectionRange`.
    pub fn supports_selection_range(&self) -> bool {
        self.inner.selection_range_provider.is_some()
    }

    /// Whether the server supports `textDocument/documentColor`.
    pub fn supports_document_color(&self) -> bool {
        self.inner.color_provider.is_some()
    }

    /// Whether the server supports `textDocument/rangeFormatting`.
    pub fn supports_range_formatting(&self) -> bool {
        self.inner.document_range_formatting_provider.is_some()
    }

    /// Whether the server supports `textDocument/onTypeFormatting`.
    pub fn supports_on_type_formatting(&self) -> bool {
        self.inner.document_on_type_formatting_provider.is_some()
    }

    /// Whether the server supports `textDocument/prepareRename`.
    pub fn supports_prepare_rename(&self) -> bool {
        match &self.inner.rename_provider {
            Some(lsp_types::OneOf::Right(opts)) => opts.prepare_provider == Some(true),
            _ => false,
        }
    }
}

impl From<ServerCapabilities> for ServerCaps {
    fn from(caps: ServerCapabilities) -> Self {
        Self::new(caps)
    }
}
