//! Integrated terminal for `SideX` — PTY management and terminal emulation.
//!
//! This crate provides:
//!
//! - **PTY process management** ([`pty`]) — spawn shells, send input, resize,
//!   read output via ring buffer, kill process trees.
//! - **Shell detection** ([`shell`]) — platform-specific default shell,
//!   available shells, and shell integration (zdotdir).
//! - **Terminal grid** ([`grid`]) — character grid with scrollback buffer,
//!   selection support, wide characters, and text search.
//! - **ANSI emulator** ([`emulator`]) — VTE-based escape sequence parser that
//!   drives the grid with full CSI/SGR/DEC/OSC support and alternate screen.
//! - **ANSI parser** ([`ansi`]) — standalone state-machine parser for
//!   VT100/VT220/xterm escape sequences.
//! - **Selection** ([`selection`]) — terminal text selection with normal, word,
//!   line, and block modes.
//! - **Instance manager** ([`manager`]) — manage multiple terminal sessions
//!   with event channels.
//! - **Command execution** ([`exec`]) — non-interactive command execution with
//!   timeout support.
//! - **Link detection** ([`link_detection`]) — detect URLs and file paths in
//!   terminal output.
//! - **Shell integration** ([`shell_integration`]) — command tracking,
//!   decorations, and shell init scripts.
//! - **Renderer** ([`renderer`]) — GPU rendering primitives for terminal display.

pub mod ansi;
pub mod emulator;
pub mod exec;
pub mod find;
pub mod grid;
pub mod link_detection;
pub mod manager;
pub mod pty;
pub mod renderer;
pub mod selection;
pub mod shell;
pub mod shell_integration;

pub use ansi::{AnsiAction, AnsiParser, OscCommand, ParserState};
pub use emulator::{MouseEncoding, MouseTracking, TerminalEmulator};
pub use exec::{exec, ExecResult};
pub use grid::{
    Cell, CellAttributes, Color, NamedColor, Scrollback, SelectionMode, SelectionPoint,
    TerminalCursor, TerminalGrid, TerminalSelection,
};
pub use link_detection::{
    detect_links, detect_links_in_grid, detect_links_with_cwd, parse_file_link, LinkKind,
    TerminalLink,
};
pub use manager::{
    detect_profiles, ManagerError, SplitGroup, SplitOrientation, TerminalEvent, TerminalId,
    TerminalInstance, TerminalManager, TerminalProfile, TerminalState,
};
pub use pty::{
    kill_process_tree, send_signal, OutputChunk, PtyError, PtyProcess, PtySpawnConfig, ReadResult,
    TermHandle, TermInfo, TerminalSize,
};
pub use renderer::{
    render_terminal, CursorShape, FontMetrics, GlyphInstance, LineInstance, RectInstance,
    TerminalRenderer, TerminalRenderOutput, UnderlineStyle,
};
pub use selection::{
    expand_selection_line, expand_selection_word, extend_selection, is_selected, selected_text,
    start_selection, update_selection,
};
pub use shell::{
    available_shells, best_shell, check_shell_exists, detect_default_shell, setup_zsh_dotdir,
    ShellInfo,
};
pub use find::{find_in_terminal, FindOptions, TerminalFind, TerminalMatch};
pub use shell_integration::{
    detect_shell, generate_shell_init, parse_shell_integration_osc, CommandEntry,
    ShellIntegration, ShellIntegrationEvent, ShellType,
};
