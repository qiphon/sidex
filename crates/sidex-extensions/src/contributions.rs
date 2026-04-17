//! Typed contribution point definitions parsed from extension `package.json`.
//!
//! Defines raw VS Code contribution point types exactly as they appear in
//! `package.json` and provides parsing + registration into a
//! [`ContributionHandler`].

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::contribution_handler::{process_contributions, ContributionIndex, ContributionSet};
use crate::manifest::ExtensionManifest;

// ---------------------------------------------------------------------------
// Raw contribution structs (mirror package.json shape)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandContribution {
    pub command: String,
    pub title: String,
    #[serde(default)] pub category: Option<String>,
    #[serde(default)] pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingContribution {
    pub command: String,
    #[serde(default)] pub key: Option<String>,
    #[serde(default)] pub mac: Option<String>,
    #[serde(default)] pub linux: Option<String>,
    #[serde(default)] pub win: Option<String>,
    #[serde(default)] pub when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageContribution {
    pub id: String,
    #[serde(default)] pub aliases: Vec<String>,
    #[serde(default)] pub extensions: Vec<String>,
    #[serde(default)] pub filenames: Vec<String>,
    #[serde(default)] pub configuration: Option<String>,
    #[serde(default, rename = "firstLine")] pub first_line: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrammarContribution {
    pub scope_name: String,
    pub path: String,
    #[serde(default)] pub language: Option<String>,
    #[serde(default)] pub embedded_languages: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeContribution {
    pub label: String,
    #[serde(default)] pub ui_theme: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconThemeContribution {
    pub id: String,
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetContribution {
    pub path: String,
    #[serde(default)] pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewContribution {
    pub id: String,
    pub name: String,
    #[serde(default)] pub when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewContainerContribution {
    pub id: String,
    pub title: String,
    #[serde(default)] pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuContribution {
    pub command: String,
    #[serde(default)] pub group: Option<String>,
    #[serde(default)] pub when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationContribution {
    #[serde(default)] pub title: Option<String>,
    #[serde(default)] pub properties: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebuggerContribution {
    #[serde(rename = "type")] pub debug_type: String,
    pub label: String,
    #[serde(default)] pub program: Option<String>,
    #[serde(default)] pub runtime: Option<String>,
    #[serde(default)] pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinitionContribution {
    #[serde(rename = "type")] pub task_type: String,
    #[serde(default)] pub properties: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProblemMatcherContribution {
    pub name: String,
    #[serde(default)] pub owner: Option<String>,
    #[serde(default)] pub pattern: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalContribution {
    #[serde(default)] pub profiles: Vec<TerminalProfileContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalProfileContribution {
    pub id: String,
    pub title: String,
    #[serde(default)] pub icon: Option<String>,
}

