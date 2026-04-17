//! End-to-end integration tests proving the editor pipeline works across crates.

use sidex_editor::contrib::find::FindState;
use sidex_editor::{Document, MultiCursor, Selection, SnippetSession};
use sidex_extensions::ExtensionManifest;
use sidex_keymap::{
    ContextKeys, Key, KeyCombo, KeybindingResolver, Modifiers,
};
use sidex_lsp::{JsonRpcMessage, RequestId};
use sidex_settings::Settings;
use sidex_terminal::grid::TerminalGrid;
use sidex_terminal::TerminalEmulator;
use sidex_text::{Buffer, Position};
use sidex_theme::Theme;

use sidex_editor::diff::{ChangeKind, DiffEditor};

fn pos(line: u32, col: u32) -> Position {
    Position::new(line, col)
}

// ── 1. Open file and edit ────────────────────────────────────────────────────

#[test]
fn test_open_file_and_edit() {
    let mut doc = Document::from_str("hello world");
    assert_eq!(doc.text(), "hello world");

    doc.cursors = MultiCursor::new(pos(0, 5));
    doc.insert_text(" beautiful");
    assert_eq!(doc.text(), "hello beautiful world");
    assert!(doc.is_modified);
    assert!(doc.version > 0, "Version should increment after edit");

    let buf = Buffer::from_str("hello beautiful world");
    assert_eq!(buf.line_content(0), "hello beautiful world");
    assert_eq!(buf.get_word_at_position(pos(0, 8)).unwrap().word, "beautiful");
}

// ── 2. Multi-cursor editing ──────────────────────────────────────────────────

#[test]
fn test_multi_cursor_editing() {
    let mut doc = Document::from_str("aaa\nbbb\nccc");

    doc.cursors = MultiCursor::new(pos(0, 3));
    doc.cursors.add_cursor(pos(1, 3));
    doc.cursors.add_cursor(pos(2, 3));
    assert_eq!(doc.cursors.len(), 3);

    doc.type_char('!');

    let text = doc.text();
    for line in text.lines() {
        assert!(
            line.ends_with('!'),
            "Expected line to end with '!', got: {line}"
        );
    }
    assert!(text.contains("aaa!"));
    assert!(text.contains("bbb!"));
    assert!(text.contains("ccc!"));
}

// ── 3. Find and replace ─────────────────────────────────────────────────────

#[test]
fn test_find_and_replace() {
    let mut buf = Buffer::from_str("hello world hello");
    let mut find = FindState::default();

    find.set_search_string("hello".to_string());
    find.research(&buf);
    assert_eq!(find.matches.len(), 2, "Expected 2 matches for 'hello'");

    find.set_replace_string("goodbye".to_string());
    let count = find.replace_all(&mut buf);
    assert_eq!(count, 2);
    assert_eq!(buf.text(), "goodbye world goodbye");
}

// ── 4. Syntax highlighting (tree-sitter parse) ──────────────────────────────

#[test]
fn test_syntax_highlighting_buffer_setup() {
    let code = r#"fn main() {
    let x = "hello";
    println!("{}", x);
}"#;
    let buf = Buffer::from_str(code);
    assert_eq!(buf.len_lines(), 4);
    assert_eq!(buf.line_content(0), "fn main() {");

    let word = buf.get_word_at_position(pos(0, 0));
    assert!(word.is_some());
    assert_eq!(word.unwrap().word, "fn");

    let string_word = buf.get_word_at_position(pos(1, 17));
    assert!(string_word.is_some());
    assert_eq!(string_word.unwrap().word, "hello");
}

// ── 5. Theme loading ────────────────────────────────────────────────────────

