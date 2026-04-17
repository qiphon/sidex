//! Structured logging — multi-channel log system modelled on VS Code's Output
//! panel.
//!
//! Each subsystem writes to a named *channel* (e.g. `"SideX"`, `"Git"`,
//! `"LSP"`). Entries are buffered in-memory, flushed to daily-rotating files,
//! and can be queried by the Output panel UI.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration constants
// ---------------------------------------------------------------------------

/// How many days of logs to retain.
const LOG_RETENTION_DAYS: u64 = 30;

/// Maximum in-memory entries per channel before forced flush.
const MAX_CHANNEL_BUFFER: usize = 2000;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Severity level for log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warning = 3,
    Error = 4,
    Critical = 5,
    Off = 6,
}

impl LogLevel {
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" | "warning" => Self::Warning,
            "error" => Self::Error,
            "critical" | "fatal" => Self::Critical,
            "off" => Self::Off,
            _ => Self::Info,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warning => "WARN",
            Self::Error => "ERROR",
            Self::Critical => "CRIT",
            Self::Off => "OFF",
        }
    }
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

/// Where log entries are written.
#[derive(Debug, Clone)]
pub enum LogOutput {
    /// Standard error (console).
    Console,
    /// Daily-rotating file under a directory.
    File(PathBuf),
    /// Named output channel (stored in memory, shown in the Output panel).
    OutputChannel(String),
}

/// A single structured log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// When the entry was created.
    pub timestamp: String,
    /// Severity.
    pub level: LogLevel,
    /// Emitting subsystem / channel name.
    pub source: String,
    /// Human-readable message.
    pub message: String,
    /// Optional structured data payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// LogService
// ---------------------------------------------------------------------------

/// Central logging service that manages multiple output channels.
pub struct LogService {
    level: LogLevel,
    outputs: Vec<LogOutput>,
    channel_logs: HashMap<String, Vec<LogEntry>>,
    log_dir: PathBuf,
    initialised: AtomicBool,
}

impl LogService {
    /// Initialise the logging service.
    pub fn init(level: LogLevel, log_dir: &Path) -> Result<Self> {
        fs::create_dir_all(log_dir).context("create log dir")?;
        Ok(Self {
            level,
            outputs: vec![LogOutput::Console, LogOutput::File(log_dir.to_path_buf())],
            channel_logs: HashMap::new(),
            log_dir: log_dir.to_path_buf(),
            initialised: AtomicBool::new(true),
        })
    }

    /// The current global log level.
    pub fn level(&self) -> LogLevel {
        self.level
    }

    /// Change the minimum log level at runtime (e.g. from the command
    /// palette "Developer: Set Log Level").
    pub fn set_level(&mut self, level: LogLevel) {
        self.level = level;
    }

    /// Configured output sinks.
    pub fn outputs(&self) -> &[LogOutput] {
        &self.outputs
    }

    /// Add an output target.
    pub fn add_output(&mut self, output: LogOutput) {
        self.outputs.push(output);
    }

    /// Write a log entry. Entries below the current level are silently
    /// discarded.
    pub fn log(
        &mut self,
        level: LogLevel,
        source: &str,
        message: &str,
        data: Option<serde_json::Value>,
    ) {
        if level < self.level || self.level == LogLevel::Off {
            return;
        }

        let entry = LogEntry {
            timestamp: iso_now(),
            level,
            source: source.into(),
            message: message.into(),
            data,
        };

        // Write to configured outputs.
        for output in &self.outputs {
            match output {
                LogOutput::Console => write_console(&entry),
                LogOutput::File(_) => {
                    if let Err(e) = write_file(&entry, &self.log_dir) {
                        eprintln!("log file write error: {e}");
                    }
                }
                LogOutput::OutputChannel(_) => {}
            }
        }

        // Buffer in the channel map (for the Output panel).
        let buf = self
            .channel_logs
            .entry(source.into())
            .or_insert_with(Vec::new);
        buf.push(entry);
        if buf.len() > MAX_CHANNEL_BUFFER {
            buf.drain(..MAX_CHANNEL_BUFFER / 2);
        }
    }

    // Convenience helpers matching VS Code's `window.createOutputChannel`.

    pub fn trace(&mut self, source: &str, message: &str) {
        self.log(LogLevel::Trace, source, message, None);
    }

    pub fn debug(&mut self, source: &str, message: &str) {
        self.log(LogLevel::Debug, source, message, None);
    }

    pub fn info(&mut self, source: &str, message: &str) {
        self.log(LogLevel::Info, source, message, None);
    }

