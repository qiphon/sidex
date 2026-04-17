//! Application state — owns all subsystems and wires them together.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use winit::window::Window;

use sidex_db::Database;
use sidex_extension_api::CommandRegistry as ExtCommandRegistry;
use sidex_extensions::ExtensionRegistry;
use sidex_gpu::color::Color as GpuColor;
use sidex_gpu::EditorView as GpuEditorView;
use sidex_gpu::GpuRenderer;
use sidex_keymap::{ContextKeys, KeybindingResolver};
use sidex_lsp::{DiagnosticCollection, LspClient, ServerRegistry};
use sidex_remote::RemoteManager;
use sidex_settings::Settings;
use sidex_syntax::LanguageRegistry;
use sidex_terminal::TerminalManager;
use sidex_theme::Theme;
use sidex_ui::Workbench;
use sidex_workspace::Workspace;

use crate::clipboard;
use crate::commands::{CommandRegistry, NavigationEntry};
use crate::document_state::DocumentState;
use crate::editor_group::{AutoSaveMode, EditorGroupLayout};
use crate::editor_view::{self, EditorViewConfig};
use crate::file_dialog;
use crate::layout::{Layout, LayoutRects};

/// The central application struct holding every subsystem.
pub struct App {
    // ── Rendering ────────────────────────────────────────────────
    pub renderer: GpuRenderer,

    // ── Documents (open files) ───────────────────────────────────
    pub documents: Vec<DocumentState>,
    pub active_document: usize,

    // ── Language ─────────────────────────────────────────────────
    pub language_registry: LanguageRegistry,
    pub lsp_clients: HashMap<String, LspClientEntry>,
    pub server_registry: ServerRegistry,
    pub diagnostics: DiagnosticCollection,

    // ── Workspace ────────────────────────────────────────────────
    pub workspace: Option<Workspace>,

    // ── Terminal ─────────────────────────────────────────────────
    pub terminal_manager: TerminalManager,

    // ── Extensions ───────────────────────────────────────────────
    pub extension_registry: ExtensionRegistry,
    pub ext_command_registry: ExtCommandRegistry,

    // ── Configuration ────────────────────────────────────────────
    pub settings: Settings,
    pub theme: Theme,
    pub keymap: KeybindingResolver,
    pub context_keys: ContextKeys,

    // ── State persistence ────────────────────────────────────────
    pub db: Database,

    // ── Remote ───────────────────────────────────────────────────
    pub remote_manager: RemoteManager,

    // ── UI ───────────────────────────────────────────────────────
    pub workbench: Workbench,
    pub commands: CommandRegistry,
    pub layout: Layout,
    pub layout_rects: LayoutRects,
    pub editor_view_config: EditorViewConfig,

    // ── GPU editor rendering ────────────────────────────────────
    pub gpu_editor_view: GpuEditorView,
    pub font_system: cosmic_text::FontSystem,

    // ── Navigation ───────────────────────────────────────────────
    pub navigation_stack_back: Vec<NavigationEntry>,
    pub navigation_stack_forward: Vec<NavigationEntry>,

    // ── Editor groups (split views) ───────────────────────────────
    pub editor_groups: EditorGroupLayout,

    // ── Auto-save ─────────────────────────────────────────────────
    pub auto_save_mode: AutoSaveMode,
    pub auto_save_delay_ms: u64,
    auto_save_timer: Option<std::time::Instant>,

    // ── UI state flags ───────────────────────────────────────────
    pub show_quick_open: bool,
    pub show_command_palette: bool,
    pub show_goto_line: bool,
    pub show_find_widget: bool,
    pub find_replace_mode: bool,
    pub show_search_panel: bool,
    pub zoom_level: i32,
    pub needs_relayout: bool,
    pub needs_render: bool,

    // ── Quick pick / suggest state ───────────────────────────────
    pub quick_pick_index: usize,
    pub suggest_index: usize,
    pub goto_line_input: String,
    pub find_query: String,
    pub context_menu_position: Option<(f32, f32)>,

    // ── Internal ─────────────────────────────────────────────────
    window: Arc<Window>,
    cursor_visible: bool,
    cursor_blink_timer: std::time::Instant,

    /// Partial chord state: if the user pressed the first key of a
    /// two-key chord, this holds that combo so the resolver can
    /// match the second key.
    pub pending_chord: Option<sidex_keymap::KeyCombo>,
}

/// Wrapper for an LSP client associated with a language.
pub struct LspClientEntry {
    pub client: LspClient,
    pub language_id: String,
}

