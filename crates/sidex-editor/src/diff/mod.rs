//! Diff and merge editing support for `SideX`.
//!
//! Provides side-by-side diff editing ([`diff_model`]), diff view state with
//! scroll synchronization ([`diff_view`]), and three-way merge conflict
//! resolution ([`merge_model`]).

pub mod diff_model;
pub mod diff_view;
pub mod merge_model;

pub use diff_model::{
    compute_diff, compute_inline_diff, ChangeKind, DiffChange, DiffEditor, DiffResult,
    InlineDiffKind, InlineDiffPart,
};
pub use diff_view::{
    compute_diff_from_strings, CharDiffKind, CharDiffPart, DiffEditorSide, DiffGutterMark,
    DiffHunkAction, DiffViewMode, DiffViewState, InlineDiffLine, InlineDiffLineKind,
};
pub use merge_model::{
    apply_resolution, parse_conflict_markers, ConflictMarkerRegion, MergeConflict, MergeEditor,
    MergeResolution,
};
