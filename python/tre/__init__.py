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
    different Signals' own subscriber lists. **M46:** this dedup set is
    now threaded straight into each pending Signal's own `_Notifiable.
    _notify(already_invoked)` (§16.2) instead of being duplicated here
    -- the same method that also carries the real reentrancy guard, so
    a batched write that re-enters its own notification hits the
    identical clear error the immediate (non-batched) path does, not a
    silently-uncovered second copy of the old bug.

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
                signal_like._notify(invoked_callbacks)


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


class _Notifiable:
    """M46 (§16.2): shared subscriber-list + notify machinery for
    `Signal` and `Computed` -- both had byte-for-byte identical
    `_subscribe`/`_unsubscribe` bodies and near-identical `_notify`
    bodies before this existed (both independently fixed for the same
    live-iteration bug in M45). Factored out once a real reentrancy
    guard needed adding to both, rather than fixing the same thing
    twice -- the same "two real call sites justify factoring out"
    precedent this codebase already uses elsewhere (e.g. `throwaway_
    node` on the Rust side).
    """

    def __init__(self):
        self._subscribers = []
        self._notifying = False

    def _subscribe(self, callback):
        """Called from Rust (`View._attach`) -- registers a binding's
        own re-evaluation trigger, or from another `Computed`/`Effect`
        tracking this object as one of its own dependencies. Not part
        of the public API; an app author never calls this directly.
        """
        self._subscribers.append(callback)

    def _unsubscribe(self, callback):
        """M43 Phase 2 (§4, §5, §8, §16.2, §16.6): `_subscribe`'s own
        real inverse -- called from Rust (`Component.remove`) so a
        removed component's own bindings stop reacting to further
        writes on a `Signal` they no longer have a live `NodeId` for,
        and from `Computed`/`Effect`'s own re-tracking on every
        recompute/rerun. A silent no-op if `callback` was never
        subscribed (already removed, or never here at all), matching
        `Tree::remove`'s own "not found is a no-op, not an error"
        convention throughout this codebase -- not part of the public
        API, same as `_subscribe`.
        """
        try:
            self._subscribers.remove(callback)
        except ValueError:
            pass

    def _notify(self, already_invoked=None):
        """Invokes every subscriber exactly once. `already_invoked`,
        when given, is a `set()` of callbacks shared across a whole
        `batch()` flush (M45) -- a callback subscribed to two Signals
        both flushed in the same batch still only runs once; `batch()`
        itself doesn't duplicate this dedup logic, it just threads its
        own shared set through here.

        A snapshot of `self._subscribers`, not a live iteration -- real
        bug caught by actually running `examples/reactivity.py` (M45):
        a `Computed`-of-`Computed` chain means the first subscriber
        invoked here can itself unsubscribe-then-resubscribe from this
        very list as part of its own `_recompute()`/`_run()`, silently
        skipping the next real subscriber and re-invoking the mutating
        one a second time if this iterated the live list instead.

        **M46 real reentrancy guard:** if this object's own `_notify()`
        is already running further up the call stack, raise immediately
        instead of recursing. Real, traced scenario this closes: a
        `Signal.get()` override (or a plain ViewModel method reachable
        from a binding) that reads a *different* Signal before writing
        to it, while an outer recording scope is open, gets that
        Signal misattributed as a dependency of the outer scope too
        (recording is ambient -- it has no notion of call-stack depth);
        combined with the outer scope's own evaluation also writing to
        that same Signal, every write re-triggers the same notify
        before the previous one returns -- unbounded recursion with no
        error, confirmed directly via `sys.setrecursionlimit` + traced
        stack prints while building M45's own regression test. This
        guard doesn't prevent the over-broad attribution itself (would
        need every plain `Signal.get()` to open its own isolated
        recording frame -- real, invasive, unjustified cost on every
        Signal read for a narrow case `untrack()` already targets) --
        it turns the *consequence* into one clear, actionable error the
        moment it would happen, for any cause, not just this one.
        """
        if self._notifying:
            raise RuntimeError(
                f"{type(self).__name__} {self!r} was written to again while still notifying "
                "its own subscribers from an earlier write on the same call stack -- something "
                "invoked during this notification wrote back to it, directly or through a "
                "chain of other Signals/Computeds. If a read caused this Signal to be "
                "over-broadly recorded as a dependency it shouldn't be, wrap that read in "
                "tre.untrack(...); otherwise restructure the code so this Signal's own "
                "subscribers don't write back to it."
            )
        self._notifying = True
        try:
            invoked = already_invoked if already_invoked is not None else set()
            for callback in list(self._subscribers):
                if callback in invoked:
                    continue
                invoked.add(callback)
                callback()
        finally:
            self._notifying = False


class Signal(_Notifiable):
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
        super().__init__()
        self._value = value

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


_UNSET = object()


class Computed(_Notifiable):
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
        super().__init__()
        self._fn = fn
        self._dependencies = []
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
    "Computed",
    "Effect",
    "MONOSPACE_FONT_FAMILY",
    "Node",
    "View",
    "Window",
    "Signal",
    "ViewModel",
    "batch",
    "untrack",
]
