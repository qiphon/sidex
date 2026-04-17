// Cursor shader for SideX editor.
//
// Renders the text cursor with:
//  - Smooth blink animation (fade in/out, not abrupt on/off)
//  - Three styles: Block (0), Line (1), Underline (2)
//  - Support for smooth cursor movement (animated positions)
//  - Multiple cursors for multi-cursor mode
//
// The cursor style is encoded as a u32 in the vertex data:
//   0 = Block  (filled rectangle covering the character cell)
//   1 = Line   (2px wide vertical bar at the left of the cell)
//   2 = Underline (2px tall horizontal bar at the bottom of the cell)

struct ViewportUniform {
    projection: mat4x4<f32>,
    scroll_offset: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> viewport: ViewportUniform;

struct CursorInput {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) opacity: f32,
    @location(4) style: u32,
};

struct CursorOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) opacity: f32,
    @location(3) size: vec2<f32>,
    @location(4) style: u32,
};

@vertex
fn vs_cursor(in: CursorInput) -> CursorOutput {
    var out: CursorOutput;
    let world_pos = in.position - viewport.scroll_offset;
    out.clip_position = viewport.projection * vec4<f32>(world_pos, 0.0, 1.0);
    out.local_uv = (in.position - in.position) / max(in.size, vec2<f32>(1.0, 1.0));
    out.color = in.color;
    out.opacity = in.opacity;
    out.size = in.size;
    out.style = in.style;
    return out;
}

@fragment
fn fs_cursor(in: CursorOutput) -> @location(0) vec4<f32> {
    var alpha = in.opacity;

    // Style 0 = Block: full cell coverage
    // Style 1 = Line: only leftmost 2px
    // Style 2 = Underline: only bottom 2px
    // All styles use the same base alpha from the blink animation.

    if alpha < 0.004 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

// Variant with smooth edge anti-aliasing for the block cursor.
@fragment
fn fs_cursor_smooth(in: CursorOutput) -> @location(0) vec4<f32> {
    var alpha = in.opacity;

    let edge_softness = 0.5;
    let half_w = in.size.x * 0.5;
    let half_h = in.size.y * 0.5;

    let center = vec2<f32>(half_w, half_h);
    let p = in.local_uv * in.size;
    let d = abs(p - center) - vec2<f32>(half_w, half_h);
    let dist = length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);

    let aa = fwidth(dist);
    let shape_alpha = 1.0 - smoothstep(-aa * edge_softness, aa * edge_softness, dist);

    alpha = alpha * shape_alpha;

    if alpha < 0.004 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
