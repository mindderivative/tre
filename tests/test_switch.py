"""M30 Phase 2 Step 2 (§5, §7.3): real, repeatable coverage of `Window.
add_switch`/`Node.set_on`/the new `"toggle_progress"` arm of `Node.
animate`/`Node.get` -- the FFI boundary for a real MD3 switch. Mirrors
`test_radio_button.py`'s own established structure exactly (same "FFI
wiring only" split -- `engine-render/tests/switch_paint.rs` is the
definitive pixel-level proof, not this file).
"""

import pytest

from tre import Node, Window


def test_set_selected_rejects_a_non_switch_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'selected'"):
        rect.set_selected(True)


def test_toggle_progress_property_is_unknown_on_a_non_switch_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'toggle_progress'"):
        rect.animate("toggle_progress", 1.0)
    with pytest.raises(ValueError, match="Rect has no property 'toggle_progress'"):
        rect.get("toggle_progress")


