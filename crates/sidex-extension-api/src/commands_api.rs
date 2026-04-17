//! `vscode.commands` API compatibility shim.
//!
//! Provides a global command registry where extensions (and the editor itself)
//! can register and execute named commands with arbitrary JSON arguments.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde_json::Value;

/// Callback type for a registered command.
pub type CommandHandler = Arc<dyn Fn(Value) -> Result<Value> + Send + Sync>;

/// Global command registry supporting registration and execution.
///
/// Thread-safe: commands can be registered and executed from any thread.
pub struct CommandRegistry {
    commands: RwLock<HashMap<String, CommandHandler>>,
}

impl CommandRegistry {
    /// Creates an empty command registry.
    pub fn new() -> Self {
        Self {
            commands: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a command handler under the given `id`.
    ///
    /// If a command with the same id already exists it is replaced.
    pub fn register(&self, id: &str, handler: CommandHandler) {
        self.commands
            .write()
            .expect("command registry lock poisoned")
            .insert(id.to_owned(), handler);
    }

    /// Executes a registered command, returning its result.
    pub fn execute(&self, id: &str, args: Value) -> Result<Value> {
        let handler = {
            let cmds = self
                .commands
                .read()
                .expect("command registry lock poisoned");
            cmds.get(id).cloned()
        };

        match handler {
            Some(h) => h(args),
            None => bail!("command not found: {id}"),
        }
    }

    /// Returns the ids of all registered commands.
    pub fn get_commands(&self) -> Vec<String> {
        self.commands
            .read()
            .expect("command registry lock poisoned")
            .keys()
            .cloned()
            .collect()
    }

    /// Returns `true` if a command with the given id is registered.
    pub fn has(&self, id: &str) -> bool {
        self.commands
            .read()
            .expect("command registry lock poisoned")
            .contains_key(id)
    }

    /// Removes a command from the registry.
    pub fn unregister(&self, id: &str) -> bool {
        self.commands
            .write()
            .expect("command registry lock poisoned")
            .remove(id)
            .is_some()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn register_and_execute() {
        let reg = CommandRegistry::new();
        reg.register(
            "test.greet",
            Arc::new(|args| {
                let name = args.as_str().unwrap_or("world");
                Ok(json!(format!("hello, {name}!")))
            }),
        );
        let result = reg.execute("test.greet", json!("rust")).unwrap();
        assert_eq!(result, json!("hello, rust!"));
    }

    #[test]
    fn execute_missing_command() {
        let reg = CommandRegistry::new();
        assert!(reg.execute("nonexistent", Value::Null).is_err());
    }

    #[test]
    fn get_commands_lists_all() {
        let reg = CommandRegistry::new();
        reg.register("a", Arc::new(|_| Ok(Value::Null)));
        reg.register("b", Arc::new(|_| Ok(Value::Null)));
        let cmds = reg.get_commands();
        assert_eq!(cmds.len(), 2);
        assert!(cmds.contains(&"a".to_owned()));
        assert!(cmds.contains(&"b".to_owned()));
    }

    #[test]
    fn has_command() {
        let reg = CommandRegistry::new();
        assert!(!reg.has("foo"));
        reg.register("foo", Arc::new(|_| Ok(Value::Null)));
        assert!(reg.has("foo"));
    }

    #[test]
    fn unregister_command() {
        let reg = CommandRegistry::new();
        reg.register("foo", Arc::new(|_| Ok(Value::Null)));
        assert!(reg.unregister("foo"));
        assert!(!reg.has("foo"));
        assert!(!reg.unregister("foo"));
    }

    #[test]
    fn replace_existing_command() {
        let reg = CommandRegistry::new();
        reg.register("cmd", Arc::new(|_| Ok(json!(1))));
        reg.register("cmd", Arc::new(|_| Ok(json!(2))));
        let result = reg.execute("cmd", Value::Null).unwrap();
        assert_eq!(result, json!(2));
    }

    #[test]
    fn thread_safety() {
        let reg = Arc::new(CommandRegistry::new());
        let reg2 = reg.clone();

        let handle = std::thread::spawn(move || {
            reg2.register("from_thread", Arc::new(|_| Ok(json!("ok"))));
        });
        handle.join().unwrap();

        assert!(reg.has("from_thread"));
        assert_eq!(
            reg.execute("from_thread", Value::Null).unwrap(),
            json!("ok")
        );
    }
}
