//! GPU vertex types for text and rectangle rendering.

use bytemuck::{Pod, Zeroable};

/// Vertex for textured glyph quads.
///
/// Each glyph quad is drawn as two triangles sharing four of these vertices.
/// The fragment shader samples the glyph atlas at `(uv_u, uv_v)` and multiplies
/// by `color`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TextVertex {
    /// Screen-space X position.
    pub x: f32,
    /// Screen-space Y position.
    pub y: f32,
    /// Atlas texture U coordinate.
    pub uv_u: f32,
    /// Atlas texture V coordinate.
    pub uv_v: f32,
    /// RGBA color packed as four floats.
    pub color: [f32; 4],
}

impl TextVertex {
    /// Returns the `wgpu` vertex buffer layout for this vertex type.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // uv
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Vertex for filled / outlined rectangles with optional rounded corners.
///
/// The fragment shader uses `rect_min`/`rect_max` and `corner_radius` to
/// produce an SDF-based rounded rectangle.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct RectVertex {
    /// Screen-space X position.
    pub x: f32,
    /// Screen-space Y position.
    pub y: f32,
    /// Rectangle minimum corner (top-left) in pixels.
    pub rect_min: [f32; 2],
    /// Rectangle maximum corner (bottom-right) in pixels.
    pub rect_max: [f32; 2],
    /// RGBA fill color.
    pub color: [f32; 4],
    /// Corner radius in pixels (0 = sharp corners).
    pub corner_radius: f32,
    /// Padding to satisfy 4-byte alignment expectations.
    pub(crate) _pad: f32,
}

impl RectVertex {
    /// Returns the `wgpu` vertex buffer layout for this vertex type.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // rect_min
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // rect_max
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // corner_radius
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

/// Vertex for anti-aliased line segments (indent guides, rulers, borders).
///
/// The fragment shader computes signed distance to the line segment
/// defined by `line_start` / `line_end` and anti-aliases using `thickness`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct LineVertex {
    /// Screen-space X position.
    pub x: f32,
    /// Screen-space Y position.
    pub y: f32,
    /// Segment start point in screen-space.
    pub line_start: [f32; 2],
    /// Segment end point in screen-space.
    pub line_end: [f32; 2],
    /// RGBA color packed as four floats.
    pub color: [f32; 4],
    /// Line thickness in pixels.
    pub thickness: f32,
    /// Padding to maintain alignment.
    pub(crate) _pad: f32,
}

impl LineVertex {
    /// Returns the `wgpu` vertex buffer layout for this vertex type.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // line_start
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // line_end
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // thickness
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}
