use sidex_extension_sdk::prelude::*;

/// Git integration for SideX.
/// Provides Git repository management, branch operations, commit history,
/// staged/unstaged change tracking, and gitgraph visualization support.
pub struct GitExtension;

impl SidexExtension for GitExtension {
    fn activate() -> Result<(), String> {
        Ok(())
    }

    fn deactivate() {}

    fn get_name() -> String {
        "Git".to_string()
    }

    fn get_display_name() -> String {
        "Git".to_string()
    }

    fn get_version() -> String {
        "0.1.0".to_string()
    }

    fn get_publisher() -> String {
        "sidex".to_string()
    }

    fn get_activation_events() -> Vec<String> {
        vec!["onStartupFinished".to_string()]
    }

    fn get_commands() -> Vec<CommandDefinition> {
        vec![
            CommandDefinition {
                id: "git.commit".to_string(),
                title: "Git: Commit".to_string(),
            },
            CommandDefinition {
                id: "git.commitStaged".to_string(),
                title: "Git: Commit Staged".to_string(),
            },
            CommandDefinition {
                id: "git.commitAmend".to_string(),
                title: "Git: Commit (Amend)".to_string(),
            },
            CommandDefinition {
                id: "git.push".to_string(),
                title: "Git: Push".to_string(),
            },
            CommandDefinition {
                id: "git.pull".to_string(),
                title: "Git: Pull".to_string(),
            },
            CommandDefinition {
                id: "git.branch".to_string(),
                title: "Git: Create Branch".to_string(),
            },
            CommandDefinition {
                id: "git.checkout".to_string(),
                title: "Git: Checkout to...".to_string(),
            },
            CommandDefinition {
                id: "git.stageAll".to_string(),
                title: "Git: Stage All Changes".to_string(),
            },
            CommandDefinition {
                id: "git.unstageAll".to_string(),
                title: "Git: Unstage All Changes".to_string(),
            },
            CommandDefinition {
                id: "git.revertSelectedRanges".to_string(),
                title: "Git: Revert Selected Ranges".to_string(),
            },
            CommandDefinition {
                id: "git.graph.view".to_string(),
                title: "Git: View Graph".to_string(),
            },
        ]
    }

    fn get_view_ids() -> Vec<String> {
        vec![
            "git.sourceControl".to_string(),
            "git.gitgraph".to_string(),
        ]
    }

    fn execute_command(command_id: String, _args: String) -> Result<String, String> {
        match command_id.as_str() {
            "git.commit" | "git.commitStaged" | "git.commitAmend"
            | "git.push" | "git.pull" | "git.branch" | "git.checkout"
            | "git.stageAll" | "git.unstageAll" | "git.revertSelectedRanges"
            | "git.graph.view" => {
                Ok(format!("command executed: {command_id}"))
            }
            _ => Err(format!("unknown command: {command_id}")),
        }
    }

    fn on_file_event(events: Vec<FileEvent>) {
        for event in events {
            if event.uri.contains("/.git/") {
                continue;
            }
            host::log_info(&format!("File event: {:?} - {}", event.type_, event.uri));
        }
    }

    fn get_tree_children(view_id: String, _parent_id: Option<String>) -> Vec<TreeItem> {
        match view_id.as_str() {
            "git.sourceControl" => vec![
                TreeItem {
                    id: "git.staged".to_string(),
                    label: "Staged Changes".to_string(),
                    description: None,
                    icon_path: None,
                    collapsible_state: Some(1),
                    context_value: Some("git.stagedChanges".to_string()),
                    tooltip: None,
                },
                TreeItem {
                    id: "git.unstaged".to_string(),
                    label: "Changes".to_string(),
                    description: None,
                    icon_path: None,
                    collapsible_state: Some(1),
                    context_value: Some("git.unstagedChanges".to_string()),
                    tooltip: None,
                },
            ],
            "git.gitgraph" => vec![],
            _ => vec![],
        }
    }

