//! Main API dispatcher for VS Code extension API compatibility.
//!
//! Receives JSON-RPC calls from the extension host and routes them to the
//! appropriate `SideX` subsystem handler based on the VS Code API namespace.

use std::sync::Arc;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::commands_api::CommandRegistry;
use crate::debug_api::DebugApi;
use crate::env_api::EnvApi;
use crate::languages_api::LanguagesApi;
use crate::scm_api::ScmApi;
use crate::tasks_api::TasksApi;
use crate::test_api::TestApi;
use crate::window::WindowApi;
use crate::workspace_api::WorkspaceApi;

/// Dispatches JSON-RPC calls from the extension host to the correct API
/// handler based on the `namespace/method` naming convention used by
/// VS Code's extension API.
pub struct ExtensionApiHandler {
    window: Arc<WindowApi>,
    workspace: Arc<WorkspaceApi>,
    languages: Arc<LanguagesApi>,
    commands: Arc<CommandRegistry>,
    debug: Arc<DebugApi>,
    tasks: Arc<TasksApi>,
    scm: Arc<ScmApi>,
    tests: Arc<TestApi>,
    env: Arc<EnvApi>,
}

impl ExtensionApiHandler {
    /// Creates a new dispatcher with the given subsystem handlers.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        window: Arc<WindowApi>,
        workspace: Arc<WorkspaceApi>,
        languages: Arc<LanguagesApi>,
        commands: Arc<CommandRegistry>,
        debug: Arc<DebugApi>,
        tasks: Arc<TasksApi>,
        scm: Arc<ScmApi>,
        tests: Arc<TestApi>,
        env: Arc<EnvApi>,
    ) -> Self {
        Self {
            window,
            workspace,
            languages,
            commands,
            debug,
            tasks,
            scm,
            tests,
            env,
        }
    }

    /// Dispatches a JSON-RPC method call to the appropriate subsystem.
    ///
    /// Method names use the pattern `"namespace/action"`, e.g.
    /// `"window/showInformationMessage"`.
    pub fn dispatch(&self, method: &str, params: &Value) -> Result<Value> {
        let (namespace, action) = method.split_once('/').unwrap_or((method, ""));

        match namespace {
            "window" => self.window.handle(action, params),
            "workspace" => self.workspace.handle(action, params),
            "languages" => self.languages.handle(action, params),
            "commands" => self.dispatch_commands(action, params),
            "debug" => self.debug.handle(action, params),
            "tasks" => self.tasks.handle(action, params),
            "scm" => self.scm.handle(action, params),
            "tests" | "testing" => self.tests.handle(action, params),
            "env" => self.env.handle(action, params),
            _ => bail!("unknown API namespace: {namespace}"),
        }
    }

    fn dispatch_commands(&self, action: &str, params: &Value) -> Result<Value> {
        match action {
            "executeCommand" => {
                let id = params.get("command").and_then(Value::as_str).unwrap_or("");
                let args = params.get("args").cloned().unwrap_or(Value::Null);
                self.commands.execute(id, args)
            }
            "getCommands" => {
                let cmds = self.commands.get_commands();
                Ok(Value::Array(cmds.into_iter().map(Value::String).collect()))
            }
            _ => bail!("unknown commands action: {action}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands_api::CommandRegistry;
    use crate::debug_api::DebugApi;
    use crate::env_api::EnvApi;
    use crate::languages_api::LanguagesApi;
    use crate::scm_api::ScmApi;
    use crate::tasks_api::TasksApi;
    use crate::test_api::TestApi;
    use crate::window::WindowApi;
    use crate::workspace_api::WorkspaceApi;
    use serde_json::json;

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
    fn dispatch_commands_execute() {
        let handler = make_handler();
        let result = handler
            .dispatch(
                "commands/executeCommand",
                &json!({ "command": "test.hello" }),
            )
            .unwrap();
        assert_eq!(result, json!("world"));
    }

    #[test]
    fn dispatch_commands_get_commands() {
        let handler = make_handler();
        let result = handler
            .dispatch("commands/getCommands", &Value::Null)
            .unwrap();
        let arr = result.as_array().unwrap();
        assert!(arr.contains(&json!("test.hello")));
    }

    #[test]
    fn dispatch_unknown_namespace() {
        let handler = make_handler();
        let result = handler.dispatch("nonexistent/foo", &Value::Null);
        assert!(result.is_err());
    }

    #[test]
    fn dispatch_window_action() {
        let handler = make_handler();
        let result = handler
            .dispatch(
                "window/showInformationMessage",
                &json!({ "message": "hello" }),
            )
            .unwrap();
        assert!(result.is_null() || result.is_string());
    }

    #[test]
    fn dispatch_tasks_action() {
        let handler = make_handler();
        let result = handler
            .dispatch("tasks/registerTaskProvider", &json!({ "type": "npm" }))
            .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn dispatch_scm_action() {
        let handler = make_handler();
        let result = handler
            .dispatch(
                "scm/createSourceControl",
                &json!({ "id": "git", "label": "Git" }),
            )
            .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn dispatch_test_action() {
        let handler = make_handler();
        let result = handler
            .dispatch(
                "tests/createTestController",
                &json!({ "id": "myTests", "label": "My Tests" }),
            )
            .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn dispatch_window_webview() {
        let handler = make_handler();
        let result = handler
            .dispatch(
                "window/createWebviewPanel",
                &json!({ "viewType": "preview", "title": "Preview" }),
            )
            .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn dispatch_workspace_watcher() {
        let handler = make_handler();
        let result = handler
            .dispatch(
                "workspace/createFileSystemWatcher",
                &json!({ "glob": "**/*.rs" }),
            )
            .unwrap();
        assert!(result.is_number());
    }

    #[test]
    fn dispatch_languages_semantic_tokens() {
        let handler = make_handler();
        let result = handler
            .dispatch(
                "languages/registerSemanticTokensProvider",
                &json!({
                    "languageId": "rust",
                    "legend": {
                        "tokenTypes": ["keyword", "variable"],
                        "tokenModifiers": ["declaration"]
                    }
                }),
            )
            .unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn dispatch_debug_adapter_factory() {
        let handler = make_handler();
        let result = handler
            .dispatch(
                "debug/registerDebugAdapterDescriptorFactory",
                &json!({ "type": "node" }),
            )
            .unwrap();
        assert!(result.is_number());
    }
}