    pub fn warn(&mut self, source: &str, message: &str) {
        self.log(LogLevel::Warning, source, message, None);
    }

    pub fn error(&mut self, source: &str, message: &str) {
        self.log(LogLevel::Error, source, message, None);
    }

    pub fn critical(&mut self, source: &str, message: &str) {
        self.log(LogLevel::Critical, source, message, None);
    }

    /// Return all buffered entries for a given channel.
    pub fn get_channel_log(&self, channel: &str) -> &[LogEntry] {
        self.channel_logs
            .get(channel)
            .map_or(&[], Vec::as_slice)
    }

    /// List all known channel names (for the Output panel dropdown).
    pub fn channels(&self) -> Vec<&str> {
        self.channel_logs.keys().map(String::as_str).collect()
    }

    /// Clear the in-memory buffer for a channel.
    pub fn clear_channel(&mut self, channel: &str) {
        if let Some(buf) = self.channel_logs.get_mut(channel) {
            buf.clear();
        }
    }

    /// Total entries buffered across all channels.
    pub fn total_entries(&self) -> usize {
        self.channel_logs.values().map(Vec::len).sum()
    }

    /// Is the service initialised?
    pub fn is_initialised(&self) -> bool {
        self.initialised.load(Ordering::Relaxed)
    }

    /// The directory where log files are stored.
    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }

    /// Rotate logs: delete daily directories older than `LOG_RETENTION_DAYS`.
    pub fn rotate(&self) -> Result<()> {
        rotate_logs(&self.log_dir, Duration::from_secs(LOG_RETENTION_DAYS * 86400))
    }
}

/// Convenience constructor used during startup.
pub fn init_logging(level: LogLevel, log_dir: &Path) -> Result<LogService> {
    LogService::init(level, log_dir)
}

// ---------------------------------------------------------------------------
// Well-known channel names
// ---------------------------------------------------------------------------

pub mod channels {
    pub const MAIN: &str = "SideX";
    pub const EXTENSION_HOST: &str = "Extension Host";
    pub const GIT: &str = "Git";
    pub const LSP: &str = "LSP";
    pub const TERMINAL: &str = "Terminal";
    pub const REMOTE: &str = "Remote";
    pub const TASKS: &str = "Tasks";
    pub const DEBUG: &str = "Debug Console";
}

// ---------------------------------------------------------------------------
// Output writers
// ---------------------------------------------------------------------------

fn write_console(entry: &LogEntry) {
    let level = entry.level.label();
    eprintln!(
        "[{} {level:<5}] [{}] {}",
        &entry.timestamp[..19.min(entry.timestamp.len())],
        entry.source,
        entry.message,
    );
}

