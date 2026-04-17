//! PTY (pseudo-terminal) process management.
//!
//! Wraps the `portable-pty` crate to provide a higher-level interface for
//! spawning shell processes, writing input, reading output, and resizing.
//! Ported from `src-tauri/src/commands/terminal.rs` and `process.rs`,
//! removing all Tauri-specific dependencies and adding ring-buffer output,
//! process-tree cleanup, and comprehensive environment setup.

use crossbeam::channel::{self, Receiver, Sender};
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::shell;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_RING_BUFFER_CAPACITY: usize = 10_000;
const OUTPUT_CHANNEL_SIZE: usize = 1_000;
const PTY_READ_BUFFER_SIZE: usize = 8192;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Terminal dimensions in rows and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

/// Errors that can occur during PTY operations.
#[derive(Debug, Error)]
pub enum PtyError {
    #[error("failed to open PTY: {0}")]
    Open(String),
    #[error("failed to spawn process: {0}")]
    Spawn(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to resize PTY: {0}")]
    Resize(String),
    #[error("PTY process is no longer running")]
    NotRunning,
    #[error("lock poisoned")]
    LockPoisoned,
    #[error("signal not allowed: {0}")]
    SignalNotAllowed(i32),
}

pub type PtyResult<T> = Result<T, PtyError>;

/// Unique handle for a terminal instance, atomically generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TermHandle(pub u32);

impl TermHandle {
    pub fn next() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(1);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

/// A single chunk of terminal output.
#[derive(Debug, Clone, Serialize)]
pub struct OutputChunk {
    pub text: String,
    pub is_stderr: bool,
    #[allow(clippy::cast_possible_truncation)]
    pub timestamp_ms: u64,
}

impl OutputChunk {
    fn now(text: String, is_stderr: bool) -> Self {
        #[allow(clippy::cast_possible_truncation)]
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            text,
            is_stderr,
            timestamp_ms,
        }
    }
}

/// Information about a running terminal.
#[derive(Debug, Clone, Serialize)]
pub struct TermInfo {
    pub handle: TermHandle,
    pub shell: String,
    pub cwd: String,
    pub pid: u32,
    pub cols: u16,
    pub rows: u16,
    pub is_alive: bool,
    pub output_lines_total: usize,
}

// ---------------------------------------------------------------------------
// Ring buffer
// ---------------------------------------------------------------------------

/// Ring buffer for terminal output with overflow tracking.
pub(crate) struct RingBuffer {
    capacity: usize,
    buffer: std::collections::VecDeque<OutputChunk>,
    dropped_count: usize,
    total_count: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: std::collections::VecDeque::with_capacity(capacity),
            dropped_count: 0,
            total_count: 0,
        }
    }

    pub fn push(&mut self, chunk: OutputChunk) {
        self.total_count += 1;
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
            self.dropped_count += 1;
        }
        self.buffer.push_back(chunk);
    }

    pub fn get_lines(&self, max: Option<usize>) -> Vec<OutputChunk> {
        let n = max.unwrap_or(100).min(self.buffer.len());
        self.buffer.iter().rev().take(n).rev().cloned().collect()
    }

    pub fn take_dropped_count(&mut self) -> usize {
        let c = self.dropped_count;
        self.dropped_count = 0;
        c
    }

    pub fn total_count(&self) -> usize {
        self.total_count
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.dropped_count = 0;
        self.total_count = 0;
    }
}

// ---------------------------------------------------------------------------
// Output channel
// ---------------------------------------------------------------------------

pub(crate) enum OutputMessage {
    Data(OutputChunk),
    Shutdown,
}

fn spawn_output_reader(
    mut reader: Box<dyn Read + Send>,
    sender: Sender<OutputMessage>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("pty-reader".to_string())
        .spawn(move || {
            let mut buf = [0u8; PTY_READ_BUFFER_SIZE];
            let mut consecutive_errors = 0u32;
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        let _ = sender.send(OutputMessage::Shutdown);
                        break;
                    }
                    Ok(n) => {
                        consecutive_errors = 0;
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        if sender
                            .send(OutputMessage::Data(OutputChunk::now(text, false)))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        consecutive_errors += 1;
                        if consecutive_errors > 5 {
                            let _ = sender.send(OutputMessage::Shutdown);
                            break;
                        }
                        let err_text = format!("\r\n[Terminal read error: {e}]\r\n");
                        let _ = sender.send(OutputMessage::Data(OutputChunk::now(err_text, true)));
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
        })
        .expect("failed to spawn pty-reader thread")
}

