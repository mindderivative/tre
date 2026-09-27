"""`click` listeners driven by `window.simulate("click", ...)` and by
keyboard activation: the Event's position and button, sibling isolation, a
raising listener being logged and non-fatal, and cyclic GC through a
listener.
"""

import gc
import weakref

from tre import Window
from helpers import add


def test_click_fires_the_registered_handler():
    window = Window(width=200, height=200)
    calls = []
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    button.on("click", lambda: calls.append("clicked"))

    window.simulate("click", node=button)

    assert calls == ["clicked"]


def test_click_gives_a_one_arg_listener_the_position_and_button():
    """A pointer click carries the window position it fired at (the
    node's center) and the button."""
    window = Window(width=200, height=200)
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, position="absolute", x=20, y=30)

    events = []
    button.on("click", lambda event: events.append(event))

    window.simulate("click", node=button)

    assert len(events) == 1
    event = events[0]
    assert event.type == "click"
    assert (event.window_x, event.window_y) == (45.0, 55.0)  # the node's center
    assert event.button == "primary"
    assert event.old_value is None
    assert event.new_value is None


def test_a_keyboard_activation_gives_no_position_or_button():
    """An Enter activation has no pointer, so position and button read
    `None`. `focusable=True` puts the box in the Tab order."""
    window = Window(width=200, height=200)
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50, position="absolute", x=20, y=30, focusable=True)

    events = []
    button.on("click", lambda event: events.append(event))

    window.simulate("key_down", key="tab")
    window.simulate("key_down", key="enter")

    assert len(events) == 1
    event = events[0]
    assert event.type == "click"
    assert event.window_x is None
    assert event.button is None


def test_click_on_a_node_with_no_registered_handler_is_a_safe_no_op():
    window = Window(width=200, height=200)
    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)

    window.simulate("click", node=button)  # must not raise


def test_click_only_fires_the_clicked_nodes_own_handler_not_a_sibling():
    window = Window(width=200, height=200)
    calls = []
    a = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    b = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    a.on("click", lambda: calls.append("a"))
    b.on("click", lambda: calls.append("b"))

    window.simulate("click", node=b)

    assert calls == ["b"]


def test_a_raising_click_handler_is_caught_logged_and_non_fatal(capfd):
    """A raising listener is caught, logged via `tracing::error!` to
    stderr, and non-fatal. `capfd`, not `capsys`: the log is written from
    Rust straight to the stderr file descriptor.
    """
    window = Window(width=200, height=200)
    calls = []

    def raiser():
        calls.append("ran")
        raise RuntimeError("boom from a click handler")

    button = add(window, "box", fill=(0, 0, 0, 255), width=50, height=50)
    button.on("click", raiser)

    window.simulate("click", node=button)  # must not raise/propagate into Python

    assert calls == ["ran"]
    captured = capfd.readouterr()
    assert "boom from a click handler" in captured.err


def test_window_participates_in_cyclic_gc_when_a_click_handler_captures_it_back():
    """A bound-method listener that holds its window forms a cycle the
    cyclic collector must reclaim (`PyWindow`'s `__traverse__`/`__clear__`).
    """

    class Holder:
        def __init__(self):
            self.window = None

        def on_click(self):
            self.window

    holder = Holder()
    window = Window(width=50, height=50)
    holder.window = window
    button = add(window, "box", fill=(0, 0, 0, 255), width=10, height=10)

    # window -> listeners -> holder.on_click (bound method) ->
    # __self__ -> holder -> .window -> window.
    button.on("click", holder.on_click)

    holder_ref = weakref.ref(holder)
    del window
    del holder
    del button
    assert holder_ref() is not None, (
        "sanity check: a real reference cycle must survive plain refcounting alone "
        "(if this fails, the test itself isn't constructing a real cycle)"
    )

    gc.collect()
    assert holder_ref() is None, (
        "the cycle (window <-> bound-method click handler <-> holder) must be "
        "collected by CPython's cyclic GC once nothing outside it references any "
        "part of it -- if this fails, __traverse__/__clear__ aren't making "
        "the window's listeners visible to the collector"
    )
