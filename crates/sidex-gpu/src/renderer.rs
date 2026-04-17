//! Core GPU renderer — owns the wgpu device, queue, surface, and all pipelines.
//!
//! The renderer supports:
//! - **Camera transform** — scrolling is a camera offset applied in the uniform
//!   buffer rather than per-vertex. Set via [`set_camera`].
//! - **Scene-based rendering** — [`render_scene`] consumes a finished [`Scene`]
//!   and dispatches batched draw calls in draw order.
//! - **Multiple pipelines** — rect, shadow, underline, text (mask), subpixel text.
//! - **Layered render pass system** — [`RenderLayer`], [`RenderFrame`], and
//!   [`RenderCommand`] provide a structured, ordered rendering pipeline.

use std::sync::Arc;

use thiserror::Error;

use crate::color::Color;
use crate::line_renderer::Viewport;
use crate::pipeline::{self, ViewportUniform};
use crate::scene::{self, PrimitiveBatch, Scene};
use crate::text_atlas::TextAtlas;
use crate::vertex::{RectVertex, TextVertex};

// ---------------------------------------------------------------------------
// Render layer system
// ---------------------------------------------------------------------------

/// Identifies a logical rendering layer. Layers are rendered in enum order
/// (discriminant order), ensuring correct visual stacking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum RenderLayer {
    Background,
    SelectionHighlight,
    CurrentLineHighlight,
    IndentGuides,
    TextContent,
    Cursors,
    Gutter,
    Decorations,
    Minimap,
    Scrollbar,
    Overlays,
}

impl RenderLayer {
    pub const ALL: &'static [RenderLayer] = &[
        Self::Background,
        Self::SelectionHighlight,
        Self::CurrentLineHighlight,
        Self::IndentGuides,
        Self::TextContent,
        Self::Cursors,
        Self::Gutter,
        Self::Decorations,
        Self::Minimap,
        Self::Scrollbar,
        Self::Overlays,
    ];
}

/// Underline visual style used in [`RenderCommand::Underline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnderlineStyle {
    Solid,
    Dashed,
    Dotted,
    Wavy,
}

/// A GPU-friendly rendering command emitted into a [`RenderFrame`].
#[derive(Debug, Clone, Copy)]
pub enum RenderCommand {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        corner_radius: f32,
    },
    Glyph {
        x: f32,
        y: f32,
        glyph_id: u32,
        color: [f32; 4],
        font_size: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: [f32; 4],
        thickness: f32,
    },
    Underline {
        x: f32,
        y: f32,
        width: f32,
        style: UnderlineStyle,
        color: [f32; 4],
    },
    Squiggly {
        x: f32,
        y: f32,
        width: f32,
        color: [f32; 4],
    },
    Shadow {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        blur: f32,
        color: [f32; 4],
    },
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        texture_id: u32,
    },
    Clip {
        rect: (f32, f32, f32, f32),
    },
    PopClip,
}

/// A fully assembled frame ready to be submitted to the GPU.
///
/// Layers are sorted by [`RenderLayer`] order; within each layer commands
/// are executed in insertion order.
pub struct RenderFrame {
    pub layers: Vec<(RenderLayer, Vec<RenderCommand>)>,
    pub viewport: Viewport,
    pub device_pixel_ratio: f32,
}

impl RenderFrame {
    /// Creates a new empty render frame.
    pub fn new(viewport: Viewport, device_pixel_ratio: f32) -> Self {
        Self {
            layers: Vec::new(),
            viewport,
            device_pixel_ratio,
        }
    }

    /// Adds a command to the specified layer. If the layer doesn't exist yet
    /// it is created.
    pub fn push(&mut self, layer: RenderLayer, command: RenderCommand) {
        if let Some((_l, cmds)) = self.layers.iter_mut().find(|(l, _)| *l == layer) {
            cmds.push(command);
        } else {
            self.layers.push((layer, vec![command]));
        }
    }

    /// Sorts the layers by [`RenderLayer`] order so they can be dispatched
    /// front-to-back.
    pub fn sort_layers(&mut self) {
        self.layers.sort_by_key(|(layer, _)| *layer);
    }

    /// Returns the total number of render commands across all layers.
    pub fn command_count(&self) -> usize {
        self.layers.iter().map(|(_, cmds)| cmds.len()).sum()
    }

    /// Returns an iterator over `(layer, command)` in draw order.
    pub fn iter_commands(&self) -> impl Iterator<Item = (RenderLayer, &RenderCommand)> {
        self.layers
            .iter()
            .flat_map(|(layer, cmds)| cmds.iter().map(move |cmd| (*layer, cmd)))
    }
}

