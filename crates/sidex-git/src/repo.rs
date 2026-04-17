//! Repository detection and metadata.

use std::path::{Path, PathBuf};

use crate::cmd::{git_command, run_git};
use crate::error::GitResult;

/// Walk up from `path` to find the `.git` directory, returning the repo root.
pub fn find_repo_root(path: &Path) -> Option<PathBuf> {
    let mut current = if path.is_file() {
        path.parent()?.to_path_buf()
    } else {
        path.to_path_buf()
    };

    loop {
        if current.join(".git").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Check whether `path` is inside a git working tree.
pub fn is_git_repo(path: &Path) -> bool {
    git_command()
        .current_dir(path)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The name of the current branch (e.g. `"main"`).
pub fn current_branch(repo_root: &Path) -> GitResult<String> {
    let output = run_git(repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok(output.trim().to_string())
}

/// List remote names (e.g. `["origin"]`).
pub fn remotes(repo_root: &Path) -> GitResult<Vec<String>> {
    let output = run_git(repo_root, &["remote"])?;
    Ok(output
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["init", "-b", "main"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["config", "user.email", "test@test.com"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["config", "user.name", "Test"])
            .output()
            .unwrap();
        tmp
    }

    #[test]
    fn find_repo_root_works() {
        let tmp = init_repo();
        let sub = tmp.path().join("sub/deep");
        fs::create_dir_all(&sub).unwrap();
        let found = find_repo_root(&sub).unwrap();
        let expected = tmp.path().canonicalize().unwrap();
        let actual = found.canonicalize().unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn is_git_repo_works() {
        let tmp = init_repo();
        assert!(is_git_repo(tmp.path()));

        let not_git = TempDir::new().unwrap();
        assert!(!is_git_repo(not_git.path()));
    }

    #[test]
    fn current_branch_works() {
        let tmp = init_repo();
        // Need at least one commit for HEAD to exist.
        fs::write(tmp.path().join("f.txt"), "x").unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(["commit", "-m", "init"])
            .output()
            .unwrap();

        let branch = current_branch(tmp.path()).unwrap();
        assert_eq!(branch, "main");
    }
}
