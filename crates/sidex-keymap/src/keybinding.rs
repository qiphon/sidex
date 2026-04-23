//! Core keybinding types: keys, modifiers, combos, chords, and bindings.

use std::fmt;
use std::str::FromStr;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Modifiers ────────────────────────────────────────────────────────────────

/// Modifier key bitflags.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const CTRL: Self = Self(1);
    pub const SHIFT: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    /// Cmd on macOS, Win/Super on other platforms.
    pub const META: Self = Self(1 << 3);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

impl fmt::Display for Modifiers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        let mut write_mod = |name: &str| -> fmt::Result {
            if !first {
                f.write_str("+")?;
            }
            first = false;
            f.write_str(name)
        };
        if self.contains(Self::CTRL) {
            write_mod("Ctrl")?;
        }
        if self.contains(Self::SHIFT) {
            write_mod("Shift")?;
        }
        if self.contains(Self::ALT) {
            write_mod("Alt")?;
        }
        if self.contains(Self::META) {
            write_mod("Meta")?;
        }
        Ok(())
    }
}

// ── Key ──────────────────────────────────────────────────────────────────────

/// A physical or logical key on the keyboard.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Enter,
    Escape,
    Tab,
    Space,
    Backspace,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Pause,
    CapsLock,
    NumLock,
    ScrollLock,
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Backquote,
    Comma,
    Period,
    Slash,
    ContextMenu,
    PrintScreen,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Digit0 => "0",
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::Digit5 => "5",
            Self::Digit6 => "6",
            Self::Digit7 => "7",
            Self::Digit8 => "8",
            Self::Digit9 => "9",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::Enter => "Enter",
            Self::Escape => "Escape",
            Self::Tab => "Tab",
            Self::Space => "Space",
            Self::Backspace => "Backspace",
            Self::Delete => "Delete",
            Self::ArrowUp => "Up",
            Self::ArrowDown => "Down",
            Self::ArrowLeft => "Left",
            Self::ArrowRight => "Right",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",
            Self::Insert => "Insert",
            Self::Pause => "Pause",
            Self::CapsLock => "CapsLock",
            Self::NumLock => "NumLock",
            Self::ScrollLock => "ScrollLock",
            Self::Minus => "-",
            Self::Equal => "=",
            Self::BracketLeft => "[",
            Self::BracketRight => "]",
            Self::Backslash => "\\",
            Self::Semicolon => ";",
            Self::Quote => "'",
            Self::Backquote => "`",
            Self::Comma => ",",
            Self::Period => ".",
            Self::Slash => "/",
            Self::ContextMenu => "ContextMenu",
            Self::PrintScreen => "PrintScreen",
        };
        f.write_str(s)
    }
}

/// Error returned when a key name string cannot be parsed.
#[derive(Debug, thiserror::Error)]
#[error("unknown key: {0}")]
pub struct UnknownKey(String);

