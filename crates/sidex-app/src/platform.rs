//! Platform abstraction — OS detection, standard directories, and shell-out
//! helpers for file management, clipboard, and URL opening.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

// ── Operating system ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatingSystem {
    MacOS { version: String },
    Linux {
        desktop: DesktopEnvironment,
        display_server: DisplayServer,
    },
    Windows { version: String, build: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEnvironment {
    Gnome,
    Kde,
    Xfce,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayServer {
    X11,
    Wayland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Architecture {
    X86_64,
    Aarch64,
    Arm,
    Unknown,
}

// ── Platform ─────────────────────────────────────────────────────────────────

/// Snapshot of the current platform's capabilities and standard paths.
#[derive(Debug, Clone)]
pub struct Platform {
    pub os: OperatingSystem,
    pub arch: Architecture,
    pub is_wayland: bool,
    pub display_scale: f64,
    pub locale: String,
    pub home_dir: PathBuf,
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Platform {
    /// Detect the running platform at startup.
    pub fn detect() -> Self {
        let os = detect_os();
        let arch = detect_arch();
        let is_wayland = matches!(
            &os,
            OperatingSystem::Linux {
                display_server: DisplayServer::Wayland,
                ..
            }
        );

        let locale = std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .unwrap_or_else(|_| "en_US.UTF-8".into());

        let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        let data_dir = dirs::data_dir().unwrap_or_else(|| home_dir.join(".local/share"));
        let config_dir = dirs::config_dir().unwrap_or_else(|| home_dir.join(".config"));
        let cache_dir = dirs::cache_dir().unwrap_or_else(|| home_dir.join(".cache"));

        Self {
            os,
            arch,
            is_wayland,
            display_scale: 1.0,
            locale,
            home_dir,
            data_dir,
            config_dir,
            cache_dir,
        }
    }

    // ── Convenience queries ──────────────────────────────────────────────

    pub fn is_mac(&self) -> bool {
        matches!(self.os, OperatingSystem::MacOS { .. })
    }

    pub fn is_linux(&self) -> bool {
        matches!(self.os, OperatingSystem::Linux { .. })
    }

    pub fn is_windows(&self) -> bool {
        matches!(self.os, OperatingSystem::Windows { .. })
    }

    /// Human-readable label for the primary accelerator key.
    pub fn modifier_key_label(&self) -> &str {
        if self.is_mac() {
            "\u{2318}" // ⌘
        } else {
            "Ctrl"
        }
    }

    // ── Shell-out helpers ────────────────────────────────────────────────

    /// Open a URL in the default browser.
    pub fn open_url(&self, url: &str) -> Result<()> {
        open::that(url).with_context(|| format!("failed to open URL: {url}"))
    }

    /// Open a file or directory with the OS default application.
    pub fn open_path(&self, path: &Path) -> Result<()> {
        open::that(path.as_os_str())
            .with_context(|| format!("failed to open path: {}", path.display()))
    }

    /// Reveal a path in the platform file manager (Finder / Nautilus / Explorer).
    pub fn reveal_in_file_manager(&self, path: &Path) -> Result<()> {
        let target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };

        if self.is_mac() {
            Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn()
                .context("failed to spawn `open -R`")?;
        } else if self.is_windows() {
            Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn()
                .context("failed to spawn explorer")?;
        } else {
            Command::new("xdg-open")
                .arg(target)
                .spawn()
                .context("failed to spawn xdg-open")?;
        }
        Ok(())
    }

    /// Move a file to the platform trash / recycle bin.
    pub fn trash_file(&self, path: &Path) -> Result<()> {
        if self.is_mac() {
            Command::new("osascript")
                .args([
                    "-e",
                    &format!(
                        "tell application \"Finder\" to delete POSIX file \"{}\"",
                        path.display()
                    ),
                ])
                .output()
                .context("osascript trash failed")?;
        } else if self.is_windows() {
            log::warn!("trash not yet implemented on Windows; removing instead");
            std::fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        } else {
            let status = Command::new("gio")
                .args(["trash", &path.display().to_string()])
                .status();
            match status {
                Ok(s) if s.success() => {}
                _ => {
                    Command::new("trash-put")
                        .arg(path)
                        .status()
                        .context("neither `gio trash` nor `trash-put` succeeded")?;
                }
            }
        }
        Ok(())
    }

    /// Query the system for available font family names.
    ///
    /// This is a best-effort scan; results depend on the platform's font
    /// enumeration tools.
    pub fn get_system_fonts(&self) -> Vec<String> {
        if self.is_mac() {
            system_fonts_macos()
        } else if self.is_linux() {
            system_fonts_linux()
        } else {
            Vec::new()
        }
    }

    /// Read text from the OS clipboard.
    pub fn get_clipboard_text(&self) -> Result<String> {
        let mut cb = arboard::Clipboard::new().context("clipboard init")?;
        cb.get_text().context("clipboard read")
    }

    /// Write text to the OS clipboard.
    pub fn set_clipboard_text(&self, text: &str) -> Result<()> {
        let mut cb = arboard::Clipboard::new().context("clipboard init")?;
        cb.set_text(text).context("clipboard write")
    }

    /// Standard SideX configuration directory (`~/.config/sidex` on Linux,
    /// `~/Library/Application Support/sidex` on macOS, etc.).
    pub fn sidex_config_dir(&self) -> PathBuf {
        self.config_dir.join("sidex")
    }

    /// Standard SideX data directory.
    pub fn sidex_data_dir(&self) -> PathBuf {
        self.data_dir.join("sidex")
    }

    /// Standard SideX cache directory.
    pub fn sidex_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("sidex")
    }
}

// ── OS detection ─────────────────────────────────────────────────────────────

fn detect_os() -> OperatingSystem {
    #[cfg(target_os = "macos")]
    {
        let version = macos_version();
        OperatingSystem::MacOS { version }
    }
    #[cfg(target_os = "linux")]
    {
        let desktop = detect_desktop_environment();
        let display_server = detect_display_server();
        OperatingSystem::Linux {
            desktop,
            display_server,
        }
    }
    #[cfg(target_os = "windows")]
    {
        OperatingSystem::Windows {
            version: "10+".into(),
            build: 0,
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        OperatingSystem::Linux {
            desktop: DesktopEnvironment::Unknown,
            display_server: DisplayServer::X11,
        }
    }
}

fn detect_arch() -> Architecture {
    match std::env::consts::ARCH {
        "x86_64" => Architecture::X86_64,
        "aarch64" => Architecture::Aarch64,
        "arm" => Architecture::Arm,
        _ => Architecture::Unknown,
    }
}

#[cfg(target_os = "macos")]
fn macos_version() -> String {
    Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

#[cfg(target_os = "linux")]
fn detect_desktop_environment() -> DesktopEnvironment {
    let de = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let lower = de.to_lowercase();
    if lower.contains("gnome") {
        DesktopEnvironment::Gnome
    } else if lower.contains("kde") || lower.contains("plasma") {
        DesktopEnvironment::Kde
    } else if lower.contains("xfce") {
        DesktopEnvironment::Xfce
    } else {
        DesktopEnvironment::Unknown
    }
}

#[cfg(target_os = "linux")]
fn detect_display_server() -> DisplayServer {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        DisplayServer::Wayland
    } else {
        DisplayServer::X11
    }
}

// ── Font enumeration helpers ─────────────────────────────────────────────────

fn system_fonts_macos() -> Vec<String> {
    Command::new("system_profiler")
        .args(["SPFontsDataType", "-detailLevel", "mini"])
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| {
                    let trimmed = l.trim();
                    if trimmed.ends_with(':') && !trimmed.starts_with('/') {
                        Some(trimmed.trim_end_matches(':').to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn system_fonts_linux() -> Vec<String> {
    Command::new("fc-list")
        .args(["--format", "%{family}\n"])
        .output()
        .ok()
        .map(|o| {
            let mut fonts: Vec<String> = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            fonts.sort();
            fonts.dedup();
            fonts
        })
        .unwrap_or_default()
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_valid_platform() {
        let p = Platform::detect();
        assert!(!p.home_dir.as_os_str().is_empty());
        assert!(!p.locale.is_empty());
    }

    #[test]
    fn arch_detected() {
        let arch = detect_arch();
        assert_ne!(arch, Architecture::Unknown);
    }

    #[test]
    fn os_boolean_helpers() {
        let p = Platform::detect();
        let count = [p.is_mac(), p.is_linux(), p.is_windows()]
            .iter()
            .filter(|&&b| b)
            .count();
        assert_eq!(count, 1, "exactly one OS should match");
    }

    #[test]
    fn modifier_label_nonempty() {
        let p = Platform::detect();
        let label = p.modifier_key_label();
        assert!(!label.is_empty());
    }

    #[test]
    fn sidex_dirs_under_standard() {
        let p = Platform::detect();
        assert!(p.sidex_config_dir().ends_with("sidex"));
        assert!(p.sidex_data_dir().ends_with("sidex"));
        assert!(p.sidex_cache_dir().ends_with("sidex"));
    }
}
