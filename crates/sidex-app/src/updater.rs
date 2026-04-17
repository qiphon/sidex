//! Auto-update system — checks for newer releases, downloads them in the
//! background, and applies the update on the user's platform.
//!
//! Mirrors VS Code's update lifecycle: check on startup, recheck every
//! 6 hours, show a notification when a new version is available, download
//! with progress, then apply + restart.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Default interval between automatic update checks (6 hours).
const DEFAULT_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Default update endpoint.
const DEFAULT_UPDATE_URL: &str = "https://update.sidex.dev/api/update";

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Describes an available remote update.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// SemVer version string (e.g. `"0.3.0"`).
    pub version: String,
    /// ISO-8601 release date.
    pub release_date: String,
    /// Markdown release notes.
    pub release_notes: String,
    /// Direct download URL for the platform artifact.
    pub download_url: String,
    /// SHA-256 digest of the download.
    pub sha256: String,
    /// Size in bytes of the download.
    pub size_bytes: u64,
}

/// The auto-update state machine.
#[derive(Debug, Clone)]
pub enum UpdateState {
    /// Not doing anything update-related.
    Idle,
    /// Currently asking the server for a newer version.
    CheckingForUpdate,
    /// A newer version was found but not yet downloaded.
    UpdateAvailable(UpdateInfo),
    /// Download is in progress. `progress` is 0.0 .. 1.0.
    Downloading { progress: f32 },
    /// The update has been downloaded and is ready to install.
    ReadyToInstall(UpdateInfo),
    /// Something went wrong.
    Error(String),
}

/// User-facing update mode (maps to `update.mode` setting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode {
    /// Check, download, and prompt to install automatically.
    Default,
    /// Only check — the user triggers download manually.
    Manual,
    /// Disable all update activity.
    None,
}

impl UpdateMode {
    pub fn from_setting(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "manual" => Self::Manual,
            "none" => Self::None,
            _ => Self::Default,
        }
    }
}

impl Default for UpdateMode {
    fn default() -> Self {
        Self::Default
    }
}

/// Central updater service.
pub struct Updater {
    pub state: UpdateState,
    pub check_interval: Duration,
    pub update_url: String,
    pub current_version: String,
    pub mode: UpdateMode,
    last_check: Option<SystemTime>,
    download_dir: PathBuf,
}

impl Updater {
    /// Create a new updater for the given `current_version`.
    pub fn new(current_version: &str) -> Self {
        let download_dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("sidex")
            .join("updates");
        Self {
            state: UpdateState::Idle,
            check_interval: DEFAULT_CHECK_INTERVAL,
            update_url: DEFAULT_UPDATE_URL.into(),
            current_version: current_version.into(),
            mode: UpdateMode::Default,
            last_check: None,
            download_dir,
        }
    }

    /// Whether enough time has elapsed since the last check.
    pub fn should_check_now(&self) -> bool {
        if self.mode == UpdateMode::None {
            return false;
        }
        match self.last_check {
            Some(t) => t.elapsed().map_or(true, |e| e >= self.check_interval),
            None => true,
        }
    }

    /// Perform an update check against the remote server.
    ///
    /// Returns `Some(info)` when a newer version exists, `None` otherwise.
    pub fn check_for_update(&mut self) -> Result<Option<UpdateInfo>> {
        if self.mode == UpdateMode::None {
            return Ok(None);
        }
        self.state = UpdateState::CheckingForUpdate;
        self.last_check = Some(SystemTime::now());

        let result = check_for_update(&self.current_version, &self.update_url);
        match &result {
            Ok(Some(info)) => self.state = UpdateState::UpdateAvailable(info.clone()),
            Ok(None) => self.state = UpdateState::Idle,
            Err(e) => self.state = UpdateState::Error(e.to_string()),
        }
        result
    }

    /// Download the update described by `info`, reporting progress via `cb`.
    pub fn download_update(
        &mut self,
        info: &UpdateInfo,
        progress_cb: impl Fn(f32),
    ) -> Result<PathBuf> {
        self.state = UpdateState::Downloading { progress: 0.0 };
        let result = download_update(info, &self.download_dir, &progress_cb);
        match &result {
            Ok(_) => self.state = UpdateState::ReadyToInstall(info.clone()),
            Err(e) => self.state = UpdateState::Error(e.to_string()),
        }
        result
    }

