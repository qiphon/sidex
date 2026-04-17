//! Text layout system built on `cosmic-text`.
//!
//! Modeled after Zed's `TextSystem` and `LineLayout`, this module provides:
//!
//! - **Font resolution** with a fallback chain.
//! - **Line shaping** via `cosmic-text` producing [`ShapedRun`] / [`ShapedGlyph`]
//!   results cached per `(text, font_config)` key.
//! - **Line wrapping** at a given pixel width.
//! - **Tab stop expansion**, letter spacing, configurable line height.
//! - **Bidirectional text** support (delegated to `cosmic-text`).
//! - **Layout caching** with an LRU line-layout cache to avoid re-shaping
//!   unchanged lines every frame.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, Weight, Wrap,
};
use linked_hash_map::LinkedHashMap;

use crate::color::Color;

// ---------------------------------------------------------------------------
// Font configuration
// ---------------------------------------------------------------------------

/// Describes a font to resolve.
#[derive(Debug, Clone, PartialEq)]
pub struct FontConfig {
    pub family: String,
    pub size: f32,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub features: Vec<(String, u16)>,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self {
            family: String::from("monospace"),
            size: 14.0,
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
            features: Vec::new(),
        }
    }
}

/// Font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Thin,
    ExtraLight,
    Light,
    Normal,
    Medium,
    SemiBold,
    Bold,
    ExtraBold,
    Black,
}

impl FontWeight {
    pub fn to_cosmic(self) -> Weight {
        match self {
            Self::Thin => Weight(100),
            Self::ExtraLight => Weight(200),
            Self::Light => Weight(300),
            Self::Normal => Weight(400),
            Self::Medium => Weight(500),
            Self::SemiBold => Weight(600),
            Self::Bold => Weight(700),
            Self::ExtraBold => Weight(800),
            Self::Black => Weight(900),
        }
    }
}

/// Font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}

impl FontStyle {
    pub fn to_cosmic(self) -> Style {
        match self {
            Self::Normal => Style::Normal,
            Self::Italic => Style::Italic,
            Self::Oblique => Style::Oblique,
        }
    }
}

// ---------------------------------------------------------------------------
// Line height mode
// ---------------------------------------------------------------------------

/// Controls how line height is calculated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeightMode {
    /// Absolute pixel value.
    Pixels(f32),
    /// Relative to font size (e.g. 1.5 = 150%).
    Relative(f32),
}

impl Default for LineHeightMode {
    fn default() -> Self {
        Self::Relative(1.4)
    }
}

impl LineHeightMode {
    pub fn resolve(self, font_size: f32) -> f32 {
        match self {
            Self::Pixels(px) => px,
            Self::Relative(factor) => font_size * factor,
        }
    }
}

// ---------------------------------------------------------------------------
// Text layout configuration
// ---------------------------------------------------------------------------

/// Full configuration for text layout.
#[derive(Debug, Clone)]
pub struct TextLayoutConfig {
    pub font: FontConfig,
    pub line_height: LineHeightMode,
    pub letter_spacing: f32,
    pub tab_size: u32,
    pub wrap_mode: WrapMode,
    pub wrap_width: Option<f32>,
}

impl Default for TextLayoutConfig {
    fn default() -> Self {
        Self {
            font: FontConfig::default(),
            line_height: LineHeightMode::default(),
            letter_spacing: 0.0,
            tab_size: 4,
            wrap_mode: WrapMode::None,
            wrap_width: None,
        }
    }
}

/// Word-wrap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WrapMode {
    None,
    Word,
    Glyph,
}

// ---------------------------------------------------------------------------
// Shaped output types (like Zed's LineLayout / ShapedRun / ShapedGlyph)
// ---------------------------------------------------------------------------

/// A fully shaped and laid-out line, ready for rendering.
#[derive(Debug, Clone)]
pub struct LineLayout {
    /// Font size used for shaping.
    pub font_size: f32,
    /// Total advance width of the line in pixels.
    pub width: f32,
    /// Ascent above the baseline.
    pub ascent: f32,
    /// Descent below the baseline (positive downward).
    pub descent: f32,
    /// Computed line height.
    pub line_height: f32,
    /// Shaped glyph runs.
    pub runs: Vec<ShapedRun>,
    /// Length of the source text in bytes.
    pub len: usize,
}

