//! Auto-detect make targets from `Makefile` and turn them into [`Task`]s.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::task::{DependsOrder, Task, TaskGroup, TaskPresentation, TaskSource, TaskType};

/// Detects make targets in the given workspace and returns them as tasks.
///
/// Reads the `Makefile` in `workspace_root` and extracts target names.
pub fn detect_make_tasks(workspace_root: &Path) -> Result<Vec<Task>> {
    let makefile = workspace_root.join("Makefile");
    if !makefile.exists() {
        let makefile_lc = workspace_root.join("makefile");
        if !makefile_lc.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&makefile_lc)
            .with_context(|| format!("failed to read {}", makefile_lc.display()))?;
        return parse_makefile(&text);
    }

    let text = std::fs::read_to_string(&makefile)
        .with_context(|| format!("failed to read {}", makefile.display()))?;
    parse_makefile(&text)
}

/// Parses a Makefile string and extracts targets as tasks.
pub fn parse_makefile(makefile_text: &str) -> Result<Vec<Task>> {
    let mut tasks = Vec::new();

    for target in extract_targets(makefile_text) {
        if target.starts_with('.') {
            continue;
        }

        let group = infer_make_group(&target);

        tasks.push(Task {
            name: format!("make: {target}"),
            task_type: TaskType::Make,
            command: target.clone(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::default(),
            group,
            presentation: TaskPresentation::default(),
            problem_matcher: vec!["$gcc".into()],
            source: TaskSource::Make,
            depends_on: Vec::new(),
            depends_order: DependsOrder::Parallel,
            is_background: false,
            prompt_on_close: false,
            is_default_build: false,
            is_default_test: false,
        });
    }

    Ok(tasks)
}

fn extract_targets(makefile_text: &str) -> Vec<String> {
    let mut targets = Vec::new();

    for line in makefile_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('\t') {
            continue;
        }
        if let Some(colon_pos) = trimmed.find(':') {
            if trimmed[colon_pos + 1..].starts_with('=') {
                continue;
            }
            let target_part = &trimmed[..colon_pos];
            for target in target_part.split_whitespace() {
                if target.contains('%') || target.contains('$') {
                    continue;
                }
                targets.push(target.to_string());
            }
        }
    }

    targets
}

fn infer_make_group(target: &str) -> TaskGroup {
    match target {
        "all" | "build" | "release" | "debug" => TaskGroup::Build,
        "test" | "check" | "tests" => TaskGroup::Test,
        _ => TaskGroup::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_makefile() {
        let makefile = r#"
CC=gcc
CFLAGS=-Wall

all: main.o utils.o
	$(CC) -o app main.o utils.o

main.o: main.c
	$(CC) $(CFLAGS) -c main.c

clean:
	rm -f *.o app

test: all
	./run_tests.sh

.PHONY: all clean test
"#;
        let tasks = parse_makefile(makefile).unwrap();
        assert!(tasks.iter().any(|t| t.name == "make: all"));
        assert!(tasks.iter().any(|t| t.name == "make: clean"));
        assert!(tasks.iter().any(|t| t.name == "make: test"));

        let all = tasks.iter().find(|t| t.name == "make: all").unwrap();
        assert_eq!(all.group, TaskGroup::Build);

        let test = tasks.iter().find(|t| t.name == "make: test").unwrap();
        assert_eq!(test.group, TaskGroup::Test);

        assert!(!tasks.iter().any(|t| t.name.starts_with("make: .")));
    }

    #[test]
    fn extract_targets_skips_variables() {
        let makefile = "FOO:=bar\nall:\n\techo done\n";
        let targets = extract_targets(makefile);
        assert!(targets.contains(&"all".to_string()));
        assert!(!targets.contains(&"FOO".to_string()));
    }

    #[test]
    fn no_makefile_returns_empty() {
        let result = detect_make_tasks(Path::new("/nonexistent/path"));
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn infer_group_works() {
        assert_eq!(infer_make_group("all"), TaskGroup::Build);
        assert_eq!(infer_make_group("build"), TaskGroup::Build);
        assert_eq!(infer_make_group("test"), TaskGroup::Test);
        assert_eq!(infer_make_group("clean"), TaskGroup::None);
        assert_eq!(infer_make_group("install"), TaskGroup::None);
    }
}
