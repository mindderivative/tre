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

    float d = sd_rounded_box(frag_uv, half_extent, radii, corner_smoothing);
    float dd = fwidth(d);
    float outer_alpha = clamp(0.5 - d / dd, 0.0, 1.0);

    vec3 fill_linear = srgb_to_linear(frag_color.rgb);
    vec3 rgb;
    float a;
    if (border_thickness > 0.0) {
        float inner_d = d + border_thickness;
        float inner_alpha = clamp(0.5 - inner_d / dd, 0.0, 1.0);
        vec3 border_linear = srgb_to_linear(border_color.rgb);
        rgb = mix(border_linear, fill_linear, inner_alpha);
        a = mix(border_color.a, frag_color.a, inner_alpha);
    } else {
        rgb = fill_linear;
        a = frag_color.a;
    }
    a *= outer_alpha;

    // ARCHITECTURE.md Section 6.1's blend state expects premultiplied
    // alpha.
    out_color = vec4(rgb * a, a);
}
