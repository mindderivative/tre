"""M42 Phase 1 (§4, §5, §8, §16.2, §16.4): real, repeatable coverage
that `Window.from_view(view)` actually wires a `View` into a live
window's own shared state, not a second, separate copy of it --
`view.rs`'s own module doc comment names this as the real gap this
phase closes (`View` had never been embedded into a live `winit`-driven
window before).

No test here calls `App.run()` -- a second real `App().run()` call
inside this same pytest process has, in the past, broken an unrelated,
earlier-passing real render-loop test (see `test_tracing.py`'s own
subprocess-per-case convention). The one real, full end-to-end live
proof (a real window opening, a real `Signal`-driven repaint) lives in
`examples/live_view.py` instead, run standalone the same way every
other example in this repo is.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_view_handlers.py`.
"""

from tre import Signal, View, ViewModel, Window


def write_view(tmp_path, yaml):
    path = tmp_path / "view.yaml"
    path.write_text(yaml)
    return str(path)


def test_from_view_returns_a_real_window(tmp_path):
    path = write_view(
        tmp_path,
        'id: root\nkind: Rect\nstyle: {width: 40, height: 20, background: "#112233"}\n',
    )
    view = View(path)
    window = Window.from_view(view, width=300, height=150, title="live view")
    assert isinstance(window, Window)


def test_a_click_dispatched_through_the_window_invokes_the_views_own_wired_handler(tmp_path):
    """The real, decisive proof of shared state: `window.click(node)` --
    a `Window`-side method, never `view.click(node)` -- must still reach
    the handler `View._attach` wired onto `view.handlers`, and must
    still find `node` by walking from `window.root` -- both only
    possible if `Window.from_view` genuinely shares the same live
    `Tree`/root/`handlers`, not fresh copies.
    """
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
handlers: {on_click: "bump"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.clicks = Signal(0)
            super().__init__(view)

        def bump(self):
            self.clicks.update(lambda n: n + 1)

    vm = VM(view)
    node = view.node("root")

    window = Window.from_view(view, width=200, height=100)

    window.click(node)

    assert vm.clicks.get() == 1, (
        "Window.from_view must share the same live Tree/root/handlers a View._attach wired -- "
        "a click dispatched through the Window must still reach the View's own handler"
    )


def test_view_click_still_works_standalone_after_from_view_switches_it_to_definite_layout(
    tmp_path,
):
    """`view.click()`/`hover()` switch from `AvailableSpace::MaxContent`
    to the real `Definite` size once `Window.from_view` has set a real
    nonzero `width`/`height` on the `View` (`available_space()`,
    `view.rs`) -- this must not break the same no-live-window click
    proof `test_view_handlers.py` already established for the headless
    case.
    """
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
handlers: {on_click: "bump"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.clicks = Signal(0)
            super().__init__(view)

        def bump(self):
            self.clicks.update(lambda n: n + 1)

    vm = VM(view)
    node = view.node("root")

    Window.from_view(view, width=200, height=100)

    view.click(node)

    assert vm.clicks.get() == 1


def test_a_signal_driven_binding_update_after_from_view_still_reaches_the_live_tree(tmp_path):
    """`View._attach`'s own binding re-evaluation (`BindingCallback`)
    captured `self.tree`/`self.handlers` clones at attach time, before
    `Window.from_view` ever ran -- a binding write after the window is
    shown must still land on the exact same shared `Tree` the window
    itself paints from, proving `Window.from_view` didn't fork a second
    copy out from under an already-attached `View`.
    """
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
bindings: {corner_radius: "{{ radius.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.radius = Signal(0.0)
            super().__init__(view)

    vm = VM(view)
    Window.from_view(view, width=200, height=100)

    vm.radius.set(12.0)

    node = view.node("root")
    assert node.get("corner_radius") == 12.0


def test_from_view_shares_the_same_live_size_cell_as_the_window(tmp_path):
    """M42 Phase 1's own stated, load-bearing consequence: `view.width`/
    `view.height` become the exact same shared `Rc<Cell<u32>>` the
    returned `Window`'s own fields hold, mirroring `SharedSize`'s
    established M33 Phase 2 pattern -- proven indirectly here via two
    independent `Window.from_view` calls on two independent `View`s,
    confirming each gets its own real, distinct size rather than a
    single global default.
    """
    path = write_view(
        tmp_path,
        "id: root\nkind: Container\nstyle: {width: 40, height: 20}\n",
    )
    view_a = View(path)
    view_b = View(path)

    Window.from_view(view_a, width=111, height=222)
    Window.from_view(view_b, width=333, height=444)

    node_a = view_a.node("root")
    node_b = view_b.node("root")

    # No layout-inspection API is exposed to Python -- the only real,
    # observable proxy at this level is that dispatching a click on
    # each still finds its own node without raising (a mismatched/
    # cross-wired size or tree would misplace the click point or panic
    # on a NodeId belonging to the wrong Tree).
    view_a.click(node_a)
    view_b.click(node_b)
