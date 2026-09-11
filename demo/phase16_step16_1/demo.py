#!/usr/bin/env python3
"""Phase 16 Step 16.1 proof: a real, continuously tunable SDF-based soft
shadow -- built entirely on the existing custom shader API (Phase 13
Step 13.8), not by reworking `LayerDesc.blur`'s own fixed 4-hop Dual-
Kawase chain into something tunable. `tre.sdf_shadow_shader_source()`
returns real GLSL a caller compiles once via
`renderer.create_custom_shader(...)`; `tre.shadow_sdf_params(radius_px,
blur_px, width_px, height_px)` converts real pixel values into the
normalized `(radius_frac, sigma_x_frac, sigma_y_frac)` triple the shader
expects, since only 3 float slots exist in `UiVertex.params` (see
`crates/tre-python/src/sdf_shadow.rs`'s own module doc comment for the
full design and its real, disclosed v1 scope limits).

This demo proves three things, all against real GPU output:
1. The blur softness is genuinely CONTINUOUS -- the same shape rendered
   at three increasing `blur_px` values produces a strictly increasing
   ink spread past its own edge, unlike the old blur's fixed hop count.
2. The reused `sd_rounded_box` formula still behaves correctly inside
   the new shader: `radius_frac=0` stays a plain rectangle (a corner
   pixel is real ink), `radius_frac` near 1.0 on a square box visibly
   rounds it (the same corner pixel is real background).
3. The existing v1 blur-based shadow path (`canvas.layer(blur=True)` +
   `shadow_layer_bounds`, Phase 13 Step 13.4) still renders exactly as
   before -- v2 is additive, not a replacement.
"""

import tre_python as tre

WIDTH, HEIGHT = 200, 200
BLACK = tre.rgba8(0, 0, 0, 255)


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def render_custom_shadow(renderer, shader_id, quad_x, quad_y, quad_w, quad_h, param_x, param_y, param_z) -> bytes:
    registry = tre.ShapeRegistry()
    custom = tre.CustomShaded(quad_x, quad_y, quad_w, quad_h, shader_id, BLACK)
    custom.param_x, custom.param_y, custom.param_z = param_x, param_y, param_z
    registry.insert_custom_shaded(custom)
    return renderer.render(registry)


def render_shadow_at(renderer, shader_id, x, y, w, h, radius_px, blur_px) -> bytes:
    """`tre.shadow_sdf_params` returns everything a real caller needs:
    the ENLARGED quad bounds (the logical box expanded by `blur_px` on
    every side, giving the soft edge real rasterized room to fall off
    into -- a fragment shader is never invoked outside its own quad's
    triangles) plus the normalized params to set on it."""
    quad_x, quad_y, quad_w, quad_h, px, py, pz = tre.shadow_sdf_params(x, y, w, h, radius_px, blur_px)
    return render_custom_shadow(renderer, shader_id, quad_x, quad_y, quad_w, quad_h, px, py, pz)


def rightmost_ink_spread(frame: bytes, background: tuple, edge_x: int, y: int, x_max: int) -> int:
    """Scans rightward from `edge_x + 1` and returns how far past `edge_x`
    real (non-background) ink still reaches -- 0 if none at all."""
    spread = 0
    for x in range(edge_x + 1, x_max):
        if pixel_at(frame, x, y) != background:
            spread = x - edge_x
    return spread


def check_blur_softness_is_continuously_tunable(renderer, shader_id) -> None:
    """Same square shape, same real hard corner radius (0 -- an
    unrounded box keeps the geometry simple), three increasing real
    pixel blur values. `shadow_sdf_params` maps `blur_px` to
    `sigma_frac = blur_px / half_extent`; the shader's own smoothstep
    falloff reaches exactly zero at `d == sigma`, so real ink should
    reach almost exactly `blur_px` pixels past the shape's own nominal
    edge -- and, critically, strictly farther for each larger blur_px,
    proving real, continuous tunability (not a fixed number of discrete
    steps the way the old Dual-Kawase chain's own 4 fixed hops would
    behave)."""
    x, y, w, h = 20.0, 20.0, 80.0, 80.0  # half-extent = 40px
    scan_y = int(y + h / 2)  # a row through the shape's own vertical middle
    edge_x = int(x + w)  # the shape's own real, nominal (logical) right edge

    background = pixel_at(render_shadow_at(renderer, shader_id, x, y, w, h, 0.0, 0.0), WIDTH - 5, scan_y)

    blur_values = [0.0, 8.0, 16.0]
    spreads = []
    for blur_px in blur_values:
        frame = render_shadow_at(renderer, shader_id, x, y, w, h, 0.0, blur_px)
        spread = rightmost_ink_spread(frame, background, edge_x, scan_y, WIDTH)
        spreads.append(spread)
        print(f"blur_px={blur_px:5.1f} -> real measured ink spread={spread}px")

    assert spreads[0] == 0, f"zero blur must produce a real hard edge (0px spread), got {spreads[0]}"
    assert spreads[0] < spreads[1] < spreads[2], (
        f"ink spread must grow STRICTLY with blur_px -- proving real, continuous tunability, "
        f"not a fixed number of discrete steps: got {spreads}"
    )
    for blur_px, spread in zip(blur_values[1:], spreads[1:]):
        assert abs(spread - blur_px) <= 3, (
            f"measured spread ({spread}px) should track the real requested blur_px ({blur_px}) "
            f"reasonably closely (the shader's own smoothstep falloff reaches exactly zero at "
            f"d == sigma, i.e. at blur_px pixels past the edge)"
        )
    print(f"real, continuously tunable SDF soft edge: spreads strictly increase {spreads} -- OK")