impl App {
    /// Initialise every subsystem and optionally open a workspace path.
    pub async fn new(window: Arc<Window>, open_path: Option<&Path>) -> Result<Self> {
        let renderer = GpuRenderer::new(window.clone())
            .await
            .context("GPU initialisation failed")?;

        let mut settings = Settings::new();
        if let Some(user_settings) = user_settings_path() {
            if user_settings.exists() {
                if let Err(e) = settings.load_user(&user_settings) {
                    log::warn!("failed to load user settings: {e}");
                }
            }
        }

        let theme = Theme::default_dark();
        let mut keymap = KeybindingResolver::new();
        if let Some(kb_path) = user_keybindings_path() {
            if kb_path.exists() {
                if let Err(e) = keymap.load_user(&kb_path) {
                    log::warn!("failed to load user keybindings: {e}");
                }
            }
        }

        let mut context_keys = ContextKeys::new();
        context_keys.set_bool("editorTextFocus", true);
        context_keys.set_bool("editorHasSelection", false);
        context_keys.set_bool("editorReadonly", false);
        context_keys.set_bool("terminalFocus", false);
        context_keys.set_bool("sideBarVisible", true);
        context_keys.set_bool("panelVisible", true);
        context_keys.set_string("editorLangId", "plaintext");

        let workspace = open_path.map(Workspace::open);

        let db = Database::open_default().unwrap_or_else(|e| {
            log::warn!("failed to open state db, using temp: {e}");
            let tmp = std::env::temp_dir().join("sidex-fallback.db");
            Database::open(&tmp).expect("fallback db must open")
        });

        if let Some(state) = sidex_db::load_window_state(&db).ok().flatten() {
            log::debug!(
                "restored window state: {}x{} at ({}, {})",
                state.width,
                state.height,
                state.x,
                state.y,
            );
        }

        let terminal_manager = TerminalManager::new();
        let commands = CommandRegistry::new();
        let language_registry = LanguageRegistry::new();
        let server_registry = ServerRegistry::new();
        let extension_registry = ExtensionRegistry::new();
        let ext_command_registry = ExtCommandRegistry::new();
        let remote_manager = RemoteManager::new();
        let diagnostics = DiagnosticCollection::new();
        let workbench = Workbench::new(&theme);

        let (w, h) = renderer.surface_size();
        let layout = Layout::default();
        let layout_rects = layout.compute(w, h);

        let editor_view_config = EditorViewConfig::default();
        let bg_color = GpuColor {
            r: 0.12,
            g: 0.12,
            b: 0.12,
            a: 1.0,
        };
        let gpu_editor_view =
            GpuEditorView::new(renderer.device.clone(), renderer.queue.clone(), bg_color);
        let font_system = cosmic_text::FontSystem::new();

        let mut app = Self {
            renderer,
            documents: Vec::new(),
            active_document: 0,
            language_registry,
            lsp_clients: HashMap::new(),
            server_registry,
            diagnostics,
            workspace,
            terminal_manager,
            extension_registry,
            ext_command_registry,
            settings,
            theme,
            keymap,
            context_keys,
            db,
            remote_manager,
            workbench,
            commands,
            layout,
            layout_rects,
            editor_view_config,
            gpu_editor_view,
            font_system,
            navigation_stack_back: Vec::new(),
            navigation_stack_forward: Vec::new(),
            editor_groups: EditorGroupLayout::new(),
            auto_save_mode: AutoSaveMode::Off,
            auto_save_delay_ms: 1000,
            auto_save_timer: None,
            show_quick_open: false,
            show_command_palette: false,
            show_goto_line: false,
            show_find_widget: false,
            find_replace_mode: false,
            show_search_panel: false,
            zoom_level: 0,
            needs_relayout: false,
            needs_render: true,
            quick_pick_index: 0,
            suggest_index: 0,
            goto_line_input: String::new(),
            find_query: String::new(),
            context_menu_position: None,
            window,
            cursor_visible: true,
            cursor_blink_timer: std::time::Instant::now(),
            pending_chord: None,
        };

        if app.documents.is_empty() {
            app.documents.push(DocumentState::new_untitled());
        }

        Ok(app)
    }

    // ── Document access helpers ──────────────────────────────────

    /// Mutable reference to the active document state, if any.
    pub fn active_document_mut(&mut self) -> Option<&mut DocumentState> {
        self.documents.get_mut(self.active_document)
    }

    /// Immutable reference to the active document state, if any.
    pub fn active_document_ref(&self) -> Option<&DocumentState> {
        self.documents.get(self.active_document)
    }

