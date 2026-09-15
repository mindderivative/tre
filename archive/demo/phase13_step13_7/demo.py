#!/usr/bin/env python3
"""Phase 13 Step 13.7 proof: real SMIL `<animate>`/`<animateTransform
type="translate">` parsing (Q4) -- `usvg` (the parser `tre-svg` already
uses for everything else) discards SMIL animation elements entirely, a
static-resolution parser by design. `tre.parse_smil(data)` reads the raw
document's own XML directly via `roxmltree` and extracts the real
keyframe data an SVG author actually wrote.

No new engine machinery needed to DRIVE it: this demo composes the
extracted keyframes with the already-shipped `tre.Tween`/`tre.Easing`
(Step 13.2) to animate a real `tre.Svg` shape's position end to end,
proving real integration, not just raw extraction.
"""

import tre_python as tre

WIDTH, HEIGHT = 200, 200

ANIMATED_SVG = b"""<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <rect x="0" y="0" width="20" height="20">
    <animate attributeName="opacity" from="0" to="1" dur="2s"/>
    <animateTransform attributeName="transform" type="translate" from="10 10" to="150 150" dur="1s"/>
  </rect>
</svg>"""

SQUARE_SVG = b"""<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
  <path d="M 0 0 L 20 0 L 20 20 L 0 20 Z" fill="black"/>
</svg>"""


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def check_real_extraction() -> None:
    parsed = tre.parse_smil(ANIMATED_SVG)

    assert len(parsed.animates) == 1
    opacity_anim = parsed.animates[0]
    assert opacity_anim.attribute_name == "opacity"
    assert opacity_anim.keyframes == [0.0, 1.0]
    assert abs(opacity_anim.duration_seconds - 2.0) < 1e-5
    print(
        f"<animate attributeName='opacity' from='0' to='1' dur='2s'> -> "
        f"keyframes={opacity_anim.keyframes}, duration={opacity_anim.duration_seconds}s -- OK"
    )

    assert len(parsed.animate_translates) == 1
    translate_anim = parsed.animate_translates[0]
    assert translate_anim.keyframes == [(10.0, 10.0), (150.0, 150.0)]
    assert abs(translate_anim.duration_seconds - 1.0) < 1e-5
    print(
        f"<animateTransform type='translate' from='10 10' to='150 150' dur='1s'> -> "
        f"keyframes={translate_anim.keyframes}, duration={translate_anim.duration_seconds}s -- OK"
    )


def check_a_real_document_with_no_animation_returns_empty() -> None:
    parsed = tre.parse_smil(SQUARE_SVG)
    assert parsed.animates == []
    assert parsed.animate_translates == []
    print("a real static (non-animated) SVG document returns zero SMIL directives -- OK")


def check_malformed_xml_is_rejected() -> None:
    try:
        tre.parse_smil(b"<svg><rect><animate")
        raise AssertionError("malformed XML should have raised ValueError")
    except ValueError as e:
        print(f"malformed XML correctly rejected: {e} -- OK")


def check_composition_with_tween_drives_a_real_shape() -> None:
    # No new engine machinery: the extracted SMIL keyframes are driven
    # through the already-shipped tre.Tween/tre.Easing (Step 13.2).
    parsed = tre.parse_smil(ANIMATED_SVG)
    translate_anim = parsed.animate_translates[0]
    from_x, from_y = translate_anim.keyframes[0]
    to_x, to_y = translate_anim.keyframes[-1]
    position_tween = tre.Tween(
        (from_x, from_y), (to_x, to_y), translate_anim.duration_seconds, tre.Easing.Linear
    )

    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    square = tre.Svg.parse(SQUARE_SVG, tre.rgba8(0, 0, 0, 255))

    positions_seen = []
    for elapsed in (0.0, 0.25, 0.5, 0.75, 1.0):
        x, y = position_tween.sample(elapsed * translate_anim.duration_seconds)
        square.x, square.y = x, y
        registry = tre.ShapeRegistry()
        registry.insert_svg(square)
        frame = renderer.render(registry)
        # A pixel at the shape's own current top-left corner (+2 to stay
        # inside the 20x20 square, away from any edge AA) must be filled.
        filled = pixel_at(frame, int(x) + 2, int(y) + 2) == (0, 0, 0, 255)
        positions_seen.append((round(x), round(y), filled))

    assert all(filled for (_, _, filled) in positions_seen), (
        f"the real shape must render at its own real SMIL-driven position at every sampled "
        f"frame, got {positions_seen}"
    )
    xs = [x for (x, _, _) in positions_seen]
    assert xs == sorted(xs), f"x must move monotonically from 10 to 150, got {xs}"
    print(f"SMIL-driven real render: shape tracked positions {positions_seen} -- OK")


def main() -> None:
    check_real_extraction()
    check_a_real_document_with_no_animation_returns_empty()
    check_malformed_xml_is_rejected()
    check_composition_with_tween_drives_a_real_shape()
    print("tre_python parse_smil (Phase 13 Step 13.7) demo: PASSED")


if __name__ == "__main__":
    main()
