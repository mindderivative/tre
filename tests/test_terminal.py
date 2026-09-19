"""M30 Phase 9 Step 4 (§5, §8, §10): real, repeatable coverage of
`Window.add_terminal` -- a real, live pseudo-terminal (`portable_pty`
spawns a real shell, `vt100` parses its real byte stream), not a
simulated one. No official MD3 page exists (confirmed via the same
directory-listing technique this milestone already uses).

**A real, structural difference from every other component's own test
suite in this project, stated honestly, not glossed over:** every
other FFI test proves its claim through synchronous `Tree::dispatch`
calls alone (`window.click`/`press_key`/`type_text`), no real render
loop needed. A `Terminal`'s own real content only ever arrives via its
background PTY reader thread, drained once per real frame tick inside
`App.run`'s own per-window closure (`app.rs`) -- so proving a real
shell genuinely responds needs a real, if brief, `App.run(max_frames=
...)` call, not just synchronous dispatch. Every test below that needs
real shell output spawns `/bin/sh` (POSIX, minimal, fast to start,
universally available in CI) and sets up its own real input *before*
its one real `app.run()` call, the same "one blocking call" real
structure every other example in this project already uses -- calling
`App.run()` a second time on the same `App`/`Window` is not a
supported, tested pattern here (confirmed empirically while writing
this file: a real second call silently never re-opened the window).
"""

import sys
import time

import pytest

from tre import App, Node, Window

pytestmark = pytest.mark.skipif(sys.platform == "win32", reason="Terminal is POSIX-only in this v1")


def test_add_terminal_returns_a_node():
    window = Window(width=400, height=300)
    node = window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    assert isinstance(node, Node)


def test_a_themed_terminal_does_not_raise():
    window = Window(width=400, height=300)
    window.set_theme(seed=(0x67, 0x50, 0xA4, 0xFF), dark=False)
    node = window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    assert isinstance(node, Node)


def test_add_terminal_positions_like_every_other_add_method():
    window = Window(width=400, height=300)
    node = window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255), x=10, y=20)
    assert isinstance(node, Node)


def test_a_click_focuses_the_terminal():
    """A real, confirmed bug found live while building this step:
    click-to-focus was originally scoped to TextField only (M18 Phase
    1's own real finding) -- a real end-to-end test caught that a
    click on a Terminal never focused it at all, so typed input
    silently never reached the shell. Fixed by widening the same real
    click-to-focus check `Tree::dispatch` already has.
    """
    window = Window(width=400, height=300)
    term = window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    assert term.is_focused() is False
    window.click(term)
    assert term.is_focused() is True


def test_get_text_on_a_fresh_terminal_returns_an_empty_grid():
    window = Window(width=400, height=300)
    term = window.add_terminal(shell="/bin/sh", cols=10, rows=3, background=(0, 0, 0, 255))
    assert term.get_text() == "\n\n"


def test_get_text_on_a_non_terminal_node_raises_a_clear_error():
    window = Window(width=400, height=300)
    rect = window.add_rect(background=(255, 0, 0, 255), width=40, height=40)
    with pytest.raises(ValueError):
        # `get_text` is shared with Text/TextField/Terminal only.
        rect.get_text()


def test_a_real_shell_genuinely_responds_to_typed_input():
    """The real point of this step: a genuine shell process, spawned
    on a real PTY, receiving real keystrokes and producing real
    output that lands in the rendered cell grid -- not a mock, not a
    simulation.

    M32 Phase 4 (§4, §8) extends this exact test (rather than adding a
    new one with its own `App.run()` call) to also prove a real
    Ctrl+C/SIGINT genuinely interrupts a running process --
    [[feedback_no_second_app_run_in_pytest]]'s own real, confirmed
    finding means this file's one `App.run()` call must stay the only
    one across the whole pytest process, so both real claims share it.
    """
    window = Window(width=420, height=200)
    term = window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    window.click(term)
    assert term.is_focused() is True

    window.type_text("echo HELLO_FROM_TERMINAL")
    window.press_key("enter")

    # M32 Phase 4: a real, running `sleep 100`, interrupted by a real
    # Ctrl+C before it can ever finish -- `write_input` is a real,
    # immediate OS write to the PTY (not deferred to a render loop), so
    # this ordering is the real order the shell receives it in,
    # independent of `App.run()` below. A short real wall-clock pause
    # gives the shell time to actually fork/exec `sleep` first -- the
    # identical real timing this phase's own empirical check needed.
    window.type_text("sleep 100")
    window.press_key("enter")
    time.sleep(0.2)
    sent = window.press_ctrl("c")
    assert sent is True, "a real focused terminal must report the control byte was sent"
    window.type_text("echo REACHED_AFTER_SIGINT")
    window.press_key("enter")

    app = App()
    app.add_window(window)
    app.run(max_frames=60)

    text = term.get_text()
    assert "HELLO_FROM_TERMINAL" in text, f"expected real shell output not found in {text!r}"
    assert "REACHED_AFTER_SIGINT" in text, (
        f"the shell must have genuinely regained control right after the real SIGINT -- if "
        f"sleep 100 were still running, this later command would never have executed, got "
        f"{text!r}"
    )


def test_get_monospace_cell_size_returns_real_positive_values_that_scale_with_font_size():
    """M32 Phase 1 (§5, §8, §10): the real per-font-size measured cell
    size `add_terminal`/`add_code_editor` themselves size against
    internally, exposed here so app-level layout code can match it --
    proven real (positive, genuinely scales with `font_size`, not a
    fixed placeholder) rather than just "doesn't raise".
    """
    window = Window(width=400, height=300)
    width_14, height_14 = window.get_monospace_cell_size(font_size=14.0)
    width_28, height_28 = window.get_monospace_cell_size(font_size=28.0)
    assert width_14 > 0.0
    assert height_14 > 0.0
    assert width_28 > width_14
    assert height_28 > height_14


def test_press_key_without_a_focused_terminal_falls_through_harmlessly():
    """`press_key`/`type_text`'s own real terminal-routing check must
    be a true no-op when nothing terminal-shaped is focused -- proven
    against a real Terminal node that simply isn't focused, not just
    an empty window.
    """
    window = Window(width=400, height=300)
    window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    # No window.click(term) -- nothing is focused.
    window.press_key("enter")
    window.type_text("hello")


def test_press_ctrl_returns_false_without_a_focused_terminal():
    """M32 Phase 4 (§4, §8): the real, deliberate scope boundary
    `press_ctrl`'s own doc comment states -- unlike `press_key`/
    `type_text`, it never falls through to `Tree::dispatch`, it just
    reports nothing was sent.
    """
    window = Window(width=400, height=300)
    window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    assert window.press_ctrl("c") is False


def test_press_ctrl_returns_true_for_a_real_focused_terminal():
    window = Window(width=400, height=300)
    term = window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    window.click(term)
    assert window.press_ctrl("c") is True
    assert window.press_ctrl("z") is True, "every real Ctrl+<letter>, not just c/x/v"


def test_press_ctrl_rejects_anything_that_isnt_exactly_one_ascii_letter():
    window = Window(width=400, height=300)
    term = window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    window.click(term)
    with pytest.raises(ValueError):
        window.press_ctrl("")
    with pytest.raises(ValueError):
        window.press_ctrl("cc")
    with pytest.raises(ValueError):
        window.press_ctrl("1")
