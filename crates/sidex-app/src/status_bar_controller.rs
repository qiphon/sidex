//! Status bar data binding — connects status bar items to live application
//! state such as git branch, diagnostics, cursor position, encoding, etc.

use sidex_lsp::DiagnosticManager;
use sidex_remote::RemoteManager;
use sidex_text::LineEnding;
use sidex_ui::workbench::status_bar::{StatusBar, StatusBarMode};

use crate::document_state::DocumentState;

/// Snapshot of editor state needed to update the status bar.
pub struct StatusBarUpdate<'a> {
    pub active_doc: Option<&'a DocumentState>,
    pub workspace_root: Option<&'a std::path::Path>,
    pub diagnostic_manager: &'a DiagnosticManager,
    pub remote_manager: &'a RemoteManager,
    pub is_debugging: bool,
    pub has_folder: bool,
    pub notification_count: u32,
}

/// Updates all status bar items to reflect the current application state.
pub fn update_status_bar<F: FnMut(&str)>(
    status_bar: &mut StatusBar<F>,
    update: &StatusBarUpdate<'_>,
) {
    // ── Mode ─────────────────────────────────────────────────────
    let is_remote = !update.remote_manager.active_connections().is_empty();
    let mode = if update.is_debugging {
        StatusBarMode::Debugging
    } else if is_remote {
        StatusBarMode::Remote
    } else if !update.has_folder {
        StatusBarMode::NoFolder
    } else {
        StatusBarMode::Normal
    };
    status_bar.set_mode(mode);

    // ── Remote indicator ─────────────────────────────────────────
    if is_remote {
        let connections = update.remote_manager.active_connections();
        let label = connections.first().map_or("Remote".into(), |c| c.label.clone());
        status_bar.set_item("remote.indicator", &label);
        status_bar.set_item_visible("remote.indicator", true);
    } else {
        status_bar.set_item_visible("remote.indicator", false);
    }

    // ── Git branch + sync status ─────────────────────────────────
    if let Some(root) = update.workspace_root {
        match sidex_git::repo::current_branch(root) {
            Ok(branch) => {
                status_bar.set_item("git.branch", &branch);
                status_bar.set_item_visible("git.branch", true);
            }
            Err(_) => {
                status_bar.set_item_visible("git.branch", false);
            }
        }
    } else {
        status_bar.set_item_visible("git.branch", false);
    }

    // ── Diagnostics ──────────────────────────────────────────────
    let counts = update.diagnostic_manager.diagnostic_counts();
    status_bar.set_item("problems.errors", &counts.errors.to_string());
    status_bar.set_item("problems.warnings", &counts.warnings.to_string());
    status_bar.set_item_tooltip(
        "problems.errors",
        &format!("{} Error(s)", counts.errors),
    );
    status_bar.set_item_tooltip(
        "problems.warnings",
        &format!("{} Warning(s)", counts.warnings),
    );

    if counts.errors > 0 {
        status_bar.set_item_color(
            "problems.errors",
            sidex_gpu::color::Color::from_hex("#f14c4c").unwrap_or(sidex_gpu::color::Color::WHITE),
        );
    }
    if counts.warnings > 0 {
        status_bar.set_item_color(
            "problems.warnings",
            sidex_gpu::color::Color::from_hex("#cca700").unwrap_or(sidex_gpu::color::Color::WHITE),
        );
    }

    // ── Editor-specific items ────────────────────────────────────
    if let Some(doc) = update.active_doc {
        let pos = doc.document.cursors.primary().selection.active;
        status_bar.set_item(
            "cursor.position",
            &format!("Ln {}, Col {}", pos.line + 1, pos.column + 1),
        );
        status_bar.set_item_visible("cursor.position", true);

        // Selection info
        let sel = doc.document.cursors.primary().selection;
        if !sel.is_empty() {
            let range = sel.range();
            let lines = range.end.line - range.start.line + 1;
            let start_off = doc.document.buffer.position_to_offset(range.start);
            let end_off = doc.document.buffer.position_to_offset(range.end);
            let chars = end_off.saturating_sub(start_off);
            status_bar.set_item(
                "selection.info",
                &format!("{chars} selected ({lines} lines)"),
            );
            status_bar.set_item_visible("selection.info", true);
        } else {
            status_bar.set_item_visible("selection.info", false);
        }

        // Language mode
        status_bar.set_item("editor.language", &doc.language_id);
        status_bar.set_item_visible("editor.language", true);

        // Encoding
        status_bar.set_item("editor.encoding", doc.encoding.label());
        status_bar.set_item_visible("editor.encoding", true);

        // Line ending
        let eol_label = match doc.document.line_ending {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
            LineEnding::Cr => "CR",
            LineEnding::Mixed => "Mixed",
        };
        status_bar.set_item("editor.eol", eol_label);
        status_bar.set_item_visible("editor.eol", true);

        // Indentation
        let indent = doc.document.buffer.detect_indentation();
        let indent_label = if indent.use_tabs {
            format!("Tab Size: {}", indent.tab_size)
        } else {
            format!("Spaces: {}", indent.tab_size)
        };
        status_bar.set_item("editor.indent", &indent_label);
        status_bar.set_item_visible("editor.indent", true);
    } else {
        status_bar.set_item_visible("cursor.position", false);
        status_bar.set_item_visible("selection.info", false);
        status_bar.set_item_visible("editor.language", false);
        status_bar.set_item_visible("editor.encoding", false);
        status_bar.set_item_visible("editor.eol", false);
        status_bar.set_item_visible("editor.indent", false);
    }

    // ── Notifications ────────────────────────────────────────────
    status_bar.set_item_visible("notifications.bell", update.notification_count > 0);
    if update.notification_count > 0 {
        status_bar.set_item_tooltip(
            "notifications.bell",
            &format!("{} new notification(s)", update.notification_count),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidex_lsp::DiagnosticManager;
    use sidex_remote::RemoteManager;
    use sidex_ui::workbench::status_bar::{default_status_bar_items, StatusBar};

    fn noop(_: &str) {}

    fn make_update<'a>(doc: Option<&'a DocumentState>) -> StatusBarUpdate<'a> {
        // Leak to extend lifetimes for test purposes
        let diag = Box::leak(Box::new(DiagnosticManager::new()));
        let remote = Box::leak(Box::new(RemoteManager::new()));
        StatusBarUpdate {
            active_doc: doc,
            workspace_root: None,
            diagnostic_manager: diag,
            remote_manager: remote,
            is_debugging: false,
            has_folder: true,
            notification_count: 0,
        }
    }

    #[test]
    fn update_hides_editor_items_with_no_doc() {
        let mut sb = StatusBar::new(default_status_bar_items(), noop);
        let upd = make_update(None);
        update_status_bar(&mut sb, &upd);

        let cursor = sb.items.iter().find(|i| i.id == "cursor.position").unwrap();
        assert!(!cursor.visible);

        let lang = sb.items.iter().find(|i| i.id == "editor.language").unwrap();
        assert!(!lang.visible);
    }

    #[test]
    fn update_shows_editor_items_with_doc() {
        let doc = DocumentState::new_untitled();
        let mut sb = StatusBar::new(default_status_bar_items(), noop);
        let upd = make_update(Some(&doc));
        update_status_bar(&mut sb, &upd);

        let cursor = sb.items.iter().find(|i| i.id == "cursor.position").unwrap();
        assert!(cursor.visible);
        assert!(cursor.text.contains("Ln"));

        let lang = sb.items.iter().find(|i| i.id == "editor.language").unwrap();
        assert!(lang.visible);
        assert_eq!(lang.text, "plaintext");

        let enc = sb.items.iter().find(|i| i.id == "editor.encoding").unwrap();
        assert!(enc.visible);
        assert_eq!(enc.text, "UTF-8");

        let eol = sb.items.iter().find(|i| i.id == "editor.eol").unwrap();
        assert!(eol.visible);
        assert_eq!(eol.text, "LF");
    }

    #[test]
    fn debug_mode_sets_mode() {
        let mut sb = StatusBar::new(default_status_bar_items(), noop);
        let diag = Box::leak(Box::new(DiagnosticManager::new()));
        let remote = Box::leak(Box::new(RemoteManager::new()));
        let upd = StatusBarUpdate {
            active_doc: None,
            workspace_root: None,
            diagnostic_manager: diag,
            remote_manager: remote,
            is_debugging: true,
            has_folder: true,
            notification_count: 0,
        };
        update_status_bar(&mut sb, &upd);
        assert_eq!(sb.mode, StatusBarMode::Debugging);
    }

    #[test]
    fn no_folder_mode() {
        let mut sb = StatusBar::new(default_status_bar_items(), noop);
        let diag = Box::leak(Box::new(DiagnosticManager::new()));
        let remote = Box::leak(Box::new(RemoteManager::new()));
        let upd = StatusBarUpdate {
            active_doc: None,
            workspace_root: None,
            diagnostic_manager: diag,
            remote_manager: remote,
            is_debugging: false,
            has_folder: false,
            notification_count: 0,
        };
        update_status_bar(&mut sb, &upd);
        assert_eq!(sb.mode, StatusBarMode::NoFolder);
    }

    #[test]
    fn notification_bell_hidden_when_zero() {
        let mut sb = StatusBar::new(default_status_bar_items(), noop);
        let upd = make_update(None);
        update_status_bar(&mut sb, &upd);

        let bell = sb.items.iter().find(|i| i.id == "notifications.bell").unwrap();
        assert!(!bell.visible);
    }
}
