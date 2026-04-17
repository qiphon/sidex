//! Auto-detect cargo targets from `Cargo.toml` and turn them into [`Task`]s.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::task::{DependsOrder, Task, TaskGroup, TaskPresentation, TaskSource, TaskType};

/// Detects cargo tasks in the given workspace and returns them as tasks.
///
/// Reads `Cargo.toml` in `workspace_root` and generates build/test/run/check
/// tasks, plus per-binary targets.
pub fn detect_cargo_tasks(workspace_root: &Path) -> Result<Vec<Task>> {
    let cargo_path = workspace_root.join("Cargo.toml");
    if !cargo_path.exists() {
        return Ok(Vec::new());
    }

    let text = std::fs::read_to_string(&cargo_path)
        .with_context(|| format!("failed to read {}", cargo_path.display()))?;
    parse_cargo_toml(&text)
}

/// Parses a `Cargo.toml` string and extracts tasks.
pub fn parse_cargo_toml(toml_text: &str) -> Result<Vec<Task>> {
    let mut tasks = Vec::new();

    let package_name = extract_package_name(toml_text);

    tasks.push(make_cargo_task(
        "cargo: build",
        "build",
        &[],
        TaskGroup::Build,
        true,
    ));
    tasks.push(make_cargo_task(
        "cargo: build --release",
        "build",
        &["--release"],
        TaskGroup::Build,
        false,
    ));
    tasks.push(make_cargo_task(
        "cargo: test",
        "test",
        &[],
        TaskGroup::Test,
        true,
    ));
    tasks.push(make_cargo_task(
        "cargo: check",
        "check",
        &[],
        TaskGroup::Build,
        false,
    ));
    tasks.push(make_cargo_task(
        "cargo: clippy",
        "clippy",
        &[],
        TaskGroup::None,
        false,
    ));

    if package_name.is_some() {
        tasks.push(make_cargo_task(
            "cargo: run",
            "run",
            &[],
            TaskGroup::None,
            false,
        ));
    }

    for bin in extract_bin_targets(toml_text) {
        tasks.push(make_cargo_task(
            &format!("cargo: run --bin {bin}"),
            "run",
            &["--bin", &bin],
            TaskGroup::None,
            false,
        ));
    }

    Ok(tasks)
}

fn make_cargo_task(
    name: &str,
    subcommand: &str,
    extra_args: &[&str],
    group: TaskGroup,
    is_default: bool,
) -> Task {
    Task {
        name: name.to_string(),
        task_type: TaskType::Cargo,
        command: subcommand.to_string(),
        args: extra_args.iter().map(|s| s.to_string()).collect(),
        cwd: None,
        env: HashMap::default(),
        group,
        presentation: TaskPresentation::default(),
        problem_matcher: vec!["$rustc".into()],
        source: TaskSource::Cargo,
        depends_on: Vec::new(),
        depends_order: DependsOrder::Parallel,
        is_background: false,
        prompt_on_close: false,
        is_default_build: is_default && group == TaskGroup::Build,
        is_default_test: is_default && group == TaskGroup::Test,
    }
}

fn extract_package_name(toml_text: &str) -> Option<String> {
    for line in toml_text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name") {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix('=') {
                let val = rest.trim().trim_matches('"');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn extract_bin_targets(toml_text: &str) -> Vec<String> {
    let mut bins = Vec::new();
    let mut in_bin_section = false;
    for line in toml_text.lines() {
        let trimmed = line.trim();
        if trimmed == "[[bin]]" {
            in_bin_section = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_bin_section = false;
            continue;
        }
        if in_bin_section {
            if let Some(rest) = trimmed.strip_prefix("name") {
                let rest = rest.trim();
                if let Some(rest) = rest.strip_prefix('=') {
                    let val = rest.trim().trim_matches('"');
                    if !val.is_empty() {
                        bins.push(val.to_string());
                    }
                }
            }
        }
    }
    bins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_cargo_toml() {
        let toml = r#"
[package]
name = "my-app"
version = "0.1.0"
"#;
        let tasks = parse_cargo_toml(toml).unwrap();
        assert!(tasks.len() >= 5);

        let build = tasks.iter().find(|t| t.name == "cargo: build").unwrap();
        assert_eq!(build.task_type, TaskType::Cargo);
        assert_eq!(build.source, TaskSource::Cargo);
        assert_eq!(build.group, TaskGroup::Build);

        let run = tasks.iter().find(|t| t.name == "cargo: run").unwrap();
        assert_eq!(run.command, "run");

        let test = tasks.iter().find(|t| t.name == "cargo: test").unwrap();
        assert_eq!(test.group, TaskGroup::Test);
    }

    #[test]
    fn parse_with_bin_targets() {
        let toml = r#"
[package]
name = "multi-bin"

[[bin]]
name = "server"

[[bin]]
name = "client"
"#;
        let tasks = parse_cargo_toml(toml).unwrap();
        assert!(tasks.iter().any(|t| t.name == "cargo: run --bin server"));
        assert!(tasks.iter().any(|t| t.name == "cargo: run --bin client"));
    }

    #[test]
    fn extract_package_name_works() {
        assert_eq!(
            extract_package_name("name = \"foo\""),
            Some("foo".to_string())
        );
        assert_eq!(extract_package_name("version = \"1.0\""), None);
    }

    #[test]
    fn no_cargo_toml_returns_empty() {
        let result = detect_cargo_tasks(Path::new("/nonexistent/path"));
        assert!(result.unwrap().is_empty());
    }
}
