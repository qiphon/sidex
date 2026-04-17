// Rectangle rendering shader for SideX editor.
//
// SDF-based rounded rectangles with anti-aliased edges.
// Supports filled rects, outlined rects (borders), and box shadows.
// The vertex data carries both the quad corners and the logical rect
// bounds so the fragment shader can evaluate the SDF at each pixel.

struct ViewportUniform {
    projection: mat4x4<f32>,
    scroll_offset: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;

// --- Filled / bordered rounded rectangles ---

struct RectVertexInput {
    @location(0) position: vec2<f32>,
    @location(1) rect_min: vec2<f32>,
    @location(2) rect_max: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) corner_radius: f32,
};

struct RectVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) rect_min: vec2<f32>,
    @location(1) rect_max: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) corner_radius: f32,
    @location(4) pixel_pos: vec2<f32>,
};

fn rounded_rect_sdf(pixel: vec2<f32>, rect_min: vec2<f32>, rect_max: vec2<f32>, radius: f32) -> f32 {
    let half_size = (rect_max - rect_min) * 0.5;
    let center = rect_min + half_size;
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(pixel - center) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@vertex
fn vs_rect(in: RectVertexInput) -> RectVertexOutput {
    var out: RectVertexOutput;
    let world_pos = in.position - viewport.scroll_offset;
    out.clip_position = viewport.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.rect_min = in.rect_min - viewport.scroll_offset;
    out.rect_max = in.rect_max - viewport.scroll_offset;
    out.color = in.color;
    out.corner_radius = in.corner_radius;
    out.pixel_pos = world_pos;
    return out;
}

@fragment
fn fs_rect(in: RectVertexOutput) -> @location(0) vec4<f32> {
    let dist = rounded_rect_sdf(in.pixel_pos, in.rect_min, in.rect_max, in.corner_radius);
    let aa = fwidth(dist);
    let alpha = 1.0 - smoothstep(-aa, aa, dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

// --- Border (outline) rectangles ---
// Uses the same vertex layout. The border is the region between
// the outer SDF and an inner SDF inset by border_width.
// Reuses corner_radius for border_width via a second pass or
// by encoding border_width in an extra field. For simplicity we
// use the same pipeline and draw borders as four thin filled rects
// from the Rust side — this shader handles the single-rect case.

@fragment
fn fs_rect_border(in: RectVertexOutput) -> @location(0) vec4<f32> {
    let border_width = max(in.corner_radius, 1.0);
    let outer_dist = rounded_rect_sdf(in.pixel_pos, in.rect_min, in.rect_max, 0.0);
    let inner_min = in.rect_min + vec2<f32>(border_width, border_width);
    let inner_max = in.rect_max - vec2<f32>(border_width, border_width);
    let inner_dist = rounded_rect_sdf(in.pixel_pos, inner_min, inner_max, 0.0);
    let aa = fwidth(outer_dist);
    let outer_alpha = 1.0 - smoothstep(-aa, aa, outer_dist);
    let inner_alpha = 1.0 - smoothstep(-aa, aa, inner_dist);
    let border_alpha = outer_alpha * (1.0 - inner_alpha);
    return vec4<f32>(in.color.rgb, in.color.a * border_alpha);
}

// --- Box shadow ---
// Inflated rect with blur falloff from the SDF.

@fragment
fn fs_shadow(in: RectVertexOutput) -> @location(0) vec4<f32> {
    let dist = rounded_rect_sdf(in.pixel_pos, in.rect_min, in.rect_max, in.corner_radius);
    let blur = max(in.corner_radius, 1.0);
    let alpha = 1.0 - smoothstep(-blur, blur, dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
