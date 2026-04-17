//! Extension activation event matching and triggering.
//!
//! VS Code extensions declare `activationEvents` in their `package.json`. When
//! a matching event occurs (e.g. a Rust file is opened), all extensions that
//! listen for that event are activated. This module implements the matching
//! logic and provides helpers for batch-activating extensions from a registry.

use crate::manifest::ExtensionManifest;
use crate::registry::ExtensionRegistry;

/// Typed activation event, parsed from the raw `activationEvents` strings in
/// an extension manifest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActivationEvent {
    /// `onLanguage:<languageId>` — activate when a file of this language opens.
    OnLanguage(String),
    /// `onCommand:<commandId>` — activate when this command is invoked.
    OnCommand(String),
    /// `onFileSystem:<scheme>` — activate when a file with this URI scheme is
    /// accessed (e.g. `ftp`, `ssh`).
    OnFileSystem(String),
    /// `onView:<viewId>` — activate when a specific tree view becomes visible.
    OnView(String),
    /// `onUri` — activate when the application's URI handler is invoked.
    OnUri,
    /// `onDebug` — activate when a debug session is about to start.
    OnDebug,
    /// `onDebugResolve:<type>` — activate to resolve a debug configuration.
    OnDebugResolve(String),
    /// `onDebugAdapterProtocolTracker:<type>` — activate for DAP tracking.
    OnDebugAdapterProtocolTracker(String),
    /// `workspaceContains:<glob>` — activate when a workspace contains a
    /// matching file.
    WorkspaceContains(String),
    /// `onStartupFinished` — activate after the window has finished loading.
    OnStartupFinished,
    /// `*` — always activate as soon as the host starts.
    Star,
}

impl ActivationEvent {
    /// Parses a raw activation event string (e.g. `"onLanguage:rust"`) into a
    /// typed [`ActivationEvent`]. Returns `None` for unrecognised events.
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed == "*" {
            return Some(Self::Star);
        }
        if trimmed == "onStartupFinished" {
            return Some(Self::OnStartupFinished);
        }
        if trimmed == "onUri" {
            return Some(Self::OnUri);
        }
        if trimmed == "onDebug" {
            return Some(Self::OnDebug);
        }

        if let Some(lang) = trimmed.strip_prefix("onLanguage:") {
            return Some(Self::OnLanguage(lang.to_owned()));
        }
        if let Some(cmd) = trimmed.strip_prefix("onCommand:") {
            return Some(Self::OnCommand(cmd.to_owned()));
        }
        if let Some(scheme) = trimmed.strip_prefix("onFileSystem:") {
            return Some(Self::OnFileSystem(scheme.to_owned()));
        }
        if let Some(view) = trimmed.strip_prefix("onView:") {
            return Some(Self::OnView(view.to_owned()));
        }
        if let Some(typ) = trimmed.strip_prefix("onDebugResolve:") {
            return Some(Self::OnDebugResolve(typ.to_owned()));
        }
        if let Some(typ) = trimmed.strip_prefix("onDebugAdapterProtocolTracker:") {
            return Some(Self::OnDebugAdapterProtocolTracker(typ.to_owned()));
        }
        if let Some(glob) = trimmed.strip_prefix("workspaceContains:") {
            return Some(Self::WorkspaceContains(glob.to_owned()));
        }

        None
    }

    /// Returns the raw string form as it appears in `package.json`.
    pub fn to_raw(&self) -> String {
        match self {
            Self::OnLanguage(l) => format!("onLanguage:{l}"),
            Self::OnCommand(c) => format!("onCommand:{c}"),
            Self::OnFileSystem(s) => format!("onFileSystem:{s}"),
            Self::OnView(v) => format!("onView:{v}"),
            Self::OnUri => "onUri".into(),
            Self::OnDebug => "onDebug".into(),
            Self::OnDebugResolve(t) => format!("onDebugResolve:{t}"),
            Self::OnDebugAdapterProtocolTracker(t) => {
                format!("onDebugAdapterProtocolTracker:{t}")
            }
            Self::WorkspaceContains(g) => format!("workspaceContains:{g}"),
            Self::OnStartupFinished => "onStartupFinished".into(),
            Self::Star => "*".into(),
        }
    }
}

/// Returns `true` if the given extension's manifest declares an activation
/// event that matches `event`.
pub fn should_activate(manifest: &ExtensionManifest, event: &ActivationEvent) -> bool {
    for raw in &manifest.activation_events {
        let Some(parsed) = ActivationEvent::parse(raw) else {
            continue;
        };
        if parsed == ActivationEvent::Star {
            return true;
        }
        if parsed == *event {
            return true;
        }
    }
    false
}

/// Scans the registry for all extensions that should activate in response to
/// `event` and returns their canonical ids.
///
/// Only considers enabled extensions. The caller is responsible for actually
/// sending `$activateExtension` to the extension host for each returned id.
pub fn activate_by_event(registry: &ExtensionRegistry, event: &ActivationEvent) -> Vec<String> {
    registry
        .all()
        .iter()
        .filter(|m| registry.is_enabled(&m.canonical_id()) && should_activate(m, event))
        .map(ExtensionManifest::canonical_id)
        .collect()
}