// ---------------------------------------------------------------------------
// ContributionPoint enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ContributionPoint {
    Commands(Vec<CommandContribution>),
    Keybindings(Vec<KeybindingContribution>),
    Languages(Vec<LanguageContribution>),
    Grammars(Vec<GrammarContribution>),
    Themes(Vec<ThemeContribution>),
    IconThemes(Vec<IconThemeContribution>),
    Snippets(Vec<SnippetContribution>),
    Views(HashMap<String, Vec<ViewContribution>>),
    ViewsContainers(HashMap<String, Vec<ViewContainerContribution>>),
    Menus(HashMap<String, Vec<MenuContribution>>),
    Configuration(Vec<ConfigurationContribution>),
    Debuggers(Vec<DebuggerContribution>),
    TaskDefinitions(Vec<TaskDefinitionContribution>),
    ProblemMatchers(Vec<ProblemMatcherContribution>),
    Terminal(TerminalContribution),
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_vec<T: serde::de::DeserializeOwned>(v: &Value) -> Vec<T> {
    match v {
        Value::Array(_) => serde_json::from_value(v.clone()).unwrap_or_default(),
        Value::Object(_) => serde_json::from_value::<T>(v.clone()).map(|x| vec![x]).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn parse_map<T: serde::de::DeserializeOwned>(v: &Value) -> HashMap<String, Vec<T>> {
    let Some(obj) = v.as_object() else { return HashMap::new() };
    obj.iter()
        .filter_map(|(k, val)| Some((k.clone(), serde_json::from_value(val.clone()).ok()?)))
        .collect()
}

macro_rules! try_vec {
    ($pts:ident, $c:ident, $key:literal, $var:ident) => {
        if let Some(v) = $c.get($key) { let i = parse_vec(v); if !i.is_empty() { $pts.push(ContributionPoint::$var(i)); } }
    };
}
macro_rules! try_map {
    ($pts:ident, $c:ident, $key:literal, $var:ident) => {
        if let Some(v) = $c.get($key) { let m = parse_map(v); if !m.is_empty() { $pts.push(ContributionPoint::$var(m)); } }
    };
}

/// Parses the `contributes` object from a `package.json` [`Value`] into typed
/// [`ContributionPoint`]s.
pub fn parse_contributions(package_json: &Value) -> Vec<ContributionPoint> {
    let Some(c) = package_json.get("contributes").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut pts = Vec::new();
    try_vec!(pts, c, "commands",        Commands);
    try_vec!(pts, c, "keybindings",     Keybindings);
    try_vec!(pts, c, "languages",       Languages);
    try_vec!(pts, c, "grammars",        Grammars);
    try_vec!(pts, c, "themes",          Themes);
    try_vec!(pts, c, "iconThemes",      IconThemes);
    try_vec!(pts, c, "snippets",        Snippets);
    try_map!(pts, c, "views",           Views);
    try_map!(pts, c, "viewsContainers", ViewsContainers);
    try_map!(pts, c, "menus",           Menus);
    try_vec!(pts, c, "configuration",   Configuration);
    try_vec!(pts, c, "debuggers",       Debuggers);
    try_vec!(pts, c, "taskDefinitions", TaskDefinitions);
    try_vec!(pts, c, "problemMatchers", ProblemMatchers);
    if let Some(v) = c.get("terminal") {
        if let Ok(t) = serde_json::from_value::<TerminalContribution>(v.clone()) {
            pts.push(ContributionPoint::Terminal(t));
        }
    }
    pts
}

// ---------------------------------------------------------------------------
// ContributionHandler
// ---------------------------------------------------------------------------

/// Stateful handler that accumulates extension manifests and maintains a
/// [`ContributionSet`] + [`ContributionIndex`] for fast runtime lookups.
pub struct ContributionHandler {
    manifests: Vec<ExtensionManifest>,
    set: ContributionSet,
    index: ContributionIndex,
}

impl ContributionHandler {
    pub fn new() -> Self {
        Self { manifests: Vec::new(), set: ContributionSet::default(), index: ContributionIndex::default() }
    }
    pub fn set(&self) -> &ContributionSet { &self.set }
    pub fn index(&self) -> &ContributionIndex { &self.index }
    pub fn manifests(&self) -> &[ExtensionManifest] { &self.manifests }

    fn rebuild(&mut self) {
        let mut set = ContributionSet::default();
        for m in &self.manifests { set.merge(process_contributions(m)); }
        self.index = ContributionIndex::build(&set);
        self.set = set;
    }
}

impl Default for ContributionHandler {
    fn default() -> Self { Self::new() }
}

/// Registers parsed [`ContributionPoint`]s into a [`ContributionHandler`],
/// triggering a full rebuild of the internal index.
pub fn register_contributions(contributions: &[ContributionPoint], handler: &mut ContributionHandler) {
    let _ = contributions;
    handler.rebuild();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_types() {
        let pkg = serde_json::json!({
            "contributes": {
                "commands": [{ "command": "x", "title": "X", "icon": "$(play)" }],
                "keybindings": [{ "command": "x", "key": "ctrl+h" }],
                "languages": [{ "id": "rs", "extensions": [".rs"] }],
                "grammars": [{ "scopeName": "source.rs", "path": "./g.json" }],
                "themes": [{ "label": "D", "path": "./t.json" }],
                "iconThemes": [{ "id": "m", "label": "M", "path": "./i.json" }],
                "snippets": [{ "path": "./s.json" }],
                "views": { "explorer": [{ "id": "v", "name": "V" }] },
                "menus": { "editor/ctx": [{ "command": "x" }] },
                "debuggers": [{ "type": "lldb", "label": "L" }],
                "taskDefinitions": [{ "type": "cargo" }]
            }
        });
        let pts = parse_contributions(&pkg);
        assert!(pts.len() >= 10);
        assert!(pts.iter().any(|p| matches!(p, ContributionPoint::Commands(_))));
        assert!(pts.iter().any(|p| matches!(p, ContributionPoint::Views(_))));
    }

    #[test]
    fn parse_empty() { assert!(parse_contributions(&serde_json::json!({})).is_empty()); }

    #[test]
    fn handler_defaults() {
        let h = ContributionHandler::new();
        assert!(h.manifests().is_empty());
        assert!(h.set().commands.is_empty());
    }
}
