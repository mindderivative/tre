#!/usr/bin/env python3
"""Phase 12 Step 12.5 proof: the redesigned `Canvas` as a real scene-
assembly/compositing context, end to end through `tre_python`.

Two registries share one `Canvas`: a full-canvas red background, and a
green rectangle flattened *inside* a `with canvas.clip(...):` block that
only exposes a small sub-region -- real scissor clipping, not just
"renders something." `canvas.tag_accessibility_node(...)` is also
exercised (no visual effect, but must not raise). Finally,
`renderer.render(registry)`'s own single-registry convenience wrapper
(now implemented in terms of the same `flatten_into`/`render_canvas`
seam) is re-checked as a regression guard.
"""

import tre_python as tre

WIDTH, HEIGHT = 300, 200
RED = tre.rgba8(255, 0, 0, 255)
GREEN = tre.rgba8(0, 255, 0, 255)

CLIP_X, CLIP_Y, CLIP_W, CLIP_H = 100, 50, 60, 40


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def main() -> None:
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)

    background = tre.ShapeRegistry()
    background.insert_rectangle(tre.Rectangle(0, 0, WIDTH, HEIGHT, RED))

    green_layer = tre.ShapeRegistry()
    green_layer.insert_rectangle(tre.Rectangle(0, 0, WIDTH, HEIGHT, GREEN))

    canvas = tre.Canvas()
    renderer.flatten_into(canvas, background)
    with canvas.clip(CLIP_X, CLIP_Y, CLIP_W, CLIP_H):
        renderer.flatten_into(canvas, green_layer)

    canvas.tag_accessibility_node(1, CLIP_X, CLIP_Y, CLIP_W, CLIP_H, tre.AccessibilityRole.Generic)
    print("tag_accessibility_node: no error -- OK")

    frame = renderer.render_canvas(canvas)

    inside_clip = pixel_at(frame, CLIP_X + CLIP_W // 2, CLIP_Y + CLIP_H // 2)
    outside_clip_left = pixel_at(frame, CLIP_X - 10, CLIP_Y + CLIP_H // 2)
    outside_clip_below = pixel_at(frame, CLIP_X + CLIP_W // 2, CLIP_Y + CLIP_H + 10)

    # rgba8 packs little-endian -> BGRA8 readback is (b, g, r, a).
    expect_green_bgra = (0, 255, 0, 255)
    expect_red_bgra = (0, 0, 255, 255)

    assert inside_clip == expect_green_bgra, (
        f"inside the clip rect: expected green {expect_green_bgra}, got {inside_clip}"
    )
    assert outside_clip_left == expect_red_bgra, (
        f"left of the clip rect: expected red (unclipped background) {expect_red_bgra}, got "
        f"{outside_clip_left}"
    )
    assert outside_clip_below == expect_red_bgra, (
        f"below the clip rect: expected red (unclipped background) {expect_red_bgra}, got "
        f"{outside_clip_below}"
    )
    print(
        f"real clip: inside={inside_clip} (green), outside={outside_clip_left}/"
        f"{outside_clip_below} (red, unclipped) -- OK"
    )

    # Regression: render(registry)'s own single-registry convenience
    # wrapper, now implemented in terms of flatten_into/render_canvas,
    # must still work unchanged.
    simple_registry = tre.ShapeRegistry()
    # Circle's own x/y are the bounding-box top-left, matching
    # Rectangle's convention -- NOT the center (see PyCircle's own doc
    # comment in crates/tre-python/src/shapes.rs).
    simple_registry.insert_circle(tre.Circle(WIDTH // 2 - 30, HEIGHT // 2 - 30, 30, GREEN))
    simple_frame = renderer.render(simple_registry)
    simple_center = pixel_at(simple_frame, WIDTH // 2, HEIGHT // 2)
    assert simple_center == expect_green_bgra, (
        f"render(registry) regression: expected green {expect_green_bgra}, got {simple_center}"
    )
    print(f"render(registry) convenience wrapper (regression): {simple_center} -- OK")

    print("tre_python Canvas redesign demo: PASSED")


if __name__ == "__main__":
    main()
