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

M45 (§16.2): `Computed`/`Effect`/`batch`/`untrack` are the richer
reactivity layer built on top of `Signal`'s own dependency-recording
primitive -- `_core._begin_recording`/`_end_recording` (the Rust-side
stack `_record_read` pushes onto) are the *same* mechanism `View.
_attach` already uses for `{{ }}` bindings, exposed here so pure-Python
code can open its own tracked evaluation scope the identical way. Both
`Computed` and `Effect` duck-type against `Signal`'s own `_subscribe`/
`_unsubscribe` shape -- `View._attach`'s own real subscribe call site
(`signal.call_method1("_subscribe", ...)`) never type-checks its
target, so a `{{ }}` binding can point straight at a `Computed.get()`
value with zero Rust changes.
"""

from tre._core import (
    App,
    CanvasContext,
    Component,
    Node,
    View,
    Window,
    _begin_recording,
    _end_recording,
    _record_read,
)

#: M32 Phase 1 (§5, §8, §10): the real bundled monospace face
#: `Window.add_terminal`/`add_code_editor` themselves always shape
#: with internally (`engine_render::MONOSPACE_FONT_FAMILY`, "Hack
#: Nerd Font Mono") -- exported here so app-composed siblings (a
#: gutter's own `Text` node, a fold toggle) that must line up with the
#: real editor grid can match its exact real font_family rather than
#: guessing or drifting out of sync with it.
MONOSPACE_FONT_FAMILY = "Hack Nerd Font Mono"

# M45 (§16.2): module-global, not `threading.local()` -- this engine's
# whole render/event/dispatch loop runs on one thread (confirmed by
# reading `engine-py::app.rs`'s real per-frame closures: nothing here
# spawns a Python-visible thread), so a plain module-level counter/list
# is the correct, simplest real choice, not a corner cut.
_batch_depth = 0
_pending_notifications = []


def _schedule_notify(signal_like):
    """Every `Signal`/`Computed` write-completion routes through this,
    instead of calling `._notify()` directly -- outside an active
    `batch()`, behaves exactly as before (fires immediately); inside
    one, defers into `_pending_notifications` instead. Since nothing
    calls `batch()` unless an app opts in, `_batch_depth` stays `0` for
    every pre-M45 code path, making this a true no-op refactor of
    `Signal`'s own prior direct-`._notify()` behavior.
    """
    if _batch_depth > 0:
        _pending_notifications.append(signal_like)
    else:
        signal_like._notify()


def batch(fn):
    """Runs `fn()` with every `Signal`/`Computed` write inside it
    deferred until `fn` returns, then fires each affected *subscriber
    callback* exactly once -- not once per individual `.set()`/
    `.update()` call, and not once per Signal it happens to depend on.
    A `Computed` that depends on two Signals both written inside one
    `batch()` recomputes exactly once, with no `Computed`-specific
    batching code anywhere: its own recompute is just a normal
    subscriber callback on its dependencies, so deferring *their*
    notification already defers it too.

    **Real bug caught by actually running this, not by inspection:** an
    earlier version deduplicated by *Signal*, then called each pending
    Signal's own `._notify()` -- which still invoked a callback
    subscribed to *multiple* pending Signals once per Signal (e.g. a
    `Computed` depending on both `x` and `y`, both written in the same
    batch, recomputed twice). Deduplication has to happen at the
    *callback* level instead: bound methods compare/hash by identity of
    `(__self__, __func__)`, not by the specific bound-method object a
    given access creates (confirmed directly: `obj.method == obj.method`
    is `True` even though `obj.method is obj.method` is `False`), so a
    plain `set()` of callbacks already dedupes correctly across
    different Signals' own subscriber lists.

    Nested `batch()` calls only flush once the *outermost* one returns
    (a plain depth counter, not a real stack -- nothing here needs
    per-level data). Pending notifications are flushed in a `finally`,
    so they still fire even if `fn` raises -- the exception itself still
    propagates normally, `batch` never swallows it.
    """
    global _batch_depth
    _batch_depth += 1
    try:
        return fn()
    finally:
        _batch_depth -= 1
        if _batch_depth == 0:
            pending, _pending_notifications[:] = _pending_notifications[:], []
            seen_signals = set()
            invoked_callbacks = set()
            for signal_like in pending:
                if id(signal_like) in seen_signals:
                    continue
                seen_signals.add(id(signal_like))
                # A snapshot, not a live iteration over `signal_like.
                # _subscribers` -- real bug caught by actually running
                # `examples/reactivity.py`: a `Computed` subscriber's
                # own recompute unsubscribes-then-resubscribes itself
                # on this very list while being invoked, which silently
                # skipped/duplicated callbacks mid-loop. See `Signal.
                # _notify`'s own matching fix and comment below.
                for callback in list(signal_like._subscribers):
                    if callback in invoked_callbacks:
                        continue
                    invoked_callbacks.add(callback)
                    callback()


def untrack(fn):
    """Runs `fn()` without its own `Signal.get()`/`Computed.get()` reads
    being captured by whatever outer `Computed`/`Effect`/binding
    recording scope is currently active, if any. Needs no dedicated
    Rust-side primitive: opening and immediately closing a fresh
    recording frame around `fn()` and discarding what it collected
    already hides those reads from the *outer* frame beneath it on the
    stack (`_record_read` only ever touches the top one) -- exactly
    `untrack`'s contract, reusing `_begin_recording`/`_end_recording`
    verbatim.
    """
    _begin_recording()
    try:
        return fn()
    finally:
        _end_recording()  # discarded on purpose -- that's the whole point


class Signal:
    """A minimal reactive value cell (§16.2). `.get()` records a
    dependency when read during a binding's evaluation; `.set()`/
    `.update()` notify every binding subscribed through that read --
    but only when the new value actually differs from the current one.

    **M14 Phase 3 real finding:** a two-way-bound widget (§16.7) is both
    a `Signal` subscriber (its own `bindings:` entry, forward direction)
    and, through `TwoWayCallback`, a `Signal` writer (reverse direction)
    -- unconditional notification turned a single real `Change` into
    infinite recursion: `set_checked(True)` fires `Change` ->
    `TwoWayCallback` writes `signal.set(True)` -> notifies the widget's
    own forward binding -> which calls `set_checked(True)` again -> ...
    Each level was individually caught and logged by `call_handler`'s
    own "an uncaught exception is non-fatal" policy (§9), which is why
    this stayed silent under `pytest` instead of crashing loudly -- only
    surfaced by actually running `examples/two_way_binding.py` end to
    end. Skipping notification when the value hasn't changed is the
    correct general fix, not a two-way-specific special case: the second
    `set(True)` in the loop above is a no-op write to a `Signal` already
    holding `True`, so the recursion terminates there on its own.
    """

    def __init__(self, value):
        self._value = value
        self._subscribers = []

    def get(self):
        _record_read(self)
        return self._value

    def set(self, value):
        if value == self._value:
            return
        self._value = value
        _schedule_notify(self)

    def update(self, fn):
        """Sets this signal's value to `fn(current_value)`, then
        notifies -- the idiomatic "read, transform, write" update, e.g.
        `clicks.update(lambda n: n + 1)` -- unless the result is the same
        value the signal already held, matching `.set()`'s own real
        change-detection above.
        """
        new_value = fn(self._value)
        if new_value == self._value:
            return
        self._value = new_value
        _schedule_notify(self)

    def _subscribe(self, callback):
        """Called from Rust (`View._attach`) -- registers a binding's
        own re-evaluation trigger. Not part of `Signal`'s own public
        API; an app author never calls this directly.
        """
        self._subscribers.append(callback)

    def _unsubscribe(self, callback):
        """M43 Phase 2 (§4, §5, §8, §16.2, §16.6): `_subscribe`'s own
        real inverse -- called from Rust (`Component.remove`) so a
        removed component's own bindings stop reacting to further
        writes on a `Signal` they no longer have a live `NodeId` for.
        Without this, a `Signal` write after removal would panic
        (`apply_binding_value`'s own `tree.borrow_mut()...` calls
        `.expect()` a `NodeId` still present in the `Tree`) -- the real,
        decisive reason this method exists, not manufactured ahead of a
        real need. A silent no-op if `callback` was never subscribed
        (already removed, or never here at all), matching `Tree::
        remove`'s own "not found is a no-op, not an error" convention
        throughout this codebase -- not part of `Signal`'s own public
        API, same as `_subscribe`.
        """
        try:
            self._subscribers.remove(callback)
        except ValueError:
            pass

    def _notify(self):
        # A snapshot, not a live iteration -- see `Computed._notify`'s
        # own matching fix for the real, concrete bug this closes (a
        # subscriber that unsubscribes/resubscribes itself from *this*
        # list during its own execution, which a `Computed` downstream
        # of a `Signal` does on every recompute).
        for callback in list(self._subscribers):
            callback()


_UNSET = object()


class Computed:
    """A derived, cached reactive value (§16.2, M45): `fn` is run once
    inside a real recording scope (`_begin_recording`/`_end_recording`,
    the same mechanism `View._attach` uses for `{{ }}` bindings) to
    discover its own real dependencies, subscribes to each, and only
    re-runs `fn` -- then only renotifies its own downstream subscribers
    -- when one of them actually changes, mirroring `Signal.set`'s own
    "skip notify if the value is unchanged" rule.

    Duck-types `Signal`'s own `.get()`/`_subscribe`/`_unsubscribe`
    shape exactly, so a `{{ }}` binding (or another `Computed`, or an
    `Effect`) can depend on one with zero special-casing anywhere:

        total = Computed(lambda: price.get() * quantity.get())
        total.get()  # recomputes only when price or quantity changes

    Real, deliberate scope limit: dependencies are re-subscribed in
    full on every recompute (unsubscribe every old one, subscribe every
    new one) rather than diffed -- the real dependency lists this is
    for are small (a handful of Signals), so the simpler unconditional
    approach is correct, not a corner cut.
    """

    def __init__(self, fn):
        self._fn = fn
        self._dependencies = []
        self._subscribers = []
        self._value = _UNSET
        self._recompute()

    def get(self):
        _record_read(self)
        return self._value

    def _recompute(self):
        for dependency in self._dependencies:
            dependency._unsubscribe(self._recompute)
        _begin_recording()
        try:
            new_value = self._fn()
        finally:
            self._dependencies = _end_recording()
        for dependency in self._dependencies:
            dependency._subscribe(self._recompute)
        if new_value != self._value:
            self._value = new_value
            _schedule_notify(self)

    def _subscribe(self, callback):
        self._subscribers.append(callback)

    def _unsubscribe(self, callback):
        try:
            self._subscribers.remove(callback)
        except ValueError:
            pass

    def _notify(self):
        # A snapshot, not a live iteration -- the real bug this closes,
        # caught by actually running `examples/reactivity.py`, not by
        # inspection: a `Computed`-of-`Computed` chain (`total_label`
        # depending on `total`) means the *first* subscriber invoked
        # here can itself unsubscribe-then-resubscribe from *this very
        # list* as part of its own `_recompute()` -- mutating `self.
        # _subscribers` while this `for` loop is still walking its old
        # positions silently skipped the next real subscriber (`Effect.
        # _run`) and re-invoked the mutating one a second time instead.
        for callback in list(self._subscribers):
            callback()


class Effect:
    """Runs `fn` once immediately, then again every time one of the
    Signals/Computeds it actually read last time changes (§16.2, M45) --
    the same real dependency-recording `Computed` uses, minus the cache:
    `Effect` has no `.get()`/downstream subscribers of its own, it's for
    side effects only (logging, a non-visual derived computation, a
    callback into other systems).

        Effect(lambda: print(f"count is now {count.get()}"))

    `dispose()` unsubscribes from every currently-tracked dependency and
    stops future reruns -- call it when whatever owns this `Effect` goes
    away. Named `dispose`, not `remove()`: `Component.remove()`'s own
    name is specific to tearing down a live `Tree` subtree, which an
    `Effect` never has.
    """

    def __init__(self, fn):
        self._fn = fn
        self._dependencies = []
        self._run()

    def _run(self):
        for dependency in self._dependencies:
            dependency._unsubscribe(self._run)
        _begin_recording()
        try:
            self._fn()
        finally:
            self._dependencies = _end_recording()
        for dependency in self._dependencies:
            dependency._subscribe(self._run)

    def dispose(self):
        for dependency in self._dependencies:
            dependency._unsubscribe(self._run)
        self._dependencies = []


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

            def bump(self):
                self.clicks.update(lambda n: n + 1)

        vm = CounterViewModel(view)
    """

    def __init__(self, view):
        self._view = view
        view._attach(self)


__all__ = [
    "App",
    "CanvasContext",
    "Component",
    "MONOSPACE_FONT_FAMILY",
    "Node",
    "View",
    "Window",
    "Signal",
    "ViewModel",
]
