use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const DEFAULT_MAX_LOG_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
const DEFAULT_MAX_LOG_FILES: usize = 5;

pub struct LoggerStore {
    loggers: Mutex<HashMap<String, LoggerState>>,
    counter: Mutex<u32>,
}

struct LoggerState {
    filepath: PathBuf,
    level: u32,
    rotating: bool,
    max_size: u64,
    max_files: usize,
    use_formatters: bool,
}

impl LoggerState {
    fn rotate_if_needed(&self) -> Result<(), String> {
        if !self.rotating {
            return Ok(());
        }

        // Check if file exists and exceeds size limit
        if self.filepath.exists() {
            let metadata = fs::metadata(&self.filepath)
                .map_err(|e| format!("Failed to get log file metadata: {e}"))?;
            
            if metadata.len() >= self.max_size {
                self.rotate_logs()?;
            }
        }
        Ok(())
    }

    fn rotate_logs(&self) -> Result<(), String> {
        // Remove oldest log if we've hit the limit
        let oldest_log = self.filepath.with_extension(&format!("log.{}", self.max_files));
        if oldest_log.exists() {
            fs::remove_file(&oldest_log)
                .map_err(|e| format!("Failed to remove oldest log: {e}"))?;
        }

        // Rotate existing logs: log.4 -> log.5, log.3 -> log.4, etc.
        for i in (1..self.max_files).rev() {
            let current = self.filepath.with_extension(&format!("log.{i}"));
            let next = self.filepath.with_extension(&format!("log.{}", i + 1));
            if current.exists() {
                fs::rename(&current, &next)
                    .map_err(|e| format!("Failed to rotate log {i}: {e}"))?;
            }
        }

        // Move current log to log.1
        if self.filepath.exists() {
            let rotated = self.filepath.with_extension("log.1");
            fs::rename(&self.filepath, &rotated)
                .map_err(|e| format!("Failed to rotate current log: {e}"))?;
        }

        Ok(())
    }

    fn format_message(&self, level: u32, message: &str) -> String {
        if !self.use_formatters {
            return message.to_string();
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let level_str = match level {
            0 => "TRACE",
            1 => "DEBUG",
            2 => "INFO",
            3 => "WARN",
            4 => "ERROR",
            _ => "UNKNOWN",
        };
        format!("[{} {}] {}", timestamp, level_str, message)
    }
}

impl LoggerStore {
    pub fn new() -> Self {
        Self {
            loggers: Mutex::new(HashMap::new()),
            counter: Mutex::new(0),
        }
    }
}

impl Default for LoggerStore {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn log_create_logger(
    state: tauri::State<'_, std::sync::Arc<LoggerStore>>,
    name: String,
    filepath: String,
    rotating: bool,
    donot_use_formatters: bool,
    level: u32,
) -> Result<String, String> {
    let path = PathBuf::from(&filepath);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).ok();
    }

    let mut counter = state.counter.lock().map_err(|e| e.to_string())?;
    *counter += 1;
    let id = format!("log-{name}-{counter}");

    let mut loggers = state.loggers.lock().map_err(|e| e.to_string())?;
    loggers.insert(
        id.clone(),
        LoggerState {
            filepath: path,
            level,
            rotating,
            max_size: DEFAULT_MAX_LOG_SIZE,
            max_files: DEFAULT_MAX_LOG_FILES,
            use_formatters: !donot_use_formatters,
        },
    );

    Ok(id)
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn log_write(
    state: tauri::State<'_, std::sync::Arc<LoggerStore>>,
    logger_id: String,
    level: u32,
    message: String,
) -> Result<(), String> {
    let loggers = state.loggers.lock().map_err(|e| e.to_string())?;
    let logger = loggers.get(&logger_id).ok_or("logger not found")?;
    if level < logger.level {
        return Ok(());
    }

    // Rotate if needed before writing
    logger.rotate_if_needed()?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&logger.filepath)
        .map_err(|e| format!("open log: {e}"))?;

    let formatted_message = logger.format_message(level, &message);
    writeln!(file, "{formatted_message}").map_err(|e| format!("write log: {e}"))?;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn log_set_level(
    state: tauri::State<'_, std::sync::Arc<LoggerStore>>,
    logger_id: String,
    level: u32,
) -> Result<(), String> {
    let mut loggers = state.loggers.lock().map_err(|e| e.to_string())?;
    let logger = loggers.get_mut(&logger_id).ok_or("logger not found")?;
    logger.level = level;
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn log_flush(
    state: tauri::State<'_, std::sync::Arc<LoggerStore>>,
    logger_id: String,
) -> Result<(), String> {
    let loggers = state.loggers.lock().map_err(|e| e.to_string())?;
    let logger = loggers.get(&logger_id).ok_or("logger not found")?;
    
    if logger.filepath.exists() {
        let file = File::open(&logger.filepath)
            .map_err(|e| format!("Failed to open log file for flushing: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Failed to flush log file: {e}"))?;
    }
    
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn log_drop(
    state: tauri::State<'_, std::sync::Arc<LoggerStore>>,
    logger_id: String,
) -> Result<(), String> {
    let mut loggers = state.loggers.lock().map_err(|e| e.to_string())?;
    loggers.remove(&logger_id);
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn log_get_size(
    state: tauri::State<'_, std::sync::Arc<LoggerStore>>,
    logger_id: String,
) -> Result<u64, String> {
    let loggers = state.loggers.lock().map_err(|e| e.to_string())?;
    let logger = loggers.get(&logger_id).ok_or("logger not found")?;
    
    if logger.filepath.exists() {
        let metadata = fs::metadata(&logger.filepath)
            .map_err(|e| format!("Failed to get log file size: {e}"))?;
        Ok(metadata.len())
    } else {
        Ok(0)
    }
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn log_clear(
    state: tauri::State<'_, std::sync::Arc<LoggerStore>>,
    logger_id: String,
) -> Result<(), String> {
    let loggers = state.loggers.lock().map_err(|e| e.to_string())?;
    let logger = loggers.get(&logger_id).ok_or("logger not found")?;
    
    if logger.filepath.exists() {
        fs::write(&logger.filepath, "")
            .map_err(|e| format!("Failed to clear log file: {e}"))?;
    }
    
    Ok(())
}