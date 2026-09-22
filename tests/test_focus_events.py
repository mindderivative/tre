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

from tre import Signal, View, ViewModel, Window


def test_window_focus_gives_a_one_arg_handler_a_real_event():
    window = Window(width=200, height=100)
    a = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40)

    events = []
    a.set_on_focus_enter(lambda event: events.append(event))

    window.focus(a)

    assert len(events) == 1
    event = events[0]
    assert event.kind == "focus_enter"
    assert event.position is None
    assert event.button is None
    assert event.old_value is None
    assert event.new_value is None


def test_focus_exit_fires_when_focus_moves_to_a_sibling():
    window = Window(width=200, height=100)
    a = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40)
    b = window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=40, height=40)

    calls = []
    a.set_on_focus_exit(lambda: calls.append("a exited"))
    b.set_on_focus_enter(lambda: calls.append("b entered"))

    window.focus(a)
    assert calls == []  # first-time focus onto `a` -- no exit yet, and `a` has no enter handler

    window.focus(b)
    assert calls == ["a exited", "b entered"]


def test_focusing_a_node_with_no_registered_handler_is_a_safe_no_op():
    window = Window(width=200, height=100)
    a = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    window.focus(a)  # must not raise


def test_refocusing_the_already_focused_node_does_not_report_a_stale_transition():
    window = Window(width=200, height=100)
    a = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    calls = []
    a.set_on_focus_enter(lambda: calls.append("entered"))

    window.focus(a)
    assert calls == ["entered"]

    window.focus(a)
    assert calls == ["entered"], "focusing an already-focused node must not fire a stale transition"


def test_real_click_to_focus_on_a_text_field_fires_focus_enter():
    """M18/M30/M53's own real click-to-focus mechanism -- now genuinely
    observable via a registered `FocusEnter` handler, not just `Node.
    is_focused()`.
    """
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xFF, 0xFF, 0xFF, 0xFF), width=100, height=30, content="hi")

    events = []
    field.set_on_focus_enter(lambda event: events.append(event))

    window.click(field)

    assert len(events) == 1
    assert events[0].kind == "focus_enter"


def test_real_right_click_to_focus_on_a_text_field_fires_focus_enter():
    """M55's own real, found-while-scoping fix: `Window.right_click`'s
    own `PointerPressed` dispatch used to discard its outcome with no
    variable at all, so a real right-click-to-focus (M53) was
    structurally unobservable from this entry point before now.
    """
    window = Window(width=200, height=100)
    field = window.add_text_field(background=(0xFF, 0xFF, 0xFF, 0xFF), width=100, height=30, content="hi")

    events = []
    field.set_on_focus_enter(lambda event: events.append(event))

    window.right_click(field)

    assert len(events) == 1
    assert events[0].kind == "focus_enter"


def test_real_tab_navigation_fires_focus_enter():
    window = Window(width=200, height=100)
    a = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)
    a.set_on_click(lambda: None)  # the real, established way to make a node Tab-reachable

    events = []
    a.set_on_focus_enter(lambda event: events.append(event))

    window.press_key("tab")

    assert len(events) == 1
    assert events[0].kind == "focus_enter"


def test_view_focus_actually_invokes_its_wired_handler(tmp_path):
    path = tmp_path / "view.yaml"
    path.write_text(
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
handlers: {on_focus_enter: "bump"}
"""
    )
    view = View(str(path))

    class VM(ViewModel):
        def __init__(self, view):
            self.focuses = Signal(0)
            super().__init__(view)

        def bump(self):
            self.focuses.update(lambda n: n + 1)

    vm = VM(view)
    node = view.node("root")

    view.focus(node)

    assert vm.focuses.get() == 1


def test_a_raising_focus_handler_is_caught_logged_and_non_fatal(capfd):
    """Matches `Window.click`/`Node.set_on_change`'s own established
    policy (§9): an uncaught exception from a real handler is caught
    and logged via `tracing::error!`, not propagated.
    """
    window = Window(width=200, height=100)
    a = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=40, height=40)

    def on_focus_enter():
        raise RuntimeError("boom from a focus handler")

    a.set_on_focus_enter(on_focus_enter)
    window.focus(a)  # must not raise

    captured = capfd.readouterr()
    assert "boom from a focus handler" in captured.err
