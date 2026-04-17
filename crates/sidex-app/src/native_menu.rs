//! Native menu bar construction.
//!
//! Builds the full VS Code-style menu structure using the types from
//! [`crate::tauri_bridge`].  The resulting [`NativeMenu`] can be
//! applied to a window via [`TauriBridge::set_menu`], or the
//! [`build_tauri_menu`] helper can construct a Tauri `Menu` directly
//! from an [`AppHandle`].

use crate::tauri_bridge::{NativeMenu, NativeMenuItem};

// ── Convenience constructors ────────────────────────────────────────

fn item(id: &str, label: &str, accel: Option<&str>) -> NativeMenuItem {
    NativeMenuItem::Item {
        id: id.to_owned(),
        label: label.to_owned(),
        accelerator: accel.map(str::to_owned),
        enabled: true,
    }
}

fn sep() -> NativeMenuItem {
    NativeMenuItem::Separator
}

fn submenu(label: &str, children: Vec<NativeMenuItem>) -> NativeMenuItem {
    NativeMenuItem::Submenu {
        label: label.to_owned(),
        children,
    }
}

// ── Menu builders ───────────────────────────────────────────────────

fn file_menu() -> NativeMenuItem {
    submenu(
        "File",
        vec![
            item("new_file", "New File", Some("CmdOrCtrl+N")),
            item("new_window", "New Window", Some("CmdOrCtrl+Shift+N")),
            sep(),
            item("open_file", "Open File...", Some("CmdOrCtrl+O")),
            item("open_folder", "Open Folder...", None),
            sep(),
            item("save", "Save", Some("CmdOrCtrl+S")),
            item("save_as", "Save As...", Some("CmdOrCtrl+Shift+S")),
            item("save_all", "Save All", Some("CmdOrCtrl+Alt+S")),
            sep(),
            item("close_editor", "Close Editor", Some("CmdOrCtrl+W")),
            item("close_window", "Close Window", Some("CmdOrCtrl+Shift+W")),
            sep(),
            item("exit", "Exit", Some("CmdOrCtrl+Q")),
        ],
    )
}

fn edit_menu() -> NativeMenuItem {
    submenu(
        "Edit",
        vec![
            item("undo", "Undo", Some("CmdOrCtrl+Z")),
            item("redo", "Redo", Some("CmdOrCtrl+Shift+Z")),
            sep(),
            item("cut", "Cut", Some("CmdOrCtrl+X")),
            item("copy", "Copy", Some("CmdOrCtrl+C")),
            item("paste", "Paste", Some("CmdOrCtrl+V")),
            sep(),
            item("find", "Find", Some("CmdOrCtrl+F")),
            item("replace", "Replace", Some("CmdOrCtrl+H")),
            sep(),
            item("find_in_files", "Find in Files", Some("CmdOrCtrl+Shift+F")),
            item(
                "replace_in_files",
                "Replace in Files",
                Some("CmdOrCtrl+Shift+H"),
            ),
            sep(),
            item(
                "toggle_line_comment",
                "Toggle Line Comment",
                Some("CmdOrCtrl+/"),
            ),
            item(
                "toggle_block_comment",
                "Toggle Block Comment",
                Some("CmdOrCtrl+Shift+/"),
            ),
        ],
    )
}

fn selection_menu() -> NativeMenuItem {
    submenu(
        "Selection",
        vec![
            item("select_all", "Select All", Some("CmdOrCtrl+A")),
            item(
                "expand_selection",
                "Expand Selection",
                Some("CmdOrCtrl+Shift+Right"),
            ),
            item(
                "shrink_selection",
                "Shrink Selection",
                Some("CmdOrCtrl+Shift+Left"),
            ),
            sep(),
            item(
                "add_cursor_above",
                "Add Cursor Above",
                Some("CmdOrCtrl+Alt+Up"),
            ),
            item(
                "add_cursor_below",
                "Add Cursor Below",
                Some("CmdOrCtrl+Alt+Down"),
            ),
            sep(),
            item(
                "select_all_occurrences",
                "Select All Occurrences",
                Some("CmdOrCtrl+Shift+L"),
            ),
        ],
    )
}

