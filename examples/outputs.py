#!/usr/bin/env python3
"""Screens (0.5.8): list the outputs and make one bar per screen, following hot-plug.

`App.outputs()` reads the compositor's screens now, before `run()`; `app.on("output_added" ...)`
follows them being plugged in and out. A `LayerWindow(output=out)` is placed on that screen. On a
session with no layer shell (X11, GNOME) there are no output events and `LayerWindow` raises
`UnsupportedPlatform`; this example then only lists what it can. Headless it drives the handlers
with `app.simulate`, like every example here. Pass `--watch` to keep the bars up.

See docs/reference/outputs.md and docs/guide/bars-and-panels.md.
"""

import sys

import tre
from tre import App, LayerWindow, UnsupportedPlatform

app = App()
bars = {}


def make_bar(out):
    bar = LayerWindow("example.screen-bar", layer="top", anchor=("top", "left", "right"),
                      size=(0, 28), exclusive_zone=28, output=out)
    bar.root.set(fill=(0x1D, 0x1B, 0x20, 0xF0), align_items="center", padding_left=12)
    label = bar.create("text", text=f"{out.name}  {out.width}x{out.height}  x{out.scale_factor:g}",
                       font_size=13, fill=(255, 255, 255, 255),
                       width="fit-content", height="fit-content")
    bar.root.add_child(label)
    return bar


def add_bar(out):
    if out.name not in bars:
        bars[out.name] = make_bar(out)
        app.add_window(bars[out.name])


app.on("output_added", add_bar)
app.on("output_removed", lambda out: bars.pop(out.name, None))  # its bar closes itself

found = app.outputs()
print(f"outputs.py: {len(found)} output(s):", ", ".join(o.name for o in found) or "none")

try:
    for out in found:
        add_bar(out)
except UnsupportedPlatform as why:
    print(f"outputs.py: no layer shell here ({why}); listed the screens only")
    found = []

# Headless: the handlers, driven the way the compositor would (on a scratch App, so no bar
# is made for a screen that does not exist).
scratch = App()
events = []
scratch.on("output_added", lambda out: events.append(("added", out.name, out.scale_factor)))
scratch.on("output_removed", lambda out: events.append(("removed", out.name)))
scratch.simulate("output_added", name="TEST-1", width=2560, height=1440, scale_factor=1.5)
scratch.simulate("output_removed", name="TEST-1")
assert events == [("added", "TEST-1", 1.5), ("removed", "TEST-1")], events

if bars and tre.supports("layer_shell"):
    app.run(max_frames=None if "--watch" in sys.argv else 90)
    print("outputs.py: exited cleanly")
