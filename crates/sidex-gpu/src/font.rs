//! Font management for the `SideX` GPU renderer.
//!
//! Provides font loading, measurement, fallback chains, ligature detection,
//! and ID-based access on top of `cosmic-text`'s `FontSystem`.

use std::collections::HashMap;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping};

/// Opaque identifier for a loaded font configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(pub(crate) u32);

/// Font weight variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Thin,
    Light,
    Normal,
    Medium,
    Bold,
    Black,
}

impl FontWeight {
    pub fn to_cosmic(self) -> cosmic_text::Weight {
        match self {
            Self::Thin => cosmic_text::Weight(100),
            Self::Light => cosmic_text::Weight(300),
            Self::Normal => cosmic_text::Weight(400),
            Self::Medium => cosmic_text::Weight(500),
            Self::Bold => cosmic_text::Weight(700),
            Self::Black => cosmic_text::Weight(900),
        }
    }
}

/// Font style variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl FontStyle {
    pub fn to_cosmic(self) -> cosmic_text::Style {
        match self {
            Self::Normal => cosmic_text::Style::Normal,
            Self::Italic => cosmic_text::Style::Italic,
            Self::Oblique => cosmic_text::Style::Oblique,
        }
    }
}

/// Configuration describing a font to load.
#[derive(Debug, Clone, PartialEq)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: String::from("monospace"),
            size: 14.0,
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
        }
    }
}

/// Metrics returned by [`measure_text`](FontManager::measure_text).
#[derive(Debug, Clone, Copy)]
pub struct TextMetrics {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
    /// Per-character advance widths (empty if not requested).
    pub advance_widths: [f32; 0],
}

/// Detailed per-string measurement including per-glyph advances.
#[derive(Debug, Clone)]
pub struct DetailedTextMetrics {
    pub width: f32,
    pub height: f32,
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
    pub advance_widths: Vec<f32>,
}

// ---------------------------------------------------------------------------
// FontFamily — a set of weight/style variants for a family
// ---------------------------------------------------------------------------

/// A complete font family with all style variants.
#[derive(Debug, Clone)]
pub struct FontFamily {
    pub name: String,
    pub regular: FontId,
    pub bold: Option<FontId>,
    pub italic: Option<FontId>,
    pub bold_italic: Option<FontId>,
}

impl FontFamily {
    /// Returns the best matching font id for the given weight and style.
    pub fn resolve(&self, weight: FontWeight, style: FontStyle) -> FontId {
        match (weight, style) {
            (FontWeight::Bold, FontStyle::Italic) | (FontWeight::Black, FontStyle::Italic) => {
                self.bold_italic.or(self.bold).or(self.italic).unwrap_or(self.regular)
            }
            (FontWeight::Bold, _) | (FontWeight::Black, _) => {
                self.bold.unwrap_or(self.regular)
            }
            (_, FontStyle::Italic) | (_, FontStyle::Oblique) => {
                self.italic.unwrap_or(self.regular)
            }
            _ => self.regular,
        }
    }
}

// ---------------------------------------------------------------------------
// FontMetrics — precise font metrics
// ---------------------------------------------------------------------------

/// Precise metrics for a loaded font at a specific size.
#[derive(Debug, Clone, Copy)]
pub struct FontMetrics {
    /// Ascent above the baseline in pixels.
    pub ascent: f32,
    /// Descent below the baseline in pixels (positive downward).
    pub descent: f32,
    /// Computed line height in pixels.
    pub line_height: f32,
    /// Maximum horizontal advance of any glyph.
    pub max_advance: f32,
    /// The font's em size in pixels.
    pub em_size: f32,
    /// Monospace character width (advance of '0').
    pub char_width: f32,
    /// Underline position below the baseline.
    pub underline_offset: f32,
    /// Underline thickness.
    pub underline_thickness: f32,
    /// Strikethrough position above the baseline.
    pub strikethrough_offset: f32,
}

// ---------------------------------------------------------------------------
// Fallback chain
// ---------------------------------------------------------------------------

/// An ordered list of font families to try when resolving glyphs.
/// If the primary font lacks a glyph, each fallback is tried in order.
#[derive(Debug, Clone)]
pub struct FontFallbackChain {
    pub families: Vec<String>,
}

impl FontFallbackChain {
    pub fn new(primary: impl Into<String>) -> Self {
        Self {
            families: vec![primary.into()],
        }
    }

    pub fn with_fallback(mut self, family: impl Into<String>) -> Self {
        self.families.push(family.into());
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.families.iter().map(String::as_str)
    }
}

