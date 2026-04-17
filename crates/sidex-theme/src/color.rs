//! RGBA color type with hex parsing, blending, and conversion utilities.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An RGBA color with 8-bit channels.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Errors that can occur when parsing a color from a hex string.
#[derive(Debug, thiserror::Error)]
pub enum ColorParseError {
    #[error("hex color must start with '#'")]
    MissingHash,
    #[error("hex color must be 7 (#rrggbb) or 9 (#rrggbbaa) characters, got {0}")]
    InvalidLength(usize),
    #[error("invalid hex digit in color string: {0}")]
    InvalidHex(#[from] std::num::ParseIntError),
}

impl Color {
    /// Fully opaque black.
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    /// Fully opaque white.
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    /// Fully transparent.
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// Create a color from RGB values with full opacity.
    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a color from RGBA values.
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parse a hex color string (`#rrggbb` or `#rrggbbaa`).
    pub fn from_hex(hex: &str) -> Result<Self, ColorParseError> {
        if !hex.starts_with('#') {
            return Err(ColorParseError::MissingHash);
        }
        let hex = &hex[1..];
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16)?;
                let g = u8::from_str_radix(&hex[2..4], 16)?;
                let b = u8::from_str_radix(&hex[4..6], 16)?;
                Ok(Self::from_rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16)?;
                let g = u8::from_str_radix(&hex[2..4], 16)?;
                let b = u8::from_str_radix(&hex[4..6], 16)?;
                let a = u8::from_str_radix(&hex[6..8], 16)?;
                Ok(Self::from_rgba(r, g, b, a))
            }
            other => Err(ColorParseError::InvalidLength(other + 1)),
        }
    }

    /// Convert to a `#rrggbb` hex string (drops alpha if fully opaque)
    /// or `#rrggbbaa` if alpha is not 255.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// Convert to normalized `[f32; 4]` in `[0.0, 1.0]` range (RGBA order).
    pub fn to_rgba_f32(self) -> [f32; 4] {
        [
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
            f32::from(self.a) / 255.0,
        ]
    }

    /// Linearly blend between `self` and `other` by factor `t` (clamped to `[0, 1]`).
    #[must_use]
    pub fn blend(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let inv = 1.0 - t;
        Self {
            r: lerp_u8(self.r, other.r, t, inv),
            g: lerp_u8(self.g, other.g, t, inv),
            b: lerp_u8(self.b, other.b, t, inv),
            a: lerp_u8(self.a, other.a, t, inv),
        }
    }

    /// Lighten the color by blending toward white.
    /// `amount` is clamped to `[0, 1]` where 1.0 yields pure white.
    #[must_use]
    pub fn lighten(self, amount: f32) -> Self {
        self.blend(Self::WHITE, amount)
    }

    /// Darken the color by blending toward black (preserving alpha).
    /// `amount` is clamped to `[0, 1]` where 1.0 yields pure black.
    #[must_use]
    pub fn darken(self, amount: f32) -> Self {
        let black_with_alpha = Self::from_rgba(0, 0, 0, self.a);
        self.blend(black_with_alpha, amount)
    }
}

fn lerp_u8(a: u8, b: u8, t: f32, inv: f32) -> u8 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let val = (f32::from(a) * inv + f32::from(b) * t).round() as u8;
    val
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

// ── Free-standing convenience functions ─────────────────────────────────────

/// Parse a hex color string, returning `None` on failure.
pub fn hex_to_color(hex: &str) -> Option<Color> {
    Color::from_hex(hex).ok()
}

/// Convert a color to a hex string.
pub fn color_to_hex(color: &Color) -> String {
    color.to_hex()
}

/// Linearly blend two colors. `factor` 0.0 = all `a`, 1.0 = all `b`.
pub fn blend_colors(a: &Color, b: &Color, factor: f32) -> Color {
    a.blend(*b, factor)
}

/// Lighten a color by blending toward white.
pub fn lighten(color: &Color, amount: f32) -> Color {
    color.lighten(amount)
}

/// Darken a color by blending toward black.
pub fn darken(color: &Color, amount: f32) -> Color {
    color.darken(amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_rgb() {
        let c = Color::from_hex("#1a2b3c").unwrap();
        assert_eq!(c, Color::from_rgba(0x1a, 0x2b, 0x3c, 0xff));
    }

    #[test]
    fn parse_hex_rgba() {
        let c = Color::from_hex("#1a2b3c80").unwrap();
        assert_eq!(c, Color::from_rgba(0x1a, 0x2b, 0x3c, 0x80));
    }

    #[test]
    fn parse_hex_missing_hash() {
        assert!(Color::from_hex("1a2b3c").is_err());
    }

    #[test]
    fn parse_hex_bad_length() {
        assert!(Color::from_hex("#1a2b").is_err());
    }

    #[test]
    fn to_hex_opaque() {
        let c = Color::from_rgb(0xff, 0x00, 0xaa);
        assert_eq!(c.to_hex(), "#ff00aa");
    }

    #[test]
    fn to_hex_with_alpha() {
        let c = Color::from_rgba(0xff, 0x00, 0xaa, 0x80);
        assert_eq!(c.to_hex(), "#ff00aa80");
    }

    #[test]
    fn to_rgba_f32_white() {
        let f = Color::WHITE.to_rgba_f32();
        assert_eq!(f, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn to_rgba_f32_black() {
        let f = Color::BLACK.to_rgba_f32();
        assert_eq!(f, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn blend_halfway() {
        let a = Color::from_rgb(0, 0, 0);
        let b = Color::from_rgb(100, 200, 50);
        let mid = a.blend(b, 0.5);
        assert_eq!(mid.r, 50);
        assert_eq!(mid.g, 100);
        assert_eq!(mid.b, 25);
    }

    #[test]
    fn lighten_zero_is_identity() {
        let c = Color::from_rgb(100, 100, 100);
        assert_eq!(c.lighten(0.0), c);
    }

    #[test]
    fn darken_one_is_black() {
        let c = Color::from_rgb(100, 100, 100);
        let d = c.darken(1.0);
        assert_eq!(d.r, 0);
        assert_eq!(d.g, 0);
        assert_eq!(d.b, 0);
    }

    #[test]
    fn serde_roundtrip() {
        let c = Color::from_rgba(0xaa, 0xbb, 0xcc, 0xdd);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"#aabbccdd\"");
        let parsed: Color = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, c);
    }

    #[test]
    fn serde_roundtrip_opaque() {
        let c = Color::from_rgb(0xaa, 0xbb, 0xcc);
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "\"#aabbcc\"");
        let parsed: Color = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, c);
    }
}
