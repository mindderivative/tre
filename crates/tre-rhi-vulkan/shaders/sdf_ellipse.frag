#version 450

// Phase 10 Step 10.2: the Circle/Ellipse SDF shader. Paired at
// pipeline-creation time with the existing `sdf_rounded_rect.vert` (a
// plain screen-size NDC projection plus position/uv/color/params
// passthrough -- nothing about it is rectangle-specific), the same
// shared-vertex-shader precedent `msdf.frag`/`sdf_rect_styled.frag`
// already use.
//
// Vertex convention (`shapes::flatten_ellipse`, `tre-engine`):
// `frag_uv` is the fragment's offset from the ellipse's own center, in
// local (untransformed) pixel units; `frag_params` is `[style_index,
// radius_x, radius_y]` -- `style_index` is a `u32` bump-allocated
// `GpuEllipseStyle` word index (`crates/tre-engine/src/gpu_style.rs`),
// carried NUMERICALLY (not bit-cast) into the float slot by
// `tre_engine::style_index_param` -- see that function's own doc
// comment for the real denormal-flush-to-zero GPU bug a bit-cast
// encoding hit.
#extension GL_EXT_nonuniform_qualifier : require

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 2) in vec3 frag_params;

layout(location = 0) out vec4 out_color;

layout(std430, set = 0, binding = 1) readonly buffer ShapeStyleBuffer {
    uint words[];
}
style_buffer;

const float TAU = 6.28318530718;

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

// A standard scaled-circle ellipse SDF approximation (NOT the exact
// closed-form quartic solve): exact when `r.x == r.y` (reduces
// algebraically to the plain circle SDF `length(p) - r`), a real if
// approximate distance field otherwise. Deliberately chosen over the
// exact closed-form for this pass -- simpler to verify correct than
// hand-transcribing a quartic root solve from memory, at the cost of
// being an approximation for a true (non-circular) ellipse
// (documentation/PLAN.md's "Scope decisions").
float sd_ellipse(vec2 p, vec2 r) {
    float k1 = length(p / r);
    float k2 = length(p / (r * r));
    return k1 * (k1 - 1.0) / k2;
}

// Phase 10 Step 10.2.1: evaluates a `GpuGradientStyle` record at
// `word_index` for the LOCAL point `p` -- see `sdf_rect_styled.frag`'s
// own identical function for the full account (duplicated here, not
// shared: this codebase's shaders have no include mechanism). Returns
// unpremultiplied linear RGB in `.rgb` and alpha in `.a`.
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
    vec2 radius = frag_params.yz;

    vec4 border_color = unpack_rgba8(style_buffer.words[style_index + 0]);
    float border_thickness = uintBitsToFloat(style_buffer.words[style_index + 1]);
    float arc_start_angle = uintBitsToFloat(style_buffer.words[style_index + 2]);
    float arc_sweep_angle = uintBitsToFloat(style_buffer.words[style_index + 3]);
    uint fill_kind = style_buffer.words[style_index + 4];
    uint gradient_word_index = style_buffer.words[style_index + 5];

    float d = sd_ellipse(frag_uv, radius);

    // A hard-edged angular sector cutoff for a partial arc/pie shape --
    // no rounded stroke caps at the cut edges this pass (disclosed,
    // documentation/PLAN.md's "Scope decisions"). Skipped entirely for a
    // full sweep so a complete ellipse never risks an off-by-epsilon
    // seam at the wraparound boundary.
    if (arc_sweep_angle < TAU - 0.0001) {
        float angle = atan(frag_uv.y, frag_uv.x);
        if (angle < 0.0) {
            angle += TAU;
        }
        float start = mod(arc_start_angle, TAU);
        float relative = angle - start;
        if (relative < 0.0) {
            relative += TAU;
        }
        if (relative > arc_sweep_angle) {
            d = max(d, 0.001);
        }
    }

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

    out_color = vec4(rgb * a, a);
}
