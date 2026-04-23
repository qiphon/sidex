//! Multi-root workspace — manages multiple workspace folders from a single
//! `.code-workspace` (or `.sidex-workspace`) file.
//!
//! Supports parsing and saving the workspace configuration, adding/removing
//! folders, per-folder settings overrides, and path resolution.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Workspace file format ───────────────────────────────────────────────

/// A folder entry in the workspace configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceFolder {
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uri: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub index: u32,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(v: &u32) -> bool {
    *v == 0
}

impl WorkspaceFolder {
    /// Create a folder entry from a path with an optional display name.
    #[must_use]
    pub fn new(path: &Path, name: Option<String>) -> Self {
        let uri = format!("file://{}", path.display());
        Self {
            path: path.to_path_buf(),
            name,
            uri,
            index: 0,
        }
    }
}

/// Extension recommendations embedded in a workspace file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceExtensions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommendations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unwanted_recommendations: Vec<String>,
}

/// Per-folder settings overlay (folder path → settings object).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FolderSettings {
    #[serde(default)]
    pub overrides: std::collections::HashMap<String, Value>,
}

/// Parsed `.code-workspace` / `.sidex-workspace` file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub folders: Vec<WorkspaceFolder>,
    #[serde(default)]
    pub settings: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<WorkspaceExtensions>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub launch: Value,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub tasks: Value,
}

// ── MultiRootWorkspace ──────────────────────────────────────────────────

/// Manages multiple workspace folders from a `.code-workspace` /
/// `.sidex-workspace` configuration.
pub struct MultiRootWorkspace {
    pub config: WorkspaceConfig,
    workspace_file: Option<PathBuf>,
    /// Per-folder settings that override the workspace-level settings.
    folder_settings: std::collections::HashMap<String, Value>,
}

impl MultiRootWorkspace {
    /// Create from an existing workspace config file.
    pub fn open(path: &Path) -> Result<Self, String> {
        let config = parse_workspace_file(path)?;
        Ok(Self {
            config,
            workspace_file: Some(path.to_path_buf()),
            folder_settings: std::collections::HashMap::new(),
        })
    }

