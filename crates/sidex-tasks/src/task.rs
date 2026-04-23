//! Task types — the core `Task` struct and related configuration types.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The type of task (what runner to use).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TaskType {
    #[default]
    Shell,
    Process,
    Npm,
    Gulp,
    Grunt,
    Jake,
    Cargo,
    Make,
    Cmake,
    Msbuild,
    Custom(String),
}

/// Task grouping — determines where the task appears in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TaskGroup {
    Build,
    Test,
    #[default]
    None,
}

/// How to reveal the terminal panel when the task runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum RevealKind {
    #[default]
    Always,
    Silent,
    Never,
}

/// How the terminal panel is shared across tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PanelKind {
    #[default]
    Shared,
    Dedicated,
    New,
}

/// Ordering for task dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DependsOrder {
    #[default]
    Parallel,
    Sequence,
}

/// Presentation options for how a task appears in the terminal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct TaskPresentation {
    #[serde(default)]
    pub reveal: RevealKind,
    #[serde(default = "default_true")]
    pub echo: bool,
    #[serde(default)]
    pub focus: bool,
    #[serde(default)]
    pub panel: PanelKind,
    #[serde(default = "default_true")]
    pub show_reuse_message: bool,
    #[serde(default)]
    pub clear: bool,
    #[serde(default)]
    pub close: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TaskPresentation {
    fn default() -> Self {
        Self {
            reveal: RevealKind::Always,
            echo: true,
            focus: false,
            panel: PanelKind::Shared,
            show_reuse_message: true,
            clear: false,
            close: false,
        }
    }
}

/// Where a task was defined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskSource {
    #[default]
    TasksJson,
    Npm,
    Make,
    Cargo,
    Gulp,
    Grunt,
    Extension(String),
}

/// The lifecycle state of a running task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Running,
    Completed(i32),
    Failed(String),
    Cancelled,
}

/// A single task definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct Task {
    pub name: String,
    #[serde(rename = "type", default)]
    pub task_type: TaskType,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub group: TaskGroup,
    #[serde(default)]
    pub presentation: TaskPresentation,
    #[serde(default)]
    pub problem_matcher: Vec<String>,
    #[serde(default)]
    pub source: TaskSource,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub depends_order: DependsOrder,
    #[serde(default)]
    pub is_background: bool,
    #[serde(default)]
    pub prompt_on_close: bool,
    #[serde(default)]
    pub is_default_build: bool,
    #[serde(default)]
    pub is_default_test: bool,
}

impl Task {
    /// Creates a minimal shell task.
    #[must_use]
    pub fn shell(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            task_type: TaskType::Shell,
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            group: TaskGroup::None,
            presentation: TaskPresentation::default(),
            problem_matcher: Vec::new(),
            source: TaskSource::TasksJson,
            depends_on: Vec::new(),
            depends_order: DependsOrder::Parallel,
            is_background: false,
            prompt_on_close: false,
            is_default_build: false,
            is_default_test: false,
        }
    }

    /// Returns the full command string including arguments.
    #[must_use]
    pub fn full_command(&self) -> String {
        if self.args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.args.join(" "))
        }
    }

    /// Returns `true` if this is a build task.
    #[must_use]
    pub fn is_build(&self) -> bool {
        self.group == TaskGroup::Build
    }

    /// Returns `true` if this is a test task.
    #[must_use]
    pub fn is_test(&self) -> bool {
        self.group == TaskGroup::Test
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_task() {
        let t = Task::shell("build", "cargo build");
        assert_eq!(t.name, "build");
        assert_eq!(t.task_type, TaskType::Shell);
        assert_eq!(t.command, "cargo build");
        assert!(t.args.is_empty());
    }

    #[test]
    fn full_command_no_args() {
        let t = Task::shell("test", "cargo test");
        assert_eq!(t.full_command(), "cargo test");
    }

    #[test]
    fn full_command_with_args() {
        let mut t = Task::shell("build", "cargo");
        t.args = vec!["build".into(), "--release".into()];
        assert_eq!(t.full_command(), "cargo build --release");
    }

    #[test]
    fn group_predicates() {
        let mut t = Task::shell("x", "x");
        assert!(!t.is_build());
        assert!(!t.is_test());

        t.group = TaskGroup::Build;
        assert!(t.is_build());

        t.group = TaskGroup::Test;
        assert!(t.is_test());
    }

    #[test]
    fn serde_roundtrip() {
        let t = Task::shell("lint", "cargo clippy");
        let json = serde_json::to_string(&t).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "lint");
        assert_eq!(deserialized.command, "cargo clippy");
    }

    #[test]
    fn defaults() {
        assert_eq!(TaskType::default(), TaskType::Shell);
        assert_eq!(TaskGroup::default(), TaskGroup::None);
        assert_eq!(RevealKind::default(), RevealKind::Always);
        assert_eq!(PanelKind::default(), PanelKind::Shared);
    }
}