// ---------------------------------------------------------------------------
// Environment helpers
// ---------------------------------------------------------------------------

fn home_dir_string() -> Option<String> {
    if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE")
            .ok()
            .or_else(|| std::env::var("HOME").ok())
    } else {
        std::env::var("HOME").ok()
    }
}

fn setup_environment(cmd: &mut CommandBuilder, extra_env: &HashMap<String, String>) {
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "SideX");

    if cfg!(target_os = "windows") {
        for key in [
            "PATH",
            "USERPROFILE",
            "USERNAME",
            "APPDATA",
            "LOCALAPPDATA",
            "HOMEDRIVE",
            "HOMEPATH",
            "COMSPEC",
            "SystemRoot",
            "HOME",
            "TEMP",
            "TMP",
        ] {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }
    } else {
        for key in ["HOME", "USER", "PATH", "LANG", "SHELL"] {
            if let Ok(val) = std::env::var(key) {
                cmd.env(key, val);
            }
        }
    }

    if std::env::var("LANG").is_err() && !cfg!(target_os = "windows") {
        cmd.env("LANG", "en_US.UTF-8");
    }

    for (k, v) in extra_env {
        cmd.env(k, v);
    }
}

fn resolve_cwd(cwd: Option<&Path>) -> PathBuf {
    if let Some(dir) = cwd {
        if dir.is_dir() {
            return dir.to_path_buf();
        }
    }
    if let Some(home) = home_dir_string() {
        let p = PathBuf::from(&home);
        if p.is_dir() {
            return p;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn login_args_for_shell(shell_path: &str) -> Vec<String> {
    let basename = Path::new(shell_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    match basename {
        "zsh" | "bash" | "sh" | "fish" => vec!["-l".to_string()],
        "powershell.exe" | "pwsh.exe" => vec!["-NoExit".to_string()],
        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Process tree kill
// ---------------------------------------------------------------------------

/// Kill a process and all of its children.
#[cfg(unix)]
#[allow(unsafe_code, clippy::cast_possible_wrap)]
pub fn kill_process_tree(pid: u32) -> PtyResult<()> {
    use std::process::Command;

    if let Ok(output) = Command::new("pgrep")
        .args(["-P", &pid.to_string()])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Ok(child_pid) = line.trim().parse::<u32>() {
                let _ = kill_process_tree(child_pid);
            }
        }
    }

    unsafe {
        let result = libc::kill(pid as i32, libc::SIGTERM);
        if result != 0 {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn kill_process_tree(pid: u32) -> PtyResult<()> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let result = Command::new("taskkill")
        .args(&["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output();

    match result {
        Ok(output) if !output.status.success() => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(PtyError::Io(std::io::Error::other(format!(
                "taskkill failed: {stderr}"
            ))))
        }
        Err(e) => Err(PtyError::Io(e)),
        _ => Ok(()),
    }
}

#[cfg(not(any(unix, windows)))]
pub fn kill_process_tree(_pid: u32) -> PtyResult<()> {
    Ok(())
}

/// Send a signal to a process (Unix only).
#[cfg(unix)]
pub fn send_signal(pid: u32, signal: i32) -> PtyResult<()> {
    const ALLOWED: &[i32] = &[2, 9, 15, 18, 19]; // INT, KILL, TERM, CONT, STOP
    if !ALLOWED.contains(&signal) {
        return Err(PtyError::SignalNotAllowed(signal));
    }
    #[allow(unsafe_code, clippy::cast_possible_wrap)]
    unsafe {
        let result = libc::kill(pid as i32, signal);
        if result != 0 {
            return Err(PtyError::Io(std::io::Error::last_os_error()));
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn send_signal(_pid: u32, _signal: i32) -> PtyResult<()> {
    Err(PtyError::Io(std::io::Error::other(
        "signals only supported on Unix",
    )))
}

// ---------------------------------------------------------------------------
// PtyProcess
// ---------------------------------------------------------------------------

/// Configuration for spawning a PTY process.
#[derive(Debug, Clone, Default)]
pub struct PtySpawnConfig {
    pub shell: Option<String>,
    pub args: Option<Vec<String>>,
    pub cwd: Option<PathBuf>,
    pub env: HashMap<String, String>,
    pub size: TerminalSize,
}

impl PtySpawnConfig {
    /// Sets an environment variable to be passed to the spawned process.
    pub fn set_env(&mut self, key: &str, value: &str) {
        self.env.insert(key.to_string(), value.to_string());
    }
}

/// A managed pseudo-terminal process with ring-buffer output and
/// event channel.
pub struct PtyProcess {
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Arc<Mutex<Box<dyn portable_pty::Child + Send>>>,
    alive: Arc<AtomicBool>,
    output: Arc<Mutex<RingBuffer>>,
    output_rx: Option<Receiver<OutputMessage>>,
    _output_tx: Sender<OutputMessage>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
    shell: String,
    cwd: PathBuf,
    cols: u16,
    rows: u16,
    title: Arc<Mutex<Option<String>>>,
}

impl PtyProcess {
    /// Spawns a new PTY process with the given configuration.
    pub fn spawn(config: &PtySpawnConfig) -> PtyResult<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: config.size.rows,
                cols: config.size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Open(e.to_string()))?;

        let shell_path = config
            .shell
            .clone()
            .unwrap_or_else(shell::detect_default_shell);

        // Validate shell exists on Unix
        if !cfg!(target_os = "windows") {
            let p = Path::new(&shell_path);
            if !p.exists() {
                let fallback = shell::detect_default_shell();
                if !Path::new(&fallback).exists() {
                    return Err(PtyError::Spawn(format!(
                        "shell '{shell_path}' not found and no fallback available"
                    )));
                }
            }
        }

        let mut cmd = CommandBuilder::new(&shell_path);

        if let Some(ref shell_args) = config.args {
            for arg in shell_args {
                cmd.arg(arg);
            }
        } else {
            for arg in login_args_for_shell(&shell_path) {
                cmd.arg(arg);
            }
        }

        setup_environment(&mut cmd, &config.env);

        let work_dir = resolve_cwd(config.cwd.as_deref());
        cmd.cwd(&work_dir);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| PtyError::Spawn(format!("'{shell_path}': {e}")))?;

        drop(pair.slave);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| PtyError::Io(std::io::Error::other(e.to_string())))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| PtyError::Io(std::io::Error::other(e.to_string())))?;

        let output = Arc::new(Mutex::new(RingBuffer::new(DEFAULT_RING_BUFFER_CAPACITY)));
        let (tx, rx) = channel::bounded(OUTPUT_CHANNEL_SIZE);

        let reader_thread = spawn_output_reader(reader, tx.clone());

        Ok(Self {
            master: pair.master,
            writer: Arc::new(Mutex::new(writer)),
            child: Arc::new(Mutex::new(child)),
            alive: Arc::new(AtomicBool::new(true)),
            output,
            output_rx: Some(rx),
            _output_tx: tx,
            reader_thread: Some(reader_thread),
            shell: shell_path,
            cwd: work_dir,
            cols: config.size.cols,
            rows: config.size.rows,
            title: Arc::new(Mutex::new(None)),
        })
    }

    /// Spawns a PTY with a simple interface (backwards-compatible).
    pub fn spawn_simple(
        shell: &str,
        cwd: &Path,
        env: &[(String, String)],
        size: TerminalSize,
    ) -> PtyResult<Self> {
        let env_map: HashMap<String, String> = env.iter().cloned().collect();
        Self::spawn(&PtySpawnConfig {
            shell: Some(shell.to_string()),
            args: None,
            cwd: Some(cwd.to_path_buf()),
            env: env_map,
            size,
        })
    }

    /// Sends input bytes to the PTY.
    pub fn write(&self, data: &[u8]) -> PtyResult<()> {
        if !self.is_alive() {
            return Err(PtyError::NotRunning);
        }
        let mut writer = self.writer.lock().map_err(|_| PtyError::LockPoisoned)?;
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    /// Sends a string to the PTY.
    pub fn write_str(&self, data: &str) -> PtyResult<()> {
        self.write(data.as_bytes())
    }

    /// Resizes the PTY to the given dimensions.
    pub fn resize(&mut self, size: TerminalSize) -> PtyResult<()> {
        self.master
            .resize(PtySize {
                rows: size.rows,
                cols: size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| PtyError::Resize(e.to_string()))?;
        self.cols = size.cols;
        self.rows = size.rows;
        Ok(())
    }

    /// Kills the PTY child process.
    pub fn kill(&self) -> PtyResult<()> {
        self.alive.store(false, Ordering::SeqCst);
        let mut child = self.child.lock().map_err(|_| PtyError::LockPoisoned)?;
        child
            .kill()
            .map_err(|e| PtyError::Io(std::io::Error::other(e.to_string())))
    }

    /// Kills the entire process tree rooted at this PTY.
    pub fn kill_tree(&self) -> PtyResult<()> {
        if let Some(pid) = self.pid() {
            kill_process_tree(pid)?;
        }
        self.alive.store(false, Ordering::SeqCst);
        let _ = self.kill();
        Ok(())
    }

    /// Registers a callback that receives output bytes from the PTY.
    ///
    /// This takes the output receiver; only one handler can be active.
    /// The callback approach is kept for backwards compatibility with the
    /// emulator integration in the manager.
    pub fn on_output<F>(&mut self, handler: F) -> PtyResult<()>
    where
        F: Fn(&[u8]) + Send + 'static,
    {
        let rx = self
            .output_rx
            .take()
            .ok_or_else(|| PtyError::Io(std::io::Error::other("output handler already set")))?;
        let output = Arc::clone(&self.output);
        let alive = Arc::clone(&self.alive);

        std::thread::Builder::new()
            .name("pty-output-dispatch".to_string())
            .spawn(move || loop {
                match rx.recv() {
                    Ok(OutputMessage::Data(chunk)) => {
                        handler(chunk.text.as_bytes());
                        if let Ok(mut buf) = output.lock() {
                            buf.push(chunk);
                        }
                    }
                    Ok(OutputMessage::Shutdown) | Err(_) => {
                        alive.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            })?;

        Ok(())
    }

    /// Drains pending output from the channel into the ring buffer and
    /// returns recent lines. Use this for poll-based reading.
    pub fn read_output(&self, max_lines: Option<usize>) -> PtyResult<ReadResult> {
        if let Some(ref rx) = self.output_rx {
            loop {
                match rx.try_recv() {
                    Ok(OutputMessage::Data(chunk)) => {
                        if let Ok(mut buf) = self.output.lock() {
                            buf.push(chunk);
                        }
                    }
                    Ok(OutputMessage::Shutdown) => {
                        self.alive.store(false, Ordering::SeqCst);
                        break;
                    }
                    Err(_) => break,
                }
            }
        }

        let mut buf = self.output.lock().map_err(|_| PtyError::LockPoisoned)?;
        let lines = buf.get_lines(max_lines);
        let dropped = buf.take_dropped_count();
        let total = buf.total_count();

        Ok(ReadResult {
            lines,
            dropped,
            total,
            is_alive: self.is_alive(),
        })
    }

    /// Clears the output ring buffer.
    pub fn clear_output(&self) -> PtyResult<()> {
        let mut buf = self.output.lock().map_err(|_| PtyError::LockPoisoned)?;
        buf.clear();
        Ok(())
    }

    /// Returns `true` if the child process is still running.
    pub fn is_alive(&self) -> bool {
        if !self.alive.load(Ordering::SeqCst) {
            return false;
        }
        if let Ok(mut child) = self.child.lock() {
            if let Ok(Some(_status)) = child.try_wait() {
                self.alive.store(false, Ordering::SeqCst);
                return false;
            }
        }
        true
    }

    /// Returns the exit code of the child process, if it has exited.
    pub fn exit_code(&self) -> Option<i32> {
        if let Ok(mut child) = self.child.lock() {
            if let Ok(Some(status)) = child.try_wait() {
                self.alive.store(false, Ordering::SeqCst);
                #[allow(clippy::cast_possible_wrap)]
                return Some(status.exit_code() as i32);
            }
        }
        None
    }

    /// Returns the PID of the child process.
    pub fn pid(&self) -> Option<u32> {
        if let Ok(child) = self.child.lock() {
            child.process_id()
        } else {
            None
        }
    }

    /// Returns the shell path used.
    pub fn shell(&self) -> &str {
        &self.shell
    }

    /// Returns the working directory the process was started in.
    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    /// Returns the current terminal dimensions.
    pub fn size(&self) -> TerminalSize {
        TerminalSize {
            rows: self.rows,
            cols: self.cols,
        }
    }

    /// Sends a `cd` command through the shell to change directory.
    pub fn set_cwd(&mut self, cwd: &Path) -> PtyResult<()> {
        let path_str = cwd.to_string_lossy();
        let cd_cmd = if cfg!(target_os = "windows") {
            format!("cd /d \"{}\"\n", path_str.replace('"', "\"\""))
        } else {
            format!("cd '{}'\n", path_str.replace('\'', "'\"'\"'"))
        };
        self.write_str(&cd_cmd)?;
        self.cwd = cwd.to_path_buf();
        Ok(())
    }

    /// Sets an environment variable by writing an `export` command to the shell.
    pub fn set_env(&self, key: &str, value: &str) -> PtyResult<()> {
        let cmd = if cfg!(target_os = "windows") {
            format!("set {}={}\n", key, value)
        } else {
            format!("export {}='{}'\n", key, value.replace('\'', "'\"'\"'"))
        };
        self.write_str(&cmd)
    }

    /// Attempts to read the current working directory of the shell process
    /// via `/proc/<pid>/cwd` on Linux or `lsof` on macOS.
    pub fn get_cwd(&self) -> Option<PathBuf> {
        let pid = self.pid()?;

        #[cfg(target_os = "linux")]
        {
            let link = format!("/proc/{pid}/cwd");
            if let Ok(path) = std::fs::read_link(&link) {
                return Some(path);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("lsof")
                .args(["-p", &pid.to_string(), "-Fn"])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if let Some(dir) = line.strip_prefix('n') {
                        let p = PathBuf::from(dir);
                        if p.is_dir() {
                            return Some(p);
                        }
                    }
                }
            }
        }

        Some(self.cwd.clone())
    }

    /// Returns the terminal title (set via OSC escape sequences).
    pub fn get_title(&self) -> Option<String> {
        self.title.lock().ok()?.clone()
    }

    /// Sets the terminal title (called by the emulator when OSC 0/2 is received).
    pub fn set_title(&self, title: &str) {
        if let Ok(mut t) = self.title.lock() {
            *t = Some(title.to_string());
        }
    }

    /// Returns a `TermInfo` snapshot of this terminal.
    pub fn info(&self, handle: TermHandle) -> TermInfo {
        let total = self.output.lock().map(|b| b.total_count()).unwrap_or(0);
        TermInfo {
            handle,
            shell: self.shell.clone(),
            cwd: self.cwd.to_string_lossy().to_string(),
            pid: self.pid().unwrap_or(0),
            cols: self.cols,
            rows: self.rows,
            is_alive: self.is_alive(),
            output_lines_total: total,
        }
    }
}

/// Result of reading terminal output.
#[derive(Debug, Serialize)]
pub struct ReadResult {
    pub lines: Vec<OutputChunk>,
    pub dropped: usize,
    pub total: usize,
    pub is_alive: bool,
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        let _ = self.kill();
        if let Some(thread) = self.reader_thread.take() {
            let _ = thread.join();
        }
    }
}
