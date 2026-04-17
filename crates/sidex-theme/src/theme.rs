//! Theme loading, parsing, and default theme construction.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::color::Color;
use crate::token_color::{FontStyle, TokenColorRule};
use crate::workbench_colors::WorkbenchColors;

/// Describes the luminance kind of a theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeKind {
    Light,
    #[default]
    Dark,
    #[serde(rename = "hc")]
    HighContrast,
    #[serde(rename = "hcLight")]
    HighContrastLight,
}

/// A complete color theme, combining workbench UI colors and syntax token
/// colors.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: ThemeKind,
    #[serde(rename = "tokenColors", default)]
    pub token_colors: Vec<TokenColorRule>,
    #[serde(rename = "colors", default)]
    pub workbench_colors: WorkbenchColors,
}

impl Theme {
    /// Parse a VS Code JSON color theme from a string.
    ///
    /// Handles the standard format with `name`, `type`, `colors`, and
    /// `tokenColors` fields.
    pub fn from_json(json: &str) -> Result<Self> {
        let v: Value = serde_json::from_str(json).context("invalid JSON")?;
        Self::from_value(&v)
    }

    /// Parse from an already-parsed `serde_json::Value`.
    pub fn from_value(v: &Value) -> Result<Self> {
        let name = v
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Untitled")
            .to_owned();

        let kind = match v.get("type").and_then(Value::as_str) {
            Some("light") => ThemeKind::Light,
            Some("hc" | "hcDark") => ThemeKind::HighContrast,
            Some("hcLight") => ThemeKind::HighContrastLight,
            _ => ThemeKind::Dark,
        };

        let workbench_colors: WorkbenchColors = v
            .get("colors")
            .map(|c| serde_json::from_value(c.clone()))
            .transpose()
            .context("failed to parse colors")?
            .unwrap_or_default();

        let token_colors = parse_token_colors(v.get("tokenColors"));

        Ok(Self {
            name,
            kind,
            token_colors,
            workbench_colors,
        })
    }

    /// A sensible dark theme — alias for [`crate::default_themes::dark_modern`].
    pub fn default_dark() -> Self {
        crate::default_themes::dark_modern()
    }

    /// A sensible light theme — alias for [`crate::default_themes::light_modern`].
    pub fn default_light() -> Self {
        crate::default_themes::light_modern()
    }
}

// ── Token color parsing ──────────────────────────────────────────────────────

fn parse_token_colors(v: Option<&Value>) -> Vec<TokenColorRule> {
    let Some(arr) = v.and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut rules = Vec::with_capacity(arr.len());
    for entry in arr {
        rules.push(parse_single_token_color(entry));
    }
    rules
}

fn parse_single_token_color(v: &Value) -> TokenColorRule {
    let scope = match v.get("scope") {
        Some(Value::String(s)) => s.split(',').map(|p| p.trim().to_owned()).collect(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => vec![String::new()],
    };

    let settings = v.get("settings").unwrap_or(v);

    let foreground = settings
        .get("foreground")
        .and_then(Value::as_str)
        .and_then(|s| Color::from_hex(s).ok());

    let background = settings
        .get("background")
        .and_then(Value::as_str)
        .and_then(|s| Color::from_hex(s).ok());

    let font_style = settings
        .get("fontStyle")
        .and_then(Value::as_str)
        .map_or(FontStyle::NONE, parse_font_style_str);

    let name = v.get("name").and_then(Value::as_str).map(str::to_owned);

    TokenColorRule {
        name,
        scope,
        foreground,
        background,
        font_style,
    }
}

fn parse_font_style_str(s: &str) -> FontStyle {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dark_theme() {
        let t = Theme::default_dark();
        assert_eq!(t.kind, ThemeKind::Dark);
        assert!(!t.token_colors.is_empty());
        assert!(t.workbench_colors.editor_background.is_some());
    }

    #[test]
    fn default_light_theme() {
        let t = Theme::default_light();
        assert_eq!(t.kind, ThemeKind::Light);
        assert!(!t.token_colors.is_empty());
    }

    #[test]
    fn parse_minimal_json() {
        let json = r##"{
            "name": "Test Theme",
            "type": "dark",
            "colors": {
                "editorBackground": "#1e1e1e"
            },
            "tokenColors": [
                {
                    "scope": "comment",
                    "settings": {
                        "foreground": "#6a9955"
                    }
                }
            ]
        }"##;
        let theme = Theme::from_json(json).unwrap();
        assert_eq!(theme.name, "Test Theme");
        assert_eq!(theme.kind, ThemeKind::Dark);
        assert_eq!(theme.token_colors.len(), 1);
        assert_eq!(theme.token_colors[0].scope, vec!["comment"]);
    }

    #[test]
    fn parse_scope_array() {
        let json = r##"{
            "name": "Test",
            "tokenColors": [
                {
                    "scope": ["comment", "string"],
                    "settings": { "foreground": "#ff0000" }
                }
            ]
        }"##;
        let theme = Theme::from_json(json).unwrap();
        assert_eq!(theme.token_colors[0].scope, vec!["comment", "string"]);
    }

    #[test]
    fn parse_scope_csv() {
        let json = r##"{
            "name": "Test",
            "tokenColors": [
                {
                    "scope": "comment, string.quoted",
                    "settings": { "foreground": "#ff0000" }
                }
            ]
        }"##;
        let theme = Theme::from_json(json).unwrap();
        assert_eq!(
            theme.token_colors[0].scope,
            vec!["comment", "string.quoted"]
        );
    }

    #[test]
    fn theme_kind_default_is_dark() {
        let json = r##"{ "name": "Bare" }"##;
        let theme = Theme::from_json(json).unwrap();
        assert_eq!(theme.kind, ThemeKind::Dark);
    }

    #[test]
    fn theme_kind_light() {
        let json = r##"{ "name": "L", "type": "light" }"##;
        let theme = Theme::from_json(json).unwrap();
        assert_eq!(theme.kind, ThemeKind::Light);
    }

    #[test]
    fn theme_kind_hc() {
        let json = r##"{ "name": "HC", "type": "hc" }"##;
        let theme = Theme::from_json(json).unwrap();
        assert_eq!(theme.kind, ThemeKind::HighContrast);
    }

    #[test]
    fn font_style_in_token_color() {
        let json = r##"{
            "name": "Styled",
            "tokenColors": [{
                "scope": "comment",
                "settings": {
                    "foreground": "#aaaaaa",
                    "fontStyle": "italic bold"
                }
            }]
        }"##;
        let theme = Theme::from_json(json).unwrap();
        let fs = theme.token_colors[0].font_style;
        assert!(fs.contains(FontStyle::ITALIC));
        assert!(fs.contains(FontStyle::BOLD));
    }
}
