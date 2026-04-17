//! Multi-window management.
//!
//! Each SideX window owns its own [`App`] instance with an independent
//! workspace, set of open documents, terminal sessions, and renderer.
//!
//! Window positions are persisted to the database and restored on the
//! next launch so users get a seamless experience across sessions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use sidex_db::Database;

use crate::app::App;
use crate::tauri_bridge::TauriBridge;

// ── Window identity ──────────────────────────────────────────────────────────

/// Unique identifier for an open window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

impl std::fmt::Display for WindowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "win-{}", self.0)
    }
}

// ── Window geometry ──────────────────────────────────────────────────────────

/// Physical bounds of a window on screen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowBounds {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 1280,
            height: 720,
        }
    }
}

/// State of a window's chrome (normal, maximized, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowState {
    Normal,
    Maximized,
    Minimized,
    Fullscreen,
}

impl Default for WindowState {
    fn default() -> Self {
        Self::Normal
    }
}

// ── Per-window metadata ──────────────────────────────────────────────────────

/// All the properties SideX tracks per window beyond the [`App`] itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppWindow {
    pub id: u64,
    pub title: String,
    pub bounds: WindowBounds,
    pub state: WindowState,
    pub workspace: Option<PathBuf>,
    pub is_focused: bool,
    pub is_fullscreen: bool,
}

impl AppWindow {
    fn new(id: u64, title: String, workspace: Option<PathBuf>) -> Self {
        Self {
            id,
            title,
            bounds: WindowBounds::default(),
            state: WindowState::Normal,
            workspace,
            is_focused: false,
            is_fullscreen: false,
        }
    }
}

/// Internal entry: owns both the `App` and the metadata.
struct WindowEntry {
    app: App,
    meta: AppWindow,
}

// ── WindowManager ────────────────────────────────────────────────────────────

/// Manages multiple SideX windows, each with its own editor state.
pub struct WindowManager {
    windows: HashMap<WindowId, WindowEntry>,
    next_id: u64,
    focused: Option<WindowId>,
    bridge: Arc<TauriBridge>,
}

impl WindowManager {
    /// Create a new window manager backed by the given Tauri bridge.
    pub fn new(bridge: Arc<TauriBridge>) -> Self {
        Self {
            windows: HashMap::new(),
            next_id: 1,
            focused: None,
            bridge,
        }
    }

    // ── Window lifecycle ─────────────────────────────────────────

    /// Open a new window, optionally rooted at `workspace_path`.
    ///
    /// The window gets its own [`App`] instance with an independent
    /// renderer, workspace, documents, and terminals.
    pub async fn create_window(&mut self, workspace: Option<&Path>) -> Result<WindowId> {
        let id = WindowId(self.next_id);
        self.next_id += 1;

        let title = format_title(None, workspace);

        let _handle = self
            .bridge
            .create_window(&title, 1280, 720)
            .context("WindowManager: failed to create native window")?;

        let winit_el = winit::event_loop::EventLoop::new()
            .context("failed to create event loop for new window")?;
        let window_attrs = winit::window::Window::default_attributes()
            .with_title(&title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0_f64, 720.0));

        #[allow(deprecated)]
        let window = Arc::new(
            winit_el
                .create_window(window_attrs)
                .context("failed to create winit window")?,
        );

        let app = App::new(window, workspace).await?;
        let meta = AppWindow::new(id.0, title, workspace.map(Path::to_path_buf));

        self.windows.insert(id, WindowEntry { app, meta });
        self.focused = Some(id);