    /// Returns the line comment prefix for the active document's language.
    pub fn active_comment_prefix(&self) -> String {
        self.active_document_ref()
            .and_then(|doc| {
                self.language_registry
                    .language_for_name(&doc.language_id)
                    .and_then(|lang| lang.line_comment.clone())
            })
            .unwrap_or_else(|| "//".to_owned())
    }

    /// Returns the block comment delimiters for the active document's language.
    pub fn active_block_comment(&self) -> (String, String) {
        self.active_document_ref()
            .and_then(|doc| {
                self.language_registry
                    .language_for_name(&doc.language_id)
                    .and_then(|lang| lang.block_comment.clone())
            })
            .unwrap_or_else(|| ("/*".to_owned(), "*/".to_owned()))
    }

    // ── File operations ──────────────────────────────────────────

    /// Create a new untitled document tab.
    pub fn new_untitled_file(&mut self) {
        self.documents.push(DocumentState::new_untitled());
        self.active_document = self.documents.len() - 1;
        self.update_context_keys();
        self.needs_render = true;
    }

    /// Open a file by path.
    pub fn open_file(&mut self, path: &Path) {
        for (i, doc) in self.documents.iter().enumerate() {
            if doc.file_path.as_deref() == Some(path) {
                self.active_document = i;
                self.update_context_keys();
                self.needs_render = true;
                return;
            }
        }

        match DocumentState::open_file(path, &self.language_registry) {
            Ok(doc_state) => {
                self.documents.push(doc_state);
                self.active_document = self.documents.len() - 1;
                if let Some(ws) = &mut self.workspace {
                    ws.add_recent(path);
                }
                self.update_context_keys();
                self.needs_render = true;
            }
            Err(e) => {
                log::error!("failed to open file {}: {e}", path.display());
            }
        }
    }

    /// Show a native file open dialog and open the selected file.
    pub fn open_file_dialog(&mut self) {
        if let Some(path) = file_dialog::open_file_dialog() {
            self.open_file(&path);
        }
    }

    /// Save the active file.
    pub fn save_active_file(&mut self) {
        let needs_save_as = self
            .active_document_ref()
            .map_or(false, |d| d.file_path.is_none());

        if needs_save_as {
            self.save_active_file_as();
            return;
        }

        if let Some(doc) = self.active_document_mut() {
            if let Err(e) = doc.save() {
                log::error!("save failed: {e}");
            }
            self.needs_render = true;
        }
    }

    /// Save the active file with a "Save As" dialog.
    pub fn save_active_file_as(&mut self) {
        let suggested = self
            .active_document_ref()
            .map(|d| d.display_name())
            .unwrap_or_default();

        if let Some(path) = file_dialog::save_file_dialog(&suggested) {
            if let Some(doc) = self.active_document_mut() {
                if let Err(e) = doc.save_as(&path) {
                    log::error!("save as failed: {e}");
                }
            }
            self.needs_render = true;
        }
    }

    /// Save all open files that have a file path.
    pub fn save_all_files(&mut self) {
        for doc in &mut self.documents {
            if doc.file_path.is_some() && doc.is_dirty() {
                if let Err(e) = doc.save() {
                    log::error!("save failed: {e}");
                }
            }
        }
        self.needs_render = true;
    }

    /// Close the active editor tab.
    pub fn close_active_editor(&mut self) {
        if self.documents.is_empty() {
            return;
        }

        let idx = self.active_document;

        if let Some(path) = self.documents[idx].file_path.as_ref() {
            self.commands
                .recently_closed
                .push(path.display().to_string());
        }

        self.documents.remove(idx);

        if self.documents.is_empty() {
            self.documents.push(DocumentState::new_untitled());
            self.active_document = 0;
        } else if self.active_document >= self.documents.len() {
            self.active_document = self.documents.len() - 1;
        }

        self.update_context_keys();
        self.needs_render = true;
    }

    /// Close all open editor tabs.
    pub fn close_all_editors(&mut self) {
        for doc in &self.documents {
            if let Some(path) = doc.file_path.as_ref() {
                self.commands
                    .recently_closed
                    .push(path.display().to_string());
            }
        }
        self.documents.clear();
        self.documents.push(DocumentState::new_untitled());
        self.active_document = 0;
        self.update_context_keys();
        self.needs_render = true;
    }

    /// Reopen the most recently closed editor.
    pub fn reopen_closed_editor(&mut self) {
        if let Some(path_str) = self.commands.recently_closed.pop() {
            let path = PathBuf::from(&path_str);
            if path.exists() {
                self.open_file(&path);
            }
        }
    }

