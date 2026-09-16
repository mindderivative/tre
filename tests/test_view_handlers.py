"""M4 Phase 4 (§16.2): real, repeatable coverage that `View`'s
declarative `on_click` handlers actually reach real dispatch now, not
just eager validation. `test_view_binding.py`'s own
`test_attach_succeeds_when_the_handler_method_exists` already proved
`_attach` validates a handler correctly -- it never proved clicking
did anything, because until this phase nothing wired the validated
method to `Tree::dispatch` at all.

`View.click(node)` is the same no-live-window-needed proof pattern
`Window.click` (M4 Phase 1 step 3) already established -- a real
`Tree::dispatch` press+release pair at the node's own computed center,
not a direct call to the handler.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

from tre import Signal, View, ViewModel


def write_view(tmp_path, yaml):
    path = tmp_path / "view.yaml"
    path.write_text(yaml)
    return str(path)


def test_clicking_a_view_node_actually_invokes_its_wired_on_click_handler(tmp_path):
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

    view.click(node)

    assert vm.clicks.get() == 1, "a real dispatched click must actually call the bound method"


def test_repeated_clicks_each_invoke_the_handler_again(tmp_path):
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

    view.click(node)
    view.click(node)
    view.click(node)

    assert vm.clicks.get() == 3


def test_a_non_click_handler_still_validates_but_a_click_does_not_invoke_it(tmp_path):
    """Only `on_click` is wired to real dispatch today (M4 Phase 6 is
    where a general `EventKind` would generalize this) -- a declared
    `on_change` handler must still validate eagerly at `_attach()` time
    (unchanged, pre-existing behavior) but clicking the node must not
    call it, since no real mechanism reaches it yet.
    """
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 40, height: 20, background: "#112233"}
handlers: {on_change: "on_change"}
""",
    )
    view = View(path)

    calls = []

    class VM(ViewModel):
        def on_change(self):
            calls.append("called")

    VM(view)  # must not raise -- validation still runs
    node = view.node("root")

    view.click(node)

    assert calls == [], "on_change has no real dispatch mechanism yet and must not fire on a click"


def test_a_raising_view_handler_is_caught_logged_and_non_fatal(capsys, tmp_path):
    """Matches `Window.click`'s own established policy (§9): an
    uncaught exception from a real handler is caught and printed, not
    propagated -- `dispatch::run_activation` is the same shared
    mechanism both paths already use.
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
        def bump(self):
            raise RuntimeError("boom")

    VM(view)
    node = view.node("root")

    view.click(node)  # must not raise

    captured = capsys.readouterr()
    assert "boom" in captured.err
    assert "RuntimeError" in captured.err
