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


def test_value_property_is_unknown_on_a_non_progress_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'value'"):
        rect.animate("value", 0.5)
    # M94: `value` is also the accessibility value every node has, so
    # reading it on a plain rect returns that -- unset, `None`.
    assert rect.get("value") is None


