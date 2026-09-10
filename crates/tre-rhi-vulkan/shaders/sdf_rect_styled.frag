#version 450

// Phase 10 Step 10.2: the non-uniform-corner / bordered / smoothed
// rounded-rectangle shader. Paired at pipeline-creation time with the
// EXISTING `sdf_rounded_rect.vert` (not a new vertex shader) -- both
// shaders' vertex-stage inputs/outputs are identical (position/uv/color/
// params passthrough plus the screen-size NDC projection), the same
// precedent `msdf.frag` already set by reusing `bindless_textured.vert`.
//
// `frag_params.x` is no longer a plain radius float here: it's a `u32`
// bump-allocated style-buffer WORD INDEX, carried NUMERICALLY (not
// bit-cast -- a bit-cast small integer is a subnormal float that real
// GPU hardware has been observed to flush to zero, see
// `tre_engine::style_index_param`'s own doc comment for the full
// account) in the float slot. Everything the uniform-radius `sdf_rounded_rect`
// shader hardcoded per-vertex (corner radius, and here also border/
// corner-smoothing) instead comes from a `GpuRectStyle` record read out
// of `style_buffer` at that word index -- `crates/tre-engine/src/
// gpu_style.rs`'s own doc comment is this shader's field-layout contract;
// the two must be kept in lockstep by hand.
#extension GL_EXT_nonuniform_qualifier : require

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 2) in vec3 frag_params;

layout(location = 0) out vec4 out_color;

layout(std430, set = 0, binding = 1) readonly buffer ShapeStyleBuffer {
    uint words[];
}
style_buffer;

vec3 srgb_to_linear(vec3 c) {
    bvec3 low = lessThanEqual(c, vec3(0.04045));
    vec3 lo = c / 12.92;
    vec3 hi = pow((c + 0.055) / 1.055, vec3(2.4));
    return mix(hi, lo, low);
}

// Matches `tre_engine::rgba8`'s little-endian packing: byte 0 (bits 0-7)
// is R, byte 3 (bits 24-31) is A.
vec4 unpack_rgba8(uint packed) {
    return vec4(
        float((packed >> 0) & 0xFFu),
        float((packed >> 8) & 0xFFu),
        float((packed >> 16) & 0xFFu),
        float((packed >> 24) & 0xFFu)
    ) / 255.0;
}

// `GpuRectStyle`'s corner-quadrant convention: `frag_uv` is the fragment's
// offset from the rect's own center, same as `sdf_rounded_rect.frag`;
// negative y is "up" (`RenderingCanvas::draw_rounded_rect`'s corners are
// built top-to-bottom in increasing-y screen space), so x<0,y<0 is the
// top-left quadrant.
float select_radius(vec2 p, vec4 radii) {
    if (p.x < 0.0 && p.y < 0.0) {
        return radii.x; // top_left
    }
    if (p.x >= 0.0 && p.y < 0.0) {
        return radii.y; // top_right
    }
    if (p.x >= 0.0 && p.y >= 0.0) {
        return radii.z; // bottom_right
    }
    return radii.w; // bottom_left
}

// A real superellipse ("squircle") blend: raising the corner falloff's
// norm from 2 (a true circular arc, IQ's exact rounded-box SDF) toward a
// higher exponent as `smoothing` -> 1 flattens the corner's curvature
// profile. This is a real, monotonic smoothing control, not a faked one
// -- but it is explicitly NOT a byte-for-byte match of any specific
// reference implementation's own squircle algorithm (e.g. Figma's), a
// disclosed simplification (documentation/PLAN.md's "Scope decisions").
float corner_norm(vec2 q, float smoothing) {
    float n = mix(2.0, 5.0, clamp(smoothing, 0.0, 1.0));
    return pow(pow(q.x, n) + pow(q.y, n), 1.0 / n);
}

// The exact (smoothing == 0) or superellipse-blended (smoothing > 0)
// signed distance to a non-uniform-corner rounded box -- the direct
// per-quadrant-radius generalization of `sdf_rounded_rect.frag`'s own
// box-SDF formula (Inigo Quilez).
float sd_rounded_box(vec2 p, vec2 half_extent, vec4 radii, float smoothing) {
    float r = select_radius(p, radii);
    vec2 q = abs(p) - half_extent + r;
    vec2 qm = max(q, vec2(0.0));
    float outer = (smoothing <= 0.0001) ? length(qm) : corner_norm(qm, smoothing);
    return outer + min(max(q.x, q.y), 0.0) - r;
}

// Phase 10 Step 10.2.1: evaluates a `GpuGradientStyle` record at
// `word_index` for the LOCAL point `p` -- the exact same field layout and
// math `gradient_fill.frag` uses for Polygon/Path (duplicated here, not
// shared, matching this file's own existing duplication of `srgb_to_
// linear`/`unpack_rgba8` -- this codebase's shaders have no include
// mechanism). Returns unpremultiplied linear RGB in `.rgb` and alpha in
// `.a`, matching `fill_linear`/`frag_color.a`'s own existing convention
// below so the border-blend math beneath this function needs no
// gradient-specific branch of its own.
const uint GRADIENT_MAX_STOPS = 8u;

