"""M39 Phase 2 Step 2 (§5, §7): real, repeatable coverage of
`Window.add_time_picker_dial` -- MD3's real *analog* Time Picker
circular drag control, genuinely distinct from `test_time_picker.py`'s
own `add_time_input_field`/`add_period_selector` (the *digital* Time
Input variant, M30 Phase 7 Step 2 -- that file's own doc comment
explicitly named this analog dial as a real, deferred gap needing "a
genuinely new drag-to-angle engine capability this project doesn't
have," which this step now builds).

The real angle-based drag math itself is proven directly at the Rust
level (`crates/engine-core/src/tree.rs`'s own `dispatch_drag_in_hour_
mode_*`/`dispatch_drag_in_minute_mode_*` tests) and via a real, live,
multi-frame render loop (`examples/time_picker_dial.py`); this file
proves the real FFI construction/getter/setter surface.
"""

import pytest

from tre import Node, Window


def test_add_time_picker_dial_returns_a_node():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial()
    assert isinstance(dial, Node)


def test_a_themed_time_picker_dial_does_not_raise():
    window = Window(width=400, height=400)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    dial = window.add_time_picker_dial()
    assert isinstance(dial, Node)


def test_a_custom_size_does_not_raise():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial(size=128.0)
    assert isinstance(dial, Node)


def test_the_initial_hour_and_minute_are_real_construction_arguments():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial(hour=15, minute=30)
    assert dial.get_time_picker_dial_time() == (15, 30)


def test_the_initial_hour_and_minute_default_to_midnight():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial()
    assert dial.get_time_picker_dial_time() == (0, 0)


def test_set_time_picker_dial_time_moves_both_hands_directly():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial()
    dial.set_time_picker_dial_time(9, 45)
    assert dial.get_time_picker_dial_time() == (9, 45)


def test_set_time_picker_dial_time_clamps_out_of_range_values():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial()
    dial.set_time_picker_dial_time(30, 90)
    assert dial.get_time_picker_dial_time() == (23, 59)


def test_the_dial_starts_in_hour_mode():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial()
    assert dial.get_time_picker_dial_mode() == "hour"


def test_set_time_picker_dial_mode_switches_to_minute():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial()
    dial.set_time_picker_dial_mode("minute")
    assert dial.get_time_picker_dial_mode() == "minute"


def test_set_time_picker_dial_mode_rejects_an_unknown_value():
    window = Window(width=400, height=400)
    dial = window.add_time_picker_dial()
    with pytest.raises(ValueError, match='"hour" or "minute"'):
        dial.set_time_picker_dial_mode("noon")


def test_time_picker_dial_getters_and_setters_reject_a_non_dial_node():
    window = Window(width=400, height=400)
    rect = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=10, height=10)
    with pytest.raises(ValueError):
        rect.get_time_picker_dial_time()
    with pytest.raises(ValueError):
        rect.set_time_picker_dial_time(1, 1)
    with pytest.raises(ValueError):
        rect.get_time_picker_dial_mode()
    with pytest.raises(ValueError):
        rect.set_time_picker_dial_mode("hour")


def test_multiple_dials_in_the_same_window_do_not_raise():
    window = Window(width=400, height=400)
    first = window.add_time_picker_dial(x=0.0, y=0.0)
    second = window.add_time_picker_dial(size=64.0, x=300.0, y=0.0)
    assert first is not second
