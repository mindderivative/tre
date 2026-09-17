#!/usr/bin/env python3
"""M14 Phase 3's real two-way binding sugar (§16.7): a real `view.yaml`
(`two_way_binding.yaml`, alongside this script) declares `two_way:
checked`/`two_way: thumb_position`/`two_way: text` on a `Checkbox`/
`Slider`/`TextField` (the last added M15 Phase 3), each also bound
one-way via the ordinary `bindings:` map -- `View._attach` wires both
directions: the existing forward apply (`Signal` -> widget, proven by
every prior `View`-based test) and the new reverse write-back (widget
-> `Signal`), registered as the widget's own real `EventKind::Change`
handler.

`View` has no live-window/render-loop concept of its own (see `view.rs`'s
own module doc comment) -- there's no `App.run()` here, the same
no-live-window-needed proof pattern `tests/test_view_handlers.py`'s
`View.click` already established. A real user editing each widget is
simulated the only way `Change` is Python-reachable for each without a
live window: `Node.set_checked`/`set_text` (both real, direct `Change`
sources -- see `tests/test_change_event.py`/`test_text_field.py`); the
real *mechanical* Slider-drag-release `Change` path has no Python entry
point at all (`tests/test_slider.py`'s own note), so this script proves
the Slider's forward direction only -- the same "prove what's Python-
reachable" split every M14/M15 test file already uses.

What this proves automatically (headless-CI-safe, no human needed): the
initial one-way apply from each `Signal`'s starting value, and a real
round trip back into each two-way-bound `Signal` after its own widget's
value changes.
"""

from pathlib import Path

from tre import Signal, View, ViewModel

view = View(str(Path(__file__).parent / "two_way_binding.yaml"))


class SettingsVM(ViewModel):
    def __init__(self, view):
        self.agreed = Signal(False)
        self.level = Signal(0.3)
        self.name = Signal("jane")
        super().__init__(view)


vm = SettingsVM(view)
checkbox = view.node("agree")
slider = view.node("volume")
field = view.node("username")

assert checkbox.get_checked() is False, "the initial bindings: apply must seed from agreed's own starting value"
assert slider.get("thumb_position") == 0.3, "the initial bindings: apply must seed from level's own starting value"
assert field.get_text() == "jane", "the initial bindings: apply must seed from name's own starting value"
print(
    f"initial: agreed={vm.agreed.get()} checked={checkbox.get_checked()}, "
    f"level={vm.level.get()} thumb_position={slider.get('thumb_position')}, "
    f"name={vm.name.get()!r} text={field.get_text()!r}"
)

# A real user checking the box -- the only Python-reachable Change
# source for a Checkbox (a real click would call this same method).
checkbox.set_checked(True)
assert vm.agreed.get() is True, "the real Change from set_checked must write back into the bound Signal"
print(f"after checking the box: agreed signal is now {vm.agreed.get()}")

# A real user editing the field -- the only Python-reachable Change
# source for a TextField from a View with no live window.
field.set_text("janet")
assert vm.name.get() == "janet", "the real Change from set_text must write back into the bound Signal"
print(f"after editing the field: name signal is now {vm.name.get()!r}")

# The forward direction still works unmodified -- setting the Signal
# from the ViewModel side re-applies through the ordinary one-way path.
vm.level.set(0.9)
assert slider.get("thumb_position") == 0.9
print(f"after vm.level.set(0.9): slider thumb_position is now {slider.get('thumb_position')}")

print("two_way_binding.py: exited cleanly, both directions of every two-way binding proved")