    /// Apply a previously downloaded update and (optionally) restart.
    pub fn apply_update(&mut self, downloaded_path: &Path) -> Result<()> {
        apply_update(downloaded_path)
    }

    /// A human-readable status string for the status bar.
    pub fn status_text(&self) -> Option<String> {
        match &self.state {
            UpdateState::Idle => None,
            UpdateState::CheckingForUpdate => Some("Checking for updates…".into()),
            UpdateState::UpdateAvailable(info) => {
                Some(format!("Update available: v{}", info.version))
            }
            UpdateState::Downloading { progress } => {
                Some(format!("Downloading update… {:.0}%", progress * 100.0))
            }
            UpdateState::ReadyToInstall(info) => {
                Some(format!("v{} ready — restart to update", info.version))
            }
            UpdateState::Error(msg) => Some(format!("Update error: {msg}")),
        }
    }

    /// Reset the updater to idle (e.g. after the user dismisses an error).
    pub fn dismiss(&mut self) {
        self.state = UpdateState::Idle;
    }
}

// ---------------------------------------------------------------------------
// Standalone functions
// ---------------------------------------------------------------------------

/// Check the update server for a newer version.
pub fn check_for_update(current_version: &str, update_url: &str) -> Result<Option<UpdateInfo>> {
    let platform = platform_string();
    let url = format!(
        "{update_url}/{platform}/{current_version}",
    );
    log::info!("checking for update: {url}");

    // In a real build this would be an async HTTP request. Here we
    // structure the call so it can later be backed by `reqwest`.
    let _ = url;
    log::info!("no update available (offline stub)");
    Ok(None)
}

/// Download `info.download_url` into `download_dir`, calling `progress_cb`
/// with values in 0.0 ..= 1.0.
pub fn download_update(
    info: &UpdateInfo,
    download_dir: &Path,
    progress_cb: impl Fn(f32),
) -> Result<PathBuf> {
    std::fs::create_dir_all(download_dir).context("create download dir")?;

    let filename = info
        .download_url
        .rsplit('/')
        .next()
        .unwrap_or("update.bin");
    let dest = download_dir.join(filename);

    log::info!("downloading update to {}", dest.display());

    // Stub: in production this performs a streaming HTTP download and
    // verifies the SHA-256 hash.
    progress_cb(0.0);
    progress_cb(1.0);
    std::fs::write(&dest, b"").context("write placeholder")?;

    verify_sha256(&dest, &info.sha256)?;

    log::info!("download complete: {}", dest.display());
    Ok(dest)
}

/// Apply the downloaded update (platform-specific).
pub fn apply_update(downloaded_path: &Path) -> Result<()> {
    if !downloaded_path.exists() {
        bail!("downloaded update not found: {}", downloaded_path.display());
    }

    log::info!("applying update from {}", downloaded_path.display());

    #[cfg(target_os = "macos")]
    {
        apply_macos(downloaded_path)?;
    }
    #[cfg(target_os = "linux")]
    {
        apply_linux(downloaded_path)?;
    }
    #[cfg(target_os = "windows")]
    {
        apply_windows(downloaded_path)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Platform-specific apply helpers
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn apply_macos(path: &Path) -> Result<()> {
    log::info!("macOS: will replace .app bundle from {}", path.display());
    // In production: unzip the .app, swap via rename, relaunch.
    Ok(())
}

#[cfg(target_os = "linux")]
fn apply_linux(path: &Path) -> Result<()> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "deb" => log::info!("Linux: installing .deb package"),
        "rpm" => log::info!("Linux: installing .rpm package"),
        _ => log::info!("Linux: extracting tar.gz archive"),
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn apply_windows(path: &Path) -> Result<()> {
    log::info!("Windows: launching installer {}", path.display());
    // In production: spawn the installer process and exit.
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn platform_string() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "darwin-arm64"
        } else {
            "darwin-x64"
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            "linux-arm64"
        } else {
            "linux-x64"
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "aarch64") {
            "win32-arm64"
        } else {
            "win32-x64"
        }
    } else {
        "unknown"
    }
}

fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    if expected.is_empty() {
        return Ok(());
    }
    let bytes = std::fs::read(path).context("read downloaded file")?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    let digest = format!("{:016x}", hasher.finish());
    log::debug!("sha256 stub hash: {digest} (expected: {expected})");
    // Full SHA-256 verification would use the `sha2` crate in production.
    Ok(())
}

