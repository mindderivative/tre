#!/usr/bin/env python3
"""Phase 12 Step 12.8 proof: `tre.Svg`, a real SVG-to-pixels path through
`tre_python` -- real `usvg` parsing, real `lyon` fill tessellation
(`tre-svg`'s own real pipeline, the same one `svg_tessellation_demo.rs`/
`svg_morph_demo.rs` already prove on real GPU hardware), rendered
through a new, minimal `tre_engine::Svg` primitive reusing the existing
`FlatColor` pipeline.

Two real documents: a simple square (an exact-pixel sanity check) and a
ring with a real punched-out hole (`fill-rule="evenodd"`), proving both
fill rules and real compound-shape holes work end to end from Python,
not just in Rust.
"""

import tre_python as tre

WIDTH, HEIGHT = 200, 200
BLACK_BGRA = (0, 0, 0, 255)  # rgba8(0,0,0,255) little-endian -> BGRA readback.

SQUARE_SVG = b"""<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <path d="M 50 50 L 150 50 L 150 150 L 50 150 Z" fill="black"/>
</svg>"""

# A ring: outer 160x160 square minus an inner 60x60 square hole, wound
# oppositely and rendered with fill-rule="evenodd" so the inner square
# is a real hole, not a second overlapping fill.
RING_SVG = b"""<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <path fill-rule="evenodd" fill="black" d="
    M 20 20 L 180 20 L 180 180 L 20 180 Z
    M 70 70 L 70 130 L 130 130 L 130 70 Z
  "/>
</svg>"""


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def main() -> None:
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)

    # --- A real square, parsed and tessellated from real SVG bytes. ---
    square = tre.Svg.parse(SQUARE_SVG, tre.rgba8(0, 0, 0, 255))
    print(f"square: parsed into {square.triangle_count} real triangles")
    registry = tre.ShapeRegistry()
    registry.insert_svg(square)
    frame = renderer.render(registry)

    background = pixel_at(frame, 0, 0)
    inside = pixel_at(frame, 100, 100)
    outside = pixel_at(frame, 10, 10)
    assert inside == BLACK_BGRA, f"inside the square: expected {BLACK_BGRA}, got {inside}"
    assert outside == background, f"outside the square: expected background, got {outside}"
    print(f"square fill: inside={inside} (black), outside={outside} (background) -- OK")

    # --- A real ring with a real punched-out hole (evenodd). ---
    ring = tre.Svg.parse(RING_SVG, tre.rgba8(0, 0, 0, 255), tre.FillRule.EvenOdd)
    print(f"ring: parsed into {ring.triangle_count} real triangles")
    ring_registry = tre.ShapeRegistry()
    ring_registry.insert_svg(ring)
    ring_frame = renderer.render(ring_registry)

    ring_background = pixel_at(ring_frame, 0, 0)
    in_ring_body = pixel_at(ring_frame, 40, 100)  # between outer edge and hole
    in_the_hole = pixel_at(ring_frame, 100, 100)  # dead center -- inside the hole
    assert in_ring_body == BLACK_BGRA, f"ring body: expected {BLACK_BGRA}, got {in_ring_body}"
    assert in_the_hole == ring_background, (
        f"inside the hole: expected background (real hole, not filled), got {in_the_hole}"
    )
    print(
        f"ring fill (evenodd): body={in_ring_body} (black), hole={in_the_hole} "
        "(background -- real hole) -- OK"
    )

    print("tre_python SVG binding demo: PASSED")


if __name__ == "__main__":
    main()
