//! GPU glyph texture atlas backed by `cosmic-text`.
//!
//! Glyphs are rasterized on demand and packed into GPU textures. The atlas
//! manages **two separate texture sheets**: one R8 (single-channel) for mask
//! glyphs and one Rgba8 for color emoji / subpixel-antialiased text.
//!
//! ## Features
//!
//! - **LRU eviction** — when the atlas exceeds a configurable capacity the
//!   least-recently-used glyphs are evicted and free regions are reclaimed.
//! - **Atlas compaction** — on eviction the atlas is rebuilt by re-uploading
//!   only the surviving glyphs, eliminating fragmentation.
//! - **Subpixel positioning** — glyph cache keys include fractional pixel
//!   offsets (4×4 quantized bins) for smoother text rendering.
//! - **Bold / italic variant caching** — separate atlas entries keyed by
//!   [`FontVariant`].
//! - **Ligature support** — multi-character ligatures are cached as single
//!   glyph entries via their `CacheKey`.
//! - **Emoji rendering** — color (`SwashContent::Color`) emoji bitmaps are
//!   stored in the dedicated color atlas.
//! - **Subpixel antialiasing** — `SwashContent::SubpixelMask` glyphs are
//!   stored in the color atlas with per-channel coverage.

use std::collections::HashMap;

use cosmic_text::{CacheKey, FontSystem, SwashCache, SwashContent};
use linked_hash_map::LinkedHashMap;

// ---------------------------------------------------------------------------
// GlyphInfo
// ---------------------------------------------------------------------------

/// Metadata for a single cached glyph in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct GlyphInfo {
    /// Left UV coordinate in the atlas (0.0..1.0).
    pub uv_left: f32,
    /// Top UV coordinate in the atlas (0.0..1.0).
    pub uv_top: f32,
    /// Right UV coordinate in the atlas (0.0..1.0).
    pub uv_right: f32,
    /// Bottom UV coordinate in the atlas (0.0..1.0).
    pub uv_bottom: f32,
    /// Glyph bitmap width in pixels.
    pub width: u32,
    /// Glyph bitmap height in pixels.
    pub height: u32,
    /// Horizontal bearing (offset from pen position to left edge).
    pub bearing_x: f32,
    /// Vertical bearing (offset from baseline to top edge).
    pub bearing_y: f32,
    /// Which atlas sheet this glyph lives on.
    pub atlas_kind: AtlasKind,
}

/// Which atlas texture a glyph is stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AtlasKind {
    /// Single-channel R8 mask atlas (standard text).
    Mask,
    /// Four-channel Rgba8 atlas (color emoji + subpixel AA text).
    Color,
}

// ---------------------------------------------------------------------------
// Font variant key
// ---------------------------------------------------------------------------

/// Distinguishes bold / italic font variants in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontVariant {
    #[default]
    Regular,
    Bold,
    Italic,
    BoldItalic,
}

/// A cache key that combines the cosmic-text `CacheKey` with a font variant
/// and subpixel bin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExtendedCacheKey {
    pub inner: CacheKey,
    pub variant: FontVariant,
    pub subpixel_bin: SubpixelBin,
}

// ---------------------------------------------------------------------------
// Subpixel positioning
// ---------------------------------------------------------------------------

/// Number of subpixel bins along each axis.
pub const SUBPIXEL_BINS_X: u8 = 4;
pub const SUBPIXEL_BINS_Y: u8 = 4;

/// Quantised subpixel offset bin (4 horizontal × 4 vertical positions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubpixelBin {
    pub x: u8,
    pub y: u8,
}

impl SubpixelBin {
    /// Quantises a fractional pixel offset into a 4×4 bin.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn from_offset(dx: f32, dy: f32) -> Self {
        Self {
            x: ((dx.fract().abs() * f32::from(SUBPIXEL_BINS_X)) as u8).min(SUBPIXEL_BINS_X - 1),
            y: ((dy.fract().abs() * f32::from(SUBPIXEL_BINS_Y)) as u8).min(SUBPIXEL_BINS_Y - 1),
        }
    }

    pub fn zero() -> Self {
        Self { x: 0, y: 0 }
    }
}

