//! Batched rectangle renderer for selections, cursors, and decorations.
//!
//! All rectangles are queued via [`draw_rect`](RectRenderer::draw_rect) or
//! [`draw_border`](RectRenderer::draw_border) and flushed in one draw call.

use crate::color::Color;
use crate::vertex::RectVertex;

/// Descriptor for a rectangle draw call.
#[derive(Clone, Copy)]
struct RectDesc {
    pos: [f32; 2],
    size: [f32; 2],
    rect_min: [f32; 2],
    rect_max: [f32; 2],
    color: [f32; 4],
    corner_radius: f32,
}

/// Batches rectangle draw calls and flushes them in a single draw.
pub struct RectRenderer {
    /// Accumulated vertices for the current batch.
    vertices: Vec<RectVertex>,
    /// Accumulated indices for the current batch.
    indices: Vec<u32>,
}

impl RectRenderer {
    /// Creates a new, empty rectangle renderer.
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Queues a filled rectangle with optional rounded corners.
    pub fn draw_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        corner_radius: f32,
    ) {
        self.push_quad(RectDesc {
            pos: [x, y],
            size: [width, height],
            rect_min: [x, y],
            rect_max: [x + width, y + height],
            color: color.to_array(),
            corner_radius,
        });
    }

    /// Queues an outlined (border-only) rectangle.
    ///
    /// Drawn as four thin filled rectangles forming the border edges.
    pub fn draw_border(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: Color,
        thickness: f32,
    ) {
        // Top edge
        self.draw_rect(x, y, width, thickness, color, 0.0);
        // Bottom edge
        self.draw_rect(x, y + height - thickness, width, thickness, color, 0.0);
        // Left edge
        self.draw_rect(
            x,
            y + thickness,
            thickness,
            height - 2.0 * thickness,
            color,
            0.0,
        );
        // Right edge
        self.draw_rect(
            x + width - thickness,
            y + thickness,
            thickness,
            height - 2.0 * thickness,
            color,
            0.0,
        );
    }

    /// Returns `true` if there is queued geometry to flush.
    pub fn has_data(&self) -> bool {
        !self.indices.is_empty()
    }

    /// Submits all batched rectangles to the given render pass.
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
            label: Some("rect_vertex_buffer"),
            size: (self.vertices.len() * std::mem::size_of::<RectVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&self.vertices));

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rect_index_buffer"),
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

    /// Pushes a single quad (two triangles, four vertices) for a rectangle.
    fn push_quad(&mut self, desc: RectDesc) {
        #[allow(clippy::cast_possible_truncation)]
        let base = self.vertices.len() as u32;

        let [px, py] = desc.pos;
        let [sw, sh] = desc.size;

        let make = |vx: f32, vy: f32| RectVertex {
            x: vx,
            y: vy,
            rect_min: desc.rect_min,
            rect_max: desc.rect_max,
            color: desc.color,
            corner_radius: desc.corner_radius,
            _pad: 0.0,
        };

        self.vertices.push(make(px, py));
        self.vertices.push(make(px + sw, py));
        self.vertices.push(make(px + sw, py + sh));
        self.vertices.push(make(px, py + sh));

        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

impl Default for RectRenderer {
    fn default() -> Self {
        Self::new()
    }
}
