//! Shell detection and enumeration.
//!
//! Platform-specific detection of the default shell, listing of available
//! shells on the system, and shell integration (zdotdir) setup.
//! Ported from `src-tauri/src/commands/terminal.rs` and `process.rs`,
//! combining the best of both implementations.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Information about an available shell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellInfo {
    /// Human-readable name (e.g. "Zsh", "Bash").
    pub name: String,
    /// Absolute path to the shell binary.
    pub path: String,
    /// Whether this is the user's default shell.
    pub is_default: bool,
    /// Default arguments to pass when launching.
    pub args: Vec<String>,
}

// ---------------------------------------------------------------------------
// Default shell detection
// ---------------------------------------------------------------------------

/// Detects the default shell for the current platform.
pub fn detect_default_shell() -> String {
    detect_default_shell_inner()
}

#[cfg(target_os = "windows")]
fn detect_default_shell_inner() -> String {
    resolve_windows_shell()
}

#[cfg(not(target_os = "windows"))]
fn detect_default_shell_inner() -> String {
    // 1. $SHELL
    if let Ok(shell) = std::env::var("SHELL") {
        if !shell.is_empty() && shell != "/bin/false" {
            return shell;
        }
    }

    // 2. passwd via libc
    #[cfg(unix)]
    {
        #[allow(unsafe_code)]
        unsafe {
            let uid = libc::getuid();
            let pw = libc::getpwuid(uid);
            if !pw.is_null() {
                let shell_cstr = std::ffi::CStr::from_ptr((*pw).pw_shell);
                if let Ok(s) = shell_cstr.to_str() {
                    if !s.is_empty() && s != "/bin/false" {
                        return s.to_string();
                    }
                }
            }
        }
    }

    // 3. Fallback
    for fb in &["/bin/zsh", "/bin/bash", "/bin/sh"] {
        if Path::new(fb).exists() {
            return fb.to_string();
        }
    }
    "/bin/sh".to_string()
}

/// Resolves the best PowerShell binary on Windows.
#[cfg(target_os = "windows")]
fn resolve_windows_shell() -> String {
    for candidate in ["pwsh.exe", "powershell.exe"] {
        if let Ok(path) = which::which(candidate) {
            return path.to_string_lossy().to_string();
        }
    }
    std::env::var("COMSPEC").unwrap_or_else(|_| "powershell.exe".to_string())
}

/// Returns the best shell for the user, optionally preferring a name/path.
pub fn best_shell(preferred: Option<&str>) -> (String, String) {
    let shells = available_shells();

    if let Some(pref) = preferred {
        for shell in &shells {
            if shell.name.eq_ignore_ascii_case(pref)
                || shell.path.to_lowercase().contains(&pref.to_lowercase())
            {
                return (shell.name.clone(), shell.path.clone());
            }
        }
    }

    for shell in &shells {
        if shell.is_default {
            return (shell.name.clone(), shell.path.clone());
        }
    }

    shells.first().map_or_else(
        || {
            if cfg!(target_os = "windows") {
                ("PowerShell".to_string(), "powershell.exe".to_string())
            } else {
                ("sh".to_string(), "/bin/sh".to_string())
            }
        },
        |s| (s.name.clone(), s.path.clone()),
    )
}

// ---------------------------------------------------------------------------
// Available shells
// ---------------------------------------------------------------------------

/// Returns a list of available shells on the current platform.
pub fn available_shells() -> Vec<ShellInfo> {
    available_shells_inner()
}