    /// Switch to a document tab by index.
    pub fn switch_to_document(&mut self, index: usize) {
        if index < self.documents.len() {
            self.active_document = index;
            self.update_context_keys();
            self.needs_render = true;
        }
    }

    // ── Clipboard operations ─────────────────────────────────────

    /// Copy the current selection to clipboard.
    pub fn clipboard_copy(&mut self) {
        if let Some(doc) = self.active_document_ref() {
            let sel = doc.document.cursors.primary().selection;
            if !sel.is_empty() {
                let start = doc.document.buffer.position_to_offset(sel.start());
                let end = doc.document.buffer.position_to_offset(sel.end());
                let text = doc.document.buffer.slice(start..end);
                if let Err(e) = clipboard::copy_to_clipboard(&text) {
                    log::warn!("clipboard copy failed: {e}");
                }
            }
        }
    }

    /// Cut the current selection to clipboard.
    pub fn clipboard_cut(&mut self) {
        self.clipboard_copy();
        if let Some(doc) = self.active_document_mut() {
            let sel = doc.document.cursors.primary().selection;
            if !sel.is_empty() {
                doc.document.delete_right();
                doc.on_edit();
            }
        }
    }

    /// Paste from clipboard.
    pub fn clipboard_paste(&mut self) {
        if let Some(text) = clipboard::paste_from_clipboard() {
            if let Some(doc) = self.active_document_mut() {
                doc.document.insert_text(&text);
                doc.on_edit();
            }
        }
    }

    // ── UI overlay management ────────────────────────────────────

    /// Dismiss all overlay widgets (quick open, command palette, find, etc.).
    pub fn dismiss_overlays(&mut self) {
        self.show_quick_open = false;
        self.show_command_palette = false;
        self.show_goto_line = false;
        self.show_find_widget = false;
        self.find_replace_mode = false;
        self.show_search_panel = false;
        self.context_menu_position = None;
        self.context_keys.set_bool("suggestWidgetVisible", false);
        self.context_keys.set_bool("findWidgetVisible", false);
        self.context_keys.set_bool("inQuickOpen", false);
    }

    // ── Quick pick navigation ────────────────────────────────────

    /// Move the quick pick selection by `delta` items.
    pub fn quick_pick_move(&mut self, delta: i32) {
        let count = if self.show_command_palette {
            self.commands.ids().len()
        } else {
            self.documents.len().max(1)
        };
        if count == 0 {
            return;
        }
        let new_idx = (self.quick_pick_index as i32 + delta).rem_euclid(count as i32) as usize;
        self.quick_pick_index = new_idx;
        self.needs_render = true;
    }

    /// Confirm the current quick pick selection.
    pub fn confirm_quick_pick(&mut self) {
        if self.show_command_palette {
            let ids = self.commands.ids();
            if let Some(cmd_id) = ids.get(self.quick_pick_index) {
                let id = cmd_id.to_string();
                if let Some(action) = self.commands.get_action(&id) {
                    self.show_command_palette = false;
                    self.context_keys.set_bool("inQuickOpen", false);
                    action(self);
                    return;
                }
            }
            self.show_command_palette = false;
        } else if self.show_quick_open {
            let idx = self.quick_pick_index;
            if idx < self.documents.len() {
                self.active_document = idx;
                self.update_context_keys();
            }
            self.show_quick_open = false;
            self.context_keys.set_bool("inQuickOpen", false);
        }
        self.needs_render = true;
    }

    /// Confirm the goto-line input.
    pub fn confirm_goto_line(&mut self) {
        if let Ok(line_num) = self.goto_line_input.trim().parse::<u32>() {
            let target_line = line_num.saturating_sub(1);
            if let Some(doc) = self.active_document_mut() {
                let max_line = doc.document.buffer.len_lines().saturating_sub(1) as u32;
                let clamped = target_line.min(max_line);
                let pos = sidex_text::Position::new(clamped, 0);
                doc.document.cursors = sidex_editor::MultiCursor::new(pos);
                doc.viewport.ensure_visible(pos);
            }
        }
        self.show_goto_line = false;
        self.goto_line_input.clear();
        self.needs_render = true;
    }

    // ── Suggest widget ──────────────────────────────────────────

    /// Move the suggest/autocomplete selection by `delta` items.
    pub fn suggest_move(&mut self, delta: i32) {
        self.suggest_index = (self.suggest_index as i32 + delta).max(0) as usize;
        self.needs_render = true;
    }

    /// Accept the current suggest/autocomplete selection.
    pub fn accept_suggest(&mut self) {
        self.context_keys.set_bool("suggestWidgetVisible", false);
        self.needs_render = true;
    }

