//! Workspace trust — tracks whether a workspace is trusted and restricts
//! capabilities in untrusted workspaces.
//!
//! When a workspace is untrusted, extensions and tasks should run in a
//! restricted sandbox (the trust decision is persisted to disk).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const TRUST_FILE_NAME: &str = ".sidex-trusted-workspaces.json";

// ── Trust state ─────────────────────────────────────────────────────────

/// High-level trust determination for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TrustState {
    Trusted,
    Untrusted,
    /// Not yet decided — first open, prompt required.
    #[default]
    Unknown,
}

// ── Restricted features ─────────────────────────────────────────────────

/// A single capability that may be restricted in an untrusted workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestrictedFeature {
    pub name: String,
    pub description: String,
    /// Whether this feature becomes available when the workspace is trusted.
    pub enabled_when_trusted: bool,
}

/// The set of built-in features that are restricted when trust is absent.
pub fn builtin_restricted_features() -> Vec<RestrictedFeature> {
    vec![
        RestrictedFeature {
            name: "tasks".into(),
            description: "Run build/test tasks from task configuration files".into(),
            enabled_when_trusted: true,
        },
        RestrictedFeature {
            name: "debug".into(),
            description: "Launch debug sessions".into(),
            enabled_when_trusted: true,
        },
        RestrictedFeature {
            name: "terminal.shellCommands".into(),
            description: "Execute shell commands in the integrated terminal".into(),
            enabled_when_trusted: true,
        },
        RestrictedFeature {
            name: "extensions.untrusted".into(),
            description: "Run extensions that haven't declared untrustedWorkspaces support".into(),
            enabled_when_trusted: true,
        },
        RestrictedFeature {
            name: "settings.workspaceWrite".into(),
            description: "Write to workspace-level settings".into(),
            enabled_when_trusted: true,
        },
    ]
}

/// Returns the restricted features filtered by trust state.
pub fn get_restricted_features(trust_state: TrustState) -> Vec<RestrictedFeature> {
    match trust_state {
        TrustState::Trusted => Vec::new(),
        TrustState::Untrusted | TrustState::Unknown => builtin_restricted_features(),
    }
}

// ── Trust prompt ────────────────────────────────────────────────────────

/// User-facing prompt options when a workspace trust decision is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustPromptResponse {
    TrustThisFolder,
    TrustParentFolder,
    DontTrust,
    Later,
}

/// Data required to display the trust prompt to the user.
#[derive(Debug, Clone)]
pub struct TrustPrompt {
    pub workspace_path: PathBuf,
    pub features_restricted: Vec<RestrictedFeature>,
}

impl TrustPrompt {
    /// Build a prompt for the given workspace.
    #[must_use]
    pub fn for_workspace(workspace_path: &Path) -> Self {
        Self {
            workspace_path: workspace_path.to_path_buf(),
            features_restricted: builtin_restricted_features(),
        }
    }
}

// ── Banner state ────────────────────────────────────────────────────────

/// Describes a restricted-mode banner that should be shown in the UI.
#[derive(Debug, Clone)]
pub struct RestrictedModeBanner {
    pub workspace_path: PathBuf,
    pub message: String,
    pub restricted_count: usize,
}

impl RestrictedModeBanner {
    /// Create a banner for an untrusted workspace.
    #[must_use]
    pub fn new(workspace_path: &Path) -> Self {
        let features = builtin_restricted_features();
        let count = features.len();
        Self {
            workspace_path: workspace_path.to_path_buf(),
            message: format!("This workspace is not trusted. {count} feature(s) are restricted."),
            restricted_count: count,
        }
    }
}

// ── Persisted trust data ────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TrustData {
    trusted: HashSet<String>,
    /// Extensions that declare they are safe in untrusted workspaces.
    #[serde(default)]
    trusted_extensions: HashSet<String>,
}

// ── Extension trust capability ──────────────────────────────────────────

/// Whether an extension supports running in untrusted workspaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExtensionTrustCapability {
    /// Extension is fully supported in untrusted workspaces.
    Supported,
    /// Extension has limited functionality in untrusted workspaces.
    Limited,
    /// Extension should be disabled in untrusted workspaces (default).
    #[default]
    Unsupported,
}

// ── Restricted mode capabilities ────────────────────────────────────────

/// Describes what is allowed in the current trust state.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct RestrictedMode {
    pub workspace: PathBuf,
    pub trust_state: TrustState,
    pub can_run_tasks: bool,
    pub can_run_debug: bool,
    pub can_use_terminal: bool,
    pub extensions_restricted: bool,
    pub can_write_settings: bool,
}

