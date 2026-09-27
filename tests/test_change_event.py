"""M14 Phase 3 (§5, §7.3): real, repeatable coverage of the new
`EventKind::Change` FFI boundary -- `Node.set_on_change`/`Node.
get_checked`, and the real, direct `Change` firing `Node.set_checked`
now does (mirroring `test_checkbox.py`'s own "FFI wiring only" split:
the real *mechanical* Slider-drag-release `Change` path has no Python
entry point at all, per `test_slider.py`'s own note, and stays proven
only by `engine-core`'s own `dispatch_release_ending_a_real_slider_
drag_produces_changed` test).

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_checkbox.py`.
"""

import pytest

from tre import Window
from helpers import add


def test_real_text_field_edits_give_old_and_new_string_content():
    """`Change`'s own `old_value` for a `TextField` is genuinely
    destroyed by the mutation that produces it -- `engine-core`'s own
    `ChangedValue::Text` snapshot (M54 Phase 1) is what makes this
    recoverable at all. M100: `set(text=...)` fires no `change` -- that
    event is for edits the user makes.
    """
    window = Window(width=200, height=200)
    field = add(window, "text_input", width=200, height=40, text="hello")
    events = []
    field.on("change", lambda event: events.append((event.old_value, event.new_value)))

    field.set(text="goodbye")
    assert events == []

    window.click(field)  # focus it
    window.press_key("backspace")
    assert events == [("goodbye", "goodby")]


