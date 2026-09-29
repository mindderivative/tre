#!/usr/bin/env python3
"""A virtual list of 1,000 rows, of which `tre` builds only the few its
200px viewport shows: `materialize(index)` returns a node for each row as
it scrolls into view, and rows scrolled away are released. A wheel
scrolls it; the scrollbar thumb paints on its right edge.

The script counts the rows actually built, then opens the window.
Headless-CI-safe: `App.run()` renders `max_frames=60` and returns quietly
without a display. See docs/guide/nodes-and-layout.md.
"""

from tre import App, Window

window = Window(width=200, height=200, title="tre -- scrollable list")

ROW_COUNT = 1_000
ROW_HEIGHT = 24.0


built = []


def materialize(idx):
    built.append(idx)
    shade = (idx * 37) % 200
    return window.create("box", fill=(shade, shade, 0xFF, 0xFF))


rows = window.create(
    "virtual_list", item_count=ROW_COUNT, item_extent=ROW_HEIGHT, materialize=materialize,
    width=168, height=168,
)
window.root.add_child(rows)
window.simulate("wheel", node=rows, delta_y=ROW_HEIGHT * 3)  # scroll down three rows
assert len(rows.children()) < 12, "only the visible rows exist"
assert max(built) < 12 and rows.children()[0].get("layout_y") < rows.get("layout_y")
print(f"scrollable_list.py: built {len(built)} of {ROW_COUNT} rows")

app = App()
app.add_window(window)
app.run(max_frames=60)
print("scrollable_list.py: exited cleanly after 60 frames")