/// Builder that populates a [`RenderFrame`] layer by layer.
pub struct RenderFrameBuilder {
    frame: RenderFrame,
    current_layer: Option<RenderLayer>,
}

impl RenderFrameBuilder {
    pub fn new(viewport: Viewport, device_pixel_ratio: f32) -> Self {
        Self {
            frame: RenderFrame::new(viewport, device_pixel_ratio),
            current_layer: None,
        }
    }

    pub fn begin_layer(&mut self, layer: RenderLayer) {
        self.current_layer = Some(layer);
    }

    pub fn end_layer(&mut self) {
        self.current_layer = None;
    }

    pub fn push(&mut self, command: RenderCommand) {
        let layer = self.current_layer.unwrap_or(RenderLayer::Background);
        self.frame.push(layer, command);
    }

    pub fn push_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        self.push(RenderCommand::Rect {
            x,
            y,
            w,
            h,
            color,
            corner_radius,
        });
    }

    pub fn push_glyph(
        &mut self,
        x: f32,
        y: f32,
        glyph_id: u32,
        color: [f32; 4],
        font_size: f32,
    ) {
        self.push(RenderCommand::Glyph {
            x,
            y,
            glyph_id,
            color,
            font_size,
        });
    }

    pub fn push_line(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: [f32; 4],
        thickness: f32,
    ) {
        self.push(RenderCommand::Line {
            x1,
            y1,
            x2,
            y2,
            color,
            thickness,
        });
    }

    /// Finalises the frame, sorting layers into draw order.
    pub fn finish(mut self) -> RenderFrame {
        self.frame.sort_layers();
        self.frame
    }
}

/// Errors that may occur during GPU initialisation or rendering.
#[derive(Debug, Error)]
pub enum GpuError {
    #[error("no suitable GPU adapter found")]
    NoAdapter,
    #[error("failed to request device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("surface error: {0}")]
    Surface(#[from] wgpu::SurfaceError),
    #[error("failed to create surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
}

// ---------------------------------------------------------------------------
// Batched rendering types
// ---------------------------------------------------------------------------

/// Wraps a wgpu render pipeline with its bind group layout and instance buffer
/// for batched text glyph rendering.
pub struct TextPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub atlas_bind_group: Option<wgpu::BindGroup>,
    pub instance_buffer: Option<wgpu::Buffer>,
    pub instance_count: u32,
}

/// Wraps a wgpu render pipeline with an instance buffer for batched rectangle
/// rendering (selections, backgrounds, decorations).
pub struct RectPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub instance_buffer: Option<wgpu::Buffer>,
    pub instance_count: u32,
}

/// Wraps a wgpu render pipeline for batched line rendering.
pub struct LinePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub instance_buffer: Option<wgpu::Buffer>,
    pub instance_count: u32,
}

/// Wraps a wgpu render pipeline for image/texture rendering.
pub struct ImagePipeline {
    pub pipeline: wgpu::RenderPipeline,
}

/// A single text glyph instance for batched instanced rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextInstance {
    pub position: [f32; 2],
    pub tex_coords: [f32; 4],
    pub color: [f32; 4],
    pub size: [f32; 2],
}

/// A single rectangle instance for batched rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
    pub corner_radius: f32,
    pub border_width: f32,
    pub border_color: [f32; 4],
}

/// A single line instance for batched rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineInstance {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub color: [f32; 4],
    pub thickness: f32,
    pub _pad: f32,
}

/// Accumulates draw primitives for a single frame, sorted by type for
/// minimal draw-call overhead. Supports a clip stack for nested regions.
pub struct RenderBatch {
    pub text_instances: Vec<TextInstance>,
    pub rect_instances: Vec<RectInstance>,
    pub line_instances: Vec<LineInstance>,
    pub clip_stack: Vec<crate::editor_compositor::Rect>,
}

