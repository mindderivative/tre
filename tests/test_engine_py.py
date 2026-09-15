"""§14 step 6: real, repeatable coverage of `engine-py`'s FFI boundary --
node creation, the one property setter (`Node.animate`), and its two
`EngineError` paths (`UnknownProperty` -> `ValueError`, `TypeMismatch` ->
`TypeError`, §8's own design). `App.run()` itself isn't exercised here
(it blocks and opens a real window) -- `examples/animate_rect.py` is
that proof; this file is the fast, no-window-needed regression coverage
that should never need a display to run.

Requires `maturin develop` to have installed the compiled extension into
the active environment first -- these tests import the real `tre`
package, not a mock.
"""

import pytest

from tre import App, Node


def test_add_rect_returns_a_node():
    app = App(width=200, height=200)
    node = app.add_rect(background=(0x67, 0x50, 0xA4, 0xFF), width=50, height=50)
    assert isinstance(node, Node)


def test_animate_accepts_each_known_paint_property():
    app = App(width=200, height=200)
    node = app.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    # None of these should raise -- registering an animation is a
    # fire-and-forget call (§8: "registers work and returns, never
    # blocks"), so success is simply the absence of an exception.
    node.animate("opacity", 0.5, duration_ms=100)
    node.animate("corner_radius", 12.0, duration_ms=100)
    node.animate("elevation", 4.0, duration_ms=100)
    node.animate("background", (255, 255, 255, 255), duration_ms=100)


def test_animate_defaults_duration_to_an_instant_snap():
    app = App(width=200, height=200)
    node = app.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    node.animate("opacity", 0.2)  # duration_ms omitted -- must not raise


def test_unknown_property_raises_value_error_naming_the_node_kind():
    app = App(width=200, height=200)
    node = app.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError, match="Rect has no property 'not_a_real_property'"):
        node.animate("not_a_real_property", 1.0)


def test_type_mismatch_raises_type_error_naming_expected_and_actual():
    app = App(width=200, height=200)
    node = app.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(TypeError, match="expects a float, got str"):
        node.animate("opacity", "not a float")


def test_background_requires_a_four_tuple_not_a_float():
    app = App(width=200, height=200)
    node = app.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    with pytest.raises(TypeError, match="expects an \\(r, g, b, a\\) tuple"):
        node.animate("background", 0.5)
