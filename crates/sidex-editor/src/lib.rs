//! Editor core — cursor, selection, undo/redo, and edit operations for `SideX`.
//!
//! This crate provides the editing logic layer built on top of [`sidex_text`].
//! It manages cursors, selections, multi-cursor editing, undo/redo history,
//! and document-level operations like line movement, commenting, and indentation.
//! Also includes a snippet engine, completion types with fuzzy matching,
//! viewport management, and text decoration support.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::similar_names
)]

pub mod completion;
pub mod contrib;
pub mod cursor;
pub mod cursors;
pub mod decoration;
pub mod diff;
pub mod document;
pub mod editing;
pub mod multi_cursor;
pub mod notebook;
pub mod scroll;
pub mod selection;
pub mod snippet;
pub mod undo;
pub mod viewport;
pub mod word;

pub use completion::{
    fuzzy_filter, fuzzy_score, CompletionItem, CompletionItemKind, CompletionList,
    CompletionTrigger, CompletionTriggerKind,
};
pub use cursor::CursorState;
pub use decoration::{Color, Decoration, DecorationCollection, DecorationOptions, DecorationSetId};
pub use diff::{
    apply_resolution, compute_diff_from_strings, parse_conflict_markers, ChangeKind, CharDiffKind,
    CharDiffPart, ConflictMarkerRegion, DiffChange, DiffEditor, DiffEditorSide, DiffGutterMark,
    DiffHunkAction, DiffResult, DiffViewMode, DiffViewState, InlineDiffKind, InlineDiffLine,
    InlineDiffLineKind, InlineDiffPart, MergeConflict, MergeEditor, MergeResolution,
};
pub use document::{
    AutoClosingEditStrategy, AutoClosingStrategy, AutoIndentStrategy, AutoSurroundStrategy,
    CompositionOutcome, Document, EditOperationType, EditorConfig,
};
pub use multi_cursor::MultiCursor;
pub use notebook::{CellKind, CellOutput, Notebook, NotebookCell};
pub use selection::Selection;
pub use snippet::{parse_snippet, Snippet, SnippetPart, SnippetSession};
pub use undo::{EditGroup, UndoEdit, UndoGroup, UndoRedoStack, UndoStack};
pub use viewport::{
    content_height, content_width, lines_per_page, process_wheel_event, process_wheel_event_fast,
    scroll_shadow_opacity, scrollbar_click_to_scroll, scrollbar_thumb_position,
    scrollbar_thumb_size, should_show_scroll_shadow, ScrollAlign, ScrollSettings, ScrollState,
    Viewport,
};
pub use word::{find_word_end, find_word_start, word_at};

pub use cursors::{CursorController, Direction, SelectionDirection};
pub use editing::EditController;
pub use scroll::{ExtendedScrollState, ScrollAnimation};

pub use contrib::emmet;
pub use contrib::local_history::{HistoryEntry, LocalHistory};

pub use contrib::find_widget::{
    find_all_matches, replace_all as find_widget_replace_all, replace_match,
    FindWidget as ContribFindWidget, FindWidgetFocus, FindWidgetOptions,
};
pub use contrib::go_to_line::{
    parse_go_to_input, GoToLineDialog, GoToLineState, GoToTarget,
};

pub use contrib::diagnostics::{
    compute_diagnostic_decorations, diagnostic_at_position, diagnostics_on_line,
    highest_severity_on_line, next_diagnostic, prev_diagnostic, Diagnostic as EditorDiagnostic,
    DiagnosticDecoration, DiagnosticSeverity as EditorDiagnosticSeverity,
};
pub use contrib::error_navigation::{
    ErrorNavigationState, ErrorSeverity, NavigationTarget, WorkspaceErrorNavigation,
};

pub use contrib::peek_view::{
    group_references, Location, PeekBreadcrumb, PeekController, PeekMode,
    PeekPreviewScroll, PeekViewState, ReferenceFileGroup, ReferenceItem,
};

pub use contrib::git_blame::{BlameAnnotation, BlameState, BlameStyle};
pub use contrib::git_decorations::{
    compute_git_decorations, DecorationSummary, GitDecorations, GitGutterStyle, LineChange,
    LineChangeKind,
};
pub use contrib::inline_diff::{
    compute_inline_changes, InlineChange, InlineChangeKind, InlineDiffColors,
    InlineDiffState as ContribInlineDiffState,
};

pub use contrib::minimap::{
    compute_minimap_data, MinimapConfig, MinimapDecorationInput, MinimapDecorationKind,
    MinimapLine, MinimapRenderData, MinimapRenderMode, MinimapSide, MinimapState, MinimapToken,
    SliderVisibility, SyntaxToken,
};
pub use contrib::breadcrumbs::{
    compute_breadcrumbs, compute_breadcrumbs_with_root, BreadcrumbAction, BreadcrumbDropdown,
    BreadcrumbIcon, BreadcrumbKind, BreadcrumbSegment, BreadcrumbsState, DocumentSymbol,
};
pub use contrib::scroll_decorations::{
    compute_scroll_marks, ScrollDecorations, ScrollMark, ScrollMarkKind, ScrollMarkRect,
};
pub use contrib::indent_guide::{
    compute_bracket_pair_guides, compute_indent_guides, compute_indent_guides_with_config,
    BracketPair, BracketPairGuide, IndentGuide, IndentGuidesConfig,
};
pub use contrib::color_decorators::{
    detect_colors, parse_css_color, ColorDecorator, ColorDecoratorState,
};

pub use contrib::testing::{
    builtin_test_patterns, detect_test_functions, LineCoverageState, TestAction,
    TestDecoration, TestDecorationController, TestGutterAction, TestInlineError,
    TestMatchKind, TestMatchRule, TestPattern, TestState as EditorTestState,
};
pub use contrib::breakpoints::{
    compute_breakpoint_ranges, BreakpointDecoration, BreakpointGutterController,
    BreakpointKind as EditorBreakpointKind, BreakpointVisual, ExecutionPoint,
    GutterClickResult, GutterContextAction, InlineBreakpoint,
};
pub use contrib::coverage::{
    parse_istanbul, parse_lcov, BranchCoverage, CoverageData, CoverageSummary,
    FileCoverage, FunctionCoverage, LineCoverage,
};

pub use contrib::conflict_decorations::{
    detect_conflict_markers, ConflictAction, ConflictCodeLens, ConflictCodeLensAction,
    ConflictDecoration, ConflictMarkerDecoration, ConflictRegionKind,
};

pub use contrib::inline_suggest::{
    InlineSuggestController, InlineSuggestState, InlineSuggestTriggerKind, InlineSuggestion,
};
pub use contrib::suggest_widget::{
    DocsSide, PopupDirection, SuggestItem, SuggestWidget,
};
pub use contrib::parameter_hints::{
    ParameterDisplay, ParameterHintsWidget, SignatureDisplay,
};
pub use contrib::peek_view::{
    PeekEmbeddedEditor, PeekEntry, PeekResizeState,
};