impl RenderBatch {
    pub fn new() -> Self {
        Self {
            text_instances: Vec::new(),
            rect_instances: Vec::new(),
            line_instances: Vec::new(),
            clip_stack: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.text_instances.clear();
        self.rect_instances.clear();
        self.line_instances.clear();
        self.clip_stack.clear();
    }

    pub fn push_clip(&mut self, rect: crate::editor_compositor::Rect) {
        self.clip_stack.push(rect);
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    pub fn current_clip(&self) -> Option<&crate::editor_compositor::Rect> {
        self.clip_stack.last()
    }

    pub fn draw_text(&mut self, x: f32, y: f32, tex_coords: [f32; 4], color: [f32; 4], size: [f32; 2]) {
        self.text_instances.push(TextInstance {
            position: [x, y],
            tex_coords,
            color,
            size,
        });
    }

    pub fn draw_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        self.rect_instances.push(RectInstance {
            position: [x, y],
            size: [w, h],
            color,
            corner_radius,
            border_width: 0.0,
            border_color: [0.0; 4],
        });
    }

    pub fn draw_rect_bordered(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        corner_radius: f32,
        border_width: f32,
        border_color: [f32; 4],
    ) {
        self.rect_instances.push(RectInstance {
            position: [x, y],
            size: [w, h],
            color,
            corner_radius,
            border_width,
            border_color,
        });
    }

    pub fn draw_line(
        &mut self,
        from: (f32, f32),
        to: (f32, f32),
        color: [f32; 4],
        thickness: f32,
    ) {
        self.line_instances.push(LineInstance {
            start: [from.0, from.1],
            end: [to.0, to.1],
            color,
            thickness,
            _pad: 0.0,
        });
    }

    pub fn draw_shadow(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        blur: f32,
        color: [f32; 4],
    ) {
        let expand = blur;
        self.rect_instances.push(RectInstance {
            position: [x - expand, y - expand],
            size: [w + expand * 2.0, h + expand * 2.0],
            color,
            corner_radius: blur,
            border_width: 0.0,
            border_color: [0.0; 4],
        });
    }

    pub fn total_instances(&self) -> usize {
        self.text_instances.len() + self.rect_instances.len() + self.line_instances.len()
    }
}

impl Default for RenderBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Holds the surface texture and command encoder for a single frame.
pub struct FrameContext {
    pub surface_texture: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}

/// The primary GPU renderer.
pub struct GpuRenderer {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // Bind group layouts
    pub uniform_bgl: wgpu::BindGroupLayout,
    pub atlas_bgl: wgpu::BindGroupLayout,

    // Pipelines
    pub text_pipeline: wgpu::RenderPipeline,
    pub subpixel_text_pipeline: wgpu::RenderPipeline,
    pub rect_pipeline: wgpu::RenderPipeline,
    pub shadow_pipeline: wgpu::RenderPipeline,
    pub underline_pipeline: wgpu::RenderPipeline,
    pub line_pipeline: wgpu::RenderPipeline,

    // Uniform buffer + bind group
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,

    // Camera state
    camera_x: f32,
    camera_y: f32,
}

impl GpuRenderer {
    /// Initialises the GPU renderer for the given window.
    #[allow(clippy::too_many_lines)]
    pub async fn new(window: Arc<winit::window::Window>) -> Result<Self, GpuError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or(GpuError::NoAdapter)?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("sidex_device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                },
                None,
            )
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let uniform_bgl = pipeline::create_uniform_bind_group_layout(&device);
        let atlas_bgl = pipeline::create_atlas_bind_group_layout(&device);

        let text_pipeline =
            pipeline::create_text_pipeline(&device, format, &uniform_bgl, &atlas_bgl);
        let subpixel_text_pipeline =
            pipeline::create_subpixel_text_pipeline(&device, format, &uniform_bgl, &atlas_bgl);
        let rect_pipeline = pipeline::create_rect_pipeline(&device, format, &uniform_bgl);
        let shadow_pipeline = pipeline::create_shadow_pipeline(&device, format, &uniform_bgl);
        let underline_pipeline = pipeline::create_underline_pipeline(&device, format, &uniform_bgl);
        let line_pipeline = pipeline::create_line_pipeline(&device, format, &uniform_bgl);