impl Default for FontFallbackChain {
    fn default() -> Self {
        Self::new("monospace")
            .with_fallback("Noto Sans")
            .with_fallback("Apple Color Emoji")
            .with_fallback("Segoe UI Emoji")
    }
}

// ---------------------------------------------------------------------------
// Ligature sequences
// ---------------------------------------------------------------------------

/// Well-known programming ligature sequences.
pub const LIGATURE_SEQUENCES: &[&str] = &[
    "!=", "!==", "=>", "->", "<-", ">=", "<=", "==", "===",
    "::", "&&", "||", ">>", "<<", "|>", "<|", "++", "--",
    "**", "..", "...", "~>", "<~", ">>>", "<<=", ">>=",
    "/*", "*/", "//", "#{", "#[", "#!", "??", "?.",
];

/// Checks whether a string starts with a known ligature sequence.
pub fn detect_ligature(text: &str) -> Option<&'static str> {
    for &lig in LIGATURE_SEQUENCES {
        if text.starts_with(lig) {
            return Some(lig);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Stored font info
// ---------------------------------------------------------------------------

struct LoadedFont {
    config: FontConfig,
    cached_char_width: f32,
    cached_line_height: f32,
    metrics: FontMetrics,
    fallback_chain: FontFallbackChain,
}

// ---------------------------------------------------------------------------
// FontManager
// ---------------------------------------------------------------------------

/// Manages font loading, caching, and measurement.
///
/// Wraps a `cosmic-text` `FontSystem` and provides ID-based font access.
pub struct FontManager {
    font_system: FontSystem,
    fonts: HashMap<FontId, LoadedFont>,
    families: HashMap<String, FontFamily>,
    next_id: u32,
    /// Line height multiplier (1.0 to 2.0).
    line_height_multiplier: f32,
    /// Extra letter spacing in pixels.
    letter_spacing: f32,
}

impl FontManager {
    /// Creates a new font manager with the system font database loaded.
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            fonts: HashMap::new(),
            families: HashMap::new(),
            next_id: 0,
            line_height_multiplier: 1.4,
            letter_spacing: 0.0,
        }
    }

    /// Creates a font manager with an existing `FontSystem`.
    pub fn with_font_system(font_system: FontSystem) -> Self {
        Self {
            font_system,
            fonts: HashMap::new(),
            families: HashMap::new(),
            next_id: 0,
            line_height_multiplier: 1.4,
            letter_spacing: 0.0,
        }
    }

    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    pub fn font_system(&self) -> &FontSystem {
        &self.font_system
    }

    /// Sets the global line-height multiplier (relative to font size).
    pub fn set_line_height_multiplier(&mut self, multiplier: f32) {
        self.line_height_multiplier = multiplier.clamp(1.0, 3.0);
    }

    /// Sets the global letter spacing in pixels.
    pub fn set_letter_spacing(&mut self, spacing: f32) {
        self.letter_spacing = spacing;
    }

    pub fn line_height_multiplier(&self) -> f32 {
        self.line_height_multiplier
    }

    pub fn letter_spacing(&self) -> f32 {
        self.letter_spacing
    }

    /// Loads a font configuration and returns a [`FontId`].
    pub fn load_font(&mut self, config: &FontConfig) -> FontId {
        let id = FontId(self.next_id);
        self.next_id += 1;

        let line_height = config.size * self.line_height_multiplier;
        let char_width = self.measure_char_advance('0', config);
        let em_size = config.size;
        let ascent = config.size;
        let descent = line_height - ascent;

        let metrics = FontMetrics {
            ascent,
            descent,
            line_height,
            max_advance: char_width,
            em_size,
            char_width,
            underline_offset: ascent + 2.0,
            underline_thickness: 1.0,
            strikethrough_offset: ascent * 0.5,
        };

        self.fonts.insert(
            id,
            LoadedFont {
                config: config.clone(),
                cached_char_width: char_width,
                cached_line_height: line_height,
                metrics,
                fallback_chain: FontFallbackChain::new(&config.family),
            },
        );

        id
    }

    /// Loads a complete font family (regular + variants).
    pub fn load_family(&mut self, name: &str, size: f32) -> FontFamily {
        let regular = self.load_font(&FontConfig {
            family: name.to_string(),
            size,
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
        });
        let bold = Some(self.load_font(&FontConfig {
            family: name.to_string(),
            size,
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        }));
        let italic = Some(self.load_font(&FontConfig {
            family: name.to_string(),
            size,
            weight: FontWeight::Normal,
            style: FontStyle::Italic,
        }));
        let bold_italic = Some(self.load_font(&FontConfig {
            family: name.to_string(),
            size,
            weight: FontWeight::Bold,
            style: FontStyle::Italic,
        }));

        let family = FontFamily {
            name: name.to_string(),
            regular,
            bold,
            italic,
            bold_italic,
        };

        self.families.insert(name.to_string(), family.clone());
        family
    }

    /// Gets a loaded font family by name.
    pub fn get_family(&self, name: &str) -> Option<&FontFamily> {
        self.families.get(name)
    }

    /// Sets a custom fallback chain for a specific font.
    pub fn set_fallback_chain(&mut self, font_id: FontId, chain: FontFallbackChain) {
        if let Some(loaded) = self.fonts.get_mut(&font_id) {
            loaded.fallback_chain = chain;
        }
    }

    /// Returns the fallback chain for a font.
    pub fn fallback_chain(&self, font_id: FontId) -> Option<&FontFallbackChain> {
        self.fonts.get(&font_id).map(|f| &f.fallback_chain)
    }

    /// Returns the configuration for a previously loaded font.
    pub fn get_config(&self, font_id: FontId) -> Option<&FontConfig> {
        self.fonts.get(&font_id).map(|f| &f.config)
    }

    /// Returns precise font metrics for a loaded font.
    pub fn get_metrics(&self, font_id: FontId) -> Option<&FontMetrics> {
        self.fonts.get(&font_id).map(|f| &f.metrics)
    }

    /// Measures a string of text using the given font.
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_text(&mut self, font_id: FontId, text: &str) -> Option<TextMetrics> {
        let loaded = self.fonts.get(&font_id)?;
        let config = &loaded.config;
        let line_height = loaded.cached_line_height;
        let metrics = Metrics::new(config.size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        let attrs = Attrs::new()
            .family(Family::Name(&config.family))
            .weight(config.weight.to_cosmic())
            .style(config.style.to_cosmic());
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut width = 0.0_f32;
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let end_x = glyph.x + glyph.w;
                if end_x > width {
                    width = end_x;
                }
            }
        }

        let ascent = config.size;
        let descent = line_height - ascent;

        Some(TextMetrics {
            width,
            height: line_height,
            ascent,
            descent,
            line_height,
            advance_widths: [],
        })
    }

    /// Measures text with per-glyph advance widths.
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_text_detailed(
        &mut self,
        font_id: FontId,
        text: &str,
    ) -> Option<DetailedTextMetrics> {
        let loaded = self.fonts.get(&font_id)?;
        let config = &loaded.config;
        let line_height = loaded.cached_line_height;
        let metrics = Metrics::new(config.size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        let attrs = Attrs::new()
            .family(Family::Name(&config.family))
            .weight(config.weight.to_cosmic())
            .style(config.style.to_cosmic());
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut width = 0.0_f32;
        let mut advance_widths = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                advance_widths.push(glyph.w + self.letter_spacing);
                let end_x = glyph.x + glyph.w;
                if end_x > width {
                    width = end_x;
                }
            }
        }

        let ascent = config.size;
        let descent = line_height - ascent;

        Some(DetailedTextMetrics {
            width,
            height: line_height,
            ascent,
            descent,
            line_height,
            advance_widths,
        })
    }

    /// Converts em units to pixels for the given font size.
    pub fn em_to_px(em: f32, font_size: f32) -> f32 {
        em * font_size
    }

    /// Returns the monospace character width for the given font.
    pub fn char_width(&self, font_id: FontId) -> f32 {
        self.fonts
            .get(&font_id)
            .map_or(8.0, |f| f.cached_char_width)
    }

    /// Returns the line height for the given font.
    pub fn line_height(&self, font_id: FontId) -> f32 {
        self.fonts
            .get(&font_id)
            .map_or(20.0, |f| f.cached_line_height)
    }

    /// Measures the advance width of a single character.
    #[allow(clippy::cast_precision_loss)]
    fn measure_char_advance(&mut self, ch: char, config: &FontConfig) -> f32 {
        let mut buf = [0u8; 4];
        let text = ch.encode_utf8(&mut buf);
        let line_height = config.size * self.line_height_multiplier;
        let metrics = Metrics::new(config.size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);

        let attrs = Attrs::new()
            .family(Family::Name(&config.family))
            .weight(config.weight.to_cosmic())
            .style(config.style.to_cosmic());
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut width = 0.0_f32;
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                width = width.max(glyph.w);
            }
        }
        if width < 1.0 {
            config.size * 0.6
        } else {
            width
        }
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}