impl FromStr for Key {
    type Err = UnknownKey;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "a" => Ok(Self::A),
            "b" => Ok(Self::B),
            "c" => Ok(Self::C),
            "d" => Ok(Self::D),
            "e" => Ok(Self::E),
            "f" => Ok(Self::F),
            "g" => Ok(Self::G),
            "h" => Ok(Self::H),
            "i" => Ok(Self::I),
            "j" => Ok(Self::J),
            "k" => Ok(Self::K),
            "l" => Ok(Self::L),
            "m" => Ok(Self::M),
            "n" => Ok(Self::N),
            "o" => Ok(Self::O),
            "p" => Ok(Self::P),
            "q" => Ok(Self::Q),
            "r" => Ok(Self::R),
            "s" => Ok(Self::S),
            "t" => Ok(Self::T),
            "u" => Ok(Self::U),
            "v" => Ok(Self::V),
            "w" => Ok(Self::W),
            "x" => Ok(Self::X),
            "y" => Ok(Self::Y),
            "z" => Ok(Self::Z),
            "0" => Ok(Self::Digit0),
            "1" => Ok(Self::Digit1),
            "2" => Ok(Self::Digit2),
            "3" => Ok(Self::Digit3),
            "4" => Ok(Self::Digit4),
            "5" => Ok(Self::Digit5),
            "6" => Ok(Self::Digit6),
            "7" => Ok(Self::Digit7),
            "8" => Ok(Self::Digit8),
            "9" => Ok(Self::Digit9),
            "f1" => Ok(Self::F1),
            "f2" => Ok(Self::F2),
            "f3" => Ok(Self::F3),
            "f4" => Ok(Self::F4),
            "f5" => Ok(Self::F5),
            "f6" => Ok(Self::F6),
            "f7" => Ok(Self::F7),
            "f8" => Ok(Self::F8),
            "f9" => Ok(Self::F9),
            "f10" => Ok(Self::F10),
            "f11" => Ok(Self::F11),
            "f12" => Ok(Self::F12),
            "enter" | "return" => Ok(Self::Enter),
            "escape" | "esc" => Ok(Self::Escape),
            "tab" => Ok(Self::Tab),
            "space" => Ok(Self::Space),
            "backspace" => Ok(Self::Backspace),
            "delete" | "del" => Ok(Self::Delete),
            "up" | "arrowup" => Ok(Self::ArrowUp),
            "down" | "arrowdown" => Ok(Self::ArrowDown),
            "left" | "arrowleft" => Ok(Self::ArrowLeft),
            "right" | "arrowright" => Ok(Self::ArrowRight),
            "home" => Ok(Self::Home),
            "end" => Ok(Self::End),
            "pageup" => Ok(Self::PageUp),
            "pagedown" => Ok(Self::PageDown),
            "insert" => Ok(Self::Insert),
            "pause" => Ok(Self::Pause),
            "capslock" => Ok(Self::CapsLock),
            "numlock" => Ok(Self::NumLock),
            "scrolllock" => Ok(Self::ScrollLock),
            "-" | "minus" => Ok(Self::Minus),
            "=" | "equal" => Ok(Self::Equal),
            "[" | "bracketleft" => Ok(Self::BracketLeft),
            "]" | "bracketright" => Ok(Self::BracketRight),
            "\\" | "backslash" => Ok(Self::Backslash),
            ";" | "semicolon" => Ok(Self::Semicolon),
            "'" | "quote" => Ok(Self::Quote),
            "`" | "backquote" => Ok(Self::Backquote),
            "," | "comma" => Ok(Self::Comma),
            "." | "period" => Ok(Self::Period),
            "/" | "slash" => Ok(Self::Slash),
            "contextmenu" => Ok(Self::ContextMenu),
            "printscreen" => Ok(Self::PrintScreen),
            other => Err(UnknownKey(other.to_owned())),
        }
    }
}

// ── KeyCombo ─────────────────────────────────────────────────────────────────

/// A single key press with optional modifier keys (e.g. `Ctrl+Shift+S`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyCombo {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl KeyCombo {
    pub const fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Parse a combo from a string like `"Ctrl+Shift+S"`.
    pub fn parse(s: &str) -> Result<Self, UnknownKey> {
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        let mut modifiers = Modifiers::NONE;
        let mut key_part = None;

        for part in &parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= Modifiers::CTRL,
                "shift" => modifiers |= Modifiers::SHIFT,
                "alt" | "option" => modifiers |= Modifiers::ALT,
                "meta" | "cmd" | "command" | "win" | "super" => modifiers |= Modifiers::META,
                _ => key_part = Some(*part),
            }
        }

        let key: Key = key_part.ok_or_else(|| UnknownKey(s.to_owned()))?.parse()?;

        Ok(Self { key, modifiers })
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.is_empty() {
            write!(f, "{}", self.key)
        } else {
            write!(f, "{}+{}", self.modifiers, self.key)
        }
    }
}

// ── KeyChord ─────────────────────────────────────────────────────────────────

/// A chord of one or two key combos (e.g. `Ctrl+K Ctrl+C`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyChord {
    pub parts: Vec<KeyCombo>,
}

impl KeyChord {
    /// Single-combo chord.
    pub fn single(combo: KeyCombo) -> Self {
        Self { parts: vec![combo] }
    }