    // ── Find widget ─────────────────────────────────────────────

    /// Navigate to the next find match.
    pub fn find_next(&mut self) {
        if self.find_query.is_empty() {
            return;
        }
        let query = self.find_query.clone();
        if let Some(doc) = self.active_document_mut() {
            let pos = doc.document.cursors.primary().position();
            let text = doc.document.text();
            let offset = doc.document.buffer.position_to_offset(pos);
            if let Some(start) = text[offset..].find(&query) {
                let abs_start = offset + start;
                let abs_end = abs_start + query.len();
                let start_pos = doc.document.buffer.offset_to_position(abs_start);
                let end_pos = doc.document.buffer.offset_to_position(abs_end);
                doc.document
                    .cursors
                    .set_primary_selection(sidex_editor::Selection::new(start_pos, end_pos));
                doc.viewport.ensure_visible(start_pos);
            }
        }
        self.needs_render = true;
    }

    /// Navigate to the previous find match.
    pub fn find_previous(&mut self) {
        if self.find_query.is_empty() {
            return;
        }
        let query = self.find_query.clone();
        if let Some(doc) = self.active_document_mut() {
            let pos = doc.document.cursors.primary().position();
            let text = doc.document.text();
            let cursor_offset = doc.document.buffer.position_to_offset(pos);
            let search_slice = &text[..cursor_offset];
            if let Some(start) = search_slice.rfind(&query) {
                let end = start + query.len();
                let start_pos = doc.document.buffer.offset_to_position(start);
                let end_pos = doc.document.buffer.offset_to_position(end);
                doc.document
                    .cursors
                    .set_primary_selection(sidex_editor::Selection::new(start_pos, end_pos));
                doc.viewport.ensure_visible(start_pos);
            }
        }
        self.needs_render = true;
    }

    // ── Zoom ────────────────────────────────────────────────────

    /// Increase the zoom level.
    pub fn zoom_in(&mut self) {
        self.zoom_level = (self.zoom_level + 1).min(10);
        self.apply_zoom();
    }

    /// Decrease the zoom level.
    pub fn zoom_out(&mut self) {
        self.zoom_level = (self.zoom_level - 1).max(-5);
        self.apply_zoom();
    }

    /// Reset the zoom level.
    pub fn zoom_reset(&mut self) {
        self.zoom_level = 0;
        self.apply_zoom();
    }

    fn apply_zoom(&mut self) {
        let base_size = 14.0_f32;
        let scale = 1.0 + (self.zoom_level as f32 * 0.1);
        self.editor_view_config.font_size = base_size * scale;
        self.editor_view_config.line_height = (base_size * scale * 1.5).round();
        self.editor_view_config.char_width = base_size * scale * 0.6;
        self.needs_render = true;
    }

    // ── Split editor ────────────────────────────────────────────

    /// Split the editor (opens a new view of the same document).
    pub fn split_editor(&mut self) {
        if let Some(doc) = self.active_document_ref() {
            if let Some(path) = &doc.file_path {
                let new_doc =
                    DocumentState::open_file(path, &self.language_registry).unwrap_or_else(|_| {
                        DocumentState::new_untitled()
                    });
                self.documents.push(new_doc);
                self.active_document = self.documents.len() - 1;
            }
        }
        self.editor_groups.split_right();
        self.needs_render = true;
    }

    // ── Editor group tab operations ──────────────────────────────

    /// Open a file in the active editor group, reusing an existing tab if
    /// the file is already open.
    pub fn open_file_in_active_group(&mut self, path: &Path) {
        self.editor_groups.active_group_mut().open_file(path);
        self.open_file(path);
    }

    /// Open a file as a preview tab (italic) in the active group.
    pub fn open_file_preview(&mut self, path: &Path) {
        self.editor_groups.active_group_mut().open_file_preview(path);

        for (i, doc) in self.documents.iter().enumerate() {
            if doc.file_path.as_deref() == Some(path) {
                self.active_document = i;
                self.update_context_keys();
                self.needs_render = true;
                return;
            }
        }

        match DocumentState::open_file(path, &self.language_registry) {
            Ok(doc_state) => {
                self.documents.push(doc_state);
                self.active_document = self.documents.len() - 1;
                self.update_context_keys();
                self.needs_render = true;
            }
            Err(e) => {
                log::error!("failed to open file {}: {e}", path.display());
            }
        }
    }