fn view_menu() -> NativeMenuItem {
    submenu(
        "View",
        vec![
            item(
                "command_palette",
                "Command Palette...",
                Some("CmdOrCtrl+Shift+P"),
            ),
            sep(),
            item("explorer", "Explorer", Some("CmdOrCtrl+Shift+E")),
            item("search", "Search", Some("CmdOrCtrl+Shift+F")),
            item(
                "source_control",
                "Source Control",
                Some("CmdOrCtrl+Shift+G"),
            ),
            item("debug", "Run and Debug", Some("CmdOrCtrl+Shift+D")),
            item("extensions", "Extensions", Some("CmdOrCtrl+Shift+X")),
            sep(),
            item("terminal", "Terminal", Some("CmdOrCtrl+`")),
            item("problems", "Problems", Some("CmdOrCtrl+Shift+M")),
            item("output", "Output", Some("CmdOrCtrl+Shift+U")),
            sep(),
            item("toggle_sidebar", "Toggle Sidebar", Some("CmdOrCtrl+B")),
            item("toggle_panel", "Toggle Panel", Some("CmdOrCtrl+J")),
            sep(),
            item("zen_mode", "Zen Mode", Some("CmdOrCtrl+K Z")),
            item("toggle_fullscreen", "Full Screen", Some("F11")),
            sep(),
            item("zoom_in", "Zoom In", Some("CmdOrCtrl+=")),
            item("zoom_out", "Zoom Out", Some("CmdOrCtrl+-")),
            item("reset_zoom", "Reset Zoom", Some("CmdOrCtrl+0")),
        ],
    )
}

fn go_menu() -> NativeMenuItem {
    submenu(
        "Go",
        vec![
            item("go_to_file", "Go to File...", Some("CmdOrCtrl+P")),
            item("go_to_symbol", "Go to Symbol...", Some("CmdOrCtrl+Shift+O")),
            sep(),
            item("go_to_definition", "Go to Definition", Some("F12")),
            item("go_to_line", "Go to Line/Column...", Some("CmdOrCtrl+G")),
            sep(),
            item("back", "Back", Some("CmdOrCtrl+Alt+Left")),
            item("forward", "Forward", Some("CmdOrCtrl+Alt+Right")),
        ],
    )
}

fn run_menu() -> NativeMenuItem {
    submenu(
        "Run",
        vec![
            item("start_debugging", "Start Debugging", Some("F5")),
            item(
                "run_without_debugging",
                "Start Without Debugging",
                Some("CmdOrCtrl+F5"),
            ),
            sep(),
            item("toggle_breakpoint", "Toggle Breakpoint", Some("F9")),
            item("add_configuration", "Add Configuration...", None),
        ],
    )
}

fn terminal_menu() -> NativeMenuItem {
    submenu(
        "Terminal",
        vec![
            item("new_terminal", "New Terminal", Some("CmdOrCtrl+Shift+`")),
            item("split_terminal", "Split Terminal", None),
            sep(),
            item("run_active_file", "Run Active File", None),
        ],
    )
}

fn help_menu() -> NativeMenuItem {
    submenu(
        "Help",
        vec![
            item("welcome", "Welcome", None),
            item("documentation", "Documentation", None),
            item("release_notes", "Release Notes", None),
            sep(),
            item("about", "About SideX", None),
        ],
    )
}

// ── Public API ──────────────────────────────────────────────────────

/// Build the complete VS Code-style native menu structure.
pub fn build_native_menu() -> NativeMenu {
    NativeMenu {
        items: vec![
            file_menu(),
            edit_menu(),
            selection_menu(),
            view_menu(),
            go_menu(),
            run_menu(),
            terminal_menu(),
            help_menu(),
        ],
    }
}

