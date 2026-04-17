//! `vscode.scm` (Source Control) API compatibility shim.
//!
//! Provides the VS Code Source Control API for registering SCM providers,
//! managing resource groups and states, and status bar integration.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Opaque handles
// ---------------------------------------------------------------------------

/// Opaque handle to a source control instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceControlId(pub u32);

/// Opaque handle to a source control resource group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceGroupId(pub u32);

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// Decoration type for SCM resource states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScmResourceDecorationKind {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
    Ignored,
    Conflicting,
}

/// A single resource state within a resource group.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceControlResourceState {
    pub resource_uri: String,
    #[serde(default)]
    pub decoration: Option<ScmResourceDecorationKind>,
    #[serde(default)]
    pub tooltip: Option<String>,
    #[serde(default)]
    pub command: Option<ScmCommand>,
    #[serde(default)]
    pub context_value: Option<String>,
    #[serde(default)]
    pub faded: bool,
    #[serde(default)]
    pub strikethrough: bool,
}

/// A command reference within an SCM resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScmCommand {
    pub title: String,
    pub command: String,
    #[serde(default)]
    pub tooltip: Option<String>,
    #[serde(default)]
    pub arguments: Vec<Value>,
}

/// Options for creating a source control instance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceControlOptions {
    #[serde(default)]
    pub status_bar_commands: Vec<ScmCommand>,
}

/// SCM input box state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScmInputBox {
    pub value: String,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub visible: bool,
    #[serde(default)]
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

#[allow(dead_code)]
struct ResourceGroupEntry {
    id: ResourceGroupId,
    group_id: String,
    label: String,
    resources: Vec<SourceControlResourceState>,
    hide_when_empty: bool,
}

#[allow(dead_code)]
struct SourceControlEntry {
    id: SourceControlId,
    provider_id: String,
    label: String,
    root_uri: Option<String>,
    input_box: ScmInputBox,
    count: u32,
    commit_template: Option<String>,
    accept_input_command: Option<ScmCommand>,
    status_bar_commands: Vec<ScmCommand>,
    resource_groups: HashMap<ResourceGroupId, ResourceGroupEntry>,
    next_group: AtomicU32,
}

// ---------------------------------------------------------------------------
// ScmApi
// ---------------------------------------------------------------------------

/// Implements the `vscode.scm.*` API surface.
pub struct ScmApi {
    next_id: AtomicU32,
    source_controls: RwLock<HashMap<SourceControlId, SourceControlEntry>>,
}

