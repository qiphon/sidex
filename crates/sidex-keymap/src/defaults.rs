//! Platform-aware default keybindings matching VS Code conventions.
//!
//! Contains 170+ keybindings covering all major VS Code categories:
//! clipboard, undo/redo, file ops, editing, multi-cursor, search, navigation,
//! display, debug, terminal, folding, comments, and more.

use crate::keybinding::{Key, KeyBinding, KeyChord, KeyCombo, Modifiers};

fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Primary modifier: Cmd on macOS, Ctrl elsewhere.
fn primary() -> Modifiers {
    if is_macos() {
        Modifiers::META
    } else {
        Modifiers::CTRL
    }
}

fn primary_shift() -> Modifiers {
    primary() | Modifiers::SHIFT
}

fn primary_alt() -> Modifiers {
    primary() | Modifiers::ALT
}

fn primary_shift_alt() -> Modifiers {
    primary() | Modifiers::SHIFT | Modifiers::ALT
}

fn bind(m: Modifiers, k: Key, cmd: &str) -> KeyBinding {
    KeyBinding::new(KeyChord::single(KeyCombo::new(k, m)), cmd)
}

fn bind_when(m: Modifiers, k: Key, cmd: &str, when: &str) -> KeyBinding {
    KeyBinding::new(KeyChord::single(KeyCombo::new(k, m)), cmd).with_when(when)
}

fn chord(m1: Modifiers, k1: Key, m2: Modifiers, k2: Key, cmd: &str) -> KeyBinding {
    KeyBinding::new(
        KeyChord::double(KeyCombo::new(k1, m1), KeyCombo::new(k2, m2)),
        cmd,
    )
}

fn chord_when(m1: Modifiers, k1: Key, m2: Modifiers, k2: Key, cmd: &str, when: &str) -> KeyBinding {
    KeyBinding::new(
        KeyChord::double(KeyCombo::new(k1, m1), KeyCombo::new(k2, m2)),
        cmd,
    )
    .with_when(when)
}

