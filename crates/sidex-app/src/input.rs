//! Input event processing — keyboard, mouse, IME, and file drop dispatch.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bitflags::bitflags;

// ── Key codes ────────────────────────────────────────────────────────────────

/// Platform-neutral key codes covering the standard 101/104-key layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Digit0, Digit1, Digit2, Digit3, Digit4,
    Digit5, Digit6, Digit7, Digit8, Digit9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Escape, Tab, CapsLock, Space, Enter, Backspace, Delete, Insert,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Home, End, PageUp, PageDown,
    Minus, Equal, BracketLeft, BracketRight, Backslash,
    Semicolon, Quote, Backquote, Comma, Period, Slash,
    NumLock, ScrollLock, PrintScreen, Pause,
    ContextMenu,
}

// ── Mouse button ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

bitflags! {
    /// Bitmask of currently-pressed mouse buttons.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MouseButtons: u8 {
        const LEFT   = 0b0000_0001;
        const RIGHT  = 0b0000_0010;
        const MIDDLE = 0b0000_0100;
        const BACK   = 0b0000_1000;
        const FORWARD = 0b0001_0000;
    }
}

impl MouseButtons {
    pub fn from_button(button: MouseButton) -> Self {
        match button {
            MouseButton::Left => Self::LEFT,
            MouseButton::Right => Self::RIGHT,
            MouseButton::Middle => Self::MIDDLE,
            MouseButton::Back => Self::BACK,
            MouseButton::Forward => Self::FORWARD,
        }
    }
}

// ── Scroll phase ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPhase {
    Start,
    Update,
    End,
}

// ── Modifier state ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierState {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl ModifierState {
    pub const EMPTY: Self = Self {
        ctrl: false,
        shift: false,
        alt: false,
        meta: false,
    };

    /// The "primary" accelerator key: Cmd on macOS, Ctrl elsewhere.
    pub fn primary_held(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.ctrl
        }
    }

    pub fn any_modifier(&self) -> bool {
        self.ctrl || self.shift || self.alt || self.meta
    }
}

// ── Input events ─────────────────────────────────────────────────────────────

/// Union of all input events the application can receive.
#[derive(Debug, Clone)]
pub enum InputEvent {
    KeyDown {
        key: KeyCode,
        modifiers: ModifierState,
        text: Option<String>,
        is_repeat: bool,
    },
    KeyUp {
        key: KeyCode,
        modifiers: ModifierState,
    },
    MouseDown {
        button: MouseButton,
        position: (f32, f32),
        modifiers: ModifierState,
        click_count: u32,
    },
    MouseUp {
        button: MouseButton,
        position: (f32, f32),
        modifiers: ModifierState,
    },
    MouseMove {
        position: (f32, f32),
        modifiers: ModifierState,
    },
    MouseWheel {
        delta: (f32, f32),
        position: (f32, f32),
        modifiers: ModifierState,
        phase: ScrollPhase,
    },
    ImeCompositionStart,
    ImeCompositionUpdate {
        text: String,
        cursor: usize,
    },
    ImeCompositionEnd {
        text: String,
    },
    FileDrop {
        paths: Vec<PathBuf>,
        position: (f32, f32),
    },
    FocusGained,
    FocusLost,
    Resize {
        width: u32,
        height: u32,
    },
    ScaleFactorChanged {
        scale: f64,
    },
}

// ── Input result ─────────────────────────────────────────────────────────────

/// Outcome of processing a single input event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputResult {
    /// The event was fully consumed.
    Handled,
    /// Nothing recognised the event.
    Unhandled,
    /// A named command should be executed.
    Command(String),
    /// Raw text to insert at the cursor.
    TextInput(String),
}

// ── Drag state ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DragState {
    pub start: (f32, f32),
    pub current: (f32, f32),
    pub button: MouseButton,
    pub dragging: bool,
}

impl DragState {
    fn new(position: (f32, f32), button: MouseButton) -> Self {
        Self {
            start: position,
            current: position,
            button,
            dragging: false,
        }
    }

    /// Pixels the pointer has moved from the drag origin.
    pub fn distance(&self) -> f32 {
        let dx = self.current.0 - self.start.0;
        let dy = self.current.1 - self.start.1;
        (dx * dx + dy * dy).sqrt()
    }
}

