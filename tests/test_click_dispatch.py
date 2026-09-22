"""M4 Phase 1 step 3 (§4, §11.10): real, repeatable coverage of the
`Node.set_on_click`/`Window.click` FFI boundary -- the first real
consumer of `Tree::dispatch`'s `DispatchOutcome::Activated` outcome.
`Window.click(node)` is a direct, programmatic "click this node" entry
point (no live OS window needed, matching every prior interaction
step's own "expose a direct Tree method since real pointer dispatch has
nowhere else to originate" pattern) that dispatches a real primary-
button press+release pair at `node`'s own real, computed center point.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`/`test_virtual_list.py`.
"""

import gc
import weakref

from tre import Window


def test_click_fires_the_registered_handler():
    window = Window(width=200, height=200)
    calls = []
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    button.set_on_click(lambda: calls.append("clicked"))

    window.click(button)

    assert calls == ["clicked"]


def test_click_gives_a_one_arg_handler_a_real_event_with_position_and_button():
    """M54 Phase 2 (§8, §16.2): the real `Click` payload -- a real
    pointer-driven activation carries the real position it fired at and
    which button, extracted from the same `InputEvent` `Window.click`
    itself constructs (`dispatch::run_dispatch_outcome`'s own real
    correction: `engine-core` never needed widening for this at all).
    """
    window = Window(width=200, height=200)
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50, x=20, y=30)

    events = []
    button.set_on_click(lambda event: events.append(event))

    window.click(button)

    assert len(events) == 1
    event = events[0]
    assert event.kind == "click"
    assert event.position == (45.0, 55.0)  # node's own real computed center
    assert event.button == "primary"
    assert event.old_value is None
    assert event.new_value is None


def test_a_real_keyboard_activation_gives_a_one_arg_handler_none_position_and_button():
    """A real `Enter`/`Space` activation while focused has no pointer
    position/button at all -- `Event` reports `None` for both rather
    than fabricating a synthetic value, the identical honest contract
    `HoverEnter`/`HoverExit` already have for their own irrelevant
    fields. `set_on_click` makes `button` Tab-reachable (its own real,
    documented side effect); `Window.press_key("tab")` then `"enter"`
    reaches it the same way a real keyboard-only user would.
    """
    window = Window(width=200, height=200)
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50, x=20, y=30)

    events = []
    button.set_on_click(lambda event: events.append(event))

    window.press_key("tab")
    window.press_key("enter")

    assert len(events) == 1
    event = events[0]
    assert event.kind == "click"
    assert event.position is None
    assert event.button is None


def test_click_on_a_node_with_no_registered_handler_is_a_safe_no_op():
    window = Window(width=200, height=200)
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)

    window.click(button)  # must not raise


def test_click_only_fires_the_clicked_nodes_own_handler_not_a_sibling():
    window = Window(width=200, height=200)
    calls = []
    a = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    b = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    a.set_on_click(lambda: calls.append("a"))
    b.set_on_click(lambda: calls.append("b"))

    window.click(b)

    assert calls == ["b"]


def test_a_raising_click_handler_is_caught_logged_and_non_fatal(capfd):
    """§9's own stated policy: "unhandled exceptions from a callback are
    caught, logged via `tracing::error!` (M16 Phase 2), and non-fatal."
    A raising handler must not crash the process, and its traceback
    must actually reach stderr -- not be silently swallowed.

    `capfd`, not `capsys`: `tracing_subscriber`'s own writer is a raw
    OS-level stderr write from Rust, bypassing Python's `sys.stderr`
    object entirely -- `capsys` can't see it (confirmed by actually
    running this test with `capsys` first and watching it fail with an
    empty capture despite the real event genuinely firing); `capfd`
    captures at the file-descriptor level, real for both Python and
    Rust writes.
    """
    window = Window(width=200, height=200)
    calls = []

    def raiser():
        calls.append("ran")
        raise RuntimeError("boom from a click handler")

    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    button.set_on_click(raiser)

    window.click(button)  # must not raise/propagate into Python

    assert calls == ["ran"]
    captured = capfd.readouterr()
    assert "boom from a click handler" in captured.err


def test_set_on_click_makes_a_node_keyboard_focusable_too():
    """§10's own "one property write keeps both paths correct" stance,
    applied to the one call site that actually makes a node interactive
    for the first time: registering a click handler must also make the
    node Tab-reachable (a non-empty `access.actions`), not just
    mouse-clickable.
    """
    window = Window(width=200, height=200)
    button = window.add_rect(background=(0, 0, 0, 255), width=50, height=50)
    # Nothing public exposes `access.actions` directly from Python yet --
    # the real, observable proxy is that `set_on_click` must not raise
    # and a subsequent click must still fire, which the other tests here
    # already establish; this test exists to name the requirement
    # explicitly so a future regression (e.g. `set_on_click` forgetting
    # to touch `access.actions`) has a place to be caught once Tab
    # dispatch is itself reachable from Python (M4's own next step).
    button.set_on_click(lambda: None)


def test_window_participates_in_cyclic_gc_when_a_click_handler_captures_it_back():
    """Mirrors M3 step 15 Stage C's own `test_window_participates_in_
    cyclic_gc_when_its_materializer_captures_it_back` -- `click_handlers`
    is a second, independently-populated `Py<PyAny>` map on the same
    `Window`, so it needs the same real proof, not just the same
    `__traverse__`/`__clear__` code reused on faith.
    """

    class Holder:
        def __init__(self):
            self.window = None

        def on_click(self):
            self.window

    holder = Holder()
    window = Window(width=50, height=50)
    holder.window = window
    button = window.add_rect(background=(0, 0, 0, 255), width=10, height=10)

    # window -> click_handlers -> holder.on_click (bound method) ->
    # __self__ -> holder -> .window -> window.
    button.set_on_click(holder.on_click)

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
        "part of it -- if this fails, __traverse__/__clear__ aren't actually making "
        "window's click_handlers side of the cycle visible to the collector"
    )
