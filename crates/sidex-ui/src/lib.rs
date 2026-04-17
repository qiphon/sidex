//! UI framework and widget library for `SideX`.
//!
//! This crate provides:
//!
//! - [`layout`] — A flexbox-style layout engine for computing widget positions.
//! - [`widget`] — The core [`Widget`](widget::Widget) trait and event types.
//! - [`widgets`] — Built-in widget implementations (buttons, lists, trees,
//!   tabs, menus, etc.).
//! - [`workbench`] — VS Code workbench layout components (title bar, activity
//!   bar, sidebar, editor area, panel, status bar).
//! - [`panels`] — Workbench panel implementations (file explorer, search,
//!   source control, debug, problems, output, terminal, extensions, settings,
//!   welcome).
//! - [`accessibility`] — ARIA roles, screen reader support, focus traps.
//! - [`animation`] — Animation system with easing curves.
//! - [`focus`] — Focus management and focus ring rendering.

pub mod accessibility;
pub mod animation;
pub mod drag_drop;
pub mod draw;
pub mod focus;
pub mod icons;
pub mod layout;
pub mod panels;
pub mod widget;
pub mod widgets;
pub mod workbench;

// ── Convenience re-exports ───────────────────────────────────────────────────

pub use draw::{CursorIcon, DrawContext, IconId, TextStyle as DrawTextStyle};
pub use layout::{compute_layout, Direction, Edges, LayoutNode, Rect, Size};
pub use widget::{EventResult, Key, Modifiers, MouseButton, UiEvent, Widget};

pub use widgets::breadcrumbs::{BreadcrumbSegment, Breadcrumbs};
pub use widgets::button::{Button, ButtonStyle};
pub use widgets::context_menu::{ContextMenu, MenuItem};
pub use widgets::context_menu::{
    editor_context_menu, explorer_context_menu, gutter_context_menu, tab_context_menu,
};
pub use widgets::label::Label;
pub use widgets::list::{List, ListRow, SelectionMode};
pub use widgets::notification::{NotificationAction, NotificationToast, Severity};
pub use widgets::quick_pick::{QuickPick, QuickPickItem};
pub use widgets::scrollbar::{Orientation, OverviewMark, Scrollbar};
pub use widgets::split_pane::SplitPane;
pub use widgets::tabs::{Tab, TabBar, TabContextAction, TabContextMenu, TabSizingMode};
pub use widgets::text_input::TextInput;
pub use widgets::tooltip::{Tooltip, TooltipPosition};
pub use widgets::tree::{Tree, TreeNode, TreeRow};

pub use widgets::menu_bar::{MenuBar, MenuBarMenu, MenuBarMenuItem, default_menus};
pub use widgets::modal_dialog::{
    DialogButton, DialogCheckbox, DialogInput, DialogResult, ModalDialog,
};
pub use widgets::notification_center::{
    NotificationCenter, Notification as CenterNotification,
    NotificationAction as CenterNotificationAction, NotificationProgress, NotificationSeverity,
    ProgressHandle,
};
pub use widgets::status_bar_items::{StatusBarBackground, StatusBarController, StatusBarItems};

pub use widgets::find_widget::FindWidget;
pub use widgets::hover_widget::{CodeAction, DiagnosticSeverity as HoverDiagnosticSeverity, HoverSection, HoverWidget};
pub use widgets::parameter_hints_widget::{Parameter, ParameterHintsWidget, Signature};
pub use widgets::rename_input::{RenameInput, RenameLocation, RenameResult};
pub use widgets::search_input::{SearchInput, SearchToggle, ToggleIcon};
pub use widgets::suggest_widget::{
    CompletionItem as SuggestCompletionItem, CompletionItemKind as SuggestCompletionItemKind,
    SuggestWidget,
};

pub use workbench::activity_bar::{
    ActivityBar, ActivityBarContextAction, ActivityBarItem, SidebarView,
    default_activity_bar_items,
};
pub use workbench::editor_area::{DropZone, EditorArea, EditorGroup};
pub use workbench::panel::{Panel, PanelTab};
pub use workbench::sidebar::{Sidebar, SidebarSection};
pub use workbench::status_bar::{
    ShowWhen, StatusBar, StatusBarAlignment, StatusBarItem, StatusBarMode,
    default_status_bar_items,
};
pub use workbench::title_bar::{MenuBarItem, Platform, TitleBar};
pub use workbench::workbench::{
    PanelPosition, SashType, SidebarPosition, Workbench, WorkbenchCompositor, WorkbenchLayout,
    WorkbenchRegion,
};

// ── Accessibility re-exports ─────────────────────────────────────────────────

pub use accessibility::{
    AccessibilityService, AccessibilityState, AccessibleAction, AccessibleElement, AccessibleState,
    AriaLive, AriaRole, FocusTrap, TabOrder,
};

// ── Animation re-exports ─────────────────────────────────────────────────────

