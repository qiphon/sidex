//! Editor profiles — named configuration bundles that can be switched
//! between, matching VS Code's Profiles feature.
//!
//! Each profile contains its own settings, keybindings, extensions list,
//! snippets, tasks, and global state. A default profile always exists and
//! cannot be deleted.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Profile flags (bitfield) ────────────────────────────────────────────

/// Which aspects of a profile inherit from the default profile.
///
/// When a flag is set, the corresponding section is taken from the
/// profile itself; when cleared, it falls through to the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileFlags(u8);

impl ProfileFlags {
    pub const SETTINGS: u8 = 0b0000_0001;
    pub const KEYBINDINGS: u8 = 0b0000_0010;
    pub const SNIPPETS: u8 = 0b0000_0100;
    pub const TASKS: u8 = 0b0000_1000;
    pub const EXTENSIONS: u8 = 0b0001_0000;
    pub const UI_STATE: u8 = 0b0010_0000;
    pub const ALL: u8 = 0b0011_1111;

    #[must_use]
    pub fn all() -> Self {
        Self(Self::ALL)
    }

    #[must_use]
    pub fn none() -> Self {
        Self(0)
    }

    #[must_use]
    pub fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn set(&mut self, flag: u8) {
        self.0 |= flag;
    }

    pub fn clear(&mut self, flag: u8) {
        self.0 &= !flag;
    }
}

impl Default for ProfileFlags {
    fn default() -> Self {
        Self::all()
    }
}

// ── Profile extension entry ─────────────────────────────────────────────

/// An extension associated with a profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileExtension {
    pub id: String,
    pub enabled: bool,
}

// ── Profile ID ──────────────────────────────────────────────────────────

/// Unique identifier for a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(u64);

impl ProfileId {
    /// The built-in default profile id.
    pub const DEFAULT: Self = Self(0);
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "profile-{}", self.0)
    }
}

// ── Profile ─────────────────────────────────────────────────────────────

/// A named configuration profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub settings: Value,
    pub keybindings: Value,
    #[serde(default)]
    pub extensions: Vec<ProfileExtension>,
    pub snippets: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_state: Option<Value>,
    pub is_default: bool,
    #[serde(default)]
    pub use_default_flags: ProfileFlags,
    #[serde(default)]
    pub is_temporary: bool,
}

impl Profile {
    fn new_default() -> Self {
        Self {
            id: ProfileId::DEFAULT,
            name: "Default".to_string(),
            icon: None,
            settings: Value::Object(serde_json::Map::new()),
            keybindings: Value::Array(Vec::new()),
            extensions: Vec::new(),
            snippets: Value::Object(serde_json::Map::new()),
            tasks: None,
            global_state: None,
            is_default: true,
            use_default_flags: ProfileFlags::all(),
            is_temporary: false,
        }
    }

    /// The list of enabled extension IDs.
    #[must_use]
    pub fn enabled_extensions(&self) -> Vec<&str> {
        self.extensions
            .iter()
            .filter(|e| e.enabled)
            .map(|e| e.id.as_str())
            .collect()
    }

    /// The list of disabled extension IDs.
    #[must_use]
    pub fn disabled_extensions(&self) -> Vec<&str> {
        self.extensions
            .iter()
            .filter(|e| !e.enabled)
            .map(|e| e.id.as_str())
            .collect()
    }
}

// ── Portable export format ──────────────────────────────────────────────

/// Format used when sharing a profile as JSON (export/import).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedProfile {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub settings: Value,
    pub keybindings: Value,
    pub extensions: Vec<ProfileExtension>,
    pub snippets: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Value>,
}

impl From<&Profile> for ExportedProfile {
    fn from(p: &Profile) -> Self {
        Self {
            name: p.name.clone(),
            icon: p.icon.clone(),
            settings: p.settings.clone(),
            keybindings: p.keybindings.clone(),
            extensions: p.extensions.clone(),
            snippets: p.snippets.clone(),
            tasks: p.tasks.clone(),
        }
    }
}

// ── Profile Manager ─────────────────────────────────────────────────────

