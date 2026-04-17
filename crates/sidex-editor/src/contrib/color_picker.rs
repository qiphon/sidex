//! Color picker — mirrors VS Code's color-picker contribution.
//!
//! Detects color literals in the document, provides HSL/RGB/HEX conversion,
//! and tracks the state for an inline color picker popup.

use sidex_text::{Buffer, Position, Range};

/// A color in RGBA (0.0–1.0 per channel).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorRGBA {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

/// A color in HSLA (h: 0–360, s/l/a: 0.0–1.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorHSLA {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

/// The output format for color strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorFormat {
    Hex,
    HexAlpha,
    Rgb,
    Rgba,
    Hsl,
    Hsla,
}

impl ColorRGBA {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Converts to a CSS hex string (e.g. `#ff00aaff`).
    #[must_use]
    pub fn to_hex(&self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        )
    }

    /// Converts to a short hex string without alpha (if alpha is 1.0).
    #[must_use]
    pub fn to_hex_short(&self) -> String {
        if (self.a - 1.0).abs() < 0.004 {
            format!(
                "#{:02x}{:02x}{:02x}",
                (self.r * 255.0) as u8,
                (self.g * 255.0) as u8,
                (self.b * 255.0) as u8,
            )
        } else {
            self.to_hex()
        }
    }

    /// Converts to a CSS `rgb()` string.
    #[must_use]
    pub fn to_rgb_string(&self) -> String {
        format!(
            "rgb({}, {}, {})",
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
        )
    }

    /// Converts to a CSS `rgba()` string.
    #[must_use]
    pub fn to_rgba_string(&self) -> String {
        format!(
            "rgba({}, {}, {}, {:.2})",
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
            self.a,
        )
    }

    /// Converts to HSLA.
    #[must_use]
    pub fn to_hsla(&self) -> ColorHSLA {
        let r = self.r;
        let g = self.g;
        let b = self.b;

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        let l = (max + min) / 2.0;
        if delta.abs() < f32::EPSILON {
            return ColorHSLA {
                h: 0.0,
                s: 0.0,
                l,
                a: self.a,
            };
        }

        let s = if l < 0.5 {
            delta / (max + min)
        } else {
            delta / (2.0 - max - min)
        };

        let h = if (max - r).abs() < f32::EPSILON {
            ((g - b) / delta) % 6.0
        } else if (max - g).abs() < f32::EPSILON {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };

        let h = (h * 60.0 + 360.0) % 360.0;
        ColorHSLA { h, s, l, a: self.a }
    }

    /// Converts to string in the given format.
    #[must_use]
    pub fn to_format(&self, format: ColorFormat) -> String {
        match format {
            ColorFormat::Hex => self.to_hex_short(),
            ColorFormat::HexAlpha => self.to_hex(),
            ColorFormat::Rgb => self.to_rgb_string(),
            ColorFormat::Rgba => self.to_rgba_string(),
            ColorFormat::Hsl => self.to_hsla().to_hsl_string(),
            ColorFormat::Hsla => self.to_hsla().to_hsla_string(),
        }
    }

    /// Parses a hex color string (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`).
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
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
        Some(Self {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: f32::from(a) / 255.0,
        })
    }

    /// Parses an `rgb(r, g, b)` or `rgba(r, g, b, a)` string.
    #[must_use]
    pub fn from_rgb_string(s: &str) -> Option<Self> {
        let inner = s
            .strip_prefix("rgba(")
            .and_then(|s| s.strip_suffix(')'))
            .or_else(|| s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')))?;
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() < 3 {
            return None;
        }
        let r: u8 = parts[0].parse().ok()?;
        let g: u8 = parts[1].parse().ok()?;
        let b: u8 = parts[2].parse().ok()?;
        let a: f32 = if parts.len() >= 4 {
            parts[3].parse().ok()?
        } else {
            1.0
        };
        Some(Self::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            a,
        ))
    }
}

