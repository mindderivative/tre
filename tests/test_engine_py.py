"""`node.animate` on a box: every animatable paint property, its errors
(`ValueError` for an unknown or non-animatable name, including the
pre-0.3.5 names; `TypeError` for a wrong value type), `on_complete` and
cyclic GC through it; and `App.run()` refusing to start with no window.
"""

import gc
import weakref

import pytest

from tre import App, Node, Window
from helpers import add


def test_create_box_returns_a_node():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0x67, 0x50, 0xA4, 0xFF), width=50, height=50)
    assert isinstance(node, Node)


def test_animate_accepts_each_known_paint_property():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    # None of these should raise -- `animate` registers the animation
    # and returns.
    node.animate("opacity", 0.5, duration_ms=100)
    node.animate("corner_radius", 12.0, duration_ms=100)
    node.animate("shadows", [((0, 0, 0, 80), 0.0, 2.0, 4.0, 0.0)], duration_ms=100)
    node.animate("fill", (255, 255, 255, 255), duration_ms=100)
    node.animate("stroke_color", (255, 0, 0, 255), duration_ms=100)
    node.animate("stroke_width", 2.0, duration_ms=100)
    node.animate("translate_x", 10.0, duration_ms=100)
    node.animate("scale", 1.5, duration_ms=100)


def test_animate_defaults_duration_to_an_instant_snap():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.animate("opacity", 0.2)  # duration_ms omitted -- must not raise


def test_an_unknown_or_non_animatable_property_raises_value_error():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="\"not_a_real_property\" isn't animatable"):
        node.animate("not_a_real_property", 1.0)
    with pytest.raises(ValueError, match="\"value\" isn't animatable"):
        node.animate("value", 0.5)
    # `value` is the accessibility value every node has -- unset, `None`.
    assert node.get("value") is None


def test_the_pre_0_3_5_property_names_are_gone():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    for name, to in [
        ("background", (0, 0, 0, 255)),
        ("foreground", (0, 0, 0, 255)),
        ("border_color", (0, 0, 0, 255)),
        ("border_width", 1.0),
        ("transform", (0.0, 0.0, 1.0)),
    ]:
        with pytest.raises(ValueError, match="isn't animatable"):
            node.animate(name, to)
        with pytest.raises(ValueError, match="unknown node property"):
            node.get(name)
        with pytest.raises(ValueError, match="unknown node property"):
            window.create("box", **{name: to})


def test_type_mismatch_raises_type_error_naming_expected_and_actual():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(TypeError, match="expects a float, got str"):
        node.animate("opacity", "not a float")


def test_fill_requires_a_four_tuple_not_a_float():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="must be an \\(r, g, b, a\\) tuple"):
        node.animate("fill", 0.5)


def test_app_requires_at_least_one_window():
    """`App.run()` installs the process-wide `tracing` subscriber before
    the window check, so the second call also proves reinstalling it is
    a no-op, not a panic."""
    app = App()
    with pytest.raises(RuntimeError, match="add_window"):
        app.run()
    with pytest.raises(RuntimeError, match="add_window"):
        App().run()  # must not panic on subscriber re-init


def test_animate_accepts_an_on_complete_callback():
    """Registering `on_complete` doesn't raise; `test_advance.py` shows
    it firing."""
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.animate("opacity", 0.5, duration_ms=100, on_complete=lambda: None)


def test_animate_still_works_with_on_complete_omitted():
    window = Window(width=200, height=200)
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    node.animate("opacity", 0.5, duration_ms=100)


def test_window_participates_in_cyclic_gc_when_an_on_complete_callback_captures_it_back():
    """An `on_complete` bound method that holds its window forms a cycle
    the cyclic collector must reclaim. A bound method, not a closure: a
    closure shares its cell with the enclosing local, so `del` would
    break the cycle by itself.
    """

    class Holder:
        def __init__(self):
            self.window = None

        def on_complete(self):
            self.window

    holder = Holder()
    window = Window(width=200, height=200)
    holder.window = window
    node = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)

    # window -> completions -> holder.on_complete (bound method) ->
    # __self__ -> holder -> .window -> window.
    node.animate("opacity", 0.5, duration_ms=100, on_complete=holder.on_complete)

    holder_ref = weakref.ref(holder)
    del window
    del node
    del holder
    assert holder_ref() is not None, (
        "sanity check: a real reference cycle must survive plain refcounting alone "
        "(if this fails, the test itself isn't constructing a real cycle)"
    )

    gc.collect()
    assert holder_ref() is None, (
        "the cycle (window <-> bound-method on_complete callback <-> holder) must be "
        "collected by CPython's cyclic GC once nothing outside it references any part of "
        "it -- if this fails, __traverse__/__clear__ aren't making the "
        "window's completions visible to the collector"
    )
