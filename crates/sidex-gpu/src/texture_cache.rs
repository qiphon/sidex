//! Texture and image cache for the GPU renderer.
//!
//! Caches file icons, extension icons, and other image assets as GPU textures.
//! Supports LRU eviction when the total memory budget is exceeded, and loading
//! from raw image data in various formats.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Opaque identifier for a cached texture.
pub type TextureId = u64;

/// Format of the source image data passed to [`TextureCache::load_texture`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Rgba8,
    Rgb8,
    GrayscaleAlpha,
    Grayscale,
}

/// A GPU texture stored in the cache along with metadata for eviction.
pub struct CachedTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub last_used: Instant,
    pub memory_bytes: u64,
}

/// GPU texture cache with LRU eviction.
///
/// Stores textures keyed by [`TextureId`] and tracks total GPU memory.
/// When the cache exceeds [`max_memory`](TextureCache::max_memory),
/// the least recently used textures are evicted.
pub struct TextureCache {
    pub textures: HashMap<TextureId, CachedTexture>,
    pub total_memory: u64,
    pub max_memory: u64,
    next_id: TextureId,
}

impl TextureCache {
    /// Creates a new texture cache with the given memory budget in bytes.
    pub fn new(max_memory_bytes: u64) -> Self {
        Self {
            textures: HashMap::new(),
            total_memory: 0,
            max_memory: max_memory_bytes,
            next_id: 1,
        }
    }

    /// Creates a cache with a default 64 MiB budget.
    pub fn with_default_budget() -> Self {
        Self::new(64 * 1024 * 1024)
    }

    /// Loads raw image data into a GPU texture and returns its [`TextureId`].
    ///
    /// The `data` slice is interpreted according to `format`. Width and height
    /// must match the data length.
    #[allow(clippy::cast_possible_truncation)]
    pub fn load_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
        width: u32,
        height: u32,
        format: ImageFormat,
    ) -> Option<TextureId> {
        let (gpu_format, rgba_data) = match format {
            ImageFormat::Rgba8 => (wgpu::TextureFormat::Rgba8UnormSrgb, data.to_vec()),
            ImageFormat::Rgb8 => {
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for chunk in data.chunks_exact(3) {
                    rgba.extend_from_slice(chunk);
                    rgba.push(255);
                }
                (wgpu::TextureFormat::Rgba8UnormSrgb, rgba)
            }
            ImageFormat::GrayscaleAlpha => {
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for chunk in data.chunks_exact(2) {
                    let v = chunk[0];
                    let a = chunk[1];
                    rgba.extend_from_slice(&[v, v, v, a]);
                }
                (wgpu::TextureFormat::Rgba8UnormSrgb, rgba)
            }
            ImageFormat::Grayscale => {
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for &v in data {
                    rgba.extend_from_slice(&[v, v, v, 255]);
                }
                (wgpu::TextureFormat::Rgba8UnormSrgb, rgba)
            }
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cached_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let memory_bytes = u64::from(width) * u64::from(height) * 4;
        let id = self.next_id;
        self.next_id += 1;

        self.textures.insert(id, CachedTexture {
            texture,
            view,
            size: (width, height),
            format: gpu_format,
            last_used: Instant::now(),
            memory_bytes,
        });
        self.total_memory += memory_bytes;

        self.evict_if_needed();

        Some(id)
    }

    /// Retrieves a cached texture by id, updating its last-used timestamp.
    pub fn get_texture(&mut self, id: TextureId) -> Option<&CachedTexture> {
        if let Some(entry) = self.textures.get_mut(&id) {
            entry.last_used = Instant::now();
            Some(entry)
        } else {
            None
        }
    }

    /// Read-only access without updating the last-used timestamp.
    pub fn peek_texture(&self, id: TextureId) -> Option<&CachedTexture> {
        self.textures.get(&id)
    }

    /// Removes a specific texture from the cache.
    pub fn remove(&mut self, id: TextureId) {
        if let Some(entry) = self.textures.remove(&id) {
            self.total_memory = self.total_memory.saturating_sub(entry.memory_bytes);
        }
    }

    /// Evicts textures that haven't been used within `max_age`.
    pub fn evict_unused(&mut self, max_age: Duration) {
        let cutoff = Instant::now() - max_age;
        let to_evict: Vec<TextureId> = self
            .textures
            .iter()
            .filter(|(_, v)| v.last_used < cutoff)
            .map(|(k, _)| *k)
            .collect();
        for id in to_evict {
            self.remove(id);
        }
    }

    /// Evicts LRU textures until total memory is within budget.
    fn evict_if_needed(&mut self) {
        while self.total_memory > self.max_memory && !self.textures.is_empty() {
            let oldest = self
                .textures
                .iter()
                .min_by_key(|(_, v)| v.last_used)
                .map(|(k, _)| *k);
            if let Some(id) = oldest {
                self.remove(id);
            } else {
                break;
            }
        }
    }

    /// Returns the number of cached textures.
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// Clears all cached textures.
    pub fn clear(&mut self) {
        self.textures.clear();
        self.total_memory = 0;
    }
}

impl Default for TextureCache {
    fn default() -> Self {
        Self::with_default_budget()
    }
}