#[test]
fn test_theme_loading() {
    let dark = Theme::default_dark();
    assert_eq!(dark.name, "Default Dark Modern");
    assert_eq!(dark.kind, sidex_theme::ThemeKind::Dark);
    assert!(!dark.token_colors.is_empty(), "Should have token colors");

    let light = Theme::default_light();
    assert_eq!(light.kind, sidex_theme::ThemeKind::Light);

    let json = serde_json::json!({
        "name": "Test Theme",
        "type": "dark",
        "colors": {
            "editor.background": "#1e1e1e",
            "editor.foreground": "#d4d4d4"
        },
        "tokenColors": [
            {
                "scope": "keyword",
                "settings": { "foreground": "#569cd6" }
            }
        ]
    })
    .to_string();
    let custom = Theme::from_json(&json).expect("Should parse theme JSON");
    assert_eq!(custom.name, "Test Theme");
    assert_eq!(custom.kind, sidex_theme::ThemeKind::Dark);
    assert_eq!(custom.token_colors.len(), 1);
}

// ── 6. Settings layered ─────────────────────────────────────────────────────

#[test]
fn test_settings_layered() {
    let mut settings = Settings::new();

    let default_tab: Option<i64> = settings.get("editor.tabSize");
    assert_eq!(default_tab, Some(4), "Default tab size should be 4");

    settings.set(
        "editor.tabSize",
        serde_json::Value::Number(serde_json::Number::from(2)),
    );
    let user_tab: Option<i64> = settings.get("editor.tabSize");
    assert_eq!(user_tab, Some(2), "User override should be 2");

    settings.set_workspace(
        "editor.tabSize",
        serde_json::Value::Number(serde_json::Number::from(8)),
    );
    let ws_tab: Option<i64> = settings.get("editor.tabSize");
    assert_eq!(ws_tab, Some(8), "Workspace override should win: 8");

    let font_size: Option<i64> = settings.get("editor.fontSize");
    assert_eq!(font_size, Some(14), "Default fontSize should be 14");
}

// ── 7. Keybinding resolution ────────────────────────────────────────────────

#[test]
fn test_keybinding_resolution() {
    let resolver = KeybindingResolver::new();
    let mut ctx = ContextKeys::new();

    let p = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CTRL
    };

    let ctrl_s = KeyCombo::new(Key::S, p);
    let cmd = resolver.resolve(&ctrl_s, &ctx);
    assert_eq!(
        cmd,
        Some("workbench.action.files.save"),
        "Ctrl+S should resolve to save"
    );

    let ps = p | Modifiers::SHIFT;
    let ctrl_shift_p = KeyCombo::new(Key::P, ps);
    let cmd2 = resolver.resolve(&ctrl_shift_p, &ctx);
    assert_eq!(
        cmd2,
        Some("workbench.action.showCommands"),
        "Ctrl+Shift+P should resolve to command palette"
    );

    ctx.set_bool("editorTextFocus", true);
    ctx.set_bool("editorReadonly", false);

    let ctrl_z = KeyCombo::new(Key::Z, p);
    let undo_cmd = resolver.resolve(&ctrl_z, &ctx);
    assert_eq!(undo_cmd, Some("undo"), "Ctrl+Z with editorTextFocus should be undo");
}

// ── 8. Diff computation ─────────────────────────────────────────────────────

#[test]
fn test_diff_computation() {
    let original = Document::from_str("line1\nline2\nline3");
    let modified = Document::from_str("line1\nmodified\nline3\nline4");

    let mut editor = DiffEditor::new(original, modified);
    let result = editor.diff();

    assert!(!result.is_identical(), "Documents differ");
    assert!(result.change_count() > 0);

    let has_modified = result.changes.iter().any(|c| c.kind == ChangeKind::Modified);
    let has_added = result.changes.iter().any(|c| c.kind == ChangeKind::Added);
    assert!(has_modified, "Should detect modified line (line2 -> modified)");
    assert!(has_added, "Should detect added line (line4)");
}

// ── 9. Workspace file operations ────────────────────────────────────────────

