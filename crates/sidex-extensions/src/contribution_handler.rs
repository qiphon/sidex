//! Extension contribution processing.
//!
//! When an extension is activated its `package.json` `contributes` section is
//! processed to register commands, menus, keybindings, languages, grammars,
//! themes, snippets, configuration schemas, views, debuggers, and task
//! definitions with the editor.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::{
    ContributedCommand, ContributedGrammar, ContributedLanguage, ContributedTheme,
    ExtensionManifest,
};

// ---------------------------------------------------------------------------
// Contribution kinds
// ---------------------------------------------------------------------------

/// A fully-resolved command contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCommand {
    pub extension_id: String,
    pub command: String,
    pub title: String,
    pub category: Option<String>,
}

/// A fully-resolved menu item contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedMenuItem {
    pub extension_id: String,
    pub menu_id: String,
    pub command: String,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub when: Option<String>,
}

/// A fully-resolved keybinding contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedKeybinding {
    pub extension_id: String,
    pub command: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub mac: Option<String>,
    #[serde(default)]
    pub linux: Option<String>,
    #[serde(default)]
    pub win: Option<String>,
    #[serde(default)]
    pub when: Option<String>,
}

/// A fully-resolved language contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedLanguage {
    pub extension_id: String,
    pub id: String,
    pub aliases: Vec<String>,
    pub extensions: Vec<String>,
    pub configuration_path: Option<PathBuf>,
}

/// A fully-resolved `TextMate` grammar contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedGrammar {
    pub extension_id: String,
    pub language: Option<String>,
    pub scope_name: String,
    pub grammar_path: PathBuf,
}

/// A fully-resolved color/icon theme contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTheme {
    pub extension_id: String,
    pub label: String,
    pub ui_theme: String,
    pub theme_path: PathBuf,
}

/// A fully-resolved snippet contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedSnippet {
    pub extension_id: String,
    pub language: Option<String>,
    pub snippet_path: PathBuf,
}

/// A fully-resolved configuration contribution (the raw JSON schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedConfiguration {
    pub extension_id: String,
    pub schema: Value,
}

/// A fully-resolved view container / view contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedView {
    pub extension_id: String,
    pub container_id: String,
    pub view_id: String,
    pub name: String,
    #[serde(default)]
    pub when: Option<String>,
}

/// A fully-resolved debugger contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedDebugger {
    pub extension_id: String,
    pub debug_type: String,
    pub label: String,
    #[serde(default)]
    pub program: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
}

/// A fully-resolved task definition contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTaskDefinition {
    pub extension_id: String,
    pub task_type: String,
    #[serde(default)]
    pub properties: Value,
}

// ---------------------------------------------------------------------------
// Aggregated result
// ---------------------------------------------------------------------------

/// All contributions extracted from a set of extension manifests.
#[derive(Debug, Clone, Default)]
pub struct ContributionSet {
    pub commands: Vec<ResolvedCommand>,
    pub menu_items: Vec<ResolvedMenuItem>,
    pub keybindings: Vec<ResolvedKeybinding>,
    pub languages: Vec<ResolvedLanguage>,
    pub grammars: Vec<ResolvedGrammar>,
    pub themes: Vec<ResolvedTheme>,
    pub snippets: Vec<ResolvedSnippet>,
    pub configurations: Vec<ResolvedConfiguration>,
    pub views: Vec<ResolvedView>,
    pub debuggers: Vec<ResolvedDebugger>,
    pub task_definitions: Vec<ResolvedTaskDefinition>,
}

impl ContributionSet {
    /// Merges `other` into `self`.
    pub fn merge(&mut self, other: ContributionSet) {
        self.commands.extend(other.commands);
        self.menu_items.extend(other.menu_items);
        self.keybindings.extend(other.keybindings);
        self.languages.extend(other.languages);
        self.grammars.extend(other.grammars);
        self.themes.extend(other.themes);
        self.snippets.extend(other.snippets);
        self.configurations.extend(other.configurations);
        self.views.extend(other.views);
        self.debuggers.extend(other.debuggers);
        self.task_definitions.extend(other.task_definitions);
    }
}