fn write_file(entry: &LogEntry, log_dir: &Path) -> Result<()> {
    let date = &entry.timestamp[..10.min(entry.timestamp.len())];
    let day_dir = log_dir.join(date);
    fs::create_dir_all(&day_dir).context("create daily log dir")?;

    let path = day_dir.join("main.log");
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .context("open log file")?;

    let level = entry.level.label();
    writeln!(
        f,
        "[{} {level:<5}] [{}] {}",
        entry.timestamp, entry.source, entry.message,
    )
    .context("write log line")?;

    if let Some(ref data) = entry.data {
        writeln!(f, "  data: {data}").context("write log data")?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Log rotation
// ---------------------------------------------------------------------------

/// Delete daily log directories older than `max_age`.
pub fn rotate_logs(log_dir: &Path, max_age: Duration) -> Result<()> {
    if !log_dir.exists() {
        return Ok(());
    }

    let now = SystemTime::now();
    let mut removed = 0u32;

    for entry in fs::read_dir(log_dir).context("read log dir")? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age > max_age {
                        let _ = fs::remove_dir_all(&path);
                        removed += 1;
                    }
                }
            }
        }
    }

    if removed > 0 {
        log::debug!("rotated {removed} old log directories");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn iso_now() -> String {
    humantime::format_rfc3339_seconds(SystemTime::now()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_from_str() {
        assert_eq!(LogLevel::from_str_loose("trace"), LogLevel::Trace);
        assert_eq!(LogLevel::from_str_loose("debug"), LogLevel::Debug);
        assert_eq!(LogLevel::from_str_loose("info"), LogLevel::Info);
        assert_eq!(LogLevel::from_str_loose("warn"), LogLevel::Warning);
        assert_eq!(LogLevel::from_str_loose("warning"), LogLevel::Warning);
        assert_eq!(LogLevel::from_str_loose("error"), LogLevel::Error);
        assert_eq!(LogLevel::from_str_loose("critical"), LogLevel::Critical);
        assert_eq!(LogLevel::from_str_loose("off"), LogLevel::Off);
        assert_eq!(LogLevel::from_str_loose("garbage"), LogLevel::Info);
    }

    #[test]
    fn level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warning);
        assert!(LogLevel::Warning < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Critical);
        assert!(LogLevel::Critical < LogLevel::Off);
    }

    #[test]
    fn init_creates_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("logs");
        let svc = init_logging(LogLevel::Info, &dir).unwrap();
        assert!(dir.exists());
        assert!(svc.is_initialised());
    }

    #[test]
    fn log_entry_buffered() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = init_logging(LogLevel::Trace, tmp.path()).unwrap();
        svc.info("Git", "pull completed");
        svc.warn("LSP", "server slow");

        assert_eq!(svc.get_channel_log("Git").len(), 1);
        assert_eq!(svc.get_channel_log("LSP").len(), 1);
        assert_eq!(svc.get_channel_log("Unknown").len(), 0);
        assert_eq!(svc.total_entries(), 2);
    }

    #[test]
    fn below_level_discarded() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = init_logging(LogLevel::Warning, tmp.path()).unwrap();
        svc.trace("SideX", "ignored");
        svc.debug("SideX", "ignored");
        svc.info("SideX", "ignored");
        svc.warn("SideX", "kept");
        svc.error("SideX", "kept");
        assert_eq!(svc.total_entries(), 2);
    }

    #[test]
    fn off_discards_all() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = init_logging(LogLevel::Off, tmp.path()).unwrap();
        svc.critical("SideX", "nope");
        assert_eq!(svc.total_entries(), 0);
    }

    #[test]
    fn set_level_runtime() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = init_logging(LogLevel::Error, tmp.path()).unwrap();
        svc.info("SideX", "hidden");
        assert_eq!(svc.total_entries(), 0);
        svc.set_level(LogLevel::Info);
        svc.info("SideX", "visible");
        assert_eq!(svc.total_entries(), 1);
    }

    #[test]
    fn channels_list() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = init_logging(LogLevel::Trace, tmp.path()).unwrap();
        svc.info("Git", "hi");
        svc.info("LSP", "hi");
        let chans = svc.channels();
        assert!(chans.contains(&"Git"));
        assert!(chans.contains(&"LSP"));
    }

    #[test]
    fn clear_channel_works() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = init_logging(LogLevel::Trace, tmp.path()).unwrap();
        svc.info("Git", "a");
        svc.info("Git", "b");
        svc.clear_channel("Git");
        assert_eq!(svc.get_channel_log("Git").len(), 0);
    }

    #[test]
    fn log_writes_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut svc = init_logging(LogLevel::Trace, tmp.path()).unwrap();
        svc.info("SideX", "hello world");

        // The file should exist under a date-stamped directory.
        let mut found = false;
        for entry in fs::read_dir(tmp.path()).unwrap() {
            let entry = entry.unwrap();
            if entry.path().is_dir() {
                let log_file = entry.path().join("main.log");
                if log_file.exists() {
                    let contents = fs::read_to_string(&log_file).unwrap();
                    assert!(contents.contains("hello world"));
                    found = true;
                }
            }
        }
        assert!(found, "log file not found");
    }

    #[test]
    fn rotate_nonexistent() {
        assert!(rotate_logs(Path::new("/nonexistent"), Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn buffer_truncation() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Use Console-only output to avoid file I/O overhead.
        let mut svc = LogService {
            level: LogLevel::Trace,
            outputs: Vec::new(),
            channel_logs: HashMap::new(),
            log_dir: tmp.path().to_path_buf(),
            initialised: AtomicBool::new(true),
        };

        for i in 0..MAX_CHANNEL_BUFFER + 100 {
            svc.info("flood", &format!("msg {i}"));
        }
        let len = svc.get_channel_log("flood").len();
        assert!(len <= MAX_CHANNEL_BUFFER, "buffer should have been trimmed");
    }

    #[test]
    fn log_entry_roundtrip() {
        let entry = LogEntry {
            timestamp: iso_now(),
            level: LogLevel::Info,
            source: "Git".into(),
            message: "pull".into(),
            data: Some(serde_json::json!({"branch": "main"})),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: LogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.source, "Git");
        assert!(parsed.data.is_some());
    }
}
