#version 450
#extension GL_EXT_nonuniform_qualifier : require

// IMPLEMENTATION.md Step 2.1: the unbounded `texture2D textures[]` array
// (SAMPLED_IMAGE, not COMBINED_IMAGE_SAMPLER -- a separate, single shared
// sampler at binding 0 instead) `VulkanDevice::new` builds the descriptor
// set layout for. The array is binding 1, not 0: Vulkan requires
// VARIABLE_DESCRIPTOR_COUNT to be on the highest-numbered binding in the
// set.
layout(set = 0, binding = 0) uniform sampler bindless_sampler;
layout(set = 0, binding = 1) uniform texture2D bindless_textures[];

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 0) out vec4 out_color;

layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index;
} pc;

void main() {
    // `BINDLESS_TEXTURE_SENTINEL` (Rust side): no texture was bound for
    // this draw, so fall back to Phase 0's flat vertex color -- keeps every
    // pre-existing non-textured draw path working unchanged.
    if (pc.texture_index == 0xFFFFFFFFu) {
        out_color = frag_color;
    } else {
        out_color = texture(
            sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
            frag_uv
        );
    }
}
