//! Batched text renderer using instanced glyph quads.
//!
//! Text is shaped with `cosmic-text`, rasterized into the [`TextAtlas`], and
//! drawn in a single instanced draw call per [`flush`](TextRenderer::flush).

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping};

use crate::color::Color;
use crate::text_atlas::TextAtlas;
use crate::vertex::TextVertex;

/// GPU resources needed by the text draw methods.
pub struct TextDrawContext<'a> {
    /// Font system for shaping and rasterizing.
    pub font_system: &'a mut FontSystem,
    /// Glyph atlas to cache rasterized glyphs.
    pub atlas: &'a mut TextAtlas,
    /// wgpu device.
    pub device: &'a wgpu::Device,
    /// wgpu queue.
    pub queue: &'a wgpu::Queue,
}

/// Batches glyph draw calls and flushes them in one instanced draw.
pub struct TextRenderer {
    /// Accumulated vertices for the current batch.
    vertices: Vec<TextVertex>,
    /// Accumulated indices for the current batch.
    indices: Vec<u32>,
}

impl TextRenderer {
    /// Creates a new, empty text renderer.
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Queues a single line of text at the given pixel position.
    pub fn draw_line(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        color: Color,
        font_size: f32,
        ctx: &mut TextDrawContext<'_>,
    ) {
        self.draw_styled_line(&[(text, color)], x, y, font_size, ctx);
    }

    /// Queues a syntax-highlighted line composed of multiple styled spans.
    #[allow(clippy::cast_precision_loss)]
    pub fn draw_styled_line(
        &mut self,
        spans: &[(&str, Color)],
        x: f32,
        y: f32,
        font_size: f32,
        ctx: &mut TextDrawContext<'_>,
    ) {
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(ctx.font_system, metrics);

        let rich: Vec<(&str, Attrs)> = spans
            .iter()
            .enumerate()
            .map(|(i, (text, _))| (*text, Attrs::new().metadata(i)))
            .collect();
        buffer.set_rich_text(ctx.font_system, rich, Attrs::new(), Shaping::Advanced);
        buffer.shape_until_scroll(ctx.font_system, false);

        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let span_color = spans.get(glyph.metadata).map_or(Color::WHITE, |(_, c)| *c);

                let physical = glyph.physical((x, y), 1.0);

                if let Some(info) = ctx.atlas.rasterize_glyph(
                    ctx.font_system,
                    ctx.device,
                    ctx.queue,
                    physical.cache_key,
                ) {
                    if info.width > 0 && info.height > 0 {
                        let gx = physical.x as f32 + info.bearing_x;
                        let gy = physical.y as f32 - info.bearing_y;
                        let gw = info.width as f32;
                        let gh = info.height as f32;
                        let color_arr = span_color.to_array();

                        self.push_quad(
                            [gx, gy, gw, gh],
                            [info.uv_left, info.uv_top, info.uv_right, info.uv_bottom],
                            color_arr,
                        );
                    }
                }
            }
        }
    }

    /// Returns `true` if there is queued geometry to flush.
    pub fn has_data(&self) -> bool {
        !self.indices.is_empty()
    }

    /// Submits all batched text to the given render pass.
    ///
    /// After this call the internal buffers are cleared.
    pub fn flush(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        if self.indices.is_empty() {
            return;
        }

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_vertex_buffer"),
            size: (self.vertices.len() * std::mem::size_of::<TextVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_index_buffer"),
            size: (self.indices.len() * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&self.indices));

        #[allow(clippy::cast_possible_truncation)]
        let num_indices = self.indices.len() as u32;

        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..num_indices, 0, 0..1);

        self.vertices.clear();
        self.indices.clear();
    }

    /// Pushes a single textured quad (two triangles, four vertices).
    ///
    /// `bounds` is `[x, y, w, h]`, `uvs` is `[left, top, right, bottom]`.
    fn push_quad(&mut self, bounds: [f32; 4], uvs: [f32; 4], color: [f32; 4]) {
        let [x, y, w, h] = bounds;
        let [uv_l, uv_t, uv_r, uv_b] = uvs;

        #[allow(clippy::cast_possible_truncation)]
        let base = self.vertices.len() as u32;

        self.vertices.push(TextVertex {
            x,
            y,
            uv_u: uv_l,
            uv_v: uv_t,
            color,
        });
        self.vertices.push(TextVertex {
            x: x + w,
            y,
            uv_u: uv_r,
            uv_v: uv_t,
            color,
        });
        self.vertices.push(TextVertex {
            x: x + w,
            y: y + h,
            uv_u: uv_r,
            uv_v: uv_b,
            color,
        });
        self.vertices.push(TextVertex {
            x,
            y: y + h,
            uv_u: uv_l,
            uv_v: uv_b,
            color,
        });

        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new()
    }
}
