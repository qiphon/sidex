// Rounded rectangle shader for SideX editor UI elements.
//
// SDF-based rendering of rounded rectangles with:
//  - Smooth anti-aliased edges
//  - Optional border (configurable width + color)
//  - Optional drop shadow (separate pass)
//
// Used for buttons, tooltips, autocomplete popups, hover cards, and
// any UI surface that needs rounded corners.

struct ViewportUniform {
    projection: mat4x4<f32>,
    scroll_offset: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;

// --- Vertex I/O ---

struct RoundedRectInput {
    @location(0) position: vec2<f32>,
    @location(1) rect_min: vec2<f32>,
    @location(2) rect_max: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) corner_radius: f32,
    @location(5) border_width: f32,
    @location(6) border_color: vec4<f32>,
};

struct RoundedRectOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) rect_min: vec2<f32>,
    @location(1) rect_max: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) corner_radius: f32,
    @location(4) border_width: f32,
    @location(5) border_color: vec4<f32>,
    @location(6) pixel_pos: vec2<f32>,
};

// --- SDF ---

fn rounded_rect_sdf(pixel: vec2<f32>, rect_min: vec2<f32>, rect_max: vec2<f32>, radius: f32) -> f32 {
    let half_size = (rect_max - rect_min) * 0.5;
    let center = rect_min + half_size;
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(pixel - center) - half_size + vec2<f32>(r, r);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

// --- Vertex shader ---

@vertex
fn vs_rounded_rect(in: RoundedRectInput) -> RoundedRectOutput {
    var out: RoundedRectOutput;
    let world_pos = in.position - viewport.scroll_offset;
    out.clip_position = viewport.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.rect_min = in.rect_min - viewport.scroll_offset;
    out.rect_max = in.rect_max - viewport.scroll_offset;
    out.color = in.color;
    out.corner_radius = in.corner_radius;
    out.border_width = in.border_width;
    out.border_color = in.border_color;
    out.pixel_pos = world_pos;
    return out;
}

// --- Fragment: filled rounded rect ---

@fragment
fn fs_rounded_rect_fill(in: RoundedRectOutput) -> @location(0) vec4<f32> {
    let dist = rounded_rect_sdf(in.pixel_pos, in.rect_min, in.rect_max, in.corner_radius);
    let aa = fwidth(dist);
    let alpha = 1.0 - smoothstep(-aa, aa, dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

// --- Fragment: bordered rounded rect ---
// Renders both fill and border. The border is the region between the
// outer SDF and an inner SDF inset by `border_width`.

@fragment
fn fs_rounded_rect_bordered(in: RoundedRectOutput) -> @location(0) vec4<f32> {
    let outer_dist = rounded_rect_sdf(in.pixel_pos, in.rect_min, in.rect_max, in.corner_radius);
    let aa = fwidth(outer_dist);
    let outer_alpha = 1.0 - smoothstep(-aa, aa, outer_dist);

    if in.border_width <= 0.0 {
        return vec4<f32>(in.color.rgb, in.color.a * outer_alpha);
    }

    let inner_min = in.rect_min + vec2<f32>(in.border_width, in.border_width);
    let inner_max = in.rect_max - vec2<f32>(in.border_width, in.border_width);
    let inner_radius = max(in.corner_radius - in.border_width, 0.0);
    let inner_dist = rounded_rect_sdf(in.pixel_pos, inner_min, inner_max, inner_radius);
    let inner_alpha = 1.0 - smoothstep(-aa, aa, inner_dist);

    let border_mask = outer_alpha * (1.0 - inner_alpha);
    let fill_alpha = outer_alpha * inner_alpha;

    let fill_color = vec4<f32>(in.color.rgb, in.color.a * fill_alpha);
    let border_result = vec4<f32>(in.border_color.rgb, in.border_color.a * border_mask);

    // Composite border over fill.
    let out_a = border_result.a + fill_color.a * (1.0 - border_result.a);
    if out_a < 0.001 {
        discard;
    }
    let out_rgb = (border_result.rgb * border_result.a + fill_color.rgb * fill_color.a * (1.0 - border_result.a)) / out_a;
    return vec4<f32>(out_rgb, out_a);
}

// --- Fragment: shadow for rounded rect ---
// Renders a soft shadow by blurring the SDF. The `corner_radius` field
// is repurposed as blur_radius in the shadow pass.

@fragment
fn fs_rounded_rect_shadow(in: RoundedRectOutput) -> @location(0) vec4<f32> {
    let dist = rounded_rect_sdf(in.pixel_pos, in.rect_min, in.rect_max, in.corner_radius);
    let blur = max(in.border_width, 1.0);
    let alpha = 1.0 - smoothstep(-blur, blur * 2.0, dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