    /// Two-combo chord.
    pub fn double(first: KeyCombo, second: KeyCombo) -> Self {
        Self {
            parts: vec![first, second],
        }
    }

    /// Parse a chord string like `"Ctrl+K Ctrl+C"`.
    pub fn parse(s: &str) -> Result<Self, UnknownKey> {
        let parts: Result<Vec<_>, _> = s.split_whitespace().map(KeyCombo::parse).collect();
        let parts = parts?;
        if parts.is_empty() {
            return Err(UnknownKey(s.to_owned()));
        }
        Ok(Self { parts })
    }

    /// Returns `true` if this is a multi-combo chord.
    pub fn is_chord(&self) -> bool {
        self.parts.len() > 1
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

// ── KeyBinding ───────────────────────────────────────────────────────────────

/// A complete keybinding: maps a key chord to a command with an optional
/// "when" clause and arguments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyBinding {
    pub key: KeyChord,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
}

impl KeyBinding {
    pub fn new(key: KeyChord, command: impl Into<String>) -> Self {
        Self {
            key,
            command: command.into(),
            when: None,
            args: None,
        }
    }

    #[must_use]
    pub fn with_when(mut self, when: impl Into<String>) -> Self {
        self.when = Some(when.into());
        self
    }

    #[must_use]
    pub fn with_args(mut self, args: Value) -> Self {
        self.args = Some(args);
        self
    }

    /// Returns `true` if this is a negative binding (command starts with `-`).
    pub fn is_removal(&self) -> bool {
        self.command.starts_with('-')
    }

    /// For a negative binding like `-editor.action.deleteLines`, return the
    /// command being removed (without the leading `-`).
    pub fn removal_target(&self) -> Option<&str> {
        self.command.strip_prefix('-')
    }
}

// ── KeybindingSource ────────────────────────────────────────────────────────

/// Where a keybinding was defined — affects priority ordering.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeybindingSource {
    #[default]
    Default,
    User,
    Extension(String),
}

impl fmt::Display for KeybindingSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => f.write_str("Default"),
            Self::User => f.write_str("User"),
            Self::Extension(name) => write!(f, "Extension({name})"),
        }
    }
}

// ── ResolvedKeybinding ──────────────────────────────────────────────────────

/// A fully-resolved keybinding with parsed key chord, evaluated source, and
/// optional parsed when clause.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolvedKeybinding {
    pub keys: Vec<KeyCombo>,
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    pub source: KeybindingSource,
    pub is_default: bool,
}

impl ResolvedKeybinding {
    pub fn from_binding(binding: &KeyBinding, source: KeybindingSource, is_default: bool) -> Self {
        Self {
            keys: binding.key.parts.clone(),
            command: binding.command.clone(),
            args: binding.args.clone(),
            when: binding.when.clone(),
            source,
            is_default,
        }
    }

    pub fn is_chord(&self) -> bool {
        self.keys.len() > 1
    }

    /// Returns `true` if this is a negative binding.
    pub fn is_removal(&self) -> bool {
        self.command.starts_with('-')
    }
}

// ── ChordState ──────────────────────────────────────────────────────────────

/// Tracks partial chord state — after the first combo of a multi-key chord
/// is pressed, we wait up to 3 seconds for the second combo.
pub struct ChordState {
    pub first_combo: KeyCombo,
    pub timeout: Instant,
}

impl ChordState {
    /// Default chord timeout (3 seconds).
    pub const TIMEOUT_SECS: u64 = 3;