/// Manages the collection of editor profiles.
pub struct ProfileManager {
    profiles: Vec<Profile>,
    active_id: ProfileId,
    next_id: u64,
}

impl ProfileManager {
    /// Create a manager with just the default profile.
    #[must_use]
    pub fn new() -> Self {
        Self {
            profiles: vec![Profile::new_default()],
            active_id: ProfileId::DEFAULT,
            next_id: 1,
        }
    }

    // ── Create / delete ─────────────────────────────────────────────────

    /// Create a new named profile and return its id.
    pub fn create_profile(&mut self, name: &str) -> Result<ProfileId, String> {
        if name.is_empty() {
            return Err("profile name cannot be empty".to_string());
        }
        if self.profiles.iter().any(|p| p.name == name) {
            return Err(format!("profile '{name}' already exists"));
        }

        let id = ProfileId(self.next_id);
        self.next_id += 1;

        self.profiles.push(Profile {
            id,
            name: name.to_string(),
            icon: None,
            settings: Value::Object(serde_json::Map::new()),
            keybindings: Value::Array(Vec::new()),
            extensions: Vec::new(),
            snippets: Value::Object(serde_json::Map::new()),
            tasks: None,
            global_state: None,
            is_default: false,
            use_default_flags: ProfileFlags::all(),
            is_temporary: false,
        });

        Ok(id)
    }

    /// Create a temporary profile (reverted on close).
    pub fn create_temporary_profile(&mut self, name: &str) -> Result<ProfileId, String> {
        let id = self.create_profile(name)?;
        if let Some(p) = self.get_profile_mut(id) {
            p.is_temporary = true;
        }
        Ok(id)
    }

    /// Delete a profile (cannot delete the default profile or the active profile).
    pub fn delete_profile(&mut self, id: ProfileId) -> Result<(), String> {
        if id == ProfileId::DEFAULT {
            return Err("cannot delete the default profile".to_string());
        }
        if id == self.active_id {
            return Err("cannot delete the active profile — switch first".to_string());
        }
        let before = self.profiles.len();
        self.profiles.retain(|p| p.id != id);
        if self.profiles.len() == before {
            return Err(format!("profile {id} not found"));
        }
        Ok(())
    }

    /// Remove all temporary profiles (called on application close).
    pub fn cleanup_temporary_profiles(&mut self) {
        if self.profiles.iter().any(|p| p.id == self.active_id && p.is_temporary) {
            self.active_id = ProfileId::DEFAULT;
        }
        self.profiles.retain(|p| !p.is_temporary);
    }

    // ── Switch ──────────────────────────────────────────────────────────

    /// Switch to a different profile.
    pub fn switch_profile(&mut self, id: ProfileId) -> Result<(), String> {
        if !self.profiles.iter().any(|p| p.id == id) {
            return Err(format!("profile {id} not found"));
        }
        self.active_id = id;
        Ok(())
    }

    // ── Queries ─────────────────────────────────────────────────────────

    /// Get the currently active profile.
    #[must_use]
    pub fn active_profile(&self) -> &Profile {
        self.profiles
            .iter()
            .find(|p| p.id == self.active_id)
            .expect("active profile must exist")
    }

    /// Get a mutable reference to the active profile.
    pub fn active_profile_mut(&mut self) -> &mut Profile {
        let id = self.active_id;
        self.profiles
            .iter_mut()
            .find(|p| p.id == id)
            .expect("active profile must exist")
    }

    /// The active profile id.
    #[must_use]
    pub fn active_id(&self) -> ProfileId {
        self.active_id
    }

    /// List all profiles.
    #[must_use]
    pub fn list_profiles(&self) -> Vec<&Profile> {
        self.profiles.iter().collect()
    }

