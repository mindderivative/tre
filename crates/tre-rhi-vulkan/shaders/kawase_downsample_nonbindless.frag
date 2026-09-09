#version 450

// Experimental diagnostic (REVIEW.md finding #130, second research
// session): byte-for-byte the same 5-tap Dual-Kawase downsample math as
// kawase_downsample.frag, but sampling through a single, plain,
// conventionally-bound `sampler2D` at set 0 binding 0 -- NOT the
// bindless texture array -- to test whether routing a same-frame
// offscreen render target through the persistent bindless array is
// itself what triggers the all-zero read, as opposed to a plain,
// dedicated binding most real-time blur implementations (game engines'
// own post-process chains) actually use for exactly this kind of
// transient, single-frame resource.
layout(set = 0, binding = 0) uniform sampler2D source_texture;

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 0) out vec4 out_color;

// Kept byte-identical to kawase_downsample.frag's own push-constant
// block (including the now-unused `texture_index` field) purely so this
// experiment's pipeline layout can declare the exact same 12-byte
// VERTEX|FRAGMENT push-constant range `bindless_textured.vert` (reused
// unchanged) was already compiled against -- not because this shader
// itself needs `texture_index`.
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index;
} pc;

void main() {
    vec2 half_pixel = 0.25 / pc.screen_size;

    vec4 sum = texture(source_texture, frag_uv) * 4.0;
    sum += texture(source_texture, frag_uv - half_pixel);
    sum += texture(source_texture, frag_uv + half_pixel);
    sum += texture(source_texture, frag_uv + vec2(half_pixel.x, -half_pixel.y));
    sum += texture(source_texture, frag_uv - vec2(half_pixel.x, -half_pixel.y));
    out_color = sum / 8.0;
}
