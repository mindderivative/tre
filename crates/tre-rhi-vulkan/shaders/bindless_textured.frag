#version 450
#extension GL_EXT_nonuniform_qualifier : require

// IMPLEMENTATION.md Step 2.1: the unbounded `texture2D textures[]` array
// (SAMPLED_IMAGE, not COMBINED_IMAGE_SAMPLER -- a separate, single shared
// sampler at binding 0 instead) `VulkanDevice::new` builds the descriptor
// set layout for. The array is binding 1, not 0: Vulkan requires
// VARIABLE_DESCRIPTOR_COUNT to be on the highest-numbered binding in the
// set.
layout(set = 0, binding = 0) uniform sampler bindless_sampler;
layout(set = 0, binding = 2) uniform texture2D bindless_textures[];

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 0) out vec4 out_color;

layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index;
} pc;

// TECHNICAL.md Section 6.2's canonical sRGB->Linear formula
// (IMPLEMENTATION.md Step 7.1, REVIEW.md finding #92): only the
// no-texture-bound fallback below reads `frag_color` (`UiVertex::color`'s
// own sRGB-authored value) -- the textured branch samples an already-
// linear texture (the layer-composite's own `Rgba16Float` transient
// target, or an MSDF atlas that isn't color data at all), so it needs no
// conversion. Alpha carries no gamma curve and is left unconverted.
vec3 srgb_to_linear(vec3 c) {
    bvec3 low = lessThanEqual(c, vec3(0.04045));
    vec3 lo = c / 12.92;
    vec3 hi = pow((c + 0.055) / 1.055, vec3(2.4));
    return mix(hi, lo, low);
}

void main() {
    // `BINDLESS_TEXTURE_SENTINEL` (Rust side): no texture was bound for
    // this draw, so fall back to Phase 0's flat vertex color -- keeps every
    // pre-existing non-textured draw path working unchanged.
    if (pc.texture_index == 0xFFFFFFFFu) {
        out_color = vec4(srgb_to_linear(frag_color.rgb), frag_color.a);
    } else {
        out_color = texture(
            sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
            frag_uv
        );
    }
}
