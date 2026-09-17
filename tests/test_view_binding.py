"""§14 step 12 (Stage C): real, repeatable coverage of §16.2's
`View`/`Signal`/`ViewModel` mechanism -- binding resolution, real
dependency tracking (a binding re-evaluates automatically when exactly
the `Signal` it read changes), and eager handler validation.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_engine_py.py`.
"""

import pytest

from tre import Signal, View, ViewModel


def write_view(tmp_path, yaml):
    path = tmp_path / "view.yaml"
    path.write_text(yaml)
    return str(path)


def test_binding_applies_its_initial_value_from_a_signal(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233", opacity: 1.0}
bindings: {opacity: "{{ level.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.level = Signal(0.3)
            super().__init__(view)

    vm = VM(view)
    node = view.node("root")
    assert node.get("opacity") == pytest.approx(0.3)


def test_binding_reevaluates_automatically_when_its_signal_is_set(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233", opacity: 1.0}
bindings: {opacity: "{{ level.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.level = Signal(0.3)
            super().__init__(view)

    vm = VM(view)
    node = view.node("root")
    assert node.get("opacity") == pytest.approx(0.3)

    vm.level.set(0.9)
    assert node.get("opacity") == pytest.approx(
        0.9
    ), "writing the Signal a binding read must automatically re-apply it"


def test_binding_reevaluates_via_update_and_supports_arithmetic(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233", opacity: 0.0}
bindings: {opacity: "{{ level.get() + 0.1 }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.level = Signal(0.2)
            super().__init__(view)

    vm = VM(view)
    node = view.node("root")
    assert node.get("opacity") == pytest.approx(0.3)

    vm.level.update(lambda n: n + 0.5)
    assert node.get("opacity") == pytest.approx(0.8)


def test_setting_a_signal_to_its_current_value_does_not_notify_subscribers():
    """M14 Phase 3 real finding (see `test_two_way_binding.py`'s own
    `test_two_way_round_trip_does_not_recurse_infinitely`): `Signal.set`
    used to notify unconditionally, which turned a two-way binding's
    write-back into infinite recursion. The general, correct fix is
    change-detection on `Signal` itself, proven here in isolation with
    no `View`/`Tree` involved at all.
    """
    level = Signal(0.3)
    calls = []
    level._subscribe(lambda: calls.append("notified"))

    level.set(0.3)
    assert calls == [], "setting a Signal to the value it already holds must not notify"

    level.set(0.9)
    assert calls == ["notified"], "setting a Signal to a genuinely new value must still notify"


def test_update_returning_the_same_value_does_not_notify_subscribers():
    count = Signal(5)
    calls = []
    count._subscribe(lambda: calls.append("notified"))

    count.update(lambda n: n)  # returns the same value unchanged
    assert calls == [], "an update() that resolves to the same value must not notify"

    count.update(lambda n: n + 1)
    assert calls == ["notified"]


def test_a_signal_never_read_by_a_binding_does_not_trigger_it(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233", opacity: 1.0}
bindings: {opacity: "{{ level.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.level = Signal(0.4)
            self.unrelated = Signal(1)
            super().__init__(view)

    vm = VM(view)
    node = view.node("root")
    assert node.get("opacity") == pytest.approx(0.4)

    vm.unrelated.set(999)
    assert node.get("opacity") == pytest.approx(
        0.4
    ), "a Signal the binding never read must not affect it at all"


def test_attach_raises_when_a_declared_handler_has_no_matching_method(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233"}
handlers: {on_click: "bump"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        pass  # no `bump` method at all

    with pytest.raises(ValueError, match="bump"):
        VM(view)


def test_attach_succeeds_when_the_handler_method_exists(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#112233"}
handlers: {on_click: "bump"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def bump(self, event):
            pass

    VM(view)  # must not raise


def test_view_node_raises_a_clear_error_for_an_unknown_widget_id(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Container
""",
    )
    view = View(path)
    with pytest.raises(ValueError, match="nope"):
        view.node("nope")