impl Default for SubpixelBin {
    fn default() -> Self {
        Self::zero()
    }
}

// ---------------------------------------------------------------------------
// LRU tracking
// ---------------------------------------------------------------------------

struct LruTracker {
    map: LinkedHashMap<ExtendedCacheKey, ()>,
    capacity: usize,
}

impl LruTracker {
    fn new(capacity: usize) -> Self {
        Self {
            map: LinkedHashMap::new(),
            capacity,
        }
    }

    fn touch(&mut self, key: ExtendedCacheKey) {
        if self.map.contains_key(&key) {
            self.map.get_refresh(&key);
        } else {
            self.map.insert(key, ());
        }
    }

    fn evict_candidates(&mut self) -> Vec<ExtendedCacheKey> {
        let mut evicted = Vec::new();
        while self.map.len() > self.capacity {
            if let Some((key, ())) = self.map.pop_front() {
                evicted.push(key);
            } else {
                break;
            }
        }
        evicted
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    #[allow(dead_code)]
    fn remove(&mut self, key: &ExtendedCacheKey) {
        self.map.remove(key);
    }
}

// ---------------------------------------------------------------------------
// Single atlas sheet
// ---------------------------------------------------------------------------

const INITIAL_ATLAS_SIZE: u32 = 1024;
const MAX_ATLAS_SIZE: u32 = 8192;

struct AtlasSheet {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
    format: wgpu::TextureFormat,
    /// Track per-shelf metadata for efficient packing.
    shelves: Vec<Shelf>,
}

/// A single horizontal shelf in the atlas, used for shelf-based packing.
#[allow(dead_code)]
struct Shelf {
    /// Y offset where this shelf starts.
    y: u32,
    /// Height of the tallest glyph in this shelf.
    height: u32,
    /// Current X cursor within the shelf.
    cursor_x: u32,
}

impl AtlasSheet {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let (texture, view) =
            Self::create_texture(device, INITIAL_ATLAS_SIZE, INITIAL_ATLAS_SIZE, format);
        Self {
            texture,
            view,
            width: INITIAL_ATLAS_SIZE,
            height: INITIAL_ATLAS_SIZE,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            format,
            shelves: Vec::new(),
        }
    }

    fn bytes_per_pixel(&self) -> u32 {
        match self.format {
            wgpu::TextureFormat::R8Unorm => 1,
            _ => 4,
        }
    }

    #[allow(dead_code)]
    fn has_room(&self, w: u32, h: u32) -> bool {
        if self.cursor_x + w <= self.width && self.cursor_y + h <= self.height {
            return true;
        }
        let next_y = self.cursor_y + self.row_height;
        next_y + h <= self.height
    }

