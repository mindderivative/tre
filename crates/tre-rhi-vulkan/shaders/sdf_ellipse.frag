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

void main() {
    uint style_index = uint(frag_params.x);
    vec2 radius = frag_params.yz;

    vec4 border_color = unpack_rgba8(style_buffer.words[style_index + 0]);
    float border_thickness = uintBitsToFloat(style_buffer.words[style_index + 1]);
    float arc_start_angle = uintBitsToFloat(style_buffer.words[style_index + 2]);
    float arc_sweep_angle = uintBitsToFloat(style_buffer.words[style_index + 3]);

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

    out_color = vec4(rgb * a, a);
}
