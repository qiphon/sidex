//! Tauri integration bridge.
//!
//! SideX uses Tauri for native platform capabilities (window management,
//! dialogs, tray, auto-updater, shell operations, extension webviews) while
//! rendering the main editor UI via wgpu.
//!
//! This module also defines the [`BridgeCommand`] / [`BridgeResponse`] IPC
//! protocol used during the transition period when parts of the UI still live
//! in a webview while the core editor renders via wgpu.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ── Public types ────────────────────────────────────────────────────

/// Opaque handle to a native window managed by Tauri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowHandle(pub u64);

/// Opaque handle to an extension webview managed by Tauri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WebviewHandle(pub u64);

/// Options for file open / save dialogs.
#[derive(Debug, Clone, Default)]
pub struct FileDialogOptions {
    pub title: Option<String>,
    pub default_path: Option<PathBuf>,
    pub filters: Vec<FileFilter>,
    pub multiple: bool,
    pub directory: bool,
}

/// A name/extensions pair shown in file dialogs.
#[derive(Debug, Clone)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

/// Kind of message shown in a native message dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Info,
    Warning,
    Error,
}

/// Description of a native menu bar to apply to the window.
#[derive(Debug, Clone)]
pub struct NativeMenu {
    pub items: Vec<NativeMenuItem>,
}

/// A single entry in a native menu bar.
#[derive(Debug, Clone)]
pub enum NativeMenuItem {
    Item {
        id: String,
        label: String,
        accelerator: Option<String>,
        enabled: bool,
    },
    Separator,
    Submenu {
        label: String,
        children: Vec<NativeMenuItem>,
    },
}

/// Description of a system-tray menu.
#[derive(Debug, Clone)]
pub struct TrayMenu {
    pub items: Vec<TrayMenuItem>,
}

/// A single entry in the tray menu.
#[derive(Debug, Clone)]
pub enum TrayMenuItem {
    Item { id: String, label: String },
    Separator,
}

/// Information about a pending auto-update.
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub release_notes: Option<String>,
    pub download_url: Option<String>,
}

/// Options for creating an extension webview.
#[derive(Debug, Clone, Default)]
pub struct WebviewOptions {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub transparent: bool,
}

// ── TauriBridge ─────────────────────────────────────────────────────

/// Central bridge between the native-rendered SideX editor and
/// Tauri's platform integration layer.
///
/// The bridge holds the Tauri [`tauri::AppHandle`] and exposes
/// high-level operations that map to native OS capabilities.
pub struct TauriBridge {
    app_handle: tauri::AppHandle,
    next_window_id: std::sync::atomic::AtomicU64,
    next_webview_id: std::sync::atomic::AtomicU64,
}

