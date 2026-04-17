//! `vscode.window` API compatibility shim.
//!
//! Maps VS Code window API calls (message dialogs, quick picks, input boxes,
//! output channels, text document display, terminal creation, webviews,
//! tree views, status bar items, progress indicators, file dialogs, and
//! URI handlers) to the `SideX` subsystems.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Opaque handles
// ---------------------------------------------------------------------------

/// Opaque handle to an output channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutputChannelId(pub u32);

/// Opaque handle to a terminal instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtTerminalId(pub u32);

/// Opaque handle to a webview panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WebviewPanelId(pub u32);

/// Opaque handle to a tree view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TreeViewId(pub u32);

/// Opaque handle to a status bar item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StatusBarItemId(pub u32);

/// Opaque handle to a progress task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProgressId(pub u32);

// ---------------------------------------------------------------------------
// Enums mirroring VS Code API
// ---------------------------------------------------------------------------

/// Alignment of a status bar item (mirrors `vscode.StatusBarAlignment`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusBarAlignment {
    #[default]
    Left = 1,
    Right = 2,
}

/// Editor column for placing a webview (mirrors `vscode.ViewColumn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewColumn {
    #[default]
    Active = -1,
    Beside = -2,
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Six = 6,
    Seven = 7,
    Eight = 8,
    Nine = 9,
}

/// Location for a progress indicator (mirrors `vscode.ProgressLocation`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ProgressLocation {
    SourceControl = 1,
    Window = 10,
    #[default]
    Notification = 15,
}

// ---------------------------------------------------------------------------
// Options structs
// ---------------------------------------------------------------------------

/// Options for creating a webview panel.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewPanelOptions {
    #[serde(default)]
    pub enable_scripts: bool,
    #[serde(default)]
    pub enable_forms: bool,
    #[serde(default)]
    pub retain_context_when_hidden: bool,
    #[serde(default)]
    pub local_resource_roots: Vec<String>,
    #[serde(default)]
    pub enable_find_widget: bool,
}

/// Options for tree view creation (mirrors `vscode.TreeViewOptions`).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeViewOptions {
    #[serde(default)]
    pub show_collapse_all: bool,
    #[serde(default)]
    pub can_select_many: bool,
    #[serde(default)]
    pub drag_and_drop_controller: bool,
    #[serde(default)]
    pub manage_checkbox_state_manually: bool,
}

/// Options for a progress indicator.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressOptions {
    #[serde(default)]
    pub location: Option<ProgressLocation>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub cancellable: bool,
}

/// Options for the open-file dialog (mirrors `vscode.OpenDialogOptions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenDialogOptions {
    #[serde(default)]
    pub default_uri: Option<String>,
    #[serde(default)]
    pub open_label: Option<String>,
    #[serde(default)]
    pub can_select_files: bool,
    #[serde(default)]
    pub can_select_folders: bool,
    #[serde(default)]
    pub can_select_many: bool,
    #[serde(default)]
    pub filters: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub title: Option<String>,
}

/// Options for the save-file dialog (mirrors `vscode.SaveDialogOptions`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDialogOptions {
    #[serde(default)]
    pub default_uri: Option<String>,
    #[serde(default)]
    pub save_label: Option<String>,
    #[serde(default)]
    pub filters: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub title: Option<String>,
}

// ---------------------------------------------------------------------------
// Internal state for tracked entities
// ---------------------------------------------------------------------------

