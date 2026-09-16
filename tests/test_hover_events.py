"""M4 Phase 6 (§7.3, §16.2): real, repeatable coverage that
`HoverEnter`/`HoverExit` actually reach a registered Python handler --
the new `EventKind`/`DispatchOutcome::HoverChanged` mechanism, and the
generalized `(NodeId, EventKind)`-keyed handler storage that replaced
`click_handlers`.

`Window.hover(node)`/`View.hover(node)` mirror `.click()`'s own
no-live-window-needed proof pattern (M4 Phase 1 step 3): a real
`Tree::dispatch` `PointerMoved` at the node's own computed center, not
a direct call to the handler.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

from tre import Signal, View, ViewModel, Window


def test_hover_enter_fires_when_the_pointer_arrives_on_the_node():
    window = Window(width=120, height=60)
    button = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)

    calls = []
    button.set_on_hover_enter(lambda: calls.append("entered"))

    window.hover(button)

    assert calls == ["entered"]


def test_hover_exit_fires_when_the_pointer_leaves_to_a_sibling():
    window = Window(width=120, height=60)
    a = window.add_rect(background=(0xFF, 0x00, 0x00, 0xFF), width=40, height=40)
    b = window.add_rect(background=(0x00, 0x00, 0xFF, 0xFF), width=40, height=40)

    calls = []
    a.set_on_hover_exit(lambda: calls.append("a exited"))
    b.set_on_hover_enter(lambda: calls.append("b entered"))

    window.hover(a)
    assert calls == []  # first-time entry onto `a` -- no exit yet, and `a` has no enter handler

    window.hover(b)
    assert calls == ["a exited", "b entered"]


def test_hovering_a_node_with_no_registered_handler_is_a_safe_no_op():
    window = Window(width=120, height=60)
    button = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)

    window.hover(button)  # must not raise


def test_hover_fires_even_without_calling_enable_interaction():
    """§7.3's own text: HoverEnter/HoverExit fires through the ordinary
    handler path independent of whether the default MD3 visual (the
    opt-in animation from M4 Phase 5's `enable_interaction`) is enabled
    -- the callback and the visual are two separate mechanisms.
    """
    window = Window(width=120, height=60)
    button = window.add_rect(background=(0xFF, 0xFF, 0xFF, 0xFF), width=80, height=40)

    calls = []
    button.set_on_hover_enter(lambda: calls.append("entered"))
    # No enable_interaction() call here on purpose.

    window.hover(button)

    assert calls == ["entered"]


def test_view_hover_enter_actually_invokes_its_wired_handler(tmp_path):
    path = tmp_path / "view.yaml"
    path.write_text(
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
handlers: {on_hover_enter: "bump"}
"""
    )
    view = View(str(path))

    class VM(ViewModel):
        def __init__(self, view):
            self.hovers = Signal(0)
            super().__init__(view)

        def bump(self):
            self.hovers.update(lambda n: n + 1)

    vm = VM(view)
    node = view.node("root")

    view.hover(node)

    assert vm.hovers.get() == 1
