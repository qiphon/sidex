use sidex_extension_sdk::prelude::*;

pub struct ReferencesViewExtension;

impl SidexExtension for ReferencesViewExtension {
    fn activate() -> Result<(), String> {
        host::log_info("References View extension activated");
        Ok(())
    }

    fn deactivate() {}

    fn get_name() -> String {
        "References View".to_string()
    }
    fn get_display_name() -> String {
        "References View".to_string()
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
        vec![]
    }

    fn provide_completion(_ctx: DocumentContext, _pos: Position) -> Option<CompletionList> {
        None
    }
    fn provide_completion_item_resolve(
        _label: String,
        _kind: Option<u32>,
        _data: Option<String>,
    ) -> Option<CompletionList> {
        None
    }

    fn provide_hover(_ctx: DocumentContext, _pos: Position) -> Option<HoverResult> {
        None
    }

    fn provide_definition(_ctx: DocumentContext, _pos: Position) -> Vec<Location> {
        vec![]
    }

    fn provide_references(_ctx: DocumentContext, _pos: Position) -> Vec<Location> {
        vec![]
    }

    fn provide_type_definition(_ctx: DocumentContext, _pos: Position) -> Vec<Location> {
        vec![]
    }

    fn provide_implementation(_ctx: DocumentContext, _pos: Position) -> Vec<Location> {
        vec![]
    }

    fn provide_declaration(_ctx: DocumentContext, _pos: Position) -> Vec<Location> {
        vec![]
    }

    fn provide_document_symbols(_ctx: DocumentContext) -> Vec<DocumentSymbol> {
        vec![]
    }

    fn provide_workspace_symbols(_query: String) -> Vec<DocumentSymbol> {
        vec![]
    }

    fn provide_workspace_symbol_resolve(
        _symbol_name: String,
        _container_name: Option<String>,
    ) -> Option<DocumentSymbol> {
        None
    }

    fn provide_code_actions(
        _ctx: DocumentContext,
        _range: Range,
        _diagnostics: Vec<Diagnostic>,
    ) -> Vec<CodeAction> {
        vec![]
    }

    fn provide_code_action_resolve(
        _title: String,
        _kind: Option<String>,
        _data: Option<String>,
    ) -> Option<CodeAction> {
        None
    }

    fn provide_code_lenses(_ctx: DocumentContext) -> Vec<CodeLens> {
        vec![]
    }

    fn provide_code_lens_resolve(
        _range: Range,
        _command_id: Option<String>,
        _data: Option<String>,
    ) -> Option<CodeLens> {
        None
    }

    fn provide_formatting(
        _ctx: DocumentContext,
        _tab_size: u32,
        _insert_spaces: bool,
    ) -> Vec<TextEdit> {
        vec![]
    }

    fn provide_range_formatting(
        _ctx: DocumentContext,
        _range: Range,
        _tab_size: u32,
        _insert_spaces: bool,
    ) -> Vec<TextEdit> {
        vec![]
    }

    fn provide_on_type_formatting(
        _ctx: DocumentContext,
        _pos: Position,
        _ch: String,
        _tab_size: u32,
        _insert_spaces: bool,
    ) -> Vec<TextEdit> {
        vec![]
    }

    fn provide_signature_help(_ctx: DocumentContext, _pos: Position) -> Option<SignatureHelpResult> {
        None
    }

    fn provide_document_highlights(_ctx: DocumentContext, _pos: Position) -> Vec<DocumentHighlight> {
        vec![]
    }

    fn provide_rename(
        _ctx: DocumentContext,
        _pos: Position,
        _new_name: String,
    ) -> Option<RenameResult> {
        None
    }

    fn prepare_rename(_ctx: DocumentContext, _pos: Position) -> Option<RenameLocation> {
        None
    }

    fn provide_folding_ranges(_ctx: DocumentContext) -> Vec<FoldingRange> {
        vec![]
    }

    fn provide_inlay_hints(_ctx: DocumentContext, _range: Range) -> Vec<InlayHint> {
        vec![]
    }

