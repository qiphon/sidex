//! `vscode.workspace` API compatibility shim.
//!
//! Maps VS Code workspace API calls (configuration, document management,
//! workspace edits, file search, file system providers, virtual documents,
//! multi-root workspace management, file watchers, and document/config
//! change events) to `SideX` subsystems.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Opaque handles
// ---------------------------------------------------------------------------

/// Opaque handle to a file system watcher.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileSystemWatcherId(pub u32);

/// Opaque handle to a registered file system provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileSystemProviderId(pub u32);

/// Opaque handle to a text document content provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentProviderId(pub u32);

/// Opaque handle to an event listener registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventListenerId(pub u32);

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Information about a text document returned to the extension host.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentInfo {
    pub uri: String,
    pub language_id: String,
    pub version: u64,
    pub line_count: u32,
}

/// Represents a workspace folder (mirrors `vscode.WorkspaceFolder`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFolder {
    pub uri: String,
    pub name: String,
    pub index: u32,
}

/// Describes a text document change event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentChangeEvent {
    pub uri: String,
    pub version: u64,
    pub content_changes: Vec<TextDocumentContentChange>,
}

/// A single content change within a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentContentChange {
    pub range_offset: u32,
    pub range_length: u32,
    pub text: String,
}

/// File system change event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileChangeType {
    Changed = 1,
    Created = 2,
    Deleted = 3,
}

/// A single file change event from a watcher.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeEvent {
    pub uri: String,
    #[serde(rename = "type")]
    pub change_type: FileChangeType,
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// Callback for file system provider operations.
pub type FileSystemProvider = Arc<dyn Fn(&str, Value) -> Result<Value> + Send + Sync>;

/// Callback for text document content providers.
pub type TextDocumentContentProvider = Arc<dyn Fn(&str) -> Result<String> + Send + Sync>;

/// Callback for event listeners.
pub type EventListener = Arc<dyn Fn(Value) -> Result<()> + Send + Sync>;

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct FsProviderEntry {
    id: FileSystemProviderId,
    scheme: String,
    handler: FileSystemProvider,
}

#[allow(dead_code)]
struct ContentProviderEntry {
    id: ContentProviderId,
    scheme: String,
    handler: TextDocumentContentProvider,
}

#[allow(dead_code)]
struct WatcherEntry {
    id: FileSystemWatcherId,
    glob_pattern: String,
}

// ---------------------------------------------------------------------------
// WorkspaceApi
// ---------------------------------------------------------------------------

/// Implements the `vscode.workspace.*` API surface.
pub struct WorkspaceApi {
    next_watcher: AtomicU32,
    next_fs_provider: AtomicU32,
    next_content_provider: AtomicU32,
    next_event_listener: AtomicU32,

    workspace_folders: RwLock<Vec<WorkspaceFolder>>,
    fs_providers: RwLock<HashMap<String, FsProviderEntry>>,
    content_providers: RwLock<HashMap<String, ContentProviderEntry>>,
    watchers: RwLock<HashMap<FileSystemWatcherId, WatcherEntry>>,

    on_did_change_text_document: RwLock<Vec<(EventListenerId, EventListener)>>,
    on_did_open_text_document: RwLock<Vec<(EventListenerId, EventListener)>>,
    on_did_close_text_document: RwLock<Vec<(EventListenerId, EventListener)>>,
    on_did_save_text_document: RwLock<Vec<(EventListenerId, EventListener)>>,
    on_did_change_configuration: RwLock<Vec<(EventListenerId, EventListener)>>,
}

