"""tre v2 -- Python-facing declarative/imperative GUI framework.

§14 step 14 (§11.1) splits `Window` back out of `App`, at exactly the
step every earlier module doc comment (Rust-side) predicted: `App`
collects one or more `Window`s and drives them all together in one
blocking `App.run()` call; each `Window` owns its own node tree and
size:

    win1 = Window(width=400, height=200, title="Main")
    win2 = Window(width=300, height=150, title="Panel")
    app = App()
    app.add_window(win1)
    app.add_window(win2)
    app.run()

§14 step 12 added the real §16.2 MVVM surface: `View` (loads a
`view.yaml`, `engine-py`'s Rust side), and `Signal`/`ViewModel` (pure
Python -- no reason for these to be Rust, since they never touch the
`Tree` directly; `View._attach` is the actual crossing point).

`Signal` implements the "evaluate once inside a recording scope,
subscribe to whatever was read" dependency tracking `View._attach`
relies on: `Signal.get()` calls `_core._record_read(self)`, a small
Rust-side function that -- only while a binding evaluation is actually
in progress -- appends `self` to that evaluation's dependency list.
Outside of an active `_attach()` call, `_record_read` is a no-op, so a
plain `signal.get()` in ordinary Python code costs one cheap call and
nothing else.
"""

from tre._core import App, Node, View, Window, _record_read


class Signal:
    """A minimal reactive value cell (§16.2). `.get()` records a
    dependency when read during a binding's evaluation; `.set()`/
    `.update()` notify every binding subscribed through that read.
    """

    def __init__(self, value):
        self._value = value
        self._subscribers = []

    def get(self):
        _record_read(self)
        return self._value

    def set(self, value):
        self._value = value
        self._notify()

    def update(self, fn):
        """Sets this signal's value to `fn(current_value)`, then
        notifies -- the idiomatic "read, transform, write" update, e.g.
        `clicks.update(lambda n: n + 1)`.
        """
        self._value = fn(self._value)
        self._notify()

    def _subscribe(self, callback):
        """Called from Rust (`View._attach`) -- registers a binding's
        own re-evaluation trigger. Not part of `Signal`'s own public
        API; an app author never calls this directly.
        """
        self._subscribers.append(callback)

    def _notify(self):
        for callback in self._subscribers:
            callback()


class ViewModel:
    """§16.2: "the ViewModel is what knows" who listens to a `View` and
    who updates it. Constructing one wires every declared handler and
    binding in the given `view` in a single call:

        view = View("counter.yaml")

        class CounterViewModel(ViewModel):
            def __init__(self, view):
                self.clicks = Signal(0)
                super().__init__(view)  # must run after clicks exists --
                                         # _attach evaluates every binding
                                         # immediately, so it needs to see
                                         # the real attributes it names.

            def bump(self, event):
                self.clicks.update(lambda n: n + 1)

        vm = CounterViewModel(view)
    """

    def __init__(self, view):
        self._view = view
        view._attach(self)


__all__ = ["App", "Node", "View", "Window", "Signal", "ViewModel"]
