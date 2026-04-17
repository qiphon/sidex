//! Git integration for `SideX`.
//!
//! All operations shell out to the `git` CLI via `std::process::Command`.

pub mod blame;
mod cmd;
pub mod diff;
pub mod error;
pub mod log;
pub mod operations;
pub mod repo;
pub mod status;

pub use blame::BlameLine;
pub use diff::{
    apply_hunks, compute_hunks, format_unified_diff, revert_hunk, DiffHunk, DiffLine, DiffLineKind,
    LineDiff, LineDiffKind,
};
pub use error::{GitError, GitResult};
pub use log::{log_graph, Commit, GitGraphEntry, GitRef, GraphCommit};
pub use operations::{
    branches, checkout, cherry_pick, clone, commit, create_branch, delete_branch,
    delete_branch_force, fetch, fetch_all, get_config, get_remotes, init, list_branches,
    list_submodules, list_tags, merge, pull, pull_detailed, push, push_detailed, rebase,
    remote_list, rename_branch, run, set_config, show_file, stage, stash, stash_action,
    stash_apply, stash_drop, stash_drop_index, stash_list, stash_list_parsed, stash_pop,
    submodule_init, submodule_update, tag, unstage, BranchInfo, GitBranch, GitRemote, MergeResult,
    PullResult, PushResult, RebaseResult, RemoteInfo, StashAction, StashEntry, SubmoduleInfo,
    TagInfo,
};
pub use repo::{current_branch, find_repo_root, is_git_repo, remotes};
pub use status::{FileStatus, StatusEntry};
