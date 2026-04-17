//! Token / syntax coloring rules that map `TextMate` scopes to styles.

use serde::{Deserialize, Serialize};

use crate::color::Color;

/// A resolved style for a `TextMate` scope, combining foreground, background,
/// and font style from the most-specific matching rule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedStyle {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub font_style: FontStyle,
}

// ── FontStyle bitflags ───────────────────────────────────────────────────────

/// Font styling attributes expressed as bitflags.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontStyle(u8);

impl FontStyle {
    pub const NONE: Self = Self(0);
    pub const BOLD: Self = Self(1);
    pub const ITALIC: Self = Self(1 << 1);
    pub const UNDERLINE: Self = Self(1 << 2);
    pub const STRIKETHROUGH: Self = Self(1 << 3);

    /// Returns `true` if `self` contains all bits in `other`.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Set the bits of `other` in `self`.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Return `true` when no flags are set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for FontStyle {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for FontStyle {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}

// ── TokenColorRule ───────────────────────────────────────────────────────────

/// A single token color rule mapping one or more `TextMate` scopes to a style.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenColorRule {
    /// Human-readable name for this rule (optional in theme files).
    #[serde(default)]
    pub name: Option<String>,
    /// `TextMate` scopes this rule applies to (e.g. `"comment"`, `"keyword.control"`).
    #[serde(deserialize_with = "deserialize_scope")]
    pub scope: Vec<String>,
    /// Foreground color.
    #[serde(default)]
    pub foreground: Option<Color>,
    /// Background color.
    #[serde(default)]
    pub background: Option<Color>,
    /// Font style flags.
    #[serde(default, deserialize_with = "deserialize_font_style")]
    pub font_style: FontStyle,
}

fn deserialize_scope<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ScopeValue {
        Single(String),
        Multiple(Vec<String>),
    }

    match ScopeValue::deserialize(d)? {
        ScopeValue::Single(s) => Ok(s.split(',').map(|part| part.trim().to_owned()).collect()),
        ScopeValue::Multiple(v) => Ok(v),
    }
}

fn deserialize_font_style<'de, D: serde::Deserializer<'de>>(d: D) -> Result<FontStyle, D::Error> {
    let s = String::deserialize(d)?;
    Ok(parse_font_style(&s))
}

fn parse_font_style(s: &str) -> FontStyle {
    let mut style = FontStyle::NONE;
    for part in s.split_whitespace() {
        match part.to_ascii_lowercase().as_str() {
            "bold" => style |= FontStyle::BOLD,
            "italic" => style |= FontStyle::ITALIC,
            "underline" => style |= FontStyle::UNDERLINE,
            "strikethrough" => style |= FontStyle::STRIKETHROUGH,
            _ => {}
        }
    }
    style
}

// ── TokenColorMap ────────────────────────────────────────────────────────────

/// Maps `TextMate` scopes to token color rules, resolving the most-specific
/// match for a given scope string.
#[derive(Clone, Debug, Default)]
pub struct TokenColorMap {
    rules: Vec<TokenColorRule>,
}

impl TokenColorMap {
    /// Build a map from a list of rules (order is preserved for priority).
    pub fn new(rules: Vec<TokenColorRule>) -> Self {
        Self { rules }
    }

    /// Resolve the best-matching style for `scope`.
    ///
    /// Uses `TextMate` scope specificity: a rule scope `keyword.control` is
    /// more specific than `keyword` when matching `keyword.control.flow`.
    /// When multiple rules match, the one with the longest (most specific)
    /// scope selector wins.
    pub fn resolve(&self, scope: &str) -> ResolvedStyle {
        let mut best: Option<(&TokenColorRule, usize)> = None;

        for rule in &self.rules {
            for rule_scope in &rule.scope {
                if scope_matches(scope, rule_scope) {
                    let specificity = rule_scope.len();
                    if best.is_none_or(|(_, best_len)| specificity > best_len) {
                        best = Some((rule, specificity));
                    }
                }
            }
        }

        best.map_or_else(ResolvedStyle::default, |(rule, _)| ResolvedStyle {
            foreground: rule.foreground,
            background: rule.background,
            font_style: rule.font_style,
        })
    }
}

/// Returns `true` if the `TextMate` `scope` is matched by `selector`.
///
/// A selector matches when `scope` starts with `selector` and either they are
/// equal or the next character in `scope` after the selector is `.`.
fn scope_matches(scope: &str, selector: &str) -> bool {
    if selector.is_empty() {
        return true;
    }
    scope == selector
        || (scope.starts_with(selector) && scope.as_bytes().get(selector.len()) == Some(&b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn rule(scope: &str, fg: &str) -> TokenColorRule {
        TokenColorRule {
            name: None,
            scope: vec![scope.to_owned()],
            foreground: Some(Color::from_hex(fg).unwrap()),
            background: None,
            font_style: FontStyle::NONE,
        }
    }

    #[test]
    fn exact_scope_match() {
        let map = TokenColorMap::new(vec![rule("comment", "#00ff00")]);
        let style = map.resolve("comment");
        assert_eq!(style.foreground, Some(Color::from_hex("#00ff00").unwrap()));
    }

    #[test]
    fn prefix_scope_match() {
        let map = TokenColorMap::new(vec![rule("keyword", "#ff0000")]);
        let style = map.resolve("keyword.control");
        assert_eq!(style.foreground, Some(Color::from_hex("#ff0000").unwrap()));
    }

    #[test]
    fn most_specific_wins() {
        let map = TokenColorMap::new(vec![
            rule("keyword", "#ff0000"),
            rule("keyword.control", "#00ff00"),
        ]);
        let style = map.resolve("keyword.control.flow");
        assert_eq!(style.foreground, Some(Color::from_hex("#00ff00").unwrap()));
    }

    #[test]
    fn no_match_returns_default() {
        let map = TokenColorMap::new(vec![rule("comment", "#00ff00")]);
        let style = map.resolve("string.quoted");
        assert_eq!(style, ResolvedStyle::default());
    }

    #[test]
    fn scope_does_not_match_partial_segment() {
        let map = TokenColorMap::new(vec![rule("key", "#ff0000")]);
        let style = map.resolve("keyword");
        assert_eq!(style, ResolvedStyle::default());
    }

    #[test]
    fn font_style_parsing() {
        assert_eq!(
            parse_font_style("bold italic"),
            FontStyle::BOLD | FontStyle::ITALIC
        );
        assert_eq!(parse_font_style("underline"), FontStyle::UNDERLINE);
        assert!(parse_font_style("").is_empty());
    }

    #[test]
    fn font_style_contains() {
        let s = FontStyle::BOLD | FontStyle::ITALIC;
        assert!(s.contains(FontStyle::BOLD));
        assert!(s.contains(FontStyle::ITALIC));
        assert!(!s.contains(FontStyle::UNDERLINE));
    }
}