    /// Open a file in a specific editor group.
    pub fn open_file_in_group(&mut self, path: &Path, group: usize) {
        if group < self.editor_groups.groups.len() {
            self.editor_groups.groups[group].open_file(path);
            self.editor_groups.active_group = group;
            self.open_file(path);
        }
    }

    /// Close a specific tab in a group, prompting to save if modified.
    pub fn close_tab(&mut self, group: usize, tab: usize) {
        if group >= self.editor_groups.groups.len() {
            return;
        }
        let grp = &self.editor_groups.groups[group];
        if tab >= grp.tabs.len() {
            return;
        }

        let editor_tab = grp.tabs[tab].clone();
        self.editor_groups.record_closed(&editor_tab, group, tab);
        self.editor_groups.groups[group].close_tab(tab);

        if let Some(path) = &editor_tab.path {
            self.commands
                .recently_closed
                .push(path.display().to_string());
        }

        self.needs_render = true;
    }

    /// Close all tabs in a group except the specified one.
    pub fn close_other_tabs(&mut self, group: usize, except: usize) {
        if group >= self.editor_groups.groups.len() {
            return;
        }
        let closed = self.editor_groups.groups[group].close_others(except);
        for tab in &closed {
            self.editor_groups.record_closed(tab, group, 0);
        }
        self.needs_render = true;
    }

    /// Close tabs to the right of a specific tab in a group.
    pub fn close_tabs_to_right(&mut self, group: usize, tab: usize) {
        if group >= self.editor_groups.groups.len() {
            return;
        }
        let closed = self.editor_groups.groups[group].close_to_right(tab);
        for t in &closed {
            self.editor_groups.record_closed(t, group, 0);
        }
        self.needs_render = true;
    }

    /// Close all tabs in a group.
    pub fn close_all_tabs(&mut self, group: usize) {
        if group >= self.editor_groups.groups.len() {
            return;
        }
        let closed = self.editor_groups.groups[group].close_all();
        for t in &closed {
            self.editor_groups.record_closed(t, group, 0);
        }
        self.needs_render = true;
    }

    /// Navigate to the next tab in the active group (Ctrl+Tab).
    pub fn next_tab(&mut self) {
        self.editor_groups.active_group_mut().next_tab();
        self.sync_active_document_from_group();
        self.needs_render = true;
    }

    /// Navigate to the previous tab in the active group (Ctrl+Shift+Tab).
    pub fn prev_tab(&mut self) {
        self.editor_groups.active_group_mut().prev_tab();
        self.sync_active_document_from_group();
        self.needs_render = true;
    }

    /// Move the active tab left within its group.
    pub fn move_tab_left(&mut self) {
        self.editor_groups.active_group_mut().move_tab_left();
        self.needs_render = true;
    }

    /// Move the active tab right within its group.
    pub fn move_tab_right(&mut self) {
        self.editor_groups.active_group_mut().move_tab_right();
        self.needs_render = true;
    }

    /// Pin a tab in a group.
    pub fn pin_tab(&mut self, group: usize, tab: usize) {
        if group < self.editor_groups.groups.len() {
            self.editor_groups.groups[group].pin_tab(tab);
            self.needs_render = true;
        }
    }

    /// Unpin a tab in a group.
    pub fn unpin_tab(&mut self, group: usize, tab: usize) {
        if group < self.editor_groups.groups.len() {
            self.editor_groups.groups[group].unpin_tab(tab);
            self.needs_render = true;
        }
    }

    /// Split the editor to the right.
    pub fn split_editor_right(&mut self) {
        if let Some(doc) = self.active_document_ref() {
            if let Some(path) = &doc.file_path {
                let new_doc =
                    DocumentState::open_file(path, &self.language_registry).unwrap_or_else(|_| {
                        DocumentState::new_untitled()
                    });
                self.documents.push(new_doc);
            }
        }
        self.editor_groups.split_right();
        self.needs_render = true;
    }

    /// Split the editor downward.
    pub fn split_editor_down(&mut self) {
        if let Some(doc) = self.active_document_ref() {
            if let Some(path) = &doc.file_path {
                let new_doc =
                    DocumentState::open_file(path, &self.language_registry).unwrap_or_else(|_| {
                        DocumentState::new_untitled()
                    });
                self.documents.push(new_doc);
            }
        }
        self.editor_groups.split_down();
        self.needs_render = true;
    }

    /// Focus a specific editor group by index.
    pub fn focus_group(&mut self, index: usize) {
        self.editor_groups.focus_group(index);
        self.sync_active_document_from_group();
        self.needs_render = true;
    }

