#!/usr/bin/env python3
"""M5 Phase 3's real `NodeKind::Canvas` (§11.10, §11.11): a real, live
window with a small custom-drawn scene -- two circular "nodes" joined by
a stroked "edge," the same shape M5 Phase 4's own node-graph validation
example builds on. Proves the real end-to-end path: a Python `draw`
callback populates a `CanvasContext`, `Window.redraw_canvas` resolves it
into real `CanvasState`, and `engine-render` paints it through the same
pipeline every other `NodeKind` uses.

What this script proves automatically (headless-CI-safe, no human
needed): the `draw` callback runs, `CanvasContext`'s drawing/hit-test
methods accept real arguments, and the whole layout renders through the
real pipeline for real frames, exiting cleanly. The definitive pixel-
level proof that the drawn content lands at the right on-screen
position is `crates/engine-render/tests/canvas_paint.rs`, not this
script -- matching this workspace's own established split (e.g.
`resizable_panes.py`/`splitter_drag_dispatch.rs`).
"""

from tre import App, Window

window = Window(width=240, height=140, title="tre v2 -- canvas")


def draw(ctx):
    # The "edge" first, so the two node circles paint on top of it.
    ctx.stroke_path(points=[(30, 30), (150, 90)], color=(0x63, 0x50, 0xA4, 0xFF), width=3.0)

    ctx.fill_circle(cx=30, cy=30, radius=16, color=(0xFF, 0xA5, 0x00, 0xFF))
    ctx.fill_circle(cx=150, cy=90, radius=16, color=(0x03, 0xDA, 0xC6, 0xFF))

    # A precise custom hit-test on the second node's own circle (§11.10's
    # "a specific plotted data point" example) -- clicking anywhere in
    # the canvas's wider bounding box, other than this circle, misses.
    ctx.set_hit_test_circle(cx=150, cy=90, radius=16)


canvas = window.add_canvas(width=200, height=120, draw=draw)
window.redraw_canvas(canvas)

app = App()
app.add_window(window)
app.run(max_frames=180)
print("canvas.py: exited cleanly after 180 frames")
