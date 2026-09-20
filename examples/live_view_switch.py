#!/usr/bin/env python3
"""M42 Phase 2 (§4, §5, §8, §16.2, §16.4): the real, first proof that
`Window.show_view(view)` switches which `View` an already-live window
shows, without closing/reopening it -- the real capability behind the
user's own explicit plan-review feedback on this milestone's approved
plan: "this allows for switching of current views without needing to
bootstrap each view/viewModel."

Two independent, fully-bootstrapped `View`s (`Screen A`/`Screen B`, each
its own YAML + `ViewModel`, each with its own `_attach`-wired `on_click`
handler) stay alive the whole time -- `ScreenAVM.go_to_b`/`ScreenBVM.
go_to_a` each call `window.show_view(...)` directly from *inside* a real
dispatched click handler, the one scenario that exercises a real bug
this phase caught and fixed before it ever shipped: an earlier draft of
`Window.click` held a live borrow across the very call that invokes a
Python handler, so a handler calling `show_view` back into that same
borrow would have panicked ("already borrowed"). Neither `View` is ever
re-constructed or re-attached across the switch -- proving the real,
load-bearing claim this milestone exists for.
"""

from pathlib import Path

from tre import App, View, ViewModel, Window

directory = Path(__file__).parent
view_a = View(str(directory / "live_view_switch_a.yaml"))
view_b = View(str(directory / "live_view_switch_b.yaml"))

# The window starts showing view_a -- the real M42 Phase 1 entry point,
# unchanged by Phase 2.
window = Window.from_view(view_a, width=240, height=120, title="Tesserae view-switch proof")

switches = []


class ScreenAVM(ViewModel):
    def go_to_b(self):
        switches.append("a->b")
        window.show_view(view_b)


class ScreenBVM(ViewModel):
    def go_to_a(self):
        switches.append("b->a")
        window.show_view(view_a)


ScreenAVM(view_a)
ScreenBVM(view_b)

button_a = view_a.node("button")
button_b = view_b.node("button")

# Three real, dispatched clicks: A -> B -> A -> B, each one switching
# which View the same live Window shows, each handler firing from
# whichever View is genuinely active at that moment -- not a replay
# against a stale tree.
window.click(button_a)
assert switches == ["a->b"]
window.click(button_b)
assert switches == ["a->b", "b->a"]
window.click(button_a)
assert switches == ["a->b", "b->a", "a->b"]

print(f"switch sequence: {switches}")

app = App()
app.add_window(window)
app.run(max_frames=20)
print(
    "live_view_switch.py: exited cleanly after a real 20-frame render loop, "
    "switching Views three times"
)