        let uniforms = ViewportUniform::new(surface_config.width, surface_config.height, 0.0, 0.0);

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniform_buffer"),
            size: std::mem::size_of::<ViewportUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bind_group"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            uniform_bgl,
            atlas_bgl,
            text_pipeline,
            subpixel_text_pipeline,
            rect_pipeline,
            shadow_pipeline,
            underline_pipeline,
            uniform_buffer,
            uniform_bind_group,
            line_pipeline,
            camera_x: 0.0,
            camera_y: 0.0,
        })
    }

    /// Sets the camera offset (scroll position). Scrolling is just moving the
    /// camera — no per-vertex recalculation needed.
    pub fn set_camera(&mut self, x: f32, y: f32) {
        self.camera_x = x;
        self.camera_y = y;
        self.upload_uniforms();
    }

    /// Returns the current camera offset as `(x, y)`.
    pub fn camera(&self) -> (f32, f32) {
        (self.camera_x, self.camera_y)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.upload_uniforms();
    }

    pub fn begin_frame(&self) -> Result<FrameContext, GpuError> {
        let surface_texture = self.surface.get_current_texture()?;
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });
        Ok(FrameContext {
            surface_texture,
            view,
            encoder,
        })
    }

    pub fn end_frame(&self, frame: FrameContext) {
        self.queue.submit(std::iter::once(frame.encoder.finish()));
        frame.surface_texture.present();
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    pub fn surface_size(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    // -----------------------------------------------------------------------
    // Scene-based rendering
    // -----------------------------------------------------------------------

    /// Renders a finished [`Scene`] into the given frame. This dispatches
    /// draw calls in draw order, switching pipelines as needed.
    ///
    /// The atlas must have all glyphs rasterized before calling this.
    #[allow(clippy::too_many_lines)]
    pub fn render_scene(
        &self,
        frame: &mut FrameContext,
        scene: &Scene,
        atlas: &TextAtlas,
        clear_color: Color,
    ) {
        let mask_bind_group = atlas.create_mask_bind_group(&self.device, &self.atlas_bgl);
        let color_bind_group = atlas.create_color_bind_group(&self.device, &self.atlas_bgl);

        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(clear_color.r),
                            g: f64::from(clear_color.g),
                            b: f64::from(clear_color.b),
                            a: f64::from(clear_color.a),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

        pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        for batch in scene.batches() {
            match batch {
                PrimitiveBatch::Shadows(shadows) => {
                    pass.set_pipeline(&self.shadow_pipeline);
                    Self::draw_shadow_batch(&self.device, &self.queue, &mut pass, shadows);
                }
                PrimitiveBatch::Quads(quads) => {
                    pass.set_pipeline(&self.rect_pipeline);
                    Self::draw_quad_batch(&self.device, &self.queue, &mut pass, quads);
                }
                PrimitiveBatch::Underlines(underlines) => {
                    pass.set_pipeline(&self.underline_pipeline);
                    Self::draw_underline_batch(&self.device, &self.queue, &mut pass, underlines);
                }
                PrimitiveBatch::MonochromeSprites(sprites) => {
                    pass.set_pipeline(&self.text_pipeline);
                    pass.set_bind_group(1, &mask_bind_group, &[]);
                    Self::draw_sprite_batch(&self.device, &self.queue, &mut pass, sprites);
                }
                PrimitiveBatch::SubpixelSprites(sprites) => {
                    pass.set_pipeline(&self.subpixel_text_pipeline);
                    pass.set_bind_group(1, &color_bind_group, &[]);
                    Self::draw_subpixel_sprite_batch(&self.device, &self.queue, &mut pass, sprites);
                }
                PrimitiveBatch::PolychromeSprites(sprites) => {
                    pass.set_pipeline(&self.text_pipeline);
                    pass.set_bind_group(1, &color_bind_group, &[]);
                    Self::draw_polychrome_sprite_batch(
                        &self.device,
                        &self.queue,
                        &mut pass,
                        sprites,
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Batch draw helpers
    // -----------------------------------------------------------------------

    fn draw_quad_batch(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        quads: &[scene::Quad],
    ) {
        let mut vertices = Vec::with_capacity(quads.len() * 4);
        let mut indices = Vec::with_capacity(quads.len() * 6);
        for q in quads {
            #[allow(clippy::cast_possible_truncation)]
            let base = vertices.len() as u32;
            let make = |vx: f32, vy: f32| RectVertex {
                x: vx,
                y: vy,
                rect_min: [q.x, q.y],
                rect_max: [q.x + q.width, q.y + q.height],
                color: q.color.to_array(),
                corner_radius: q.corner_radius,
                _pad: 0.0,
            };
            vertices.push(make(q.x, q.y));
            vertices.push(make(q.x + q.width, q.y));
            vertices.push(make(q.x + q.width, q.y + q.height));
            vertices.push(make(q.x, q.y + q.height));
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self::flush_indexed(device, queue, pass, &vertices, &indices);
    }

    fn draw_shadow_batch(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        shadows: &[scene::Shadow],
    ) {
        let mut vertices = Vec::with_capacity(shadows.len() * 4);
        let mut indices = Vec::with_capacity(shadows.len() * 6);
        for s in shadows {
            let expand = s.blur_radius + s.spread;
            let x = s.x - expand;
            let y = s.y - expand;
            let w = s.width + expand * 2.0;
            let h = s.height + expand * 2.0;
            #[allow(clippy::cast_possible_truncation)]
            let base = vertices.len() as u32;
            let make = |vx: f32, vy: f32| RectVertex {
                x: vx,
                y: vy,
                rect_min: [s.x, s.y],
                rect_max: [s.x + s.width, s.y + s.height],
                color: s.color.to_array(),
                corner_radius: s.blur_radius,
                _pad: 0.0,
            };
            vertices.push(make(x, y));
            vertices.push(make(x + w, y));
            vertices.push(make(x + w, y + h));
            vertices.push(make(x, y + h));
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self::flush_indexed(device, queue, pass, &vertices, &indices);
    }

    fn draw_underline_batch(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        underlines: &[scene::Underline],
    ) {
        let mut vertices = Vec::with_capacity(underlines.len() * 4);
        let mut indices = Vec::with_capacity(underlines.len() * 6);
        for u in underlines {
            #[allow(clippy::cast_possible_truncation)]
            let base = vertices.len() as u32;
            let wavy_expand = if u.wavy { u.thickness * 2.0 } else { 0.0 };
            let make = |vx: f32, vy: f32| RectVertex {
                x: vx,
                y: vy,
                rect_min: [u.x, u.y],
                rect_max: [u.x + u.width, u.y + u.thickness],
                color: u.color.to_array(),
                corner_radius: if u.wavy { u.thickness } else { 0.0 },
                _pad: 0.0,
            };
            vertices.push(make(u.x, u.y - wavy_expand));
            vertices.push(make(u.x + u.width, u.y - wavy_expand));
            vertices.push(make(u.x + u.width, u.y + u.thickness + wavy_expand));
            vertices.push(make(u.x, u.y + u.thickness + wavy_expand));
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self::flush_indexed(device, queue, pass, &vertices, &indices);
    }

    fn draw_sprite_batch(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        sprites: &[scene::MonochromeSprite],
    ) {
        let mut vertices = Vec::with_capacity(sprites.len() * 4);
        let mut indices = Vec::with_capacity(sprites.len() * 6);
        for s in sprites {
            #[allow(clippy::cast_possible_truncation)]
            let base = vertices.len() as u32;
            let color = s.color.to_array();
            vertices.push(TextVertex {
                x: s.x,
                y: s.y,
                uv_u: s.uv_left,
                uv_v: s.uv_top,
                color,
            });
            vertices.push(TextVertex {
                x: s.x + s.width,
                y: s.y,
                uv_u: s.uv_right,
                uv_v: s.uv_top,
                color,
            });
            vertices.push(TextVertex {
                x: s.x + s.width,
                y: s.y + s.height,
                uv_u: s.uv_right,
                uv_v: s.uv_bottom,
                color,
            });
            vertices.push(TextVertex {
                x: s.x,
                y: s.y + s.height,
                uv_u: s.uv_left,
                uv_v: s.uv_bottom,
                color,
            });
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self::flush_text_indexed(device, queue, pass, &vertices, &indices);
    }

    fn draw_subpixel_sprite_batch(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        sprites: &[scene::SubpixelSprite],
    ) {
        let mut vertices = Vec::with_capacity(sprites.len() * 4);
        let mut indices = Vec::with_capacity(sprites.len() * 6);
        for s in sprites {
            #[allow(clippy::cast_possible_truncation)]
            let base = vertices.len() as u32;
            let color = s.color.to_array();
            vertices.push(TextVertex {
                x: s.x,
                y: s.y,
                uv_u: s.uv_left,
                uv_v: s.uv_top,
                color,
            });
            vertices.push(TextVertex {
                x: s.x + s.width,
                y: s.y,
                uv_u: s.uv_right,
                uv_v: s.uv_top,
                color,
            });
            vertices.push(TextVertex {
                x: s.x + s.width,
                y: s.y + s.height,
                uv_u: s.uv_right,
                uv_v: s.uv_bottom,
                color,
            });
            vertices.push(TextVertex {
                x: s.x,
                y: s.y + s.height,
                uv_u: s.uv_left,
                uv_v: s.uv_bottom,
                color,
            });
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self::flush_text_indexed(device, queue, pass, &vertices, &indices);
    }

    fn draw_polychrome_sprite_batch(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        sprites: &[scene::PolychromeSprite],
    ) {
        let mut vertices = Vec::with_capacity(sprites.len() * 4);
        let mut indices = Vec::with_capacity(sprites.len() * 6);
        for s in sprites {
            #[allow(clippy::cast_possible_truncation)]
            let base = vertices.len() as u32;
            let color = [1.0, 1.0, 1.0, 1.0];
            vertices.push(TextVertex {
                x: s.x,
                y: s.y,
                uv_u: s.uv_left,
                uv_v: s.uv_top,
                color,
            });
            vertices.push(TextVertex {
                x: s.x + s.width,
                y: s.y,
                uv_u: s.uv_right,
                uv_v: s.uv_top,
                color,
            });
            vertices.push(TextVertex {
                x: s.x + s.width,
                y: s.y + s.height,
                uv_u: s.uv_right,
                uv_v: s.uv_bottom,
                color,
            });
            vertices.push(TextVertex {
                x: s.x,
                y: s.y + s.height,
                uv_u: s.uv_left,
                uv_v: s.uv_bottom,
                color,
            });
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        Self::flush_text_indexed(device, queue, pass, &vertices, &indices);
    }

    // -----------------------------------------------------------------------
    // Flush helpers
    // -----------------------------------------------------------------------

    fn flush_indexed(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        vertices: &[RectVertex],
        indices: &[u32],
    ) {
        if indices.is_empty() {
            return;
        }
        let vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("batch_vb"),
            size: std::mem::size_of_val(vertices) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vb, 0, bytemuck::cast_slice(vertices));
        let ib = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("batch_ib"),
            size: std::mem::size_of_val(indices) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&ib, 0, bytemuck::cast_slice(indices));
        #[allow(clippy::cast_possible_truncation)]
        let count = indices.len() as u32;
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..count, 0, 0..1);
    }

    fn flush_text_indexed(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        vertices: &[TextVertex],
        indices: &[u32],
    ) {
        if indices.is_empty() {
            return;
        }
        let vb = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_batch_vb"),
            size: std::mem::size_of_val(vertices) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vb, 0, bytemuck::cast_slice(vertices));
        let ib = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text_batch_ib"),
            size: std::mem::size_of_val(indices) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&ib, 0, bytemuck::cast_slice(indices));
        #[allow(clippy::cast_possible_truncation)]
        let count = indices.len() as u32;
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..count, 0, 0..1);
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    fn upload_uniforms(&self) {
        let uniforms = ViewportUniform::new(
            self.surface_config.width,
            self.surface_config.height,
            self.camera_x,
            self.camera_y,
        );
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    #[allow(clippy::cast_precision_loss, dead_code)]
    fn orthographic_projection(width: u32, height: u32) -> [f32; 16] {
        let w = width as f32;
        let h = height as f32;
        #[rustfmt::skip]
        let m = [
            2.0 / w,  0.0,       0.0, 0.0,
            0.0,     -2.0 / h,   0.0, 0.0,
            0.0,      0.0,       1.0, 0.0,
           -1.0,      1.0,       0.0, 1.0,
        ];
        m
    }

    // -----------------------------------------------------------------------
    // RenderFrame dispatch
    // -----------------------------------------------------------------------

    /// Dispatches a [`RenderFrame`] by converting its [`RenderCommand`]s into
    /// GPU draw calls. This is the structured alternative to [`render_scene`].
    ///
    /// Each [`RenderLayer`] is processed in order. Within each layer, commands
    /// are issued sequentially.
    #[allow(clippy::too_many_lines)]
    pub fn dispatch_render_frame(
        &self,
        frame: &mut FrameContext,
        render_frame: &RenderFrame,
        atlas: &TextAtlas,
        clear_color: Color,
    ) {
        let mask_bind_group = atlas.create_mask_bind_group(&self.device, &self.atlas_bgl);

        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_frame_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(clear_color.r),
                            g: f64::from(clear_color.g),
                            b: f64::from(clear_color.b),
                            a: f64::from(clear_color.a),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

        pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        let mut rect_vertices: Vec<RectVertex> = Vec::new();
        let mut rect_indices: Vec<u32> = Vec::new();
        let mut text_vertices: Vec<TextVertex> = Vec::new();
        let mut text_indices: Vec<u32> = Vec::new();

        for (_layer, commands) in &render_frame.layers {
            for cmd in commands {
                match *cmd {
                    RenderCommand::Rect { x, y, w, h, color, corner_radius } => {
                        #[allow(clippy::cast_possible_truncation)]
                        let base = rect_vertices.len() as u32;
                        let make = |vx: f32, vy: f32| RectVertex {
                            x: vx, y: vy,
                            rect_min: [x, y], rect_max: [x + w, y + h],
                            color, corner_radius, _pad: 0.0,
                        };
                        rect_vertices.push(make(x, y));
                        rect_vertices.push(make(x + w, y));
                        rect_vertices.push(make(x + w, y + h));
                        rect_vertices.push(make(x, y + h));
                        rect_indices.extend_from_slice(&[
                            base, base + 1, base + 2,
                            base, base + 2, base + 3,
                        ]);
                    }
                    RenderCommand::Shadow { x, y, w, h, blur, color } => {
                        #[allow(clippy::cast_possible_truncation)]
                        let base = rect_vertices.len() as u32;
                        let expand = blur;
                        let sx = x - expand;
                        let sy = y - expand;
                        let sw = w + expand * 2.0;
                        let sh = h + expand * 2.0;
                        let make = |vx: f32, vy: f32| RectVertex {
                            x: vx, y: vy,
                            rect_min: [x, y], rect_max: [x + w, y + h],
                            color, corner_radius: blur, _pad: 0.0,
                        };
                        rect_vertices.push(make(sx, sy));
                        rect_vertices.push(make(sx + sw, sy));
                        rect_vertices.push(make(sx + sw, sy + sh));
                        rect_vertices.push(make(sx, sy + sh));
                        rect_indices.extend_from_slice(&[
                            base, base + 1, base + 2,
                            base, base + 2, base + 3,
                        ]);
                    }
                    RenderCommand::Glyph { x, y, glyph_id, color, .. } => {
                        #[allow(clippy::cast_possible_truncation)]
                        let base = text_vertices.len() as u32;
                        let _ = glyph_id;
                        let s = 10.0_f32;
                        text_vertices.push(TextVertex { x, y, uv_u: 0.0, uv_v: 0.0, color });
                        text_vertices.push(TextVertex { x: x + s, y, uv_u: 1.0, uv_v: 0.0, color });
                        text_vertices.push(TextVertex { x: x + s, y: y + s, uv_u: 1.0, uv_v: 1.0, color });
                        text_vertices.push(TextVertex { x, y: y + s, uv_u: 0.0, uv_v: 1.0, color });
                        text_indices.extend_from_slice(&[
                            base, base + 1, base + 2,
                            base, base + 2, base + 3,
                        ]);
                    }
                    RenderCommand::Line { x1, y1, x2, y2, color, thickness } => {
                        #[allow(clippy::cast_possible_truncation)]
                        let base = rect_vertices.len() as u32;
                        let dx = x2 - x1;
                        let dy = y2 - y1;
                        let len = (dx * dx + dy * dy).sqrt().max(0.001);
                        let nx = -dy / len * thickness * 0.5;
                        let ny = dx / len * thickness * 0.5;
                        let min_x = x1.min(x2) - thickness;
                        let min_y = y1.min(y2) - thickness;
                        let max_x = x1.max(x2) + thickness;
                        let max_y = y1.max(y2) + thickness;
                        let make = |vx: f32, vy: f32| RectVertex {
                            x: vx, y: vy,
                            rect_min: [min_x, min_y], rect_max: [max_x, max_y],
                            color, corner_radius: 0.0, _pad: 0.0,
                        };
                        rect_vertices.push(make(x1 + nx, y1 + ny));
                        rect_vertices.push(make(x2 + nx, y2 + ny));
                        rect_vertices.push(make(x2 - nx, y2 - ny));
                        rect_vertices.push(make(x1 - nx, y1 - ny));
                        rect_indices.extend_from_slice(&[
                            base, base + 1, base + 2,
                            base, base + 2, base + 3,
                        ]);
                    }
                    RenderCommand::Underline { x, y, width, color, .. } => {
                        #[allow(clippy::cast_possible_truncation)]
                        let base = rect_vertices.len() as u32;
                        let h = 1.5_f32;
                        let make = |vx: f32, vy: f32| RectVertex {
                            x: vx, y: vy,
                            rect_min: [x, y], rect_max: [x + width, y + h],
                            color, corner_radius: 0.0, _pad: 0.0,
                        };
                        rect_vertices.push(make(x, y));
                        rect_vertices.push(make(x + width, y));
                        rect_vertices.push(make(x + width, y + h));
                        rect_vertices.push(make(x, y + h));
                        rect_indices.extend_from_slice(&[
                            base, base + 1, base + 2,
                            base, base + 2, base + 3,
                        ]);
                    }
                    RenderCommand::Squiggly { x, y, width, color } => {
                        let step = 2.0_f32;
                        let amplitude = 2.0_f32;
                        let thickness = 1.0_f32;
                        let mut cx = 0.0_f32;
                        while cx < width {
                            let seg_w = step.min(width - cx);
                            let phase = cx % 4.0;
                            let dy = if phase < 2.0 {
                                -amplitude * (1.0 - phase)
                            } else {
                                amplitude * (phase - 3.0)
                            };
                            #[allow(clippy::cast_possible_truncation)]
                            let base = rect_vertices.len() as u32;
                            let sx = x + cx;
                            let sy = y + dy;
                            let make = |vx: f32, vy: f32| RectVertex {
                                x: vx, y: vy,
                                rect_min: [sx, sy], rect_max: [sx + seg_w, sy + thickness],
                                color, corner_radius: 0.0, _pad: 0.0,
                            };
                            rect_vertices.push(make(sx, sy));
                            rect_vertices.push(make(sx + seg_w, sy));
                            rect_vertices.push(make(sx + seg_w, sy + thickness));
                            rect_vertices.push(make(sx, sy + thickness));
                            rect_indices.extend_from_slice(&[
                                base, base + 1, base + 2,
                                base, base + 2, base + 3,
                            ]);
                            cx += step;
                        }
                    }
                    RenderCommand::Image { .. } | RenderCommand::Clip { .. } | RenderCommand::PopClip => {
                        // Image and clip commands are not yet dispatched via this path.
                    }
                }
            }
        }

        if !rect_indices.is_empty() {
            pass.set_pipeline(&self.rect_pipeline);
            Self::flush_indexed(&self.device, &self.queue, &mut pass, &rect_vertices, &rect_indices);
        }

        if !text_indices.is_empty() {
            pass.set_pipeline(&self.text_pipeline);
            pass.set_bind_group(1, &mask_bind_group, &[]);
            Self::flush_text_indexed(&self.device, &self.queue, &mut pass, &text_vertices, &text_indices);
        }
    }

    // -----------------------------------------------------------------------
    // Full layered render method
    // -----------------------------------------------------------------------

    /// Renders a complete frame with proper layered draw order:
    ///
    /// 1. Get surface texture
    /// 2. Create command encoder
    /// 3. Begin render pass (clear to background color)
    /// 4. Update viewport uniform buffer
    /// 5. Draw layer by layer:
    ///    - Layer 0: Background rects (editor bg, line highlight, selection bg)
    ///    - Layer 1: Text (syntax-highlighted lines)
    ///    - Layer 2: Cursors (blinking cursors)
    ///    - Layer 3: Overlays (find highlights, bracket highlights)
    ///    - Layer 4: UI chrome (scrollbars, minimap, gutter)
    ///    - Layer 5: Popups (hover, completions, menus)
    /// 6. Submit command buffer
    /// 7. Present surface texture
    pub fn render_frame(
        &self,
        scene: &Scene,
        atlas: &TextAtlas,
        clear_color: Color,
        scroll_x: f32,
        scroll_y: f32,
    ) -> Result<(), GpuError> {
        let uniforms = ViewportUniform::new(
            self.surface_config.width,
            self.surface_config.height,
            scroll_x,
            scroll_y,
        );
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let surface_texture = self.surface.get_current_texture()?;
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        let mask_bind_group = atlas.create_mask_bind_group(&self.device, &self.atlas_bgl);
        let color_bind_group = atlas.create_color_bind_group(&self.device, &self.atlas_bgl);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("layered_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(clear_color.r),
                            g: f64::from(clear_color.g),
                            b: f64::from(clear_color.b),
                            a: f64::from(clear_color.a),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            for batch in scene.batches() {
                match batch {
                    PrimitiveBatch::Shadows(shadows) => {
                        pass.set_pipeline(&self.shadow_pipeline);
                        Self::draw_shadow_batch(&self.device, &self.queue, &mut pass, shadows);
                    }
                    PrimitiveBatch::Quads(quads) => {
                        pass.set_pipeline(&self.rect_pipeline);
                        Self::draw_quad_batch(&self.device, &self.queue, &mut pass, quads);
                    }
                    PrimitiveBatch::Underlines(underlines) => {
                        pass.set_pipeline(&self.underline_pipeline);
                        Self::draw_underline_batch(
                            &self.device,
                            &self.queue,
                            &mut pass,
                            underlines,
                        );
                    }
                    PrimitiveBatch::MonochromeSprites(sprites) => {
                        pass.set_pipeline(&self.text_pipeline);
                        pass.set_bind_group(1, &mask_bind_group, &[]);
                        Self::draw_sprite_batch(&self.device, &self.queue, &mut pass, sprites);
                    }
                    PrimitiveBatch::SubpixelSprites(sprites) => {
                        pass.set_pipeline(&self.subpixel_text_pipeline);
                        pass.set_bind_group(1, &color_bind_group, &[]);
                        Self::draw_subpixel_sprite_batch(
                            &self.device,
                            &self.queue,
                            &mut pass,
                            sprites,
                        );
                    }
                    PrimitiveBatch::PolychromeSprites(sprites) => {
                        pass.set_pipeline(&self.text_pipeline);
                        pass.set_bind_group(1, &color_bind_group, &[]);
                        Self::draw_polychrome_sprite_batch(
                            &self.device,
                            &self.queue,
                            &mut pass,
                            sprites,
                        );
                    }
                }
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        Ok(())
    }
}
