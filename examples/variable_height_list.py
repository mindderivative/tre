#!/usr/bin/env python3
"""A virtual list whose rows differ in height: `size_hint(index)` gives each
row its own height in place of `item_extent`. As in `scrollable_list.py`,
only the rows in view are built.

Headless-CI-safe: `App.run()` renders `max_frames=60` and returns quietly
without a display or GPU.
"""

from tre import App, Window

window = Window(width=200, height=200, title="tre -- variable-height list")

ROW_COUNT = 500
ROW_HEIGHTS = [16.0, 32.0, 48.0, 24.0, 40.0]  # a repeating, non-uniform pattern


def size_hint(idx):
    return ROW_HEIGHTS[idx % len(ROW_HEIGHTS)]


def materialize(idx):
    shade = (idx * 37) % 200
    return window.create("box", fill=(shade, shade, 0xFF, 0xFF))


rows = window.create(
    "virtual_list", item_count=ROW_COUNT, size_hint=size_hint, materialize=materialize,
    width=168, height=168,
)
window.root.add_child(rows)
window.simulate("wheel", node=rows, delta_y=150.0)  # scroll down into the non-uniform rows

app = App()
app.add_window(window)
app.run(max_frames=60)
print("variable_height_list.py: exited cleanly after 60 frames")
