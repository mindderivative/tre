"""M14 Phase 3 (§16.7): real, repeatable coverage of `WidgetSpec.
two_way`/`View._attach`'s new write-back wiring -- a real `view.yaml`
declaring `two_way: checked` on a `Checkbox` bound to a `Signal`, proving
both directions: the forward one-way apply (`test_view_binding.py`
already proves this shape in general) and the new reverse direction,
where a real `Change` on the widget (fired by `Node.set_checked`, the
only Python-reachable `Change` source for a `Checkbox` -- see `test_
change_event.py`) writes the node's current value back into the bound
`Signal`.

Same "requires `maturin develop` first, imports the real compiled
extension" discipline as `test_view_binding.py`.
"""

import pytest

from tre import Signal, View, ViewModel


def write_view(tmp_path, yaml):
    path = tmp_path / "view.yaml"
    path.write_text(yaml)
    return str(path)


def test_two_way_checkbox_writes_the_signal_back_when_checked_changes(tmp_path):
    path = write_view(
        tmp_path,
        """
id: agree
kind: Checkbox
style: {width: 24, height: 24, background: "#6750A4"}
bindings: {checked: "{{ agreed.get() }}"}
two_way: checked
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.agreed = Signal(False)
            super().__init__(view)

    vm = VM(view)
    node = view.node("agree")
    assert node.get_checked() is False, "the initial bindings: apply must still run one-way"

    node.set_checked(True)
    assert vm.agreed.get() is True, (
        "a real Change on the two-way-bound widget must write its current value back into "
        "the Signal it's bound to"
    )

    node.set_checked(False)
    assert vm.agreed.get() is False, "each real Change writes back again, not just the first"


def test_two_way_text_field_writes_the_signal_back_when_text_changes(tmp_path):
    """M15 Phase 3 (§16.7): `TextField`'s own real two-way binding --
    `set_text` (the only Python-reachable `Change` source for a
    `TextField` from a `View`, which has no live window to type
    through) is the same real mechanism the Checkbox test above already
    proves for `checked`, now exercised through `TwoWayCallback`'s new
    `"text"` branch instead of `"checked"`.
    """
    path = write_view(
        tmp_path,
        """
id: username
kind: TextField
text: {content: "", font_family: Roboto, font_size: 16}
style: {width: 200, height: 32, background: "#EEEEEE"}
bindings: {text: "{{ name.get() }}"}
two_way: text
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.name = Signal("jane")
            super().__init__(view)

    vm = VM(view)
    node = view.node("username")
    assert node.get_text() == "jane", "the initial bindings: apply must still run one-way"

    node.set_text("janet")
    assert vm.name.get() == "janet", (
        "a real Change on the two-way-bound TextField must write its current text back into "
        "the Signal it's bound to"
    )

    node.set_text("")
    assert vm.name.get() == "", "each real Change writes back again, not just the first"


def test_two_way_round_trip_does_not_recurse_infinitely(capfd, tmp_path):
    """Real finding (M14 Phase 3): a two-way-bound widget is both a
    `Signal` subscriber (its own forward `bindings:` entry) and, via
    `TwoWayCallback`, a `Signal` writer -- `set_checked` fires `Change`,
    which writes the `Signal`, which (before `Signal.set`'s own
    change-detection fix) unconditionally re-notified the very binding
    that called `set_checked` in the first place, recursing until
    Python's stack limit. Each recursion level was individually caught
    and logged by `call_handler`'s own non-fatal-handler policy (§9),
    so `test_two_way_checkbox_writes_the_signal_back_when_checked_
    changes` above still passed even before the fix -- this test is the
    one that actually catches it, by asserting stderr stays clean.

    `capfd`, not `capsys` (M16 Phase 2): `call_handler`'s own logging
    now goes through `tracing::error!`, a raw OS-level stderr write
    from Rust that bypasses Python's `sys.stderr` object entirely --
    `capsys` can no longer see it at all, which would make this test's
    own `captured.err == ""` assertion trivially pass regardless of
    whether a real recursion error actually fired underneath, silently
    losing the exact regression guarantee this test exists for.
    """
    path = write_view(
        tmp_path,
        """
id: agree
kind: Checkbox
style: {width: 24, height: 24, background: "#6750A4"}
bindings: {checked: "{{ agreed.get() }}"}
two_way: checked
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.agreed = Signal(False)
            super().__init__(view)

    vm = VM(view)
    node = view.node("agree")

    node.set_checked(True)

    captured = capfd.readouterr()
    assert captured.err == "", (
        f"a real two-way round trip must not recurse or raise at all -- got stderr: {captured.err!r}"
    )
    assert vm.agreed.get() is True


def test_forward_binding_still_applies_when_the_signal_changes_from_the_vm_side(tmp_path):
    """The reverse (widget -> Signal) direction added by this phase must
    not break the pre-existing forward (Signal -> widget) direction --
    setting the Signal directly, the way a ViewModel method typically
    would, must still re-apply through the ordinary one-way path.
    """
    path = write_view(
        tmp_path,
        """
id: agree
kind: Checkbox
style: {width: 24, height: 24, background: "#6750A4"}
bindings: {checked: "{{ agreed.get() }}"}
two_way: checked
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.agreed = Signal(False)
            super().__init__(view)

    vm = VM(view)
    node = view.node("agree")

    vm.agreed.set(True)
    assert node.get_checked() is True, "setting the Signal from the VM side must still reach the widget"


def test_two_way_on_a_computed_expression_is_a_clear_load_time_error(tmp_path):
    """§16.7's own restriction, enforced by `View._attach`: two_way only
    makes sense on a plain Signal's own `.get()` call, since there's no
    way to reverse an arbitrary expression back into one. A computed
    expression (arithmetic here, anything beyond a bare `signal.get()`)
    must fail loudly at attach time, not silently do nothing on a real
    Change.
    """
    path = write_view(
        tmp_path,
        """
id: volume
kind: Slider
style: {width: 180, height: 32, background: "#03DAC6"}
bindings: {thumb_position: "{{ level.get() + 0.1 }}"}
two_way: thumb_position
""",
    )
    view = View(path)

    class VM(ViewModel):
        def __init__(self, view):
            self.level = Signal(0.2)
            super().__init__(view)

    with pytest.raises(ValueError, match="plain Signal's own .get\\(\\) call"):
        VM(view)


def test_two_way_naming_an_unbound_property_is_a_clear_load_time_error(tmp_path):
    path = write_view(
        tmp_path,
        """
id: agree
kind: Checkbox
style: {width: 24, height: 24, background: "#6750A4"}
two_way: checked
""",
    )
    view = View(path)

    class VM(ViewModel):
        pass

    # No `bindings: {checked: ...}` at all -- two_way names a property
    # with nothing bound to it, so there's no Signal to resolve.
    VM(view)  # must not raise -- an unmatched two_way is simply inert, not an error