#[derive(Debug)]
#[allow(dead_code)]
struct WebviewPanel {
    id: WebviewPanelId,
    view_type: String,
    title: String,
    column: ViewColumn,
    options: WebviewPanelOptions,
    html: String,
    visible: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
struct TreeView {
    id: TreeViewId,
    view_id: String,
    options: TreeViewOptions,
}

#[derive(Debug)]
#[allow(dead_code)]
struct StatusBarItem {
    id: StatusBarItemId,
    alignment: StatusBarAlignment,
    priority: i32,
    text: String,
    tooltip: Option<String>,
    command: Option<String>,
    visible: bool,
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// Callback invoked when a URI is opened via a custom URI scheme.
pub type UriHandler = std::sync::Arc<dyn Fn(&str) -> Result<()> + Send + Sync>;

/// Callback invoked when tree data is requested.
pub type TreeDataProvider = std::sync::Arc<dyn Fn(Value) -> Result<Value> + Send + Sync>;

/// Callback invoked for webview view providers.
pub type WebviewViewProvider = std::sync::Arc<dyn Fn(Value) -> Result<Value> + Send + Sync>;

// ---------------------------------------------------------------------------
// WindowApi
// ---------------------------------------------------------------------------

/// Implements the `vscode.window.*` API surface.
pub struct WindowApi {
    next_output_channel: AtomicU32,
    next_terminal: AtomicU32,
    next_webview_panel: AtomicU32,
    next_tree_view: AtomicU32,
    next_status_bar_item: AtomicU32,
    next_progress: AtomicU32,

    webview_panels: RwLock<HashMap<WebviewPanelId, WebviewPanel>>,
    tree_views: RwLock<HashMap<TreeViewId, TreeView>>,
    tree_data_providers: RwLock<HashMap<String, TreeDataProvider>>,
    webview_view_providers: RwLock<HashMap<String, WebviewViewProvider>>,
    status_bar_items: RwLock<HashMap<StatusBarItemId, StatusBarItem>>,
    uri_handler: RwLock<Option<UriHandler>>,
}

impl WindowApi {
    /// Creates a new window API handler.
    pub fn new() -> Self {
        Self {
            next_output_channel: AtomicU32::new(1),
            next_terminal: AtomicU32::new(1),
            next_webview_panel: AtomicU32::new(1),
            next_tree_view: AtomicU32::new(1),
            next_status_bar_item: AtomicU32::new(1),
            next_progress: AtomicU32::new(1),
            webview_panels: RwLock::new(HashMap::new()),
            tree_views: RwLock::new(HashMap::new()),
            tree_data_providers: RwLock::new(HashMap::new()),
            webview_view_providers: RwLock::new(HashMap::new()),
            status_bar_items: RwLock::new(HashMap::new()),
            uri_handler: RwLock::new(None),
        }
    }

    /// Dispatches a window API action.
    #[allow(clippy::too_many_lines)]
    pub fn handle(&self, action: &str, params: &Value) -> Result<Value> {
        match action {
            // -- existing message dialogs --
            "showInformationMessage" => {
                let msg = extract_message(params)?;
                let items = extract_items(params);
                self.show_information_message(&msg, &items)
            }
            "showWarningMessage" => {
                let msg = extract_message(params)?;
                let items = extract_items(params);
                self.show_warning_message(&msg, &items)
            }
            "showErrorMessage" => {
                let msg = extract_message(params)?;
                let items = extract_items(params);
                self.show_error_message(&msg, &items)
            }
            "showQuickPick" => {
                let items = params
                    .get("items")
                    .cloned()
                    .unwrap_or(Value::Array(Vec::new()));
                let options = params.get("options").cloned().unwrap_or(Value::Null);
                self.show_quick_pick(&items, options)
            }
            "showInputBox" => {
                let options = params.get("options").cloned().unwrap_or(Value::Null);
                self.show_input_box(options)
            }
            "createOutputChannel" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Output");
                let id = self.create_output_channel(name);
                Ok(serde_json::to_value(id)?)
            }
            "showTextDocument" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                self.show_text_document(uri, params)
            }
            "createTerminal" => {
                let id = self.create_terminal(params);
                Ok(serde_json::to_value(id)?)
            }

            // -- webview panels --
            "createWebviewPanel" => {
                let view_type = params
                    .get("viewType")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let title = params
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or("Webview");
                let column: ViewColumn = params
                    .get("column")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let options: WebviewPanelOptions = params
                    .get("options")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let id = self.create_webview_panel(view_type, title, column, options);
                Ok(serde_json::to_value(id)?)
            }

