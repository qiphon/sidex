//! Parse `.vscode/tasks.json` into [`Task`] definitions with variable
//! substitution.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::task::{Task, TaskGroup, TaskPresentation, TaskSource, TaskType};

/// Raw JSON representation of a tasks.json file.
#[derive(Debug, serde::Deserialize)]
struct TasksJsonFile {
    #[serde(default)]
    #[allow(dead_code)]
    version: String,
    #[serde(default)]
    tasks: Vec<RawTask>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTask {
    label: Option<String>,
    #[serde(rename = "type")]
    task_type: Option<String>,
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    options: TaskOptions,
    group: Option<GroupSpec>,
    #[serde(default)]
    presentation: Option<RawPresentation>,
    #[serde(default)]
    problem_matcher: ProblemMatcherSpec,
    #[serde(default)]
    depends_on: DependsOnSpec,
    #[serde(default)]
    depends_order: Option<String>,
    #[serde(default)]
    is_background: bool,
    #[serde(default)]
    prompt_on_close: bool,
}

#[derive(Debug, Default, serde::Deserialize)]
struct TaskOptions {
    cwd: Option<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum GroupSpec {
    Simple(String),
    Complex {
        kind: String,
        #[serde(default, rename = "isDefault")]
        is_default: Option<bool>,
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPresentation {
    reveal: Option<String>,
    echo: Option<bool>,
    focus: Option<bool>,
    panel: Option<String>,
    show_reuse_message: Option<bool>,
    clear: Option<bool>,
    close: Option<bool>,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(untagged)]
enum ProblemMatcherSpec {
    #[default]
    None,
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(untagged)]
enum DependsOnSpec {
    #[default]
    None,
    Single(String),
    Multiple(Vec<String>),
}

/// Parses a `tasks.json` file from disk.
pub fn parse_tasks_json(path: &Path) -> Result<Vec<Task>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read tasks.json at {}", path.display()))?;
    parse_tasks_json_str(&text)
}

/// Parses the content of a `tasks.json` string.
pub fn parse_tasks_json_str(json: &str) -> Result<Vec<Task>> {
    let file: TasksJsonFile = serde_json::from_str(json).context("failed to parse tasks.json")?;

    let mut tasks = Vec::with_capacity(file.tasks.len());
    for raw in file.tasks {
        tasks.push(raw_to_task(raw));
    }
    Ok(tasks)
}

fn raw_to_task(raw: RawTask) -> Task {
    let task_type = match raw.task_type.as_deref() {
        Some("shell") | None => TaskType::Shell,
        Some("process") => TaskType::Process,
        Some("npm") => TaskType::Npm,
        Some("gulp") => TaskType::Gulp,
        Some("grunt") => TaskType::Grunt,
        Some("jake") => TaskType::Jake,
        Some("cargo") => TaskType::Cargo,
        Some("make") => TaskType::Make,
        Some("cmake") => TaskType::Cmake,
        Some("msbuild") => TaskType::Msbuild,
        Some(other) => TaskType::Custom(other.to_string()),
    };

    let (group, is_default_build, is_default_test) = match &raw.group {
        Some(GroupSpec::Simple(s)) => match s.as_str() {
            "build" => (TaskGroup::Build, false, false),
            "test" => (TaskGroup::Test, false, false),
            _ => (TaskGroup::None, false, false),
        },
        Some(GroupSpec::Complex { kind, is_default }) => {
            let g = match kind.as_str() {
                "build" => TaskGroup::Build,
                "test" => TaskGroup::Test,
                _ => TaskGroup::None,
            };
            let db = g == TaskGroup::Build && is_default.unwrap_or(false);
            let dt = g == TaskGroup::Test && is_default.unwrap_or(false);
            (g, db, dt)
        }
        None => (TaskGroup::None, false, false),
    };

    let presentation = raw
        .presentation
        .map(|p| {
            let mut pres = TaskPresentation::default();
            if let Some(r) = p.reveal {
                match r.as_str() {
                    "silent" => pres.reveal = crate::task::RevealKind::Silent,
                    "never" => pres.reveal = crate::task::RevealKind::Never,
                    _ => {}
                }
            }
            if let Some(e) = p.echo {
                pres.echo = e;
            }
            if let Some(f) = p.focus {
                pres.focus = f;
            }
            if let Some(panel) = p.panel {
                match panel.as_str() {
                    "dedicated" => pres.panel = crate::task::PanelKind::Dedicated,
                    "new" => pres.panel = crate::task::PanelKind::New,
                    _ => {}
                }
            }
            if let Some(v) = p.show_reuse_message {
                pres.show_reuse_message = v;
            }
            if let Some(v) = p.clear {
                pres.clear = v;
            }
            if let Some(v) = p.close {
                pres.close = v;
            }
            pres
        })
        .unwrap_or_default();

    let problem_matcher = match raw.problem_matcher {
        ProblemMatcherSpec::None => Vec::new(),
        ProblemMatcherSpec::Single(s) => vec![s],
        ProblemMatcherSpec::Multiple(v) => v,
    };

    let depends_on = match raw.depends_on {
        DependsOnSpec::None => Vec::new(),
        DependsOnSpec::Single(s) => vec![s],
        DependsOnSpec::Multiple(v) => v,
    };

    let depends_order = match raw.depends_order.as_deref() {
        Some("sequence") => crate::task::DependsOrder::Sequence,
        _ => crate::task::DependsOrder::Parallel,
    };

    Task {
        name: raw.label.unwrap_or_default(),
        task_type,
        command: raw.command.unwrap_or_default(),
        args: raw.args,
        cwd: raw.options.cwd.map(Into::into),
        env: raw.options.env,
        group,
        presentation,
        problem_matcher,
        source: TaskSource::TasksJson,
        depends_on,
        depends_order,
        is_background: raw.is_background,
        prompt_on_close: raw.prompt_on_close,
        is_default_build,
        is_default_test,
    }
}

/// Substitutes VS Code-style variables in a string:
/// `${workspaceFolder}`, `${file}`, `${fileDirname}`, `${fileBasename}`,
/// `${env:VAR_NAME}`.
#[must_use]
pub fn substitute_variables(input: &str, vars: &VariableContext) -> String {
    let mut result = input.to_string();
    result = result.replace("${workspaceFolder}", &vars.workspace_folder);
    result = result.replace("${file}", &vars.file);
    result = result.replace("${fileDirname}", &vars.file_dirname);
    result = result.replace("${fileBasename}", &vars.file_basename);

    // Handle ${env:VAR} patterns
    while let Some(start) = result.find("${env:") {
        let rest = &result[start + 6..];
        if let Some(end) = rest.find('}') {
            let var_name = &rest[..end];
            let value = std::env::var(var_name).unwrap_or_default();
            let pattern = format!("${{env:{var_name}}}");
            result = result.replace(&pattern, &value);
        } else {
            break;
        }
    }

    result
}

/// Context for variable substitution.
#[derive(Debug, Clone, Default)]
pub struct VariableContext {
    pub workspace_folder: String,
    pub file: String,
    pub file_dirname: String,
    pub file_basename: String,
}

impl VariableContext {
    /// Builds a variable context from a workspace root and the active file path.
    #[must_use]
    pub fn from_paths(workspace_root: &Path, active_file: Option<&Path>) -> Self {
        let workspace_folder = workspace_root.to_string_lossy().into_owned();
        let (file, file_dirname, file_basename) = if let Some(f) = active_file {
            (
                f.to_string_lossy().into_owned(),
                f.parent()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                f.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            )
        } else {
            (String::new(), String::new(), String::new())
        };
        Self {
            workspace_folder,
            file,
            file_dirname,
            file_basename,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "build",
                    "type": "shell",
                    "command": "cargo build"
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "build");
        assert_eq!(tasks[0].task_type, TaskType::Shell);
        assert_eq!(tasks[0].command, "cargo build");
    }

    #[test]
    fn parse_with_group_and_args() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "test",
                    "type": "shell",
                    "command": "cargo",
                    "args": ["test", "--workspace"],
                    "group": "test"
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks[0].group, TaskGroup::Test);
        assert_eq!(tasks[0].args, vec!["test", "--workspace"]);
    }

