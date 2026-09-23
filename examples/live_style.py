#!/usr/bin/env python3
"""M48 (§5, §7, §11): the real, end-to-end proof that this milestone's
new "general live property exposure" work actually reaches a real
window through the real FFI, not just pytest. Before M48, none of the
following were reachable at all from Python or YAML (confirmed via grep
before this milestone -- see `BUILD_TRACKER.md`'s M48 section for the
full audit): a node's `width`/`height`/`padding`/`gap` could never
change after construction, and `border_color`/`border_width` (real
`PaintProperties` fields since M30 Phase 1) were unreachable from
`animate()`, YAML bindings, and every widget constructor.

This script proves two distinct real mechanisms, deliberately kept
separate (matching `Node::set_layout`'s own doc comment -- layout
fields aren't `Animated<T>`, so they can never go through the same
binding path border does):

- `box`'s `border_width`/`border_color` are declared as `{{ }}`
  bindings in `live_style.yaml`, driven by two `Signal`s -- the exact
  same live-binding mechanism `bindable_background.py` already proves
  for `background`, just widened to the two new properties.
- `box`'s own size is changed imperatively, live, via `Node.
  set_layout(width=..., height=...)` -- the new general layout-mutation
  API this milestone adds, generalizing `Window.resize_terminal`'s own
  narrow, size-only precedent.

M59 (§5, §16.3) widens the same real `set_layout` call with per-side
padding and `align_items`/`justify_content` -- the layout API breadth
this milestone closes (before it, `padding` was uniform-scalar-only and
neither field existed anywhere in this codebase, confirmed via grep).

`tre` has no Python-facing getter for a node's currently-applied
`border_color` or its real pixel box (`Node.get` only returns `f64`,
and no layout-box readback exists at all -- confirmed via grep, the
same real, stated limit `test_resize.py`/`bindable_background.py` both
already establish) -- so, matching those files' own precedent, this
script proves each mechanism *applies without raising* through a real
dispatched click, and renders the result in a real window so a human
running this locally can see the border and size actually change.
`max_frames=60` matches this workspace's own headless-CI-safe
convention.
"""

from pathlib import Path

from tre import App, Signal, View, ViewModel, Window

directory = Path(__file__).parent
view = View(str(directory / "live_style.yaml"))

BORDERS = [
    (2.0, "#FFFFFF"),
    (6.0, "#03DAC6"),
    (0.0, "#000000"),
]
SIZES = [(160.0, 80.0), (200.0, 100.0), (120.0, 60.0)]
# M59: one real per-side padding value per cycle step, applied to
# `root` (the flex container `box`/`cycle_button` sit inside -- padding
# and align/justify only have a real visible effect on a container with
# children, not on `box` itself, a childless leaf `Rect`) plus a real
# align_items/justify_content pair -- the new layout API breadth.
PADDING_TOPS = [16.0, 32.0, 8.0]
ALIGNMENTS = [("stretch", "start"), ("center", "space_between"), ("end", "center")]


class LiveStyleViewModel(ViewModel):
    def __init__(self, view):
        self._index = 0
        thickness, outline = BORDERS[0]
        self.thickness = Signal(thickness)
        self.outline = Signal(outline)
        super().__init__(view)

    def cycle(self):
        self._index = (self._index + 1) % len(BORDERS)
        thickness, outline = BORDERS[self._index]
        self.thickness.set(thickness)  # a live border_width binding
        self.outline.set(outline)  # a live border_color binding

        # M48: the imperative, non-bindable resize path -- `width`/
        # `height` aren't `Animated<T>`, so this goes through `Node.
        # set_layout` directly, not a `{{ }}` binding.
        width, height = SIZES[self._index]
        view.node("box").set_layout(width=width, height=height)

        # M59: per-side padding and align_items/justify_content, applied
        # to `root` (the real flex container) -- both only have a real
        # visible effect on a node with children, unlike `box` itself.
        align_items, justify_content = ALIGNMENTS[self._index]
        view.node("root").set_layout(
            padding_top=PADDING_TOPS[self._index],
            align_items=align_items,
            justify_content=justify_content,
        )


vm = LiveStyleViewModel(view)  # must not raise -- the real M48 wiring

button = view.node("cycle_button")
for expected_index in range(1, len(BORDERS)):
    view.click(button)
    assert vm._index == expected_index
    assert vm.thickness.get() == BORDERS[expected_index][0]
    assert vm.outline.get() == BORDERS[expected_index][1]

print(
    f"cycled through {len(BORDERS)} border/size combinations: "
    f"final thickness={vm.thickness.get()!r}, outline={vm.outline.get()!r}"
)

window = Window.from_view(view, width=300, height=260, title="Live Style")
app = App()
app.add_window(window)
app.run(max_frames=60)
print("live_style.py: exited cleanly after a real 60-frame render loop")
