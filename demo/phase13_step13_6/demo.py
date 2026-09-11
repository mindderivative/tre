#!/usr/bin/env python3
"""Phase 13 Step 13.6 proof: real vertex animation (Q12) -- `tre.Svg.
morph(from_, to, t)`, interpolating two same-topology SVG meshes' own
already-tessellated vertex positions via `tre_math::lerp_points_batch`
(the identical real SIMD primitive `tre_svg::morph_into` itself uses).

No new `treAnimation` class was needed for this: `Svg.morph` is pure,
stateless sampling (mirroring `Tween::sample`'s own contract) -- driving
it frame-by-frame is just composing it with the `tre.Tween`/`tre.Easing`
machinery that already shipped in Step 13.2: `t = progress_tween.
sample(elapsed)`, then `frame_svg = tre.Svg.morph(a, b, t)`. This demo
proves that composition works end to end, not just the raw math.
"""

import tre_python as tre

WIDTH, HEIGHT = 200, 200

BIG_SQUARE = b"""<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <path d="M 30 30 L 170 30 L 170 170 L 30 170 Z" fill="black"/>
</svg>"""
SMALL_SQUARE = b"""<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <path d="M 80 80 L 120 80 L 120 120 L 80 120 Z" fill="black"/>
</svg>"""
# A triangle -- deliberately a DIFFERENT vertex count than the squares
# above, to prove morph rejects mismatched topology rather than silently
# producing garbage.
TRIANGLE = b"""<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <path d="M 100 30 L 170 170 L 30 170 Z" fill="black"/>
</svg>"""


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def render_svg(renderer: "tre.HeadlessRenderer", svg: "tre.Svg") -> bytes:
    # HeadlessRenderer opens a real winit EventLoop, and winit permits at
    # most one per process -- a fresh renderer per call (as an earlier
    # draft of Step 12.9's own demo discovered) fails deterministically
    # past the first one. One renderer is created once in main() and
    # reused for every render below.
    registry = tre.ShapeRegistry()
    registry.insert_svg(svg)
    return renderer.render(registry)


def check_morph_endpoints_match_the_real_keyframes_exactly(renderer: "tre.HeadlessRenderer") -> None:
    big = tre.Svg.parse(BIG_SQUARE, tre.rgba8(0, 0, 0, 255))
    small = tre.Svg.parse(SMALL_SQUARE, tre.rgba8(0, 0, 0, 255))
    assert big.triangle_count == small.triangle_count == 2

    at_zero = tre.Svg.morph(big, small, 0.0)
    at_one = tre.Svg.morph(big, small, 1.0)

    frame_big = render_svg(renderer, big)
    frame_at_zero = render_svg(renderer, at_zero)
    frame_small = render_svg(renderer, small)
    frame_at_one = render_svg(renderer, at_one)

    assert frame_at_zero == frame_big, "morph(big, small, t=0.0) must render EXACTLY like big itself"
    assert frame_at_one == frame_small, "morph(big, small, t=1.0) must render EXACTLY like small itself"
    print(
        "morph(..., t=0.0) == the 'from' keyframe exactly, morph(..., t=1.0) == the 'to' keyframe "
        "exactly -- OK"
    )


def check_morph_at_the_midpoint_is_a_real_intermediate_size(renderer: "tre.HeadlessRenderer") -> None:
    big = tre.Svg.parse(BIG_SQUARE, tre.rgba8(0, 0, 0, 255))
    small = tre.Svg.parse(SMALL_SQUARE, tre.rgba8(0, 0, 0, 255))
    midpoint = tre.Svg.morph(big, small, 0.5)
    frame = render_svg(renderer, midpoint)

    # big spans x/y in [30,170]; small spans [80,120]. The real linear
    # midpoint of each corresponding vertex pair spans [55,145].
    inside_big_only = pixel_at(frame, 40, 100)  # in big's footprint, outside the real midpoint's
    inside_midpoint = pixel_at(frame, 100, 100)  # dead center: inside big, small, AND the midpoint
    background = pixel_at(frame, 5, 5)

    assert inside_midpoint == (0, 0, 0, 255), "the midpoint mesh's own dead center must still be filled"
    assert inside_big_only == background, (
        "x=40 is inside the ORIGINAL big square but outside the real linearly-interpolated "
        f"midpoint footprint [55,145] -- a real morph must have shrunk there, got {inside_big_only}"
    )
    print("morph(..., t=0.5) produces a real intermediate size, not a hard cut between keyframes -- OK")


def check_mismatched_topology_is_rejected() -> None:
    big = tre.Svg.parse(BIG_SQUARE, tre.rgba8(0, 0, 0, 255))
    triangle = tre.Svg.parse(TRIANGLE, tre.rgba8(0, 0, 0, 255))
    try:
        tre.Svg.morph(big, triangle, 0.5)
        raise AssertionError("morph() between mismatched vertex counts should have raised ValueError")
    except ValueError as e:
        print(f"Svg.morph(square, triangle) correctly rejected: {e} -- OK")


def check_composition_with_tween_drives_a_real_animation(renderer: "tre.HeadlessRenderer") -> None:
    # No new treAnimation class needed: t itself is driven by the
    # already-shipped tre.Tween (Step 13.2), composed with Svg.morph.
    big = tre.Svg.parse(BIG_SQUARE, tre.rgba8(0, 0, 0, 255))
    small = tre.Svg.parse(SMALL_SQUARE, tre.rgba8(0, 0, 0, 255))
    progress = tre.Tween(0.0, 1.0, 1.0, tre.Easing.Linear)

    sizes = []
    for elapsed in (0.0, 0.25, 0.5, 0.75, 1.0):
        t = progress.sample(elapsed)
        frame_svg = tre.Svg.morph(big, small, t)
        frame = render_svg(renderer, frame_svg)
        # Count filled pixels along one horizontal scanline through the
        # shape's own center as a proxy for its current real width.
        filled = sum(1 for x in range(WIDTH) if pixel_at(frame, x, 100) == (0, 0, 0, 255))
        sizes.append(filled)

    assert sizes == sorted(sizes, reverse=True), (
        f"driving t from 0->1 via a real Tween must monotonically shrink the shape, got widths {sizes}"
    )
    print(f"Tween-driven morph animation: widths {sizes} monotonically shrink from big to small -- OK")


def main() -> None:
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    check_morph_endpoints_match_the_real_keyframes_exactly(renderer)
    check_morph_at_the_midpoint_is_a_real_intermediate_size(renderer)
    check_mismatched_topology_is_rejected()
    check_composition_with_tween_drives_a_real_animation(renderer)
    print("tre_python Svg.morph (vertex animation, Phase 13 Step 13.6) demo: PASSED")


if __name__ == "__main__":
    main()