    fn provide_inlay_hint_resolve(
        _position: Position,
        _label: String,
        _kind: Option<u32>,
    ) -> Option<InlayHint> {
        None
    }

    fn provide_document_links(_ctx: DocumentContext) -> Vec<DocumentLink> {
        vec![]
    }

    fn provide_document_link_resolve(
        _range: Range,
        _target: Option<String>,
    ) -> Option<DocumentLink> {
        None
    }

    fn provide_selection_ranges(_ctx: DocumentContext, _positions: Vec<Position>) -> Vec<SelectionRange> {
        vec![]
    }

    fn provide_semantic_tokens(_ctx: DocumentContext) -> Option<SemanticTokens> {
        None
    }

    fn provide_semantic_tokens_range(_ctx: DocumentContext, _range: Range) -> Option<SemanticTokens> {
        None
    }

    fn provide_semantic_tokens_delta(_ctx: DocumentContext, _previous_result_id: String) -> Option<SemanticTokens> {
        None
    }

    fn get_semantic_tokens_legend() -> Option<SemanticTokensLegend> {
        None
    }

    fn provide_document_colors(_ctx: DocumentContext) -> Vec<ColorInfo> {
        vec![]
    }

    fn provide_color_presentation(
        _ctx: DocumentContext,
        _color: ColorInfo,
        _range: Range,
    ) -> Vec<TextEdit> {
        vec![]
    }

    fn provide_call_hierarchy_incoming(_ctx: DocumentContext, _pos: Position) -> Vec<DocumentSymbol> {
        vec![]
    }

    fn provide_call_hierarchy_outgoing(_ctx: DocumentContext, _pos: Position) -> Vec<DocumentSymbol> {
        vec![]
    }

    fn provide_type_hierarchy_subtypes(_ctx: DocumentContext, _pos: Position) -> Vec<DocumentSymbol> {
        vec![]
    }

    fn provide_type_hierarchy_supertypes(_ctx: DocumentContext, _pos: Position) -> Vec<DocumentSymbol> {
        vec![]
    }

    fn provide_linked_editing_ranges(_ctx: DocumentContext, _pos: Position) -> Vec<Range> {
        vec![]
    }

    fn execute_command(id: String, _args: String) -> Result<String, String> {
        Err(format!("unknown command: {id}"))
    }

    fn on_file_event(_events: Vec<FileEvent>) {}

    fn on_document_opened(_ctx: DocumentContext) {}
    fn on_document_closed(_ctx: DocumentContext) {}
    fn on_document_changed(_ctx: DocumentContext, _changes: Vec<TextEdit>) {}
    fn on_document_saved(_ctx: DocumentContext, _reason: u32) {}
    fn on_document_will_save(_ctx: DocumentContext, _reason: u32) -> Vec<TextEdit> {
        vec![]
    }
    fn on_document_language_changed(_uri: String, _old_lang: String, _new_lang: String) {}

    fn on_configuration_changed(_section: String) {}
    fn on_workspace_folders_changed(_added: Vec<String>, _removed: Vec<String>) {}
    fn on_files_created(_uris: Vec<String>) {}
    fn on_files_renamed(_old_uris: Vec<String>, _new_uris: Vec<String>) {}
    fn on_files_deleted(_uris: Vec<String>) {}
    fn on_files_will_create(_uris: Vec<String>) -> Option<Vec<(String, Vec<TextEdit>)>> {
        None
    }
    fn on_files_will_rename(_old_uris: Vec<String>, _new_uris: Vec<String>) -> Option<Vec<(String, Vec<TextEdit>)>> {
        None
    }
    fn on_files_will_delete(_uris: Vec<String>) -> Option<Vec<(String, Vec<TextEdit>)>> {
        None
    }

    fn on_active_editor_changed(_uri: Option<String>) {}
    fn on_visible_editors_changed(_uris: Vec<String>) {}
    fn on_editor_selections_changed(_uri: String, _selections: Vec<Range>) {}
    fn on_editor_scroll_changed(_uri: String, _visible_ranges: Vec<Range>) {}
    fn on_editor_view_column_changed(_uri: String, _view_column: u32) {}

