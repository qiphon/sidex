use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};

struct DebugAdapterHandle {
    child: Child,
    stdin: Option<std::process::ChildStdin>,
}

pub struct DebugAdapterStore {
    adapters: Mutex<HashMap<u32, DebugAdapterHandle>>,
    next_id: Mutex<u32>,
}

impl DebugAdapterStore {
    pub fn new() -> Self {
        Self {
            adapters: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DebugOutputEvent {
    adapter_id: u32,
    data: String,
}

#[derive(Debug, Clone, Serialize)]
struct DebugErrorEvent {
    adapter_id: u32,
    data: String,
}

#[derive(Debug, Clone, Serialize)]
struct DebugExitEvent {
    adapter_id: u32,
    exit_code: Option<i32>,
}

/// Spawn a debug adapter process and return its `adapter_id`.
/// The adapter communicates via stdin/stdout using the DAP wire protocol.
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
#[tauri::command]
pub fn debug_spawn_adapter(
    app: AppHandle,
    state: State<'_, Arc<DebugAdapterStore>>,
    executable: String,
    args: Option<Vec<String>>,
    cwd: Option<String>,
    env: Option<HashMap<String, String>>,
) -> Result<u32, String> {
    let mut cmd = Command::new(&executable);

    if let Some(ref a) = args {
        cmd.args(a);
    }

    if let Some(ref dir) = cwd {
        if !dir.is_empty() && std::path::Path::new(dir).is_dir() {
            cmd.current_dir(dir);
        }
    }

    if let Some(env_vars) = env {
        for (k, v) in env_vars {
            cmd.env(k, v);
        }
    }

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn debug adapter '{executable}': {e}"))?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to capture debug adapter stdin".to_string())?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture debug adapter stdout".to_string())?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture debug adapter stderr".to_string())?;

    let id = {
        let mut next = state.next_id.lock().map_err(|e| e.to_string())?;
        let id = *next;
        *next += 1;
        id
    };

    {
        let mut adapters = state.adapters.lock().map_err(|e| e.to_string())?;
        adapters.insert(
            id,
            DebugAdapterHandle {
                child,
                stdin: Some(stdin),
            },
        );
    }

    let adapter_id = id;
    let app_stdout = app.clone();
    let state_clone = state.inner().clone();

    // Stdout reader thread: emits `debug-output` events with raw DAP wire data
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_stdout.emit(
                        "debug-output",
                        DebugOutputEvent {
                            adapter_id,
                            data: text,
                        },
                    );
                }
            }
        }

        // Process exited — retrieve exit code
        let exit_code = {
            let Ok(mut adapters) = state_clone.adapters.lock() else {
                let _ = app_stdout.emit(
                    "debug-exit",
                    DebugExitEvent {
                        adapter_id,
                        exit_code: None,
                    },
                );
                return;
            };
            if let Some(handle) = adapters.get_mut(&adapter_id) {
                match handle.child.try_wait() {
                    Ok(Some(status)) => status.code(),
                    _ => None,
                }
            } else {
                None
            }
        };

        let _ = app_stdout.emit(
            "debug-exit",
            DebugExitEvent {
                adapter_id,
                exit_code,
            },
        );
    });

    // Stderr reader thread: emits `debug-error` events
    let app_stderr = app.clone();
    std::thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stderr);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let _ = app_stderr.emit(
                        "debug-error",
                        DebugErrorEvent {
                            adapter_id,
                            data: text,
                        },
                    );
                }
            }
        }
    });

    Ok(id)
}

/// Send raw data (DAP wire-format bytes) to the debug adapter's stdin.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn debug_send(
    state: State<'_, Arc<DebugAdapterStore>>,
    adapter_id: u32,
    data: String,
) -> Result<(), String> {
    let mut adapters = state.adapters.lock().map_err(|e| e.to_string())?;
    let handle = adapters
        .get_mut(&adapter_id)
        .ok_or_else(|| format!("Debug adapter {adapter_id} not found"))?;

    let stdin = handle
        .stdin
        .as_mut()
        .ok_or_else(|| format!("Debug adapter {adapter_id} stdin not available"))?;

    stdin
        .write_all(data.as_bytes())
        .map_err(|e| format!("Failed to write to debug adapter {adapter_id}: {e}"))?;

    stdin
        .flush()
        .map_err(|e| format!("Failed to flush debug adapter {adapter_id}: {e}"))?;

    Ok(())
}

/// Kill a running debug adapter process.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn debug_kill(state: State<'_, Arc<DebugAdapterStore>>, adapter_id: u32) -> Result<(), String> {
    let mut adapters = state.adapters.lock().map_err(|e| e.to_string())?;
    let mut handle = adapters
        .remove(&adapter_id)
        .ok_or_else(|| format!("Debug adapter {adapter_id} not found"))?;

    handle.stdin.take();

    handle
        .child
        .kill()
        .map_err(|e| format!("Failed to kill debug adapter {adapter_id}: {e}"))?;

    let _ = handle.child.wait();

    Ok(())
}

/// List currently running debug adapter IDs.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn debug_list_adapters(state: State<'_, Arc<DebugAdapterStore>>) -> Result<Vec<u32>, String> {
    let adapters = state.adapters.lock().map_err(|e| e.to_string())?;
    Ok(adapters.keys().copied().collect())
}

// ---------------------------------------------------------------------------
// sidex-dap integration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DapLaunchConfigResponse {
    pub configs: Vec<sidex_dap::LaunchConfig>,
    pub compounds: Vec<sidex_dap::CompoundLaunchConfig>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugAdapterInfo {
    pub type_name: String,
    pub command: String,
    pub args: Vec<String>,
    pub runtime: Option<String>,
    pub command_line: String,
}

/// Parse `.vscode/launch.json` (JSONC) from the given workspace root.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn dap_get_launch_configs(workspace: String) -> Result<DapLaunchConfigResponse, String> {
    let launch_path = PathBuf::from(&workspace).join(".vscode/launch.json");
    if !launch_path.is_file() {
        return Ok(DapLaunchConfigResponse {
            configs: Vec::new(),
            compounds: Vec::new(),
        });
    }
    let (configs, compounds) =
        sidex_dap::parse_launch_json(&launch_path).map_err(|e| e.to_string())?;
    Ok(DapLaunchConfigResponse { configs, compounds })
}

/// Return the built-in debug adapter registry.
#[tauri::command]
pub fn dap_get_adapter_registry() -> Result<Vec<DebugAdapterInfo>, String> {
    let registry = sidex_dap::DebugAdapterRegistry::with_builtins();
    let infos = registry
        .registered_types()
        .into_iter()
        .filter_map(|t| {
            let desc = registry.get(t)?;
            Some(DebugAdapterInfo {
                type_name: desc.type_name.clone(),
                command: desc.command.clone(),
                args: desc.args.clone(),
                runtime: desc.runtime.clone(),
                command_line: registry.command_line(t).unwrap_or_default(),
            })
        })
        .collect();
    Ok(infos)
}
