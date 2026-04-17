//! Color decorators — inline color swatches next to CSS colour values,
//! mirrors VS Code's `ColorDetector` contribution.
//!
//! Detects colour literals (hex, `rgb()`, `rgba()`, `hsl()`, `hsla()`, named
//! CSS colours) in source text and produces inline swatch descriptors that the
//! renderer can draw as small coloured squares.

use crate::decoration::Color;

/// A detected colour value with its position in the document.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorDecorator {
    pub line: u32,
    pub column: u32,
    pub end_column: u32,
    pub color: Color,
    pub original_text: String,
}

impl Eq for ColorDecorator {}

// ── CSS named colours (subset — the 17 standard + common extras) ────────────

const NAMED_COLORS: &[(&str, Color)] = &[
    ("black", Color::new(0.0, 0.0, 0.0, 1.0)),
    ("white", Color::new(1.0, 1.0, 1.0, 1.0)),
    ("red", Color::new(1.0, 0.0, 0.0, 1.0)),
    ("green", Color::new(0.0, 0.502, 0.0, 1.0)),
    ("blue", Color::new(0.0, 0.0, 1.0, 1.0)),
    ("yellow", Color::new(1.0, 1.0, 0.0, 1.0)),
    ("cyan", Color::new(0.0, 1.0, 1.0, 1.0)),
    ("magenta", Color::new(1.0, 0.0, 1.0, 1.0)),
    ("orange", Color::new(1.0, 0.647, 0.0, 1.0)),
    ("purple", Color::new(0.502, 0.0, 0.502, 1.0)),
    ("pink", Color::new(1.0, 0.753, 0.796, 1.0)),
    ("gray", Color::new(0.502, 0.502, 0.502, 1.0)),
    ("grey", Color::new(0.502, 0.502, 0.502, 1.0)),
    ("silver", Color::new(0.753, 0.753, 0.753, 1.0)),
    ("maroon", Color::new(0.502, 0.0, 0.0, 1.0)),
    ("olive", Color::new(0.502, 0.502, 0.0, 1.0)),
    ("navy", Color::new(0.0, 0.0, 0.502, 1.0)),
    ("teal", Color::new(0.0, 0.502, 0.502, 1.0)),
    ("aqua", Color::new(0.0, 1.0, 1.0, 1.0)),
    ("fuchsia", Color::new(1.0, 0.0, 1.0, 1.0)),
    ("lime", Color::new(0.0, 1.0, 0.0, 1.0)),
    ("coral", Color::new(1.0, 0.498, 0.314, 1.0)),
    ("salmon", Color::new(0.980, 0.502, 0.447, 1.0)),
    ("gold", Color::new(1.0, 0.843, 0.0, 1.0)),
    ("transparent", Color::new(0.0, 0.0, 0.0, 0.0)),
];

// ── Public API ──────────────────────────────────────────────────────────────

/// Detects all colour values in a single line of text.
#[must_use]
pub fn detect_colors(line_text: &str, line_number: u32) -> Vec<ColorDecorator> {
    let mut results = Vec::new();
    detect_hex_colors(line_text, line_number, &mut results);
    detect_functional_colors(line_text, line_number, &mut results);
    detect_named_colors(line_text, line_number, &mut results);
    results.sort_by_key(|d| d.column);
    results
}

/// Parses a CSS colour string into an RGBA `Color`.
///
/// Supports `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`,
/// `hsl()`, `hsla()`, and CSS named colours.
#[must_use]
pub fn parse_css_color(text: &str) -> Option<Color> {
    let trimmed = text.trim();
    if trimmed.starts_with('#') {
        parse_hex(trimmed)
    } else if trimmed.starts_with("rgb") {
        parse_rgb_func(trimmed)
    } else if trimmed.starts_with("hsl") {
        parse_hsl_func(trimmed)
    } else {
        lookup_named(trimmed)
    }
}

// ── Hex parsing ─────────────────────────────────────────────────────────────

fn parse_hex(hex: &str) -> Option<Color> {
    let hex = hex.strip_prefix('#')?;
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            (r, g, b, 255u8)
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
            (r, g, b, a)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255u8)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };
    Some(Color::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        f32::from(a) / 255.0,
    ))
}

