//! Application-wide event bus.
//!
//! Provides a publish-subscribe event system modelled after VS Code's rich
//! event infrastructure. Every subsystem can emit events and any number of
//! listeners can subscribe to them.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

// ── Event types ──────────────────────────────────────────────────

/// Categories of events the editor can produce.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EventType {
    // File events
    FileCreated,
    FileChanged,
    FileDeleted,
    FileRenamed,
    FileSaved,
    FileOpened,
    FileClosed,

    // Editor events
    ActiveEditorChanged,
    EditorSelectionChanged,
    EditorScrollChanged,
    EditorVisibleRangesChanged,
    TextDocumentChanged,

    // Workspace events
    WorkspaceOpened,
    WorkspaceClosed,
    FoldersChanged,
    ConfigurationChanged,

    // Window events
    WindowFocusChanged,
    WindowStateChanged,
    ColorThemeChanged,

    // Terminal events
    TerminalCreated,
    TerminalClosed,
    TerminalDataWritten,

    // Extension events
    ExtensionInstalled,
    ExtensionUninstalled,
    ExtensionEnabled,
    ExtensionDisabled,
    ExtensionActivated,

    // Debug events
    DebugSessionStarted,
    DebugSessionStopped,
    BreakpointChanged,

    // Git events
    GitStatusChanged,
    GitBranchChanged,
    GitRepositoryChanged,

    // Task events
    TaskStarted,
    TaskCompleted,
    TaskFailed,

    // Custom (from extensions)
    Custom(String),
}

/// A single event instance.
#[derive(Debug, Clone)]
pub struct Event {
    pub event_type: EventType,
    pub data: Value,
    pub timestamp: Instant,
    pub source: String,
}