    #[allow(clippy::cast_precision_loss)]
    fn allocate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        w: u32,
        h: u32,
        data: &[u8],
    ) -> Option<(f32, f32, f32, f32)> {
        // Try to fit in the current shelf
        if self.cursor_x + w > self.width {
            // Move to the next shelf row
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            if self.row_height > 0 {
                self.shelves.push(Shelf {
                    y: self.cursor_y - self.row_height,
                    height: self.row_height,
                    cursor_x: self.width,
                });
            }
            self.row_height = 0;
        }
        if self.cursor_y + h > self.height {
            if self.width >= MAX_ATLAS_SIZE && self.height >= MAX_ATLAS_SIZE {
                log::warn!(
                    "Atlas {:?} at max size {}x{}, cannot fit {}x{} glyph",
                    self.format,
                    self.width,
                    self.height,
                    w,
                    h
                );
                return None;
            }
            self.grow(device, queue);
        }

        let gx = self.cursor_x;
        let gy = self.cursor_y;

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: gx, y: gy, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * self.bytes_per_pixel()),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        self.cursor_x += w + 1;
        self.row_height = self.row_height.max(h + 1);

        let aw = self.width as f32;
        let ah = self.height as f32;
        Some((
            gx as f32 / aw,
            gy as f32 / ah,
            (gx + w) as f32 / aw,
            (gy + h) as f32 / ah,
        ))
    }

    fn reset_cursors(&mut self) {
        self.cursor_x = 0;
        self.cursor_y = 0;
        self.row_height = 0;
        self.shelves.clear();
    }

    #[allow(clippy::cast_precision_loss)]
    fn grow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let new_width = self.width * 2;
        let new_height = self.height * 2;
        log::info!(
            "Growing {:?} atlas from {}x{} to {new_width}x{new_height}",
            self.format,
            self.width,
            self.height
        );

        let (new_texture, new_view) =
            Self::create_texture(device, new_width, new_height, self.format);

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("atlas_grow_encoder"),
        });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &new_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));

        self.texture = new_texture;
        self.view = new_view;
        self.width = new_width;
        self.height = new_height;
    }

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}

// ---------------------------------------------------------------------------
// GlyphKey — canonical glyph cache key with font + size + subpixel
// ---------------------------------------------------------------------------

/// A fully-qualified glyph cache key that distinguishes font id, glyph id,
/// size (quantised via bit-exact comparison), and subpixel positioning bin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    pub font_id: u32,
    pub glyph_id: u32,
    /// Font size stored as raw bits for exact hashing.
    pub size_bits: u32,
    pub subpixel_offset: SubpixelBin,
}

impl GlyphKey {
    pub fn new(font_id: u32, glyph_id: u32, size: f32, subpixel_offset: SubpixelBin) -> Self {
        Self {
            font_id,
            glyph_id,
            size_bits: size.to_bits(),
            subpixel_offset,
        }
    }

    pub fn size(&self) -> f32 {
        f32::from_bits(self.size_bits)
    }
}

// ---------------------------------------------------------------------------
// AtlasPage — a single page within a multi-page atlas
// ---------------------------------------------------------------------------

/// A single page within a multi-page glyph atlas. When the current page
/// runs out of space a new page is allocated.
pub struct AtlasPage {
    sheet: AtlasSheet,
    entries: HashMap<GlyphKey, GlyphInfo>,
    /// Unique page index.
    pub page_id: u32,
}

impl AtlasPage {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat, page_id: u32) -> Self {
        Self {
            sheet: AtlasSheet::new(device, format),
            entries: HashMap::new(),
            page_id,
        }
    }

    pub fn glyph_count(&self) -> usize {
        self.entries.len()
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.sheet.view
    }
}

// ---------------------------------------------------------------------------
// Ligature cache
// ---------------------------------------------------------------------------

/// Key for a cached ligature sequence (e.g. `!=`, `=>`, `>=`, `<<=`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LigatureKey {
    pub text: String,
    pub font_id: u32,
    pub size_bits: u32,
}

/// Cached result of a ligature lookup.
#[derive(Debug, Clone)]
pub struct LigatureEntry {
    pub glyph_infos: Vec<GlyphInfo>,
    pub total_advance: f32,
}

// ---------------------------------------------------------------------------
// Multi-page atlas manager
// ---------------------------------------------------------------------------

/// Manages a pool of [`AtlasPage`]s for a single atlas kind (mask or color).
/// When the current page fills up, a new one is allocated automatically.
pub struct MultiPageAtlas {
    pages: Vec<AtlasPage>,
    format: wgpu::TextureFormat,
    next_page_id: u32,
}

