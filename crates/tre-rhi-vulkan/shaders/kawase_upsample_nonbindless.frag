#version 450

// REVIEW.md finding #130, real fix: byte-for-byte the same 8-tap
// Dual-Kawase upsample math as kawase_upsample.frag, but sampling
// through a single, plain, conventionally-bound `sampler2D` at set 0
// binding 0 -- the same non-bindless approach
// `kawase_downsample_nonbindless.frag`/`dual_kawase_nonbindless_
// experiment.rs` already proved correct.
layout(set = 0, binding = 0) uniform sampler2D source_texture;

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 0) out vec4 out_color;

// Kept byte-identical to kawase_upsample.frag's own push-constant block
// (including the now-unused `texture_index` field) so this pipeline's
// layout stays compatible with `bindless_textured.vert`, reused
// unchanged.
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index;
} pc;

void main() {
    vec2 half_pixel = 1.0 / pc.screen_size;

    vec4 sum = texture(source_texture, frag_uv - vec2(2.0 * half_pixel.x, 0.0));
    sum += texture(source_texture, frag_uv - vec2(half_pixel.x, -half_pixel.y)) * 2.0;
    sum += texture(source_texture, frag_uv + vec2(0.0, 2.0 * half_pixel.y));
    sum += texture(source_texture, frag_uv + half_pixel) * 2.0;
    sum += texture(source_texture, frag_uv + vec2(2.0 * half_pixel.x, 0.0));
    sum += texture(source_texture, frag_uv + vec2(half_pixel.x, -half_pixel.y)) * 2.0;
    sum += texture(source_texture, frag_uv - vec2(0.0, 2.0 * half_pixel.y));
    sum += texture(source_texture, frag_uv - half_pixel) * 2.0;

    out_color = sum / 12.0;
}
