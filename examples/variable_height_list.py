#!/usr/bin/env python3
"""A variable-height virtualized list (§11.7, M12): `size_hint(index)`
gives each row its own height in place of `item_extent`'s single value
-- here a repeating five-step pattern of 500 rows. As with
`scrollable_list.py`, `tre` builds only the rows its viewport shows.

Headless-CI-safe: it renders `max_frames=60` and exits. The proof that
each row's position and height really differ is
`crates/engine-core/src/tree.rs`'s M12 tests.
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
window.scroll(rows, 150.0)  # scroll down into the non-uniform rows

app = App()
app.add_window(window)
app.run(max_frames=60)
print("variable_height_list.py: exited cleanly after 60 frames")
