use sidex_extension_sdk::prelude::*;

/// NPM script support for SideX.
/// Provides task detection from package.json scripts and basic NPM commands.
pub struct NpmExtension;

impl SidexExtension for NpmExtension {
    fn activate() -> Result<(), String> {
        host::log_info("NPM Support extension activated");
        Ok(())
    }

    fn deactivate() {}

    fn get_name() -> String {
        "NPM Support".to_string()
    }
    fn get_display_name() -> String {
        "NPM Support".to_string()
    }
    fn get_version() -> String {
        "0.1.0".to_string()
    }
    fn get_publisher() -> String {
        "sidex".to_string()
    }

    fn get_activation_events() -> Vec<String> {
        vec!["workspaceContains:package.json".to_string()]
    }

    fn get_commands() -> Vec<CommandDefinition> {
        vec![
            CommandDefinition {
                id: "npm.runScript".to_string(),
                title: "NPM: Run Script".to_string(),
            },
            CommandDefinition {
                id: "npm.refreshScripts".to_string(),
                title: "NPM: Refresh Scripts".to_string(),
            },
            CommandDefinition {
                id: "npm.install".to_string(),
                title: "NPM: Install Dependencies".to_string(),
            },
        ]
    }

    fn get_task_types() -> Vec<TaskDefinition> {
        vec![TaskDefinition {
            task_type: "npm".to_string(),
            required: vec!["script".to_string()],
            properties: vec![TaskProperty {
                name: "script".to_string(),
                description: Some("The NPM script to run".to_string()),
                default_value: None,
            }],
        }]
    }

    fn provide_tasks(_filter_type: Option<String>) -> Vec<TaskExecution> {
        get_scripts_from_workspace()
    }

    fn execute_command(command_id: String, args: String) -> Result<String, String> {
        match command_id.as_str() {
            "npm.runScript" => {
                let script = args.trim_matches('"');
                if script.is_empty() {
                    return Err("No script specified".to_string());
                }
                let task = TaskExecution {
                    id: format!("npm-run-{script}"),
                    name: format!("npm run {script}"),
                    source: "npm".to_string(),
                    detail: Some(format!("Run NPM script: {script}")),
                    is_background: false,
                    kind: TaskKind::Shell,
                    command: Some("npm".to_string()),
                    args: vec!["run".to_string(), script.to_string()],
                };
                match host::execute_task(&task) {
                    Ok(_) => Ok(format!("started: {script}")),
                    Err(e) => Err(e),
                }
            }
            "npm.install" => {
                let task = TaskExecution {
                    id: "npm-install".to_string(),
                    name: "npm install".to_string(),
                    source: "npm".to_string(),
                    detail: Some("Install NPM dependencies".to_string()),
                    is_background: false,
                    kind: TaskKind::Shell,
                    command: Some("npm".to_string()),
                    args: vec!["install".to_string()],
                };
                match host::execute_task(&task) {
                    Ok(_) => Ok("started: npm install".to_string()),
                    Err(e) => Err(e),
                }
            }
            _ => Err(format!("unknown command: {command_id}")),
        }
    }

