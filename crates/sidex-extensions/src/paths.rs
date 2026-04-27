//! Standard filesystem paths for `SideX` extension storage.
//!
//! Ported from `src-tauri/src/commands/extension_platform.rs`.

use std::path::PathBuf;

/// Root `SideX` data directory (`~/.sidex`).
pub fn sidex_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".sidex")
}

/// User-installed extensions directory (`~/.sidex/extensions`).
pub fn user_extensions_dir() -> PathBuf {
    let dir = sidex_data_dir().join("extensions");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Global extension storage directory.
pub fn global_storage_dir() -> PathBuf {
    let dir = sidex_data_dir()
        .join("data")
        .join("User")
        .join("globalStorage");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// User data directory.
pub fn user_data_dir() -> PathBuf {
    sidex_data_dir().join("data")
}

/// Node.js runtime resolution (system-only, no bundled/Tauri paths).
pub fn resolve_node_runtime() -> Result<NodeRuntime, String> {
    if let Ok(path) = std::env::var("SIDEX_NODE_BINARY") {
        if is_usable_node(&path) {
            return Ok(NodeRuntime {
                path: path.clone(),
                version: read_node_version(&path),
                source: "env",
                bundled: false,
            });
        }
    }

    let candidates = if cfg!(target_os = "windows") {
        vec!["node.exe", "node"]
    } else {
        vec![
            "node",
            "/usr/local/bin/node",
            "/usr/bin/node",
            "/opt/homebrew/bin/node",
        ]
    };

    for candidate in candidates {
        if is_usable_node(candidate) {
            return Ok(NodeRuntime {
                path: candidate.to_string(),
                version: read_node_version(candidate),
                source: "system",
                bundled: false,
            });
        }
    }

    Err("Node runtime not found. Install Node.js (>=18) or set SIDEX_NODE_BINARY.".into())
}

/// Information about a resolved Node.js runtime.
#[derive(Debug, Clone)]
pub struct NodeRuntime {
    pub path: String,
    pub version: Option<String>,
    pub source: &'static str,
    pub bundled: bool,
}

fn read_node_version(binary: &str) -> Option<String> {
    let mut cmd = std::process::Command::new(binary);
    cmd.arg("--version")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn is_usable_node(binary: &str) -> bool {
    let mut cmd = std::process::Command::new(binary);
    cmd.arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.status().is_ok_and(|s| s.success())
}