        log::info!("opened window {id}");
        Ok(id)
    }

    /// Close a window and clean up its resources.
    pub fn close_window(&mut self, id: WindowId) -> Result<()> {
        let entry = self.windows.remove(&id).context("window not found")?;

        entry.app.save_state();
        log::info!("closed window {id}");

        if self.focused == Some(id) {
            self.focused = self.windows.keys().next().copied();
        }

        Ok(())
    }

    /// Bring a window to the front.
    pub fn focus_window(&mut self, id: WindowId) {
        if !self.windows.contains_key(&id) {
            return;
        }

        if let Some(prev) = self.focused {
            if prev != id {
                if let Some(old) = self.windows.get_mut(&prev) {
                    old.meta.is_focused = false;
                }
            }
        }

        if let Some(entry) = self.windows.get_mut(&id) {
            entry.meta.is_focused = true;
            entry.app.window().focus_window();
        }
        self.focused = Some(id);
        log::debug!("focused window {id}");
    }

    // ── Window state changes ─────────────────────────────────────

    /// Toggle fullscreen for a window.
    pub fn toggle_fullscreen(&mut self, id: WindowId) {
        if let Some(entry) = self.windows.get_mut(&id) {
            entry.meta.is_fullscreen = !entry.meta.is_fullscreen;
            entry.meta.state = if entry.meta.is_fullscreen {
                WindowState::Fullscreen
            } else {
                WindowState::Normal
            };
            log::debug!(
                "window {id} fullscreen = {}",
                entry.meta.is_fullscreen
            );
        }
    }

    /// Set the window state (normal, maximized, minimized, fullscreen).
    pub fn set_window_state(&mut self, id: WindowId, state: WindowState) {
        if let Some(entry) = self.windows.get_mut(&id) {
            entry.meta.state = state;
            entry.meta.is_fullscreen = state == WindowState::Fullscreen;
        }
    }

    /// Update the stored bounds for a window (called on resize/move events).
    pub fn update_bounds(&mut self, id: WindowId, bounds: WindowBounds) {
        if let Some(entry) = self.windows.get_mut(&id) {
            entry.meta.bounds = bounds;
        }
    }

    // ── Title management ─────────────────────────────────────────

    /// Update a window's title bar.  Follows the VS Code convention:
    /// `filename — workspace — SideX`.
    pub fn update_title(&mut self, id: WindowId, filename: Option<&str>, workspace: Option<&Path>) {
        if let Some(entry) = self.windows.get_mut(&id) {
            entry.meta.title = format_title(filename, workspace);
            let _ = entry.app.window().set_title(&entry.meta.title);
        }
    }

    // ── Tab-to-window operations ─────────────────────────────────

    /// Move the active editor tab from `src` into a brand-new window.
    pub async fn move_active_tab_to_new_window(&mut self, src: WindowId) -> Result<Option<WindowId>> {
        let path = {
            let entry = self.windows.get(&src).context("source window not found")?;
            entry.app.active_document_ref().and_then(|d| d.file_path.clone())
        };

        let path = match path {
            Some(p) => p,
            None => return Ok(None),
        };

        if let Some(entry) = self.windows.get_mut(&src) {
            entry.app.close_active_editor();
        }

        let new_id = self.create_window(None).await?;
        if let Some(entry) = self.windows.get_mut(&new_id) {
            entry.app.open_file(&path);
        }

        Ok(Some(new_id))
    }

    // ── Queries ──────────────────────────────────────────────────

    /// List all open window IDs.
    pub fn windows(&self) -> Vec<WindowId> {
        self.windows.keys().copied().collect()
    }

    /// List the metadata for all open windows.
    pub fn window_list(&self) -> Vec<&AppWindow> {
        self.windows.values().map(|e| &e.meta).collect()
    }

    /// Number of open windows.
    pub fn count(&self) -> usize {
        self.windows.len()
    }

    /// Returns the currently focused window ID, if any.
    pub fn focused(&self) -> Option<WindowId> {
        self.focused
    }

    /// Mutable access to the [`App`] in a specific window.
    pub fn app_mut(&mut self, id: WindowId) -> Option<&mut App> {
        self.windows.get_mut(&id).map(|e| &mut e.app)
    }

    /// Immutable access to the [`App`] in a specific window.
    pub fn app(&self, id: WindowId) -> Option<&App> {
        self.windows.get(&id).map(|e| &e.app)
    }

    /// Returns the workspace path associated with a window.
    pub fn workspace_path(&self, id: WindowId) -> Option<&Path> {
        self.windows
            .get(&id)
            .and_then(|e| e.meta.workspace.as_deref())
    }

    /// Returns `true` if any window has unsaved changes.
    pub fn has_unsaved_changes(&self) -> bool {
        self.windows.values().any(|e| e.app.has_unsaved_changes())
    }

    /// Mutable access to the focused window's [`App`].
    pub fn focused_app_mut(&mut self) -> Option<&mut App> {
        let id = self.focused?;
        self.app_mut(id)
    }

    // ── Persistence ──────────────────────────────────────────────

    /// Save state for every open window (called at exit).
    pub fn save_all_state(&self) {
        for (id, entry) in &self.windows {
            entry.app.save_state();
            log::debug!("saved state for window {id}");
        }
    }

    /// Persist the positions/states of all windows so they can be restored.
    pub fn save_window_positions(&self, db: &Database) -> Result<()> {
        let records: Vec<AppWindow> = self.windows.values().map(|e| e.meta.clone()).collect();
        let json = serde_json::to_string(&records).context("serialise window positions")?;
        db.conn()
            .execute(
                "INSERT INTO state_kv (scope, key, value) VALUES ('global', 'window_positions', ?1)
                 ON CONFLICT(scope, key) DO UPDATE SET value = excluded.value",
                rusqlite::params![json],
            )
            .context("save window positions")?;
        Ok(())
    }

    /// Restore previously persisted window positions (returns the metadata; the
    /// caller is responsible for actually creating the windows).
    pub fn restore_window_positions(db: &Database) -> Result<Vec<AppWindow>> {
        let mut stmt = db
            .conn()
            .prepare_cached(
                "SELECT value FROM state_kv WHERE scope = 'global' AND key = 'window_positions'",
            )
            .context("prepare restore window positions")?;

        let result: Option<String> = stmt
            .query_row([], |row| row.get(0))
            .optional()
            .context("query window positions")?;

        match result {
            Some(json) => {
                let records: Vec<AppWindow> =
                    serde_json::from_str(&json).context("deserialise window positions")?;
                Ok(records)
            }
            None => Ok(Vec::new()),
        }
    }
}

