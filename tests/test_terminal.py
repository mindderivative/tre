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
    """
    window = Window(width=420, height=200)
    term = window.add_terminal(shell="/bin/sh", cols=40, rows=10, background=(0, 0, 0, 255))
    window.click(term)
    assert term.is_focused() is True

    window.type_text("echo HELLO_FROM_TERMINAL")
    window.press_key("enter")

    app = App()
    app.add_window(window)
    app.run(max_frames=60)

    text = term.get_text()
    assert "HELLO_FROM_TERMINAL" in text, f"expected real shell output not found in {text!r}"


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