    fn get_tree_children(_view_id: String, _element_id: Option<String>) -> Vec<TreeItem> {
        vec![]
    }
    fn get_tree_item(_view_id: String, _element_id: String) -> Option<TreeItem> {
        None
    }
    fn on_tree_item_activated(_view_id: String, _element_id: String) {}
    fn on_tree_visibility_changed(_view_id: String, _visible: bool) {}

    fn get_languages() -> Vec<String> {
        vec![]
    }
    fn get_task_types() -> Vec<TaskDefinition> {
        vec![]
    }
    fn get_debug_types() -> Vec<String> {
        vec![]
    }
    fn get_view_ids() -> Vec<String> {
        vec![]
    }
    fn get_notebook_types() -> Vec<String> {
        vec![]
    }
    fn get_custom_editor_types() -> Vec<String> {
        vec![]
    }

    fn provide_tasks(_filter_type: Option<String>) -> Vec<TaskExecution> {
        vec![]
    }
    fn resolve_task(_task_id: String, _task_type: String) -> Option<TaskExecution> {
        None
    }
    fn on_task_started(_execution: TaskExecution) {}
    fn on_task_ended(_execution: TaskExecution, _exit_code: Option<i32>) {}
    fn on_task_process_started(_execution: TaskExecution, _pid: u32) {}
    fn on_task_process_ended(_execution: TaskExecution, _exit_code: Option<i32>) {}

    fn create_debug_adapter_descriptor(
        _debug_type: String,
        _executable: String,
        _args: Vec<String>,
    ) -> Result<String, String> {
        Err("not supported".into())
    }
    fn on_debug_session_started(_session_id: String, _session_name: String, _debug_type: String) {}
    fn on_debug_session_stopped(_session_id: String) {}
    fn on_debug_breakpoints_changed(
        _added: Vec<String>,
        _removed: Vec<String>,
        _changed: Vec<String>,
    ) {}

    fn provide_notebook_serializer_deserialize(
        _notebook_type: String,
        _data: Vec<u8>,
    ) -> Result<Vec<NotebookCell>, String> {
        Err("not supported".into())
    }
    fn provide_notebook_serializer_serialize(
        _notebook_type: String,
        _cells: Vec<NotebookCell>,
    ) -> Result<Vec<u8>, String> {
        Err("not supported".into())
    }
    fn provide_notebook_kernel_execute_all(
        _notebook_uri: String,
        _cells: Vec<NotebookCell>,
    ) -> Vec<NotebookCellOutput> {
        vec![]
    }
    fn provide_notebook_kernel_execute_cell(
        _notebook_uri: String,
        _cell_index: u32,
        _cell: NotebookCell,
    ) -> NotebookCellOutput {
        NotebookCellOutput { items: vec![] }
    }
    fn provide_notebook_kernel_interrupt(_notebook_uri: String) {}

    fn provide_tests_resolve_children(
        _controller_id: String,
        _item_id: Option<String>,
    ) -> Vec<TestItem> {
        vec![]
    }
    fn provide_tests_run(
        _controller_id: String,
        _run_id: String,
        _items_to_run: Vec<String>,
        _items_to_exclude: Vec<String>,
    ) {}
    fn provide_tests_debug(
        _controller_id: String,
        _run_id: String,
        _items_to_run: Vec<String>,
        _items_to_exclude: Vec<String>,
    ) {}
    fn provide_tests_cancel_run(_controller_id: String, _run_id: String) {}

    fn custom_editor_open(
        _editor_type: String,
        _uri: String,
        _view_column: u32,
    ) -> Result<String, String> {
        Err("not supported".into())
    }
    fn custom_editor_update(_editor_id: String, _changes: Vec<TextEdit>) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_save(_editor_id: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_save_as(_editor_id: String, _destination_uri: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_revert(_editor_id: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_dispose(_editor_id: String) {}

    fn webview_receive_message(_panel_id: String, _message: String) {}
    fn on_webview_disposed(_panel_id: String) {}
    fn on_webview_visibility_changed(_panel_id: String, _visible: bool) {}
}

sidex_extension_sdk::export_extension!(ReferencesViewExtension);
