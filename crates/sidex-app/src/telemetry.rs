//! Telemetry — opt-in anonymous usage data collection.
//!
//! **No data is ever sent** unless the user explicitly sets
//! `telemetry.telemetryLevel` to a value other than `Off`. Events are
//! accumulated locally in the SQLite state database and can be exported
//! by the user at any time.

use std::collections::HashMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// Controls which telemetry events are recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TelemetryLevel {
    /// No telemetry at all (default).
    Off,
    /// Crash reports only.
    Crash,
    /// Crash + error events.
    Error,
    /// Everything: crashes, errors, usage events.
    All,
}

impl TelemetryLevel {
    /// Parse from the `telemetry.telemetryLevel` setting string.
    pub fn from_setting(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "crash" => Self::Crash,
            "error" => Self::Error,
            "all" => Self::All,
            _ => Self::Off,
        }
    }
}

impl Default for TelemetryLevel {
    fn default() -> Self {
        Self::Off
    }
}

/// A single recorded telemetry event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Machine-readable event name (e.g. `"editor.opened"`).
    pub event_name: String,
    /// Flat set of properties attached to the event.
    pub properties: HashMap<String, String>,
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// Classification: `"usage"`, `"error"`, or `"crash"`.
    pub classification: String,
}

/// Collects and stores telemetry events locally.
pub struct TelemetryService {
    level: TelemetryLevel,
    events: Vec<TelemetryEvent>,
    session_id: String,
    machine_id: String,
}

impl TelemetryService {
    /// Create a new service with the given level.
    pub fn new(level: TelemetryLevel) -> Self {
        Self {
            level,
            events: Vec::new(),
            session_id: generate_id(),
            machine_id: machine_id(),
        }
    }

    /// The current telemetry level.
    pub fn level(&self) -> TelemetryLevel {
        self.level
    }

    /// Change the telemetry level (mirrors the user setting).
    pub fn set_level(&mut self, level: TelemetryLevel) {
        self.level = level;
        if level == TelemetryLevel::Off {
            log::info!("telemetry disabled");
        }
    }

    /// Whether any telemetry is being recorded.
    pub fn is_enabled(&self) -> bool {
        self.level != TelemetryLevel::Off
    }

    /// Log a general usage event. Only recorded when level is `All`.
    pub fn log_event(&mut self, event_name: &str, properties: &HashMap<String, String>) {
        if self.level != TelemetryLevel::All {
            return;
        }
        self.record(event_name, properties, "usage");
    }

    /// Log an error event. Recorded when level is `Error` or `All`.
    pub fn log_error(&mut self, error_name: &str, message: &str) {
        if self.level == TelemetryLevel::Off || self.level == TelemetryLevel::Crash {
            return;
        }
        let mut props = HashMap::new();
        props.insert("message".into(), message.into());
        self.record(error_name, &props, "error");
    }

    /// Log a crash event. Recorded at any level except `Off`.
    pub fn log_crash(&mut self, error_name: &str, message: &str) {
        if self.level == TelemetryLevel::Off {
            return;
        }
        let mut props = HashMap::new();
        props.insert("message".into(), message.into());
        self.record(error_name, &props, "crash");
    }

    /// Return all locally stored events (for export / review by the user).
    pub fn events(&self) -> &[TelemetryEvent] {
        &self.events
    }

    /// Export all events as a JSON string.
    pub fn export_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.events)
    }

    /// Clear all stored events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Total number of stored events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Persist events to the SQLite database.
    ///
    /// Writes each un-persisted event as a JSON blob into the `telemetry_events`
    /// table, then clears the in-memory buffer.
    pub fn flush_to_db(&mut self, db: &sidex_db::Database) -> anyhow::Result<()> {
        if self.events.is_empty() {
            return Ok(());
        }

        let tx = db.conn();
        for event in &self.events {
            let json = serde_json::to_string(event)?;
            tx.execute(
                "INSERT INTO telemetry_events (event_name, classification, payload, timestamp)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    event.event_name,
                    event.classification,
                    json,
                    event.timestamp,
                ],
            )?;
        }
        self.events.clear();
        Ok(())
    }

    /// The unique session ID (generated at startup).
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    fn record(
        &mut self,
        event_name: &str,
        properties: &HashMap<String, String>,
        classification: &str,
    ) {
        let mut props = properties.clone();
        props.insert("sessionId".into(), self.session_id.clone());
        props.insert("machineId".into(), self.machine_id.clone());

        self.events.push(TelemetryEvent {
            event_name: event_name.into(),
            properties: props,
            timestamp: iso_now(),
            classification: classification.into(),
        });
    }
}