    fn get_languages() -> Vec<String> {
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
    fn get_semantic_tokens_legend() -> Option<SemanticTokensLegend> {
        None
    }

    // -- All provider stubs return empty/None --

    fn provide_completion(_: DocumentContext, _: Position) -> Option<CompletionList> {
        None
    }
    fn provide_hover(_: DocumentContext, _: Position) -> Option<HoverResult> {
        None
    }
    fn provide_definition(_: DocumentContext, _: Position) -> Vec<Location> {
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
    fn provide_references(_: DocumentContext, _: Position) -> Vec<Location> {
        vec![]
    }
    fn provide_document_symbols(_: DocumentContext) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn provide_workspace_symbols(_: String) -> Vec<DocumentSymbol> {
        vec![]
    }
    fn provide_code_actions(
        _: DocumentContext,
        _: Range,
        _: Vec<Diagnostic>,
    ) -> Vec<CodeAction> {
        vec![]
    }
    fn provide_code_lenses(_: DocumentContext) -> Vec<CodeLens> {
        vec![]
    }
    fn provide_signature_help(_: DocumentContext, _: Position) -> Option<SignatureHelpResult> {
        None
    }
    fn provide_rename(_: DocumentContext, _: Position, _: String) -> Option<RenameResult> {
        None
    }
    fn prepare_rename(_: DocumentContext, _: Position) -> Option<RenameLocation> {
        None
    }
    fn provide_formatting(_: DocumentContext, _: u32, _: bool) -> Vec<TextEdit> {
        vec![]
    }
    fn provide_range_formatting(_: DocumentContext, _: Range, _: u32, _: bool) -> Vec<TextEdit> {
        vec![]
    }
    fn provide_on_type_formatting(
        _: DocumentContext,
        _: Position,
        _: String,
        _: u32,
        _: bool,
    ) -> Vec<TextEdit> {
        vec![]
    }
    fn provide_document_highlights(_: DocumentContext, _: Position) -> Vec<DocumentHighlight> {
        vec![]
    }
    fn provide_folding_ranges(_: DocumentContext) -> Vec<FoldingRange> {
        vec![]
    }
    fn provide_inlay_hints(_: DocumentContext, _: Range) -> Vec<InlayHint> {
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
    fn provide_semantic_tokens_range(_: DocumentContext, _: Range) -> Option<SemanticTokens> {
        None
    }
    fn provide_semantic_tokens_delta(_: DocumentContext, _: String) -> Option<SemanticTokens> {
        None
    }
    fn provide_document_colors(_: DocumentContext) -> Vec<ColorInfo> {
        vec![]
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

    // -- Resolve stubs --

    fn provide_completion_item_resolve(
        _: String,
        _: Option<u32>,
        _: Option<String>,
    ) -> Option<CompletionList> {
        None
    }
    fn provide_workspace_symbol_resolve(
        _: String,
        _: Option<String>,
    ) -> Option<DocumentSymbol> {
        None
    }
    fn provide_code_action_resolve(_: String, _: Option<String>, _: Option<String>) -> Option<CodeAction> {
        None
    }
    fn provide_code_lens_resolve(_: Range, _: Option<String>, _: Option<String>) -> Option<CodeLens> {
        None
    }
    fn provide_inlay_hint_resolve(_: Position, _: String, _: Option<u32>) -> Option<InlayHint> {
        None
    }
    fn provide_document_link_resolve(_: Range, _: Option<String>) -> Option<DocumentLink> {
        None
    }

    // -- Event stubs --

    fn on_file_event(_: Vec<FileEvent>) {}
    fn on_document_opened(_: DocumentContext) {}
    fn on_document_closed(_: DocumentContext) {}
    fn on_document_changed(_: DocumentContext, _: Vec<TextEdit>) {}
    fn on_document_saved(_: DocumentContext, _: u32) {}
    fn on_document_will_save(_: DocumentContext, _: u32) -> Vec<TextEdit> {
        vec![]
    }
    fn on_document_language_changed(_: String, _: String, _: String) {}
    fn on_configuration_changed(_: String) {}
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

    // -- Tree view stubs --

    fn get_tree_children(_: String, _: Option<String>) -> Vec<TreeItem> {
        vec![]
    }
    fn get_tree_item(_: String, _: String) -> Option<TreeItem> {
        None
    }
    fn on_tree_item_activated(_: String, _: String) {}
    fn on_tree_visibility_changed(_: String, _: bool) {}

    // -- Task stubs --

    fn resolve_task(_: String, _: String) -> Option<TaskExecution> {
        None
    }
    fn on_task_started(_: TaskExecution) {}
    fn on_task_ended(_: TaskExecution, _: Option<i32>) {}
    fn on_task_process_started(_: TaskExecution, _: u32) {}
    fn on_task_process_ended(_: TaskExecution, _: Option<i32>) {}

    // -- Debug stubs --

    fn create_debug_adapter_descriptor(_: String, _: String, _: Vec<String>) -> Result<String, String> {
        Err("not supported".into())
    }
    fn on_debug_session_started(_: String, _: String, _: String) {}
    fn on_debug_session_stopped(_: String) {}
    fn on_debug_breakpoints_changed(_: Vec<String>, _: Vec<String>, _: Vec<String>) {}

    // -- Notebook stubs --

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

    // -- Test stubs --

    fn provide_tests_resolve_children(_: String, _: Option<String>) -> Vec<TestItem> {
        vec![]
    }
    fn provide_tests_run(_: String, _: String, _: Vec<String>, _: Vec<String>) {}
    fn provide_tests_debug(_: String, _: String, _: Vec<String>, _: Vec<String>) {}
    fn provide_tests_cancel_run(_: String, _: String) {}

    // -- Custom editor stubs --

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

    // -- Webview stubs --

    fn webview_receive_message(_: String, _: String) {}
    fn on_webview_disposed(_: String) {}
    fn on_webview_visibility_changed(_: String, _: bool) {}
}

/// Parses package.json files in the workspace and returns NPM scripts as tasks.
fn get_scripts_from_workspace() -> Vec<TaskExecution> {
    let folders = host::get_workspace_folders();
    let mut tasks = Vec::new();

    for folder in &folders {
        let package_json_uri = format!("{folder}/package.json");
        if let Some(content) = host::get_document_text(&package_json_uri) {
            let scripts = parse_npm_scripts(&content);
            for (name, _cmd) in scripts {
                tasks.push(TaskExecution {
                    id: format!("npm:{name}"),
                    name: format!("npm run {name}"),
                    source: "npm".to_string(),
                    detail: Some(format!("NPM script: {name}")),
                    is_background: false,
                    kind: TaskKind::Shell,
                    command: Some("npm".to_string()),
                    args: vec!["run".to_string(), name.clone()],
                });
            }
        }
    }

    tasks
}

/// Simple JSON parser to extract script names from package.json "scripts" section.
/// This is a minimal parser that looks for key-value pairs inside the "scripts" object.
fn parse_npm_scripts(content: &str) -> Vec<(String, String)> {
    let mut scripts = Vec::new();

    // Find the "scripts" section
    let scripts_key = r#""scripts""#;
    let scripts_start = match content.find(scripts_key) {
        Some(pos) => pos,
        None => return scripts,
    };

    // Find the opening brace of the scripts object
    let after_key = &content[scripts_start + scripts_key.len()..];
    let brace_pos = match after_key.find('{') {
        Some(pos) => pos,
        None => return scripts,
    };

    let scripts_body_start = scripts_start + scripts_key.len() + brace_pos + 1;

    // Find the matching closing brace
    let mut depth = 1;
    let mut pos = scripts_body_start;
    let chars: Vec<char> = content.chars().collect();
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &ch) in chars.iter().enumerate().skip(scripts_body_start) {
        if escape_next {
            escape_next = false;
            continue;
        }
        match ch {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    pos = i;
                    break;
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return scripts;
    }

    let scripts_content: String = chars[scripts_body_start..pos].iter().collect();

    // Parse key-value pairs from the scripts object
    let mut scanner = scripts_content.chars().peekable();
    loop {
        // Skip whitespace and commas
        skip_whitespace_and_commas(&mut scanner);

        // Check for end
        if scanner.peek().is_none() {
            break;
        }

        // Parse key (script name)
        let key = match parse_json_string(&mut scanner) {
            Some(k) => k,
            None => break,
        };

        // Skip colon
        skip_whitespace_and_commas(&mut scanner);
        if scanner.next() != Some(':') {
            break;
        }
        skip_whitespace_and_commas(&mut scanner);

        // Parse value (script command)
        let value = match parse_json_string(&mut scanner) {
            Some(v) => v,
            None => break,
        };

        scripts.push((key, value));
    }

    scripts
}

fn skip_whitespace_and_commas(chars: &mut std::iter::Peekable<std::str::Chars>) {
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() || ch == ',' {
            chars.next();
        } else {
            break;
        }
    }
}

fn parse_json_string(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<String> {
    if chars.next() != Some('"') {
        return None;
    }

    let mut result = String::new();
    let mut escape_next = false;

    loop {
        match chars.next()? {
            '"' if !escape_next => break,
            '\\' if !escape_next => escape_next = true,
            c => {
                result.push(c);
                escape_next = false;
            }
        }
    }

    Some(result)
}

sidex_extension_sdk::export_extension!(NpmExtension);