impl Default for LineLayout {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            width: 0.0,
            ascent: 0.0,
            descent: 0.0,
            line_height: 20.0,
            runs: Vec::new(),
            len: 0,
        }
    }
}

impl LineLayout {
    /// Returns the x-coordinate for the character at the given byte index.
    pub fn x_for_index(&self, index: usize) -> f32 {
        for run in &self.runs {
            for glyph in &run.glyphs {
                if glyph.byte_index >= index {
                    return glyph.x;
                }
            }
        }
        self.width
    }

    /// Returns the byte index for the character at the given x coordinate.
    pub fn index_for_x(&self, x: f32) -> usize {
        if x >= self.width {
            return self.len;
        }
        let mut prev_index = 0;
        let mut prev_x = 0.0_f32;
        for run in &self.runs {
            for glyph in &run.glyphs {
                if glyph.x >= x {
                    if (glyph.x - x) < (x - prev_x) {
                        return glyph.byte_index;
                    }
                    return prev_index;
                }
                prev_index = glyph.byte_index;
                prev_x = glyph.x;
            }
        }
        self.len
    }

    /// Returns wrap boundaries for the given wrap width.
    pub fn compute_wrap_boundaries(&self, wrap_width: f32) -> Vec<WrapBoundary> {
        let mut boundaries = Vec::new();
        let mut last_boundary_x = 0.0_f32;
        let mut last_space: Option<WrapBoundary> = None;

        for (run_ix, run) in self.runs.iter().enumerate() {
            for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
                if glyph.is_whitespace {
                    last_space = Some(WrapBoundary { run_ix, glyph_ix });
                }
                let width = glyph.x + glyph.advance - last_boundary_x;
                if width > wrap_width {
                    if let Some(boundary) = last_space.take() {
                        let bx = self.runs[boundary.run_ix].glyphs[boundary.glyph_ix].x
                            + self.runs[boundary.run_ix].glyphs[boundary.glyph_ix].advance;
                        last_boundary_x = bx;
                        boundaries.push(boundary);
                    } else {
                        let boundary = WrapBoundary { run_ix, glyph_ix };
                        last_boundary_x = glyph.x;
                        boundaries.push(boundary);
                    }
                }
            }
        }
        boundaries
    }
}

/// A wrap boundary at a specific run/glyph index.
#[derive(Debug, Clone, Copy)]
pub struct WrapBoundary {
    pub run_ix: usize,
    pub glyph_ix: usize,
}

/// A run of shaped glyphs sharing the same font.
#[derive(Debug, Clone)]
pub struct ShapedRun {
    pub font_family: String,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub glyphs: Vec<ShapedGlyph>,
}

/// A single shaped glyph ready for rasterization.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    pub cache_key: CacheKey,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub byte_index: usize,
    pub is_emoji: bool,
    pub is_whitespace: bool,
}

// ---------------------------------------------------------------------------
// Styled text run (input to layout)
// ---------------------------------------------------------------------------

/// A run of text with associated style, used as input to line shaping.
#[derive(Debug, Clone)]
pub struct StyledTextRun {
    pub text: String,
    pub color: Color,
    pub weight: FontWeight,
    pub style: FontStyle,
}

// ---------------------------------------------------------------------------
// Layout cache key
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct LayoutCacheKey {
    text: String,
    font_family: String,
    font_size_bits: u32,
    weight: FontWeight,
    style: FontStyle,
    letter_spacing_bits: u32,
    tab_size: u32,
}

impl PartialEq for LayoutCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.font_family == other.font_family
            && self.font_size_bits == other.font_size_bits
            && self.weight == other.weight
            && self.style == other.style
            && self.letter_spacing_bits == other.letter_spacing_bits
            && self.tab_size == other.tab_size
    }
}

impl Eq for LayoutCacheKey {}

impl Hash for LayoutCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.text.hash(state);
        self.font_family.hash(state);
        self.font_size_bits.hash(state);
        self.weight.hash(state);
        self.style.hash(state);
        self.letter_spacing_bits.hash(state);
        self.tab_size.hash(state);
    }
}

// ---------------------------------------------------------------------------
// TextLayoutSystem
// ---------------------------------------------------------------------------

const DEFAULT_CACHE_CAPACITY: usize = 4096;

