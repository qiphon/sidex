//! Built-in command registry.
//!
//! Each command is identified by a VS Code-compatible string ID. Commands are
//! registered with a human-readable label and an action callback that receives
//! mutable access to the application.

use std::collections::HashMap;

use crate::app::App;

/// Callback type for command execution.
type CommandAction = fn(&mut App);

/// A registered command with its human-readable label and action.
struct Command {
    label: String,
    action: CommandAction,
}

/// Registry of all built-in editor commands.
pub struct CommandRegistry {
    commands: HashMap<String, Command>,
    /// Recently closed editors for reopen (file paths).
    pub recently_closed: Vec<String>,
}

impl CommandRegistry {
    /// Creates a registry populated with all built-in commands.
    pub fn new() -> Self {
        let mut registry = Self {
            commands: HashMap::new(),
            recently_closed: Vec::new(),
        };
        registry.register_builtins();
        registry
    }

    /// Returns `true` if a command with the given ID exists.
    pub fn has(&self, id: &str) -> bool {
        self.commands.contains_key(id)
    }

    /// Returns the human-readable label for a command.
    pub fn label(&self, id: &str) -> Option<&str> {
        self.commands.get(id).map(|c| c.label.as_str())
    }

    /// Returns all registered command IDs (unsorted).
    pub fn ids(&self) -> Vec<&str> {
        self.commands.keys().map(String::as_str).collect()
    }

    /// Execute a command by ID against the given app.
    pub fn execute(&self, id: &str, app: &mut App) -> bool {
        if let Some(cmd) = self.commands.get(id) {
            (cmd.action)(app);
            log::debug!("executed command: {id}");
            true
        } else {
            log::warn!("unknown command: {id}");
            false
        }
    }

    /// Look up the action function pointer for a command, so it can be
    /// called after releasing the borrow on the registry.
    pub fn get_action(&self, id: &str) -> Option<CommandAction> {
        self.commands.get(id).map(|c| c.action)
    }

    fn register(&mut self, id: &str, label: &str, action: fn(&mut App)) {
        self.commands.insert(
            id.to_owned(),
            Command {
                label: label.to_owned(),
                action,
            },
        );
    }

    fn register_noop(&mut self, id: &str, label: &str) {
        self.register(id, label, |_| {});
    }

    fn register_builtins(&mut self) {
        self.register_file_commands();
        self.register_edit_commands();
        self.register_navigation_commands();
        self.register_view_commands();
        self.register_find_commands();
        self.register_terminal_commands();
        self.register_debug_commands();
        self.register_selection_commands();
        self.register_suggest_commands();
        self.register_code_action_commands();
        self.register_folding_commands();
        self.register_font_zoom_commands();
        self.register_workbench_commands();
        self.register_editor_group_commands();
        self.register_git_commands();
        self.register_task_commands();
        self.register_extension_commands();
        self.register_preferences_commands();
        self.register_window_commands();
        self.register_breadcrumb_commands();
        self.register_diff_commands();
        self.register_markdown_commands();
        self.register_snippet_commands();
        self.register_emmet_commands();
        self.register_accessibility_commands();
        self.register_notification_commands();
        self.register_scm_commands();
    }

    // ── File commands ────────────────────────────────────────────

    fn register_file_commands(&mut self) {
        self.register(
            "workbench.action.files.newUntitledFile",
            "New File",
            |app| {
                app.new_untitled_file();
            },
        );

        self.register("workbench.action.files.openFile", "Open File...", |app| {
            app.open_file_dialog();
        });

        self.register("workbench.action.files.save", "Save", |app| {
            app.save_active_file();
        });

        self.register("workbench.action.files.saveAs", "Save As...", |app| {
            app.save_active_file_as();
        });

        self.register("workbench.action.files.saveAll", "Save All", |app| {
            app.save_all_files();
        });

        self.register(
            "workbench.action.closeActiveEditor",
            "Close Editor",
            |app| {
                app.close_active_editor();
            },
        );

        self.register(
            "workbench.action.closeAllEditors",
            "Close All Editors",
            |app| {
                app.close_all_editors();
            },
        );

        self.register(
            "workbench.action.reopenClosedEditor",
            "Reopen Closed Editor",
            |app| {
                app.reopen_closed_editor();
            },
        );
    }

    // ── Edit commands ────────────────────────────────────────────

