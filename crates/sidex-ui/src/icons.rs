//! Codicon icon registry mapping icon names to Unicode codepoints.
//!
//! Based on the [VS Code Codicon](https://microsoft.github.io/vscode-codicons/) icon font.
//! Each entry maps a kebab-case icon name to the `char` codepoint used by the
//! codicon font, allowing widgets and renderers to resolve icons at runtime.

use std::collections::HashMap;
use std::sync::LazyLock;

/// A single codicon entry: `(name, codepoint)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodiconEntry {
    pub name: &'static str,
    pub codepoint: char,
}

/// Global icon registry – built once on first access.
static REGISTRY: LazyLock<IconRegistry> = LazyLock::new(IconRegistry::default_codicons);

/// Registry that maps icon name → Unicode codepoint.
pub struct IconRegistry {
    map: HashMap<&'static str, char>,
}

impl IconRegistry {
    /// Build the default codicon registry with 150+ icons.
    #[allow(clippy::too_many_lines)]
    fn default_codicons() -> Self {
        let entries: &[(&str, u32)] = &[
            // ── Files ────────────────────────────────────────────────────
            ("file",                    0xEB60),
            ("file-text",               0xEB64),
            ("file-code",               0xEB40),
            ("file-media",              0xEB63),
            ("file-pdf",                0xEB44),
            ("file-zip",                0xEB45),
            ("file-binary",             0xEB41),
            ("file-submodule",          0xEB46),
            ("file-symlink-file",       0xEB47),
            ("file-symlink-directory",  0xEB48),
            ("file-add",                0xEA7F),
            ("file-directory-create",   0xEB56),
            ("new-file",                0xEA7F),
            ("new-folder",              0xEA80),

            // ── Folders ──────────────────────────────────────────────────
            ("folder",                  0xEA83),
            ("folder-opened",           0xEA84),
            ("folder-active",           0xEB34),
            ("folder-library",          0xEB35),
            ("root-folder",             0xEA82),
            ("root-folder-opened",      0xEA83),

            // ── Editor ───────────────────────────────────────────────────
            ("edit",                    0xEA73),
            ("editor-layout",           0xEB9F),
            ("copy",                    0xEBCD),
            ("save",                    0xEB4B),
            ("save-all",                0xEB4C),
            ("save-as",                 0xEB4D),
            ("close",                   0xEA76),
            ("close-all",               0xEB98),
            ("preview",                 0xEB2A),
            ("open-preview",            0xEB2A),
            ("split-horizontal",        0xEB56),
            ("split-vertical",          0xEB57),
            ("whole-word",              0xEB7E),
            ("case-sensitive",          0xEB7F),
            ("regex",                   0xEB38),
            ("replace",                 0xEB3D),
            ("replace-all",             0xEB3E),
            ("find",                    0xEB13),
            ("selection",               0xEB4E),
            ("indent",                  0xEB59),
            ("text-size",               0xEB5B),
            ("word-wrap",               0xEB57),
            ("whitespace",              0xEB7D),

            // ── Navigation ───────────────────────────────────────────────
            ("chevron-right",           0xEAB6),
            ("chevron-down",            0xEAB4),
            ("chevron-up",              0xEAB8),
            ("chevron-left",            0xEAB5),
            ("arrow-up",                0xEA76),
            ("arrow-down",              0xEA74),
            ("arrow-left",              0xEA75),
            ("arrow-right",             0xEA77),
            ("arrow-small-up",          0xEB97),
            ("arrow-small-down",        0xEB96),
            ("arrow-both",              0xEA78),
            ("go-to-file",              0xEA94),
            ("link-external",           0xEB07),
            ("link",                    0xEB06),
            ("list-ordered",            0xEB09),
            ("list-unordered",          0xEB0A),
            ("pinned",                  0xEB9D),

            // ── Symbols ──────────────────────────────────────────────────
            ("symbol-class",            0xEB5B),
            ("symbol-color",            0xEB5C),
            ("symbol-constant",         0xEB5D),
            ("symbol-constructor",      0xEB5E),
            ("symbol-enum",             0xEB5F),
            ("symbol-enum-member",      0xEB60),
            ("symbol-event",            0xEB61),
            ("symbol-field",            0xEB5E),
            ("symbol-file",             0xEB62),
            ("symbol-function",         0xEB63),
            ("symbol-interface",        0xEB64),
            ("symbol-key",              0xEB65),
            ("symbol-keyword",          0xEB66),
            ("symbol-method",           0xEB67),
            ("symbol-misc",             0xEB68),
            ("symbol-module",           0xEB69),
            ("symbol-namespace",        0xEB6A),
            ("symbol-number",           0xEB6B),
            ("symbol-object",           0xEB6C),
            ("symbol-operator",         0xEB6D),
            ("symbol-package",          0xEB6E),
            ("symbol-parameter",        0xEB6F),
            ("symbol-property",         0xEB70),
            ("symbol-reference",        0xEB71),
            ("symbol-ruler",            0xEB72),
            ("symbol-snippet",          0xEB73),
            ("symbol-string",           0xEB74),
            ("symbol-struct",           0xEB75),
            ("symbol-text",             0xEB76),
            ("symbol-type-parameter",   0xEB77),
            ("symbol-value",            0xEB78),
            ("symbol-variable",         0xEB79),

            // ── Git / SCM ────────────────────────────────────────────────
            ("git-branch",              0xEA68),
            ("git-commit",              0xEA69),
            ("git-compare",             0xEA6A),
            ("git-merge",               0xEA6B),
            ("git-pull-request",        0xEA6C),
            ("git-pull-request-closed", 0xEB3C),
            ("git-pull-request-draft",  0xEB3D),
            ("git-stash",               0xEB9C),
            ("git-stash-apply",         0xEB9E),
            ("git-stash-pop",           0xEB9F),
            ("repo",                    0xEA62),
            ("repo-clone",              0xEA63),
            ("repo-forked",             0xEA64),
            ("repo-push",               0xEA65),
            ("diff",                    0xEAF1),
            ("diff-added",              0xEAAD),
            ("diff-modified",           0xEAAE),
            ("diff-removed",            0xEAAF),
            ("diff-renamed",            0xEAB0),
            ("source-control",          0xEA68),

            // ── Debug ────────────────────────────────────────────────────
            ("debug",                   0xEA87),
            ("debug-alt",               0xEB91),
            ("debug-breakpoint",        0xEB8B),
            ("debug-breakpoint-conditional", 0xEB8C),
            ("debug-breakpoint-data",   0xEB8D),
            ("debug-breakpoint-function", 0xEB8E),
            ("debug-breakpoint-log",    0xEB8F),
            ("debug-breakpoint-unsupported", 0xEB90),
            ("debug-console",           0xEA88),
            ("debug-continue",          0xEA89),
            ("debug-disconnect",        0xEB8A),
            ("debug-pause",             0xEA8A),
            ("debug-restart",           0xEA8C),
            ("debug-start",             0xEA8D),
            ("debug-step-back",         0xEA8E),
            ("debug-step-into",         0xEA8F),
            ("debug-step-out",          0xEA90),
            ("debug-step-over",         0xEA91),
            ("debug-stop",              0xEA92),
            ("call-incoming",           0xEB9A),
            ("call-outgoing",           0xEB9B),
            ("variable-group",          0xEB79),
            ("watch",                   0xEA6F),

            // ── Status / Info ────────────────────────────────────────────
            ("error",                   0xEA87),
            ("warning",                 0xEA6C),
            ("info",                    0xEA74),
            ("check",                   0xEAB2),
            ("check-all",               0xEAB3),
            ("circle-filled",           0xEA71),
            ("circle-outline",          0xEA72),
            ("circle-slash",            0xEABD),
            ("pass",                    0xEAB2),
            ("pass-filled",             0xEBC2),
            ("stop-circle",             0xEB4F),
            ("bell",                    0xEA7A),
            ("bell-dot",                0xEB9A),
            ("loading",                 0xEB52),
            ("sync",                    0xEB4A),

            // ── Terminal ─────────────────────────────────────────────────
            ("terminal",                0xEA85),
            ("terminal-bash",           0xEBCA),
            ("terminal-cmd",            0xEBCB),
            ("terminal-powershell",     0xEBCC),
            ("terminal-tmux",           0xEBCD),
            ("terminal-ubuntu",         0xEBCE),
            ("terminal-linux",          0xEBCF),
            ("terminal-debian",         0xEBD0),
            ("console",                 0xEA88),
            ("output",                  0xEB9D),

            // ── Misc / Actions ───────────────────────────────────────────
            ("add",                     0xEA60),
            ("remove",                  0xEB5E),
            ("trash",                   0xEA81),
            ("search",                  0xEA6D),
            ("search-stop",             0xEABB),
            ("filter",                  0xEA6E),
            ("filter-filled",           0xEB83),
            ("gear",                    0xEB51),
            ("settings-gear",           0xEB52),
            ("extensions",              0xEA78),
            ("account",                 0xEB99),
            ("home",                    0xEB50),
            ("bookmark",                0xEA62),
            ("tag",                     0xEA66),
            ("eye",                     0xEA70),
            ("eye-closed",              0xEBD1),
            ("lock",                    0xEA75),
            ("unlock",                  0xEBD2),
            ("shield",                  0xEB85),
            ("comment",                 0xEA6B),
            ("comment-discussion",      0xEA6C),
            ("mail",                    0xEB0B),
            ("milestone",               0xEB0C),
            ("globe",                   0xEB01),
            ("refresh",                 0xEB37),
            ("run",                     0xEB49),
            ("run-all",                 0xEB9E),
            ("play",                    0xEB49),
            ("record",                  0xEB3F),
            ("stop",                    0xEB4F),
            ("question",                0xEB39),
            ("lightbulb",               0xEA61),
            ("lightbulb-autofix",       0xEB87),
            ("rocket",                  0xEB44),
            ("color-mode",              0xEB36),
            ("graph",                   0xEB99),
            ("graph-line",              0xEB9A),
            ("table",                   0xEB58),
            ("cloud",                   0xEBBE),
            ("cloud-download",          0xEBBF),
            ("cloud-upload",            0xEBC0),
            ("remote",                  0xEB62),
            ("remote-explorer",         0xEB63),
            ("vm",                      0xEA7D),
            ("server",                  0xEB53),
            ("database",                0xEACB),
            ("layout",                  0xEB9F),
            ("group-by-ref-type",       0xEB84),
            ("json",                    0xEB59),
            ("bracket",                 0xEB60),
            ("sparkle",                 0xEBD3),
            ("copilot",                 0xEBD4),
        ];

        let mut map = HashMap::with_capacity(entries.len());
        for &(name, code) in entries {
            if let Some(c) = char::from_u32(code) {
                map.insert(name, c);
            }
        }

        Self { map }
    }

