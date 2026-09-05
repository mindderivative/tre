#version 450

layout(location = 0) in vec2 in_position;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;

layout(location = 0) out vec4 frag_color;
layout(location = 1) out vec2 frag_uv;

// Same screen-space -> NDC placeholder as walking_skeleton.vert
// (IMPLEMENTATION.md Phase 3's transform stack doesn't exist yet), extended
// with `texture_index` (IMPLEMENTATION.md Step 2.1's bindless array
// selection) -- unused here, but its presence keeps this block's layout
// identical to the fragment shader's, matching the single 12-byte push
// constant range `create_pipeline` declares for both stages.
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index;
} pc;

void main() {
    vec2 ndc = (in_position / pc.screen_size) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    frag_color = in_color;
    frag_uv = in_uv;
}