    #[test]
    fn parse_complex_group() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "build",
                    "command": "make",
                    "group": { "kind": "build" }
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks[0].group, TaskGroup::Build);
    }

    #[test]
    fn parse_presentation() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "lint",
                    "command": "eslint .",
                    "presentation": {
                        "reveal": "silent",
                        "echo": false,
                        "panel": "dedicated"
                    }
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(
            tasks[0].presentation.reveal,
            crate::task::RevealKind::Silent
        );
        assert!(!tasks[0].presentation.echo);
        assert_eq!(
            tasks[0].presentation.panel,
            crate::task::PanelKind::Dedicated
        );
    }

    #[test]
    fn parse_problem_matcher_single() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "tsc",
                    "command": "tsc",
                    "problemMatcher": "$tsc"
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks[0].problem_matcher, vec!["$tsc"]);
    }

    #[test]
    fn parse_problem_matcher_array() {
        let json = r#"{
            "version": "2.0.0",
            "tasks": [
                {
                    "label": "build",
                    "command": "make",
                    "problemMatcher": ["$gcc", "$eslint"]
                }
            ]
        }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert_eq!(tasks[0].problem_matcher, vec!["$gcc", "$eslint"]);
    }

    #[test]
    fn parse_empty_tasks() {
        let json = r#"{ "version": "2.0.0", "tasks": [] }"#;
        let tasks = parse_tasks_json_str(json).unwrap();
        assert!(tasks.is_empty());
    }

    #[test]
    fn substitute_workspace_folder() {
        let ctx = VariableContext {
            workspace_folder: "/home/user/project".into(),
            file: "/home/user/project/src/main.rs".into(),
            file_dirname: "/home/user/project/src".into(),
            file_basename: "main.rs".into(),
        };
        let result = substitute_variables("${workspaceFolder}/build", &ctx);
        assert_eq!(result, "/home/user/project/build");
    }

    #[test]
    fn substitute_file_parts() {
        let ctx = VariableContext {
            workspace_folder: "/ws".into(),
            file: "/ws/src/lib.rs".into(),
            file_dirname: "/ws/src".into(),
            file_basename: "lib.rs".into(),
        };
        assert_eq!(substitute_variables("${file}", &ctx), "/ws/src/lib.rs");
        assert_eq!(substitute_variables("${fileDirname}", &ctx), "/ws/src");
        assert_eq!(substitute_variables("${fileBasename}", &ctx), "lib.rs");
    }

    #[test]
    fn variable_context_from_paths() {
        let ws = Path::new("/workspace");
        let file = Path::new("/workspace/src/main.rs");
        let ctx = VariableContext::from_paths(ws, Some(file));
        assert_eq!(ctx.workspace_folder, "/workspace");
        assert_eq!(ctx.file, "/workspace/src/main.rs");
        assert_eq!(ctx.file_dirname, "/workspace/src");
        assert_eq!(ctx.file_basename, "main.rs");
    }
}