    /// Look up a codicon by name.
    pub fn get(name: &str) -> Option<char> {
        REGISTRY.map.get(name).copied()
    }

    /// Look up a codicon, returning the replacement character on miss.
    pub fn get_or_fallback(name: &str) -> char {
        Self::get(name).unwrap_or('\u{FFFD}')
    }

    /// Returns the number of registered icons.
    pub fn len() -> usize {
        REGISTRY.map.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty() -> bool {
        REGISTRY.map.is_empty()
    }

    /// Iterate over all registered `(name, codepoint)` pairs.
    pub fn iter() -> impl Iterator<Item = (&'static str, char)> {
        REGISTRY.map.iter().map(|(&k, &v)| (k, v))
    }

    /// Check whether the registry contains a given icon name.
    pub fn contains(name: &str) -> bool {
        REGISTRY.map.contains_key(name)
    }

    /// Format an icon name as the string a codicon font would render.
    pub fn render_str(name: &str) -> Option<String> {
        Self::get(name).map(|c| c.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_at_least_150_icons() {
        assert!(
            IconRegistry::len() >= 150,
            "expected ≥150 icons, got {}",
            IconRegistry::len()
        );
    }

    #[test]
    fn lookup_known_icons() {
        assert!(IconRegistry::get("file").is_some());
        assert!(IconRegistry::get("folder").is_some());
        assert!(IconRegistry::get("git-branch").is_some());
        assert!(IconRegistry::get("debug").is_some());
        assert!(IconRegistry::get("terminal").is_some());
        assert!(IconRegistry::get("search").is_some());
    }

    #[test]
    fn lookup_missing_icon_returns_none() {
        assert!(IconRegistry::get("nonexistent-icon-xyz").is_none());
    }

    #[test]
    fn fallback_returns_replacement_char() {
        assert_eq!(
            IconRegistry::get_or_fallback("nonexistent"),
            '\u{FFFD}'
        );
    }

    #[test]
    fn iter_yields_entries() {
        let count = IconRegistry::iter().count();
        assert_eq!(count, IconRegistry::len());
    }
}
