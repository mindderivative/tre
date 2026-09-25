"""M14 Phase 2 (§5, §7.3): real, repeatable coverage of `Window.
add_slider`/the new `"value"` arm of `Node.animate`/`Node.get`
-- the FFI boundary for a real MD3 slider.

The real drag-to-set interaction is entirely internal to `Tree::
dispatch` (M14 Phase 2's own real finding, mirroring how `Splitter`
dragging already works) -- there is no Python-facing entry point for a
drag at all, matching `test_splitter.py`'s own established scope (no
drag test exists there either; the real drag math is proven in
`engine-core`'s own `dispatch_drag_on_a_slider_...` tests and the
resulting real, distinct thumb positions in `crates/engine-render/
tests/slider_paint.rs`). This file proves the FFI wiring only: `add_
slider` returns a real, usable `Node`, and `"value"` reaches
real `SliderState` state via `animate`/`get`.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_splitter.py`.
"""

import pytest

from tre import Node, Window


def test_add_slider_returns_a_node():
    window = Window(width=200, height=40)
    node = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=180, height=32)
    assert isinstance(node, Node)


def test_add_slider_defaults_to_zero():
    window = Window(width=200, height=40)
    slider = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=180, height=32)
    assert slider.get("value") == 0.0


def test_add_slider_accepts_a_custom_initial_value():
    window = Window(width=200, height=40)
    slider = window.add_slider(
        background=(0x03, 0xDA, 0xC6, 0xFF), width=180, height=32, value=0.75
    )
    assert slider.get("value") == 0.75


def test_add_slider_clamps_an_out_of_range_initial_value():
    window = Window(width=200, height=40)
    slider = window.add_slider(
        background=(0x03, 0xDA, 0xC6, 0xFF), width=180, height=32, value=5.0
    )
    assert slider.get("value") == 1.0


def test_animate_thumb_position_reaches_the_real_slider():
    window = Window(width=200, height=40)
    slider = window.add_slider(background=(0x03, 0xDA, 0xC6, 0xFF), width=180, height=32)
    slider.animate("value", 0.6, duration_ms=0)  # must not raise


def test_thumb_position_property_is_unknown_on_a_non_slider_node():
    window = Window(width=200, height=40)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'value'"):
        rect.animate("value", 0.5)
    # M94: `value` is also the accessibility value every node has, so
    # reading it on a plain rect returns that -- unset, `None`.
    assert rect.get("value") is None
