//! Platform-specific install backends.
//!
//! Each OS has its own `install` function. Every backend is expected to
//! take the downloaded artifact and leave the running application in a
//! state where [`super::manager::UpdateManager::quit_and_install`] can
//! relaunch on the next startup.

use std::path::Path;

use crate::UpdateResult;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

/// Applies the downloaded update to the installed `SideX` bundle.
///
/// On macOS/Linux this replaces the `.app` / `AppImage` in place. On Windows
/// we spawn the extracted installer (or NSIS uninstaller-first helper).
pub async fn install(artifact: &Path) -> UpdateResult<()> {
    #[cfg(target_os = "macos")]
    {
        return macos::install(artifact).await;
    }
    #[cfg(target_os = "linux")]
    {
        return linux::install(artifact).await;
    }
    #[cfg(target_os = "windows")]
    {
        return windows::install(artifact).await;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = artifact;
        Err(crate::UpdateError::InstallFailed(
            "platform not supported".into(),
        ))
    }
}

/// Spawns a detached helper that relaunches the app on exit.
///
/// Each platform provides its own backend because the right UX differs:
/// on macOS we re-open the bundle, on Linux we re-exec the binary, on
/// Windows we hand control to `Update.exe` / the installer.
pub fn relaunch(install_root: &Path) -> UpdateResult<()> {
    #[cfg(target_os = "macos")]
    {
        macos::relaunch(install_root)
    }
    #[cfg(target_os = "linux")]
    {
        linux::relaunch(install_root)
    }
    #[cfg(target_os = "windows")]
    {
        return windows::relaunch(install_root);
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = install_root;
        Ok(())
    }
}
