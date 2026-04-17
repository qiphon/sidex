//! Keybinding resolution — maps key presses to commands using context-aware
//! matching with "when" clause evaluation, chord state machine, and negative
//! binding support.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::context::{evaluate, ContextKeys};
use crate::defaults::default_keybindings;
use crate::keybinding::{
    ChordState, KeyBinding, KeyChord, KeyCombo, KeybindingMatch, KeybindingSource,
    ResolvedKeybinding,
};

/// Resolves key presses (and chords) to command identifiers by searching
/// through registered keybindings in reverse-priority order.
///
/// Supports:
/// - Single-key bindings and two-key chords (`Ctrl+K Ctrl+C`)
/// - "when" clause context evaluation
/// - Negative bindings (command starting with `-`) to remove earlier bindings
/// - Chord timeout (3 seconds between first and second key)
/// - Source-based priority: User > Extension > Default
#[derive(Clone, Debug)]
pub struct KeybindingResolver {
    bindings: Vec<KeyBinding>,
    sources: Vec<KeybindingSource>,
    chord_first: Option<KeyCombo>,
}

impl Default for KeybindingResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl KeybindingResolver {
    /// Create a resolver pre-loaded with platform-appropriate defaults.
    pub fn new() -> Self {
        let mut resolver = Self {
            bindings: Vec::new(),
            sources: Vec::new(),
            chord_first: None,
        };
        resolver.load_defaults();
        resolver
    }

    /// Load the built-in platform defaults (appends to existing bindings).
    pub fn load_defaults(&mut self) {
        for b in default_keybindings() {
            self.sources.push(KeybindingSource::Default);
            self.bindings.push(b);
        }
    }

    /// Load user keybindings from a JSON file. User bindings take priority
    /// over defaults (they are searched last → highest priority).
    pub fn load_user(&mut self, path: &Path) -> Result<()> {
        let contents =
            std::fs::read_to_string(path).context("failed to read user keybindings file")?;
        let parsed: Vec<UserKeybinding> =
            serde_json::from_str(&contents).context("failed to parse user keybindings JSON")?;

        for entry in parsed {
            let key = match KeyChord::parse(&entry.key) {
                Ok(k) => k,
                Err(e) => {
                    log::warn!("skipping invalid keybinding '{}': {e}", entry.key);
                    continue;
                }
            };
            let binding = KeyBinding {
                key,
                command: entry.command,
                when: entry.when,
                args: entry.args,
            };
            self.sources.push(KeybindingSource::User);
            self.bindings.push(binding);
        }

        Ok(())
    }

    /// Register keybindings contributed by an extension.
    pub fn load_extension(&mut self, extension_id: &str, bindings: Vec<KeyBinding>) {
        let source = KeybindingSource::Extension(extension_id.to_owned());
        for b in bindings {
            self.sources.push(source.clone());
            self.bindings.push(b);
        }
    }

    /// Add a single keybinding (highest priority, user source).
    pub fn add(&mut self, binding: KeyBinding) {
        self.sources.push(KeybindingSource::User);
        self.bindings.push(binding);
    }

    /// Add a single keybinding with explicit source.
    pub fn add_with_source(&mut self, binding: KeyBinding, source: KeybindingSource) {
        self.sources.push(source);
        self.bindings.push(binding);
    }

    // ── Chord state machine ─────────────────────────────────────────────

    /// Process a key press through the chord state machine.
    /// Returns a `KeybindingMatch` indicating whether the key was consumed.
    pub fn process_key(
        &mut self,
        combo: &KeyCombo,
        context: &ContextKeys,
    ) -> KeybindingMatch {
        if let Some(first) = self.chord_first.take() {
            if let Some((cmd, args)) = self.resolve_chord_full(&first, combo, context) {
                return KeybindingMatch::Full {
                    command: cmd.to_owned(),
                    args: args.cloned(),
                };
            }
            return KeybindingMatch::None;
        }

        if self.is_chord_prefix(combo, context) {
            self.chord_first = Some(combo.clone());
            return KeybindingMatch::PartialChord;
        }

        match self.resolve_with_negatives(combo, context) {
            Some((cmd, args)) => KeybindingMatch::Full {
                command: cmd.to_owned(),
                args: args.cloned(),
            },
            None => KeybindingMatch::None,
        }
    }

    /// Reset the chord state (e.g. on timeout or Escape).
    pub fn reset_chord(&mut self) {
        self.chord_first = None;
    }

    /// Check if we're currently waiting for the second key of a chord.
    pub fn in_chord(&self) -> bool {
        self.chord_first.is_some()
    }

