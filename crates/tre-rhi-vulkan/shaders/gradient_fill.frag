#version 450
#extension GL_EXT_nonuniform_qualifier : require

// Phase 10 Step 10.2.1: Polygon/Path gradient fill. Paired at
// pipeline-creation time with the EXISTING `bindless_textured.vert` (not
// a new vertex shader) -- that shader already outputs `frag_uv` (this
// pipeline's own repurposing of it: `RenderingCanvas::draw_gradient_
// polygon` writes each vertex's own LOCAL, pre-transform position into
// `UiVertex::uv`, not a real texture coordinate) and already declares a
// `texture_index` push constant (here repurposed as a `GpuGradientStyle`
// word index, not a bindless texture array index) -- the same
// shared-vertex-shader precedent `msdf.frag`/`sdf_rect_styled.frag`
// already established for reusing an existing vertex stage whose real
// inputs/outputs already match what a new fragment shader needs.
//
// Reads the SAME per-shape style storage buffer (binding 1)
// `sdf_rect_styled.frag`/`sdf_ellipse.frag` already read from -- a
// `GpuGradientStyle` record is just another word-indexed record in that
// same buffer (`crates/tre-engine/src/gpu_style.rs`'s own doc comment has
// the full field-layout contract this shader's word offsets below must
// stay in lockstep with by hand).

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;

layout(location = 0) out vec4 out_color;

layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index; // reinterpreted here: a GpuGradientStyle word index
} pc;

layout(std430, set = 0, binding = 1) readonly buffer ShapeStyleBuffer {
    uint words[];
}
style_buffer;

// Matches `GRADIENT_MAX_STOPS` (`crates/tre-engine/src/gpu_style.rs`) --
// kept in lockstep by hand, like every other style-buffer field offset in
// this codebase's shaders.
const uint GRADIENT_MAX_STOPS = 8u;

vec3 srgb_to_linear(vec3 c) {
    bvec3 low = lessThanEqual(c, vec3(0.04045));
    vec3 lo = c / 12.92;
    vec3 hi = pow((c + 0.055) / 1.055, vec3(2.4));
    return mix(hi, lo, low);
}

// Matches `tre_engine::rgba8`'s little-endian packing (see
// `sdf_rect_styled.frag`'s identical helper).
vec4 unpack_rgba8(uint packed) {
    return vec4(
        float((packed >> 0) & 0xFFu),
        float((packed >> 8) & 0xFFu),
        float((packed >> 16) & 0xFFu),
        float((packed >> 24) & 0xFFu)
    ) / 255.0;
}

void main() {
    uint base = pc.texture_index;
    uint kind = style_buffer.words[base + 0];
    vec2 point0 = vec2(
        uintBitsToFloat(style_buffer.words[base + 1]),
        uintBitsToFloat(style_buffer.words[base + 2])
    );
    vec2 point1_or_radius = vec2(
        uintBitsToFloat(style_buffer.words[base + 3]),
        uintBitsToFloat(style_buffer.words[base + 4])
    );
    uint stop_count = style_buffer.words[base + 5];

    // GradientKind::Linear (kind == 0): project the local point onto the
    // start->end axis. GradientKind::Radial (kind == 1): normalize the
    // local point's own distance from center by radius. Both clamped to
    // [0, 1] -- past either end of the gradient repeats its own last
    // real stop's color (no wrap/mirror this pass, matching this
    // project's own disclosed "linear and radial only" scope decision).
    float t;
    if (kind == 0u) {
        vec2 axis = point1_or_radius - point0;
        float len_sq = dot(axis, axis);
        t = len_sq > 0.0 ? dot(frag_uv - point0, axis) / len_sq : 0.0;
    } else {
        float radius = point1_or_radius.x;
        t = radius > 0.0 ? length(frag_uv - point0) / radius : 0.0;
    }
    t = clamp(t, 0.0, 1.0);

    uint positions_base = base + 6u;
    uint colors_base = base + 6u + GRADIENT_MAX_STOPS;

    vec3 color_linear;
    float alpha;
    if (stop_count <= 1u) {
        vec4 c = unpack_rgba8(style_buffer.words[colors_base + 0]);
        color_linear = srgb_to_linear(c.rgb);
        alpha = c.a;
    } else {
        float first_pos = uintBitsToFloat(style_buffer.words[positions_base + 0]);
        float last_pos = uintBitsToFloat(style_buffer.words[positions_base + stop_count - 1u]);
        if (t <= first_pos) {
            vec4 c = unpack_rgba8(style_buffer.words[colors_base + 0]);
            color_linear = srgb_to_linear(c.rgb);
            alpha = c.a;
        } else if (t >= last_pos) {
            vec4 c = unpack_rgba8(style_buffer.words[colors_base + stop_count - 1u]);
            color_linear = srgb_to_linear(c.rgb);
            alpha = c.a;
        } else {
            // Find the bracketing stop pair -- stop_count is at most
            // GRADIENT_MAX_STOPS (8), so a dynamic-bound loop here costs
            // nothing real.
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
            float local_t =
                (pos_upper > pos_lower) ? (t - pos_lower) / (pos_upper - pos_lower) : 0.0;
            vec4 c0 = unpack_rgba8(style_buffer.words[colors_base + lower]);
            vec4 c1 = unpack_rgba8(style_buffer.words[colors_base + lower + 1u]);
            // Interpolating in LINEAR space (documentation/ARCHITECTURE.md
            // Section 6.1's own established blending discipline), not
            // sRGB space, avoids the well-known "muddy midpoint" artifact.
            color_linear = mix(srgb_to_linear(c0.rgb), srgb_to_linear(c1.rgb), local_t);
            alpha = mix(c0.a, c1.a, local_t);
        }
    }

    // Premultiplied alpha, matching ARCHITECTURE.md Section 6.1's blend
    // state and REVIEW.md finding #164's own fix to this pipeline's
    // solid-fill sibling shader.
    out_color = vec4(color_linear * alpha, alpha);
}
