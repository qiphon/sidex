//! Task runner for `SideX` — npm scripts, make targets, shell commands, and
//! custom tasks defined in `tasks.json`.
//!
//! Mirrors VS Code's `workbench/contrib/tasks` subsystem, providing:
//! - Task types and configuration parsing ([`task`], [`tasks_json`])
//! - Auto-detection of npm scripts ([`npm_task_provider`])
//! - Task execution with streaming output ([`runner`])
//! - Problem-matcher based diagnostics ([`problem_matcher`])

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::similar_names
)]

pub mod cargo_task_provider;
pub mod make_task_provider;
pub mod npm_task_provider;
pub mod problem_matcher;
pub mod runner;
pub mod task;
pub mod tasks_json;

pub use cargo_task_provider::detect_cargo_tasks;
pub use make_task_provider::detect_make_tasks;
pub use npm_task_provider::detect_npm_tasks;
pub use problem_matcher::{
    match_line, parse_problem_output, BackgroundMatcher, DiagnosticSeverity, FileLocation,
    ProblemMatcher, ProblemPattern,
};
pub use runner::{TaskExecution, TaskRunner};
pub use task::{DependsOrder, Task, TaskGroup, TaskPresentation, TaskSource, TaskState, TaskType};
pub use tasks_json::parse_tasks_json;
