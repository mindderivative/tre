"""M45 (§16.2): real, repeatable coverage of the richer-reactivity layer
built on top of `Signal`'s own dependency-recording primitive --
`Computed`, `Effect`, `batch()`, `untrack()`. Same "requires `maturin
develop` first, imports the real compiled extension" discipline as
`test_view_binding.py`.

`Computed`/`Effect` duck-type against `Signal`'s own `_subscribe`/
`_unsubscribe` shape -- several tests below prove real composition (a
`{{ }}` YAML binding pointed directly at a `Computed`, a `Computed`-of-
`Computed` chain) rather than just exercising each primitive alone.
"""

import pytest

from tre import Computed, Effect, Signal, View, ViewModel, batch, untrack


def write_view(tmp_path, yaml):
    path = tmp_path / "view.yaml"
    path.write_text(yaml)
    return str(path)


# ---------------------------------------------------------------------
# Computed
# ---------------------------------------------------------------------


def test_computed_derives_its_initial_value_from_its_dependencies():
    price = Signal(10)
    quantity = Signal(3)
    total = Computed(lambda: price.get() * quantity.get())
    assert total.get() == 30


def test_computed_recomputes_when_a_real_dependency_changes():
    price = Signal(10)
    quantity = Signal(3)
    total = Computed(lambda: price.get() * quantity.get())
    assert total.get() == 30

    price.set(20)
    assert total.get() == 60, "a Computed must re-derive after a dependency it read changes"


def test_computed_does_not_renotify_subscribers_when_the_recomputed_value_is_unchanged():
    parity = Signal(4)
    even = Computed(lambda: parity.get() % 2 == 0)
    notifications = []
    even._subscribe(lambda: notifications.append("notified"))

    parity.set(6)  # still even -> recomputed value is unchanged
    assert even.get() is True
    assert notifications == [], "recomputing to the same value must not notify downstream"

    parity.set(7)  # now odd -> value genuinely changes
    assert even.get() is False
    assert notifications == ["notified"]


def test_computed_of_computed_chains_correctly():
    a = Signal(1)
    b = Signal(2)
    total = Computed(lambda: a.get() + b.get())
    doubled = Computed(lambda: total.get() * 2)

    assert doubled.get() == 6
    a.set(10)
    assert total.get() == 12
    assert doubled.get() == 24, "a Computed-of-a-Computed must re-derive through the whole chain"


def test_a_signal_never_read_by_a_computed_does_not_trigger_it():
    tracked = Signal(1)
    untracked = Signal(999)
    computed = Computed(lambda: tracked.get())
    recomputes = []
    orig = computed._recompute

    def counting():
        recomputes.append(1)
        orig()

    computed._recompute = counting
    for dep in list(computed._dependencies):
        dep._unsubscribe(orig)
        dep._subscribe(counting)

    untracked.set(1000)
    assert recomputes == [], "a Signal a Computed never read must not trigger a recompute"
    tracked.set(2)
    assert len(recomputes) == 1