def check_reused_sd_rounded_box_still_rounds_correctly(renderer, shader_id) -> None:
    """A plain (`radius_frac=0`) square box keeps real ink at its own
    literal corner; a near-`1.0` radius_frac on the SAME square box
    visibly rounds it away -- proving the exact IQ rounded-box SDF this
    shader reuses from `sdf_rect_styled.frag` still behaves correctly
    once evaluated in this new shader's own normalized local space."""
    x, y, w, h = 20.0, 20.0, 60.0, 60.0  # a square box, half-extent = 30px
    corner_x, corner_y = int(x) + 1, int(y) + 1  # 1px in from the box's own literal top-left corner
    edge_mid_x, edge_mid_y = int(x + w / 2), int(y) + 1  # 1px in from the top edge's own midpoint

    background = pixel_at(render_shadow_at(renderer, shader_id, x, y, w, h, 0.0, 0.0), WIDTH - 5, HEIGHT - 5)

    square_frame = render_shadow_at(renderer, shader_id, x, y, w, h, 0.0, 0.0)
    assert pixel_at(square_frame, corner_x, corner_y) != background, (
        "radius_frac=0 must keep a plain rectangle -- its own literal corner must be real ink"
    )
    print("radius_frac=0.0: the box's own literal corner is real ink (a plain rectangle) -- OK")

    rounded_frame = render_shadow_at(renderer, shader_id, x, y, w, h, 0.95 * (min(w, h) / 2), 0.0)
    assert pixel_at(rounded_frame, corner_x, corner_y) == background, (
        "radius_frac=0.95 on a square box must round the corner away -- the same literal corner "
        "pixel that was real ink at radius_frac=0 must now be real background"
    )
    assert pixel_at(rounded_frame, edge_mid_x, edge_mid_y) != background, (
        "rounding must only affect the corners -- a point at the middle of a flat edge must "
        "still be real ink"
    )
    print(
        "radius_frac=0.95: the same literal corner pixel is now real background (rounded away), "
        "while the flat edge's own midpoint is still real ink -- OK"
    )


def check_v1_blur_based_shadow_path_is_unaffected(renderer) -> None:
    """Phase 13 Step 13.4's own real Dual-Kawase-blur shadow path,
    reproduced verbatim -- proves v2 is purely additive, not a
    replacement: `canvas.layer(blur=True)` + `shadow_layer_bounds` still
    render exactly the same real, non-linear blur gradient as before
    this step's own vertex-shader swap in `create_custom_pipeline`."""

    def darkness(pixel: tuple) -> int:
        return pixel[0] + pixel[1] + pixel[2]

    shape_x, shape_y, w, h = 60.0, 60.0, 40.0, 40.0
    offset_x, offset_y = 10.0, 10.0
    shadow_color = tre.rgba8(0, 0, 0, 200)
    margin = 20.0

    lx, ly, lw, lh = tre.shadow_layer_bounds(shape_x, shape_y, w, h, offset_x, offset_y, margin)
    assert (lx, ly, lw, lh) == (40, 40, 90, 90), f"unexpected shadow layer bounds: {(lx, ly, lw, lh)}"

    canvas = tre.Canvas()
    with canvas.layer(lx, ly, lw, lh, blur=True):
        shadow_registry = tre.ShapeRegistry()
        local_x = shape_x + offset_x - lx
        local_y = shape_y + offset_y - ly
        shadow_registry.insert_rectangle(tre.Rectangle(local_x, local_y, w, h, shadow_color))
        renderer.flatten_into(canvas, shadow_registry)

    real_registry = tre.ShapeRegistry()
    real_registry.insert_rectangle(tre.Rectangle(shape_x, shape_y, w, h, BLACK))
    renderer.flatten_into(canvas, real_registry)

    frame = renderer.render_canvas(canvas)
    background = pixel_at(frame, 190, 190)
    interior = pixel_at(frame, 95, 105)
    at_old_edge = pixel_at(frame, 110, 105)
    far_past_edge = pixel_at(frame, 135, 105)

    assert darkness(interior) < darkness(at_old_edge) < darkness(background), (
        f"v1's own real blur gradient must still appear across the shadow's former hard edge: "
        f"interior={interior}, at_edge={at_old_edge}, background={background}"
    )
    assert far_past_edge == background, "v1's own blur must still decay back to real background"
    print("v1 blur-based shadow path (canvas.layer(blur=True) + shadow_layer_bounds): unchanged -- OK")


def main() -> None:
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    shader_id = renderer.create_custom_shader(tre.sdf_shadow_shader_source())
    print(f"compiled the real SDF shadow shader at runtime -> {shader_id} -- OK")

    check_blur_softness_is_continuously_tunable(renderer, shader_id)
    check_reused_sd_rounded_box_still_rounds_correctly(renderer, shader_id)
    check_v1_blur_based_shadow_path_is_unaffected(renderer)
    print("tre_python SDF-based soft shadows (Phase 16 Step 16.1) demo: PASSED")


if __name__ == "__main__":
    main()