    /// Cycle focus to the next editor group.
    pub fn next_group(&mut self) {
        self.editor_groups.next_group();
        self.sync_active_document_from_group();
        self.needs_render = true;
    }

    /// Cycle focus to the previous editor group.
    pub fn prev_group(&mut self) {
        self.editor_groups.prev_group();
        self.sync_active_document_from_group();
        self.needs_render = true;
    }

    /// Sync the `active_document` index to match the active tab in the
    /// active editor group. This bridges the tab model and the document model.
    fn sync_active_document_from_group(&mut self) {
        let group = self.editor_groups.active_group();
        if let Some(tab) = group.active_tab_ref() {
            if let Some(path) = &tab.path {
                for (i, doc) in self.documents.iter().enumerate() {
                    if doc.file_path.as_deref() == Some(path.as_path()) {
                        self.active_document = i;
                        self.update_context_keys();
                        return;
                    }
                }
            }
        }
    }

    /// Sync modified state from documents to editor tabs.
    pub fn sync_dirty_state(&mut self) {
        for group in &mut self.editor_groups.groups {
            for tab in &mut group.tabs {
                if let Some(path) = &tab.path {
                    for doc in &self.documents {
                        if doc.file_path.as_deref() == Some(path.as_path()) {
                            tab.is_modified = doc.is_dirty();
                            break;
                        }
                    }
                }
            }
        }
    }

    // ── Auto-save ────────────────────────────────────────────────

    /// Configure auto-save from settings values.
    pub fn configure_auto_save(&mut self, mode: &str, delay_ms: u64) {
        self.auto_save_mode = AutoSaveMode::from_setting(mode);
        self.auto_save_delay_ms = delay_ms;
    }

    /// Notify the auto-save system that an edit occurred.
    pub fn on_document_edit(&mut self) {
        if self.auto_save_mode == AutoSaveMode::AfterDelay {
            self.auto_save_timer = Some(std::time::Instant::now());
        }
        self.sync_dirty_state();
    }

    /// Notify the auto-save system that the editor lost focus.
    pub fn on_editor_focus_change(&mut self) {
        if self.auto_save_mode == AutoSaveMode::OnFocusChange {
            self.save_all_files();
        }
    }

    /// Notify the auto-save system that the window lost focus.
    pub fn on_window_focus_change(&mut self) {
        if self.auto_save_mode == AutoSaveMode::OnWindowChange {
            self.save_all_files();
        }
    }

    /// Tick the auto-save timer; called from the main update loop.
    fn tick_auto_save(&mut self) {
        if self.auto_save_mode != AutoSaveMode::AfterDelay {
            return;
        }
        if let Some(timer) = self.auto_save_timer {
            let delay = std::time::Duration::from_millis(self.auto_save_delay_ms);
            if timer.elapsed() >= delay {
                self.auto_save_timer = None;
                self.save_all_files();
            }
        }
    }

    /// Count of modified tabs across all groups (for "Save N files?" dialog).
    pub fn unsaved_tab_count(&self) -> usize {
        self.documents.iter().filter(|d| d.is_dirty()).count()
    }

    // ── Terminal ─────────────────────────────────────────────────

    /// Create a new terminal instance.
    pub fn new_terminal(&mut self) {
        if let Err(e) = self.terminal_manager.create(None, None) {
            log::error!("failed to create terminal: {e}");
        }
        self.layout.panel_visible = true;
        self.needs_relayout = true;
    }

    // ── Code actions / format / rename ──────────────────────────

    /// Trigger the suggest/autocomplete widget.
    pub fn trigger_suggest(&mut self) {
        self.context_keys.set_bool("suggestWidgetVisible", true);
        self.suggest_index = 0;
        self.needs_render = true;
    }

    /// Show code action quick-fix menu.
    pub fn show_code_actions(&mut self) {
        log::debug!("show code actions (LSP quick fix)");
        self.needs_render = true;
    }

    /// Start a rename refactoring operation.
    pub fn start_rename(&mut self) {
        log::debug!("start rename refactoring");
        self.context_keys.set_bool("renameInputVisible", true);
        self.needs_render = true;
    }

    /// Format the active document via LSP.
    pub fn format_document(&mut self) {
        log::debug!("format document requested");
        self.needs_render = true;
    }

    // ── Context key management ───────────────────────────────────

    /// Update context keys to reflect current editor state.
    pub fn update_context_keys(&mut self) {
        if let Some(doc) = self.documents.get(self.active_document) {
            let has_selection = !doc.document.cursors.primary().selection.is_empty();
            self.context_keys
                .set_bool("editorHasSelection", has_selection);
            self.context_keys
                .set_string("editorLangId", &doc.language_id);
            self.context_keys.set_bool("editorReadonly", false);
        }
        self.context_keys
            .set_bool("sideBarVisible", self.layout.sidebar_visible);
        self.context_keys
            .set_bool("panelVisible", self.layout.panel_visible);
    }

