//! Linux installer: atomically swap the running `AppImage` or the binary
//! shipped inside a `.deb` / `.tar.gz`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use tokio::task;

use crate::{UpdateError, UpdateResult};

pub(super) async fn install(artifact: &Path) -> UpdateResult<()> {
    let artifact = artifact.to_path_buf();
    task::spawn_blocking(move || install_blocking(&artifact))
        .await
        .map_err(|e| UpdateError::InstallFailed(format!("join error: {e}")))?
}

fn install_blocking(artifact: &Path) -> UpdateResult<()> {
    let current = std::env::current_exe()?;
    if is_appimage(&current) {
        replace_appimage(&current, artifact)
    } else {
        replace_binary(&current, artifact)
    }
}

pub(super) fn relaunch(install_root: &Path) -> UpdateResult<()> {
    let target = if install_root.is_file() {
        install_root.to_path_buf()
    } else {
        std::env::current_exe().unwrap_or_else(|_| install_root.to_path_buf())
    };
    Command::new(target)
        .spawn()
        .map_err(|e| UpdateError::InstallFailed(format!("relaunch spawn: {e}")))?;
    Ok(())
}

fn is_appimage(path: &Path) -> bool {
    std::env::var_os("APPIMAGE").is_some()
        || path
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|name| name.to_ascii_lowercase().ends_with(".appimage"))
}

fn replace_appimage(current: &Path, artifact: &Path) -> UpdateResult<()> {
    let meta = fs::metadata(current)?;
    fs::copy(artifact, current)
        .map_err(|e| UpdateError::InstallFailed(format!("copy AppImage: {e}")))?;
    fs::set_permissions(current, meta.permissions())?;
    Ok(())
}

fn replace_binary(current: &Path, artifact: &Path) -> UpdateResult<()> {
    // Delegate to `install` (GNU coreutils) so SELinux contexts are preserved.
    let target = current.to_path_buf();
    let mode = fs::metadata(current)
        .ok()
        .map_or(0o755, |m| m.permissions().mode() & 0o7777);

    let scratch_parent = current
        .parent()
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let scratch = scratch_parent.join(".sidex-update.tmp");

    if scratch.exists() {
        let _ = fs::remove_file(&scratch);
    }

    fs::copy(artifact, &scratch)
        .map_err(|e| UpdateError::InstallFailed(format!("copy staging: {e}")))?;
    fs::set_permissions(&scratch, fs::Permissions::from_mode(mode))?;

    fs::rename(&scratch, &target)
        .map_err(|e| UpdateError::InstallFailed(format!("rename: {e}")))?;
    Ok(())
}