    fn register_edit_commands(&mut self) {
        self.register("editor.action.undo", "Undo", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.undo();
                doc.on_edit();
            }
        });

        self.register("editor.action.redo", "Redo", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.redo();
                doc.on_edit();
            }
        });

        self.register("editor.action.clipboardCutAction", "Cut", |app| {
            app.clipboard_cut();
        });

        self.register("editor.action.clipboardCopyAction", "Copy", |app| {
            app.clipboard_copy();
        });

        self.register("editor.action.clipboardPasteAction", "Paste", |app| {
            app.clipboard_paste();
        });

        self.register("editor.action.selectAll", "Select All", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.select_all();
            }
        });

        self.register("editor.action.commentLine", "Toggle Line Comment", |app| {
            let comment_prefix = app.active_comment_prefix();
            if let Some(doc) = app.active_document_mut() {
                doc.document.toggle_line_comment(&comment_prefix);
                doc.on_edit();
            }
        });

        self.register(
            "editor.action.blockComment",
            "Toggle Block Comment",
            |app| {
                let (open, close) = app.active_block_comment();
                if let Some(doc) = app.active_document_mut() {
                    doc.document.toggle_block_comment(&open, &close);
                    doc.on_edit();
                }
            },
        );

        self.register("editor.action.indentLines", "Indent Lines", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.indent();
                doc.on_edit();
            }
        });

        self.register("editor.action.outdentLines", "Outdent Lines", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.outdent();
                doc.on_edit();
            }
        });

        self.register("editor.action.moveLinesUpAction", "Move Lines Up", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.move_line_up();
                doc.on_edit();
            }
        });

        self.register(
            "editor.action.moveLinesDownAction",
            "Move Lines Down",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.move_line_down();
                    doc.on_edit();
                }
            },
        );

        self.register("editor.action.copyLinesUpAction", "Copy Lines Up", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.copy_line_up();
                doc.on_edit();
            }
        });

        self.register(
            "editor.action.copyLinesDownAction",
            "Copy Lines Down",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.copy_line_down();
                    doc.on_edit();
                }
            },
        );

        self.register("editor.action.deleteLines", "Delete Lines", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.delete_line();
                doc.on_edit();
            }
        });

        self.register("editor.action.joinLines", "Join Lines", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.join_lines();
                doc.on_edit();
            }
        });

        self.register(
            "editor.action.sortLinesAscending",
            "Sort Lines Ascending",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.sort_lines_ascending();
                    doc.on_edit();
                }
            },
        );

        self.register(
            "editor.action.sortLinesDescending",
            "Sort Lines Descending",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.sort_lines_descending();
                    doc.on_edit();
                }
            },
        );

        self.register(
            "editor.action.trimTrailingWhitespace",
            "Trim Trailing Whitespace",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.trim_trailing_whitespace();
                    doc.on_edit();
                }
            },
        );

        self.register(
            "editor.action.transformToUppercase",
            "Transform to Uppercase",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.transform_to_uppercase();
                    doc.on_edit();
                }
            },
        );

        self.register(
            "editor.action.transformToLowercase",
            "Transform to Lowercase",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.transform_to_lowercase();
                    doc.on_edit();
                }
            },
        );

        self.register(
            "editor.action.insertLineAfter",
            "Insert Line Below",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.insert_line_below();
                    doc.on_edit();
                }
            },
        );

        self.register(
            "editor.action.insertLineBefore",
            "Insert Line Above",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.insert_line_above();
                    doc.on_edit();
                }
            },
        );

        self.register(
            "editor.action.transposeLetters",
            "Transpose Characters",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.transpose_characters();
                    doc.on_edit();
                }
            },
        );

        self.register_noop("editor.action.transpose", "Transpose");

        self.register_noop(
            "editor.action.addSelectionToNextFindMatch",
            "Add Selection to Next Find Match",
        );

        self.register_noop(
            "editor.action.addCursorsToBottom",
            "Add Cursors to Bottom",
        );
        self.register_noop(
            "editor.action.addCursorsToTop",
            "Add Cursors to Top",
        );
        self.register_noop(
            "editor.action.duplicateSelection",
            "Duplicate Selection",
        );
        self.register_noop(
            "editor.action.removeDuplicateLines",
            "Remove Duplicate Lines",
        );
        self.register_noop(
            "editor.action.transformToTitlecase",
            "Transform to Title Case",
        );
        self.register_noop(
            "editor.action.transformToSnakecase",
            "Transform to Snake Case",
        );
        self.register_noop(
            "editor.action.transformToCamelcase",
            "Transform to Camel Case",
        );
        self.register_noop(
            "editor.action.transformToKebabcase",
            "Transform to Kebab Case",
        );
    }

    // ── Navigation commands ──────────────────────────────────────

    fn register_navigation_commands(&mut self) {
        self.register("workbench.action.quickOpen", "Go to File...", |app| {
            app.show_quick_open = true;
        });

        self.register(
            "workbench.action.showCommands",
            "Command Palette...",
            |app| {
                app.show_command_palette = true;
            },
        );

        self.register("workbench.action.gotoLine", "Go to Line...", |app| {
            app.show_goto_line = true;
        });

        self.register_noop("editor.action.goToDeclaration", "Go to Declaration");
        self.register_noop("editor.action.goToImplementation", "Go to Implementation");
        self.register_noop("editor.action.goToReferences", "Go to References");
        self.register_noop("editor.action.revealDefinition", "Go to Definition");
        self.register_noop("editor.action.goToTypeDefinition", "Go to Type Definition");
        self.register_noop("editor.action.referenceSearch.trigger", "Peek References");
        self.register_noop("editor.action.peekDefinition", "Peek Definition");
        self.register_noop("editor.action.peekImplementation", "Peek Implementation");
        self.register_noop("editor.action.peekTypeDefinition", "Peek Type Definition");
        self.register_noop("editor.action.showHover", "Show Hover");
        self.register_noop("editor.action.triggerParameterHints", "Trigger Parameter Hints");
        self.register_noop("workbench.action.gotoSymbol", "Go to Symbol in Editor...");

        self.register("workbench.action.navigateBack", "Go Back", |app| {
            if app.navigation_stack_back.is_empty() {
                return;
            }
            let entry = app.navigation_stack_back.pop().unwrap();
            app.navigation_stack_forward.push(NavigationEntry {
                doc_index: app.active_document,
                line: app
                    .active_document_ref()
                    .map_or(0, |d| d.document.cursors.primary().position().line),
            });
            app.active_document = entry.doc_index;
        });

        self.register("workbench.action.navigateForward", "Go Forward", |app| {
            if app.navigation_stack_forward.is_empty() {
                return;
            }
            let entry = app.navigation_stack_forward.pop().unwrap();
            app.navigation_stack_back.push(NavigationEntry {
                doc_index: app.active_document,
                line: app
                    .active_document_ref()
                    .map_or(0, |d| d.document.cursors.primary().position().line),
            });
            app.active_document = entry.doc_index;
        });
    }

    // ── View commands ────────────────────────────────────────────

    fn register_view_commands(&mut self) {
        self.register(
            "workbench.action.toggleSidebarVisibility",
            "Toggle Sidebar",
            |app| {
                app.layout.sidebar_visible = !app.layout.sidebar_visible;
                app.needs_relayout = true;
            },
        );

        self.register("workbench.action.togglePanel", "Toggle Panel", |app| {
            app.layout.panel_visible = !app.layout.panel_visible;
            app.needs_relayout = true;
        });

        self.register(
            "workbench.action.terminal.toggleTerminal",
            "Toggle Terminal",
            |app| {
                app.layout.panel_visible = !app.layout.panel_visible;
                app.needs_relayout = true;
            },
        );

        self.register("workbench.action.zoomIn", "Zoom In", |app| {
            app.zoom_in();
        });

        self.register("workbench.action.zoomOut", "Zoom Out", |app| {
            app.zoom_out();
        });

        self.register("workbench.action.zoomReset", "Reset Zoom", |app| {
            app.zoom_reset();
        });

        self.register_noop("workbench.action.toggleFullScreen", "Toggle Full Screen");

        self.register("workbench.action.splitEditor", "Split Editor Right", |app| {
            app.split_editor();
        });
        self.register_noop("workbench.action.splitEditorDown", "Split Editor Down");
    }

    // ── Find commands ────────────────────────────────────────────

    fn register_find_commands(&mut self) {
        self.register("actions.find", "Find", |app| {
            app.show_find_widget = true;
            app.context_keys.set_bool("findWidgetVisible", true);
        });

        self.register(
            "editor.action.startFindReplaceAction",
            "Find and Replace",
            |app| {
                app.show_find_widget = true;
                app.find_replace_mode = true;
                app.context_keys.set_bool("findWidgetVisible", true);
            },
        );

        self.register(
            "editor.action.nextMatchFindAction",
            "Find Next",
            |app| {
                app.find_next();
            },
        );

        self.register(
            "editor.action.previousMatchFindAction",
            "Find Previous",
            |app| {
                app.find_previous();
            },
        );

        self.register("workbench.action.findInFiles", "Search in Files", |app| {
            app.show_search_panel = true;
        });
    }

    // ── Terminal commands ────────────────────────────────────────

    fn register_terminal_commands(&mut self) {
        self.register("workbench.action.terminal.new", "New Terminal", |app| {
            if let Err(e) = app.terminal_manager.create(None, None) {
                log::error!("failed to create terminal: {e}");
            }
            app.layout.panel_visible = true;
            app.needs_relayout = true;
        });

        self.register("workbench.action.terminal.split", "Split Terminal", |app| {
            if let Err(e) = app.terminal_manager.create(None, None) {
                log::error!("failed to create terminal: {e}");
            }
        });

        self.register("workbench.action.terminal.kill", "Kill Terminal", |app| {
            let ids = app.terminal_manager.list();
            if let Some(last_id) = ids.last() {
                if let Err(e) = app.terminal_manager.remove(*last_id) {
                    log::error!("failed to kill terminal: {e}");
                }
            }
        });

        self.register_noop("workbench.action.terminal.clear", "Clear Terminal");
        self.register_noop("workbench.action.terminal.focusNext", "Focus Next Terminal");
        self.register_noop("workbench.action.terminal.focusPrevious", "Focus Previous Terminal");
        self.register_noop("workbench.action.terminal.rename", "Rename Terminal");
        self.register_noop("workbench.action.terminal.sendSequence", "Send Sequence to Terminal");
        self.register_noop("workbench.action.terminal.scrollUp", "Terminal: Scroll Up");
        self.register_noop("workbench.action.terminal.scrollDown", "Terminal: Scroll Down");
        self.register_noop("workbench.action.terminal.scrollToTop", "Terminal: Scroll to Top");
        self.register_noop("workbench.action.terminal.scrollToBottom", "Terminal: Scroll to Bottom");
        self.register_noop("workbench.action.terminal.selectAll", "Terminal: Select All");
        self.register_noop("workbench.action.terminal.copySelection", "Terminal: Copy Selection");
        self.register_noop("workbench.action.terminal.paste", "Terminal: Paste");
        self.register_noop("workbench.action.terminal.runSelectedText", "Terminal: Run Selected Text");
        self.register_noop("workbench.action.terminal.runActiveFile", "Terminal: Run Active File");
        self.register_noop(
            "workbench.action.terminal.changeIcon",
            "Terminal: Change Icon",
        );
        self.register_noop(
            "workbench.action.terminal.changeColor",
            "Terminal: Change Color",
        );
    }

    // ── Debug commands ───────────────────────────────────────────

    fn register_debug_commands(&mut self) {
        self.register_noop("workbench.action.debug.start", "Start Debugging");
        self.register_noop("workbench.action.debug.run", "Run Without Debugging");
        self.register_noop("workbench.action.debug.stop", "Stop Debugging");
        self.register_noop("workbench.action.debug.restart", "Restart Debugging");
        self.register_noop("editor.debug.action.toggleBreakpoint", "Toggle Breakpoint");
        self.register_noop("editor.debug.action.conditionalBreakpoint", "Add Conditional Breakpoint...");
        self.register_noop("editor.debug.action.toggleInlineBreakpoint", "Toggle Inline Breakpoint");
        self.register_noop("editor.debug.action.addLogPoint", "Add Log Point...");
        self.register_noop("workbench.action.debug.stepOver", "Step Over");
        self.register_noop("workbench.action.debug.stepInto", "Step Into");
        self.register_noop("workbench.action.debug.stepOut", "Step Out");
        self.register_noop("workbench.action.debug.continue", "Continue");
        self.register_noop("workbench.action.debug.pause", "Pause");
        self.register_noop("workbench.action.debug.selectandstart", "Select and Start Debugging");
        self.register_noop("workbench.action.debug.configure", "Open launch.json");
        self.register_noop("editor.debug.action.runToCursor", "Run to Cursor");
        self.register_noop("workbench.debug.viewlet.action.addFunctionBreakpoint", "Add Function Breakpoint...");
        self.register_noop("workbench.debug.viewlet.action.removeAllBreakpoints", "Remove All Breakpoints");
        self.register_noop("workbench.debug.viewlet.action.enableAllBreakpoints", "Enable All Breakpoints");
        self.register_noop("workbench.debug.viewlet.action.disableAllBreakpoints", "Disable All Breakpoints");
    }

    // ── Selection commands ───────────────────────────────────────

    fn register_selection_commands(&mut self) {
        self.register(
            "editor.action.smartSelect.expand",
            "Expand Selection",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.smart_select_grow();
                }
            },
        );

        self.register(
            "editor.action.smartSelect.shrink",
            "Shrink Selection",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.smart_select_shrink();
                }
            },
        );

        self.register(
            "editor.action.selectHighlights",
            "Add Cursors to Line Selections",
            |app| {
                if let Some(doc) = app.active_document_mut() {
                    doc.document.add_cursor_at_each_selection_line();
                }
            },
        );

        self.register("editor.action.wordWrap", "Toggle Word Wrap", |app| {
            if let Some(doc) = app.active_document_mut() {
                doc.document.toggle_word_wrap();
            }
        });
    }

    // ── Suggest / autocomplete commands ──────────────────────────

    fn register_suggest_commands(&mut self) {
        self.register(
            "editor.action.triggerSuggest",
            "Trigger Suggest",
            |app| {
                app.trigger_suggest();
            },
        );

        self.register(
            "acceptSelectedSuggestion",
            "Accept Suggestion",
            |app| {
                app.accept_suggest();
            },
        );

        self.register(
            "hideSuggestWidget",
            "Hide Suggest Widget",
            |app| {
                app.context_keys.set_bool("suggestWidgetVisible", false);
                app.needs_render = true;
            },
        );
    }

    // ── Code action / refactor / format commands ────────────────

    fn register_code_action_commands(&mut self) {
        self.register("editor.action.quickFix", "Quick Fix...", |app| {
            app.show_code_actions();
        });

        self.register("editor.action.rename", "Rename Symbol", |app| {
            app.start_rename();
        });

        self.register(
            "editor.action.formatDocument",
            "Format Document",
            |app| {
                app.format_document();
            },
        );

        self.register_noop("editor.action.formatSelection", "Format Selection");
        self.register_noop("editor.action.organizeImports", "Organize Imports");
        self.register_noop("editor.action.sourceAction", "Source Action...");
        self.register_noop("editor.action.refactor", "Refactor...");
    }

    // ── Folding commands ──────────────────────────────────────────

    fn register_folding_commands(&mut self) {
        self.register_noop("editor.fold", "Fold");
        self.register_noop("editor.unfold", "Unfold");
        self.register_noop("editor.foldAll", "Fold All");
        self.register_noop("editor.unfoldAll", "Unfold All");
        self.register_noop("editor.foldAllBlockComments", "Fold All Block Comments");
        self.register_noop("editor.foldAllMarkerRegions", "Fold All Marker Regions");
        self.register_noop("editor.unfoldAllMarkerRegions", "Unfold All Marker Regions");
        self.register_noop("editor.foldRecursively", "Fold Recursively");
        self.register_noop("editor.unfoldRecursively", "Unfold Recursively");
        self.register_noop("editor.foldLevel1", "Fold Level 1");
        self.register_noop("editor.foldLevel2", "Fold Level 2");
        self.register_noop("editor.foldLevel3", "Fold Level 3");
        self.register_noop("editor.foldLevel4", "Fold Level 4");
        self.register_noop("editor.foldLevel5", "Fold Level 5");
        self.register_noop("editor.foldLevel6", "Fold Level 6");
        self.register_noop("editor.foldLevel7", "Fold Level 7");
        self.register_noop("editor.toggleFold", "Toggle Fold");
    }

    // ── Font zoom commands ────────────────────────────────────────

    fn register_font_zoom_commands(&mut self) {
        self.register("editor.action.fontZoomIn", "Font Zoom In", |app| {
            app.zoom_in();
        });
        self.register("editor.action.fontZoomOut", "Font Zoom Out", |app| {
            app.zoom_out();
        });
        self.register("editor.action.fontZoomReset", "Font Zoom Reset", |app| {
            app.zoom_reset();
        });
    }

    // ── Workbench commands ────────────────────────────────────────

    fn register_workbench_commands(&mut self) {
        self.register(
            "workbench.action.openSettings",
            "Open Settings",
            |app| {
                app.show_command_palette = true;
            },
        );
        self.register(
            "workbench.action.openKeybindings",
            "Open Keyboard Shortcuts",
            |app| {
                app.show_command_palette = true;
            },
        );
        self.register(
            "workbench.action.openKeybindingsJSON",
            "Open Keyboard Shortcuts (JSON)",
            |_| {},
        );
        self.register_noop(
            "workbench.action.openSettingsJson",
            "Open User Settings (JSON)",
        );
        self.register_noop(
            "workbench.action.openWorkspaceSettings",
            "Open Workspace Settings",
        );
        self.register_noop(
            "workbench.action.openWorkspaceSettingsJSON",
            "Open Workspace Settings (JSON)",
        );
        self.register_noop(
            "workbench.action.openFolderSettings",
            "Open Folder Settings",
        );
        self.register_noop(
            "workbench.action.selectTheme",
            "Color Theme",
        );
        self.register_noop(
            "workbench.action.selectIconTheme",
            "File Icon Theme",
        );
        self.register_noop(
            "workbench.action.toggleActivityBarVisibility",
            "Toggle Activity Bar Visibility",
        );
        self.register_noop(
            "workbench.action.toggleStatusbarVisibility",
            "Toggle Status Bar Visibility",
        );
        self.register_noop("workbench.action.toggleZenMode", "Toggle Zen Mode");
        self.register_noop(
            "workbench.action.toggleCenteredLayout",
            "Toggle Centered Layout",
        );
        self.register_noop(
            "workbench.action.toggleMenuBar",
            "Toggle Menu Bar",
        );
        self.register_noop(
            "workbench.action.toggleBreadcrumbs",
            "Toggle Breadcrumbs",
        );
        self.register_noop(
            "workbench.action.toggleTabsVisibility",
            "Toggle Tabs Visibility",
        );
        self.register(
            "workbench.action.showAllSymbols",
            "Go to Symbol in Workspace...",
            |app| {
                app.show_command_palette = true;
            },
        );
        self.register_noop(
            "workbench.action.showAllEditors",
            "Show All Editors",
        );
        self.register_noop(
            "workbench.action.showAllEditorsByMostRecentlyUsed",
            "Show All Editors By Most Recently Used",
        );
        self.register_noop(
            "workbench.action.quickOpenPreviousEditor",
            "Open Previous Editor from History",
        );
        self.register_noop(
            "workbench.action.quickOpenNavigateNext",
            "Navigate to Next Quick Open Item",
        );
        self.register_noop(
            "workbench.action.quickOpenNavigatePrevious",
            "Navigate to Previous Quick Open Item",
        );
        self.register(
            "workbench.action.closeOtherEditors",
            "Close Other Editors",
            |app| {
                let idx = app.active_document;
                app.close_other_tabs(0, idx);
            },
        );
        self.register_noop(
            "workbench.action.closeEditorsInGroup",
            "Close All Editors in Group",
        );
        self.register_noop(
            "workbench.action.closeEditorsToTheLeft",
            "Close Editors to the Left",
        );
        self.register_noop(
            "workbench.action.closeEditorsToTheRight",
            "Close Editors to the Right",
        );
        self.register_noop(
            "workbench.action.pinEditor",
            "Pin Editor",
        );
        self.register_noop(
            "workbench.action.unpinEditor",
            "Unpin Editor",
        );
        self.register(
            "workbench.action.files.openFolder",
            "Open Folder...",
            |_| {},
        );
        self.register(
            "workbench.action.previousEditor",
            "Previous Editor",
            |app| {
                app.prev_tab();
            },
        );
        self.register(
            "workbench.action.nextEditor",
            "Next Editor",
            |app| {
                app.next_tab();
            },
        );
        self.register(
            "workbench.action.splitEditorDown",
            "Split Editor Down",
            |app| {
                app.split_editor_down();
            },
        );
        self.register(
            "workbench.action.reloadWindow",
            "Reload Window",
            |_| {},
        );
        self.register(
            "workbench.action.newWindow",
            "New Window",
            |_| {},
        );
        self.register(
            "workbench.action.closeWindow",
            "Close Window",
            |_| {},
        );
    }

    // ── Editor group commands ─────────────────────────────────────

    fn register_editor_group_commands(&mut self) {
        self.register(
            "workbench.action.focusFirstEditorGroup",
            "Focus First Editor Group",
            |app| {
                app.focus_group(0);
            },
        );
        self.register(
            "workbench.action.focusSecondEditorGroup",
            "Focus Second Editor Group",
            |app| {
                app.focus_group(1);
            },
        );
        self.register(
            "workbench.action.focusThirdEditorGroup",
            "Focus Third Editor Group",
            |app| {
                app.focus_group(2);
            },
        );
        self.register(
            "workbench.action.focusFourthEditorGroup",
            "Focus Fourth Editor Group",
            |app| {
                app.focus_group(3);
            },
        );
        self.register(
            "workbench.action.focusFifthEditorGroup",
            "Focus Fifth Editor Group",
            |app| {
                app.focus_group(4);
            },
        );
        self.register(
            "workbench.action.focusNextGroup",
            "Focus Next Editor Group",
            |app| {
                app.next_group();
            },
        );
        self.register(
            "workbench.action.focusPreviousGroup",
            "Focus Previous Editor Group",
            |app| {
                app.prev_group();
            },
        );
        self.register_noop(
            "workbench.action.moveEditorToNextGroup",
            "Move Editor to Next Group",
        );
        self.register_noop(
            "workbench.action.moveEditorToPreviousGroup",
            "Move Editor to Previous Group",
        );
        self.register_noop(
            "workbench.action.moveEditorToFirstGroup",
            "Move Editor to First Group",
        );
        self.register_noop(
            "workbench.action.moveEditorToLastGroup",
            "Move Editor to Last Group",
        );
        self.register_noop(
            "workbench.action.splitEditorLeft",
            "Split Editor Left",
        );
        self.register_noop(
            "workbench.action.splitEditorUp",
            "Split Editor Up",
        );
        self.register(
            "workbench.action.splitEditorOrthogonal",
            "Split Editor Orthogonal",
            |app| {
                app.split_editor_down();
            },
        );
        self.register_noop(
            "workbench.action.toggleEditorGroupLayout",
            "Toggle Editor Group Layout",
        );
        self.register_noop(
            "workbench.action.maximizeEditor",
            "Toggle Maximise Editor Group",
        );
        self.register_noop(
            "workbench.action.evenEditorWidths",
            "Reset Editor Group Sizes",
        );
        self.register_noop(
            "workbench.action.closeGroup",
            "Close Editor Group",
        );
    }

    // ── Git commands ──────────────────────────────────────────────

    fn register_git_commands(&mut self) {
        self.register_noop("git.commit", "Git: Commit");
        self.register_noop("git.commitStaged", "Git: Commit Staged");
        self.register_noop("git.commitAll", "Git: Commit All");
        self.register_noop("git.push", "Git: Push");
        self.register_noop("git.pushForce", "Git: Push (Force)");
        self.register_noop("git.pull", "Git: Pull");
        self.register_noop("git.pullRebase", "Git: Pull (Rebase)");
        self.register_noop("git.sync", "Git: Sync");
        self.register_noop("git.checkout", "Git: Checkout to...");
        self.register_noop("git.branch", "Git: Create Branch...");
        self.register_noop("git.deleteBranch", "Git: Delete Branch...");
        self.register_noop("git.merge", "Git: Merge Branch...");
        self.register_noop("git.rebase", "Git: Rebase Branch...");
        self.register_noop("git.stash", "Git: Stash");
        self.register_noop("git.stashPop", "Git: Pop Stash...");
        self.register_noop("git.stashDrop", "Git: Drop Stash...");
        self.register_noop("git.stage", "Git: Stage Changes");
        self.register_noop("git.stageAll", "Git: Stage All Changes");
        self.register_noop("git.unstage", "Git: Unstage Changes");
        self.register_noop("git.unstageAll", "Git: Unstage All Changes");
        self.register_noop("git.clean", "Git: Discard Changes");
        self.register_noop("git.cleanAll", "Git: Discard All Changes");
        self.register_noop("git.openChange", "Git: Open Changes");
        self.register_noop("git.openFile", "Git: Open File");
        self.register_noop("git.init", "Git: Initialize Repository");
        self.register_noop("git.clone", "Git: Clone...");
        self.register_noop("git.fetch", "Git: Fetch");
        self.register_noop("git.fetchPrune", "Git: Fetch (Prune)");
        self.register_noop("git.addRemote", "Git: Add Remote...");
        self.register_noop("git.removeRemote", "Git: Remove Remote...");
        self.register_noop("git.publish", "Git: Publish Branch...");
        self.register_noop("git.showOutput", "Git: Show Git Output");
        self.register_noop("git.timeline.openDiff", "Git: Open Timeline Diff");
    }

    // ── Task commands ─────────────────────────────────────────────

    fn register_task_commands(&mut self) {
        self.register_noop("workbench.action.tasks.runTask", "Run Task...");
        self.register_noop("workbench.action.tasks.build", "Run Build Task");
        self.register_noop("workbench.action.tasks.test", "Run Test Task");
        self.register_noop("workbench.action.tasks.terminate", "Terminate Task");
        self.register_noop("workbench.action.tasks.restartTask", "Restart Running Task");
        self.register_noop("workbench.action.tasks.showLog", "Show Task Log");
        self.register_noop(
            "workbench.action.tasks.configureTaskRunner",
            "Configure Task Runner",
        );
        self.register_noop(
            "workbench.action.tasks.configureDefaultBuildTask",
            "Configure Default Build Task",
        );
        self.register_noop(
            "workbench.action.tasks.configureDefaultTestTask",
            "Configure Default Test Task",
        );
        self.register_noop("workbench.action.tasks.reRunTask", "Rerun Last Task");
    }

    // ── Extension commands ────────────────────────────────────────

    fn register_extension_commands(&mut self) {
        self.register_noop(
            "workbench.extensions.installExtension",
            "Install Extension...",
        );
        self.register_noop(
            "workbench.extensions.uninstallExtension",
            "Uninstall Extension",
        );
        self.register_noop(
            "workbench.extensions.action.enableAll",
            "Enable All Extensions",
        );
        self.register_noop(
            "workbench.extensions.action.disableAll",
            "Disable All Extensions",
        );
        self.register_noop(
            "workbench.extensions.action.showInstalledExtensions",
            "Show Installed Extensions",
        );
        self.register_noop(
            "workbench.extensions.action.showEnabledExtensions",
            "Show Enabled Extensions",
        );
        self.register_noop(
            "workbench.extensions.action.showDisabledExtensions",
            "Show Disabled Extensions",
        );
        self.register_noop(
            "workbench.extensions.action.showRecommendedExtensions",
            "Show Recommended Extensions",
        );
        self.register_noop(
            "workbench.extensions.action.showPopularExtensions",
            "Show Popular Extensions",
        );
        self.register_noop(
            "workbench.extensions.action.checkForUpdates",
            "Check for Extension Updates",
        );
        self.register_noop(
            "workbench.extensions.action.updateAllExtensions",
            "Update All Extensions",
        );
    }

    // ── Preferences commands ──────────────────────────────────────

    fn register_preferences_commands(&mut self) {
        self.register_noop(
            "workbench.action.openSnippets",
            "Configure User Snippets",
        );
        self.register_noop(
            "workbench.action.configureLanguageBasedSettings",
            "Configure Language Specific Settings",
        );
        self.register_noop(
            "workbench.action.openGlobalKeybindings",
            "Open Keyboard Shortcuts",
        );
        self.register_noop(
            "workbench.profiles.import",
            "Import Settings Profile...",
        );
        self.register_noop(
            "workbench.profiles.export",
            "Export Settings Profile...",
        );
    }

    // ── Window commands ───────────────────────────────────────────

    fn register_window_commands(&mut self) {
        self.register_noop(
            "workbench.action.toggleEditorWidths",
            "Toggle Editor Widths",
        );
        self.register_noop(
            "workbench.action.toggleWordWrap",
            "Toggle Word Wrap",
        );
        self.register("editor.action.toggleMinimap", "Toggle Minimap", |_| {});
        self.register_noop(
            "workbench.action.toggleRenderWhitespace",
            "Toggle Render Whitespace",
        );
        self.register_noop(
            "workbench.action.toggleRenderControlCharacters",
            "Toggle Render Control Characters",
        );
        self.register_noop(
            "workbench.action.toggleScreencastMode",
            "Toggle Screencast Mode",
        );
    }

    // ── Breadcrumb commands ───────────────────────────────────────

    fn register_breadcrumb_commands(&mut self) {
        self.register_noop("breadcrumbs.focus", "Focus Breadcrumbs");
        self.register_noop("breadcrumbs.focusNext", "Focus Next Breadcrumb");
        self.register_noop("breadcrumbs.focusPrevious", "Focus Previous Breadcrumb");
        self.register_noop("breadcrumbs.selectFocused", "Select Focused Breadcrumb");
        self.register_noop("breadcrumbs.toggleToOn", "Enable Breadcrumbs");
        self.register_noop("breadcrumbs.toggleToOff", "Disable Breadcrumbs");
    }

    // ── Diff commands ─────────────────────────────────────────────

    fn register_diff_commands(&mut self) {
        self.register_noop(
            "workbench.action.compareEditor.nextChange",
            "Next Change",
        );
        self.register_noop(
            "workbench.action.compareEditor.previousChange",
            "Previous Change",
        );
        self.register_noop(
            "workbench.files.action.compareWithClipboard",
            "Compare Active File with Clipboard",
        );
        self.register_noop(
            "workbench.files.action.compareWithSaved",
            "Compare Active File with Saved",
        );
        self.register_noop(
            "workbench.files.action.compareNewUntitledTextFiles",
            "Compare New Untitled Text Files",
        );
    }

    // ── Markdown commands ─────────────────────────────────────────

    fn register_markdown_commands(&mut self) {
        self.register_noop(
            "markdown.showPreview",
            "Markdown: Open Preview",
        );
        self.register_noop(
            "markdown.showPreviewToSide",
            "Markdown: Open Preview to the Side",
        );
        self.register_noop(
            "markdown.extension.toggleBold",
            "Markdown: Toggle Bold",
        );
        self.register_noop(
            "markdown.extension.toggleItalic",
            "Markdown: Toggle Italic",
        );
        self.register_noop(
            "markdown.extension.toggleStrikethrough",
            "Markdown: Toggle Strikethrough",
        );
    }

    // ── Snippet commands ──────────────────────────────────────────

    fn register_snippet_commands(&mut self) {
        self.register_noop(
            "editor.action.insertSnippet",
            "Insert Snippet...",
        );
        self.register_noop(
            "editor.action.showSnippets",
            "Show Snippets",
        );
        self.register_noop(
            "editor.action.nextSnippetPlaceholder",
            "Next Snippet Placeholder",
        );
        self.register_noop(
            "editor.action.prevSnippetPlaceholder",
            "Previous Snippet Placeholder",
        );
    }

    // ── Emmet commands ────────────────────────────────────────────

    fn register_emmet_commands(&mut self) {
        self.register_noop("editor.emmet.action.expandAbbreviation", "Emmet: Expand Abbreviation");
        self.register_noop("editor.emmet.action.wrapWithAbbreviation", "Emmet: Wrap with Abbreviation");
        self.register_noop("editor.emmet.action.removeTag", "Emmet: Remove Tag");
        self.register_noop("editor.emmet.action.balanceIn", "Emmet: Balance Inward");
        self.register_noop("editor.emmet.action.balanceOut", "Emmet: Balance Outward");
        self.register_noop("editor.emmet.action.nextEditPoint", "Emmet: Next Edit Point");
        self.register_noop("editor.emmet.action.prevEditPoint", "Emmet: Previous Edit Point");
    }

    // ── Accessibility commands ────────────────────────────────────

    fn register_accessibility_commands(&mut self) {
        self.register_noop(
            "editor.action.accessibilityHelp",
            "Open Accessibility Help",
        );
        self.register_noop(
            "workbench.action.toggleScreenReaderOptimized",
            "Toggle Screen Reader Optimized Mode",
        );
        self.register_noop(
            "editor.action.inspectTokens",
            "Developer: Inspect Editor Tokens and Scopes",
        );
        self.register_noop(
            "workbench.action.toggleDevTools",
            "Developer: Toggle Developer Tools",
        );
        self.register_noop(
            "workbench.action.openProcessExplorer",
            "Developer: Open Process Explorer",
        );
    }

    // ── Notification commands ─────────────────────────────────────

    fn register_notification_commands(&mut self) {
        self.register_noop(
            "notifications.clearAll",
            "Clear All Notifications",
        );
        self.register_noop(
            "notifications.toggleList",
            "Toggle Notifications",
        );
        self.register_noop(
            "notifications.focusToasts",
            "Focus Notification Toasts",
        );
    }

    // ── SCM (Source Control) commands ─────────────────────────────

    fn register_scm_commands(&mut self) {
        self.register_noop("workbench.view.scm", "Show Source Control");
        self.register_noop(
            "workbench.scm.action.acceptInput",
            "SCM: Accept Input",
        );
        self.register_noop("workbench.view.explorer", "Show Explorer");
        self.register_noop("workbench.view.search", "Show Search");
        self.register_noop("workbench.view.debug", "Show Run and Debug");
        self.register_noop("workbench.view.extensions", "Show Extensions");
        self.register_noop("workbench.action.output.toggleOutput", "Toggle Output");
        self.register_noop("workbench.action.problems.focus", "Focus Problems Panel");
        self.register_noop("workbench.debug.action.toggleRepl", "Toggle Debug Console");
        self.register_noop("workbench.action.focusSideBar", "Focus Side Bar");
        self.register_noop("workbench.action.focusPanel", "Focus Panel");
        self.register_noop("workbench.action.focusActiveEditorGroup", "Focus Active Editor Group");
        self.register_noop("workbench.action.terminal.focus", "Focus Terminal");
        self.register_noop("workbench.action.focusStatusBar", "Focus Status Bar");
        self.register_noop("workbench.action.openView", "Open View...");
        self.register_noop("workbench.action.quickOpenView", "Quick Open View");
        self.register_noop("workbench.action.toggleMaximizedPanel", "Toggle Maximised Panel");
        self.register_noop("workbench.action.toggleSidebarPosition", "Toggle Sidebar Position");
        self.register_noop("workbench.action.togglePanelPosition", "Toggle Panel Position");
        self.register_noop("workbench.action.moveSideBarLeft", "Move Side Bar Left");
        self.register_noop("workbench.action.moveSideBarRight", "Move Side Bar Right");
        self.register_noop("workbench.action.closePanel", "Close Panel");
        self.register_noop("workbench.action.closeSidebar", "Close Sidebar");
        self.register_noop("workbench.action.openRecent", "Open Recent...");
        self.register_noop(
            "workbench.action.clearRecentFiles",
            "Clear Recently Opened",
        );
        self.register_noop(
            "workbench.action.showAboutDialog",
            "About",
        );
        self.register_noop(
            "workbench.action.openDocumentationUrl",
            "Documentation",
        );
        self.register_noop(
            "workbench.action.openTipsAndTricksUrl",
            "Tips and Tricks",
        );
        self.register_noop(
            "workbench.action.reportIssue",
            "Report Issue",
        );
        self.register_noop(
            "workbench.action.openIssueReporter",
            "Open Issue Reporter",
        );
        self.register_noop(
            "workbench.action.checkForUpdates",
            "Check for Updates",
        );
        self.register_noop(
            "editor.action.goToLine",
            "Go to Line/Column...",
        );
        self.register_noop(
            "editor.action.marker.nextInFiles",
            "Go to Next Problem",
        );
        self.register_noop(
            "editor.action.marker.prevInFiles",
            "Go to Previous Problem",
        );
        self.register_noop(
            "editor.action.marker.next",
            "Go to Next Problem in File",
        );
        self.register_noop(
            "editor.action.marker.prev",
            "Go to Previous Problem in File",
        );
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Entry in the navigation history stack for back/forward.
#[derive(Debug, Clone)]
pub struct NavigationEntry {
    pub doc_index: usize,
    pub line: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.files.save"));
        assert!(reg.has("editor.action.undo"));
        assert!(reg.has("workbench.action.terminal.new"));
    }

    #[test]
    fn label_lookup() {
        let reg = CommandRegistry::new();
        assert_eq!(reg.label("workbench.action.files.save"), Some("Save"));
    }

    #[test]
    fn missing_command() {
        let reg = CommandRegistry::new();
        assert!(!reg.has("nonexistent.command"));
        assert!(reg.label("nonexistent.command").is_none());
    }

    #[test]
    fn all_ids_nonempty() {
        let reg = CommandRegistry::new();
        assert!(reg.ids().len() >= 200);
    }

    #[test]
    fn file_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.files.newUntitledFile"));
        assert!(reg.has("workbench.action.files.openFile"));
        assert!(reg.has("workbench.action.files.saveAs"));
        assert!(reg.has("workbench.action.files.saveAll"));
        assert!(reg.has("workbench.action.closeActiveEditor"));
        assert!(reg.has("workbench.action.closeAllEditors"));
        assert!(reg.has("workbench.action.reopenClosedEditor"));
    }

    #[test]
    fn edit_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("editor.action.clipboardCutAction"));
        assert!(reg.has("editor.action.clipboardCopyAction"));
        assert!(reg.has("editor.action.clipboardPasteAction"));
        assert!(reg.has("editor.action.commentLine"));
        assert!(reg.has("editor.action.blockComment"));
        assert!(reg.has("editor.action.indentLines"));
        assert!(reg.has("editor.action.outdentLines"));
        assert!(reg.has("editor.action.moveLinesUpAction"));
        assert!(reg.has("editor.action.moveLinesDownAction"));
        assert!(reg.has("editor.action.copyLinesUpAction"));
        assert!(reg.has("editor.action.copyLinesDownAction"));
        assert!(reg.has("editor.action.deleteLines"));
        assert!(reg.has("editor.action.joinLines"));
        assert!(reg.has("editor.action.sortLinesAscending"));
        assert!(reg.has("editor.action.sortLinesDescending"));
        assert!(reg.has("editor.action.trimTrailingWhitespace"));
        assert!(reg.has("editor.action.transformToUppercase"));
        assert!(reg.has("editor.action.transformToLowercase"));
    }

    #[test]
    fn navigation_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.quickOpen"));
        assert!(reg.has("workbench.action.showCommands"));
        assert!(reg.has("workbench.action.gotoLine"));
        assert!(reg.has("editor.action.goToDeclaration"));
        assert!(reg.has("editor.action.goToImplementation"));
        assert!(reg.has("editor.action.goToReferences"));
        assert!(reg.has("workbench.action.navigateBack"));
        assert!(reg.has("workbench.action.navigateForward"));
    }

    #[test]
    fn view_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.toggleSidebarVisibility"));
        assert!(reg.has("workbench.action.togglePanel"));
        assert!(reg.has("workbench.action.terminal.toggleTerminal"));
        assert!(reg.has("workbench.action.zoomIn"));
        assert!(reg.has("workbench.action.zoomOut"));
        assert!(reg.has("workbench.action.zoomReset"));
    }

    #[test]
    fn find_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("actions.find"));
        assert!(reg.has("editor.action.startFindReplaceAction"));
        assert!(reg.has("workbench.action.findInFiles"));
    }

    #[test]
    fn terminal_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.terminal.new"));
        assert!(reg.has("workbench.action.terminal.split"));
        assert!(reg.has("workbench.action.terminal.kill"));
        assert!(reg.has("workbench.action.terminal.clear"));
        assert!(reg.has("workbench.action.terminal.focusNext"));
        assert!(reg.has("workbench.action.terminal.focusPrevious"));
        assert!(reg.has("workbench.action.terminal.rename"));
        assert!(reg.has("workbench.action.terminal.sendSequence"));
    }

    #[test]
    fn debug_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.debug.start"));
        assert!(reg.has("workbench.action.debug.run"));
        assert!(reg.has("workbench.action.debug.stop"));
        assert!(reg.has("workbench.action.debug.restart"));
        assert!(reg.has("workbench.action.debug.continue"));
        assert!(reg.has("workbench.action.debug.pause"));
        assert!(reg.has("workbench.action.debug.stepOver"));
        assert!(reg.has("workbench.action.debug.stepInto"));
        assert!(reg.has("workbench.action.debug.stepOut"));
        assert!(reg.has("editor.debug.action.toggleBreakpoint"));
        assert!(reg.has("editor.debug.action.conditionalBreakpoint"));
    }

    #[test]
    fn git_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("git.commit"));
        assert!(reg.has("git.push"));
        assert!(reg.has("git.pull"));
        assert!(reg.has("git.sync"));
        assert!(reg.has("git.checkout"));
        assert!(reg.has("git.branch"));
        assert!(reg.has("git.stash"));
        assert!(reg.has("git.stashPop"));
        assert!(reg.has("git.stage"));
        assert!(reg.has("git.unstage"));
        assert!(reg.has("git.clean"));
        assert!(reg.has("git.openChange"));
        assert!(reg.has("git.openFile"));
    }

    #[test]
    fn folding_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("editor.fold"));
        assert!(reg.has("editor.unfold"));
        assert!(reg.has("editor.foldAll"));
        assert!(reg.has("editor.unfoldAll"));
        assert!(reg.has("editor.foldAllBlockComments"));
        assert!(reg.has("editor.toggleFold"));
    }

    #[test]
    fn workbench_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.openSettings"));
        assert!(reg.has("workbench.action.openKeybindings"));
        assert!(reg.has("workbench.action.showAllSymbols"));
        assert!(reg.has("workbench.action.toggleZenMode"));
        assert!(reg.has("workbench.action.closeOtherEditors"));
        assert!(reg.has("workbench.action.previousEditor"));
        assert!(reg.has("workbench.action.nextEditor"));
        assert!(reg.has("workbench.action.files.openFolder"));
        assert!(reg.has("workbench.action.newWindow"));
        assert!(reg.has("workbench.action.closeWindow"));
        assert!(reg.has("workbench.action.reloadWindow"));
    }

    #[test]
    fn editor_group_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.focusFirstEditorGroup"));
        assert!(reg.has("workbench.action.focusSecondEditorGroup"));
        assert!(reg.has("workbench.action.focusThirdEditorGroup"));
        assert!(reg.has("workbench.action.focusNextGroup"));
        assert!(reg.has("workbench.action.focusPreviousGroup"));
    }

    #[test]
    fn font_zoom_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("editor.action.fontZoomIn"));
        assert!(reg.has("editor.action.fontZoomOut"));
        assert!(reg.has("editor.action.fontZoomReset"));
    }

    #[test]
    fn task_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.action.tasks.runTask"));
        assert!(reg.has("workbench.action.tasks.build"));
        assert!(reg.has("workbench.action.tasks.test"));
        assert!(reg.has("workbench.action.tasks.terminate"));
    }

    #[test]
    fn extension_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.extensions.installExtension"));
        assert!(reg.has("workbench.extensions.uninstallExtension"));
        assert!(reg.has("workbench.extensions.action.enableAll"));
        assert!(reg.has("workbench.extensions.action.disableAll"));
    }

    #[test]
    fn scm_and_panel_commands_registered() {
        let reg = CommandRegistry::new();
        assert!(reg.has("workbench.view.scm"));
        assert!(reg.has("workbench.view.explorer"));
        assert!(reg.has("workbench.view.search"));
        assert!(reg.has("workbench.view.debug"));
        assert!(reg.has("workbench.view.extensions"));
    }
}