fn detect_hex_colors(line: &str, line_num: u32, out: &mut Vec<ColorDecorator>) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'#' {
            let start = i;
            i += 1;
            let hex_start = i;
            while i < len && bytes[i].is_ascii_hexdigit() {
                i += 1;
            }
            let hex_len = i - hex_start;
            if matches!(hex_len, 3 | 4 | 6 | 8) {
                let text = &line[start..i];
                if let Some(color) = parse_hex(text) {
                    out.push(ColorDecorator {
                        line: line_num,
                        column: start as u32,
                        end_column: i as u32,
                        color,
                        original_text: text.to_string(),
                    });
                }
            }
        } else {
            i += 1;
        }
    }
}

// ── Functional (rgb/rgba/hsl/hsla) parsing ──────────────────────────────────

fn parse_rgb_func(s: &str) -> Option<Color> {
    let inner = s
        .strip_prefix("rgba(")
        .and_then(|s| s.strip_suffix(')'))
        .or_else(|| s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')))?;
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() < 3 {
        return None;
    }
    let r = parse_channel_u8(parts[0])?;
    let g = parse_channel_u8(parts[1])?;
    let b = parse_channel_u8(parts[2])?;
    let a: f32 = if parts.len() >= 4 {
        parts[3].parse().ok()?
    } else {
        1.0
    };
    Some(Color::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        a.clamp(0.0, 1.0),
    ))
}

fn parse_hsl_func(s: &str) -> Option<Color> {
    let inner = s
        .strip_prefix("hsla(")
        .and_then(|s| s.strip_suffix(')'))
        .or_else(|| s.strip_prefix("hsl(").and_then(|s| s.strip_suffix(')')))?;
    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() < 3 {
        return None;
    }
    let h: f32 = parts[0].trim_end_matches("deg").parse().ok()?;
    let s_pct: f32 = parts[1].trim_end_matches('%').parse().ok()?;
    let l_pct: f32 = parts[2].trim_end_matches('%').parse().ok()?;
    let a: f32 = if parts.len() >= 4 {
        parts[3].parse().ok()?
    } else {
        1.0
    };
    let (r, g, b) = hsl_to_rgb(h, s_pct / 100.0, l_pct / 100.0);
    Some(Color::new(r, g, b, a.clamp(0.0, 1.0)))
}

