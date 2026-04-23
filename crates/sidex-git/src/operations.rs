//! Git operations — stage, unstage, commit, push, pull, checkout, branch, stash, fetch, clone, show, run.

use std::path::Path;

use serde::Serialize;

use crate::cmd::{git_command, run_git};
use crate::error::{GitError, GitResult};

/// A branch entry from `git branch -a`.
#[derive(Debug, Clone, Serialize)]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    pub remote: bool,
}

/// Detailed branch info with upstream tracking data.
#[derive(Debug, Clone, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    pub is_current: bool,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub last_commit: String,
}

/// A stash entry parsed from `git stash list`.
#[derive(Debug, Clone, Serialize)]
pub struct StashEntry {
    pub index: usize,
    pub branch: String,
    pub message: String,
    pub date: String,
}

/// Result of a push operation.
#[derive(Debug, Clone, Serialize)]
pub struct PushResult {
    pub success: bool,
    pub rejected: Vec<String>,
    pub new_branch: bool,
}

/// Result of a pull operation.
#[derive(Debug, Clone, Serialize)]
pub struct PullResult {
    pub success: bool,
    pub conflicts: Vec<String>,
    pub fast_forward: bool,
    pub merge_commit: Option<String>,
}

/// Submodule information.
#[derive(Debug, Clone, Serialize)]
pub struct SubmoduleInfo {
    pub name: String,
    pub path: String,
    pub url: String,
    pub commit: String,
}

/// A remote entry from `git remote -v`.
#[derive(Debug, Clone, Serialize)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
    pub remote_type: String,
}

/// Stash sub-command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashAction {
    Push,
    Pop,
    List,
    Drop,
}

impl StashAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Pop => "pop",
            Self::List => "list",
            Self::Drop => "drop",
        }
    }
}

/// Stage files.
pub fn stage(repo_root: &Path, paths: &[&Path]) -> GitResult<()> {
    let mut args: Vec<&str> = vec!["add", "--"];
    let strs: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let refs: Vec<&str> = strs.iter().map(String::as_str).collect();
    args.extend(refs);
    run_git(repo_root, &args)?;
    Ok(())
}

/// Unstage files (reset HEAD).
pub fn unstage(repo_root: &Path, paths: &[&Path]) -> GitResult<()> {
    let mut args: Vec<&str> = vec!["reset", "HEAD", "--"];
    let strs: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let refs: Vec<&str> = strs.iter().map(String::as_str).collect();
    args.extend(refs);
    run_git(repo_root, &args)?;
    Ok(())
}

/// Commit staged changes, returning the new commit hash.
pub fn commit(repo_root: &Path, message: &str) -> GitResult<String> {
    run_git(repo_root, &["commit", "-m", message])?;
    let hash = run_git(repo_root, &["rev-parse", "HEAD"])?;
    Ok(hash.trim().to_string())
}

/// Push to a remote. Pass `None` to use defaults.
pub fn push(repo_root: &Path, remote: Option<&str>, branch: Option<&str>) -> GitResult<String> {
    let mut args = vec!["push"];
    if let Some(r) = remote {
        args.push(r);
    }
    if let Some(b) = branch {
        args.push(b);
    }
    run_git(repo_root, &args)
}

/// Pull from a remote. Pass `None` to use defaults.
pub fn pull(repo_root: &Path, remote: Option<&str>, branch: Option<&str>) -> GitResult<String> {
    let mut args = vec!["pull"];
    if let Some(r) = remote {
        args.push(r);
    }
    if let Some(b) = branch {
        args.push(b);
    }
    run_git(repo_root, &args)
}

/// Fetch from a remote. Pass `None` to fetch from the default remote.
pub fn fetch(repo_root: &Path, remote: Option<&str>) -> GitResult<String> {
    let mut args = vec!["fetch"];
    if let Some(r) = remote {
        args.push(r);
    }
    run_git(repo_root, &args)
}