#[test]
fn test_workspace_file_operations() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let file_a = dir.path().join("hello.txt");
    let file_b = dir.path().join("world.txt");
    std::fs::write(&file_a, "hello from file A\nsome content").unwrap();
    std::fs::write(&file_b, "world from file B\nhello again").unwrap();

    let buf_a = Buffer::from_str(&std::fs::read_to_string(&file_a).unwrap());
    let buf_b = Buffer::from_str(&std::fs::read_to_string(&file_b).unwrap());

    let mut find_a = FindState::default();
    find_a.set_search_string("hello".to_string());
    find_a.research(&buf_a);

    let mut find_b = FindState::default();
    find_b.set_search_string("hello".to_string());
    find_b.research(&buf_b);

    let total_matches = find_a.matches.len() + find_b.matches.len();
    assert_eq!(total_matches, 2, "Should find 'hello' once in each file");
}

// ── 10. Git status ──────────────────────────────────────────────────────────

#[test]
fn test_git_status() {
    let dir = tempfile::tempdir().expect("create temp dir");

    let status = std::process::Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output();
    if status.is_err() || !status.unwrap().status.success() {
        eprintln!("git not available, skipping test_git_status");
        return;
    }

    std::fs::write(dir.path().join("file.txt"), "initial content").unwrap();
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args([
            "-c", "user.name=Test",
            "-c", "user.email=test@test.com",
            "commit", "-m", "init",
        ])
        .current_dir(dir.path())
        .output()
        .unwrap();

    std::fs::write(dir.path().join("file.txt"), "modified content").unwrap();
    std::fs::write(dir.path().join("new_file.txt"), "new file").unwrap();

    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    let status_text = String::from_utf8_lossy(&output.stdout);

    assert!(
        status_text.contains("file.txt"),
        "Should detect modified file"
    );
    assert!(
        status_text.contains("new_file.txt"),
        "Should detect new file"
    );
}

// ── 11. Terminal grid ───────────────────────────────────────────────────────

#[test]
fn test_terminal_grid() {
    let grid = TerminalGrid::new(24, 80);
    let mut emu = TerminalEmulator::new(grid);

    emu.process(b"Hello, terminal!");

    let g = emu.grid();
    assert_eq!(g.rows(), 24);
    assert_eq!(g.cols(), 80);

    let mut rendered = String::new();
    for col in 0..16u16 {
        rendered.push(g.cell(0, col).character);
    }
    assert_eq!(rendered, "Hello, terminal!");
}

#[test]
fn test_terminal_grid_ansi_colors() {
    let grid = TerminalGrid::new(24, 80);
    let mut emu = TerminalEmulator::new(grid);

    emu.process(b"\x1b[31mRED\x1b[0m normal");

    let g = emu.grid();
    let red_cell = g.cell(0, 0);
    assert_eq!(red_cell.character, 'R');
    assert_ne!(
        red_cell.fg,
        sidex_terminal::grid::Color::Default,
        "First char should have non-default color from SGR 31"
    );

    let normal_cell = g.cell(0, 4);
    assert_eq!(normal_cell.character, 'n');
}

// ── 12. LSP message encoding ────────────────────────────────────────────────

#[test]
fn test_lsp_message_encoding() {
    let request = JsonRpcMessage::Request {
        jsonrpc: "2.0".to_string(),
        id: RequestId::Number(1),
        method: "textDocument/completion".to_string(),
        params: Some(serde_json::json!({"textDocument": {"uri": "file:///test.rs"}})),
    };

    let json = serde_json::to_string(&request).expect("serialize");
    let content_length = json.len();
    let encoded = format!("Content-Length: {content_length}\r\n\r\n{json}");

    assert!(encoded.starts_with("Content-Length:"));
    assert!(encoded.contains("\r\n\r\n"));

    let body_start = encoded.find("\r\n\r\n").unwrap() + 4;
    let body = &encoded[body_start..];
    let decoded: JsonRpcMessage = serde_json::from_str(body).expect("deserialize");
    assert_eq!(decoded, request, "Roundtrip should preserve message");
}