    /// Create a new empty multi-root workspace.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: WorkspaceConfig::default(),
            workspace_file: None,
            folder_settings: std::collections::HashMap::new(),
        }
    }

    /// Create from a single root folder.
    #[must_use]
    pub fn from_single(root: &Path) -> Self {
        let mut ws = Self::new();
        ws.add_folder(root, None);
        ws
    }

    // ── Folder management ───────────────────────────────────────────────

    /// Add a folder to the workspace.
    pub fn add_folder(&mut self, path: &Path, name: Option<String>) {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        if self
            .config
            .folders
            .iter()
            .any(|f| std::fs::canonicalize(&f.path).unwrap_or_else(|_| f.path.clone()) == canonical)
        {
            return;
        }

        #[allow(clippy::cast_possible_truncation)]
        let index = self.config.folders.len() as u32;
        self.config.folders.push(WorkspaceFolder {
            path: path.to_path_buf(),
            name,
            uri: format!("file://{}", canonical.display()),
            index,
        });
    }

    /// Add a folder at a specific position.
    pub fn add_folder_at_index(
        &mut self,
        path: &Path,
        name: Option<String>,
        index: usize,
    ) -> Result<(), String> {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        if self
            .config
            .folders
            .iter()
            .any(|f| std::fs::canonicalize(&f.path).unwrap_or_else(|_| f.path.clone()) == canonical)
        {
            return Err("folder already in workspace".into());
        }

        let clamped = index.min(self.config.folders.len());
        self.config.folders.insert(
            clamped,
            WorkspaceFolder {
                path: path.to_path_buf(),
                name,
                uri: format!("file://{}", canonical.display()),
                index: 0,
            },
        );
        self.reindex_folders();
        Ok(())
    }

    /// Remove a folder from the workspace by path.
    pub fn remove_folder(&mut self, path: &Path) {
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.config.folders.retain(|f| {
            std::fs::canonicalize(&f.path).unwrap_or_else(|_| f.path.clone()) != canonical
        });
        self.reindex_folders();
    }

    /// Remove a folder from the workspace by index.
    pub fn remove_folder_at_index(&mut self, index: u32) -> Result<(), String> {
        let idx = index as usize;
        if idx >= self.config.folders.len() {
            return Err(format!("folder index {index} out of range"));
        }
        self.config.folders.remove(idx);
        self.reindex_folders();
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn reindex_folders(&mut self) {
        for (i, folder) in self.config.folders.iter_mut().enumerate() {
            folder.index = i as u32;
        }
    }

    /// List all workspace folders.
    #[must_use]
    pub fn folders(&self) -> &[WorkspaceFolder] {
        &self.config.folders
    }

    /// Number of workspace folders.
    #[must_use]
    pub fn folder_count(&self) -> usize {
        self.config.folders.len()
    }

    /// Whether this is a multi-root workspace (more than one folder).
    #[must_use]
    pub fn is_multi_root(&self) -> bool {
        self.config.folders.len() > 1
    }

    /// Get the display name for a folder (uses the override name, or the dir name).
    #[must_use]
    pub fn folder_name(folder: &WorkspaceFolder) -> String {
        folder.name.clone().unwrap_or_else(|| {
            folder.path.file_name().map_or_else(
                || folder.path.to_string_lossy().to_string(),
                |n| n.to_string_lossy().to_string(),
            )
        })
    }

    /// Find which workspace folder a file belongs to.
    #[must_use]
    pub fn find_folder_for_file(&self, file_path: &Path) -> Option<&WorkspaceFolder> {
        let canonical =
            std::fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
        self.config.folders.iter().find(|f| {
            let folder_path = std::fs::canonicalize(&f.path).unwrap_or_else(|_| f.path.clone());
            canonical.starts_with(&folder_path)
        })
    }

    /// Get the folder at the given index.
    #[must_use]
    pub fn get_folder(&self, index: u32) -> Option<&WorkspaceFolder> {
        self.config.folders.get(index as usize)
    }

    // ── Path resolution ─────────────────────────────────────────────────

    /// Resolve a relative path against the workspace file's directory, or
    /// against the first workspace folder if no workspace file is set.
    #[must_use]
    pub fn resolve_workspace_path(&self, relative: &str) -> PathBuf {
        let base = self
            .workspace_file
            .as_ref()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
            .or_else(|| self.config.folders.first().map(|f| f.path.clone()))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join(relative)
    }

    /// Make a path relative to the workspace file directory.
    #[must_use]
    pub fn make_relative(&self, absolute: &Path) -> Option<PathBuf> {
        let base = self.workspace_file.as_ref().and_then(|p| p.parent())?;
        pathdiff::diff_paths(absolute, base)
    }

    // ── Per-folder settings ─────────────────────────────────────────────

    /// Set settings override for a specific folder.
    pub fn set_folder_settings(&mut self, folder_path: &Path, settings: Value) {
        let key = folder_path.to_string_lossy().to_string();
        self.folder_settings.insert(key, settings);
    }

    /// Get the effective setting value for a file, checking folder overrides
    /// first, then workspace-level settings.
    #[must_use]
    pub fn get_effective_setting(&self, file_path: &Path, key: &str) -> Option<&Value> {
        if let Some(folder) = self.find_folder_for_file(file_path) {
            let folder_key = folder.path.to_string_lossy().to_string();
            if let Some(folder_settings) = self.folder_settings.get(&folder_key) {
                if let Some(val) = folder_settings.as_object().and_then(|o| o.get(key)) {
                    return Some(val);
                }
            }
        }
        self.config.settings.as_object().and_then(|o| o.get(key))
    }

    // ── Settings ────────────────────────────────────────────────────────

    /// Update workspace-level settings.
    pub fn set_settings(&mut self, settings: Value) {
        self.config.settings = settings;
    }

    /// Get the workspace-level settings.
    #[must_use]
    pub fn settings(&self) -> &Value {
        &self.config.settings
    }

    // ── Extension recommendations ───────────────────────────────────────

    /// Get extension recommendations.
    #[must_use]
    pub fn extension_recommendations(&self) -> Vec<String> {
        self.config
            .extensions
            .as_ref()
            .map(|e| e.recommendations.clone())
            .unwrap_or_default()
    }

    /// Add an extension recommendation.
    pub fn add_extension_recommendation(&mut self, extension_id: &str) {
        let ext = self
            .config
            .extensions
            .get_or_insert_with(WorkspaceExtensions::default);
        if !ext.recommendations.contains(&extension_id.to_string()) {
            ext.recommendations.push(extension_id.to_string());
        }
    }

    /// Mark an extension as unwanted.
    pub fn add_unwanted_recommendation(&mut self, extension_id: &str) {
        let ext = self
            .config
            .extensions
            .get_or_insert_with(WorkspaceExtensions::default);
        if !ext
            .unwanted_recommendations
            .contains(&extension_id.to_string())
        {
            ext.unwanted_recommendations.push(extension_id.to_string());
        }
    }

    // ── Persistence ─────────────────────────────────────────────────────

    /// Save the workspace configuration to a file.
    pub fn save_workspace(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.config)
            .map_err(|e| format!("serialize workspace: {e}"))?;
        std::fs::write(path, json).map_err(|e| format!("write workspace file: {e}"))
    }

    /// Save to the original workspace file (if opened from one).
    pub fn save(&self) -> Result<(), String> {
        match &self.workspace_file {
            Some(path) => self.save_workspace(path),
            None => Err("no workspace file path set".to_string()),
        }
    }

    /// The path of the workspace file, if any.
    #[must_use]
    pub fn workspace_file_path(&self) -> Option<&Path> {
        self.workspace_file.as_deref()
    }
}

