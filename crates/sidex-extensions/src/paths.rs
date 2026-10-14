//! Standard filesystem paths for `SideX` extension storage.
//!
//! On Linux, follows the XDG Base Directory Specification:
//!   - Config: `$XDG_CONFIG_HOME/SideX` (default `~/.config/SideX`)
//!   - Data: `$XDG_DATA_HOME/SideX` (default `~/.local/share/SideX`)
//!
//! On macOS: `~/Library/Application Support/SideX`
//! On Windows: `%APPDATA%/SideX`

use std::path::PathBuf;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const APP_NAME: &str = "SideX";

/// Root `SideX` config directory following platform conventions.
///
/// - Linux: `$XDG_CONFIG_HOME/SideX` (default `~/.config/SideX`)
/// - macOS: `~/Library/Application Support/SideX`
/// - Windows: `%APPDATA%/SideX`
pub fn sidex_config_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            });
        base.join(APP_NAME)
    }

    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library")
            .join("Application Support")
            .join(APP_NAME)
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
    }
}

/// Root `SideX` data directory (backward-compatible alias).
///
/// Previously this was hard-coded to `~/.sidex`. Now it returns the
/// platform-appropriate config directory.
pub fn sidex_data_dir() -> PathBuf {
    sidex_config_dir()
}

/// User-installed extensions directory.
///
/// - Linux: `$XDG_DATA_HOME/SideX/extensions` (default `~/.local/share/SideX/extensions`)
/// - macOS: `~/Library/Application Support/SideX/extensions`
/// - Windows: `%APPDATA%/SideX/extensions`
pub fn user_extensions_dir() -> PathBuf {
    let dir = extensions_base_dir().join("extensions");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Global extension storage directory.
pub fn global_storage_dir() -> PathBuf {
    let dir = sidex_config_dir()
        .join("data")
        .join("User")
        .join("globalStorage");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// User data directory.
pub fn user_data_dir() -> PathBuf {
    sidex_config_dir().join("data")
}

/// Base directory for extensions on Linux uses XDG_DATA_HOME to keep large
/// extension binaries separate from config. On other platforms, uses the
/// same directory as config.
fn extensions_base_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local")
                    .join("share")
            });
        base.join(APP_NAME)
    }

    #[cfg(not(target_os = "linux"))]
    {
        sidex_config_dir()
    }
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

    // GUI-launched processes often miss version-manager shims from PATH.
    for candidate in discoverable_node_candidates() {
        if let Some(path) = candidate.to_str() {
            if candidate.exists() && is_usable_node(path) {
                return Ok(NodeRuntime {
                    path: path.to_string(),
                    version: read_node_version(path),
                    source: "discovered",
                    bundled: false,
                });
            }
        }
    }

    Err(
        "Node runtime not found. Searched PATH and common install locations \
         (nvm, fnm, volta, asdf, Homebrew). Set SIDEX_NODE_BINARY to a Node.js \
         (>=18) executable, or install Node.js."
            .into(),
    )
}

/// Well-known Node.js install locations that are frequently missing from the
/// PATH of GUI-launched processes. Covers the popular version managers and
/// user-local prefixes on every supported platform. For versioned directories
/// (nvm, fnm, mise, nvm-windows) the newest version is preferred.
pub fn discoverable_node_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    let home = dirs::home_dir();
    let exe = if cfg!(target_os = "windows") {
        "node.exe"
    } else {
        "node"
    };

    if cfg!(target_os = "windows") {
        // nvm-windows keeps versions in %APPDATA%\nvm\vX.Y.Z\node.exe.
        if let Some(appdata) = dirs::data_dir() {
            if let Some(best) = newest_child_dir(&appdata.join("nvm")) {
                out.push(best.join(exe));
            }
            if let Some(best) = newest_child_dir(&appdata.join("fnm").join("node-versions")) {
                out.push(best.join("installation").join(exe));
            }
        }
        if let Some(local) = dirs::data_local_dir() {
            out.push(local.join("Volta").join("bin").join(exe));
        }
        if let Some(prog) = std::env::var_os("ProgramFiles") {
            out.push(PathBuf::from(prog).join("nodejs").join(exe));
        }
        if let Some(prog) = std::env::var_os("ProgramFiles(x86)") {
            out.push(PathBuf::from(prog).join("nodejs").join(exe));
        }
        if let Some(home) = home {
            out.push(home.join("scoop").join("apps").join("nodejs").join("current").join(exe));
        }
        return out;
    }

    let Some(home) = home else {
        return out;
    };

    // nvm: $NVM_DIR/versions/node/<version>/bin/node
    let nvm_dir = std::env::var("NVM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".nvm"));
    if let Some(best) = newest_child_dir(&nvm_dir.join("versions").join("node")) {
        out.push(best.join("bin").join(exe));
    }

    // fnm: $FNM_DIR (default ~/.local/share/fnm) and legacy ~/.fnm
    let fnm_dirs = [
        std::env::var("FNM_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".local").join("share").join("fnm")),
        home.join(".fnm"),
    ];
    for fnm_dir in fnm_dirs {
        if let Some(best) = newest_child_dir(&fnm_dir.join("node-versions")) {
            out.push(best.join("installation").join("bin").join(exe));
        }
    }

    // mise: ~/.local/share/mise/installs/node/<version>/bin/node
    if let Some(best) = newest_child_dir(
        &home
            .join(".local")
            .join("share")
            .join("mise")
            .join("installs")
            .join("node"),
    ) {
        out.push(best.join("bin").join(exe));
    }

    // Volta and asdf shims.
    out.push(home.join(".volta").join("bin").join(exe));
    out.push(home.join(".asdf").join("shims").join(exe));

    // User-local prefixes and Linux package managers.
    out.push(home.join(".local").join("bin").join(exe));
    out.push(home.join("bin").join(exe));
    out.push(PathBuf::from("/snap/bin/node"));
    out.push(PathBuf::from("/usr/local/bin/node"));
    out.push(PathBuf::from("/usr/bin/node"));
    out.push(PathBuf::from("/opt/homebrew/bin/node"));
    out.push(PathBuf::from("/opt/homebrew/opt/node/bin/node"));

    out
}

/// Picks the child directory with the highest version-like name
/// (`v20.11.0`, `20.11.0`, ...). Returns `None` when the directory does not
/// exist or contains no version-like subdirectories.
fn newest_child_dir(dir: &std::path::Path) -> Option<PathBuf> {
    let mut best: Option<(Vec<(u64, String)>, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let key = version_sort_key(name);
        if best.as_ref().map_or(true, |(b, _)| key > *b) {
            best = Some((key, path));
        }
    }
    best.map(|(_, path)| path)
}

/// Comparison key for version-like directory names. Numeric segments compare
/// numerically, anything else compares lexically after all numeric segments.
fn version_sort_key(name: &str) -> Vec<(u64, String)> {
    let trimmed = name.trim_start_matches(['v', 'V']);
    trimmed
        .split(['.', '-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            match digits.parse::<u64>() {
                Ok(n) if !digits.is_empty() => (n, String::new()),
                _ => (u64::MAX, part.to_string()),
            }
        })
        .collect()
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
        cmd.creation_flags(0x0800_0000);
    }

    cmd.status().is_ok_and(|s| s.success())
}