            // -- tree views --
            "registerTreeDataProvider" => {
                let view_id = params.get("viewId").and_then(Value::as_str).unwrap_or("");
                self.register_tree_data_provider(view_id, std::sync::Arc::new(|_| Ok(Value::Null)));
                Ok(Value::Bool(true))
            }
            "createTreeView" => {
                let view_id = params.get("viewId").and_then(Value::as_str).unwrap_or("");
                let options: TreeViewOptions = params
                    .get("options")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let id = self.create_tree_view(view_id, options);
                Ok(serde_json::to_value(id)?)
            }

            // -- URI handler --
            "registerUriHandler" => {
                self.register_uri_handler(std::sync::Arc::new(|_| Ok(())));
                Ok(Value::Bool(true))
            }

            // -- file dialogs --
            "showOpenDialog" => {
                let options: OpenDialogOptions = params
                    .get("options")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                self.show_open_dialog(&options)
            }
            "showSaveDialog" => {
                let options: SaveDialogOptions = params
                    .get("options")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                self.show_save_dialog(&options)
            }

            // -- status bar --
            "createStatusBarItem" => {
                let alignment: StatusBarAlignment = params
                    .get("alignment")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let priority = params
                    .get("priority")
                    .and_then(Value::as_i64)
                    .and_then(|v| i32::try_from(v).ok())
                    .unwrap_or(0);
                let id = self.create_status_bar_item(alignment, priority);
                Ok(serde_json::to_value(id)?)
            }
            "setStatusBarMessage" => {
                let text = params.get("text").and_then(Value::as_str).unwrap_or("");
                let timeout = params.get("timeout").and_then(Value::as_u64);
                self.set_status_bar_message(text, timeout)
            }

            // -- progress --
            "withProgress" => {
                let options: ProgressOptions = params
                    .get("options")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let id = self.with_progress(&options);
                Ok(serde_json::to_value(id)?)
            }

            // -- webview view provider --
            "registerWebviewViewProvider" => {
                let view_id = params.get("viewId").and_then(Value::as_str).unwrap_or("");
                self.register_webview_view_provider(
                    view_id,
                    std::sync::Arc::new(|_| Ok(Value::Null)),
                );
                Ok(Value::Bool(true))
            }

            _ => bail!("unknown window action: {action}"),
        }
    }

    // -----------------------------------------------------------------------
    // Message dialogs
    // -----------------------------------------------------------------------

    /// Shows an information-level message and returns the selected item.
    pub fn show_information_message(&self, message: &str, items: &[String]) -> Result<Value> {
        log::info!("[ext] info: {message}");
        Ok(items
            .first()
            .map_or(Value::Null, |s| Value::String(s.clone())))
    }

    /// Shows a warning-level message and returns the selected item.
    pub fn show_warning_message(&self, message: &str, items: &[String]) -> Result<Value> {
        log::warn!("[ext] warning: {message}");
        Ok(items
            .first()
            .map_or(Value::Null, |s| Value::String(s.clone())))
    }

    /// Shows an error-level message and returns the selected item.
    pub fn show_error_message(&self, message: &str, items: &[String]) -> Result<Value> {
        log::error!("[ext] error: {message}");
        Ok(items
            .first()
            .map_or(Value::Null, |s| Value::String(s.clone())))
    }

    // -----------------------------------------------------------------------
    // Quick pick / input box
    // -----------------------------------------------------------------------

    /// Shows a quick-pick list and returns the selected item.
    pub fn show_quick_pick(&self, items: &Value, _options: Value) -> Result<Value> {
        let first = items.as_array().and_then(|a| a.first().cloned());
        Ok(first.unwrap_or(Value::Null))
    }

    /// Shows an input box and returns the user's input.
    pub fn show_input_box(&self, _options: Value) -> Result<Value> {
        Ok(Value::Null)
    }

    // -----------------------------------------------------------------------
    // Output channels
    // -----------------------------------------------------------------------

    /// Creates a named output channel and returns its id.
    pub fn create_output_channel(&self, name: &str) -> OutputChannelId {
        let id = self.next_output_channel.fetch_add(1, Ordering::Relaxed);
        log::debug!("[ext] created output channel '{name}' -> {id}");
        OutputChannelId(id)
    }

    // -----------------------------------------------------------------------
    // Text document / terminal
    // -----------------------------------------------------------------------

