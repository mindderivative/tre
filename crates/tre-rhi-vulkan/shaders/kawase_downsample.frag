#version 450
#extension GL_EXT_nonuniform_qualifier : require

// IMPLEMENTATION.md Step 7.2.1 / TECHNICAL.md Section 5.5: the
// Dual-Kawase blur's downsample half -- a 5-tap filter (center weight 4,
// four diagonal taps weight 1 each, sum/8), paired with the existing
// `bindless_textured.vert` unchanged (its outputs already match what a
// full-target textured quad needs).
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
    // `create_pipeline`'s shared push-constant layout has no room for a
    // dedicated `half_pixel` field (its 12-byte range is fixed across
    // every pipeline) -- derived instead from `screen_size` (this pass's
    // own destination dimensions), since a downsample pass's real caller
    // (`dual_kawase_blur_demo.rs`) always sizes the source texture at
    // exactly 2x the destination's own dimensions: `half_pixel = 0.5 /
    // source_dimensions = 0.5 / (2 * screen_size) = 0.25 / screen_size`.
    vec2 half_pixel = 0.25 / pc.screen_size;

    // The bindless lookup is fully re-inlined at every sample (no local
    // variable for the index, no function boundary for the texture) --
    // matching `bindless_textured.frag`/`msdf.frag`'s own established
    // pattern exactly. `nonuniformEXT` must decorate the array-index
    // expression at its actual point of use for SPIR-V to preserve it
    // correctly; routing it through an intermediate local `uint` or a
    // function parameter is not guaranteed to survive `glslc`'s own
    // decoration propagation the same way.
    vec4 sum = texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv
    ) * 4.0;
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv - half_pixel
    );
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv + half_pixel
    );
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv + vec2(half_pixel.x, -half_pixel.y)
    );
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv - vec2(half_pixel.x, -half_pixel.y)
    );
    out_color = sum / 8.0;
}
