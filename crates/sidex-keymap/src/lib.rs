//! Keybinding system for `SideX`.
//!
//! Provides a VS Code-compatible keybinding system with support for key
//! chords, modifier keys, context-aware "when" clauses, and platform-aware
//! default bindings.

pub mod context;
pub mod defaults;
pub mod keyboard_shortcut_editor;
pub mod keybinding;
pub mod resolver;

pub use context::{
    evaluate, keys, parse_when_clause, ContextKeyService, ContextKeys, ContextValue, WhenClause,
    WhenClauseError,
};
pub use defaults::default_keybindings;
pub use keyboard_shortcut_editor::{
    format_keybinding, get_all_keybinding_entries, keybinding_to_json, parse_keybinding_string,
    CommandInfo, KeybindingEntry, KeybindingRecorder,
};
pub use keybinding::{
    ChordState, Key, KeyBinding, KeyChord, KeyCombo, KeybindingMatch, KeybindingSource, Modifiers,
    ResolvedKeybinding,
};
pub use resolver::KeybindingResolver;
