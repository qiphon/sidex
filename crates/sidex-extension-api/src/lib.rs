//! VS Code extension API compatibility layer for `SideX`.
//!
//! This crate bridges the gap between VS Code's extension API and `SideX`'s
//! native Rust subsystems. The extension host sends JSON-RPC calls which are
//! dispatched through [`ExtensionApiHandler`] to the appropriate API shim:
//!
//! - [`window`] — `vscode.window` (messages, quick picks, output channels,
//!   webviews, tree views, status bar, progress, file dialogs, URI handlers)
//! - [`workspace_api`] — `vscode.workspace` (configuration, documents, edits,
//!   file system providers, virtual documents, multi-root workspace, watchers,
//!   document/config change events)
//! - [`commands_api`] — `vscode.commands` (command registry)
//! - [`languages_api`] — `vscode.languages` (all language feature providers,
//!   semantic tokens, language configuration)
//! - [`debug_api`] — `vscode.debug` (full DAP support, adapter factories,
//!   configuration providers, breakpoints, session events)
//! - [`tasks_api`] — `vscode.tasks` (task providers, execution, lifecycle)
//! - [`scm_api`] — `vscode.scm` (source control providers, resource groups,
//!   decorations, input box, status bar integration)
//! - [`test_api`] — `vscode.tests` (test controllers, items, runs, profiles,
//!   results, messages)
//! - [`env_api`] — `vscode.env` (app name, language, clipboard, shell, UI kind,
//!   log level, telemetry)
//! - [`types`] — Core types: `Uri`, `Position`, `Range`, `Selection`,
//!   `Disposable`, `CancellationToken`, `EventEmitter`, `DiagnosticCollection`,
//!   `AuthenticationProvider`, `NotebookSerializer`, `CustomEditorProvider`

pub mod api;
pub mod commands_api;
pub mod debug_api;
pub mod env_api;
pub mod languages_api;
pub mod message_handler;
pub mod scm_api;
pub mod tasks_api;
pub mod test_api;
pub mod types;
pub mod window;
pub mod workspace_api;

pub use api::ExtensionApiHandler;
pub use message_handler::{
    activate_extension, handle_ext_host_message, notify_active_editor_changed,
    notify_configuration_changed, notify_document_changed, notify_document_closed,
    notify_document_opened, notify_document_saved, notify_extension_host,
    notify_visible_editors_changed, notify_workspace_folders_changed, request_code_actions,
    request_completions, request_definition, request_document_formatting, request_extension_host,
    request_hover, request_references, set_decorations, start_extension_host,
};
pub use commands_api::{CommandHandler, CommandRegistry};
pub use debug_api::{
    BreakpointLocation, DapEvent, DapRequest, DapRequestKind, DapResponse, DebugAdapterDescriptor,
    DebugApi, DebugConfiguration, DebugSessionId,
};
pub use env_api::{EnvApi, LogLevel, UiKind};
pub use languages_api::{LanguageConfiguration, LanguagesApi, ProviderKind, SemanticTokensLegend};
pub use scm_api::{ScmApi, SourceControlId, SourceControlResourceState};
pub use tasks_api::{Task, TaskDefinition, TaskExecutionId, TasksApi};
pub use test_api::{TestApi, TestControllerId, TestItem, TestMessage, TestRunId, TestRunResult};
pub use types::{
    AuthenticationProviderRegistry, AuthenticationSession, CancellationToken,
    CustomEditorProviderRegistry, Diagnostic, DiagnosticCollection, DiagnosticSeverity, Disposable,
    NotebookSerializerRegistry, Uri,
};
pub use window::{ExtTerminalId, OutputChannelId, StatusBarItemId, WebviewPanelId, WindowApi};
pub use workspace_api::{TextDocumentInfo, WorkspaceApi, WorkspaceFolder};
