"""`focus`/`unfocus` listeners: the Event they get, focus moving between
siblings, refocusing firing nothing, focus from a click, a secondary click,
or Tab, and a raising listener being logged and non-fatal.
"""

from tre import Window
from helpers import add


def test_simulated_focus_gives_a_one_arg_listener_an_event():
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


def test_unfocus_fires_when_focus_moves_to_a_sibling():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40)
    b = add(window, "box", fill=(0x00, 0x00, 0xFF, 0xFF), width=40, height=40)

    calls = []
    a.on("unfocus", lambda: calls.append("a exited"))
    b.on("focus", lambda: calls.append("b entered"))

    window.simulate("focus", node=a)
    assert calls == []  # first focus onto `a` -- no unfocus yet, and `a` has no focus listener

    window.simulate("focus", node=b)
    assert calls == ["a exited", "b entered"]


def test_focusing_a_node_with_no_listener_is_a_safe_no_op():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    window.simulate("focus", node=a)  # must not raise


def test_refocusing_the_focused_node_fires_nothing():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    calls = []
    a.on("focus", lambda: calls.append("entered"))

    window.simulate("focus", node=a)
    assert calls == ["entered"]

    window.simulate("focus", node=a)
    assert calls == ["entered"], "focusing an already-focused node must not fire a stale transition"


def test_clicking_a_text_input_fires_focus():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=100, height=30, text="hi")

    events = []
    field.on("focus", lambda event: events.append(event))

    window.simulate("click", node=field)

    assert len(events) == 1
    assert events[0].type == "focus"


def test_a_secondary_click_on_a_text_input_fires_focus():
    window = Window(width=200, height=100)
    field = add(window, "text_input", width=100, height=30, text="hi")

    events = []
    field.on("focus", lambda event: events.append(event))

    window.simulate("secondary_click", node=field)

    assert len(events) == 1
    assert events[0].type == "focus"


def test_tab_navigation_fires_focus():
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40, focusable=True)

    events = []
    a.on("focus", lambda event: events.append(event))

    window.simulate("key_down", key="tab")

    assert len(events) == 1
    assert events[0].type == "focus"


def test_a_raising_focus_listener_is_caught_logged_and_non_fatal(capfd):
    """Like every listener, a raising one is logged via `tracing::error!`,
    not propagated."""
    window = Window(width=200, height=100)
    a = add(window, "box", fill=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    def on_focus_enter():
        raise RuntimeError("boom from a focus handler")

    a.on("focus", on_focus_enter)
    window.simulate("focus", node=a)  # must not raise

    captured = capfd.readouterr()
    assert "boom from a focus handler" in captured.err