// ---------------------------------------------------------------------------
// Processing
// ---------------------------------------------------------------------------

/// Processes a single extension manifest's contributions, resolving relative
/// paths against `ext_dir`.
pub fn process_contributions(manifest: &ExtensionManifest) -> ContributionSet {
    let ext_id = manifest.canonical_id();
    let ext_dir = Path::new(&manifest.path);
    let c = &manifest.contributes;

    ContributionSet {
        commands: resolve_commands(&ext_id, &c.commands),
        menu_items: resolve_menus(&ext_id, &c.menus),
        keybindings: resolve_keybindings(&ext_id, &c.keybindings),
        languages: resolve_languages(&ext_id, ext_dir, &c.languages),
        grammars: resolve_grammars(&ext_id, ext_dir, &c.grammars),
        themes: resolve_themes(&ext_id, ext_dir, &c.themes),
        snippets: resolve_snippets(&ext_id, ext_dir, &c.snippets),
        configurations: resolve_configurations(&ext_id, &c.configuration),
        views: resolve_views(&ext_id, &c.views),
        debuggers: resolve_debuggers(&ext_id, &c.debuggers),
        task_definitions: resolve_task_definitions(&ext_id, &c.task_definitions),
    }
}

/// Processes all extensions in a slice of manifests.
pub fn process_all_contributions(manifests: &[ExtensionManifest]) -> ContributionSet {
    let mut result = ContributionSet::default();
    for m in manifests {
        result.merge(process_contributions(m));
    }
    result
}

// ---------------------------------------------------------------------------
// Individual resolvers
// ---------------------------------------------------------------------------

fn resolve_commands(ext_id: &str, commands: &[ContributedCommand]) -> Vec<ResolvedCommand> {
    commands
        .iter()
        .map(|cmd| ResolvedCommand {
            extension_id: ext_id.to_owned(),
            command: cmd.command.clone(),
            title: cmd.title.clone(),
            category: cmd.category.clone(),
        })
        .collect()
}

fn resolve_menus(ext_id: &str, menus: &Value) -> Vec<ResolvedMenuItem> {
    let mut out = Vec::new();
    let Some(obj) = menus.as_object() else {
        return out;
    };
    for (menu_id, items) in obj {
        let Some(arr) = items.as_array() else {
            continue;
        };
        for item in arr {
            let command = item
                .get("command")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if command.is_empty() {
                continue;
            }
            out.push(ResolvedMenuItem {
                extension_id: ext_id.to_owned(),
                menu_id: menu_id.clone(),
                command,
                group: item.get("group").and_then(Value::as_str).map(str::to_owned),
                when: item.get("when").and_then(Value::as_str).map(str::to_owned),
            });
        }
    }
    out
}

