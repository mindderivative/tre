#!/usr/bin/env python3
"""Phase 10 Step 10.4 proof: a real, end-to-end round trip through the
`tre_python` PyO3 binding -- construct a `ShapeRegistry` with three real
shape kinds (Rectangle/Circle/Polygon) from pure Python, render it on the
real GPU via `HeadlessRenderer`, and assert real, specific pixel values
read back from the finished frame.

Colors are deliberately pure primaries (each channel 0 or 255). 0.0 and
1.0 are the two fixed points of the sRGB transfer function, so these
particular colors read back byte-for-byte identical regardless of
whichever linear/sRGB conversion the pipeline applies internally --
letting this demo assert *exact* pixel equality the same way the Rust
`shape_registry_demo.rs` example does for its own pure-white rectangle,
without needing to hand-compute a partial color's sRGB encoding.
"""

import tre_python as tre

WIDTH, HEIGHT = 400, 300

RED = tre.rgba8(255, 0, 0, 255)
GREEN = tre.rgba8(0, 255, 0, 255)
BLUE = tre.rgba8(0, 0, 255, 255)

# rgba8 packs little-endian (r | g<<8 | b<<16 | a<<24), so BGRA8 readback
# bytes for each pure color are just its (b, g, r, a) permutation.
EXPECT_RED_BGRA = (0, 0, 255, 255)
EXPECT_GREEN_BGRA = (0, 255, 0, 255)
EXPECT_BLUE_BGRA = (255, 0, 0, 255)


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def main() -> None:
    registry = tre.ShapeRegistry()

    rect = tre.Rectangle(x=20, y=20, width=100, height=80, fill_color=RED)
    rect_center = (20 + 100 // 2, 20 + 80 // 2)
    registry.insert_rectangle(rect)

    # tre_engine::Circle's `x`/`y` are the bounding-box top-left (same
    # convention as Rectangle), not the center -- see PyCircle's own
    # doc comment. Center at (200, 60), radius 40 -> top-left (160, 20).
    circle_center = (200, 60)
    circle = tre.Circle(x=circle_center[0] - 40, y=circle_center[1] - 40, radius=40, fill_color=GREEN)
    registry.insert_circle(circle)

    polygon = tre.Polygon(x=320, y=150, sides=6, radius=50, fill_color=BLUE)
    polygon_center = (320, 150)
    registry.insert_polygon(polygon)

    assert len(registry) == 3, f"expected 3 shapes in the registry, got {len(registry)}"

    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    assert renderer.width == WIDTH
    assert renderer.height == HEIGHT

    frame = renderer.render(registry)
    assert isinstance(frame, bytes)
    assert len(frame) == WIDTH * HEIGHT * 4, (
        f"expected {WIDTH * HEIGHT * 4} bytes of tightly-packed BGRA8, got {len(frame)}"
    )

    background = pixel_at(frame, 0, 0)
    rect_px = pixel_at(frame, *rect_center)
    circle_px = pixel_at(frame, *circle_center)
    polygon_px = pixel_at(frame, *polygon_center)

    assert rect_px == EXPECT_RED_BGRA, f"rectangle center: expected {EXPECT_RED_BGRA}, got {rect_px}"
    assert circle_px == EXPECT_GREEN_BGRA, f"circle center: expected {EXPECT_GREEN_BGRA}, got {circle_px}"
    assert polygon_px == EXPECT_BLUE_BGRA, f"polygon center: expected {EXPECT_BLUE_BGRA}, got {polygon_px}"
    for name, px in (("rectangle", rect_px), ("circle", circle_px), ("polygon", polygon_px)):
        assert px != background, f"{name} center must not read back as background"

    print(f"background (0,0): {background}")
    print(f"rectangle center {rect_center}: {rect_px} -- OK (exact red)")
    print(f"circle center {circle_center}: {circle_px} -- OK (exact green)")
    print(f"polygon center {polygon_center}: {polygon_px} -- OK (exact blue)")

    try:
        from PIL import Image

        rgba = bytearray(frame)
        for i in range(0, len(rgba), 4):
            rgba[i], rgba[i + 2] = rgba[i + 2], rgba[i]  # BGRA -> RGBA
        img = Image.frombytes("RGBA", (WIDTH, HEIGHT), bytes(rgba))
        out_path = "phase10_step10_4_output.png"
        img.save(out_path)
        print(f"wrote {out_path}")
    except ImportError:
        print("Pillow not installed -- skipping PNG output")

    print("tre_python end-to-end demo: PASSED")


if __name__ == "__main__":
    main()