/// Checkout a branch.
pub fn checkout(repo_root: &Path, branch: &str) -> GitResult<()> {
    run_git(repo_root, &["checkout", branch])?;
    Ok(())
}

pub fn restore(
    repo_root: &Path,
    paths: &[String],
    source: Option<&str>,
    staged: bool,
    worktree: bool,
) -> GitResult<()> {
    let mut args: Vec<&str> = vec!["restore"];
    if staged {
        args.push("--staged");
    }
    if worktree {
        args.push("--worktree");
    }
    let source_arg;
    if let Some(src) = source {
        source_arg = format!("--source={src}");
        args.push(&source_arg);
    }
    args.push("--");
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    args.extend(path_refs);
    run_git(repo_root, &args)?;
    Ok(())
}

pub fn clean(repo_root: &Path, paths: &[String], dirs: bool) -> GitResult<()> {
    let mut args: Vec<&str> = vec!["clean", "-f"];
    if dirs {
        args.push("-d");
    }
    args.push("--");
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    args.extend(path_refs);
    run_git(repo_root, &args)?;
    Ok(())
}

pub fn checkout_files(repo_root: &Path, treeish: &str, paths: &[String]) -> GitResult<()> {
    let mut args: Vec<&str> = vec!["checkout", treeish, "--"];
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    args.extend(path_refs);
    run_git(repo_root, &args)?;
    Ok(())
}

/// Create a new branch and switch to it, optionally from a start point.
pub fn create_branch(repo_root: &Path, name: &str, start_point: Option<&str>) -> GitResult<()> {
    let mut args = vec!["checkout", "-b", name];
    if let Some(sp) = start_point {
        args.push(sp);
    }
    run_git(repo_root, &args)?;
    Ok(())
}

/// Delete a local branch.
pub fn delete_branch(repo_root: &Path, name: &str) -> GitResult<()> {
    run_git(repo_root, &["branch", "-d", name])?;
    Ok(())
}

/// List all branches (local and remote).
pub fn branches(repo_root: &Path) -> GitResult<Vec<GitBranch>> {
    let output = run_git(repo_root, &["branch", "-a"])?;
    let result = output
        .lines()
        .filter(|l| !l.is_empty())
        .filter(|l| !l.contains("->"))
        .map(|line| {
            let current = line.starts_with('*');
            let name = line.trim_start_matches('*').trim().to_string();
            let remote = name.starts_with("remotes/");
            let name = name.trim_start_matches("remotes/").to_string();
            GitBranch {
                name,
                current,
                remote,
            }
        })
        .collect();
    Ok(result)
}

/// Stash uncommitted changes with an optional message.
pub fn stash(repo_root: &Path, message: Option<&str>) -> GitResult<String> {
    let mut args = vec!["stash", "push"];
    if let Some(m) = message {
        args.push("-m");
        args.push(m);
    }
    run_git(repo_root, &args)
}

/// Pop the most recent stash.
pub fn stash_pop(repo_root: &Path) -> GitResult<String> {
    run_git(repo_root, &["stash", "pop"])
}

/// List stash entries.
pub fn stash_list(repo_root: &Path) -> GitResult<String> {
    run_git(repo_root, &["stash", "list"])
}

/// Drop the most recent stash entry.
pub fn stash_drop(repo_root: &Path) -> GitResult<String> {
    run_git(repo_root, &["stash", "drop"])
}

/// Run a stash sub-command with an optional message (for push).
pub fn stash_action(
    repo_root: &Path,
    action: StashAction,
    message: Option<&str>,
) -> GitResult<String> {
    let mut args = vec!["stash", action.as_str()];
    if action == StashAction::Push {
        if let Some(m) = message {
            args.push("-m");
            args.push(m);
        }
    }
    run_git(repo_root, &args)
}

