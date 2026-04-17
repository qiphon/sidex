// Text rendering shader for SideX editor.
//
// Supports both monochrome (alpha-mask from R8 atlas) and subpixel
// (RGB coverage from Rgba8 atlas) rendering modes. The fragment shader
// entry points are split: `fs_mono` for grayscale text and `fs_subpixel`
// for per-channel coverage.

struct ViewportUniform {
    projection: mat4x4<f32>,
    scroll_offset: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;
@group(1) @binding(0) var atlas_texture: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = in.position - viewport.scroll_offset;
    out.clip_position = viewport.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

// Monochrome text: single alpha channel from R8 atlas.
@fragment
fn fs_mono(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(atlas_texture, atlas_sampler, in.uv).r;
    if alpha < 0.004 {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

// Subpixel text: separate R, G, B coverage channels from Rgba8 atlas.
// Each channel carries independent coverage for its subpixel stripe,
// producing sharper text on LCD panels.
@fragment
fn fs_subpixel(in: VertexOutput) -> @location(0) vec4<f32> {
    let coverage = textureSample(atlas_texture, atlas_sampler, in.uv);
    let r = in.color.r * coverage.r;
    let g = in.color.g * coverage.g;
    let b = in.color.b * coverage.b;
    let a = max(max(coverage.r, coverage.g), coverage.b) * in.color.a;
    if a < 0.004 {
        discard;
    }
    return vec4<f32>(r, g, b, a);
}