impl RestrictedMode {
    fn full_access() -> Self {
        Self {
            workspace: PathBuf::new(),
            trust_state: TrustState::Trusted,
            can_run_tasks: true,
            can_run_debug: true,
            can_use_terminal: true,
            extensions_restricted: false,
            can_write_settings: true,
        }
    }

    fn restricted(workspace: &Path) -> Self {
        Self {
            workspace: workspace.to_path_buf(),
            trust_state: TrustState::Untrusted,
            can_run_tasks: false,
            can_run_debug: false,
            can_use_terminal: false,
            extensions_restricted: true,
            can_write_settings: false,
        }
    }
}

// ── WorkspaceTrust manager ──────────────────────────────────────────────

/// Tracks which workspaces are trusted.
pub struct WorkspaceTrust {
    data: TrustData,
    config_path: PathBuf,
}

impl WorkspaceTrust {
    /// Create a new trust manager using the platform config directory.
    #[must_use]
    pub fn new() -> Self {
        let config_path = config_dir().join(TRUST_FILE_NAME);
        let data = load_trust_data(&config_path).unwrap_or_default();
        Self { data, config_path }
    }

    /// Create a trust manager rooted at a custom config directory (for testing).
    #[must_use]
    pub fn with_config_dir(dir: &Path) -> Self {
        let config_path = dir.join(TRUST_FILE_NAME);
        let data = load_trust_data(&config_path).unwrap_or_default();
        Self { data, config_path }
    }

    // ── Core trust queries ──────────────────────────────────────────────

    /// Check whether the given workspace is trusted.
    #[must_use]
    pub fn is_trusted(&self, workspace_root: &Path) -> bool {
        let key = canonical_key(workspace_root);
        self.data.trusted.contains(&key)
    }

    /// Get the trust state for a workspace.
    #[must_use]
    pub fn trust_state(&self, workspace_root: &Path) -> TrustState {
        if self.is_trusted(workspace_root) {
            TrustState::Trusted
        } else if self.was_ever_prompted(workspace_root) {
            TrustState::Untrusted
        } else {
            TrustState::Unknown
        }
    }

    /// Whether the user has already been prompted about this workspace
    /// (they explicitly chose "Don't trust").
    #[must_use]
    fn was_ever_prompted(&self, workspace_root: &Path) -> bool {
        let key = format!("prompted:{}", canonical_key(workspace_root));
        self.data.trusted.contains(&key)
    }

    fn mark_prompted(&mut self, workspace_root: &Path) {
        let key = format!("prompted:{}", canonical_key(workspace_root));
        self.data.trusted.insert(key);
    }

    // ── Trust mutations ─────────────────────────────────────────────────

    /// Mark a workspace as trusted (equivalent to "Trust this folder").
    pub fn grant_trust(&mut self, workspace_root: &Path) -> Result<(), String> {
        let key = canonical_key(workspace_root);
        self.data.trusted.insert(key);
        self.mark_prompted(workspace_root);
        self.persist()
    }

    /// Mark a workspace *and* its parent as trusted ("Trust parent folder").
    pub fn grant_trust_parent(&mut self, workspace_root: &Path) -> Result<(), String> {
        self.grant_trust(workspace_root)?;
        if let Some(parent) = workspace_root.parent() {
            self.grant_trust(parent)?;
        }
        Ok(())
    }

    /// Remove trust for a workspace ("Don't trust" / revoke).
    pub fn revoke_trust(&mut self, workspace_root: &Path) -> Result<(), String> {
        let key = canonical_key(workspace_root);
        self.data.trusted.remove(&key);
        self.mark_prompted(workspace_root);
        self.persist()
    }

    /// Respond to the initial trust prompt.
    pub fn respond_to_prompt(
        &mut self,
        workspace_root: &Path,
        response: TrustPromptResponse,
    ) -> Result<(), String> {
        match response {
            TrustPromptResponse::TrustThisFolder => self.grant_trust(workspace_root),
            TrustPromptResponse::TrustParentFolder => self.grant_trust_parent(workspace_root),
            TrustPromptResponse::DontTrust => self.revoke_trust(workspace_root),
            TrustPromptResponse::Later => Ok(()),
        }
    }

    // ── Legacy aliases for backward compat ──────────────────────────────

    /// Alias for [`grant_trust`](Self::grant_trust).
    pub fn trust(&mut self, workspace_root: &Path) {
        let _ = self.grant_trust(workspace_root);
    }