    /// Opens a text document in the editor.
    pub fn show_text_document(&self, uri: &str, _options: &Value) -> Result<Value> {
        log::debug!("[ext] showTextDocument: {uri}");
        Ok(Value::Bool(true))
    }

    /// Creates a new integrated terminal instance.
    pub fn create_terminal(&self, _options: &Value) -> ExtTerminalId {
        let id = self.next_terminal.fetch_add(1, Ordering::Relaxed);
        log::debug!("[ext] created terminal -> {id}");
        ExtTerminalId(id)
    }

    // -----------------------------------------------------------------------
    // Webview panels
    // -----------------------------------------------------------------------

    /// Creates a webview panel and returns its handle.
    pub fn create_webview_panel(
        &self,
        view_type: &str,
        title: &str,
        column: ViewColumn,
        options: WebviewPanelOptions,
    ) -> WebviewPanelId {
        let raw = self.next_webview_panel.fetch_add(1, Ordering::Relaxed);
        let id = WebviewPanelId(raw);
        log::debug!("[ext] createWebviewPanel({view_type}, {title}) -> {raw}");
        self.webview_panels
            .write()
            .expect("webview panels lock poisoned")
            .insert(
                id,
                WebviewPanel {
                    id,
                    view_type: view_type.to_owned(),
                    title: title.to_owned(),
                    column,
                    options,
                    html: String::new(),
                    visible: true,
                },
            );
        id
    }

    /// Sets the HTML content of a webview panel.
    pub fn set_webview_html(&self, id: WebviewPanelId, html: &str) {
        if let Some(panel) = self
            .webview_panels
            .write()
            .expect("webview panels lock poisoned")
            .get_mut(&id)
        {
            html.clone_into(&mut panel.html);
        }
    }

    /// Disposes a webview panel.
    pub fn dispose_webview_panel(&self, id: WebviewPanelId) {
        self.webview_panels
            .write()
            .expect("webview panels lock poisoned")
            .remove(&id);
    }

    // -----------------------------------------------------------------------
    // Tree views
    // -----------------------------------------------------------------------

    /// Registers a tree data provider for a view id.
    pub fn register_tree_data_provider(&self, view_id: &str, provider: TreeDataProvider) {
        log::debug!("[ext] registerTreeDataProvider({view_id})");
        self.tree_data_providers
            .write()
            .expect("tree data providers lock poisoned")
            .insert(view_id.to_owned(), provider);
    }

    /// Creates a tree view with options (drag-drop, multi-select, etc.).
    pub fn create_tree_view(&self, view_id: &str, options: TreeViewOptions) -> TreeViewId {
        let raw = self.next_tree_view.fetch_add(1, Ordering::Relaxed);
        let id = TreeViewId(raw);
        log::debug!("[ext] createTreeView({view_id}) -> {raw}");
        self.tree_views
            .write()
            .expect("tree views lock poisoned")
            .insert(
                id,
                TreeView {
                    id,
                    view_id: view_id.to_owned(),
                    options,
                },
            );
        id
    }

    // -----------------------------------------------------------------------
    // URI handler
    // -----------------------------------------------------------------------

    /// Registers a handler for custom URI scheme opens.
    pub fn register_uri_handler(&self, handler: UriHandler) {
        log::debug!("[ext] registerUriHandler");
        *self.uri_handler.write().expect("uri handler lock poisoned") = Some(handler);
    }

