use sidex_extension_sdk::prelude::*;

/// Markdown language features for SideX.
/// Provides: formatting, folding, document links, and document symbols.
pub struct MarkdownExtension;

impl SidexExtension for MarkdownExtension {
    fn activate() -> Result<(), String> {
        Ok(())
    }

    fn deactivate() {}

    fn get_name() -> String {
        "Markdown Support".to_string()
    }

    fn get_display_name() -> String {
        "Markdown Support".to_string()
    }

    fn get_version() -> String {
        "0.1.0".to_string()
    }

    fn get_publisher() -> String {
        "sidex".to_string()
    }

    fn get_activation_events() -> Vec<String> {
        vec!["onLanguage:markdown".to_string()]
    }

    fn get_languages() -> Vec<String> {
        vec!["markdown".to_string()]
    }

    fn get_commands() -> Vec<CommandDefinition> {
        vec![
            CommandDefinition {
                id: "markdown.openPreview".to_string(),
                title: "Markdown: Open Preview".to_string(),
            },
            CommandDefinition {
                id: "markdown.openPreviewToSide".to_string(),
                title: "Markdown: Open Preview to the Side".to_string(),
            },
            CommandDefinition {
                id: "markdown.showLockedViewToSide".to_string(),
                title: "Markdown: Show Locked Preview to the Side".to_string(),
            },
        ]
    }

    fn provide_folding_ranges(ctx: DocumentContext) -> Vec<FoldingRange> {
        if !is_markdown(&ctx.language_id) {
            return vec![];
        }
        let text = match host::get_document_text(&ctx.uri) {
            Some(t) => t,
            None => return vec![],
        };
        parse_markdown_folding_ranges(&text)
    }

    fn provide_formatting(ctx: DocumentContext, _tab_size: u32, _insert_spaces: bool) -> Vec<TextEdit> {
        if !is_markdown(&ctx.language_id) {
            return vec![];
        }
        // Stub: return empty edits. Full formatting would trim trailing
        // whitespace and ensure a single trailing newline.
        vec![]
    }

    fn provide_range_formatting(
        ctx: DocumentContext,
        _range: Range,
        _tab_size: u32,
        _insert_spaces: bool,
    ) -> Vec<TextEdit> {
        if !is_markdown(&ctx.language_id) {
            return vec![];
        }
        vec![]
    }

    fn provide_document_links(ctx: DocumentContext) -> Vec<DocumentLink> {
        if !is_markdown(&ctx.language_id) {
            return vec![];
        }
        let text = match host::get_document_text(&ctx.uri) {
            Some(t) => t,
            None => return vec![],
        };
        parse_markdown_links(&text)
    }

    fn provide_document_symbols(ctx: DocumentContext) -> Vec<DocumentSymbol> {
        if !is_markdown(&ctx.language_id) {
            return vec![];
        }
        let text = match host::get_document_text(&ctx.uri) {
            Some(t) => t,
            None => return vec![],
        };
        parse_markdown_headings(&text)
    }