    /// Alias for [`revoke_trust`](Self::revoke_trust).
    pub fn untrust(&mut self, workspace_root: &Path) {
        let _ = self.revoke_trust(workspace_root);
    }

    // ── Trusted folder listing ──────────────────────────────────────────

    /// List all trusted workspace paths.
    #[must_use]
    pub fn trusted_workspaces(&self) -> Vec<PathBuf> {
        self.data
            .trusted
            .iter()
            .filter(|k| !k.starts_with("prompted:"))
            .map(PathBuf::from)
            .collect()
    }

    // ── Extension trust ─────────────────────────────────────────────────

    /// Register an extension as trusted for untrusted workspaces.
    pub fn mark_extension_trusted(&mut self, extension_id: &str) {
        self.data
            .trusted_extensions
            .insert(extension_id.to_string());
        let _ = self.persist();
    }

    /// Check if an extension has declared `untrustedWorkspaces` support.
    #[must_use]
    pub fn is_extension_trusted(&self, extension_id: &str) -> bool {
        self.data.trusted_extensions.contains(extension_id)
    }

    /// List extensions that are restricted in untrusted workspaces
    /// (all known minus the ones that declared support).
    #[must_use]
    pub fn untrusted_extensions(&self, all_extensions: &[String]) -> Vec<String> {
        all_extensions
            .iter()
            .filter(|id| !self.data.trusted_extensions.contains(id.as_str()))
            .cloned()
            .collect()
    }

    // ── Restricted capabilities ─────────────────────────────────────────

    /// Returns the set of restricted capabilities for a workspace.
    #[must_use]
    pub fn restricted_capabilities(workspace_root: &Path, is_trusted: bool) -> RestrictedMode {
        if is_trusted {
            RestrictedMode::full_access()
        } else {
            RestrictedMode::restricted(workspace_root)
        }
    }

    /// Build a `RestrictedMode` from the current trust data.
    #[must_use]
    pub fn capabilities_for(&self, workspace_root: &Path) -> RestrictedMode {
        Self::restricted_capabilities(workspace_root, self.is_trusted(workspace_root))
    }

    /// Whether a restricted-mode banner should be shown for this workspace.
    #[must_use]
    pub fn should_show_banner(&self, workspace_root: &Path) -> Option<RestrictedModeBanner> {
        if self.is_trusted(workspace_root) {
            None
        } else {
            Some(RestrictedModeBanner::new(workspace_root))
        }
    }

    // ── Persistence ─────────────────────────────────────────────────────

    fn persist(&self) -> Result<(), String> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(&self.data)
            .map_err(|e| format!("serialize trust data: {e}"))?;
        std::fs::write(&self.config_path, json).map_err(|e| format!("write trust file: {e}"))
    }
}

impl Default for WorkspaceTrust {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn canonical_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn config_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config").join("sidex")
    } else if let Ok(home) = std::env::var("USERPROFILE") {
        PathBuf::from(home).join(".config").join("sidex")
    } else {
        PathBuf::from(".config").join("sidex")
    }
}

