//! `vscode.languages` API compatibility shim.
//!
//! Routes language feature requests (completion, hover, diagnostics, definitions,
//! references, symbols, code actions, code lenses, formatting, rename, signature
//! help, inlay hints, document links, color, folding, selection ranges, semantic
//! tokens, highlights, type definitions, implementations, declarations, and
//! language configuration) to the appropriate LSP server or native provider.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Provider kind enum
// ---------------------------------------------------------------------------

/// Identifies a language feature provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    Completion,
    Hover,
    Definition,
    References,
    DocumentHighlight,
    DocumentSymbol,
    WorkspaceSymbol,
    CodeAction,
    CodeLens,
    DocumentFormatting,
    RangeFormatting,
    Rename,
    SignatureHelp,
    FoldingRange,
    InlayHint,
    DocumentLink,
    Color,
    SelectionRange,
    SemanticTokensFull,
    SemanticTokensRange,
    TypeDefinition,
    Implementation,
    Declaration,
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Document selector entry (mirrors `vscode.DocumentFilter`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentFilter {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub scheme: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
}

/// Semantic tokens legend (mirrors `vscode.SemanticTokensLegend`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticTokensLegend {
    pub token_types: Vec<String>,
    pub token_modifiers: Vec<String>,
}

/// Language configuration (mirrors `vscode.LanguageConfiguration`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageConfiguration {
    #[serde(default)]
    pub comments: Option<CommentRule>,
    #[serde(default)]
    pub brackets: Vec<[String; 2]>,
    #[serde(default)]
    pub word_pattern: Option<String>,
    #[serde(default)]
    pub indentation_rules: Option<IndentationRules>,
    #[serde(default)]
    pub auto_closing_pairs: Vec<AutoClosingPair>,
    #[serde(default)]
    pub surrounding_pairs: Vec<[String; 2]>,
    #[serde(default)]
    pub folding_markers: Option<FoldingMarkers>,
    #[serde(default)]
    pub on_enter_rules: Vec<OnEnterRule>,
}

/// Comment rule configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentRule {
    #[serde(default)]
    pub line_comment: Option<String>,
    #[serde(default)]
    pub block_comment: Option<[String; 2]>,
}

/// Indentation rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndentationRules {
    #[serde(default)]
    pub increase_indent_pattern: Option<String>,
    #[serde(default)]
    pub decrease_indent_pattern: Option<String>,
    #[serde(default)]
    pub indent_next_line_pattern: Option<String>,
    #[serde(default)]
    pub unindented_line_pattern: Option<String>,
}

/// Auto-closing pair.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoClosingPair {
    pub open: String,
    pub close: String,
    #[serde(default)]
    pub not_in: Vec<String>,
}

/// Folding markers for region-based folding.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FoldingMarkers {
    pub start: String,
    pub end: String,
}

/// On-enter rule for auto-indentation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnEnterRule {
    pub before_text: String,
    #[serde(default)]
    pub after_text: Option<String>,
    #[serde(default)]
    pub previously_not_in: Vec<String>,
    pub action: EnterAction,
}

/// Action to take on enter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnterAction {
    pub indent_action: u32,
    #[serde(default)]
    pub append_text: Option<String>,
    #[serde(default)]
    pub remove_text: Option<u32>,
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// Callback invoked when a language feature is requested.
pub type ProviderHandler = Arc<dyn Fn(Value) -> Result<Value> + Send + Sync>;

// ---------------------------------------------------------------------------
// Internal registration
// ---------------------------------------------------------------------------

struct ProviderEntry {
    language_id: String,
    handler: ProviderHandler,
    #[allow(dead_code)]
    trigger_characters: Vec<String>,
    #[allow(dead_code)]
    semantic_legend: Option<SemanticTokensLegend>,
}

// ---------------------------------------------------------------------------
// LanguagesApi
// ---------------------------------------------------------------------------

/// Implements the `vscode.languages.*` API surface.
pub struct LanguagesApi {
    providers: RwLock<HashMap<ProviderKind, Vec<ProviderEntry>>>,
    language_configs: RwLock<HashMap<String, LanguageConfiguration>>,
}