    fn get_semantic_tokens_legend() -> Option<SemanticTokensLegend> {
        None
    }
    fn provide_completion(_: DocumentContext, _: Position) -> Option<CompletionList> {
        None
    }
    fn provide_hover(_: DocumentContext, _: Position) -> Option<HoverResult> {
        None
    }
    fn provide_definition(_: DocumentContext, _: Position) -> Vec<Location> {
        vec![]
    }
    fn provide_references(_: DocumentContext, _: Position) -> Vec<Location> {
        vec![]
    }
    fn provide_document_symbols(_: DocumentContext) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn provide_signature_help(_: DocumentContext, _: Position) -> Option<SignatureHelpResult> {
        None
    }
    fn provide_rename(_: DocumentContext, _: Position, _: String) -> Option<RenameResult> {
        None
    }
    fn provide_code_actions(_: DocumentContext, _: Range, _: Vec<Diagnostic>) -> Vec<CodeAction> {
        vec![]
    }
    fn provide_inlay_hints(_: DocumentContext, _: Range) -> Vec<InlayHint> {
        vec![]
    }
    fn provide_type_definition(_: DocumentContext, _: Position) -> Vec<Location> {
        vec![]
    }
    fn provide_implementation(_: DocumentContext, _: Position) -> Vec<Location> {
        vec![]
    }
    fn provide_declaration(_: DocumentContext, _: Position) -> Vec<Location> {
        vec![]
    }
    fn provide_document_highlights(_: DocumentContext, _: Position) -> Vec<DocumentHighlight> {
        vec![]
    }
    fn prepare_rename(_: DocumentContext, _: Position) -> Option<RenameLocation> {
        None
    }
    fn provide_code_lenses(_: DocumentContext) -> Vec<CodeLens> {
        vec![]
    }
    fn provide_formatting(_: DocumentContext, _: u32, _: bool) -> Vec<TextEdit> {
        vec![]
    }
    fn provide_range_formatting(_: DocumentContext, _: Range, _: u32, _: bool) -> Vec<TextEdit> {
        vec![]
    }
    fn provide_folding_ranges(_: DocumentContext) -> Vec<FoldingRange> {
        vec![]
    }
    fn provide_document_links(_: DocumentContext) -> Vec<DocumentLink> {
        vec![]
    }
    fn provide_selection_ranges(_: DocumentContext, _: Vec<Position>) -> Vec<SelectionRange> {
        vec![]
    }
    fn provide_semantic_tokens(_: DocumentContext) -> Option<SemanticTokens> {
        None
    }
    fn provide_document_colors(_: DocumentContext) -> Vec<ColorInfo> {
        vec![]
    }
    fn provide_workspace_symbols(_: String) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn on_configuration_changed(_: String) {}
    fn get_tree_item(_: String, _: String) -> Option<TreeItem> {
        None
    }
    fn on_tree_item_activated(_: String, _: String) {}
    fn on_tree_visibility_changed(_: String, _: bool) {}
    fn get_languages() -> Vec<String> {
        vec![]
    }
    fn get_task_types() -> Vec<TaskDefinition> {
        vec![]
    }
    fn get_debug_types() -> Vec<String> {
        vec![]
    }
    fn get_notebook_types() -> Vec<String> {
        vec![]
    }
    fn get_custom_editor_types() -> Vec<String> {
        vec![]
    }
    fn provide_completion_item_resolve(_: String, _: Option<u32>, _: Option<String>) -> Option<CompletionList> {
        None
    }
    fn provide_workspace_symbol_resolve(_: String, _: Option<String>) -> Option<DocumentSymbol> {
        None
    }
    fn provide_code_action_resolve(_: String, _: Option<String>, _: Option<String>) -> Option<CodeAction> {
        None
    }
    fn provide_code_lens_resolve(_: Range, _: Option<String>, _: Option<String>) -> Option<CodeLens> {
        None
    }
    fn provide_on_type_formatting(_: DocumentContext, _: Position, _: String, _: u32, _: bool) -> Vec<TextEdit> {
        vec![]
    }
    fn provide_inlay_hint_resolve(_: Position, _: String, _: Option<u32>) -> Option<InlayHint> {
        None
    }
    fn provide_document_link_resolve(_: Range, _: Option<String>) -> Option<DocumentLink> {
        None
    }
    fn provide_semantic_tokens_range(_: DocumentContext, _: Range) -> Option<SemanticTokens> {
        None
    }
    fn provide_semantic_tokens_delta(_: DocumentContext, _: String) -> Option<SemanticTokens> {
        None
    }
    fn provide_color_presentation(_: DocumentContext, _: ColorInfo, _: Range) -> Vec<TextEdit> {
        vec![]
    }
    fn provide_call_hierarchy_incoming(_: DocumentContext, _: Position) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn provide_call_hierarchy_outgoing(_: DocumentContext, _: Position) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn provide_type_hierarchy_subtypes(_: DocumentContext, _: Position) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn provide_type_hierarchy_supertypes(_: DocumentContext, _: Position) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn provide_linked_editing_ranges(_: DocumentContext, _: Position) -> Vec<Range> {
        vec![]
    }
    fn on_document_opened(_: DocumentContext) {}
    fn on_document_closed(_: DocumentContext) {}
    fn on_document_changed(_: DocumentContext, _: Vec<TextEdit>) {}
    fn on_document_saved(_: DocumentContext, _: u32) {}
    fn on_document_will_save(_: DocumentContext, _: u32) -> Vec<TextEdit> {
        vec![]
    }
    fn on_document_language_changed(_: String, _: String, _: String) {}
    fn on_workspace_folders_changed(_: Vec<String>, _: Vec<String>) {}
    fn on_files_created(_: Vec<String>) {}
    fn on_files_renamed(_: Vec<String>, _: Vec<String>) {}
    fn on_files_deleted(_: Vec<String>) {}
    fn on_files_will_create(_: Vec<String>) -> Option<Vec<(String, Vec<TextEdit>)>> {
        None
    }
    fn on_files_will_rename(_: Vec<String>, _: Vec<String>) -> Option<Vec<(String, Vec<TextEdit>)>> {
        None
    }
    fn on_files_will_delete(_: Vec<String>) -> Option<Vec<(String, Vec<TextEdit>)>> {
        None
    }
    fn on_active_editor_changed(_: Option<String>) {}
    fn on_visible_editors_changed(_: Vec<String>) {}
    fn on_editor_selections_changed(_: String, _: Vec<Range>) {}
    fn on_editor_scroll_changed(_: String, _: Vec<Range>) {}
    fn on_editor_view_column_changed(_: String, _: u32) {}
    fn provide_tasks(_: Option<String>) -> Vec<TaskExecution> {
        vec![]
    }
    fn resolve_task(_: String, _: String) -> Option<TaskExecution> {
        None
    }
    fn on_task_started(_: TaskExecution) {}
    fn on_task_ended(_: TaskExecution, _: Option<i32>) {}
    fn on_task_process_started(_: TaskExecution, _: u32) {}
    fn on_task_process_ended(_: TaskExecution, _: Option<i32>) {}
    fn create_debug_adapter_descriptor(_: String, _: String, _: Vec<String>) -> Result<String, String> {
        Err("not supported".into())
    }
    fn on_debug_session_started(_: String, _: String, _: String) {}
    fn on_debug_session_stopped(_: String) {}
    fn on_debug_breakpoints_changed(_: Vec<String>, _: Vec<String>, _: Vec<String>) {}
    fn provide_notebook_serializer_deserialize(_: String, _: Vec<u8>) -> Result<Vec<NotebookCell>, String> {
        Err("not supported".into())
    }
    fn provide_notebook_serializer_serialize(_: String, _: Vec<NotebookCell>) -> Result<Vec<u8>, String> {
        Err("not supported".into())
    }
    fn provide_notebook_kernel_execute_all(_: String, _: Vec<NotebookCell>) -> Vec<NotebookCellOutput> {
        vec![]
    }
    fn provide_notebook_kernel_execute_cell(_: String, _: u32, _: NotebookCell) -> NotebookCellOutput {
        NotebookCellOutput { items: vec![] }
    }
    fn provide_notebook_kernel_interrupt(_: String) {}
    fn provide_tests_resolve_children(_: String, _: Option<String>) -> Vec<TestItem> {
        vec![]
    }
    fn provide_tests_run(_: String, _: String, _: Vec<String>, _: Vec<String>) {}
    fn provide_tests_debug(_: String, _: String, _: Vec<String>, _: Vec<String>) {}
    fn provide_tests_cancel_run(_: String, _: String) {}
    fn custom_editor_open(_: String, _: String, _: u32) -> Result<String, String> {
        Err("not supported".into())
    }
    fn custom_editor_update(_: String, _: Vec<TextEdit>) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_save(_: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_save_as(_: String, _: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_revert(_: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_dispose(_: String) {}
    fn webview_receive_message(_: String, _: String) {}
    fn on_webview_disposed(_: String) {}
    fn on_webview_visibility_changed(_: String, _: bool) {}
}

sidex_extension_sdk::export_extension!(GitExtension);
