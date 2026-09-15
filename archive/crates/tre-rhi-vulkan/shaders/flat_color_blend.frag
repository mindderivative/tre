#version 450

// Phase 10 Step 10.2.3: pairs with the existing `walking_skeleton.vert`
// (this pipeline needs no new vertex-stage logic -- `PipelineKind::
// FlatColorBlend` still draws plain `UiVertex` triangles, only the
// fragment stage differs). `VK_EXT_blend_operation_advanced` (this
// step's originally-planned primary path) is not implemented by RADV,
// this project's own real dev GPU/driver -- see `documentation/
// REVIEW.md` for the full account. This shader instead implements the
// real, portable `VK_KHR_dynamic_rendering_local_read` alternative:
// reading the destination pixel a PRECEDING draw in this same frame
// already wrote via a real input attachment (`subpassLoad`), computing
// the blend formula itself, and writing the fully-composited result
// directly -- this pipeline's own `blend_enable(false)` means no
// fixed-function hardware blending happens afterward.
//
// Scope of this pass (disclosed, not accidental): opaque source AND
// destination only (both alphas assumed 1) -- with both alphas 1 the
// general W3C alpha-weighted compositing formula collapses to
// `Co = B(Cb, Cs)` directly, so no unpremultiply/premultiply step is
// needed here. A shape drawn under an active `Canvas` opacity
// (`state.alpha < 1`, e.g. inside a `save`/`restore` pair with `alpha`
// set) does NOT get that opacity correctly applied to a blend-mode
// fill in this pass -- full alpha-weighted compositing is disclosed
// follow-up work, not built speculatively here.
layout(input_attachment_index = 0, set = 1, binding = 0) uniform subpassInput dest_color;

layout(location = 0) in vec4 frag_color;
layout(location = 0) out vec4 out_color;

// `PipelineKind::FlatColorBlend`'s own push constant range is identical
// in size/offset to every other pipeline's (`vec2 screen_size, uint
// texture_index` -- 12 bytes total) -- only the THIRD field's meaning
// is repurposed here (`RenderingCanvas::draw_flat_polygon_blended`
// writes a `BlendMode as u32` into what every other pipeline treats as
// a bindless texture index), so the struct shape must still match
// exactly even though this pipeline never samples a bindless texture.
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint blend_mode;
} pc;

// TECHNICAL.md Section 6.2's canonical sRGB->Linear formula
// (IMPLEMENTATION.md Step 7.1, REVIEW.md finding #92) -- `frag_color`
// carries `UiVertex::color`'s own sRGB-authored value, same as
// `walking_skeleton.frag`.
vec3 srgb_to_linear(vec3 c) {
    bvec3 low = lessThanEqual(c, vec3(0.04045));
    vec3 lo = c / 12.92;
    vec3 hi = pow((c + 0.055) / 1.055, vec3(2.4));
    return mix(hi, lo, low);
}

// W3C Compositing and Blending Level 1's separable blend functions
// (https://www.w3.org/TR/compositing-1/#blending), evaluated per
// channel in linear space -- `Cb` is the backdrop (destination, read
// back via `subpassLoad`), `Cs` is the source (this draw's own color).
vec3 blend_multiply(vec3 cb, vec3 cs) {
    return cb * cs;
}

vec3 blend_screen(vec3 cb, vec3 cs) {
    return cb + cs - cb * cs;
}

vec3 blend_overlay(vec3 cb, vec3 cs) {
    // Overlay(Cb, Cs) = HardLight(Cs, Cb): the backdrop and source
    // swap roles relative to the hard-light formula below.
    bvec3 dark = lessThanEqual(cb, vec3(0.5));
    vec3 lo = 2.0 * cs * cb;
    vec3 hi = 1.0 - 2.0 * (1.0 - cs) * (1.0 - cb);
    return mix(hi, lo, dark);
}

vec3 blend_soft_light_d(vec3 cb) {
    bvec3 small = lessThanEqual(cb, vec3(0.25));
    vec3 lo = ((16.0 * cb - 12.0) * cb + 4.0) * cb;
    vec3 hi = sqrt(cb);
    return mix(hi, lo, small);
}

vec3 blend_soft_light(vec3 cb, vec3 cs) {
    bvec3 darken = lessThanEqual(cs, vec3(0.5));
    vec3 lo = cb - (1.0 - 2.0 * cs) * cb * (1.0 - cb);
    vec3 hi = cb + (2.0 * cs - 1.0) * (blend_soft_light_d(cb) - cb);
    return mix(hi, lo, darken);
}

vec3 blend_color_dodge(vec3 cb, vec3 cs) {
    bvec3 zero_backdrop = lessThanEqual(cb, vec3(0.0));
    bvec3 full_source = greaterThanEqual(cs, vec3(1.0));
    vec3 dodged = min(vec3(1.0), cb / (1.0 - cs));
    return mix(mix(dodged, vec3(1.0), full_source), vec3(0.0), zero_backdrop);
}

void main() {
    vec3 cs = srgb_to_linear(frag_color.rgb);
    // `subpassLoad` on this `_SRGB`-format attachment decodes back to
    // linear, exactly like `cs` above -- both operands are in the same
    // linear space the blend formulas below expect.
    vec3 cb = subpassLoad(dest_color).rgb;

    vec3 blended;
    // `BlendMode`'s real discriminants (`crates/tre-engine/src/
    // shapes.rs`): Normal = 0 never reaches this shader (`shapes.rs`'s
    // own dispatch routes it to the ordinary `FlatColor` pipeline
    // instead), Multiply = 1, Screen = 2, Overlay = 3, SoftLight = 4,
    // ColorDodge = 5.
    if (pc.blend_mode == 1u) {
        blended = blend_multiply(cb, cs);
    } else if (pc.blend_mode == 2u) {
        blended = blend_screen(cb, cs);
    } else if (pc.blend_mode == 3u) {
        blended = blend_overlay(cb, cs);
    } else if (pc.blend_mode == 4u) {
        blended = blend_soft_light(cb, cs);
    } else {
        blended = blend_color_dodge(cb, cs);
    }

    // Opaque-only scope (this shader's own doc comment above): alpha is
    // always 1, so the fully-composited premultiplied output is just
    // the blended color itself.
    out_color = vec4(blended, 1.0);
}