#[cfg(target_os = "windows")]
fn available_shells_inner() -> Vec<ShellInfo> {
    let candidates: &[(&str, &str)] = &[
        ("PowerShell", "powershell.exe"),
        ("PowerShell Core", "pwsh.exe"),
        ("Command Prompt", "cmd.exe"),
        ("Git Bash", "bash.exe"),
        ("WSL", "wsl.exe"),
    ];
    let default_shell = detect_default_shell();
    let mut seen = HashSet::new();
    let mut shells = Vec::new();
    for (name, path) in candidates {
        if let Ok(resolved) = which::which(path) {
            let resolved_str = resolved.to_string_lossy().to_string();
            if !seen.insert(name.to_string()) {
                continue;
            }
            shells.push(ShellInfo {
                name: name.to_string(),
                path: resolved_str.clone(),
                is_default: resolved_str.eq_ignore_ascii_case(&default_shell),
                args: vec![],
            });
        }
    }
    if shells.is_empty() {
        shells.push(ShellInfo {
            name: "PowerShell".to_string(),
            path: "powershell.exe".to_string(),
            is_default: true,
            args: vec![],
        });
    }
    shells
}

#[cfg(not(target_os = "windows"))]
fn available_shells_inner() -> Vec<ShellInfo> {
    let default_shell = detect_default_shell();
    let mut seen_paths = HashSet::new();
    let mut shells = Vec::new();

    // Read /etc/shells (same approach as VS Code)
    if let Ok(contents) = std::fs::read_to_string("/etc/shells") {
        for line in contents.lines() {
            let trimmed = if let Some(idx) = line.find('#') {
                &line[..idx]
            } else {
                line
            };
            let trimmed = trimmed.trim();
            if trimmed.is_empty() {
                continue;
            }
            let path = Path::new(trimmed);
            if path.exists() && seen_paths.insert(trimmed.to_string()) {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("sh")
                    .to_string();
                let is_default = trimmed == default_shell
                    || path.file_name().and_then(|n| n.to_str())
                        == Path::new(&default_shell)
                            .file_name()
                            .and_then(|n| n.to_str());
                shells.push(ShellInfo {
                    name: name.clone(),
                    path: trimmed.to_string(),
                    is_default,
                    args: vec![],
                });
            }
        }
    }

    // Fallback if /etc/shells wasn't readable or empty
    if shells.is_empty() {
        let candidates: &[(&str, &str)] = &[
            ("zsh", "/bin/zsh"),
            ("bash", "/bin/bash"),
            ("fish", "/usr/bin/fish"),
            ("fish", "/usr/local/bin/fish"),
            ("fish", "/opt/homebrew/bin/fish"),
            ("dash", "/bin/dash"),
            ("sh", "/bin/sh"),
        ];
        let mut seen_names = HashSet::new();
        for &(name, shell_path) in candidates {
            if Path::new(shell_path).exists() && seen_names.insert(name) {
                shells.push(ShellInfo {
                    name: name.to_string(),
                    path: shell_path.to_string(),
                    is_default: shell_path == default_shell,
                    args: vec![],
                });
            }
        }
    }

    if shells.is_empty() {
        shells.push(ShellInfo {
            name: "sh".to_string(),
            path: "/bin/sh".to_string(),
            is_default: true,
            args: vec![],
        });
    }

    shells
}

/// Checks whether a shell binary exists.
pub fn check_shell_exists(path: &str) -> bool {
    if cfg!(target_os = "windows") {
        Path::new(path).exists() || which::which(path).is_ok()
    } else {
        Path::new(path).exists()
    }
}

// ---------------------------------------------------------------------------
// Shell integration (zdotdir setup)
// ---------------------------------------------------------------------------

