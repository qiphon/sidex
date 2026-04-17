//! Application startup sequence.
//!
//! Defines a phased boot process so that expensive subsystems (GPU, extensions,
//! language servers) are initialised in a predictable order with timing
//! information for every phase.

use std::fmt;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::services::AppContext;

// ── Phase status ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PhaseStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

impl fmt::Display for PhaseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed(msg) => write!(f, "failed: {msg}"),
        }
    }
}

// ── Startup phase ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StartupPhase {
    pub name: String,
    pub duration: Option<Duration>,
    pub status: PhaseStatus,
}

impl StartupPhase {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            duration: None,
            status: PhaseStatus::Pending,
        }
    }
}

// ── Startup sequence ─────────────────────────────────────────────────────────

pub struct StartupSequence {
    pub phases: Vec<StartupPhase>,
    pub current_phase: usize,
    pub start_time: Instant,
}

impl StartupSequence {
    pub fn new() -> Self {
        let phases = vec![
            StartupPhase::new("Load configuration"),
            StartupPhase::new("Initialize database"),
            StartupPhase::new("Initialize GPU renderer"),
            StartupPhase::new("Restore session"),
            StartupPhase::new("Start extension host"),
            StartupPhase::new("Activate extensions"),
            StartupPhase::new("Start file watcher"),
            StartupPhase::new("Start language servers"),
            StartupPhase::new("Check for updates"),
            StartupPhase::new("Show welcome page"),
        ];

        Self {
            phases,
            current_phase: 0,
            start_time: Instant::now(),
        }
    }

    /// Execute all startup phases in order.
    ///
    /// Each phase is timed individually. A phase failure is recorded but does
    /// **not** abort the remaining phases — the caller can inspect
    /// [`get_phase_times`] to decide what to do about partial failures.
    pub fn run(&mut self, context: &mut AppContext) -> Result<()> {
        self.start_time = Instant::now();

        let phase_fns: Vec<fn(&mut AppContext) -> Result<()>> = vec![
            phase_load_configuration,
            phase_initialize_database,
            phase_initialize_gpu,
            phase_restore_session,
            phase_start_extension_host,
            phase_activate_extensions,
            phase_start_file_watcher,
            phase_start_language_servers,
            phase_check_for_updates,
            phase_show_welcome,
        ];

        for (i, phase_fn) in phase_fns.into_iter().enumerate() {
            self.current_phase = i;
            self.phases[i].status = PhaseStatus::Running;
            let t0 = Instant::now();

            match phase_fn(context) {
                Ok(()) => {
                    self.phases[i].duration = Some(t0.elapsed());
                    self.phases[i].status = PhaseStatus::Completed;
                    log::info!(
                        "startup phase '{}' completed in {:?}",
                        self.phases[i].name,
                        t0.elapsed()
                    );
                }
                Err(e) => {
                    self.phases[i].duration = Some(t0.elapsed());
                    self.phases[i].status = PhaseStatus::Failed(format!("{e:#}"));
                    log::error!("startup phase '{}' failed: {e:#}", self.phases[i].name);
                }
            }
        }

        let total = self.start_time.elapsed();
        log::info!("startup completed in {total:?}");
        Ok(())
    }

    /// Wall-clock time from the start of `run()` to now (or to completion).
    pub fn get_startup_time(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Per-phase timings, in order.
    pub fn get_phase_times(&self) -> Vec<(&str, Duration)> {
        self.phases
            .iter()
            .filter_map(|p| p.duration.map(|d| (p.name.as_str(), d)))
            .collect()
    }

    /// Returns `true` once every phase has either completed or failed.
    pub fn is_finished(&self) -> bool {
        self.phases.iter().all(|p| {
            matches!(p.status, PhaseStatus::Completed | PhaseStatus::Failed(_))
        })
    }

    /// Returns the names of phases that failed.
    pub fn failed_phases(&self) -> Vec<&str> {
        self.phases
            .iter()
            .filter(|p| matches!(p.status, PhaseStatus::Failed(_)))
            .map(|p| p.name.as_str())
            .collect()
    }
}

impl Default for StartupSequence {
    fn default() -> Self {
        Self::new()
    }
}

// ── Individual phase implementations ─────────────────────────────────────────

fn phase_load_configuration(ctx: &mut AppContext) -> Result<()> {
    log::debug!("loading settings, keybindings, and themes");
    ctx.services
        .get::<crate::services::SettingsService>()
        .context("SettingsService not registered")?;
    ctx.services
        .get::<crate::services::KeybindingService>()
        .context("KeybindingService not registered")?;
    ctx.services
        .get::<crate::services::ThemeService>()
        .context("ThemeService not registered")?;
    Ok(())
}

fn phase_initialize_database(ctx: &mut AppContext) -> Result<()> {
    log::debug!("initializing database");
    ctx.services
        .get::<crate::services::DatabaseService>()
        .context("DatabaseService not registered")?;
    Ok(())
}

fn phase_initialize_gpu(_ctx: &mut AppContext) -> Result<()> {
    log::debug!("initializing GPU renderer (deferred to window creation)");
    Ok(())
}

fn phase_restore_session(_ctx: &mut AppContext) -> Result<()> {
    log::debug!("restoring previous session state");
    Ok(())
}

fn phase_start_extension_host(ctx: &mut AppContext) -> Result<()> {
    log::debug!("starting extension host");
    ctx.services
        .get::<crate::services::ExtensionService>()
        .context("ExtensionService not registered")?;
    Ok(())
}

fn phase_activate_extensions(_ctx: &mut AppContext) -> Result<()> {
    log::debug!("activating extensions based on activation events");
    Ok(())
}

fn phase_start_file_watcher(ctx: &mut AppContext) -> Result<()> {
    log::debug!("starting file watcher");
    ctx.services
        .get::<crate::services::FileService>()
        .context("FileService not registered")?;
    Ok(())
}

fn phase_start_language_servers(ctx: &mut AppContext) -> Result<()> {
    log::debug!("starting language servers");
    ctx.services
        .get::<crate::services::LanguageService>()
        .context("LanguageService not registered")?;
    Ok(())
}

fn phase_check_for_updates(ctx: &mut AppContext) -> Result<()> {
    log::debug!("checking for application updates");
    ctx.services
        .get::<crate::services::UpdateService>()
        .context("UpdateService not registered")?;
    Ok(())
}

fn phase_show_welcome(_ctx: &mut AppContext) -> Result<()> {
    log::debug!("first-launch welcome page (skipped if not first run)");
    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_all_phases() {
        let seq = StartupSequence::new();
        assert_eq!(seq.phases.len(), 10);
        assert!(seq.phases.iter().all(|p| matches!(p.status, PhaseStatus::Pending)));
    }

    #[test]
    fn run_completes_all_phases() {
        let mut seq = StartupSequence::new();
        let mut ctx = AppContext::new();
        seq.run(&mut ctx).unwrap();
        assert!(seq.is_finished());
        assert!(seq.failed_phases().is_empty());
    }

    #[test]
    fn phase_times_populated_after_run() {
        let mut seq = StartupSequence::new();
        let mut ctx = AppContext::new();
        seq.run(&mut ctx).unwrap();
        let times = seq.get_phase_times();
        assert_eq!(times.len(), 10);
    }

    #[test]
    fn startup_time_is_nonzero_after_run() {
        let mut seq = StartupSequence::new();
        let mut ctx = AppContext::new();
        seq.run(&mut ctx).unwrap();
        assert!(seq.get_startup_time() > Duration::ZERO);
    }
}