fn parse_channel_u8(s: &str) -> Option<u8> {
    if let Some(pct) = s.strip_suffix('%') {
        let v: f32 = pct.parse().ok()?;
        Some((v * 2.55).round() as u8)
    } else {
        s.parse().ok()
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < f32::EPSILON {
        return (l, l, l);
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hk = h / 360.0;
    let r = hue_to_rgb(p, q, hk + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, hk);
    let b = hue_to_rgb(p, q, hk - 1.0 / 3.0);
    (r, g, b)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 0.5 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

fn detect_functional_colors(line: &str, line_num: u32, out: &mut Vec<ColorDecorator>) {
    for prefix in &["rgba(", "rgb(", "hsla(", "hsl("] {
        let mut search_start = 0;
        while let Some(found) = line[search_start..].find(prefix) {
            let abs_start = search_start + found;
            if let Some(close) = line[abs_start..].find(')') {
                let end = abs_start + close + 1;
                let text = &line[abs_start..end];
                if let Some(color) = parse_css_color(text) {
                    out.push(ColorDecorator {
                        line: line_num,
                        column: abs_start as u32,
                        end_column: end as u32,
                        color,
                        original_text: text.to_string(),
                    });
                }
                search_start = end;
            } else {
                break;
            }
        }
    }
}

// ── Named colour detection ──────────────────────────────────────────────────

fn lookup_named(name: &str) -> Option<Color> {
    let lower = name.to_lowercase();
    NAMED_COLORS
        .iter()
        .find(|&&(n, _)| n == lower)
        .map(|&(_, c)| c)
}

fn detect_named_colors(line: &str, line_num: u32, out: &mut Vec<ColorDecorator>) {
    let lower = line.to_lowercase();
    for &(name, color) in NAMED_COLORS {
        let mut search_start = 0;
        while let Some(pos) = lower[search_start..].find(name) {
            let abs_start = search_start + pos;
            let abs_end = abs_start + name.len();

            let before_ok = abs_start == 0
                || !line.as_bytes()[abs_start - 1].is_ascii_alphanumeric();
            let after_ok = abs_end >= line.len()
                || !line.as_bytes()[abs_end].is_ascii_alphanumeric();

            if before_ok && after_ok {
                out.push(ColorDecorator {
                    line: line_num,
                    column: abs_start as u32,
                    end_column: abs_end as u32,
                    color,
                    original_text: line[abs_start..abs_end].to_string(),
                });
            }
            search_start = abs_end;
        }
    }
}

// ── State ───────────────────────────────────────────────────────────────────

/// Full state for the color-decorator feature.
#[derive(Debug, Clone, Default)]
pub struct ColorDecoratorState {
    pub decorators: Vec<ColorDecorator>,
    pub enabled: bool,
}

impl ColorDecoratorState {
    pub fn new() -> Self {
        Self {
            decorators: Vec::new(),
            enabled: true,
        }
    }

    /// Re-scans the given line texts and rebuilds the decorator list.
    pub fn scan_lines(&mut self, lines: &[&str]) {
        self.decorators.clear();
        if !self.enabled {
            return;
        }
        for (i, line) in lines.iter().enumerate() {
            let mut detected = detect_colors(line, i as u32);
            self.decorators.append(&mut detected);
        }
    }

    /// Returns decorators on the given line.
    #[must_use]
    pub fn decorators_on_line(&self, line: u32) -> Vec<&ColorDecorator> {
        self.decorators.iter().filter(|d| d.line == line).collect()
    }

    pub fn clear(&mut self) {
        self.decorators.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_3() {
        let c = parse_css_color("#f00").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g.abs() < 0.01);
    }

    #[test]
    fn parse_hex_6() {
        let c = parse_css_color("#ff8040").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.502).abs() < 0.01);
    }

    #[test]
    fn parse_hex_8() {
        let c = parse_css_color("#ff000080").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.a - 0.502).abs() < 0.01);
    }

    #[test]
    fn parse_rgb() {
        let c = parse_css_color("rgb(255, 128, 0)").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.502).abs() < 0.01);
    }

    #[test]
    fn parse_rgba() {
        let c = parse_css_color("rgba(255, 0, 0, 0.5)").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.a - 0.5).abs() < 0.01);
    }

    #[test]
    fn parse_hsl() {
        let c = parse_css_color("hsl(0, 100%, 50%)").unwrap();
        assert!((c.r - 1.0).abs() < 0.02);
        assert!(c.g.abs() < 0.02);
    }

    #[test]
    fn parse_hsla() {
        let c = parse_css_color("hsla(120, 100%, 50%, 0.5)").unwrap();
        assert!(c.g > 0.9);
        assert!((c.a - 0.5).abs() < 0.01);
    }

    #[test]
    fn parse_named() {
        let c = parse_css_color("red").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g.abs() < 0.01);
    }

    #[test]
    fn parse_named_case_insensitive() {
        let c = parse_css_color("Blue").unwrap();
        assert!((c.b - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_invalid_returns_none() {
        assert!(parse_css_color("not-a-color").is_none());
        assert!(parse_css_color("#gg0000").is_none());
        assert!(parse_css_color("rgb(a, b, c)").is_none());
    }

    #[test]
    fn detect_colors_in_line() {
        let decos = detect_colors("color: #ff0000; bg: rgb(0, 128, 255);", 0);
        assert!(decos.len() >= 2);
        assert_eq!(decos[0].original_text, "#ff0000");
    }

    #[test]
    fn detect_hex_short() {
        let decos = detect_colors("border: 1px solid #f00;", 5);
        assert_eq!(decos.len(), 1);
        assert_eq!(decos[0].line, 5);
        assert_eq!(decos[0].original_text, "#f00");
    }

    #[test]
    fn detect_hsl_color() {
        let decos = detect_colors("background: hsl(200, 80%, 50%);", 0);
        assert_eq!(decos.len(), 1);
        assert!(decos[0].original_text.starts_with("hsl("));
    }

    #[test]
    fn detect_named_color_word_boundary() {
        let decos = detect_colors("color: red; /* not reddish */", 0);
        let red_decos: Vec<_> = decos
            .iter()
            .filter(|d| d.original_text == "red")
            .collect();
        assert_eq!(red_decos.len(), 1);
    }

    #[test]
    fn state_scan_and_query() {
        let mut state = ColorDecoratorState::new();
        state.enabled = true;
        state.scan_lines(&["#fff", "color: blue;"]);
        assert!(!state.decorators.is_empty());
        assert!(!state.decorators_on_line(0).is_empty());
        assert!(!state.decorators_on_line(1).is_empty());
    }

    #[test]
    fn state_disabled() {
        let mut state = ColorDecoratorState::new();
        state.enabled = false;
        state.scan_lines(&["#fff"]);
        assert!(state.decorators.is_empty());
    }
}