/// Apply a stash entry by index without removing it from the stash list.
pub fn stash_apply(repo_root: &Path, index: usize) -> GitResult<String> {
    let stash_ref = format!("stash@{{{index}}}");
    run_git(repo_root, &["stash", "apply", &stash_ref])
}

/// Drop a specific stash entry by index.
pub fn stash_drop_index(repo_root: &Path, index: usize) -> GitResult<String> {
    let stash_ref = format!("stash@{{{index}}}");
    run_git(repo_root, &["stash", "drop", &stash_ref])
}

/// Parse `git stash list` output into structured [`StashEntry`] values.
pub fn stash_list_parsed(repo_root: &Path) -> GitResult<Vec<StashEntry>> {
    let output = run_git(repo_root, &["stash", "list", "--format=%gd%n%gs%n%aI"])?;
    Ok(parse_stash_list(&output))
}

fn parse_stash_list(output: &str) -> Vec<StashEntry> {
    let lines: Vec<&str> = output.lines().collect();
    let mut entries = Vec::new();

    for chunk in lines.chunks(3) {
        if chunk.len() < 3 {
            break;
        }
        let ref_str = chunk[0];
        let message_str = chunk[1];
        let date = chunk[2].to_string();

        let index = ref_str
            .strip_prefix("stash@{")
            .and_then(|s| s.strip_suffix('}'))
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);

        let (branch, message) = if let Some(rest) = message_str.strip_prefix("WIP on ") {
            if let Some((b, m)) = rest.split_once(": ") {
                (b.to_string(), m.to_string())
            } else {
                (String::new(), rest.to_string())
            }
        } else if let Some(rest) = message_str.strip_prefix("On ") {
            if let Some((b, m)) = rest.split_once(": ") {
                (b.to_string(), m.to_string())
            } else {
                (String::new(), rest.to_string())
            }
        } else {
            (String::new(), message_str.to_string())
        };

        entries.push(StashEntry {
            index,
            branch,
            message,
            date,
        });
    }

    entries
}

/// Rename a local branch.
pub fn rename_branch(repo_root: &Path, old_name: &str, new_name: &str) -> GitResult<()> {
    run_git(repo_root, &["branch", "-m", old_name, new_name])?;
    Ok(())
}

/// Delete a local branch, optionally force.
pub fn delete_branch_force(repo_root: &Path, name: &str, force: bool) -> GitResult<()> {
    let flag = if force { "-D" } else { "-d" };
    run_git(repo_root, &["branch", flag, name])?;
    Ok(())
}

/// List branches with detailed info (upstream, ahead/behind).
pub fn list_branches(repo_root: &Path, show_remote: bool) -> GitResult<Vec<BranchInfo>> {
    let format_str =
        "%(HEAD)|%(refname:short)|%(upstream:short)|%(upstream:track)|%(objectname:short)|%(subject)";
    let format_arg = format!("--format={format_str}");
    let mut args = vec!["branch", &format_arg];
    if show_remote {
        args.push("-a");
    }
    let output = run_git(repo_root, &args)?;
    let mut infos = Vec::new();

    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(6, '|').collect();
        if parts.len() < 6 {
            continue;
        }

        let is_current = parts[0].trim() == "*";
        let name = parts[1].to_string();
        let is_remote = name.contains('/');
        let upstream = if parts[2].is_empty() {
            None
        } else {
            Some(parts[2].to_string())
        };

        let (ahead, behind) = parse_track_info(parts[3]);
        let last_commit = format!("{} {}", parts[4], parts[5]);

        infos.push(BranchInfo {
            name,
            is_remote,
            is_current,
            upstream,
            ahead,
            behind,
            last_commit,
        });
    }

    Ok(infos)
}

