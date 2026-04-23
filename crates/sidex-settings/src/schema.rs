//! Settings schema definitions used for validation, UI generation, and
//! extension-contributed setting registration.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Describes the type of a setting value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum SettingType {
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
    Null,
    Enum { values: Vec<String> },
}

/// Describes the scope at which a setting applies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingScope {
    /// Applies to the entire application (persisted once, globally).
    Application,
    /// Tied to a specific machine installation (not synced).
    Machine,
    /// Per-window settings.
    #[default]
    Window,
    /// Per-resource (file/folder) settings.
    Resource,
    /// Language-override settings, e.g. `"[rust]"`.
    Language,
}

/// Schema describing a single setting: its key, type, default value,
/// human-readable description, scope, validation constraints, and more.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingSchema {
    /// Dot-separated key, e.g. `"editor.fontSize"`.
    pub key: String,
    /// The value type.
    #[serde(rename = "type")]
    pub setting_type: SettingType,
    /// Default value as a JSON value.
    pub default: Value,
    /// Human-readable description shown in the settings UI.
    pub description: String,
    /// The scope at which this setting applies.
    #[serde(default)]
    pub scope: SettingScope,
    /// Valid enum values (when `setting_type` is `Enum` or a constrained
    /// string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<Value>>,
    /// Descriptions matching each `enum_values` entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_descriptions: Option<Vec<String>>,
    /// If set, the setting is deprecated and this message is shown to users.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_message: Option<String>,
    /// Tags for filtering/searching settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Minimum value for numeric settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    /// Maximum value for numeric settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    /// Regex pattern for string validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

/// Validate a setting value against its schema.
///
/// Returns `Ok(())` if the value is valid, or an error message describing
/// the validation failure.
pub fn validate_setting(key: &str, value: &Value, schema: &SettingSchema) -> Result<(), String> {
    match &schema.setting_type {
        SettingType::Boolean => {
            if !value.is_boolean() {
                return Err(format!("{key}: expected boolean"));
            }
        }
        SettingType::Number => {
            if !value.is_number() {
                return Err(format!("{key}: expected number"));
            }
            if let Some(n) = value.as_f64() {
                if let Some(min) = schema.minimum {
                    if n < min {
                        return Err(format!("{key}: value {n} is below minimum {min}"));
                    }
                }
                if let Some(max) = schema.maximum {
                    if n > max {
                        return Err(format!("{key}: value {n} is above maximum {max}"));
                    }
                }
            }
        }
        SettingType::Integer => {
            if !value.is_i64() && !value.is_u64() {
                return Err(format!("{key}: expected integer"));
            }
            if let Some(n) = value.as_f64() {
                if let Some(min) = schema.minimum {
                    if n < min {
                        return Err(format!("{key}: value {n} is below minimum {min}"));
                    }
                }
                if let Some(max) = schema.maximum {
                    if n > max {
                        return Err(format!("{key}: value {n} is above maximum {max}"));
                    }
                }
            }
        }
        SettingType::String => {
            if !value.is_string() {
                return Err(format!("{key}: expected string"));
            }
        }
        SettingType::Array => {
            if !value.is_array() {
                return Err(format!("{key}: expected array"));
            }
        }
        SettingType::Object => {
            if !value.is_object() {
                return Err(format!("{key}: expected object"));
            }
        }
        SettingType::Null => {
            if !value.is_null() {
                return Err(format!("{key}: expected null"));
            }
        }
        SettingType::Enum { values } => {
            let s = value
                .as_str()
                .ok_or_else(|| format!("{key}: expected string for enum"))?;
            if !values.iter().any(|v| v == s) {
                return Err(format!(
                    "{key}: '{s}' is not a valid enum value; expected one of: {}",
                    values.join(", ")
                ));
            }
        }
    }

    if let Some(enum_vals) = &schema.enum_values {
        if !enum_vals.contains(value) && !enum_vals.is_empty() {
            let allowed: Vec<String> = enum_vals
                .iter()
                .map(std::string::ToString::to_string)
                .collect();
            return Err(format!(
                "{key}: value not in allowed set: [{}]",
                allowed.join(", ")
            ));
        }
    }

    Ok(())
}

/// Registry that collects setting schemas contributed by core modules and
/// extensions.
#[derive(Clone, Debug, Default)]
pub struct SchemaRegistry {
    schemas: Vec<SettingSchema>,
}

