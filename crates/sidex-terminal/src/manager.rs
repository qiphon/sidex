//! Terminal instance manager — profiles, split groups, focus navigation,
//! and process-tree cleanup.

use crate::emulator::TerminalEmulator;
use crate::grid::TerminalGrid;
use crate::pty::{
    PtyError, PtyProcess, PtySpawnConfig, ReadResult, TermHandle, TermInfo, TerminalSize,
};
use crate::shell;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TerminalId(pub u32);

impl From<TermHandle> for TerminalId {
    fn from(h: TermHandle) -> Self {
        Self(h.0)
    }
}
impl From<TerminalId> for TermHandle {
    fn from(id: TerminalId) -> Self {
        Self(id.0)
    }
}

#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("terminal not found: {0:?}")]
    NotFound(TerminalId),
    #[error("PTY error: {0}")]
    Pty(#[from] PtyError),
    #[error("profile not found: {0}")]
    ProfileNotFound(String),
    #[error("lock poisoned")]
    LockPoisoned,
}
type ManagerResult<T> = Result<T, ManagerError>;

#[derive(Debug, Clone)]
pub enum TerminalEvent {
    Data {
        id: TerminalId,
        text: String,
    },
    Exit {
        id: TerminalId,
        exit_code: i32,
    },
    Started {
        id: TerminalId,
        shell: String,
        pid: u32,
        cwd: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalState {
    Running,
    Exited(i32),
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalProfile {
    pub name: String,
    pub shell_path: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub icon: String,
    pub color: Option<String>,
}

pub struct TerminalInstance {
    pub pty: PtyProcess,
    pub emulator: TerminalEmulator,
    pub id: u32,
    pub name: String,
    pub shell: String,
    pub pid: u32,
    pub state: TerminalState,
    pub cwd: PathBuf,
    pub profile: TerminalProfile,
    pub size: TerminalSize,
    handle: TermHandle,
}

impl TerminalInstance {
    pub fn handle(&self) -> TermHandle {
        self.handle
    }
    pub fn info(&self) -> TermInfo {
        self.pty.info(self.handle)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitOrientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitGroup {
    pub terminals: Vec<u32>,
    pub orientation: SplitOrientation,
    pub ratios: Vec<f32>,
}

impl SplitGroup {
    fn new(first: u32) -> Self {
        Self {
            terminals: vec![first],
            orientation: SplitOrientation::Horizontal,
            ratios: vec![1.0],
        }
    }
    fn add(&mut self, id: u32) {
        self.terminals.push(id);
        self.rebalance();
    }
    fn remove(&mut self, id: u32) {
        if let Some(pos) = self.terminals.iter().position(|&t| t == id) {
            self.terminals.remove(pos);
            self.rebalance();
        }
    }
    #[allow(clippy::cast_precision_loss)]
    fn rebalance(&mut self) {
        let n = self.terminals.len().max(1) as f32;
        self.ratios = vec![1.0 / n; self.terminals.len()];
    }
}

/// Detect available terminal profiles from installed shells.
pub fn detect_profiles() -> Vec<TerminalProfile> {
    shell::available_shells()
        .into_iter()
        .map(|s| {
            let icon = match s.name.as_str() {
                "zsh" => "terminal-zsh",
                "bash" => "terminal-bash",
                "fish" => "terminal-fish",
                n if n.contains("owerShell") || n.contains("owershell") => "terminal-powershell",
                n if n.contains("cmd") || n.contains("Command") => "terminal-cmd",
                _ => "terminal",
            };
            TerminalProfile {
                name: s.name,
                shell_path: s.path,
                args: s.args,
                env: HashMap::new(),
                icon: icon.to_string(),
                color: None,
            }
        })
        .collect()
}

fn default_profile() -> TerminalProfile {
    let ds = shell::detect_default_shell();
    let base = Path::new(&ds)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sh")
        .to_string();
    detect_profiles()
        .into_iter()
        .find(|p| p.shell_path == ds || p.name == base)
        .unwrap_or(TerminalProfile {
            name: base,
            shell_path: ds,
            args: vec![],
            env: HashMap::new(),
            icon: "terminal".to_string(),
            color: None,
        })
}

fn lock<T>(m: &Mutex<T>) -> ManagerResult<std::sync::MutexGuard<'_, T>> {
    m.lock().map_err(|_| ManagerError::LockPoisoned)
}

pub struct TerminalManager {
    terminals: HashMap<TerminalId, Arc<Mutex<TerminalInstance>>>,
    order: Vec<TerminalId>,
    active_instance: Option<TerminalId>,
    split_groups: Vec<SplitGroup>,
    default_size: TerminalSize,
    event_tx: Option<crossbeam::channel::Sender<TerminalEvent>>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            terminals: HashMap::new(),
            order: Vec::new(),
            active_instance: None,
            split_groups: Vec::new(),
            default_size: TerminalSize::default(),
            event_tx: None,
        }
    }

    pub fn with_default_size(size: TerminalSize) -> Self {
        Self {
            default_size: size,
            ..Self::new()
        }
    }

    pub fn set_event_channel(&mut self) -> crossbeam::channel::Receiver<TerminalEvent> {
        let (tx, rx) = crossbeam::channel::unbounded();
        self.event_tx = Some(tx);
        rx
    }

    fn get_inst(&self, id: TerminalId) -> ManagerResult<&Arc<Mutex<TerminalInstance>>> {
        self.terminals.get(&id).ok_or(ManagerError::NotFound(id))
    }

    fn spawn_instance(
        &mut self,
        profile: &TerminalProfile,
        cwd: Option<&Path>,
        size: TerminalSize,
    ) -> ManagerResult<TerminalId> {
        let config = PtySpawnConfig {
            shell: Some(profile.shell_path.clone()),
            args: if profile.args.is_empty() {
                None
            } else {
                Some(profile.args.clone())
            },
            cwd: cwd.map(Path::to_path_buf),
            env: profile.env.clone(),
            size,
        };
        let mut pty = PtyProcess::spawn(&config)?;
        let handle = TermHandle::next();
        let id = TerminalId::from(handle);
        let pid = pty.pid().unwrap_or(0);
        let cwd_path = pty.cwd().to_path_buf();
        let shell_str = pty.shell().to_string();
        if let Some(ref tx) = self.event_tx {
            let tx_c = tx.clone();
            let tid = id;
            pty.on_output(move |data| {
                let _ = tx_c.send(TerminalEvent::Data {
                    id: tid,
                    text: String::from_utf8_lossy(data).to_string(),
                });
            })?;
            let _ = tx.send(TerminalEvent::Started {
                id,
                shell: shell_str.clone(),
                pid,
                cwd: cwd_path.to_string_lossy().to_string(),
            });
        }
        let instance = TerminalInstance {
            emulator: TerminalEmulator::new(TerminalGrid::new(size.rows, size.cols)),
            pty,
            id: id.0,
            name: format!("{} {}", profile.name, id.0),
            shell: shell_str,
            pid,
            state: TerminalState::Running,
            cwd: cwd_path,
            profile: profile.clone(),
            size,
            handle,
        };
        self.terminals.insert(id, Arc::new(Mutex::new(instance)));
        self.order.push(id);
        self.active_instance = Some(id);
        Ok(id)
    }

    pub fn create_terminal(&mut self, profile_name: Option<&str>) -> ManagerResult<u32> {
        let profile = match profile_name {
            Some(name) => detect_profiles()
                .into_iter()
                .find(|p| p.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| ManagerError::ProfileNotFound(name.to_string()))?,
            None => default_profile(),
        };
        self.spawn_instance(&profile, None, self.default_size)
            .map(|id| id.0)
    }

    pub fn create(&mut self, shell: Option<&str>, cwd: Option<&Path>) -> ManagerResult<TerminalId> {
        let mut p = default_profile();
        if let Some(s) = shell {
            p.shell_path = s.to_string();
        }
        self.spawn_instance(&p, cwd, self.default_size)
    }

    pub fn create_with_config(&mut self, config: &PtySpawnConfig) -> ManagerResult<TerminalId> {
        let mut p = default_profile();
        if let Some(ref s) = config.shell {
            p.shell_path.clone_from(s);
        }
        if let Some(ref a) = config.args {
            p.args.clone_from(a);
        }
        p.env.clone_from(&config.env);
        self.spawn_instance(&p, config.cwd.as_deref(), config.size)
    }

    pub fn split_terminal(&mut self, source_id: u32) -> ManagerResult<u32> {
        let prof = lock(self.get_inst(TerminalId(source_id))?)?.profile.clone();
        let new_id = self.spawn_instance(&prof, None, self.default_size)?.0;
        if let Some(g) = self
            .split_groups
            .iter_mut()
            .find(|g| g.terminals.contains(&source_id))
        {
            g.add(new_id);
        } else {
            let mut g = SplitGroup::new(source_id);
            g.add(new_id);
            self.split_groups.push(g);
        }
        Ok(new_id)
    }

    pub fn close_terminal(&mut self, id: u32) -> ManagerResult<()> {
        let tid = TerminalId(id);
        let inst = self
            .terminals
            .remove(&tid)
            .ok_or(ManagerError::NotFound(tid))?;
        if let Ok(l) = inst.lock() {
            let _ = l.pty.kill_tree();
            if let Some(ref tx) = self.event_tx {
                let _ = tx.send(TerminalEvent::Exit {
                    id: tid,
                    exit_code: l.pty.exit_code().unwrap_or(0),
                });
            }
        }
        self.order.retain(|&t| t != tid);
        for g in &mut self.split_groups {
            g.remove(id);
        }
        self.split_groups.retain(|g| !g.terminals.is_empty());
        if self.active_instance == Some(tid) {
            self.active_instance = self.order.last().copied();
        }
        Ok(())
    }

    pub fn remove(&mut self, id: TerminalId) -> ManagerResult<()> {
        self.close_terminal(id.0)
    }

    pub fn rename_terminal(&self, id: u32, name: &str) {
        if let Some(inst) = self.terminals.get(&TerminalId(id)) {
            if let Ok(mut l) = inst.lock() {
                l.name = name.to_string();
            }
        }
    }

    pub fn focus_terminal(&mut self, id: u32) {
        let tid = TerminalId(id);
        if self.terminals.contains_key(&tid) {
            self.active_instance = Some(tid);
        }
    }

    pub fn focus_next(&mut self) {
        self.active_instance = self.adjacent(1);
    }
    pub fn focus_previous(&mut self) {
        self.active_instance = self.adjacent(-1);
    }

    #[allow(clippy::cast_possible_wrap)]
    fn adjacent(&self, delta: isize) -> Option<TerminalId> {
        let cur = self.active_instance?;
        let pos = self.order.iter().position(|&t| t == cur)?;
        let len = self.order.len() as isize;
        Some(self.order[((pos as isize + delta).rem_euclid(len)) as usize])
    }

    pub fn list_terminals(&self) -> Vec<TerminalId> {
        self.order.clone()
    }
    pub fn list(&self) -> Vec<TerminalId> {
        self.order.clone()
    }
    pub fn get(&self, id: TerminalId) -> Option<Arc<Mutex<TerminalInstance>>> {
        self.terminals.get(&id).cloned()
    }
    pub fn count(&self) -> usize {
        self.terminals.len()
    }
    pub fn active(&self) -> Option<TerminalId> {
        self.active_instance
    }
    pub fn split_groups(&self) -> &[SplitGroup] {
        &self.split_groups
    }

    pub fn read_output(&self, id: TerminalId, max: Option<usize>) -> ManagerResult<ReadResult> {
        lock(self.get_inst(id)?)?
            .pty
            .read_output(max)
            .map_err(ManagerError::Pty)
    }
    pub fn write(&self, id: TerminalId, data: &str) -> ManagerResult<()> {
        lock(self.get_inst(id)?)?
            .pty
            .write_str(data)
            .map_err(ManagerError::Pty)
    }
    pub fn resize(&self, id: TerminalId, size: TerminalSize) -> ManagerResult<()> {
        let mut l = lock(self.get_inst(id)?)?;
        l.pty.resize(size)?;
        l.emulator.grid_mut().resize(size.rows, size.cols);
        l.size = size;
        Ok(())
    }
    pub fn info(&self, id: TerminalId) -> ManagerResult<TermInfo> {
        Ok(lock(self.get_inst(id)?)?.info())
    }
    pub fn send_signal(&self, id: TerminalId, signal: i32) -> ManagerResult<()> {
        if let Some(pid) = lock(self.get_inst(id)?)?.pty.pid() {
            crate::pty::send_signal(pid, signal).map_err(ManagerError::Pty)?;
        }
        Ok(())
    }
}

impl Default for TerminalManager {
    fn default() -> Self {
        Self::new()
    }
}
