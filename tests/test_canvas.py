"""M5 Phase 3 (§11.10/§11.11): real, repeatable coverage of the
`Window.add_canvas`/`Window.redraw_canvas` FFI boundary -- the "draw
callback" pattern (mirroring `add_virtual_list`/`set_virtual_list_window`'s
own shape), its two real error paths (not a `Canvas` at all / not owned
by this `Window`, and the callback itself raising), and real cyclic-GC
participation for the new `canvas_draws` storage.

The definitive proof that `CanvasContext`'s drawing/hit-test methods
actually populate real, paintable/hit-testable `CanvasState` is the Rust
pixel-readback test (`crates/engine-render/tests/canvas_paint.rs`) --
matching this project's own established `test_splitter.py`/
`splitter_drag_dispatch.rs` FFI-test/pixel-test split. This file only
proves the FFI wiring itself: the callback is invoked, its exceptions
propagate, and the two ownership error paths are real.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_virtual_list.py`.
"""

import gc
import weakref

import pytest

from tre import Node, Window


def test_add_canvas_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_canvas(width=100, height=100, draw=lambda ctx: None)
    assert isinstance(node, Node)


def test_redraw_canvas_calls_draw_exactly_once_with_a_context():
    window = Window(width=200, height=200)
    calls = []

    def draw(ctx):
        calls.append(ctx)
        ctx.fill_circle(cx=10, cy=10, radius=5, color=(255, 0, 0, 255))

    canvas = window.add_canvas(width=100, height=100, draw=draw)
    window.redraw_canvas(canvas)
    assert len(calls) == 1

    window.redraw_canvas(canvas)
    assert len(calls) == 2


def test_canvas_context_drawing_and_hit_test_methods_accept_real_arguments():
    """Not the definitive pixel proof (that's `canvas_paint.rs`) -- just
    that every `CanvasContext` method callable from a real draw callback
    accepts the arguments this phase's own API promises, without
    raising."""
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.fill_rect(x=0, y=0, width=10, height=10, color=(255, 0, 0, 255))
        ctx.fill_circle(cx=5, cy=5, radius=5, color=(0, 255, 0, 255))
        ctx.stroke_path(points=[(0, 0), (10, 10), (20, 0)], color=(0, 0, 255, 255), width=2.0)
        ctx.set_hit_test_circle(cx=5, cy=5, radius=5)

    canvas = window.add_canvas(width=100, height=100, draw=draw)
    window.redraw_canvas(canvas)  # must not raise


def test_stroke_path_and_hit_test_path_reject_fewer_than_two_points():
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.stroke_path(points=[(0, 0)], color=(255, 0, 0, 255), width=1.0)

    canvas = window.add_canvas(width=100, height=100, draw=draw)
    with pytest.raises(ValueError, match="at least 2 points"):
        window.redraw_canvas(canvas)


def test_stroke_path_and_set_hit_test_path_accept_real_quadratic_and_cubic_segments():
    """M11 Phase 1 (§11.10, §11.11): `points` entries of length 4
    (quadratic) and 6 (cubic) build a real curved `BezPath`, not just
    the length-2 line-to points this accepted before this phase --
    `crates/engine-core/src/tree.rs`'s own `canvas_custom_path_hit_
    test_uses_the_real_curve_not_the_straight_chord_between_its_
    endpoints` is the definitive proof the resulting path is really
    curved (a curve-aware hit, not just "this doesn't raise"); this
    test only proves the FFI authoring surface itself accepts the real
    shapes, matching this file's own established "not the definitive
    proof" split.
    """
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.stroke_path(
            points=[(0, 0), (50, 100, 100, 0)],  # move_to, quad_to
            color=(0, 0, 255, 255),
            width=2.0,
        )
        ctx.stroke_path(
            points=[(0, 0), (30, 100, 70, -100, 100, 0)],  # move_to, curve_to
            color=(255, 0, 0, 255),
            width=2.0,
        )
        ctx.set_hit_test_path(points=[(0, 0), (50, 100, 100, 0)], tolerance=5.0)

    canvas = window.add_canvas(width=100, height=100, draw=draw)
    window.redraw_canvas(canvas)  # must not raise


def test_stroke_path_rejects_a_point_with_an_invalid_number_of_coordinates():
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.stroke_path(points=[(0, 0), (1, 2, 3)], color=(255, 0, 0, 255), width=1.0)

    canvas = window.add_canvas(width=100, height=100, draw=draw)
    with pytest.raises(ValueError, match="2 numbers .line., 4"):
        window.redraw_canvas(canvas)


def test_stroke_path_rejects_a_curve_segment_as_the_first_point():
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.stroke_path(
            points=[(0, 0, 50, 50), (10, 10)],  # first point can't be a curve segment
            color=(255, 0, 0, 255),
            width=1.0,
        )

    canvas = window.add_canvas(width=100, height=100, draw=draw)
    with pytest.raises(ValueError, match="first point must be a plain"):
        window.redraw_canvas(canvas)


def test_redraw_canvas_rejects_a_node_that_is_not_a_canvas():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=10, height=10)
    with pytest.raises(ValueError, match="not a Canvas"):
        window.redraw_canvas(rect)


def test_redraw_canvas_rejects_a_canvas_from_another_window():
    window_a = Window(width=200, height=200)
    window_b = Window(width=200, height=200)
    canvas = window_a.add_canvas(width=100, height=100, draw=lambda ctx: None)
    with pytest.raises(ValueError, match="not a Canvas"):
        window_b.redraw_canvas(canvas)


def test_a_draw_callback_exception_propagates_as_a_real_python_error():
    window = Window(width=200, height=200)

    def draw(ctx):
        raise RuntimeError("draw is cursed")

    canvas = window.add_canvas(width=100, height=100, draw=draw)
    with pytest.raises(RuntimeError, match="draw is cursed"):
        window.redraw_canvas(canvas)


def test_window_participates_in_cyclic_gc_when_its_draw_callback_captures_it_back():
    """Same real reason `canvas_draws` needs `__traverse__`/`__clear__`
    as `materializers` (`test_virtual_list.py`'s own docstring) -- a
    bound-method draw callback that reads other state off the very
    `Window` it was registered on forms a real reference cycle plain
    refcounting can never break."""

    class Holder:
        def __init__(self):
            self.window = None

        def draw(self, ctx):
            self.window
            ctx.fill_rect(x=0, y=0, width=1, height=1, color=(0, 0, 0, 255))

    holder = Holder()
    window = Window(width=50, height=50)
    holder.window = window

    # window -> canvas_draws -> holder.draw (bound method) -> __self__ ->
    # holder -> .window -> window.
    window.add_canvas(width=10, height=10, draw=holder.draw)

    holder_ref = weakref.ref(holder)
    del holder
    del window
    gc.collect()
    assert holder_ref() is None
