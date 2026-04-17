//! Shader pipelines for the `SideX` GPU renderer.
//!
//! Pipelines:
//! - **Text** — monochrome glyph quads (alpha mask from R8 atlas).
//! - **Subpixel text** — subpixel-antialiased glyph quads (RGB coverage from Rgba8 atlas).
//! - **Rectangle** — filled/outlined rounded rectangles via SDF.
//! - **Shadow** — box shadow via SDF with blur.
//! - **Underline** — straight or wavy underlines/strikethroughs.
//! - **Line** — anti-aliased line segments for indent guides, rulers, borders.
//!
//! All shaders are loaded from `.wgsl` files via `include_str!`.

use crate::vertex::{LineVertex, RectVertex, TextVertex};

// ---------------------------------------------------------------------------
// WGSL sources — loaded from external files
// ---------------------------------------------------------------------------

const TEXT_SHADER_SRC: &str = include_str!("shaders/text.wgsl");
const RECT_SHADER_SRC: &str = include_str!("shaders/rect.wgsl");
const LINE_SHADER_SRC: &str = include_str!("shaders/line.wgsl");

/// Uniform data uploaded to the GPU each frame.
/// Shared by all pipelines via bind group 0.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewportUniform {
    /// Orthographic projection matrix mapping pixel coordinates to clip space.
    pub projection: [[f32; 4]; 4],
    /// Camera / scroll offset applied in the vertex shader.
    pub scroll_offset: [f32; 2],
    pub _pad: [f32; 2],
}

impl ViewportUniform {
    /// Creates a viewport uniform for the given window size and scroll position.
    #[allow(clippy::cast_precision_loss)]
    pub fn new(width: u32, height: u32, scroll_x: f32, scroll_y: f32) -> Self {
        let w = width as f32;
        let h = height as f32;
        #[rustfmt::skip]
        let projection = [
            [2.0 / w,  0.0,       0.0, 0.0],
            [0.0,     -2.0 / h,   0.0, 0.0],
            [0.0,      0.0,       1.0, 0.0],
            [-1.0,     1.0,       0.0, 1.0],
        ];
        Self {
            projection,
            scroll_offset: [scroll_x, scroll_y],
            _pad: [0.0, 0.0],
        }
    }
}

// ---------------------------------------------------------------------------
// Pipeline construction helpers
// ---------------------------------------------------------------------------

fn alpha_blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

/// Creates the uniform bind group layout shared by all pipelines.
pub fn create_uniform_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("uniform_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// Creates the bind group layout for atlas texture + sampler.
pub fn create_atlas_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("atlas_bind_group_layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_textured_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_bgl: &wgpu::BindGroupLayout,
    atlas_bgl: &wgpu::BindGroupLayout,
    shader_src: &str,
    vs_entry: &str,
    fs_entry: &str,
    label: &str,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label}_layout")),
        bind_group_layouts: &[uniform_bgl, atlas_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(vs_entry),
            buffers: &[TextVertex::layout()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(fs_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(alpha_blend_state()),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview: None,
        cache: None,
    })
}

fn create_rect_like_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_bgl: &wgpu::BindGroupLayout,
    shader_src: &str,
    vs_entry: &str,
    fs_entry: &str,
    label: &str,
    vertex_layout: wgpu::VertexBufferLayout<'static>,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(shader_src.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{label}_layout")),
        bind_group_layouts: &[uniform_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some(vs_entry),
            buffers: &[vertex_layout],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some(fs_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(alpha_blend_state()),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        multiview: None,
        cache: None,
    })
}

// ---------------------------------------------------------------------------
// Public pipeline constructors
// ---------------------------------------------------------------------------

/// Creates the monochrome text pipeline (alpha mask from R8 atlas).
pub fn create_text_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_bgl: &wgpu::BindGroupLayout,
    atlas_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_textured_pipeline(
        device,
        format,
        uniform_bgl,
        atlas_bgl,
        TEXT_SHADER_SRC,
        "vs_main",
        "fs_mono",
        "text_pipeline",
    )
}

/// Creates the subpixel-antialiased text pipeline (RGB coverage from Rgba8 atlas).
pub fn create_subpixel_text_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_bgl: &wgpu::BindGroupLayout,
    atlas_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_textured_pipeline(
        device,
        format,
        uniform_bgl,
        atlas_bgl,
        TEXT_SHADER_SRC,
        "vs_main",
        "fs_subpixel",
        "subpixel_text_pipeline",
    )
}

/// Creates the render pipeline for drawing colored, rounded rectangles.
pub fn create_rect_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_rect_like_pipeline(
        device,
        format,
        uniform_bgl,
        RECT_SHADER_SRC,
        "vs_rect",
        "fs_rect",
        "rect_pipeline",
        RectVertex::layout(),
    )
}

/// Creates the box shadow pipeline.
pub fn create_shadow_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_rect_like_pipeline(
        device,
        format,
        uniform_bgl,
        RECT_SHADER_SRC,
        "vs_rect",
        "fs_shadow",
        "shadow_pipeline",
        RectVertex::layout(),
    )
}

/// Creates the underline/strikethrough pipeline.
/// Re-uses the rect shader since underlines are thin filled rectangles.
pub fn create_underline_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_rect_like_pipeline(
        device,
        format,
        uniform_bgl,
        RECT_SHADER_SRC,
        "vs_rect",
        "fs_rect",
        "underline_pipeline",
        RectVertex::layout(),
    )
}

/// Creates the anti-aliased line pipeline for indent guides, rulers, and borders.
pub fn create_line_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    create_rect_like_pipeline(
        device,
        format,
        uniform_bgl,
        LINE_SHADER_SRC,
        "vs_line",
        "fs_line",
        "line_pipeline",
        LineVertex::layout(),
    )
}
