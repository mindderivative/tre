#version 450

// Phase 0 placeholder: flat per-vertex color, not the real analytical SDF
// rounded-rect evaluation (TECHNICAL.md Section 5.2), which is
// IMPLEMENTATION.md Phase 3.2's job.
layout(location = 0) in vec4 frag_color;
layout(location = 0) out vec4 out_color;

void main() {
    out_color = frag_color;
}
