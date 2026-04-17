//! Settings migration — rename, transform, or deprecate settings keys
//! between versions so that user configuration remains valid across upgrades.

use serde_json::{Map, Value};

/// A single migration rule that transforms settings from one version to
/// the next.
#[derive(Clone, Debug)]
pub struct MigrationRule {
    /// Version this migration upgrades *from*.
    pub from_version: u32,
    /// Version this migration upgrades *to*.
    pub to_version: u32,
    /// The transformation to apply.
    pub action: MigrationAction,
}

/// The kind of transformation to perform on the settings map.
#[derive(Clone, Debug)]
pub enum MigrationAction {
    /// Rename a settings key.
    Rename {
        old_key: String,
        new_key: String,
    },
    /// Remove a deprecated key.
    Remove {
        key: String,
    },
    /// Replace a key's value with a new default if it matches `old_value`.
    ReplaceValue {
        key: String,
        old_value: Value,
        new_value: Value,
    },
    /// Copy the value from one key to another (keeping the source).
    CopyKey {
        source: String,
        destination: String,
    },
}

/// Apply a sequence of migration rules to a settings map, transforming it
/// from `current_version` up to `target_version`.
///
/// Returns the new version after all applicable migrations have been applied.
pub fn migrate_settings(
    settings: &mut Map<String, Value>,
    current_version: u32,
    target_version: u32,
    rules: &[MigrationRule],
) -> u32 {
    let mut version = current_version;

    let mut applicable: Vec<&MigrationRule> = rules
        .iter()
        .filter(|r| r.from_version >= current_version && r.to_version <= target_version)
        .collect();
    applicable.sort_by_key(|r| r.from_version);

    for rule in applicable {
        if rule.from_version >= version {
            apply_action(settings, &rule.action);
            version = rule.to_version;
        }
    }

    version
}

fn apply_action(settings: &mut Map<String, Value>, action: &MigrationAction) {
    match action {
        MigrationAction::Rename { old_key, new_key } => {
            if let Some(val) = settings.remove(old_key) {
                settings.insert(new_key.clone(), val);
            }
        }
        MigrationAction::Remove { key } => {
            settings.remove(key);
        }
        MigrationAction::ReplaceValue {
            key,
            old_value,
            new_value,
        } => {
            if let Some(current) = settings.get(key) {
                if current == old_value {
                    settings.insert(key.clone(), new_value.clone());
                }
            }
        }
        MigrationAction::CopyKey {
            source,
            destination,
        } => {
            if let Some(val) = settings.get(source).cloned() {
                settings.insert(destination.clone(), val);
            }
        }
    }
}

/// Convenience: create a rename migration.
pub fn rename_rule(from_version: u32, to_version: u32, old_key: &str, new_key: &str) -> MigrationRule {
    MigrationRule {
        from_version,
        to_version,
        action: MigrationAction::Rename {
            old_key: old_key.to_owned(),
            new_key: new_key.to_owned(),
        },
    }
}

/// Convenience: create a remove migration.
pub fn remove_rule(from_version: u32, to_version: u32, key: &str) -> MigrationRule {
    MigrationRule {
        from_version,
        to_version,
        action: MigrationAction::Remove {
            key: key.to_owned(),
        },
    }
}

/// Built-in migration rules (extend as needed between releases).
pub fn builtin_migrations() -> Vec<MigrationRule> {
    vec![
        rename_rule(
            1,
            2,
            "editor.autoIndent",
            "editor.autoIndent",
        ),
        rename_rule(
            1,
            2,
            "editor.quickSuggestions.comments",
            "editor.quickSuggestions",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rename_migration() {
        let mut settings = Map::new();
        settings.insert("old.key".to_owned(), json!(42));

        let rules = vec![rename_rule(1, 2, "old.key", "new.key")];
        let v = migrate_settings(&mut settings, 1, 2, &rules);

        assert_eq!(v, 2);
        assert!(settings.get("old.key").is_none());
        assert_eq!(settings.get("new.key"), Some(&json!(42)));
    }

    #[test]
    fn remove_migration() {
        let mut settings = Map::new();
        settings.insert("deprecated.key".to_owned(), json!("value"));

        let rules = vec![remove_rule(1, 2, "deprecated.key")];
        migrate_settings(&mut settings, 1, 2, &rules);

        assert!(settings.get("deprecated.key").is_none());
    }

    #[test]
    fn replace_value_migration() {
        let mut settings = Map::new();
        settings.insert("editor.mode".to_owned(), json!("legacy"));

        let rules = vec![MigrationRule {
            from_version: 1,
            to_version: 2,
            action: MigrationAction::ReplaceValue {
                key: "editor.mode".to_owned(),
                old_value: json!("legacy"),
                new_value: json!("modern"),
            },
        }];
        migrate_settings(&mut settings, 1, 2, &rules);

        assert_eq!(settings.get("editor.mode"), Some(&json!("modern")));
    }

    #[test]
    fn replace_value_no_op_when_different() {
        let mut settings = Map::new();
        settings.insert("editor.mode".to_owned(), json!("custom"));

        let rules = vec![MigrationRule {
            from_version: 1,
            to_version: 2,
            action: MigrationAction::ReplaceValue {
                key: "editor.mode".to_owned(),
                old_value: json!("legacy"),
                new_value: json!("modern"),
            },
        }];
        migrate_settings(&mut settings, 1, 2, &rules);

        assert_eq!(settings.get("editor.mode"), Some(&json!("custom")));
    }

    #[test]
    fn copy_key_migration() {
        let mut settings = Map::new();
        settings.insert("source".to_owned(), json!("data"));

        let rules = vec![MigrationRule {
            from_version: 1,
            to_version: 2,
            action: MigrationAction::CopyKey {
                source: "source".to_owned(),
                destination: "dest".to_owned(),
            },
        }];
        migrate_settings(&mut settings, 1, 2, &rules);

        assert_eq!(settings.get("source"), Some(&json!("data")));
        assert_eq!(settings.get("dest"), Some(&json!("data")));
    }

    #[test]
    fn skips_out_of_range_migrations() {
        let mut settings = Map::new();
        settings.insert("a".to_owned(), json!(1));

        let rules = vec![
            rename_rule(1, 2, "a", "b"),
            rename_rule(3, 4, "b", "c"),
        ];
        let v = migrate_settings(&mut settings, 1, 2, &rules);

        assert_eq!(v, 2);
        assert!(settings.get("b").is_some());
        assert!(settings.get("c").is_none());
    }

    #[test]
    fn builtin_migrations_exist() {
        let m = builtin_migrations();
        assert!(!m.is_empty());
    }
}
