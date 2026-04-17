//! Core widget trait and UI event types.
//!
//! Every UI element implements [`Widget`], providing methods for layout,
//! rendering, and event handling.

use crate::layout::{LayoutNode, Rect};
use sidex_gpu::GpuRenderer;

// ── Events ───────────────────────────────────────────────────────────────────

/// Mouse button identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Keyboard modifier flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };

    /// Returns `true` if the platform command key is held (Ctrl on
    /// Linux/Windows, Meta on macOS).
    pub fn command(self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.ctrl
        }
    }
}

/// A key identifier for keyboard events.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Space,
    /// Function keys F1–F12.
    F(u8),
}

/// Events that widgets can receive.
#[derive(Clone, Debug)]
pub enum UiEvent {
    MouseDown { x: f32, y: f32, button: MouseButton },
    MouseUp { x: f32, y: f32, button: MouseButton },
    MouseMove { x: f32, y: f32 },
    MouseScroll { dx: f32, dy: f32 },
    DoubleClick { x: f32, y: f32 },
    KeyPress { key: Key, modifiers: Modifiers },
    Focus,
    Blur,
}

/// The result of handling a UI event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResult {
    /// The widget consumed the event; stop propagation.
    Handled,
    /// The widget did not handle the event.
    Ignored,
    /// Move focus to the next widget.
    FocusNext,
    /// Move focus to the previous widget.
    FocusPrev,
}

// ── Widget trait ─────────────────────────────────────────────────────────────

/// The core trait for all UI elements.
///
/// Widgets participate in a three-phase cycle:
/// 1. **Layout** — return a [`LayoutNode`] describing sizing constraints.
/// 2. **Render** — draw into the computed [`Rect`] using the GPU renderer.
/// 3. **Event handling** — react to user input.
pub trait Widget {
    /// Returns the layout description for this widget.
    fn layout(&self) -> LayoutNode;

    /// Renders the widget into `rect` using the GPU renderer.
    fn render(&self, rect: Rect, renderer: &mut GpuRenderer);

    /// Handles a UI event, returning whether the event was consumed.
    fn handle_event(&mut self, event: &UiEvent, rect: Rect) -> EventResult;
}