    /// Get a profile by id.
    #[must_use]
    pub fn get_profile(&self, id: ProfileId) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Get a mutable profile by id.
    pub fn get_profile_mut(&mut self, id: ProfileId) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    /// Find a profile by name.
    #[must_use]
    pub fn find_by_name(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    // ── Duplicate ───────────────────────────────────────────────────────

    /// Duplicate an existing profile under a new name.
    pub fn duplicate_profile(
        &mut self,
        source_id: ProfileId,
        new_name: &str,
    ) -> Result<ProfileId, String> {
        let source = self
            .profiles
            .iter()
            .find(|p| p.id == source_id)
            .ok_or_else(|| format!("profile {source_id} not found"))?
            .clone();

        if self.profiles.iter().any(|p| p.name == new_name) {
            return Err(format!("profile '{new_name}' already exists"));
        }

        let id = ProfileId(self.next_id);
        self.next_id += 1;

        self.profiles.push(Profile {
            id,
            name: new_name.to_string(),
            icon: source.icon,
            settings: source.settings,
            keybindings: source.keybindings,
            extensions: source.extensions,
            snippets: source.snippets,
            tasks: source.tasks,
            global_state: source.global_state,
            is_default: false,
            use_default_flags: source.use_default_flags,
            is_temporary: false,
        });

        Ok(id)
    }

    // ── Rename ──────────────────────────────────────────────────────────

    /// Rename a profile.
    pub fn rename_profile(&mut self, id: ProfileId, new_name: &str) -> Result<(), String> {
        if id == ProfileId::DEFAULT {
            return Err("cannot rename the default profile".into());
        }
        if new_name.is_empty() {
            return Err("profile name cannot be empty".into());
        }
        if self.profiles.iter().any(|p| p.name == new_name && p.id != id) {
            return Err(format!("profile '{new_name}' already exists"));
        }
        let profile = self
            .get_profile_mut(id)
            .ok_or_else(|| format!("profile {id} not found"))?;
        profile.name = new_name.to_string();
        Ok(())
    }

    // ── Export / Import ─────────────────────────────────────────────────

    /// Export a single profile as a JSON string.
    pub fn export_profile(&self, id: ProfileId) -> Result<String, String> {
        let profile = self
            .get_profile(id)
            .ok_or_else(|| format!("profile {id} not found"))?;
        let exported = ExportedProfile::from(profile);
        serde_json::to_string_pretty(&exported).map_err(|e| format!("serialize: {e}"))
    }

    /// Import a profile from a JSON string.
    pub fn import_profile(&mut self, json: &str) -> Result<ProfileId, String> {
        let exported: ExportedProfile =
            serde_json::from_str(json).map_err(|e| format!("parse profile: {e}"))?;

        let name = if self.profiles.iter().any(|p| p.name == exported.name) {
            format!("{} (imported)", exported.name)
        } else {
            exported.name
        };

        let id = ProfileId(self.next_id);
        self.next_id += 1;

        self.profiles.push(Profile {
            id,
            name,
            icon: exported.icon,
            settings: exported.settings,
            keybindings: exported.keybindings,
            extensions: exported.extensions,
            snippets: exported.snippets,
            tasks: exported.tasks,
            global_state: None,
            is_default: false,
            use_default_flags: ProfileFlags::all(),
            is_temporary: false,
        });

        Ok(id)
    }

    /// Export all profiles as JSON.
    pub fn export_all(&self) -> Result<String, String> {
        let exported: Vec<ExportedProfile> = self
            .profiles
            .iter()
            .filter(|p| !p.is_default)
            .map(ExportedProfile::from)
            .collect();
        serde_json::to_string_pretty(&exported).map_err(|e| format!("serialize: {e}"))
    }

    /// Import profiles from JSON, merging with existing (skips duplicates by name).
    pub fn import(&mut self, json: &str) -> Result<usize, String> {
        let imported: Vec<ExportedProfile> =
            serde_json::from_str(json).map_err(|e| format!("parse profiles: {e}"))?;
        let mut count = 0;
        for ep in imported {
            if self.profiles.iter().any(|existing| existing.name == ep.name) {
                continue;
            }

            let id = ProfileId(self.next_id);
            self.next_id += 1;

            self.profiles.push(Profile {
                id,
                name: ep.name,
                icon: ep.icon,
                settings: ep.settings,
                keybindings: ep.keybindings,
                extensions: ep.extensions,
                snippets: ep.snippets,
                tasks: ep.tasks,
                global_state: None,
                is_default: false,
                use_default_flags: ProfileFlags::all(),
                is_temporary: false,
            });
            count += 1;
        }
        Ok(count)
    }

    // ── Persistence ─────────────────────────────────────────────────────

    /// Save all profiles to a file.
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.profiles)
            .map_err(|e| format!("serialize profiles: {e}"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
        }
        std::fs::write(path, json).map_err(|e| format!("write profiles: {e}"))
    }

