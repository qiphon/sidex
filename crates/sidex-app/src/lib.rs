//! # sidex-app
//!
//! Main application binary for the `SideX` code editor.
//!
//! Re-exports the core [`App`] type and supporting modules for use by
//! integration tests or embedding scenarios.

pub mod app;
pub mod backup;
pub mod cli;
pub mod clipboard;
pub mod command_palette;
pub mod commands;
pub mod crash_reporter;
pub mod document_state;
pub mod editor_group;
pub mod editor_view;
pub mod event_bus;
pub mod event_loop;
pub mod file_dialog;
pub mod i18n;
pub mod input;

pub mod layout;
pub mod logging;
pub mod native;
pub mod native_menu;
pub mod navigation;
pub mod performance;
pub mod platform;
pub mod product;
pub mod quick_open;
pub mod recent;
pub mod services;
pub mod session;
pub mod startup;
pub mod status_bar_controller;
pub mod symbol_navigation;
pub mod tauri_bridge;
pub mod telemetry;
pub mod test_runner;
pub mod title_bar;
pub mod updater;
pub mod window_manager;
pub mod zoom;

pub use app::App;
pub use backup::{BackupEntry, BackupService};
pub use cli::{parse_cli_args, CliArgs, CliSubcommand, GotoLocation};
pub use clipboard::{
    ClipboardEntry, ClipboardService, ClipboardSource, copy_to_clipboard, cut_to_clipboard,
    paste_from_clipboard,
};
pub use command_palette::{CommandCategory, CommandPaletteItem, CommandPaletteState};
pub use commands::{CommandRegistry, NavigationEntry};
pub use crash_reporter::{CrashReport, CrashReporter};
pub use document_state::DocumentState;
pub use editor_group::{
    AutoSaveMode, ClosedTabInfo, EditorGroup as AppEditorGroup, EditorGroupLayout,
    EditorGroupManager, EditorTab, GroupLayout, GroupOrientation, TabIcon, TabId,
    TabSizingMode as AppTabSizingMode,
};
pub use event_bus::{Event, EventBus, EventType, ListenerId};
pub use i18n::{I18n, LocaleInfo};
pub use input::{
    DragState, ImeState, InputEvent, InputHandler, InputResult, KeyCode, KeyboardState,
    ModifierState, MouseButton, MouseButtons, MouseState, ScrollPhase,
};

pub use layout::{
    ActivityBarLayout, ActivityBarPosition, EditorAreaLayout, EditorGroupLayoutInfo,
    GroupOrientation as LayoutGroupOrientation, Layout, LayoutRects, MinimapLayout, MinimapSide,
    PanelLayout, PanelPosition, Rect, SidebarLayout, SidebarPosition, StatusBarLayout, TabLayout,
    TitleBarLayout, TitleBarStyle, WorkbenchLayout,
};
pub use logging::{LogEntry, LogLevel, LogService};
pub use native_menu::build_native_menu;
pub use navigation::{HistoryEntry, HistorySelection, NavigationHistory};
pub use platform::{Architecture, DesktopEnvironment, DisplayServer, OperatingSystem, Platform};
pub use product::ProductConfig;
pub use quick_open::{QuickOpenItem, QuickOpenMode, QuickOpenState};
pub use recent::{RecentItem, RecentManager};
pub use session::{HotExitData, OpenFileState, SessionWindowState};
pub use services::{
    AppContext, DatabaseService, DebugService, DialogService, EditorService,
    ExtensionService, FileService, GitService, I18nService, KeybindingService, LanguageService,
    NotificationService, SearchService, ServiceContainer, SettingsService,
    TaskService, TerminalService, ThemeService, UpdateService,
    WorkspaceService,
    ClipboardService as SvcClipboardService,
    LogService as SvcLogService,
    TelemetryService as SvcTelemetryService,
};
pub use symbol_navigation::{
    DocumentSymbol, SymbolInFileState, SymbolInWorkspaceState, SymbolItem, SymbolKind,
    WorkspaceSymbol,
};
pub use tauri_bridge::{BridgeCommand, BridgeResponse, TauriBridge};
pub use native::{
    ColorScheme, FileFilter as NativeFileFilter, MessageBoxOptions, MessageBoxResult,
    MessageBoxType, NativeIntegration, OpenDialogOptions, SaveDialogOptions,
};
pub use startup::{PhaseStatus, StartupPhase, StartupSequence};
pub use performance::PerformanceMonitor;
pub use telemetry::{TelemetryLevel, TelemetryService};
pub use test_runner::{
    parse_cargo_test_line, parse_go_test_line, parse_pytest_line, RunnerKind, TestError,
    TestLocation as TestRunnerLocation, TestResult, TestRun, TestRunKind as AppTestRunKind,
    TestRunProfile as AppTestRunProfile, TestRunState, TestRunner, TestState as AppTestState,
};
pub use updater::{UpdateInfo, UpdateMode, UpdateState, Updater};
pub use window_manager::{
    AppWindow, WindowBounds, WindowId, WindowManager, WindowState as WinState, format_title,
};
pub use title_bar::{MenuBarItem, MenuBarState, TitleBar, WindowControls};
pub use zoom::ZoomService;
