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


def test_real_text_field_edits_give_old_and_new_string_content():
    """`Change`'s own `old_value` for a `TextField` is genuinely
    destroyed by the mutation that produces it -- `engine-core`'s own
    `ChangedValue::Text` snapshot (M54 Phase 1) is what makes this
    recoverable at all, for both a direct `set_text` and a real
    keyboard-driven edit `Tree::dispatch` itself detects.
    """
    window = Window(width=200, height=200)
    field = window.add_text_field(
        background=(0xFF, 0xFF, 0xFF, 0xFF), width=200, height=40, content="hello"
    )
    events = []
    field.set_on_change(lambda event: events.append((event.old_value, event.new_value)))

    field.set_text("goodbye")
    assert events == [("hello", "goodbye")]

    window.click(field)  # focus it
    window.press_key("backspace")
    assert events[-1] == ("goodbye", "goodby")


