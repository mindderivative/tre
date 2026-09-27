"""Terminal nodes: a real `/bin/sh` on a PTY. Creation, click-to-focus, the
`text` grid, `selection`, resizing with `cols`/`rows`, wheel scrollback, and
input with nothing focused. Shell output only reaches the grid inside a
frame, so one test runs a bounded `App.run()` -- the only one in the pytest
process -- and skips when no display renders a frame.
"""

import sys
import time

import pytest

from tre import App, MONOSPACE_FONT_FAMILY, Node, Window
from helpers import add

pytestmark = pytest.mark.skipif(sys.platform == "win32", reason="Terminal is POSIX-only in this v1")


def test_create_terminal_returns_a_node():
    window = Window(width=400, height=300)
    node = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    assert isinstance(node, Node)


def test_create_terminal_accepts_an_absolute_position():
    window = Window(width=400, height=300)
    node = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)}, position="absolute", x=10, y=20)
    assert isinstance(node, Node)


def test_a_click_focuses_the_terminal():
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    assert term.get("focused") is False
    window.simulate("click", node=term)
    assert term.get("focused") is True


def test_text_of_a_fresh_terminal_is_an_empty_grid():
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=10, rows=3, palette={"background": (0, 0, 0, 255)})
    assert term.get("text") == "\n\n"


def test_text_on_a_box_raises_a_clear_error():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=40, height=40)
    with pytest.raises(ValueError):
        # `text` applies to text, text_input, and terminal nodes only.
        rect.get("text")


def test_a_real_shell_responds_to_typed_input():
    """A real shell on a real PTY: typed commands run and their output
    lands in the grid, Ctrl+C interrupts a running `sleep`, the wheel
    reveals scrollback, and a selection over echoed output round-trips.

    All of it shares one `App.run()`: a second real event loop in one
    pytest process can break other render-loop tests. With no display
    (e.g. headless CI) `App.run()` renders no frames, so PTY output never
    drains; a `call_soon` marker detects that and the test skips. The
    600-frame budget and the pause before Ctrl+C are headroom for slow
    machines.
    """
    window = Window(width=420, height=200)
    term = add(window, "terminal", shell="/bin/sh", cols=40, rows=5, scrollback_lines=200, palette={"background": (0, 0, 0, 255)})
    window.simulate("click", node=term)
    assert term.get("focused") is True

    window.simulate("input", text="echo HELLO_FROM_TERMINAL")
    window.simulate("key_down", key="enter")

    # More lines than the 5-row viewport holds, so there is scrollback
    # for the wheel below to reveal.
    window.simulate("input", text="for i in 1 2 3 4 5 6 7 8; do echo SCROLLBACK_LINE_$i; done")
    window.simulate("key_down", key="enter")

    # A running `sleep 100`, interrupted by Ctrl+C. Input is written to
    # the PTY immediately, not deferred to a frame, so this is the order
    # the shell receives it in; the pause lets the shell fork/exec `sleep`
    # first. Typed last, so its output stays in the unscrolled viewport.
    window.simulate("input", text="sleep 100")
    window.simulate("key_down", key="enter")
    time.sleep(0.5)
    window.simulate("key_down", key="c", ctrl=True)  # SIGINT
    window.simulate("input", text="echo REACHED_AFTER_SIGINT")
    window.simulate("key_down", key="enter")

    app = App()
    app.add_window(window)
    # A frame probe -- see the docstring.
    frames_ran = []
    app.thread_handle().call_soon(lambda: frames_ran.append(True))
    app.run(max_frames=600)
    if not frames_ran:
        pytest.skip("no display reachable: App.run() rendered no frames, so PTY output never drained")

    text = term.get("text")
    assert "REACHED_AFTER_SIGINT" in text, (
        f"the shell must have regained control right after the SIGINT -- if "
        f"sleep 100 were still running, this later command would never have executed, got "
        f"{text!r}"
    )
    assert "HELLO_FROM_TERMINAL" not in text, (
        "the first line typed must have already scrolled off a 5-row viewport by now"
    )

    # A wheel resyncs the grid synchronously, no frame needed. A huge
    # scroll clamps to the top of history, revealing the first line typed
    # and pushing the most recent one out of view.
    window.simulate("wheel", node=term, delta_y=-400.0)  # up: a negative wheel delta_y
    scrolled_text = term.get("text")
    assert "HELLO_FROM_TERMINAL" in scrolled_text, (
        f"a scroll must reveal previously-scrolled-off history, got {scrolled_text!r}"
    )
    assert "REACHED_AFTER_SIGINT" not in scrolled_text, (
        "scrolled all the way to the top of history, the most recent line must no longer "
        "be in view"
    )

    # A selection over echoed shell output: `HELLO_FROM_TERMINAL` is in
    # view after the scroll, so its range can be selected and read back.
    line = next(line for line in scrolled_text.split("\n") if "HELLO_FROM_TERMINAL" in line)
    col = line.index("HELLO_FROM_TERMINAL")
    row = scrolled_text.split("\n").index(line)
    selection = (row, col, row, col + len("HELLO_FROM_TERMINAL"))
    term.set(selection=selection)
    assert term.get("selection") == selection