fn parse_track_info(track: &str) -> (u32, u32) {
    let mut ahead = 0u32;
    let mut behind = 0u32;

    if track.contains("ahead") {
        for part in track.split(',') {
            let part = part.trim().trim_matches(|c| c == '[' || c == ']');
            if let Some(n) = part.strip_prefix("ahead ") {
                ahead = n.trim().parse().unwrap_or(0);
            }
            if let Some(n) = part.strip_prefix("behind ") {
                behind = n.trim().parse().unwrap_or(0);
            }
        }
    } else if track.contains("behind") {
        for part in track.split(',') {
            let part = part.trim().trim_matches(|c| c == '[' || c == ']');
            if let Some(n) = part.strip_prefix("behind ") {
                behind = n.trim().parse().unwrap_or(0);
            }
        }
    }

    (ahead, behind)
}

/// Push to a remote with detailed result.
pub fn push_detailed(
    repo_root: &Path,
    remote: &str,
    branch: &str,
    force: bool,
) -> GitResult<PushResult> {
    let mut args = vec!["push", remote, branch];
    if force {
        args.push("--force-with-lease");
    }

    let output = git_command()
        .current_dir(repo_root)
        .args(&args)
        .output()
        .map_err(GitError::Exec)?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        let new_branch = stderr.contains("new branch");
        Ok(PushResult {
            success: true,
            rejected: Vec::new(),
            new_branch,
        })
    } else {
        let rejected: Vec<String> = stderr
            .lines()
            .filter(|l| l.contains("rejected"))
            .map(|l| l.trim().to_string())
            .collect();
        Ok(PushResult {
            success: false,
            rejected,
            new_branch: false,
        })
    }
}

/// Pull from a remote with detailed result.
pub fn pull_detailed(
    repo_root: &Path,
    remote: &str,
    branch: &str,
    rebase: bool,
) -> GitResult<PullResult> {
    let mut args = vec!["pull", remote, branch];
    if rebase {
        args.push("--rebase");
    }

    let output = git_command()
        .current_dir(repo_root)
        .args(&args)
        .output()
        .map_err(GitError::Exec)?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if output.status.success() {
        let fast_forward = stdout.contains("Fast-forward");
        let merge_commit = if fast_forward {
            None
        } else {
            run_git(repo_root, &["rev-parse", "HEAD"])
                .ok()
                .map(|h| h.trim().to_string())
        };
        Ok(PullResult {
            success: true,
            conflicts: Vec::new(),
            fast_forward,
            merge_commit,
        })
    } else {
        let conflicts = collect_conflict_paths(repo_root)?;
        Ok(PullResult {
            success: false,
            conflicts,
            fast_forward: false,
            merge_commit: None,
        })
    }
}

/// Fetch from all remotes.
pub fn fetch_all(repo_root: &Path) -> GitResult<()> {
    run_git(repo_root, &["fetch", "--all"])?;
    Ok(())
}

/// Initialize git submodules.
pub fn submodule_init(repo_root: &Path) -> GitResult<()> {
    run_git(repo_root, &["submodule", "init"])?;
    Ok(())
}

/// Update git submodules.
pub fn submodule_update(repo_root: &Path) -> GitResult<()> {
    run_git(repo_root, &["submodule", "update", "--init", "--recursive"])?;
    Ok(())
}

/// List git submodules.
pub fn list_submodules(repo_root: &Path) -> GitResult<Vec<SubmoduleInfo>> {
    let output = run_git(repo_root, &["submodule", "status"])?;
    let mut submodules = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim().trim_start_matches(['+', '-', 'U']);
        let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
        if parts.len() >= 2 {
            let commit = parts[0].to_string();
            let path = parts[1].to_string();
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();

            let url = run_git(repo_root, &["config", &format!("submodule.{name}.url")])
                .unwrap_or_default()
                .trim()
                .to_string();

            submodules.push(SubmoduleInfo {
                name,
                path,
                url,
                commit,
            });
        }
    }

    Ok(submodules)
}

