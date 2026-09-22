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


def test_get_checked_reads_back_the_real_checkbox_state():
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)
    assert checkbox.get_checked() is False

    checkbox.set_checked(True)
    assert checkbox.get_checked() is True


def test_get_checked_rejects_a_non_checkbox_node():
    window = Window(width=200, height=200)
    rect = window.add_rect(background=(0, 0, 0, 255), width=24, height=24)
    with pytest.raises(ValueError, match="Rect has no property 'checked'"):
        rect.get_checked()


def test_set_checked_fires_a_real_registered_on_change_handler():
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)

    calls = []
    checkbox.set_on_change(lambda: calls.append(checkbox.get_checked()))

    checkbox.set_checked(True)
    assert calls == [True], "set_checked(True) must fire the real registered on_change handler"

    checkbox.set_checked(False)
    assert calls == [True, False], "each real set_checked call fires on_change again"


def test_set_checked_with_no_registered_handler_does_not_raise():
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)
    checkbox.set_checked(True)  # must not raise -- an unregistered Change handler is a no-op


def test_set_checked_gives_a_one_arg_handler_a_real_event_with_old_and_new_value():
    """M54 Phase 2 (§8, §16.2): the real payload gap this milestone
    closes -- a handler that declares one parameter is arity-sniffed at
    registration and receives a real `Event`, not the zero-argument
    call every handler used to get unconditionally.
    """
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)

    events = []
    checkbox.set_on_change(lambda event: events.append(event))

    checkbox.set_checked(True)
    assert len(events) == 1
    event = events[0]
    assert event.kind == "change"
    assert event.old_value is False
    assert event.new_value is True
    assert event.position is None
    assert event.button is None


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


def test_a_slider_click_release_gives_a_real_numeric_old_and_new_value():
    """The real, mechanical `Tree::dispatch`-detected `Changed` path for
    a `Slider` -- `Window.click` is a real primary-button press+release
    pair at the same point (a zero-movement drag start/end), the
    synthetic no-window-needed way to reach it (`test_change_event.py`'s
    own module doc comment: no Python entry point exists for a real
    *moving* drag). `old_value`/`new_value` are both real Python
    floats, not the `str`/`bool` another `Changed` producer would give.
    """
    window = Window(width=200, height=200)
    slider = window.add_slider(background=(0x79, 0x74, 0x7A, 0xFF), width=160, height=20)
    slider.enable_interaction()
    events = []
    slider.set_on_change(lambda event: events.append((event.old_value, event.new_value)))

    window.click(slider)
    assert len(events) == 1
    old_value, new_value = events[0]
    assert isinstance(old_value, float)
    assert isinstance(new_value, float)
    assert old_value == pytest.approx(0.0)
    assert new_value == pytest.approx(0.0)


def test_a_raising_on_change_handler_is_caught_logged_and_non_fatal(capfd):
    """Matches `Window.click`'s own established policy (§9): an uncaught
    exception from a real handler is caught and logged via `tracing::
    error!` (M16 Phase 2), not propagated -- `call_handler` is the same
    shared mechanism `Node.set_checked` now reuses for a direct `Change`
    fire.

    `capfd`, not `capsys`: `tracing_subscriber`'s own writer is a raw
    OS-level stderr write from Rust, bypassing Python's `sys.stderr`
    object entirely -- `capsys` (which only monkeypatches that Python
    object) can't see it, confirmed by actually running this test with
    `capsys` and watching it fail with an empty capture even though the
    real event was genuinely emitted (`capfd` captures at the file-
    descriptor level, real for both Python and Rust writes).
    """
    window = Window(width=200, height=200)
    checkbox = window.add_checkbox(background=(0x63, 0x50, 0xA4, 0xFF), width=24, height=24)

    def on_change():
        raise RuntimeError("boom")

    checkbox.set_on_change(on_change)
    checkbox.set_checked(True)  # must not raise

    captured = capfd.readouterr()
    assert "boom" in captured.err
    assert "RuntimeError" in captured.err