impl WorkspaceApi {
    /// Creates a new workspace API handler.
    pub fn new() -> Self {
        Self {
            next_watcher: AtomicU32::new(1),
            next_fs_provider: AtomicU32::new(1),
            next_content_provider: AtomicU32::new(1),
            next_event_listener: AtomicU32::new(1),
            workspace_folders: RwLock::new(Vec::new()),
            fs_providers: RwLock::new(HashMap::new()),
            content_providers: RwLock::new(HashMap::new()),
            watchers: RwLock::new(HashMap::new()),
            on_did_change_text_document: RwLock::new(Vec::new()),
            on_did_open_text_document: RwLock::new(Vec::new()),
            on_did_close_text_document: RwLock::new(Vec::new()),
            on_did_save_text_document: RwLock::new(Vec::new()),
            on_did_change_configuration: RwLock::new(Vec::new()),
        }
    }

    /// Dispatches a workspace API action.
    pub fn handle(&self, action: &str, params: &Value) -> Result<Value> {
        match action {
            "getConfiguration" => {
                let section = params.get("section").and_then(Value::as_str).unwrap_or("");
                self.get_configuration(section)
            }
            "openTextDocument" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                let info = self.open_text_document(uri)?;
                Ok(serde_json::to_value(info)?)
            }
            "applyEdit" => {
                let edit = params.get("edit").cloned().unwrap_or(Value::Null);
                let ok = self.apply_edit(edit)?;
                Ok(Value::Bool(ok))
            }
            "findFiles" => {
                let pattern = params
                    .get("pattern")
                    .and_then(Value::as_str)
                    .unwrap_or("**/*");
                let exclude = params.get("exclude").and_then(Value::as_str);
                let files = self.find_files(pattern, exclude)?;
                Ok(Value::Array(files.into_iter().map(Value::String).collect()))
            }
            "saveAll" => {
                let ok = self.save_all()?;
                Ok(Value::Bool(ok))
            }

            // -- file system provider --
            "registerFileSystemProvider" => {
                let scheme = params.get("scheme").and_then(Value::as_str).unwrap_or("");
                let id = self.register_file_system_provider(
                    scheme,
                    Arc::new(|_op, _params| Ok(Value::Null)),
                );
                Ok(serde_json::to_value(id)?)
            }

            // -- text document content provider --
            "registerTextDocumentContentProvider" => {
                let scheme = params.get("scheme").and_then(Value::as_str).unwrap_or("");
                let id = self.register_text_document_content_provider(
                    scheme,
                    Arc::new(|_uri| Ok(String::new())),
                );
                Ok(serde_json::to_value(id)?)
            }

            // -- workspace folders --
            "getWorkspaceFolders" => {
                let folders = self.get_workspace_folders();
                Ok(serde_json::to_value(folders)?)
            }
            "updateWorkspaceFolders" => {
                #[allow(clippy::cast_possible_truncation)]
                let start = params.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
                #[allow(clippy::cast_possible_truncation)]
                let delete_count = params
                    .get("deleteCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                let folders_to_add: Vec<WorkspaceFolder> = params
                    .get("foldersToAdd")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                let ok = self.update_workspace_folders(start, delete_count, &folders_to_add);
                Ok(Value::Bool(ok))
            }
            "getWorkspaceFolder" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                match self.get_workspace_folder(uri) {
                    Some(f) => Ok(serde_json::to_value(f)?),
                    None => Ok(Value::Null),
                }
            }

            // -- file system watcher --
            "createFileSystemWatcher" => {
                let glob = params.get("glob").and_then(Value::as_str).unwrap_or("**/*");
                let id = self.create_file_system_watcher(glob);
                Ok(serde_json::to_value(id)?)
            }

            // -- document / config event subscriptions --
            "onDidChangeTextDocument" => {
                let id = self.subscribe_on_did_change_text_document(Arc::new(|_| Ok(())));
                Ok(serde_json::to_value(id)?)
            }
            "onDidOpenTextDocument" => {
                let id = self.subscribe_on_did_open_text_document(Arc::new(|_| Ok(())));
                Ok(serde_json::to_value(id)?)
            }
            "onDidCloseTextDocument" => {
                let id = self.subscribe_on_did_close_text_document(Arc::new(|_| Ok(())));
                Ok(serde_json::to_value(id)?)
            }
            "onDidSaveTextDocument" => {
                let id = self.subscribe_on_did_save_text_document(Arc::new(|_| Ok(())));
                Ok(serde_json::to_value(id)?)
            }
            "onDidChangeConfiguration" => {
                let id = self.subscribe_on_did_change_configuration(Arc::new(|_| Ok(())));
                Ok(serde_json::to_value(id)?)
            }

            _ => bail!("unknown workspace action: {action}"),
        }
    }

    // -----------------------------------------------------------------------
    // Existing methods
    // -----------------------------------------------------------------------

    /// Returns configuration values for the given section.
    pub fn get_configuration(&self, section: &str) -> Result<Value> {
        log::debug!("[ext] getConfiguration: {section}");
        Ok(Value::Object(serde_json::Map::new()))
    }

    /// Opens (or retrieves) a text document by URI.
    pub fn open_text_document(&self, uri: &str) -> Result<TextDocumentInfo> {
        log::debug!("[ext] openTextDocument: {uri}");
        Ok(TextDocumentInfo {
            uri: uri.to_owned(),
            language_id: detect_language_id(uri),
            version: 1,
            line_count: 0,
        })
    }

    /// Applies a workspace edit (set of file edits).
    pub fn apply_edit(&self, _edit: Value) -> Result<bool> {
        log::debug!("[ext] applyEdit");
        Ok(true)
    }

    /// Finds files matching a glob pattern.
    pub fn find_files(&self, pattern: &str, _exclude: Option<&str>) -> Result<Vec<String>> {
        log::debug!("[ext] findFiles: {pattern}");
        Ok(Vec::new())
    }

    /// Saves all dirty documents.
    pub fn save_all(&self) -> Result<bool> {
        log::debug!("[ext] saveAll");
        Ok(true)
    }

    // -----------------------------------------------------------------------
    // File system provider
    // -----------------------------------------------------------------------

    /// Registers a custom file system provider for a URI scheme.
    pub fn register_file_system_provider(
        &self,
        scheme: &str,
        handler: FileSystemProvider,
    ) -> FileSystemProviderId {
        let raw = self.next_fs_provider.fetch_add(1, Ordering::Relaxed);
        let id = FileSystemProviderId(raw);
        log::debug!("[ext] registerFileSystemProvider({scheme}) -> {raw}");
        self.fs_providers
            .write()
            .expect("fs providers lock poisoned")
            .insert(
                scheme.to_owned(),
                FsProviderEntry {
                    id,
                    scheme: scheme.to_owned(),
                    handler,
                },
            );
        id
    }

    // -----------------------------------------------------------------------
    // Text document content provider
    // -----------------------------------------------------------------------

    /// Registers a virtual document content provider for a URI scheme.
    pub fn register_text_document_content_provider(
        &self,
        scheme: &str,
        handler: TextDocumentContentProvider,
    ) -> ContentProviderId {
        let raw = self.next_content_provider.fetch_add(1, Ordering::Relaxed);
        let id = ContentProviderId(raw);
        log::debug!("[ext] registerTextDocumentContentProvider({scheme}) -> {raw}");
        self.content_providers
            .write()
            .expect("content providers lock poisoned")
            .insert(
                scheme.to_owned(),
                ContentProviderEntry {
                    id,
                    scheme: scheme.to_owned(),
                    handler,
                },
            );
        id
    }

    /// Provides content for a virtual document URI.
    pub fn provide_text_document_content(&self, uri: &str) -> Result<Option<String>> {
        let scheme = uri.split(':').next().unwrap_or("");
        let providers = self
            .content_providers
            .read()
            .expect("content providers lock poisoned");
        match providers.get(scheme) {
            Some(entry) => Ok(Some((entry.handler)(uri)?)),
            None => Ok(None),
        }
    }

    // -----------------------------------------------------------------------
    // Workspace folders (multi-root)
    // -----------------------------------------------------------------------

    /// Returns all workspace folders.
    pub fn get_workspace_folders(&self) -> Vec<WorkspaceFolder> {
        self.workspace_folders
            .read()
            .expect("workspace folders lock poisoned")
            .clone()
    }

    /// Updates workspace folders (splice semantics like VS Code).
    pub fn update_workspace_folders(
        &self,
        start: usize,
        delete_count: usize,
        folders_to_add: &[WorkspaceFolder],
    ) -> bool {
        log::debug!(
            "[ext] updateWorkspaceFolders(start={start}, delete={delete_count}, add={})",
            folders_to_add.len()
        );
        let mut folders = self
            .workspace_folders
            .write()
            .expect("workspace folders lock poisoned");

        let end = (start + delete_count).min(folders.len());
        folders.drain(start..end);

        for (i, folder) in folders_to_add.iter().enumerate() {
            let pos = (start + i).min(folders.len());
            folders.insert(pos, folder.clone());
        }

        for (i, folder) in folders.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            {
                folder.index = i as u32;
            }
        }
        true
    }

    /// Returns the workspace folder containing the given URI, if any.
    pub fn get_workspace_folder(&self, uri: &str) -> Option<WorkspaceFolder> {
        self.workspace_folders
            .read()
            .expect("workspace folders lock poisoned")
            .iter()
            .find(|f| uri.starts_with(&f.uri))
            .cloned()
    }

    /// Adds a workspace folder (convenience method).
    pub fn add_workspace_folder(&self, uri: &str, name: &str) {
        let mut folders = self
            .workspace_folders
            .write()
            .expect("workspace folders lock poisoned");
        #[allow(clippy::cast_possible_truncation)]
        let index = folders.len() as u32;
        folders.push(WorkspaceFolder {
            uri: uri.to_owned(),
            name: name.to_owned(),
            index,
        });
    }

    // -----------------------------------------------------------------------
    // File system watcher
    // -----------------------------------------------------------------------

    /// Creates a file system watcher for the given glob pattern.
    pub fn create_file_system_watcher(&self, glob_pattern: &str) -> FileSystemWatcherId {
        let raw = self.next_watcher.fetch_add(1, Ordering::Relaxed);
        let id = FileSystemWatcherId(raw);
        log::debug!("[ext] createFileSystemWatcher({glob_pattern}) -> {raw}");
        self.watchers
            .write()
            .expect("watchers lock poisoned")
            .insert(
                id,
                WatcherEntry {
                    id,
                    glob_pattern: glob_pattern.to_owned(),
                },
            );
        id
    }

    /// Disposes a file system watcher.
    pub fn dispose_file_system_watcher(&self, id: FileSystemWatcherId) {
        self.watchers
            .write()
            .expect("watchers lock poisoned")
            .remove(&id);
    }

    // -----------------------------------------------------------------------
    // Event subscriptions
    // -----------------------------------------------------------------------

    fn next_event_id(&self) -> EventListenerId {
        EventListenerId(self.next_event_listener.fetch_add(1, Ordering::Relaxed))
    }

    /// Subscribes to `onDidChangeTextDocument`.
    pub fn subscribe_on_did_change_text_document(
        &self,
        listener: EventListener,
    ) -> EventListenerId {
        let id = self.next_event_id();
        self.on_did_change_text_document
            .write()
            .expect("event lock poisoned")
            .push((id, listener));
        id
    }

    /// Fires `onDidChangeTextDocument` to all listeners.
    pub fn fire_did_change_text_document(&self, event: &TextDocumentChangeEvent) -> Result<()> {
        let val = serde_json::to_value(event)?;
        let listeners = self
            .on_did_change_text_document
            .read()
            .expect("event lock poisoned");
        for (_id, listener) in listeners.iter() {
            listener(val.clone())?;
        }
        Ok(())
    }

    /// Subscribes to `onDidOpenTextDocument`.
    pub fn subscribe_on_did_open_text_document(&self, listener: EventListener) -> EventListenerId {
        let id = self.next_event_id();
        self.on_did_open_text_document
            .write()
            .expect("event lock poisoned")
            .push((id, listener));
        id
    }

    /// Fires `onDidOpenTextDocument` to all listeners.
    pub fn fire_did_open_text_document(&self, doc: &TextDocumentInfo) -> Result<()> {
        let val = serde_json::to_value(doc)?;
        let listeners = self
            .on_did_open_text_document
            .read()
            .expect("event lock poisoned");
        for (_id, listener) in listeners.iter() {
            listener(val.clone())?;
        }
        Ok(())
    }

    /// Subscribes to `onDidCloseTextDocument`.
    pub fn subscribe_on_did_close_text_document(&self, listener: EventListener) -> EventListenerId {
        let id = self.next_event_id();
        self.on_did_close_text_document
            .write()
            .expect("event lock poisoned")
            .push((id, listener));
        id
    }

    /// Fires `onDidCloseTextDocument` to all listeners.
    pub fn fire_did_close_text_document(&self, doc: &TextDocumentInfo) -> Result<()> {
        let val = serde_json::to_value(doc)?;
        let listeners = self
            .on_did_close_text_document
            .read()
            .expect("event lock poisoned");
        for (_id, listener) in listeners.iter() {
            listener(val.clone())?;
        }
        Ok(())
    }

    /// Subscribes to `onDidSaveTextDocument`.
    pub fn subscribe_on_did_save_text_document(&self, listener: EventListener) -> EventListenerId {
        let id = self.next_event_id();
        self.on_did_save_text_document
            .write()
            .expect("event lock poisoned")
            .push((id, listener));
        id
    }

    /// Fires `onDidSaveTextDocument` to all listeners.
    pub fn fire_did_save_text_document(&self, doc: &TextDocumentInfo) -> Result<()> {
        let val = serde_json::to_value(doc)?;
        let listeners = self
            .on_did_save_text_document
            .read()
            .expect("event lock poisoned");
        for (_id, listener) in listeners.iter() {
            listener(val.clone())?;
        }
        Ok(())
    }

    /// Subscribes to `onDidChangeConfiguration`.
    pub fn subscribe_on_did_change_configuration(
        &self,
        listener: EventListener,
    ) -> EventListenerId {
        let id = self.next_event_id();
        self.on_did_change_configuration
            .write()
            .expect("event lock poisoned")
            .push((id, listener));
        id
    }

    /// Fires `onDidChangeConfiguration` to all listeners.
    pub fn fire_did_change_configuration(&self, event: &Value) -> Result<()> {
        let listeners = self
            .on_did_change_configuration
            .read()
            .expect("event lock poisoned");
        for (_id, listener) in listeners.iter() {
            listener(event.clone())?;
        }
        Ok(())
    }
}

