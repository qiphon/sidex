//! Debug adapter registry — maps debug types to adapter descriptors.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Source of a debug adapter registration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AdapterSource {
    /// Built-in adapter (shipped with SideX).
    Builtin,
    /// Adapter contributed by an extension.
    Extension { extension_id: String },
}

/// Describes how to launch a debug adapter for a given debug type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugAdapterDescriptor {
    /// Debug type name (e.g. "node", "python", "cppdbg", "lldb").
    pub type_name: String,
    /// Command to run the adapter.
    pub command: String,
    /// Arguments to pass to the adapter command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional runtime (e.g. "node" for JS-based adapters).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// Where this adapter was registered from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AdapterSource>,
    /// Optional: relative path base for resolving program/runtime paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension_path: Option<String>,
    /// Optional: label for display purposes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Optional: language IDs this debugger applies to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
}

/// Registry that maps debug type names to their adapter descriptors.
#[derive(Debug, Default)]
pub struct DebugAdapterRegistry {
    adapters: HashMap<String, DebugAdapterDescriptor>,
}

impl DebugAdapterRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry pre-populated with built-in adapter configurations.
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();

        registry.register(
            "node",
            DebugAdapterDescriptor {
                type_name: "node".to_owned(),
                command: "js-debug-adapter".to_owned(),
                args: Vec::new(),
                runtime: Some("node".to_owned()),
                source: Some(AdapterSource::Builtin),
                extension_path: None,
                label: Some("Node.js Debug".to_owned()),
                languages: vec!["javascript".to_owned(), "typescript".to_owned()],
            },
        );

        registry.register(
            "python",
            DebugAdapterDescriptor {
                type_name: "python".to_owned(),
                command: "debugpy-adapter".to_owned(),
                args: Vec::new(),
                runtime: Some("python3".to_owned()),
                source: Some(AdapterSource::Builtin),
                extension_path: None,
                label: Some("Python Debug".to_owned()),
                languages: vec!["python".to_owned()],
            },
        );

        registry.register(
            "cppdbg",
            DebugAdapterDescriptor {
                type_name: "cppdbg".to_owned(),
                command: "OpenDebugAD7".to_owned(),
                args: Vec::new(),
                runtime: None,
                source: Some(AdapterSource::Builtin),
                extension_path: None,
                label: Some("C++ Debug".to_owned()),
                languages: vec!["cpp".to_owned(), "c".to_owned()],
            },
        );

        registry.register(
            "lldb",
            DebugAdapterDescriptor {
                type_name: "lldb".to_owned(),
                command: "lldb-dap".to_owned(),
                args: Vec::new(),
                runtime: None,
                source: Some(AdapterSource::Builtin),
                extension_path: None,
                label: Some("LLDB Debug".to_owned()),
                languages: vec!["rust".to_owned(), "cpp".to_owned(), "c".to_owned()],
            },
        );

        registry.register(
            "go",
            DebugAdapterDescriptor {
                type_name: "go".to_owned(),
                command: "dlv".to_owned(),
                args: vec!["dap".to_owned()],
                runtime: None,
                source: Some(AdapterSource::Builtin),
                extension_path: None,
                label: Some("Go Debug".to_owned()),
                languages: vec!["go".to_owned()],
            },
        );

        registry
    }

    /// Registers a debug adapter descriptor for a debug type.
    pub fn register(&mut self, type_name: &str, descriptor: DebugAdapterDescriptor) {
        self.adapters.insert(type_name.to_owned(), descriptor);
    }

    /// Registers a debug adapter from an extension contribution.
    /// Returns true if the adapter was newly registered, false if it already existed.
    pub fn register_from_extension(
        &mut self,
        extension_id: &str,
        debug_type: &str,
        label: &str,
        program: Option<&str>,
        runtime: Option<&str>,
        args: Option<Vec<String>>,
        extension_path: Option<&str>,
        languages: Vec<String>,
    ) -> bool {
        let already_exists = self.adapters.contains_key(debug_type);

        if already_exists {
            // Update existing adapter with extension info
            if let Some(existing) = self.adapters.get_mut(debug_type) {
                existing.source = Some(AdapterSource::Extension {
                    extension_id: extension_id.to_owned(),
                });
                if let Some(prog) = program {
                    existing.command = prog.to_owned();
                }
                if let Some(rt) = runtime {
                    existing.runtime = Some(rt.to_owned());
                }
                if let Some(a) = args {
                    existing.args = a;
                }
                existing.extension_path = extension_path.map(str::to_owned);
                existing.label = Some(label.to_owned());
                existing.languages = languages;
            }
            false
        } else {
            // Register new adapter
            let descriptor = DebugAdapterDescriptor {
                type_name: debug_type.to_owned(),
                command: program.unwrap_or(debug_type).to_owned(),
                args: args.unwrap_or_default(),
                runtime: runtime.map(str::to_owned),
                source: Some(AdapterSource::Extension {
                    extension_id: extension_id.to_owned(),
                }),
                extension_path: extension_path.map(str::to_owned),
                label: Some(label.to_owned()),
                languages,
            };
            self.adapters.insert(debug_type.to_owned(), descriptor);
            true
        }
    }

    /// Unregisters a debug adapter by type name.
    /// Returns true if the adapter was removed, false if it didn't exist.
    pub fn unregister(&mut self, type_name: &str) -> bool {
        self.adapters.remove(type_name).is_some()
    }

    /// Unregisters all adapters contributed by a specific extension.
    pub fn unregister_extension(&mut self, extension_id: &str) -> Vec<String> {
        let to_remove: Vec<String> = self
            .adapters
            .iter()
            .filter(|(_, desc)| {
                matches!(
                    &desc.source,
                    Some(AdapterSource::Extension { extension_id: id }) if id == extension_id
                )
            })
            .map(|(type_name, _)| type_name.clone())
            .collect();

        for type_name in &to_remove {
            self.adapters.remove(type_name);
        }

        to_remove
    }

    /// Looks up a descriptor by debug type name.
    pub fn get(&self, type_name: &str) -> Option<&DebugAdapterDescriptor> {
        self.adapters.get(type_name)
    }

    /// Returns all registered debug type names.
    pub fn registered_types(&self) -> Vec<&str> {
        self.adapters.keys().map(String::as_str).collect()
    }

    /// Returns all registered adapters with their metadata.
    pub fn all_adapters(&self) -> Vec<&DebugAdapterDescriptor> {
        self.adapters.values().collect()
    }

    /// Returns adapters contributed by extensions (excluding builtins).
    pub fn extension_adapters(&self) -> Vec<&DebugAdapterDescriptor> {
        self.adapters
            .values()
            .filter(|desc| matches!(desc.source, Some(AdapterSource::Extension { .. })))
            .collect()
    }

    /// Builds the full command line to launch the adapter for a given type.
    pub fn command_line(&self, type_name: &str) -> Option<String> {
        self.adapters.get(type_name).map(|d| {
            let mut parts = Vec::new();
            if let Some(ref rt) = d.runtime {
                parts.push(rt.as_str());
            }
            parts.push(&d.command);
            for arg in &d.args {
                parts.push(arg);
            }
            parts.join(" ")
        })
    }

    /// Resolves the absolute path for a program or runtime, using the extension path as base.
    pub fn resolve_path(&self, type_name: &str, relative_path: &str) -> Option<String> {
        self.adapters.get(type_name).and_then(|d| {
            if let Some(ref ext_path) = d.extension_path {
                // Use std::path::PathBuf to resolve
                let base = std::path::Path::new(ext_path);
                let resolved = base.join(relative_path);
                resolved.to_str().map(str::to_owned)
            } else {
                Some(relative_path.to_owned())
            }
        })
    }

    /// Returns the count of registered adapters.
    pub fn count(&self) -> usize {
        self.adapters.len()
    }

    /// Returns the count of extension-contributed adapters.
    pub fn extension_count(&self) -> usize {
        self.extension_adapters().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_get() {
        let mut registry = DebugAdapterRegistry::new();
        registry.register(
            "rust",
            DebugAdapterDescriptor {
                type_name: "rust".to_owned(),
                command: "lldb-dap".to_owned(),
                args: Vec::new(),
                runtime: None,
            },
        );

        let desc = registry.get("rust").unwrap();
        assert_eq!(desc.command, "lldb-dap");
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn builtins_present() {
        let registry = DebugAdapterRegistry::with_builtins();
        assert!(registry.get("node").is_some());
        assert!(registry.get("python").is_some());
        assert!(registry.get("cppdbg").is_some());
        assert!(registry.get("lldb").is_some());
        assert!(registry.get("go").is_some());
    }

    #[test]
    fn command_line_with_runtime() {
        let registry = DebugAdapterRegistry::with_builtins();
        let cmd = registry.command_line("node").unwrap();
        assert_eq!(cmd, "node js-debug-adapter");
    }

    #[test]
    fn command_line_with_args() {
        let registry = DebugAdapterRegistry::with_builtins();
        let cmd = registry.command_line("go").unwrap();
        assert_eq!(cmd, "dlv dap");
    }

    #[test]
    fn registered_types_lists_all() {
        let registry = DebugAdapterRegistry::with_builtins();
        let types = registry.registered_types();
        assert!(types.contains(&"node"));
        assert!(types.contains(&"python"));
    }

    #[test]
    fn descriptor_serialization() {
        let desc = DebugAdapterDescriptor {
            type_name: "test".to_owned(),
            command: "test-adapter".to_owned(),
            args: vec!["--verbose".to_owned()],
            runtime: Some("node".to_owned()),
        };
        let json = serde_json::to_string(&desc).unwrap();
        let back: DebugAdapterDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(back.type_name, "test");
        assert_eq!(back.args.len(), 1);
    }
}