/// Sets up a zsh ZDOTDIR with shell integration scripts.
///
/// `scripts_dir` is where the shell-integration scripts live (e.g.
/// `shellIntegration-rc.zsh`). `data_dir` is an app-writable directory
/// where we create the `zsh-integration/` folder.
///
/// Returns the path to the zdotdir that should be set as `ZDOTDIR` when
/// spawning zsh.
pub fn setup_zsh_dotdir(scripts_dir: &Path, data_dir: &Path) -> Result<PathBuf, String> {
    let zdotdir = data_dir.join("zsh-integration");
    std::fs::create_dir_all(&zdotdir).map_err(|e| format!("failed to create zdotdir: {e}"))?;

    let scripts = scripts_dir.to_string_lossy();

    let zshrc = format!(
        "# SideX Shell Integration - Auto-generated\n\
         VSCODE_SHELL_INTEGRATION=1\n\
         VSCODE_INJECTION=1\n\
         if [[ -f \"{scripts}/shellIntegration-rc.zsh\" ]]; then\n\
         \x20   USER_ZDOTDIR=\"${{ZDOTDIR:-$HOME}}\"\n\
         \x20   . \"{scripts}/shellIntegration-rc.zsh\"\n\
         fi\n"
    );
    std::fs::write(zdotdir.join(".zshrc"), zshrc)
        .map_err(|e| format!("failed to write .zshrc: {e}"))?;

    let zshenv = format!(
        "# SideX Shell Integration - Auto-generated\n\
         USER_ZDOTDIR=\"${{ZDOTDIR:-$HOME}}\"\n\
         if [[ -f \"{scripts}/shellIntegration-env.zsh\" ]]; then\n\
         \x20   . \"{scripts}/shellIntegration-env.zsh\"\n\
         fi\n"
    );
    std::fs::write(zdotdir.join(".zshenv"), zshenv)
        .map_err(|e| format!("failed to write .zshenv: {e}"))?;

    let zprofile = format!(
        "# SideX Shell Integration - Auto-generated\n\
         if [[ -f \"{scripts}/shellIntegration-profile.zsh\" ]]; then\n\
         \x20   . \"{scripts}/shellIntegration-profile.zsh\"\n\
         fi\n"
    );
    std::fs::write(zdotdir.join(".zprofile"), zprofile)
        .map_err(|e| format!("failed to write .zprofile: {e}"))?;

    let zlogin = format!(
        "# SideX Shell Integration - Auto-generated\n\
         if [[ -f \"{scripts}/shellIntegration-login.zsh\" ]]; then\n\
         \x20   . \"{scripts}/shellIntegration-login.zsh\"\n\
         fi\n"
    );
    std::fs::write(zdotdir.join(".zlogin"), zlogin)
        .map_err(|e| format!("failed to write .zlogin: {e}"))?;

    Ok(zdotdir)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_default_shell_returns_non_empty() {
        let shell = detect_default_shell();
        assert!(!shell.is_empty(), "default shell should not be empty");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn detect_default_shell_macos() {
        let shell = detect_default_shell();
        assert!(
            shell.contains("zsh") || shell.contains("bash") || shell.contains("sh"),
            "expected a unix shell, got: {shell}"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn detect_default_shell_linux() {
        let shell = detect_default_shell();
        assert!(
            shell.contains("bash") || shell.contains("sh") || shell.contains("zsh"),
            "expected a unix shell, got: {shell}"
        );
    }

    #[test]
    fn available_shells_returns_at_least_one() {
        let shells = available_shells();
        assert!(!shells.is_empty(), "should find at least one shell");
        for s in &shells {
            assert!(!s.name.is_empty());
            assert!(!s.path.is_empty());
        }
    }

    #[test]
    fn best_shell_returns_something() {
        let (name, path) = best_shell(None);
        assert!(!name.is_empty());
        assert!(!path.is_empty());
    }

    #[test]
    fn check_shell_exists_works() {
        assert!(check_shell_exists("/bin/sh") || cfg!(target_os = "windows"));
        assert!(!check_shell_exists("/nonexistent/shell/binary"));
    }

    #[test]
    fn setup_zsh_dotdir_creates_files() {
        let tmp_scripts = tempfile::TempDir::new().unwrap();
        let tmp_data = tempfile::TempDir::new().unwrap();
        let result = setup_zsh_dotdir(tmp_scripts.path(), tmp_data.path());
        assert!(result.is_ok());
        let zdot = result.unwrap();
        assert!(zdot.join(".zshrc").exists());
        assert!(zdot.join(".zshenv").exists());
        assert!(zdot.join(".zprofile").exists());
        assert!(zdot.join(".zlogin").exists());
    }
}
