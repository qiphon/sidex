//! Task execution — spawn tasks as child processes and stream their output.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::task::{Task, TaskType};

/// A handle to a running task.
#[derive(Debug)]
pub struct TaskExecution {
    /// The task that was started.
    pub task: Task,
    /// Receiver for output lines from the task's stdout/stderr.
    pub output_rx: mpsc::Receiver<OutputLine>,
    /// Whether the process is still running.
    running: Arc<AtomicBool>,
    /// The child PID, if available.
    pid: Option<u32>,
    /// A handle used to kill the child.
    kill_tx: Option<mpsc::Sender<()>>,
}

/// A single line of output from a running task.
#[derive(Debug, Clone)]
pub struct OutputLine {
    /// The text content.
    pub text: String,
    /// Whether this came from stderr.
    pub is_stderr: bool,
}

impl TaskExecution {
    /// Returns `true` if the task is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Returns the PID of the child process, if available.
    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Sends a kill signal to the running task.
    pub async fn kill(&self) -> Result<()> {
        if let Some(tx) = &self.kill_tx {
            tx.send(()).await.ok();
        }
        Ok(())
    }
}

/// Manages running tasks.
#[derive(Debug, Default)]
pub struct TaskRunner {
    /// Active task executions.
    active_count: u32,
}

impl TaskRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { active_count: 0 }
    }

    /// Returns the number of currently active tasks.
    #[must_use]
    pub fn active_count(&self) -> u32 {
        self.active_count
    }

    /// Runs a task, spawning it as a child process.
    #[allow(clippy::unused_async)]
    pub async fn run(&mut self, task: &Task, workspace_root: &Path) -> Result<TaskExecution> {
        let (cmd, args) = build_command(task);

        let cwd = task.cwd.as_deref().unwrap_or(workspace_root);

        let mut command = Command::new(&cmd);
        command
            .args(&args)
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for (k, v) in &task.env {
            command.env(k, v);
        }

        let mut child = command
            .spawn()
            .with_context(|| format!("failed to spawn task '{}'", task.name))?;

        let pid = child.id();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        let (output_tx, output_rx) = mpsc::channel(256);
        let (kill_tx, mut kill_rx) = mpsc::channel::<()>(1);

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let out_tx = output_tx.clone();
        if let Some(stdout) = stdout {
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if out_tx
                        .send(OutputLine {
                            text: line,
                            is_stderr: false,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }

        let err_tx = output_tx;
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if err_tx
                        .send(OutputLine {
                            text: line,
                            is_stderr: true,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }

        tokio::spawn(async move {
            tokio::select! {
                _ = child.wait() => {}
                _ = kill_rx.recv() => {
                    child.kill().await.ok();
                }
            }
            running_clone.store(false, Ordering::SeqCst);
        });

        self.active_count += 1;

        Ok(TaskExecution {
            task: task.clone(),
            output_rx,
            running,
            pid,
            kill_tx: Some(kill_tx),
        })
    }
}

fn build_command(task: &Task) -> (String, Vec<String>) {
    match task.task_type {
        TaskType::Shell => {
            if cfg!(target_os = "windows") {
                (
                    "cmd".to_string(),
                    vec!["/C".to_string(), task.full_command()],
                )
            } else {
                (
                    "sh".to_string(),
                    vec!["-c".to_string(), task.full_command()],
                )
            }
        }
        TaskType::Process => (task.command.clone(), task.args.clone()),
        TaskType::Npm => {
            let script = task
                .command
                .strip_prefix("npm run ")
                .unwrap_or(&task.command);
            (
                "npm".to_string(),
                vec!["run".to_string(), script.to_string()],
            )
        }
        TaskType::Gulp => {
            let task_name = task
                .command
                .strip_prefix("gulp ")
                .unwrap_or(&task.command);
            (
                "gulp".to_string(),
                vec![task_name.to_string()],
            )
        }
        TaskType::Grunt => {
            let task_name = task
                .command
                .strip_prefix("grunt ")
                .unwrap_or(&task.command);
            (
                "grunt".to_string(),
                vec![task_name.to_string()],
            )
        }
        TaskType::Jake => {
            let task_name = task
                .command
                .strip_prefix("jake ")
                .unwrap_or(&task.command);
            (
                "jake".to_string(),
                vec![task_name.to_string()],
            )
        }
        TaskType::Cargo => {
            let mut args = vec![task.command.clone()];
            args.extend(task.args.clone());
            ("cargo".to_string(), args)
        }
        TaskType::Make => {
            let mut args = task.args.clone();
            if !task.command.is_empty() && task.command != "make" {
                args.insert(0, task.command.clone());
            }
            ("make".to_string(), args)
        }
        TaskType::Cmake => {
            let mut args = vec!["--build".to_string(), ".".to_string()];
            args.extend(task.args.clone());
            ("cmake".to_string(), args)
        }
        TaskType::Msbuild => {
            let mut args = vec![task.command.clone()];
            args.extend(task.args.clone());
            ("msbuild".to_string(), args)
        }
        TaskType::Custom(_) => (
            "sh".to_string(),
            vec!["-c".to_string(), task.full_command()],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_command_shell() {
        let task = Task::shell("test", "cargo test");
        let (cmd, args) = build_command(&task);
        if cfg!(target_os = "windows") {
            assert_eq!(cmd, "cmd");
            assert_eq!(args, vec!["/C", "cargo test"]);
        } else {
            assert_eq!(cmd, "sh");
            assert_eq!(args, vec!["-c", "cargo test"]);
        }
    }

    #[test]
    fn build_command_process() {
        let mut task = Task::shell("node", "node");
        task.task_type = TaskType::Process;
        task.args = vec!["app.js".into()];
        let (cmd, args) = build_command(&task);
        assert_eq!(cmd, "node");
        assert_eq!(args, vec!["app.js"]);
    }

    #[test]
    fn build_command_npm() {
        let mut task = Task::shell("npm: build", "npm run build");
        task.task_type = TaskType::Npm;
        let (cmd, args) = build_command(&task);
        assert_eq!(cmd, "npm");
        assert_eq!(args, vec!["run", "build"]);
    }

    #[test]
    fn runner_starts_with_zero() {
        let runner = TaskRunner::new();
        assert_eq!(runner.active_count(), 0);
    }

    #[tokio::test]
    async fn run_echo_task() {
        let mut runner = TaskRunner::new();
        let task = Task::shell("echo-test", "echo hello");
        let workspace = std::env::temp_dir();
        let mut exec = runner.run(&task, &workspace).await.unwrap();

        assert_eq!(runner.active_count(), 1);
        assert!(exec.pid().is_some());

        let mut saw_hello = false;
        while let Some(line) = exec.output_rx.recv().await {
            if line.text.contains("hello") {
                saw_hello = true;
            }
        }
        assert!(saw_hello);

        // Give the wait task a moment to flip the flag.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!exec.is_running());
    }
}