// ── IME state ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ImeState {
    pub composing: bool,
    pub composition_text: String,
    pub cursor_position: usize,
}

// ── Keyboard state ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct KeyboardState {
    pub pressed_keys: HashSet<KeyCode>,
    pub repeat_timer: Option<(KeyCode, Instant)>,
    pub repeat_delay: Duration,
    pub repeat_interval: Duration,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            repeat_timer: None,
            repeat_delay: Duration::from_millis(500),
            repeat_interval: Duration::from_millis(33),
        }
    }
}

// ── Mouse state ──────────────────────────────────────────────────────────────

const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(400);
const DOUBLE_CLICK_DISTANCE: f32 = 5.0;
const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Debug, Clone)]
pub struct MouseState {
    pub position: (f32, f32),
    pub buttons: MouseButtons,
    pub click_count: u32,
    pub last_click_time: Instant,
    pub last_click_position: (f32, f32),
    pub drag: Option<DragState>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0),
            buttons: MouseButtons::empty(),
            click_count: 0,
            last_click_time: Instant::now(),
            last_click_position: (0.0, 0.0),
            drag: None,
        }
    }
}

// ── Input handler ────────────────────────────────────────────────────────────

/// Centralised input state machine that tracks keyboard, mouse, IME, and
/// modifier state, then converts raw OS events into [`InputResult`] values
/// the application can act on.
pub struct InputHandler {
    pub keyboard: KeyboardState,
    pub mouse: MouseState,
    pub ime: ImeState,
    pub modifiers: ModifierState,
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl InputHandler {
    pub fn new() -> Self {
        Self {
            keyboard: KeyboardState::default(),
            mouse: MouseState::default(),
            ime: ImeState::default(),
            modifiers: ModifierState::EMPTY,
        }
    }

    /// Top-level event dispatch. Returns an [`InputResult`] indicating what
    /// the application layer should do.
    pub fn process_event(&mut self, event: &InputEvent) -> InputResult {
        match event {
            // ── Keyboard ─────────────────────────────────────
            InputEvent::KeyDown {
                key,
                modifiers,
                text,
                is_repeat,
            } => {
                self.modifiers = *modifiers;
                self.keyboard.pressed_keys.insert(*key);
                if !is_repeat {
                    self.keyboard.repeat_timer = Some((*key, Instant::now()));
                }

                if let Some(cmd) = self.match_key_command(*key, modifiers) {
                    return InputResult::Command(cmd);
                }

                if let Some(t) = text {
                    if !t.is_empty() && !modifiers.primary_held() {
                        return InputResult::TextInput(t.clone());
                    }
                }

                InputResult::Unhandled
            }
            InputEvent::KeyUp { key, modifiers } => {
                self.modifiers = *modifiers;
                self.keyboard.pressed_keys.remove(key);
                if self
                    .keyboard
                    .repeat_timer
                    .as_ref()
                    .is_some_and(|(k, _)| k == key)
                {
                    self.keyboard.repeat_timer = None;
                }
                InputResult::Handled
            }

            // ── Mouse ────────────────────────────────────────
            InputEvent::MouseDown {
                button,
                position,
                modifiers,
                click_count,
            } => {
                self.modifiers = *modifiers;
                self.mouse.buttons |= MouseButtons::from_button(*button);
                self.mouse.position = *position;

                self.update_click_count(*position, *click_count);

                if *button == MouseButton::Left {
                    self.mouse.drag = Some(DragState::new(*position, *button));
                }

                self.process_mouse_down(*button, *position, modifiers)
            }
            InputEvent::MouseUp {
                button,
                position,
                modifiers,
            } => {
                self.modifiers = *modifiers;
                self.mouse.buttons -= MouseButtons::from_button(*button);
                self.mouse.position = *position;

                if self
                    .mouse
                    .drag
                    .as_ref()
                    .is_some_and(|d| d.button == *button)
                {
                    self.mouse.drag = None;
                }

                InputResult::Handled
            }
            InputEvent::MouseMove {
                position,
                modifiers,
            } => {
                self.modifiers = *modifiers;
                self.mouse.position = *position;

                if let Some(drag) = &mut self.mouse.drag {
                    drag.current = *position;
                    if !drag.dragging && drag.distance() > DRAG_THRESHOLD {
                        drag.dragging = true;
                    }
                }

                InputResult::Handled
            }
            InputEvent::MouseWheel {
                delta,
                modifiers,
                ..
            } => {
                self.modifiers = *modifiers;

                if modifiers.primary_held() && delta.1.abs() > f32::EPSILON {
                    if delta.1 > 0.0 {
                        return InputResult::Command("workbench.action.zoomIn".into());
                    }
                    return InputResult::Command("workbench.action.zoomOut".into());
                }

                InputResult::Handled
            }

            // ── IME ──────────────────────────────────────────
            InputEvent::ImeCompositionStart => {
                self.ime.composing = true;
                self.ime.composition_text.clear();
                self.ime.cursor_position = 0;
                InputResult::Handled
            }
            InputEvent::ImeCompositionUpdate { text, cursor } => {
                self.ime.composition_text.clone_from(text);
                self.ime.cursor_position = *cursor;
                InputResult::Handled
            }
            InputEvent::ImeCompositionEnd { text } => {
                self.ime.composing = false;
                self.ime.composition_text.clear();
                self.ime.cursor_position = 0;
                if text.is_empty() {
                    InputResult::Handled
                } else {
                    InputResult::TextInput(text.clone())
                }
            }

            // ── File drop ────────────────────────────────────
            InputEvent::FileDrop { paths, .. } => {
                if paths.len() == 1 {
                    InputResult::Command(format!("_openFile:{}", paths[0].display()))
                } else {
                    InputResult::Handled
                }
            }

            // ── Window lifecycle ─────────────────────────────
            InputEvent::FocusGained | InputEvent::FocusLost => InputResult::Handled,
            InputEvent::Resize { .. } | InputEvent::ScaleFactorChanged { .. } => {
                InputResult::Handled
            }
        }
    }

    /// Whether the user is currently performing a drag selection.
    pub fn is_dragging(&self) -> bool {
        self.mouse
            .drag
            .as_ref()
            .is_some_and(|d| d.dragging)
    }

    /// Whether the IME is mid-composition.
    pub fn is_composing(&self) -> bool {
        self.ime.composing
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    fn update_click_count(&mut self, position: (f32, f32), raw_count: u32) {
        if raw_count > 0 {
            self.mouse.click_count = raw_count;
            self.mouse.last_click_time = Instant::now();
            self.mouse.last_click_position = position;
            return;
        }

        let now = Instant::now();
        let dt = now.duration_since(self.mouse.last_click_time);
        let dx = (position.0 - self.mouse.last_click_position.0).abs();
        let dy = (position.1 - self.mouse.last_click_position.1).abs();
        let dist = (dx * dx + dy * dy).sqrt();

        if dt < DOUBLE_CLICK_THRESHOLD && dist < DOUBLE_CLICK_DISTANCE {
            self.mouse.click_count = (self.mouse.click_count % 3) + 1;
        } else {
            self.mouse.click_count = 1;
        }

        self.mouse.last_click_time = now;
        self.mouse.last_click_position = position;
    }

    fn process_mouse_down(
        &self,
        button: MouseButton,
        _position: (f32, f32),
        modifiers: &ModifierState,
    ) -> InputResult {
        match button {
            MouseButton::Left => {
                if modifiers.alt {
                    return InputResult::Command("editor.action.addCursorAtClick".into());
                }
                if modifiers.primary_held() {
                    return InputResult::Command("editor.action.revealDefinition".into());
                }
                match self.mouse.click_count {
                    2 => InputResult::Command("editor.action.selectWord".into()),
                    3 => InputResult::Command("editor.action.selectLine".into()),
                    _ => InputResult::Handled,
                }
            }
            MouseButton::Right => {
                InputResult::Command("editor.action.showContextMenu".into())
            }
            MouseButton::Middle => {
                if cfg!(target_os = "linux") {
                    InputResult::Command("editor.action.pasteFromSelection".into())
                } else {
                    InputResult::Handled
                }
            }
            MouseButton::Back => {
                InputResult::Command("workbench.action.navigateBack".into())
            }
            MouseButton::Forward => {
                InputResult::Command("workbench.action.navigateForward".into())
            }
        }
    }

    fn match_key_command(&self, key: KeyCode, mods: &ModifierState) -> Option<String> {
        if !mods.any_modifier() {
            return None;
        }

        let cmd = mods.primary_held();
        let shift = mods.shift;
        let alt = mods.alt;

        match (key, cmd, shift, alt) {
            (KeyCode::Equal, true, false, false) => {
                Some("workbench.action.zoomIn".into())
            }
            (KeyCode::Minus, true, false, false) => {
                Some("workbench.action.zoomOut".into())
            }
            (KeyCode::Digit0, true, false, false) => {
                Some("workbench.action.zoomReset".into())
            }
            _ => None,
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_handler_starts_clean() {
        let h = InputHandler::new();
        assert!(h.keyboard.pressed_keys.is_empty());
        assert!(!h.is_dragging());
        assert!(!h.is_composing());
    }

    #[test]
    fn key_down_tracks_pressed() {
        let mut h = InputHandler::new();
        let evt = InputEvent::KeyDown {
            key: KeyCode::A,
            modifiers: ModifierState::EMPTY,
            text: Some("a".into()),
            is_repeat: false,
        };
        let result = h.process_event(&evt);
        assert_eq!(result, InputResult::TextInput("a".into()));
        assert!(h.keyboard.pressed_keys.contains(&KeyCode::A));
    }

    #[test]
    fn key_up_clears_pressed() {
        let mut h = InputHandler::new();
        h.keyboard.pressed_keys.insert(KeyCode::A);
        let evt = InputEvent::KeyUp {
            key: KeyCode::A,
            modifiers: ModifierState::EMPTY,
        };
        h.process_event(&evt);
        assert!(!h.keyboard.pressed_keys.contains(&KeyCode::A));
    }

    #[test]
    fn ctrl_equals_zooms_in() {
        let mut h = InputHandler::new();
        let mods = ModifierState {
            ctrl: true,
            ..Default::default()
        };
        let evt = InputEvent::KeyDown {
            key: KeyCode::Equal,
            modifiers: mods,
            text: None,
            is_repeat: false,
        };
        let result = h.process_event(&evt);
        if cfg!(target_os = "macos") {
            assert_eq!(result, InputResult::Unhandled);
        } else {
            assert_eq!(
                result,
                InputResult::Command("workbench.action.zoomIn".into())
            );
        }
    }

    #[test]
    fn mouse_double_click_selects_word() {
        let mut h = InputHandler::new();
        let pos = (100.0, 200.0);
        let mods = ModifierState::EMPTY;
        let evt = InputEvent::MouseDown {
            button: MouseButton::Left,
            position: pos,
            modifiers: mods,
            click_count: 2,
        };
        let result = h.process_event(&evt);
        assert_eq!(
            result,
            InputResult::Command("editor.action.selectWord".into())
        );
    }

    #[test]
    fn mouse_triple_click_selects_line() {
        let mut h = InputHandler::new();
        let pos = (100.0, 200.0);
        let mods = ModifierState::EMPTY;
        let evt = InputEvent::MouseDown {
            button: MouseButton::Left,
            position: pos,
            modifiers: mods,
            click_count: 3,
        };
        let result = h.process_event(&evt);
        assert_eq!(
            result,
            InputResult::Command("editor.action.selectLine".into())
        );
    }

    #[test]
    fn alt_click_adds_cursor() {
        let mut h = InputHandler::new();
        let mods = ModifierState {
            alt: true,
            ..Default::default()
        };
        let evt = InputEvent::MouseDown {
            button: MouseButton::Left,
            position: (50.0, 50.0),
            modifiers: mods,
            click_count: 1,
        };
        let result = h.process_event(&evt);
        assert_eq!(
            result,
            InputResult::Command("editor.action.addCursorAtClick".into())
        );
    }

    #[test]
    fn right_click_shows_context_menu() {
        let mut h = InputHandler::new();
        let evt = InputEvent::MouseDown {
            button: MouseButton::Right,
            position: (10.0, 10.0),
            modifiers: ModifierState::EMPTY,
            click_count: 1,
        };
        let result = h.process_event(&evt);
        assert_eq!(
            result,
            InputResult::Command("editor.action.showContextMenu".into())
        );
    }

    #[test]
    fn drag_detection() {
        let mut h = InputHandler::new();
        let down = InputEvent::MouseDown {
            button: MouseButton::Left,
            position: (100.0, 100.0),
            modifiers: ModifierState::EMPTY,
            click_count: 1,
        };
        h.process_event(&down);
        assert!(!h.is_dragging());

        let far_move = InputEvent::MouseMove {
            position: (110.0, 110.0),
            modifiers: ModifierState::EMPTY,
        };
        h.process_event(&far_move);
        assert!(h.is_dragging());
    }

    #[test]
    fn ime_composition_lifecycle() {
        let mut h = InputHandler::new();

        h.process_event(&InputEvent::ImeCompositionStart);
        assert!(h.is_composing());

        h.process_event(&InputEvent::ImeCompositionUpdate {
            text: "ni".into(),
            cursor: 2,
        });
        assert_eq!(h.ime.composition_text, "ni");

        let result = h.process_event(&InputEvent::ImeCompositionEnd {
            text: "\u{4f60}".into(),
        });
        assert!(!h.is_composing());
        assert_eq!(result, InputResult::TextInput("\u{4f60}".into()));
    }

    #[test]
    fn ctrl_scroll_zooms() {
        let mut h = InputHandler::new();
        let mods = ModifierState {
            ctrl: true,
            ..Default::default()
        };
        let evt = InputEvent::MouseWheel {
            delta: (0.0, 3.0),
            position: (0.0, 0.0),
            modifiers: mods,
            phase: ScrollPhase::Update,
        };
        let result = h.process_event(&evt);
        if cfg!(target_os = "macos") {
            assert_eq!(result, InputResult::Handled);
        } else {
            assert_eq!(
                result,
                InputResult::Command("workbench.action.zoomIn".into())
            );
        }
    }

    #[test]
    fn mouse_back_forward_navigate() {
        let mut h = InputHandler::new();
        let back = InputEvent::MouseDown {
            button: MouseButton::Back,
            position: (0.0, 0.0),
            modifiers: ModifierState::EMPTY,
            click_count: 1,
        };
        assert_eq!(
            h.process_event(&back),
            InputResult::Command("workbench.action.navigateBack".into())
        );

        let fwd = InputEvent::MouseDown {
            button: MouseButton::Forward,
            position: (0.0, 0.0),
            modifiers: ModifierState::EMPTY,
            click_count: 1,
        };
        assert_eq!(
            h.process_event(&fwd),
            InputResult::Command("workbench.action.navigateForward".into())
        );
    }

    #[test]
    fn file_drop_single() {
        let mut h = InputHandler::new();
        let evt = InputEvent::FileDrop {
            paths: vec![PathBuf::from("/tmp/test.rs")],
            position: (0.0, 0.0),
        };
        let result = h.process_event(&evt);
        assert_eq!(
            result,
            InputResult::Command("_openFile:/tmp/test.rs".into())
        );
    }

    #[test]
    fn modifier_state_primary() {
        let mac_meta = ModifierState {
            meta: true,
            ..Default::default()
        };
        let ctrl = ModifierState {
            ctrl: true,
            ..Default::default()
        };

        if cfg!(target_os = "macos") {
            assert!(mac_meta.primary_held());
            assert!(!ctrl.primary_held());
        } else {
            assert!(!mac_meta.primary_held());
            assert!(ctrl.primary_held());
        }
    }

    #[test]
    fn mouse_buttons_bitflags() {
        let mut btns = MouseButtons::empty();
        btns |= MouseButtons::LEFT;
        btns |= MouseButtons::RIGHT;
        assert!(btns.contains(MouseButtons::LEFT));
        assert!(btns.contains(MouseButtons::RIGHT));
        assert!(!btns.contains(MouseButtons::MIDDLE));
        btns -= MouseButtons::LEFT;
        assert!(!btns.contains(MouseButtons::LEFT));
    }
}