/// The text layout system. Owns the `cosmic-text` `FontSystem` and provides
/// line shaping with caching.
pub struct TextLayoutSystem {
    font_system: FontSystem,
    layout_cache: LinkedHashMap<LayoutCacheKey, Arc<LineLayout>>,
    cache_capacity: usize,
}

impl TextLayoutSystem {
    /// Creates a new text layout system with the system font database.
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            layout_cache: LinkedHashMap::new(),
            cache_capacity: DEFAULT_CACHE_CAPACITY,
        }
    }

    /// Creates a new text layout system with a provided `FontSystem`.
    pub fn with_font_system(font_system: FontSystem) -> Self {
        Self {
            font_system,
            layout_cache: LinkedHashMap::new(),
            cache_capacity: DEFAULT_CACHE_CAPACITY,
        }
    }

    /// Returns a mutable reference to the underlying font system.
    pub fn font_system_mut(&mut self) -> &mut FontSystem {
        &mut self.font_system
    }

    /// Returns a reference to the underlying font system.
    pub fn font_system(&self) -> &FontSystem {
        &self.font_system
    }

    /// Sets the maximum number of cached line layouts.
    pub fn set_cache_capacity(&mut self, capacity: usize) {
        self.cache_capacity = capacity;
    }

    /// Clears the layout cache.
    pub fn clear_cache(&mut self) {
        self.layout_cache.clear();
    }

    /// Shapes a single line of text and returns its layout.
    ///
    /// Results are cached — repeated calls with the same parameters
    /// return a cached `Arc<LineLayout>`.
    #[allow(clippy::cast_precision_loss)]
    pub fn layout_line(&mut self, text: &str, config: &TextLayoutConfig) -> Arc<LineLayout> {
        let key = LayoutCacheKey {
            text: text.to_string(),
            font_family: config.font.family.clone(),
            font_size_bits: config.font.size.to_bits(),
            weight: config.font.weight,
            style: config.font.style,
            letter_spacing_bits: config.letter_spacing.to_bits(),
            tab_size: config.tab_size,
        };

        if let Some(cached) = self.layout_cache.get_refresh(&key) {
            return Arc::clone(cached);
        }

        let layout = self.shape_line(text, config);
        let layout = Arc::new(layout);

        while self.layout_cache.len() >= self.cache_capacity {
            self.layout_cache.pop_front();
        }
        self.layout_cache.insert(key, Arc::clone(&layout));

        layout
    }

    /// Shapes a line with multiple styled runs, returning a unified layout.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn layout_styled_line(
        &mut self,
        runs: &[StyledTextRun],
        config: &TextLayoutConfig,
    ) -> Arc<LineLayout> {
        let full_text: String = runs.iter().map(|r| r.text.as_str()).collect();
        let line_height = config.line_height.resolve(config.font.size);
        let metrics = Metrics::new(config.font.size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_tab_width(&mut self.font_system, config.tab_size as u16);

        let wrap = match config.wrap_mode {
            WrapMode::None => Wrap::None,
            WrapMode::Word => Wrap::Word,
            WrapMode::Glyph => Wrap::Glyph,
        };
        buffer.set_wrap(&mut self.font_system, wrap);
        if let Some(w) = config.wrap_width {
            buffer.set_size(&mut self.font_system, Some(w), None);
        }

        let rich_spans: Vec<(&str, Attrs)> = runs
            .iter()
            .map(|run| {
                let attrs = Attrs::new()
                    .family(Family::Name(&config.font.family))
                    .weight(run.weight.to_cosmic())
                    .style(run.style.to_cosmic());
                (run.text.as_str(), attrs)
            })
            .collect();

        buffer.set_rich_text(
            &mut self.font_system,
            rich_spans,
            Attrs::new().family(Family::Name(&config.font.family)),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut shaped_runs = Vec::new();
        let mut total_width = 0.0_f32;
        let mut ascent = 0.0_f32;

        for layout_run in buffer.layout_runs() {
            let mut glyphs = Vec::new();
            ascent = ascent.max(layout_run.line_y);
            for glyph in layout_run.glyphs {
                let advance = glyph.w + config.letter_spacing;
                glyphs.push(ShapedGlyph {
                    cache_key: glyph.physical((0., 0.), 1.0).cache_key,
                    x: glyph.x + config.letter_spacing * glyph.start as f32,
                    y: layout_run.line_y,
                    advance,
                    byte_index: glyph.start,
                    is_emoji: glyph.color_opt.is_some(),
                    is_whitespace: full_text
                        .as_bytes()
                        .get(glyph.start)
                        .is_some_and(|&b| b == b' ' || b == b'\t'),
                });
                let end_x = glyph.x + advance;
                if end_x > total_width {
                    total_width = end_x;
                }
            }
            if !glyphs.is_empty() {
                shaped_runs.push(ShapedRun {
                    font_family: config.font.family.clone(),
                    font_weight: config.font.weight,
                    font_style: config.font.style,
                    glyphs,
                });
            }
        }

        let layout_ascent = metrics.font_size;
        let layout_descent = line_height - layout_ascent;

        Arc::new(LineLayout {
            font_size: config.font.size,
            width: total_width,
            ascent: layout_ascent,
            descent: layout_descent,
            line_height,
            runs: shaped_runs,
            len: full_text.len(),
        })
    }

    /// Internal shaping for a plain-text line.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn shape_line(&mut self, text: &str, config: &TextLayoutConfig) -> LineLayout {
        let line_height = config.line_height.resolve(config.font.size);
        let metrics = Metrics::new(config.font.size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_tab_width(&mut self.font_system, config.tab_size as u16);

        let wrap = match config.wrap_mode {
            WrapMode::None => Wrap::None,
            WrapMode::Word => Wrap::Word,
            WrapMode::Glyph => Wrap::Glyph,
        };
        buffer.set_wrap(&mut self.font_system, wrap);
        if let Some(w) = config.wrap_width {
            buffer.set_size(&mut self.font_system, Some(w), None);
        }

        let attrs = Attrs::new()
            .family(Family::Name(&config.font.family))
            .weight(config.font.weight.to_cosmic())
            .style(config.font.style.to_cosmic());
        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut shaped_runs = Vec::new();
        let mut total_width = 0.0_f32;

        for layout_run in buffer.layout_runs() {
            let mut glyphs = Vec::new();
            for glyph in layout_run.glyphs {
                let advance = glyph.w + config.letter_spacing;
                glyphs.push(ShapedGlyph {
                    cache_key: glyph.physical((0., 0.), 1.0).cache_key,
                    x: glyph.x,
                    y: layout_run.line_y,
                    advance,
                    byte_index: glyph.start,
                    is_emoji: glyph.color_opt.is_some(),
                    is_whitespace: text
                        .as_bytes()
                        .get(glyph.start)
                        .is_some_and(|&b| b == b' ' || b == b'\t'),
                });
                let end_x = glyph.x + advance;
                if end_x > total_width {
                    total_width = end_x;
                }
            }
            if !glyphs.is_empty() {
                shaped_runs.push(ShapedRun {
                    font_family: config.font.family.clone(),
                    font_weight: config.font.weight,
                    font_style: config.font.style,
                    glyphs,
                });
            }
        }

        let layout_ascent = metrics.font_size;
        let layout_descent = line_height - layout_ascent;

        LineLayout {
            font_size: config.font.size,
            width: total_width,
            ascent: layout_ascent,
            descent: layout_descent,
            line_height,
            runs: shaped_runs,
            len: text.len(),
        }
    }

    /// Measures the advance width of a single character.
    #[allow(clippy::cast_precision_loss)]
    pub fn measure_char(&mut self, ch: char, config: &FontConfig) -> f32 {
        let mut buf = [0u8; 4];
        let text = ch.encode_utf8(&mut buf);
        let layout_cfg = TextLayoutConfig {
            font: config.clone(),
            ..Default::default()
        };
        let layout = self.shape_line(text, &layout_cfg);
        layout.width
    }

    /// Measures the advance width of the `m` character (em-width).
    pub fn em_width(&mut self, config: &FontConfig) -> f32 {
        self.measure_char('m', config)
    }

    /// Measures the advance width of the `0` character (ch-width).
    pub fn ch_width(&mut self, config: &FontConfig) -> f32 {
        self.measure_char('0', config)
    }
}

impl Default for TextLayoutSystem {
    fn default() -> Self {
        Self::new()
    }
}