// ── Title formatting ─────────────────────────────────────────────────────────

/// Build a title string following the VS Code pattern:
/// `filename — workspace — SideX`.
pub fn format_title(filename: Option<&str>, workspace: Option<&Path>) -> String {
    let ws_name = workspace
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str());

    match (filename, ws_name) {
        (Some(f), Some(w)) => format!("{f} — {w} — SideX"),
        (Some(f), None) => format!("{f} — SideX"),
        (None, Some(w)) => format!("{w} — SideX"),
        (None, None) => "SideX".to_owned(),
    }
}

// ── Helper ───────────────────────────────────────────────────────────────────

trait OptionalExt<T> {
    fn optional(self) -> rusqlite::Result<Option<T>>;
}

impl<T> OptionalExt<T> for rusqlite::Result<T> {
    fn optional(self) -> rusqlite::Result<Option<T>> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_title_full() {
        let t = format_title(Some("main.rs"), Some(Path::new("/projects/sidex")));
        assert_eq!(t, "main.rs — sidex — SideX");
    }

    #[test]
    fn format_title_no_file() {
        let t = format_title(None, Some(Path::new("/projects/sidex")));
        assert_eq!(t, "sidex — SideX");
    }

    #[test]
    fn format_title_no_workspace() {
        let t = format_title(Some("readme.md"), None);
        assert_eq!(t, "readme.md — SideX");
    }

    #[test]
    fn format_title_bare() {
        assert_eq!(format_title(None, None), "SideX");
    }

    #[test]
    fn window_bounds_default() {
        let b = WindowBounds::default();
        assert_eq!(b.width, 1280);
        assert_eq!(b.height, 720);
    }

    #[test]
    fn window_state_default_is_normal() {
        assert_eq!(WindowState::default(), WindowState::Normal);
    }

    #[test]
    fn app_window_new() {
        let w = AppWindow::new(1, "SideX".into(), None);
        assert_eq!(w.id, 1);
        assert!(!w.is_focused);
        assert!(!w.is_fullscreen);
        assert_eq!(w.state, WindowState::Normal);
    }

    #[test]
    fn persistence_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::open(&tmp.path().join("test.db")).unwrap();
        let records = vec![AppWindow::new(
            1,
            "test — SideX".into(),
            Some(PathBuf::from("/projects/test")),
        )];
        let json = serde_json::to_string(&records).unwrap();
        db.conn()
            .execute(
                "INSERT INTO state_kv (scope, key, value) VALUES ('global', 'window_positions', ?1)
                 ON CONFLICT(scope, key) DO UPDATE SET value = excluded.value",
                rusqlite::params![json],
            )
            .unwrap();

        let restored = WindowManager::restore_window_positions(&db).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].title, "test — SideX");
        assert_eq!(
            restored[0].workspace.as_deref(),
            Some(Path::new("/projects/test"))
        );
    }
}
