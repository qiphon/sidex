//! Service container and application context.
//!
//! Provides a lightweight dependency-injection container (`ServiceContainer`)
//! and the top-level `AppContext` that wires together the event bus, command
//! registry, and all registered services.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::commands::CommandRegistry;
use crate::event_bus::EventBus;

// ── Service container ───────────────────────────────────────────

/// Type-erased service locator. Each concrete service type can be
/// registered at most once and retrieved by `TypeId`.
pub struct ServiceContainer {
    services: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl ServiceContainer {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register a service instance. Overwrites any previous registration
    /// for the same concrete type.
    pub fn register<T: 'static + Send + Sync>(&mut self, service: T) {
        self.services.insert(TypeId::of::<T>(), Box::new(service));
    }

    /// Retrieve a shared reference to a previously registered service.
    pub fn get<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.services
            .get(&TypeId::of::<T>())
            .and_then(|b| b.downcast_ref::<T>())
    }

    /// Retrieve a mutable reference to a previously registered service.
    pub fn get_mut<T: 'static + Send + Sync>(&mut self) -> Option<&mut T> {
        self.services
            .get_mut(&TypeId::of::<T>())
            .and_then(|b| b.downcast_mut::<T>())
    }

    /// Returns `true` if a service of type `T` is registered.
    pub fn has<T: 'static + Send + Sync>(&self) -> bool {
        self.services.contains_key(&TypeId::of::<T>())
    }

    /// Remove and return a previously registered service.
    pub fn remove<T: 'static + Send + Sync>(&mut self) -> Option<T> {
        self.services
            .remove(&TypeId::of::<T>())
            .and_then(|b| b.downcast::<T>().ok())
            .map(|b| *b)
    }

    /// Returns the number of registered services.
    pub fn len(&self) -> usize {
        self.services.len()
    }

    /// Returns `true` if no services are registered.
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
}

impl Default for ServiceContainer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Concrete service marker types ───────────────────────────────
//
// Thin wrappers so each logical service has a distinct `TypeId` even
// when the inner state is trivial. Extensions and subsystems can
// later add richer state to these.