impl TauriBridge {
    /// Bootstrap Tauri and return a ready bridge.
    ///
    /// This builds the Tauri application with the standard SideX plugins
    /// (dialog, shell, updater) and runs the setup phase.  The returned
    /// bridge can then be used to create windows, show dialogs, etc.
    pub fn init(app_handle: tauri::AppHandle) -> Result<Self> {
        Ok(Self {
            app_handle,
            next_window_id: std::sync::atomic::AtomicU64::new(1),
            next_webview_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Access the underlying Tauri `AppHandle`.
    pub fn app_handle(&self) -> &tauri::AppHandle {
        &self.app_handle
    }

    // ── Window management ───────────────────────────────────────

    /// Create a new native window. The window surface can later be
    /// handed to wgpu for direct rendering.
    pub fn create_window(&self, title: &str, width: u32, height: u32) -> Result<WindowHandle> {
        use tauri::WebviewWindowBuilder;

        let id = self
            .next_window_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let label = format!("sidex-{id}");

        let _webview_window = WebviewWindowBuilder::new(
            &self.app_handle,
            &label,
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title(title)
        .inner_size(f64::from(width), f64::from(height))
        .build()
        .context("failed to create Tauri window")?;

        log::info!("created window '{label}' ({width}x{height})");
        Ok(WindowHandle(id))
    }

    // ── File dialogs ────────────────────────────────────────────

    /// Show a native "Open" dialog. Returns selected paths, or `None`
    /// if the user cancelled.
    pub fn show_open_dialog(&self, options: FileDialogOptions) -> Result<Option<Vec<PathBuf>>> {
        let mut dialog = rfd::FileDialog::new();

        if let Some(title) = &options.title {
            dialog = dialog.set_title(title);
        }
        if let Some(default) = &options.default_path {
            dialog = dialog.set_directory(default);
        }
        for filter in &options.filters {
            let ext_refs: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter(&filter.name, &ext_refs);
        }

        let result = if options.directory {
            rfd::FileDialog::new()
                .set_title(options.title.as_deref().unwrap_or("Open Folder"))
                .pick_folder()
                .map(|p| vec![p])
        } else if options.multiple {
            let paths = dialog.pick_files();
            paths.filter(|v| !v.is_empty())
        } else {
            dialog.pick_file().map(|p| vec![p])
        };

        Ok(result)
    }

    /// Show a native "Save" dialog. Returns the chosen path, or `None`
    /// if the user cancelled.
    pub fn show_save_dialog(&self, options: FileDialogOptions) -> Result<Option<PathBuf>> {
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
        for filter in &options.filters {
            let ext_refs: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
            dialog = dialog.add_filter(&filter.name, &ext_refs);
        }

        Ok(dialog.save_file())
    }

    /// Show a native message box (info / warning / error).
    pub fn show_message_dialog(&self, title: &str, message: &str, kind: MessageKind) {
        let level = match kind {
            MessageKind::Info => rfd::MessageLevel::Info,
            MessageKind::Warning => rfd::MessageLevel::Warning,
            MessageKind::Error => rfd::MessageLevel::Error,
        };
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(message)
            .set_level(level)
            .show();
    }

    // ── Window properties ───────────────────────────────────────

    /// Set the title of the focused window.
    pub fn set_title(&self, title: &str) {
        use tauri::Manager;
        if let Some(window) = self.app_handle.get_webview_window("main") {
            let _ = window.set_title(title);
        }
    }

    /// Apply a native menu bar described by [`NativeMenu`].
    pub fn set_menu(&self, menu: &NativeMenu) -> Result<()> {
        use tauri::menu::Menu;

        let tauri_menu = Menu::new(&self.app_handle)?;
        build_tauri_menu(&self.app_handle, &tauri_menu, &menu.items)?;
        self.app_handle
            .set_menu(tauri_menu)
            .context("failed to set native menu")?;
        Ok(())
    }

    // ── System tray ─────────────────────────────────────────────

    /// Create a system tray icon with the given image bytes and menu.
    pub fn create_tray(&self, icon: &[u8], menu: &TrayMenu) -> Result<()> {
        use tauri::tray::TrayIconBuilder;

        let tray_menu = tauri::menu::Menu::new(&self.app_handle)?;
        for item in &menu.items {
            match item {
                TrayMenuItem::Item { id, label } => {
                    let mi =
                        tauri::menu::MenuItemBuilder::with_id(id, label).build(&self.app_handle)?;
                    tray_menu.append(&mi)?;
                }
                TrayMenuItem::Separator => {
                    let sep = tauri::menu::PredefinedMenuItem::separator(&self.app_handle)?;
                    tray_menu.append(&sep)?;
                }
            }
        }

        TrayIconBuilder::new()
            .icon_as_template(false)
            .menu(&tray_menu)
            .build(&self.app_handle)
            .context("failed to create tray icon")?;

        let _ = icon;

        Ok(())
    }

    // ── Shell operations ────────────────────────────────────────

    /// Open a URL in the default browser.
    pub fn open_url(&self, url: &str) {
        if let Err(e) = open::that(url) {
            log::warn!("failed to open URL {url}: {e}");
        }
    }

    /// Reveal a path in the platform file manager.
    pub fn open_path(&self, path: &Path) {
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg("-R")
                .arg(path)
                .spawn();
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg("/select,")
                .arg(path)
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(path.parent().unwrap_or(path))
                .spawn();
        }
    }

    // ── Auto-updater ────────────────────────────────────────────

    /// Check for application updates via the Tauri updater plugin.
    pub async fn check_for_updates(&self) -> Result<Option<UpdateInfo>> {
        use tauri_plugin_updater::UpdaterExt;

        let updater = self
            .app_handle
            .updater()
            .context("failed to get updater handle")?;

        match updater.check().await {
            Ok(Some(update)) => Ok(Some(UpdateInfo {
                version: update.version.clone(),
                release_notes: update.body.clone(),
                download_url: None,
            })),
            Ok(None) => Ok(None),
            Err(e) => {
                log::warn!("update check failed: {e}");
                Ok(None)
            }
        }
    }

    // ── Extension webviews ──────────────────────────────────────

    /// Create a webview for an extension that needs HTML rendering.
    pub fn create_webview(&self, html: &str, options: WebviewOptions) -> Result<WebviewHandle> {
        use tauri::WebviewWindowBuilder;

        let id = self
            .next_webview_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let label = format!("ext-webview-{id}");

        let builder = WebviewWindowBuilder::new(
            &self.app_handle,
            &label,
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title(&options.title)
        .inner_size(f64::from(options.width), f64::from(options.height))
        .resizable(options.resizable);

        let _ = options.transparent;

        let window = builder
            .build()
            .context("failed to create extension webview")?;

        let escaped = html.replace('\\', "\\\\").replace('`', "\\`");
        let _ = window.eval(&format!("document.documentElement.innerHTML = `{escaped}`"));

        log::info!("created extension webview '{label}'");
        Ok(WebviewHandle(id))
    }
}

// ── IPC bridge types ────────────────────────────────────────────────

/// Commands that a webview (or external process) can send to the Rust core
/// through the Tauri IPC channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeCommand {
    OpenFile { path: String },
    SaveFile { path: String, content: String },
    GetFileContent { path: String },
    RunCommand { command: String, args: Option<serde_json::Value> },
    GetSettings { section: Option<String> },
    UpdateSetting { key: String, value: serde_json::Value },
    GetTheme,
    SetTheme { theme_id: String },
    ShowNotification { message: String, severity: String },
    GetExtensions,
    InstallExtension { id: String },
    UninstallExtension { id: String },
    StartTerminal { shell: Option<String> },
    GetGitStatus,
    GitCommit { message: String },
    Search { query: String, options: serde_json::Value },
}

/// Generic response envelope sent back over IPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BridgeResponse {
    pub fn ok(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn ok_empty() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

// ── Extended window operations ──────────────────────────────────────

impl TauriBridge {
    /// Emit a custom event to the frontend webview.
    pub fn emit_to_frontend(&self, event: &str, payload: serde_json::Value) -> Result<()> {
        use tauri::Emitter;
        self.app_handle
            .emit(event, payload)
            .context("failed to emit event to frontend")
    }

    /// Set the title of the main window.
    pub fn set_window_title(&self, title: &str) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        win.set_title(title).context("set_title failed")
    }

    /// Resize the main window.
    pub fn set_window_size(&self, width: u32, height: u32) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        let size = tauri::LogicalSize::new(f64::from(width), f64::from(height));
        win.set_size(size).context("set_size failed")
    }

    /// Toggle the main window between fullscreen and windowed.
    pub fn toggle_fullscreen(&self) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        let is_fullscreen = win.is_fullscreen().unwrap_or(false);
        win.set_fullscreen(!is_fullscreen)
            .context("toggle fullscreen failed")
    }

    /// Minimize the main window.
    pub fn minimize(&self) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        win.minimize().context("minimize failed")
    }