impl LanguagesApi {
    /// Creates a new languages API handler.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            language_configs: RwLock::new(HashMap::new()),
        }
    }

    /// Dispatches a languages API action.
    #[allow(clippy::too_many_lines)]
    pub fn handle(&self, action: &str, params: &Value) -> Result<Value> {
        match action {
            // -- registration --
            "registerCompletionProvider" => {
                self.register_from_params(ProviderKind::Completion, params)
            }
            "registerHoverProvider" => self.register_from_params(ProviderKind::Hover, params),
            "registerDefinitionProvider" => {
                self.register_from_params(ProviderKind::Definition, params)
            }
            "registerReferencesProvider" => {
                self.register_from_params(ProviderKind::References, params)
            }
            "registerDocumentHighlightProvider" => {
                self.register_from_params(ProviderKind::DocumentHighlight, params)
            }
            "registerDocumentSymbolProvider" => {
                self.register_from_params(ProviderKind::DocumentSymbol, params)
            }
            "registerWorkspaceSymbolProvider" => {
                self.register_from_params(ProviderKind::WorkspaceSymbol, params)
            }
            "registerCodeActionProvider" => {
                self.register_from_params(ProviderKind::CodeAction, params)
            }
            "registerCodeLensProvider" => self.register_from_params(ProviderKind::CodeLens, params),
            "registerDocumentFormattingProvider" => {
                self.register_from_params(ProviderKind::DocumentFormatting, params)
            }
            "registerDocumentRangeFormattingProvider" => {
                self.register_from_params(ProviderKind::RangeFormatting, params)
            }
            "registerRenameProvider" => self.register_from_params(ProviderKind::Rename, params),
            "registerSignatureHelpProvider" => {
                self.register_from_params(ProviderKind::SignatureHelp, params)
            }
            "registerFoldingRangeProvider" => {
                self.register_from_params(ProviderKind::FoldingRange, params)
            }
            "registerInlayHintsProvider" => {
                self.register_from_params(ProviderKind::InlayHint, params)
            }
            "registerDocumentLinkProvider" => {
                self.register_from_params(ProviderKind::DocumentLink, params)
            }
            "registerColorProvider" => self.register_from_params(ProviderKind::Color, params),
            "registerSelectionRangeProvider" => {
                self.register_from_params(ProviderKind::SelectionRange, params)
            }
            "registerSemanticTokensProvider" => self.register_semantic_tokens_from_params(params),
            "registerTypeDefinitionProvider" => {
                self.register_from_params(ProviderKind::TypeDefinition, params)
            }
            "registerImplementationProvider" => {
                self.register_from_params(ProviderKind::Implementation, params)
            }
            "registerDeclarationProvider" => {
                self.register_from_params(ProviderKind::Declaration, params)
            }
            "setLanguageConfiguration" => {
                let language_id = params
                    .get("languageId")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let config: LanguageConfiguration = params
                    .get("configuration")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                self.set_language_configuration(language_id, config);
                Ok(Value::Bool(true))
            }

            // -- invocation --
            "provideCompletion" => self.invoke(ProviderKind::Completion, params),
            "provideHover" => self.invoke(ProviderKind::Hover, params),
            "provideDefinition" => self.invoke(ProviderKind::Definition, params),
            "provideReferences" => self.invoke(ProviderKind::References, params),
            "provideDocumentHighlight" => self.invoke(ProviderKind::DocumentHighlight, params),
            "provideDocumentSymbol" => self.invoke(ProviderKind::DocumentSymbol, params),
            "provideWorkspaceSymbol" => self.invoke(ProviderKind::WorkspaceSymbol, params),
            "provideCodeAction" => self.invoke(ProviderKind::CodeAction, params),
            "provideCodeLens" => self.invoke(ProviderKind::CodeLens, params),
            "provideDocumentFormatting" => self.invoke(ProviderKind::DocumentFormatting, params),
            "provideDocumentRangeFormatting" => self.invoke(ProviderKind::RangeFormatting, params),
            "provideRename" => self.invoke(ProviderKind::Rename, params),
            "provideSignatureHelp" => self.invoke(ProviderKind::SignatureHelp, params),
            "provideFoldingRange" => self.invoke(ProviderKind::FoldingRange, params),
            "provideInlayHint" => self.invoke(ProviderKind::InlayHint, params),
            "provideDocumentLink" => self.invoke(ProviderKind::DocumentLink, params),
            "provideColor" => self.invoke(ProviderKind::Color, params),
            "provideSelectionRange" => self.invoke(ProviderKind::SelectionRange, params),
            "provideSemanticTokens" => self.invoke(ProviderKind::SemanticTokensFull, params),
            "provideTypeDefinition" => self.invoke(ProviderKind::TypeDefinition, params),
            "provideImplementation" => self.invoke(ProviderKind::Implementation, params),
            "provideDeclaration" => self.invoke(ProviderKind::Declaration, params),

            _ => bail!("unknown languages action: {action}"),
        }
    }

    // -----------------------------------------------------------------------
    // Public registration API
    // -----------------------------------------------------------------------

    /// Registers a language feature provider.
    pub fn register_provider(
        &self,
        kind: ProviderKind,
        language_id: &str,
        handler: ProviderHandler,
    ) {
        self.register_provider_with_options(kind, language_id, handler, Vec::new(), None);
    }

    /// Registers a provider with trigger characters and optional semantic legend.
    pub fn register_provider_with_options(
        &self,
        kind: ProviderKind,
        language_id: &str,
        handler: ProviderHandler,
        trigger_characters: Vec<String>,
        semantic_legend: Option<SemanticTokensLegend>,
    ) {
        self.providers
            .write()
            .expect("languages lock poisoned")
            .entry(kind)
            .or_default()
            .push(ProviderEntry {
                language_id: language_id.to_owned(),
                handler,
                trigger_characters,
                semantic_legend,
            });
    }

    /// Sets the language configuration for a language id.
    pub fn set_language_configuration(&self, language_id: &str, config: LanguageConfiguration) {
        log::debug!("[ext] setLanguageConfiguration({language_id})");
        self.language_configs
            .write()
            .expect("language configs lock poisoned")
            .insert(language_id.to_owned(), config);
    }

    /// Returns the language configuration for a language id, if set.
    pub fn get_language_configuration(&self, language_id: &str) -> Option<LanguageConfiguration> {
        self.language_configs
            .read()
            .expect("language configs lock poisoned")
            .get(language_id)
            .cloned()
    }

    // -----------------------------------------------------------------------
    // Invocation
    // -----------------------------------------------------------------------

    /// Invokes the first matching provider for a given kind and language.
    fn invoke(&self, kind: ProviderKind, params: &Value) -> Result<Value> {
        let language_id = params
            .get("languageId")
            .and_then(Value::as_str)
            .unwrap_or("");

        let providers = self.providers.read().expect("languages lock poisoned");

        let entries = providers.get(&kind);
        let handler = entries.and_then(|entries| {
            entries
                .iter()
                .find(|e| e.language_id == language_id || e.language_id == "*")
                .map(|e| e.handler.clone())
        });

        match handler {
            Some(h) => h(params.clone()),
            None => Ok(Value::Null),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    #[allow(clippy::unnecessary_wraps)]
    fn register_from_params(&self, kind: ProviderKind, params: &Value) -> Result<Value> {
        let lang = params
            .get("languageId")
            .and_then(Value::as_str)
            .unwrap_or("*");

        let triggers: Vec<String> = params
            .get("triggerCharacters")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        log::debug!("[ext] registerProvider({kind:?}, {lang})");

        self.register_provider_with_options(
            kind,
            lang,
            Arc::new(|_| Ok(Value::Null)),
            triggers,
            None,
        );
        Ok(Value::Bool(true))
    }

    #[allow(clippy::unnecessary_wraps)]
    fn register_semantic_tokens_from_params(&self, params: &Value) -> Result<Value> {
        let lang = params
            .get("languageId")
            .and_then(Value::as_str)
            .unwrap_or("*");

        let legend: Option<SemanticTokensLegend> = params
            .get("legend")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        log::debug!("[ext] registerSemanticTokensProvider({lang})");

        self.register_provider_with_options(
            ProviderKind::SemanticTokensFull,
            lang,
            Arc::new(|_| Ok(Value::Null)),
            Vec::new(),
            legend,
        );
        Ok(Value::Bool(true))
    }
}

impl Default for LanguagesApi {
    fn default() -> Self {
        Self::new()
    }
}
