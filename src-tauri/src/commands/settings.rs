use std::sync::{Arc, RwLock};

use serde_json::Value;
use sidex_settings::{parse_jsonc, modify_jsonc, Settings};
use tauri::State;

pub struct SettingsStore {
    inner: RwLock<Settings>,
}

impl SettingsStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Settings::new()),
        }
    }

    pub fn load_user(&self, path: &std::path::Path) -> Result<(), String> {
        self.inner
            .write()
            .map_err(|e| e.to_string())?
            .load_user(path)
            .map_err(|e| e.to_string())
    }

    pub fn load_workspace(&self, path: &std::path::Path) -> Result<(), String> {
        self.inner
            .write()
            .map_err(|e| e.to_string())?
            .load_workspace(path)
            .map_err(|e| e.to_string())
    }
}

/// Get settings. If `section` is provided, returns only the value for that key;
/// otherwise returns the full merged settings object.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_get(
    state: State<'_, Arc<SettingsStore>>,
    section: Option<String>,
) -> Result<Value, String> {
    let settings = state.inner.read().map_err(|e| e.to_string())?;

    match section {
        Some(key) => Ok(settings.get_raw(&key).cloned().unwrap_or(Value::Null)),
        None => {
            let mut merged = serde_json::Map::new();
            // Collect all keys across layers via the builtin defaults as the
            // canonical key set, then overlay user/workspace via get_raw.
            let defaults = sidex_settings::builtin_defaults();
            if let Some(obj) = defaults.as_object() {
                for key in obj.keys() {
                    if let Some(val) = settings.get_raw(key) {
                        merged.insert(key.clone(), val.clone());
                    }
                }
            }
            Ok(Value::Object(merged))
        }
    }
}

/// Update a setting.
///
/// `scope` must be `"user"` or `"workspace"`.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_update(
    state: State<'_, Arc<SettingsStore>>,
    key: String,
    value: Value,
    scope: String,
) -> Result<(), String> {
    let mut settings = state.inner.write().map_err(|e| e.to_string())?;

    match scope.as_str() {
        "user" => settings.set(&key, value),
        "workspace" => settings.set_workspace(&key, value),
        _ => return Err(format!("invalid scope '{scope}': expected \"user\" or \"workspace\"")),
    }
    Ok(())
}

/// Load settings from a JSONC file into the specified layer.
///
/// `scope` must be `"user"` or `"workspace"`.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_load(
    state: State<'_, Arc<SettingsStore>>,
    path: String,
    scope: String,
) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    match scope.as_str() {
        "user" => state.load_user(p),
        "workspace" => state.load_workspace(p),
        _ => Err(format!("invalid scope '{scope}': expected \"user\" or \"workspace\"")),
    }
}

/// Parse a JSONC string (strips comments & trailing commas) and return the
/// resulting JSON value. Useful for the frontend to preview or validate
/// settings files.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_parse_jsonc(input: String) -> Result<Value, String> {
    parse_jsonc(&input).map_err(|e| e.to_string())
}

/// Edit a value inside a JSONC document by key-path, preserving surrounding
/// comments and formatting. Returns the modified JSONC string.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn settings_modify_jsonc(
    input: String,
    path: Vec<String>,
    value: Value,
) -> Result<String, String> {
    let refs: Vec<&str> = path.iter().map(String::as_str).collect();
    modify_jsonc(&input, &refs, &value).map_err(|e| e.to_string())
}
