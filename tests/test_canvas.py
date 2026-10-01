"""Canvas nodes: `window.create("canvas", draw=...)` and `node.redraw()`, the
`CanvasContext` drawing and hit-test methods' argument checking, a raising
draw callback, and cyclic GC through the draw callback. What a canvas paints
is covered by `engine-render`'s `canvas_paint.rs`.
"""

import gc
import weakref

import pytest

from tre import Node, Window
from helpers import add


def test_create_canvas_returns_a_node():
    window = Window(width=200, height=200)
    node = add(window, "canvas", width=100, height=100, draw=lambda ctx: None)
    assert isinstance(node, Node)


def test_a_canvas_draws_when_created_and_once_per_redraw():
    window = Window(width=200, height=200)
    calls = []

    def draw(ctx):
        calls.append(ctx)
        ctx.fill_circle(cx=10, cy=10, radius=5, color=(255, 0, 0, 255))

    canvas = add(window, "canvas", width=100, height=100, draw=draw)
    assert len(calls) == 1

    canvas.redraw()
    assert len(calls) == 2


def test_canvas_context_drawing_and_hit_test_methods_accept_their_arguments():
    """Every `CanvasContext` method accepts its documented arguments
    (the pixels are `canvas_paint.rs`'s job)."""
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.fill_rect(x=0, y=0, width=10, height=10, color=(255, 0, 0, 255))
        ctx.fill_circle(cx=5, cy=5, radius=5, color=(0, 255, 0, 255))
        ctx.stroke_path(points=[(0, 0), (10, 10), (20, 0)], color=(0, 0, 255, 255), width=2.0)
        ctx.set_hit_test_circle(cx=5, cy=5, radius=5)

    canvas = add(window, "canvas", width=100, height=100, draw=draw)
    canvas.redraw()  # must not raise


def test_stroke_path_and_hit_test_path_reject_fewer_than_two_points():
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.stroke_path(points=[(0, 0)], color=(255, 0, 0, 255), width=1.0)

    with pytest.raises(ValueError, match="at least 2 points"):
        add(window, "canvas", width=100, height=100, draw=draw)


def test_stroke_path_and_set_hit_test_path_accept_quadratic_and_cubic_segments():
    """`points` entries of 4 numbers are quadratic segments and of 6 are
    cubic ones; `engine-core`'s tree tests prove the hit test follows
    the curve."""
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

    canvas = add(window, "canvas", width=100, height=100, draw=draw)
    canvas.redraw()  # must not raise


def test_stroke_path_rejects_a_point_with_an_invalid_number_of_coordinates():
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.stroke_path(points=[(0, 0), (1, 2, 3)], color=(255, 0, 0, 255), width=1.0)

    with pytest.raises(ValueError, match="2 numbers .line., 4"):
        add(window, "canvas", width=100, height=100, draw=draw)


def test_stroke_path_rejects_a_curve_segment_as_the_first_point():
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.stroke_path(
            points=[(0, 0, 50, 50), (10, 10)],  # first point can't be a curve segment
            color=(255, 0, 0, 255),
            width=1.0,
        )

    with pytest.raises(ValueError, match="first point must be a plain"):
        add(window, "canvas", width=100, height=100, draw=draw)


def test_redraw_rejects_a_node_that_is_not_a_canvas():
    window = Window(width=200, height=200)
    rect = add(window, "box", fill=(0, 0, 0, 255), width=10, height=10)
    with pytest.raises(ValueError, match="applies only to a canvas"):
        rect.redraw()


def test_a_draw_callback_exception_propagates_to_the_caller():
    window = Window(width=200, height=200)

    def draw(ctx):
        raise RuntimeError("draw is cursed")

    with pytest.raises(RuntimeError, match="draw is cursed"):
        add(window, "canvas", width=100, height=100, draw=draw)


def test_window_participates_in_cyclic_gc_when_its_draw_callback_captures_it_back():
    """A bound-method draw callback that holds the window it draws in
    forms a cycle only the cyclic collector can break."""

    class Holder:
        def __init__(self):
            self.window = None

        def draw(self, ctx):
            self.window
            ctx.fill_rect(x=0, y=0, width=1, height=1, color=(0, 0, 0, 255))

    holder = Holder()
    window = Window(width=50, height=50)
    holder.window = window

    # window -> draw callback (bound method) -> __self__ -> holder ->
    # .window -> window.
    add(window, "canvas", width=10, height=10, draw=holder.draw)

    holder_ref = weakref.ref(holder)
    del holder
    del window
    gc.collect()
    assert holder_ref() is None


def test_a_custom_hit_shape_is_in_painter_coordinates_inside_the_padding():
    """0.5.1 (#53): a canvas's painter coordinates start at its padding, so a
    circle at painter (10, 10) in a canvas padded 30 left and 20 top is hit at
    the node's local (40, 30), and the node's own corner misses."""
    window = Window(width=200, height=200)

    def draw(ctx):
        ctx.set_hit_test_circle(cx=10, cy=10, radius=5)

    canvas = add(window, "canvas", width=100, height=100,
                 padding_left=30, padding_top=20, draw=draw)
    hits = []
    canvas.on("click", lambda: hits.append("hit"))
    window.simulate("click", node=canvas, x=40, y=30)
    window.simulate("click", node=canvas, x=10, y=10)
    assert hits == ["hit"]