impl ScmApi {
    /// Creates a new SCM API handler.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            source_controls: RwLock::new(HashMap::new()),
        }
    }

    /// Dispatches an SCM API action.
    #[allow(clippy::too_many_lines)]
    pub fn handle(&self, action: &str, params: &Value) -> Result<Value> {
        match action {
            "createSourceControl" => {
                let id = params.get("id").and_then(Value::as_str).unwrap_or("");
                let label = params.get("label").and_then(Value::as_str).unwrap_or("");
                let root_uri = params
                    .get("rootUri")
                    .and_then(Value::as_str)
                    .map(String::from);
                let sc_id = self.create_source_control(id, label, root_uri.as_deref());
                Ok(serde_json::to_value(sc_id)?)
            }
            "createResourceGroup" => {
                let sc_id = params
                    .get("sourceControlId")
                    .and_then(Value::as_u64)
                    .map(|n| SourceControlId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing sourceControlId"))?;
                let group_id = params.get("id").and_then(Value::as_str).unwrap_or("");
                let label = params.get("label").and_then(Value::as_str).unwrap_or("");
                let rg_id = self.create_resource_group(sc_id, group_id, label)?;
                Ok(serde_json::to_value(rg_id)?)
            }
            "updateResourceStates" => {
                let sc_id = params
                    .get("sourceControlId")
                    .and_then(Value::as_u64)
                    .map(|n| SourceControlId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing sourceControlId"))?;
                let rg_id = params
                    .get("resourceGroupId")
                    .and_then(Value::as_u64)
                    .map(|n| ResourceGroupId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing resourceGroupId"))?;
                let states: Vec<SourceControlResourceState> = params
                    .get("resourceStates")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                self.update_resource_states(sc_id, rg_id, states)?;
                Ok(Value::Bool(true))
            }
            "setInputBox" => {
                let sc_id = params
                    .get("sourceControlId")
                    .and_then(Value::as_u64)
                    .map(|n| SourceControlId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing sourceControlId"))?;
                let value = params.get("value").and_then(Value::as_str).unwrap_or("");
                let placeholder = params
                    .get("placeholder")
                    .and_then(Value::as_str)
                    .map(String::from);
                self.set_input_box(sc_id, value, placeholder.as_deref())?;
                Ok(Value::Bool(true))
            }
            "setCount" => {
                let sc_id = params
                    .get("sourceControlId")
                    .and_then(Value::as_u64)
                    .map(|n| SourceControlId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing sourceControlId"))?;
                let count = params
                    .get("count")
                    .and_then(Value::as_u64)
                    .and_then(|v| u32::try_from(v).ok())
                    .unwrap_or(0);
                self.set_count(sc_id, count)?;
                Ok(Value::Bool(true))
            }
            "setCommitTemplate" => {
                let sc_id = params
                    .get("sourceControlId")
                    .and_then(Value::as_u64)
                    .map(|n| SourceControlId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing sourceControlId"))?;
                let template = params.get("template").and_then(Value::as_str).unwrap_or("");
                self.set_commit_template(sc_id, template)?;
                Ok(Value::Bool(true))
            }
            "dispose" => {
                let sc_id = params
                    .get("sourceControlId")
                    .and_then(Value::as_u64)
                    .map(|n| SourceControlId(u32::try_from(n).unwrap_or(0)))
                    .ok_or_else(|| anyhow::anyhow!("missing sourceControlId"))?;
                self.dispose(sc_id);
                Ok(Value::Bool(true))
            }
            _ => bail!("unknown scm action: {action}"),
        }
    }

    // -----------------------------------------------------------------------
    // Source control management
    // -----------------------------------------------------------------------

    /// Creates a source control provider.
    pub fn create_source_control(
        &self,
        provider_id: &str,
        label: &str,
        root_uri: Option<&str>,
    ) -> SourceControlId {
        let raw = self.next_id.fetch_add(1, Ordering::Relaxed);
        let id = SourceControlId(raw);
        log::debug!("[ext] createSourceControl({provider_id}, {label}) -> {raw}");
        self.source_controls
            .write()
            .expect("scm lock poisoned")
            .insert(
                id,
                SourceControlEntry {
                    id,
                    provider_id: provider_id.to_owned(),
                    label: label.to_owned(),
                    root_uri: root_uri.map(String::from),
                    input_box: ScmInputBox {
                        visible: true,
                        enabled: true,
                        ..Default::default()
                    },
                    count: 0,
                    commit_template: None,
                    accept_input_command: None,
                    status_bar_commands: Vec::new(),
                    resource_groups: HashMap::new(),
                    next_group: AtomicU32::new(1),
                },
            );
        id
    }

    /// Creates a resource group within a source control.
    pub fn create_resource_group(
        &self,
        sc_id: SourceControlId,
        group_id: &str,
        label: &str,
    ) -> Result<ResourceGroupId> {
        let mut controls = self.source_controls.write().expect("scm lock poisoned");
        let sc = controls
            .get_mut(&sc_id)
            .ok_or_else(|| anyhow::anyhow!("source control {} not found", sc_id.0))?;

        let raw = sc.next_group.fetch_add(1, Ordering::Relaxed);
        let rg_id = ResourceGroupId(raw);
        log::debug!("[ext] createResourceGroup({group_id}, {label}) -> {raw}");
        sc.resource_groups.insert(
            rg_id,
            ResourceGroupEntry {
                id: rg_id,
                group_id: group_id.to_owned(),
                label: label.to_owned(),
                resources: Vec::new(),
                hide_when_empty: false,
            },
        );
        Ok(rg_id)
    }

    /// Updates the resource states within a resource group.
    pub fn update_resource_states(
        &self,
        sc_id: SourceControlId,
        rg_id: ResourceGroupId,
        states: Vec<SourceControlResourceState>,
    ) -> Result<()> {
        let mut controls = self.source_controls.write().expect("scm lock poisoned");
        let sc = controls
            .get_mut(&sc_id)
            .ok_or_else(|| anyhow::anyhow!("source control {} not found", sc_id.0))?;
        let group = sc
            .resource_groups
            .get_mut(&rg_id)
            .ok_or_else(|| anyhow::anyhow!("resource group {} not found", rg_id.0))?;
        group.resources = states;
        Ok(())
    }

    /// Sets the SCM input box value and placeholder.
    pub fn set_input_box(
        &self,
        sc_id: SourceControlId,
        value: &str,
        placeholder: Option<&str>,
    ) -> Result<()> {
        let mut controls = self.source_controls.write().expect("scm lock poisoned");
        let sc = controls
            .get_mut(&sc_id)
            .ok_or_else(|| anyhow::anyhow!("source control {} not found", sc_id.0))?;
        value.clone_into(&mut sc.input_box.value);
        if let Some(p) = placeholder {
            sc.input_box.placeholder = Some(p.to_owned());
        }
        Ok(())
    }

    /// Sets the badge count for a source control.
    pub fn set_count(&self, sc_id: SourceControlId, count: u32) -> Result<()> {
        let mut controls = self.source_controls.write().expect("scm lock poisoned");
        let sc = controls
            .get_mut(&sc_id)
            .ok_or_else(|| anyhow::anyhow!("source control {} not found", sc_id.0))?;
        sc.count = count;
        Ok(())
    }

    /// Sets the commit template for a source control.
    pub fn set_commit_template(&self, sc_id: SourceControlId, template: &str) -> Result<()> {
        let mut controls = self.source_controls.write().expect("scm lock poisoned");
        let sc = controls
            .get_mut(&sc_id)
            .ok_or_else(|| anyhow::anyhow!("source control {} not found", sc_id.0))?;
        sc.commit_template = Some(template.to_owned());
        Ok(())
    }

    /// Disposes a source control provider.
    pub fn dispose(&self, sc_id: SourceControlId) {
        self.source_controls
            .write()
            .expect("scm lock poisoned")
            .remove(&sc_id);
    }
}

impl Default for ScmApi {
    fn default() -> Self {
        Self::new()
    }
}