impl ColorHSLA {
    /// Converts to RGBA.
    #[must_use]
    pub fn to_rgba(&self) -> ColorRGBA {
        let h = self.h;
        let s = self.s;
        let l = self.l;

        if s.abs() < f32::EPSILON {
            return ColorRGBA::new(l, l, l, self.a);
        }

        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;

        let r = Self::hue_to_rgb(p, q, h / 360.0 + 1.0 / 3.0);
        let g = Self::hue_to_rgb(p, q, h / 360.0);
        let b = Self::hue_to_rgb(p, q, h / 360.0 - 1.0 / 3.0);

        ColorRGBA::new(r, g, b, self.a)
    }

    /// Converts to a CSS `hsl()` string.
    #[must_use]
    pub fn to_hsl_string(&self) -> String {
        format!(
            "hsl({:.0}, {:.0}%, {:.0}%)",
            self.h,
            self.s * 100.0,
            self.l * 100.0
        )
    }

    /// Converts to a CSS `hsla()` string.
    #[must_use]
    pub fn to_hsla_string(&self) -> String {
        format!(
            "hsla({:.0}, {:.0}%, {:.0}%, {:.2})",
            self.h,
            self.s * 100.0,
            self.l * 100.0,
            self.a
        )
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
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    }
}

/// A detected color in the document.
#[derive(Debug, Clone)]
pub struct DocumentColor {
    /// The range of the color literal in the source.
    pub range: Range,
    /// The parsed colour value.
    pub color: ColorRGBA,
    /// The original format of the color in source.
    pub format: ColorFormat,
}

/// Swatch rendering info for the gutter/inline color preview.
#[derive(Debug, Clone)]
pub struct ColorSwatch {
    pub line: u32,
    pub column: u32,
    pub color: ColorRGBA,
}

/// Full state for the color-picker feature.
#[derive(Debug, Clone, Default)]
pub struct ColorPickerState {
    /// All detected colors in the document.
    pub colors: Vec<DocumentColor>,
    /// Index of the color whose picker is currently open, if any.
    pub active_picker: Option<usize>,
    /// Whether the color provider is loading.
    pub is_loading: bool,
    /// Swatches to render inline.
    pub swatches: Vec<ColorSwatch>,
}

impl ColorPickerState {
    /// Sets the detected document colors from the language server.
    pub fn set_colors(&mut self, colors: Vec<DocumentColor>) {
        self.swatches = colors
            .iter()
            .map(|dc| ColorSwatch {
                line: dc.range.start.line,
                column: dc.range.start.column,
                color: dc.color,
            })
            .collect();
        self.colors = colors;
        self.is_loading = false;
    }

    /// Detects color literals in the buffer using regex patterns.
    pub fn detect_colors(&mut self, buffer: &Buffer) {
        self.colors.clear();
        self.swatches.clear();

        for line_idx in 0..buffer.len_lines() {
            let content = buffer.line_content(line_idx);
            self.detect_hex_colors(&content, line_idx as u32);
            self.detect_rgb_colors(&content, line_idx as u32);
        }
    }

    /// Opens the color picker for the color at `index`.
    pub fn open_picker(&mut self, index: usize) {
        if index < self.colors.len() {
            self.active_picker = Some(index);
        }
    }

    /// Closes the active picker.
    pub fn close_picker(&mut self) {
        self.active_picker = None;
    }

    /// Returns the document color being edited, if any.
    #[must_use]
    pub fn active_color(&self) -> Option<&DocumentColor> {
        self.active_picker.and_then(|i| self.colors.get(i))
    }

    /// Updates the color value for the active picker.
    pub fn update_active_color(&mut self, new_color: ColorRGBA) {
        if let Some(idx) = self.active_picker {
            if let Some(dc) = self.colors.get_mut(idx) {
                dc.color = new_color;
            }
            if let Some(sw) = self.swatches.get_mut(idx) {
                sw.color = new_color;
            }
        }
    }

    /// Clears all colors.
    pub fn clear(&mut self) {
        self.colors.clear();
        self.active_picker = None;
        self.is_loading = false;
        self.swatches.clear();
    }

    /// Returns the color at the given position, if any.
    #[must_use]
    pub fn color_at(&self, pos: Position) -> Option<(usize, &DocumentColor)> {
        self.colors
            .iter()
            .enumerate()
            .find(|(_, dc)| dc.range.contains(pos))
    }