/// Read a git config value.
pub fn get_config(repo_root: &Path, key: &str) -> GitResult<Option<String>> {
    match run_git(repo_root, &["config", "--get", key]) {
        Ok(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(_) => Ok(None),
    }
}

/// Set a git config value.
pub fn set_config(repo_root: &Path, key: &str, value: &str) -> GitResult<()> {
    run_git(repo_root, &["config", key, value])?;
    Ok(())
}

/// Initialize a new git repository.
pub fn init(repo_root: &Path) -> GitResult<()> {
    run_git(repo_root, &["init"])?;
    Ok(())
}

/// List remotes with their URLs.
pub fn remote_list(repo_root: &Path) -> GitResult<Vec<GitRemote>> {
    let output = run_git(repo_root, &["remote", "-v"])?;
    let mut remotes = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let remote_type = parts[2].trim_matches(|c| c == '(' || c == ')').to_string();
            remotes.push(GitRemote {
                name: parts[0].to_string(),
                url: parts[1].to_string(),
                remote_type,
            });
        }
    }
    Ok(remotes)
}

/// Show the HEAD version of a file as raw bytes.
pub fn show_file(repo_root: &Path, file: &str) -> GitResult<Vec<u8>> {
    let rev_file = format!("HEAD:{file}");
    let output = git_command()
        .current_dir(repo_root)
        .args(["show", &rev_file])
        .output()
        .map_err(GitError::Exec)?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(GitError::Command {
            message: format!("git show error: {}", stderr.trim()),
        })
    }
}

/// Clone a repository. Performs a `--no-checkout` clone followed by a hooks-disabled checkout.
pub fn clone(url: &str, dest: &Path) -> GitResult<()> {
    let dest_str = dest.to_string_lossy();

    if dest
        .components()
        .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(GitError::Command {
            message: "clone destination must not contain '..'".to_string(),
        });
    }

    let output = git_command()
        .args(["clone", "--no-checkout", url, &dest_str])
        .output()
        .map_err(GitError::Exec)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(GitError::Command {
            message: format!("git clone error: {}", stderr.trim()),
        });
    }

    let checkout_out = git_command()
        .current_dir(dest)
        .args(["-c", "core.hooksPath=/dev/null", "checkout"])
        .output()
        .map_err(GitError::Exec)?;

    if checkout_out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&checkout_out.stderr);
        Err(GitError::Command {
            message: format!("git checkout error: {}", stderr.trim()),
        })
    }
}

// ── Extended remote / merge / rebase / tag operations ────────────────────────

/// Detailed remote information with separate fetch and push URLs.
#[derive(Debug, Clone, Serialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    pub fetch_url: String,
    pub push_url: String,
}

/// Get detailed remote info for all remotes.
pub fn get_remotes(repo_root: &Path) -> GitResult<Vec<RemoteInfo>> {
    let output = run_git(repo_root, &["remote", "-v"])?;
    let mut map: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let name = parts[0].to_string();
            let url = parts[1].to_string();
            let kind = parts[2].trim_matches(|c| c == '(' || c == ')');
            let entry = map
                .entry(name)
                .or_insert_with(|| (String::new(), String::new()));
            match kind {
                "fetch" => entry.0 = url,
                "push" => entry.1 = url,
                _ => {}
            }
        }
    }

    Ok(map
        .into_iter()
        .map(|(name, (fetch_url, push_url))| {
            let url = if fetch_url.is_empty() {
                push_url.clone()
            } else {
                fetch_url.clone()
            };
            RemoteInfo {
                name,
                url,
                fetch_url,
                push_url,
            }
        })
        .collect())
}

/// Result of a merge operation.
#[derive(Debug, Clone, Serialize)]
pub struct MergeResult {
    pub success: bool,
    pub conflicts: Vec<String>,
}

/// Merge a branch into the current branch.
pub fn merge(repo_root: &Path, branch: &str) -> GitResult<MergeResult> {
    let output = git_command()
        .current_dir(repo_root)
        .args(["merge", "--no-edit", branch])
        .output()
        .map_err(GitError::Exec)?;

    if output.status.success() {
        return Ok(MergeResult {
            success: true,
            conflicts: Vec::new(),
        });
    }

    let conflicts = collect_conflict_paths(repo_root)?;
    Ok(MergeResult {
        success: false,
        conflicts,
    })
}

