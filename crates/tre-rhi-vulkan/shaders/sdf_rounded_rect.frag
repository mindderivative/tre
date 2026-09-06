#version 450

// IMPLEMENTATION.md Step 3.2 / TECHNICAL.md Section 5.2: a real analytical
// SDF rounded rectangle, anti-aliased via screen-space derivatives -- the
// first shader in this project to output a genuine fractional alpha rather
// than always-opaque flat color.
//
// `frag_uv` is each fragment's position relative to the rect's center, in
// pixel units (RenderingCanvas::draw_rounded_rect's convention); `frag_params`
// is (radius, half_width, half_height). This is the standard box-SDF
// formula (Inigo Quilez): q = abs(p) - b + r, where b = (half_width,
// half_height) is the box's own half-extent (not shrunk by r) and r is the
// corner radius subtracted back out of the box-plus-radius Minkowski sum.
layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 2) in vec3 frag_params;

layout(location = 0) out vec4 out_color;

void main() {
    float radius = frag_params.x;
    vec2 half_extent = frag_params.yz;

    vec2 q = abs(frag_uv) - half_extent + radius;
    float d = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;

    // TECHNICAL.md Section 5.2's fwidth-based AA formula: d < 0 is inside
    // the shape, and the transition band is exactly one screen-space pixel
    // wide (fwidth(d) approximates |d(p+dx)-d(p)| across a pixel).
    float alpha = clamp(0.5 - d / fwidth(d), 0.0, 1.0);

    // ARCHITECTURE.md Section 6.1's blend state expects premultiplied
    // alpha -- both color and alpha channels scaled by the same computed
    // coverage, not just the alpha channel.
    out_color = vec4(frag_color.rgb * alpha, frag_color.a * alpha);
}
