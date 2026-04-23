//! Settings sync — export/import settings, keybindings, extensions, snippets,
//! global state, and profiles between machines.
//!
//! Provides a JSON-based sync format with three-way merge support for
//! conflict resolution, account-based sync state management, and
//! auto-sync capabilities.

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Sync resource ───────────────────────────────────────────────────────

/// Identifies which category of data to synchronize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SyncResource {
    Settings,
    Keybindings,
    Extensions,
    Snippets,
    GlobalState,
    Profiles,
}

impl SyncResource {
    /// All resource kinds.
    pub const ALL: &'static [SyncResource] = &[
        SyncResource::Settings,
        SyncResource::Keybindings,
        SyncResource::Extensions,
        SyncResource::Snippets,
        SyncResource::GlobalState,
        SyncResource::Profiles,
    ];

    /// The file name used when persisting this resource.
    #[must_use]
    pub fn file_name(self) -> &'static str {
        match self {
            SyncResource::Settings => "settings.json",
            SyncResource::Keybindings => "keybindings.json",
            SyncResource::Extensions => "extensions.json",
            SyncResource::Snippets => "snippets.json",
            SyncResource::GlobalState => "globalState.json",
            SyncResource::Profiles => "profiles.json",
        }
    }

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            SyncResource::Settings => "Settings",
            SyncResource::Keybindings => "Keybindings",
            SyncResource::Extensions => "Extensions",
            SyncResource::Snippets => "Snippets",
            SyncResource::GlobalState => "UI State",
            SyncResource::Profiles => "Profiles",
        }
    }
}

// ── Sync state ──────────────────────────────────────────────────────────

/// Overall state of the sync system.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncState {
    #[default]
    Off,
    Syncing,
    Synced,
    Error(String),
    HasConflicts,
}

// ── Conflict resolution ─────────────────────────────────────────────────

/// A conflict between local and remote versions of a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    pub resource: SyncResource,
    pub local_content: Value,
    pub remote_content: Value,
}

/// How to resolve a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictResolution {
    AcceptLocal,
    AcceptRemote,
    Merge,
}

/// The result of resolving a conflict.
#[derive(Debug, Clone)]
pub struct ResolvedConflict {
    pub resource: SyncResource,
    pub resolution: ConflictResolution,
    pub content: Value,
}

// ── Sync result ─────────────────────────────────────────────────────────

/// Summary of a sync operation.
#[derive(Debug, Clone)]
pub struct SyncResult {
    pub resources_synced: Vec<SyncResource>,
    pub conflicts: Vec<SyncConflict>,
    pub errors: Vec<String>,
}

impl SyncResult {
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.errors.is_empty() && self.conflicts.is_empty()
    }
}

// ── Sync account ────────────────────────────────────────────────────────

/// Represents the account used for syncing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAccount {
    pub provider: SyncAuthProvider,
    pub account_name: String,
    pub session_id: String,
}

/// Supported authentication providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncAuthProvider {
    GitHub,
    Microsoft,
}

impl std::fmt::Display for SyncAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncAuthProvider::GitHub => write!(f, "GitHub"),
            SyncAuthProvider::Microsoft => write!(f, "Microsoft"),
        }
    }
}

// ── Sync data payload ───────────────────────────────────────────────────

/// Container for all synchronized data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncData {
    pub version: u32,
    pub machine_id: String,
    pub resources: Vec<SyncResourceData>,
}

/// A single resource's synchronized payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResourceData {
    pub resource: SyncResource,
    pub content: Value,
}

// ── Sync data provider trait ────────────────────────────────────────────

/// Trait for reading/writing sync resources (implemented by the settings layer).
pub trait SyncDataProvider {
    fn read_resource(&self, resource: SyncResource) -> Result<Value, String>;
    fn write_resource(&mut self, resource: SyncResource, value: &Value) -> Result<(), String>;
}

// ── SettingsSync manager ────────────────────────────────────────────────

/// Manages settings synchronization including account state, conflict
/// tracking, and auto-sync behavior.
pub struct SettingsSync {
    machine_id: String,
    state: SyncState,
    account: Option<SyncAccount>,
    enabled_resources: Vec<SyncResource>,
    last_sync: Option<Instant>,
    conflicts: Vec<SyncConflict>,
    auto_sync: bool,
}

