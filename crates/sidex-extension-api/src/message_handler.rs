//! Main RPC dispatcher for extension host ↔ `SideX` communication.
//!
//! **Inbound** — [`handle_ext_host_message`] routes JSON-RPC calls arriving
//! from the Node.js extension host to the correct [`ExtensionApiHandler`]
//! subsystem.
//!
//! **Outbound** — the `notify_*` / `request_*` family of functions send
//! document-sync events, editor-state notifications, and language-feature
//! requests from `SideX` *to* the extension host.

use anyhow::Result;
use serde_json::{json, Value};

use sidex_extensions::protocol::{
    CompletionContext, DecorationData, EditorInfo, Position, Range, Selection,
};
use sidex_extensions::ExtensionHost;

use crate::api::ExtensionApiHandler;
use crate::workspace_api::TextDocumentContentChange;

// ============================================================================
// Inbound: extension host → SideX
// ============================================================================

/// Routes an incoming JSON-RPC method call from the extension host to the
/// appropriate API handler.
///
/// Method names follow the `"namespace/action"` convention (e.g.
/// `"window/showInformationMessage"`). The [`ExtensionApiHandler`] does the
/// actual fan-out to the subsystem.
pub fn handle_ext_host_message(
    handler: &ExtensionApiHandler,
    method: &str,
    params: &Value,
) -> Result<Value> {
    handler.dispatch(method, params)
}

// ============================================================================
// Outbound helpers: SideX → extension host
// ============================================================================

/// Sends a fire-and-forget notification to the extension host.
pub async fn notify_extension_host(
    host: &ExtensionHost,
    method: &str,
    params: Value,
) -> Result<()> {
    host.send_notification(method, params).await
}

/// Sends a JSON-RPC request to the extension host and returns the response.
pub async fn request_extension_host(
    host: &ExtensionHost,
    method: &str,
    params: Value,
) -> Result<Value> {
    host.send_request(method, params).await
}

// ---------------------------------------------------------------------------
// Document sync notifications
// ---------------------------------------------------------------------------

/// Notifies the extension host that a document was opened.
pub async fn notify_document_opened(
    host: &ExtensionHost,
    uri: &str,
    language_id: &str,
    version: i32,
    text: &str,
) {
    notify_extension_host(
        host,
        "textDocument/didOpen",
        json!({
            "uri": uri,
            "languageId": language_id,
            "version": version,
            "text": text,
        }),
    )
    .await
    .ok();
}

/// Notifies the extension host that a document changed.
pub async fn notify_document_changed(
    host: &ExtensionHost,
    uri: &str,
    version: i32,
    changes: &[TextDocumentContentChange],
) {
    let changes_json: Vec<Value> = changes
        .iter()
        .map(|c| {
            json!({
                "rangeOffset": c.range_offset,
                "rangeLength": c.range_length,
                "text": c.text,
            })
        })
        .collect();

    notify_extension_host(
        host,
        "textDocument/didChange",
        json!({
            "uri": uri,
            "version": version,
            "contentChanges": changes_json,
        }),
    )
    .await
    .ok();
}

/// Notifies the extension host that a document was saved.
pub async fn notify_document_saved(host: &ExtensionHost, uri: &str) {
    notify_extension_host(host, "textDocument/didSave", json!({ "uri": uri }))
        .await
        .ok();
}

/// Notifies the extension host that a document was closed.
pub async fn notify_document_closed(host: &ExtensionHost, uri: &str) {
    notify_extension_host(host, "textDocument/didClose", json!({ "uri": uri }))
        .await
        .ok();
}

// ---------------------------------------------------------------------------
// Language feature requests (SideX → extension host)
// ---------------------------------------------------------------------------

/// Requests completion items from an extension-side provider.
pub async fn request_completions(
    host: &ExtensionHost,
    handle: u64,
    uri: &str,
    position: &Position,
    context: &CompletionContext,
) -> Result<Value> {
    request_extension_host(
        host,
        "$provideCompletionItems",
        json!({
            "handle": handle,
            "uri": uri,
            "position": position,
            "context": context,
        }),
    )
    .await
}