    /// Create a `ChordState` snapshot if we're in a chord.
    pub fn chord_state(&self) -> Option<ChordState> {
        self.chord_first.as_ref().map(|c| ChordState::new(c.clone()))
    }

    // ── Resolution with negative binding support ────────────────────────

    /// Resolve a single key combo, respecting negative bindings.
    fn resolve_with_negatives<'a>(
        &'a self,
        key: &KeyCombo,
        context: &ContextKeys,
    ) -> Option<(&'a str, Option<&'a serde_json::Value>)> {
        let mut removed: HashMap<&str, bool> = HashMap::new();

        for (i, b) in self.bindings.iter().enumerate().rev() {
            if b.key.is_chord() || b.key.parts.first() != Some(key) {
                continue;
            }
            if !Self::when_matches(b, context) {
                continue;
            }
            if let Some(target) = b.removal_target() {
                removed.insert(target, true);
                continue;
            }
            if removed.contains_key(b.command.as_str()) {
                continue;
            }
            let _ = i; // used for iteration
            return Some((b.command.as_str(), b.args.as_ref()));
        }
        None
    }

    /// Resolve a single key combo to a command (simple API, ignores args).
    pub fn resolve<'a>(&'a self, key: &KeyCombo, context: &ContextKeys) -> Option<&'a str> {
        self.resolve_with_negatives(key, context).map(|(cmd, _)| cmd)
    }

    /// Resolve a two-combo chord, respecting negative bindings.
    fn resolve_chord_full<'a>(
        &'a self,
        first: &KeyCombo,
        second: &KeyCombo,
        context: &ContextKeys,
    ) -> Option<(&'a str, Option<&'a serde_json::Value>)> {
        let mut removed: HashMap<&str, bool> = HashMap::new();

        for b in self.bindings.iter().rev() {
            if b.key.parts.len() != 2 {
                continue;
            }
            if b.key.parts[0] != *first || b.key.parts[1] != *second {
                continue;
            }
            if !Self::when_matches(b, context) {
                continue;
            }
            if let Some(target) = b.removal_target() {
                removed.insert(target, true);
                continue;
            }
            if removed.contains_key(b.command.as_str()) {
                continue;
            }
            return Some((b.command.as_str(), b.args.as_ref()));
        }
        None
    }

    /// Resolve a two-combo chord to a command (simple API).
    pub fn resolve_chord<'a>(
        &'a self,
        first: &KeyCombo,
        second: &KeyCombo,
        context: &ContextKeys,
    ) -> Option<&'a str> {
        self.resolve_chord_full(first, second, context)
            .map(|(cmd, _)| cmd)
    }

    /// Check if any binding starts with this combo as the first part of a
    /// chord. Used to know when to enter chord-wait mode.
    pub fn is_chord_prefix(&self, combo: &KeyCombo, context: &ContextKeys) -> bool {
        self.bindings
            .iter()
            .filter(|b| b.key.parts.len() >= 2)
            .filter(|b| b.key.parts[0] == *combo)
            .any(|b| Self::when_matches(b, context))
    }

    /// Return all registered bindings (read-only).
    pub fn bindings(&self) -> &[KeyBinding] {
        &self.bindings
    }

    /// Return all resolved keybindings with source information.
    pub fn resolved_bindings(&self) -> Vec<ResolvedKeybinding> {
        self.bindings
            .iter()
            .zip(self.sources.iter())
            .map(|(b, s)| {
                let is_default = matches!(s, KeybindingSource::Default);
                ResolvedKeybinding::from_binding(b, s.clone(), is_default)
            })
            .collect()
    }

    /// Find all bindings that map to a given command.
    pub fn bindings_for_command(&self, command: &str) -> Vec<&KeyBinding> {
        self.bindings
            .iter()
            .filter(|b| b.command == command)
            .collect()
    }

    /// Return the number of registered bindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    fn when_matches(binding: &KeyBinding, context: &ContextKeys) -> bool {
        match &binding.when {
            None => true,
            Some(expr) => evaluate(expr, context),
        }
    }
}

/// Intermediate type for deserializing user `keybindings.json` entries.
#[derive(serde::Deserialize)]
struct UserKeybinding {
    key: String,
    command: String,
    #[serde(default)]
    when: Option<String>,
    #[serde(default)]
    args: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinding::{Key, Modifiers};