/// Collects all `*` (eager) extensions that should activate at host start.
pub fn eager_activation_ids(registry: &ExtensionRegistry) -> Vec<String> {
    activate_by_event(registry, &ActivationEvent::Star)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::parse_manifest_str;

    fn manifest_with_events_named(name: &str, events: &[&str]) -> ExtensionManifest {
        let events_json: Vec<String> = events.iter().map(|e| format!("\"{e}\"")).collect();
        let json = format!(
            r#"{{
                "name": "{name}",
                "publisher": "test",
                "version": "1.0.0",
                "activationEvents": [{}]
            }}"#,
            events_json.join(", ")
        );
        parse_manifest_str(&json).unwrap()
    }

    fn manifest_with_events(events: &[&str]) -> ExtensionManifest {
        manifest_with_events_named("test-ext", events)
    }

    #[test]
    fn parse_on_language() {
        let e = ActivationEvent::parse("onLanguage:rust").unwrap();
        assert_eq!(e, ActivationEvent::OnLanguage("rust".into()));
    }

    #[test]
    fn parse_on_command() {
        let e = ActivationEvent::parse("onCommand:extension.run").unwrap();
        assert_eq!(e, ActivationEvent::OnCommand("extension.run".into()));
    }

    #[test]
    fn parse_star() {
        let e = ActivationEvent::parse("*").unwrap();
        assert_eq!(e, ActivationEvent::Star);
    }

    #[test]
    fn parse_on_startup_finished() {
        let e = ActivationEvent::parse("onStartupFinished").unwrap();
        assert_eq!(e, ActivationEvent::OnStartupFinished);
    }

    #[test]
    fn parse_on_view() {
        let e = ActivationEvent::parse("onView:myExtView").unwrap();
        assert_eq!(e, ActivationEvent::OnView("myExtView".into()));
    }

    #[test]
    fn parse_workspace_contains() {
        let e = ActivationEvent::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(
            e,
            ActivationEvent::WorkspaceContains("**/Cargo.toml".into())
        );
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert!(ActivationEvent::parse("onSomethingWeird:abc").is_none());
    }

    #[test]
    fn roundtrip_to_raw() {
        let cases = vec![
            "onLanguage:python",
            "onCommand:editor.format",
            "onFileSystem:ssh",
            "onView:explorer",
            "onUri",
            "onDebug",
            "onDebugResolve:node",
            "workspaceContains:*.toml",
            "onStartupFinished",
            "*",
        ];
        for raw in cases {
            let parsed = ActivationEvent::parse(raw).unwrap();
            assert_eq!(parsed.to_raw(), raw);
        }
    }

    #[test]
    fn should_activate_on_language_match() {
        let m = manifest_with_events(&["onLanguage:rust"]);
        assert!(should_activate(
            &m,
            &ActivationEvent::OnLanguage("rust".into())
        ));
        assert!(!should_activate(
            &m,
            &ActivationEvent::OnLanguage("python".into())
        ));
    }

    #[test]
    fn should_activate_star_matches_everything() {
        let m = manifest_with_events(&["*"]);
        assert!(should_activate(
            &m,
            &ActivationEvent::OnLanguage("anything".into())
        ));
        assert!(should_activate(&m, &ActivationEvent::OnStartupFinished));
    }

    #[test]
    fn should_activate_multiple_events() {
        let m = manifest_with_events(&["onLanguage:rust", "onCommand:ext.run"]);
        assert!(should_activate(
            &m,
            &ActivationEvent::OnLanguage("rust".into())
        ));
        assert!(should_activate(
            &m,
            &ActivationEvent::OnCommand("ext.run".into())
        ));
        assert!(!should_activate(
            &m,
            &ActivationEvent::OnLanguage("go".into())
        ));
    }

    #[test]
    fn should_activate_no_events() {
        let m = manifest_with_events(&[]);
        assert!(!should_activate(
            &m,
            &ActivationEvent::OnLanguage("rust".into())
        ));
    }

    #[test]
    fn activate_by_event_from_registry() {
        let mut reg = ExtensionRegistry::new();

        let m1 = manifest_with_events_named("ext-rust", &["onLanguage:rust"]);
        let m2 = manifest_with_events_named("ext-python", &["onLanguage:python"]);
        let m3 = manifest_with_events_named("ext-star", &["*"]);

        reg.add(m1);
        reg.add(m2);
        reg.add(m3);

        let ids = activate_by_event(&reg, &ActivationEvent::OnLanguage("rust".into()));
        // ext-rust matches directly, ext-star matches via *, ext-python does not
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"test.ext-rust".to_string()));
        assert!(ids.contains(&"test.ext-star".to_string()));
    }

    #[test]
    fn activate_by_event_skips_disabled() {
        let mut reg = ExtensionRegistry::new();
        let m = manifest_with_events(&["onLanguage:rust"]);
        reg.add(m);
        reg.disable("test.test-ext");

        let ids = activate_by_event(&reg, &ActivationEvent::OnLanguage("rust".into()));
        assert!(ids.is_empty());
    }
}