    pub fn new(first_combo: KeyCombo) -> Self {
        Self {
            first_combo,
            timeout: Instant::now() + std::time::Duration::from_secs(Self::TIMEOUT_SECS),
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() > self.timeout
    }
}

// ── KeybindingMatch ─────────────────────────────────────────────────────────

/// The result of resolving a key press against the keybinding table.
#[derive(Clone, Debug)]
pub enum KeybindingMatch {
    /// No matching binding found.
    None,
    /// First chord of a multi-key binding was matched; waiting for second key.
    PartialChord,
    /// A binding was fully resolved.
    Full {
        command: String,
        args: Option<Value>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_combo() {
        let combo = KeyCombo::parse("Ctrl+S").unwrap();
        assert_eq!(combo.key, Key::S);
        assert!(combo.modifiers.contains(Modifiers::CTRL));
        assert!(!combo.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn parse_multi_modifier() {
        let combo = KeyCombo::parse("Ctrl+Shift+P").unwrap();
        assert_eq!(combo.key, Key::P);
        assert!(combo.modifiers.contains(Modifiers::CTRL));
        assert!(combo.modifiers.contains(Modifiers::SHIFT));
    }

    #[test]
    fn parse_meta() {
        let combo = KeyCombo::parse("Cmd+C").unwrap();
        assert!(combo.modifiers.contains(Modifiers::META));
    }

    #[test]
    fn parse_chord() {
        let chord = KeyChord::parse("Ctrl+K Ctrl+C").unwrap();
        assert!(chord.is_chord());
        assert_eq!(chord.parts.len(), 2);
        assert_eq!(chord.parts[0].key, Key::K);
        assert_eq!(chord.parts[1].key, Key::C);
    }

    #[test]
    fn display_combo() {
        let combo = KeyCombo::new(Key::S, Modifiers::CTRL | Modifiers::SHIFT);
        let s = combo.to_string();
        assert!(s.contains("Ctrl"));
        assert!(s.contains("Shift"));
        assert!(s.contains("S"));
    }

    #[test]
    fn display_chord() {
        let chord = KeyChord::parse("Ctrl+K Ctrl+C").unwrap();
        let s = chord.to_string();
        assert!(s.contains(' '));
    }

    #[test]
    fn key_from_str_aliases() {
        assert_eq!("esc".parse::<Key>().unwrap(), Key::Escape);
        assert_eq!("return".parse::<Key>().unwrap(), Key::Enter);
        assert_eq!("del".parse::<Key>().unwrap(), Key::Delete);
    }

    #[test]
    fn unknown_key_error() {
        assert!("foobar".parse::<Key>().is_err());
    }

    #[test]
    fn single_chord_not_chord() {
        let chord = KeyChord::single(KeyCombo::new(Key::A, Modifiers::NONE));
        assert!(!chord.is_chord());
    }

    #[test]
    fn keybinding_with_args() {
        let binding = KeyBinding::new(
            KeyChord::single(KeyCombo::new(Key::F, Modifiers::CTRL)),
            "actions.find",
        )
        .with_args(serde_json::json!({"query": "test"}));
        assert!(binding.args.is_some());
    }

    #[test]
    fn negative_binding() {
        let binding = KeyBinding::new(
            KeyChord::single(KeyCombo::new(Key::K, Modifiers::CTRL)),
            "-editor.action.deleteLines",
        );
        assert!(binding.is_removal());
        assert_eq!(binding.removal_target(), Some("editor.action.deleteLines"));
    }

    #[test]
    fn non_negative_binding() {
        let binding = KeyBinding::new(
            KeyChord::single(KeyCombo::new(Key::S, Modifiers::CTRL)),
            "workbench.action.files.save",
        );
        assert!(!binding.is_removal());
        assert!(binding.removal_target().is_none());
    }

    #[test]
    fn keybinding_source_display() {
        assert_eq!(KeybindingSource::Default.to_string(), "Default");
        assert_eq!(KeybindingSource::User.to_string(), "User");
        assert_eq!(
            KeybindingSource::Extension("rust-analyzer".into()).to_string(),
            "Extension(rust-analyzer)"
        );
    }

    #[test]
    fn resolved_keybinding_from_binding() {
        let binding = KeyBinding::new(
            KeyChord::parse("Ctrl+K Ctrl+C").unwrap(),
            "editor.action.addCommentLine",
        )
        .with_when("editorTextFocus");
        let resolved = ResolvedKeybinding::from_binding(&binding, KeybindingSource::Default, true);
        assert_eq!(resolved.keys.len(), 2);
        assert!(resolved.is_chord());
        assert!(resolved.is_default);
        assert_eq!(resolved.command, "editor.action.addCommentLine");
    }
}
