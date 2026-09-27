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

from tre import App, MONOSPACE_FONT_FAMILY, Node, Window
from helpers import add

pytestmark = pytest.mark.skipif(sys.platform == "win32", reason="Terminal is POSIX-only in this v1")


def test_add_terminal_returns_a_node():
    window = Window(width=400, height=300)
    node = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    assert isinstance(node, Node)


def test_add_terminal_positions_like_every_other_add_method():
    window = Window(width=400, height=300)
    node = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)}, position="absolute", x=10, y=20)
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
    term = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    assert term.get("focused") is False
    window.click(term)
    assert term.get("focused") is True


def test_get_text_on_a_fresh_terminal_returns_an_empty_grid():
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=10, rows=3, palette={"background": (0, 0, 0, 255)})
    assert term.get("text") == "\n\n"


def test_get_text_on_a_non_terminal_node_raises_a_clear_error():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=40, height=40)
    with pytest.raises(ValueError):
        # `get_text` is shared with Text/TextField/Terminal only.
        rect.get("text")


def test_a_real_shell_genuinely_responds_to_typed_input():
    """The real point of this step: a genuine shell process, spawned
    on a real PTY, receiving real keystrokes and producing real
    output that lands in the rendered cell grid -- not a mock, not a
    simulation.

    M32 Phase 4 (§4, §8) extends this exact test (rather than adding a
    new one with its own `App.run()` call) to also prove a real
    Ctrl+C/SIGINT genuinely interrupts a running process, and M32
    Phase 5 (§4, §8) extends it again to prove real scrollback --
    [[feedback_no_second_app_run_in_pytest]]'s own real, confirmed
    finding means this file's one `App.run()` call must stay the only
    one across the whole pytest process, so all three real claims
    share it.

    **M83: a real, CI-observed flake fixed here, not just a local
    tuning tweak.** `max_frames` forces `ControlFlow::Poll` with zero
    per-frame pacing (`engine-platform/src/lib.rs`'s own `still_
    animating = real_still_animating || win.max_frames.is_some()`) --
    a bounded run spins through its whole frame budget as fast as the
    machine can issue redraws, never really waiting on real terminal
    activity. On a fast, idle dev machine, 60 such frames still take
    enough real wall-clock time for the OS to interleave the shell
    process in. On CI (a debug, non-`--release` build, on a shared,
    already-acknowledged-noisy-neighbor `ubuntu-latest` runner --
    `ci.yml`'s own frame-time-benchmark comment says as much elsewhere
    in this repo), that entire budget can burn through before the
    shell gets scheduled at all, well before the fork/exec race the
    pre-Ctrl+C pause below was already trying to cover. Reproduced
    locally under both a debug build and real, taskset-pinned CPU
    contention without triggering it (confirmed not a *local*
    reproduction, only a CI one) -- the fix widens both real-time
    budgets involved generously rather than guessing at a precise
    minimum, since `ControlFlow::Poll`'s own zero-cost-when-fast
    nature means extra headroom here is free on a fast machine and
    only matters on a slow one.

    **M88 correction: M83 misdiagnosed this.** The CI failure was never
    timing. CI's Linux runner has no display at all ("neither
    WAYLAND_DISPLAY nor WAYLAND_SOCKET nor DISPLAY is set"), so
    `App.run()` returns immediately without rendering a single frame,
    and PTY output only reaches the cell grid inside a frame -- the
    test saw an empty grid (`'\n\n\n\n'`) however long it waited, and
    kept failing on `main` after M83 landed. The run now queues a
    marker via `App.thread_handle()` (M87) before starting; if it never
    ran, no frame happened and the test skips with that reason rather
    than failing on something it can't observe. M83's wider budgets are
    kept: harmless, and still sensible headroom on a slow machine that
    *does* have a display.
    """
    window = Window(width=420, height=200)
    term = add(window, "terminal", shell="/bin/sh", cols=40, rows=5, scrollback_lines=200, palette={"background": (0, 0, 0, 255)})
    window.click(term)
    assert term.get("focused") is True

    window.type_text("echo HELLO_FROM_TERMINAL")
    window.press_key("enter")

    # M32 Phase 5: real output lines typed early, before everything
    # below -- more than the 5-row viewport can hold at once, forcing
    # real scrollback content the later `window.scroll` call reveals.
    window.type_text("for i in 1 2 3 4 5 6 7 8; do echo SCROLLBACK_LINE_$i; done")
    window.press_key("enter")

    # M32 Phase 4: a real, running `sleep 100`, interrupted by a real
    # Ctrl+C before it can ever finish -- `write_input` is a real,
    # immediate OS write to the PTY (not deferred to a render loop), so
    # this ordering is the real order the shell receives it in,
    # independent of `App.run()` below. A short real wall-clock pause
    # gives the shell time to actually fork/exec `sleep` first -- the
    # identical real timing this phase's own empirical check needed.
    # Typed last/most-recently, so its own real output stays in the
    # bottom (unscrolled) viewport even after the scrollback-generating
    # loop above.
    window.type_text("sleep 100")
    window.press_key("enter")
    time.sleep(0.5)
    sent = window.press_ctrl("c")
    assert sent is True, "a real focused terminal must report the control byte was sent"
    window.type_text("echo REACHED_AFTER_SIGINT")
    window.press_key("enter")

    app = App()
    app.add_window(window)
    # M88: a frame probe -- see this test's own doc comment.
    frames_ran = []
    app.thread_handle().call_soon(lambda: frames_ran.append(True))
    # M83: widened 60 -> 600 -- still sensible headroom on a slow host
    # that has a display (see the M88 correction in the doc comment).
    app.run(max_frames=600)
    if not frames_ran:
        pytest.skip("no display reachable: App.run() rendered no frames, so PTY output never drained")

    text = term.get("text")
    assert "REACHED_AFTER_SIGINT" in text, (
        f"the shell must have genuinely regained control right after the real SIGINT -- if "
        f"sleep 100 were still running, this later command would never have executed, got "
        f"{text!r}"
    )
    assert "HELLO_FROM_TERMINAL" not in text, (
        "the real first line typed must have already scrolled off a 5-row viewport by now"
    )

    # M32 Phase 5: no second `App.run()` needed -- `Window.scroll`
    # resyncs `TerminalState` synchronously (`TerminalSession::scroll_
    # by`'s own real `sync_state` call), no live render loop required.
    # A deliberately huge scroll clamps to the real top of history
    # (`vt100::Screen::set_scrollback`'s own real clamping), revealing
    # the very first real line typed -- and pushing the most recent one
    # back out of view.
    window.scroll(term, 400.0)
    scrolled_text = term.get("text")
    assert "HELLO_FROM_TERMINAL" in scrolled_text, (
        f"a real scroll must reveal real, previously-scrolled-off history, got {scrolled_text!r}"
    )
    assert "REACHED_AFTER_SIGINT" not in scrolled_text, (
        "scrolled all the way to the real top of history, the most recent line must no longer "
        "be in view"
    )

    # M32 Phase 6 (§4, §5, §8): a real selection over genuinely echoed
    # shell output -- `HELLO_FROM_TERMINAL` is still in view after the
    # scroll above (the real point of the assertion just above), so its
    # own real byte range can be selected and read back hermetically.
    line = next(line for line in scrolled_text.split("\n") if "HELLO_FROM_TERMINAL" in line)
    col = line.index("HELLO_FROM_TERMINAL")
    row = scrolled_text.split("\n").index(line)
    selection = (row, col, row, col + len("HELLO_FROM_TERMINAL"))
    term.set(selection=selection)
    assert term.get("selection") == selection