    fn get_semantic_tokens_legend() -> Option<SemanticTokensLegend> {
        None
    }
    fn provide_completion(_ctx: DocumentContext, _pos: Position) -> Option<CompletionList> {
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
    fn provide_document_highlights(_ctx: DocumentContext, _pos: Position) -> Vec<DocumentHighlight> {
        vec![]
    }
    fn provide_signature_help(_ctx: DocumentContext, _pos: Position) -> Option<SignatureHelpResult> {
        None
    }
    fn provide_rename(_ctx: DocumentContext, _pos: Position, _new_name: String) -> Option<RenameResult> {
        None
    }
    fn prepare_rename(_ctx: DocumentContext, _pos: Position) -> Option<RenameLocation> {
        None
    }
    fn provide_code_actions(_ctx: DocumentContext, _range: Range, _diags: Vec<Diagnostic>) -> Vec<CodeAction> {
        vec![]
    }
    fn provide_code_lenses(_ctx: DocumentContext) -> Vec<CodeLens> {
        vec![]
    }
    fn provide_inlay_hints(_ctx: DocumentContext, _range: Range) -> Vec<InlayHint> {
        vec![]
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
    fn provide_document_colors(_ctx: DocumentContext) -> Vec<ColorInfo> {
        vec![]
    }
    fn provide_color_presentation(_ctx: DocumentContext, _color: ColorInfo, _range: Range) -> Vec<TextEdit> {
        vec![]
    }
    fn provide_workspace_symbols(_query: String) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn provide_on_type_formatting(_ctx: DocumentContext, _pos: Position, _ch: String, _tab_size: u32, _insert_spaces: bool) -> Vec<TextEdit> {
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
    fn on_file_event(_events: Vec<FileEvent>) {}
    fn execute_command(_command_id: String, _args: String) -> Result<String, String> {
        Err("unknown command".to_string())
    }
    fn on_configuration_changed(_config: String) {}
    fn get_tree_children(_view_id: String, _element: Option<String>) -> Vec<TreeItem> {
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
        vec![
            "markdown.preview".to_string(),
        ]
    }
    fn provide_completion_item_resolve(_item_label: String, _kind: Option<u32>, _detail: Option<String>) -> Option<CompletionList> {
        None
    }
    fn provide_workspace_symbol_resolve(_name: String, _kind: Option<u32>) -> Option<DocumentSymbol> {
        None
    }
    fn provide_code_action_resolve(_title: String, _kind: Option<String>, _command_id: Option<String>) -> Option<CodeAction> {
        None
    }
    fn provide_code_lens_resolve(_range: Range, _command_id: Option<String>, _uri: Option<String>) -> Option<CodeLens> {
        None
    }
    fn provide_inlay_hint_resolve(_pos: Position, _label: String, _kind: Option<u32>) -> Option<InlayHint> {
        None
    }
    fn provide_document_link_resolve(_range: Range, _uri: Option<String>) -> Option<DocumentLink> {
        None
    }
    fn on_document_opened(_ctx: DocumentContext) {}
    fn on_document_closed(_ctx: DocumentContext) {}
    fn on_document_changed(_ctx: DocumentContext, _changes: Vec<TextEdit>) {}
    fn on_document_saved(_ctx: DocumentContext, _version: u32) {}
    fn on_document_will_save(_ctx: DocumentContext, _version: u32) -> Vec<TextEdit> {
        vec![]
    }
    fn on_document_language_changed(_uri: String, _old_language: String, _new_language: String) {}
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
    fn get_tree_item(_view_id: String, _element: String) -> Option<TreeItem> {
        None
    }
    fn on_tree_item_activated(_view_id: String, _element: String) {}
    fn on_tree_visibility_changed(_view_id: String, _visible: bool) {}
    fn provide_tasks(_filter: Option<String>) -> Vec<TaskExecution> {
        vec![]
    }
    fn resolve_task(_type_: String, _label: String) -> Option<TaskExecution> {
        None
    }
    fn on_task_started(_task: TaskExecution) {}
    fn on_task_ended(_task: TaskExecution, _exit_code: Option<i32>) {}
    fn on_task_process_started(_task: TaskExecution, _process_id: u32) {}
    fn on_task_process_ended(_task: TaskExecution, _exit_code: Option<i32>) {}
    fn create_debug_adapter_descriptor(_type_: String, _uri: String, _args: Vec<String>) -> Result<String, String> {
        Err("not supported".into())
    }
    fn on_debug_session_started(_type_: String, _request: String, _uri: String) {}
    fn on_debug_session_stopped(_reason: String) {}
    fn on_debug_breakpoints_changed(_added: Vec<String>, _removed: Vec<String>, _changed: Vec<String>) {}
    fn provide_notebook_serializer_deserialize(_type_: String, _data: Vec<u8>) -> Result<Vec<NotebookCell>, String> {
        Err("not supported".into())
    }
    fn provide_notebook_serializer_serialize(_type_: String, _cells: Vec<NotebookCell>) -> Result<Vec<u8>, String> {
        Err("not supported".into())
    }
    fn provide_notebook_kernel_execute_all(_kernel: String, _cells: Vec<NotebookCell>) -> Vec<NotebookCellOutput> {
        vec![]
    }
    fn provide_notebook_kernel_execute_cell(_kernel: String, _index: u32, _cell: NotebookCell) -> NotebookCellOutput {
        NotebookCellOutput { items: vec![] }
    }
    fn provide_notebook_kernel_interrupt(_kernel: String) {}
    fn provide_tests_resolve_children(_provider: String, _element: Option<String>) -> Vec<TestItem> {
        vec![]
    }
    fn provide_tests_run(_provider: String, _kind: String, _items: Vec<String>, _exclude: Vec<String>) {}
    fn provide_tests_debug(_provider: String, _kind: String, _items: Vec<String>, _exclude: Vec<String>) {}
    fn provide_tests_cancel_run(_provider: String, _kind: String) {}
    fn custom_editor_open(_editor_type: String, _uri: String, _options: u32) -> Result<String, String> {
        Err("not supported".into())
    }
    fn custom_editor_update(_uri: String, _edits: Vec<TextEdit>) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_save(_uri: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_save_as(_old_uri: String, _new_uri: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_revert(_uri: String) -> Result<(), String> {
        Err("not supported".into())
    }
    fn custom_editor_dispose(_uri: String) {}
    fn webview_receive_message(_handle: String, _message: String) {}
    fn on_webview_disposed(_handle: String) {}
    fn on_webview_visibility_changed(_handle: String, _visible: bool) {}
}

fn is_markdown(lang: &str) -> bool {
    lang == "markdown"
}

fn parse_markdown_folding_ranges(text: &str) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with('#') && !line.starts_with("##") {
            let start = i;
            i += 1;
            while i < lines.len() && !lines[i].starts_with('#') {
                i += 1;
            }
            if i > start + 1 {
                ranges.push(FoldingRange {
                    start_line: start as u32,
                    end_line: (i - 1) as u32,
                    kind: None,
                });
            }
        } else if line.starts_with("##") {
            let start = i;
            i += 1;
            while i < lines.len() && !lines[i].starts_with('#') {
                i += 1;
            }
            if i > start + 1 {
                ranges.push(FoldingRange {
                    start_line: start as u32,
                    end_line: (i - 1) as u32,
                    kind: None,
                });
            }
        } else {
            i += 1;
        }
    }

    ranges
}

fn parse_markdown_links(text: &str) -> Vec<DocumentLink> {
    let mut links = Vec::new();

    for (line_idx, line) in text.lines().enumerate() {
        let mut col = 0;
        let chars: Vec<char> = line.chars().collect();

        while col < chars.len() {
            if chars[col] == '[' {
                if let Some(close_bracket) = chars[col + 1..].iter().position(|&c| c == ']') {
                    let bracket_end = col + 1 + close_bracket;
                    if bracket_end + 1 < chars.len() && chars[bracket_end + 1] == '(' {
                        if let Some(close_paren) = chars[bracket_end + 2..].iter().position(|&c| c == ')') {
                            let url_start = bracket_end + 2;
                            let url_end = url_start + close_paren;
                            links.push(DocumentLink {
                                range: Range {
                                    start: Position {
                                        line: line_idx as u32,
                                        character: url_start as u32,
                                    },
                                    end: Position {
                                        line: line_idx as u32,
                                        character: url_end as u32,
                                    },
                                },
                                target: None,
                                tooltip: None,
                            });
                            col = url_end + 1;
                            continue;
                        }
                    }
                }
            }
            col += 1;
        }
    }

    links
}

fn parse_markdown_headings(text: &str) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for (line_idx, line) in text.lines().enumerate() {
        if let Some(stripped) = line.strip_prefix('#') {
            let trimmed = stripped.trim_start();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                let heading_text = trimmed.splitn(2, '#').next().unwrap_or(trimmed).trim();
                let level = line.chars().take_while(|&c| c == '#').count();
                symbols.push(DocumentSymbol {
                    name: heading_text.to_string(),
                    detail: Some(format!("Heading level {level}")),
                    kind: 14,
                    range: Range {
                        start: Position {
                            line: line_idx as u32,
                            character: 0,
                        },
                        end: Position {
                            line: line_idx as u32,
                            character: line.len() as u32,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: line_idx as u32,
                            character: 0,
                        },
                        end: Position {
                            line: line_idx as u32,
                            character: line.len() as u32,
                        },
                    },
                });
            }
        }
    }

    symbols
}

sidex_extension_sdk::export_extension!(MarkdownExtension);
