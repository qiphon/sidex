use serde::{Deserialize, Serialize};
use std::path::Path;

use sidex_git::{
    BlameLine, BranchInfo, MergeResult, PullResult, PushResult, RebaseResult, RemoteInfo,
    StashEntry, SubmoduleInfo, TagInfo,
};

use super::validation::validate_path;

#[derive(Debug, Serialize, Deserialize)]
pub struct GitChange {
    pub path: String,
    pub status: String,
    pub staged: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: String,
    pub changes: Vec<GitChange>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_hashes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files_changed: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insertions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deletions: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    pub remote: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitRemote {
    pub name: String,
    pub url: String,
    #[serde(rename = "type")]
    pub remote_type: String,
}

#[allow(clippy::needless_pass_by_value)]
fn git_err(e: sidex_git::GitError) -> String {
    format!("Git error: {e}")
}

fn file_status_str(s: sidex_git::status::FileStatus) -> &'static str {
    match s {
        sidex_git::status::FileStatus::Modified => "modified",
        sidex_git::status::FileStatus::Added => "added",
        sidex_git::status::FileStatus::Deleted => "deleted",
        sidex_git::status::FileStatus::Renamed => "renamed",
        sidex_git::status::FileStatus::Copied => "copied",
        sidex_git::status::FileStatus::Untracked => "untracked",
        sidex_git::status::FileStatus::Ignored => "ignored",
        sidex_git::status::FileStatus::Conflicted => "conflicted",
    }
}

#[tauri::command]
pub async fn git_status(path: String) -> Result<GitStatus, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);

    let branch = sidex_git::current_branch(repo).unwrap_or_else(|_| "HEAD".to_string());

    let entries = sidex_git::status::get_status(repo).map_err(git_err)?;

    let changes = entries
        .into_iter()
        .map(|e| GitChange {
            path: e.path,
            status: file_status_str(e.status).to_string(),
            staged: e.staged,
        })
        .collect();

    Ok(GitStatus { branch, changes })
}

#[tauri::command]
pub async fn git_diff(path: String, file: Option<String>, staged: bool) -> Result<String, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);

    match (file, staged) {
        (Some(f), true) => sidex_git::diff::get_diff_staged(repo, Path::new(&f)).map_err(git_err),
        (Some(f), false) => sidex_git::diff::get_diff(repo, Path::new(&f)).map_err(git_err),
        (None, staged) => {
            let mut args = vec!["diff"];
            if staged {
                args.push("--staged");
            }
            sidex_git::run(repo, &args).map_err(git_err)
        }
    }
}

#[tauri::command]
pub async fn git_log(path: String, limit: Option<u32>) -> Result<Vec<GitLogEntry>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    let count = limit.unwrap_or(50) as usize;

    let commits = sidex_git::log::get_log(repo, count).map_err(git_err)?;

    let entries = commits
        .into_iter()
        .map(|c| GitLogEntry {
            hash: c.hash,
            message: c.message,
            author: c.author,
            date: c.date,
            parent_hashes: None,
            email: None,
            files_changed: None,
            insertions: None,
            deletions: None,
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
pub async fn git_add(path: String, files: Vec<String>) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    let paths: Vec<&Path> = files.iter().map(|f| Path::new(f.as_str())).collect();
    sidex_git::stage(repo, &paths).map_err(git_err)
}

#[tauri::command]
pub async fn git_commit(path: String, message: String) -> Result<String, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::commit(repo, &message).map_err(git_err)
}

#[tauri::command]
pub async fn git_checkout(path: String, branch: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::checkout(repo, &branch).map_err(git_err)
}

#[tauri::command]
pub async fn git_restore(
    path: String,
    files: Vec<String>,
    source: Option<String>,
    staged: bool,
    worktree: bool,
) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::restore(repo, &files, source.as_deref(), staged, worktree).map_err(git_err)
}

#[tauri::command]
pub async fn git_clean(path: String, files: Vec<String>, dirs: bool) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::clean(repo, &files, dirs).map_err(git_err)
}

#[tauri::command]
pub async fn git_checkout_file(
    path: String,
    treeish: String,
    files: Vec<String>,
) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::checkout_files(repo, &treeish, &files).map_err(git_err)
}

