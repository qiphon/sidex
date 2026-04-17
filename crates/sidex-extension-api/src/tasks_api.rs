//! `vscode.tasks` API compatibility shim.
//!
//! Provides the VS Code Task system API for registering custom task providers,
//! fetching, and executing tasks.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Opaque handles
// ---------------------------------------------------------------------------

/// Opaque handle to a task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskExecutionId(pub u32);

/// Opaque handle to a task provider registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskProviderId(pub u32);

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Task definition (mirrors `vscode.TaskDefinition`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDefinition {
    #[serde(rename = "type")]
    pub task_type: String,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

/// Execution kind for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskExecution {
    Process {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        options: TaskProcessOptions,
    },
    Shell {
        command_line: String,
        #[serde(default)]
        options: TaskProcessOptions,
    },
    Custom {
        #[serde(default)]
        data: Value,
    },
}

/// Process options for task execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProcessOptions {
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub shell: Option<TaskShellOptions>,
}

/// Shell options for task execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskShellOptions {
    #[serde(default)]
    pub executable: Option<String>,
    #[serde(default)]
    pub shell_args: Vec<String>,
}

/// Task group kind (Build, Test, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskGroup {
    Build,
    Test,
    Clean,
    Rebuild,
}

/// Task presentation options.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPresentationOptions {
    #[serde(default)]
    pub reveal: Option<String>,
    #[serde(default)]
    pub echo: bool,
    #[serde(default)]
    pub focus: bool,
    #[serde(default)]
    pub panel: Option<String>,
    #[serde(default)]
    pub show_reuse_message: bool,
    #[serde(default)]
    pub clear: bool,
    #[serde(default)]
    pub close: bool,
}

/// A complete task (mirrors `vscode.Task`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub definition: TaskDefinition,
    pub name: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    pub execution: TaskExecution,
    #[serde(default)]
    pub group: Option<TaskGroup>,
    #[serde(default)]
    pub presentation_options: Option<TaskPresentationOptions>,
    #[serde(default)]
    pub is_background: bool,
    #[serde(default)]
    pub problem_matchers: Vec<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

/// Filter for fetching tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFilter {
    #[serde(rename = "type", default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

/// Event emitted when a task process starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProcessStartEvent {
    pub execution_id: TaskExecutionId,
    pub process_id: u32,
}

/// Event emitted when a task process ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProcessEndEvent {
    pub execution_id: TaskExecutionId,
    pub exit_code: Option<i32>,
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// Task provider callback — returns a list of provided tasks.
pub type TaskProvider = Arc<dyn Fn(&TaskFilter) -> Result<Vec<Task>> + Send + Sync>;

/// Callback for task lifecycle events.
pub type TaskEventListener = Arc<dyn Fn(Value) -> Result<()> + Send + Sync>;

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct TaskProviderEntry {
    id: TaskProviderId,
    task_type: String,
    provider: TaskProvider,
}

#[allow(dead_code)]
struct RunningTask {
    execution_id: TaskExecutionId,
    task: Task,
}

// ---------------------------------------------------------------------------
// TasksApi
// ---------------------------------------------------------------------------

/// Implements the `vscode.tasks.*` API surface.
pub struct TasksApi {
    next_provider: AtomicU32,
    next_execution: AtomicU32,

    providers: RwLock<HashMap<String, TaskProviderEntry>>,
    running: RwLock<HashMap<TaskExecutionId, RunningTask>>,

    on_did_start_task: RwLock<Vec<TaskEventListener>>,
    on_did_end_task: RwLock<Vec<TaskEventListener>>,
}

impl TasksApi {
    /// Creates a new tasks API handler.
    pub fn new() -> Self {
        Self {
            next_provider: AtomicU32::new(1),
            next_execution: AtomicU32::new(1),
            providers: RwLock::new(HashMap::new()),
            running: RwLock::new(HashMap::new()),
            on_did_start_task: RwLock::new(Vec::new()),
            on_did_end_task: RwLock::new(Vec::new()),
        }
    }

