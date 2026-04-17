//! Workspace management — file tree, watcher, search, indexing, path utilities for `SideX`.

pub mod dirty_diff;
pub mod error;
pub mod file_associations;
pub mod file_decorations;
pub mod file_operations;
pub mod file_ops;
pub mod file_tree;
pub mod file_watcher_events;
pub mod fuzzy_file_finder;
pub mod index;
pub mod multi_root;
pub mod outline;
pub mod path_util;
pub mod search;
pub mod timeline;
pub mod trust;
pub mod watcher;
pub mod workspace;

pub use dirty_diff::{
    compute_dirty_diff, get_original_content, DiffHunk as DirtyDiffHunk, DirtyDiffKind,
    DirtyDiffProvider, FileDirtyDiff,
};
pub use error::{WorkspaceError, WorkspaceResult};
pub use file_associations::{FileAssociations, LanguageAssociation};
pub use file_decorations::{
    compute_diagnostic_decorations, compute_git_decorations, propagate_decorations,
    DecorationProvider, DiagnosticSummary, FileDecorationService, GitFileStatus, GitStatusInput,
};
pub use file_operations::{
    DirEntryInfo, FileOperationEvent, FileOperationService, FileStatInfo,
    FileType as FileOperationType,
};
pub use file_ops::{DirEntry, FileStat};
pub use file_tree::{
    FileClipboard, FileDecoration, FileDecorations, FileNestingRule, FileNode, FileSelection,
    FileSortOrder, FileTree, OpenEditorEntry, OpenEditorsSection,
};
pub use file_watcher_events::{
    ConflictResolution, EventThrottler, FileConflict, FileConflictResolver, FileWatcherReaction,
};
pub use fuzzy_file_finder::{FileIndex, FileMatch as FuzzyFileMatch};
pub use index::{IndexOptions, IndexSearchOptions, IndexSearchResult, IndexStats, InvertedIndex};
pub use multi_root::{
    is_workspace_file, parse_workspace_file, save_workspace_file, FolderSettings,
    MultiRootWorkspace, WorkspaceConfig, WorkspaceExtensions, WorkspaceFolder,
};
pub use outline::{DocumentSymbol, OutlinePanel, OutlineSortOrder, SymbolKind};
pub use path_util::PathInfo;
pub use search::{
    CancellationToken, FileEdit, FileMatch, FileReplacement, FileSearchOptions, ReplaceReport,
    ReplacementEdit, SearchEngine, SearchMatchWithContext, SearchOptions, SearchProgress,
    SearchProgressCallback, SearchProgressInfo, SearchQuery, SearchResult, SearchResultCache,
    SearchResultGroup,
};
pub use timeline::{Timeline, TimelineEntry, TimelineIcon, TimelineSource};
pub use trust::{
    ExtensionTrustCapability, RestrictedFeature, RestrictedMode, RestrictedModeBanner, TrustPrompt,
    TrustPromptResponse, TrustState, WorkspaceTrust,
};
pub use watcher::{FileEvent, FileEventKind, FileWatcher};
pub use workspace::Workspace;