fn load_trust_data(path: &Path) -> Option<TrustData> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_trust_dir(name: &str) -> PathBuf {
        let tmp = std::env::temp_dir().join(format!("sidex-trust-{name}"));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        tmp
    }

    #[test]
    fn trust_and_untrust_roundtrip() {
        let tmp = temp_trust_dir("roundtrip");
        let ws = tmp.join("my-project");
        fs::create_dir_all(&ws).unwrap();

        let mut trust = WorkspaceTrust::with_config_dir(&tmp);
        assert!(!trust.is_trusted(&ws));

        trust.trust(&ws);
        assert!(trust.is_trusted(&ws));

        let workspaces = trust.trusted_workspaces();
        assert!(!workspaces.is_empty());

        trust.untrust(&ws);
        assert!(!trust.is_trusted(&ws));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn persistence_across_instances() {
        let tmp = temp_trust_dir("persist");
        let ws = tmp.join("project");
        fs::create_dir_all(&ws).unwrap();

        {
            let mut trust = WorkspaceTrust::with_config_dir(&tmp);
            trust.trust(&ws);
        }

        {
            let trust = WorkspaceTrust::with_config_dir(&tmp);
            assert!(trust.is_trusted(&ws));
        }

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn restricted_mode_untrusted() {
        let ws = PathBuf::from("/some/project");
        let mode = WorkspaceTrust::restricted_capabilities(&ws, false);
        assert!(!mode.can_run_tasks);
        assert!(!mode.can_run_debug);
        assert!(!mode.can_use_terminal);
        assert!(mode.extensions_restricted);
        assert_eq!(mode.trust_state, TrustState::Untrusted);
    }

    #[test]
    fn restricted_mode_trusted() {
        let ws = PathBuf::from("/some/project");
        let mode = WorkspaceTrust::restricted_capabilities(&ws, true);
        assert!(mode.can_run_tasks);
        assert!(mode.can_run_debug);
        assert!(!mode.extensions_restricted);
        assert_eq!(mode.trust_state, TrustState::Trusted);
    }

    #[test]
    fn trust_state_transitions() {
        let tmp = temp_trust_dir("states");
        let ws = tmp.join("proj");
        fs::create_dir_all(&ws).unwrap();

        let mut trust = WorkspaceTrust::with_config_dir(&tmp);
        assert_eq!(trust.trust_state(&ws), TrustState::Unknown);

        trust.grant_trust(&ws).unwrap();
        assert_eq!(trust.trust_state(&ws), TrustState::Trusted);

        trust.revoke_trust(&ws).unwrap();
        assert_eq!(trust.trust_state(&ws), TrustState::Untrusted);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn grant_trust_parent() {
        let tmp = temp_trust_dir("parent");
        let parent = tmp.join("repos");
        let ws = parent.join("my-proj");
        fs::create_dir_all(&ws).unwrap();

        let mut trust = WorkspaceTrust::with_config_dir(&tmp);
        trust.grant_trust_parent(&ws).unwrap();
        assert!(trust.is_trusted(&ws));
        assert!(trust.is_trusted(&parent));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn prompt_response_trust() {
        let tmp = temp_trust_dir("prompt-trust");
        let ws = tmp.join("proj");
        fs::create_dir_all(&ws).unwrap();

        let mut trust = WorkspaceTrust::with_config_dir(&tmp);
        trust
            .respond_to_prompt(&ws, TrustPromptResponse::TrustThisFolder)
            .unwrap();
        assert!(trust.is_trusted(&ws));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn prompt_response_dont_trust() {
        let tmp = temp_trust_dir("prompt-deny");
        let ws = tmp.join("proj");
        fs::create_dir_all(&ws).unwrap();

        let mut trust = WorkspaceTrust::with_config_dir(&tmp);
        trust
            .respond_to_prompt(&ws, TrustPromptResponse::DontTrust)
            .unwrap();
        assert!(!trust.is_trusted(&ws));
        assert_eq!(trust.trust_state(&ws), TrustState::Untrusted);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn extension_trust() {
        let tmp = temp_trust_dir("ext-trust");
        let mut trust = WorkspaceTrust::with_config_dir(&tmp);

        let all = vec![
            "ext-a".to_string(),
            "ext-b".to_string(),
            "ext-c".to_string(),
        ];
        assert_eq!(trust.untrusted_extensions(&all).len(), 3);

        trust.mark_extension_trusted("ext-b");
        assert!(trust.is_extension_trusted("ext-b"));
        assert_eq!(trust.untrusted_extensions(&all).len(), 2);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn capabilities_for_workspace() {
        let tmp = temp_trust_dir("caps");
        let ws = tmp.join("project");
        fs::create_dir_all(&ws).unwrap();

        let mut trust = WorkspaceTrust::with_config_dir(&tmp);
        let caps = trust.capabilities_for(&ws);
        assert!(!caps.can_run_tasks);

        trust.grant_trust(&ws).unwrap();
        let caps = trust.capabilities_for(&ws);
        assert!(caps.can_run_tasks);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn banner_visibility() {
        let tmp = temp_trust_dir("banner");
        let ws = tmp.join("project");
        fs::create_dir_all(&ws).unwrap();

        let mut trust = WorkspaceTrust::with_config_dir(&tmp);
        assert!(trust.should_show_banner(&ws).is_some());

        trust.grant_trust(&ws).unwrap();
        assert!(trust.should_show_banner(&ws).is_none());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn restricted_features_by_state() {
        assert!(get_restricted_features(TrustState::Trusted).is_empty());
        assert!(!get_restricted_features(TrustState::Untrusted).is_empty());
        assert!(!get_restricted_features(TrustState::Unknown).is_empty());
    }

    #[test]
    fn trust_prompt_construction() {
        let prompt = TrustPrompt::for_workspace(Path::new("/tmp/ws"));
        assert_eq!(prompt.workspace_path, PathBuf::from("/tmp/ws"));
        assert!(!prompt.features_restricted.is_empty());
    }
}