    // ── Tick / update ────────────────────────────────────────────

    /// Tick logic: cursor blink, auto-save timers, layout recomputation.
    pub fn update(&mut self) {
        const BLINK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(530);
        if self.cursor_blink_timer.elapsed() >= BLINK_INTERVAL {
            self.cursor_visible = !self.cursor_visible;
            self.cursor_blink_timer = std::time::Instant::now();
            self.needs_render = true;
        }

        self.tick_auto_save();

        if self.needs_relayout {
            let (w, h) = self.renderer.surface_size();
            self.layout_rects = self.layout.compute(w, h);
            self.needs_relayout = false;
            self.needs_render = true;
        }
    }

    // ── Render ───────────────────────────────────────────────────

    /// Render the current frame.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    pub fn render(&mut self) {
        let mut frame = match self.renderer.begin_frame() {
            Ok(f) => f,
            Err(e) => {
                log::error!("begin_frame failed: {e}");
                return;
            }
        };

        let default_fg = self
            .theme
            .workbench_colors
            .editor_foreground
            .map(|c| GpuColor {
                r: f32::from(c.r) / 255.0,
                g: f32::from(c.g) / 255.0,
                b: f32::from(c.b) / 255.0,
                a: f32::from(c.a) / 255.0,
            })
            .unwrap_or(GpuColor::WHITE);

        if let Some(doc) = self.documents.get(self.active_document) {
            let styled_lines = editor_view::build_styled_lines(doc, &self.theme, default_fg);
            let doc_snapshot = editor_view::build_document_snapshot(
                &styled_lines,
                self.editor_view_config.char_width,
            );
            let highlight_result = sidex_gpu::HighlightResult {
                lines: styled_lines,
            };

            let editor_rect = &self.layout_rects.editor_area;
            let gpu_viewport =
                editor_view::build_gpu_viewport(doc, editor_rect, &self.editor_view_config);
            let cursor_positions = editor_view::build_cursor_positions(
                doc,
                &self.editor_view_config,
                self.cursor_visible,
            );
            let selections = editor_view::build_selection_rects(doc, &self.editor_view_config);
            let active_line = doc.document.cursors.primary().selection.active.line;

            let gpu_config =
                editor_view::build_gpu_editor_config(&self.editor_view_config, &self.theme);

            let dt = 1.0 / 60.0;
            let input = editor_view::build_frame_input(
                gpu_viewport,
                &gpu_config,
                &cursor_positions,
                &selections,
                active_line,
                dt,
            );

            self.gpu_editor_view.render(
                &mut self.font_system,
                &mut frame,
                &doc_snapshot,
                &highlight_result,
                &input,
                &self.renderer,
            );
        }

        self.renderer.end_frame(frame);
        self.needs_render = false;
    }

    // ── State persistence ────────────────────────────────────────

    /// Persist application state before exit.
    pub fn save_state(&self) {
        let size = self.window.inner_size();
        let pos = self.window.outer_position().unwrap_or_default();
        let active_path = self
            .active_document_ref()
            .and_then(|d| d.file_path.as_ref())
            .map(|p| p.display().to_string());

        let state = sidex_db::WindowState {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
            is_maximized: self.window.is_maximized(),
            sidebar_width: f64::from(self.layout.sidebar_width),
            panel_height: f64::from(self.layout.panel_height),
            active_editor: active_path,
        };
        if let Err(e) = sidex_db::save_window_state(&self.db, &state) {
            log::warn!("failed to save window state: {e}");
        }
    }

    /// Check if any open documents have unsaved changes.
    pub fn has_unsaved_changes(&self) -> bool {
        self.documents.iter().any(DocumentState::is_dirty)
    }

    /// Get the window reference.
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Whether the cursor should be drawn (blink state).
    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    /// Reset cursor blink so it's visible immediately (e.g. after typing).
    pub fn reset_cursor_blink(&mut self) {
        self.cursor_visible = true;
        self.cursor_blink_timer = std::time::Instant::now();
    }
}

/// Returns the user settings file path (`~/.config/sidex/settings.json`).
fn user_settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("sidex").join("settings.json"))
}

/// Returns the user keybindings file path (`~/.config/sidex/keybindings.json`).
fn user_keybindings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("sidex").join("keybindings.json"))
}