def test_a_yaml_binding_can_bind_directly_to_a_computed(tmp_path):
    """The real, concrete proof of the duck-typing composition claim:
    `Computed` needs zero Rust-side changes to work as a `{{ }}` binding
    target, since `View._attach`'s own subscribe call site never type-
    checks what it subscribes to.
    """
    path = write_view(
        tmp_path,
        """
id: root
kind: Text
style: {width: 200, height: 30, foreground: "#000000"}
text: {content: "", font_family: Roboto, font_size: 14}
bindings: {text: "{{ full_name.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.first = Signal("Jane")
            self.last = Signal("Doe")
            self.full_name = Computed(lambda: f"{self.first.get()} {self.last.get()}")
            super().__init__(view)

    vm = VM(view)
    node = view.node("root")
    assert node.get_text() == "Jane Doe"

    vm.first.set("John")
    assert node.get_text() == "John Doe", "the binding must re-evaluate when the Computed changes"


# ---------------------------------------------------------------------
# Effect
# ---------------------------------------------------------------------


def test_effect_runs_immediately_on_construction():
    count = Signal(5)
    log = []
    Effect(lambda: log.append(count.get()))
    assert log == [5]


def test_effect_reruns_exactly_once_per_real_dependency_change():
    count = Signal(0)
    log = []
    Effect(lambda: log.append(count.get()))

    count.set(1)
    count.set(2)
    assert log == [0, 1, 2]


def test_effect_dispose_stops_further_reruns():
    count = Signal(0)
    log = []
    effect = Effect(lambda: log.append(count.get()))
    assert log == [0]

    effect.dispose()
    count.set(1)
    count.set(2)
    assert log == [0], "a disposed Effect must not react to further Signal writes"


def test_effect_only_tracks_signals_it_actually_read_this_run():
    branch = Signal(True)
    a = Signal("a")
    b = Signal("b")
    log = []

    def run():
        log.append(a.get() if branch.get() else b.get())

    Effect(run)
    assert log == ["a"]

    b.set("changed")  # not read on the last run (branch is True) -- must not rerun
    assert log == ["a"]

    branch.set(False)  # rerun -- this time it reads b, not a
    assert log == ["a", "changed"]

    a.set("changed again")  # no longer read -- must not rerun
    assert log == ["a", "changed"]


# ---------------------------------------------------------------------
# batch()
# ---------------------------------------------------------------------


def test_batch_collapses_multiple_writes_into_one_notification_pass():
    a = Signal(1)
    b = Signal(1)
    notifications = []
    a._subscribe(lambda: notifications.append("notified"))
    b._subscribe(lambda: notifications.append("notified"))

    def writes():
        a.set(2)
        b.set(2)

    batch(writes)
    assert notifications == ["notified", "notified"], "each Signal's own subscriber still fires"


def test_batch_causes_a_shared_computed_to_recompute_exactly_once():
    x = Signal(1)
    y = Signal(1)
    recomputes = []
    total = Computed(lambda: x.get() + y.get())
    orig = total._recompute

    def counting():
        recomputes.append(1)
        orig()

    total._recompute = counting
    for dep in list(total._dependencies):
        dep._unsubscribe(orig)
        dep._subscribe(counting)

    def writes():
        x.set(10)
        y.set(20)

    batch(writes)
    assert total.get() == 30
    assert len(recomputes) == 1, (
        "a Computed depending on two Signals both written in one batch must recompute exactly "
        "once, not once per dependency -- a real bug caught by running this: deduplicating by "
        "Signal alone still re-invoked a shared subscriber once per Signal"
    )


def test_nested_batch_only_flushes_at_the_outermost_exit():
    signal = Signal(0)
    notifications = []
    signal._subscribe(lambda: notifications.append("notified"))

    def outer():
        signal.set(1)

        def inner():
            signal.set(2)

        batch(inner)
        assert notifications == [], "an inner batch exiting must not flush while an outer one is open"

    batch(outer)
    assert notifications == ["notified"], "only the outermost batch exit flushes"


def test_batch_flushes_pending_notifications_even_when_the_function_raises():
    signal = Signal(0)
    notifications = []
    signal._subscribe(lambda: notifications.append("notified"))

    def boom():
        signal.set(1)
        raise RuntimeError("boom")

    with pytest.raises(RuntimeError, match="boom"):
        batch(boom)

    assert notifications == ["notified"], "a pending notification must still flush after an exception"


# ---------------------------------------------------------------------
# untrack()
# ---------------------------------------------------------------------


def test_untrack_hides_a_read_from_the_enclosing_effect():
    tracked = Signal("a")
    hidden = Signal("b")
    log = []

    def run():
        log.append(tracked.get())
        untrack(lambda: hidden.get())

    effect = Effect(run)
    assert len(effect._dependencies) == 1, "the untracked read must not become a real dependency"

    hidden.set("changed")
    assert log == ["a"], "a Signal only ever read inside untrack() must not trigger a rerun"

    tracked.set("changed")
    assert log == ["a", "changed"]


def test_untrack_returns_the_wrapped_functions_result():
    signal = Signal(42)
    assert untrack(lambda: signal.get()) == 42


# ---------------------------------------------------------------------
# The real M45 prerequisite fix: nested recording scopes
# ---------------------------------------------------------------------


def test_nested_recording_does_not_corrupt_an_outer_bindings_own_dependencies(tmp_path):
    """The single most important regression proof named in this
    milestone's own plan: before the `RECORDING` stack fix, a YAML
    binding's own recording scope was a single flat slot. The binding
    grammar already permits a zero-arg method call with real side
    effects -- if that side effect (a `Signal.set()`) synchronously
    triggers a `Computed`'s own eager, *nested* recording scope, the
    inner `end_recording()` used to wipe out the outer scope's already-
    recorded dependencies entirely, silently breaking that binding's own
    reactivity forever. This proves the fix: the outer binding still
    reacts correctly after a real nested recompute happens mid-
    evaluation.

    **Real, deliberate construction detail, found by actually running
    an earlier draft of this test:** `trigger_b` sets `b` to a value it
    already has in hand (a plain counter), *never* calling `b.get()` of
    its own. Calling `b.get()` here too would record `b` into the same
    still-open outer frame `a` is recorded into (recording is ambient --
    it doesn't know or care how deep the call stack is), making `b` an
    *over-broad* dependency of this same binding; since the binding's
    own evaluation also writes to `b`, that combination is a real,
    separate, genuinely pathological hazard (a binding that ends up
    subscribed to a Signal its own evaluation writes to) -- not the
    nested-recording-stack bug this test exists to prove. Avoiding it
    here keeps this test isolated to the one real thing it's checking.
    """

    class TriggeringSignal(Signal):
        """A real Signal subclass whose own `.get()` -- the exact
        method a `{{ a.get() }}` binding calls -- has a real side
        effect, the mechanism that makes nesting reachable at all.
        """

        def __init__(self, value, on_trigger):
            super().__init__(value)
            self._on_trigger = on_trigger

        def get(self):
            result = super().get()
            self._on_trigger()
            return result

    path = write_view(
        tmp_path,
        """
id: root
kind: Text
style: {width: 200, height: 30, foreground: "#000000"}
text: {content: "", font_family: Roboto, font_size: 14}
bindings: {text: "{{ a.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.b = Signal(1)
            self.b_derived = Computed(lambda: self.b.get() * 100)
            self._n = 1

            def trigger_b():
                self._n += 1
                self.b.set(self._n)

            self.a = TriggeringSignal("a1", trigger_b)
            super().__init__(view)

    vm = VM(view)
    node = view.node("root")
    assert node.get_text() == "a1"
    # b was bumped once by the binding's own initial apply reading `a`.
    assert vm.b_derived.get() == 200

    vm.a.set("a2")
    assert node.get_text() == "a2", (
        "REGRESSION: the outer binding lost its own dependency tracking after a nested "
        "recording scope opened mid-evaluation"
    )


# ---------------------------------------------------------------------
# M46: the reentrant-notification guard -- the real fix for the hazard
# named (and deliberately left unfixed) in M45's own BUILD_TRACKER.md
# entry. `_Notifiable._notify` (the shared base `Signal`/`Computed` both
# use) now raises a clear `RuntimeError` the moment an object's own
# notify is re-entered while still running, instead of recursing until
# a `RecursionError` -- for any cause, not just the one scenario found
# while building M45's own regression test.
# ---------------------------------------------------------------------


def test_reentrant_notify_raises_a_clear_error_instead_of_recursing():
    """The minimal, isolated proof of the guard itself, with no View/
    binding machinery involved: a Signal subscriber whose own call
    writes back to the very Signal notifying it.
    """
    b = Signal(1)

    def loopback():
        b.set(b.get() + 1)

    b._subscribe(loopback)
    with pytest.raises(RuntimeError, match="written to again while still notifying"):
        b.set(2)


def test_reentrant_notify_guard_resets_after_a_caught_exception():
    """The `_notifying` flag must reset via `finally`, not stay stuck
    `True` forever after the first reentrant write is caught -- a later,
    unrelated write on the same Signal must still work normally.
    """
    b = Signal(1)

    def loopback():
        b.set(b.get() + 1)

    b._subscribe(loopback)
    with pytest.raises(RuntimeError):
        b.set(2)

    b._unsubscribe(loopback)
    b.set(99)  # must not raise -- the guard must not be permanently tripped
    assert b.get() == 99


def test_a_legitimate_non_cyclic_chain_does_not_trip_the_guard():
    """A real, deep dependency chain (a -> b -> c, no path back to a)
    must notify cleanly -- the guard is specifically for an object
    re-entering *its own* notify, not for depth in general.
    """
    a = Signal(1)
    b = Computed(lambda: a.get() * 10)
    c = Computed(lambda: b.get() + 1)
    log = []
    Effect(lambda: log.append(c.get()))

    a.set(2)  # must not raise
    assert log == [11, 21]


def test_batch_flush_also_hits_the_reentrant_notify_guard():
    """M46's own real finding: `batch()`'s flush loop doesn't call
    `_notify()` through the ordinary immediate path -- it threads a
    shared dedup set straight into `_Notifiable._notify`. This proves
    the guard fires there too, not just on the immediate (non-batched)
    path.
    """
    b = Signal(1)

    def loopback():
        b.set(b.get() + 1)

    b._subscribe(loopback)

    def writes():
        b.set(2)

    with pytest.raises(RuntimeError, match="written to again while still notifying"):
        batch(writes)


def test_the_original_read_before_write_binding_hazard_now_raises_clearly(tmp_path):
    """The real, originally-observed scenario (traced while building
    M45's own nested-recording regression test): a `Signal.get()`
    override that reads a *different* Signal's current value before
    writing to it, while an outer binding's own recording scope is
    open. `b` gets over-broadly recorded as a dependency of the "text"
    binding (recording is ambient); since the binding's own evaluation
    also writes `b`, every write used to re-trigger the same binding
    before the previous notification returned -- confirmed via `sys.
    setrecursionlimit` + traced stack prints while first finding this.
    Now it raises one clear error instead.
    """

    class TriggeringSignal(Signal):
        def __init__(self, value, on_read):
            super().__init__(value)
            self._on_read = on_read

        def get(self):
            result = super().get()
            self._on_read()
            return result

    path = write_view(
        tmp_path,
        """
id: root
kind: Text
style: {width: 200, height: 30, foreground: "#000000"}
text: {content: "", font_family: Roboto, font_size: 14}
bindings: {text: "{{ a.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.b = Signal(1)

            def trigger_b():
                self.b.set(self.b.get() + 1)

            self.a = TriggeringSignal("a1", trigger_b)
            super().__init__(view)

    vm = VM(view)
    with pytest.raises(ValueError, match="written to again while still notifying"):
        vm.a.set("a2")


def test_untrack_is_the_real_documented_fix_for_the_read_before_write_hazard(tmp_path):
    """The error message's own suggested remedy, verified end to end,
    not just asserted: wrapping the *offending read* (not the write) in
    `untrack()` stops it from being over-broadly recorded as a
    dependency of the outer binding, which is what breaks the
    self-referential subscription that caused the reentrant loop above.
    """

    class TriggeringSignal(Signal):
        def __init__(self, value, on_read):
            super().__init__(value)
            self._on_read = on_read

        def get(self):
            result = super().get()
            self._on_read()
            return result

    path = write_view(
        tmp_path,
        """
id: root
kind: Text
style: {width: 200, height: 30, foreground: "#000000"}
text: {content: "", font_family: Roboto, font_size: 14}
bindings: {text: "{{ a.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.b = Signal(1)

            def trigger_b():
                current = untrack(lambda: self.b.get())
                self.b.set(current + 1)

            self.a = TriggeringSignal("a1", trigger_b)
            super().__init__(view)

    vm = VM(view)
    node = view.node("root")
    assert vm.b.get() == 2  # bumped once by the initial attach's own read of `a`

    vm.a.set("a2")  # must not raise
    assert node.get_text() == "a2"
    assert vm.b.get() == 3  # bumped again by this second read of `a`