#[tauri::command]
pub async fn git_branches(path: String) -> Result<Vec<GitBranch>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);

    let crate_branches = sidex_git::branches(repo).map_err(git_err)?;

    let branches = crate_branches
        .into_iter()
        .map(|b| GitBranch {
            name: b.name,
            current: b.current,
            remote: b.remote,
        })
        .collect();

    Ok(branches)
}

#[tauri::command]
pub async fn git_init(path: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::init(repo).map_err(git_err)
}

#[tauri::command]
pub async fn git_is_repo(path: String) -> Result<bool, String> {
    validate_path(&path)?;
    Ok(sidex_git::is_git_repo(Path::new(&path)))
}

#[tauri::command]
pub async fn git_push(
    path: String,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<String, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::push(repo, remote.as_deref(), branch.as_deref()).map_err(git_err)
}

#[tauri::command]
pub async fn git_pull(
    path: String,
    remote: Option<String>,
    branch: Option<String>,
) -> Result<String, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::pull(repo, remote.as_deref(), branch.as_deref()).map_err(git_err)
}

#[tauri::command]
pub async fn git_fetch(path: String, remote: Option<String>) -> Result<String, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::fetch(repo, remote.as_deref()).map_err(git_err)
}

#[tauri::command]
pub async fn git_stash(
    path: String,
    action: String,
    message: Option<String>,
) -> Result<String, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);

    let stash_action = match action.as_str() {
        "push" => sidex_git::StashAction::Push,
        "pop" => sidex_git::StashAction::Pop,
        "list" => sidex_git::StashAction::List,
        "drop" => sidex_git::StashAction::Drop,
        other => return Err(format!("Unknown stash action: {other}")),
    };

    sidex_git::stash_action(repo, stash_action, message.as_deref()).map_err(git_err)
}

#[tauri::command]
pub async fn git_create_branch(
    path: String,
    name: String,
    start_point: Option<String>,
) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::create_branch(repo, &name, start_point.as_deref()).map_err(git_err)
}

#[tauri::command]
pub async fn git_delete_branch(path: String, name: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::delete_branch(repo, &name).map_err(git_err)
}

#[tauri::command]
pub async fn git_remote_list(path: String) -> Result<Vec<GitRemote>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);

    let crate_remotes = sidex_git::remote_list(repo).map_err(git_err)?;

    let remotes = crate_remotes
        .into_iter()
        .map(|r| GitRemote {
            name: r.name,
            url: r.url,
            remote_type: r.remote_type,
        })
        .collect();

    Ok(remotes)
}

#[tauri::command]
pub async fn git_clone(url: String, path: String) -> Result<(), String> {
    if let Ok(parsed) = reqwest::Url::parse(&url) {
        match parsed.scheme() {
            "https" | "http" | "ssh" | "git" => {}
            scheme => return Err(format!("git clone: blocked URL scheme '{scheme}'")),
        }
    }

    sidex_git::clone(&url, Path::new(&path)).map_err(git_err)
}

#[tauri::command]
pub async fn git_reset(path: String, files: Vec<String>) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    let paths: Vec<&Path> = files.iter().map(|f| Path::new(f.as_str())).collect();
    sidex_git::unstage(repo, &paths).map_err(git_err)
}

#[tauri::command]
pub async fn git_show(path: String, file: String) -> Result<Vec<u8>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::show_file(repo, &file).map_err(git_err)
}

#[tauri::command]
pub async fn git_run(path: String, args: Vec<String>) -> Result<String, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    sidex_git::run(repo, &arg_refs).map_err(git_err)
}

#[tauri::command]
pub async fn git_log_graph(path: String, limit: Option<u32>) -> Result<Vec<GitLogEntry>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    let count = limit.unwrap_or(50) as usize;

    let commits = sidex_git::log::get_log_graph(repo, count).map_err(git_err)?;

    let entries = commits
        .into_iter()
        .map(|c| GitLogEntry {
            hash: c.hash,
            message: c.message,
            author: c.author,
            date: c.date,
            parent_hashes: c.parent_hashes,
            email: c.email,
            files_changed: c.files_changed,
            insertions: c.insertions,
            deletions: c.deletions,
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
pub async fn git_blame(path: String, file: String) -> Result<Vec<BlameLine>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::blame::blame(repo, Path::new(&file)).map_err(git_err)
}

#[tauri::command]
pub async fn git_tag(path: String, name: String, message: Option<String>) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::tag(repo, &name, message.as_deref()).map_err(git_err)
}

