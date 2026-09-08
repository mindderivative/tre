#version 450
#extension GL_EXT_nonuniform_qualifier : require

// IMPLEMENTATION.md Step 7.2.1 / TECHNICAL.md Section 5.5: the
// Dual-Kawase blur's upsample half -- an 8-tap ring filter around the
// source texel (no center sample), axis taps weight 1, diagonal taps
// weight 2, sum/12. Paired with the existing `bindless_textured.vert`
// unchanged, matching `kawase_downsample.frag`'s own reasoning.
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
    // Same reasoning as `kawase_downsample.frag`'s own comment, mirrored:
    // an upsample pass's real caller always sizes the source texture at
    // exactly half the destination's own dimensions, so `half_pixel =
    // 0.5 / source_dimensions = 0.5 / (0.5 * screen_size) = 1.0 /
    // screen_size`.
    vec2 half_pixel = 1.0 / pc.screen_size;

    // Fully re-inlined at every sample -- see `kawase_downsample.frag`'s
    // own comment for why (no local variable/function-parameter
    // indirection between `nonuniformEXT` and the array index it must
    // decorate).
    vec4 sum = texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv - vec2(2.0 * half_pixel.x, 0.0)
    );
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv - vec2(half_pixel.x, -half_pixel.y)
    ) * 2.0;
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv + vec2(0.0, 2.0 * half_pixel.y)
    );
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv + half_pixel
    ) * 2.0;
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv + vec2(2.0 * half_pixel.x, 0.0)
    );
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv + vec2(half_pixel.x, -half_pixel.y)
    ) * 2.0;
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv - vec2(0.0, 2.0 * half_pixel.y)
    );
    sum += texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv - half_pixel
    ) * 2.0;

    out_color = sum / 12.0;
}