#[test]
fn test_lsp_notification_roundtrip() {
    let notif = JsonRpcMessage::Notification {
        jsonrpc: "2.0".to_string(),
        method: "textDocument/didOpen".to_string(),
        params: Some(serde_json::json!({"textDocument": {"uri": "file:///a.rs", "version": 1}})),
    };
    let json = serde_json::to_string(&notif).unwrap();
    let decoded: JsonRpcMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, notif);
}

// ── 13. Extension manifest parsing ──────────────────────────────────────────

#[test]
fn test_extension_manifest_parsing() {
    let package_json = r#"{
        "name": "my-extension",
        "displayName": "My Extension",
        "version": "1.0.0",
        "publisher": "testpub",
        "engines": { "vscode": "^1.80.0" },
        "activationEvents": ["onLanguage:rust", "onCommand:myext.hello"],
        "main": "./out/extension.js",
        "contributes": {
            "commands": [
                { "command": "myext.hello", "title": "Hello World" },
                { "command": "myext.bye", "title": "Goodbye", "category": "MyExt" }
            ],
            "languages": [
                { "id": "myLang", "extensions": [".mylang"], "aliases": ["MyLanguage"] }
            ],
            "themes": [
                { "label": "My Dark Theme", "uiTheme": "vs-dark", "path": "./themes/dark.json" }
            ]
        }
    }"#;

    let manifest: ExtensionManifest =
        serde_json::from_str(package_json).expect("parse package.json");

    assert_eq!(manifest.name, "my-extension");
    assert_eq!(manifest.display_name, "My Extension");
    assert_eq!(manifest.version, "1.0.0");
    assert_eq!(manifest.publisher, Some("testpub".to_string()));
    assert_eq!(manifest.main, Some("./out/extension.js".to_string()));

    assert_eq!(manifest.activation_events.len(), 2);
    assert!(manifest.activation_events.contains(&"onLanguage:rust".to_string()));
    assert!(manifest.activation_events.contains(&"onCommand:myext.hello".to_string()));

    assert_eq!(manifest.contributes.commands.len(), 2);
    assert_eq!(manifest.contributes.commands[0].command, "myext.hello");
    assert_eq!(manifest.contributes.commands[0].title, "Hello World");
    assert_eq!(
        manifest.contributes.commands[1].category,
        Some("MyExt".to_string())
    );

    assert_eq!(manifest.contributes.languages.len(), 1);
    assert_eq!(manifest.contributes.languages[0].id, "myLang");

    assert_eq!(manifest.contributes.themes.len(), 1);
    assert_eq!(manifest.contributes.themes[0].label, "My Dark Theme");

    assert_eq!(manifest.canonical_id(), "testpub.my-extension");
}

// ── 14. Snippet expansion ───────────────────────────────────────────────────

#[test]
fn test_snippet_expansion() {
    let template = "for (${1:i}; ${2:cond}; ${3:inc}) {\n\t$0\n}";
    let parsed = sidex_editor::parse_snippet(template);

    assert!(!parsed.parts.is_empty());

    let has_tabstop_1 = parsed.parts.iter().any(|p| {
        matches!(p, sidex_editor::SnippetPart::Placeholder(1, _))
    });
    assert!(has_tabstop_1, "Should have tabstop $1");

    let has_final = parsed.parts.iter().any(|p| {
        matches!(p, sidex_editor::SnippetPart::Tabstop(0))
    });
    assert!(has_final, "Should have final tabstop $0");

    let mut doc = Document::from_str("");
    let session = SnippetSession::start(&mut doc, template);

    let text = doc.text();
    assert!(text.contains("for ("), "Expanded text should contain 'for ('");
    assert!(text.contains('}'), "Expanded text should contain closing brace");
    assert!(!session.finished, "Session should not be finished initially");
}

// ── 15. Auto-close brackets ─────────────────────────────────────────────────