fn resolve_keybindings(ext_id: &str, bindings: &[Value]) -> Vec<ResolvedKeybinding> {
    bindings
        .iter()
        .filter_map(|v| {
            let command = v.get("command")?.as_str()?.to_owned();
            Some(ResolvedKeybinding {
                extension_id: ext_id.to_owned(),
                command,
                key: v.get("key").and_then(Value::as_str).map(str::to_owned),
                mac: v.get("mac").and_then(Value::as_str).map(str::to_owned),
                linux: v.get("linux").and_then(Value::as_str).map(str::to_owned),
                win: v.get("win").and_then(Value::as_str).map(str::to_owned),
                when: v.get("when").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

fn resolve_languages(
    ext_id: &str,
    ext_dir: &Path,
    languages: &[ContributedLanguage],
) -> Vec<ResolvedLanguage> {
    languages
        .iter()
        .map(|lang| ResolvedLanguage {
            extension_id: ext_id.to_owned(),
            id: lang.id.clone(),
            aliases: lang.aliases.clone(),
            extensions: lang.extensions.clone(),
            configuration_path: lang.configuration.as_ref().map(|p| ext_dir.join(p)),
        })
        .collect()
}

fn resolve_grammars(
    ext_id: &str,
    ext_dir: &Path,
    grammars: &[ContributedGrammar],
) -> Vec<ResolvedGrammar> {
    grammars
        .iter()
        .map(|g| ResolvedGrammar {
            extension_id: ext_id.to_owned(),
            language: g.language.clone(),
            scope_name: g.scope_name.clone(),
            grammar_path: ext_dir.join(&g.path),
        })
        .collect()
}

fn resolve_themes(ext_id: &str, ext_dir: &Path, themes: &[ContributedTheme]) -> Vec<ResolvedTheme> {
    themes
        .iter()
        .map(|t| ResolvedTheme {
            extension_id: ext_id.to_owned(),
            label: t.label.clone(),
            ui_theme: t.ui_theme.clone(),
            theme_path: ext_dir.join(&t.path),
        })
        .collect()
}

fn resolve_snippets(ext_id: &str, ext_dir: &Path, snippets: &[Value]) -> Vec<ResolvedSnippet> {
    snippets
        .iter()
        .filter_map(|v| {
            let path = v.get("path")?.as_str()?;
            Some(ResolvedSnippet {
                extension_id: ext_id.to_owned(),
                language: v.get("language").and_then(Value::as_str).map(str::to_owned),
                snippet_path: ext_dir.join(path),
            })
        })
        .collect()
}

fn resolve_configurations(ext_id: &str, config: &Value) -> Vec<ResolvedConfiguration> {
    if config.is_null() || (config.is_object() && config.as_object().unwrap().is_empty()) {
        return Vec::new();
    }
    vec![ResolvedConfiguration {
        extension_id: ext_id.to_owned(),
        schema: config.clone(),
    }]
}

fn resolve_views(ext_id: &str, views: &Value) -> Vec<ResolvedView> {
    let mut out = Vec::new();
    let Some(obj) = views.as_object() else {
        return out;
    };
    for (container_id, items) in obj {
        let Some(arr) = items.as_array() else {
            continue;
        };
        for item in arr {
            let view_id = match item.get("id").and_then(Value::as_str) {
                Some(id) => id.to_owned(),
                None => continue,
            };
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(&view_id)
                .to_owned();
            out.push(ResolvedView {
                extension_id: ext_id.to_owned(),
                container_id: container_id.clone(),
                view_id,
                name,
                when: item.get("when").and_then(Value::as_str).map(str::to_owned),
            });
        }
    }
    out
}

fn resolve_debuggers(ext_id: &str, debuggers: &[Value]) -> Vec<ResolvedDebugger> {
    debuggers
        .iter()
        .filter_map(|v| {
            let debug_type = v.get("type")?.as_str()?.to_owned();
            let label = v
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or(&debug_type)
                .to_owned();
            Some(ResolvedDebugger {
                extension_id: ext_id.to_owned(),
                debug_type,
                label,
                program: v.get("program").and_then(Value::as_str).map(str::to_owned),
                runtime: v.get("runtime").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
}

fn resolve_task_definitions(ext_id: &str, defs: &[Value]) -> Vec<ResolvedTaskDefinition> {
    defs.iter()
        .filter_map(|v| {
            let task_type = v.get("type")?.as_str()?.to_owned();
            let properties = v.get("properties").cloned().unwrap_or(Value::Null);
            Some(ResolvedTaskDefinition {
                extension_id: ext_id.to_owned(),
                task_type,
                properties,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Contribution registry (runtime lookup)
// ---------------------------------------------------------------------------

/// Fast-lookup index over contributions for use at runtime.
#[derive(Debug, Default)]
pub struct ContributionIndex {
    commands_by_id: HashMap<String, ResolvedCommand>,
    grammars_by_scope: HashMap<String, ResolvedGrammar>,
    languages_by_id: HashMap<String, ResolvedLanguage>,
    themes_by_label: HashMap<String, ResolvedTheme>,
    debuggers_by_type: HashMap<String, ResolvedDebugger>,
    task_defs_by_type: HashMap<String, ResolvedTaskDefinition>,
}

impl ContributionIndex {
    /// Build an index from a `ContributionSet`.
    pub fn build(set: &ContributionSet) -> Self {
        let mut idx = Self::default();
        for cmd in &set.commands {
            idx.commands_by_id.insert(cmd.command.clone(), cmd.clone());
        }
        for g in &set.grammars {
            idx.grammars_by_scope
                .insert(g.scope_name.clone(), g.clone());
        }
        for l in &set.languages {
            idx.languages_by_id.insert(l.id.clone(), l.clone());
        }
        for t in &set.themes {
            idx.themes_by_label.insert(t.label.clone(), t.clone());
        }
        for d in &set.debuggers {
            idx.debuggers_by_type
                .insert(d.debug_type.clone(), d.clone());
        }
        for td in &set.task_definitions {
            idx.task_defs_by_type
                .insert(td.task_type.clone(), td.clone());
        }
        idx
    }

    pub fn command(&self, id: &str) -> Option<&ResolvedCommand> {
        self.commands_by_id.get(id)
    }

    pub fn grammar_by_scope(&self, scope: &str) -> Option<&ResolvedGrammar> {
        self.grammars_by_scope.get(scope)
    }

    pub fn language(&self, id: &str) -> Option<&ResolvedLanguage> {
        self.languages_by_id.get(id)
    }

    pub fn theme(&self, label: &str) -> Option<&ResolvedTheme> {
        self.themes_by_label.get(label)
    }

    pub fn debugger(&self, debug_type: &str) -> Option<&ResolvedDebugger> {
        self.debuggers_by_type.get(debug_type)
    }

    pub fn task_definition(&self, task_type: &str) -> Option<&ResolvedTaskDefinition> {
        self.task_defs_by_type.get(task_type)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::parse_manifest_str;

    fn manifest_with_contributes(contributes_json: &str) -> ExtensionManifest {
        let json = format!(
            r#"{{
                "name": "test-ext",
                "publisher": "acme",
                "version": "1.0.0",
                "contributes": {contributes_json}
            }}"#
        );
        parse_manifest_str(&json).unwrap()
    }

    #[test]
    fn process_commands() {
        let m = manifest_with_contributes(
            r#"{
                "commands": [
                    { "command": "ext.hello", "title": "Hello" },
                    { "command": "ext.bye", "title": "Bye", "category": "Greetings" }
                ]
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.commands.len(), 2);
        assert_eq!(cs.commands[0].command, "ext.hello");
        assert_eq!(cs.commands[1].category.as_deref(), Some("Greetings"));
        assert_eq!(cs.commands[0].extension_id, "acme.test-ext");
    }

    #[test]
    fn process_menus() {
        let m = manifest_with_contributes(
            r#"{
                "menus": {
                    "editor/context": [
                        { "command": "ext.run", "group": "navigation", "when": "editorFocus" }
                    ]
                }
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.menu_items.len(), 1);
        assert_eq!(cs.menu_items[0].menu_id, "editor/context");
        assert_eq!(cs.menu_items[0].command, "ext.run");
        assert_eq!(cs.menu_items[0].group.as_deref(), Some("navigation"));
    }

    #[test]
    fn process_keybindings() {
        let m = manifest_with_contributes(
            r#"{
                "keybindings": [
                    { "command": "ext.run", "key": "ctrl+shift+r", "mac": "cmd+shift+r", "when": "editorFocus" }
                ]
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.keybindings.len(), 1);
        assert_eq!(cs.keybindings[0].key.as_deref(), Some("ctrl+shift+r"));
        assert_eq!(cs.keybindings[0].mac.as_deref(), Some("cmd+shift+r"));
    }

    #[test]
    fn process_languages() {
        let m = manifest_with_contributes(
            r#"{
                "languages": [
                    { "id": "rust", "extensions": [".rs"], "aliases": ["Rust"] }
                ]
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.languages.len(), 1);
        assert_eq!(cs.languages[0].id, "rust");
        assert_eq!(cs.languages[0].extensions, vec![".rs"]);
    }

    #[test]
    fn process_grammars() {
        let m = manifest_with_contributes(
            r#"{
                "grammars": [
                    { "language": "rust", "scopeName": "source.rust", "path": "./syntaxes/rust.tmLanguage.json" }
                ]
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.grammars.len(), 1);
        assert_eq!(cs.grammars[0].scope_name, "source.rust");
        assert!(cs.grammars[0]
            .grammar_path
            .to_string_lossy()
            .contains("syntaxes"));
    }

    #[test]
    fn process_themes() {
        let m = manifest_with_contributes(
            r#"{
                "themes": [
                    { "label": "One Dark", "uiTheme": "vs-dark", "path": "./themes/one-dark.json" }
                ]
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.themes.len(), 1);
        assert_eq!(cs.themes[0].label, "One Dark");
        assert_eq!(cs.themes[0].ui_theme, "vs-dark");
    }

    #[test]
    fn process_views() {
        let m = manifest_with_contributes(
            r#"{
                "views": {
                    "explorer": [
                        { "id": "ext.treeView", "name": "My Tree" }
                    ]
                }
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.views.len(), 1);
        assert_eq!(cs.views[0].container_id, "explorer");
        assert_eq!(cs.views[0].view_id, "ext.treeView");
        assert_eq!(cs.views[0].name, "My Tree");
    }

    #[test]
    fn process_debuggers() {
        let m = manifest_with_contributes(
            r#"{
                "debuggers": [
                    { "type": "node", "label": "Node.js", "program": "./out/debug.js", "runtime": "node" }
                ]
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.debuggers.len(), 1);
        assert_eq!(cs.debuggers[0].debug_type, "node");
        assert_eq!(cs.debuggers[0].label, "Node.js");
    }

    #[test]
    fn process_task_definitions() {
        let m = manifest_with_contributes(
            r#"{
                "taskDefinitions": [
                    { "type": "npm", "properties": { "script": { "type": "string" } } }
                ]
            }"#,
        );
        let cs = process_contributions(&m);
        assert_eq!(cs.task_definitions.len(), 1);
        assert_eq!(cs.task_definitions[0].task_type, "npm");
    }

    #[test]
    fn contribution_set_merge() {
        let mut a = ContributionSet::default();
        a.commands.push(ResolvedCommand {
            extension_id: "a".into(),
            command: "a.cmd".into(),
            title: "A".into(),
            category: None,
        });

        let mut b = ContributionSet::default();
        b.commands.push(ResolvedCommand {
            extension_id: "b".into(),
            command: "b.cmd".into(),
            title: "B".into(),
            category: None,
        });

        a.merge(b);
        assert_eq!(a.commands.len(), 2);
    }

    #[test]
    fn contribution_index_lookup() {
        let m = manifest_with_contributes(
            r#"{
                "commands": [{ "command": "ext.hello", "title": "Hello" }],
                "grammars": [{ "language": "rust", "scopeName": "source.rust", "path": "./g.json" }],
                "debuggers": [{ "type": "lldb", "label": "LLDB" }]
            }"#,
        );
        let cs = process_contributions(&m);
        let idx = ContributionIndex::build(&cs);

        assert!(idx.command("ext.hello").is_some());
        assert!(idx.command("nonexistent").is_none());
        assert!(idx.grammar_by_scope("source.rust").is_some());
        assert!(idx.debugger("lldb").is_some());
    }

    #[test]
    fn empty_contributions() {
        let m = manifest_with_contributes("{}");
        let cs = process_contributions(&m);
        assert!(cs.commands.is_empty());
        assert!(cs.menu_items.is_empty());
        assert!(cs.keybindings.is_empty());
        assert!(cs.languages.is_empty());
        assert!(cs.grammars.is_empty());
        assert!(cs.themes.is_empty());
        assert!(cs.snippets.is_empty());
        assert!(cs.configurations.is_empty());
        assert!(cs.views.is_empty());
        assert!(cs.debuggers.is_empty());
        assert!(cs.task_definitions.is_empty());
    }
}