/// Requests hover information from an extension-side provider.
pub async fn request_hover(
    host: &ExtensionHost,
    handle: u64,
    uri: &str,
    position: &Position,
) -> Result<Value> {
    request_extension_host(
        host,
        "$provideHover",
        json!({
            "handle": handle,
            "uri": uri,
            "position": position,
        }),
    )
    .await
}

/// Requests go-to-definition locations from an extension-side provider.
pub async fn request_definition(
    host: &ExtensionHost,
    handle: u64,
    uri: &str,
    position: &Position,
) -> Result<Value> {
    request_extension_host(
        host,
        "$provideDefinition",
        json!({
            "handle": handle,
            "uri": uri,
            "position": position,
        }),
    )
    .await
}

/// Requests code actions from an extension-side provider.
pub async fn request_code_actions(
    host: &ExtensionHost,
    handle: u64,
    uri: &str,
    range: &Range,
    diagnostics: &[Value],
) -> Result<Value> {
    request_extension_host(
        host,
        "$provideCodeActions",
        json!({
            "handle": handle,
            "uri": uri,
            "range": range,
            "diagnostics": diagnostics,
        }),
    )
    .await
}

/// Requests references from an extension-side provider.
pub async fn request_references(
    host: &ExtensionHost,
    handle: u64,
    uri: &str,
    position: &Position,
    include_declaration: bool,
) -> Result<Value> {
    request_extension_host(
        host,
        "$provideReferences",
        json!({
            "handle": handle,
            "uri": uri,
            "position": position,
            "context": { "includeDeclaration": include_declaration },
        }),
    )
    .await
}

/// Requests document formatting edits from an extension-side provider.
pub async fn request_document_formatting(
    host: &ExtensionHost,
    handle: u64,
    uri: &str,
    tab_size: u32,
    insert_spaces: bool,
) -> Result<Value> {
    request_extension_host(
        host,
        "$provideDocumentFormattingEdits",
        json!({
            "handle": handle,
            "uri": uri,
            "options": {
                "tabSize": tab_size,
                "insertSpaces": insert_spaces,
            },
        }),
    )
    .await
}

// ---------------------------------------------------------------------------
// Editor state notifications
// ---------------------------------------------------------------------------

/// Notifies the extension host that the active editor changed.
pub async fn notify_active_editor_changed(
    host: &ExtensionHost,
    uri: Option<&str>,
    selections: &[Selection],
) {
    let params = match uri {
        Some(u) => json!({
            "uri": u,
            "selections": selections,
        }),
        None => json!({
            "uri": null,
            "selections": [],
        }),
    };
    notify_extension_host(host, "$setActiveEditor", params)
        .await
        .ok();
}

/// Notifies the extension host that the set of visible editors changed.
pub async fn notify_visible_editors_changed(host: &ExtensionHost, editors: &[EditorInfo]) {
    notify_extension_host(host, "$setVisibleEditors", json!({ "editors": editors }))
        .await
        .ok();
}

/// Notifies the extension host that a configuration section changed.
pub async fn notify_configuration_changed(host: &ExtensionHost, section: &str) {
    notify_extension_host(host, "$setConfiguration", json!({ "section": section }))
        .await
        .ok();
}

// ---------------------------------------------------------------------------
// Extension lifecycle
// ---------------------------------------------------------------------------

/// Asks the extension host to activate a specific extension.
pub async fn activate_extension(
    host: &ExtensionHost,
    extension_id: &str,
    activation_event: &str,
) -> Result<Value> {
    request_extension_host(
        host,
        "$activateExtension",
        json!({
            "extensionId": extension_id,
            "activationEvent": activation_event,
        }),
    )
    .await
}

/// Sends the full set of extension descriptions to kick off the host.
pub async fn start_extension_host(host: &ExtensionHost, extensions: &[Value]) -> Result<Value> {
    request_extension_host(
        host,
        "$startExtensionHost",
        json!({ "extensions": extensions }),
    )
    .await
}

// ---------------------------------------------------------------------------
// Decoration forwarding
// ---------------------------------------------------------------------------