#[tauri::command]
pub async fn git_list_tags(path: String) -> Result<Vec<TagInfo>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::list_tags(repo).map_err(git_err)
}

#[tauri::command]
pub async fn git_merge(path: String, branch: String) -> Result<MergeResult, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::merge(repo, &branch).map_err(git_err)
}

#[tauri::command]
pub async fn git_rebase(path: String, branch: String) -> Result<RebaseResult, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::rebase(repo, &branch).map_err(git_err)
}

#[tauri::command]
pub async fn git_cherry_pick(path: String, commit: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::cherry_pick(repo, &commit).map_err(git_err)
}

#[tauri::command]
pub async fn git_rename_branch(
    path: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::rename_branch(repo, &old_name, &new_name).map_err(git_err)
}

#[tauri::command]
pub async fn git_delete_branch_force(path: String, name: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::delete_branch_force(repo, &name, true).map_err(git_err)
}

#[tauri::command]
pub async fn git_list_branches(path: String) -> Result<Vec<BranchInfo>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::list_branches(repo, true).map_err(git_err)
}

#[tauri::command]
pub async fn git_push_detailed(
    path: String,
    remote: Option<String>,
    branch: Option<String>,
    force: bool,
) -> Result<PushResult, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    let remote_str = remote.unwrap_or_else(|| "origin".to_string());
    let branch_str = match branch {
        Some(b) => b,
        None => sidex_git::current_branch(repo).map_err(git_err)?,
    };
    sidex_git::push_detailed(repo, &remote_str, &branch_str, force).map_err(git_err)
}

#[tauri::command]
pub async fn git_pull_detailed(
    path: String,
    remote: Option<String>,
    branch: Option<String>,
    rebase: bool,
) -> Result<PullResult, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    let remote_str = remote.unwrap_or_else(|| "origin".to_string());
    let branch_str = match branch {
        Some(b) => b,
        None => sidex_git::current_branch(repo).map_err(git_err)?,
    };
    sidex_git::pull_detailed(repo, &remote_str, &branch_str, rebase).map_err(git_err)
}

#[tauri::command]
pub async fn git_fetch_all(path: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::fetch_all(repo).map_err(git_err)
}

#[tauri::command]
pub async fn git_stash_apply(path: String, index: Option<u32>) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    let idx = index.unwrap_or(0) as usize;
    sidex_git::stash_apply(repo, idx).map_err(git_err)?;
    Ok(())
}

#[tauri::command]
pub async fn git_stash_drop_index(path: String, index: u32) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::stash_drop_index(repo, index as usize).map_err(git_err)?;
    Ok(())
}

#[tauri::command]
pub async fn git_stash_list_parsed(path: String) -> Result<Vec<StashEntry>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::stash_list_parsed(repo).map_err(git_err)
}

#[tauri::command]
pub async fn git_get_config(path: String, key: String) -> Result<Option<String>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::get_config(repo, &key).map_err(git_err)
}

#[tauri::command]
pub async fn git_set_config(path: String, key: String, value: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::set_config(repo, &key, &value).map_err(git_err)
}

#[tauri::command]
pub async fn git_submodule_init(path: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::submodule_init(repo).map_err(git_err)
}

#[tauri::command]
pub async fn git_submodule_update(path: String) -> Result<(), String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::submodule_update(repo).map_err(git_err)
}

#[tauri::command]
pub async fn git_list_submodules(path: String) -> Result<Vec<SubmoduleInfo>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::list_submodules(repo).map_err(git_err)
}

#[tauri::command]
pub async fn git_get_remotes(path: String) -> Result<Vec<RemoteInfo>, String> {
    validate_path(&path)?;
    let repo = Path::new(&path);
    sidex_git::get_remotes(repo).map_err(git_err)
}
