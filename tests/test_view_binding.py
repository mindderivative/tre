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


# M44 (§16.2): `apply_binding_value` used to dispatch purely on the
# resolved `Value`'s own runtime type, ignoring the declared property
# name entirely -- a `background:` binding resolving to a string was
# unconditionally routed to `set_text` (wrong for a non-text widget: it
# raised, just via the wrong path/message), and one resolving to any
# non-primitive Python value (`Value::Handle`, e.g. an `(r,g,b,a)`
# tuple) was rejected outright even though `Node.animate` already
# accepts that exact shape imperatively. These tests prove the new,
# property-name-first dispatch actually reaches `background` for both
# shapes, and that the error paths stay clear and specific.
#
# Real, honest limitation (not worked around): `tre` has no Python-
# facing getter for a node's currently-applied `background` color at
# all (`Node.get` only returns `f64`; confirmed by grep of `node.rs`/
# `_core.pyi`) -- so the positive-path tests below can only prove the
# binding *applies without raising* (and that the node/Tree stays
# healthy afterward, via a co-bound `opacity` on the same widget), not
# read the resulting color back. The actual color math itself (hex/
# CSS-named string -> `(r,g,b,a)`) has real, exact-value coverage as a
# Rust unit test instead (`crates/engine-py/src/view.rs`'s own
# `parse_background_color_*` tests) -- pure, GIL-free logic that needs
# no Python interpreter to verify precisely.


def test_background_binding_accepts_a_hex_color_signal(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#000000", opacity: 1.0}
bindings: {background: "{{ color.get() }}", opacity: "{{ level.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.color = Signal("#FF3366")
            self.level = Signal(0.5)
            super().__init__(view)

    vm = VM(view)  # must not raise -- the real M44 bug
    node = view.node("root")
    assert node.get("opacity") == pytest.approx(
        0.5
    ), "the node/Tree must stay healthy after a background binding applies"

    vm.color.set("#00FF00")  # re-evaluation must not raise either
    vm.level.set(0.75)
    assert node.get("opacity") == pytest.approx(0.75)


def test_background_binding_accepts_an_rgba_tuple_signal(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#000000", opacity: 1.0}
bindings: {background: "{{ color.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.color = Signal((255, 51, 102, 255))
            super().__init__(view)

    vm = VM(view)  # must not raise -- a Value::Handle reaching animate()
    node = view.node("root")
    assert node.get("opacity") == pytest.approx(1.0)

    vm.color.set((0, 255, 0, 200))  # re-evaluation must not raise either


def test_background_binding_rejects_an_invalid_color_string(tmp_path):
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#000000"}
bindings: {background: "{{ color.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.color = Signal("not a real color")
            super().__init__(view)

    with pytest.raises(ValueError, match="not a real color"):
        VM(view)


def test_background_binding_rejects_a_boolean_value(tmp_path):
    """Real M44 regression coverage: under the old value-type-first
    dispatch, *any* `Bool`-resolved binding -- regardless of its
    declared property -- was routed unconditionally to `set_checked`,
    which would raise a `Rect`-is-not-a-Checkbox error. The new
    property-name-first dispatch rejects it directly instead, with a
    message naming the actual property.
    """
    path = write_view(
        tmp_path,
        """
id: root
kind: Rect
style: {width: 10, height: 10, background: "#000000"}
bindings: {background: "{{ flag.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.flag = Signal(True)
            super().__init__(view)

    with pytest.raises(ValueError, match="background"):
        VM(view)


def test_checked_binding_gives_a_clear_error_for_a_non_boolean_value(tmp_path):
    path = write_view(
        tmp_path,
        """
id: box
kind: Checkbox
style: {width: 24, height: 24, background: "#6750A4"}
bindings: {checked: "{{ level.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.level = Signal(1)  # an int, not a bool
            super().__init__(view)

    with pytest.raises(ValueError, match="checked.*expects a boolean binding"):
        VM(view)


def test_text_binding_gives_a_clear_error_for_a_non_string_value(tmp_path):
    path = write_view(
        tmp_path,
        """
id: label
kind: TextField
text: {content: "", font_family: Roboto, font_size: 16}
style: {width: 100, height: 24, background: "#FFFFFF"}
bindings: {text: "{{ level.get() }}"}
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.level = Signal(3.14)  # a float, not a string
            super().__init__(view)

    with pytest.raises(ValueError, match="text.*expects a string binding"):
        VM(view)
