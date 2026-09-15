#version 450

layout(location = 0) in vec2 in_position;
layout(location = 1) in vec2 in_uv;
layout(location = 2) in vec4 in_color;

layout(location = 0) out vec4 frag_color;

// Screen-space pixel coordinates -> NDC. No projection/transform stack yet
// (IMPLEMENTATION.md Phase 3's Vector Math Engine); the push constant here
// is the Phase 0 placeholder for that.
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
} pc;

void main() {
    vec2 ndc = (in_position / pc.screen_size) * 2.0 - 1.0;
    gl_Position = vec4(ndc, 0.0, 1.0);
    frag_color = in_color;
}
