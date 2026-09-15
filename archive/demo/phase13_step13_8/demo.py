#!/usr/bin/env python3
"""Phase 13 Step 13.8 proof: a real, user-facing custom shader API
(Q13) -- `renderer.create_custom_shader(fragment_glsl_source)` compiles
REAL GLSL to SPIR-V at runtime via `shaderc` (the identical real library
`build.rs` already wraps through `glslc` for this project's own
compile-time shaders), then registers the result as a real Vulkan
pipeline a `tre.CustomShaded` shape can render through.

No new RHI-level plumbing was needed to make this render: `execute_frame`
already resolves any `pipeline_state_id` generically via
`PipelineRegistry::get`, confirmed by reading its own source before
building this -- the only real gap was compiling user-supplied GLSL at
runtime and reaching that already-generic dispatch path from a real
`ShapePrimitive`.
"""

import tre_python as tre

WIDTH, HEIGHT = 100, 100

# Real, disclosed v1 interface contract (see VulkanDevice::
# create_custom_pipeline's own doc comment): a custom fragment shader is
# paired with the SAME real vertex shader TexturedQuad/GradientFill/
# MsdfText already use, so it must declare this exact interface.
UV_GRADIENT_SHADER = """
#version 450
layout(location = 0) in vec4 frag_color;
layout(location = 1) in vec2 frag_uv;
layout(location = 0) out vec4 out_color;
layout(push_constant) uniform PushConstants {
    vec2 screen_size;
    uint texture_index;
} pc;
void main() {
    // A real per-pixel computation using the shape's own UV coordinates
    // -- proof this is genuine shader execution, not a flat fill.
    out_color = vec4(frag_uv.x, frag_uv.y, 0.0, 1.0);
}
"""

BROKEN_SHADER = """
#version 450
void main() {
    this is not valid GLSL;
}
"""


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    # BGRA8 readback.
    offset = (y * WIDTH + x) * 4
    b, g, r, a = buf[offset : offset + 4]
    return (r, g, b, a)


def check_real_per_pixel_shader_execution(renderer: "tre.HeadlessRenderer") -> None:
    shader_id = renderer.create_custom_shader(UV_GRADIENT_SHADER)
    print(f"compiled a real GLSL fragment shader at runtime -> {shader_id} -- OK")

    custom = tre.CustomShaded(10.0, 10.0, 80.0, 80.0, shader_id, tre.rgba8(255, 255, 255, 255))
    registry = tre.ShapeRegistry()
    registry.insert_custom_shaded(custom)
    frame = renderer.render(registry)

    top_left = pixel_at(frame, 15, 15)  # low uv.x, low uv.y
    top_right = pixel_at(frame, 85, 15)  # high uv.x, low uv.y
    bottom_left = pixel_at(frame, 15, 85)  # low uv.x, high uv.y

    # The shader hardcodes blue=0.0 everywhere -- a real, checkable
    # invariant proving the shader's own logic actually ran (not some
    # unrelated fallback path).
    assert top_left[2] == 0 and top_right[2] == 0 and bottom_left[2] == 0, (
        "the shader hardcodes out_color.b = 0.0 -- every sampled pixel must have blue=0"
    )
    # Red channel must increase left-to-right (frag_uv.x increases).
    assert top_right[0] > top_left[0], (
        f"red channel must increase with uv.x (left->right), got left={top_left}, right={top_right}"
    )
    # Green channel must increase top-to-bottom (frag_uv.y increases).
    assert bottom_left[1] > top_left[1], (
        f"green channel must increase with uv.y (top->bottom), got top={top_left}, bottom={bottom_left}"
    )
    print(
        f"real per-pixel UV gradient confirmed: top_left={top_left}, top_right={top_right}, "
        f"bottom_left={bottom_left} -- OK"
    )


def check_a_broken_shader_raises_a_real_diagnostic(renderer: "tre.HeadlessRenderer") -> None:
    try:
        renderer.create_custom_shader(BROKEN_SHADER)
        raise AssertionError("invalid GLSL should have raised ValueError")
    except ValueError as e:
        message = str(e)
        assert message, "the error must carry shaderc's own real compiler diagnostic"
        print(
            f"broken GLSL correctly rejected with a real compiler diagnostic: {message[:80]}... -- OK"
        )


def check_custom_shaded_composes_with_other_shapes_in_one_registry(renderer: "tre.HeadlessRenderer") -> None:
    shader_id = renderer.create_custom_shader(UV_GRADIENT_SHADER)

    registry = tre.ShapeRegistry()
    registry.insert_rectangle(tre.Rectangle(0.0, 0.0, 20.0, 20.0, tre.rgba8(0, 0, 0, 255)))
    registry.insert_custom_shaded(
        tre.CustomShaded(50.0, 50.0, 40.0, 40.0, shader_id, tre.rgba8(255, 255, 255, 255))
    )
    frame = renderer.render(registry)

    rect_pixel = pixel_at(frame, 10, 10)
    custom_pixel = pixel_at(frame, 70, 70)
    assert rect_pixel == (0, 0, 0, 255), "a plain Rectangle in the same registry must still render normally"
    assert custom_pixel != (0, 0, 0, 255) and custom_pixel[3] == 255, (
        "the custom-shaded quad must render its own real shader output, not the rectangle's flat color"
    )
    print(
        f"CustomShaded composes with a plain Rectangle in one registry: rect={rect_pixel}, "
        f"custom={custom_pixel} -- OK"
    )


def main() -> None:
    # HeadlessRenderer opens a real winit EventLoop, and winit permits at
    # most one per process (a real constraint discovered while building
    # Step 12.9's own demo) -- one renderer is created once and reused.
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    check_real_per_pixel_shader_execution(renderer)
    check_a_broken_shader_raises_a_real_diagnostic(renderer)
    check_custom_shaded_composes_with_other_shapes_in_one_registry(renderer)
    print("tre_python custom shader API (Phase 13 Step 13.8) demo: PASSED")


if __name__ == "__main__":
    main()