fn iso_now() -> String {
    humantime::format_rfc3339_seconds(SystemTime::now()).to_string()
}

/// Clean up old downloads in the update cache.
pub fn cleanup_download_cache(download_dir: &Path, max_age: Duration) -> Result<()> {
    if !download_dir.exists() {
        return Ok(());
    }
    let now = SystemTime::now();
    for entry in std::fs::read_dir(download_dir).context("read download dir")? {
        let entry = entry?;
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = now.duration_since(modified) {
                    if age > max_age {
                        let _ = std::fs::remove_file(entry.path());
                        log::debug!("removed stale download: {}", entry.path().display());
                    }
                }
            }
        }
    }
    Ok(())
}

// Suppress unused import warning for iso_now during early development.
const _: () = {
    fn _use(_: fn() -> String) {}
    fn _touch() {
        _use(iso_now);
    }
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_from_setting() {
        assert_eq!(UpdateMode::from_setting("default"), UpdateMode::Default);
        assert_eq!(UpdateMode::from_setting("manual"), UpdateMode::Manual);
        assert_eq!(UpdateMode::from_setting("none"), UpdateMode::None);
        assert_eq!(UpdateMode::from_setting("garbage"), UpdateMode::Default);
    }

    #[test]
    fn new_updater_is_idle() {
        let u = Updater::new("0.2.0");
        assert!(matches!(u.state, UpdateState::Idle));
        assert_eq!(u.current_version, "0.2.0");
        assert!(u.should_check_now());
    }

    #[test]
    fn should_check_now_false_when_none() {
        let mut u = Updater::new("0.2.0");
        u.mode = UpdateMode::None;
        assert!(!u.should_check_now());
    }

    #[test]
    fn check_for_update_returns_none_stub() {
        let mut u = Updater::new("0.2.0");
        let result = u.check_for_update().unwrap();
        assert!(result.is_none());
        assert!(matches!(u.state, UpdateState::Idle));
    }

    #[test]
    fn status_text_idle_is_none() {
        let u = Updater::new("0.2.0");
        assert!(u.status_text().is_none());
    }

    #[test]
    fn status_text_available() {
        let mut u = Updater::new("0.2.0");
        u.state = UpdateState::UpdateAvailable(UpdateInfo {
            version: "0.3.0".into(),
            release_date: "2025-01-01".into(),
            release_notes: "New stuff".into(),
            download_url: "https://example.com/update.tar.gz".into(),
            sha256: String::new(),
            size_bytes: 1024,
        });
        let text = u.status_text().unwrap();
        assert!(text.contains("0.3.0"));
    }

    #[test]
    fn status_text_downloading() {
        let mut u = Updater::new("0.2.0");
        u.state = UpdateState::Downloading { progress: 0.42 };
        let text = u.status_text().unwrap();
        assert!(text.contains("42%"));
    }

    #[test]
    fn dismiss_returns_to_idle() {
        let mut u = Updater::new("0.2.0");
        u.state = UpdateState::Error("fail".into());
        u.dismiss();
        assert!(matches!(u.state, UpdateState::Idle));
    }

    #[test]
    fn platform_string_is_known() {
        let p = platform_string();
        assert!(
            ["darwin-arm64", "darwin-x64", "linux-arm64", "linux-x64", "win32-arm64", "win32-x64"]
                .contains(&p)
                || p == "unknown"
        );
    }

    #[test]
    fn cleanup_cache_nonexistent_dir() {
        let result = cleanup_download_cache(Path::new("/nonexistent/dir"), Duration::from_secs(1));
        assert!(result.is_ok());
    }

    #[test]
    fn download_creates_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let info = UpdateInfo {
            version: "0.3.0".into(),
            release_date: "2025-01-01".into(),
            release_notes: String::new(),
            download_url: "https://example.com/sidex-0.3.0.tar.gz".into(),
            sha256: String::new(),
            size_bytes: 0,
        };
        let dest = download_update(&info, &tmp.path().join("sub"), |_| {}).unwrap();
        assert!(dest.exists());
    }

    #[test]
    fn apply_update_missing_file() {
        let result = apply_update(Path::new("/nonexistent/file.bin"));
        assert!(result.is_err());
    }
}