vec4 eval_gradient(uint word_index, vec2 p) {
    uint kind = style_buffer.words[word_index + 0];
    vec2 point0 = vec2(
        uintBitsToFloat(style_buffer.words[word_index + 1]),
        uintBitsToFloat(style_buffer.words[word_index + 2])
    );
    vec2 point1_or_radius = vec2(
        uintBitsToFloat(style_buffer.words[word_index + 3]),
        uintBitsToFloat(style_buffer.words[word_index + 4])
    );
    uint stop_count = style_buffer.words[word_index + 5];

    float t;
    if (kind == 0u) {
        vec2 axis = point1_or_radius - point0;
        float len_sq = dot(axis, axis);
        t = len_sq > 0.0 ? dot(p - point0, axis) / len_sq : 0.0;
    } else {
        float radius = point1_or_radius.x;
        t = radius > 0.0 ? length(p - point0) / radius : 0.0;
    }
    t = clamp(t, 0.0, 1.0);

    uint positions_base = word_index + 6u;
    uint colors_base = word_index + 6u + GRADIENT_MAX_STOPS;

    if (stop_count <= 1u) {
        vec4 c = unpack_rgba8(style_buffer.words[colors_base + 0]);
        return vec4(srgb_to_linear(c.rgb), c.a);
    }
    float first_pos = uintBitsToFloat(style_buffer.words[positions_base + 0]);
    float last_pos = uintBitsToFloat(style_buffer.words[positions_base + stop_count - 1u]);
    if (t <= first_pos) {
        vec4 c = unpack_rgba8(style_buffer.words[colors_base + 0]);
        return vec4(srgb_to_linear(c.rgb), c.a);
    }
    if (t >= last_pos) {
        vec4 c = unpack_rgba8(style_buffer.words[colors_base + stop_count - 1u]);
        return vec4(srgb_to_linear(c.rgb), c.a);
    }
    uint lower = 0u;
    for (uint i = 0u; i < stop_count - 1u; i++) {
        float pos_i = uintBitsToFloat(style_buffer.words[positions_base + i]);
        float pos_next = uintBitsToFloat(style_buffer.words[positions_base + i + 1u]);
        if (t >= pos_i && t <= pos_next) {
            lower = i;
        }
    }
    float pos_lower = uintBitsToFloat(style_buffer.words[positions_base + lower]);
    float pos_upper = uintBitsToFloat(style_buffer.words[positions_base + lower + 1u]);
    float local_t = (pos_upper > pos_lower) ? (t - pos_lower) / (pos_upper - pos_lower) : 0.0;
    vec4 c0 = unpack_rgba8(style_buffer.words[colors_base + lower]);
    vec4 c1 = unpack_rgba8(style_buffer.words[colors_base + lower + 1u]);
    return vec4(mix(srgb_to_linear(c0.rgb), srgb_to_linear(c1.rgb), local_t), mix(c0.a, c1.a, local_t));
}

void main() {
    uint style_index = uint(frag_params.x);
    vec2 half_extent = frag_params.yz;

    vec4 radii = vec4(
        uintBitsToFloat(style_buffer.words[style_index + 0]),
        uintBitsToFloat(style_buffer.words[style_index + 1]),
        uintBitsToFloat(style_buffer.words[style_index + 2]),
        uintBitsToFloat(style_buffer.words[style_index + 3])
    );
    vec4 border_color = unpack_rgba8(style_buffer.words[style_index + 4]);
    float border_thickness = uintBitsToFloat(style_buffer.words[style_index + 5]);
    float corner_smoothing = uintBitsToFloat(style_buffer.words[style_index + 6]);
    uint fill_kind = style_buffer.words[style_index + 7];
    uint gradient_word_index = style_buffer.words[style_index + 8];

    float d = sd_rounded_box(frag_uv, half_extent, radii, corner_smoothing);
    float dd = fwidth(d);
    float outer_alpha = clamp(0.5 - d / dd, 0.0, 1.0);

    vec4 fill = (fill_kind == 1u)
        ? eval_gradient(gradient_word_index, frag_uv)
        : vec4(srgb_to_linear(frag_color.rgb), frag_color.a);
    vec3 fill_linear = fill.rgb;
    float fill_alpha = fill.a;
    vec3 rgb;
    float a;
    if (border_thickness > 0.0) {
        float inner_d = d + border_thickness;
        float inner_alpha = clamp(0.5 - inner_d / dd, 0.0, 1.0);
        vec3 border_linear = srgb_to_linear(border_color.rgb);
        rgb = mix(border_linear, fill_linear, inner_alpha);
        a = mix(border_color.a, fill_alpha, inner_alpha);
    } else {
        rgb = fill_linear;
        a = fill_alpha;
    }
    a *= outer_alpha;

    // ARCHITECTURE.md Section 6.1's blend state expects premultiplied
    // alpha.
    out_color = vec4(rgb * a, a);
}
