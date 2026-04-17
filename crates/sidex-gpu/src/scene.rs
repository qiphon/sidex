//! Scene graph with draw call batching and z-ordered layer dispatch.
//!
//! Modeled after Zed's `gpui::scene`, the [`Scene`] collects rendering
//! primitives (quads, shadows, underlines, monochrome sprites, subpixel
//! sprites, color sprites) during a frame, sorts them by draw order, and
//! yields [`PrimitiveBatch`] slices ready for the GPU.
//!
//! ## Layer system
//!
//! Layers provide z-ordering. Push a layer before painting a group of
//! primitives, then pop it when done. The layer stack determines draw order:
//! primitives in earlier layers are drawn first (behind later layers).
//!
//! The canonical editor layer order is:
//! ```text
//! Background < Selections < Text < Cursors < Overlays < Popups
//! ```

use crate::color::Color;

/// Numeric draw order — lower values paint first (behind).
pub type DrawOrder = u32;

// ---------------------------------------------------------------------------
// Layer constants
// ---------------------------------------------------------------------------

/// Pre-defined layer indices for the editor rendering pipeline.
/// Each layer is a range of draw-order space so that primitives within
/// a layer can still be ordered relative to each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u32)]
pub enum Layer {
    Background = 0,
    LineHighlights = 100,
    Selections = 200,
    Text = 300,
    Decorations = 400,
    InlayHints = 500,
    StickyHeaders = 600,
    Cursors = 700,
    BracketHighlights = 800,
    Gutter = 900,
    Minimap = 1000,
    Scrollbars = 1100,
    ScrollShadow = 1200,
    Overlays = 1300,
    Popups = 1400,
}

impl Layer {
    pub fn order(self) -> DrawOrder {
        self as DrawOrder
    }
}

// ---------------------------------------------------------------------------
// Content mask (clip region)
// ---------------------------------------------------------------------------