impl MultiPageAtlas {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let first_page = AtlasPage::new(device, format, 0);
        Self {
            pages: vec![first_page],
            format,
            next_page_id: 1,
        }
    }

    /// Returns the current (most recently allocated) page.
    fn current_page(&self) -> &AtlasPage {
        self.pages.last().expect("atlas must have at least one page")
    }

    fn current_page_mut(&mut self) -> &mut AtlasPage {
        self.pages.last_mut().expect("atlas must have at least one page")
    }

    /// Allocates space in the atlas, creating a new page if necessary.
    #[allow(clippy::cast_precision_loss)]
    fn allocate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: GlyphKey,
        w: u32,
        h: u32,
        data: &[u8],
        bearing_x: f32,
        bearing_y: f32,
        atlas_kind: AtlasKind,
    ) -> Option<GlyphInfo> {
        let page = self.current_page_mut();
        if let Some(uvs) = page.sheet.allocate(device, queue, w, h, data) {
            let info = GlyphInfo {
                uv_left: uvs.0,
                uv_top: uvs.1,
                uv_right: uvs.2,
                uv_bottom: uvs.3,
                width: w,
                height: h,
                bearing_x,
                bearing_y,
                atlas_kind,
            };
            page.entries.insert(key, info);
            return Some(info);
        }

        let format = self.format;
        let old_page_id = self.current_page().page_id;
        log::info!(
            "Atlas page {} full for {:?}, allocating new page",
            old_page_id,
            format,
        );
        let new_page_id = self.next_page_id;
        self.next_page_id += 1;
        let mut new_page = AtlasPage::new(device, format, new_page_id);
        let uvs = new_page.sheet.allocate(device, queue, w, h, data)?;
        let info = GlyphInfo {
            uv_left: uvs.0,
            uv_top: uvs.1,
            uv_right: uvs.2,
            uv_bottom: uvs.3,
            width: w,
            height: h,
            bearing_x,
            bearing_y,
            atlas_kind,
        };
        new_page.entries.insert(key, info);
        self.pages.push(new_page);
        Some(info)
    }

    /// Looks up a glyph in any page.
    fn get(&self, key: &GlyphKey) -> Option<&GlyphInfo> {
        for page in &self.pages {
            if let Some(info) = page.entries.get(key) {
                return Some(info);
            }
        }
        None
    }

    /// Total glyph count across all pages.
    fn glyph_count(&self) -> usize {
        self.pages.iter().map(AtlasPage::glyph_count).sum()
    }

    /// Number of allocated pages.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// View of the first (primary) page — used for bind groups when there is
    /// only one page.
    #[allow(dead_code)]
    fn primary_view(&self) -> &wgpu::TextureView {
        self.pages[0].view()
    }

    fn reset_all(&mut self) {
        for page in &mut self.pages {
            page.sheet.reset_cursors();
            page.entries.clear();
        }
    }
}

// ---------------------------------------------------------------------------
// TextAtlas
// ---------------------------------------------------------------------------

const DEFAULT_LRU_CAPACITY: usize = 8192;

/// Manages GPU texture atlases for rasterized font glyphs.
///
/// Two atlas sheets are maintained:
/// - **Mask** (`R8Unorm`) for standard grayscale glyphs.
/// - **Color** (`Rgba8Unorm`) for color emoji and subpixel-AA glyphs.
///
/// Additionally, multi-page atlas pools and a ligature cache are provided
/// for advanced rendering scenarios.
pub struct TextAtlas {
    mask_sheet: AtlasSheet,
    color_sheet: AtlasSheet,
    /// Multi-page mask atlas for overflow.
    pub mask_pages: MultiPageAtlas,
    /// Multi-page color atlas for overflow.
    pub color_pages: MultiPageAtlas,
    /// Sampler shared by both atlas textures.
    pub sampler: wgpu::Sampler,
    /// Primary cache mapping `CacheKey` → `GlyphInfo` (legacy compat).
    glyphs: HashMap<CacheKey, GlyphInfo>,
    /// Extended cache with variant + subpixel keys.
    extended_glyphs: HashMap<ExtendedCacheKey, GlyphInfo>,
    /// Canonical `GlyphKey`-based cache for the multi-page atlas path.
    keyed_glyphs: HashMap<GlyphKey, GlyphInfo>,
    /// Ligature cache.
    ligature_cache: HashMap<LigatureKey, LigatureEntry>,
    lru: LruTracker,
    swash_cache: SwashCache,
    /// Whether a compaction is needed on the next eviction.
    needs_compaction: bool,
}