impl SettingsSync {
    /// Create a new sync manager with the given machine identifier.
    #[must_use]
    pub fn new(machine_id: impl Into<String>) -> Self {
        Self {
            machine_id: machine_id.into(),
            state: SyncState::Off,
            account: None,
            enabled_resources: SyncResource::ALL.to_vec(),
            last_sync: None,
            conflicts: Vec::new(),
            auto_sync: true,
        }
    }

    // ── Account management ──────────────────────────────────────────────

    /// Enable sync with the given account.
    pub fn enable_sync(&mut self, account: SyncAccount) {
        self.account = Some(account);
        self.state = SyncState::Synced;
    }

    /// Disable sync entirely.
    pub fn disable_sync(&mut self) {
        self.account = None;
        self.state = SyncState::Off;
        self.last_sync = None;
        self.conflicts.clear();
    }

    /// Whether sync is currently enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.account.is_some() && self.state != SyncState::Off
    }

    /// The current sync account, if any.
    #[must_use]
    pub fn account(&self) -> Option<&SyncAccount> {
        self.account.as_ref()
    }

    /// Current sync state.
    #[must_use]
    pub fn state(&self) -> &SyncState {
        &self.state
    }

    /// When the last successful sync occurred.
    #[must_use]
    pub fn last_sync(&self) -> Option<Instant> {
        self.last_sync
    }

    // ── Resource selection ──────────────────────────────────────────────

    /// Choose which resources to sync.
    pub fn set_enabled_resources(&mut self, resources: Vec<SyncResource>) {
        self.enabled_resources = resources;
    }

    /// Currently enabled resources.
    #[must_use]
    pub fn enabled_resources(&self) -> &[SyncResource] {
        &self.enabled_resources
    }

    /// Toggle a specific resource on/off.
    pub fn toggle_resource(&mut self, resource: SyncResource) {
        if let Some(pos) = self.enabled_resources.iter().position(|r| *r == resource) {
            self.enabled_resources.remove(pos);
        } else {
            self.enabled_resources.push(resource);
        }
    }

    // ── Auto-sync ───────────────────────────────────────────────────────

    #[must_use]
    pub fn auto_sync_enabled(&self) -> bool {
        self.auto_sync
    }

    pub fn set_auto_sync(&mut self, enabled: bool) {
        self.auto_sync = enabled;
    }

    // ── Conflict management ─────────────────────────────────────────────

    /// Current unresolved conflicts.
    #[must_use]
    pub fn conflicts(&self) -> &[SyncConflict] {
        &self.conflicts
    }

    /// Whether there are unresolved conflicts.
    #[must_use]
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    /// Resolve a conflict for a given resource.
    pub fn resolve_conflict(
        &mut self,
        resource: SyncResource,
        resolution: ConflictResolution,
        provider: &mut dyn SyncDataProvider,
    ) -> Result<(), String> {
        let conflict = self
            .conflicts
            .iter()
            .find(|c| c.resource == resource)
            .ok_or("no conflict for this resource")?
            .clone();

        let resolved_value = match resolution {
            ConflictResolution::AcceptLocal => conflict.local_content,
            ConflictResolution::AcceptRemote => conflict.remote_content.clone(),
            ConflictResolution::Merge => merge(&conflict.local_content, &conflict.remote_content),
        };

        provider.write_resource(resource, &resolved_value)?;
        self.conflicts.retain(|c| c.resource != resource);

        if self.conflicts.is_empty() && self.state == SyncState::HasConflicts {
            self.state = SyncState::Synced;
        }

        Ok(())
    }

    // ── Core sync operations ────────────────────────────────────────────

    /// Export specified resources into a portable `SyncData` bundle.
    pub fn export(
        &self,
        resources: &[SyncResource],
        provider: &dyn SyncDataProvider,
    ) -> Result<SyncData, String> {
        let mut entries = Vec::new();
        for &res in resources {
            if !self.enabled_resources.contains(&res) {
                continue;
            }
            let content = provider
                .read_resource(res)
                .map_err(|e| format!("read {res:?}: {e}"))?;
            entries.push(SyncResourceData {
                resource: res,
                content,
            });
        }
        Ok(SyncData {
            version: 1,
            machine_id: self.machine_id.clone(),
            resources: entries,
        })
    }

    /// Import resources from a `SyncData` bundle, merging with local state.
    /// Returns a `SyncResult` with any conflicts detected.
    pub fn import(
        &mut self,
        data: &SyncData,
        provider: &mut dyn SyncDataProvider,
    ) -> Result<SyncResult, String> {
        self.state = SyncState::Syncing;

        let mut result = SyncResult {
            resources_synced: Vec::new(),
            conflicts: Vec::new(),
            errors: Vec::new(),
        };

        for entry in &data.resources {
            if !self.enabled_resources.contains(&entry.resource) {
                continue;
            }

            let local = provider
                .read_resource(entry.resource)
                .unwrap_or(Value::Null);

            if local != Value::Null
                && local != entry.content
                && has_local_changes(&local, &entry.content)
            {
                let conflict = SyncConflict {
                    resource: entry.resource,
                    local_content: local,
                    remote_content: entry.content.clone(),
                };
                result.conflicts.push(conflict.clone());
                self.conflicts.push(conflict);
            } else {
                let merged = merge(&local, &entry.content);
                match provider.write_resource(entry.resource, &merged) {
                    Ok(()) => result.resources_synced.push(entry.resource),
                    Err(e) => result.errors.push(format!("{:?}: {e}", entry.resource)),
                }
            }
        }

        self.last_sync = Some(Instant::now());

        if !self.conflicts.is_empty() {
            self.state = SyncState::HasConflicts;
        } else if result.errors.is_empty() {
            self.state = SyncState::Synced;
        } else {
            self.state = SyncState::Error(result.errors.join("; "));
        }

        Ok(result)
    }

    /// Perform a full sync: export local, compare with remote data, and import.
    pub fn sync_now(
        &mut self,
        remote_data: Option<&SyncData>,
        provider: &mut dyn SyncDataProvider,
    ) -> Result<SyncResult, String> {
        if let Some(data) = remote_data {
            self.import(data, provider)
        } else {
            self.last_sync = Some(Instant::now());
            self.state = SyncState::Synced;
            Ok(SyncResult {
                resources_synced: Vec::new(),
                conflicts: Vec::new(),
                errors: Vec::new(),
            })
        }
    }

    /// Export all resources to JSON string.
    pub fn export_json(
        &self,
        resources: &[SyncResource],
        provider: &dyn SyncDataProvider,
    ) -> Result<String, String> {
        let data = self.export(resources, provider)?;
        serde_json::to_string_pretty(&data).map_err(|e| format!("serialize: {e}"))
    }

    /// Import from a JSON string.
    pub fn import_json(
        &mut self,
        json: &str,
        provider: &mut dyn SyncDataProvider,
    ) -> Result<SyncResult, String> {
        let data: SyncData =
            serde_json::from_str(json).map_err(|e| format!("parse sync data: {e}"))?;
        self.import(&data, provider)
    }
}

