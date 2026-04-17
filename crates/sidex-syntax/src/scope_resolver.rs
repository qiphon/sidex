//! `TextMate` scope-to-highlight mapping.
//!
//! Maps `TextMate` scope names (e.g. `"string.quoted.double.rust"`) to highlight
//! categories using longest-prefix matching. More specific scopes override
//! less specific ones.

use serde::{Deserialize, Serialize};

/// A text style resolved from a scope stack.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextStyle {
    pub foreground: Option<String>,
    pub background: Option<String>,
    pub font_style: FontStyle,
}

/// Font style flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct FontStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
}

/// A single token color rule from a theme, mapping scope selectors to styles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenColorRule {
    /// Space-separated scope selector (e.g. `"comment"`, `"string.quoted"`).
    pub scope: String,
    #[serde(default)]
    pub foreground: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub font_style: Option<String>,
}

/// Resolves a `TextMate` scope stack against a set of theme token color rules.
///
/// For each scope in the stack (innermost first), finds the rule whose `scope`
/// selector is the longest prefix match. Styles from more-specific (deeper)
/// scopes override less-specific ones.
pub fn resolve_scope(scopes: &[&str], theme_rules: &[TokenColorRule]) -> TextStyle {
    let mut style = TextStyle::default();

    // Process from outermost to innermost so inner scopes win.
    for scope in scopes {
        if let Some(rule) = find_best_match(scope, theme_rules) {
            if let Some(ref fg) = rule.foreground {
                style.foreground = Some(fg.clone());
            }
            if let Some(ref bg) = rule.background {
                style.background = Some(bg.clone());
            }
            if let Some(ref fs) = rule.font_style {
                style.font_style = parse_font_style(fs);
            }
        }
    }

    style
}

/// Finds the rule with the longest prefix match for the given scope name.
fn find_best_match<'a>(scope: &str, rules: &'a [TokenColorRule]) -> Option<&'a TokenColorRule> {
    let mut best: Option<(&TokenColorRule, usize)> = None;

    for rule in rules {
        for selector in rule.scope.split(',') {
            let selector = selector.trim();
            if scope_matches(scope, selector) {
                let specificity = selector.len();
                if best.is_none_or(|(_, s)| specificity > s) {
                    best = Some((rule, specificity));
                }
            }
        }
    }

    best.map(|(rule, _)| rule)
}

/// Returns `true` if `scope` starts with `selector` at a dot boundary.
///
/// For example `"string.quoted.double.rust"` matches `"string.quoted"` and
/// `"string"` but not `"string.q"`.
fn scope_matches(scope: &str, selector: &str) -> bool {
    if scope == selector {
        return true;
    }
    if let Some(rest) = scope.strip_prefix(selector) {
        return rest.starts_with('.');
    }
    false
}

fn parse_font_style(s: &str) -> FontStyle {
    let mut fs = FontStyle::default();
    for token in s.split_whitespace() {
        match token {
            "bold" => fs.bold = true,
            "italic" => fs.italic = true,
            "underline" => fs.underline = true,
            "strikethrough" => fs.strikethrough = true,
            _ => {}
        }
    }
    fs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rules() -> Vec<TokenColorRule> {
        vec![
            TokenColorRule {
                scope: "comment".into(),
                foreground: Some("#6A9955".into()),
                background: None,
                font_style: Some("italic".into()),
            },
            TokenColorRule {
                scope: "string".into(),
                foreground: Some("#CE9178".into()),
                background: None,
                font_style: None,
            },
            TokenColorRule {
                scope: "string.quoted.double".into(),
                foreground: Some("#D69D85".into()),
                background: None,
                font_style: None,
            },
            TokenColorRule {
                scope: "keyword".into(),
                foreground: Some("#569CD6".into()),
                background: None,
                font_style: None,
            },
            TokenColorRule {
                scope: "keyword.control".into(),
                foreground: Some("#C586C0".into()),
                background: None,
                font_style: Some("bold".into()),
            },
            TokenColorRule {
                scope: "constant.numeric, constant.language".into(),
                foreground: Some("#B5CEA8".into()),
                background: None,
                font_style: None,
            },
        ]
    }

    #[test]
    fn resolve_exact_match() {
        let rules = sample_rules();
        let style = resolve_scope(&["comment"], &rules);
        assert_eq!(style.foreground.as_deref(), Some("#6A9955"));
        assert!(style.font_style.italic);
    }

    #[test]
    fn resolve_prefix_match() {
        let rules = sample_rules();
        let style = resolve_scope(&["comment.line.double-slash.rust"], &rules);
        assert_eq!(style.foreground.as_deref(), Some("#6A9955"));
    }

    #[test]
    fn resolve_longest_prefix_wins() {
        let rules = sample_rules();
        let style = resolve_scope(&["string.quoted.double.rust"], &rules);
        assert_eq!(
            style.foreground.as_deref(),
            Some("#D69D85"),
            "more specific string.quoted.double should win over string"
        );
    }

    #[test]
    fn resolve_keyword_control() {
        let rules = sample_rules();
        let style = resolve_scope(&["keyword.control.rust"], &rules);
        assert_eq!(style.foreground.as_deref(), Some("#C586C0"));
        assert!(style.font_style.bold);
    }

    #[test]
    fn resolve_scope_stack_inner_wins() {
        let rules = sample_rules();
        let style = resolve_scope(&["source.rust", "keyword.control"], &rules);
        assert_eq!(
            style.foreground.as_deref(),
            Some("#C586C0"),
            "innermost matched scope should win"
        );
    }

    #[test]
    fn resolve_no_match() {
        let rules = sample_rules();
        let style = resolve_scope(&["meta.unknown.foo"], &rules);
        assert!(style.foreground.is_none());
        assert!(style.background.is_none());
    }

    #[test]
    fn resolve_comma_separated_selectors() {
        let rules = sample_rules();
        let style = resolve_scope(&["constant.numeric.integer"], &rules);
        assert_eq!(style.foreground.as_deref(), Some("#B5CEA8"));

        let style2 = resolve_scope(&["constant.language.boolean"], &rules);
        assert_eq!(style2.foreground.as_deref(), Some("#B5CEA8"));
    }

    #[test]
    fn scope_matches_boundary() {
        assert!(scope_matches("string.quoted.double", "string"));
        assert!(scope_matches("string.quoted.double", "string.quoted"));
        assert!(scope_matches(
            "string.quoted.double",
            "string.quoted.double"
        ));
        assert!(!scope_matches("string.quoted.double", "string.q"));
        assert!(!scope_matches("string.quoted.double", "strin"));
    }

    #[test]
    fn parse_font_style_multiple() {
        let fs = parse_font_style("bold italic underline strikethrough");
        assert!(fs.bold);
        assert!(fs.italic);
        assert!(fs.underline);
        assert!(fs.strikethrough);
    }

    #[test]
    fn text_style_default() {
        let style = TextStyle::default();
        assert!(style.foreground.is_none());
        assert!(style.background.is_none());
        assert!(!style.font_style.bold);
    }

    #[test]
    fn empty_scopes_return_default() {
        let rules = sample_rules();
        let style = resolve_scope(&[], &rules);
        assert_eq!(style, TextStyle::default());
    }

    #[test]
    fn empty_rules_return_default() {
        let style = resolve_scope(&["keyword"], &[]);
        assert_eq!(style, TextStyle::default());
    }
}