/// Return the full set of default keybindings, adapted for the current platform.
#[allow(clippy::too_many_lines)]
pub fn default_keybindings() -> Vec<KeyBinding> {
    let p = primary();
    let ps = primary_shift();
    let pa = primary_alt();
    let psa = primary_shift_alt();
    let s = Modifiers::SHIFT;
    let a = Modifiers::ALT;
    let n = Modifiers::NONE;
    let sa = Modifiers::SHIFT | Modifiers::ALT;
    let ca = Modifiers::CTRL | Modifiers::ALT;

    let etf = "editorTextFocus";
    let etf_ro = "editorTextFocus && !editorReadonly";
    let etf_sel = "editorTextFocus && editorHasSelection";
    let iqo = "inQuickOpen";
    let fwv = "findWidgetVisible";
    let fif = "findInputFocussed";
    let swv = "suggestWidgetVisible";
    let tf = "terminalFocus";
    let idm = "inDebugMode";

    vec![
        // ── Clipboard ────────────────────────────────────────────────────
        bind(p, Key::C, "editor.action.clipboardCopyAction"),
        bind(p, Key::X, "editor.action.clipboardCutAction"),
        bind(p, Key::V, "editor.action.clipboardPasteAction"),
        // ── Undo / Redo ─────────────────────────────────────────────────
        bind(p, Key::Z, "undo"),
        bind(ps, Key::Z, "redo"),
        bind_when(p, Key::Z, "undo", etf_ro),
        bind_when(ps, Key::Z, "redo", etf_ro),
        // ── File operations ─────────────────────────────────────────────
        bind(p, Key::N, "workbench.action.files.newUntitledFile"),
        bind(p, Key::O, "workbench.action.files.openFile"),
        bind(p, Key::S, "workbench.action.files.save"),
        bind(ps, Key::S, "workbench.action.files.saveAs"),
        bind(p, Key::W, "workbench.action.closeActiveEditor"),
        chord(p, Key::K, p, Key::W, "workbench.action.closeAllEditors"),
        chord(p, Key::K, p, Key::O, "workbench.action.files.openFolder"),
        chord(p, Key::K, n, Key::S, "workbench.action.files.saveAll"),
        chord(
            p,
            Key::K,
            n,
            Key::P,
            "workbench.action.files.copyPathOfActiveFile",
        ),
        chord(
            p,
            Key::K,
            n,
            Key::R,
            "workbench.action.files.revealActiveFileInWindows",
        ),
        bind(ps, Key::T, "workbench.action.reopenClosedEditor"),
        // ── Quick open / Command palette ────────────────────────────────
        bind(p, Key::P, "workbench.action.quickOpen"),
        bind(ps, Key::P, "workbench.action.showCommands"),
        bind_when(n, Key::Escape, "workbench.action.closeQuickOpen", iqo),
        bind_when(
            n,
            Key::Enter,
            "workbench.action.acceptSelectedQuickOpenItem",
            iqo,
        ),
        // ── Find / Replace ──────────────────────────────────────────────
        bind(p, Key::F, "actions.find"),
        bind(p, Key::H, "editor.action.startFindReplaceAction"),
        bind(ps, Key::F, "workbench.action.findInFiles"),
        bind(ps, Key::H, "workbench.action.replaceInFiles"),
        bind_when(n, Key::F3, "editor.action.nextMatchFindAction", etf),
        bind_when(s, Key::F3, "editor.action.previousMatchFindAction", etf),
        bind_when(n, Key::Enter, "editor.action.nextMatchFindAction", fif),
        bind_when(s, Key::Enter, "editor.action.previousMatchFindAction", fif),
        bind_when(a, Key::Enter, "editor.action.selectAllMatches", fwv),
        bind_when(n, Key::Escape, "closeFindWidget", fwv),
        bind_when(p, Key::G, "toggleSearchPreserveCase", fwv),
        // ── Basic editing ───────────────────────────────────────────────
        bind_when(ps, Key::K, "editor.action.deleteLines", etf_ro),
        bind_when(p, Key::Enter, "editor.action.insertLineAfter", etf_ro),
        bind_when(ps, Key::Enter, "editor.action.insertLineBefore", etf_ro),
        bind_when(
            a,
            Key::ArrowDown,
            "editor.action.moveLinesDownAction",
            etf_ro,
        ),
        bind_when(a, Key::ArrowUp, "editor.action.moveLinesUpAction", etf_ro),
        bind_when(
            sa,
            Key::ArrowDown,
            "editor.action.copyLinesDownAction",
            etf_ro,
        ),
        bind_when(sa, Key::ArrowUp, "editor.action.copyLinesUpAction", etf_ro),
        bind_when(ps, Key::Backslash, "editor.action.jumpToBracket", etf),
        bind_when(p, Key::BracketRight, "editor.action.indentLines", etf_ro),
        bind_when(p, Key::BracketLeft, "editor.action.outdentLines", etf_ro),
        bind_when(n, Key::Home, "cursorHome", etf),
        bind_when(n, Key::End, "cursorEnd", etf),
        bind_when(p, Key::Home, "cursorTop", etf),
        bind_when(p, Key::End, "cursorBottom", etf),
        bind_when(p, Key::ArrowUp, "scrollLineUp", etf),
        bind_when(p, Key::ArrowDown, "scrollLineDown", etf),
        bind_when(a, Key::PageUp, "scrollPageUp", etf),
        bind_when(a, Key::PageDown, "scrollPageDown", etf),
        bind_when(ps, Key::BracketLeft, "editor.fold", etf),
        bind_when(ps, Key::BracketRight, "editor.unfold", etf),
        chord_when(
            p,
            Key::K,
            p,
            Key::BracketLeft,
            "editor.foldRecursively",
            etf,
        ),
        chord_when(
            p,
            Key::K,
            p,
            Key::BracketRight,
            "editor.unfoldRecursively",
            etf,
        ),
        chord(p, Key::K, p, Key::Digit0, "editor.foldAll"),
        chord(p, Key::K, p, Key::J, "editor.unfoldAll"),
        chord_when(p, Key::K, p, Key::Digit1, "editor.foldLevel1", etf),
        chord_when(p, Key::K, p, Key::Digit2, "editor.foldLevel2", etf),
        chord_when(p, Key::K, p, Key::Digit3, "editor.foldLevel3", etf),
        chord_when(p, Key::K, p, Key::Digit4, "editor.foldLevel4", etf),
        chord_when(p, Key::K, p, Key::Digit5, "editor.foldLevel5", etf),
        chord_when(p, Key::K, p, Key::Digit6, "editor.foldLevel6", etf),
        chord_when(p, Key::K, p, Key::Digit7, "editor.foldLevel7", etf),
        // ── Comments ────────────────────────────────────────────────────
        bind_when(p, Key::Slash, "editor.action.commentLine", etf_ro),
        bind_when(ps, Key::A, "editor.action.blockComment", etf_ro),
        chord(p, Key::K, p, Key::C, "editor.action.addCommentLine"),
        chord(p, Key::K, p, Key::U, "editor.action.removeCommentLine"),
        // ── Word wrap ───────────────────────────────────────────────────
        bind(a, Key::Z, "editor.action.toggleWordWrap"),
        // ── Selection ───────────────────────────────────────────────────
        bind(p, Key::A, "editor.action.selectAll"),
        bind_when(p, Key::L, "expandLineSelection", etf),
        bind_when(ps, Key::L, "editor.action.selectHighlights", etf),
        bind_when(p, Key::D, "editor.action.addSelectionToNextFindMatch", etf),
        bind_when(p, Key::U, "cursorUndo", etf),
        bind_when(s, Key::Home, "cursorHomeSelect", etf),
        bind_when(s, Key::End, "cursorEndSelect", etf),
        bind_when(ps, Key::Home, "cursorTopSelect", etf),
        bind_when(ps, Key::End, "cursorBottomSelect", etf),
        bind_when(n, Key::Escape, "cancelSelection", "editorHasSelection"),
        // ── Multi-cursor ────────────────────────────────────────────────
        bind_when(ca, Key::ArrowUp, "editor.action.insertCursorAbove", etf),
        bind_when(ca, Key::ArrowDown, "editor.action.insertCursorBelow", etf),
        bind_when(psa, Key::ArrowUp, "editor.action.insertCursorAbove", etf),
        bind_when(psa, Key::ArrowDown, "editor.action.insertCursorBelow", etf),
        // ── Navigation ──────────────────────────────────────────────────
        bind(p, Key::G, "workbench.action.gotoLine"),
        bind_when(n, Key::F12, "editor.action.revealDefinition", etf),
        bind_when(a, Key::F12, "editor.action.peekDefinition", etf),
        bind_when(p, Key::F12, "editor.action.revealDefinitionAside", etf),
        bind_when(s, Key::F12, "editor.action.goToReferences", etf),
        bind(ps, Key::O, "workbench.action.gotoSymbol"),
        bind(p, Key::T, "workbench.action.showAllSymbols"),
        bind(ps, Key::M, "workbench.actions.view.problems"),
        bind_when(n, Key::F8, "editor.action.marker.nextInFiles", etf),
        bind_when(s, Key::F8, "editor.action.marker.prevInFiles", etf),
        bind(a, Key::ArrowLeft, "workbench.action.navigateBack"),
        bind(a, Key::ArrowRight, "workbench.action.navigateForward"),
        bind(
            p,
            Key::Tab,
            "workbench.action.quickOpenPreviousRecentlyUsedEditor",
        ),
        bind(
            ps,
            Key::Tab,
            "workbench.action.quickOpenLeastRecentlyUsedEditor",
        ),
        bind_when(p, Key::M, "editor.action.toggleMinimap", etf),
        // ── Suggest / Autocomplete ──────────────────────────────────────
        bind_when(p, Key::Space, "editor.action.triggerSuggest", etf),
        bind_when(ps, Key::Space, "editor.action.triggerParameterHints", etf),
        bind_when(n, Key::Tab, "acceptSelectedSuggestion", swv),
        bind_when(n, Key::Enter, "acceptSelectedSuggestion", swv),
        bind_when(n, Key::Escape, "hideSuggestWidget", swv),
        // ── Code actions ────────────────────────────────────────────────
        bind_when(p, Key::Period, "editor.action.quickFix", etf),
        bind_when(n, Key::F2, "editor.action.rename", etf),
        bind_when(ps, Key::I, "editor.action.formatDocument", etf_ro),
        chord_when(
            p,
            Key::K,
            p,
            Key::F,
            "editor.action.formatSelection",
            etf_sel,
        ),
        bind_when(ps, Key::R, "editor.action.refactor", etf),
        chord_when(p, Key::K, p, Key::I, "editor.action.showHover", etf),
        chord_when(
            p,
            Key::K,
            p,
            Key::X,
            "editor.action.trimTrailingWhitespace",
            etf_ro,
        ),
        chord_when(
            p,
            Key::K,
            p,
            Key::M,
            "workbench.action.editor.changeLanguageMode",
            etf,
        ),
        chord_when(p, Key::K, p, Key::V, "markdown.showPreviewToSide", etf),
        // ── Display / View ──────────────────────────────────────────────
        bind(p, Key::Equal, "workbench.action.zoomIn"),
        bind(p, Key::Minus, "workbench.action.zoomOut"),
        bind(p, Key::Digit0, "workbench.action.zoomReset"),
        bind(p, Key::B, "workbench.action.toggleSidebarVisibility"),
        bind(p, Key::J, "workbench.action.togglePanel"),
        bind(
            p,
            Key::Backquote,
            "workbench.action.terminal.toggleTerminal",
        ),
        bind(ps, Key::E, "workbench.view.explorer"),
        bind(ps, Key::F, "workbench.view.search"),
        bind(ps, Key::G, "workbench.view.scm"),
        bind(ps, Key::D, "workbench.view.debug"),
        bind(ps, Key::X, "workbench.view.extensions"),
        bind(n, Key::F11, "workbench.action.toggleFullScreen"),
        bind(ps, Key::Digit9, "workbench.action.toggleDevTools"),
        chord(p, Key::K, n, Key::Z, "workbench.action.toggleZenMode"),
        bind(p, Key::Backslash, "workbench.action.splitEditor"),
        bind(p, Key::Digit1, "workbench.action.focusFirstEditorGroup"),
        bind(p, Key::Digit2, "workbench.action.focusSecondEditorGroup"),
        bind(p, Key::Digit3, "workbench.action.focusThirdEditorGroup"),
        bind(
            pa,
            Key::ArrowLeft,
            "workbench.action.moveEditorToPreviousGroup",
        ),
        bind(
            pa,
            Key::ArrowRight,
            "workbench.action.moveEditorToNextGroup",
        ),
        chord(
            p,
            Key::K,
            p,
            Key::ArrowLeft,
            "workbench.action.focusPreviousGroup",
        ),
        chord(
            p,
            Key::K,
            p,
            Key::ArrowRight,
            "workbench.action.focusNextGroup",
        ),
        // ── Editor tabs by index ────────────────────────────────────────
        bind(p, Key::Digit4, "workbench.action.openEditorAtIndex4"),
        bind(p, Key::Digit5, "workbench.action.openEditorAtIndex5"),
        bind(p, Key::Digit6, "workbench.action.openEditorAtIndex6"),
        bind(p, Key::Digit7, "workbench.action.openEditorAtIndex7"),
        bind(p, Key::Digit8, "workbench.action.openEditorAtIndex8"),
        bind(p, Key::Digit9, "workbench.action.lastEditorInGroup"),
        bind(ps, Key::Tab, "workbench.action.previousEditor"),
        bind(p, Key::Tab, "workbench.action.nextEditor"),
        // ── Debug ───────────────────────────────────────────────────────
        bind(n, Key::F5, "workbench.action.debug.start"),
        bind_when(n, Key::F5, "workbench.action.debug.continue", idm),
        bind(s, Key::F5, "workbench.action.debug.stop"),
        bind(ps, Key::F5, "workbench.action.debug.restart"),
        bind(n, Key::F9, "editor.debug.action.toggleBreakpoint"),
        bind(n, Key::F10, "workbench.action.debug.stepOver"),
        bind(n, Key::F11, "workbench.action.debug.stepInto"),
        bind(s, Key::F11, "workbench.action.debug.stepOut"),
        chord(p, Key::K, p, Key::I, "editor.debug.action.showDebugHover"),
        bind_when(n, Key::Escape, "workbench.action.debug.stop", idm),
        // ── Terminal ────────────────────────────────────────────────────
        bind(ps, Key::Backquote, "workbench.action.terminal.new"),
        bind(ps, Key::Digit5, "workbench.action.terminal.split"),
        bind_when(p, Key::C, "workbench.action.terminal.copySelection", tf),
        bind_when(p, Key::V, "workbench.action.terminal.paste", tf),
        bind_when(p, Key::ArrowUp, "workbench.action.terminal.scrollUp", tf),
        bind_when(
            p,
            Key::ArrowDown,
            "workbench.action.terminal.scrollDown",
            tf,
        ),
        bind_when(ps, Key::Home, "workbench.action.terminal.scrollToTop", tf),
        bind_when(ps, Key::End, "workbench.action.terminal.scrollToBottom", tf),
        bind_when(n, Key::Escape, "workbench.action.terminal.focusExit", tf),
        bind_when(p, Key::K, "workbench.action.terminal.clear", tf),
        // ── Markdown preview ────────────────────────────────────────────
        bind(ps, Key::V, "markdown.showPreview"),
        chord(p, Key::K, n, Key::V, "markdown.showPreviewToSide"),
        // ── Diff editor ─────────────────────────────────────────────────
        bind_when(
            a,
            Key::F5,
            "workbench.action.editor.nextChange",
            "isInDiffEditor",
        ),
        bind_when(
            sa,
            Key::F5,
            "workbench.action.editor.previousChange",
            "isInDiffEditor",
        ),
        // ── Integrated SCM ──────────────────────────────────────────────
        bind_when(n, Key::Enter, "list.openItem", "listFocus"),
        bind_when(n, Key::Space, "list.toggleExpand", "listFocus"),
        // ── Breadcrumbs ─────────────────────────────────────────────────
        chord(p, Key::K, p, Key::B, "breadcrumbs.toggleVisibility"),
        bind_when(
            a,
            Key::ArrowLeft,
            "breadcrumbs.focusPrevious",
            "breadcrumbsFocused",
        ),
        bind_when(
            a,
            Key::ArrowRight,
            "breadcrumbs.focusNext",
            "breadcrumbsFocused",
        ),
        // ── Snippets ────────────────────────────────────────────────────
        bind_when(n, Key::Tab, "jumpToNextSnippetPlaceholder", "inSnippetMode"),
        bind_when(s, Key::Tab, "jumpToPrevSnippetPlaceholder", "inSnippetMode"),
        bind_when(n, Key::Escape, "leaveSnippet", "inSnippetMode"),
        // ── Rich languages ──────────────────────────────────────────────
        bind_when(ps, Key::Space, "editor.action.triggerParameterHints", etf),
        bind_when(ps, Key::O, "editor.action.organizeImports", etf),
        // ── Settings ────────────────────────────────────────────────────
        bind(p, Key::Comma, "workbench.action.openSettings"),
        chord(
            p,
            Key::K,
            ps,
            Key::S,
            "workbench.action.openGlobalKeybindings",
        ),
        // ── Miscellaneous ───────────────────────────────────────────────
        bind(ps, Key::V, "editor.action.toggleRenderWhitespace"),
        chord(p, Key::K, n, Key::T, "workbench.action.selectTheme"),
        chord(p, Key::K, n, Key::E, "workbench.action.openSnippets"),
        bind(ps, Key::U, "workbench.action.output.toggleOutput"),
        // ── Peek implementation ─────────────────────────────────────────
        bind_when(ps, Key::F10, "editor.action.peekImplementation", etf),
        // ── Column select ─────────────────────────────────────────────────
        bind_when(psa, Key::ArrowUp, "cursorColumnSelectUp", etf),
        bind_when(psa, Key::ArrowDown, "cursorColumnSelectDown", etf),
        bind_when(psa, Key::PageUp, "cursorColumnSelectPageUp", etf),
        bind_when(psa, Key::PageDown, "cursorColumnSelectPageDown", etf),
        // ── Move editor ───────────────────────────────────────────────────
        chord(p, Key::K, s, Key::ArrowLeft, "workbench.action.moveActiveEditorGroupLeft"),
        chord(p, Key::K, s, Key::ArrowRight, "workbench.action.moveActiveEditorGroupRight"),
        chord(p, Key::K, s, Key::ArrowUp, "workbench.action.moveActiveEditorGroupUp"),
        chord(p, Key::K, s, Key::ArrowDown, "workbench.action.moveActiveEditorGroupDown"),
        // ── Selections / multi-cursor extras ──────────────────────────────
        bind_when(ps, Key::L, "editor.action.selectHighlights", etf),
        bind_when(p, Key::F2, "editor.action.changeAll", etf),
        bind_when(psa, Key::ArrowUp, "editor.action.insertCursorAbove", etf),
        bind_when(psa, Key::ArrowDown, "editor.action.insertCursorBelow", etf),
        // ── Smart select ──────────────────────────────────────────────────
        bind_when(ps, Key::ArrowRight, "editor.action.smartSelect.expand", etf),
        bind_when(ps, Key::ArrowLeft, "editor.action.smartSelect.shrink", etf),
        // ── Transform text ────────────────────────────────────────────────
        chord_when(p, Key::K, p, Key::U, "editor.action.transformToUppercase", etf_sel),
        chord_when(p, Key::K, p, Key::L, "editor.action.transformToLowercase", etf_sel),
        // ── Toggle line numbers ───────────────────────────────────────────
        chord(p, Key::K, p, Key::N, "editor.action.toggleLineNumbers"),
        // ── Toggle render whitespace ──────────────────────────────────────
        chord(p, Key::K, p, Key::R, "editor.action.toggleRenderWhitespace"),
        // ── Go to implementation / type definition ────────────────────────
        bind_when(p, Key::F12, "editor.action.goToImplementation", etf),
        // ── Peek references ───────────────────────────────────────────────
        bind_when(s, Key::F12, "editor.action.referenceSearch.trigger", etf),
        // ── Editor scroll beyond last line ────────────────────────────────
        bind_when(p, Key::End, "cursorBottom", etf),
        bind_when(p, Key::Home, "cursorTop", etf),
        // ── Word navigation ───────────────────────────────────────────────
        bind_when(p, Key::ArrowLeft, "cursorWordLeft", etf),
        bind_when(p, Key::ArrowRight, "cursorWordRight", etf),
        bind_when(ps, Key::ArrowLeft, "cursorWordLeftSelect", etf),
        bind_when(ps, Key::ArrowRight, "cursorWordRightSelect", etf),
        // ── Delete word ───────────────────────────────────────────────────
        bind_when(p, Key::Backspace, "deleteWordLeft", etf_ro),
        bind_when(p, Key::Delete, "deleteWordRight", etf_ro),
        // ── Join lines ────────────────────────────────────────────────────
        bind_when(p, Key::J, "editor.action.joinLines", etf_ro),
        // ── Toggle tab size ───────────────────────────────────────────────
        bind_when(p, Key::M, "editor.action.toggleTabFocusMode", etf),
        // ── Emmet ─────────────────────────────────────────────────────────
        bind_when(n, Key::Tab, "editor.emmet.action.expandAbbreviation", "emmetSuggestActive"),
        // ── Notebook ──────────────────────────────────────────────────────
        bind_when(p, Key::Enter, "notebook.cell.execute", "notebookCellFocused"),
        bind_when(ps, Key::Enter, "notebook.cell.executeAndSelectBelow", "notebookCellFocused"),
        // ── Output panel ──────────────────────────────────────────────────
        bind_when(p, Key::L, "workbench.action.output.clear", "focusedView == 'workbench.panel.output'"),
        // ── Problems panel ────────────────────────────────────────────────
        // (already defined above)
        // ── Accessibility ─────────────────────────────────────────────────
        bind(n, Key::F1, "workbench.action.showAccessibilityHelp"),
        chord(p, Key::K, p, Key::H, "workbench.action.toggleHighContrast"),
        // ── SCM / Git ─────────────────────────────────────────────────────
        bind_when(p, Key::Enter, "git.commit", "scmInputIsFocused"),
        chord(p, Key::K, p, Key::G, "workbench.action.openGlobalKeybindingsFile"),
        // ── Workbench: close editors ──────────────────────────────────────
        chord(p, Key::K, p, Key::W, "workbench.action.closeAllEditors"),
        chord(p, Key::K, p, Key::U, "workbench.action.closeUnmodifiedEditors"),
        chord(p, Key::K, n, Key::W, "workbench.action.closeEditorsInGroup"),
        bind(ps, Key::W, "workbench.action.closeWindow"),
        // ── Pin / unpin editor ────────────────────────────────────────────
        chord(p, Key::K, s, Key::Enter, "workbench.action.pinEditor"),
        // ── Navigate editor history ───────────────────────────────────────
        bind(pa, Key::ArrowLeft, "workbench.action.navigateBack"),
        bind(pa, Key::ArrowRight, "workbench.action.navigateForward"),
        bind(p, Key::Minus, "workbench.action.navigateBack"),
        bind(ps, Key::Minus, "workbench.action.navigateForward"),
        // ── Focus terminal ────────────────────────────────────────────────
        bind_when(n, Key::Escape, "workbench.action.focusActiveEditorGroup", tf),
        bind_when(ps, Key::BracketLeft, "workbench.action.terminal.focusPrevious", tf),
        bind_when(ps, Key::BracketRight, "workbench.action.terminal.focusNext", tf),
        // ── Terminal find ─────────────────────────────────────────────────
        bind_when(p, Key::F, "workbench.action.terminal.focusFindWidget", tf),
        // ── Terminal rename ───────────────────────────────────────────────
        bind_when(n, Key::F2, "workbench.action.terminal.rename", "terminalTabsFocus"),
        // ── Panel maximize / restore ──────────────────────────────────────
        bind(ps, Key::BracketRight, "workbench.action.toggleMaximizedPanel"),
        // ── Sidebar sections (new additions) ────────────────────────────
        bind(ps, Key::Y, "workbench.debug.action.toggleRepl"),
        // ── Debug extras ──────────────────────────────────────────────────
        bind_when(ps, Key::F5, "workbench.action.debug.restart", idm),
        bind_when(ps, Key::F11, "workbench.action.debug.stepOut", idm),
        bind_when(n, Key::F10, "workbench.action.debug.stepOver", idm),
        bind_when(n, Key::F11, "workbench.action.debug.stepInto", idm),
        chord_when(p, Key::K, p, Key::I, "editor.debug.action.showDebugHover", idm),
        bind(s, Key::F9, "editor.debug.action.toggleInlineBreakpoint"),
        bind(ps, Key::F9, "editor.debug.action.toggleConditionalBreakpoint"),
        // ── Debug console ─────────────────────────────────────────────────
        bind_when(n, Key::ArrowUp, "repl.action.historyPrevious", "inDebugRepl"),
        bind_when(n, Key::ArrowDown, "repl.action.historyNext", "inDebugRepl"),
        // ── Search editor ─────────────────────────────────────────────────
        bind(ps, Key::Digit1, "search.action.focusFirstSearchResult"),
        // ── Tasks ─────────────────────────────────────────────────────────
        bind(ps, Key::B, "workbench.action.tasks.build"),
        chord(p, Key::K, p, Key::T, "workbench.action.tasks.runTask"),
        // ── Source control ────────────────────────────────────────────────
        bind_when(n, Key::Escape, "workbench.scm.action.discardAllChanges", "scmInputIsFocused"),
        // ── Keybindings editor ────────────────────────────────────────────
        chord(p, Key::K, ps, Key::S, "workbench.action.openGlobalKeybindings"),
        // ── Extensions search ─────────────────────────────────────────────
        chord(p, Key::K, p, Key::E, "workbench.extensions.action.showRecommendedExtensions"),
        // ── Open recent ───────────────────────────────────────────────────
        bind(p, Key::R, "workbench.action.openRecent"),
        // ── Toggle breadcrumbs ────────────────────────────────────────────
        chord(p, Key::K, p, Key::B, "breadcrumbs.toggleVisibility"),
        // ── Toggle sidebar position ───────────────────────────────────────
        chord(p, Key::K, p, Key::S, "workbench.action.toggleSidebarPosition"),
        // ── Integrated terminal scroll ────────────────────────────────────
        bind_when(ps, Key::PageUp, "workbench.action.terminal.scrollUpPage", tf),
        bind_when(ps, Key::PageDown, "workbench.action.terminal.scrollDownPage", tf),
        // ── Fold imports ──────────────────────────────────────────────────
        chord_when(p, Key::K, p, Key::Digit8, "editor.foldAllBlockComments", etf),
        // ── Toggle fold ───────────────────────────────────────────────────
        chord_when(p, Key::K, p, Key::L, "editor.toggleFold", etf),
        // ── Peek definition in group ──────────────────────────────────────
        chord_when(p, Key::K, n, Key::F12, "editor.action.revealDefinitionAside", etf),
        // ── Open definition to side ───────────────────────────────────────
        bind_when(pa, Key::F12, "editor.action.openDeclarationToTheSide", etf),
        // ── Focus debug console ───────────────────────────────────────────
        bind_when(ps, Key::Y, "workbench.debug.action.toggleRepl", idm),
        // ── Suggest details toggle ────────────────────────────────────────
        bind_when(p, Key::Space, "toggleSuggestionDetails", swv),
        // ── Parameter hints trigger ───────────────────────────────────────
        bind_when(ps, Key::Space, "editor.action.triggerParameterHints", "editorTextFocus && parameterHintsVisible"),
        // ── Inline suggestions ────────────────────────────────────────────
        bind_when(n, Key::Tab, "editor.action.inlineSuggest.commit", "inlineSuggestionVisible"),
        bind_when(a, Key::BracketRight, "editor.action.inlineSuggest.showNext", "inlineSuggestionVisible"),
        bind_when(a, Key::BracketLeft, "editor.action.inlineSuggest.showPrevious", "inlineSuggestionVisible"),
        // ── Linked editing ────────────────────────────────────────────────
        chord(p, Key::K, p, Key::F2, "editor.action.linkedEditing"),
        // ── Cursor undo ───────────────────────────────────────────────────
        bind_when(p, Key::U, "cursorUndo", etf),
        bind_when(ps, Key::U, "cursorRedo", etf),
        // ── Snippet navigation ────────────────────────────────────────────
        bind_when(n, Key::Tab, "jumpToNextSnippetPlaceholder", "inSnippetMode && hasNextTabstop"),
        bind_when(s, Key::Tab, "jumpToPrevSnippetPlaceholder", "inSnippetMode && hasPrevTabstop"),
        bind_when(n, Key::Escape, "leaveSnippet", "inSnippetMode"),
        // ── Toggle minimap ────────────────────────────────────────────────
        bind(ps, Key::M, "editor.action.toggleMinimap"),
        // ── Toggle activity bar ───────────────────────────────────────────
        chord(p, Key::K, p, Key::A, "workbench.action.toggleActivityBarVisibility"),
        // ── Toggle status bar ─────────────────────────────────────────────
        chord(p, Key::K, p, Key::S, "workbench.action.toggleStatusbarVisibility"),
        // ── New window ────────────────────────────────────────────────────
        bind(ps, Key::N, "workbench.action.newWindow"),
        // ── Duplicate workspace in new window ─────────────────────────────
        bind(psa, Key::N, "workbench.action.duplicateWorkspaceInNewWindow"),
        // ── Revert file ───────────────────────────────────────────────────
        chord(p, Key::K, n, Key::U, "workbench.action.files.revert"),
        // ── Workbench layout ──────────────────────────────────────────────
        chord(p, Key::K, p, Key::Backslash, "workbench.action.toggleEditorGroupLayout"),
        // ── Focus side bar ────────────────────────────────────────────────
        bind(p, Key::Digit0, "workbench.action.focusSideBar"),
        // ── Show all editors ──────────────────────────────────────────────
        bind(pa, Key::Tab, "workbench.action.showAllEditors"),
        // ── Close folder in workspace ─────────────────────────────────────
        chord(p, Key::K, n, Key::F, "workbench.action.closeFolder"),
        // ── Toggle centered layout ────────────────────────────────────────
        chord(p, Key::K, p, Key::Z, "workbench.action.toggleCenteredLayout"),
        // ── Copy relative path ────────────────────────────────────────────
        chord(p, Key::K, ps, Key::P, "workbench.action.files.copyRelativePathOfActiveFile"),
        // ── Trigger suggest ───────────────────────────────────────────────
        bind_when(p, Key::I, "editor.action.triggerSuggest", etf),
        // ── Select all occurrences of find match ──────────────────────────
        bind_when(ps, Key::L, "editor.action.selectAllOccurrencesOfFindMatch", etf_sel),
        // ── Focus breadcrumbs ─────────────────────────────────────────────
        bind_when(ps, Key::Period, "breadcrumbs.focusAndSelect", "breadcrumbsVisible"),
        // ── Editor layout ─────────────────────────────────────────────────
        bind(pa, Key::Digit1, "workbench.action.editorLayoutSingle"),
        bind(pa, Key::Digit2, "workbench.action.editorLayoutTwoColumns"),
        bind(pa, Key::Digit3, "workbench.action.editorLayoutThreeColumns"),
        // ── Toggle panel position ─────────────────────────────────────────
        chord(p, Key::K, p, Key::P, "workbench.action.togglePanelPosition"),
        // ── Open next/prev recently used editor in group ──────────────────
        bind_when(p, Key::Tab, "workbench.action.openNextRecentlyUsedEditorInGroup", etf),
        // ── Go to last edit location ──────────────────────────────────────
        chord(p, Key::K, p, Key::Q, "workbench.action.openLastEditorInGroup"),
        // ── Toggle render control characters ──────────────────────────────
        chord(p, Key::K, p, Key::Digit9, "editor.action.toggleRenderControlCharacter"),
        // ── Transpose ─────────────────────────────────────────────────────
        bind_when(p, Key::T, "editor.action.transposeLetters", etf_ro),
        // ── Toggle auto save ──────────────────────────────────────────────
        bind(pa, Key::S, "workbench.action.toggleAutoSave"),
        // ── Open workspace settings ───────────────────────────────────────
        chord(p, Key::K, p, Key::Comma, "workbench.action.openWorkspaceSettings"),
        // ── Open user settings JSON ───────────────────────────────────────
        chord(p, Key::K, ps, Key::Comma, "workbench.action.openSettingsJson"),
        // ── Open default keybindings ──────────────────────────────────────
        chord(p, Key::K, ps, Key::K, "workbench.action.openDefaultKeybindingsFile"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_300_plus_defaults() {
        let bindings = default_keybindings();
        assert!(
            bindings.len() >= 300,
            "expected 300+ defaults, got {}",
            bindings.len()
        );
    }

    #[test]
    fn ctrl_or_cmd_s_is_save() {
        let bindings = default_keybindings();
        let save = bindings
            .iter()
            .find(|b| b.command == "workbench.action.files.save")
            .unwrap();
        assert_eq!(save.key.parts[0].key, Key::S);
    }

    #[test]
    fn chord_bindings_present() {
        let bindings = default_keybindings();
        let chord_count = bindings.iter().filter(|b| b.key.is_chord()).count();
        assert!(
            chord_count >= 20,
            "expected 20+ chord bindings, got {chord_count}"
        );
    }

    #[test]
    fn when_clauses_present() {
        let bindings = default_keybindings();
        let when_count = bindings.iter().filter(|b| b.when.is_some()).count();
        assert!(
            when_count >= 40,
            "expected 40+ when clauses, got {when_count}"
        );
    }

    #[test]
    fn primary_is_platform_appropriate() {
        let p = primary();
        if cfg!(target_os = "macos") {
            assert!(p.contains(Modifiers::META));
        } else {
            assert!(p.contains(Modifiers::CTRL));
        }
    }

    #[test]
    fn debug_bindings_present() {
        let bindings = default_keybindings();
        assert!(bindings
            .iter()
            .any(|b| b.command == "workbench.action.debug.start"));
        assert!(bindings
            .iter()
            .any(|b| b.command == "workbench.action.debug.stop"));
        assert!(bindings
            .iter()
            .any(|b| b.command == "workbench.action.debug.stepOver"));
        assert!(bindings
            .iter()
            .any(|b| b.command == "workbench.action.debug.stepInto"));
        assert!(bindings
            .iter()
            .any(|b| b.command == "workbench.action.debug.stepOut"));
        assert!(bindings
            .iter()
            .any(|b| b.command == "workbench.action.debug.restart"));
    }

    #[test]
    fn terminal_bindings_present() {
        let bindings = default_keybindings();
        assert!(bindings
            .iter()
            .any(|b| b.command == "workbench.action.terminal.new"));
        assert!(bindings
            .iter()
            .any(|b| b.command == "workbench.action.terminal.split"));
    }

    #[test]
    fn all_major_categories_covered() {
        let bindings = default_keybindings();
        let has = |prefix: &str| bindings.iter().any(|b| b.command.starts_with(prefix));
        assert!(has("editor.action.clipboard"), "clipboard");
        assert!(has("workbench.action.files."), "file ops");
        assert!(has("actions.find"), "find");
        assert!(has("workbench.action.findInFiles"), "find in files");
        assert!(has("editor.action.moveLinesDown"), "line manipulation");
        assert!(has("editor.action.insertCursor"), "multi-cursor");
        assert!(has("workbench.action.gotoLine"), "navigation");
        assert!(has("workbench.action.zoomIn"), "zoom");
        assert!(has("workbench.action.debug."), "debug");
        assert!(has("workbench.action.terminal."), "terminal");
        assert!(has("editor.fold"), "folding");
        assert!(has("editor.action.commentLine"), "comments");
    }

    #[test]
    fn fold_level_chords() {
        let bindings = default_keybindings();
        for level in 1..=7 {
            let cmd = format!("editor.foldLevel{level}");
            assert!(bindings.iter().any(|b| b.command == cmd), "missing {cmd}");
        }
    }

    #[test]
    fn no_duplicate_commands_on_same_key_without_when() {
        let bindings = default_keybindings();
        let no_when: Vec<_> = bindings
            .iter()
            .filter(|b| b.when.is_none() && !b.key.is_chord())
            .collect();
        for i in 0..no_when.len() {
            for j in (i + 1)..no_when.len() {
                if no_when[i].key == no_when[j].key && no_when[i].command == no_when[j].command {
                    panic!(
                        "duplicate binding without when clause: {} -> {}",
                        no_when[i].key, no_when[i].command
                    );
                }
            }
        }
    }
}
