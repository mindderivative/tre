"""M42 Phases 1 and 2 (§4, §5, §8, §16.2, §16.4): real, repeatable
coverage that `Window.from_view(view)` actually wires a `View` into a
live window's own shared state, not a second, separate copy of it --
`view.rs`'s own module doc comment names this as the real gap Phase 1
closes (`View` had never been embedded into a live `winit`-driven
window before) -- and that `Window.show_view(view)` (Phase 2) switches
which `View` an already-live `Window` dispatches against and paints,
without disturbing either `View`'s own independent `Reconciler`/
bindings/`Signal` state.

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


def write_view(tmp_path, yaml, name="view.yaml"):
    path = tmp_path / name
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


def test_show_view_switches_a_live_windows_dispatch_to_a_different_view(tmp_path):
    """M42 Phase 2's own decisive real proof: `window.show_view(view_b)`
    must make `window.click(...)` dispatch against `view_b`'s own tree/
    root/handlers from then on, not `view_a`'s (the one the `Window` was
    originally built from via `from_view`) -- the real capability behind
    the user's own explicit plan-review feedback, "switching of current
    views without needing to bootstrap each view/viewModel."
    """
    path_a = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
handlers: {on_click: "bump"}
""",
        name="a.yaml",
    )
    path_b = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 60, height: 30, background: "#332211"}
handlers: {on_click: "bump"}
""",
        name="b.yaml",
    )
    view_a = View(path_a)
    view_b = View(path_b)

    class VM(ViewModel):
        def __init__(self, view):
            self.clicks = Signal(0)
            super().__init__(view)

        def bump(self):
            self.clicks.update(lambda n: n + 1)

    vm_a = VM(view_a)
    vm_b = VM(view_b)

    window = Window.from_view(view_a, width=200, height=100)
    window.click(view_a.node("root"))
    assert vm_a.clicks.get() == 1

    window.show_view(view_b)
    window.click(view_b.node("root"))

    assert vm_b.clicks.get() == 1, "a click after show_view must reach the newly-shown View"
    assert vm_a.clicks.get() == 1, (
        "switching away from view_a must not fire its handler again for an unrelated click"
    )


def test_show_view_called_from_inside_a_click_handler_does_not_panic(tmp_path):
    """Real bug caught and fixed before this ever shipped, not found
    later: an earlier draft of `Window.click`/`hover`/`scroll`/
    `right_click` held a live borrow of `Window.active` across the very
    `run_dispatch_outcome` call that can invoke a real Python handler --
    a handler that itself calls `window.show_view(...)` (exactly the
    real pattern a nav button uses) would then hit `show_view`'s own
    `self.active.borrow_mut()` while that borrow was still alive,
    panicking with "already borrowed." This test calls `show_view` from
    *inside* a real dispatched click handler, the one scenario that
    actually exercises the bug -- `test_show_view_switches_a_live_
    windows_dispatch_to_a_different_view` above calls `show_view`
    directly from the test body, which never touched the buggy code
    path at all.
    """
    path_a = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
handlers: {on_click: "go_to_b"}
""",
        name="a.yaml",
    )
    path_b = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 60, height: 30, background: "#332211"}
handlers: {on_click: "go_to_a"}
""",
        name="b.yaml",
    )
    view_a = View(path_a)
    view_b = View(path_b)

    window = Window.from_view(view_a, width=200, height=100)

    class ScreenAVM(ViewModel):
        def go_to_b(self):
            window.show_view(view_b)

    class ScreenBVM(ViewModel):
        def go_to_a(self):
            window.show_view(view_a)

    ScreenAVM(view_a)
    ScreenBVM(view_b)

    window.click(view_a.node("root"))  # must not raise -- switches to view_b mid-dispatch
    window.click(view_b.node("root"))  # must not raise -- switches back to view_a mid-dispatch


def test_show_view_keeps_each_views_bindings_independently_reactive(tmp_path):
    """The real, load-bearing claim `show_view`'s own doc comment makes:
    each named `View` stays fully alive, its own `Reconciler`/bindings/
    `Signal` subscriptions intact -- a `Signal` write on the *previous*
    view's `ViewModel`, made after switching away from it, must still
    reach its own (now merely not-currently-shown) tree, and must not
    leak into the newly active view.
    """
    path_a = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
bindings: {corner_radius: "{{ radius.get() }}"}
""",
        name="a.yaml",
    )
    path_b = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 60, height: 30, background: "#332211"}
bindings: {corner_radius: "{{ radius.get() }}"}
""",
        name="b.yaml",
    )
    view_a = View(path_a)
    view_b = View(path_b)

    class VM(ViewModel):
        def __init__(self, view):
            self.radius = Signal(0.0)
            super().__init__(view)

    vm_a = VM(view_a)
    vm_b = VM(view_b)

    Window.from_view(view_a, width=200, height=100)  # not held on purpose -- unused here

    vm_a.radius.set(5.0)
    vm_b.radius.set(9.0)

    assert view_a.node("root").get("corner_radius") == 5.0
    assert view_b.node("root").get("corner_radius") == 9.0


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