impl Default for MultiRootWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

// ── Parsing ─────────────────────────────────────────────────────────────

/// Parse a `.code-workspace` or `.sidex-workspace` JSON file.
pub fn parse_workspace_file(path: &Path) -> Result<WorkspaceConfig, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read workspace file: {e}"))?;
    let mut config: WorkspaceConfig =
        serde_json::from_str(&content).map_err(|e| format!("parse workspace JSON: {e}"))?;
    #[allow(clippy::cast_possible_truncation)]
    for (i, folder) in config.folders.iter_mut().enumerate() {
        folder.index = i as u32;
        if folder.uri.is_empty() {
            folder.uri = format!("file://{}", folder.path.display());
        }
    }
    Ok(config)
}

/// Save a workspace config to a file.
pub fn save_workspace_file(workspace: &WorkspaceConfig, path: &Path) -> Result<(), String> {
    let json =
        serde_json::to_string_pretty(workspace).map_err(|e| format!("serialize workspace: {e}"))?;
    std::fs::write(path, json).map_err(|e| format!("write workspace file: {e}"))
}

/// Check if a path looks like a workspace file (by extension).
#[must_use]
pub fn is_workspace_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(ext, "code-workspace" | "sidex-workspace")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_workspace_config() {
        let json = r#"{
            "folders": [
                { "path": "/home/user/project-a" },
                { "path": "/home/user/project-b", "name": "Backend" }
            ],
            "settings": { "editor.fontSize": 14 }
        }"#;

        let config: WorkspaceConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.folders.len(), 2);
        assert_eq!(
            config.folders[0].path,
            PathBuf::from("/home/user/project-a")
        );
        assert_eq!(config.folders[1].name.as_deref(), Some("Backend"));
        assert!(config.settings.is_object());
    }

    #[test]
    fn add_and_remove_folders() {
        let mut ws = MultiRootWorkspace::new();
        assert_eq!(ws.folder_count(), 0);
        assert!(!ws.is_multi_root());

        ws.add_folder(Path::new("/tmp/a"), None);
        assert_eq!(ws.folder_count(), 1);

        ws.add_folder(Path::new("/tmp/b"), Some("Second".into()));
        assert_eq!(ws.folder_count(), 2);
        assert!(ws.is_multi_root());

        ws.remove_folder(Path::new("/tmp/a"));
        assert_eq!(ws.folder_count(), 1);
        assert!(!ws.is_multi_root());
    }

    #[test]
    fn remove_by_index() {
        let mut ws = MultiRootWorkspace::new();
        ws.add_folder(Path::new("/tmp/a"), None);
        ws.add_folder(Path::new("/tmp/b"), None);
        ws.add_folder(Path::new("/tmp/c"), None);

        ws.remove_folder_at_index(1).unwrap();
        assert_eq!(ws.folder_count(), 2);
        assert_eq!(ws.folders()[0].index, 0);
        assert_eq!(ws.folders()[1].index, 1);
    }

    #[test]
    fn remove_by_index_out_of_range() {
        let mut ws = MultiRootWorkspace::new();
        assert!(ws.remove_folder_at_index(5).is_err());
    }

    #[test]
    fn no_duplicate_folders() {
        let mut ws = MultiRootWorkspace::new();
        ws.add_folder(Path::new("/tmp/dup"), None);
        ws.add_folder(Path::new("/tmp/dup"), None);
        assert_eq!(ws.folder_count(), 1);
    }

    #[test]
    fn folder_name_display() {
        let f1 = WorkspaceFolder {
            path: PathBuf::from("/home/user/my-project"),
            name: None,
            uri: String::new(),
            index: 0,
        };
        assert_eq!(MultiRootWorkspace::folder_name(&f1), "my-project");

        let f2 = WorkspaceFolder {
            path: PathBuf::from("/home/user/my-project"),
            name: Some("Custom Name".into()),
            uri: String::new(),
            index: 0,
        };
        assert_eq!(MultiRootWorkspace::folder_name(&f2), "Custom Name");
    }

    #[test]
    fn from_single_folder() {
        let ws = MultiRootWorkspace::from_single(Path::new("/tmp/single"));
        assert_eq!(ws.folder_count(), 1);
        assert!(!ws.is_multi_root());
    }

    #[test]
    fn save_and_reload() {
        let tmp = std::env::temp_dir().join("sidex-ws-test.code-workspace");

        let mut ws = MultiRootWorkspace::new();
        ws.add_folder(Path::new("/tmp/folder-a"), None);
        ws.add_folder(Path::new("/tmp/folder-b"), Some("B".into()));
        ws.save_workspace(&tmp).unwrap();

        let loaded = MultiRootWorkspace::open(&tmp).unwrap();
        assert_eq!(loaded.folder_count(), 2);

        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn settings_get_set() {
        let mut ws = MultiRootWorkspace::new();
        ws.set_settings(serde_json::json!({"editor.tabSize": 2}));
        assert!(ws.settings().is_object());
    }

    #[test]
    fn serialization_roundtrip() {
        let config = WorkspaceConfig {
            folders: vec![WorkspaceFolder {
                path: PathBuf::from("./src"),
                name: Some("Source".into()),
                uri: "file://./src".into(),
                index: 0,
            }],
            settings: serde_json::json!({}),
            extensions: None,
            launch: Value::Null,
            tasks: Value::Null,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: WorkspaceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.folders.len(), 1);
    }

    #[test]
    fn resolve_workspace_path_relative() {
        let mut ws = MultiRootWorkspace::new();
        ws.add_folder(Path::new("/home/user/project"), None);
        let resolved = ws.resolve_workspace_path("src/main.rs");
        assert!(resolved.to_string_lossy().contains("src/main.rs"));
    }

    #[test]
    fn folder_settings_override() {
        let mut ws = MultiRootWorkspace::new();
        ws.add_folder(Path::new("/tmp/proj"), None);
        ws.set_settings(serde_json::json!({"editor.tabSize": 4}));
        ws.set_folder_settings(
            Path::new("/tmp/proj"),
            serde_json::json!({"editor.tabSize": 2}),
        );

        let val = ws.get_effective_setting(Path::new("/tmp/proj/src/main.rs"), "editor.tabSize");
        assert_eq!(val, Some(&serde_json::json!(2)));
    }

    #[test]
    fn extension_recommendations() {
        let mut ws = MultiRootWorkspace::new();
        assert!(ws.extension_recommendations().is_empty());

        ws.add_extension_recommendation("rust-lang.rust-analyzer");
        assert_eq!(ws.extension_recommendations().len(), 1);

        ws.add_extension_recommendation("rust-lang.rust-analyzer");
        assert_eq!(ws.extension_recommendations().len(), 1);

        ws.add_unwanted_recommendation("ms-toolsai.jupyter");
        assert_eq!(
            ws.config
                .extensions
                .as_ref()
                .unwrap()
                .unwanted_recommendations
                .len(),
            1
        );
    }

    #[test]
    fn is_workspace_file_detection() {
        assert!(is_workspace_file(Path::new("project.code-workspace")));
        assert!(is_workspace_file(Path::new("project.sidex-workspace")));
        assert!(!is_workspace_file(Path::new("project.json")));
    }

    #[test]
    fn workspace_folder_new() {
        let folder = WorkspaceFolder::new(Path::new("/tmp/project"), Some("My Project".into()));
        assert_eq!(folder.path, PathBuf::from("/tmp/project"));
        assert_eq!(folder.name.as_deref(), Some("My Project"));
        assert!(folder.uri.starts_with("file://"));
    }

    #[test]
    fn save_workspace_file_fn() {
        let tmp = std::env::temp_dir().join("sidex-save-ws-test.sidex-workspace");
        let config = WorkspaceConfig {
            folders: vec![WorkspaceFolder::new(Path::new("/tmp/a"), None)],
            settings: serde_json::json!({}),
            extensions: None,
            launch: Value::Null,
            tasks: Value::Null,
        };
        save_workspace_file(&config, &tmp).unwrap();
        let reloaded = parse_workspace_file(&tmp).unwrap();
        assert_eq!(reloaded.folders.len(), 1);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn get_folder_by_index() {
        let mut ws = MultiRootWorkspace::new();
        ws.add_folder(Path::new("/tmp/x"), None);
        ws.add_folder(Path::new("/tmp/y"), None);
        assert!(ws.get_folder(0).is_some());
        assert!(ws.get_folder(1).is_some());
        assert!(ws.get_folder(2).is_none());
    }
}