/// Build a Tauri [`Menu`] directly from an [`AppHandle`], matching the
/// existing `src-tauri/src/lib.rs` menu structure.
///
/// This is the equivalent of the old `build_menu` function, ported to
/// the new crate-based architecture.
#[cfg(target_os = "macos")]
#[allow(clippy::too_many_lines)]
pub fn build_tauri_menu(app: &tauri::AppHandle) -> anyhow::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(
            &MenuItemBuilder::with_id("new_file", "New File")
                .accelerator("CmdOrCtrl+N")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("new_window", "New Window")
                .accelerator("CmdOrCtrl+Shift+N")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("open_file", "Open File...")
                .accelerator("CmdOrCtrl+O")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("open_folder", "Open Folder...").build(app)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("save", "Save")
                .accelerator("CmdOrCtrl+S")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("save_as", "Save As...")
                .accelerator("CmdOrCtrl+Shift+S")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("save_all", "Save All")
                .accelerator("CmdOrCtrl+Alt+S")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("close_editor", "Close Editor")
                .accelerator("CmdOrCtrl+W")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("close_window", "Close Window")
                .accelerator("CmdOrCtrl+Shift+W")
                .build(app)?,
        )
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .separator()
        .item(
            &MenuItemBuilder::with_id("find", "Find")
                .accelerator("CmdOrCtrl+F")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("replace", "Replace")
                .accelerator("CmdOrCtrl+H")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("find_in_files", "Find in Files")
                .accelerator("CmdOrCtrl+Shift+F")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("replace_in_files", "Replace in Files")
                .accelerator("CmdOrCtrl+Shift+H")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("toggle_line_comment", "Toggle Line Comment")
                .accelerator("CmdOrCtrl+/")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("toggle_block_comment", "Toggle Block Comment")
                .accelerator("CmdOrCtrl+Shift+/")
                .build(app)?,
        )
        .build()?;

    let selection_menu = SubmenuBuilder::new(app, "Selection")
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .item(
            &MenuItemBuilder::with_id("expand_selection", "Expand Selection")
                .accelerator("CmdOrCtrl+Shift+Right")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("shrink_selection", "Shrink Selection")
                .accelerator("CmdOrCtrl+Shift+Left")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("add_cursor_above", "Add Cursor Above")
                .accelerator("CmdOrCtrl+Alt+Up")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("add_cursor_below", "Add Cursor Below")
                .accelerator("CmdOrCtrl+Alt+Down")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("select_all_occurrences", "Select All Occurrences")
                .accelerator("CmdOrCtrl+Shift+L")
                .build(app)?,
        )
        .build()?;

    let view_menu = SubmenuBuilder::new(app, "View")
        .item(
            &MenuItemBuilder::with_id("command_palette", "Command Palette...")
                .accelerator("CmdOrCtrl+Shift+P")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("explorer", "Explorer")
                .accelerator("CmdOrCtrl+Shift+E")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("search", "Search")
                .accelerator("CmdOrCtrl+Shift+F")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("source_control", "Source Control")
                .accelerator("CmdOrCtrl+Shift+G")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("debug", "Run and Debug")
                .accelerator("CmdOrCtrl+Shift+D")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("extensions", "Extensions")
                .accelerator("CmdOrCtrl+Shift+X")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("terminal", "Terminal")
                .accelerator("CmdOrCtrl+`")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("problems", "Problems")
                .accelerator("CmdOrCtrl+Shift+M")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("output", "Output")
                .accelerator("CmdOrCtrl+Shift+U")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("toggle_sidebar", "Toggle Sidebar")
                .accelerator("CmdOrCtrl+B")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("toggle_panel", "Toggle Panel")
                .accelerator("CmdOrCtrl+J")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("toggle_fullscreen", "Full Screen")
                .accelerator("F11")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("zoom_in", "Zoom In")
                .accelerator("CmdOrCtrl+=")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("zoom_out", "Zoom Out")
                .accelerator("CmdOrCtrl+-")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("reset_zoom", "Reset Zoom")
                .accelerator("CmdOrCtrl+0")
                .build(app)?,
        )
        .build()?;

    let go_menu = SubmenuBuilder::new(app, "Go")
        .item(
            &MenuItemBuilder::with_id("go_to_file", "Go to File...")
                .accelerator("CmdOrCtrl+P")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("go_to_symbol", "Go to Symbol...")
                .accelerator("CmdOrCtrl+Shift+O")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("go_to_definition", "Go to Definition")
                .accelerator("F12")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("go_to_line", "Go to Line/Column...")
                .accelerator("CmdOrCtrl+G")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("back", "Back")
                .accelerator("CmdOrCtrl+Alt+Left")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("forward", "Forward")
                .accelerator("CmdOrCtrl+Alt+Right")
                .build(app)?,
        )
        .build()?;

    let run_menu = SubmenuBuilder::new(app, "Run")
        .item(
            &MenuItemBuilder::with_id("start_debugging", "Start Debugging")
                .accelerator("F5")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("run_without_debugging", "Start Without Debugging")
                .accelerator("CmdOrCtrl+F5")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("toggle_breakpoint", "Toggle Breakpoint")
                .accelerator("F9")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("add_configuration", "Add Configuration...").build(app)?)
        .build()?;

    let terminal_menu = SubmenuBuilder::new(app, "Terminal")
        .item(
            &MenuItemBuilder::with_id("new_terminal", "New Terminal")
                .accelerator("CmdOrCtrl+Shift+`")
                .build(app)?,
        )
        .item(&MenuItemBuilder::with_id("split_terminal", "Split Terminal").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("run_active_file", "Run Active File").build(app)?)
        .build()?;

    let help_menu = SubmenuBuilder::new(app, "Help")
        .item(&MenuItemBuilder::with_id("welcome", "Welcome").build(app)?)
        .item(&MenuItemBuilder::with_id("documentation", "Documentation").build(app)?)
        .item(&MenuItemBuilder::with_id("release_notes", "Release Notes").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("about", "About SideX").build(app)?)
        .build()?;

    let sidex_menu = SubmenuBuilder::new(app, "SideX")
        .item(&PredefinedMenuItem::about(app, Some("About SideX"), None)?)
        .separator()
        .item(&PredefinedMenuItem::services(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .item(&PredefinedMenuItem::show_all(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;

    let menu = Menu::with_items(
        app,
        &[
            &sidex_menu,
            &file_menu,
            &edit_menu,
            &selection_menu,
            &view_menu,
            &go_menu,
            &run_menu,
            &terminal_menu,
            &help_menu,
        ],
    )?;

    Ok(menu)
}

/// Map a native menu item ID to a VS Code-compatible command ID used
/// by the [`CommandRegistry`](crate::commands::CommandRegistry).
pub fn menu_id_to_command(menu_id: &str) -> Option<&'static str> {
    Some(match menu_id {
        // File
        "new_file" => "workbench.action.files.newUntitledFile",
        "new_window" => "workbench.action.newWindow",
        "open_file" => "workbench.action.files.openFile",
        "open_folder" => "workbench.action.files.openFolder",
        "save" => "workbench.action.files.save",
        "save_as" => "workbench.action.files.saveAs",
        "save_all" => "workbench.action.files.saveAll",
        "close_editor" => "workbench.action.closeActiveEditor",
        "close_window" => "workbench.action.closeWindow",
        "exit" => "workbench.action.quit",

        // Edit
        "undo" => "editor.action.undo",
        "redo" => "editor.action.redo",
        "cut" => "editor.action.clipboardCutAction",
        "copy" => "editor.action.clipboardCopyAction",
        "paste" => "editor.action.clipboardPasteAction",
        "find" => "actions.find",
        "replace" => "editor.action.startFindReplaceAction",
        "find_in_files" => "workbench.action.findInFiles",
        "replace_in_files" => "workbench.action.replaceInFiles",
        "toggle_line_comment" => "editor.action.commentLine",
        "toggle_block_comment" => "editor.action.blockComment",

        // Selection
        "select_all" => "editor.action.selectAll",
        "expand_selection" => "editor.action.smartSelect.expand",
        "shrink_selection" => "editor.action.smartSelect.shrink",
        "add_cursor_above" => "editor.action.insertCursorAbove",
        "add_cursor_below" => "editor.action.insertCursorBelow",
        "select_all_occurrences" => "editor.action.selectHighlights",

        // View
        "command_palette" => "workbench.action.showCommands",
        "explorer" => "workbench.view.explorer",
        "search" => "workbench.view.search",
        "source_control" => "workbench.view.scm",
        "debug" => "workbench.view.debug",
        "extensions" => "workbench.view.extensions",
        "terminal" => "workbench.action.terminal.toggleTerminal",
        "problems" => "workbench.actions.view.problems",
        "output" => "workbench.action.output.toggleOutput",
        "toggle_sidebar" => "workbench.action.toggleSidebarVisibility",
        "toggle_panel" => "workbench.action.togglePanel",
        "zen_mode" => "workbench.action.toggleZenMode",
        "toggle_fullscreen" => "workbench.action.toggleFullScreen",
        "zoom_in" => "workbench.action.zoomIn",
        "zoom_out" => "workbench.action.zoomOut",
        "reset_zoom" => "workbench.action.zoomReset",

        // Go
        "go_to_file" => "workbench.action.quickOpen",
        "go_to_symbol" => "workbench.action.gotoSymbol",
        "go_to_definition" => "editor.action.revealDefinition",
        "go_to_line" => "workbench.action.gotoLine",
        "back" => "workbench.action.navigateBack",
        "forward" => "workbench.action.navigateForward",

        // Run
        "start_debugging" => "workbench.action.debug.start",
        "run_without_debugging" => "workbench.action.debug.run",
        "toggle_breakpoint" => "editor.debug.action.toggleBreakpoint",
        "add_configuration" => "debug.addConfiguration",

        // Terminal
        "new_terminal" => "workbench.action.terminal.new",
        "split_terminal" => "workbench.action.terminal.split",
        "run_active_file" => "workbench.action.terminal.runActiveFile",

        // Help
        "welcome" => "workbench.action.showWelcomePage",
        "documentation" => "workbench.action.openDocumentationUrl",
        "release_notes" => "update.showCurrentReleaseNotes",
        "about" => "workbench.action.showAboutDialog",

        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_menu_has_all_top_level() {
        let menu = build_native_menu();
        let labels: Vec<&str> = menu
            .items
            .iter()
            .filter_map(|item| match item {
                NativeMenuItem::Submenu { label, .. } => Some(label.as_str()),
                _ => None,
            })
            .collect();

        assert!(labels.contains(&"File"));
        assert!(labels.contains(&"Edit"));
        assert!(labels.contains(&"Selection"));
        assert!(labels.contains(&"View"));
        assert!(labels.contains(&"Go"));
        assert!(labels.contains(&"Run"));
        assert!(labels.contains(&"Terminal"));
        assert!(labels.contains(&"Help"));
    }

    #[test]
    fn menu_id_mapping_coverage() {
        assert_eq!(
            menu_id_to_command("save"),
            Some("workbench.action.files.save")
        );
        assert_eq!(menu_id_to_command("undo"), Some("editor.action.undo"));
        assert_eq!(
            menu_id_to_command("start_debugging"),
            Some("workbench.action.debug.start")
        );
        assert!(menu_id_to_command("nonexistent").is_none());
    }
}