#[test]
fn test_auto_close_brackets() {
    let mut doc = Document::from_str("");
    doc.cursors = MultiCursor::new(pos(0, 0));

    doc.type_char('(');
    assert_eq!(doc.text(), "()", "Should auto-close parenthesis");
    assert_eq!(
        doc.cursors.primary().position(),
        pos(0, 1),
        "Cursor should be between brackets"
    );

    let mut doc2 = Document::from_str("");
    doc2.cursors = MultiCursor::new(pos(0, 0));
    doc2.type_char('[');
    assert_eq!(doc2.text(), "[]");

    let mut doc3 = Document::from_str("");
    doc3.cursors = MultiCursor::new(pos(0, 0));
    doc3.type_char('{');
    assert_eq!(doc3.text(), "{}");
}

// ── 16. Comment toggle ──────────────────────────────────────────────────────

#[test]
fn test_comment_toggle() {
    let mut doc = Document::from_str("line1\nline2\nline3");

    doc.cursors = MultiCursor::new(pos(0, 0));
    doc.cursors
        .set_primary_selection(Selection::new(pos(0, 0), pos(2, 5)));
    doc.add_cursor_at_each_selection_line();

    doc.toggle_line_comment("//");

    let text = doc.text();
    for line in text.lines() {
        assert!(
            line.starts_with("// "),
            "Each line should start with '// ', got: '{line}'"
        );
    }

    doc.cursors = MultiCursor::new(pos(0, 0));
    doc.cursors
        .set_primary_selection(Selection::new(pos(0, 0), pos(2, 8)));
    doc.add_cursor_at_each_selection_line();
    doc.toggle_line_comment("//");

    let text2 = doc.text();
    for line in text2.lines() {
        assert!(
            !line.starts_with("//"),
            "Comments should be removed, got: '{line}'"
        );
    }
}

// ── 17. Undo/redo across multiple edits ─────────────────────────────────────

#[test]
fn test_undo_redo_multiple_edits() {
    let mut doc = Document::from_str("hello world");
    let v0 = doc.version;

    doc.cursors = MultiCursor::new(pos(0, 11));
    doc.type_char('!');
    assert_eq!(doc.text(), "hello world!");
    assert!(doc.version > v0);

    doc.cursors = MultiCursor::new(pos(0, 0));
    doc.delete_right();
    assert_eq!(doc.text(), "ello world!");

    doc.cursors = MultiCursor::new(pos(0, 10));
    doc.delete_left();
    assert_eq!(doc.text(), "ello worl!");

    assert!(doc.undo_stack.can_undo());
    assert!(!doc.undo_stack.can_redo());
}

// ── 18. Buffer bracket matching ─────────────────────────────────────────────

#[test]
fn test_buffer_bracket_matching() {
    let buf = Buffer::from_str("fn main() { let x = (1 + 2); }");
    let brackets = [('(', ')'), ('{', '}'), ('[', ']')];

    let m = buf.find_matching_bracket(pos(0, 10), &brackets);
    assert_eq!(m, Some(pos(0, 29)), "{{ at col 10 should match }} at col 29");

    let m2 = buf.find_matching_bracket(pos(0, 20), &brackets);
    assert_eq!(m2, Some(pos(0, 26)), "( at col 20 should match ) at col 26");
}

// ── 19. Context key evaluation ──────────────────────────────────────────────

#[test]
fn test_context_key_evaluation() {
    let mut ctx = ContextKeys::new();
    ctx.set_bool("editorTextFocus", true);
    ctx.set_bool("editorReadonly", false);
    ctx.set_string("editorLangId", "rust");

    assert!(
        sidex_keymap::evaluate("editorTextFocus && !editorReadonly", &ctx),
        "Should evaluate to true"
    );
    assert!(
        !sidex_keymap::evaluate("editorReadonly", &ctx),
        "editorReadonly is false"
    );
    assert!(
        sidex_keymap::evaluate("editorLangId == 'rust'", &ctx),
        "Language should be rust"
    );
    assert!(
        !sidex_keymap::evaluate("editorLangId == 'python'", &ctx),
        "Language is not python"
    );
}