pub use animation::{
    Animation, AnimationComplete, AnimationGroup, AnimationRepeat, Easing, Lerp, Presets,
};

// ── Focus re-exports ─────────────────────────────────────────────────────────

pub use focus::{
    FocusDirection, FocusManager, FocusRing, FocusService, FocusZone, FocusableElement, WidgetId,
};

// ── Drag-drop re-exports ─────────────────────────────────────────────────────

pub use drag_drop::{
    DragData, DragDataKind, DragDropManager, DragEffect, DragPreview, DragSession, DragSource,
    DropTarget, DropZoneIndicator, DropZonePosition,
};

// ── Panel re-exports ─────────────────────────────────────────────────────────

pub use panels::debug_panel::{
    Breakpoint as DebugBreakpoint, BreakpointEntry, BreakpointKind, BreakpointsView,
    CallStackView, ConsoleEntry, ConsoleHistory, DataAccessType, DataBreakpoint,
    DebugAction, DebugPanel, DebugSections, DebugSessionState, DebugState, DebugThread,
    DebugToolbar, DebugToolbarButton, ExceptionBreakpoint, FramePresentation, FrameSource,
    FunctionBreakpoint, InlineValue, LoadedScript, OutputCategory, StackFrame, ThreadState,
    ThreadStatus, Variable, VariableScope, VariablesView, WatchEvent, WatchExpression,
    WatchView,
};
pub use panels::extensions_panel::{
    ExtensionAction, ExtensionBisect, ExtensionDetail, ExtensionDetailTab, ExtensionFilter,
    ExtensionInfo, ExtensionListItem, ExtensionRecommendation, ExtensionReview,
    ExtensionRuntimeStatus, ExtensionState, ExtensionView, ExtensionsPanel, RecommendationReason,
    WorkspaceRecommendations,
};
pub use panels::file_explorer::{
    CompactFolderChain, DragState, DropPosition, ExplorerAction, ExplorerFilter, FileDecoration,
    FileEntry, FileExplorer, FileIcon, FileNestingRule, InlineEditState, OpenEditor,
};
pub use panels::output_panel::{OutputChannel, OutputLevel, OutputLine, OutputPanel};
pub use panels::problems_panel::{
    Diagnostic, DiagnosticSeverity, FileDiagnostics, ProblemsFilter, ProblemsGrouping,
    ProblemsPanel, ProblemsSortOrder, QuickFix, QuickFixKind,
};
pub use panels::scm_panel::{
    ChangeGroup, ChangeStatus, CommitGraphEntry, CommitRef, CommitRefKind, DiffHunk, DiffLine,
    DiffLineKind, FileChange, InlineDiffState, MergeConflict, MergeResolution, ScmAction, ScmPanel,
    StashAction, StashEntry,
};
pub use panels::search_panel::{
    FileResultAction, FileSearchResult, ReplacePreview, ReplaceScope, SearchContextLine,
    SearchField, SearchGlobs, SearchHistory, SearchMatch, SearchOptions, SearchPanel,
    SearchProgressInfo, SearchStreamState,
};
pub use panels::settings_panel::{
    SettingControl, SettingEntry, SettingScope, SettingsCategory, SettingsFilterMode,
    SettingsPanel, SettingsViewMode,
};
pub use panels::terminal_panel::{
    DetectedCommand, ShellIntegration, ShellType, TabDragState, TerminalAction, TerminalFindState,
    TerminalInstance, TerminalLink, TerminalLinkKind, TerminalPanel, TerminalSplit,
};
pub use panels::welcome_panel::{
    RecentItem, ShortcutEntry, Walkthrough, WalkthroughStep, WelcomeAction, WelcomePanel,
};

pub use panels::debug_console::{
    ConsoleSuggestion, DebugConsole, DebugConsoleEntry, DebugConsoleEvent, DebugInputHistory,
    DebugOutputCategory,
};
pub use panels::keybindings_editor::{
    KeybindingContextAction, KeybindingDisplayEntry, KeybindingSourceFilter, KeybindingsEditor,
    KeybindingsEvent, SortColumn as KeybindingSortColumn, SortDirection as KeybindingSortDirection,
};
pub use panels::settings_editor::{
    SettingGroup, SettingType, SettingValueScope, SettingsEditor, SettingsScope,
    TocEntry,
    SettingEntry as SettingsEditorEntry,
};
pub use panels::welcome_page::{
    RecentItem as WelcomePageRecentItem, WalkthroughCategory, WalkthroughStep as WelcomePageWalkthroughStep,
    WelcomeItem, WelcomePage, WelcomePageAction, WelcomeSection as WelcomePageSection,
};
pub use panels::test_panel::{
    CoverageSummaryDisplay, TestExplorer, TestExplorerEvent, TestLocation as TestPanelLocation,
    TestNodeKind, TestRunKind, TestRunProfile as TestPanelRunProfile, TestSortOrder,
    TestState as TestPanelState, TestTreeNode,
};