/// Forwards decoration data to the extension host for rendering.
pub async fn set_decorations(
    host: &ExtensionHost,
    handle: u64,
    uri: &str,
    decorations: &[DecorationData],
) {
    notify_extension_host(
        host,
        "$setDecorations",
        json!({
            "handle": handle,
            "uri": uri,
            "decorations": decorations,
        }),
    )
    .await
    .ok();
}

// ---------------------------------------------------------------------------
// Workspace folder changes
// ---------------------------------------------------------------------------

/// Notifies the extension host about workspace folder additions/removals.
pub async fn notify_workspace_folders_changed(
    host: &ExtensionHost,
    added: &[Value],
    removed: &[Value],
) {
    notify_extension_host(
        host,
        "$onDidChangeWorkspaceFolders",
        json!({ "added": added, "removed": removed }),
    )
    .await
    .ok();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use crate::commands_api::CommandRegistry;
    use crate::debug_api::DebugApi;
    use crate::env_api::EnvApi;
    use crate::languages_api::LanguagesApi;
    use crate::scm_api::ScmApi;
    use crate::tasks_api::TasksApi;
    use crate::test_api::TestApi;
    use crate::window::WindowApi;
    use crate::workspace_api::WorkspaceApi;

    fn make_handler() -> ExtensionApiHandler {
        let commands = Arc::new(CommandRegistry::new());
        commands.register("test.hello", Arc::new(|_| Ok(json!("world"))));

        ExtensionApiHandler::new(
            Arc::new(WindowApi::new()),
            Arc::new(WorkspaceApi::new()),
            Arc::new(LanguagesApi::new()),
            commands,
            Arc::new(DebugApi::new()),
            Arc::new(TasksApi::new()),
            Arc::new(ScmApi::new()),
            Arc::new(TestApi::new()),
            Arc::new(EnvApi::new()),
        )
    }

    #[test]
    fn handle_window_show_info() {
        let handler = make_handler();
        let result = handle_ext_host_message(
            &handler,
            "window/showInformationMessage",
            &json!({ "message": "Hello from extension" }),
        )
        .unwrap();
        assert!(result.is_null() || result.is_string());
    }

    #[test]
    fn handle_commands_execute() {
        let handler = make_handler();
        let result = handle_ext_host_message(
            &handler,
            "commands/executeCommand",
            &json!({ "command": "test.hello" }),
        )
        .unwrap();
        assert_eq!(result, json!("world"));
    }

    #[test]
    fn handle_workspace_get_configuration() {
        let handler = make_handler();
        let result = handle_ext_host_message(
            &handler,
            "workspace/getConfiguration",
            &json!({ "section": "editor" }),
        )
        .unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn handle_unknown_namespace_fails() {
        let handler = make_handler();
        let result = handle_ext_host_message(&handler, "foobar/doThing", &Value::Null);
        assert!(result.is_err());
    }

    #[test]
    fn handle_scm_create_source_control() {
        let handler = make_handler();
        let result = handle_ext_host_message(
            &handler,
            "scm/createSourceControl",
            &json!({ "id": "git", "label": "Git" }),
        )
        .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn handle_debug_start() {
        let handler = make_handler();
        let result = handle_ext_host_message(
            &handler,
            "debug/startDebugging",
            &json!({
                "config": {
                    "type": "node",
                    "name": "Launch",
                    "request": "launch"
                }
            }),
        )
        .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn handle_tasks_register() {
        let handler = make_handler();
        let result = handle_ext_host_message(
            &handler,
            "tasks/registerTaskProvider",
            &json!({ "type": "npm" }),
        )
        .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn handle_tests_create_controller() {
        let handler = make_handler();
        let result = handle_ext_host_message(
            &handler,
            "tests/createTestController",
            &json!({ "id": "rust", "label": "Rust Tests" }),
        )
        .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn handle_languages_register_hover() {
        let handler = make_handler();
        let result = handle_ext_host_message(
            &handler,
            "languages/registerHoverProvider",
            &json!({ "languageId": "rust" }),
        )
        .unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn handle_workspace_save_all() {
        let handler = make_handler();
        let result = handle_ext_host_message(&handler, "workspace/saveAll", &Value::Null).unwrap();
        assert_eq!(result, json!(true));
    }
}