fn collect_conflict_paths(repo_root: &Path) -> GitResult<Vec<String>> {
    let output = run_git(repo_root, &["diff", "--name-only", "--diff-filter=U"])?;
    Ok(output
        .lines()
        .filter(|l| !l.is_empty())
        .map(std::string::ToString::to_string)
        .collect())
}

/// Result of a rebase operation.
#[derive(Debug, Clone, Serialize)]
pub struct RebaseResult {
    pub success: bool,
    pub conflicts: Vec<String>,
}

/// Rebase the current branch onto `onto`.
pub fn rebase(repo_root: &Path, onto: &str) -> GitResult<RebaseResult> {
    let output = git_command()
        .current_dir(repo_root)
        .args(["rebase", onto])
        .output()
        .map_err(GitError::Exec)?;

    if output.status.success() {
        return Ok(RebaseResult {
            success: true,
            conflicts: Vec::new(),
        });
    }

    let conflicts = collect_conflict_paths(repo_root)?;
    Ok(RebaseResult {
        success: false,
        conflicts,
    })
}

/// Cherry-pick a single commit.
pub fn cherry_pick(repo_root: &Path, commit: &str) -> GitResult<()> {
    run_git(repo_root, &["cherry-pick", commit])?;
    Ok(())
}

/// Tag info returned by [`list_tags`].
#[derive(Debug, Clone, Serialize)]
pub struct TagInfo {
    pub name: String,
    pub hash: String,
    pub message: Option<String>,
}

/// Create a tag with an optional message.
pub fn tag(repo_root: &Path, name: &str, message: Option<&str>) -> GitResult<()> {
    if let Some(msg) = message {
        run_git(repo_root, &["tag", "-a", name, "-m", msg])?;
    } else {
        run_git(repo_root, &["tag", name])?;
    }
    Ok(())
}

/// List all tags.
pub fn list_tags(repo_root: &Path) -> GitResult<Vec<TagInfo>> {
    let output = run_git(
        repo_root,
        &[
            "tag",
            "-l",
            "--format=%(objectname:short)\t%(refname:short)\t%(subject)",
        ],
    )?;

    let tags = output
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            TagInfo {
                hash: parts.first().copied().unwrap_or("").to_string(),
                name: parts.get(1).copied().unwrap_or("").to_string(),
                message: parts.get(2).and_then(|s| {
                    let s = s.to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                }),
            }
        })
        .collect();
    Ok(tags)
}

const BLOCKED_GIT_FLAGS: &[&str] = &[
    "-c",
    "--exec",
    "--upload-pack",
    "--receive-pack",
    "--config",
    "--exec-path",
];

const ALLOWED_GIT_SUBCOMMANDS: &[&str] = &[
    "add",
    "am",
    "apply",
    "archive",
    "bisect",
    "blame",
    "branch",
    "cat-file",
    "cherry-pick",
    "checkout",
    "clean",
    "clone",
    "commit",
    "describe",
    "diff",
    "diff-tree",
    "fetch",
    "for-each-ref",
    "format-patch",
    "gc",
    "grep",
    "hash-object",
    "init",
    "log",
    "ls-files",
    "ls-remote",
    "ls-tree",
    "merge",
    "pack-refs",
    "prune",
    "pull",
    "push",
    "rebase",
    "reflog",
    "remote",
    "reset",
    "revert",
    "rev-parse",
    "shortlog",
    "show",
    "stash",
    "status",
    "submodule",
    "tag",
    "worktree",
];