impl Default for WorkspaceApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Best-effort language detection from a file URI/path.
fn detect_language_id(uri: &str) -> String {
    let ext = uri.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        "html" => "html",
        "css" => "css",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        _ => "plaintext",
    }
    .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rust() {
        assert_eq!(detect_language_id("src/main.rs"), "rust");
    }

    #[test]
    fn detect_typescript() {
        assert_eq!(detect_language_id("app.tsx"), "typescript");
    }

    #[test]
    fn detect_unknown() {
        assert_eq!(detect_language_id("Makefile"), "plaintext");
    }

    #[test]
    fn handle_get_configuration() {
        let api = WorkspaceApi::new();
        let result = api
            .handle(
                "getConfiguration",
                &serde_json::json!({ "section": "editor" }),
            )
            .unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn handle_save_all() {
        let api = WorkspaceApi::new();
        let result = api.handle("saveAll", &serde_json::Value::Null).unwrap();
        assert_eq!(result, serde_json::Value::Bool(true));
    }

    #[test]
    fn workspace_folder_management() {
        let api = WorkspaceApi::new();
        api.add_workspace_folder("file:///project", "project");
        assert_eq!(api.get_workspace_folders().len(), 1);
        assert!(api
            .get_workspace_folder("file:///project/src/main.rs")
            .is_some());
        assert!(api.get_workspace_folder("file:///other").is_none());
    }
}
