#!/usr/bin/env python3
"""M44 (§16.2): the real, end-to-end proof that `background:` is now a
live-bindable property, not just an imperative `Node.animate("background",
...)` target. `apply_binding_value` (`view.rs`) used to dispatch purely
on the resolved `Value`'s own runtime type -- a `background:` binding
resolving to a `Str` was unconditionally routed to `set_text` (wrong,
and would raise on a non-text widget like `Rect`), and one resolving to
any non-primitive Python value (`Value::Handle`, e.g. an `(r,g,b,a)`
tuple) was rejected outright even though `Node.animate` already accepts
that exact shape imperatively. This script proves both of the newly
reachable shapes:

- `hex_swatch`'s `background:` binding reads a `Signal[str]` -- a hex
  color string, parsed the same way a *static* YAML color is (§16.2's
  own `parse_background_color`, mirroring `engine_spec::build::
  resolve_color`'s real hex/CSS-named parser).
- `tuple_swatch`'s `background:` binding reads a `Signal[tuple]` -- an
  `(r, g, b, a)` u8 tuple, recovered from `engine_spec::Value::Handle`
  via `PyViewModelResolver::to_pyobject` and forwarded to `Node.animate`
  verbatim, identical to what `animate_rect.py` already proves works
  imperatively.

`tre` has no Python-facing getter for a node's currently-applied
`background` color (`Node.get` only returns `f64`) -- so, matching
`tests/test_view_binding.py`'s own real, stated limitation, this script
proves the binding *applies without raising* through a real dispatched
click that changes both Signals together, and renders the result in a
real window so a human running this locally can see the swatches
actually change color. `max_frames=60` matches this workspace's own
headless-CI-safe convention -- exits cleanly with no display/GPU
reachable, same as `animate_rect.py`.
"""

from pathlib import Path

from tre import App, Signal, View, ViewModel, Window

directory = Path(__file__).parent
view = View(str(directory / "bindable_background.yaml"))

PALETTE = [
    ("#6750A4", (0x03, 0xDA, 0xC6, 0xFF)),
    ("#B00020", (0xFF, 0xB4, 0xAB, 0xFF)),
    ("#00695C", (0xFF, 0xD6, 0x00, 0xFF)),
]


class SwatchViewModel(ViewModel):
    def __init__(self, view):
        self._index = 0
        hex_color, rgba_color = PALETTE[0]
        self.hex_color = Signal(hex_color)
        self.rgba_color = Signal(rgba_color)
        super().__init__(view)

    def cycle(self):
        self._index = (self._index + 1) % len(PALETTE)
        hex_color, rgba_color = PALETTE[self._index]
        self.hex_color.set(hex_color)  # a Str binding -> parsed as a color
        self.rgba_color.set(rgba_color)  # a Handle binding -> forwarded to animate()


vm = SwatchViewModel(view)  # must not raise -- the real M44 fix

button = view.node("cycle_button")
for expected_index in range(1, len(PALETTE)):
    view.click(button)
    assert vm._index == expected_index
    assert vm.hex_color.get() == PALETTE[expected_index][0]
    assert vm.rgba_color.get() == PALETTE[expected_index][1]

print(
    f"cycled through {len(PALETTE)} palette entries: "
    f"final hex_color={vm.hex_color.get()!r}, rgba_color={vm.rgba_color.get()!r}"
)

window = Window.from_view(view, width=300, height=200, title="Bindable Background")
app = App()
app.add_window(window)
app.run(max_frames=60)
print("bindable_background.py: exited cleanly after a real 60-frame render loop")