    /// Dispatches a tasks API action.
    pub fn handle(&self, action: &str, params: &Value) -> Result<Value> {
        match action {
            "registerTaskProvider" => {
                let task_type = params.get("type").and_then(Value::as_str).unwrap_or("");
                let id = self.register_task_provider(task_type, Arc::new(|_| Ok(Vec::new())));
                Ok(serde_json::to_value(id)?)
            }
            "fetchTasks" => {
                let filter: TaskFilter = params
                    .get("filter")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let tasks = self.fetch_tasks(&filter)?;
                Ok(serde_json::to_value(tasks)?)
            }
            "executeTask" => {
                let task: Task =
                    serde_json::from_value(params.get("task").cloned().unwrap_or(params.clone()))?;
                let id = self.execute_task(task)?;
                Ok(serde_json::to_value(id)?)
            }
            "terminateTask" => {
                let id = params
                    .get("executionId")
                    .and_then(Value::as_u64)
                    .map(|n| TaskExecutionId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing executionId"))?;
                self.terminate_task(id)?;
                Ok(Value::Bool(true))
            }
            "onDidStartTask" => {
                self.on_did_start_task
                    .write()
                    .expect("lock poisoned")
                    .push(Arc::new(|_| Ok(())));
                Ok(Value::Bool(true))
            }
            "onDidEndTask" => {
                self.on_did_end_task
                    .write()
                    .expect("lock poisoned")
                    .push(Arc::new(|_| Ok(())));
                Ok(Value::Bool(true))
            }
            _ => bail!("unknown tasks action: {action}"),
        }
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Registers a custom task provider for a task type.
    pub fn register_task_provider(
        &self,
        task_type: &str,
        provider: TaskProvider,
    ) -> TaskProviderId {
        let raw = self.next_provider.fetch_add(1, Ordering::Relaxed);
        let id = TaskProviderId(raw);
        log::debug!("[ext] registerTaskProvider({task_type}) -> {raw}");
        self.providers
            .write()
            .expect("task providers lock poisoned")
            .insert(
                task_type.to_owned(),
                TaskProviderEntry {
                    id,
                    task_type: task_type.to_owned(),
                    provider,
                },
            );
        id
    }

    // -----------------------------------------------------------------------
    // Fetching
    // -----------------------------------------------------------------------

    /// Fetches all tasks matching the filter.
    pub fn fetch_tasks(&self, filter: &TaskFilter) -> Result<Vec<Task>> {
        log::debug!("[ext] fetchTasks(type={:?})", filter.task_type);
        let providers = self.providers.read().expect("task providers lock poisoned");

        let mut tasks = Vec::new();
        for entry in providers.values() {
            if let Some(ref ft) = filter.task_type {
                if entry.task_type != *ft {
                    continue;
                }
            }
            tasks.extend((entry.provider)(filter)?);
        }
        Ok(tasks)
    }

    // -----------------------------------------------------------------------
    // Execution
    // -----------------------------------------------------------------------

    /// Executes a task and returns its execution handle.
    pub fn execute_task(&self, task: Task) -> Result<TaskExecutionId> {
        let raw = self.next_execution.fetch_add(1, Ordering::Relaxed);
        let id = TaskExecutionId(raw);
        log::info!("[ext] executeTask({}) -> {raw}", task.name);

        self.running
            .write()
            .expect("running tasks lock poisoned")
            .insert(
                id,
                RunningTask {
                    execution_id: id,
                    task,
                },
            );

        let val = serde_json::to_value(id).unwrap_or(Value::Null);
        for listener in self.on_did_start_task.read().expect("lock poisoned").iter() {
            let _ = listener(val.clone());
        }

        Ok(id)
    }

    /// Terminates a running task.
    pub fn terminate_task(&self, id: TaskExecutionId) -> Result<()> {
        log::info!("[ext] terminateTask({})", id.0);
        self.running
            .write()
            .expect("running tasks lock poisoned")
            .remove(&id);

        let val = serde_json::to_value(id).unwrap_or(Value::Null);
        for listener in self.on_did_end_task.read().expect("lock poisoned").iter() {
            let _ = listener(val.clone());
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Event subscriptions (programmatic)
    // -----------------------------------------------------------------------

    /// Subscribes to task start events.
    pub fn on_did_start_task(&self, listener: TaskEventListener) {
        self.on_did_start_task
            .write()
            .expect("lock poisoned")
            .push(listener);
    }

    /// Subscribes to task end events.
    pub fn on_did_end_task(&self, listener: TaskEventListener) {
        self.on_did_end_task
            .write()
            .expect("lock poisoned")
            .push(listener);
    }
}

impl Default for TasksApi {
    fn default() -> Self {
        Self::new()
    }
}