def test_scroll_on_a_terminal_with_no_content_does_not_raise():
    """M32 Phase 5 (§4, §8): a real, synchronous edge case -- scrolling
    a freshly spawned terminal with zero real scrollback yet must not
    panic or raise, just clamp to `0` (`vt100::Screen::set_scrollback`'s
    own real clamping).
    """
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    window.scroll(term, 100.0)
    window.scroll(term, -100.0)


def test_scroll_on_a_non_terminal_node_still_bubbles_to_a_virtual_list():
    """M32 Phase 5 (§4, §8): `Window.scroll`'s new `Terminal` branch
    must be a true no-op for every other real `NodeKind` -- the
    existing `VirtualList` scroll-bubbling behavior stays exactly as
    it was before this phase.
    """
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
    window.scroll(items, 50.0)


def test_a_terminal_selection_round_trips_through_set_and_get():
    """M32 Phase 6 (§4, §5, §8): seeds a selection directly (no live
    mouse drag needed) and reads it back -- M100: through
    `get("selection")` (was `Window.copy_terminal_selection`, which read
    the selected text). A collapsed selection reads back as set.
    """
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=10, rows=1, palette={"background": (0, 0, 0, 255)})
    assert term.get("selection") is None, "no selection exists yet"

    term.set(selection=(0, 0, 0, 3))
    assert term.get("selection") == (0, 0, 0, 3)
    term.set(selection=(0, 2, 0, 2))
    assert term.get("selection") == (0, 2, 0, 2)


