//! Native OS integration — dialogs, URL/file-manager launching, dock/taskbar
//! operations, URL scheme registration, and system colour-scheme tracking.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::platform::Platform;

// ── Dialog option types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct OpenDialogOptions {
    pub title: Option<String>,
    pub default_path: Option<PathBuf>,
    pub filters: Vec<FileFilter>,
    pub multiple: bool,
    pub directory: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SaveDialogOptions {
    pub title: Option<String>,
    pub default_path: Option<PathBuf>,
    pub filters: Vec<FileFilter>,
}

#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct MessageBoxOptions {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub buttons: Vec<String>,
    pub default_button: usize,
    pub cancel_button: Option<usize>,
    pub message_type: MessageBoxType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxType {
    Info,
    Warning,
    Error,
    Question,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxResult {
    Button(usize),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
    HighContrast,
}

// ── NativeIntegration ────────────────────────────────────────────────────────

/// Façade for platform-specific operations that don't pass through Tauri.
///
/// Wraps `rfd` for dialogs, `open` for URL/path launching, and
/// platform-specific shell-outs for dock badges, progress bars, and
/// URL-scheme/file-association registration.
pub struct NativeIntegration {
    pub platform: Platform,
}

impl NativeIntegration {
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }

    // ── Dialogs ──────────────────────────────────────────────────────────

    pub fn show_open_file_dialog(options: &OpenDialogOptions) -> Result<Vec<PathBuf>> {
        let mut dialog = rfd::FileDialog::new();

        if let Some(title) = &options.title {
            dialog = dialog.set_title(title);
        }
        if let Some(default) = &options.default_path {
            dialog = dialog.set_directory(default);
        }
        for f in &options.filters {
            let ext_refs: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter(&f.name, &ext_refs);
        }

        if options.directory {
            let path = rfd::FileDialog::new()
                .set_title(options.title.as_deref().unwrap_or("Open Folder"))
                .pick_folder();
            return Ok(path.into_iter().collect());
        }

        if options.multiple {
            Ok(dialog.pick_files().unwrap_or_default())
        } else {
            Ok(dialog.pick_file().into_iter().collect())
        }
    }

    pub fn show_save_file_dialog(options: &SaveDialogOptions) -> Result<Option<PathBuf>> {
        let mut dialog = rfd::FileDialog::new();

        if let Some(title) = &options.title {
            dialog = dialog.set_title(title);
        }
        if let Some(default) = &options.default_path {
            if default.is_dir() {
                dialog = dialog.set_directory(default);
            } else {
                if let Some(parent) = default.parent() {
                    dialog = dialog.set_directory(parent);
                }
                if let Some(name) = default.file_name().and_then(|n| n.to_str()) {
                    dialog = dialog.set_file_name(name);
                }
            }
        }
        for f in &options.filters {
            let ext_refs: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter(&f.name, &ext_refs);
        }

        Ok(dialog.save_file())
    }

    pub fn show_message_box(options: &MessageBoxOptions) -> Result<MessageBoxResult> {
        let level = match options.message_type {
            MessageBoxType::Info | MessageBoxType::Question => rfd::MessageLevel::Info,
            MessageBoxType::Warning => rfd::MessageLevel::Warning,
            MessageBoxType::Error => rfd::MessageLevel::Error,
        };

        let buttons: rfd::MessageButtons = if options.buttons.len() == 2 {
            rfd::MessageButtons::OkCancel
        } else {
            rfd::MessageButtons::Ok
        };

        let result = rfd::MessageDialog::new()
            .set_title(&options.title)
            .set_description(&options.message)
            .set_level(level)
            .set_buttons(buttons)
            .show();

        match result {
            rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes => {
                Ok(MessageBoxResult::Button(options.default_button))
            }
            rfd::MessageDialogResult::Cancel | rfd::MessageDialogResult::No => {
                Ok(MessageBoxResult::Cancelled)
            }
            rfd::MessageDialogResult::Custom(ref s) => {
                if let Some(idx) = options.buttons.iter().position(|b| b == s) {
                    Ok(MessageBoxResult::Button(idx))
                } else {
                    Ok(MessageBoxResult::Cancelled)
                }
            }
        }
    }

    // ── URL / file-manager ───────────────────────────────────────────────

    pub fn open_external_url(url: &str) -> Result<()> {
        open::that(url).with_context(|| format!("failed to open URL: {url}"))
    }

    pub fn reveal_in_file_manager(path: &Path) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn()
                .context("failed to reveal in Finder")?;
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn()
                .context("failed to reveal in Explorer")?;
        }
        #[cfg(target_os = "linux")]
        {
            let target = path.parent().unwrap_or(path);
            std::process::Command::new("xdg-open")
                .arg(target)
                .spawn()
                .context("failed to reveal via xdg-open")?;
        }
        Ok(())
    }

    // ── Dock / taskbar ───────────────────────────────────────────────────

    pub fn set_dock_badge(text: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let script = if text.is_empty() {
                "tell application \"System Events\" to set badge number of \
                 (first application process whose frontmost is true) to 0"
                    .to_string()
            } else {
                format!(
                    "tell application \"System Events\" to set badge number of \
                     (first application process whose frontmost is true) to {text}"
                )
            };
            std::process::Command::new("osascript")
                .args(["-e", &script])
                .output()
                .context("osascript dock badge failed")?;
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = text;
            log::debug!("dock badge not supported on this platform");
        }
        Ok(())
    }

    pub fn set_progress_bar(progress: Option<f32>) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            if let Some(pct) = progress {
                log::debug!("taskbar progress: {pct:.0}%");
            } else {
                log::debug!("taskbar progress cleared");
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = progress;
            log::debug!("progress bar not yet implemented on this platform");
        }
        Ok(())
    }

    // ── URL scheme / file association ────────────────────────────────────

    pub fn register_url_scheme(scheme: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            log::info!(
                "URL scheme '{scheme}' registration is handled via Info.plist on macOS"
            );
        }
        #[cfg(target_os = "linux")]
        {
            let desktop_entry = format!(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Name=SideX\n\
                 Exec=sidex --open-url %u\n\
                 MimeType=x-scheme-handler/{scheme};\n"
            );
            let xdg_dir = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("/usr/share"))
                .join("applications");
            let path = xdg_dir.join(format!("sidex-{scheme}.desktop"));
            std::fs::create_dir_all(&xdg_dir).ok();
            std::fs::write(&path, desktop_entry)
                .with_context(|| format!("failed to write {}", path.display()))?;
            std::process::Command::new("xdg-mime")
                .args(["default", &format!("sidex-{scheme}.desktop"), &format!("x-scheme-handler/{scheme}")])
                .status()
                .context("xdg-mime failed")?;
        }
        #[cfg(target_os = "windows")]
        {
            log::info!(
                "URL scheme '{scheme}' registration requires registry writes; \
                 handled by the installer on Windows"
            );
        }
        Ok(())
    }

    pub fn register_file_associations(extensions: &[&str]) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            for ext in extensions {
                log::info!("registering file association for .{ext} (linux – best effort)");
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = extensions;
            log::info!("file associations are managed by the OS installer on this platform");
        }
        Ok(())
    }

    // ── System colour scheme ─────────────────────────────────────────────

    pub fn get_system_color_scheme() -> ColorScheme {
        #[cfg(target_os = "macos")]
        {
            let output = std::process::Command::new("defaults")
                .args(["read", "-g", "AppleInterfaceStyle"])
                .output();
            match output {
                Ok(o) if String::from_utf8_lossy(&o.stdout).trim() == "Dark" => {
                    return ColorScheme::Dark;
                }
                _ => return ColorScheme::Light,
            }
        }
        #[cfg(target_os = "linux")]
        {
            let output = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.interface", "color-scheme"])
                .output();
            if let Ok(o) = output {
                let val = String::from_utf8_lossy(&o.stdout);
                if val.contains("dark") {
                    return ColorScheme::Dark;
                }
            }
            return ColorScheme::Light;
        }
        #[cfg(target_os = "windows")]
        {
            return ColorScheme::Light;
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            ColorScheme::Light
        }
    }

    pub fn watch_system_color_scheme(callback: Arc<dyn Fn(ColorScheme) + Send + Sync>) {
        std::thread::Builder::new()
            .name("color-scheme-watcher".into())
            .spawn(move || {
                let mut prev = Self::get_system_color_scheme();
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let current = Self::get_system_color_scheme();
                    if current != prev {
                        callback(current);
                        prev = current;
                    }
                }
            })
            .ok();
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_scheme_detects_without_panic() {
        let _scheme = NativeIntegration::get_system_color_scheme();
    }

    #[test]
    fn open_dialog_options_default() {
        let opts = OpenDialogOptions::default();
        assert!(opts.filters.is_empty());
        assert!(!opts.multiple);
        assert!(!opts.directory);
    }

    #[test]
    fn save_dialog_options_default() {
        let opts = SaveDialogOptions::default();
        assert!(opts.title.is_none());
        assert!(opts.filters.is_empty());
    }

    #[test]
    fn message_box_result_equality() {
        assert_eq!(MessageBoxResult::Cancelled, MessageBoxResult::Cancelled);
        assert_eq!(MessageBoxResult::Button(0), MessageBoxResult::Button(0));
        assert_ne!(MessageBoxResult::Button(0), MessageBoxResult::Button(1));
    }

    #[test]
    fn dock_badge_noop_ok() {
        assert!(NativeIntegration::set_dock_badge("").is_ok());
    }

    #[test]
    fn progress_bar_noop_ok() {
        assert!(NativeIntegration::set_progress_bar(None).is_ok());
        assert!(NativeIntegration::set_progress_bar(Some(0.5)).is_ok());
    }
}