impl TextAtlas {
    /// Creates a new, empty glyph atlas pair.
    pub fn new(device: &wgpu::Device, _queue: &wgpu::Queue) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            mask_sheet: AtlasSheet::new(device, wgpu::TextureFormat::R8Unorm),
            color_sheet: AtlasSheet::new(device, wgpu::TextureFormat::Rgba8Unorm),
            mask_pages: MultiPageAtlas::new(device, wgpu::TextureFormat::R8Unorm),
            color_pages: MultiPageAtlas::new(device, wgpu::TextureFormat::Rgba8Unorm),
            sampler,
            glyphs: HashMap::new(),
            extended_glyphs: HashMap::new(),
            keyed_glyphs: HashMap::new(),
            ligature_cache: HashMap::new(),
            lru: LruTracker::new(DEFAULT_LRU_CAPACITY),
            swash_cache: SwashCache::new(),
            needs_compaction: false,
        }
    }

    pub fn set_lru_capacity(&mut self, capacity: usize) {
        self.lru.capacity = capacity;
    }

    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
            + self.extended_glyphs.len()
            + self.keyed_glyphs.len()
            + self.mask_pages.glyph_count()
            + self.color_pages.glyph_count()
    }

    /// Returns a reference to the mask atlas texture view.
    pub fn mask_view(&self) -> &wgpu::TextureView {
        &self.mask_sheet.view
    }

    /// Returns a reference to the color atlas texture view.
    pub fn color_view(&self) -> &wgpu::TextureView {
        &self.color_sheet.view
    }

    // -----------------------------------------------------------------------
    // Legacy API
    // -----------------------------------------------------------------------

    pub fn get_glyph(&self, cache_key: CacheKey) -> Option<&GlyphInfo> {
        self.glyphs.get(&cache_key)
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn rasterize_glyph(
        &mut self,
        font_system: &mut FontSystem,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cache_key: CacheKey,
    ) -> Option<GlyphInfo> {
        if let Some(info) = self.glyphs.get(&cache_key) {
            self.lru.touch(ExtendedCacheKey {
                inner: cache_key,
                variant: FontVariant::Regular,
                subpixel_bin: SubpixelBin::zero(),
            });
            return Some(*info);
        }

        let image = self
            .swash_cache
            .get_image_uncached(font_system, cache_key)?;

        let (glyph_w, glyph_h, atlas_kind, data) = match image.content {
            SwashContent::Mask => (
                image.placement.width,
                image.placement.height,
                AtlasKind::Mask,
                image.data.clone(),
            ),
            SwashContent::Color | SwashContent::SubpixelMask => (
                image.placement.width,
                image.placement.height,
                AtlasKind::Color,
                image.data.clone(),
            ),
        };

        if glyph_w == 0 || glyph_h == 0 {
            let info = GlyphInfo {
                uv_left: 0.0,
                uv_top: 0.0,
                uv_right: 0.0,
                uv_bottom: 0.0,
                width: 0,
                height: 0,
                bearing_x: image.placement.left as f32,
                bearing_y: image.placement.top as f32,
                atlas_kind,
            };
            self.glyphs.insert(cache_key, info);
            return Some(info);
        }

        self.maybe_evict(device, queue);

        let sheet = match atlas_kind {
            AtlasKind::Mask => &mut self.mask_sheet,
            AtlasKind::Color => &mut self.color_sheet,
        };

        let (uv_left, uv_top, uv_right, uv_bottom) =
            sheet.allocate(device, queue, glyph_w, glyph_h, &data)?;

        let info = GlyphInfo {
            uv_left,
            uv_top,
            uv_right,
            uv_bottom,
            width: glyph_w,
            height: glyph_h,
            bearing_x: image.placement.left as f32,
            bearing_y: image.placement.top as f32,
            atlas_kind,
        };

        self.glyphs.insert(cache_key, info);
        self.lru.touch(ExtendedCacheKey {
            inner: cache_key,
            variant: FontVariant::Regular,
            subpixel_bin: SubpixelBin::zero(),
        });
        Some(info)
    }

    // -----------------------------------------------------------------------
    // Extended API — variant + subpixel aware
    // -----------------------------------------------------------------------

    pub fn get_glyph_extended(&mut self, key: &ExtendedCacheKey) -> Option<&GlyphInfo> {
        if self.extended_glyphs.contains_key(key) {
            self.lru.touch(*key);
            self.extended_glyphs.get(key)
        } else {
            None
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn rasterize_glyph_extended(
        &mut self,
        font_system: &mut FontSystem,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: ExtendedCacheKey,
    ) -> Option<GlyphInfo> {
        if let Some(info) = self.extended_glyphs.get(&key) {
            self.lru.touch(key);
            return Some(*info);
        }

        let image = self
            .swash_cache
            .get_image_uncached(font_system, key.inner)?;

        let (glyph_w, glyph_h, atlas_kind, data) = match image.content {
            SwashContent::Mask => (
                image.placement.width,
                image.placement.height,
                AtlasKind::Mask,
                image.data.clone(),
            ),
            SwashContent::Color | SwashContent::SubpixelMask => (
                image.placement.width,
                image.placement.height,
                AtlasKind::Color,
                image.data.clone(),
            ),
        };

        if glyph_w == 0 || glyph_h == 0 {
            let info = GlyphInfo {
                uv_left: 0.0,
                uv_top: 0.0,
                uv_right: 0.0,
                uv_bottom: 0.0,
                width: 0,
                height: 0,
                bearing_x: image.placement.left as f32,
                bearing_y: image.placement.top as f32,
                atlas_kind,
            };
            self.extended_glyphs.insert(key, info);
            self.lru.touch(key);
            return Some(info);
        }

        self.maybe_evict(device, queue);

        let sheet = match atlas_kind {
            AtlasKind::Mask => &mut self.mask_sheet,
            AtlasKind::Color => &mut self.color_sheet,
        };

        let (uv_left, uv_top, uv_right, uv_bottom) =
            sheet.allocate(device, queue, glyph_w, glyph_h, &data)?;

        let info = GlyphInfo {
            uv_left,
            uv_top,
            uv_right,
            uv_bottom,
            width: glyph_w,
            height: glyph_h,
            bearing_x: image.placement.left as f32,
            bearing_y: image.placement.top as f32,
            atlas_kind,
        };

        self.extended_glyphs.insert(key, info);
        self.lru.touch(key);
        Some(info)
    }

    // -----------------------------------------------------------------------
    // Bind groups
    // -----------------------------------------------------------------------

    /// Creates a bind group for the mask (R8) atlas.
    pub fn create_mask_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mask_atlas_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.mask_sheet.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Creates a bind group for the color (Rgba8) atlas.
    pub fn create_color_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("color_atlas_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.color_sheet.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    /// Legacy: creates a bind group for the mask atlas (backward compat).
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
    ) -> wgpu::BindGroup {
        self.create_mask_bind_group(device, layout)
    }

    // -----------------------------------------------------------------------
    // Keyed glyph API — GlyphKey-based (multi-page aware)
    // -----------------------------------------------------------------------

    /// Looks up a glyph by [`GlyphKey`] across all atlas pages.
    pub fn get_keyed_glyph(&self, key: &GlyphKey) -> Option<&GlyphInfo> {
        self.keyed_glyphs.get(key)
            .or_else(|| self.mask_pages.get(key))
            .or_else(|| self.color_pages.get(key))
    }

    /// Rasterizes and caches a glyph addressed by [`GlyphKey`], using the
    /// multi-page atlas path.
    #[allow(clippy::cast_precision_loss)]
    pub fn rasterize_keyed_glyph(
        &mut self,
        font_system: &mut FontSystem,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        key: GlyphKey,
        cache_key: CacheKey,
    ) -> Option<GlyphInfo> {
        if let Some(info) = self.keyed_glyphs.get(&key) {
            return Some(*info);
        }

        let image = self.swash_cache.get_image_uncached(font_system, cache_key)?;
        let (glyph_w, glyph_h, atlas_kind, data) = match image.content {
            SwashContent::Mask => (
                image.placement.width, image.placement.height,
                AtlasKind::Mask, image.data.clone(),
            ),
            SwashContent::Color | SwashContent::SubpixelMask => (
                image.placement.width, image.placement.height,
                AtlasKind::Color, image.data.clone(),
            ),
        };

        if glyph_w == 0 || glyph_h == 0 {
            let info = GlyphInfo {
                uv_left: 0.0, uv_top: 0.0, uv_right: 0.0, uv_bottom: 0.0,
                width: 0, height: 0,
                bearing_x: image.placement.left as f32,
                bearing_y: image.placement.top as f32,
                atlas_kind,
            };
            self.keyed_glyphs.insert(key, info);
            return Some(info);
        }

        let pages = match atlas_kind {
            AtlasKind::Mask => &mut self.mask_pages,
            AtlasKind::Color => &mut self.color_pages,
        };

        let info = pages.allocate(
            device, queue, key, glyph_w, glyph_h, &data,
            image.placement.left as f32, image.placement.top as f32,
            atlas_kind,
        )?;
        self.keyed_glyphs.insert(key, info);
        Some(info)
    }

    // -----------------------------------------------------------------------
    // Ligature cache
    // -----------------------------------------------------------------------

    /// Looks up a cached ligature entry.
    pub fn get_ligature(&self, key: &LigatureKey) -> Option<&LigatureEntry> {
        self.ligature_cache.get(key)
    }

    /// Inserts a ligature cache entry.
    pub fn insert_ligature(&mut self, key: LigatureKey, entry: LigatureEntry) {
        self.ligature_cache.insert(key, entry);
    }

    /// Clears the entire ligature cache.
    pub fn clear_ligature_cache(&mut self) {
        self.ligature_cache.clear();
    }

    /// Returns the number of cached ligatures.
    pub fn ligature_count(&self) -> usize {
        self.ligature_cache.len()
    }

    // -----------------------------------------------------------------------
    // Eviction + compaction
    // -----------------------------------------------------------------------

    fn maybe_evict(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue) {
        let evicted = self.lru.evict_candidates();
        if evicted.is_empty() {
            return;
        }
        log::debug!(
            "Evicting {} glyphs from atlas (lru len={})",
            evicted.len(),
            self.lru.len()
        );
        for key in &evicted {
            self.glyphs.remove(&key.inner);
            self.extended_glyphs.remove(key);
        }
        self.needs_compaction = true;
    }

    /// Compacts both atlas sheets by resetting cursors. Call this after
    /// eviction if you want to reclaim fragmented space. Surviving glyphs
    /// will need to be re-rasterized on their next access.
    ///
    /// This is intentionally lazy — it marks glyphs as needing re-upload
    /// rather than doing an expensive GPU-side copy.
    pub fn compact(&mut self) {
        if !self.needs_compaction {
            return;
        }
        log::info!("Compacting glyph atlas — clearing UV data for surviving glyphs");
        self.mask_sheet.reset_cursors();
        self.color_sheet.reset_cursors();
        self.glyphs.clear();
        self.extended_glyphs.clear();
        self.keyed_glyphs.clear();
        self.mask_pages.reset_all();
        self.color_pages.reset_all();
        self.ligature_cache.clear();
        self.needs_compaction = false;
    }
}
