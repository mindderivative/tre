"""A text input's clipboard shortcuts and the OS clipboard.

`window.simulate("key_down", key="c", ctrl=True)` reaches the same
Ctrl+C/X/V/A handling a live key press does, and `read_clipboard`/
`write_clipboard` reach the OS clipboard itself. Where no clipboard service
is reachable (some headless and sandboxed environments) the tests that need
one skip (`needs_clipboard`). Select-all and typed input need none.
"""

import pytest

from tre import Window
from helpers import add


def needs_clipboard(window, probe="tre-clipboard-probe"):
    """Skips unless the OS clipboard round-trips `probe`."""
    if not window.write_clipboard(probe) or window.read_clipboard() != probe:
        pytest.skip("no OS clipboard service reachable in this environment")


def focused_field(text="hello"):
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text=text)
    window.simulate("key_down", key="tab")
    return window, field


def ctrl(window, letter, **fields):
    window.simulate("key_down", key=letter, ctrl=True, **fields)


def test_write_then_read_round_trips_through_the_os_clipboard():
    window = Window(width=200, height=100)
    needs_clipboard(window)
    assert window.write_clipboard("round trip") is True
    assert window.read_clipboard() == "round trip"


def test_ctrl_a_selects_the_focused_fields_whole_text():
    window, field = focused_field()
    ctrl(window, "a")
    assert field.get("selection") == (0, 5)
    assert field.get("text") == "hello", "select-all never edits"


def test_ctrl_a_with_nothing_focused_does_nothing():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=180, height=24, text="hi")
    ctrl(window, "a")
    assert field.get("selection") == (2, 2)


def test_ctrl_c_copies_the_selection_without_editing_or_firing_change():
    window, field = focused_field()
    needs_clipboard(window)
    changes = []
    field.on("change", lambda: changes.append(field.get("text")))
    ctrl(window, "a")
    ctrl(window, "c")
    assert window.read_clipboard() == "hello"
    assert field.get("text") == "hello"
    assert changes == []


def test_ctrl_c_with_no_selection_leaves_the_clipboard_alone():
    window, field = focused_field()
    needs_clipboard(window, "untouched")
    ctrl(window, "c")
    assert window.read_clipboard() == "untouched"


def test_ctrl_x_moves_the_selection_to_the_clipboard_and_fires_change():
    window, field = focused_field()
    needs_clipboard(window)
    changes = []
    field.on("change", lambda: changes.append(field.get("text")))
    ctrl(window, "a")
    ctrl(window, "x")
    assert window.read_clipboard() == "hello"
    assert field.get("text") == ""
    assert changes == [""]


def test_ctrl_x_with_no_selection_edits_nothing():
    window, field = focused_field()
    needs_clipboard(window, "untouched")
    ctrl(window, "x")
    assert field.get("text") == "hello"
    assert window.read_clipboard() == "untouched"


def test_ctrl_v_types_the_clipboards_text_at_the_caret():
    window, field = focused_field()
    needs_clipboard(window, "XY")
    inputs = []
    field.on("input", lambda e: inputs.append(e.text))
    window.simulate("key_down", key="home")
    ctrl(window, "v")
    assert field.get("text") == "XYhello"
    assert inputs == ["XY"], "a paste is typed input, so `input` fires"


def test_ctrl_v_replaces_the_selection():
    window, field = focused_field()
    needs_clipboard(window, "HI")
    ctrl(window, "a")
    ctrl(window, "v")
    assert field.get("text") == "HI"


def test_typed_input_inserts_at_the_caret_and_replaces_a_selection():
    window, field = focused_field()
    window.simulate("key_down", key="home")
    window.simulate("input", text="XY")
    assert field.get("text") == "XYhello"
    ctrl(window, "a")
    window.simulate("input", text="HI")
    assert field.get("text") == "HI"
