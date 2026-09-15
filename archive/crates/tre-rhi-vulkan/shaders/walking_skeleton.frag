#version 450

// Phase 0 placeholder: flat per-vertex color, not the real analytical SDF
// rounded-rect evaluation (TECHNICAL.md Section 5.2), which is
// IMPLEMENTATION.md Phase 3.2's job.
layout(location = 0) in vec4 frag_color;
layout(location = 0) out vec4 out_color;

// TECHNICAL.md Section 6.2's canonical sRGB->Linear formula
// (IMPLEMENTATION.md Step 7.1, REVIEW.md finding #92): `frag_color.rgb`
// is `UiVertex::color`'s own sRGB-authored value -- must be linearized
// before the premultiplied-alpha math below, so the swapchain's
// `_SRGB` attachment (which auto-encodes back to sRGB on store) isn't
// fed an already-encoded value a second time. Alpha carries no gamma
// curve and is left unconverted.
vec3 srgb_to_linear(vec3 c) {
    bvec3 low = lessThanEqual(c, vec3(0.04045));
    vec3 lo = c / 12.92;
    vec3 hi = pow((c + 0.055) / 1.055, vec3(2.4));
    return mix(hi, lo, low);
}

void main() {
    // ARCHITECTURE.md Section 6.1's blend state expects premultiplied
    // alpha (REVIEW.md finding #164) -- matches every other real
    // fragment shader in this codebase.
    vec3 linear_color = srgb_to_linear(frag_color.rgb);
    out_color = vec4(linear_color * frag_color.a, frag_color.a);
}
