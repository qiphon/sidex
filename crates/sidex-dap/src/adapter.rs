//! Debug adapter registry — maps debug types to adapter descriptors.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Describes how to launch a debug adapter for a given debug type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugAdapterDescriptor {
    /// Debug type name (e.g. "node", "python", "cppdbg", "lldb").
    pub type_name: String,
    /// Command to run the adapter.
    pub command: String,
    /// Arguments to pass to the adapter command.
    pub args: Vec<String>,
    /// Optional runtime (e.g. "node" for JS-based adapters).
    pub runtime: Option<String>,
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
            },
        );

        registry.register(
            "python",
            DebugAdapterDescriptor {
                type_name: "python".to_owned(),
                command: "debugpy-adapter".to_owned(),
                args: Vec::new(),
                runtime: Some("python3".to_owned()),
            },
        );

        registry.register(
            "cppdbg",
            DebugAdapterDescriptor {
                type_name: "cppdbg".to_owned(),
                command: "OpenDebugAD7".to_owned(),
                args: Vec::new(),
                runtime: None,
            },
        );

        registry.register(
            "lldb",
            DebugAdapterDescriptor {
                type_name: "lldb".to_owned(),
                command: "lldb-dap".to_owned(),
                args: Vec::new(),
                runtime: None,
            },
        );

        registry.register(
            "go",
            DebugAdapterDescriptor {
                type_name: "go".to_owned(),
                command: "dlv".to_owned(),
                args: vec!["dap".to_owned()],
                runtime: None,
            },
        );

        registry
    }

    /// Registers a debug adapter descriptor for a debug type.
    pub fn register(&mut self, type_name: &str, descriptor: DebugAdapterDescriptor) {
        self.adapters.insert(type_name.to_owned(), descriptor);
    }

    /// Looks up a descriptor by debug type name.
    pub fn get(&self, type_name: &str) -> Option<&DebugAdapterDescriptor> {
        self.adapters.get(type_name)
    }

    /// Returns all registered debug type names.
    pub fn registered_types(&self) -> Vec<&str> {
        self.adapters.keys().map(String::as_str).collect()
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
