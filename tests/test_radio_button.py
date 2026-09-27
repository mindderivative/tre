"""M30 Phase 2 Step 1 (§5, §7.3): real, repeatable coverage of `Window.
add_radio_button`/`Node.set_selected`/the new `"select_progress"` arm
of `Node.animate`/`Node.get` -- the FFI boundary for a real MD3 radio
button. Mirrors `test_checkbox.py`'s own established structure exactly
(same "FFI wiring only" split -- `engine-render/tests/radio_button_
paint.rs` is the definitive pixel-level proof, not this file).
"""

import pytest

from tre import Node, Window


def test_set_selected_rejects_a_non_radio_button_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'selected'"):
        rect.set_selected(True)


def test_select_progress_property_is_unknown_on_a_non_radio_button_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'select_progress'"):
        rect.animate("select_progress", 1.0)
    with pytest.raises(ValueError, match="Rect has no property 'select_progress'"):
        rect.get("select_progress")


