#version 450

// Experimental diagnostic (REVIEW.md finding #130, second research
// session): a plain textured-quad passthrough, sampling through a
// single, conventionally-bound `sampler2D` at set 0 binding 0 -- used
// to composite the non-bindless downsample experiment's own result back
// onto the swapchain without routing it back through the bindless array
// (which would reintroduce the exact variable this experiment exists to
// eliminate).
layout(set = 0, binding = 0) uniform sampler2D source_texture;

layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 0) out vec4 out_color;

void main() {
    out_color = texture(source_texture, frag_uv);
}