    fn detect_hex_colors(&mut self, line: &str, line_num: u32) {
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
                    let hex_str = &line[start..i];
                    if let Some(color) = ColorRGBA::from_hex(hex_str) {
                        let format = if hex_len <= 4 {
                            ColorFormat::Hex
                        } else {
                            ColorFormat::HexAlpha
                        };
                        self.colors.push(DocumentColor {
                            range: Range::new(
                                Position::new(line_num, start as u32),
                                Position::new(line_num, i as u32),
                            ),
                            color,
                            format,
                        });
                        self.swatches.push(ColorSwatch {
                            line: line_num,
                            column: start as u32,
                            color,
                        });
                    }
                }
            } else {
                i += 1;
            }
        }
    }

    fn detect_rgb_colors(&mut self, line: &str, line_num: u32) {
        for prefix in &["rgba(", "rgb("] {
            let mut search_start = 0;
            while let Some(found) = line[search_start..].find(prefix) {
                let abs_start = search_start + found;
                if let Some(close) = line[abs_start..].find(')') {
                    let end = abs_start + close + 1;
                    let substr = &line[abs_start..end];
                    if let Some(color) = ColorRGBA::from_rgb_string(substr) {
                        let format = if prefix.starts_with("rgba") {
                            ColorFormat::Rgba
                        } else {
                            ColorFormat::Rgb
                        };
                        self.colors.push(DocumentColor {
                            range: Range::new(
                                Position::new(line_num, abs_start as u32),
                                Position::new(line_num, end as u32),
                            ),
                            color,
                            format,
                        });
                        self.swatches.push(ColorSwatch {
                            line: line_num,
                            column: abs_start as u32,
                            color,
                        });
                    }
                    search_start = end;
                } else {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        let c = ColorRGBA::from_hex("#ff8040").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.502).abs() < 0.01);
        let hex = c.to_hex();
        assert!(hex.starts_with("#ff"));
    }

    #[test]
    fn short_hex() {
        let c = ColorRGBA::from_hex("#f00").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g.abs() < 0.01);
    }

    #[test]
    fn rgb_parse() {
        let c = ColorRGBA::from_rgb_string("rgb(255, 128, 0)").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.502).abs() < 0.01);
    }

    #[test]
    fn rgba_parse() {
        let c = ColorRGBA::from_rgb_string("rgba(255, 0, 0, 0.5)").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.a - 0.5).abs() < 0.01);
    }

    #[test]
    fn hsl_conversion() {
        let red = ColorRGBA::new(1.0, 0.0, 0.0, 1.0);
        let hsl = red.to_hsla();
        assert!((hsl.h - 0.0).abs() < 1.0);
        assert!((hsl.s - 1.0).abs() < 0.01);
        assert!((hsl.l - 0.5).abs() < 0.01);

        let back = hsl.to_rgba();
        assert!((back.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn detect_colors_in_buffer() {
        let buffer = Buffer::from_str("color: #ff0000; bg: rgb(0, 128, 255);");
        let mut state = ColorPickerState::default();
        state.detect_colors(&buffer);
        assert_eq!(state.colors.len(), 2);
        assert_eq!(state.swatches.len(), 2);
    }

    #[test]
    fn picker_lifecycle() {
        let mut state = ColorPickerState::default();
        state.set_colors(vec![DocumentColor {
            range: Range::new(Position::new(0, 0), Position::new(0, 7)),
            color: ColorRGBA::new(1.0, 0.0, 0.0, 1.0),
            format: ColorFormat::Hex,
        }]);
        state.open_picker(0);
        assert!(state.active_color().is_some());
        state.close_picker();
        assert!(state.active_color().is_none());
    }

    #[test]
    fn format_output() {
        let c = ColorRGBA::new(1.0, 0.5, 0.0, 1.0);
        assert!(c.to_format(ColorFormat::Hex).starts_with('#'));
        assert!(c.to_format(ColorFormat::Rgb).starts_with("rgb("));
        assert!(c.to_format(ColorFormat::Hsl).starts_with("hsl("));
    }
}