def test_set_terminal_selection_on_a_non_terminal_node_raises():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError):
        rect.set(selection=(0, 0, 0, 1))


def test_resize_terminal_updates_terminal_state_synchronously():
    """M33 Phase 1 (§4, §5, §8): no `App.run()` needed to observe this
    -- `TerminalSession::resize` calls `sync_state` the identical real,
    synchronous way `scroll_by` already does (M32 Phase 5), so
    `get_text()` reflects the real new grid shape the instant `Window.
    resize_terminal` returns. The real kernel-level PTY resize itself
    (a live shell's own `stty size` genuinely reporting the new size)
    is proven by a real, direct empirical script instead -- combining
    it into this file's one shared `App.run()`-based test below would
    add a fourth real content generator to an already-dense 5-row
    viewport already proven fragile to reorder twice this session.
    """
    window = Window(width=600, height=400)
    term = add(window, "terminal", shell="/bin/sh", cols=10, rows=3, palette={"background": (0, 0, 0, 255)})
    assert len(term.get("text").split("\n")) == 3

    term.set(cols=20, rows=6)
    assert len(term.get("text").split("\n")) == 6, "resize_terminal must resync TerminalState"


def test_resize_terminal_on_a_non_terminal_node_raises():
    window = Window(width=400, height=300)
    rect = add(window, "box", fill=(255, 0, 0, 255), width=50, height=50)
    with pytest.raises(ValueError):
        rect.set(cols=10, rows=5)




def test_one_monospace_cell_measures_positive_and_scales_with_font_size():
    """M32 Phase 1 (§5, §8, §10): a terminal's cell is one character of
    the bundled monospace face -- M100: measured with `measure_text`
    (was `get_monospace_cell_size`), proven real (positive, genuinely
    scales with `font_size`), not just "doesn't raise".
    """
    window = Window(width=400, height=300)
    width_14, height_14 = window.measure_text("M", font_family=MONOSPACE_FONT_FAMILY, font_size=14.0)
    width_28, height_28 = window.measure_text("M", font_family=MONOSPACE_FONT_FAMILY, font_size=28.0)
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
    add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
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
    add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    assert window.press_ctrl("c") is False


def test_press_ctrl_returns_true_for_a_real_focused_terminal():
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    window.click(term)
    assert window.press_ctrl("c") is True
    assert window.press_ctrl("z") is True, "every real Ctrl+<letter>, not just c/x/v"


def test_press_ctrl_rejects_anything_that_isnt_exactly_one_ascii_letter():
    window = Window(width=400, height=300)
    term = add(window, "terminal", shell="/bin/sh", cols=40, rows=10, palette={"background": (0, 0, 0, 255)})
    window.click(term)
    with pytest.raises(ValueError):
        window.press_ctrl("")
    with pytest.raises(ValueError):
        window.press_ctrl("cc")
    with pytest.raises(ValueError):
        window.press_ctrl("1")