impl Event {
    pub fn new(event_type: EventType, source: impl Into<String>) -> Self {
        Self {
            event_type,
            data: Value::Null,
            timestamp: Instant::now(),
            source: source.into(),
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }
}

// ── Listener bookkeeping ────────────────────────────────────────

/// Opaque handle returned by [`EventBus::on`] / [`EventBus::once`].
/// Pass it to [`EventBus::off`] to unsubscribe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ListenerId(u64);

static NEXT_LISTENER_ID: AtomicU64 = AtomicU64::new(1);

fn next_id() -> ListenerId {
    ListenerId(NEXT_LISTENER_ID.fetch_add(1, Ordering::Relaxed))
}

pub type EventListener = Box<dyn Fn(&Event) + Send + Sync>;

struct ListenerEntry {
    id: ListenerId,
    callback: Arc<EventListener>,
    once: bool,
}

// ── EventBus ────────────────────────────────────────────────────

/// Central publish-subscribe event bus.
pub struct EventBus {
    listeners: HashMap<EventType, Vec<ListenerEntry>>,
    pending_events: VecDeque<Event>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            listeners: HashMap::new(),
            pending_events: VecDeque::new(),
        }
    }

    /// Subscribe to an event type. Returns a [`ListenerId`] that can be
    /// used to unsubscribe later.
    pub fn on(&mut self, event_type: EventType, listener: EventListener) -> ListenerId {
        let id = next_id();
        self.listeners
            .entry(event_type)
            .or_default()
            .push(ListenerEntry {
                id,
                callback: Arc::new(listener),
                once: false,
            });
        id
    }

    /// Subscribe to an event type for a single invocation. The listener is
    /// automatically removed after the first matching event is dispatched.
    pub fn once(&mut self, event_type: EventType, listener: EventListener) -> ListenerId {
        let id = next_id();
        self.listeners
            .entry(event_type)
            .or_default()
            .push(ListenerEntry {
                id,
                callback: Arc::new(listener),
                once: true,
            });
        id
    }

    /// Remove a previously registered listener by its id.
    pub fn off(&mut self, id: ListenerId) {
        for entries in self.listeners.values_mut() {
            entries.retain(|e| e.id != id);
        }
    }

    /// Synchronously emit an event, invoking all matching listeners
    /// immediately.
    pub fn emit(&mut self, event: Event) {
        if let Some(entries) = self.listeners.get(&event.event_type) {
            let callbacks: Vec<(Arc<EventListener>, bool)> = entries
                .iter()
                .map(|e| (Arc::clone(&e.callback), e.once))
                .collect();

            for (cb, _) in &callbacks {
                cb(&event);
            }

            if let Some(entries) = self.listeners.get_mut(&event.event_type) {
                entries.retain(|e| !e.once);
            }
        }
    }

    /// Queue an event for deferred processing. Call [`drain`] to retrieve
    /// all pending events.
    pub fn emit_async(&mut self, event: Event) {
        self.pending_events.push_back(event);
    }

    /// Drain and return all pending (async-queued) events, clearing the
    /// internal queue.
    pub fn drain(&mut self) -> Vec<Event> {
        self.pending_events.drain(..).collect()
    }

    /// Process all pending events by dispatching them to listeners.
    pub fn flush(&mut self) {
        let events: Vec<Event> = self.pending_events.drain(..).collect();
        for event in events {
            self.emit(event);
        }
    }

    /// Returns the number of listeners registered for a given event type.
    pub fn listener_count(&self, event_type: &EventType) -> usize {
        self.listeners
            .get(event_type)
            .map_or(0, Vec::len)
    }

    /// Returns true if there are pending events waiting to be flushed.
    pub fn has_pending(&self) -> bool {
        !self.pending_events.is_empty()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn emit_invokes_listener() {
        let mut bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        bus.on(EventType::FileSaved, Box::new(move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        }));

        bus.emit(Event::new(EventType::FileSaved, "test"));
        assert_eq!(count.load(Ordering::Relaxed), 1);

        bus.emit(Event::new(EventType::FileSaved, "test"));
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn once_fires_only_once() {
        let mut bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        bus.once(EventType::FileOpened, Box::new(move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        }));

        bus.emit(Event::new(EventType::FileOpened, "test"));
        bus.emit(Event::new(EventType::FileOpened, "test"));
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn off_removes_listener() {
        let mut bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        let id = bus.on(EventType::FileSaved, Box::new(move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        }));

        bus.emit(Event::new(EventType::FileSaved, "test"));
        assert_eq!(count.load(Ordering::Relaxed), 1);

        bus.off(id);
        bus.emit(Event::new(EventType::FileSaved, "test"));
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn async_emit_queues_events() {
        let mut bus = EventBus::new();
        bus.emit_async(Event::new(EventType::FileCreated, "test"));
        bus.emit_async(Event::new(EventType::FileDeleted, "test"));
        assert!(bus.has_pending());

        let events = bus.drain();
        assert_eq!(events.len(), 2);
        assert!(!bus.has_pending());
    }

    #[test]
    fn flush_dispatches_pending() {
        let mut bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        bus.on(EventType::FileSaved, Box::new(move |_| {
            c.fetch_add(1, Ordering::Relaxed);
        }));

        bus.emit_async(Event::new(EventType::FileSaved, "test"));
        bus.emit_async(Event::new(EventType::FileSaved, "test"));
        assert_eq!(count.load(Ordering::Relaxed), 0);

        bus.flush();
        assert_eq!(count.load(Ordering::Relaxed), 2);
        assert!(!bus.has_pending());
    }

    #[test]
    fn listener_count() {
        let mut bus = EventBus::new();
        assert_eq!(bus.listener_count(&EventType::FileSaved), 0);

        let id1 = bus.on(EventType::FileSaved, Box::new(|_| {}));
        let _id2 = bus.on(EventType::FileSaved, Box::new(|_| {}));
        assert_eq!(bus.listener_count(&EventType::FileSaved), 2);

        bus.off(id1);
        assert_eq!(bus.listener_count(&EventType::FileSaved), 1);
    }

    #[test]
    fn custom_event_type() {
        let mut bus = EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&count);
        bus.on(
            EventType::Custom("myext.custom".into()),
            Box::new(move |_| {
                c.fetch_add(1, Ordering::Relaxed);
            }),
        );

        bus.emit(Event::new(EventType::Custom("myext.custom".into()), "ext"));
        assert_eq!(count.load(Ordering::Relaxed), 1);

        bus.emit(Event::new(EventType::Custom("other".into()), "ext"));
        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn event_carries_data() {
        let mut bus = EventBus::new();
        let captured = Arc::new(std::sync::Mutex::new(Value::Null));
        let cap = Arc::clone(&captured);
        bus.on(EventType::FileCreated, Box::new(move |e| {
            *cap.lock().unwrap() = e.data.clone();
        }));

        let data = serde_json::json!({"path": "/tmp/foo.rs"});
        bus.emit(Event::new(EventType::FileCreated, "fs").with_data(data.clone()));
        assert_eq!(*captured.lock().unwrap(), data);
    }
}
