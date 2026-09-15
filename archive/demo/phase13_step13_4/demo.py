#!/usr/bin/env python3
"""Phase 13 Step 13.4 proof: real, blur-based drop shadows -- built
ENTIRELY on infrastructure already real and exposed since Phase 12 Step
12.5 (`canvas.layer(..., blur=True)`, backed by a real, already-shipped
Dual-Kawase blur, Step 7.2.2). The only new code this step adds is
`tre.shadow_layer_bounds(...)`, pure geometry computing where the
shadow's own offscreen layer must sit to contain both the shape and its
offset silhouette plus room for the blur to spread into.

This is deliberately NOT the "copy the shape, apply a gradient"
approach -- a linear alpha gradient can only ever approximate blur, and
needs a separate primitive-shaped gradient authored per shape. A real
Dual-Kawase blur produces a genuine, non-linear soft falloff, proven
below by scanning across the shadow's own original (pre-blur) edge and
finding a real gradient, not a hard cliff -- the one property that
distinguishes real blur from a solid alpha-blended duplicate.
"""

import tre_python as tre

WIDTH, HEIGHT = 200, 200


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def darkness(pixel: tuple) -> int:
    """Sum of RGB channels -- lower means darker (closer to the shadow)."""
    return pixel[0] + pixel[1] + pixel[2]


def main() -> None:
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)

    shape_x, shape_y, w, h = 60.0, 60.0, 40.0, 40.0
    offset_x, offset_y = 10.0, 10.0
    shadow_color = tre.rgba8(0, 0, 0, 200)
    margin = 20.0

    lx, ly, lw, lh = tre.shadow_layer_bounds(shape_x, shape_y, w, h, offset_x, offset_y, margin)
    print(f"shadow_layer_bounds: x={lx}, y={ly}, width={lw}, height={lh}")
    # Union of the shape's own AABB [60,100] and its offset copy
    # [70,110], expanded by the 20px margin on every side.
    assert (lx, ly, lw, lh) == (40, 40, 90, 90), f"unexpected shadow layer bounds: {(lx, ly, lw, lh)}"

    canvas = tre.Canvas()
    with canvas.layer(lx, ly, lw, lh, blur=True):
        # Content drawn inside a layer uses LOCAL coordinates relative
        # to the layer's own top-left (lx, ly), not the outer canvas's
        # absolute coordinates -- confirmed directly against tre-engine's
        # own RHI doc comment (`begin_render_to_texture`'s NDC mapping
        # uses only logical_width/logical_height, no offset) before
        # writing this demo.
        shadow_registry = tre.ShapeRegistry()
        local_x = shape_x + offset_x - lx
        local_y = shape_y + offset_y - ly
        shadow_registry.insert_rectangle(tre.Rectangle(local_x, local_y, w, h, shadow_color))
        renderer.flatten_into(canvas, shadow_registry)

    # The real shape, drawn AFTER (on top of) the shadow layer's own
    # composite -- so the shadow sits visually behind it.
    real_registry = tre.ShapeRegistry()
    real_registry.insert_rectangle(tre.Rectangle(shape_x, shape_y, w, h, tre.rgba8(0, 0, 0, 255)))
    renderer.flatten_into(canvas, real_registry)

    frame = renderer.render_canvas(canvas)

    background = pixel_at(frame, 190, 190)
    real_shape_pixel = pixel_at(frame, 80, 80)
    assert real_shape_pixel == (0, 0, 0, 255), "the real shape must render fully opaque on top"
    print(f"real shape interior: {real_shape_pixel} (opaque black, unaffected by the shadow beneath) -- OK")

    # The shadow rectangle's own UNBLURRED absolute edge sits at x=110
    # (local_x=30 + width=40, offset by the layer's own lx=40). Scan
    # across that edge along y=105 (below the real shape, in
    # shadow-only territory) and prove a real gradient, not a cliff.
    interior = pixel_at(frame, 95, 105)  # well inside the original hard-edged rect
    at_old_edge = pixel_at(frame, 110, 105)  # exactly at the ORIGINAL (pre-blur) edge
    just_past_edge = pixel_at(frame, 115, 105)  # 5px past the original edge
    far_past_edge = pixel_at(frame, 135, 105)  # far enough for blur to have fully decayed

    assert darkness(interior) < darkness(background), "the shadow's own interior must be visibly darker"
    assert darkness(interior) < darkness(at_old_edge) < darkness(background), (
        f"a REAL blur must show a graduated value AT the shape's own former hard edge, strictly "
        f"between interior ({interior}) and background ({background}) -- got {at_old_edge}. A "
        "solid alpha-blended duplicate (no real blur) would jump straight from interior to "
        "background exactly at this edge instead."
    )
    assert darkness(just_past_edge) < darkness(background), (
        f"REAL blur must spread some darkness PAST the shape's own original silhouette edge -- "
        f"got {just_past_edge} at 5px past it, expected strictly darker than background "
        f"{background}. This is the one property a hard-edged duplicate could never produce."
    )
    assert far_past_edge == background, (
        f"blur must not spread indefinitely -- far enough past the edge must be exactly "
        f"background, got {far_past_edge}"
    )
    print(
        f"real blur gradient confirmed across the shadow's own former hard edge: "
        f"interior={interior} -> at_edge={at_old_edge} -> past_edge={just_past_edge} -> "
        f"far={far_past_edge}=background -- OK"
    )

    print("tre_python shadow (blur-based, Phase 13 Step 13.4) demo: PASSED")


if __name__ == "__main__":
    main()