macro_rules! define_service {
    ($($(#[doc = $doc:literal])* $name:ident),+ $(,)?) => {
        $(
            $(#[doc = $doc])*
            pub struct $name {
                pub enabled: bool,
            }

            impl $name {
                pub fn new() -> Self {
                    Self { enabled: true }
                }
            }

            impl Default for $name {
                fn default() -> Self {
                    Self::new()
                }
            }
        )+
    };
}

define_service!(
    /// Manages the active colour theme.
    ThemeService,
    /// Reads / writes user and workspace settings.
    SettingsService,
    /// Manages keybinding resolution and customisation.
    KeybindingService,
    /// Tracks the open workspace and its folders.
    WorkspaceService,
    /// File-system operations (read, write, watch).
    FileService,
    /// Full-text search across the workspace.
    SearchService,
    /// Manages open editor instances and their state.
    EditorService,
    /// Manages integrated terminal instances.
    TerminalService,
    /// Hosts and manages editor extensions.
    ExtensionService,
    /// Language intelligence (LSP, tree-sitter).
    LanguageService,
    /// Git integration (status, diff, blame).
    GitService,
    /// Debug adapter protocol integration.
    DebugService,
    /// Task runner (npm, cargo, make, etc.).
    TaskService,
    /// Anonymous usage telemetry.
    TelemetryService,
    /// Structured application logging.
    LogService,
    /// SQLite-backed persistent state.
    DatabaseService,
    /// System clipboard access.
    ClipboardService,
    /// Toast / notification UI.
    NotificationService,
    /// Native dialog (open file, save, message box).
    DialogService,
    /// Internationalization / locale handling.
    I18nService,
    /// Application update checks.
    UpdateService,
);

// ── AppContext ───────────────────────────────────────────────────

/// Top-level application context combining the service container with
/// the event bus and command registry.
pub struct AppContext {
    pub services: ServiceContainer,
    pub event_bus: EventBus,
    pub commands: CommandRegistry,
}

impl AppContext {
    pub fn new() -> Self {
        let mut ctx = Self {
            services: ServiceContainer::new(),
            event_bus: EventBus::new(),
            commands: CommandRegistry::new(),
        };
        ctx.register_default_services();
        ctx
    }

    /// Registers all built-in services with default configuration.
    fn register_default_services(&mut self) {
        self.services.register(ThemeService::new());
        self.services.register(SettingsService::new());
        self.services.register(KeybindingService::new());
        self.services.register(WorkspaceService::new());
        self.services.register(FileService::new());
        self.services.register(SearchService::new());
        self.services.register(EditorService::new());
        self.services.register(TerminalService::new());
        self.services.register(ExtensionService::new());
        self.services.register(LanguageService::new());
        self.services.register(GitService::new());
        self.services.register(DebugService::new());
        self.services.register(TaskService::new());
        self.services.register(TelemetryService::new());
        self.services.register(LogService::new());
        self.services.register(DatabaseService::new());
        self.services.register(ClipboardService::new());
        self.services.register(NotificationService::new());
        self.services.register(DialogService::new());
        self.services.register(I18nService::new());
        self.services.register(UpdateService::new());
    }

    /// Convenience: retrieve a service by type.
    pub fn service<T: 'static + Send + Sync>(&self) -> Option<&T> {
        self.services.get::<T>()
    }

    /// Convenience: retrieve a mutable service by type.
    pub fn service_mut<T: 'static + Send + Sync>(&mut self) -> Option<&mut T> {
        self.services.get_mut::<T>()
    }
}

impl Default for AppContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_retrieve() {
        let mut c = ServiceContainer::new();
        c.register(42_u32);
        assert_eq!(c.get::<u32>(), Some(&42));
    }

    #[test]
    fn get_mut_works() {
        let mut c = ServiceContainer::new();
        c.register(10_i64);
        if let Some(v) = c.get_mut::<i64>() {
            *v = 20;
        }
        assert_eq!(c.get::<i64>(), Some(&20));
    }

    #[test]
    fn missing_returns_none() {
        let c = ServiceContainer::new();
        assert!(c.get::<String>().is_none());
    }

    #[test]
    fn has_checks_presence() {
        let mut c = ServiceContainer::new();
        assert!(!c.has::<u8>());
        c.register(1_u8);
        assert!(c.has::<u8>());
    }

    #[test]
    fn remove_returns_value() {
        let mut c = ServiceContainer::new();
        c.register(String::from("hello"));
        let v = c.remove::<String>();
        assert_eq!(v.as_deref(), Some("hello"));
        assert!(!c.has::<String>());
    }

    #[test]
    fn app_context_has_default_services() {
        let ctx = AppContext::new();
        assert!(ctx.services.has::<ThemeService>());
        assert!(ctx.services.has::<SettingsService>());
        assert!(ctx.services.has::<KeybindingService>());
        assert!(ctx.services.has::<WorkspaceService>());
        assert!(ctx.services.has::<FileService>());
        assert!(ctx.services.has::<SearchService>());
        assert!(ctx.services.has::<EditorService>());
        assert!(ctx.services.has::<TerminalService>());
        assert!(ctx.services.has::<ExtensionService>());
        assert!(ctx.services.has::<LanguageService>());
        assert!(ctx.services.has::<GitService>());
        assert!(ctx.services.has::<DebugService>());
        assert!(ctx.services.has::<TaskService>());
        assert!(ctx.services.has::<TelemetryService>());
        assert!(ctx.services.has::<LogService>());
        assert!(ctx.services.has::<DatabaseService>());
        assert!(ctx.services.has::<ClipboardService>());
        assert!(ctx.services.has::<NotificationService>());
        assert!(ctx.services.has::<DialogService>());
        assert!(ctx.services.has::<I18nService>());
        assert!(ctx.services.has::<UpdateService>());
        assert_eq!(ctx.services.len(), 21);
    }

    #[test]
    fn service_convenience_methods() {
        let mut ctx = AppContext::new();
        assert!(ctx.service::<ThemeService>().unwrap().enabled);
        ctx.service_mut::<ThemeService>().unwrap().enabled = false;
        assert!(!ctx.service::<ThemeService>().unwrap().enabled);
    }
}
