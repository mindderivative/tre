#version 450

layout(location = 0) in vec2 in_position;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;

layout(location = 0) out vec4 frag_color;
layout(location = 1) out vec2 frag_uv;

// IMPLEMENTATION.md Step 7.2.2: unlike bindless_textured.vert,
// `in_position` here is already authored directly in NDC space (-1..1)
// -- `apply_layer_blur`'s own cached unit quad, reused unchanged for
// every downsample/upsample hop of every call regardless of the caller's
// real width/height, so no per-call vertex-buffer upload is ever needed
// (this crate's own "no dynamic RHI allocation inside the render tick"
// discipline, DESIGN.md Section 2.6). No push-constant division is
// needed for position at all. Declares the identical 12-byte push-
// constant block (unused by this stage) purely so this pipeline's
// layout stays compatible with `kawase_downsample_nonbindless.frag`/
// `kawase_upsample_nonbindless.frag`'s own real use of `screen_size` for
// their `half_pixel` derivation.
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index;
} pc;

void main() {
    gl_Position = vec4(in_position, 0.0, 1.0);
    frag_color = in_color;
    frag_uv = in_uv;
}
