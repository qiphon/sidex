//! Extension lifecycle management and context handling.
//!
//! Provides the core infrastructure for managing extension activation,
//! deactivation, context storage, and extension metadata.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Extension Metadata
// ---------------------------------------------------------------------------

/// Extension identifier (publisher.name format).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionId(pub String);

impl ExtensionId {
    pub fn new(publisher: &str, name: &str) -> Self {
        Self(format!("{publisher}.{name}"))
    }

    pub fn publisher(&self) -> &str {
        self.0.split('.').next().unwrap_or("")
    }

    pub fn name(&self) -> &str {
        self.0.split('.').nth(1).unwrap_or("")
    }
}

/// Extension metadata from package.json / sidex.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub version: String,
    pub publisher: String,
    #[serde(default)]
    pub engines: Engines,
    #[serde(default)]
    pub activation_events: Vec<String>,
    #[serde(default)]
    pub contributes: Contributes,
    #[serde(default)]
    pub extension_kind: Vec<ExtensionKind>,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default)]
    pub wasm: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Engines {
    #[serde(default)]
    pub vscode: Option<String>,
    #[serde(default)]
    pub sidex: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contributes {
    #[serde(default)]
    pub commands: Vec<CommandContribution>,
    #[serde(default)]
    pub configuration: Option<ConfigurationContribution>,
    #[serde(default)]
    pub languages: Vec<LanguageContribution>,
    #[serde(default)]
    pub grammars: Vec<GrammarContribution>,
    #[serde(default)]
    pub themes: Vec<ThemeContribution>,
    #[serde(default)]
    pub views: HashMap<String, Vec<ViewContribution>>,
    #[serde(default)]
    pub menus: HashMap<String, Vec<MenuContribution>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandContribution {
    pub command: String,
    pub title: String,
    pub category: Option<String>,
    pub icon: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationContribution {
    pub title: String,
    pub properties: HashMap<String, ConfigurationProperty>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationProperty {
    #[serde(rename = "type")]
    pub property_type: String,
    pub description: Option<String>,
    pub default: Option<Value>,
    pub enum_values: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageContribution {
    pub id: String,
    pub aliases: Option<Vec<String>>,
    pub extensions: Option<Vec<String>>,
    pub filenames: Option<Vec<String>>,
    pub first_line: Option<String>,
    pub configuration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarContribution {
    pub language: String,
    pub scope_name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeContribution {
    pub label: String,
    pub ui_theme: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewContribution {
    pub id: String,
    pub name: String,
    pub when: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuContribution {
    pub command: String,
    pub when: Option<String>,
    pub group: Option<String>,
}

/// Extension kind (UI vs Worker).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionKind {
    Ui,
    Workspace,
}

// ---------------------------------------------------------------------------
// Extension State
// ---------------------------------------------------------------------------

/// Extension activation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionState {
    #[default]
    Disabled,
    Enabled,
    Activating,
    Activated,
    Deactivated,
}

/// Memento storage for extension state.
pub struct Memento {
    data: RwLock<HashMap<String, Value>>,
}

impl Memento {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    pub fn keys(&self) -> Vec<String> {
        self.data.read().expect("lock").keys().cloned().collect()
    }

    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data
            .read()
            .expect("lock")
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn set(&self, key: &str, value: Value) -> Result<()> {
        self.data.write().expect("lock").insert(key.to_string(), value);
        Ok(())
    }

    pub fn delete(&self, key: &str) -> bool {
        self.data.write().expect("lock").remove(key).is_some()
    }

    pub fn clear(&self) {
        self.data.write().expect("lock").clear();
    }
}

impl Default for Memento {
    fn default() -> Self {
        Self::new()
    }
}

/// Secret storage for sensitive data.
pub struct SecretStorage {
    data: RwLock<HashMap<String, String>>,
}

impl SecretStorage {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.data.read().expect("lock").get(key).cloned())
    }

    pub fn store(&self, key: &str, value: &str) -> Result<()> {
        self.data.write().expect("lock").insert(key.to_string(), value.to_string());
        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<bool> {
        Ok(self.data.write().expect("lock").remove(key).is_some())
    }
}

impl Default for SecretStorage {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Extension Context
// ---------------------------------------------------------------------------

/// Extension mode (mirrors `vscode.ExtensionMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionMode {
    /// Extension is running in production.
    Production,
    /// Extension is running in development/debug mode.
    #[default]
    Development,
    /// Extension is running in test mode.
    Test,
}

/// Environment variable collection for terminals.
#[derive(Debug, Default)]
pub struct GlobalEnvironmentVariableCollection {
    variables: RwLock<HashMap<String, String>>,
}

impl Clone for GlobalEnvironmentVariableCollection {
    fn clone(&self) -> Self {
        Self {
            variables: RwLock::new(self.variables.read().expect("lock").clone()),
        }
    }
}

impl GlobalEnvironmentVariableCollection {
    pub fn new() -> Self {
        Self {
            variables: RwLock::new(HashMap::new()),
        }
    }

    pub fn set(&self, key: &str, value: &str) {
        self.variables.write().expect("lock").insert(key.to_string(), value.to_string());
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.variables.read().expect("lock").get(key).cloned()
    }

    pub fn delete(&self, key: &str) -> bool {
        self.variables.write().expect("lock").remove(key).is_some()
    }

    pub fn clear(&self) {
        self.variables.write().expect("lock").clear();
    }

    pub fn apply_to_process(&self) -> std::collections::HashMap<String, String> {
        self.variables.read().expect("lock").clone()
    }
}

/// Extension context provided to activate/deactivate functions.
pub struct ExtensionContext {
    extension_id: ExtensionId,
    extension_path: String,
    extension_uri: String,
    workspace_state: Arc<Memento>,
    global_state: Arc<Memento>,
    secrets: Arc<SecretStorage>,
    environment_variable_collection: Arc<GlobalEnvironmentVariableCollection>,
    log_uri: String,
    storage_uri: Option<String>,
    global_storage_uri: String,
    extension_mode: ExtensionMode,
    subscriptions: RwLock<Vec<Box<dyn FnOnce() + Send>>>,
}

impl ExtensionContext {
    /// Creates a new extension context.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        extension_id: ExtensionId,
        extension_path: String,
        extension_uri: String,
        workspace_state: Arc<Memento>,
        global_state: Arc<Memento>,
        secrets: Arc<SecretStorage>,
        log_uri: String,
        storage_uri: Option<String>,
        global_storage_uri: String,
        extension_mode: ExtensionMode,
    ) -> Self {
        Self {
            extension_id,
            extension_path,
            extension_uri,
            workspace_state,
            global_state,
            secrets,
            environment_variable_collection: Arc::new(GlobalEnvironmentVariableCollection::new()),
            log_uri,
            storage_uri,
            global_storage_uri,
            extension_mode,
            subscriptions: RwLock::new(Vec::new()),
        }
    }

    pub fn extension_id(&self) -> &ExtensionId {
        &self.extension_id
    }

    pub fn extension_path(&self) -> &str {
        &self.extension_path
    }

    pub fn extension_uri(&self) -> &str {
        &self.extension_uri
    }

    pub fn workspace_state(&self) -> &Memento {
        &self.workspace_state
    }

    pub fn global_state(&self) -> &Memento {
        &self.global_state
    }

    pub fn secrets(&self) -> &SecretStorage {
        &self.secrets
    }

    pub fn environment_variable_collection(&self) -> &GlobalEnvironmentVariableCollection {
        &self.environment_variable_collection
    }

    pub fn log_uri(&self) -> &str {
        &self.log_uri
    }

    pub fn storage_uri(&self) -> Option<&str> {
        self.storage_uri.as_deref()
    }

    pub fn global_storage_uri(&self) -> &str {
        &self.global_storage_uri
    }

    pub fn extension_mode(&self) -> ExtensionMode {
        self.extension_mode
    }

    /// Adds a disposable to be cleaned up on deactivation.
    pub fn subscribe<F>(&self, dispose_fn: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.subscriptions
            .write()
            .expect("lock")
            .push(Box::new(dispose_fn));
    }

    /// Disposes all subscriptions.
    pub fn dispose_all(&self) {
        let subscriptions = self.subscriptions.write().expect("lock").drain(..).collect::<Vec<_>>();
        for sub in subscriptions {
            sub();
        }
    }

    /// Gets the absolute path of a resource within the extension.
    pub fn as_absolute_path(&self, relative_path: &str) -> String {
        std::path::Path::new(&self.extension_path)
            .join(relative_path)
            .to_string_lossy()
            .to_string()
    }
}

// ---------------------------------------------------------------------------
// Extension Manager
// ---------------------------------------------------------------------------

/// Manages extension lifecycle and state.
pub struct ExtensionManager {
    next_id: AtomicU32,
    extensions: RwLock<HashMap<ExtensionId, ExtensionInfo>>,
    activated_extensions: RwLock<HashMap<ExtensionId, Arc<ExtensionContext>>>,
}

struct ExtensionInfo {
    manifest: ExtensionManifest,
    state: ExtensionState,
    wasm_path: Option<String>,
    main_path: Option<String>,
}

impl ExtensionManager {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            extensions: RwLock::new(HashMap::new()),
            activated_extensions: RwLock::new(HashMap::new()),
        }
    }

    /// Registers an extension with the manager.
    pub fn register(&self, manifest: ExtensionManifest) -> Result<ExtensionId> {
        let extension_id = ExtensionId(manifest.id.clone());
        
        let info = ExtensionInfo {
            wasm_path: manifest.wasm.clone(),
            main_path: manifest.main.clone(),
            manifest,
            state: ExtensionState::Enabled,
        };

        self.extensions.write().expect("lock").insert(extension_id.clone(), info);
        Ok(extension_id)
    }

    /// Gets an extension by ID.
    pub fn get_extension(&self, id: &ExtensionId) -> Option<ExtensionManifest> {
        self.extensions
            .read()
            .expect("lock")
            .get(id)
            .map(|info| info.manifest.clone())
    }

    /// Lists all registered extensions.
    pub fn list_extensions(&self) -> Vec<ExtensionManifest> {
        self.extensions
            .read()
            .expect("lock")
            .values()
            .map(|info| info.manifest.clone())
            .collect()
    }

    /// Activates an extension.
    pub fn activate(
        &self,
        extension_id: &ExtensionId,
        context: Arc<ExtensionContext>,
    ) -> Result<()> {
        let mut extensions = self.extensions.write().expect("lock");
        
        let info = extensions
            .get_mut(extension_id)
            .ok_or_else(|| anyhow::anyhow!("Extension not found: {}", extension_id.0))?;

        if info.state == ExtensionState::Activated {
            return Ok(()); // Already activated
        }

        info.state = ExtensionState::Activating;
        
        // Store the context for later access
        drop(extensions);
        self.activated_extensions
            .write()
            .expect("lock")
            .insert(extension_id.clone(), context);

        // Update state to activated
        let mut extensions = self.extensions.write().expect("lock");
        if let Some(info) = extensions.get_mut(extension_id) {
            info.state = ExtensionState::Activated;
        }

        Ok(())
    }

    /// Deactivates an extension.
    pub fn deactivate(&self, extension_id: &ExtensionId) -> Result<()> {
        let mut extensions = self.extensions.write().expect("lock");
        
        let info = extensions
            .get_mut(extension_id)
            .ok_or_else(|| anyhow::anyhow!("Extension not found: {}", extension_id.0))?;

        if info.state != ExtensionState::Activated {
            return Ok(()); // Not activated
        }

        info.state = ExtensionState::Deactivated;
        drop(extensions);

        // Clean up context subscriptions
        if let Some(context) = self.activated_extensions.write().expect("lock").remove(extension_id) {
            context.dispose_all();
        }

        Ok(())
    }

    /// Checks if an extension is active.
    pub fn is_active(&self, extension_id: &ExtensionId) -> bool {
        self.extensions
            .read()
            .expect("lock")
            .get(extension_id)
            .map(|info| info.state == ExtensionState::Activated)
            .unwrap_or(false)
    }

    /// Gets the activation state of an extension.
    pub fn get_state(&self, extension_id: &ExtensionId) -> Option<ExtensionState> {
        self.extensions
            .read()
            .expect("lock")
            .get(extension_id)
            .map(|info| info.state)
    }

    /// Enables an extension.
    pub fn enable(&self, extension_id: &ExtensionId) -> Result<()> {
        let mut extensions = self.extensions.write().expect("lock");
        
        let info = extensions
            .get_mut(extension_id)
            .ok_or_else(|| anyhow::anyhow!("Extension not found: {}", extension_id.0))?;

        info.state = ExtensionState::Enabled;
        Ok(())
    }

    /// Disables an extension.
    pub fn disable(&self, extension_id: &ExtensionId) -> Result<()> {
        let mut extensions = self.extensions.write().expect("lock");
        
        let info = extensions
            .get_mut(extension_id)
            .ok_or_else(|| anyhow::anyhow!("Extension not found: {}", extension_id.0))?;

        // Deactivate if currently active
        if info.state == ExtensionState::Activated {
            drop(extensions);
            self.deactivate(extension_id)?;
            // Re-acquire lock after reactivation
            let mut extensions = self.extensions.write().expect("lock");
            let info = extensions
                .get_mut(extension_id)
                .ok_or_else(|| anyhow::anyhow!("Extension not found: {}", extension_id.0))?;
            info.state = ExtensionState::Disabled;
        } else {
            info.state = ExtensionState::Disabled;
        }
        
        Ok(())
    }

    /// Finds extensions that should be activated for a given event.
    pub fn find_extensions_for_event(&self, event: &str) -> Vec<ExtensionId> {
        self.extensions
            .read()
            .expect("lock")
            .iter()
            .filter(|(_, info)| {
                info.state == ExtensionState::Enabled
                    && info.manifest.activation_events.iter().any(|e| {
                        e == event || e == "*" || event.starts_with(&format!("{}:", e.trim_end_matches(':')))
                    })
            })
            .map(|(id, _)| id.clone())
            .collect()
    }
}

impl Default for ExtensionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_id() {
        let id = ExtensionId::new("publisher", "name");
        assert_eq!(id.0, "publisher.name");
        assert_eq!(id.publisher(), "publisher");
        assert_eq!(id.name(), "name");
    }

    #[test]
    fn test_memento() {
        let memento = Memento::new();
        memento.set("key1", Value::String("value1".to_string())).unwrap();
        memento.set("key2", Value::Number(42.into())).unwrap();
        
        assert_eq!(memento.keys().len(), 2);
        assert_eq!(memento.get::<String>("key1"), Some("value1".to_string()));
        assert_eq!(memento.get::<i32>("key2"), Some(42));
        
        memento.delete("key1");
        assert_eq!(memento.keys().len(), 1);
    }

    #[test]
    fn test_secret_storage() {
        let storage = SecretStorage::new();
        storage.store("token", "secret123").unwrap();
        
        assert_eq!(storage.get("token").unwrap(), Some("secret123".to_string()));
        
        storage.delete("token").unwrap();
        assert_eq!(storage.get("token").unwrap(), None);
    }

    #[test]
    fn test_extension_manager_lifecycle() {
        let manager = ExtensionManager::new();
        
        let manifest = ExtensionManifest {
            id: "test.ext".to_string(),
            name: "Test Extension".to_string(),
            display_name: Some("Test Extension".to_string()),
            description: Some("A test extension".to_string()),
            version: "1.0.0".to_string(),
            publisher: "test".to_string(),
            engines: Engines::default(),
            activation_events: vec!["onStartupFinished".to_string()],
            contributes: Contributes::default(),
            extension_kind: vec![ExtensionKind::Ui],
            main: None,
            wasm: None,
        };

        let ext_id = manager.register(manifest).unwrap();
        assert_eq!(manager.get_state(&ext_id), Some(ExtensionState::Enabled));
        
        let context = Arc::new(ExtensionContext::new(
            ext_id.clone(),
            "/path/to/ext".to_string(),
            "file:///path/to/ext".to_string(),
            Arc::new(Memento::new()),
            Arc::new(Memento::new()),
            Arc::new(SecretStorage::new()),
            "file:///logs".to_string(),
            Some("/storage".to_string()),
            "/global-storage".to_string(),
            ExtensionMode::Development,
        ));

        manager.activate(&ext_id, context).unwrap();
        assert_eq!(manager.get_state(&ext_id), Some(ExtensionState::Activated));
        assert!(manager.is_active(&ext_id));

        manager.deactivate(&ext_id).unwrap();
        assert_eq!(manager.get_state(&ext_id), Some(ExtensionState::Deactivated));
    }

    #[test]
    fn test_activation_events() {
        let manager = ExtensionManager::new();
        
        let manifest = ExtensionManifest {
            id: "test.lang".to_string(),
            name: "Language Extension".to_string(),
            display_name: None,
            description: None,
            version: "1.0.0".to_string(),
            publisher: "test".to_string(),
            engines: Engines::default(),
            activation_events: vec!["onLanguage:rust".to_string()],
            contributes: Contributes::default(),
            extension_kind: vec![ExtensionKind::Workspace],
            main: None,
            wasm: None,
        };

        let ext_id = manager.register(manifest).unwrap();
        
        // Should match onLanguage:rust event
        let matches = manager.find_extensions_for_event("onLanguage:rust");
        assert!(matches.contains(&ext_id));
        
        // Should not match other events
        let matches = manager.find_extensions_for_event("onLanguage:typescript");
        assert!(!matches.contains(&ext_id));
    }
}