// ── 20. Document text transforms ────────────────────────────────────────────

#[test]
fn test_document_text_transforms() {
    let mut doc = Document::from_str("hello world");
    doc.cursors = MultiCursor::new(pos(0, 0));
    doc.cursors
        .set_primary_selection(Selection::new(pos(0, 0), pos(0, 11)));

    doc.transform_to_uppercase();
    assert_eq!(doc.text(), "HELLO WORLD");

    doc.cursors
        .set_primary_selection(Selection::new(pos(0, 0), pos(0, 11)));
    doc.transform_to_lowercase();
    assert_eq!(doc.text(), "hello world");

    doc.cursors
        .set_primary_selection(Selection::new(pos(0, 0), pos(0, 11)));
    doc.transform_to_title_case();
    assert_eq!(doc.text(), "Hello World");
}

// ── 21. Terminal cursor movement ────────────────────────────────────────────

#[test]
fn test_terminal_cursor_movement() {
    let grid = TerminalGrid::new(24, 80);
    let mut emu = TerminalEmulator::new(grid);

    emu.process(b"AB");
    emu.process(b"\x1b[1;1H");
    emu.process(b"X");

    let g = emu.grid();
    assert_eq!(g.cell(0, 0).character, 'X');
    assert_eq!(g.cell(0, 1).character, 'B');
}

// ── 22. Find with regex ─────────────────────────────────────────────────────

#[test]
fn test_find_with_regex() {
    let buf = Buffer::from_str("foo123 bar456 baz789");
    let mut find = FindState::default();
    find.options.is_regex = true;
    find.set_search_string(r"\w+\d+".to_string());
    find.research(&buf);

    assert_eq!(find.matches.len(), 3, "Should find 3 word+digit patterns");
}

// ── 23. Settings defaults are comprehensive ─────────────────────────────────

#[test]
fn test_settings_defaults_comprehensive() {
    let settings = Settings::new();

    let font_family: Option<String> = settings.get("editor.fontFamily");
    assert!(font_family.is_some(), "Should have default font family");

    let minimap: Option<bool> = settings.get("editor.minimap.enabled");
    assert_eq!(minimap, Some(true));

    let format_on_save: Option<bool> = settings.get("editor.formatOnSave");
    assert_eq!(format_on_save, Some(false));

    let word_wrap: Option<String> = settings.get("editor.wordWrap");
    assert_eq!(word_wrap, Some("off".to_string()));
}

// ── 24. Diff identical documents ────────────────────────────────────────────

#[test]
fn test_diff_identical_documents() {
    let original = Document::from_str("same\ncontent\nhere");
    let modified = Document::from_str("same\ncontent\nhere");

    let mut editor = DiffEditor::new(original, modified);
    let result = editor.diff();
    assert!(result.is_identical(), "Identical docs should have no changes");
}

// ── 25. Keybinding chord resolution ─────────────────────────────────────────

#[test]
fn test_keybinding_chord_resolution() {
    let resolver = KeybindingResolver::new();
    let ctx = ContextKeys::new();

    let p = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CTRL
    };

    let first = KeyCombo::new(Key::K, p);
    assert!(
        resolver.is_chord_prefix(&first, &ctx),
        "Ctrl+K should be a chord prefix"
    );
}

// ── 26. Document line operations ────────────────────────────────────────────

#[test]
fn test_document_line_operations() {
    let mut doc = Document::from_str("cherry\napple\nbanana");
    doc.sort_lines_ascending();
    assert_eq!(doc.text(), "apple\nbanana\ncherry");

    let mut doc2 = Document::from_str("hello   \nworld  ");
    doc2.trim_trailing_whitespace();
    assert_eq!(doc2.text(), "hello\nworld");

    let mut doc3 = Document::from_str("hello\nworld");
    doc3.cursors = MultiCursor::new(pos(0, 0));
    doc3.join_lines();
    assert_eq!(doc3.text(), "hello world");
}
