//! Workbench panel implementations.
//!
//! Each panel corresponds to a major VS Code sidebar or bottom-panel view:
//! File Explorer, Search, Source Control, Debug, Problems, Output, Terminal,
//! Extensions, Settings, Welcome, and Timeline.

pub mod debug_console;
pub mod debug_panel;
pub mod extensions_panel;
pub mod file_explorer;
pub mod keybindings_editor;
pub mod output_panel;
pub mod problems_panel;
pub mod scm_commit;
pub mod scm_panel;
pub mod search_panel;
pub mod settings_editor;
pub mod settings_panel;
pub mod terminal_panel;
pub mod test_panel;
pub mod timeline_panel;
pub mod welcome_page;
pub mod welcome_panel;
