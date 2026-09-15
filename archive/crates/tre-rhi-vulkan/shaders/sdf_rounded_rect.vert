#version 450

layout(location = 0) in vec2 in_position;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;
layout(location = 3) in vec3 in_params;

layout(location = 0) out vec4 frag_color;
layout(location = 1) out vec2 frag_uv;
layout(location = 2) out vec3 frag_params;

// Same screen-space pixel coordinates -> NDC placeholder as
// walking_skeleton.vert (IMPLEMENTATION.md Phase 3's Vector Math Engine
// owns the real projection/transform stack).
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
} pc;

void main() {
    vec2 ndc = (in_position / pc.screen_size) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    frag_color = in_color;
    frag_uv = in_uv;
    frag_params = in_params;
}