    fn primary() -> Modifiers {
        if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CTRL
        }
    }

    #[test]
    fn resolve_ctrl_s() {
        let resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::S, primary());
        let cmd = resolver.resolve(&combo, &ctx);
        assert_eq!(cmd, Some("workbench.action.files.save"));
    }

    #[test]
    fn resolve_ctrl_c() {
        let resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::C, primary());
        let cmd = resolver.resolve(&combo, &ctx);
        assert_eq!(cmd, Some("editor.action.clipboardCopyAction"));
    }

    #[test]
    fn resolve_with_when_clause() {
        let resolver = KeybindingResolver::new();
        let mut ctx = ContextKeys::new();
        ctx.set_bool("editorTextFocus", true);
        let combo = KeyCombo::new(Key::Period, primary());
        let cmd = resolver.resolve(&combo, &ctx);
        assert_eq!(cmd, Some("editor.action.quickFix"));
    }

    #[test]
    fn resolve_when_clause_fails() {
        let resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::Period, primary());
        let cmd = resolver.resolve(&combo, &ctx);
        assert!(cmd.is_none());
    }

    #[test]
    fn resolve_chord() {
        let resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let p = primary();
        let first = KeyCombo::new(Key::K, p);
        let second = KeyCombo::new(Key::C, p);
        let cmd = resolver.resolve_chord(&first, &second, &ctx);
        assert_eq!(cmd, Some("editor.action.addCommentLine"));
    }

    #[test]
    fn is_chord_prefix_true() {
        let resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::K, primary());
        assert!(resolver.is_chord_prefix(&combo, &ctx));
    }

    #[test]
    fn is_chord_prefix_false() {
        let resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::Q, Modifiers::ALT);
        assert!(!resolver.is_chord_prefix(&combo, &ctx));
    }

    #[test]
    fn user_binding_overrides_default() {
        let mut resolver = KeybindingResolver::new();
        let custom = KeyBinding::new(
            KeyChord::single(KeyCombo::new(Key::S, primary())),
            "custom.save",
        );
        resolver.add(custom);
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::S, primary());
        assert_eq!(resolver.resolve(&combo, &ctx), Some("custom.save"));
    }

    #[test]
    fn no_match_returns_none() {
        let resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::Q, Modifiers::ALT | Modifiers::SHIFT);
        assert!(resolver.resolve(&combo, &ctx).is_none());
    }

    #[test]
    fn negative_binding_removes_command() {
        let mut resolver = KeybindingResolver::new();
        let removal = KeyBinding::new(
            KeyChord::single(KeyCombo::new(Key::S, primary())),
            "-workbench.action.files.save",
        );
        resolver.add(removal);
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::S, primary());
        assert!(resolver.resolve(&combo, &ctx).is_none());
    }

    #[test]
    fn process_key_single() {
        let mut resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::S, primary());
        match resolver.process_key(&combo, &ctx) {
            KeybindingMatch::Full { command, .. } => {
                assert_eq!(command, "workbench.action.files.save");
            }
            other => panic!("expected Full, got {other:?}"),
        }
    }

    #[test]
    fn process_key_chord_sequence() {
        let mut resolver = KeybindingResolver::new();
        let ctx = ContextKeys::new();
        let p = primary();

        let first = KeyCombo::new(Key::K, p);
        match resolver.process_key(&first, &ctx) {
            KeybindingMatch::PartialChord => {}
            other => panic!("expected PartialChord, got {other:?}"),
        }
        assert!(resolver.in_chord());

        let second = KeyCombo::new(Key::C, p);
        match resolver.process_key(&second, &ctx) {
            KeybindingMatch::Full { command, .. } => {
                assert_eq!(command, "editor.action.addCommentLine");
            }
            other => panic!("expected Full, got {other:?}"),
        }
        assert!(!resolver.in_chord());
    }

    #[test]
    fn resolved_bindings_have_sources() {
        let resolver = KeybindingResolver::new();
        let resolved = resolver.resolved_bindings();
        assert!(!resolved.is_empty());
        assert!(resolved.iter().all(|r| r.is_default));
    }

    #[test]
    fn bindings_for_command() {
        let resolver = KeybindingResolver::new();
        let save_bindings = resolver.bindings_for_command("workbench.action.files.save");
        assert!(!save_bindings.is_empty());
    }

    #[test]
    fn extension_bindings() {
        let mut resolver = KeybindingResolver::new();
        let ext_binding = KeyBinding::new(
            KeyChord::single(KeyCombo::new(Key::F1, Modifiers::ALT)),
            "rust-analyzer.run",
        );
        resolver.load_extension("rust-analyzer", vec![ext_binding]);
        let ctx = ContextKeys::new();
        let combo = KeyCombo::new(Key::F1, Modifiers::ALT);
        assert_eq!(resolver.resolve(&combo, &ctx), Some("rust-analyzer.run"));

        let resolved = resolver.resolved_bindings();
        let ext = resolved.iter().find(|r| r.command == "rust-analyzer.run").unwrap();
        assert_eq!(ext.source, KeybindingSource::Extension("rust-analyzer".into()));
        assert!(!ext.is_default);
    }
}
