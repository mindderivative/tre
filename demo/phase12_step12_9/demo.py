#!/usr/bin/env python3
"""Phase 12 Step 12.9 proof: two real `tre-python` Q&A follow-up fixes.

1. `border_enabled: bool` -- a real on/off switch for a shape's border,
   independent of `border_thickness`. Before this fix, the only way to
   turn a border off was to zero `border_thickness`, discarding the
   configured width. Proven by an exact-byte comparison: a rectangle
   with `border_thickness=8, border_enabled=False` must render IDENTICAL
   pixels to one with `border_thickness=0` -- a disabled border of any
   width is indistinguishable from no border at all.

2. `scale_x`/`scale_y`/`rotation` -- `tre_engine::Transform2D` already
   carried real scale/rotation fields (used by every native Rust demo),
   but `tre-python`'s own `common()` helper silently hardcoded them to
   identity, discarding them for every Python-inserted shape. Proven by
   exact-pixel assertions against the real transform math
   (`Transform2D::to_affine2`: scale first, then rotate, then
   translate -- confirmed by reading `tre-engine`'s own source, not
   assumed) for both a pure non-uniform scale and a 90-degree rotation
   pivoting on the shape's own local origin (top-left, matching every
   other shape's position convention).

One `HeadlessRenderer` is created ONCE and reused for every render below
-- `HeadlessRenderer` opens a real `PlatformConnection` (a real winit
`EventLoop` under the hood, confirmed by reading `tre-platform`'s own
`ConnectionFailed` error text), and winit enforces a real, hard OS-level
rule: at most one `EventLoop` per process, ever. A fresh renderer per
render call is therefore not just wasteful but broken past the first one.
"""

import math

import tre_python as tre

WIDTH, HEIGHT = 200, 200
FILL_COLOR = tre.rgba8(0, 0, 0, 255)  # packed u32, for Rectangle(fill_color=...).
BLACK = (0, 0, 0, 255)  # a readback BGRA pixel tuple, for comparing against render() output.

CONSTRUCTOR_FIELDS = {"x", "y", "width", "height", "fill_color", "scale_x", "scale_y", "rotation"}


def pixel_at(buf: bytes, x: int, y: int) -> tuple:
    offset = (y * WIDTH + x) * 4
    return tuple(buf[offset : offset + 4])


def render_rectangle(renderer: "tre.HeadlessRenderer", **kwargs) -> bytes:
    ctor_kwargs = {k: v for k, v in kwargs.items() if k in CONSTRUCTOR_FIELDS}
    rect = tre.Rectangle(**ctor_kwargs)
    for key, value in kwargs.items():
        if key not in CONSTRUCTOR_FIELDS:
            setattr(rect, key, value)
    registry = tre.ShapeRegistry()
    registry.insert_rectangle(rect)
    return renderer.render(registry)


def check_border_enabled(renderer: "tre.HeadlessRenderer") -> None:
    no_border = render_rectangle(renderer, x=50, y=50, width=60, height=60, fill_color=FILL_COLOR)

    thick_disabled = render_rectangle(
        renderer,
        x=50,
        y=50,
        width=60,
        height=60,
        fill_color=FILL_COLOR,
        border_color=tre.rgba8(255, 0, 0, 255),
        border_thickness=8.0,
        border_enabled=False,
    )
    assert thick_disabled == no_border, (
        "border_enabled=False with border_thickness=8.0 must render "
        "byte-identical to no border configured at all"
    )
    print("border_enabled=False (thickness=8) == no border at all -- OK")

    thick_enabled = render_rectangle(
        renderer,
        x=50,
        y=50,
        width=60,
        height=60,
        fill_color=FILL_COLOR,
        border_color=tre.rgba8(255, 0, 0, 255),
        border_thickness=8.0,
        border_enabled=True,
    )
    assert thick_enabled != no_border, (
        "border_enabled=True with a real thickness must NOT match a borderless render"
    )
    print("border_enabled=True (thickness=8) != no border -- OK (real border drawn)")

    # Real settable attribute: toggle the SAME object and confirm the
    # configured thickness survives being turned off then back on.
    rect = tre.Rectangle(x=50, y=50, width=60, height=60, fill_color=FILL_COLOR)
    rect.border_color = tre.rgba8(255, 0, 0, 255)
    rect.border_thickness = 8.0
    rect.border_enabled = False
    assert rect.border_thickness == 8.0, "disabling the border must not zero its thickness"
    rect.border_enabled = True
    registry = tre.ShapeRegistry()
    registry.insert_rectangle(rect)
    frame = renderer.render(registry)
    assert frame == thick_enabled, "re-enabling must restore the exact same border render"
    print("toggling border_enabled off/on preserves border_thickness=8.0 -- OK")