// ── Merge helpers ───────────────────────────────────────────────────────

/// Check whether the local content diverges from the remote in a way that
/// constitutes a genuine conflict (not just a trivial overwrite).
fn has_local_changes(local: &Value, remote: &Value) -> bool {
    match (local, remote) {
        (Value::Object(lm), Value::Object(rm)) => {
            for (key, local_val) in lm {
                match rm.get(key) {
                    Some(remote_val) if local_val != remote_val => return true,
                    None => return true,
                    _ => {}
                }
            }
            false
        }
        (l, r) => l != r,
    }
}

/// Three-way merge of two JSON values.
///
/// For objects: merges keys from both sides (remote wins on conflict at leaf level).
/// For arrays: uses the remote value if different.
/// For scalars: remote wins.
#[must_use]
pub fn merge(local: &Value, remote: &Value) -> Value {
    match (local, remote) {
        (Value::Object(local_map), Value::Object(remote_map)) => {
            let mut merged = local_map.clone();
            for (key, remote_val) in remote_map {
                let merged_val = if let Some(local_val) = local_map.get(key) {
                    merge(local_val, remote_val)
                } else {
                    remote_val.clone()
                };
                merged.insert(key.clone(), merged_val);
            }
            Value::Object(merged)
        }
        (_, remote) => remote.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    struct TestProvider {
        data: HashMap<SyncResource, Value>,
    }

    impl TestProvider {
        fn new() -> Self {
            Self {
                data: HashMap::new(),
            }
        }
    }

    impl SyncDataProvider for TestProvider {
        fn read_resource(&self, resource: SyncResource) -> Result<Value, String> {
            Ok(self.data.get(&resource).cloned().unwrap_or(Value::Null))
        }

        fn write_resource(&mut self, resource: SyncResource, value: &Value) -> Result<(), String> {
            self.data.insert(resource, value.clone());
            Ok(())
        }
    }

    #[test]
    fn merge_objects() {
        let local = json!({"a": 1, "b": 2});
        let remote = json!({"b": 3, "c": 4});
        let result = merge(&local, &remote);
        assert_eq!(result, json!({"a": 1, "b": 3, "c": 4}));
    }

    #[test]
    fn merge_nested() {
        let local = json!({"editor": {"fontSize": 14, "tabSize": 4}});
        let remote = json!({"editor": {"fontSize": 16, "wordWrap": "on"}});
        let result = merge(&local, &remote);
        assert_eq!(
            result,
            json!({"editor": {"fontSize": 16, "tabSize": 4, "wordWrap": "on"}})
        );
    }

    #[test]
    fn merge_scalar() {
        let local = json!(42);
        let remote = json!(99);
        assert_eq!(merge(&local, &remote), json!(99));
    }

    #[test]
    fn export_import_roundtrip() {
        let mut sync = SettingsSync::new("machine-1");
        sync.enable_sync(SyncAccount {
            provider: SyncAuthProvider::GitHub,
            account_name: "test-user".into(),
            session_id: "sess-1".into(),
        });

        let mut source = TestProvider::new();
        source
            .data
            .insert(SyncResource::Settings, json!({"theme": "dark"}));
        source
            .data
            .insert(SyncResource::Keybindings, json!([{"key": "ctrl+s"}]));

        let exported = sync
            .export(
                &[SyncResource::Settings, SyncResource::Keybindings],
                &source,
            )
            .unwrap();

        assert_eq!(exported.resources.len(), 2);
        assert_eq!(exported.machine_id, "machine-1");

        let mut target = TestProvider::new();
        let mut sync2 = SettingsSync::new("machine-2");
        sync2.enable_sync(SyncAccount {
            provider: SyncAuthProvider::GitHub,
            account_name: "test-user".into(),
            session_id: "sess-2".into(),
        });
        let result = sync2.import(&exported, &mut target).unwrap();
        assert!(result.is_success());

        assert_eq!(
            target.data.get(&SyncResource::Settings).unwrap(),
            &json!({"theme": "dark"})
        );
    }

    #[test]
    fn export_import_json() {
        let mut sync = SettingsSync::new("m2");
        sync.enable_sync(SyncAccount {
            provider: SyncAuthProvider::Microsoft,
            account_name: "user".into(),
            session_id: "s".into(),
        });

        let mut provider = TestProvider::new();
        provider
            .data
            .insert(SyncResource::Extensions, json!(["ext-a"]));

        let json_str = sync
            .export_json(&[SyncResource::Extensions], &provider)
            .unwrap();
        assert!(json_str.contains("ext-a"));

        let mut target = TestProvider::new();
        let mut sync2 = SettingsSync::new("m3");
        sync2.enable_sync(SyncAccount {
            provider: SyncAuthProvider::Microsoft,
            account_name: "user".into(),
            session_id: "s".into(),
        });
        let result = sync2.import_json(&json_str, &mut target).unwrap();
        assert!(result.is_success());
        assert_eq!(
            target.data.get(&SyncResource::Extensions).unwrap(),
            &json!(["ext-a"])
        );
    }

    #[test]
    fn resource_file_names() {
        assert_eq!(SyncResource::Settings.file_name(), "settings.json");
        assert_eq!(SyncResource::Profiles.file_name(), "profiles.json");
    }

    #[test]
    fn resource_labels() {
        assert_eq!(SyncResource::Settings.label(), "Settings");
        assert_eq!(SyncResource::GlobalState.label(), "UI State");
    }

    #[test]
    fn all_resources() {
        assert_eq!(SyncResource::ALL.len(), 6);
    }

    #[test]
    fn sync_state_management() {
        let mut sync = SettingsSync::new("m1");
        assert_eq!(*sync.state(), SyncState::Off);
        assert!(!sync.is_enabled());

        sync.enable_sync(SyncAccount {
            provider: SyncAuthProvider::GitHub,
            account_name: "user".into(),
            session_id: "s".into(),
        });
        assert!(sync.is_enabled());
        assert_eq!(*sync.state(), SyncState::Synced);

        sync.disable_sync();
        assert!(!sync.is_enabled());
        assert_eq!(*sync.state(), SyncState::Off);
    }

    #[test]
    fn toggle_resource() {
        let mut sync = SettingsSync::new("m1");
        assert_eq!(sync.enabled_resources().len(), 6);

        sync.toggle_resource(SyncResource::Snippets);
        assert_eq!(sync.enabled_resources().len(), 5);
        assert!(!sync.enabled_resources().contains(&SyncResource::Snippets));

        sync.toggle_resource(SyncResource::Snippets);
        assert!(sync.enabled_resources().contains(&SyncResource::Snippets));
    }

    #[test]
    fn auto_sync() {
        let mut sync = SettingsSync::new("m1");
        assert!(sync.auto_sync_enabled());
        sync.set_auto_sync(false);
        assert!(!sync.auto_sync_enabled());
    }

    #[test]
    fn conflict_detection_and_resolution() {
        let mut sync = SettingsSync::new("m1");
        sync.enable_sync(SyncAccount {
            provider: SyncAuthProvider::GitHub,
            account_name: "user".into(),
            session_id: "s".into(),
        });

        let mut provider = TestProvider::new();
        provider.data.insert(
            SyncResource::Settings,
            json!({"theme": "dark", "fontSize": 14}),
        );

        let remote = SyncData {
            version: 1,
            machine_id: "other".into(),
            resources: vec![SyncResourceData {
                resource: SyncResource::Settings,
                content: json!({"theme": "light", "fontSize": 16}),
            }],
        };

        let result = sync.import(&remote, &mut provider).unwrap();
        assert!(!result.conflicts.is_empty());
        assert!(sync.has_conflicts());
        assert_eq!(*sync.state(), SyncState::HasConflicts);

        sync.resolve_conflict(
            SyncResource::Settings,
            ConflictResolution::AcceptRemote,
            &mut provider,
        )
        .unwrap();
        assert!(!sync.has_conflicts());
        assert_eq!(*sync.state(), SyncState::Synced);
        assert_eq!(
            provider.data.get(&SyncResource::Settings).unwrap(),
            &json!({"theme": "light", "fontSize": 16})
        );
    }

    #[test]
    fn sync_now_no_remote() {
        let mut sync = SettingsSync::new("m1");
        sync.enable_sync(SyncAccount {
            provider: SyncAuthProvider::GitHub,
            account_name: "user".into(),
            session_id: "s".into(),
        });

        let mut provider = TestProvider::new();
        let result = sync.sync_now(None, &mut provider).unwrap();
        assert!(result.is_success());
        assert!(sync.last_sync().is_some());
    }

    #[test]
    fn skips_disabled_resources() {
        let mut sync = SettingsSync::new("m1");
        sync.enable_sync(SyncAccount {
            provider: SyncAuthProvider::GitHub,
            account_name: "user".into(),
            session_id: "s".into(),
        });
        sync.set_enabled_resources(vec![SyncResource::Settings]);

        let mut provider = TestProvider::new();
        let exported = sync
            .export(
                &[SyncResource::Settings, SyncResource::Keybindings],
                &provider,
            )
            .unwrap();

        assert_eq!(exported.resources.len(), 1);
        assert_eq!(exported.resources[0].resource, SyncResource::Settings);

        let remote = SyncData {
            version: 1,
            machine_id: "other".into(),
            resources: vec![
                SyncResourceData {
                    resource: SyncResource::Settings,
                    content: json!({"theme": "dark"}),
                },
                SyncResourceData {
                    resource: SyncResource::Keybindings,
                    content: json!([{"key": "ctrl+s"}]),
                },
            ],
        };

        let result = sync.import(&remote, &mut provider).unwrap();
        assert_eq!(result.resources_synced.len(), 1);
        assert!(provider.data.get(&SyncResource::Keybindings).is_none());
    }

    #[test]
    fn has_local_changes_detection() {
        assert!(has_local_changes(&json!({"a": 1}), &json!({"a": 2})));
        assert!(!has_local_changes(
            &json!({"a": 1}),
            &json!({"a": 1, "b": 2})
        ));
        assert!(has_local_changes(
            &json!({"a": 1, "b": 2}),
            &json!({"a": 1})
        ));
        assert!(!has_local_changes(&json!(42), &json!(42)));
        assert!(has_local_changes(&json!(42), &json!(43)));
    }

    #[test]
    fn auth_provider_display() {
        assert_eq!(format!("{}", SyncAuthProvider::GitHub), "GitHub");
        assert_eq!(format!("{}", SyncAuthProvider::Microsoft), "Microsoft");
    }
}
