#!/usr/bin/env python3
"""Path morphing (M95): a square `path` morphs into a triangle over
800ms by animating its `data` -- any two closed paths morph, resampled
by length. It replaces the MD3 shape library `tre` 0.3.5 removed: a
framework keeps its own shapes as path data.

Headless-CI-safe: it renders `max_frames=60` and exits. The pixel proof
of a mid-morph frame is `crates/engine-render/tests/m95_paint.rs`.
"""

from tre import App, Window

SQUARE = "M0,0 L100,0 L100,100 L0,100 Z"
TRIANGLE = "M50,0 L100,100 L0,100 Z"

window = Window(width=160, height=160, title="tre -- path morph")

shape = window.create(
    "path", data=SQUARE, view_box=(0, 0, 100, 100), width=100, height=100,
    fill=(0x67, 0x50, 0xA4, 0xFF),
)
window.root.add_child(shape)
shape.animate("data", TRIANGLE, duration_ms=800)

app = App()
app.add_window(window)
app.run(max_frames=60)
print("path_morph.py: exited cleanly after 60 frames")
