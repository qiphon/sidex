//! Simple non-interactive command execution.
//!
//! Provides `exec()` for running a command with optional timeout, capturing
//! stdout/stderr. Ported from `src-tauri/src/commands/process.rs`,
//! removing the Tauri `#[tauri::command]` wrapper.

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

const DEFAULT_EXEC_TIMEOUT_MS: u64 = 30_000;

/// Result of a simple command execution.
#[derive(Debug, Serialize)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}

/// Executes a command (non-interactive) with optional timeout.
#[allow(clippy::implicit_hasher)]
pub async fn exec(
    command: &str,
    args: &[String],
    cwd: Option<&Path>,
    env: Option<&HashMap<String, String>>,
    timeout_ms: Option<u64>,
) -> Result<ExecResult, String> {
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};

    let timeout_duration = Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_EXEC_TIMEOUT_MS));

    let mut cmd = Command::new(command);
    cmd.args(args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    if let Some(env_vars) = env {
        for (k, v) in env_vars {
            cmd.env(k, v);
        }
    }

    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn '{command}': {e}"))?;

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        if let Some(mut stdout) = stdout_handle {
            let mut buf = String::new();
            let _ = stdout.read_to_string(&mut buf).await;
            buf
        } else {
            String::new()
        }
    });

    let stderr_task = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        if let Some(mut stderr) = stderr_handle {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf).await;
            buf
        } else {
            String::new()
        }
    });

    let result = timeout(timeout_duration, child.wait()).await;

    match result {
        Ok(Ok(status)) => {
            let stdout = stdout_task.await.unwrap_or_default();
            let stderr = stderr_task.await.unwrap_or_default();
            Ok(ExecResult {
                stdout,
                stderr,
                exit_code: status.code(),
                timed_out: false,
            })
        }
        Ok(Err(e)) => Err(format!("process error: {e}")),
        Err(_) => {
            let _ = child.kill().await;
            let stdout = stdout_task.await.unwrap_or_default();
            let stderr = stderr_task.await.unwrap_or_default();
            Ok(ExecResult {
                stdout,
                stderr,
                exit_code: None,
                timed_out: true,
            })
        }
    }
}
