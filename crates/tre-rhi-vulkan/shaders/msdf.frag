#version 450
#extension GL_EXT_nonuniform_qualifier : require

// IMPLEMENTATION.md Step 4.2.3: the real MSDF evaluation shader --
// TECHNICAL.md Section 5.3's exact canonical formula, not re-derived
// here. Samples the same bindless texture array Step 2.1's
// `bindless_textured.frag` already established (this shader is paired
// with that same file's `bindless_textured.vert` unchanged; nothing
// about the vertex stage differs).
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
    vec3 msdf = texture(
        sampler2D(bindless_textures[nonuniformEXT(pc.texture_index)], bindless_sampler),
        frag_uv
    ).rgb;

    // TECHNICAL.md Section 5.3's canonical formula: the median of the
    // three channels rejects a single channel's own local corner
    // distortion (the entire point of encoding three independently
    // colored edge distances instead of one), and the `fwidth`-based
    // ramp is exactly one screen-space pixel wide regardless of how much
    // the source texture is magnified on screen -- true resolution
    // independence, not a fixed-size blur.
    float sig_dist = max(min(msdf.r, msdf.g), min(max(msdf.r, msdf.g), msdf.b)) - 0.5;
    float opacity = clamp(sig_dist / fwidth(sig_dist) + 0.5, 0.0, 1.0);

    // ARCHITECTURE.md Section 6.1's blend state expects premultiplied
    // alpha -- both color and alpha channels scaled by the same computed
    // coverage, matching `sdf_rounded_rect.frag`'s own closing line.
    out_color = vec4(frag_color.rgb * opacity, frag_color.a * opacity);
}
