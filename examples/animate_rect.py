#!/usr/bin/env python3
"""§14 build-order step 6: "drive step 2's animation from a .py script."

Real proof, not a smoke test alone: creates a node from Python
(`Window.add_rect`), animates its opacity from Python (`Node.animate`,
§8's own "one property setter"), and runs the real render loop --
`engine-py`'s `App.run()` opens an actual OS window and renders actual
frames via the same `engine-render` pipeline every other step's Rust
demo already uses.

§14 step 14 (§11.1) split `Window` back out of `App` -- node creation
now happens on a `Window`, `App` just collects and runs one or more of
them (see `two_windows.py` for the real multi-window proof).

`max_frames=60` matches this workspace's own headless-CI-safe
convention (TRE v1 finding #261): exits cleanly after 60 frames instead
of waiting for a human to close the window, and `App.run()` itself
exits 0 (doesn't raise) when no display/GPU is reachable, so this
script is safe to run unattended.
"""

from tre import App, Window

window = Window(width=420, height=140)

swatch = window.add_rect(background=(0x67, 0x50, 0xA4, 0xFF), width=100, height=100)
swatch.animate("opacity", 0.3, duration_ms=800)
swatch.animate("corner_radius", 24.0, duration_ms=800)

second = window.add_rect(background=(0x03, 0xDA, 0xC6, 0xFF), width=100, height=100)
second.animate("background", (0x67, 0x50, 0xA4, 0xFF), duration_ms=800)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("animate_rect.py: exited cleanly after 60 frames")
