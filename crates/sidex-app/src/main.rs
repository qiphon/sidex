//! SideX application entry point.
//!
//! Startup sequence:
//!  1. Parse CLI args (path to open, flags)
//!  2. Initialize logging
//!  3. Load database (recent files, window state)
//!  4. Load user settings
//!  5. Load theme
//!  6. Initialize Tauri bridge (when a Tauri AppHandle is provided)
//!  7. Initialize wgpu renderer on the window
//!  8. Load workspace (if path provided)
//!  9. Restore previous session (open files, cursor positions)
//! 10. Start extension host
//! 11. Enter event loop

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use winit::event_loop::EventLoop;

mod app;
mod backup;
mod cli;
mod clipboard;
mod command_palette;
mod commands;
mod crash_reporter;
mod document_state;
mod editor_group;
mod editor_view;
mod event_loop;
mod file_dialog;
mod i18n;
mod layout;
mod logging;
pub mod native_menu;
mod navigation;
mod product;
mod quick_open;
mod recent;
mod session;
mod symbol_navigation;
pub mod tauri_bridge;
mod telemetry;
mod updater;
pub mod window_manager;

/// SideX — a fast, native code editor.
#[derive(Parser, Debug)]
#[command(name = "sidex", version, about)]
struct Cli {
    /// File or folder to open.
    path: Option<PathBuf>,

    /// Open in a new window (even if SideX is already running).
    #[arg(long)]
    new_window: bool,

    /// Wait for the file to be closed before returning.
    #[arg(long)]
    wait: bool,

    /// Show a diff between two files.
    #[arg(long, num_args = 2, value_names = ["FILE1", "FILE2"])]
    diff: Option<Vec<PathBuf>>,

    /// Start with extensions disabled.
    #[arg(long)]
    disable_extensions: bool,

    /// Override the user data directory.
    #[arg(long)]
    user_data_dir: Option<PathBuf>,

    /// Log level override (trace, debug, info, warn, error).
    #[arg(long, default_value = "info")]
    log_level: String,
}

fn main() {
    // ── 1. Parse CLI args ───────────────────────────────────────
    let cli = Cli::parse();

    // ── 2. Initialize logging ───────────────────────────────────
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cli.log_level))
        .init();
    log::info!("SideX starting");
    log::debug!("CLI args: {cli:?}");

    // ── 3. Load database (recent files, window state) ───────────
    let db = sidex_db::Database::open_default().unwrap_or_else(|e| {
        log::warn!("failed to open state db, using temp: {e}");
        let tmp = std::env::temp_dir().join("sidex-fallback.db");
        sidex_db::Database::open(&tmp).expect("fallback db must open")
    });

    let saved_window_state = sidex_db::load_window_state(&db).ok().flatten();
    if let Some(ref state) = saved_window_state {
        log::debug!(
            "restored window state: {}x{} at ({}, {})",
            state.width,
            state.height,
            state.x,
            state.y,
        );
    }

    // ── 4. Load user settings ───────────────────────────────────
    let mut settings = sidex_settings::Settings::new();
    if let Some(user_settings) = user_settings_path(&cli) {
        if user_settings.exists() {
            if let Err(e) = settings.load_user(&user_settings) {
                log::warn!("failed to load user settings: {e}");
            }
        }
    }

    // ── 5. Load theme ───────────────────────────────────────────
    let theme = sidex_theme::Theme::default_dark();
    log::debug!("loaded theme: default dark");

    // ── 6. Tauri bridge is initialized separately ───────────────
    //
    // When SideX is launched via the Tauri entry point (src-tauri),
    // the `TauriBridge` is constructed from the Tauri `AppHandle`
    // passed during setup. In standalone (winit-only) mode the
    // bridge is not used and native dialogs go through `rfd`.
    //
    // See `sidex_app::tauri_bridge::TauriBridge::init()`.
    log::info!("Tauri bridge deferred to src-tauri entry point");

    // ── 7. Initialize wgpu renderer on the window ───────────────
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let (win_w, win_h) = saved_window_state
        .as_ref()
        .map_or((1280, 720), |s| (s.width, s.height));

    let window_attrs = winit::window::Window::default_attributes()
        .with_title("SideX")
        .with_inner_size(winit::dpi::LogicalSize::new(
            f64::from(win_w),
            f64::from(win_h),
        ));

    #[allow(deprecated)]
    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .expect("failed to create window"),
    );

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

    let mut application = rt
        .block_on(app::App::new(window.clone(), cli.path.as_deref()))
        .expect("failed to initialise application");

    // Inject the pre-loaded subsystems so we don't double-load them.
    application.settings = settings;
    application.theme = theme;
    application.db = db;

    // ── 8. Load workspace (if path provided) ────────────────────
    if let Some(path) = &cli.path {
        if path.is_file() {
            application.open_file(path);
        }
    }

    // ── 9. Restore previous session (open files, cursor positions)
    restore_session(&mut application);

    // ── 10. Start extension host ────────────────────────────────
    if !cli.disable_extensions {
        start_extension_host(&mut application);
    }

    // ── 11. Enter event loop ────────────────────────────────────
    log::info!("entering main event loop");
    event_loop::run(event_loop, &mut application, &window);
}

/// Restore previously open files and cursor positions from the database.
fn restore_session(app: &mut app::App) {
    let recent = match sidex_db::recent::recent_files(&app.db, 50) {
        Ok(files) => files,
        Err(e) => {
            log::warn!("failed to load recent files: {e}");
            return;
        }
    };

    if let Some(state) = sidex_db::load_window_state(&app.db).ok().flatten() {
        if let Some(active_path) = &state.active_editor {
            let path = PathBuf::from(active_path);
            if path.exists() {
                app.open_file(&path);
                log::debug!("restored active editor: {active_path}");
            }
        }
    }

    log::debug!("session has {} recent files", recent.len());
}

/// Boot the extension host (loads installed extensions).
fn start_extension_host(app: &mut app::App) {
    log::info!("starting extension host");

    let installed = app.extension_registry.all();
    log::info!("loaded {} installed extensions", installed.len());
}

/// Resolve the user settings file path, respecting CLI overrides.
fn user_settings_path(cli: &Cli) -> Option<PathBuf> {
    if let Some(ref dir) = cli.user_data_dir {
        return Some(dir.join("settings.json"));
    }
    dirs::config_dir().map(|d| d.join("sidex").join("settings.json"))
}
