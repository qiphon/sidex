// Line rendering shader for SideX editor.
//
// Used for indent guides, rulers, borders, and other thin lines.
// Takes start/end points, thickness, and color. The fragment shader
// computes the signed distance to the line segment and anti-aliases
// the edge.

struct ViewportUniform {
    projection: mat4x4<f32>,
    scroll_offset: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;

struct LineVertexInput {
    @location(0) position: vec2<f32>,
    @location(1) line_start: vec2<f32>,
    @location(2) line_end: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) thickness: f32,
};

struct LineVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) line_start: vec2<f32>,
    @location(1) line_end: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) thickness: f32,
    @location(4) pixel_pos: vec2<f32>,
};

@vertex
fn vs_line(in: LineVertexInput) -> LineVertexOutput {
    var out: LineVertexOutput;
    let world_pos = in.position - viewport.scroll_offset;
    out.clip_position = viewport.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.line_start = in.line_start - viewport.scroll_offset;
    out.line_end = in.line_end - viewport.scroll_offset;
    out.color = in.color;
    out.thickness = in.thickness;
    out.pixel_pos = world_pos;
    return out;
}

// Signed distance from point `p` to line segment `a`-`b`.
fn segment_sdf(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

@fragment
fn fs_line(in: LineVertexOutput) -> @location(0) vec4<f32> {
    let dist = segment_sdf(in.pixel_pos, in.line_start, in.line_end);
    let half_thickness = in.thickness * 0.5;
    let edge_dist = dist - half_thickness;
    let aa = fwidth(edge_dist);
    let alpha = 1.0 - smoothstep(-aa, aa, edge_dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
