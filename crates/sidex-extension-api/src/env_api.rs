//! `vscode.env` API compatibility shim.
//!
//! Provides app identity, locale, clipboard, shell, telemetry, and UI kind
//! information to extensions.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiKind {
    Desktop = 1,
    Web = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Off = 0,
    Trace = 1,
    Debug = 2,
    Info = 3,
    Warning = 4,
    Error = 5,
}

/// Implements the `vscode.env.*` API surface.
pub struct EnvApi {
    app_name: String,
    app_root: String,
    app_host: String,
    language: RwLock<String>,
    machine_id: String,
    session_id: String,
    ui_kind: UiKind,
    log_level: RwLock<LogLevel>,
    is_telemetry_enabled: RwLock<bool>,
    clipboard_text: RwLock<String>,
    shell: String,
    uri_scheme: String,
    is_new_app_install: bool,
}

impl EnvApi {
    pub fn new() -> Self {
        Self {
            app_name: "SideX".into(),
            app_root: std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .into(),
            app_host: "desktop".into(),
            language: RwLock::new("en".into()),
            machine_id: uuid::Uuid::new_v4().to_string(),
            session_id: uuid::Uuid::new_v4().to_string(),
            ui_kind: UiKind::Desktop,
            log_level: RwLock::new(LogLevel::Info),
            is_telemetry_enabled: RwLock::new(false),
            clipboard_text: RwLock::new(String::new()),
            shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()),
            uri_scheme: "sidex".into(),
            is_new_app_install: false,
        }
    }

    pub fn handle(&self, action: &str, params: &Value) -> Result<Value> {
        match action {
            "appName" => Ok(Value::String(self.app_name.clone())),
            "appRoot" => Ok(Value::String(self.app_root.clone())),
            "appHost" => Ok(Value::String(self.app_host.clone())),
            "language" => Ok(Value::String(self.language.read().expect("lock").clone())),
            "machineId" => Ok(Value::String(self.machine_id.clone())),
            "sessionId" => Ok(Value::String(self.session_id.clone())),
            "uiKind" => Ok(serde_json::to_value(self.ui_kind)?),
            "logLevel" => Ok(serde_json::to_value(*self.log_level.read().expect("lock"))?),
            "isTelemetryEnabled" => Ok(Value::Bool(
                *self.is_telemetry_enabled.read().expect("lock"),
            )),
            "shell" => Ok(Value::String(self.shell.clone())),
            "uriScheme" => Ok(Value::String(self.uri_scheme.clone())),
            "isNewAppInstall" => Ok(Value::Bool(self.is_new_app_install)),
            "clipboard/readText" => Ok(Value::String(
                self.clipboard_text.read().expect("lock").clone(),
            )),
            "clipboard/writeText" => {
                let text = params.get("text").and_then(Value::as_str).unwrap_or("");
                text.clone_into(&mut self.clipboard_text.write().expect("lock"));
                Ok(Value::Null)
            }
            "openExternal" => {
                let _uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                log::debug!("[ext] env.openExternal");
                Ok(Value::Bool(true))
            }
            "asExternalUri" => {
                let uri = params.get("uri").and_then(Value::as_str).unwrap_or("");
                Ok(Value::String(uri.to_owned()))
            }
            _ => bail!("unknown env action: {action}"),
        }
    }

    pub fn set_language(&self, lang: &str) {
        lang.clone_into(&mut self.language.write().expect("lock"));
    }
    pub fn set_log_level(&self, level: LogLevel) {
        *self.log_level.write().expect("lock") = level;
    }
    pub fn set_telemetry_enabled(&self, enabled: bool) {
        *self.is_telemetry_enabled.write().expect("lock") = enabled;
    }
}

impl Default for EnvApi {
    fn default() -> Self {
        Self::new()
    }
}