/// A rectangular clip region that primitives are tested against.
#[derive(Debug, Clone, Copy)]
pub struct ContentMask {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ContentMask {
    pub fn full_screen(width: f32, height: f32) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    /// Returns true if the given bounds are completely outside this mask.
    pub fn clips(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        x + w <= self.x || x >= self.x + self.width || y + h <= self.y || y >= self.y + self.height
    }
}

impl Default for ContentMask {
    fn default() -> Self {
        Self::full_screen(f32::MAX, f32::MAX)
    }
}

// ---------------------------------------------------------------------------
// Quad (rounded rectangle with optional border)
// ---------------------------------------------------------------------------

/// A filled rectangle with optional corner radius and border.
#[derive(Debug, Clone, Copy)]
pub struct Quad {
    pub order: DrawOrder,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: Color,
    pub corner_radius: f32,
    pub border_color: Color,
    pub border_width: f32,
    pub content_mask: ContentMask,
}

// ---------------------------------------------------------------------------
// Shadow
// ---------------------------------------------------------------------------

/// A box shadow rendered behind a quad.
#[derive(Debug, Clone, Copy)]
pub struct Shadow {
    pub order: DrawOrder,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub corner_radius: f32,
    pub color: Color,
    pub blur_radius: f32,
    pub spread: f32,
    pub content_mask: ContentMask,
}

// ---------------------------------------------------------------------------
// Underline
// ---------------------------------------------------------------------------

/// An underline or strikethrough line segment.
#[derive(Debug, Clone, Copy)]
pub struct Underline {
    pub order: DrawOrder,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub thickness: f32,
    pub color: Color,
    pub wavy: bool,
    pub content_mask: ContentMask,
}

// ---------------------------------------------------------------------------
// Sprite types (for text atlas glyphs)
// ---------------------------------------------------------------------------

/// A monochrome (alpha-mask) glyph sprite.
#[derive(Debug, Clone, Copy)]
pub struct MonochromeSprite {
    pub order: DrawOrder,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub uv_left: f32,
    pub uv_top: f32,
    pub uv_right: f32,
    pub uv_bottom: f32,
    pub color: Color,
    pub content_mask: ContentMask,
}

/// A subpixel-antialiased glyph sprite (RGB channels encode coverage).
#[derive(Debug, Clone, Copy)]
pub struct SubpixelSprite {
    pub order: DrawOrder,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub uv_left: f32,
    pub uv_top: f32,
    pub uv_right: f32,
    pub uv_bottom: f32,
    pub color: Color,
    pub content_mask: ContentMask,
}

/// A full-color (RGBA) sprite — emoji, images.
#[derive(Debug, Clone, Copy)]
pub struct PolychromeSprite {
    pub order: DrawOrder,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub uv_left: f32,
    pub uv_top: f32,
    pub uv_right: f32,
    pub uv_bottom: f32,
    pub content_mask: ContentMask,
}

// ---------------------------------------------------------------------------
// Primitive enum
// ---------------------------------------------------------------------------

/// A tagged union of all renderable primitives.
#[derive(Debug, Clone, Copy)]
pub enum Primitive {
    Quad(Quad),
    Shadow(Shadow),
    Underline(Underline),
    MonochromeSprite(MonochromeSprite),
    SubpixelSprite(SubpixelSprite),
    PolychromeSprite(PolychromeSprite),
}

impl Primitive {
    pub fn order(&self) -> DrawOrder {
        match self {
            Self::Quad(q) => q.order,
            Self::Shadow(s) => s.order,
            Self::Underline(u) => u.order,
            Self::MonochromeSprite(s) => s.order,
            Self::SubpixelSprite(s) => s.order,
            Self::PolychromeSprite(s) => s.order,
        }
    }
}

// ---------------------------------------------------------------------------
// PrimitiveBatch — what the GPU renderer consumes
// ---------------------------------------------------------------------------

/// A contiguous slice of sorted, same-type primitives ready for a single
/// draw call. The renderer iterates over batches in draw order.
#[derive(Debug)]
pub enum PrimitiveBatch<'a> {
    Shadows(&'a [Shadow]),
    Quads(&'a [Quad]),
    Underlines(&'a [Underline]),
    MonochromeSprites(&'a [MonochromeSprite]),
    SubpixelSprites(&'a [SubpixelSprite]),
    PolychromeSprites(&'a [PolychromeSprite]),
}

// ---------------------------------------------------------------------------
// PrimitiveKind — used for batch iteration ordering
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PrimitiveKind {
    Shadow,
    Quad,
    Underline,
    MonochromeSprite,
    SubpixelSprite,
    PolychromeSprite,
}

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

/// Collects all rendering primitives for a single frame, then sorts and
/// batches them for efficient GPU dispatch.
///
/// Usage:
/// ```ignore
/// scene.clear();
/// scene.push_layer(Layer::Background);
/// scene.insert_quad(Quad { ... });
/// scene.pop_layer();
/// scene.push_layer(Layer::Text);
/// scene.insert_monochrome_sprite(MonochromeSprite { ... });
/// scene.pop_layer();
/// scene.finish();
/// for batch in scene.batches() { /* render */ }
/// ```
#[derive(Default)]
pub struct Scene {
    layer_stack: Vec<DrawOrder>,
    next_order: DrawOrder,

    pub shadows: Vec<Shadow>,
    pub quads: Vec<Quad>,
    pub underlines: Vec<Underline>,
    pub monochrome_sprites: Vec<MonochromeSprite>,
    pub subpixel_sprites: Vec<SubpixelSprite>,
    pub polychrome_sprites: Vec<PolychromeSprite>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resets the scene for a new frame.
    pub fn clear(&mut self) {
        self.layer_stack.clear();
        self.next_order = 0;
        self.shadows.clear();
        self.quads.clear();
        self.underlines.clear();
        self.monochrome_sprites.clear();
        self.subpixel_sprites.clear();
        self.polychrome_sprites.clear();
    }

    /// Total number of primitives across all types.
    pub fn len(&self) -> usize {
        self.shadows.len()
            + self.quads.len()
            + self.underlines.len()
            + self.monochrome_sprites.len()
            + self.subpixel_sprites.len()
            + self.polychrome_sprites.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // -- Layer management ---------------------------------------------------

    /// Pushes a rendering layer. All primitives inserted while this layer
    /// is active receive its draw order.
    pub fn push_layer(&mut self, layer: Layer) {
        self.layer_stack.push(layer.order());
    }

    /// Pushes a layer with a custom draw order value.
    pub fn push_layer_order(&mut self, order: DrawOrder) {
        self.layer_stack.push(order);
    }

    /// Pops the most recent layer.
    pub fn pop_layer(&mut self) {
        self.layer_stack.pop();
    }

    /// Returns the current draw order (from the layer stack, or an
    /// auto-incrementing counter if no layer is active).
    fn current_order(&mut self) -> DrawOrder {
        self.layer_stack.last().copied().unwrap_or_else(|| {
            let o = self.next_order;
            self.next_order += 1;
            o
        })
    }

    // -- Primitive insertion ------------------------------------------------

    pub fn insert_quad(&mut self, mut quad: Quad) {
        if quad
            .content_mask
            .clips(quad.x, quad.y, quad.width, quad.height)
        {
            return;
        }
        quad.order = self.current_order();
        self.quads.push(quad);
    }

    pub fn insert_shadow(&mut self, mut shadow: Shadow) {
        let total_spread = shadow.blur_radius + shadow.spread;
        if shadow.content_mask.clips(
            shadow.x - total_spread,
            shadow.y - total_spread,
            shadow.width + total_spread * 2.0,
            shadow.height + total_spread * 2.0,
        ) {
            return;
        }
        shadow.order = self.current_order();
        self.shadows.push(shadow);
    }

    pub fn insert_underline(&mut self, mut underline: Underline) {
        if underline.content_mask.clips(
            underline.x,
            underline.y,
            underline.width,
            underline.thickness,
        ) {
            return;
        }
        underline.order = self.current_order();
        self.underlines.push(underline);
    }

    pub fn insert_monochrome_sprite(&mut self, mut sprite: MonochromeSprite) {
        if sprite
            .content_mask
            .clips(sprite.x, sprite.y, sprite.width, sprite.height)
        {
            return;
        }
        sprite.order = self.current_order();
        self.monochrome_sprites.push(sprite);
    }

    pub fn insert_subpixel_sprite(&mut self, mut sprite: SubpixelSprite) {
        if sprite
            .content_mask
            .clips(sprite.x, sprite.y, sprite.width, sprite.height)
        {
            return;
        }
        sprite.order = self.current_order();
        self.subpixel_sprites.push(sprite);
    }

    pub fn insert_polychrome_sprite(&mut self, mut sprite: PolychromeSprite) {
        if sprite
            .content_mask
            .clips(sprite.x, sprite.y, sprite.width, sprite.height)
        {
            return;
        }
        sprite.order = self.current_order();
        self.polychrome_sprites.push(sprite);
    }

    // -- Convenience helpers ------------------------------------------------

    /// Inserts a filled rectangle (shorthand for a Quad with no border).
    pub fn insert_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        corner_radius: f32,
    ) {
        self.insert_quad(Quad {
            order: 0,
            x,
            y,
            width,
            height,
            color,
            corner_radius,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            content_mask: ContentMask::default(),
        });
    }

    // -- Finalize -----------------------------------------------------------

    /// Sorts all primitive lists by draw order for correct rendering.
    /// Must be called after all primitives have been inserted and before
    /// calling [`batches`].
    pub fn finish(&mut self) {
        self.shadows.sort_by_key(|s| s.order);
        self.quads.sort_by_key(|q| q.order);
        self.underlines.sort_by_key(|u| u.order);
        self.monochrome_sprites.sort_by_key(|s| s.order);
        self.subpixel_sprites.sort_by_key(|s| s.order);
        self.polychrome_sprites.sort_by_key(|s| s.order);
    }

    /// Returns an iterator over [`PrimitiveBatch`] values in draw order.
    ///
    /// The iterator merges across primitive types: it picks whichever type
    /// has the lowest current draw order, emits a contiguous batch of that
    /// type at that order, then advances. This ensures correct interleaving
    /// (e.g., a shadow at order 5 is drawn before a quad at order 10).
    pub fn batches(&self) -> BatchIterator<'_> {
        BatchIterator {
            shadows_start: 0,
            shadows: &self.shadows,
            quads_start: 0,
            quads: &self.quads,
            underlines_start: 0,
            underlines: &self.underlines,
            monochrome_start: 0,
            monochrome: &self.monochrome_sprites,
            subpixel_start: 0,
            subpixel: &self.subpixel_sprites,
            polychrome_start: 0,
            polychrome: &self.polychrome_sprites,
        }
    }
}

// ---------------------------------------------------------------------------
// BatchIterator
// ---------------------------------------------------------------------------

/// Iterates over the scene's primitives in draw-order, yielding contiguous
/// same-type batches.
pub struct BatchIterator<'a> {
    shadows_start: usize,
    shadows: &'a [Shadow],
    quads_start: usize,
    quads: &'a [Quad],
    underlines_start: usize,
    underlines: &'a [Underline],
    monochrome_start: usize,
    monochrome: &'a [MonochromeSprite],
    subpixel_start: usize,
    subpixel: &'a [SubpixelSprite],
    polychrome_start: usize,
    polychrome: &'a [PolychromeSprite],
}

impl<'a> Iterator for BatchIterator<'a> {
    type Item = PrimitiveBatch<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let candidates = [
            (
                self.shadows.get(self.shadows_start).map(|s| s.order),
                PrimitiveKind::Shadow,
            ),
            (
                self.quads.get(self.quads_start).map(|q| q.order),
                PrimitiveKind::Quad,
            ),
            (
                self.underlines.get(self.underlines_start).map(|u| u.order),
                PrimitiveKind::Underline,
            ),
            (
                self.monochrome.get(self.monochrome_start).map(|s| s.order),
                PrimitiveKind::MonochromeSprite,
            ),
            (
                self.subpixel.get(self.subpixel_start).map(|s| s.order),
                PrimitiveKind::SubpixelSprite,
            ),
            (
                self.polychrome.get(self.polychrome_start).map(|s| s.order),
                PrimitiveKind::PolychromeSprite,
            ),
        ];

        let (min_order, min_kind) = candidates
            .iter()
            .filter_map(|(order, kind)| order.map(|o| (o, *kind)))
            .min_by_key(|(order, kind)| (*order, *kind))?;

        match min_kind {
            PrimitiveKind::Shadow => {
                let start = self.shadows_start;
                let end = self.shadows[start..]
                    .iter()
                    .position(|s| s.order != min_order)
                    .map_or(self.shadows.len(), |p| start + p);
                self.shadows_start = end;
                Some(PrimitiveBatch::Shadows(&self.shadows[start..end]))
            }
            PrimitiveKind::Quad => {
                let start = self.quads_start;
                let end = self.quads[start..]
                    .iter()
                    .position(|q| q.order != min_order)
                    .map_or(self.quads.len(), |p| start + p);
                self.quads_start = end;
                Some(PrimitiveBatch::Quads(&self.quads[start..end]))
            }
            PrimitiveKind::Underline => {
                let start = self.underlines_start;
                let end = self.underlines[start..]
                    .iter()
                    .position(|u| u.order != min_order)
                    .map_or(self.underlines.len(), |p| start + p);
                self.underlines_start = end;
                Some(PrimitiveBatch::Underlines(&self.underlines[start..end]))
            }
            PrimitiveKind::MonochromeSprite => {
                let start = self.monochrome_start;
                let end = self.monochrome[start..]
                    .iter()
                    .position(|s| s.order != min_order)
                    .map_or(self.monochrome.len(), |p| start + p);
                self.monochrome_start = end;
                Some(PrimitiveBatch::MonochromeSprites(
                    &self.monochrome[start..end],
                ))
            }
            PrimitiveKind::SubpixelSprite => {
                let start = self.subpixel_start;
                let end = self.subpixel[start..]
                    .iter()
                    .position(|s| s.order != min_order)
                    .map_or(self.subpixel.len(), |p| start + p);
                self.subpixel_start = end;
                Some(PrimitiveBatch::SubpixelSprites(&self.subpixel[start..end]))
            }
            PrimitiveKind::PolychromeSprite => {
                let start = self.polychrome_start;
                let end = self.polychrome[start..]
                    .iter()
                    .position(|s| s.order != min_order)
                    .map_or(self.polychrome.len(), |p| start + p);
                self.polychrome_start = end;
                Some(PrimitiveBatch::PolychromeSprites(
                    &self.polychrome[start..end],
                ))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_ordering() {
        let mut scene = Scene::new();
        scene.push_layer(Layer::Text);
        scene.insert_rect(0.0, 0.0, 10.0, 10.0, Color::WHITE, 0.0);
        scene.pop_layer();
        scene.push_layer(Layer::Background);
        scene.insert_rect(0.0, 0.0, 10.0, 10.0, Color::BLACK, 0.0);
        scene.pop_layer();
        scene.finish();

        let batches: Vec<_> = scene.batches().collect();
        assert_eq!(batches.len(), 2);

        match &batches[0] {
            PrimitiveBatch::Quads(quads) => {
                assert_eq!(quads[0].order, Layer::Background.order());
            }
            _ => panic!("expected Quads batch first"),
        }
        match &batches[1] {
            PrimitiveBatch::Quads(quads) => {
                assert_eq!(quads[0].order, Layer::Text.order());
            }
            _ => panic!("expected Quads batch second"),
        }
    }

    #[test]
    fn clipped_primitives_are_discarded() {
        let mut scene = Scene::new();
        scene.insert_quad(Quad {
            order: 0,
            x: 100.0,
            y: 100.0,
            width: 10.0,
            height: 10.0,
            color: Color::WHITE,
            corner_radius: 0.0,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            content_mask: ContentMask {
                x: 0.0,
                y: 0.0,
                width: 50.0,
                height: 50.0,
            },
        });
        scene.finish();
        assert!(scene.quads.is_empty());
    }

    #[test]
    fn mixed_type_batch_interleaving() {
        let mut scene = Scene::new();
        scene.push_layer(Layer::Background);
        scene.insert_shadow(Shadow {
            order: 0,
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            corner_radius: 0.0,
            color: Color::BLACK,
            blur_radius: 4.0,
            spread: 0.0,
            content_mask: ContentMask::default(),
        });
        scene.pop_layer();
        scene.push_layer(Layer::Text);
        scene.insert_rect(0.0, 0.0, 10.0, 10.0, Color::WHITE, 0.0);
        scene.pop_layer();
        scene.finish();

        let kinds: Vec<&str> = scene
            .batches()
            .map(|b| match b {
                PrimitiveBatch::Shadows(_) => "shadow",
                PrimitiveBatch::Quads(_) => "quad",
                _ => "other",
            })
            .collect();
        assert_eq!(kinds, vec!["shadow", "quad"]);
    }
}