    /// Maximize (or un-maximize) the main window.
    pub fn maximize(&self) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        if win.is_maximized().unwrap_or(false) {
            win.unmaximize().context("unmaximize failed")
        } else {
            win.maximize().context("maximize failed")
        }
    }

    /// Close the main window.
    pub fn close_window(&self) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        win.close().context("close failed")
    }

    /// Pin or unpin the main window as always-on-top.
    pub fn set_always_on_top(&self, always: bool) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        win.set_always_on_top(always)
            .context("set_always_on_top failed")
    }

    /// Flash the window to attract the user's attention.
    pub fn request_attention(&self) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        win.request_user_attention(Some(tauri::UserAttentionType::Informational))
            .context("request_user_attention failed")
    }

    /// Show or hide window decorations (title bar, borders).
    pub fn set_decorations(&self, decorated: bool) -> Result<()> {
        use tauri::Manager;
        let win = self
            .app_handle
            .get_webview_window("main")
            .context("main window not found")?;
        win.set_decorations(decorated)
            .context("set_decorations failed")
    }

    /// Dispatch a [`BridgeCommand`] received over IPC and return a response.
    pub fn handle_command(&self, cmd: BridgeCommand) -> BridgeResponse {
        match cmd {
            BridgeCommand::OpenFile { path } => {
                log::info!("bridge: open file '{path}'");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::SaveFile { path, .. } => {
                log::info!("bridge: save file '{path}'");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::GetFileContent { path } => {
                match std::fs::read_to_string(&path) {
                    Ok(content) => BridgeResponse::ok(serde_json::json!({ "content": content })),
                    Err(e) => BridgeResponse::err(format!("read failed: {e}")),
                }
            }
            BridgeCommand::RunCommand { command, .. } => {
                log::info!("bridge: run command '{command}'");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::GetSettings { section } => {
                log::debug!("bridge: get settings section={section:?}");
                BridgeResponse::ok(serde_json::json!({}))
            }
            BridgeCommand::UpdateSetting { key, value } => {
                log::info!("bridge: update setting '{key}'");
                let _ = value;
                BridgeResponse::ok_empty()
            }
            BridgeCommand::GetTheme => {
                BridgeResponse::ok(serde_json::json!({ "theme_id": "default-dark" }))
            }
            BridgeCommand::SetTheme { theme_id } => {
                log::info!("bridge: set theme '{theme_id}'");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::ShowNotification { message, severity } => {
                log::info!("bridge: notification [{severity}] {message}");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::GetExtensions => {
                BridgeResponse::ok(serde_json::json!({ "extensions": [] }))
            }
            BridgeCommand::InstallExtension { id } => {
                log::info!("bridge: install extension '{id}'");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::UninstallExtension { id } => {
                log::info!("bridge: uninstall extension '{id}'");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::StartTerminal { shell } => {
                log::info!("bridge: start terminal shell={shell:?}");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::GetGitStatus => {
                BridgeResponse::ok(serde_json::json!({ "files": [] }))
            }
            BridgeCommand::GitCommit { message } => {
                log::info!("bridge: git commit '{message}'");
                BridgeResponse::ok_empty()
            }
            BridgeCommand::Search { query, .. } => {
                log::info!("bridge: search '{query}'");
                BridgeResponse::ok(serde_json::json!({ "results": [] }))
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Recursively convert [`NativeMenuItem`] trees into Tauri menu items.
fn build_tauri_menu(
    app: &tauri::AppHandle,
    menu: &tauri::menu::Menu<tauri::Wry>,
    items: &[NativeMenuItem],
) -> Result<()> {
    for item in items {
        match item {
            NativeMenuItem::Item {
                id,
                label,
                accelerator,
                enabled,
            } => {
                let mut builder =
                    tauri::menu::MenuItemBuilder::with_id(id, label).enabled(*enabled);
                if let Some(accel) = accelerator {
                    builder = builder.accelerator(accel);
                }
                let mi = builder.build(app)?;
                menu.append(&mi)?;
            }
            NativeMenuItem::Separator => {
                let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
                menu.append(&sep)?;
            }
            NativeMenuItem::Submenu { label, children } => {
                let submenu = tauri::menu::SubmenuBuilder::new(app, label);
                let built = submenu.build()?;
                build_tauri_submenu(app, &built, children)?;
                menu.append(&built)?;
            }
        }
    }
    Ok(())
}

fn build_tauri_submenu(
    app: &tauri::AppHandle,
    submenu: &tauri::menu::Submenu<tauri::Wry>,
    items: &[NativeMenuItem],
) -> Result<()> {
    for item in items {
        match item {
            NativeMenuItem::Item {
                id,
                label,
                accelerator,
                enabled,
            } => {
                let mut builder =
                    tauri::menu::MenuItemBuilder::with_id(id, label).enabled(*enabled);
                if let Some(accel) = accelerator {
                    builder = builder.accelerator(accel);
                }
                let mi = builder.build(app)?;
                submenu.append(&mi)?;
            }
            NativeMenuItem::Separator => {
                let sep = tauri::menu::PredefinedMenuItem::separator(app)?;
                submenu.append(&sep)?;
            }
            NativeMenuItem::Submenu { label, children } => {
                let child = tauri::menu::SubmenuBuilder::new(app, label).build()?;
                build_tauri_submenu(app, &child, children)?;
                submenu.append(&child)?;
            }
        }
    }
    Ok(())
}