def check_scale(renderer: "tre.HeadlessRenderer") -> None:
    unscaled = render_rectangle(renderer, x=50, y=50, width=60, height=20, fill_color=FILL_COLOR)
    scaled = render_rectangle(
        renderer, x=50, y=50, width=60, height=20, fill_color=FILL_COLOR, scale_x=2.0, scale_y=1.0
    )
    background = pixel_at(unscaled, 0, 0)
    # x=160 is inside [50, 50+60*2=170] only when scale_x=2 is applied.
    probe = (160, 60)
    assert pixel_at(unscaled, *probe) == background, "unscaled rect must not reach x=160"
    assert pixel_at(scaled, *probe) == BLACK, "scale_x=2.0 must stretch the rect out to x=160"
    print(f"scale_x=2.0 real stretch confirmed at pixel {probe} -- OK")


def check_rotation(renderer: "tre.HeadlessRenderer") -> None:
    # A wide, short rectangle, top-left pivot at (50, 50). Local corners
    # (relative to the pivot) are (0,0)/(60,0)/(0,20)/(60,20).
    # Transform2D::to_affine2 composes scale, then rotation, then
    # translation (confirmed directly in tre-engine's own source) --
    # Affine2::from_rotation(theta) maps (x, y) -> (x*cos - y*sin,
    # x*sin + y*cos) (confirmed directly in tre-math's own source and
    # its own test asserting a 90-degree turn sends (1,0) -> (0,1)).
    # At theta = 90 degrees: (x, y) -> (-y, x). The bottom-right local
    # corner (60, 20) therefore maps to (-20, 60), so the whole
    # rectangle rotates into the strip x in [30, 50], y in [50, 110] --
    # entirely to the LEFT of its own unrotated footprint x in [50, 110].
    unrotated = render_rectangle(renderer, x=50, y=50, width=60, height=20, fill_color=FILL_COLOR)
    rotated = render_rectangle(
        renderer, x=50, y=50, width=60, height=20, fill_color=FILL_COLOR, rotation=math.pi / 2.0
    )
    background = pixel_at(unrotated, 0, 0)

    inside_rotated_only = (40, 100)  # x in [30,50], y in [50,110]: only the rotated footprint.
    inside_unrotated_only = (100, 60)  # x in [50,110], y in [50,70]: only the original footprint.

    assert pixel_at(unrotated, *inside_rotated_only) == background
    assert pixel_at(rotated, *inside_rotated_only) == BLACK
    assert pixel_at(unrotated, *inside_unrotated_only) == BLACK
    assert pixel_at(rotated, *inside_unrotated_only) == background
    print("rotation=pi/2 real 90-degree turn confirmed against exact transform math -- OK")


def main() -> None:
    renderer = tre.HeadlessRenderer(WIDTH, HEIGHT)
    check_border_enabled(renderer)
    check_scale(renderer)
    check_rotation(renderer)
    print("tre_python border_enabled + scale/rotation exposure demo: PASSED")


if __name__ == "__main__":
    main()