    /// Invokes the registered URI handler.
    pub fn handle_uri(&self, uri: &str) -> Result<()> {
        let guard = self.uri_handler.read().expect("uri handler lock poisoned");
        if let Some(ref handler) = *guard {
            handler(uri)?;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // File dialogs
    // -----------------------------------------------------------------------

    /// Shows a file-open dialog and returns selected URIs.
    pub fn show_open_dialog(&self, options: &OpenDialogOptions) -> Result<Value> {
        log::debug!("[ext] showOpenDialog (label={:?})", options.open_label);
        Ok(Value::Null)
    }

    /// Shows a file-save dialog and returns the chosen URI.
    pub fn show_save_dialog(&self, options: &SaveDialogOptions) -> Result<Value> {
        log::debug!("[ext] showSaveDialog (label={:?})", options.save_label);
        Ok(Value::Null)
    }

    // -----------------------------------------------------------------------
    // Status bar
    // -----------------------------------------------------------------------

    /// Creates a status bar item with the given alignment and priority.
    pub fn create_status_bar_item(
        &self,
        alignment: StatusBarAlignment,
        priority: i32,
    ) -> StatusBarItemId {
        let raw = self.next_status_bar_item.fetch_add(1, Ordering::Relaxed);
        let id = StatusBarItemId(raw);
        log::debug!("[ext] createStatusBarItem({alignment:?}, priority={priority}) -> {raw}");
        self.status_bar_items
            .write()
            .expect("status bar items lock poisoned")
            .insert(
                id,
                StatusBarItem {
                    id,
                    alignment,
                    priority,
                    text: String::new(),
                    tooltip: None,
                    command: None,
                    visible: false,
                },
            );
        id
    }

    /// Updates the text of a status bar item.
    pub fn update_status_bar_item(
        &self,
        id: StatusBarItemId,
        text: &str,
        tooltip: Option<&str>,
        command: Option<&str>,
    ) {
        if let Some(item) = self
            .status_bar_items
            .write()
            .expect("status bar items lock poisoned")
            .get_mut(&id)
        {
            text.clone_into(&mut item.text);
            item.tooltip = tooltip.map(String::from);
            item.command = command.map(String::from);
        }
    }

    /// Shows a status bar item.
    pub fn show_status_bar_item(&self, id: StatusBarItemId) {
        if let Some(item) = self
            .status_bar_items
            .write()
            .expect("status bar items lock poisoned")
            .get_mut(&id)
        {
            item.visible = true;
        }
    }

    /// Hides a status bar item.
    pub fn hide_status_bar_item(&self, id: StatusBarItemId) {
        if let Some(item) = self
            .status_bar_items
            .write()
            .expect("status bar items lock poisoned")
            .get_mut(&id)
        {
            item.visible = false;
        }
    }

    /// Disposes a status bar item.
    pub fn dispose_status_bar_item(&self, id: StatusBarItemId) {
        self.status_bar_items
            .write()
            .expect("status bar items lock poisoned")
            .remove(&id);
    }

    /// Temporarily shows a message in the status bar.
    pub fn set_status_bar_message(&self, text: &str, timeout: Option<u64>) -> Result<Value> {
        log::debug!("[ext] setStatusBarMessage({text}, timeout={timeout:?})");
        Ok(Value::Bool(true))
    }

    // -----------------------------------------------------------------------
    // Progress
    // -----------------------------------------------------------------------

    /// Starts a progress indicator and returns its handle.
    pub fn with_progress(&self, options: &ProgressOptions) -> ProgressId {
        let raw = self.next_progress.fetch_add(1, Ordering::Relaxed);
        log::debug!(
            "[ext] withProgress(title={:?}, loc={:?}) -> {raw}",
            options.title,
            options.location,
        );
        ProgressId(raw)
    }

    /// Reports progress for a given progress handle.
    pub fn report_progress(&self, id: ProgressId, increment: Option<f64>, message: Option<&str>) {
        log::debug!(
            "[ext] reportProgress({}, inc={increment:?}, msg={message:?})",
            id.0,
        );
    }

    // -----------------------------------------------------------------------
    // Webview view providers (sidebar webviews)
    // -----------------------------------------------------------------------

    /// Registers a webview view provider for a sidebar view id.
    pub fn register_webview_view_provider(&self, view_id: &str, provider: WebviewViewProvider) {
        log::debug!("[ext] registerWebviewViewProvider({view_id})");
        self.webview_view_providers
            .write()
            .expect("webview view providers lock poisoned")
            .insert(view_id.to_owned(), provider);
    }
}

impl Default for WindowApi {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_message(params: &Value) -> Result<String> {
    params
        .get("message")
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("missing 'message' parameter"))
}

fn extract_items(params: &Value) -> Vec<String> {
    params
        .get("items")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