fn validate_git_args(args: &[&str]) -> GitResult<()> {
    let subcommand = args.first().copied().unwrap_or("");
    if !ALLOWED_GIT_SUBCOMMANDS.contains(&subcommand) {
        return Err(GitError::Command {
            message: format!("git subcommand '{subcommand}' is not allowed"),
        });
    }
    for arg in args.iter().skip(1) {
        let lower = arg.to_lowercase();
        for blocked in BLOCKED_GIT_FLAGS {
            if lower == *blocked || lower.starts_with(&format!("{blocked}=")) {
                return Err(GitError::Command {
                    message: format!("git flag '{arg}' is not allowed"),
                });
            }
        }
    }
    Ok(())
}

/// Run an arbitrary (validated) git subcommand. Only allowed subcommands are
/// accepted, and dangerous flags are blocked.
pub fn run(repo_root: &Path, args: &[&str]) -> GitResult<String> {
    validate_git_args(args)?;
    run_git(repo_root, args)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_repo_with_commit() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path();
        std::process::Command::new("git")
            .current_dir(p)
            .args(["init", "-b", "main"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(p)
            .args(["config", "user.email", "t@t.com"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(p)
            .args(["config", "user.name", "T"])
            .output()
            .unwrap();
        fs::write(p.join("init.txt"), "init").unwrap();
        std::process::Command::new("git")
            .current_dir(p)
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .current_dir(p)
            .args(["commit", "-m", "init"])
            .output()
            .unwrap();
        tmp
    }

    #[test]
    fn stage_and_commit() {
        let tmp = init_repo_with_commit();
        fs::write(tmp.path().join("new.txt"), "data").unwrap();

        let new_path = Path::new("new.txt");
        stage(tmp.path(), &[new_path]).unwrap();
        let hash = commit(tmp.path(), "add new file").unwrap();
        assert!(!hash.is_empty());
    }

    #[test]
    fn create_and_checkout_branch() {
        let tmp = init_repo_with_commit();
        create_branch(tmp.path(), "feature", None).unwrap();

        let branch = crate::repo::current_branch(tmp.path()).unwrap();
        assert_eq!(branch, "feature");

        checkout(tmp.path(), "main").unwrap();
        let branch = crate::repo::current_branch(tmp.path()).unwrap();
        assert_eq!(branch, "main");
    }

    #[test]
    fn list_branches() {
        let tmp = init_repo_with_commit();
        create_branch(tmp.path(), "dev", None).unwrap();
        checkout(tmp.path(), "main").unwrap();

        let all = branches(tmp.path()).unwrap();
        assert!(all.iter().any(|b| b.name == "main" && b.current));
        assert!(all.iter().any(|b| b.name == "dev" && !b.current));
    }

    #[test]
    fn delete_branch_works() {
        let tmp = init_repo_with_commit();
        create_branch(tmp.path(), "temp", None).unwrap();
        checkout(tmp.path(), "main").unwrap();
        delete_branch(tmp.path(), "temp").unwrap();
        let all = branches(tmp.path()).unwrap();
        assert!(!all.iter().any(|b| b.name == "temp"));
    }

    #[test]
    fn validate_git_args_blocks_dangerous_flags() {
        assert!(validate_git_args(&["rm", "-rf", "/"]).is_err());
        assert!(validate_git_args(&["status"]).is_ok());
        assert!(validate_git_args(&["log", "--config=x"]).is_err());
    }

    #[test]
    fn run_validated() {
        let tmp = init_repo_with_commit();
        let output = run(tmp.path(), &["status", "--porcelain"]).unwrap();
        assert!(output.is_empty() || output.contains(' '));
    }

    #[test]
    fn show_file_works() {
        let tmp = init_repo_with_commit();
        let content = show_file(tmp.path(), "init.txt").unwrap();
        assert_eq!(String::from_utf8(content).unwrap(), "init");
    }

    #[test]
    fn init_works() {
        let tmp = TempDir::new().unwrap();
        init(tmp.path()).unwrap();
        assert!(tmp.path().join(".git").exists());
    }
}