    /// Load profiles from a file, keeping the default profile.
    pub fn load_from_file(&mut self, path: &Path) -> Result<(), String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("read profiles: {e}"))?;
        let loaded: Vec<Profile> =
            serde_json::from_str(&content).map_err(|e| format!("parse profiles: {e}"))?;

        self.profiles.retain(|p| p.is_default);
        let mut max_id = 0u64;
        for p in loaded {
            if p.is_default {
                continue;
            }
            if p.id.0 > max_id {
                max_id = p.id.0;
            }
            self.profiles.push(p);
        }
        self.next_id = max_id + 1;

        if !self.profiles.iter().any(|p| p.id == self.active_id) {
            self.active_id = ProfileId::DEFAULT;
        }
        Ok(())
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_exists() {
        let mgr = ProfileManager::new();
        assert_eq!(mgr.list_profiles().len(), 1);
        assert!(mgr.active_profile().is_default);
        assert_eq!(mgr.active_profile().name, "Default");
    }

    #[test]
    fn create_and_switch() {
        let mut mgr = ProfileManager::new();
        let id = mgr.create_profile("Work").unwrap();
        assert_eq!(mgr.list_profiles().len(), 2);

        mgr.switch_profile(id).unwrap();
        assert_eq!(mgr.active_profile().name, "Work");
    }

    #[test]
    fn cannot_delete_default() {
        let mut mgr = ProfileManager::new();
        assert!(mgr.delete_profile(ProfileId::DEFAULT).is_err());
    }

    #[test]
    fn cannot_delete_active() {
        let mut mgr = ProfileManager::new();
        let id = mgr.create_profile("Test").unwrap();
        mgr.switch_profile(id).unwrap();
        assert!(mgr.delete_profile(id).is_err());
    }

    #[test]
    fn delete_profile() {
        let mut mgr = ProfileManager::new();
        let id = mgr.create_profile("Temp").unwrap();
        assert_eq!(mgr.list_profiles().len(), 2);

        mgr.delete_profile(id).unwrap();
        assert_eq!(mgr.list_profiles().len(), 1);
    }

    #[test]
    fn duplicate_profile() {
        let mut mgr = ProfileManager::new();
        let id = mgr.create_profile("Source").unwrap();
        if let Some(p) = mgr.get_profile_mut(id) {
            p.extensions = vec![
                ProfileExtension { id: "ext-a".into(), enabled: true },
                ProfileExtension { id: "ext-b".into(), enabled: true },
            ];
        }

        let dup_id = mgr.duplicate_profile(id, "Copy").unwrap();
        let dup = mgr.get_profile(dup_id).unwrap();
        assert_eq!(dup.name, "Copy");
        assert_eq!(dup.extensions.len(), 2);
    }

    #[test]
    fn no_duplicate_names() {
        let mut mgr = ProfileManager::new();
        mgr.create_profile("A").unwrap();
        assert!(mgr.create_profile("A").is_err());
    }

    #[test]
    fn switch_nonexistent() {
        let mut mgr = ProfileManager::new();
        assert!(mgr.switch_profile(ProfileId(999)).is_err());
    }

    #[test]
    fn export_import_single() {
        let mut mgr = ProfileManager::new();
        let id = mgr.create_profile("Exported").unwrap();
        let json = mgr.export_profile(id).unwrap();

        let mut mgr2 = ProfileManager::new();
        let imported_id = mgr2.import_profile(&json).unwrap();
        assert_eq!(mgr2.get_profile(imported_id).unwrap().name, "Exported");
    }

    #[test]
    fn export_import_all() {
        let mut mgr = ProfileManager::new();
        mgr.create_profile("A").unwrap();
        mgr.create_profile("B").unwrap();
        let json = mgr.export_all().unwrap();

        let mut mgr2 = ProfileManager::new();
        let count = mgr2.import(&json).unwrap();
        assert_eq!(count, 2);
        assert_eq!(mgr2.list_profiles().len(), 3);
    }

    #[test]
    fn import_deduplicates_name() {
        let mut mgr = ProfileManager::new();
        mgr.create_profile("Work").unwrap();
        let json = r#"{"name":"Work","settings":{},"keybindings":[],"extensions":[],"snippets":{}}"#;
        let id = mgr.import_profile(json).unwrap();
        assert_eq!(mgr.get_profile(id).unwrap().name, "Work (imported)");
    }

    #[test]
    fn active_profile_mut() {
        let mut mgr = ProfileManager::new();
        mgr.active_profile_mut().settings = serde_json::json!({"theme": "dark"});
        assert!(mgr.active_profile().settings.is_object());
    }

    #[test]
    fn rename_profile() {
        let mut mgr = ProfileManager::new();
        let id = mgr.create_profile("Old").unwrap();
        mgr.rename_profile(id, "New").unwrap();
        assert_eq!(mgr.get_profile(id).unwrap().name, "New");
    }

    #[test]
    fn cannot_rename_default() {
        let mut mgr = ProfileManager::new();
        assert!(mgr.rename_profile(ProfileId::DEFAULT, "X").is_err());
    }

    #[test]
    fn temporary_profile() {
        let mut mgr = ProfileManager::new();
        let id = mgr.create_temporary_profile("Temp").unwrap();
        assert!(mgr.get_profile(id).unwrap().is_temporary);

        mgr.switch_profile(id).unwrap();
        mgr.cleanup_temporary_profiles();
        assert_eq!(mgr.active_id(), ProfileId::DEFAULT);
        assert_eq!(mgr.list_profiles().len(), 1);
    }

    #[test]
    fn find_by_name() {
        let mut mgr = ProfileManager::new();
        mgr.create_profile("Work").unwrap();
        assert!(mgr.find_by_name("Work").is_some());
        assert!(mgr.find_by_name("Nope").is_none());
    }

    #[test]
    fn profile_flags() {
        let mut flags = ProfileFlags::all();
        assert!(flags.contains(ProfileFlags::SETTINGS));
        assert!(flags.contains(ProfileFlags::EXTENSIONS));

        flags.clear(ProfileFlags::SETTINGS);
        assert!(!flags.contains(ProfileFlags::SETTINGS));
        assert!(flags.contains(ProfileFlags::EXTENSIONS));

        flags.set(ProfileFlags::SETTINGS);
        assert!(flags.contains(ProfileFlags::SETTINGS));
    }

    #[test]
    fn profile_extensions_query() {
        let p = Profile {
            id: ProfileId(1),
            name: "Test".into(),
            icon: None,
            settings: Value::Null,
            keybindings: Value::Null,
            extensions: vec![
                ProfileExtension { id: "a".into(), enabled: true },
                ProfileExtension { id: "b".into(), enabled: false },
                ProfileExtension { id: "c".into(), enabled: true },
            ],
            snippets: Value::Null,
            tasks: None,
            global_state: None,
            is_default: false,
            use_default_flags: ProfileFlags::all(),
            is_temporary: false,
        };
        assert_eq!(p.enabled_extensions(), vec!["a", "c"]);
        assert_eq!(p.disabled_extensions(), vec!["b"]);
    }

    #[test]
    fn save_and_load() {
        let tmp = std::env::temp_dir().join("sidex-profiles-test.json");

        let mut mgr = ProfileManager::new();
        mgr.create_profile("Saved").unwrap();
        mgr.save_to_file(&tmp).unwrap();

        let mut mgr2 = ProfileManager::new();
        mgr2.load_from_file(&tmp).unwrap();
        assert_eq!(mgr2.list_profiles().len(), 2);
        assert!(mgr2.find_by_name("Saved").is_some());

        let _ = std::fs::remove_file(&tmp);
    }
}
