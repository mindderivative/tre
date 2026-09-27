"""M55 (§10, §16.2): real, repeatable coverage that `FocusEnter`/
`FocusExit` actually reach a registered Python handler -- the new
`EventKind`/`DispatchOutcome::FocusChanged` mechanism, mirroring
`test_hover_events.py`'s own real coverage of the identical `Hover`
transition shape.

`Window.focus(node)`/`View.focus(node)` (both new, M55) are the direct,
no-live-window-needed way to test a real focus transition without
relying on Tab-order or click-to-focus side effects -- the same real
proof pattern `Window.click`/`.hover` already established for `Click`/
`Hover`.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

from tre import Window
from helpers import add


def test_window_focus_gives_a_one_arg_handler_a_real_event():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40)

    events = []
    a.on("focus", lambda event: events.append(event))

    window.simulate("focus", node=a)

    assert len(events) == 1
    event = events[0]
    assert event.type == "focus"
    assert event.button is None
    assert event.old_value is None
    assert event.new_value is None


def test_focus_exit_fires_when_focus_moves_to_a_sibling():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40)
    b = add(window, "box", fill=(0x00, 0x00, 0xFF, 0xFF), width=40, height=40)

    calls = []
    a.on("unfocus", lambda: calls.append("a exited"))
    b.on("focus", lambda: calls.append("b entered"))

    window.simulate("focus", node=a)
    assert calls == []  # first-time focus onto `a` -- no exit yet, and `a` has no enter handler

    window.simulate("focus", node=b)
    assert calls == ["a exited", "b entered"]


def test_focusing_a_node_with_no_registered_handler_is_a_safe_no_op():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    window.simulate("focus", node=a)  # must not raise


def test_refocusing_the_already_focused_node_does_not_report_a_stale_transition():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    calls = []
    a.on("focus", lambda: calls.append("entered"))

    window.simulate("focus", node=a)
    assert calls == ["entered"]

    window.simulate("focus", node=a)
    assert calls == ["entered"], "focusing an already-focused node must not fire a stale transition"


def test_real_click_to_focus_on_a_text_field_fires_focus_enter():
    """M18/M30/M53's own real click-to-focus mechanism -- now genuinely
    observable via a registered `FocusEnter` handler, not just `Node.
    is_focused()`.
    """
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=100, height=30, text="hi")

    events = []
    field.on("focus", lambda event: events.append(event))

    window.simulate("click", node=field)

    assert len(events) == 1
    assert events[0].type == "focus"


def test_real_right_click_to_focus_on_a_text_field_fires_focus_enter():
    """M55's own real, found-while-scoping fix: `Window.right_click`'s
    own `PointerPressed` dispatch used to discard its outcome with no
    variable at all, so a real right-click-to-focus (M53) was
    structurally unobservable from this entry point before now.
    """
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=100, height=30, text="hi")

    events = []
    field.on("focus", lambda event: events.append(event))

    window.simulate("secondary_click", node=field)

    assert len(events) == 1
    assert events[0].type == "focus"


def test_real_tab_navigation_fires_focus_enter():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40, focusable=True)

    events = []
    a.on("focus", lambda event: events.append(event))

    window.simulate("key_down", key="tab")

    assert len(events) == 1
    assert events[0].type == "focus"


def test_a_raising_focus_handler_is_caught_logged_and_non_fatal(capfd):
    """Matches `Window.click`/`Node.set_on_change`'s own established
    policy (§9): an uncaught exception from a real handler is caught
    and logged via `tracing::error!`, not propagated.
    """
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    def on_focus_enter():
        raise RuntimeError("boom from a focus handler")

    a.on("focus", on_focus_enter)
    window.simulate("focus", node=a)  # must not raise

    captured = capfd.readouterr()
    assert "boom from a focus handler" in captured.err
