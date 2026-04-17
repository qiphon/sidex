// Selection highlight shader for SideX editor.
//
// Renders selection rectangles with:
//  - Semi-transparent fill (default blue at ~30% opacity)
//  - Slightly rounded corners
//  - Support for multiple selections (multi-cursor)
//
// Each selection is a separate quad using the same vertex layout as
// the rounded rect shader.

struct ViewportUniform {
    projection: mat4x4<f32>,
    scroll_offset: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;

struct SelectionInput {
    @location(0) position: vec2<f32>,
    @location(1) rect_min: vec2<f32>,
    @location(2) rect_max: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) corner_radius: f32,
};

struct SelectionOutput {
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
fn vs_selection(in: SelectionInput) -> SelectionOutput {
    var out: SelectionOutput;
    let world_pos = in.position - viewport.scroll_offset;
    out.clip_position = viewport.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.rect_min = in.rect_min - viewport.scroll_offset;
    out.rect_max = in.rect_max - viewport.scroll_offset;
    out.color = in.color;
    out.corner_radius = in.corner_radius;
    out.pixel_pos = world_pos;
    return out;
}

// Semi-transparent selection fill with slightly rounded corners.
@fragment
fn fs_selection(in: SelectionOutput) -> @location(0) vec4<f32> {
    let dist = rounded_rect_sdf(in.pixel_pos, in.rect_min, in.rect_max, in.corner_radius);
    let aa = fwidth(dist);
    let alpha = 1.0 - smoothstep(-aa, aa, dist);
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

// Highlighted selection with a subtle border (for the current/active selection).
@fragment
fn fs_selection_active(in: SelectionOutput) -> @location(0) vec4<f32> {
    let dist = rounded_rect_sdf(in.pixel_pos, in.rect_min, in.rect_max, in.corner_radius);
    let aa = fwidth(dist);
    let fill_alpha = 1.0 - smoothstep(-aa, aa, dist);

    let border_width = 1.0;
    let inner_min = in.rect_min + vec2<f32>(border_width, border_width);
    let inner_max = in.rect_max - vec2<f32>(border_width, border_width);
    let inner_dist = rounded_rect_sdf(in.pixel_pos, inner_min, inner_max, max(in.corner_radius - border_width, 0.0));
    let inner_alpha = 1.0 - smoothstep(-aa, aa, inner_dist);

    let border_mask = fill_alpha * (1.0 - inner_alpha);
    let border_color = vec4<f32>(in.color.rgb * 1.5, min(in.color.a * 2.0, 1.0));
    let fill_color = vec4<f32>(in.color.rgb, in.color.a * fill_alpha * inner_alpha);

    let out_a = border_color.a * border_mask + fill_color.a * (1.0 - border_mask);
    if out_a < 0.001 {
        discard;
    }
    let out_rgb = (border_color.rgb * border_color.a * border_mask + fill_color.rgb * fill_color.a * (1.0 - border_mask)) / out_a;
    return vec4<f32>(out_rgb, out_a);
}
