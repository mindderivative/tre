"""M30 Phase 3 Step 2 (§5, §7): real, repeatable coverage of `Window.
add_linear_progress`/`add_circular_progress`, and the `"value"` arm of
`Node.animate`/`Node.get` -- the FFI boundary for MD3's real linear
and circular progress indicators. Mirrors `test_slider.py`'s own
established structure (same "FFI wiring only" split --
`engine-render/tests/progress_paint.rs` is the definitive pixel-level
proof, not this file).
"""

import pytest

from tre import Node, Window


def test_add_linear_progress_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_linear_progress(width=120)
    assert isinstance(node, Node)


def test_add_linear_progress_defaults_to_zero():
    window = Window(width=200, height=200)
    bar = window.add_linear_progress(width=120)
    assert bar.get("value") == 0.0


def test_add_linear_progress_accepts_an_initial_value():
    window = Window(width=200, height=200)
    bar = window.add_linear_progress(width=120, value=0.5)
    assert bar.get("value") == 0.5


def test_add_circular_progress_returns_a_node():
    window = Window(width=200, height=200)
    node = window.add_circular_progress()
    assert isinstance(node, Node)


def test_add_circular_progress_defaults_to_zero():
    window = Window(width=200, height=200)
    ring = window.add_circular_progress()
    assert ring.get("value") == 0.0


def test_animate_value_reaches_both_real_progress_indicators():
    window = Window(width=200, height=200)
    bar = window.add_linear_progress(width=120)
    ring = window.add_circular_progress()

    bar.animate("value", 0.75, duration_ms=0)  # must not raise
    ring.animate("value", 0.75, duration_ms=0)  # must not raise


def test_value_property_is_unknown_on_a_non_progress_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'value'"):
        rect.animate("value", 0.5)
    # M94: `value` is also the accessibility value every node has, so
    # reading it on a plain rect returns that -- unset, `None`.
    assert rect.get("value") is None


def test_a_themed_progress_indicator_does_not_raise():
    window = Window(width=200, height=200)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    bar = window.add_linear_progress(width=120, value=0.3)
    ring = window.add_circular_progress(value=0.3)
    assert isinstance(bar, Node)
    assert isinstance(ring, Node)