def test_wheel_on_a_terminal_with_no_scrollback_does_not_raise():
    """A fresh terminal has no scrollback; a wheel clamps to 0."""
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    window.simulate("wheel", node=term, delta_y=100.0)
    window.simulate("wheel", node=term, delta_y=-100.0)


def test_wheel_on_a_virtual_list_is_unaffected_by_terminal_handling():
    """The terminal's wheel handling leaves other kinds, like a virtual
    list, alone."""
    window = Window(width=400, height=300)
    items = add(
        window,
        "virtual_list",
        item_count=10,
        materialize=lambda _i: window.create("box", fill=(255, 255, 255, 255)),
        item_extent=20.0,
        width=200,
        height=100,
    )
    window.simulate("wheel", node=items, delta_y=50.0)


def test_a_terminal_selection_round_trips_through_set_and_get():
    """A selection set directly (no drag needed) reads back as set,
    collapsed ones included."""
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=10, rows=1, palette={"background": (0, 0, 0, 255)})
    assert term.get("selection") is None, "no selection exists yet"

    term.set(selection=(0, 0, 0, 3))
    assert term.get("selection") == (0, 0, 0, 3)
    term.set(selection=(0, 2, 0, 2))
    assert term.get("selection") == (0, 2, 0, 2)


def test_selection_on_a_box_raises():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError):
        rect.set(selection=(0, 0, 0, 1))


def test_setting_cols_and_rows_resizes_the_grid_synchronously():
    """`set(cols=..., rows=...)` resyncs the grid immediately, so
    `get("text")` has the new shape with no frame. The PTY-level resize
    (`stty size` in a live shell) isn't checked here."""
    window = Window(width=600, height=400)
    term = add(window, "terminal", shell="/bin/sh", cols=10, rows=3, palette={"background": (0, 0, 0, 255)})
    assert len(term.get("text").split("\n")) == 3

    term.set(cols=20, rows=6)
    assert len(term.get("text").split("\n")) == 6, "setting cols/rows must resync the grid"


def test_cols_and_rows_on_a_box_raise():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError):
        rect.set(cols=10, rows=5)


def test_one_monospace_cell_measures_positive_and_scales_with_font_size():
    """A terminal cell is one character of the bundled monospace face,
    measured with `measure_text`: positive, and growing with `font_size`.
    """
    window = Window(width=400, height=300)
    width_14, height_14 = window.measure_text("M", font_family=MONOSPACE_FONT_FAMILY, font_size=14.0)
    width_28, height_28 = window.measure_text("M", font_family=MONOSPACE_FONT_FAMILY, font_size=28.0)
    assert width_14 > 0.0
    assert height_14 > 0.0
    assert width_28 > width_14
    assert height_28 > height_14


def test_keys_and_input_without_a_focused_terminal_fall_through_harmlessly():
    """Key and text input reach a terminal only when it's focused; with
    an unfocused terminal in the tree they are harmless."""
    window = Window(width=400, height=300)
    add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    # No click on the terminal -- nothing is focused.
    window.simulate("key_down", key="enter")
    window.simulate("input", text="hello")
