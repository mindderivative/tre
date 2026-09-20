#!/usr/bin/env python3
"""M42 Phase 1 (§4, §5, §8, §16.2, §16.4): the real, first proof that a
`View` (§16.2's declarative YAML + `ViewModel` layer) can be shown in an
actual, on-screen, `winit`-driven window and stay live and interactive
there -- every prior `View`-using example (`hot_reload.py`,
`two_way_binding.py`, `view_composition.py`) is headless, proving
dispatch/binding/reconciliation work correctly against `View`'s own
`Tree` in isolation, but never that any of it survives being shown for
real.

`Window.from_view(view)` (`crates/engine-py/src/window.rs`) shares
`view`'s own `Rc<RefCell<Tree>>`/root/`handlers`/`theme`/`completions`
directly into a real `Window` -- no second tree, no reconciliation
between two copies. This script proves the whole real stack end to end:
a declarative `on_click` handler (`button`'s own `increment`), wired by
`View._attach`, fires through a real dispatched click; the `ViewModel`'s
own `Signal` write re-evaluates `label`'s bound `text` property
(`{{ label.get() }}`) through the exact same fine-grained `Node::
animate`/`set_text` path every headless binding example already proves;
and the whole thing then runs for real, live frames through `App.run()`
-- the real `engine-render`/GPU pipeline every other windowed example in
this repo already exercises, not a new one built just for `View`.

`max_frames=30` matches this workspace's own headless-CI-safe convention
(TRE v1 finding #261): exits cleanly after 30 frames instead of waiting
for a human to close the window, and `App.run()` itself exits 0 (doesn't
raise) when no display/GPU is reachable, so this script is safe to run
unattended.
"""

from pathlib import Path

from tre import App, Signal, View, ViewModel, Window

yaml_path = Path(__file__).parent / "live_view.yaml"

view = View(str(yaml_path))


class CounterVM(ViewModel):
    def __init__(self, view):
        self._count = 0
        self.label = Signal("Count: 0")
        super().__init__(view)

    def increment(self):
        self._count += 1
        self.label.set(f"Count: {self._count}")


vm = CounterVM(view)

# The real, decisive M42 Phase 1 step: `view` is now shown in an actual
# window, sharing its own Tree/root/handlers directly -- not a second,
# separate copy `App.run()` would need to reconcile against.
window = Window.from_view(view, width=240, height=120, title="Tesserae live counter proof")

button = view.node("button")
label = view.node("label")

print(f"before any click: label={label.get_text()!r}")
assert label.get_text() == "Count: 0"

# Five real, dispatched clicks (the same no-live-window-needed
# `Tree::dispatch` press+release pair `view.click()` already proves
# headlessly, `test_view_handlers.py`) -- proving the identical
# mechanism still fires correctly now that `view`'s own layout has
# switched from `AvailableSpace::MaxContent` to the real `Definite` size
# this window was just shown at (`available_space()`, `view.rs`).
for _ in range(5):
    view.click(button)

assert vm._count == 5, "5 real clicks against the live-shown View must have reached the handler"
print(f"after 5 clicks: label={label.get_text()!r}")
assert label.get_text() == "Count: 5", (
    "the Signal write from increment() must have re-evaluated label's bound text property "
    "through the same live Tree this window paints from"
)

app = App()
app.add_window(window)
app.run(max_frames=30)
print("live_view.py: exited cleanly after a real 30-frame render loop with a live View")