impl Default for TelemetryService {
    fn default() -> Self {
        Self::new(TelemetryLevel::Off)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn iso_now() -> String {
    humantime::format_rfc3339_seconds(SystemTime::now()).to_string()
}

fn generate_id() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    SystemTime::now().hash(&mut hasher);
    std::process::id().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn machine_id() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if let Ok(name) = hostname::get() {
        name.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}

// ---------------------------------------------------------------------------
// Well-known event names
// ---------------------------------------------------------------------------

/// Standard event names matching VS Code telemetry conventions.
pub mod events {
    pub const EDITOR_OPENED: &str = "editor.opened";
    pub const FILE_TYPE: &str = "editor.fileType";
    pub const EXTENSION_INSTALLED: &str = "extension.installed";
    pub const EXTENSION_UNINSTALLED: &str = "extension.uninstalled";
    pub const COMMAND_EXECUTED: &str = "command.executed";
    pub const FEATURE_USED: &str = "feature.used";
    pub const WORKSPACE_OPENED: &str = "workspace.opened";
    pub const TERMINAL_CREATED: &str = "terminal.created";
    pub const DEBUG_SESSION_STARTED: &str = "debug.sessionStarted";
    pub const SEARCH_PERFORMED: &str = "search.performed";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_by_default() {
        let svc = TelemetryService::default();
        assert_eq!(svc.level(), TelemetryLevel::Off);
        assert!(!svc.is_enabled());
    }

    #[test]
    fn events_not_recorded_when_off() {
        let mut svc = TelemetryService::new(TelemetryLevel::Off);
        svc.log_event("test", &HashMap::new());
        svc.log_error("err", "msg");
        svc.log_crash("crash", "msg");
        assert_eq!(svc.event_count(), 0);
    }

    #[test]
    fn crash_level_records_crashes_only() {
        let mut svc = TelemetryService::new(TelemetryLevel::Crash);
        svc.log_event("test", &HashMap::new());
        svc.log_error("err", "msg");
        svc.log_crash("crash", "msg");
        assert_eq!(svc.event_count(), 1);
        assert_eq!(svc.events()[0].classification, "crash");
    }

    #[test]
    fn error_level_records_errors_and_crashes() {
        let mut svc = TelemetryService::new(TelemetryLevel::Error);
        svc.log_event("test", &HashMap::new());
        svc.log_error("err", "msg");
        svc.log_crash("crash", "msg");
        assert_eq!(svc.event_count(), 2);
    }

    #[test]
    fn all_level_records_everything() {
        let mut svc = TelemetryService::new(TelemetryLevel::All);
        svc.log_event("test", &HashMap::new());
        svc.log_error("err", "msg");
        svc.log_crash("crash", "msg");
        assert_eq!(svc.event_count(), 3);
    }

    #[test]
    fn level_from_setting() {
        assert_eq!(TelemetryLevel::from_setting("off"), TelemetryLevel::Off);
        assert_eq!(TelemetryLevel::from_setting("crash"), TelemetryLevel::Crash);
        assert_eq!(TelemetryLevel::from_setting("error"), TelemetryLevel::Error);
        assert_eq!(TelemetryLevel::from_setting("all"), TelemetryLevel::All);
        assert_eq!(TelemetryLevel::from_setting("garbage"), TelemetryLevel::Off);
    }

    #[test]
    fn export_json_roundtrip() {
        let mut svc = TelemetryService::new(TelemetryLevel::All);
        let mut props = HashMap::new();
        props.insert("lang".into(), "rust".into());
        svc.log_event("editor.opened", &props);

        let json = svc.export_json().unwrap();
        let parsed: Vec<TelemetryEvent> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].event_name, "editor.opened");
    }

    #[test]
    fn clear_removes_all() {
        let mut svc = TelemetryService::new(TelemetryLevel::All);
        svc.log_event("a", &HashMap::new());
        svc.log_event("b", &HashMap::new());
        assert_eq!(svc.event_count(), 2);
        svc.clear();
        assert_eq!(svc.event_count(), 0);
    }

    #[test]
    fn set_level_works() {
        let mut svc = TelemetryService::new(TelemetryLevel::Off);
        svc.set_level(TelemetryLevel::All);
        assert!(svc.is_enabled());
        svc.log_event("test", &HashMap::new());
        assert_eq!(svc.event_count(), 1);
    }

    #[test]
    fn session_id_is_stable() {
        let svc = TelemetryService::new(TelemetryLevel::Off);
        let id = svc.session_id().to_owned();
        assert_eq!(id.len(), 16);
        assert_eq!(svc.session_id(), id);
    }
}