impl SchemaRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a setting schema.
    pub fn register(&mut self, schema: SettingSchema) {
        if let Some(existing) = self.schemas.iter_mut().find(|s| s.key == schema.key) {
            *existing = schema;
        } else {
            self.schemas.push(schema);
        }
    }

    /// Register multiple schemas at once.
    pub fn register_many(&mut self, schemas: Vec<SettingSchema>) {
        for schema in schemas {
            self.register(schema);
        }
    }

    /// Look up a schema by key.
    pub fn get(&self, key: &str) -> Option<&SettingSchema> {
        self.schemas.iter().find(|s| s.key == key)
    }

    /// Return all registered schemas.
    pub fn all(&self) -> &[SettingSchema] {
        &self.schemas
    }

    /// Return schemas filtered by scope.
    pub fn by_scope(&self, scope: SettingScope) -> Vec<&SettingSchema> {
        self.schemas.iter().filter(|s| s.scope == scope).collect()
    }

    /// Return schemas filtered by tag.
    pub fn by_tag(&self, tag: &str) -> Vec<&SettingSchema> {
        self.schemas
            .iter()
            .filter(|s| s.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Return all deprecated schemas.
    pub fn deprecated(&self) -> Vec<&SettingSchema> {
        self.schemas
            .iter()
            .filter(|s| s.deprecated_message.is_some())
            .collect()
    }

    /// Validate a key-value pair against its registered schema.
    pub fn validate(&self, key: &str, value: &Value) -> Result<(), String> {
        if let Some(schema) = self.get(key) {
            validate_setting(key, value, schema)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_schema(key: &str) -> SettingSchema {
        SettingSchema {
            key: key.to_owned(),
            setting_type: SettingType::Number,
            default: json!(14),
            description: "Font size".to_owned(),
            scope: SettingScope::Resource,
            enum_values: None,
            enum_descriptions: None,
            deprecated_message: None,
            tags: vec!["editor".to_owned()],
            minimum: Some(1.0),
            maximum: Some(100.0),
            pattern: None,
        }
    }

    #[test]
    fn register_and_get() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_schema("editor.fontSize"));
        assert!(reg.get("editor.fontSize").is_some());
        assert!(reg.get("editor.tabSize").is_none());
    }

    #[test]
    fn register_overwrites() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_schema("editor.fontSize"));
        let mut updated = sample_schema("editor.fontSize");
        updated.default = json!(16);
        reg.register(updated);
        assert_eq!(reg.get("editor.fontSize").unwrap().default, json!(16));
        assert_eq!(reg.all().len(), 1);
    }

    #[test]
    fn all_returns_everything() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_schema("a"));
        reg.register(sample_schema("b"));
        assert_eq!(reg.all().len(), 2);
    }

    #[test]
    fn by_scope() {
        let mut reg = SchemaRegistry::new();
        let mut s1 = sample_schema("a");
        s1.scope = SettingScope::Resource;
        let mut s2 = sample_schema("b");
        s2.scope = SettingScope::Window;
        reg.register(s1);
        reg.register(s2);
        assert_eq!(reg.by_scope(SettingScope::Resource).len(), 1);
    }

    #[test]
    fn by_tag() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_schema("a"));
        assert_eq!(reg.by_tag("editor").len(), 1);
        assert_eq!(reg.by_tag("terminal").len(), 0);
    }

    #[test]
    fn validate_number_in_range() {
        let schema = sample_schema("editor.fontSize");
        assert!(validate_setting("editor.fontSize", &json!(14), &schema).is_ok());
        assert!(validate_setting("editor.fontSize", &json!(0), &schema).is_err());
        assert!(validate_setting("editor.fontSize", &json!(200), &schema).is_err());
    }

    #[test]
    fn validate_type_mismatch() {
        let schema = sample_schema("editor.fontSize");
        assert!(validate_setting("editor.fontSize", &json!("not a number"), &schema).is_err());
    }

    #[test]
    fn validate_boolean() {
        let schema = SettingSchema {
            key: "editor.minimap.enabled".to_owned(),
            setting_type: SettingType::Boolean,
            default: json!(true),
            description: "Enable minimap".to_owned(),
            scope: SettingScope::Resource,
            enum_values: None,
            enum_descriptions: None,
            deprecated_message: None,
            tags: vec![],
            minimum: None,
            maximum: None,
            pattern: None,
        };
        assert!(validate_setting("editor.minimap.enabled", &json!(true), &schema).is_ok());
        assert!(validate_setting("editor.minimap.enabled", &json!(42), &schema).is_err());
    }

    #[test]
    fn validate_enum() {
        let schema = SettingSchema {
            key: "editor.wordWrap".to_owned(),
            setting_type: SettingType::Enum {
                values: vec![
                    "off".to_owned(),
                    "on".to_owned(),
                    "wordWrapColumn".to_owned(),
                ],
            },
            default: json!("off"),
            description: "Word wrap".to_owned(),
            scope: SettingScope::Resource,
            enum_values: None,
            enum_descriptions: None,
            deprecated_message: None,
            tags: vec![],
            minimum: None,
            maximum: None,
            pattern: None,
        };
        assert!(validate_setting("editor.wordWrap", &json!("off"), &schema).is_ok());
        assert!(validate_setting("editor.wordWrap", &json!("invalid"), &schema).is_err());
    }

    #[test]
    fn registry_validate() {
        let mut reg = SchemaRegistry::new();
        reg.register(sample_schema("editor.fontSize"));
        assert!(reg.validate("editor.fontSize", &json!(14)).is_ok());
        assert!(reg.validate("editor.fontSize", &json!("bad")).is_err());
        assert!(reg.validate("unknown.key", &json!("anything")).is_ok());
    }
}
