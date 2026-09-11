#!/usr/bin/env python3
"""Phase 12 Step 12.4 proof: `fill_color` as a real `int | GradientId |
Texture` union, end to end through `tre_python`. One scene mixes all
three fill kinds:

- A plain-int solid fill (regression check: existing code must keep
  working unchanged).
- A linear-gradient-filled rectangle (`registry.create_gradient(...)`).
- A texture-filled circle (`renderer.create_texture(...)`, a real 2x2
  checkerboard).
"""

import tre_python as tre

WIDTH, HEIGHT = 300, 200

RED = tre.rgba8(255, 0, 0, 255)
BLUE = tre.rgba8(0, 0, 255, 255)
GREEN = tre.rgba8(0, 255, 0, 255)


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def main() -> None:
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    registry = tre.ShapeRegistry()

    # --- Regression: plain int fill_color must still work unchanged. ---
    solid = tre.Rectangle(10, 10, 40, 40, GREEN)
    registry.insert_rectangle(solid)
    solid_center = (10 + 20, 10 + 20)

    # --- Gradient fill: real create_gradient() -> real GradientId. ---
    gradient = tre.Gradient.linear((0.0, 0.0), (100.0, 0.0), [(0.0, RED), (1.0, BLUE)])
    gradient_id = registry.create_gradient(gradient)
    grad_rect = tre.Rectangle(80, 10, 100, 60, gradient_id)
    registry.insert_rectangle(grad_rect)
    grad_left = (80 + 5, 10 + 30)
    grad_right = (80 + 95, 10 + 30)

    # --- Texture fill: real renderer.create_texture() -> real Texture. ---
    # A tiny 4x4 solid-magenta texture -- simple enough to assert an
    # exact expected color, the same "pure, exact-roundtrip color"
    # precedent phase10_step10_4's own demo already established.
    pixels = bytes([255, 0, 255, 255]) * (4 * 4)
    texture = renderer.create_texture(4, 4, tre.TextureFormat.Rgba8Unorm, pixels)
    tex_circle = tre.Circle(210, 30, 40, texture)
    registry.insert_circle(tex_circle)
    tex_center = (210 + 40, 30 + 40)

    assert len(registry) == 3

    frame = renderer.render(registry)
    background = pixel_at(frame, 0, 0)

    solid_px = pixel_at(frame, *solid_center)
    assert solid_px != background, "solid-fill rectangle must not read back as background"
    print(f"solid fill_color (unchanged, regression): {solid_px} -- OK")

    grad_left_px = pixel_at(frame, *grad_left)
    grad_right_px = pixel_at(frame, *grad_right)
    assert grad_left_px != background
    assert grad_right_px != background
    assert grad_left_px != grad_right_px, (
        f"a gradient fill must vary across the shape, got the same color {grad_left_px} at both ends"
    )
    print(f"gradient fill: left={grad_left_px}, right={grad_right_px} (real gradation) -- OK")

    tex_px = pixel_at(frame, *tex_center)
    # rgba8 packs little-endian -> BGRA8 readback is (b, g, r, a).
    expected_bgra = (255, 0, 255, 255)
    assert tex_px == expected_bgra, f"texture fill center: expected {expected_bgra}, got {tex_px}"
    print(f"texture fill: {tex_px} (exact magenta round trip) -- OK")

    print("tre_python Gradient/Texture fill demo: PASSED")


if __name__ == "__main__":
    main()
